use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dir = out_dir.join("bin");

    // Create the bin directory if it doesn't exist
    if !dir.exists() {
        fs::create_dir_all(&dir).unwrap();
    }
}
