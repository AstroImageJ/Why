use std::fs::File;
use std::io;
use std::io::BufRead;
use std::path::{Path};
use config::{Config, FileFormat};

pub struct LauncherConfig {
    pub jvm_path: Option<String>,
    pub main_class: Option<String>,
    pub launch_options_file: Option<String>,
    pub classpath: Option<String>,
    pub allows_system_java: bool,
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
            launch_options_file: None,
            allows_system_java: true,
            allows_java_location_lookup: true
        }
    }
}

