use std::{env, thread};
use std::path::Path;
use crate::display_handler::message;
use crate::file_handler::get_java_version_of_main;
use crate::java_launcher::{create_and_run_jvm, LaunchOpts};
use crate::launch_config::{get_jvm_options, parse_config, LauncherConfig};

mod display_handler;
mod java_launcher;
mod launch_config;
mod file_handler;

pub const DEBUG: bool = false;

/// Entrypoint
fn main() {
    println!("Launcher starting!");

    // todo true when publishing
    if true {
        correct_directory();
    } else {
        env::set_current_dir("./test").expect("could not set test directory");
        println!("{:?}", env::current_dir());
    }

    launch();
}

/// Setup the environment and launch the application
fn launch() {
    let cfgPath = env::current_exe().unwrap().with_extension(".cfg");

    // Build launch opts
    let mut m = LaunchOpts {
        config: parse_config(cfgPath).unwrap(),
        jvm_opts: vec![],                    //this can be relative
        program_opts: env::args().collect(), // Forward launch args to the app
    };

    // The first element is the launcher path, no need to pass it on
    if m.program_opts.len() >= 1 {
        m.program_opts.remove(0);
    }

    // Build classpath
    m.jvm_opts.append(&mut get_jvm_options(&m.config));
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
    let exe_home = env::current_exe();
    if let Ok(exe_home) = exe_home {
        if let Some(exe_home) = exe_home.parent() {
            let _ = env::set_current_dir(exe_home);
        }
    }
}
