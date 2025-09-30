//! Detailed game cache operations with LRU caching and TTL support

use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::LazyLock;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

use super::types::CachedDetailedGameData;
use crate::data_fetcher::models::DetailedGameResponse;

// LRU cache structure for detailed game responses to avoid repeated API calls
pub static DETAILED_GAME_CACHE: LazyLock<RwLock<LruCache<String, CachedDetailedGameData>>> =
    LazyLock::new(|| RwLock::new(LruCache::new(NonZeroUsize::new(200).unwrap())));

/// Creates a cache key for detailed game data
pub fn create_detailed_game_key(season: i32, game_id: i32) -> String {
    format!("detailed_game_{season}_{game_id}")
}

/// Caches detailed game data with automatic live game detection
#[instrument(skip(season, game_id, data), fields(season = %season, game_id = %game_id))]
pub async fn cache_detailed_game_data(
    season: i32,
    game_id: i32,
    data: DetailedGameResponse,
    is_live_game: bool,
) {
    let key = create_detailed_game_key(season, game_id);
    debug!(
        "Caching detailed game data: key={}, is_live={}",
        key, is_live_game
    );

    let cached_data = CachedDetailedGameData::new(data, is_live_game);
    let mut cache = DETAILED_GAME_CACHE.write().await;
    cache.put(key.clone(), cached_data);

    info!(
        "Successfully cached detailed game data: key={}, is_live={}",
        key, is_live_game
    );
}

/// Retrieves cached detailed game data if it's not expired
#[instrument(skip(season, game_id), fields(season = %season, game_id = %game_id))]
pub async fn get_cached_detailed_game_data(
    season: i32,
    game_id: i32,
) -> Option<DetailedGameResponse> {
    let key = create_detailed_game_key(season, game_id);
    debug!(
        "Attempting to retrieve detailed game data from cache: key={}",
        key
    );

    let mut cache = DETAILED_GAME_CACHE.write().await;

    if let Some(cached_entry) = cache.get(&key) {
        debug!("Found cached detailed game data: key={key}");

        if !cached_entry.is_expired() {
            let is_live = cached_entry.is_live_game;
            debug!(
                "Cache hit for detailed game data: key={}, is_live={}, age={:?}",
                key,
                is_live,
                cached_entry.cached_at.elapsed()
            );
            return Some(cached_entry.data.clone());
        } else {
            // Remove expired entry
            warn!(
                "Removing expired detailed game cache entry: key={}, age={:?}, ttl={:?}",
                key,
                cached_entry.cached_at.elapsed(),
                cached_entry.get_ttl()
            );
            cache.pop(&key);
        }
    } else {
        debug!("Cache miss for detailed game data: key={key}");
    }

    None
}

/// Gets the current detailed game cache size for monitoring purposes
#[allow(dead_code)]
pub async fn get_detailed_game_cache_size() -> usize {
    DETAILED_GAME_CACHE.read().await.len()
}

/// Gets the detailed game cache capacity for monitoring purposes
#[allow(dead_code)]
pub async fn get_detailed_game_cache_capacity() -> usize {
    DETAILED_GAME_CACHE.read().await.cap().get()
}

/// Clears all detailed game cache entries
#[allow(dead_code)]
pub async fn clear_detailed_game_cache() {
    DETAILED_GAME_CACHE.write().await.clear();
}
