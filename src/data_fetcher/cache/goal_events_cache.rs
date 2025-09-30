//! Goal events cache operations with LRU caching and TTL support

use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::LazyLock;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

use super::types::CachedGoalEventsData;
use crate::data_fetcher::models::GoalEventData;

// LRU cache structure for processed goal events to avoid reprocessing
pub static GOAL_EVENTS_CACHE: LazyLock<RwLock<LruCache<String, CachedGoalEventsData>>> =
    LazyLock::new(|| RwLock::new(LruCache::new(NonZeroUsize::new(300).unwrap())));

/// Creates a cache key for goal events data
pub fn create_goal_events_key(season: i32, game_id: i32) -> String {
    format!("goal_events_{season}_{game_id}")
}

/// Caches processed goal events data
#[instrument(skip(season, game_id, data), fields(season = %season, game_id = %game_id))]
pub async fn cache_goal_events_data(
    season: i32,
    game_id: i32,
    data: Vec<GoalEventData>,
    is_live_game: bool,
) {
    let key = create_goal_events_key(season, game_id);
    let event_count = data.len();
    debug!(
        "Caching goal events data: key={}, event_count={}, is_live_game={}",
        key, event_count, is_live_game
    );

    let cached_data = CachedGoalEventsData::new(data, game_id, season, is_live_game);
    let mut cache = GOAL_EVENTS_CACHE.write().await;
    cache.put(key.clone(), cached_data);

    info!(
        "Successfully cached goal events data: key={}, event_count={}, is_live_game={}",
        key, event_count, is_live_game
    );
}

/// Retrieves cached goal events data if it's not expired
#[instrument(skip(season, game_id), fields(season = %season, game_id = %game_id))]
pub async fn get_cached_goal_events_data(season: i32, game_id: i32) -> Option<Vec<GoalEventData>> {
    let key = create_goal_events_key(season, game_id);
    debug!(
        "Attempting to retrieve goal events data from cache: key={}",
        key
    );

    let mut cache = GOAL_EVENTS_CACHE.write().await;

    if let Some(cached_entry) = cache.get(&key) {
        debug!("Found cached goal events data: key={}", key);

        if !cached_entry.is_expired() {
            let event_count = cached_entry.data.len();
            debug!(
                "Cache hit for goal events data: key={}, event_count={}, age={:?}",
                key,
                event_count,
                cached_entry.cached_at.elapsed()
            );
            return Some(cached_entry.data.clone());
        } else {
            // Remove expired entry
            warn!(
                "Removing expired goal events cache entry: key={}, age={:?}, ttl={:?}",
                key,
                cached_entry.cached_at.elapsed(),
                cached_entry.get_ttl()
            );
            cache.pop(&key);
        }
    } else {
        debug!("Cache miss for goal events data: key={}", key);
    }

    None
}

/// Retrieves the full cached goal events entry structure for metadata access
#[instrument(skip(season, game_id), fields(season = %season, game_id = %game_id))]
#[allow(dead_code)]
pub async fn get_cached_goal_events_entry(
    season: i32,
    game_id: i32,
) -> Option<CachedGoalEventsData> {
    let key = create_goal_events_key(season, game_id);
    debug!(
        "Attempting to retrieve goal events entry from cache: key={}",
        key
    );

    let mut cache = GOAL_EVENTS_CACHE.write().await;

    if let Some(cached_entry) = cache.get(&key) {
        debug!("Found cached goal events entry: key={}", key);

        if !cached_entry.is_expired() {
            let event_count = cached_entry.data.len();
            debug!(
                "Cache hit for goal events entry: key={}, event_count={}, age={:?}, was_cleared={}, last_known_score={:?}",
                key,
                event_count,
                cached_entry.cached_at.elapsed(),
                cached_entry.was_cleared,
                cached_entry.last_known_score
            );
            return Some(cached_entry.clone());
        } else {
            // Remove expired entry
            warn!(
                "Removing expired goal events cache entry: key={}, age={:?}, ttl={:?}",
                key,
                cached_entry.cached_at.elapsed(),
                cached_entry.get_ttl()
            );
            cache.pop(&key);
        }
    } else {
        debug!("Cache miss for goal events entry: key={}", key);
    }

    None
}

/// Gets the current goal events cache size for monitoring purposes
#[allow(dead_code)]
pub async fn get_goal_events_cache_size() -> usize {
    GOAL_EVENTS_CACHE.read().await.len()
}

/// Gets the goal events cache capacity for monitoring purposes
#[allow(dead_code)]
pub async fn get_goal_events_cache_capacity() -> usize {
    GOAL_EVENTS_CACHE.read().await.cap().get()
}

/// Clears all goal events cache entries
#[allow(dead_code)]
pub async fn clear_goal_events_cache() {
    GOAL_EVENTS_CACHE.write().await.clear();
}

/// Clears goal events cache for a specific game
#[allow(dead_code)]
pub async fn clear_goal_events_cache_for_game(season: i32, game_id: i32) {
    let key = create_goal_events_key(season, game_id);
    let mut cache = GOAL_EVENTS_CACHE.write().await;

    // Get the current cached data to extract the last known score and live-state
    let (last_known_score, was_live) = if let Some(cached_entry) = cache.get(&key) {
        // Extract the last known score from the cached goal events
        let score = cached_entry.data.last().map(|last_event| {
            format!(
                "{}-{}",
                last_event.home_team_score, last_event.away_team_score
            )
        });
        (score, cached_entry.is_live_game)
    } else {
        (None, true)
    };

    // Remove the current entry
    cache.pop(&key);

    // If we had a last known score, create a cleared cache entry with that score
    if let Some(score) = last_known_score {
        let mut cleared_entry =
            CachedGoalEventsData::new_cleared(game_id, season, score.clone(), was_live);
        // keep the previous live-state
        cleared_entry.is_live_game = was_live;
        cache.put(key, cleared_entry);
        debug!(
            "Cleared goal events cache for game: season={}, game_id={}, last_known_score={}",
            season, game_id, score
        );
    } else {
        debug!(
            "Cleared goal events cache for game: season={}, game_id={} (no previous score)",
            season, game_id
        );
    }
}
