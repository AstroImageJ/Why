use std::{env, thread};
use std::path::{Path, PathBuf};
use crate::display_handler::message;
use crate::file_handler::{get_app_dir_path, get_java_version_of_main};
use crate::java_launcher::{create_and_run_jvm, LaunchOpts};
use crate::launch_config::{process_config, parse_config, JPackageLaunchConfig};
use crate::manifest_handler::read_manifest;

mod display_handler;
mod java_launcher;
mod launch_config;
mod file_handler;
mod manifest_handler;

pub const DEBUG: bool = true;

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
    let cfgPath = env::current_exe().unwrap().with_extension("cfg");
    println!("{:?}", cfgPath);

    let mf = read_manifest(PathBuf::from("ij.jar"));
    println!("{:?}", mf);

    let mf = read_manifest(PathBuf::from("ij"));
    println!("{:?}", mf);

    let cfgPath = get_app_dir_path().join(env::current_exe().unwrap().with_extension("cfg").file_name().unwrap());
    println!("{:?}", cfgPath);
    let _ = env::set_current_dir(get_app_dir_path());

    // Build launch opts
    let mut m = LaunchOpts {
        config: process_config(&parse_config(cfgPath).unwrap()),//todo handle missing file error
        jvm_opts: vec![],                    //this can be relative
        program_opts: env::args().collect(), // Forward launch args to the app
    };

    // The first element is the launcher path, no need to pass it on
    if m.program_opts.len() >= 1 {
        m.program_opts.remove(0);
    }

    // Build classpath
    m.jvm_opts.append(&mut m.config.java_opts);
    println!("{:?}", m);
    println!("{:?}", m.jvm_opts);
    /*if m.config.classpath.is_some() {
        m.jvm_opts
            .push("-Djava.class.path=".to_string() + &*m.config.classpath.as_ref().unwrap());
    }*/

    // Run the app
    create_and_run_jvm(&m)
}

/// This makes sure the current working directory is the exe's home.<br>
/// This can differ from the current working directory in cases where you are running the exe
/// from command line or script from a different location.
//todo set to app/ directory made by jpackage
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
