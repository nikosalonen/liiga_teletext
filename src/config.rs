// src/config.rs
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub api_domain: String,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn Error>> {
        let config_path = Config::get_config_path();

        if Path::new(&config_path).exists() {
            let content = fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            // Try to copy from example config
            let example_paths = [
                Path::new("example.config.toml").to_path_buf(), // Current directory
                std::env::current_exe()?
                    .parent()
                    .unwrap()
                    .join("example.config.toml"), // Executable directory
            ];

            for example_path in example_paths {
                if example_path.exists() {
                    let example_content = fs::read_to_string(&example_path)?;
                    let mut config: Config = toml::from_str(&example_content)?;

                    // If api_domain is ###, prompt user for input
                    if config.api_domain == "###" {
                        print!("Please enter your API domain: ");
                        io::stdout().flush()?;
                        let mut input = String::new();
                        io::stdin().read_line(&mut input)?;
                        config.api_domain = input.trim().to_string();
                    }

                    config.save()?; // Save the copied config
                    return Ok(config);
                }
            }

            Err(format!(
                "Neither config nor example config found. Expected config at {} or example at example.config.toml",
                config_path
            )
            .into())
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let config_path = Config::get_config_path();
        let config_dir = Path::new(&config_path).parent().unwrap();

        if !config_dir.exists() {
            fs::create_dir_all(config_dir)?;
        }

        let content = toml::to_string_pretty(self)?;
        let mut file = fs::File::create(&config_path)?;
        file.write_all(content.as_bytes())?;

        Ok(())
    }

    pub fn get_config_path() -> String {
        dirs::config_dir()
            .unwrap_or_else(|| Path::new(".").to_path_buf())
            .join("liiga_teletext")
            .join("config.toml")
            .to_string_lossy()
            .to_string()
    }
}
