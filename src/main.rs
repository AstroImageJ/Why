#![windows_subsystem = "windows"]

mod display_handler;
mod launch_config;
mod java_launcher;

use std::env;
use crate::display_handler::message;
use crate::java_launcher::{create_and_run_jvm, LaunchOpts};
use crate::launch_config::LauncherConfig;

/// Entrypoint
fn main() {
    println!("Launcher starting!");
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
        jvm_opts: vec![],//this can be relative
        program_opts: env::args().collect() // Forward launch args to the app
    };

    // Build classpath
    m.jvm_opts.append(&mut m.config.read_launch_opts());
    if m.config.classpath.is_some() {
        m.jvm_opts.push("-Djava.class.path=".to_string() + &*m.config.classpath.as_ref().unwrap());
    }

    // Run the app
    create_and_run_jvm(&m)
}