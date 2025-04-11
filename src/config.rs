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
            print!("Please enter your API domain: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            let config = Config {
                api_domain: input.trim().to_string(),
            };

            config.save()?;
            Ok(config)
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
