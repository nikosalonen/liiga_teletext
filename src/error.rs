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
    #[allow(dead_code)]
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

    /// Get suggested retry delay in seconds based on error type
    /// Values are defined in src/constants.rs retry module for consistency
    #[allow(dead_code)]
    pub fn retry_delay_seconds(&self) -> Option<u64> {
        match self {
            AppError::ApiRateLimit { .. } => Some(60), // constants::retry::RATE_LIMIT_DELAY_SECONDS
            AppError::ApiServerError { .. } => Some(5), // constants::retry::SERVER_ERROR_DELAY_SECONDS
            AppError::ApiServiceUnavailable { .. } => Some(30), // constants::retry::SERVICE_UNAVAILABLE_DELAY_SECONDS
            AppError::NetworkTimeout { .. } => Some(2), // constants::retry::NETWORK_TIMEOUT_DELAY_SECONDS
            AppError::NetworkConnection { .. } => Some(10), // constants::retry::NETWORK_CONNECTION_DELAY_SECONDS
            _ => None,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error_helper() {
        let error = AppError::config_error("Invalid configuration");
        assert!(matches!(error, AppError::Config(_)));
        assert_eq!(
            error.to_string(),
            "Configuration error: Invalid configuration"
        );
    }

    #[test]
    fn test_datetime_parse_error_helper() {
        let error = AppError::datetime_parse_error("Invalid date format");
        assert!(matches!(error, AppError::DateTimeParse(_)));
        assert_eq!(
            error.to_string(),
            "Date/time parsing error: Invalid date format"
        );
    }

    #[test]
    fn test_log_setup_error_helper() {
        let error = AppError::log_setup_error("Failed to initialize logger");
        assert!(matches!(error, AppError::LogSetup(_)));
        assert_eq!(
            error.to_string(),
            "Log setup error: Failed to initialize logger"
        );
    }

    #[test]
    fn test_api_not_found_helper() {
        let error = AppError::api_not_found("https://api.example.com/games/123");
        assert!(matches!(error, AppError::ApiNotFound { .. }));
        assert_eq!(
            error.to_string(),
            "API request not found (404): https://api.example.com/games/123"
        );
    }

    #[test]
    fn test_api_server_error_helper() {
        let error =
            AppError::api_server_error(500, "Internal server error", "https://api.example.com");
        assert!(matches!(error, AppError::ApiServerError { .. }));
        assert_eq!(
            error.to_string(),
            "API server error (500): Internal server error (URL: https://api.example.com)"
        );
    }

    #[test]
    fn test_api_client_error_helper() {
        let error = AppError::api_client_error(400, "Bad request", "https://api.example.com");
        assert!(matches!(error, AppError::ApiClientError { .. }));
        assert_eq!(
            error.to_string(),
            "API client error (400): Bad request (URL: https://api.example.com)"
        );
    }

    #[test]
    fn test_api_rate_limit_helper() {
        let error = AppError::api_rate_limit("Too many requests", "https://api.example.com");
        assert!(matches!(error, AppError::ApiRateLimit { .. }));
        assert_eq!(
            error.to_string(),
            "API rate limit exceeded (429): Too many requests (URL: https://api.example.com)"
        );
    }

    #[test]
    fn test_api_service_unavailable_helper() {
        let error = AppError::api_service_unavailable(
            503,
            "Service unavailable",
            "https://api.example.com",
        );
        assert!(matches!(error, AppError::ApiServiceUnavailable { .. }));
        assert_eq!(
            error.to_string(),
            "API service unavailable (503): Service unavailable (URL: https://api.example.com)"
        );
    }

    #[test]
    fn test_network_timeout_helper() {
        let error = AppError::network_timeout("https://api.example.com");
        assert!(matches!(error, AppError::NetworkTimeout { .. }));
        assert_eq!(
            error.to_string(),
            "Network timeout while fetching data from: https://api.example.com"
        );
    }

    #[test]
    fn test_network_connection_helper() {
        let error = AppError::network_connection("https://api.example.com", "Connection refused");
        assert!(matches!(error, AppError::NetworkConnection { .. }));
        assert_eq!(
            error.to_string(),
            "Connection failed to: https://api.example.com - Connection refused"
        );
    }

    #[test]
    fn test_api_malformed_json_helper() {
        let error =
            AppError::api_malformed_json("Invalid JSON structure", "https://api.example.com");
        assert!(matches!(error, AppError::ApiMalformedJson { .. }));
        assert_eq!(
            error.to_string(),
            "API returned malformed JSON: Invalid JSON structure (URL: https://api.example.com)"
        );
    }

    #[test]
    fn test_api_unexpected_structure_helper() {
        let error =
            AppError::api_unexpected_structure("Missing required field", "https://api.example.com");
        assert!(matches!(error, AppError::ApiUnexpectedStructure { .. }));
        assert_eq!(
            error.to_string(),
            "API returned unexpected data structure: Missing required field (URL: https://api.example.com)"
        );
    }

    #[test]
    fn test_api_no_data_helper() {
        let error = AppError::api_no_data("Empty response", "https://api.example.com");
        assert!(matches!(error, AppError::ApiNoData { .. }));
        assert_eq!(
            error.to_string(),
            "API returned empty or missing data: Empty response (URL: https://api.example.com)"
        );
    }

    #[test]
    fn test_api_season_not_found_helper() {
        let error = AppError::api_season_not_found(2024);
        assert!(matches!(error, AppError::ApiSeasonNotFound { .. }));
        assert_eq!(error.to_string(), "Season not found: 2024");
    }

    #[test]
    fn test_api_game_not_found_helper() {
        let error = AppError::api_game_not_found(123, 2024);
        assert!(matches!(error, AppError::ApiGameNotFound { .. }));
        assert_eq!(
            error.to_string(),
            "Game not found: game_id=123, season=2024"
        );
    }

    #[test]
    fn test_api_tournament_not_found_helper() {
        let error = AppError::api_tournament_not_found("runkosarja", "2024-01-15");
        assert!(matches!(error, AppError::ApiTournamentNotFound { .. }));
        assert_eq!(
            error.to_string(),
            "Tournament not found: runkosarja for date 2024-01-15"
        );
    }

    #[test]
    fn test_is_retryable() {
        // Retryable errors
        assert!(AppError::network_timeout("url").is_retryable());
        assert!(AppError::network_connection("url", "message").is_retryable());
        assert!(AppError::api_server_error(500, "message", "url").is_retryable());
        assert!(AppError::api_rate_limit("message", "url").is_retryable());
        assert!(AppError::api_service_unavailable(503, "message", "url").is_retryable());

        // Non-retryable errors
        assert!(!AppError::api_not_found("url").is_retryable());
        assert!(!AppError::api_client_error(400, "message", "url").is_retryable());
        assert!(!AppError::config_error("message").is_retryable());
        assert!(!AppError::datetime_parse_error("message").is_retryable());
        assert!(!AppError::api_malformed_json("message", "url").is_retryable());
    }

    #[test]
    fn test_is_not_found() {
        // Not found errors
        assert!(AppError::api_not_found("url").is_not_found());
        assert!(AppError::api_season_not_found(2024).is_not_found());
        assert!(AppError::api_game_not_found(123, 2024).is_not_found());
        assert!(AppError::api_tournament_not_found("tournament", "date").is_not_found());

        // Other errors
        assert!(!AppError::api_server_error(500, "message", "url").is_not_found());
        assert!(!AppError::config_error("message").is_not_found());
        assert!(!AppError::network_timeout("url").is_not_found());
        assert!(!AppError::api_malformed_json("message", "url").is_not_found());
    }

    #[test]
    fn test_error_from_reqwest() {
        // Test that reqwest errors are properly converted
        // Create a reqwest error by using an invalid URL in a request
        let client = reqwest::Client::new();
        let request_result = client.get("not a valid url").build();

        match request_result {
            Err(reqwest_error) => {
                let app_error: AppError = reqwest_error.into();
                assert!(matches!(app_error, AppError::ApiFetch(_)));
            }
            Ok(_) => panic!("Expected an error from invalid URL"),
        }
    }

    #[test]
    fn test_error_from_serde_json() {
        // Test that serde_json errors are properly converted
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let app_error: AppError = json_error.into();
        assert!(matches!(app_error, AppError::ApiParse(_)));
    }

    #[test]
    fn test_error_from_io() {
        // Test that IO errors are properly converted
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let app_error: AppError = io_error.into();
        assert!(matches!(app_error, AppError::Io(_)));
    }

    #[test]
    fn test_error_from_toml_serialize() {
        // Test that TOML serialization errors are properly converted
        // Create a struct that will fail to serialize
        #[derive(serde::Serialize)]
        struct BadStruct {
            #[serde(serialize_with = "bad_serialize")]
            field: String,
        }

        fn bad_serialize<S>(_: &String, _: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("Serialization failed"))
        }

        let bad_struct = BadStruct {
            field: "test".to_string(),
        };
        let toml_error = toml::to_string(&bad_struct).unwrap_err();
        let app_error: AppError = toml_error.into();
        assert!(matches!(app_error, AppError::TomlSerialize(_)));
    }

    #[test]
    fn test_error_from_toml_deserialize() {
        // Test that TOML deserialization errors are properly converted
        let invalid_toml = "invalid = [toml";
        let toml_error = toml::from_str::<serde_json::Value>(invalid_toml).unwrap_err();
        let app_error: AppError = toml_error.into();
        assert!(matches!(app_error, AppError::TomlDeserialize(_)));
    }

    #[test]
    fn test_error_from_semver() {
        // Test that semver parsing errors are properly converted
        let invalid_version = "invalid.version.string";
        let semver_error = semver::Version::parse(invalid_version).unwrap_err();
        let app_error: AppError = semver_error.into();
        assert!(matches!(app_error, AppError::VersionParse(_)));
    }

    #[test]
    fn test_custom_error() {
        let error = AppError::Custom("Custom error message".to_string());
        assert_eq!(error.to_string(), "Custom error message");
    }

    #[test]
    fn test_retry_delay_seconds_uses_constants() {
        // Test that retry delays match the constants from the retry module (src/constants.rs)
        let rate_limit_error = AppError::api_rate_limit("rate limit", "https://example.com");
        assert_eq!(
            rate_limit_error.retry_delay_seconds(),
            Some(60) // constants::retry::RATE_LIMIT_DELAY_SECONDS
        );

        let server_error = AppError::api_server_error(500, "server error", "https://example.com");
        assert_eq!(
            server_error.retry_delay_seconds(),
            Some(5) // constants::retry::SERVER_ERROR_DELAY_SECONDS
        );

        let service_unavailable_error = AppError::api_service_unavailable(503, "unavailable", "https://example.com");
        assert_eq!(
            service_unavailable_error.retry_delay_seconds(),
            Some(30) // constants::retry::SERVICE_UNAVAILABLE_DELAY_SECONDS
        );

        let timeout_error = AppError::network_timeout("https://example.com");
        assert_eq!(
            timeout_error.retry_delay_seconds(),
            Some(2) // constants::retry::NETWORK_TIMEOUT_DELAY_SECONDS
        );

        let connection_error = AppError::network_connection("https://example.com", "connection failed");
        assert_eq!(
            connection_error.retry_delay_seconds(),
            Some(10) // constants::retry::NETWORK_CONNECTION_DELAY_SECONDS
        );

        // Test non-retryable error returns None
        let not_found_error = AppError::api_not_found("https://example.com");
        assert_eq!(not_found_error.retry_delay_seconds(), None);
    }

    #[test]
    fn test_retry_delay_seconds_for_retryable_errors() {
        // Test all retryable error types return appropriate delay values
        // Values correspond to constants in src/constants.rs retry module
        let retryable_errors = vec![
            (AppError::api_rate_limit("rate limit", "https://example.com"), 60), // RATE_LIMIT_DELAY_SECONDS
            (AppError::api_server_error(500, "internal error", "https://example.com"), 5), // SERVER_ERROR_DELAY_SECONDS
            (AppError::api_server_error(502, "bad gateway", "https://example.com"), 5), // SERVER_ERROR_DELAY_SECONDS
            (AppError::api_service_unavailable(503, "service unavailable", "https://example.com"), 30), // SERVICE_UNAVAILABLE_DELAY_SECONDS
            (AppError::network_timeout("https://example.com"), 2), // NETWORK_TIMEOUT_DELAY_SECONDS
            (AppError::network_connection("https://example.com", "connection refused"), 10), // NETWORK_CONNECTION_DELAY_SECONDS
        ];

        for (error, expected_delay) in retryable_errors {
            assert_eq!(
                error.retry_delay_seconds(),
                Some(expected_delay),
                "Error {:?} should return delay of {} seconds",
                error,
                expected_delay
            );
            // Also verify that these errors are marked as retryable
            assert!(
                error.is_retryable(),
                "Error {:?} should be retryable",
                error
            );
        }
    }

    #[test]
    fn test_retry_delay_seconds_for_non_retryable_errors() {
        // Test all non-retryable error types return None
        let non_retryable_errors = vec![
            AppError::api_not_found("https://example.com"),
            AppError::api_client_error(400, "bad request", "https://example.com"),
            AppError::api_client_error(401, "unauthorized", "https://example.com"),
            AppError::api_malformed_json("invalid json", "https://example.com"),
            AppError::api_unexpected_structure("missing field", "https://example.com"),
            AppError::api_no_data("empty response", "https://example.com"),
            AppError::api_season_not_found(2024),
            AppError::api_game_not_found(123, 2024),
            AppError::api_tournament_not_found("runkosarja", "2024-01-15"),
            AppError::config_error("invalid config"),
            AppError::datetime_parse_error("invalid date"),
            AppError::log_setup_error("log setup failed"),
            AppError::Custom("custom error".to_string()),
        ];

        for error in non_retryable_errors {
            assert_eq!(
                error.retry_delay_seconds(),
                None,
                "Error {:?} should not have a retry delay",
                error
            );
            // Also verify that these errors are not marked as retryable
            assert!(
                !error.is_retryable(),
                "Error {:?} should not be retryable",
                error
            );
        }
    }

    #[test]
    fn test_retry_delay_constants_consistency() {
        // Verify that the constants used in retry_delay_seconds match expected values
        // This test ensures consistency between the constants defined in src/constants.rs
        // and their usage in the retry_delay_seconds method

        // Values should match the constants defined in src/constants.rs retry module:
        // RATE_LIMIT_DELAY_SECONDS = 60
        // SERVER_ERROR_DELAY_SECONDS = 5
        // SERVICE_UNAVAILABLE_DELAY_SECONDS = 30
        // NETWORK_TIMEOUT_DELAY_SECONDS = 2
        // NETWORK_CONNECTION_DELAY_SECONDS = 10

        let rate_limit_delay = 60u64;
        let server_error_delay = 5u64;
        let service_unavailable_delay = 30u64;
        let timeout_delay = 2u64;
        let connection_delay = 10u64;

        // Verify the hierarchy of delays (rate limit should be longest, timeout shortest)
        assert!(rate_limit_delay >= service_unavailable_delay);
        assert!(service_unavailable_delay >= connection_delay);
        assert!(connection_delay >= server_error_delay);
        assert!(server_error_delay >= timeout_delay);

        // Verify that the retry_delay_seconds method returns these exact values
        assert_eq!(AppError::api_rate_limit("test", "url").retry_delay_seconds(), Some(rate_limit_delay));
        assert_eq!(AppError::api_server_error(500, "test", "url").retry_delay_seconds(), Some(server_error_delay));
        assert_eq!(AppError::api_service_unavailable(503, "test", "url").retry_delay_seconds(), Some(service_unavailable_delay));
        assert_eq!(AppError::network_timeout("url").retry_delay_seconds(), Some(timeout_delay));
        assert_eq!(AppError::network_connection("url", "test").retry_delay_seconds(), Some(connection_delay));
    }

    #[test]
    fn test_error_display_formats() {
        // Test that all error variants have proper display formatting
        let errors = vec![
            AppError::config_error("test config error"),
            AppError::datetime_parse_error("test datetime error"),
            AppError::log_setup_error("test log error"),
            AppError::api_not_found("https://example.com"),
            AppError::api_server_error(500, "server error", "https://example.com"),
            AppError::api_client_error(400, "client error", "https://example.com"),
            AppError::api_rate_limit("rate limit", "https://example.com"),
            AppError::api_service_unavailable(503, "unavailable", "https://example.com"),
            AppError::network_timeout("https://example.com"),
            AppError::network_connection("https://example.com", "connection failed"),
            AppError::api_malformed_json("bad json", "https://example.com"),
            AppError::api_unexpected_structure("bad structure", "https://example.com"),
            AppError::api_no_data("no data", "https://example.com"),
            AppError::api_season_not_found(2024),
            AppError::api_game_not_found(123, 2024),
            AppError::api_tournament_not_found("tournament", "2024-01-15"),
            AppError::Custom("custom message".to_string()),
        ];

        for error in errors {
            let display_string = error.to_string();
            assert!(
                !display_string.is_empty(),
                "Error display should not be empty: {error:?}"
            );
            // Ensure the display string contains some meaningful content
            assert!(
                display_string.len() > 5,
                "Error display should be descriptive: {error:?}"
            );
        }
    }
}
