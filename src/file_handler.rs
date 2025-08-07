use crate::LaunchOpts;
use core::option::Option;
use core::option::Option::{None, Some};
use std::env;
use std::fs::File;
use std::io::Read;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};
use zip::ZipArchive;
use dunce::canonicalize;

/// The fallback locations to look for a Java installation, drawn from common install locations.
const JVM_LOC_QUERIES: &'static [&str] = &[
    "$USER$/.gradle/jdks",
    #[cfg(target_os = "windows")]
    "C:/Program Files/Java",
    #[cfg(target_os = "windows")]
    "C:/Program Files/AdoptOpenJDK",
    #[cfg(target_os = "windows")]
    "C:/Program Files/JavaSoft/Java Runtime Environment",
    #[cfg(target_os = "windows")]
    "C:/Program Files/JavaSoft/Java Development Kit",
    #[cfg(target_os = "windows")]
    "C:/Program Files/JavaSoft/JRE",
    #[cfg(target_os = "windows")]
    "C:/Program Files/JavaSoft/JDK",
    #[cfg(target_os = "windows")]
    "C:/Program Files/Eclipse Foundation/JDK",
    #[cfg(target_os = "windows")]
    "C:/Program Files/Eclipse Adoptium/JDK",
    #[cfg(target_os = "windows")]
    "C:/Program Files/Eclipse Adoptium/JRE",
    #[cfg(target_os = "windows")]
    "C:/Program Files/Azul Systems/Zulu",
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    "/usr/lib/jvm",
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    "/usr/java",
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    "/usr/local/java",
    #[cfg(target_os = "macos")]
    "/Library/Java/JavaVirtualMachines",
    #[cfg(target_os = "macos")]
    "/System/Library/Java/JavaVirtualMachines",
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
pub fn get_java_version_of_main(
    main_class: &Option<String>,
    classpath: &Vec<String>,
) -> Option<u16> {
    if let Some(main_class) = main_class {
        // Convert main class to path format
        let class_path = main_class.replace(".", "/") + ".class";

        // Search through classpath entries
        for jar_str in classpath {
            let jar_path = Path::new(jar_str);

            if !jar_path.exists() {
                continue;
            }

            if jar_path.is_dir() {
                // Search in directory
                if let Some(class_file_path) = find_file_with_path(jar_path, &class_path) {
                    if let Ok(class_file) = File::open(class_file_path) {
                        if let Some(version) = read_class_version_to_java(class_file) {
                            return Some(version);
                        }
                    }
                }
            } else {
                // Try to open as JAR
                if let Ok(jar) = File::open(jar_path) {
                    if let Ok(mut zip_jar) = ZipArchive::new(jar) {
                        if let Ok(class_file) = zip_jar.by_name(&class_path) {
                            if let Some(version) = read_class_version_to_java(class_file) {
                                return Some(version);
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

/// Get all valid paths to [`DYN_JAVA_LIB`],
/// skipping hidden paths.<br>
/// If [`Config::jvm_path`] is `None`, search the current working directory.
/// If `Some`, search the given path.<br>
/// If [`Config::allows_java_location_lookup`] is `true`,
/// will search [`JVM_LOC_QUERIES`] for a valid path.<br>
/// Also checks Java version for compatibility, find at most 3 JVMs to attempt.
pub fn get_jvm_paths(
    launch_opts: &LaunchOpts,
) -> Vec<Box<dyn FnOnce(&LaunchOpts) -> Option<PathBuf>>> {
    let mut jvm_paths: Vec<Box<dyn FnOnce(&LaunchOpts) -> Option<PathBuf>>> = Vec::new();

    match &launch_opts.config.runtime {
        // Search current directory
        None => {
            jvm_paths.push(Box::new(|opts: &LaunchOpts| {
                let min_java_ver = opts.config.min_java.unwrap_or(0) as i32;
                if let Ok(c_dir) = env::current_dir() {
                    let p = valid_path(find_file(c_dir.to_str().unwrap_or(""), DYN_JAVA_LIB));
                    if let Some(valid_path) = p {
                        if let Some(compatible) = compatible_java_version(&valid_path, min_java_ver) {
                            if compatible {
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
                if let Some(path) = &opts.config.runtime {
                    let p = valid_path(find_file(path.to_str().unwrap_or(""), DYN_JAVA_LIB));
                    if let Some(valid_path) = p {
                        if let Some(compatible) = compatible_java_version(&valid_path, min_java_ver) {
                            if compatible {
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
    if jvm_paths.len() < 4 {
        jvm_paths.push(Box::new(|opts: &LaunchOpts| {
            let min_java_ver = opts.config.min_java.unwrap_or(0) as i32;

            // Check JAVA_HOME environment variable
            if let Ok(path) = env::var("JAVA_HOME") {
                if !path.is_empty() {
                    let pb = PathBuf::from(&path);
                    // Look for the JVM library
                    if let Some(valid_path) = valid_path(find_file(&path, DYN_JAVA_LIB)) {
                        if let Some(compatible) = compatible_java_version(&valid_path, min_java_ver) {
                            if compatible {
                                if let Ok(resolved_path) = canonicalize(&valid_path) {
                                    return Some(resolved_path);
                                }
                            }
                        }
                    }
                }
            }
            None
        }));
    }

    // Check JAVA_HOME
    if jvm_paths.len() < 4 {
        jvm_paths.push(Box::new(|opts: &LaunchOpts| {
            match &env::var("JAVA_HOME") {
                Ok(path) if !path.is_empty() => {
                    let pb = PathBuf::from(path);
                    let min_java_ver = opts.config.min_java.unwrap_or(0) as i32;
                    if let Some(compatible) = compatible_java_version(&pb, min_java_ver) {
                        if compatible {
                            if let Ok(resolved_path) = canonicalize(&*pb) {
                                return Some(resolved_path);
                            }
                        }
                    }
                }
                _ => {}
            }
            return None;
        }));
    }

    // Search fallback locations
    if jvm_paths.len() < 4 {
        // Search common install locations
        for loc in JVM_LOC_QUERIES.iter() {
            jvm_paths.push(Box::new(|opts: &LaunchOpts| {
                let p = valid_path(find_file(process_path(loc).as_str(), DYN_JAVA_LIB));
                if let Some(valid_path) = p {
                    let min_java_ver = opts.config.min_java.unwrap_or(0) as i32;
                    if let Some(compatible) = compatible_java_version(&valid_path, min_java_ver) {
                        if compatible {
                            if let Ok(resolved_path) = canonicalize(&*valid_path) {
                                return Some(resolved_path);
                            }
                        }
                    }
                }
                return None;
            }));

            if jvm_paths.len() > 3 {
                break;
            }
        }
    }

    jvm_paths
}

/// This checks the path of the Java dynamic library for a `release` file,
/// reading the first integer of the `.` separated value of `JAVA_VERSION` as the Java version,
/// returns `Some(found_ver >= req_ver)` or `None` if the `release` could not be found,
/// or another error occurs.
fn compatible_java_version(jvm_path: &PathBuf, req_ver: i32) -> Option<bool> {
    // Get the parent directory of jvm.dll/libjvm.so
    let mut parent = jvm_path.parent()?;
    // Look for release file in parent or grandparent directory
    let mut release_path = parent.join("release");

    let mut c = 0;
    while !release_path.exists() && c < 4 {
        parent = parent.parent()?;
        release_path = parent.join("release");
        c += 1;
    }

    // Try to read the release file
    if let Ok(file) = File::open(release_path) {
        let reader = BufReader::new(file);
        for line in reader.lines().filter_map(|l| l.ok()) {
            if line.starts_with("JAVA_VERSION=") {
                // Extract version number
                if let Some(ver_str) = line.split('=').nth(1) {
                    // Remove quotes if present
                    let ver_str = ver_str.trim_matches('"');
                    // Get first number before dot
                    if let Some(ver) = ver_str.split('.').next() {
                        if let Ok(found_ver) = ver.parse::<i32>() {
                            return Some(found_ver >= req_ver);
                        }
                    }
                }
            }
        }
    }

    None
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
        None => None,
        Some(p) => {
            if p.exists() {
                Some(p)
            } else {
                None
            }
        }
    }
}

/// Locates a file in a given path at max depth 5
/// Skips hidden files
fn find_file_with_path<P: AsRef<Path>>(root: P, file: &str) -> Option<PathBuf> {
    return find_file(root.as_ref().to_str()?, file);
}

/// Locates a file in a given path at max depth 5
/// Skips hidden files
fn find_file(root: &str, file: &str) -> Option<PathBuf> {
    let walker = WalkDir::new(root).max_depth(5).into_iter();
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
    entry
        .file_name()
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
        /*let minor_version = u16::from_be_bytes((buffer[4..6]).try_into().ok().unwrap_or_default());*/
        let major_version = u16::from_be_bytes((buffer[6..8]).try_into().ok().unwrap_or_default());

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

/// The path to the `app` folder.
#[cfg(target_os = "windows")]
pub fn get_app_dir_path() -> PathBuf {
    get_app_image_root().join("app")
}

#[cfg(target_os = "windows")]
pub fn get_default_runtime_path() -> PathBuf {
    get_app_image_root().join("runtime")
}

#[cfg(target_os = "windows")]
pub fn get_app_image_root() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned() // meh-app
}

/// The path to the `app` folder.
#[cfg(target_os = "linux")]
pub fn get_app_dir_path() -> PathBuf {
    get_app_image_root().join("lib").join("app")
}

#[cfg(target_os = "linux")]
pub fn get_default_runtime_path() -> PathBuf {
    get_app_image_root().join("lib").join("runtime")
}

#[cfg(target_os = "linux")]
pub fn get_app_image_root() -> PathBuf {
    std::env::current_exe().unwrap()
        .parent().unwrap() // bin
        .parent().unwrap().to_owned() //meh-app
}

/// The path to the `app` folder.
#[cfg(target_os = "macos")]
pub fn get_app_dir_path() -> PathBuf {
    get_app_image_root().join("Contents").join("app")
}

#[cfg(target_os = "macos")]
pub fn get_default_runtime_path() -> PathBuf {
    get_app_image_root().join("Contents").join("runtime")
}

#[cfg(target_os = "macos")]
pub fn get_app_image_root() -> PathBuf {
    std::env::current_exe().unwrap()
        .parent().unwrap() // MacOs
        .parent().unwrap() // Contents
        .parent().unwrap().to_owned() // meh.app
}

pub fn get_exec_path() -> String {
    dunce::canonicalize(env::current_exe().unwrap())
        .unwrap().to_str()
        .unwrap_or("").to_string()
}
