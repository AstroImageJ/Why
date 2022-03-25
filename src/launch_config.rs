use std::fs::File;
use std::io;
use std::io::BufRead;
use std::path::Path;

use config::{Config, FileFormat};
use sysinfo::{System, SystemExt};
use crate::get_java_version_of_main;

/// These module paths must be in the form of opt=value
const MODULE_OPTS: &'static [&str] = &["--add-reads", "--add-exports", "--add-opens",
    "--add-modules", "--limit-modules", "--module-path",
    "--patch-module", "--upgrade-module-path"];

/// The options must match the form accepted by the JVM, otherwise a crash will occur
/// This is only a sampling of commonly used ones to prevent user error.
/// These options are assumed to contain numbers
const STANDARD_OPTS: &'static [&str] = &["-Xmx", "-Xms"];

/// These are read in from launcher.ini from the current working directory
pub struct LauncherConfig {
    /// key: jvm_install; format: String (path), can be relative by preceding with './';
    /// what it does: the path to the location of the jvm.dll -
    /// it will recursively search into this path up to a depth of 4 for the jvm.dll
    /// REQUIRED if allows_system_java and allows_java_lookup are disabled
    pub jvm_path: Option<String>,
    /// key: mainclass; format: String (as it would appear in a jar manifest);
    /// what it does: the main class, as it would appear in a jar manifest
    /// REQUIRED
    pub main_class: Option<String>,
    /// key: launch_options; format: String (path), can be relative by preceding with './';
    /// what it does: the Launch4J-style config to read JVM options from
    pub launch_options_file: Option<String>,
    /// key: classpath; format: same as the launch argument - ';' separated paths;
    /// what it does: sets the classpath; If given a jar, it will respect the jar
    /// manifest's classpath entry
    /// REQUIRED
    pub classpath: Option<String>,
    /// key: min_java; format: integer; what it does: only tries to run Java that is
    /// equal to or greater than this Java version
    pub min_java: Option<i64>,
    /// key: allow_system_java; format: boolean; what it does: whether the launcher
    /// should use the Java listed in JAVA_HOME
    pub allows_system_java: bool,
    /// key: allow_java_location_lookup; format: boolean;
    /// what it does: whether the launcher should check common Java
    /// installation directories for a Java install
    pub allows_java_location_lookup: bool,
    /// key: maximum_heap_percentage; format: integer;
    /// what it does: sets the -Xmx to this value if missing from the launch args.
    pub max_mem_percent: Option<i64>,
    /// key: check_main_class; format: boolean;
    /// what it does: whether the launcher should check check the main class' Java version
    /// requirement and use that as the min_java if the current min_java is not specified or
    /// less than the found main class requirement. Otherwise, use the specified min_java.
    pub check_main_class: bool,
}

/// Sets the defaults
impl Default for LauncherConfig {
    fn default() -> Self {
        LauncherConfig {
            jvm_path: None,
            main_class: None,
            classpath: None,
            min_java: None,
            max_mem_percent: None,
            launch_options_file: None,
            allows_system_java: true,
            allows_java_location_lookup: true,
            check_main_class: true,
        }
    }
}

impl LauncherConfig {
    /// Ensure that enough information is provided to actually start Java
    pub fn validate(&self) -> bool {
        self.main_class.is_some() && self.classpath.is_some()
    }

    /// Read `launcher.ini` and setup the launcher config.<br>
    /// This will also ensure that the main class can be run by the minimum Java requirement,
    /// if enabled.
    pub fn read_file() -> Self {
        let config_file = Config::builder()
            .add_source(config::File::new("launcher.ini", FileFormat::Ini))
            .build();
        return if let Ok(c) = config_file {
            let mut cfg = LauncherConfig {
                main_class: c.get_string("mainclass").ok(),
                classpath: c.get_string("classpath").ok(),
                jvm_path: c.get_string("jvm_install").ok(),
                min_java: c.get_int("min_java").ok(),
                launch_options_file: c.get_string("launch_options").ok(),
                allows_system_java: c.get_bool("allow_system_java").unwrap_or(true),
                allows_java_location_lookup: c.get_bool("allow_java_location_lookup").unwrap_or(true),
                max_mem_percent: c.get_int("maximum_heap_percentage").ok(),
                check_main_class: c.get_bool("check_main_class").unwrap_or(true),
                ..Default::default()
            };
            cfg.ensure_correct_java();
            cfg
        } else {
            Default::default()
        };
    }

    /// Read `launch_options_file` into a series of launch options,
    /// sanitizing and correcting where possible.
    pub fn read_launch_opts(&self) -> Vec<String> {
        let mut out: Vec<String> = vec![];
        if self.launch_options_file.as_ref().is_some() {
            if let Ok(lines) = read_lines(self.launch_options_file.as_ref().unwrap().as_str()) {
                // Consumes the iterator, returns an (Optional) String
                for line in lines {
                    if let Ok(ip) = line {
                        let sanitized_line = verify_line(ip);
                        let mut opts = parse_line(sanitized_line).iter()
                            .map(|o| verify_opt(o.to_owned())).collect();
                        out.append(&mut opts)
                    }
                }
            }
        }

        if let Some(mem_per) = self.max_mem_percent {
            let has_xmx = out.iter().any(|s| s.starts_with("-Xmx"));
            if !has_xmx {
                out.push(format!("-Xmx{}kb", get_max_heap(mem_per)))
            }
        }

        return out;
    }

    /// Make sure the minimum Java requirement is not less than that needed for the main class.
    pub fn ensure_correct_java(&mut self) {
        if self.check_main_class {
            let new_min = get_java_version_of_main(self);
            if let Some(new_min) = new_min {
                if let Some(min_java) = self.min_java {
                    if new_min as i64 > min_java {
                        self.min_java = Some(new_min as i64);
                    }
                } else {
                    self.min_java = Some(new_min as i64);
                }
            }
        }
    }
}

/// Convert a line into several strings, splitting on spaces.
pub fn parse_line(line: String) -> Vec<String> {
    let out: Vec<String> = vec![];
    if !line.starts_with("#") {
        let m = line.split(" ");
        let a: Vec<String> = m.map(String::from).collect();
        return a;
    }

    out
}

/// Verify Java standard options (those beginning with `-X`).<br>
/// If wrongly formatted, the JVM will fail on startup.
fn verify_opt(input: String) -> String {
    for opt in STANDARD_OPTS {
        if input.starts_with(opt) {
            let numeral_part = &input[opt.len()..];
            if let Some(b) = numeral_part.chars().next().and_then(|c| Some(c.is_numeric())) {
                if b {
                    return input;
                }
            }
            println!("Launcher rejected a launch option ({}) due to incorrect format.", input);
            return "".to_string();
        }
    }
    input
}

/// Handle verification of args with spaces
fn verify_line(mut line: String) -> String {
    for opt in MODULE_OPTS {
        if line.contains((opt.to_string() + " ").as_str()) {
            println!("Launcher corrected a module option ({})! They must be in the form of opt=val.", opt)
        }
        line = line.replace((opt.to_string() + " ").as_str(), (opt.to_string() + "=").as_str());
    }
    line
}

/// Read file as lines
fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
    where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

/// Gets the maximum ram on this system in kb
fn get_max_heap(mem_per: i64) -> u64 {
    let mut sys = System::new_all();
    sys.refresh_all();
    let max_mem = sys.total_memory();
    let mut mem_frac: f64 = ((mem_per as f64) / 100f64);
    if mem_frac <= 0.01f64 || mem_frac >= 1f64 {
        mem_frac = 0.2f64;
    }
    ((max_mem as f64) * mem_frac) as u64
}

