use std::fs::File;
use std::io;
use std::io::BufRead;
use std::path::{Path};
use config::{Config, FileFormat};

pub struct LauncherConfig {
    /// key: jvm_install; format: String (path), can be relative by preceding with './';
    /// what it does: the path to the location of the jvm.dll -
    /// it will recursively search into this path uo to a depth of 4 for the jvm.dll
    /// REQUIRED if allows_system_java and allows_java_lookup are disabled
    pub jvm_path: Option<String>,
    /// key: mainclass; format: String, can be relative by preceding with './';
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
    pub allows_java_location_lookup: bool
}

impl LauncherConfig {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn validate(&self) -> bool {
        self.main_class.is_some() && self.classpath.is_some()
    }

    pub fn read_file() -> Self {
        let config_file = Config::builder()
            .add_source(config::File::new("launcher.ini", FileFormat::Ini))
            .build();
        return if let Ok(c) = config_file {
            LauncherConfig {
                main_class: c.get_string("mainclass").ok(),
                classpath: c.get_string("classpath").ok(),
                jvm_path: c.get_string("jvm_install").ok(),
                min_java: c.get_int("min_java").ok(),
                launch_options_file: c.get_string("launch_options").ok(),
                allows_system_java: c.get_bool("allow_system_java").unwrap_or(true),
                allows_java_location_lookup: c.get_bool("allow_java_location_lookup").unwrap_or(true),
                ..Default::default()
            }
        } else {
            Default::default()
        }
    }

    pub fn read_launch_opts(&self) -> Vec<String> {
        let mut out: Vec<String> = vec![];
        if self.launch_options_file.as_ref().is_some() {
            if let Ok(lines) = read_lines(self.launch_options_file.as_ref().unwrap().as_str()) {
                // Consumes the iterator, returns an (Optional) String
                for line in lines {
                    if let Ok(ip) = line {
                        out.append(&mut parse_line(ip))
                    }
                }
            }
        }

        return out
    }
}

//todo sanitize for things like : in -Xmx1000 as it will cause crashes on startup
pub fn parse_line(line: String) -> Vec<String> {
    let out: Vec<String> = vec![];
    if !line.starts_with("#") {
        let m= line.split(" ");
        let a: Vec<String> = m.map(String::from).collect();
        return a
    }

    out
}

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
    where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

impl Default for LauncherConfig {
    fn default() -> Self {
        LauncherConfig {
            jvm_path: None,
            main_class: None,
            classpath: None,
            min_java: None,
            launch_options_file: None,
            allows_system_java: true,
            allows_java_location_lookup: true
        }
    }
}

