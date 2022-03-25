use core::option::Option;
use core::option::Option::{None, Some};
use std::fs::File;
use std::io::{Read};
use std::path::Path;
use zip::ZipArchive;
use crate::{LauncherConfig, LaunchOpts};

/// Try and find the main class from the given classpath (without resolving it)
/// and return its required Java version.
pub fn get_java_version_of_main(launch_cfg: &LauncherConfig) -> Option<u16> {
    // Not enough info provided
    if !launch_cfg.validate() {
        return None
    }

    // Go over the classpath
    return if let Some(classpath) = &launch_cfg.classpath {
        let jars: Vec<&str> = classpath.split(";").collect();
        for jar_path in jars {
            // Open the jar
            if let Ok(jar) = File::open(Path::new(jar_path)) {
                if let Ok(mut zip_jar) = ZipArchive::new(jar) {
                    // Find main class
                    if let Ok(class) = zip_jar.by_name((launch_cfg.main_class
                        .as_ref().unwrap().to_string()
                        .replace(".", "/") + ".class").as_str()) {
                        // Found main class, get the version
                        return read_class_version_to_java(class)
                    }
                }
            }
        }
        None
    } else {
        None
    }
}

/// Reads in the first few bytes of a file to determine if it is a class file,
/// and if so what Java version it was compiled for.<br>
/// Returns the Java version a class needs.<br>
/// See <https://docs.oracle.com/javase/specs/jvms/se17/html/jvms-4.html>
fn read_class_version_to_java<R: Read>(mut reader: R) -> Option<u16> {
    let mut buffer: [u8; 8] = [0; 8];

    // Read file into buffer
    reader.read_exact(&mut *&mut buffer).ok()?;

    // Ensure bytes are Big Endian per the JVM spec.
    buffer = buffer.map(|b| b.to_be());

    // Get magic byte
    let num_bytes: [u8; 4] = (buffer[0..4]).try_into().ok()?;
    let magic_number: u32 = u32::from_be_bytes(num_bytes);

    // Is Java class?
    if magic_number == 0xCAFEBABE {
        /*let minor_version =
            u16::from_be_bytes((buffer[4..6]).try_into().ok().unwrap_or_default());*/
        let major_version =
            u16::from_be_bytes((buffer[6..8]).try_into().ok().unwrap_or_default());

        // If smaller than 45, it likely isn't a Java class
        if major_version >= 45 {
            // Convert to Java major version
            return Some(major_version - 44);
        } else {
            None
        }
    } else {
        None
    }
}
