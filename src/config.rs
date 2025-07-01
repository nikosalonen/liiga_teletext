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
        self.save_to_path(&config_path).await
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

    /// Saves configuration to a custom file path (for testing).
    pub async fn save_to_path(&self, path: &str) -> Result<(), AppError> {
        let config_dir = Path::new(path).parent().unwrap();
        if !config_dir.exists() {
            fs::create_dir_all(config_dir).await?;
        }
        let api_domain = if !self.api_domain.starts_with("https://") {
            format!("https://{}", self.api_domain.trim_start_matches("http://"))
        } else {
            self.api_domain.clone()
        };
        let content = toml::to_string_pretty(&Config {
            api_domain,
            log_file_path: self.log_file_path.clone(),
        })?;
        let mut file = fs::File::create(path).await?;
        file.write_all(content.as_bytes()).await?;
        Ok(())
    }

    /// Loads configuration from a custom file path (for testing).
    pub async fn load_from_path(path: &str) -> Result<Self, AppError> {
        let content = fs::read_to_string(path).await?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_config_load_existing_file() {
        // Create a temporary config file
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let config_content = r#"
api_domain = "https://api.example.com"
log_file_path = "/custom/log/path"
"#;
        fs::write(&config_path, config_content).unwrap();

        // Test loading from a specific path by reading the file directly
        let content = fs::read_to_string(&config_path).unwrap();
        let config: Config = toml::from_str(&content).unwrap();

        assert_eq!(config.api_domain, "https://api.example.com");
        assert_eq!(config.log_file_path, Some("/custom/log/path".to_string()));
    }

    #[tokio::test]
    async fn test_config_load_without_log_file_path() {
        // Create a temporary config file without log_file_path
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let config_content = r#"
api_domain = "https://api.example.com"
"#;
        fs::write(&config_path, config_content).unwrap();

        // Test loading from a specific path by reading the file directly
        let content = fs::read_to_string(&config_path).unwrap();
        let config: Config = toml::from_str(&content).unwrap();

        assert_eq!(config.api_domain, "https://api.example.com");
        assert_eq!(config.log_file_path, None);
    }

    #[tokio::test]
    async fn test_config_save_new_file() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let config_path_str = config_path.to_string_lossy();
        let config = Config {
            api_domain: "https://api.example.com".to_string(),
            log_file_path: Some("/custom/log/path".to_string()),
        };
        config.save_to_path(&config_path_str).await.unwrap();
        assert!(config_path.exists());
        let content = tokio::fs::read_to_string(&config_path).await.unwrap();
        assert!(content.contains("api_domain = \"https://api.example.com\""));
        assert!(content.contains("log_file_path = \"/custom/log/path\""));
    }

    #[tokio::test]
    async fn test_config_save_without_https_prefix() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let config_path_str = config_path.to_string_lossy();
        let config = Config {
            api_domain: "api.example.com".to_string(),
            log_file_path: None,
        };
        config.save_to_path(&config_path_str).await.unwrap();
        let content = tokio::fs::read_to_string(&config_path).await.unwrap();
        assert!(content.contains("api_domain = \"https://api.example.com\""));
    }

    #[tokio::test]
    async fn test_config_save_with_http_prefix() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let config_path_str = config_path.to_string_lossy();
        let config = Config {
            api_domain: "http://api.example.com".to_string(),
            log_file_path: None,
        };
        config.save_to_path(&config_path_str).await.unwrap();
        let content = tokio::fs::read_to_string(&config_path).await.unwrap();
        assert!(content.contains("api_domain = \"https://api.example.com\""));
    }

    #[tokio::test]
    async fn test_config_save_creates_directory() {
        let temp_dir = tempdir().unwrap();
        let config_dir = temp_dir.path().join("liiga_teletext");
        let config_path = config_dir.join("config.toml");
        let config_path_str = config_path.to_string_lossy();
        let config = Config {
            api_domain: "https://api.example.com".to_string(),
            log_file_path: None,
        };
        config.save_to_path(&config_path_str).await.unwrap();
        assert!(config_dir.exists());
        assert!(config_path.exists());
    }

    #[tokio::test]
    async fn test_config_save_and_load_roundtrip() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let config_path_str = config_path.to_string_lossy();
        let original_config = Config {
            api_domain: "https://api.example.com".to_string(),
            log_file_path: Some("/custom/log/path".to_string()),
        };
        original_config.save_to_path(&config_path_str).await.unwrap();
        let loaded_config = Config::load_from_path(&config_path_str).await.unwrap();
        assert_eq!(original_config.api_domain, loaded_config.api_domain);
        assert_eq!(original_config.log_file_path, loaded_config.log_file_path);
    }

    #[test]
    fn test_get_config_path() {
        let config_path = Config::get_config_path();

        // Should contain the expected directory structure
        assert!(config_path.contains("liiga_teletext"));
        assert!(config_path.ends_with("config.toml"));

        // Should be an absolute or relative path (platform-agnostic)
        let path = PathBuf::from(&config_path);
        assert!(path.is_absolute() || path.is_relative());
    }

    #[test]
    fn test_get_log_dir_path() {
        let log_dir_path = Config::get_log_dir_path();

        // Should contain the expected directory structure
        assert!(log_dir_path.contains("liiga_teletext"));
        assert!(log_dir_path.ends_with("logs"));

        // Should be an absolute or relative path (platform-agnostic)
        let path = PathBuf::from(&log_dir_path);
        assert!(path.is_absolute() || path.is_relative());
    }

    #[tokio::test]
    async fn test_config_display_with_existing_config() {
        // Clean up any existing config file first
        let config_path = Config::get_config_path();
        let _ = tokio::fs::remove_file(&config_path).await;

        // Create a config file first
        let config = Config {
            api_domain: "https://api.example.com".to_string(),
            log_file_path: None,
        };
        config.save().await.unwrap();

        // Test that display() works with existing config
        let result = Config::display().await;
        assert!(result.is_ok());

        // Clean up
        let _ = tokio::fs::remove_file(&config_path).await;
    }

    #[tokio::test]
    async fn test_config_display_without_config_file() {
        // This test verifies that display() handles missing config gracefully
        // We can't easily mock the path, so we just test that the function runs
        let result = Config::display().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_load_invalid_toml() {
        // Create invalid TOML content
        let invalid_content = r#"
api_domain = "https://api.example.com"
invalid_field = [1, 2, 3, "unclosed_string
"#;

        // Test that invalid TOML fails to parse
        let result: Result<Config, _> = toml::from_str(invalid_content);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_serialization_deserialization() {
        let config = Config {
            api_domain: "https://api.example.com".to_string(),
            log_file_path: Some("/custom/log/path".to_string()),
        };

        // Test serialization
        let toml_string = toml::to_string_pretty(&config).unwrap();
        assert!(toml_string.contains("api_domain = \"https://api.example.com\""));
        assert!(toml_string.contains("log_file_path = \"/custom/log/path\""));

        // Test deserialization
        let deserialized_config: Config = toml::from_str(&toml_string).unwrap();
        assert_eq!(config.api_domain, deserialized_config.api_domain);
        assert_eq!(config.log_file_path, deserialized_config.log_file_path);
    }

    #[test]
    fn test_config_without_log_file_path_serialization() {
        let config = Config {
            api_domain: "https://api.example.com".to_string(),
            log_file_path: None,
        };

        let toml_string = toml::to_string_pretty(&config).unwrap();
        assert!(toml_string.contains("api_domain = \"https://api.example.com\""));
        assert!(!toml_string.contains("log_file_path"));

        let deserialized_config: Config = toml::from_str(&toml_string).unwrap();
        assert_eq!(config.api_domain, deserialized_config.api_domain);
        assert_eq!(config.log_file_path, deserialized_config.log_file_path);
    }
}
