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
    name: &'static str,
}

impl<K: Eq + Hash + Clone, V: Clone> TtlCache<K, V> {
    /// Create a new named cache with the given maximum capacity.
    ///
    /// # Panics
    /// Panics if `capacity` is 0.
    pub fn new(name: &'static str, capacity: usize) -> Self {
        Self {
            inner: RwLock::new(LruCache::new(
                NonZeroUsize::new(capacity).expect("cache capacity must be > 0"),
            )),
            name,
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
                    "{}: evicting expired entry (age={:?}, ttl={:?})",
                    self.name,
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
        debug_assert!(ttl > Duration::ZERO, "TTL must be non-zero");
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
                    "{}: evicting entry (expired={}, age={:?}, ttl={:?})",
                    self.name,
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

    fn test_cache(capacity: usize) -> TtlCache<String, i32> {
        TtlCache::new("test", capacity)
    }

    #[tokio::test]
    async fn test_insert_and_get() {
        let cache = test_cache(10);
        cache
            .insert("key1".to_string(), 42, Duration::from_secs(60))
            .await;

        let value = cache.get(&"key1".to_string()).await;
        assert_eq!(value, Some(42));
    }

    #[tokio::test]
    async fn test_expired_entry_returns_none_and_is_evicted() {
        let cache = test_cache(10);
        cache
            .insert("key1".to_string(), 42, Duration::from_millis(1))
            .await;
        assert_eq!(cache.len().await, 1);

        tokio::time::sleep(Duration::from_millis(10)).await;

        let value = cache.get(&"key1".to_string()).await;
        assert_eq!(value, None);
        // Entry must be removed from the LRU, not just hidden
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = test_cache(10);
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
        let cache = test_cache(10);
        cache
            .insert("key1".to_string(), 42, Duration::from_secs(60))
            .await;

        let value = cache.get_if(&"key1".to_string(), |_cached_at| true).await;
        assert_eq!(value, Some(42));
    }

    #[tokio::test]
    async fn test_get_if_fails_and_evicts() {
        let cache = test_cache(10);
        cache
            .insert("key1".to_string(), 42, Duration::from_secs(60))
            .await;
        assert_eq!(cache.len().await, 1);

        // Predicate rejects -> entry removed
        let value = cache.get_if(&"key1".to_string(), |_cached_at| false).await;
        assert_eq!(value, None);

        // Entry must be fully removed from the LRU
        assert_eq!(cache.len().await, 0);
        assert_eq!(cache.get(&"key1".to_string()).await, None);
    }

    #[tokio::test]
    async fn test_insert_overwrite_updates_value_and_ttl() {
        let cache = test_cache(10);
        cache
            .insert("key1".to_string(), 1, Duration::from_millis(1))
            .await;

        // Overwrite with new value and longer TTL
        cache
            .insert("key1".to_string(), 2, Duration::from_secs(60))
            .await;

        // len should not increase
        assert_eq!(cache.len().await, 1);

        // Should return the updated value
        assert_eq!(cache.get(&"key1".to_string()).await, Some(2));

        // Wait past the original TTL — entry should still be alive (TTL was reset)
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(cache.get(&"key1".to_string()).await, Some(2));
    }

    #[tokio::test]
    async fn test_remove() {
        let cache = test_cache(10);
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
        let cache = test_cache(2);
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
        let cache: TtlCache<String, i32> = TtlCache::new("test", 42);
        assert_eq!(cache.capacity().await, 42);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        use std::sync::Arc;

        let cache = Arc::new(TtlCache::new("concurrent_test", 100));
        let mut handles = Vec::new();

        // Spawn writers
        for i in 0..10 {
            let cache = Arc::clone(&cache);
            handles.push(tokio::spawn(async move {
                for j in 0..20 {
                    cache
                        .insert(format!("w{i}-{j}"), i * 100 + j, Duration::from_secs(60))
                        .await;
                }
            }));
        }

        // Spawn readers (interleaved with writers)
        for i in 0..10 {
            let cache = Arc::clone(&cache);
            handles.push(tokio::spawn(async move {
                for j in 0..20 {
                    let _ = cache.get(&format!("w{i}-{j}")).await;
                }
            }));
        }

        // Spawn get_if operations
        for i in 0..5 {
            let cache = Arc::clone(&cache);
            handles.push(tokio::spawn(async move {
                for j in 0..10 {
                    let _ = cache.get_if(&format!("w{i}-{j}"), |_| j % 2 == 0).await;
                }
            }));
        }

        // All tasks must complete without panic or deadlock
        for handle in handles {
            handle.await.unwrap();
        }

        // Cache should still be functional
        cache
            .insert("final".to_string(), 999, Duration::from_secs(60))
            .await;
        assert_eq!(cache.get(&"final".to_string()).await, Some(999));
    }
}
