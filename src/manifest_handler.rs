use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use zip::ZipArchive;

pub type Section = HashMap<String, String>;

/// Main manifest section has key: None
/// Other sections use the key listed in their Name
pub type Manifest = HashMap<Option<String>, Section>;

/// Reads a JAR manifest (either from a compressed .jar or an exploded directory)
/// and parses it into a Manifest, mapping section names to key-value pairs.
pub fn read_manifest(jar_path: PathBuf) -> Result<Manifest, String> {
    if !jar_path.is_dir() {
        if let Ok(jar) = File::open(jar_path) {
            if let Ok(mut zip_jar) = ZipArchive::new(jar) {
                if let Ok(f) = zip_jar.by_name("META-INF/MANIFEST.MF") {
                    return parse_manifest(BufReader::new(f));
                }
            }
        }
    } else {
        let file = match File::open(jar_path.join("META-INF/MANIFEST.MF")) {
            Ok(f) => f,
            Err(e) => {
                return Err(format!("Error reading META-INF/MANIFEST.MF: {}", e));
            }
        };

        return parse_manifest(BufReader::new(file));
    }

    Err("Manifest not found".to_string())
}

fn parse_manifest<P: Read>(manifest_file: BufReader<P>) -> Result<Manifest, String> {
    let mut manifest: Manifest = Manifest::new();
    let mut current_section_key: Option<String> = None;
    let mut current_section = Section::new();
    let mut current_key: Option<String> = None;
    let mut current_value = String::new();

    for line_res in manifest_file.lines() {
        let line = line_res.map_err(|e| format!("Error reading manifest line: {}", e))?;
        if line.is_empty() {
            // Flush any pending header
            if let Some(prev_key) = current_key.take() {
                current_section.insert(prev_key, current_value.clone());
            }
            // End of current section
            manifest.insert(current_section_key.clone(), current_section.clone());
            // Reset for next section
            current_section = Section::new();
            current_section_key = None;
            current_value.clear();
            continue;
        }
        if let Some(rest) = line.strip_prefix(' ') {
            // Continuation of previous header
            if let Some(_) = current_key {
                current_value.push_str(rest);
            }
        } else if let Some((key, value)) = line.split_once(": ") {
            // New header encountered
            // Flush previous header
            if let Some(prev_key) = current_key.take() {
                // Don't add Name entry to section, as the section is already given a name
                if prev_key != "Name" {
                    current_section.insert(prev_key, current_value.clone());
                }
            }

            // Start new header
            current_key = Some(key.to_string());
            current_value = value.to_string();

            // If this header is "Name", set section key
            if key == "Name" {
                current_section_key = Some(value.to_string());
            }
        } else {
            return Err(format!("Malformed manifest line: '{}'", line));
        }
    }

    // Flush last header and section at EOF
    if current_key.is_some() || !current_section.is_empty() {
        if let Some(prev_key) = current_key.take() {
            current_section.insert(prev_key, current_value.clone());
        }
        manifest.insert(current_section_key.clone(), current_section.clone());
    }
    Ok(manifest)
}
