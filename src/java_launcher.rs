use std::path::{Path, PathBuf};
use config::{Config, FileFormat};
use jni::{InitArgs, InitArgsBuilder, JavaVM, JNIVersion, JvmError};
use jni::objects::JValue;
use jni::sys::{jint, JNIInvokeInterface_};
use walkdir::{DirEntry, WalkDir};
use crate::launch_config::LauncherConfig;
use crate::message;

const JVM_LOC_QUERIES: Vec<String> = vec![];//todo fill

pub struct LaunchOpts {
    pub config: LauncherConfig,
    pub jvm_opts: Vec<String>,
    pub program_opts: Vec<String>,
}

pub fn create_and_run_jvm(launch_opts: &LaunchOpts) {
    if !launch_opts.config.validate() {
        message("Invalid launcher config.\n\
        Please contact the developers.");
        return;
    }

    let mut jvm_path: Option<PathBuf> = None;
    let mut had_jvm_path = false;
    if let Some(paths) = get_jvm_paths(launch_opts) {
        // Try and validate Java version -
        // if no valid version is found, and a version check failed, try using that one
        for p in paths {
            if let Some(is_compat) = compatible_java_version(&p, launch_opts.config.min_java.unwrap_or(0) as i32) {
                if is_compat {
                    println!("Found valid java!");
                    jvm_path = Some(p);
                }
            } else {
                jvm_path = Some(p);
            }
            had_jvm_path = true;
        }
    } else {
        message("Failed to find a valid Java installation.\n\
        Please contact the developers or install a valid version of Java");
        return;
    }

    if let Some(jvm_path) = jvm_path {
        // This is needed for the lookup
        let path_getter = || {
            Ok(jvm_path.as_path())
        };

        let args = make_jvm_args(launch_opts);
        if args.is_err() {
            message("Failed to create JVM arguments.\n\
            Please contact the developers or undo any changes to the configuration.");
            return;
        }

        // Create a new VM
        let maybe_jvm = JavaVM::with_libjvm(args.unwrap(), path_getter);
        let jvm = match maybe_jvm {
            Ok(vm) => {vm}
            Err(e) => {
                println!("{}", e);
                if !launch_opts.config.allows_system_java {
                    message("Failed to load Java from the searched paths.\n\
                    Please contact the developers.");
                    return;
                }
                let maybe_jvm = JavaVM::new(make_jvm_args(launch_opts).unwrap());
                match maybe_jvm {
                    Ok(vm) => {vm}
                    Err(_) => {
                        message("Failed to load Java from the searched paths and failed again \
                        when trying the installation at JAVA_HOME.\n\
                        Please contact the developers.");
                        return;
                    }
                }
            }
        };

        // Attach the current thread to call into Java — see extra options in
        // "Attaching Native Threads" section.
        //
        // This method returns the guard that will detach the current thread when dropped,
        // also freeing any local references created in it
        let maybe_env = jvm.attach_current_thread_as_daemon();
        if let Ok(env) = maybe_env {

            // Convert program args for forwarding
            let opts: Vec<JValue> = launch_opts.program_opts.iter()
                .map(|s| env.new_string(s)) // Convert to JString (maybe)
                .filter(|m| m.is_ok()).map(|m| m.unwrap())// Remove invalid JStrings
                .map(|s| JValue::Object(*s)).collect(); // Convert to something usable

            // Ensure correct format
            let main_class = launch_opts.config.main_class.as_ref().unwrap().replace(".", "/");

            // Call main method
            let v = env.call_static_method(main_class, "main", "([Ljava/lang/String;)V", &opts[..]);

            if let Err(e) = v {
                println!("{}", e);
                message("Failed to start the app, the classname was invalid or \
                not on the classpath, or the main method could not be found.\n\
                Please contact the developers.");
                return;
            }

            // This hangs and waits for all java threads to close before shutting down
            // Also keeps the JVM open, without this we immediately shut down
            if let Ok(j) = env.get_java_vm() { // This gets around ownership issues
                close_jvm(j);
            }
        } else {
            if let Err(e) = maybe_env {
                println!("{}", e);
                message("Java successfully started, but failed to attach to it and therefore cannot proceed.\n\
                Please contact the developers.")
            }
        }

        close_jvm(jvm)
    } else {
        if had_jvm_path {
            // String formatting? What's that?
            let version = launch_opts.config.min_java.unwrap_or(0);
            let mut inst = "any Java.".to_owned();
            if version > 0 {
                let mut x = "Java ".to_owned();
                x.push_str(version.to_string().as_str());
                x.push_str(" or newer.");
                inst = x.clone();
            }
            message(&("A valid Java installation was found but it was too old.\n\
            Please install ".to_owned() + inst.as_str()))
        } else {
            message("Failed to find a valid Java installation and giving up.\n\
            Please contact the developers or install a valid version of Java.")
        }
    }
}

fn convert_opts<T, const N: usize>(v: Vec<T>) -> [T; N] {
    use std::convert::TryInto;
    v.try_into()
        .unwrap_or_else(|v: Vec<T>| panic!("Expected a Vec of length {} but it was {}", N, v.len()))
}

fn close_jvm(jvm: JavaVM) {
    unsafe {
        let f : Option<unsafe extern "system" fn(*mut *const JNIInvokeInterface_) -> jint> =
            (*(*jvm.get_java_vm_pointer())).DestroyJavaVM;
        if let Some(func) = f {
            func(jvm.get_java_vm_pointer());
        }
    }
}

fn compatible_java_version(jvm_path: &PathBuf, req_ver: i32) -> Option<bool> {
    // First we go up 3 levels from jvm.dll path to get runtime info
    let mut java_folder = jvm_path.to_path_buf();
    for _ in 0..3 {
        if let Some(r) = java_folder.parent() {
            java_folder = r.to_path_buf();
        }
    }

    if let Some(release_path) = valid_path(find_file(java_folder.to_str()?, "release")) {
        let release_info = Config::builder()
            .add_source(config::File::from(release_path).format(FileFormat::Ini))
            .build().ok()?;
        let ver_str = release_info.get_string("JAVA_VERSION").ok()?;
        let parts: Vec<&str> = ver_str.split(".").collect();
        let ver = parts.first()?.parse::<i32>().unwrap();

        return Some(ver >= req_ver);
    }

    return None
}

fn make_jvm_args(launch_opts: &LaunchOpts) -> Result<InitArgs, JvmError> {
    let mut jvm_args = InitArgsBuilder::new()
        .version(JNIVersion::V2)// No touchy or things breaky
        .ignore_unrecognized(true);

    for jvm_opt in &launch_opts.jvm_opts {
        jvm_args = jvm_args.option(jvm_opt.as_str());
    }

    jvm_args.build()
}

/// Get all valid paths to JVM.dll
fn get_jvm_paths(launch_opts: &LaunchOpts) -> Option<Vec<PathBuf>> {
    let mut jvm_paths: Vec<PathBuf> = vec![];

    match &launch_opts.config.jvm_path {
        None => {
            if let Ok(c_dir) = std::env::current_dir() {
                let p = valid_path(find_file(c_dir.to_str().unwrap_or(""), "jvm.dll"));
                if let Some(valid_path) = p {
                    jvm_paths.push(valid_path)
                }
            }
        }
        Some(path_str) => {
            //todo not just windows, but not me
            let p = valid_path(find_file(path_str, "jvm.dll"));
            if let Some(valid_path) = p {
                jvm_paths.push(valid_path)
            }
        }
    }

    if launch_opts.config.allows_java_location_lookup {
        for loc in JVM_LOC_QUERIES.iter() {
            let p = valid_path(find_file(loc, "jvm.dll"));
            if let Some(valid_path) = p {
                jvm_paths.push(valid_path)
            }
        }
    }

    Some(jvm_paths)
}

fn valid_path(path: Option<PathBuf>) -> Option<PathBuf> {
    match path {
        None => {None}
        Some(p) => {
            if p.exists() {Some(p)} else { None }
        }
    }
}

/// Locates a file in a given path at max depth 4
fn find_file(root: &str, file: &str) -> Option<PathBuf> {
    let walker = WalkDir::new(root)
        .max_depth(4)
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
                    }
                }
            }
        }
    }
    if has_path {Some(path)} else { None }
}

/// Used to skip hidden files
fn is_hidden(entry: &DirEntry) -> bool {
    entry.file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}