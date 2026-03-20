mod core;
pub mod goal_events_cache;
pub mod player_cache;
pub mod tournament_cache;
pub mod ttl_cache;
pub mod types;

use std::sync::LazyLock;
use std::time::Duration;

use ttl_cache::TtlCache;

use crate::constants::cache_ttl;
use crate::data_fetcher::models::DetailedGameResponse;

// Re-export cache types
// Re-export player cache functions
pub use player_cache::*;
// Re-export tournament cache functions
pub use tournament_cache::*;
// Re-export goal events cache functions
pub use goal_events_cache::*;
// Re-export core cache functions
pub use core::*;

// --- HTTP response cache (backed by generic TtlCache) ---

pub static HTTP_RESPONSE_CACHE: LazyLock<TtlCache<String, String>> =
    LazyLock::new(|| TtlCache::new(100));

/// Cache an HTTP response with a specific TTL.
pub async fn cache_http_response(url: String, data: String, ttl_seconds: u64) {
    HTTP_RESPONSE_CACHE
        .insert(url, data, Duration::from_secs(ttl_seconds))
        .await;
}

/// Get a cached HTTP response if not expired.
pub async fn get_cached_http_response(url: &str) -> Option<String> {
    HTTP_RESPONSE_CACHE.get(&url.to_string()).await
}

/// Clear all HTTP response cache entries.
#[allow(dead_code)]
pub async fn clear_http_response_cache() {
    HTTP_RESPONSE_CACHE.clear().await;
}

// --- Detailed game cache (backed by generic TtlCache) ---

pub static DETAILED_GAME_CACHE: LazyLock<TtlCache<String, DetailedGameResponse>> =
    LazyLock::new(|| TtlCache::new(200));

/// Compute TTL based on whether a game is live or completed.
///
/// Reusable by other cache wrapper functions in this module.
pub(super) fn game_state_ttl(is_live: bool) -> Duration {
    if is_live {
        Duration::from_secs(cache_ttl::LIVE_GAMES_SECONDS)
    } else {
        Duration::from_secs(cache_ttl::COMPLETED_GAMES_SECONDS)
    }
}

/// Creates a cache key for detailed game data.
pub fn create_detailed_game_key(season: i32, game_id: i32) -> String {
    format!("detailed_game_{season}_{game_id}")
}

/// Caches detailed game data with a TTL that depends on game liveness.
pub async fn cache_detailed_game_data(
    season: i32,
    game_id: i32,
    data: DetailedGameResponse,
    is_live_game: bool,
) {
    let key = create_detailed_game_key(season, game_id);
    DETAILED_GAME_CACHE
        .insert(key, data, game_state_ttl(is_live_game))
        .await;
}

/// Retrieves cached detailed game data if it has not expired.
pub async fn get_cached_detailed_game_data(
    season: i32,
    game_id: i32,
) -> Option<DetailedGameResponse> {
    let key = create_detailed_game_key(season, game_id);
    DETAILED_GAME_CACHE.get(&key).await
}

/// Gets the current detailed game cache size for monitoring purposes.
#[allow(dead_code)]
pub async fn get_detailed_game_cache_size() -> usize {
    DETAILED_GAME_CACHE.len().await
}

/// Clears all detailed game cache entries.
#[allow(dead_code)]
pub async fn clear_detailed_game_cache() {
    DETAILED_GAME_CACHE.clear().await;
}
