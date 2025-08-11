use crate::file_handler::{
    get_app_dir_path, get_app_image_root, get_config_overlay_path,
    get_default_runtime_path, get_exec_path, get_java_version_of_main
};
use crate::manifest_handler::read_manifest;
use crate::DEBUG;
use std::path::PathBuf;
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, BufReader},
    path::Path,
};

#[cfg(target_os = "windows")]
const SEPARATOR: &str = ";";

#[cfg(not(target_os = "windows"))]
const SEPARATOR: &str = ":";

/// A map of keys to all their values within a section.
pub type Section = HashMap<String, Vec<String>>;

/// The full config: section name → Section.
pub type JPackageLaunchConfig = HashMap<String, Section>;

#[derive(Debug)]
pub struct LaunchConfig {
    pub main_class: String,
    pub runtime: Option<PathBuf>,
    pub min_java: Option<u16>,
    pub java_opts: Vec<String>,
    #[allow(dead_code)]
    pub classpath: Vec<String>,
    pub program_opts: Vec<String>,
}

/// Reads and parses a configuration file, optionally merging it with a secondary configuration
/// if available. The secondary configuration takes precedence over the primary when conflicts occur.
pub fn read_config<P: AsRef<Path>>(path: P) -> io::Result<LaunchConfig> {
    let primary = parse_config(&path)?;
    let root_name = path.as_ref().file_stem().and_then(|s| s.to_str());

    if let Some(secondary_path) = get_config_overlay_path(root_name) {
        if DEBUG {
            println!("secondary_path: {:?}", secondary_path);
        }

        if secondary_path.exists() {
            let secondary = parse_config(&secondary_path)?;

            // Merge the two configs, with secondary taking precedence.
            // While merging vectors, avoid inserting duplicates.
            let mut merged = primary.clone();
            for (section_name, section) in secondary.iter() {
                for (key, values) in section.iter() {
                    let entry_vec = merged
                        .entry(section_name.clone())
                        .or_insert_with(Section::new)
                        .entry(key.clone())
                        .or_default();

                    // Avoid inserting duplicates
                    for v in values {
                        if !entry_vec.contains(v) {
                            entry_vec.push(v.clone());
                        }
                    }
                }
            }

            return Ok(process_config(&merged))
        }
    }

    Ok(process_config(&primary))
}

/// Parse an INI‑style file at `path` into a `Config`,
/// preserving duplicate keys as multiple values.
/// See https://github.com/openjdk/jdk/blob/master/src/jdk.jpackage/share/native/applauncher/CfgFile.cpp#L198
pub fn parse_config<P: AsRef<Path>>(path: P) -> io::Result<JPackageLaunchConfig> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut config = JPackageLaunchConfig::new();
    // Default section for keys before any [section]
    let mut current_section = String::from("default");

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();

        let line = line
            .replace("$APPDIR", get_app_dir_path().to_str().unwrap())
            .replace("$ROOTDIR", get_app_image_root().to_str().unwrap())
            .replace("$BINDIR", std::env::current_exe()?.parent().unwrap().to_str().unwrap());

        // Skip blank lines and comments
        // Jpackage only supports ; for comments
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        // Section header
        if line.starts_with('[') {
            let e = line.rfind("]");
            if let Some(e) = e {
                current_section = line[1..e].to_string();
                continue;
            } else {
                panic!("Invalid config line: {}", line);
            }
        }

        // Key=value
        if let Some(idx) = line.find('=') {
            let key = line[..idx].trim().to_string();
            let val = line[idx + 1..].trim().to_string();

            let section = config
                .entry(current_section.clone())
                .or_insert_with(Section::new);
            section.entry(key).or_default().push(val);
        }
    }

    Ok(config)
}

pub fn process_config(cfg: &JPackageLaunchConfig) -> LaunchConfig {
    let mut options: Vec<String> = Vec::new();
    let mut classpath: Vec<String> = Vec::new();
    let mut program_opts: Vec<String> = Vec::new();
    let mut runtime: Option<PathBuf> = None;
    let mut main_class: Option<String> = None;
    let mut lookup_path: Vec<String> = Vec::new();

    if DEBUG {
        println!("{:#?}", cfg);
    }

    if let Some(java_sec) = cfg.get("JavaOptions") {
        if let Some(opts) = java_sec.get("java-options") {
            options.append(&mut opts.clone());
        }
    }

    if let Some(app_sec) = cfg.get("Application") {
        if let Some(cp) = app_sec.get("app.classpath") {
            classpath.append(&mut cp.clone());
        }

        if let Some(_version) = app_sec.get("app.version") {
            // Doesn't seem to be handled by jpackage despite being mentioned in code
        }

        if let Some(main_jar) = app_sec.get("app.mainjar") {
            match read_manifest(&PathBuf::from(main_jar.last().unwrap().clone())) {
                Ok(manifest) => {
                    let main_sec = manifest[&None].clone();

                    if let Some(mc) = main_sec.get("Main-Class") {
                        main_class = Some(mc.clone());
                    }

                    if let Some(_mc) = main_sec.get("Launcher-Agent-Class") {
                        //todo
                    }

                    if let Some(cp) = main_sec.get("Class-Path") {
                        cp.split(" ").for_each(|s| classpath.push(s.to_string()));
                    }

                    if let Some(ex) = main_sec.get("Add-Exports") {
                        ex.split(' ').for_each(|s| {
                            options.push("--add-exports".to_string());
                            options.push(format!("{}=ALL-UNNAMED", s));
                        })
                    }

                    if let Some(ex) = main_sec.get("Add-Opens") {
                        ex.split(' ').for_each(|s| {
                            options.push("--add-opens".to_string());
                            options.push(format!("{}=ALL-UNNAMED", s));
                        })
                    }

                    if let Some(ex) = main_sec.get("Enable-Native-Access") {
                        // This is the only valid value when entered into the manifest
                        if ex == "ALL-UNNAMED" {
                            options.push(format!("--enable-native-access={}", ex));
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{}", e);
                }
            }

            lookup_path.push(main_jar[0].clone());
            classpath.push(main_jar[0].clone());
        }

        if let Some(mc) = app_sec.get("app.mainclass") {
            main_class = Some(mc.last().unwrap().clone());
        }

        if let Some(main_module) = app_sec.get("app.mainmodule") {
            options.push("-m".to_string());
            options.append(&mut main_module.clone());
        }

        if let Some(module_path) = app_sec.get("app.modulepath") {
            options.push("--module-path".to_string());
            options.push(module_path.join(SEPARATOR));
        }

        if let Some(rt) = app_sec.get("app.runtime") {
            runtime = Some(PathBuf::from(rt.last().unwrap()));
        } else {
            runtime = Some(get_default_runtime_path());
        }

        if let Some(_splash) = app_sec.get("app.splash") {
            /*options.push("-splash".to_string());
            options.append(&mut splash.clone());*/
            //NO-OP JNI does not support
            //would need to manually invoke the splash screen classes for launch
            //https://docs.oracle.com/javase/tutorial/uiswing/misc/splashscreen.html#:~:text=how%20to%20use%20the%20command-line%20argument%20to%20display%20a%20splash%20screen
        }

        if let Some(_memory) = app_sec.get("app.memory") {
            // Doesn't seem to be handled by jpackage despite being mentioned in code
            //https://github.com/search?q=repo%3Aopenjdk%2Fjdk+memory+path%3Ajdk.jpackage&type=code
        }
    }

    if let Some(app_options) = cfg.get("ArgOptions") {
        if let Some(args) = app_options.get("arguments") {
            program_opts.append(&mut args.clone())
        }
    }

    options.push(format!("-Djpackage.app-path={}", get_exec_path()));

    if classpath.len() > 0 {
        options.push(format!("-Djava.class.path={}", classpath.join(SEPARATOR)));
    }

    // If the main jar is specified, use that as the lookup path to avoid searching the entire
    // classpath, otherwise use the entire classpath.
    if lookup_path.is_empty() {
        lookup_path = classpath.clone();
    }

    return LaunchConfig {
        main_class: main_class.clone().unwrap(),
        runtime,
        min_java: get_java_version_of_main(&main_class, &lookup_path),
        java_opts: options.clone(),
        classpath,
        program_opts,
    };
}
