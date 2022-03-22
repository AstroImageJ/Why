#![windows_subsystem = "windows"]

mod display_handler;
mod launch_config;
mod java_launcher;

use std::env;
use crate::display_handler::message;
use crate::java_launcher::{create_and_run_jvm, LaunchOpts};
use crate::launch_config::LauncherConfig;

fn main() {
    println!("Starting application!");
    // todo comment when publishing
    //std::env::set_current_dir("./test");

    launch();
}

fn launch() {
    let mut m = LaunchOpts {
        config: LauncherConfig {
            ..LauncherConfig::read_file()
        },
        jvm_opts: vec![],//this can be relative
        program_opts: env::args().collect() // Forward launch args to the app
    };

    m.jvm_opts.append(&mut m.config.read_launch_opts());
    if m.config.classpath.is_some() {
        m.jvm_opts.push("-Djava.class.path=".to_string() + &*m.config.classpath.as_ref().unwrap());
    }

    create_and_run_jvm(&m)
}