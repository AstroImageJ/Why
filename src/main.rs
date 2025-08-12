use crate::display_handler::message;
use crate::file_handler::get_app_dir_path;
use crate::java_launcher::{create_and_run_jvm, LaunchOpts};
use crate::launch_config::read_config;
use std::env;

mod display_handler;
mod file_handler;
mod java_launcher;
mod launch_config;
mod manifest_handler;
mod zip_handler;

pub const DEBUG: bool = false;

/// Entrypoint
fn main() {
    if DEBUG {
        println!("Launcher starting!");
    }

    pre_launch();

    correct_directory();

    launch();
}

/// Setup the environment and launch the application
fn launch() {
    let cfg_path = get_app_dir_path().join(
        env::current_exe()
            .unwrap()
            .with_extension("cfg")
            .file_name()
            .unwrap(),
    );

    // Set current directory to app folder
    let _ = env::set_current_dir(get_app_dir_path());

    if !cfg_path.exists() {
        message(format!("Failed to find configuration file at {:?}", cfg_path).as_str())
    }

    // Build launch opts
    let mut launch_options = LaunchOpts {
        config: read_config(cfg_path).unwrap(),
        jvm_opts: vec![],
        program_opts: env::args().skip(1).collect(), // Forward launch args to the app
    };

    // Forward jvm options to primary config struct
    launch_options
        .jvm_opts
        .append(&mut launch_options.config.java_opts);

    // Forward embedded program options to primary config struct
    launch_options
        .program_opts
        .append(&mut launch_options.config.program_opts);

    if DEBUG {
        println!("{:?}", launch_options);
    }

    // Run the app
    #[cfg(not(target_os = "macos"))]
    {
        pre_jvm_launch();
        create_and_run_jvm(&launch_options);
    }

    #[cfg(target_os = "macos")]
    {
        // More complicated handling so that AWT/Swing can run on the main thread
        // and Apple events can be handled
        use std::thread;

        // On macOS, we need to run the JVM in a separate thread
        thread::spawn(move || {
            //pre_jvm_launch(); // Has to be disabled for AWT to work for some reason
            create_and_run_jvm(&launch_options);
        });

        // Parks the thread to handle apple events and AWt as the gui needs to run on
        // the main thread on mac.
        // This is code from Roast, licensed under Apache 2.0, which adapts code from the JDK's JLI
        // library.
        // https://github.com/fourlastor-alexandria/roast
        {
            use core_foundation::date::CFAbsoluteTime;
            use core_foundation::runloop::{
                kCFRunLoopDefaultMode, CFRunLoop, CFRunLoopRunResult, CFRunLoopTimer,
                CFRunLoopTimerRef,
            };
            use std::{ffi::c_void, ptr, time::Duration};

            extern "C" fn dummy_timer(_: CFRunLoopTimerRef, _: *mut c_void) {}

            // Create a dummy timer with a far future fire time
            let timer = CFRunLoopTimer::new(
                CFAbsoluteTime::from(1.0e5), // Fire time
                0.0,                         // Interval
                0,                           // Flags
                0,                           // Order
                dummy_timer,                 // Dummy callback
                ptr::null_mut(),
            );

            unsafe {
                // Add the timer to the current run loop in default mode
                let current_run_loop = CFRunLoop::get_current();
                current_run_loop.add_timer(&timer, kCFRunLoopDefaultMode);

                // Park the thread in the run loop
                loop {
                    let result = CFRunLoop::run_in_mode(
                        kCFRunLoopDefaultMode,
                        Duration::from_secs_f64(1.0e5),
                        false,
                    );

                    if result == CFRunLoopRunResult::Finished {
                        break;
                    }
                }
            }
        }
    }
}

/// This makes sure the current working directory is the exe's home.<br>
/// This can differ from the current working directory in cases where you are running the exe
/// from the command line or script from a different location.
fn correct_directory() {
    // This gets the location of the exe file, not its current working directory
    // These can differ if say running the exe through the command line when in a different folder
    let exe_home = env::current_exe();
    if let Ok(exe_home) = exe_home {
        if let Some(exe_home) = exe_home.parent() {
            let _ = env::set_current_dir(exe_home);
        }
    }
}

fn pre_launch() {
    #[cfg(target_os = "windows")]
    {
        // Dpi awareness is set in the manifest, see build.rs
    }

    #[cfg(target_os = "macos")]
    {

    }

    #[cfg(target_os = "linux")]
    {

    }
}

#[allow(dead_code)]
fn pre_jvm_launch() {
    #[cfg(target_os = "windows")]
    {

    }

    #[cfg(target_os = "macos")]
    {
        // Recreate -XstartOnFirstThread JVM option, with hack to bypass it
        // https://github.com/openjdk/jdk/blob/master/src/java.base/macosx/native/libjli/java_md_macosx.m#L877
        // https://github.com/openjdk/jdk/blob/master/src/java.desktop/macosx/native/libawt_lwawt/awt/LWCToolkit.m#L798
        {
            if env::var_os("HACK_IGNORE_START_ON_FIRST_THREAD").is_some() {
                return;
            }

            let pid = std::process::id();
            let key = format!("JAVA_STARTED_ON_FIRST_THREAD_{}", pid);

            unsafe {
                env::set_var(key, "1");
            }
        }
    }

    #[cfg(target_os = "linux")]
    {

    }
}