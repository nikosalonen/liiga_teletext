use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Get the target directory, either from CARGO_TARGET_DIR or default to ./target
    let target_dir = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());
    let target_dir = Path::new(&target_dir);

    // Copy to both debug and release directories
    let target_dirs = [target_dir.join("debug"), target_dir.join("release")];

    for dir in target_dirs {
        if dir.exists() {
            println!("cargo:warning=Copying example.config.toml to {:?}", dir);
            if let Err(e) = fs::copy("example.config.toml", dir.join("example.config.toml")) {
                println!(
                    "cargo:warning=Failed to copy example.config.toml to {:?}: {}",
                    dir, e
                );
            }
        }
    }

    // Tell Cargo to re-run this if example.config.toml changes
    println!("cargo:rerun-if-changed=example.config.toml");
}
