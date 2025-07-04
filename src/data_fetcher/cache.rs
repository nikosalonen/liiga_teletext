use lazy_static::lazy_static;
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use tokio::sync::RwLock;

use crate::data_fetcher::player_names::format_for_display;

// LRU cache structure for formatted player information
// Using LRU ensures that when we need to evict entries, we remove the least recently used ones
lazy_static! {
    pub static ref PLAYER_CACHE: RwLock<LruCache<i32, HashMap<i64, String>>> =
        RwLock::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
}

/// Retrieves cached formatted player information for a specific game.
/// This operation also updates the LRU order, making this entry the most recently used.
///
/// # Arguments
/// * `game_id` - The unique identifier of the game
///
/// # Returns
/// * `Option<HashMap<i64, String>>` - Some(HashMap) with player_id -> formatted_name mapping if found, None if not cached
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::cache::get_cached_players;
///
/// #[tokio::main]
/// async fn main() {
///     if let Some(players) = get_cached_players(12345).await {
///         println!("Found {} cached players", players.len());
///     }
/// }
/// ```
pub async fn get_cached_players(game_id: i32) -> Option<HashMap<i64, String>> {
    PLAYER_CACHE.write().await.get(&game_id).cloned()
}

/// Caches formatted player information for a specific game.
/// Updates existing cache entry if game_id already exists.
/// This operation makes the entry the most recently used.
///
/// # Arguments
/// * `game_id` - The unique identifier of the game
/// * `players` - HashMap mapping player IDs to their formatted names
///
/// # Example
/// ```
/// use std::collections::HashMap;
/// use liiga_teletext::data_fetcher::cache::cache_players;
///
/// #[tokio::main]
/// async fn main() {
///     let mut players = HashMap::new();
///     players.insert(123, "Koivu".to_string()); // Already formatted
///     cache_players(12345, players).await;
/// }
/// ```
pub async fn cache_players(game_id: i32, players: HashMap<i64, String>) {
    PLAYER_CACHE.write().await.put(game_id, players);
}

/// Caches player information with automatic formatting for a specific game.
/// This function takes raw player data and formats the names before caching.
/// This operation makes the entry the most recently used.
///
/// # Arguments
/// * `game_id` - The unique identifier of the game
/// * `raw_players` - HashMap mapping player IDs to their full names
///
/// # Example
/// ```
/// use std::collections::HashMap;
/// use liiga_teletext::data_fetcher::cache::cache_players_with_formatting;
///
/// #[tokio::main]
/// async fn main() {
///     let mut raw_players = HashMap::new();
///     raw_players.insert(123, "Mikko Koivu".to_string());
///     raw_players.insert(456, "Teemu Selänne".to_string());
///     cache_players_with_formatting(12345, raw_players).await;
///     // Names will be cached as "Koivu" and "Selänne"
/// }
/// ```
pub async fn cache_players_with_formatting(game_id: i32, raw_players: HashMap<i64, String>) {
    let formatted_players: HashMap<i64, String> = raw_players
        .into_iter()
        .map(|(id, full_name)| (id, format_for_display(&full_name)))
        .collect();
    cache_players(game_id, formatted_players).await;
}

/// Gets the current cache size for monitoring purposes
pub async fn get_cache_size() -> usize {
    PLAYER_CACHE.read().await.len()
}

/// Gets the cache capacity for monitoring purposes
pub async fn get_cache_capacity() -> usize {
    PLAYER_CACHE.read().await.cap().get()
}

/// Clears all entries from the cache
/// This is primarily used for testing purposes
#[allow(dead_code)]
pub async fn clear_cache() {
    PLAYER_CACHE.write().await.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::Mutex;

    // Mutex to ensure LRU tests run sequentially to avoid cache interference
    static TEST_MUTEX: Mutex<()> = Mutex::const_new(());

    #[tokio::test]
    async fn test_cache_players_with_formatting() {
        // Use unique IDs starting from 40000 to avoid interference with other tests
        let _guard = TEST_MUTEX.lock().await;

        // Clear cache to ensure clean state
        clear_cache().await;

        let mut raw_players = HashMap::new();
        raw_players.insert(123, "Mikko Koivu".to_string());
        raw_players.insert(456, "Teemu Selänne".to_string());
        raw_players.insert(789, "John Smith".to_string());

        cache_players_with_formatting(40999, raw_players).await;

        let cached_players = get_cached_players(40999).await.unwrap();
        assert_eq!(cached_players.get(&123), Some(&"Koivu".to_string()));
        assert_eq!(cached_players.get(&456), Some(&"Selänne".to_string()));
        assert_eq!(cached_players.get(&789), Some(&"Smith".to_string()));

        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    async fn test_lru_simple() {
        // Test that the LRU cache actually works at all
        // Use unique IDs starting from 10000 to avoid interference with other tests

        let _guard = TEST_MUTEX.lock().await;

        // Clear cache to ensure clean state
        clear_cache().await;

        // Add one entry
        let mut players = HashMap::new();
        players.insert(1, "Player 10001".to_string());
        cache_players(10001, players).await;

        // Should be able to retrieve it
        assert!(get_cached_players(10001).await.is_some());

        // Add 100 more entries to fill the cache
        for i in 10002..10102 {
            let mut players = HashMap::new();
            players.insert(i as i64, format!("Player {i}"));
            cache_players(i, players).await;
        }

        // The first entry should be evicted
        assert!(get_cached_players(10001).await.is_none());

        // The last entry should still be there
        assert!(get_cached_players(10101).await.is_some());

        // Cache should be at capacity
        assert_eq!(get_cache_size().await, 100);

        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    async fn test_lru_access_order() {
        // Test that accessing an entry makes it most recently used
        // Use unique IDs starting from 20000 to avoid interference with other tests

        let _guard = TEST_MUTEX.lock().await;

        // Clear cache to ensure clean state
        clear_cache().await;

        // Add exactly 100 entries to fill the cache
        for i in 20000..20100 {
            let mut players = HashMap::new();
            players.insert(i as i64, format!("Player {i}"));
            cache_players(i, players).await;
        }

        // Verify cache is at capacity
        assert_eq!(get_cache_size().await, 100);

        // Access an entry in the middle to make it most recently used
        let _ = get_cached_players(20050).await;

        // Add one more entry, which should evict the least recently used entry
        let mut players = HashMap::new();
        players.insert(99999, "New Player".to_string());
        cache_players(20999, players).await;

        // The accessed entry (20050) should still be there
        assert!(get_cached_players(20050).await.is_some());

        // The new entry should be there
        assert!(get_cached_players(20999).await.is_some());

        // Cache should still be at capacity
        assert_eq!(get_cache_size().await, 100);

        // Some older entry should have been evicted (we don't test which specific one)
        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    async fn test_lru_simple_access_order() {
        // Simpler test to verify LRU access order behavior
        // Use unique IDs starting from 30000 to avoid interference with other tests

        let _guard = TEST_MUTEX.lock().await;

        // Clear cache to ensure clean state
        clear_cache().await;

        // Add 5 entries
        for i in 30000..30005 {
            let mut players = HashMap::new();
            players.insert(i as i64, format!("Player {i}"));
            cache_players(i, players).await;
        }

        // Access entry 30000 to make it most recently used
        let _ = get_cached_players(30000).await;

        // Add 96 more entries to reach capacity (100)
        for i in 30005..30101 {
            let mut players = HashMap::new();
            players.insert(i as i64, format!("Player {i}"));
            cache_players(i, players).await;
        }

        // Entry 30000 should still be there because it was accessed
        assert!(get_cached_players(30000).await.is_some());

        // At least one of the original entries should have been evicted
        // The exact order depends on the LRU implementation, but we know entry 30001 was evicted
        assert!(get_cached_players(30001).await.is_none());

        // Clear cache after test
        clear_cache().await;
    }
}
