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

    #[error("{0}")]
    Custom(String),
}
