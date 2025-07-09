use crate::{LaunchOpts, LauncherConfig};
use config::{Config, FileFormat};
use core::option::Option;
use core::option::Option::{None, Some};
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};
use zip::ZipArchive;

/// The fallback locations to look for a Java installation, drawn from common install locations.
const JVM_LOC_QUERIES: &'static [&str] = &[
    "$USER$/.gradle/jdks",
    "C:/Program Files/Java",
    "C:/Program Files/AdoptOpenJDK",
    "C:/Program Files/JavaSoft/Java Runtime Environment",
    "C:/Program Files/JavaSoft/Java Development Kit",
    "C:/Program Files/JavaSoft/JRE",
    "C:/Program Files/JavaSoft/JDK",
    "C:/Program Files/Eclipse Foundation/JDK",
    "C:/Program Files/Eclipse Adoptium/JDK",
    "C:/Program Files/Eclipse Adoptium/JRE",
    "C:/Program Files/Azul Systems/Zulu",
];

#[cfg(target_os = "windows")]
/// Name of the dynamic Java library file.
const DYN_JAVA_LIB: &str = "jvm.dll";
#[cfg(target_os = "macos")]
/// Name of the dynamic Java library file.
const DYN_JAVA_LIB: &str = "libjvm.dylib";
#[cfg(target_os = "linux")]
/// Name of the dynamic Java library file.
const DYN_JAVA_LIB: &str = "libjvm.so";

/// Try and find the main class from the given classpath (without resolving it)
/// and return its required Java version.
pub fn get_java_version_of_main(launch_cfg: &LauncherConfig) -> Option<u16> {
    // Not enough info provided
    if !launch_cfg.validate() {
        return None
    }

    // Go over the classpath
    return if let Some(classpath) = &launch_cfg.classpath {
        let jars: Vec<&str> = classpath.split(";").collect();
        for jar_str in jars {
            // Open the jar
            let jar_path = Path::new(jar_str);

            if let Ok(jar) = File::open(jar_path) {
                if jar_path.is_dir() {
                    if let Ok(mut zip_jar) = ZipArchive::new(jar) {
                        // Find main class
                        if let Ok(class) = zip_jar.by_name((launch_cfg.main_class
                            .as_ref().unwrap().to_string()
                            .replace(".", "/") + ".class").as_str()) {
                            // Found main class, get the version
                            return read_class_version_to_java(class)
                        }
                    }
                } else {
                    if let Some(class) = find_file_with_path(jar_path, (launch_cfg.main_class
                        .as_ref().unwrap().to_string()
                        .replace(".", "/") + ".class").as_str()) {
                        if let Ok(class_file) = File::open(class) {
                            // Found main class, get the version
                            return read_class_version_to_java(class_file)
                        }
                    }
                }
            }
        }
        None
    } else {
        None
    }
}

/// Get all valid paths to [`DYN_JAVA_LIB`],
/// skipping hidden paths.<br>
/// If [`Config::jvm_path`] is `None`, search the current working directory.
/// If `Some`, search the given path.<br>
/// If [`Config::allows_java_location_lookup`] is `true`,
/// will search [`JVM_LOC_QUERIES`] for a valid path.<br>
/// Also checks Java version for compatibility, find at most 3 JVMs to attempt.
pub fn get_jvm_paths(launch_opts: &LaunchOpts) -> Vec<Box<dyn FnOnce(&LaunchOpts) -> Option<PathBuf>>> {
    let mut jvm_paths: Vec<Box<dyn FnOnce(&LaunchOpts) -> Option<PathBuf>>> = Vec::new();

    match &launch_opts.config.jvm_path {
        // Search current directory
        None => {
            jvm_paths.push(Box::new(|opts: &LaunchOpts| {
                let min_java_ver = opts.config.min_java.unwrap_or(0) as i32;
                if let Ok(c_dir) = env::current_dir() {
                    let p = valid_path(find_file(c_dir.to_str().unwrap_or(""), DYN_JAVA_LIB));
                    if let Some(valid_path) = p {
                        if let Some(compatible) = compatible_java_version(&valid_path, min_java_ver) {
                            if compatible {
                                use dunce::canonicalize;
                                if let Ok(resolved_path) = canonicalize(&*valid_path) {
                                    return Some(resolved_path);
                                }
                            }
                        }
                    }
                }
                return None;
            }));
        }
        // Search specified directory
        Some(_) => {
            jvm_paths.push(Box::new(|opts: &LaunchOpts| {
                let min_java_ver = (&opts.config.min_java).unwrap_or(0) as i32;
                if let Some(path) = &opts.config.jvm_path {
                    let p = valid_path(find_file(path, DYN_JAVA_LIB));
                    if let Some(valid_path) = p {
                        if let Some(compatible) = compatible_java_version(&valid_path, min_java_ver) {
                            if compatible {
                                use dunce::canonicalize;
                                if let Ok(resolved_path) = canonicalize(&*valid_path) {
                                    return Some(resolved_path);
                                }
                            }
                        } else if min_java_ver == 0 {
                            return Some(valid_path);
                        }
                    }
                }
                return None;
            }));
        }
    }

    // Check system Java install
    if launch_opts.config.allows_system_java && !jvm_paths.len() > 4 {
        jvm_paths.push(Box::new(|opts: &LaunchOpts| {
            match &env::var("JAVA_HOME") {
                Ok(path) if !path.is_empty() => {
                    let pb = PathBuf::from(path);
                    let min_java_ver = opts.config.min_java.unwrap_or(0) as i32;
                    if let Some(compatible) = compatible_java_version(&pb, min_java_ver) {
                        if compatible {
                            use dunce::canonicalize;
                            if let Ok(resolved_path) = canonicalize(&*pb) {
                                return Some(resolved_path);
                            }
                        }
                    }
                }
                _ => {
                }
            }
            return None;
        }));
    }

    // Search fallback locations
    if launch_opts.config.allows_java_location_lookup && !jvm_paths.len() > 4 {
        // Search current directory if we don't have a path
        jvm_paths.push(Box::new(|opts: &LaunchOpts| {
            if let Ok(c_dir) = env::current_dir() {
                let p = valid_path(find_file(c_dir.to_str().unwrap_or(""), DYN_JAVA_LIB));
                if let Some(valid_path) = p {
                    let min_java_ver = opts.config.min_java.unwrap_or(0) as i32;
                    if let Some(compatible) = compatible_java_version(&valid_path, min_java_ver) {
                        if compatible {
                            use dunce::canonicalize;
                            if let Ok(resolved_path) = canonicalize(&*valid_path) {
                                return Some(resolved_path);
                            }
                        }
                    }
                }
            }
            return None;
        }));

        // Search common install locations
        for loc in JVM_LOC_QUERIES.iter() {
            jvm_paths.push(Box::new(|opts: &LaunchOpts| {
                let p = valid_path(find_file(process_path(loc).as_str(), DYN_JAVA_LIB));
                if let Some(valid_path) = p {
                    let min_java_ver = opts.config.min_java.unwrap_or(0) as i32;
                    if let Some(compatible) = compatible_java_version(&valid_path, min_java_ver) {
                        if compatible {
                            use dunce::canonicalize;
                            if let Ok(resolved_path) = canonicalize(&*valid_path) {
                                return Some(resolved_path);
                            }
                        }
                    }
                }
                return None;
            }));

            if jvm_paths.len() > 3 { break }
        }
    }

    jvm_paths
}

/// This checks the path of the Java dynamic library for a `release` file,
/// reading the first integer of the `.` separated value of `JAVA_VERSION` as the Java version,
/// returns `Some(found_ver >= req_ver)` or `None` if the `release` could not be found,
/// or another error occurs.
fn compatible_java_version(jvm_path: &PathBuf, req_ver: i32) -> Option<bool> {
    // First we go up 3 levels from jvm.dll path to get runtime info
    let mut java_folder = jvm_path.to_path_buf();
    for _ in 0..3 {
        if let Some(r) = java_folder.parent() {
            java_folder = r.to_path_buf();
        }
    }

    // Try and get the Java version of the installation
    if let Some(release_path) = valid_path(find_file(java_folder.to_str()?, "release")) {
        let release_info = Config::builder()
            .add_source(config::File::from(release_path).format(FileFormat::Ini))
            .build().ok()?;
        let ver_str = release_info.get_string("JAVA_VERSION").ok()?;
        let parts: Vec<&str> = ver_str.split(".").collect();
        let ver = parts.first()?.parse::<i32>().unwrap();

        return Some(ver >= req_ver);
    }

    return None;
}

/// Replace tokens with their real values
fn process_path(path: &str) -> String {
    let user_path = dirs::home_dir().unwrap_or_default();
    let user = user_path.to_str().unwrap_or("");
    path.replace("$USER$", user)
}

/// Checks if the path points to an existing file
fn valid_path(path: Option<PathBuf>) -> Option<PathBuf> {
    match path {
        None => { None }
        Some(p) => {
            if p.exists() { Some(p) } else { None }
        }
    }
}

/// Locates a file in a given path at max depth 5
/// Skips hidden files
fn find_file_with_path<P: AsRef<Path>>(root: P, file: &str) -> Option<PathBuf> {
    return find_file(root.as_ref().to_str()?, file)
}

/// Locates a file in a given path at max depth 5
/// Skips hidden files
fn find_file(root: &str, file: &str) -> Option<PathBuf> {
    let walker = WalkDir::new(root)
        .max_depth(5)
        .into_iter();
    let mut path = Path::new(root).to_path_buf();

    if path.ends_with(file) {
        return Some(path);
    }

    let mut has_path: bool = false;
    for entry in walker.filter_entry(|e| !is_hidden(e)) {
        if let Ok(e) = entry {
            match e.file_name().to_str() {
                None => {}
                Some(name) => {
                    if name == file {
                        path = e.into_path();
                        has_path = true;
                        // Don't break here in case of multiple installs in one folder
                    }
                }
            }
        }
    }
    if has_path { Some(path) } else { None }
}

/// Used to skip hidden files
fn is_hidden(entry: &DirEntry) -> bool {
    entry.file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}

/// Reads in the first few bytes of a file to determine if it is a class file,
/// and if so what Java version it was compiled for.<br>
/// Returns the Java version a class needs.<br>
/// See <https://docs.oracle.com/javase/specs/jvms/se17/html/jvms-4.html>
fn read_class_version_to_java<R: Read>(mut reader: R) -> Option<u16> {
    let mut buffer: [u8; 8] = [0; 8];

    // Read file into buffer
    reader.read_exact(&mut *&mut buffer).ok()?;

    // Ensure bytes are Big Endian per the JVM spec.
    buffer = buffer.map(|b| b.to_be());

    // Get magic byte
    let num_bytes: [u8; 4] = (buffer[0..4]).try_into().ok()?;
    let magic_number: u32 = u32::from_be_bytes(num_bytes);

    // Is Java class?
    if magic_number == 0xCAFEBABE {
        /*let minor_version =
            u16::from_be_bytes((buffer[4..6]).try_into().ok().unwrap_or_default());*/
        let major_version =
            u16::from_be_bytes((buffer[6..8]).try_into().ok().unwrap_or_default());

        // If smaller than 45, it likely isn't a Java class
        if major_version >= 45 {
            // Convert to Java major version
            return Some(major_version - 44);
        } else {
            None
        }
    } else {
        None
    }
}
