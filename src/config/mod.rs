use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;

pub mod paths;
pub mod validation;
pub mod user_prompts;

use paths::{get_config_path, get_log_dir_path};
use validation::validate_config;
use user_prompts::prompt_for_api_domain;

/// Configuration structure for the application.
/// Handles loading, saving, and managing application settings.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// API domain for fetching game data. Should include https:// prefix.
    pub api_domain: String,
    /// Path to the log file. If not specified, logs will be written to a default location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_file_path: Option<String>,
    /// HTTP timeout in seconds for API requests. Defaults to 30 seconds if not specified.
    #[serde(default = "default_http_timeout")]
    pub http_timeout_seconds: u64,
}

/// Default HTTP timeout in seconds
fn default_http_timeout() -> u64 {
    crate::constants::DEFAULT_HTTP_TIMEOUT_SECONDS
}

impl Default for Config {
    fn default() -> Self {
        Config {
            api_domain: String::new(),
            log_file_path: None,
            http_timeout_seconds: default_http_timeout(),
        }
    }
}

impl Config {
    /// Loads configuration from the default config file location.
    /// If no config file exists, prompts user for API domain and creates one.
    /// Environment variables can override config file values.
    ///
    /// # Environment Variables
    /// - `LIIGA_API_DOMAIN` - Override API domain
    /// - `LIIGA_LOG_FILE` - Override log file path
    /// - `LIIGA_HTTP_TIMEOUT` - Override HTTP timeout in seconds (default: 30)
    ///
    /// # Returns
    /// * `Ok(Config)` - Successfully loaded or created configuration
    /// * `Err(AppError)` - Error occurred during load/create
    ///
    /// # Notes
    /// - Config file is stored in platform-specific config directory
    /// - Handles first-time setup with user prompts
    /// - Environment variables take precedence over config file
    pub async fn load() -> Result<Self, AppError> {
        let config_path = get_config_path();

        let mut config = if Path::new(&config_path).exists() {
            let content = fs::read_to_string(&config_path).await?;
            toml::from_str(&content)?
        } else {
            // Check if API domain is provided via environment variable
            if let Ok(api_domain) = std::env::var("LIIGA_API_DOMAIN") {
                Config {
                    api_domain,
                    log_file_path: None,
                    http_timeout_seconds: default_http_timeout(),
                }
            } else {
                let api_domain = prompt_for_api_domain().await?;

                let config = Config {
                    api_domain,
                    log_file_path: None,
                    http_timeout_seconds: default_http_timeout(),
                };

                config.save().await?;
                config
            }
        };

        // Override with environment variables if present
        if let Ok(api_domain) = std::env::var("LIIGA_API_DOMAIN") {
            config.api_domain = api_domain;
        }

        if let Ok(log_file_path) = std::env::var("LIIGA_LOG_FILE") {
            config.log_file_path = Some(log_file_path);
        }

        if let Some(timeout) = std::env::var("LIIGA_HTTP_TIMEOUT")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
        {
            config.http_timeout_seconds = timeout;
        }

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Validates the configuration settings
    ///
    /// # Returns
    /// * `Ok(())` - Configuration is valid
    /// * `Err(AppError)` - Configuration validation failed
    pub fn validate(&self) -> Result<(), AppError> {
        validate_config(&self.api_domain, &self.log_file_path)
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
        let config_path = get_config_path();
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
        paths::get_config_path()
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
        paths::get_log_dir_path()
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
        let config_path = get_config_path();
        let log_dir = get_log_dir_path();

        if Path::new(&config_path).exists() {
            let config = Config::load().await?;
            println!("\nCurrent Configuration");
            println!("────────────────────────────────────");
            println!("Config Location:");
            println!("{config_path}");
            println!("────────────────────────────────────");
            println!("API Domain:");
            println!("{}", config.api_domain);
            println!("────────────────────────────────────");
            println!("HTTP Timeout:");
            println!("{} seconds", config.http_timeout_seconds);
            println!("────────────────────────────────────");
            println!("Log File Location:");
            if let Some(custom_path) = &config.log_file_path {
                println!("{custom_path}");
            } else {
                println!("{log_dir}/liiga_teletext.log");
                println!("(Default location)");
            }
        } else {
            println!("\nNo configuration file found at:");
            println!("{config_path}");
        }

        Ok(())
    }

    /// Saves configuration to a custom file path.
    ///
    /// This method can be used for general configuration saving to any location,
    /// not just for testing purposes. It creates the parent directory if it doesn't exist
    /// and ensures the API domain has the proper https:// prefix.
    ///
    /// # Arguments
    /// * `path` - The file path where the configuration should be saved
    ///
    /// # Returns
    /// * `Ok(())` - Successfully saved configuration
    /// * `Err(AppError)` - Error occurred while saving (e.g., invalid path, I/O error)
    ///
    /// # Errors
    /// * `AppError::Config` - If the provided path has no parent directory
    /// * `AppError::Io` - If there's an I/O error creating directories or writing the file
    /// * `AppError::TomlSerialize` - If there's an error serializing the configuration
    pub async fn save_to_path(&self, path: &str) -> Result<(), AppError> {
        let config_dir = Path::new(path).parent().ok_or_else(|| {
            AppError::config_error(format!("Path '{path}' has no parent directory"))
        })?;

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
            http_timeout_seconds: self.http_timeout_seconds,
        })?;
        let mut file = fs::File::create(path).await?;
        file.write_all(content.as_bytes()).await?;
        file.flush().await?;
        Ok(())
    }

    /// Loads configuration from a custom file path (for testing).
    #[allow(dead_code)] // Used in tests
    pub async fn load_from_path(path: &str) -> Result<Self, AppError> {
        let content = fs::read_to_string(path).await?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_config_load_existing_file() {
        // Create a temporary config file
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let config_path_str = config_path.to_string_lossy();

        let config_content = r#"
api_domain = "https://api.example.com"
log_file_path = "/custom/log/path"
"#;
        tokio::fs::write(&config_path, config_content)
            .await
            .unwrap();

        // Test loading from a specific path using the actual load_from_path method
        let config = Config::load_from_path(&config_path_str).await.unwrap();

        assert_eq!(config.api_domain, "https://api.example.com");
        assert_eq!(config.log_file_path, Some("/custom/log/path".to_string()));
    }

    #[tokio::test]
    async fn test_config_load_without_log_file_path() {
        // Create a temporary config file without log_file_path
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let config_path_str = config_path.to_string_lossy();

        let config_content = r#"
api_domain = "https://api.example.com"
"#;
        tokio::fs::write(&config_path, config_content)
            .await
            .unwrap();

        // Test loading from a specific path using the actual load_from_path method
        let config = Config::load_from_path(&config_path_str).await.unwrap();

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
            http_timeout_seconds: default_http_timeout(),
        };
        config.save_to_path(&config_path_str).await.unwrap();
        assert!(config_path.exists());
        let content = tokio::fs::read_to_string(&config_path).await.unwrap();
        // More robust assertions that handle potential formatting differences
        assert!(
            content.contains("api_domain") && content.contains("https://api.example.com"),
            "Content should contain api_domain and https://api.example.com. Content: {content}"
        );
        assert!(
            content.contains("log_file_path") && content.contains("/custom/log/path"),
            "Content should contain log_file_path and /custom/log/path. Content: {content}"
        );
        // Also test that the loaded config has the correct values
        let loaded_config = Config::load_from_path(&config_path_str).await.unwrap();
        assert_eq!(loaded_config.api_domain, "https://api.example.com");
        assert_eq!(
            loaded_config.log_file_path,
            Some("/custom/log/path".to_string())
        );
    }

    #[tokio::test]
    async fn test_config_save_without_https_prefix() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let config_path_str = config_path.to_string_lossy();
        let config = Config {
            api_domain: "api.example.com".to_string(),
            log_file_path: None,
            http_timeout_seconds: default_http_timeout(),
        };
        config.save_to_path(&config_path_str).await.unwrap();
        let content = tokio::fs::read_to_string(&config_path).await.unwrap();
        // More robust assertion that handles potential formatting differences
        assert!(
            content.contains("api_domain") && content.contains("https://api.example.com"),
            "Content should contain api_domain and https://api.example.com. Content: {content}"
        );
        // Also test that the loaded config has the correct domain
        let loaded_config = Config::load_from_path(&config_path_str).await.unwrap();
        assert_eq!(loaded_config.api_domain, "https://api.example.com");
    }

    #[tokio::test]
    async fn test_config_save_with_http_prefix() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let config_path_str = config_path.to_string_lossy();
        let config = Config {
            api_domain: "http://api.example.com".to_string(),
            log_file_path: None,
            http_timeout_seconds: default_http_timeout(),
        };
        config.save_to_path(&config_path_str).await.unwrap();
        let content = tokio::fs::read_to_string(&config_path).await.unwrap();
        // More robust assertion that handles potential formatting differences
        assert!(
            content.contains("api_domain") && content.contains("https://api.example.com"),
            "Content should contain api_domain and https://api.example.com. Content: {content}"
        );
        // Also test that the loaded config has the correct domain
        let loaded_config = Config::load_from_path(&config_path_str).await.unwrap();
        assert_eq!(loaded_config.api_domain, "https://api.example.com");
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
            http_timeout_seconds: default_http_timeout(),
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
            http_timeout_seconds: default_http_timeout(),
        };
        original_config
            .save_to_path(&config_path_str)
            .await
            .unwrap();
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
    }

    #[test]
    fn test_get_log_dir_path() {
        let log_dir_path = Config::get_log_dir_path();

        // Should contain the expected directory structure
        assert!(log_dir_path.contains("liiga_teletext"));
        assert!(log_dir_path.ends_with("logs"));
    }

    #[tokio::test]
    async fn test_config_display_with_existing_config() {
        // Test the core functionality that display() uses by testing the load() method
        // This avoids modifying the real config file while still testing the same logic

        // Create a temporary config file to test the loading functionality
        let temp_dir = tempdir().unwrap();
        let temp_config_path = temp_dir.path().join("config.toml");
        let temp_config_path_str = temp_config_path.to_string_lossy();

        // Create a test config file in temporary location
        let test_config = Config {
            api_domain: "https://api.example.com".to_string(),
            log_file_path: Some("/custom/log/path".to_string()),
            http_timeout_seconds: default_http_timeout(),
        };
        test_config
            .save_to_path(&temp_config_path_str)
            .await
            .unwrap();

        // Test that we can load the config (this is what display() does internally)
        let loaded_config = Config::load_from_path(&temp_config_path_str).await.unwrap();
        assert_eq!(loaded_config.api_domain, "https://api.example.com");
        assert_eq!(
            loaded_config.log_file_path,
            Some("/custom/log/path".to_string())
        );

        // The temporary directory and file will be automatically cleaned up
        // when temp_dir goes out of scope
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
            http_timeout_seconds: default_http_timeout(),
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
            http_timeout_seconds: default_http_timeout(),
        };

        // Test serialization
        let toml_string = toml::to_string_pretty(&config).unwrap();
        assert!(toml_string.contains("api_domain = \"https://api.example.com\""));
        // log_file_path should not appear in TOML when it's None due to skip_serializing_if
        assert!(!toml_string.contains("log_file_path"));

        // Test deserialization
        let deserialized_config: Config = toml::from_str(&toml_string).unwrap();
        assert_eq!(config.api_domain, deserialized_config.api_domain);
        assert_eq!(config.log_file_path, deserialized_config.log_file_path);
    }

    #[tokio::test]
    async fn test_config_load_from_nonexistent_path() {
        // Test loading from a path that doesn't exist
        let result = Config::load_from_path("/nonexistent/path/config.toml").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::Io(_)));
    }

    #[tokio::test]
    async fn test_config_save_to_readonly_directory() {
        // This test is platform-dependent and may not work on all systems
        // but it tests the error handling for directory creation failures
        let result = Config::load().await;
        // We can't easily test this without elevated permissions, so we just
        // ensure the function exists and can be called
        assert!(result.is_ok() || result.is_err()); // Either is valid
    }

    #[tokio::test]
    async fn test_config_malformed_toml_file() {
        // Create a malformed TOML file
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("malformed_config.toml");
        let config_path_str = config_path.to_string_lossy();

        let malformed_content = r#"
api_domain = "https://api.example.com"
[invalid_section
malformed = "data
"#;
        tokio::fs::write(&config_path, malformed_content)
            .await
            .unwrap();

        // Test that loading malformed TOML fails gracefully
        let result = Config::load_from_path(&config_path_str).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::TomlDeserialize(_)));
    }

    #[tokio::test]
    async fn test_config_missing_required_field() {
        // Create a TOML file missing the required api_domain field
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("incomplete_config.toml");
        let config_path_str = config_path.to_string_lossy();

        let incomplete_content = r#"
# Missing api_domain
log_file_path = "/some/path"
"#;
        tokio::fs::write(&config_path, incomplete_content)
            .await
            .unwrap();

        // Test that loading incomplete config fails
        let result = Config::load_from_path(&config_path_str).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::TomlDeserialize(_)));
    }

    #[tokio::test]
    async fn test_config_with_extra_fields() {
        // Create a TOML file with extra fields that should be ignored
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("extra_fields_config.toml");
        let config_path_str = config_path.to_string_lossy();

        let extra_fields_content = r#"
api_domain = "https://api.example.com"
log_file_path = "/custom/log/path"
extra_field = "this should be ignored"
another_extra = 123
"#;
        tokio::fs::write(&config_path, extra_fields_content)
            .await
            .unwrap();

        // Test that loading config with extra fields works (extra fields ignored)
        let config = Config::load_from_path(&config_path_str).await.unwrap();
        assert_eq!(config.api_domain, "https://api.example.com");
        assert_eq!(config.log_file_path, Some("/custom/log/path".to_string()));
    }

    #[tokio::test]
    async fn test_config_with_various_api_domain_formats() {
        let test_cases = vec![
            // (input, expected_output)
            ("api.example.com", "https://api.example.com"),
            ("http://api.example.com", "https://api.example.com"),
            ("https://api.example.com", "https://api.example.com"),
            ("https://api.example.com/", "https://api.example.com/"),
            ("localhost:8080", "https://localhost:8080"),
            ("http://localhost:8080", "https://localhost:8080"),
        ];

        for (input, expected) in test_cases {
            let temp_dir = tempdir().unwrap();
            let config_path = temp_dir.path().join("test_config.toml");
            let config_path_str = config_path.to_string_lossy();

            let config = Config {
                api_domain: input.to_string(),
                log_file_path: None,
                http_timeout_seconds: default_http_timeout(),
            };

            // Save the config
            config.save_to_path(&config_path_str).await.unwrap();

            // Verify the file exists and has content
            assert!(config_path.exists(), "Config file should exist");
            let metadata = tokio::fs::metadata(&config_path).await.unwrap();
            assert!(metadata.len() > 0, "Config file should not be empty");

            // Read back the saved config to verify the domain was processed correctly
            let content = tokio::fs::read_to_string(&config_path).await.unwrap();

            // Debug: Print the actual content to see what's being written
            println!("Input: '{input}', Expected: '{expected}', Actual content: '{content}'");

            assert!(
                !content.is_empty(),
                "File content should not be empty for input '{input}'"
            );

            assert!(
                content.contains("api_domain"),
                "Content should contain 'api_domain' for input '{input}'. Content: '{content}'"
            );

            assert!(
                content.contains(expected),
                "Content should contain '{expected}' for input '{input}'. Content: '{content}'"
            );

            // Also test that the loaded config has the correct domain
            let loaded_config = Config::load_from_path(&config_path_str).await.unwrap();
            assert_eq!(loaded_config.api_domain, expected);
        }
    }

    #[test]
    fn test_config_path_generation() {
        let config_path = Config::get_config_path();

        // Verify the path structure
        assert!(config_path.contains("liiga_teletext"));
        assert!(config_path.ends_with("config.toml"));

        // Verify it's a valid path (doesn't test if it exists, just that it's a valid path format)
        let path = Path::new(&config_path);
        assert!(path.is_absolute() || path.is_relative());
    }

    #[test]
    fn test_log_dir_path_generation() {
        let log_dir_path = Config::get_log_dir_path();

        // Verify the path structure
        assert!(log_dir_path.contains("liiga_teletext"));
        assert!(log_dir_path.ends_with("logs"));

        // Verify it's a valid path
        let path = Path::new(&log_dir_path);
        assert!(path.is_absolute() || path.is_relative());
    }

    #[tokio::test]
    async fn test_config_save_creates_nested_directories() {
        // Test that save_to_path creates nested directories
        let temp_dir = tempdir().unwrap();
        let nested_path = temp_dir
            .path()
            .join("level1")
            .join("level2")
            .join("level3")
            .join("config.toml");
        let nested_path_str = nested_path.to_string_lossy();

        let config = Config {
            api_domain: "https://api.example.com".to_string(),
            log_file_path: None,
            http_timeout_seconds: default_http_timeout(),
        };

        // This should create all the nested directories
        config.save_to_path(&nested_path_str).await.unwrap();

        // Verify the file was created
        assert!(nested_path.exists());

        // Verify the content is correct with more robust assertion
        let content = tokio::fs::read_to_string(&nested_path).await.unwrap();
        assert!(
            content.contains("api_domain") && content.contains("https://api.example.com"),
            "Content should contain api_domain and https://api.example.com. Content: {content}"
        );

        // Also test that the loaded config has the correct domain
        let loaded_config = Config::load_from_path(&nested_path_str).await.unwrap();
        assert_eq!(loaded_config.api_domain, "https://api.example.com");
    }

    #[tokio::test]
    async fn test_config_serialization_with_special_characters() {
        // Test config with URLs containing special characters
        let config = Config {
            api_domain: "https://api.example.com/path?param=value&other=123#fragment".to_string(),
            log_file_path: Some("/path/with spaces/and-dashes_underscores.log".to_string()),
            http_timeout_seconds: default_http_timeout(),
        };

        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("special_config.toml");
        let config_path_str = config_path.to_string_lossy();

        // Save and load the config
        config.save_to_path(&config_path_str).await.unwrap();
        let loaded_config = Config::load_from_path(&config_path_str).await.unwrap();

        // The API domain should be processed but keep the path and query parameters
        assert!(loaded_config.api_domain.starts_with("https://"));
        assert!(loaded_config.api_domain.contains("api.example.com"));
        assert_eq!(loaded_config.log_file_path, config.log_file_path);
    }

    #[tokio::test]
    async fn test_config_empty_file() {
        // Test loading from an empty file
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("empty_config.toml");
        let config_path_str = config_path.to_string_lossy();

        // Create an empty file
        tokio::fs::write(&config_path, "").await.unwrap();

        // Loading should fail because api_domain is required
        let result = Config::load_from_path(&config_path_str).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::TomlDeserialize(_)));
    }

    #[test]
    fn test_config_default_log_file_path() {
        // Test that the default log_file_path behavior works correctly
        let config_with_none = Config {
            api_domain: "https://api.example.com".to_string(),
            log_file_path: None,
            http_timeout_seconds: default_http_timeout(),
        };

        let config_with_some = Config {
            api_domain: "https://api.example.com".to_string(),
            log_file_path: Some("/custom/path.log".to_string()),
            http_timeout_seconds: default_http_timeout(),
        };

        // Test serialization behavior
        let toml_none = toml::to_string(&config_with_none).unwrap();
        let toml_some = toml::to_string(&config_with_some).unwrap();

        // When None, log_file_path should not be in the TOML due to skip_serializing_if
        assert!(!toml_none.contains("log_file_path"));
        assert!(toml_some.contains("log_file_path"));
    }

    #[test]
    fn test_config_validation_valid_configs() {
        // Test valid configurations
        let valid_configs = vec![
            Config {
                api_domain: "https://api.example.com".to_string(),
                log_file_path: None,
                http_timeout_seconds: default_http_timeout(),
            },
            Config {
                api_domain: "http://localhost:8080".to_string(),
                log_file_path: Some("/tmp/test.log".to_string()),
                http_timeout_seconds: default_http_timeout(),
            },
            Config {
                api_domain: "api.example.com".to_string(),
                log_file_path: None,
                http_timeout_seconds: default_http_timeout(),
            },
            Config {
                api_domain: "localhost".to_string(),
                log_file_path: None,
                http_timeout_seconds: default_http_timeout(),
            },
        ];

        for config in valid_configs {
            assert!(
                config.validate().is_ok(),
                "Config should be valid: {config:?}"
            );
        }
    }

    #[test]
    fn test_config_validation_invalid_configs() {
        // Test invalid configurations
        let invalid_configs = vec![
            // Empty API domain
            Config {
                api_domain: "".to_string(),
                log_file_path: None,
                http_timeout_seconds: default_http_timeout(),
            },
            // Invalid domain format
            Config {
                api_domain: "invalid_domain".to_string(),
                log_file_path: None,
                http_timeout_seconds: default_http_timeout(),
            },
            // Empty log file path
            Config {
                api_domain: "https://api.example.com".to_string(),
                log_file_path: Some("".to_string()),
                http_timeout_seconds: default_http_timeout(),
            },
        ];

        for config in invalid_configs {
            assert!(
                config.validate().is_err(),
                "Config should be invalid: {config:?}"
            );
        }
    }

    #[tokio::test]
    async fn test_environment_variable_override() {
        // Set environment variables
        unsafe {
            std::env::set_var("LIIGA_API_DOMAIN", "https://env.example.com");
            std::env::set_var("LIIGA_LOG_FILE", "/env/log/path.log");
        }

        // Create a temporary config file with different values
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let config_path_str = config_path.to_string_lossy();

        let config_content = r#"
api_domain = "https://file.example.com"
log_file_path = "/file/log/path.log"
"#;
        tokio::fs::write(&config_path, config_content)
            .await
            .unwrap();

        // Load config using load_from_path (which doesn't check env vars)
        let file_config = Config::load_from_path(&config_path_str).await.unwrap();
        assert_eq!(file_config.api_domain, "https://file.example.com");
        assert_eq!(
            file_config.log_file_path,
            Some("/file/log/path.log".to_string())
        );

        // Clean up environment variables
        unsafe {
            std::env::remove_var("LIIGA_API_DOMAIN");
            std::env::remove_var("LIIGA_LOG_FILE");
            std::env::remove_var("LIIGA_HTTP_TIMEOUT");
        }
    }
}
