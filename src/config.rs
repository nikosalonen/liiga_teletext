// src/config.rs
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Configuration structure for the application.
/// Handles loading, saving, and managing application settings.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// API domain for fetching game data. Should include https:// prefix.
    pub api_domain: String,
}

impl Config {
    /// Loads configuration from the default config file location.
    /// If no config file exists, prompts user for API domain and creates one.
    ///
    /// # Returns
    /// * `Ok(Config)` - Successfully loaded or created configuration
    /// * `Err(Box<dyn Error>)` - Error occurred during load/create
    ///
    /// # Notes
    /// - Config file is stored in platform-specific config directory
    /// - Handles first-time setup with user prompts
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

    /// Saves current configuration to the default config file location.
    ///
    /// # Returns
    /// * `Ok(())` - Successfully saved configuration
    /// * `Err(Box<dyn Error>)` - Error occurred during save
    ///
    /// # Notes
    /// - Creates config directory if it doesn't exist
    /// - Ensures api_domain has https:// prefix
    /// - Uses TOML format for storage
    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let config_path = Config::get_config_path();
        let config_dir = Path::new(&config_path).parent().unwrap();

        if !config_dir.exists() {
            fs::create_dir_all(config_dir)?;
        }

        // Ensure api_domain has https:// prefix
        let api_domain = if !self.api_domain.starts_with("https://") {
            format!("https://{}", self.api_domain.trim_start_matches("http://"))
        } else {
            self.api_domain.clone()
        };

        let content = toml::to_string_pretty(&Config { api_domain })?;
        let mut file = fs::File::create(&config_path)?;
        file.write_all(content.as_bytes())?;

        Ok(())
    }

    /// Returns the platform-specific path for the config file.
    ///
    /// # Returns
    /// String containing the absolute path to the config file
    ///
    /// # Notes
    /// - Uses platform-specific config directory (e.g., ~/.config on Linux)
    /// - Falls back to current directory if config directory is unavailable
    pub fn get_config_path() -> String {
        dirs::config_dir()
            .unwrap_or_else(|| Path::new(".").to_path_buf())
            .join("liiga_teletext")
            .join("config.toml")
            .to_string_lossy()
            .to_string()
    }

    /// Displays current configuration settings to stdout.
    ///
    /// # Returns
    /// * `Ok(())` - Successfully displayed configuration
    /// * `Err(Box<dyn Error>)` - Error occurred while reading config
    ///
    /// # Notes
    /// - Shows config file location and current settings
    /// - Handles case when no config file exists
    pub fn display() -> Result<(), Box<dyn Error>> {
        let config_path = Config::get_config_path();

        if Path::new(&config_path).exists() {
            let config = Config::load()?;
            println!("\nCurrent Configuration");
            println!("────────────────────────────────────");
            println!("Config Location:");
            println!("{}", config_path);
            println!("────────────────────────────────────");
            println!("API Domain:");
            println!("{}", config.api_domain);
        } else {
            println!("\nNo configuration file found at:");
            println!("{}", config_path);
        }

        Ok(())
    }
}
