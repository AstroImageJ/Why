use jni::{InitArgs, InitArgsBuilder, JNIVersion, JavaVM, JvmError};
use std::path::PathBuf;

use crate::file_handler::get_jvm_paths;
use crate::launch_config::LaunchConfig;
use crate::{message, DEBUG};

/// The launcher options, such as JVM args and where the JVM is located.
#[derive(Debug)]
pub struct LaunchOpts {
    pub config: LaunchConfig,
    pub jvm_opts: Vec<String>,
    pub program_opts: Vec<String>,
}

/// Create the JVM, attach to it, and run the `main` method of the given `launch_opts`.<br>
/// Blocks until the JVM has shut down.
pub fn create_and_run_jvm(launch_opts: &LaunchOpts) {
    // The launch attempt
    if let Some(jvm) = try_launch_jvm(launch_opts) {
        // Attach the current thread to call into Java
        // This method returns the guard that will detach the current thread when dropped,
        // also freeing any local references created in it
        match jvm.attach_current_thread_as_daemon() {
            Ok(mut env) => {
                if let Err(e) = call_main_method(&mut env, launch_opts) {
                    eprintln!("Failed to invoke main method: {:?}", e);
                    message(
                        "Failed to start the app. Ensure the classname is valid and available on the classpath.",
                    );
                }
            }
            Err(err) => {
                eprintln!("Failed to attach to JVM: {:?}", err);
                message("Java started successfully, but attaching failed. Please contact the developers.");
            }
        }
        close_jvm(jvm);
    } else {
        let msg = match launch_opts.config.min_java {
            Some(version) => format!(
                "A minimum of Java {} or newer is required. Please install an appropriate version.",
                version
            ),
            None => "No valid Java installations found. Please install any Java version.".to_string(),
        };
        message(&msg);
    }
}

fn call_main_method(env: &mut jni::JNIEnv, launch_opts: &LaunchOpts) -> Result<(), jni::errors::Error> {
    let opts = launch_opts
        .program_opts
        .iter()
        .map(|s| env.new_string(s))
        .collect::<Result<Vec<_>, _>>()?;

    let arg_array = env.new_object_array(
        opts.len() as i32,
        "java/lang/String",
        env.new_string("")?,
    )?;

    for (i, jstring) in opts.into_iter().enumerate() {
        env.set_object_array_element(&arg_array, i as i32, jstring)?;
    }

    let main_class = launch_opts.config.main_class.replace('.', "/");

    if DEBUG {
        println!("{:?}", main_class);
        println!("{:?}", arg_array);
    }

    env.call_static_method(main_class, "main", "([Ljava/lang/String;)V", &[(&arg_array).into()])?;
    Ok(())
}

/// Create the JVM if possible
fn try_launch_jvm(launch_opts: &LaunchOpts) -> Option<JavaVM> {
    for jvm_path_fn in get_jvm_paths(launch_opts) {
        if let Some(jvm_path) = jvm_path_fn(launch_opts) {
            // Make sure the system can find the needed dynamic libraries
            // not really needed now that the paths are fully resolved
            set_dynamic_library_lookup_loc(&jvm_path);

            if let Ok(args) = make_jvm_args(launch_opts) {
                if DEBUG {
                    println!("{:?}", launch_opts);
                }

                if let Ok(vm) = JavaVM::with_libjvm(args, || Ok(jvm_path.as_path())) {
                    return Some(vm);
                }
            } else {
                message("Failed to create JVM arguments.\n\
                Please contact the developers or undo any changes to the configuration.");
            }
        }
    }
    message("No valid Java installations or launch arguments found.");
    None
}

/// Sets the DLL path to the bin folder of the Java runtime,
/// needed for the dynamic libraries to load properly.
/// Subsequent calls replace the path of the previous call.
///
/// see: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-setdlldirectoryw
#[cfg(target_os = "windows")]
fn set_dynamic_library_lookup_loc(jvm_path: &PathBuf) {
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::LibraryLoader::SetDllDirectoryW;
    if let Some(jvm_dll_folder) = jvm_path.parent() {
        if let Some(bin) = jvm_dll_folder.parent() {
            let bin_as_lpcwstr: Vec<u16> = bin.as_os_str().encode_wide().chain(once(0)).collect();
            unsafe {
                SetDllDirectoryW(bin_as_lpcwstr.as_ptr());
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn set_dynamic_library_lookup_loc(_jvm_path: &PathBuf) {
    // No-op for non-Windows systems
}

/// Calls `DestroyJavaVM` of JNI - it blocks until all Java threads are closed <br>
/// See <https://docs.oracle.com/en/java/javase/17/docs/specs/jni/invocation.html#unloading-the-vm>
fn close_jvm(jvm: JavaVM) {
    if let Err(err) = unsafe { jvm.destroy() } {
        eprintln!("Failed to close JVM: {:?}", err);
    }
}

/// Convert string args to the proper format and add to the launch args.<br>
/// Sets the JVM to ignore unrecognized `-X` args and to expect calls to JNI 2
fn make_jvm_args(launch_opts: &LaunchOpts) -> Result<InitArgs<'_>, JvmError> {
    let mut jvm_args = InitArgsBuilder::new()
        .version(JNIVersion::V2) // No touchy or things breaky
        .ignore_unrecognized(true);

    for jvm_opt in &launch_opts.jvm_opts {
        if DEBUG {
            println!("{:?}", jvm_opt);
        }
        jvm_args = jvm_args.option(jvm_opt.as_str());
    }

    jvm_args.build()
}
