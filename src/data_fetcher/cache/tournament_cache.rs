//! Tournament cache operations with TTL and live game detection

use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

use crate::constants::cache_ttl;
use crate::data_fetcher::models::{GameData, ScheduleResponse};
use crate::teletext_ui::ScoreType;

use super::types::CachedTournamentData;

// LRU cache structure for tournament data with TTL support
pub static TOURNAMENT_CACHE: LazyLock<RwLock<LruCache<String, CachedTournamentData>>> =
    LazyLock::new(|| RwLock::new(LruCache::new(NonZeroUsize::new(50).unwrap())));

/// Determines if a ScheduleResponse contains live games
pub fn has_live_games(response: &ScheduleResponse) -> bool {
    response.games.iter().any(|game| {
        // Game is live if it's started but not ended
        game.started && !game.ended
    })
}

/// Caches tournament data with automatic live game detection
#[instrument(skip(key, data), fields(cache_key = %key))]
pub async fn cache_tournament_data(key: String, data: ScheduleResponse) {
    let games_count = data.games.len();
    let has_live = has_live_games(&data);

    debug!(
        "Caching tournament data: key={}, games={}, has_live={}",
        key, games_count, has_live
    );

    let cached_data = CachedTournamentData::new(data, has_live);

    let mut cache = TOURNAMENT_CACHE.write().await;
    cache.put(key.clone(), cached_data);

    // Enhanced logging for live game cache entries
    if has_live {
        info!(
            "Live game cache entry created: key={}, games={}, ttl={}s",
            key,
            games_count,
            cache_ttl::LIVE_GAMES_SECONDS
        );
    } else {
        info!(
            "Completed game cache entry created: key={}, games={}, ttl={}s",
            key,
            games_count,
            cache_ttl::COMPLETED_GAMES_SECONDS
        );
    }
}

/// Retrieves cached tournament data if it's not expired
#[instrument(skip(key), fields(cache_key = %key))]
#[allow(dead_code)]
pub async fn get_cached_tournament_data(key: &str) -> Option<ScheduleResponse> {
    debug!(
        "Attempting to retrieve tournament data from cache for key: {}",
        key
    );

    let mut cache = TOURNAMENT_CACHE.write().await;

    if let Some(cached_entry) = cache.get(key) {
        debug!("Found cached tournament data for key: {}", key);

        if !cached_entry.is_expired() {
            let games_count = cached_entry.data.games.len();
            let has_live = cached_entry.has_live_games;
            debug!(
                "Cache hit for tournament data: key={}, games={}, has_live={}, age={:?}",
                key,
                games_count,
                has_live,
                cached_entry.cached_at.elapsed()
            );
            return Some(cached_entry.data.clone());
        } else {
            // Enhanced logging for expired cache entries during auto-refresh
            let has_live = cached_entry.has_live_games;
            let age = cached_entry.cached_at.elapsed();
            let ttl = cached_entry.get_ttl();

            if has_live {
                info!(
                    "Cache bypass: Expired live game cache entry removed during auto-refresh: key={}, age={:?}, ttl={:?}",
                    key, age, ttl
                );
            } else {
                warn!(
                    "Removing expired tournament cache entry: key={}, age={:?}, ttl={:?}",
                    key, age, ttl
                );
            }
            cache.pop(key);
        }
    } else {
        debug!("Cache miss for tournament data: key={}", key);
    }

    None
}

/// Retrieves cached tournament data with custom live game state check
#[allow(dead_code)]
pub async fn get_cached_tournament_data_with_live_check(
    key: &str,
    current_games: &[GameData],
) -> Option<ScheduleResponse> {
    use super::has_live_games_from_game_data;
    
    let mut cache = TOURNAMENT_CACHE.write().await;

    if let Some(cached_entry) = cache.get(key) {
        // Check if we have live games in the current state
        let has_live = has_live_games_from_game_data(current_games);

        // If the live state has changed, consider the cache expired
        if cached_entry.has_live_games != has_live {
            debug!(
                "Cache invalidated due to live game state change: key={}, cached_has_live={}, current_has_live={}",
                key, cached_entry.has_live_games, has_live
            );
            cache.pop(key);
            return None;
        }

        if !cached_entry.is_expired() {
            return Some(cached_entry.data.clone());
        } else {
            // Remove expired entry
            debug!(
                "Removing expired cache entry during live game state check: key={}, age={:?}",
                key,
                cached_entry.cached_at.elapsed()
            );
            cache.pop(key);
        }
    }

    None
}

/// Retrieves cached tournament data specifically for auto-refresh scenarios
/// This function provides enhanced logging and ensures proper cache bypass for expired entries
#[allow(dead_code)]
pub async fn get_cached_tournament_data_for_auto_refresh(key: &str) -> Option<ScheduleResponse> {
    debug!(
        "Auto-refresh: Attempting to retrieve tournament data from cache for key: {}",
        key
    );

    let mut cache = TOURNAMENT_CACHE.write().await;

    if let Some(cached_entry) = cache.get(key) {
        let has_live = cached_entry.has_live_games;
        let age = cached_entry.cached_at.elapsed();
        let ttl = cached_entry.get_ttl();

        debug!(
            "Auto-refresh: Found cached tournament data: key={}, has_live={}, age={:?}, ttl={:?}",
            key, has_live, age, ttl
        );

        if !cached_entry.is_expired() {
            let games_count = cached_entry.data.games.len();
            info!(
                "Auto-refresh: Using cached data: key={}, games={}, has_live={}, age={:?}",
                key, games_count, has_live, age
            );
            return Some(cached_entry.data.clone());
        } else {
            // Enhanced logging for auto-refresh cache bypass
            if has_live {
                info!(
                    "Auto-refresh: Cache bypass for expired live game entry: key={}, age={:?}, ttl={:?} - fetching fresh data",
                    key, age, ttl
                );
            } else {
                info!(
                    "Auto-refresh: Cache bypass for expired completed game entry: key={}, age={:?}, ttl={:?}",
                    key, age, ttl
                );
            }
            cache.pop(key);
        }
    } else {
        debug!("Auto-refresh: Cache miss for tournament data: key={}", key);
    }

    None
}

/// Invalidates all tournament cache entries for a specific date
#[allow(dead_code)]
pub async fn invalidate_tournament_cache_for_date(date: &str) {
    let mut cache = TOURNAMENT_CACHE.write().await;

    // Remove all entries for this date
    let keys_to_remove: Vec<String> = cache
        .iter()
        .filter(|(key, _)| key.contains(date))
        .map(|(key, _)| key.clone())
        .collect();

    for key in keys_to_remove {
        cache.pop(&key);
    }
}

/// Aggressively invalidates cache for games that should be starting soon
/// This is called when we detect games are near their scheduled start time
#[allow(dead_code)]
pub async fn invalidate_cache_for_games_near_start_time(date: &str) {
    let mut cache = TOURNAMENT_CACHE.write().await;

    // Find and remove cache entries for the given date
    let keys_to_remove: Vec<String> = cache
        .iter()
        .filter(|(key, _)| key.contains(date))
        .map(|(key, _)| key.clone())
        .collect();

    for key in keys_to_remove {
        info!(
            "Aggressively invalidating cache for games near start time: {}",
            key
        );
        cache.pop(&key);
    }
}

/// Completely bypasses cache for games that should be starting soon
/// This ensures fresh data is always fetched for games near their start time
pub async fn should_bypass_cache_for_starting_games(current_games: &[GameData]) -> bool {
    // Check if any games are near their start time
    let has_starting_games = current_games.iter().any(|game| {
        if game.score_type != ScoreType::Scheduled || game.start.is_empty() {
            return false;
        }

        match chrono::DateTime::parse_from_rfc3339(&game.start) {
            Ok(game_start) => {
                let now = chrono::Utc::now();
                let time_diff = now.signed_duration_since(game_start);

                // Extended window: Check if game should start within the next 5 minutes or started within the last 10 minutes
                let is_near_start = time_diff >= chrono::Duration::minutes(-5)
                    && time_diff <= chrono::Duration::minutes(10);

                if is_near_start {
                    info!(
                        "Cache bypass triggered for game near start time: {} vs {} - start: {}, time_diff: {:?}",
                        game.home_team,
                        game.away_team,
                        game_start,
                        time_diff
                    );
                }

                is_near_start
            }
            Err(_) => false,
        }
    });

    if has_starting_games {
        debug!("Cache bypass enabled for games near start time");
    }

    has_starting_games
}

/// Enhanced cache expiration check that considers games that might be starting
pub async fn get_cached_tournament_data_with_start_check(
    key: &str,
    current_games: &[GameData],
) -> Option<ScheduleResponse> {
    let mut cache = TOURNAMENT_CACHE.write().await;

    if let Some(cached_entry) = cache.get(key) {
        // Check if we have any games that might be starting
        let has_starting_games = current_games
            .iter()
            .any(|game| game.score_type == ScoreType::Scheduled && !game.start.is_empty());

        // If we have starting games, consider cache expired more aggressively
        if has_starting_games {
            let age = cached_entry.cached_at.elapsed();
            let aggressive_ttl = Duration::from_secs(cache_ttl::STARTING_GAMES_SECONDS); // 10 seconds for starting games

            if age > aggressive_ttl {
                info!(
                    "Cache expired for starting games: key={}, age={:?}, aggressive_ttl={:?}",
                    key, age, aggressive_ttl
                );
                cache.pop(key);
                return None;
            }
        }

        if !cached_entry.is_expired() {
            return Some(cached_entry.data.clone());
        } else {
            cache.pop(key);
        }
    }

    None
}

/// Gets the current tournament cache size for monitoring purposes
#[allow(dead_code)]
pub async fn get_tournament_cache_size() -> usize {
    TOURNAMENT_CACHE.read().await.len()
}

/// Gets the tournament cache capacity for monitoring purposes
#[allow(dead_code)]
pub async fn get_tournament_cache_capacity() -> usize {
    TOURNAMENT_CACHE.read().await.cap().get()
}

/// Clears all tournament cache entries
#[allow(dead_code)]
pub async fn clear_tournament_cache() {
    TOURNAMENT_CACHE.write().await.clear();
}