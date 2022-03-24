use std::path::{Path, PathBuf};

use config::{Config, FileFormat};
use jni::{InitArgs, InitArgsBuilder, JavaVM, JNIVersion, JvmError};
use jni::objects::JValue;
use jni::sys::{jint, JNIInvokeInterface_};
use walkdir::{DirEntry, WalkDir};

use crate::launch_config::LauncherConfig;
use crate::message;

/// The fallback locations to look for a Java installation, drawn from common install locations.
const JVM_LOC_QUERIES: &'static [&str] = &["$USER$/.gradle/jdks"];//todo expand

/// Name of the dynamic Java library file.
const DYN_JAVA_LIB: &str = "jvm.dll";

/// The launcher options, such as JVM args and where the JVM is located.
pub struct LaunchOpts {
    pub config: LauncherConfig,
    pub jvm_opts: Vec<String>,
    pub program_opts: Vec<String>,
}

/// Create the JVM, attach to it, and run the `main` method of the given `launch_opts`.<br>
/// Blocks until the JVM has shut down.
pub fn create_and_run_jvm(launch_opts: &LaunchOpts) {
    // Not enough information provided in the launcher config
    if !launch_opts.config.validate() {
        message("Invalid launcher config.\n\
        Please contact the developers.");
        return;
    }

    // Try and find a valid JVM outside of `JAVA_HOME`
    let mut jvm_path: Option<PathBuf> = None;
    let mut had_jvm_path = false; // Used for error message output
    if let Some(paths) = get_jvm_paths(launch_opts) {
        for p in paths {
            jvm_path = Some(p);
            had_jvm_path = true;
            break;
        }
    } else {
        message("Failed to find a valid Java installation.\n\
        Please contact the developers or install a valid version of Java");
        return;
    }

    // The launch attempt
    if let Some(jvm) = try_launch_jvm(jvm_path, launch_opts) {
        // Attach the current thread to call into Java
        // This method returns the guard that will detach the current thread when dropped,
        // also freeing any local references created in it
        let maybe_env = jvm.attach_current_thread_as_daemon();

        // Starting the app
        match maybe_env {
            Ok(env) => {
                // Convert program args for forwarding
                let opts: Vec<JValue> = launch_opts.program_opts.iter()
                    .map(|s| env.new_string(s)) // Convert to JString (maybe)
                    .filter(|m| m.is_ok()).map(|m| m.unwrap())// Remove invalid JStrings
                    .map(|s| JValue::Object(*s)).collect(); // Convert to something usable

                // Ensure correct format of main class
                let main_class = launch_opts.config.main_class.as_ref().unwrap().replace(".", "/");

                // Call main method
                let v = env.call_static_method(main_class, "main", "([Ljava/lang/String;)V", &opts[..]);

                // Launch failed
                if let Err(e) = v {
                    println!("{:?}", e);
                    message("Failed to start the app, the classname was invalid or \
                    not on the classpath, or the main method could not be found.\n\
                    Please contact the developers.");
                    return;
                }

                // This hangs and waits for all Java threads to close before shutting down
                // Also keeps the JVM open, without this we immediately shut down
                if let Ok(j) = env.get_java_vm() { // This gets around ownership issues
                    close_jvm(j);
                }
            }
            Err(e) => {
                println!("{:?}", e);
                message("Java successfully started, but failed to attach to it and therefore cannot proceed.\n\
                Please contact the developers.")
            }
        }

        // Ensure the JVM is closed
        close_jvm(jvm)
    } else {
        // Error messages
        if !had_jvm_path {
            // String formatting? What's that?
            let version = launch_opts.config.min_java.unwrap_or(0);
            let mut inst = "any Java.".to_owned();
            if version > 0 {
                let mut x = "Java ".to_owned();
                x.push_str(version.to_string().as_str());
                x.push_str(" or newer.");
                inst = x.clone();
            }
            message(&("A missing or older Java installation was found.\n\
                        Please install ".to_owned() + inst.as_str()))
        } else {
            message("Java failed to start. Please check the launch options,\n\
                    an invalid option was likely used and could not be automatically recovered.")
        }
    }
}

/// Create the JVM if possible
fn try_launch_jvm(jvm_path: Option<PathBuf>, launch_opts: &LaunchOpts) -> Option<JavaVM> {
    return if let Some(jvm_path) = jvm_path {
        // This is needed for the lookup passed to with_libjvm
        let path_getter = || {
            Ok(jvm_path.as_path())
        };

        // Create JVM arguments
        let args = make_jvm_args(launch_opts);
        if args.is_err() {
            message("Failed to create JVM arguments.\n\
            Please contact the developers or undo any changes to the configuration.");
            return None;
        }

        // Create a new VM
        let maybe_jvm = JavaVM::with_libjvm(args.unwrap(), path_getter);
        match maybe_jvm {
            Ok(vm) => { Some(vm) }
            Err(e) => {
                println!("{:?}", e);
                None
            }
        }
    } else {
        None
    };
}

/// Calls `DestroyJavaVM` of JNI - it blocks until all Java threads are closed <br>
/// See <https://docs.oracle.com/en/java/javase/17/docs/specs/jni/invocation.html#unloading-the-vm>
fn close_jvm(jvm: JavaVM) {
    unsafe {
        let f: Option<unsafe extern "system" fn(*mut *const JNIInvokeInterface_) -> jint> =
            (*(*jvm.get_java_vm_pointer())).DestroyJavaVM;
        if let Some(func) = f {
            func(jvm.get_java_vm_pointer());
        }
    }
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

/// Convert string args to the proper format and add to the launch args.<br>
/// Sets the JVM to ignore unrecognized `-X` args and to expect calls to JNI 2
fn make_jvm_args(launch_opts: &LaunchOpts) -> Result<InitArgs, JvmError> {
    let mut jvm_args = InitArgsBuilder::new()
        .version(JNIVersion::V2)// No touchy or things breaky
        .ignore_unrecognized(true);

    for jvm_opt in &launch_opts.jvm_opts {
        jvm_args = jvm_args.option(jvm_opt.as_str());
    }

    jvm_args.build()
}

/// Get all valid paths to [`DYN_JAVA_LIB`],
/// skipping hidden paths.<br>
/// If [`Config::jvm_path`] is `None`, search the current working directory.
/// If `Some`, search the given path.<br>
/// If [`Config::allows_java_location_lookup`] is `true`,
/// will search [`JVM_LOC_QUERIES`] for a valid path.<br>
/// Also checks Java version for compatibility.
fn get_jvm_paths(launch_opts: &LaunchOpts) -> Option<Vec<PathBuf>> {
    let mut jvm_paths: Vec<PathBuf> = vec![];
    let min_java_ver = launch_opts.config.min_java.unwrap_or(0) as i32;
    let mut done: bool = false;

    match &launch_opts.config.jvm_path {
        // Search current directory
        None => {
            if let Ok(c_dir) = std::env::current_dir() {
                let p = valid_path(find_file(c_dir.to_str().unwrap_or(""), DYN_JAVA_LIB));
                if let Some(valid_path) = p {
                    if let Some(compatible) = compatible_java_version(&valid_path, min_java_ver) {
                        if compatible {
                            done = true;
                            jvm_paths.push(valid_path);
                        }
                    }
                }
            }
        }
        // Search specified directory
        Some(path_str) => {
            let p = valid_path(find_file(path_str, DYN_JAVA_LIB));
            if let Some(valid_path) = p {
                if let Some(compatible) = compatible_java_version(&valid_path, min_java_ver) {
                    if compatible {
                        done = true;
                        jvm_paths.push(valid_path);
                    }
                }
            }
        }
    }

    // Check system Java install
    if launch_opts.config.allows_system_java && !done {
        if let Ok(path) = java_locator::locate_jvm_dyn_library() {
            let pb = PathBuf::from(path);
            if let Some(compatible) = compatible_java_version(&pb, min_java_ver) {
                if compatible {
                    done = true;
                    jvm_paths.push(pb);
                }
            }
        }
    }

    // Search fallback locations
    if launch_opts.config.allows_java_location_lookup && !done {
        for loc in JVM_LOC_QUERIES.iter() {
            let p = valid_path(find_file(process_path(loc).as_str(), DYN_JAVA_LIB));
            if let Some(valid_path) = p {
                if let Some(compatible) = compatible_java_version(&valid_path, min_java_ver) {
                    if compatible {
                        done = true;
                        jvm_paths.push(valid_path);
                    }
                }
            }
            if done { break }
        }
    }

    Some(jvm_paths)
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