#![windows_subsystem = "windows"]

use std::env;

use crate::display_handler::message;
use crate::java_launcher::{create_and_run_jvm, LaunchOpts};
use crate::launch_config::LauncherConfig;

mod display_handler;
mod java_launcher;
mod launch_config;

/// Entrypoint
fn main() {
    println!("Launcher starting!");

    correct_directory();

    // todo comment when publishing
    //std::env::set_current_dir("./test");

    launch();
}

/// Setup the environment and launch the application
fn launch() {
    // Build launch opts
    let mut m = LaunchOpts {
        config: LauncherConfig {
            ..LauncherConfig::read_file()
        },
        jvm_opts: vec![],                    //this can be relative
        program_opts: env::args().collect(), // Forward launch args to the app
    };

    // Build classpath
    m.jvm_opts.append(&mut m.config.read_launch_opts());
    if m.config.classpath.is_some() {
        m.jvm_opts
            .push("-Djava.class.path=".to_string() + &*m.config.classpath.as_ref().unwrap());
    }

    // Run the app
    create_and_run_jvm(&m)
}

/// This makes sure the current working directory is the exe's home.<br>
/// This can differ from the current working directory in cases where you are running the exe
/// from command line or script from a different location.
fn correct_directory() {
    // This gets the location of the exe file, not its current working directory
    // These can differ if say running the exe through command line when in a different folder
    let exe_home = std::env::current_exe();
    if let Ok(exe_home) = exe_home {
        if let Some(exe_home) = exe_home.parent() {
            let _ = std::env::set_current_dir(exe_home);
        }
    }
}
