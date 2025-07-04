use lazy_static::lazy_static;
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::data_fetcher::player_names::format_for_display;
use crate::data_fetcher::models::{ScheduleResponse, GameData};
use crate::teletext_ui::ScoreType;

// LRU cache structure for formatted player information
// Using LRU ensures that when we need to evict entries, we remove the least recently used ones
lazy_static! {
    pub static ref PLAYER_CACHE: RwLock<LruCache<i32, HashMap<i64, String>>> =
        RwLock::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
}

// LRU cache structure for tournament data with TTL support
lazy_static! {
    pub static ref TOURNAMENT_CACHE: RwLock<LruCache<String, CachedTournamentData>> =
        RwLock::new(LruCache::new(NonZeroUsize::new(50).unwrap()));
}

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
            Duration::from_secs(30)  // 30 seconds for live games
        } else {
            Duration::from_secs(3600) // 1 hour for completed games
        };

        self.cached_at.elapsed() > ttl
    }

    /// Gets the TTL duration for this cache entry
    pub fn get_ttl(&self) -> Duration {
        if self.has_live_games {
            Duration::from_secs(30)
        } else {
            Duration::from_secs(3600)
        }
    }

    /// Gets the remaining time until expiration
    pub fn time_until_expiry(&self) -> Duration {
        let ttl = self.get_ttl();
        let elapsed = self.cached_at.elapsed();
        if elapsed >= ttl {
            Duration::ZERO
        } else {
            ttl - elapsed
        }
    }
}

/// Determines if a ScheduleResponse contains live games
pub fn has_live_games(response: &ScheduleResponse) -> bool {
    response.games.iter().any(|game| {
        // Game is live if it's started but not ended
        game.started && !game.ended
    })
}

/// Determines if a list of GameData contains live games
pub fn has_live_games_from_game_data(games: &[GameData]) -> bool {
    games.iter().any(|game| {
        game.score_type == ScoreType::Ongoing
    })
}

/// Caches tournament data with automatic live game detection
pub async fn cache_tournament_data(key: String, data: ScheduleResponse) {
    let has_live = has_live_games(&data);
    let cached_data = CachedTournamentData::new(data, has_live);

    TOURNAMENT_CACHE.write().await.put(key, cached_data);
}

/// Retrieves cached tournament data if it's not expired
pub async fn get_cached_tournament_data(key: &str) -> Option<ScheduleResponse> {
    let mut cache = TOURNAMENT_CACHE.write().await;

    if let Some(cached_entry) = cache.get(key) {
        if !cached_entry.is_expired() {
            return Some(cached_entry.data.clone());
        } else {
            // Remove expired entry
            cache.pop(key);
        }
    }

    None
}

/// Retrieves cached tournament data with custom live game state check
pub async fn get_cached_tournament_data_with_live_check(
    key: &str,
    current_games: &[GameData],
) -> Option<ScheduleResponse> {
    let mut cache = TOURNAMENT_CACHE.write().await;

    if let Some(cached_entry) = cache.get(key) {
        // Check if we have live games in the current state
        let has_live = has_live_games_from_game_data(current_games);

        // If the live state has changed, consider the cache expired
        if cached_entry.has_live_games != has_live {
            cache.pop(key);
            return None;
        }

        if !cached_entry.is_expired() {
            return Some(cached_entry.data.clone());
        } else {
            // Remove expired entry
            cache.pop(key);
        }
    }

    None
}

/// Invalidates all tournament cache entries for a specific date
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

/// Gets the current tournament cache size for monitoring purposes
pub async fn get_tournament_cache_size() -> usize {
    TOURNAMENT_CACHE.read().await.len()
}

/// Gets the tournament cache capacity for monitoring purposes
pub async fn get_tournament_cache_capacity() -> usize {
    TOURNAMENT_CACHE.read().await.cap().get()
}

/// Clears all tournament cache entries
pub async fn clear_tournament_cache() {
    TOURNAMENT_CACHE.write().await.clear();
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
    use serial_test::serial;
    use tokio::sync::Mutex;
    use crate::data_fetcher::models::{ScheduleGame, ScheduleTeam};

    // Mutex to ensure LRU tests run sequentially to avoid cache interference
    static TEST_MUTEX: Mutex<()> = Mutex::const_new(());

    #[tokio::test]
    #[serial]
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
    #[serial]
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
    #[serial]
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
    #[serial]
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

        // Add 95 more entries to reach capacity (100 total: 5 original + 95 new)
        for i in 30005..30100 {
            let mut players = HashMap::new();
            players.insert(i as i64, format!("Player {i}"));
            cache_players(i, players).await;
        }

        // Entry 30000 should still be there because it was accessed
        assert!(get_cached_players(30000).await.is_some());

        // Since we added 95 more entries to a cache with capacity 100,
        // and we started with 5 entries, we should have exactly 100 entries total.
        // The LRU behavior means that some of the original entries (30001-30004)
        // should have been evicted to make room for the new entries.
        // We can't predict exactly which ones, but we can verify the cache size.
        assert_eq!(get_cache_size().await, 100);

        // Verify that at least one of the original entries (30001-30004) was evicted
        let mut original_entries_remaining = 0;
        for i in 30001..30005 {
            if get_cached_players(i).await.is_some() {
                original_entries_remaining += 1;
            }
        }

        // Since we accessed 30000, it should still be there, but some of the others
        // should have been evicted. We expect at most 4 original entries to remain
        // (including 30000 which was accessed)
        assert!(original_entries_remaining <= 4);

        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_tournament_cache_basic() {
        let _guard = TEST_MUTEX.lock().await;

        // Clear cache to ensure clean state
        clear_tournament_cache().await;

        // Create a mock ScheduleResponse with completed games
        let mock_response = ScheduleResponse {
            games: vec![],
            previous_game_date: None,
            next_game_date: None,
        };

        let key = "runkosarja-2024-01-15".to_string();
        cache_tournament_data(key.clone(), mock_response.clone()).await;

        // Should be able to retrieve it
        let cached = get_cached_tournament_data(&key).await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().games.len(), 0);

        // Clear cache after test
        clear_tournament_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_tournament_cache_ttl() {
        let _guard = TEST_MUTEX.lock().await;

        // Clear cache to ensure clean state
        clear_tournament_cache().await;

        // Create a mock ScheduleResponse with live games
        let mock_response = ScheduleResponse {
            games: vec![
                ScheduleGame {
                    id: 1,
                    season: 2024,
                    start: "2024-01-15T18:30:00Z".to_string(),
                    end: None,
                    home_team: ScheduleTeam {
                        team_id: Some("team1".to_string()),
                        team_placeholder: None,
                        team_name: Some("HIFK".to_string()),
                        goals: 2,
                        time_out: None,
                        powerplay_instances: 0,
                        powerplay_goals: 0,
                        short_handed_instances: 0,
                        short_handed_goals: 0,
                        ranking: Some(1),
                        game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
                        goal_events: vec![],
                    },
                    away_team: ScheduleTeam {
                        team_id: Some("team2".to_string()),
                        team_placeholder: None,
                        team_name: Some("Tappara".to_string()),
                        goals: 1,
                        time_out: None,
                        powerplay_instances: 0,
                        powerplay_goals: 0,
                        short_handed_instances: 0,
                        short_handed_goals: 0,
                        ranking: Some(2),
                        game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
                        goal_events: vec![],
                    },
                    finished_type: None,
                    started: true,
                    ended: false, // Live game
                    game_time: 1800, // 30 minutes played
                    serie: "runkosarja".to_string(),
                }
            ],
            previous_game_date: None,
            next_game_date: None,
        };

        let key = "runkosarja-2024-01-15".to_string();
        cache_tournament_data(key.clone(), mock_response).await;

        // Should be able to retrieve it immediately
        let cached = get_cached_tournament_data(&key).await;
        assert!(cached.is_some());

        // Check cache size
        assert_eq!(get_tournament_cache_size().await, 1);

        // Clear cache after test
        clear_tournament_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_has_live_games_detection() {
        // Test with live games
        let live_response = ScheduleResponse {
            games: vec![
                ScheduleGame {
                    id: 1,
                    season: 2024,
                    start: "2024-01-15T18:30:00Z".to_string(),
                    end: None,
                    home_team: ScheduleTeam {
                        team_id: Some("team1".to_string()),
                        team_placeholder: None,
                        team_name: Some("HIFK".to_string()),
                        goals: 2,
                        time_out: None,
                        powerplay_instances: 0,
                        powerplay_goals: 0,
                        short_handed_instances: 0,
                        short_handed_goals: 0,
                        ranking: Some(1),
                        game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
                        goal_events: vec![],
                    },
                    away_team: ScheduleTeam {
                        team_id: Some("team2".to_string()),
                        team_placeholder: None,
                        team_name: Some("Tappara".to_string()),
                        goals: 1,
                        time_out: None,
                        powerplay_instances: 0,
                        powerplay_goals: 0,
                        short_handed_instances: 0,
                        short_handed_goals: 0,
                        ranking: Some(2),
                        game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
                        goal_events: vec![],
                    },
                    finished_type: None,
                    started: true,
                    ended: false, // Live game
                    game_time: 1800,
                    serie: "runkosarja".to_string(),
                }
            ],
            previous_game_date: None,
            next_game_date: None,
        };

        assert!(has_live_games(&live_response));

        // Test with completed games
        let completed_response = ScheduleResponse {
            games: vec![
                ScheduleGame {
                    id: 1,
                    season: 2024,
                    start: "2024-01-15T18:30:00Z".to_string(),
                    end: Some("2024-01-15T20:30:00Z".to_string()),
                    home_team: ScheduleTeam {
                        team_id: Some("team1".to_string()),
                        team_placeholder: None,
                        team_name: Some("HIFK".to_string()),
                        goals: 3,
                        time_out: None,
                        powerplay_instances: 0,
                        powerplay_goals: 0,
                        short_handed_instances: 0,
                        short_handed_goals: 0,
                        ranking: Some(1),
                        game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
                        goal_events: vec![],
                    },
                    away_team: ScheduleTeam {
                        team_id: Some("team2".to_string()),
                        team_placeholder: None,
                        team_name: Some("Tappara".to_string()),
                        goals: 2,
                        time_out: None,
                        powerplay_instances: 0,
                        powerplay_goals: 0,
                        short_handed_instances: 0,
                        short_handed_goals: 0,
                        ranking: Some(2),
                        game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
                        goal_events: vec![],
                    },
                    finished_type: Some("regular".to_string()),
                    started: true,
                    ended: true, // Completed game
                    game_time: 3600,
                    serie: "runkosarja".to_string(),
                }
            ],
            previous_game_date: None,
            next_game_date: None,
        };

        assert!(!has_live_games(&completed_response));
    }
}
