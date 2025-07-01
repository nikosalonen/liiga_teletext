use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};

/// Configuration structure for the application.
/// Handles loading, saving, and managing application settings.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// API domain for fetching game data. Should include https:// prefix.
    pub api_domain: String,
    /// Path to the log file. If not specified, logs will be written to a default location.
    #[serde(default)]
    pub log_file_path: Option<String>,
}

impl Config {
    /// Loads configuration from the default config file location.
    /// If no config file exists, prompts user for API domain and creates one.
    ///
    /// # Returns
    /// * `Ok(Config)` - Successfully loaded or created configuration
    /// * `Err(AppError)` - Error occurred during load/create
    ///
    /// # Notes
    /// - Config file is stored in platform-specific config directory
    /// - Handles first-time setup with user prompts
    pub async fn load() -> Result<Self, AppError> {
        let config_path = Config::get_config_path();

        if Path::new(&config_path).exists() {
            let content = fs::read_to_string(&config_path).await?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            println!("Please enter your API domain: ");
            let mut input = String::new();
            let stdin = io::stdin();
            let mut reader = io::BufReader::new(stdin);
            reader.read_line(&mut input).await?;

            let config = Config {
                api_domain: input.trim().to_string(),
                log_file_path: None,
            };

            config.save().await?;
            Ok(config)
        }
    }

    /// Saves current configuration to the default config file location.
    ///
    /// # Returns
    /// * `Ok(())` - Successfully saved configuration
    /// * `Err(AppError)` - Error occurred during save
    ///
    /// # Notes
    /// - Creates config directory if it doesn't exist
    /// - Ensures api_domain has https:// prefix
    /// - Uses TOML format for storage
    pub async fn save(&self) -> Result<(), AppError> {
        let config_path = Config::get_config_path();
        let config_dir = Path::new(&config_path).parent().unwrap();

        if !config_dir.exists() {
            fs::create_dir_all(config_dir).await?;
        }

        // Ensure api_domain has https:// prefix
        let api_domain = if !self.api_domain.starts_with("https://") {
            format!("https://{}", self.api_domain.trim_start_matches("http://"))
        } else {
            self.api_domain.clone()
        };

        let content = toml::to_string_pretty(&Config {
            api_domain,
            log_file_path: self.log_file_path.clone(),
        })?;
        let mut file = fs::File::create(&config_path).await?;
        file.write_all(content.as_bytes()).await?;

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

    /// Returns the platform-specific path for the log directory.
    ///
    /// # Returns
    /// String containing the absolute path to the log directory
    ///
    /// # Notes
    /// - Uses platform-specific config directory (e.g., ~/.config on Linux)
    /// - Falls back to current directory if config directory is unavailable
    pub fn get_log_dir_path() -> String {
        dirs::config_dir()
            .unwrap_or_else(|| Path::new(".").to_path_buf())
            .join("liiga_teletext")
            .join("logs")
            .to_string_lossy()
            .to_string()
    }

    /// Displays current configuration settings to stdout.
    ///
    /// # Returns
    /// * `Ok(())` - Successfully displayed configuration
    /// * `Err(AppError)` - Error occurred while reading config
    ///
    /// # Notes
    /// - Shows config file location and current settings
    /// - Handles case when no config file exists
    pub async fn display() -> Result<(), AppError> {
        let config_path = Config::get_config_path();
        let log_dir = Config::get_log_dir_path();

        if Path::new(&config_path).exists() {
            let config = Config::load().await?;
            println!("\nCurrent Configuration");
            println!("────────────────────────────────────");
            println!("Config Location:");
            println!("{}", config_path);
            println!("────────────────────────────────────");
            println!("API Domain:");
            println!("{}", config.api_domain);
            println!("────────────────────────────────────");
            println!("Log File Location:");
            if let Some(custom_path) = &config.log_file_path {
                println!("{}", custom_path);
            } else {
                println!("{}/liiga_teletext.log", log_dir);
                println!("(Default location)");
            }
        } else {
            println!("\nNo configuration file found at:");
            println!("{}", config_path);
        }

        Ok(())
    }
}
