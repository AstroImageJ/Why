use crate::display_handler::message;
use crate::file_handler::get_app_dir_path;
use crate::java_launcher::{create_and_run_jvm, LaunchOpts};
use crate::launch_config::{parse_config, process_config};
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
        config: process_config(&parse_config(cfg_path).unwrap()),
        jvm_opts: vec![],
        program_opts: env::args().collect(), // Forward launch args to the app
    };

    // The first element is the launcher path, no need to pass it on
    if launch_options.program_opts.len() >= 1 {
        launch_options.program_opts.remove(0);
    }

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
    create_and_run_jvm(&launch_options)
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