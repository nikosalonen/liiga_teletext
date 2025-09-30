//! Cache data structures with TTL support

use std::time::{Duration, Instant};
use tracing::debug;

use crate::constants::cache_ttl;
use crate::data_fetcher::models::{DetailedGameResponse, GoalEventData, ScheduleResponse};

/// Cached tournament data with TTL support
#[derive(Debug, Clone)]
pub struct CachedTournamentData {
    pub data: ScheduleResponse,
    pub cached_at: Instant,
    pub has_live_games: bool,
}

impl CachedTournamentData {
    /// Creates a new cached tournament data entry
    pub fn new(data: ScheduleResponse, has_live_games: bool) -> Self {
        Self {
            data,
            cached_at: Instant::now(),
            has_live_games,
        }
    }

    /// Checks if the cached data is expired based on game state
    pub fn is_expired(&self) -> bool {
        let ttl = if self.has_live_games {
            Duration::from_secs(cache_ttl::LIVE_GAMES_SECONDS) // 15 seconds for live games
        } else {
            Duration::from_secs(cache_ttl::COMPLETED_GAMES_SECONDS) // 1 hour for completed games
        };

        let age = self.cached_at.elapsed();
        let is_expired = age > ttl;

        debug!(
            "Cache expiration check: has_live_games={}, age={:?}, ttl={:?}, is_expired={}",
            self.has_live_games, age, ttl, is_expired
        );

        is_expired
    }

    /// Gets the TTL duration for this cache entry
    pub fn get_ttl(&self) -> Duration {
        if self.has_live_games {
            Duration::from_secs(cache_ttl::LIVE_GAMES_SECONDS)
        } else {
            Duration::from_secs(cache_ttl::COMPLETED_GAMES_SECONDS)
        }
    }

    /// Gets the remaining time until expiration
    #[allow(dead_code)]
    pub fn time_until_expiry(&self) -> Duration {
        let ttl = self.get_ttl();
        let elapsed = self.cached_at.elapsed();
        ttl.saturating_sub(elapsed)
    }
}

/// Cached detailed game data with TTL support
#[derive(Debug, Clone)]
pub struct CachedDetailedGameData {
    pub data: DetailedGameResponse,
    pub cached_at: Instant,
    pub is_live_game: bool,
}

impl CachedDetailedGameData {
    /// Creates a new cached detailed game data entry
    pub fn new(data: DetailedGameResponse, is_live_game: bool) -> Self {
        Self {
            data,
            cached_at: Instant::now(),
            is_live_game,
        }
    }

    /// Checks if the cached data is expired based on game state
    pub fn is_expired(&self) -> bool {
        let ttl = if self.is_live_game {
            Duration::from_secs(cache_ttl::LIVE_GAMES_SECONDS) // 30 seconds for live games
        } else {
            Duration::from_secs(cache_ttl::COMPLETED_GAMES_SECONDS) // 1 hour for completed games
        };

        self.cached_at.elapsed() > ttl
    }

    /// Gets the TTL duration for this cache entry
    pub fn get_ttl(&self) -> Duration {
        if self.is_live_game {
            Duration::from_secs(cache_ttl::LIVE_GAMES_SECONDS)
        } else {
            Duration::from_secs(cache_ttl::COMPLETED_GAMES_SECONDS)
        }
    }
}

/// Cached goal events data with TTL support
#[derive(Debug, Clone)]
pub struct CachedGoalEventsData {
    pub data: Vec<GoalEventData>,
    pub cached_at: Instant,
    pub game_id: i32,
    pub season: i32,
    pub is_live_game: bool,
    #[allow(dead_code)]
    pub last_known_score: Option<String>, // Store the last known score when cache was cleared
    #[allow(dead_code)]
    pub was_cleared: bool, // Flag to indicate if cache was intentionally cleared
}

impl CachedGoalEventsData {
    /// Creates a new cached goal events data entry
    pub fn new(data: Vec<GoalEventData>, game_id: i32, season: i32, is_live_game: bool) -> Self {
        Self {
            data,
            cached_at: Instant::now(),
            game_id,
            season,
            is_live_game,
            last_known_score: None,
            was_cleared: false,
        }
    }

    pub fn new_cleared(
        game_id: i32,
        season: i32,
        last_known_score: String,
        is_live_game: bool,
    ) -> Self {
        Self {
            data: Vec::new(),
            cached_at: Instant::now(),
            game_id,
            season,

            is_live_game,
            last_known_score: Some(last_known_score),
            was_cleared: true,
        }
    }

    /// Checks if the cached data is expired based on game state
    pub fn is_expired(&self) -> bool {
        let ttl = if self.is_live_game {
            Duration::from_secs(cache_ttl::LIVE_GAMES_SECONDS) // 30 seconds for live games
        } else {
            Duration::from_secs(cache_ttl::COMPLETED_GAMES_SECONDS) // 1 hour for completed games
        };

        let age = self.cached_at.elapsed();
        let is_expired = age > ttl;

        debug!(
            "Goal events cache expiration check: is_live_game={}, age={:?}, ttl={:?}, is_expired={}",
            self.is_live_game, age, ttl, is_expired
        );

        is_expired
    }

    /// Gets the TTL duration for this cache entry
    #[allow(dead_code)]
    pub fn get_ttl(&self) -> Duration {
        if self.is_live_game {
            Duration::from_secs(cache_ttl::LIVE_GAMES_SECONDS)
        } else {
            Duration::from_secs(cache_ttl::COMPLETED_GAMES_SECONDS)
        }
    }

    /// Gets the game ID associated with this cached data (useful for debugging and logging)
    pub fn get_game_id(&self) -> i32 {
        self.game_id
    }

    /// Gets the season associated with this cached data (useful for debugging and logging)
    pub fn get_season(&self) -> i32 {
        self.season
    }

    /// Gets cache metadata including game ID and season for monitoring and debugging
    pub fn get_cache_info(&self) -> (i32, i32, usize, bool) {
        (
            self.game_id,
            self.season,
            self.data.len(),
            self.is_expired(),
        )
    }
}

/// Cached HTTP response with TTL support
#[derive(Debug, Clone)]
pub struct CachedHttpResponse {
    pub data: String,
    pub cached_at: Instant,
    pub ttl_seconds: u64,
}

impl CachedHttpResponse {
    /// Creates a new cached HTTP response entry
    pub fn new(data: String, ttl_seconds: u64) -> Self {
        Self {
            data,
            cached_at: Instant::now(),
            ttl_seconds,
        }
    }

    /// Checks if the cached data is expired
    pub fn is_expired(&self) -> bool {
        let ttl = Duration::from_secs(self.ttl_seconds);
        self.cached_at.elapsed() > ttl
    }
}
