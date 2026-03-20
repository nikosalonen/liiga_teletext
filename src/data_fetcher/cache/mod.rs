mod core;
pub mod detailed_game_cache;
pub mod goal_events_cache;
pub mod player_cache;
pub mod tournament_cache;
pub mod ttl_cache;
pub mod types;

use std::sync::LazyLock;
use std::time::Duration;

use ttl_cache::TtlCache;

// Re-export cache types
// Re-export player cache functions
pub use player_cache::*;
// Re-export tournament cache functions
pub use tournament_cache::*;
// Re-export detailed game cache functions
pub use detailed_game_cache::*;
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
