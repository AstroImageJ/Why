use std::path::{PathBuf};

use jni::{InitArgs, InitArgsBuilder, JavaVM, JNIVersion, JvmError};
use jni::objects::JValue;
use jni::sys::{jint, JNIInvokeInterface_};
use crate::file_handler::get_jvm_paths;

use crate::launch_config::LauncherConfig;
use crate::message;

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
    let jvm_paths = get_jvm_paths(launch_opts);

    // The launch attempt
    if let Some(jvm) = try_launch_jvm(jvm_paths, launch_opts) {
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
    }
}

/// Create the JVM if possible
fn try_launch_jvm(ref jvm_paths: Option<Vec<PathBuf>>, launch_opts: &LaunchOpts) -> Option<JavaVM> {
    if let Some(jvm_paths) = jvm_paths {
        for jvm_path in jvm_paths {
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
                Ok(vm) => { return Some(vm) }
                Err(e) => {
                    println!("{:?}", e);
                    continue
                }
            }
        }
        if jvm_paths.len() > 0 {
            message("A valid Java installation was found, failed to start.\n\
                Please check the launch arguments as they may be invalid.\n\
                Please contact the developers.")
        }
    }
    None
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