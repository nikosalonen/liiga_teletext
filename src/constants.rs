//! Application-wide constants and configuration values
//!
//! This module centralizes all magic numbers and configuration constants
//! to improve maintainability and make the codebase more configurable.

/// Default timeout for HTTP requests in seconds
pub const DEFAULT_HTTP_TIMEOUT_SECONDS: u64 = 10;

/// Default connect timeout for HTTP requests in seconds.
/// Shorter than the overall request timeout to quickly detect unreachable hosts.
pub const DEFAULT_HTTP_CONNECT_TIMEOUT_SECONDS: u64 = 5;

/// Maximum number of connections per host in the HTTP client pool
pub const HTTP_POOL_MAX_IDLE_PER_HOST: usize = 100;

/// Cache TTL (Time To Live) values in seconds
pub mod cache_ttl {
    /// TTL for live games - set to match auto-refresh interval to prevent cache expiration
    /// between refresh cycles, which causes flickering and inconsistent data display
    pub const LIVE_GAMES_SECONDS: u64 = 15;

    /// TTL for completed games (1 hour)
    pub const COMPLETED_GAMES_SECONDS: u64 = 3600;

    /// TTL for games that should be starting soon (increased from 5 to 30 seconds to reduce API calls)
    /// This should still catch the moment games become live but with less aggressive polling
    pub const STARTING_GAMES_SECONDS: u64 = 30;
}

/// Auto-refresh interval constants in seconds
pub mod refresh {
    /// Auto-refresh interval for live games.
    /// Kept in sync with `cache_ttl::LIVE_GAMES_SECONDS` to prevent cache expiration
    /// between refresh cycles.
    pub const LIVE_GAMES_INTERVAL_SECONDS: u64 = super::cache_ttl::LIVE_GAMES_SECONDS;
}

/// Teletext color constants
pub mod colors {
    use crossterm::style::Color;

    /// Teletext white color
    pub const TELETEXT_WHITE: Color = Color::AnsiValue(231);

    /// Teletext cyan color
    pub const TELETEXT_CYAN: Color = Color::AnsiValue(51);

    /// Teletext green color
    pub const TELETEXT_GREEN: Color = Color::AnsiValue(46);

    /// Teletext yellow color
    pub const TELETEXT_YELLOW: Color = Color::AnsiValue(226);

    /// Teletext red color
    pub const TELETEXT_RED: Color = Color::AnsiValue(196);
}

/// Environment variable names
pub mod env_vars {
    /// Environment variable for API fetch timeout in seconds (default: 5)
    /// Used for fallback player name fetching when cached names are missing
    pub const API_FETCH_TIMEOUT: &str = "LIIGA_API_FETCH_TIMEOUT";
}

/// Retry configuration
pub mod retry {
    /// Maximum number of retry attempts for API calls
    pub const MAX_ATTEMPTS: u32 = 3;

    /// Initial backoff delay for HTTP fetch retries (milliseconds)
    pub const INITIAL_BACKOFF_MS: u64 = 250;
}

/// Maximum length for player names
// Used by integration tests (tests/disambiguation_display_tests.rs)
#[allow(dead_code)]
pub const MAX_PLAYER_NAME_LENGTH: usize = 100;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttl_constants_are_reasonable() {
        let live = cache_ttl::LIVE_GAMES_SECONDS;
        let starting = cache_ttl::STARTING_GAMES_SECONDS;
        let completed = cache_ttl::COMPLETED_GAMES_SECONDS;

        // With rate limiting protection, starting games can have longer TTL than live games
        // to reduce API calls while still catching the moment games become live
        assert!(starting >= live); // Starting games TTL >= live games TTL
        // Live games should have shorter TTL than completed games
        assert!(live < completed);

        // Ensure starting games TTL is reasonable (not too short, not too long)
        assert!(starting >= 15); // At least 15 seconds (same as live games)
        assert!(starting <= 60); // At most 60 seconds (increased from 30)
    }

    #[test]
    fn test_retry_constants_are_reasonable() {
        let max_attempts = retry::MAX_ATTEMPTS;
        assert!(max_attempts > 0);

        let initial_backoff = retry::INITIAL_BACKOFF_MS;
        assert!(initial_backoff > 0);
    }

    #[test]
    fn test_env_var_names_are_not_empty() {
        assert!(!env_vars::API_FETCH_TIMEOUT.is_empty());
    }

    #[test]
    fn test_max_player_name_length_is_reasonable() {
        const { assert!(MAX_PLAYER_NAME_LENGTH > 0) };
    }
}
