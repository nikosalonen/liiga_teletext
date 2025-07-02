use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Failed to fetch data from API: {0}")]
    ApiFetch(#[from] reqwest::Error),

    #[error("Failed to parse API response: {0}")]
    ApiParse(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("TOML deserialization error: {0}")]
    TomlDeserialize(#[from] toml::de::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Version parsing error: {0}")]
    VersionParse(#[from] semver::Error),

    #[error("Date/time parsing error: {0}")]
    DateTimeParse(String),

    #[error("Log setup error: {0}")]
    LogSetup(String),

    #[error("{0}")]
    Custom(String),
}

impl AppError {
    /// Create a configuration error with context
    pub fn config_error(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create a date/time parsing error with context
    pub fn datetime_parse_error(msg: impl Into<String>) -> Self {
        Self::DateTimeParse(msg.into())
    }

    /// Create a log setup error with context
    pub fn log_setup_error(msg: impl Into<String>) -> Self {
        Self::LogSetup(msg.into())
    }
}
