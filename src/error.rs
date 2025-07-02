use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Failed to fetch data from API: {0}")]
    ApiFetch(#[from] reqwest::Error),

    #[error("Failed to parse API response: {0}")]
    ApiParse(#[from] serde_json::Error),

    // Specific HTTP status code errors
    #[error("API request not found (404): {url}")]
    ApiNotFound { url: String },

    #[error("API server error ({status}): {message} (URL: {url})")]
    ApiServerError {
        status: u16,
        message: String,
        url: String,
    },

    #[error("API client error ({status}): {message} (URL: {url})")]
    ApiClientError {
        status: u16,
        message: String,
        url: String,
    },

    #[error("API rate limit exceeded (429): {message} (URL: {url})")]
    ApiRateLimit { message: String, url: String },

    #[error("API service unavailable ({status}): {message} (URL: {url})")]
    ApiServiceUnavailable {
        status: u16,
        message: String,
        url: String,
    },

    // Network-specific errors
    #[error("Network timeout while fetching data from: {url}")]
    NetworkTimeout { url: String },

    #[error("Connection failed to: {url} - {message}")]
    NetworkConnection { url: String, message: String },

    // Data parsing and validation errors
    #[error("API returned malformed JSON: {message} (URL: {url})")]
    ApiMalformedJson { message: String, url: String },

    #[error("API returned unexpected data structure: {message} (URL: {url})")]
    ApiUnexpectedStructure { message: String, url: String },

    #[error("API returned empty or missing data: {message} (URL: {url})")]
    ApiNoData { message: String, url: String },

    // API-specific business logic errors
    #[error("Season not found: {season}")]
    ApiSeasonNotFound { season: i32 },

    #[error("Game not found: game_id={game_id}, season={season}")]
    ApiGameNotFound { game_id: i32, season: i32 },

    #[error("Tournament not found: {tournament} for date {date}")]
    ApiTournamentNotFound { tournament: String, date: String },

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
    #[allow(dead_code)] // Kept for backward compatibility and future use
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

    /// Create an API not found error
    pub fn api_not_found(url: impl Into<String>) -> Self {
        Self::ApiNotFound { url: url.into() }
    }

    /// Create an API server error (5xx status codes)
    pub fn api_server_error(
        status: u16,
        message: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        Self::ApiServerError {
            status,
            message: message.into(),
            url: url.into(),
        }
    }

    /// Create an API client error (4xx status codes except 404 and 429)
    pub fn api_client_error(
        status: u16,
        message: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        Self::ApiClientError {
            status,
            message: message.into(),
            url: url.into(),
        }
    }

    /// Create an API rate limit error
    pub fn api_rate_limit(message: impl Into<String>, url: impl Into<String>) -> Self {
        Self::ApiRateLimit {
            message: message.into(),
            url: url.into(),
        }
    }

    /// Create an API service unavailable error
    pub fn api_service_unavailable(
        status: u16,
        message: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        Self::ApiServiceUnavailable {
            status,
            message: message.into(),
            url: url.into(),
        }
    }

    /// Create a network timeout error
    pub fn network_timeout(url: impl Into<String>) -> Self {
        Self::NetworkTimeout { url: url.into() }
    }

    /// Create a network connection error
    pub fn network_connection(url: impl Into<String>, message: impl Into<String>) -> Self {
        Self::NetworkConnection {
            url: url.into(),
            message: message.into(),
        }
    }

    /// Create a malformed JSON error
    pub fn api_malformed_json(message: impl Into<String>, url: impl Into<String>) -> Self {
        Self::ApiMalformedJson {
            message: message.into(),
            url: url.into(),
        }
    }

    /// Create an unexpected data structure error
    pub fn api_unexpected_structure(message: impl Into<String>, url: impl Into<String>) -> Self {
        Self::ApiUnexpectedStructure {
            message: message.into(),
            url: url.into(),
        }
    }

    /// Create a no data error
    pub fn api_no_data(message: impl Into<String>, url: impl Into<String>) -> Self {
        Self::ApiNoData {
            message: message.into(),
            url: url.into(),
        }
    }

    /// Create a season not found error
    pub fn api_season_not_found(season: i32) -> Self {
        Self::ApiSeasonNotFound { season }
    }

    /// Create a game not found error
    pub fn api_game_not_found(game_id: i32, season: i32) -> Self {
        Self::ApiGameNotFound { game_id, season }
    }

    /// Create a tournament not found error
    pub fn api_tournament_not_found(
        tournament: impl Into<String>,
        date: impl Into<String>,
    ) -> Self {
        Self::ApiTournamentNotFound {
            tournament: tournament.into(),
            date: date.into(),
        }
    }

    /// Check if error is retryable (network issues, server errors, rate limits)
    #[allow(dead_code)] // Utility method for future error handling patterns
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            AppError::NetworkTimeout { .. }
                | AppError::NetworkConnection { .. }
                | AppError::ApiServerError { .. }
                | AppError::ApiServiceUnavailable { .. }
                | AppError::ApiRateLimit { .. }
        )
    }

    /// Check if error indicates data not found (business logic, not technical error)
    #[allow(dead_code)] // Utility method for future error handling patterns
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            AppError::ApiNotFound { .. }
                | AppError::ApiSeasonNotFound { .. }
                | AppError::ApiGameNotFound { .. }
                | AppError::ApiTournamentNotFound { .. }
                | AppError::ApiNoData { .. }
        )
    }
}
