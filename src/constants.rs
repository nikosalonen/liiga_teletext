//! Application-wide constants and configuration values
//!
//! This module centralizes all magic numbers and configuration constants
//! to improve maintainability and make the codebase more configurable.

#![allow(dead_code)]

/// Default timeout for HTTP requests in seconds
pub const DEFAULT_HTTP_TIMEOUT_SECONDS: u64 = 30;

/// Maximum number of connections per host in the HTTP client pool
pub const HTTP_POOL_MAX_IDLE_PER_HOST: usize = 100;

/// Cache TTL (Time To Live) values in seconds
pub mod cache_ttl {
    /// TTL for live games (reduced from 15 to 8 seconds to ensure fresh goal events)
    /// This should be shorter than the auto-refresh interval (15s) to prevent stale data
    pub const LIVE_GAMES_SECONDS: u64 = 8;

    /// TTL for completed games (1 hour)
    pub const COMPLETED_GAMES_SECONDS: u64 = 3600;

    /// TTL for games that should be starting soon (increased from 5 to 30 seconds to reduce API calls)
    /// This should still catch the moment games become live but with less aggressive polling
    pub const STARTING_GAMES_SECONDS: u64 = 30;

    /// TTL for player data (24 hours)
    pub const PLAYER_DATA_SECONDS: u64 = 86400;

    /// Default TTL for HTTP responses (5 minutes). Note: Actual TTL is determined dynamically
    /// based on URL type and game state in the fetch function.
    pub const HTTP_RESPONSE_SECONDS: u64 = 300;
}

/// UI polling intervals in milliseconds
pub mod polling {
    /// Polling interval for active use (< 5 seconds idle)
    pub const ACTIVE_MS: u64 = 50;

    /// Polling interval for semi-active use (5-30 seconds idle)
    pub const SEMI_ACTIVE_MS: u64 = 200;

    /// Polling interval for idle use (> 30 seconds idle)
    pub const IDLE_MS: u64 = 500;

    /// Threshold for considering user as idle (seconds)
    pub const IDLE_THRESHOLD_SECONDS: u64 = 30;

    /// Threshold for considering user as semi-active (seconds)
    pub const SEMI_ACTIVE_THRESHOLD_SECONDS: u64 = 5;
}

/// Tournament season constants for month-based logic
pub mod tournament {
    /// Preseason start month (May)
    pub const PRESEASON_START_MONTH: u32 = 5;

    /// Preseason end month (September)
    pub const PRESEASON_END_MONTH: u32 = 9;

    /// Playoffs start month (March)
    pub const PLAYOFFS_START_MONTH: u32 = 3;

    /// Playoffs end month (June)
    pub const PLAYOFFS_END_MONTH: u32 = 6;
}

/// UI layout constants
pub mod ui {
    /// Offset for away team display
    pub const AWAY_TEAM_OFFSET: usize = 25;

    /// Position for separator display
    pub const SEPARATOR_OFFSET: usize = 23;

    /// Content margin from terminal border
    pub const CONTENT_MARGIN: usize = 2;

    /// Maximum lines per page before pagination
    pub const MAX_LINES_PER_PAGE: usize = 20;
}

/// Environment variable names
pub mod env_vars {
    /// Environment variable for API domain override
    pub const API_DOMAIN: &str = "LIIGA_API_DOMAIN";

    /// Environment variable for log file path override
    pub const LOG_FILE: &str = "LIIGA_LOG_FILE";

    /// Environment variable for debug mode
    pub const DEBUG_MODE: &str = "LIIGA_DEBUG";

    /// Environment variable for cache size override
    pub const CACHE_SIZE: &str = "LIIGA_CACHE_SIZE";

    /// Environment variable for API fetch timeout in seconds (default: 5)
    /// Used for fallback player name fetching when cached names are missing
    pub const API_FETCH_TIMEOUT: &str = "LIIGA_API_FETCH_TIMEOUT";
}

/// Retry configuration
pub mod retry {
    /// Maximum number of retry attempts for API calls
    pub const MAX_ATTEMPTS: u32 = 3;

    /// Base delay for exponential backoff (milliseconds)
    pub const BASE_DELAY_MS: u64 = 1000;

    /// Maximum delay between retries (seconds)
    pub const MAX_DELAY_SECONDS: u64 = 30;

    /// Retry delay for rate limit errors (seconds)
    pub const RATE_LIMIT_DELAY_SECONDS: u64 = 60;

    /// Retry delay for server errors (seconds)
    pub const SERVER_ERROR_DELAY_SECONDS: u64 = 5;

    /// Retry delay for service unavailable errors (seconds)
    pub const SERVICE_UNAVAILABLE_DELAY_SECONDS: u64 = 30;

    /// Retry delay for network timeout errors (seconds)
    pub const NETWORK_TIMEOUT_DELAY_SECONDS: u64 = 2;

    /// Retry delay for network connection errors (seconds)
    pub const NETWORK_CONNECTION_DELAY_SECONDS: u64 = 10;
}

/// Player name background resolution tuning
pub mod player_name_fetch {
    /// If more than this many player names are missing for a game, defer to
    /// the background queue instead of fetching synchronously.
    pub const SYNC_FETCH_THRESHOLD: usize = 3;

    /// Minimum spacing between background detailed player fetches (milliseconds).
    /// Keeps overall request rate low at startup to avoid 429s.
    pub const MIN_SPACING_MS: u64 = 450;

    /// Jitter range applied to spacing (+/- percentage of MIN_SPACING_MS).
    /// Use 20% to avoid thundering herd across clients.
    pub const JITTER_FRACTION: f64 = 0.2;

    /// Background queue capacity for player name resolution jobs.
    pub const QUEUE_CAPACITY: usize = 256;
}

// Re-export commonly used validation constants at the module level for convenience
#[allow(unused_imports)]
pub use validation::MAX_PLAYER_NAME_LENGTH;

/// Validation limits
pub mod validation {
    /// Maximum reasonable game time in minutes
    pub const MAX_GAME_TIME_MINUTES: i32 = 200;

    /// Minimum reasonable game time in minutes
    pub const MIN_GAME_TIME_MINUTES: i32 = 0;

    /// Maximum reasonable score for a single team
    pub const MAX_TEAM_SCORE: i32 = 50;

    /// Maximum length for team names
    pub const MAX_TEAM_NAME_LENGTH: usize = 50;

    /// Maximum length for player names
    pub const MAX_PLAYER_NAME_LENGTH: usize = 100;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttl_constants_are_reasonable() {
        // Test that TTL constants make sense for rate limiting
        let live = cache_ttl::LIVE_GAMES_SECONDS;
        let starting = cache_ttl::STARTING_GAMES_SECONDS;
        let completed = cache_ttl::COMPLETED_GAMES_SECONDS;
        let player = cache_ttl::PLAYER_DATA_SECONDS;
        let http = cache_ttl::HTTP_RESPONSE_SECONDS;

        // With rate limiting protection, starting games can have longer TTL than live games
        // to reduce API calls while still catching the moment games become live
        assert!(starting >= live); // Starting games TTL >= live games TTL
        // Live games should have shorter TTL than completed games
        assert!(live < completed);
        // Completed games should have shorter TTL than player data
        assert!(completed < player);
        // HTTP responses should have some reasonable TTL
        assert!(http > 0);

        // Ensure starting games TTL is reasonable (not too short, not too long)
        assert!(starting >= 15); // At least 15 seconds (same as live games)
        assert!(starting <= 60); // At most 60 seconds (increased from 30)
    }

    #[test]
    fn test_polling_constants_are_reasonable() {
        // Test that polling intervals make sense
        let active = polling::ACTIVE_MS;
        let semi_active = polling::SEMI_ACTIVE_MS;
        let idle = polling::IDLE_MS;

        // Active should be fastest, idle should be slowest
        assert!(active < semi_active);
        assert!(semi_active < idle);

        // Thresholds should make sense
        let semi_threshold = polling::SEMI_ACTIVE_THRESHOLD_SECONDS;
        let idle_threshold = polling::IDLE_THRESHOLD_SECONDS;
        assert!(semi_threshold < idle_threshold);
    }

    #[test]
    fn test_tournament_constants_are_valid_months() {
        // Ensure tournament months are valid (1-12)
        assert!((1..=12).contains(&tournament::PRESEASON_START_MONTH));
        assert!((1..=12).contains(&tournament::PRESEASON_END_MONTH));
        assert!((1..=12).contains(&tournament::PLAYOFFS_START_MONTH));
        assert!((1..=12).contains(&tournament::PLAYOFFS_END_MONTH));
    }

    #[test]
    fn test_ui_constants_are_reasonable() {
        // Ensure UI constants make sense by checking at runtime
        let away_offset = ui::AWAY_TEAM_OFFSET;
        let separator_offset = ui::SEPARATOR_OFFSET;
        let margin = ui::CONTENT_MARGIN;
        let max_lines = ui::MAX_LINES_PER_PAGE;

        assert!(away_offset > separator_offset);
        assert!(margin > 0);
        assert!(max_lines > 5);
    }

    #[test]
    fn test_validation_constants_are_reasonable() {
        // Ensure validation limits make sense by checking at runtime
        let max_time = validation::MAX_GAME_TIME_MINUTES;
        let min_time = validation::MIN_GAME_TIME_MINUTES;
        let max_score = validation::MAX_TEAM_SCORE;
        let max_team_name = validation::MAX_TEAM_NAME_LENGTH;
        let max_player_name = MAX_PLAYER_NAME_LENGTH;

        assert!(max_time > min_time);
        assert!(max_score > 0);
        assert!(max_team_name > 0);
        assert!(max_player_name > 0);
    }

    #[test]
    fn test_retry_constants_are_reasonable() {
        // Ensure retry configuration is reasonable by checking at runtime
        let max_attempts = retry::MAX_ATTEMPTS;
        let base_delay = retry::BASE_DELAY_MS;
        let max_delay = retry::MAX_DELAY_SECONDS;

        assert!(max_attempts > 0);
        assert!(base_delay > 0);
        assert!(max_delay > 0);

        // Test specific retry delay constants
        let rate_limit_delay = retry::RATE_LIMIT_DELAY_SECONDS;
        let server_error_delay = retry::SERVER_ERROR_DELAY_SECONDS;
        let service_unavailable_delay = retry::SERVICE_UNAVAILABLE_DELAY_SECONDS;
        let timeout_delay = retry::NETWORK_TIMEOUT_DELAY_SECONDS;
        let connection_delay = retry::NETWORK_CONNECTION_DELAY_SECONDS;

        // All delays should be positive
        assert!(rate_limit_delay > 0);
        assert!(server_error_delay > 0);
        assert!(service_unavailable_delay > 0);
        assert!(timeout_delay > 0);
        assert!(connection_delay > 0);

        // Rate limit delay should be the longest (most severe)
        assert!(rate_limit_delay >= service_unavailable_delay);
        assert!(rate_limit_delay >= connection_delay);
        assert!(rate_limit_delay >= server_error_delay);
        assert!(rate_limit_delay >= timeout_delay);

        // Timeout delay should be the shortest (least severe)
        assert!(timeout_delay <= server_error_delay);
        assert!(timeout_delay <= connection_delay);
        assert!(timeout_delay <= service_unavailable_delay);
    }

    #[test]
    fn test_env_var_names_are_not_empty() {
        // Ensure environment variable names are not empty by checking at runtime
        let api_domain = env_vars::API_DOMAIN;
        let log_file = env_vars::LOG_FILE;
        let debug_mode = env_vars::DEBUG_MODE;
        let cache_size = env_vars::CACHE_SIZE;

        assert!(!api_domain.is_empty());
        assert!(!log_file.is_empty());
        assert!(!debug_mode.is_empty());
        assert!(!cache_size.is_empty());
    }
}
