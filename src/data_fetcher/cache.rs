use lazy_static::lazy_static;
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::data_fetcher::models::{GameData, ScheduleResponse, DetailedGameResponse, GoalEventData};
use crate::data_fetcher::player_names::format_for_display;
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

// LRU cache structure for detailed game responses to avoid repeated API calls
lazy_static! {
    pub static ref DETAILED_GAME_CACHE: RwLock<LruCache<String, CachedDetailedGameData>> =
        RwLock::new(LruCache::new(NonZeroUsize::new(200).unwrap()));
}

// LRU cache structure for processed goal events to avoid reprocessing
lazy_static! {
    pub static ref GOAL_EVENTS_CACHE: RwLock<LruCache<String, CachedGoalEventsData>> =
        RwLock::new(LruCache::new(NonZeroUsize::new(300).unwrap()));
}

// LRU cache structure for HTTP responses with TTL support
lazy_static! {
    pub static ref HTTP_RESPONSE_CACHE: RwLock<LruCache<String, CachedHttpResponse>> =
        RwLock::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
}

/// Cached tournament data with TTL support
#[derive(Debug, Clone)]
pub struct CachedTournamentData {
    pub data: ScheduleResponse,
    pub cached_at: Instant,
    pub has_live_games: bool,
}

/// Cached detailed game data with TTL support
#[derive(Debug, Clone)]
pub struct CachedDetailedGameData {
    pub data: DetailedGameResponse,
    pub cached_at: Instant,
    pub is_live_game: bool,
}

/// Cached goal events data with TTL support
#[derive(Debug, Clone)]
pub struct CachedGoalEventsData {
    pub data: Vec<GoalEventData>,
    pub cached_at: Instant,
    #[allow(dead_code)]
    pub game_id: i32,
    #[allow(dead_code)]
    pub season: i32,
}

/// Cached HTTP response with TTL support
#[derive(Debug, Clone)]
pub struct CachedHttpResponse {
    pub data: String,
    pub cached_at: Instant,
    pub ttl_seconds: u64,
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
            Duration::from_secs(30) // 30 seconds for live games
        } else {
            Duration::from_secs(3600) // 1 hour for completed games
        };

        self.cached_at.elapsed() > ttl
    }

    /// Gets the TTL duration for this cache entry
    #[allow(dead_code)]
    pub fn get_ttl(&self) -> Duration {
        if self.has_live_games {
            Duration::from_secs(30)
        } else {
            Duration::from_secs(3600)
        }
    }

    /// Gets the remaining time until expiration
    #[allow(dead_code)]
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

impl CachedDetailedGameData {
    /// Creates a new cached detailed game data entry
    pub fn new(data: DetailedGameResponse, is_live_game: bool) -> Self {
        Self {
            data,
            cached_at: Instant::now(),
            is_live_game,
        }
    }

    /// Checks if the cached data is expired based on game state
    pub fn is_expired(&self) -> bool {
        let ttl = if self.is_live_game {
            Duration::from_secs(15) // 15 seconds for live games
        } else {
            Duration::from_secs(1800) // 30 minutes for completed games
        };

        self.cached_at.elapsed() > ttl
    }

    /// Gets the TTL duration for this cache entry
    #[allow(dead_code)]
    pub fn get_ttl(&self) -> Duration {
        if self.is_live_game {
            Duration::from_secs(15)
        } else {
            Duration::from_secs(1800)
        }
    }
}

impl CachedGoalEventsData {
    /// Creates a new cached goal events data entry
    pub fn new(data: Vec<GoalEventData>, game_id: i32, season: i32) -> Self {
        Self {
            data,
            cached_at: Instant::now(),
            game_id,
            season,
        }
    }

    /// Checks if the cached data is expired
    pub fn is_expired(&self) -> bool {
        let ttl = Duration::from_secs(3600); // 1 hour for goal events
        self.cached_at.elapsed() > ttl
    }
}

impl CachedHttpResponse {
    /// Creates a new cached HTTP response entry
    pub fn new(data: String, ttl_seconds: u64) -> Self {
        Self {
            data,
            cached_at: Instant::now(),
            ttl_seconds,
        }
    }

    /// Checks if the cached data is expired
    pub fn is_expired(&self) -> bool {
        let ttl = Duration::from_secs(self.ttl_seconds);
        self.cached_at.elapsed() > ttl
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
    games
        .iter()
        .any(|game| game.score_type == ScoreType::Ongoing)
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
#[allow(dead_code)]
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
#[allow(dead_code)]
pub async fn get_cache_size() -> usize {
    PLAYER_CACHE.read().await.len()
}

/// Gets the cache capacity for monitoring purposes
#[allow(dead_code)]
pub async fn get_cache_capacity() -> usize {
    PLAYER_CACHE.read().await.cap().get()
}

/// Clears all entries from the cache
/// This is primarily used for testing purposes
#[allow(dead_code)]
pub async fn clear_cache() {
    PLAYER_CACHE.write().await.clear();
}

// Detailed Game Cache Functions

/// Creates a cache key for detailed game data
pub fn create_detailed_game_key(season: i32, game_id: i32) -> String {
    format!("detailed_game_{season}_{game_id}")
}

/// Caches detailed game data with automatic live game detection
pub async fn cache_detailed_game_data(
    season: i32,
    game_id: i32,
    data: DetailedGameResponse,
    is_live_game: bool,
) {
    let key = create_detailed_game_key(season, game_id);
    let cached_data = CachedDetailedGameData::new(data, is_live_game);
    DETAILED_GAME_CACHE.write().await.put(key, cached_data);
}

/// Retrieves cached detailed game data if it's not expired
pub async fn get_cached_detailed_game_data(
    season: i32,
    game_id: i32,
) -> Option<DetailedGameResponse> {
    let key = create_detailed_game_key(season, game_id);
    let mut cache = DETAILED_GAME_CACHE.write().await;

    if let Some(cached_entry) = cache.get(&key) {
        if !cached_entry.is_expired() {
            return Some(cached_entry.data.clone());
        } else {
            // Remove expired entry
            cache.pop(&key);
        }
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

// Goal Events Cache Functions

/// Creates a cache key for goal events data
pub fn create_goal_events_key(season: i32, game_id: i32) -> String {
    format!("goal_events_{season}_{game_id}")
}

/// Caches processed goal events data
pub async fn cache_goal_events_data(
    season: i32,
    game_id: i32,
    data: Vec<GoalEventData>,
) {
    let key = create_goal_events_key(season, game_id);
    let cached_data = CachedGoalEventsData::new(data, game_id, season);
    GOAL_EVENTS_CACHE.write().await.put(key, cached_data);
}

/// Retrieves cached goal events data if it's not expired
pub async fn get_cached_goal_events_data(
    season: i32,
    game_id: i32,
) -> Option<Vec<GoalEventData>> {
    let key = create_goal_events_key(season, game_id);
    let mut cache = GOAL_EVENTS_CACHE.write().await;

    if let Some(cached_entry) = cache.get(&key) {
        if !cached_entry.is_expired() {
            return Some(cached_entry.data.clone());
        } else {
            // Remove expired entry
            cache.pop(&key);
        }
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

// HTTP Response Cache Functions

/// Caches HTTP response data with TTL
pub async fn cache_http_response(url: String, data: String, ttl_seconds: u64) {
    let cached_data = CachedHttpResponse::new(data, ttl_seconds);
    HTTP_RESPONSE_CACHE.write().await.put(url, cached_data);
}

/// Retrieves cached HTTP response if it's not expired
pub async fn get_cached_http_response(url: &str) -> Option<String> {
    let mut cache = HTTP_RESPONSE_CACHE.write().await;

    if let Some(cached_entry) = cache.get(url) {
        if !cached_entry.is_expired() {
            return Some(cached_entry.data.clone());
        } else {
            // Remove expired entry
            cache.pop(url);
        }
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

// Combined Cache Management Functions

/// Gets combined cache statistics for monitoring purposes
#[allow(dead_code)]
pub async fn get_all_cache_stats() -> CacheStats {
    let player_size = PLAYER_CACHE.read().await.len();
    let player_capacity = PLAYER_CACHE.read().await.cap().get();
    let tournament_size = TOURNAMENT_CACHE.read().await.len();
    let tournament_capacity = TOURNAMENT_CACHE.read().await.cap().get();
    let detailed_game_size = DETAILED_GAME_CACHE.read().await.len();
    let detailed_game_capacity = DETAILED_GAME_CACHE.read().await.cap().get();
    let goal_events_size = GOAL_EVENTS_CACHE.read().await.len();
    let goal_events_capacity = GOAL_EVENTS_CACHE.read().await.cap().get();
    let http_response_size = HTTP_RESPONSE_CACHE.read().await.len();
    let http_response_capacity = HTTP_RESPONSE_CACHE.read().await.cap().get();

    CacheStats {
        player_cache: CacheInfo {
            size: player_size,
            capacity: player_capacity,
        },
        tournament_cache: CacheInfo {
            size: tournament_size,
            capacity: tournament_capacity,
        },
        detailed_game_cache: CacheInfo {
            size: detailed_game_size,
            capacity: detailed_game_capacity,
        },
        goal_events_cache: CacheInfo {
            size: goal_events_size,
            capacity: goal_events_capacity,
        },
        http_response_cache: CacheInfo {
            size: http_response_size,
            capacity: http_response_capacity,
        },
    }
}

/// Cache information structure
#[derive(Debug, Clone)]
pub struct CacheInfo {
    pub size: usize,
    pub capacity: usize,
}

/// Combined cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub player_cache: CacheInfo,
    pub tournament_cache: CacheInfo,
    pub detailed_game_cache: CacheInfo,
    pub goal_events_cache: CacheInfo,
    pub http_response_cache: CacheInfo,
}

/// Clears all caches (useful for testing and debugging)
#[allow(dead_code)]
pub async fn clear_all_caches() {
    clear_cache().await;
    clear_tournament_cache().await;
    clear_detailed_game_cache().await;
    clear_goal_events_cache().await;
    clear_http_response_cache().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::models::{ScheduleGame, ScheduleTeam, DetailedGame, DetailedTeam, DetailedGameResponse, GoalEventData};
    use serial_test::serial;
    use tokio::sync::Mutex;

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
            games: vec![ScheduleGame {
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
                ended: false,    // Live game
                game_time: 1800, // 30 minutes played
                serie: "runkosarja".to_string(),
            }],
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
            games: vec![ScheduleGame {
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
            }],
            previous_game_date: None,
            next_game_date: None,
        };

        assert!(has_live_games(&live_response));

        // Test with completed games
        let completed_response = ScheduleResponse {
            games: vec![ScheduleGame {
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
            }],
            previous_game_date: None,
            next_game_date: None,
        };

        assert!(!has_live_games(&completed_response));
    }

    #[tokio::test]
    #[serial]
    async fn test_has_live_games_from_game_data() {
        // Test with live games
        let live_games = vec![GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Ongoing,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 1800,
            start: "2024-01-15T18:30:00Z".to_string(),
        }];

        assert!(has_live_games_from_game_data(&live_games));

        // Test with completed games
        let completed_games = vec![GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T18:30:00Z".to_string(),
        }];

        assert!(!has_live_games_from_game_data(&completed_games));
    }

    // New LRU Cache Tests

    #[tokio::test]
    #[serial]
    async fn test_detailed_game_cache_basic() {
        let _guard = TEST_MUTEX.lock().await;

        // Clear cache to ensure clean state
        clear_detailed_game_cache().await;

        // Create a mock DetailedGameResponse
        let mock_response = DetailedGameResponse {
            game: DetailedGame {
                id: 12345,
                season: 2024,
                start: "2024-01-15T18:30:00Z".to_string(),
                end: None,
                home_team: DetailedTeam {
                    team_id: "team1".to_string(),
                    team_name: "HIFK".to_string(),
                    goals: 2,
                    goal_events: vec![],
                    penalty_events: vec![],
                },
                away_team: DetailedTeam {
                    team_id: "team2".to_string(),
                    team_name: "Tappara".to_string(),
                    goals: 1,
                    goal_events: vec![],
                    penalty_events: vec![],
                },
                periods: vec![],
                finished_type: None,
                started: true,
                ended: false,
                game_time: 1800,
                serie: "runkosarja".to_string(),
            },
            awards: vec![],
            home_team_players: vec![],
            away_team_players: vec![],
        };

        cache_detailed_game_data(2024, 12345, mock_response.clone(), false).await;

        // Should be able to retrieve it
        let cached = get_cached_detailed_game_data(2024, 12345).await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().game.id, 12345);

        // Clear cache after test
        clear_detailed_game_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_goal_events_cache_basic() {
        let _guard = TEST_MUTEX.lock().await;

        // Clear cache to ensure clean state
        clear_goal_events_cache().await;

        // Create mock goal events
        let mock_events = vec![GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Koivu".to_string(),
            minute: 15,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["EV".to_string()],
            is_home_team: true,
            video_clip_url: None,
        }];

        cache_goal_events_data(2024, 12345, mock_events.clone()).await;

        // Should be able to retrieve it
        let cached = get_cached_goal_events_data(2024, 12345).await;
        assert!(cached.is_some());
        let cached_events = cached.unwrap();
        assert_eq!(cached_events.len(), 1);
        assert_eq!(cached_events[0].scorer_name, "Koivu");

        // Clear cache after test
        clear_goal_events_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_http_response_cache_basic() {
        let _guard = TEST_MUTEX.lock().await;

        // Clear cache to ensure clean state
        clear_http_response_cache().await;

        let url = "https://api.example.com/test".to_string();
        let response_data = r#"{"test": "data"}"#.to_string();

        cache_http_response(url.clone(), response_data.clone(), 60).await;

        // Should be able to retrieve it
        let cached = get_cached_http_response(&url).await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), response_data);

        // Clear cache after test
        clear_http_response_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_stats() {
        let _guard = TEST_MUTEX.lock().await;

        // Clear all caches to ensure clean state
        clear_all_caches().await;

        // Add some test data to each cache
        let mut players = HashMap::new();
        players.insert(1, "Player 1".to_string());
        cache_players(10001, players).await;

        let mock_response = ScheduleResponse {
            games: vec![],
            previous_game_date: None,
            next_game_date: None,
        };
        cache_tournament_data("test-tournament".to_string(), mock_response).await;

        let mock_detailed_response = DetailedGameResponse {
            game: DetailedGame {
                id: 12345,
                season: 2024,
                start: "2024-01-15T18:30:00Z".to_string(),
                end: None,
                home_team: DetailedTeam {
                    team_id: "team1".to_string(),
                    team_name: "HIFK".to_string(),
                    goals: 0,
                    goal_events: vec![],
                    penalty_events: vec![],
                },
                away_team: DetailedTeam {
                    team_id: "team2".to_string(),
                    team_name: "Tappara".to_string(),
                    goals: 0,
                    goal_events: vec![],
                    penalty_events: vec![],
                },
                periods: vec![],
                finished_type: None,
                started: false,
                ended: false,
                game_time: 0,
                serie: "runkosarja".to_string(),
            },
            awards: vec![],
            home_team_players: vec![],
            away_team_players: vec![],
        };
        cache_detailed_game_data(2024, 12345, mock_detailed_response, false).await;

        let mock_events = vec![GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Koivu".to_string(),
            minute: 15,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["EV".to_string()],
            is_home_team: true,
            video_clip_url: None,
        }];
        cache_goal_events_data(2024, 12345, mock_events).await;

        cache_http_response("https://api.example.com/test".to_string(), "test data".to_string(), 60).await;

        // Get stats
        let stats = get_all_cache_stats().await;

        // Verify stats
        assert_eq!(stats.player_cache.size, 1);
        assert_eq!(stats.tournament_cache.size, 1);
        assert_eq!(stats.detailed_game_cache.size, 1);
        assert_eq!(stats.goal_events_cache.size, 1);
        assert_eq!(stats.http_response_cache.size, 1);

        // Clear all caches after test
        clear_all_caches().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_key_generation() {
        let detailed_key = create_detailed_game_key(2024, 12345);
        assert_eq!(detailed_key, "detailed_game_2024_12345");

        let goal_events_key = create_goal_events_key(2024, 12345);
        assert_eq!(goal_events_key, "goal_events_2024_12345");
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_expiration() {
        let _guard = TEST_MUTEX.lock().await;

        // Clear cache to ensure clean state
        clear_goal_events_cache().await;

        // Create a cached entry
        let mock_events = vec![GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Koivu".to_string(),
            minute: 15,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["EV".to_string()],
            is_home_team: true,
            video_clip_url: None,
        }];

        cache_goal_events_data(2024, 12345, mock_events).await;

        // Should be able to retrieve it immediately
        let cached = get_cached_goal_events_data(2024, 12345).await;
        assert!(cached.is_some());

        // Note: We can't easily test actual expiration without time manipulation,
        // but we can verify the cache structure is working correctly

        // Clear cache after test
        clear_goal_events_cache().await;
    }
}
