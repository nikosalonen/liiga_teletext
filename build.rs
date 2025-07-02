use std::env;
use std::fs;
use std::path::PathBuf;

/// Build script for liiga_teletext
///
/// Note: This file intentionally uses synchronous std::fs operations because:
/// - Build scripts run outside the normal async runtime
/// - They execute during compilation, not during application runtime
/// - Cargo expects build scripts to use blocking operations
/// - There's no async runtime available in the build context
fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dir = out_dir.join("bin");

    // Create the bin directory if it doesn't exist
    // Using std::fs here is correct for build scripts
    if !dir.exists() {
        fs::create_dir_all(&dir).unwrap();
    }
}
