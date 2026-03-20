//! Generic TTL-aware LRU cache wrapper
//!
//! Provides `TtlCache<K, V>`, a thread-safe cache that combines LRU eviction
//! with per-entry time-to-live expiration. Intended to replace the repetitive
//! per-type cache boilerplate throughout the codebase.

use std::hash::Hash;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use lru::LruCache;
use tokio::sync::RwLock;
use tracing::debug;

/// A single cache entry storing the value alongside its TTL metadata.
#[derive(Debug, Clone)]
struct CacheEntry<V> {
    data: V,
    cached_at: Instant,
    ttl: Duration,
}

impl<V> CacheEntry<V> {
    fn is_expired(&self) -> bool {
        self.cached_at.elapsed() > self.ttl
    }
}

/// A thread-safe LRU cache where each entry carries its own TTL.
///
/// - Expired entries are lazily removed on access (`get`, `get_if`).
/// - LRU eviction kicks in when the cache exceeds its capacity.
/// - All public methods are `async` and use `tokio::sync::RwLock` internally.
pub struct TtlCache<K: Eq + Hash, V> {
    inner: RwLock<LruCache<K, CacheEntry<V>>>,
}

impl<K: Eq + Hash + Clone, V: Clone> TtlCache<K, V> {
    /// Create a new cache with the given maximum capacity.
    ///
    /// # Panics
    /// Panics if `capacity` is 0.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: RwLock::new(LruCache::new(
                NonZeroUsize::new(capacity).expect("cache capacity must be > 0"),
            )),
        }
    }

    /// Retrieve a value if it exists and has not expired.
    ///
    /// Takes a write lock so that expired entries can be evicted immediately.
    pub async fn get(&self, key: &K) -> Option<V> {
        let mut cache = self.inner.write().await;
        if let Some(entry) = cache.get(key) {
            if entry.is_expired() {
                debug!(
                    "TtlCache: evicting expired entry (age={:?}, ttl={:?})",
                    entry.cached_at.elapsed(),
                    entry.ttl
                );
                cache.pop(key);
                None
            } else {
                Some(entry.data.clone())
            }
        } else {
            None
        }
    }

    /// Insert a value with a specific TTL.
    pub async fn insert(&self, key: K, value: V, ttl: Duration) {
        let entry = CacheEntry {
            data: value,
            cached_at: Instant::now(),
            ttl,
        };
        let mut cache = self.inner.write().await;
        cache.put(key, entry);
    }

    /// Retrieve a value only if it is not expired **and** a custom freshness
    /// predicate passes. The predicate receives the `cached_at` timestamp.
    ///
    /// If the predicate returns `false`, the entry is removed from the cache.
    /// This supports use-cases like aggressive TTL for games about to start.
    pub async fn get_if(&self, key: &K, predicate: impl FnOnce(Instant) -> bool) -> Option<V> {
        let mut cache = self.inner.write().await;
        if let Some(entry) = cache.get(key) {
            if entry.is_expired() || !predicate(entry.cached_at) {
                debug!(
                    "TtlCache: evicting entry (expired={}, age={:?}, ttl={:?})",
                    entry.is_expired(),
                    entry.cached_at.elapsed(),
                    entry.ttl
                );
                cache.pop(key);
                None
            } else {
                Some(entry.data.clone())
            }
        } else {
            None
        }
    }

    /// Remove all entries from the cache.
    pub async fn clear(&self) {
        let mut cache = self.inner.write().await;
        cache.clear();
    }

    /// Returns `(len, capacity)` under a single lock acquisition.
    pub async fn stats(&self) -> (usize, usize) {
        let cache = self.inner.read().await;
        (cache.len(), cache.cap().into())
    }

    /// Number of entries currently stored (including possibly expired ones).
    #[cfg(test)]
    #[allow(clippy::len_without_is_empty)]
    pub async fn len(&self) -> usize {
        let cache = self.inner.read().await;
        cache.len()
    }

    /// Remove an entry by key. Returns `true` if the key was present.
    #[cfg(test)]
    pub async fn remove(&self, key: &K) -> bool {
        let mut cache = self.inner.write().await;
        cache.pop(key).is_some()
    }

    /// The maximum number of entries the cache can hold before LRU eviction.
    #[cfg(test)]
    pub async fn capacity(&self) -> usize {
        let cache = self.inner.read().await;
        cache.cap().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_insert_and_get() {
        let cache = TtlCache::new(10);
        cache
            .insert("key1".to_string(), 42, Duration::from_secs(60))
            .await;

        let value = cache.get(&"key1".to_string()).await;
        assert_eq!(value, Some(42));
    }

    #[tokio::test]
    async fn test_expired_entry_returns_none() {
        let cache = TtlCache::new(10);
        cache
            .insert("key1".to_string(), 42, Duration::from_millis(1))
            .await;

        tokio::time::sleep(Duration::from_millis(10)).await;

        let value = cache.get(&"key1".to_string()).await;
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = TtlCache::new(10);
        cache
            .insert("a".to_string(), 1, Duration::from_secs(60))
            .await;
        cache
            .insert("b".to_string(), 2, Duration::from_secs(60))
            .await;

        assert_eq!(cache.len().await, 2);

        cache.clear().await;

        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_get_if_passes() {
        let cache = TtlCache::new(10);
        cache
            .insert("key1".to_string(), 42, Duration::from_secs(60))
            .await;

        let value = cache.get_if(&"key1".to_string(), |_cached_at| true).await;
        assert_eq!(value, Some(42));
    }

    #[tokio::test]
    async fn test_get_if_fails() {
        let cache = TtlCache::new(10);
        cache
            .insert("key1".to_string(), 42, Duration::from_secs(60))
            .await;

        // Predicate rejects -> entry removed
        let value = cache.get_if(&"key1".to_string(), |_cached_at| false).await;
        assert_eq!(value, None);

        // Entry should be gone
        let value = cache.get(&"key1".to_string()).await;
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_remove() {
        let cache = TtlCache::new(10);
        cache
            .insert("key1".to_string(), 42, Duration::from_secs(60))
            .await;

        let removed = cache.remove(&"key1".to_string()).await;
        assert!(removed);

        let value = cache.get(&"key1".to_string()).await;
        assert_eq!(value, None);

        // Removing again returns false
        let removed_again = cache.remove(&"key1".to_string()).await;
        assert!(!removed_again);
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let cache = TtlCache::new(2);
        cache
            .insert("a".to_string(), 1, Duration::from_secs(60))
            .await;
        cache
            .insert("b".to_string(), 2, Duration::from_secs(60))
            .await;
        // This should evict "a" (the least recently used)
        cache
            .insert("c".to_string(), 3, Duration::from_secs(60))
            .await;

        assert_eq!(cache.get(&"a".to_string()).await, None);
        assert_eq!(cache.get(&"b".to_string()).await, Some(2));
        assert_eq!(cache.get(&"c".to_string()).await, Some(3));
    }

    #[tokio::test]
    async fn test_capacity() {
        let cache: TtlCache<String, i32> = TtlCache::new(42);
        assert_eq!(cache.capacity().await, 42);
    }
}
