//! Cache data structures with TTL support

use std::time::{Duration, Instant};
use tracing::debug;

use crate::constants::cache_ttl;
use crate::data_fetcher::models::ScheduleResponse;

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
}
