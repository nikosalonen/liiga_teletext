use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

// Import cache types from sibling module
use super::types::CachedHttpResponse;

// LRU cache structure for HTTP responses with TTL support
pub static HTTP_RESPONSE_CACHE: LazyLock<RwLock<LruCache<String, CachedHttpResponse>>> =
    LazyLock::new(|| RwLock::new(LruCache::new(NonZeroUsize::new(100).unwrap())));

/// Caches HTTP response data with TTL
#[instrument(skip(url, data), fields(url = %url))]
pub async fn cache_http_response(url: String, data: String, ttl_seconds: u64) {
    let data_size = data.len();
    debug!(
        "Caching HTTP response: url={}, data_size={}, ttl={}s",
        url, data_size, ttl_seconds
    );

    let cached_data = CachedHttpResponse::new(data, ttl_seconds);
    let mut cache = HTTP_RESPONSE_CACHE.write().await;
    cache.put(url.clone(), cached_data);

    info!(
        "Successfully cached HTTP response: url={}, data_size={}, ttl={}s",
        url, data_size, ttl_seconds
    );
}

/// Retrieves cached HTTP response if it's not expired
#[instrument(skip(url), fields(url = %url))]
pub async fn get_cached_http_response(url: &str) -> Option<String> {
    debug!(
        "Attempting to retrieve HTTP response from cache: url={}",
        url
    );

    let mut cache = HTTP_RESPONSE_CACHE.write().await;

    if let Some(cached_entry) = cache.get(url) {
        debug!("Found cached HTTP response: url={}", url);

        if !cached_entry.is_expired() {
            let data_size = cached_entry.data.len();
            debug!(
                "Cache hit for HTTP response: url={}, data_size={}, age={:?}",
                url,
                data_size,
                cached_entry.cached_at.elapsed()
            );
            return Some(cached_entry.data.clone());
        } else {
            // Remove expired entry
            warn!(
                "Removing expired HTTP response cache entry: url={}, age={:?}, ttl={:?}",
                url,
                cached_entry.cached_at.elapsed(),
                Duration::from_secs(cached_entry.ttl_seconds)
            );
            cache.pop(url);
        }
    } else {
        debug!("Cache miss for HTTP response: url={}", url);
    }

    None
}

/// Gets the current HTTP response cache size for monitoring purposes
#[allow(dead_code)]
pub async fn get_http_response_cache_size() -> usize {
    HTTP_RESPONSE_CACHE.read().await.len()
}

/// Gets the HTTP response cache capacity for monitoring purposes
#[allow(dead_code)]
pub async fn get_http_response_cache_capacity() -> usize {
    HTTP_RESPONSE_CACHE.read().await.cap().get()
}

/// Clears all HTTP response cache entries
#[allow(dead_code)]
pub async fn clear_http_response_cache() {
    HTTP_RESPONSE_CACHE.write().await.clear();
}