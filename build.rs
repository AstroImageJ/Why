use embed_manifest::{embed_manifest, new_manifest};
use embed_manifest::manifest::DpiAwareness;

fn main() {
    // Set DPI awareness as per monitor
    // Done in manifest as the documentation says to prefer it over doing so in code
    // https://learn.microsoft.com/en-us/windows/win32/hidpi/setting-the-default-dpi-awareness-for-a-process
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(new_manifest("Why.JavaLauncher").dpi_awareness(DpiAwareness::PerMonitorV2))
            .expect("unable to embed manifest file");
    }
    println!("cargo:rerun-if-changed=build.rs");
}