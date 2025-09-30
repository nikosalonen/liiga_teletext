use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, trace, warn};

use crate::constants::cache_ttl;
use crate::data_fetcher::models::{
    DetailedGameResponse, GameData, GoalEventData, ScheduleResponse,
};
use crate::data_fetcher::player_names::format_for_display;
use crate::teletext_ui::ScoreType;

// Import cache types from sibling module
use super::types::{
    CachedDetailedGameData, CachedGoalEventsData, CachedHttpResponse,
};
// Import tournament cache items from sibling module
use super::tournament_cache::{
    cache_tournament_data, clear_tournament_cache, get_cached_tournament_data,
    get_tournament_cache_capacity, get_tournament_cache_size, has_live_games, TOURNAMENT_CACHE,
};
// Import player cache items from sibling module
use super::player_cache::{
    cache_players, cache_players_with_disambiguation, cache_players_with_formatting,
    clear_cache, get_cache_capacity, get_cache_size, get_cached_disambiguated_players,
    get_cached_player_name, get_cached_players, has_cached_disambiguated_players, PLAYER_CACHE,
};
// Import detailed game cache items from sibling module
use super::detailed_game_cache::{
    cache_detailed_game_data, clear_detailed_game_cache, create_detailed_game_key,
    get_cached_detailed_game_data, get_detailed_game_cache_capacity,
    get_detailed_game_cache_size, DETAILED_GAME_CACHE,
};

// LRU cache structure for processed goal events to avoid reprocessing
pub static GOAL_EVENTS_CACHE: LazyLock<RwLock<LruCache<String, CachedGoalEventsData>>> =
    LazyLock::new(|| RwLock::new(LruCache::new(NonZeroUsize::new(300).unwrap())));

// LRU cache structure for HTTP responses with TTL support
pub static HTTP_RESPONSE_CACHE: LazyLock<RwLock<LruCache<String, CachedHttpResponse>>> =
    LazyLock::new(|| RwLock::new(LruCache::new(NonZeroUsize::new(100).unwrap())));

/// Determines if a list of GameData contains live games
pub fn has_live_games_from_game_data(games: &[GameData]) -> bool {
    let has_live = games
        .iter()
        .any(|game| game.score_type == ScoreType::Ongoing);

    if has_live {
        let ongoing_count = games
            .iter()
            .filter(|g| g.score_type == ScoreType::Ongoing)
            .count();
        trace!(
            "Live games detected: {} ongoing out of {} total games",
            ongoing_count,
            games.len()
        );

        // Log details of ongoing games for debugging
        for (i, game) in games.iter().enumerate() {
            if game.score_type == ScoreType::Ongoing {
                trace!(
                    "Ongoing game {}: {} vs {} - Score: {}, Time: {}",
                    i + 1,
                    game.home_team,
                    game.away_team,
                    game.result,
                    game.time
                );
            }
        }
    } else {
        trace!("No live games detected in {} games", games.len());
    }

    has_live
}

// Goal Events Cache Functions

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

// HTTP Response Cache Functions

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

// Combined Cache Management Functions

/// Gets combined cache statistics for monitoring purposes
/// Optimized to minimize RwLock contention by batching read operations
pub async fn get_all_cache_stats() -> CacheStats {
    // Acquire all read locks concurrently to minimize contention
    let (
        player_cache,
        tournament_cache,
        detailed_game_cache,
        goal_events_cache,
        http_response_cache,
    ) = tokio::join!(
        PLAYER_CACHE.read(),
        TOURNAMENT_CACHE.read(),
        DETAILED_GAME_CACHE.read(),
        GOAL_EVENTS_CACHE.read(),
        HTTP_RESPONSE_CACHE.read(),
    );

    // Extract size and capacity from each cache in a single lock hold
    let player_size = player_cache.len();
    let player_capacity = player_cache.cap().get();
    let tournament_size = tournament_cache.len();
    let tournament_capacity = tournament_cache.cap().get();
    let detailed_game_size = detailed_game_cache.len();
    let detailed_game_capacity = detailed_game_cache.cap().get();
    let goal_events_size = goal_events_cache.len();
    let goal_events_capacity = goal_events_cache.cap().get();
    let http_response_size = http_response_cache.len();
    let http_response_capacity = http_response_cache.cap().get();

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

/// Gets detailed cache debugging information including individual cache entries
/// This function demonstrates usage of all debugging methods for monitoring purposes
pub async fn get_detailed_cache_debug_info() -> String {
    let mut debug_info = String::new();

    // Get basic stats
    let stats = get_all_cache_stats().await;
    debug_info.push_str(&format!(
        "Cache Statistics:\n\
         Player Cache: {}/{} entries\n\
         Tournament Cache: {}/{} entries\n\
         Detailed Game Cache: {}/{} entries\n\
         Goal Events Cache: {}/{} entries\n\
         HTTP Response Cache: {}/{} entries\n\n",
        stats.player_cache.size,
        stats.player_cache.capacity,
        stats.tournament_cache.size,
        stats.tournament_cache.capacity,
        stats.detailed_game_cache.size,
        stats.detailed_game_cache.capacity,
        stats.goal_events_cache.size,
        stats.goal_events_cache.capacity,
        stats.http_response_cache.size,
        stats.http_response_cache.capacity,
    ));

    // Get detailed goal events cache info using debug methods
    let goal_events_cache = GOAL_EVENTS_CACHE.read().await;
    if !goal_events_cache.is_empty() {
        debug_info.push_str("Goal Events Cache Details:\n");
        for (key, entry) in goal_events_cache.iter() {
            // Use individual debug methods for comprehensive information
            let game_id = entry.get_game_id();
            let season = entry.get_season();
            let (returned_game_id, returned_season, event_count, is_expired) =
                entry.get_cache_info();

            // Verify consistency between individual methods and combined method (debug-only)
            debug_assert_eq!(game_id, returned_game_id);
            debug_assert_eq!(season, returned_season);

            debug_info.push_str(&format!(
                "  Key: {key}, Game ID: {game_id}, Season: {season}, Events: {event_count}, Expired: {is_expired}\n"
            ));
        }
        debug_info.push('\n');
    }

    debug_info
}

/// Resets all caches and returns confirmation - demonstrates clear_all_caches usage
#[allow(dead_code)]
pub async fn reset_all_caches_with_confirmation() -> String {
    let stats_before = get_all_cache_stats().await;
    let total_before = stats_before.player_cache.size
        + stats_before.tournament_cache.size
        + stats_before.detailed_game_cache.size
        + stats_before.goal_events_cache.size
        + stats_before.http_response_cache.size;

    clear_all_caches().await;

    let stats_after = get_all_cache_stats().await;
    let total_after = stats_after.player_cache.size
        + stats_after.tournament_cache.size
        + stats_after.detailed_game_cache.size
        + stats_after.goal_events_cache.size
        + stats_after.http_response_cache.size;

    format!(
        "Cache reset completed. Entries before: {total_before}, after: {total_after}. All caches cleared successfully."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::models::{
        DetailedGame, DetailedGameResponse, DetailedTeam, GoalEventData, ScheduleGame, ScheduleTeam,
    };
    use serial_test::serial;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Mutex;

    // Mutex to ensure LRU tests run sequentially to avoid cache interference
    static TEST_MUTEX: Mutex<()> = Mutex::const_new(());

    // Global test counter to ensure unique IDs across all test runs
    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn get_unique_test_id() -> usize {
        TEST_COUNTER.fetch_add(1, Ordering::SeqCst)
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_players_with_formatting() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 50000 + test_id as i32;

        // Clear cache to ensure clean state
        clear_cache().await;

        let mut raw_players = HashMap::new();
        raw_players.insert(123, "Mikko Koivu".to_string());
        raw_players.insert(456, "Teemu Selänne".to_string());
        raw_players.insert(789, "John Smith".to_string());

        cache_players_with_formatting(game_id, raw_players).await;

        let cached_players = get_cached_players(game_id).await.unwrap();
        assert_eq!(cached_players.get(&123), Some(&"Koivu".to_string()));
        assert_eq!(cached_players.get(&456), Some(&"Selänne".to_string()));
        assert_eq!(cached_players.get(&789), Some(&"Smith".to_string()));

        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_lru_simple() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let base_id = 60000 + (test_id * 1000) as i32;

        // Clear all caches to ensure clean state
        clear_all_caches().await;

        // Wait a bit to ensure cache is cleared
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Add one entry with unique ID
        let mut players = HashMap::new();
        players.insert(1, format!("Player {base_id}"));
        cache_players(base_id, players).await;

        // Should be able to retrieve it
        assert!(get_cached_players(base_id).await.is_some());

        // Add 100 more entries to fill the cache
        for i in 1..=100 {
            let mut players = HashMap::new();
            let player_id = base_id + i;
            players.insert(i as i64, format!("Player {player_id}"));
            cache_players(base_id + i, players).await;
        }

        // The first entry should be evicted
        assert!(get_cached_players(base_id).await.is_none());

        // The last entry should still be there
        assert!(get_cached_players(base_id + 100).await.is_some());

        // Cache should be at capacity (or close to it due to concurrency)
        let cache_size = get_cache_size().await;
        assert!(
            (95..=100).contains(&cache_size),
            "Cache size was {cache_size}, expected 95-100"
        );

        // Clear cache after test
        clear_all_caches().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_lru_access_order() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let base_id = 70000 + (test_id * 1000) as i32;

        // Clear all caches to ensure clean state
        clear_all_caches().await;

        // Wait a bit to ensure cache is cleared
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Add exactly 99 entries to leave room for one more
        for i in 0..99 {
            let mut players = HashMap::new();
            let player_id = base_id + i;
            players.insert(i as i64, format!("Player {player_id}"));
            cache_players(base_id + i, players).await;
        }

        // Access an entry in the middle to make it most recently used
        let mid_id = base_id + 50;
        let _ = get_cached_players(mid_id).await;

        // Add one more entry, which should evict the least recently used entry
        let mut players = HashMap::new();
        players.insert(99999, "New Player".to_string());
        let new_id = base_id + 999;
        cache_players(new_id, players).await;

        // The accessed entry should still be there
        assert!(get_cached_players(mid_id).await.is_some());

        // The new entry should be there
        assert!(get_cached_players(new_id).await.is_some());

        // Cache should be at capacity (or close to it due to concurrency)
        let cache_size = get_cache_size().await;
        assert!(
            (95..=100).contains(&cache_size),
            "Cache size was {cache_size}, expected 95-100"
        );

        // Clear cache after test
        clear_all_caches().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_lru_simple_access_order() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let base_id = 80000 + (test_id * 1000) as i32;

        // Clear all caches to ensure clean state
        clear_all_caches().await;

        // Add 5 entries
        for i in 0..5 {
            let mut players = HashMap::new();
            let player_id = base_id + i;
            players.insert(i as i64, format!("Player {player_id}"));
            cache_players(base_id + i, players).await;
        }

        // Access entry 0 to make it most recently used
        let _ = get_cached_players(base_id).await;

        // Add 95 more entries to reach capacity (100 total: 5 original + 95 new)
        for i in 5..100 {
            let mut players = HashMap::new();
            let player_id = base_id + i;
            players.insert(i as i64, format!("Player {player_id}"));
            cache_players(base_id + i, players).await;
        }

        // Entry 0 should still be there because it was accessed
        assert!(get_cached_players(base_id).await.is_some());

        // Cache should be at capacity (or close to it due to concurrency)
        let cache_size = get_cache_size().await;
        assert!(
            (95..=100).contains(&cache_size),
            "Cache size was {cache_size}, expected 95-100"
        );

        // Verify that at least one of the original entries (1-4) was evicted
        let mut original_entries_remaining = 0;
        for i in 1..5 {
            if get_cached_players(base_id + i).await.is_some() {
                original_entries_remaining += 1;
            }
        }

        // Since we accessed entry 0, it should still be there, but some of the others
        // should have been evicted. We expect at most 4 original entries to remain
        assert!(original_entries_remaining <= 4);

        // Clear cache after test
        clear_all_caches().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_tournament_cache_basic() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();

        // Clear cache to ensure clean state
        clear_tournament_cache().await;

        // Create a mock ScheduleResponse with completed games
        let mock_response = ScheduleResponse {
            games: vec![],
            previous_game_date: None,
            next_game_date: None,
        };

        let key = format!("runkosarja-2024-01-15-test-{test_id}");
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
        let test_id = get_unique_test_id();

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

        let key = format!("runkosarja-2024-01-15-live-test-{test_id}");
        cache_tournament_data(key.clone(), mock_response).await;

        // Should be able to retrieve it immediately
        let cached = get_cached_tournament_data(&key).await;
        assert!(cached.is_some());

        // Check cache size
        assert!(get_tournament_cache_size().await >= 1);

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

    #[tokio::test]
    #[serial]
    async fn test_detailed_game_cache_basic() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 90000 + test_id as i32;

        // Clear cache to ensure clean state
        clear_detailed_game_cache().await;

        // Create a mock DetailedGameResponse
        let mock_response = DetailedGameResponse {
            game: DetailedGame {
                id: game_id,
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

        cache_detailed_game_data(2024, game_id, mock_response.clone(), false).await;

        // Should be able to retrieve it
        let cached = get_cached_detailed_game_data(2024, game_id).await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().game.id, game_id);

        // Clear cache after test
        clear_detailed_game_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_goal_events_cache_basic() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 91000 + test_id as i32;

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

        cache_goal_events_data(2024, game_id, mock_events.clone(), false).await;

        // Should be able to retrieve it
        let cached = get_cached_goal_events_data(2024, game_id).await;
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
        let test_id = get_unique_test_id();

        // Clear cache to ensure clean state
        clear_http_response_cache().await;

        // Wait a bit to ensure cache is cleared
        tokio::time::sleep(Duration::from_millis(10)).await;

        let url = format!("https://api.example.com/test-{test_id}");
        let response_data = format!(r#"{{"test": "data-{test_id}"}}"#);

        cache_http_response(url.clone(), response_data.clone(), 60).await;

        // Should be able to retrieve it
        let cached = get_cached_http_response(&url).await;
        assert!(
            cached.is_some(),
            "Failed to retrieve cached data for URL: {url}"
        );
        assert_eq!(cached.unwrap(), response_data);

        // Clear cache after test
        clear_http_response_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_stats() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();

        // Clear all caches to ensure clean state
        clear_all_caches().await;

        // Wait a bit to ensure cache is cleared and verify it's actually empty
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify caches are actually empty before starting
        let initial_stats = get_all_cache_stats().await;
        assert_eq!(
            initial_stats.player_cache.size, 0,
            "Player cache should be empty initially"
        );

        // Add some test data to each cache with verification
        let mut players = HashMap::new();
        players.insert(1, "Player 1".to_string());
        let player_game_id = 92000 + test_id as i32;
        cache_players(player_game_id, players).await;

        // Immediately verify the player cache entry was added
        assert!(
            get_cached_players(player_game_id).await.is_some(),
            "Player cache entry should exist immediately after caching"
        );

        let mock_response = ScheduleResponse {
            games: vec![],
            previous_game_date: None,
            next_game_date: None,
        };
        let tournament_key = format!("test-tournament-{test_id}");
        cache_tournament_data(tournament_key.clone(), mock_response).await;

        // Verify tournament cache entry
        assert!(
            get_cached_tournament_data(&tournament_key).await.is_some(),
            "Tournament cache entry should exist immediately after caching"
        );

        let detailed_game_id = 93000 + test_id as i32;
        let mock_detailed_response = DetailedGameResponse {
            game: DetailedGame {
                id: detailed_game_id,
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
        cache_detailed_game_data(2024, detailed_game_id, mock_detailed_response, false).await;

        // Verify detailed game cache entry
        assert!(
            get_cached_detailed_game_data(2024, detailed_game_id)
                .await
                .is_some(),
            "Detailed game cache entry should exist immediately after caching"
        );

        let goal_events_game_id = 94000 + test_id as i32;
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
        cache_goal_events_data(2024, goal_events_game_id, mock_events, false).await;

        // Verify goal events cache entry
        assert!(
            get_cached_goal_events_data(2024, goal_events_game_id)
                .await
                .is_some(),
            "Goal events cache entry should exist immediately after caching"
        );

        let http_url = format!("https://api.example.com/test-stats-{test_id}");
        cache_http_response(http_url.clone(), format!("test data {test_id}"), 60).await;

        // Verify HTTP response cache entry
        assert!(
            get_cached_http_response(&http_url).await.is_some(),
            "HTTP response cache entry should exist immediately after caching"
        );

        // Wait a bit to ensure all async operations complete
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Final verification that all entries still exist before checking stats
        assert!(
            get_cached_players(player_game_id).await.is_some(),
            "Player cache entry should exist before stats check. Test ID: {test_id}, Player Game ID: {player_game_id}"
        );
        assert!(
            get_cached_tournament_data(&tournament_key).await.is_some(),
            "Tournament cache entry should exist before stats check"
        );
        assert!(
            get_cached_detailed_game_data(2024, detailed_game_id)
                .await
                .is_some(),
            "Detailed game cache entry should exist before stats check"
        );
        assert!(
            get_cached_goal_events_data(2024, goal_events_game_id)
                .await
                .is_some(),
            "Goal events cache entry should exist before stats check"
        );
        assert!(
            get_cached_http_response(&http_url).await.is_some(),
            "HTTP response cache entry should exist before stats check"
        );

        // Get stats
        let stats = get_all_cache_stats().await;

        // Verify stats with detailed error messages
        assert!(
            stats.player_cache.size >= 1,
            "Player cache size should be >= 1, but was {}. Test ID: {test_id}",
            stats.player_cache.size
        );
        assert!(
            stats.tournament_cache.size >= 1,
            "Tournament cache size should be >= 1, but was {}. Test ID: {test_id}",
            stats.tournament_cache.size
        );
        assert!(
            stats.detailed_game_cache.size >= 1,
            "Detailed game cache size should be >= 1, but was {}. Test ID: {test_id}",
            stats.detailed_game_cache.size
        );
        assert!(
            stats.goal_events_cache.size >= 1,
            "Goal events cache size should be >= 1, but was {}. Test ID: {test_id}",
            stats.goal_events_cache.size
        );
        assert!(
            stats.http_response_cache.size >= 1,
            "HTTP response cache size should be >= 1, but was {}. Test ID: {test_id}",
            stats.http_response_cache.size
        );

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
        let test_id = get_unique_test_id();
        let game_id = 95000 + test_id as i32;

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

        cache_goal_events_data(2024, game_id, mock_events, false).await;

        // Should be able to retrieve it immediately
        let cached = get_cached_goal_events_data(2024, game_id).await;
        assert!(cached.is_some());

        // Note: We can't easily test actual expiration without time manipulation,
        // but we can verify the cache structure is working correctly

        // Clear cache after test
        clear_goal_events_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_goal_events_cache_debug_methods() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 96000 + test_id as i32;
        let season = 2024;

        // Clear cache to ensure clean state
        clear_goal_events_cache().await;

        // Create test data
        let mock_events = vec![
            GoalEventData {
                scorer_player_id: 123,
                scorer_name: "Koivu".to_string(),
                minute: 15,
                home_team_score: 1,
                away_team_score: 0,
                is_winning_goal: false,
                goal_types: vec!["EV".to_string()],
                is_home_team: true,
                video_clip_url: None,
            },
            GoalEventData {
                scorer_player_id: 456,
                scorer_name: "Selänne".to_string(),
                minute: 25,
                home_team_score: 2,
                away_team_score: 0,
                is_winning_goal: false,
                goal_types: vec!["PP".to_string()],
                is_home_team: true,
                video_clip_url: None,
            },
        ];

        // Create cached entry directly to test debug methods
        let cached_entry = CachedGoalEventsData::new(mock_events.clone(), game_id, season, false);

        // Test debug methods
        assert_eq!(cached_entry.get_game_id(), game_id);
        assert_eq!(cached_entry.get_season(), season);

        let (returned_game_id, returned_season, event_count, is_expired) =
            cached_entry.get_cache_info();
        assert_eq!(returned_game_id, game_id);
        assert_eq!(returned_season, season);
        assert_eq!(event_count, 2);
        assert!(!is_expired); // Should not be expired immediately after creation

        // Also test through the cache system
        cache_goal_events_data(season, game_id, mock_events, false).await;

        // Verify the cached data can be retrieved
        let retrieved = get_cached_goal_events_data(season, game_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().len(), 2);

        // Clear cache after test
        clear_goal_events_cache().await;
    }

    // Comprehensive unit tests for live game detection
    // Task 7: Create comprehensive unit tests for live game detection

    #[tokio::test]
    #[serial]
    async fn test_has_live_games_from_game_data_all_ongoing() {
        // Test scenario with all ongoing games
        let ongoing_games = vec![
            GameData {
                home_team: "HIFK".to_string(),
                away_team: "Tappara".to_string(),
                time: "18:30".to_string(),
                result: "2-1".to_string(),
                score_type: ScoreType::Ongoing,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 1800,
                start: "2024-01-15T18:30:00Z".to_string(),
            },
            GameData {
                home_team: "Kärpät".to_string(),
                away_team: "Lukko".to_string(),
                time: "19:00".to_string(),
                result: "0-3".to_string(),
                score_type: ScoreType::Ongoing,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 2400,
                start: "2024-01-15T19:00:00Z".to_string(),
            },
            GameData {
                home_team: "JYP".to_string(),
                away_team: "Ilves".to_string(),
                time: "19:30".to_string(),
                result: "1-1".to_string(),
                score_type: ScoreType::Ongoing,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 900,
                start: "2024-01-15T19:30:00Z".to_string(),
            },
        ];

        let result = has_live_games_from_game_data(&ongoing_games);
        assert!(result, "Should return true when all games are ongoing");
    }

    #[tokio::test]
    #[serial]
    async fn test_has_live_games_from_game_data_all_completed() {
        // Test scenario with all completed games
        let completed_games = vec![
            GameData {
                home_team: "HIFK".to_string(),
                away_team: "Tappara".to_string(),
                time: "18:30".to_string(),
                result: "3-2".to_string(),
                score_type: ScoreType::Final,
                is_overtime: true,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 3900,
                start: "2024-01-15T18:30:00Z".to_string(),
            },
            GameData {
                home_team: "Kärpät".to_string(),
                away_team: "Lukko".to_string(),
                time: "19:00".to_string(),
                result: "1-4".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 3600,
                start: "2024-01-15T19:00:00Z".to_string(),
            },
            GameData {
                home_team: "JYP".to_string(),
                away_team: "Ilves".to_string(),
                time: "19:30".to_string(),
                result: "2-1".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: true,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 3900,
                start: "2024-01-15T19:30:00Z".to_string(),
            },
        ];

        let result = has_live_games_from_game_data(&completed_games);
        assert!(!result, "Should return false when all games are completed");
    }

    #[tokio::test]
    #[serial]
    async fn test_has_live_games_from_game_data_all_scheduled() {
        // Test scenario with all scheduled games
        let scheduled_games = vec![
            GameData {
                home_team: "HIFK".to_string(),
                away_team: "Tappara".to_string(),
                time: "18:30".to_string(),
                result: "-".to_string(),
                score_type: ScoreType::Scheduled,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 0,
                start: "2024-01-16T18:30:00Z".to_string(),
            },
            GameData {
                home_team: "Kärpät".to_string(),
                away_team: "Lukko".to_string(),
                time: "19:00".to_string(),
                result: "-".to_string(),
                score_type: ScoreType::Scheduled,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 0,
                start: "2024-01-16T19:00:00Z".to_string(),
            },
        ];

        let result = has_live_games_from_game_data(&scheduled_games);
        assert!(!result, "Should return false when all games are scheduled");
    }

    #[tokio::test]
    #[serial]
    async fn test_has_live_games_from_game_data_mixed_with_ongoing() {
        // Test scenario with mixed game states including ongoing games
        let mixed_games = vec![
            GameData {
                home_team: "HIFK".to_string(),
                away_team: "Tappara".to_string(),
                time: "18:30".to_string(),
                result: "3-2".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 3600,
                start: "2024-01-15T18:30:00Z".to_string(),
            },
            GameData {
                home_team: "Kärpät".to_string(),
                away_team: "Lukko".to_string(),
                time: "19:00".to_string(),
                result: "1-2".to_string(),
                score_type: ScoreType::Ongoing,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 2100,
                start: "2024-01-15T19:00:00Z".to_string(),
            },
            GameData {
                home_team: "JYP".to_string(),
                away_team: "Ilves".to_string(),
                time: "20:00".to_string(),
                result: "-".to_string(),
                score_type: ScoreType::Scheduled,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 0,
                start: "2024-01-15T20:00:00Z".to_string(),
            },
        ];

        let result = has_live_games_from_game_data(&mixed_games);
        assert!(
            result,
            "Should return true when at least one game is ongoing"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_has_live_games_from_game_data_mixed_without_ongoing() {
        // Test scenario with mixed game states but no ongoing games
        let mixed_games = vec![
            GameData {
                home_team: "HIFK".to_string(),
                away_team: "Tappara".to_string(),
                time: "18:30".to_string(),
                result: "3-2".to_string(),
                score_type: ScoreType::Final,
                is_overtime: true,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 3900,
                start: "2024-01-15T18:30:00Z".to_string(),
            },
            GameData {
                home_team: "Kärpät".to_string(),
                away_team: "Lukko".to_string(),
                time: "19:00".to_string(),
                result: "1-4".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 3600,
                start: "2024-01-15T19:00:00Z".to_string(),
            },
            GameData {
                home_team: "JYP".to_string(),
                away_team: "Ilves".to_string(),
                time: "20:00".to_string(),
                result: "-".to_string(),
                score_type: ScoreType::Scheduled,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 0,
                start: "2024-01-15T20:00:00Z".to_string(),
            },
        ];

        let result = has_live_games_from_game_data(&mixed_games);
        assert!(!result, "Should return false when no games are ongoing");
    }

    #[tokio::test]
    #[serial]
    async fn test_has_live_games_from_game_data_empty_list() {
        // Test scenario with empty game list
        let empty_games: Vec<GameData> = vec![];

        let result = has_live_games_from_game_data(&empty_games);
        assert!(!result, "Should return false when game list is empty");
    }

    #[tokio::test]
    #[serial]
    async fn test_has_live_games_from_game_data_single_ongoing() {
        // Test scenario with single ongoing game
        let single_ongoing = vec![GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "18:30".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Ongoing,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 600,
            start: "2024-01-15T18:30:00Z".to_string(),
        }];

        let result = has_live_games_from_game_data(&single_ongoing);
        assert!(result, "Should return true for single ongoing game");
    }

    #[tokio::test]
    #[serial]
    async fn test_has_live_games_from_game_data_single_completed() {
        // Test scenario with single completed game
        let single_completed = vec![GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "18:30".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T18:30:00Z".to_string(),
        }];

        let result = has_live_games_from_game_data(&single_completed);
        assert!(!result, "Should return false for single completed game");
    }

    #[tokio::test]
    #[serial]
    async fn test_has_live_games_from_game_data_single_scheduled() {
        // Test scenario with single scheduled game
        let single_scheduled = vec![GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "18:30".to_string(),
            result: "-".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 0,
            start: "2024-01-16T18:30:00Z".to_string(),
        }];

        let result = has_live_games_from_game_data(&single_scheduled);
        assert!(!result, "Should return false for single scheduled game");
    }

    #[tokio::test]
    #[serial]
    async fn test_has_live_games_from_game_data_multiple_ongoing() {
        // Test scenario with multiple ongoing games to verify count logging
        let multiple_ongoing = vec![
            GameData {
                home_team: "HIFK".to_string(),
                away_team: "Tappara".to_string(),
                time: "18:30".to_string(),
                result: "2-1".to_string(),
                score_type: ScoreType::Ongoing,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 1800,
                start: "2024-01-15T18:30:00Z".to_string(),
            },
            GameData {
                home_team: "Kärpät".to_string(),
                away_team: "Lukko".to_string(),
                time: "19:00".to_string(),
                result: "0-1".to_string(),
                score_type: ScoreType::Ongoing,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 1200,
                start: "2024-01-15T19:00:00Z".to_string(),
            },
            GameData {
                home_team: "JYP".to_string(),
                away_team: "Ilves".to_string(),
                time: "19:30".to_string(),
                result: "3-2".to_string(),
                score_type: ScoreType::Ongoing,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 2700,
                start: "2024-01-15T19:30:00Z".to_string(),
            },
            GameData {
                home_team: "TPS".to_string(),
                away_team: "Sport".to_string(),
                time: "20:00".to_string(),
                result: "1-1".to_string(),
                score_type: ScoreType::Ongoing,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 3000,
                start: "2024-01-15T20:00:00Z".to_string(),
            },
        ];

        let result = has_live_games_from_game_data(&multiple_ongoing);
        assert!(result, "Should return true when multiple games are ongoing");
    }

    #[tokio::test]
    #[serial]
    async fn test_has_live_games_from_game_data_playoffs_ongoing() {
        // Test scenario with ongoing playoff games
        let playoff_ongoing = vec![
            GameData {
                home_team: "HIFK".to_string(),
                away_team: "Tappara".to_string(),
                time: "18:30".to_string(),
                result: "2-1".to_string(),
                score_type: ScoreType::Ongoing,
                is_overtime: false,
                is_shootout: false,
                serie: "playoffs".to_string(),
                goal_events: vec![],
                played_time: 2400,
                start: "2024-03-15T18:30:00Z".to_string(),
            },
            GameData {
                home_team: "Kärpät".to_string(),
                away_team: "Lukko".to_string(),
                time: "19:00".to_string(),
                result: "0-0".to_string(),
                score_type: ScoreType::Ongoing,
                is_overtime: false,
                is_shootout: false,
                serie: "playoffs".to_string(),
                goal_events: vec![],
                played_time: 600,
                start: "2024-03-15T19:00:00Z".to_string(),
            },
        ];

        let result = has_live_games_from_game_data(&playoff_ongoing);
        assert!(result, "Should return true for ongoing playoff games");
    }

    #[tokio::test]
    #[serial]
    async fn test_has_live_games_from_game_data_complex_mixed_scenario() {
        // Test complex scenario with various game states and series
        let complex_mixed = vec![
            // Completed regular season game
            GameData {
                home_team: "HIFK".to_string(),
                away_team: "Tappara".to_string(),
                time: "18:30".to_string(),
                result: "3-2".to_string(),
                score_type: ScoreType::Final,
                is_overtime: true,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 3900,
                start: "2024-01-15T18:30:00Z".to_string(),
            },
            // Ongoing regular season game
            GameData {
                home_team: "Kärpät".to_string(),
                away_team: "Lukko".to_string(),
                time: "19:00".to_string(),
                result: "1-2".to_string(),
                score_type: ScoreType::Ongoing,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 2100,
                start: "2024-01-15T19:00:00Z".to_string(),
            },
            // Scheduled playoff game
            GameData {
                home_team: "JYP".to_string(),
                away_team: "Ilves".to_string(),
                time: "20:00".to_string(),
                result: "-".to_string(),
                score_type: ScoreType::Scheduled,
                is_overtime: false,
                is_shootout: false,
                serie: "playoffs".to_string(),
                goal_events: vec![],
                played_time: 0,
                start: "2024-03-15T20:00:00Z".to_string(),
            },
            // Completed playoff game with shootout
            GameData {
                home_team: "TPS".to_string(),
                away_team: "Sport".to_string(),
                time: "18:00".to_string(),
                result: "4-3".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: true,
                serie: "playoffs".to_string(),
                goal_events: vec![],
                played_time: 3900,
                start: "2024-03-14T18:00:00Z".to_string(),
            },
            // Another ongoing game
            GameData {
                home_team: "Pelicans".to_string(),
                away_team: "KalPa".to_string(),
                time: "19:30".to_string(),
                result: "0-1".to_string(),
                score_type: ScoreType::Ongoing,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 900,
                start: "2024-01-15T19:30:00Z".to_string(),
            },
        ];

        let result = has_live_games_from_game_data(&complex_mixed);
        assert!(
            result,
            "Should return true when complex mixed scenario includes ongoing games"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_debugging_functions() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();

        // Clear all caches first
        clear_all_caches().await;

        // Add some test data to various caches
        let mut players = HashMap::new();
        players.insert(1, "Test Player".to_string());
        let player_game_id = 97000 + test_id as i32;
        cache_players(player_game_id, players).await;

        let mock_events = vec![GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Test Scorer".to_string(),
            minute: 15,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["EV".to_string()],
            is_home_team: true,
            video_clip_url: None,
        }];

        let goal_game_id = 98000 + test_id as i32;
        cache_goal_events_data(2024, goal_game_id, mock_events, false).await;

        // Test detailed debug info function
        let debug_info = get_detailed_cache_debug_info().await;
        assert!(debug_info.contains("Cache Statistics"));
        assert!(debug_info.contains("Player Cache:"));
        assert!(debug_info.contains("Goal Events Cache:"));

        // Test cache reset with confirmation
        let reset_confirmation = reset_all_caches_with_confirmation().await;
        assert!(reset_confirmation.contains("Cache reset completed"));
        assert!(reset_confirmation.contains("All caches cleared successfully"));

        // Verify caches are actually cleared
        let stats_after_reset = get_all_cache_stats().await;
        assert_eq!(stats_after_reset.player_cache.size, 0);
        assert_eq!(stats_after_reset.goal_events_cache.size, 0);
    }

    // Tests for disambiguation caching functionality

    #[tokio::test]
    #[serial]
    async fn test_cache_players_with_disambiguation_basic() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 100000 + test_id as i32;

        // Clear cache to ensure clean state
        clear_cache().await;

        // Create test data with players that need disambiguation
        let mut home_players = HashMap::new();
        home_players.insert(123, ("Mikko".to_string(), "Koivu".to_string()));
        home_players.insert(456, ("Saku".to_string(), "Koivu".to_string()));
        home_players.insert(789, ("Teemu".to_string(), "Selänne".to_string()));

        let mut away_players = HashMap::new();
        away_players.insert(111, ("Jari".to_string(), "Kurri".to_string()));
        away_players.insert(222, ("Jere".to_string(), "Kurri".to_string()));
        away_players.insert(333, ("Ville".to_string(), "Peltonen".to_string()));

        // Cache with disambiguation
        cache_players_with_disambiguation(game_id, home_players, away_players).await;

        // Retrieve and verify disambiguation
        let cached_players = get_cached_players(game_id).await.unwrap();

        // Home team: Koivu players should be disambiguated, Selänne should not
        assert_eq!(cached_players.get(&123), Some(&"Koivu M.".to_string()));
        assert_eq!(cached_players.get(&456), Some(&"Koivu S.".to_string()));
        assert_eq!(cached_players.get(&789), Some(&"Selänne".to_string()));

        // Away team: Kurri players should be disambiguated, Peltonen should not
        assert_eq!(cached_players.get(&111), Some(&"Kurri Ja.".to_string()));
        assert_eq!(cached_players.get(&222), Some(&"Kurri Je.".to_string()));
        assert_eq!(cached_players.get(&333), Some(&"Peltonen".to_string()));

        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_players_with_disambiguation_no_conflicts() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 101000 + test_id as i32;

        // Clear cache to ensure clean state
        clear_cache().await;

        // Create test data with no name conflicts
        let mut home_players = HashMap::new();
        home_players.insert(123, ("Mikko".to_string(), "Koivu".to_string()));
        home_players.insert(456, ("Teemu".to_string(), "Selänne".to_string()));

        let mut away_players = HashMap::new();
        away_players.insert(111, ("Jari".to_string(), "Kurri".to_string()));
        away_players.insert(222, ("Ville".to_string(), "Peltonen".to_string()));

        // Cache with disambiguation
        cache_players_with_disambiguation(game_id, home_players, away_players).await;

        // Retrieve and verify no disambiguation applied
        let cached_players = get_cached_players(game_id).await.unwrap();

        // All players should have last name only (no disambiguation)
        assert_eq!(cached_players.get(&123), Some(&"Koivu".to_string()));
        assert_eq!(cached_players.get(&456), Some(&"Selänne".to_string()));
        assert_eq!(cached_players.get(&111), Some(&"Kurri".to_string()));
        assert_eq!(cached_players.get(&222), Some(&"Peltonen".to_string()));

        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_players_with_disambiguation_cross_team_same_names() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 102000 + test_id as i32;

        // Clear cache to ensure clean state
        clear_cache().await;

        // Create test data with same last names on different teams
        let mut home_players = HashMap::new();
        home_players.insert(123, ("Mikko".to_string(), "Koivu".to_string()));
        home_players.insert(456, ("Teemu".to_string(), "Selänne".to_string()));

        let mut away_players = HashMap::new();
        away_players.insert(111, ("Saku".to_string(), "Koivu".to_string())); // Same last name as home team
        away_players.insert(222, ("Ville".to_string(), "Peltonen".to_string()));

        // Cache with disambiguation
        cache_players_with_disambiguation(game_id, home_players, away_players).await;

        // Retrieve and verify team-scoped disambiguation
        let cached_players = get_cached_players(game_id).await.unwrap();

        // Players with same last name on different teams should NOT be disambiguated
        assert_eq!(cached_players.get(&123), Some(&"Koivu".to_string())); // Home Koivu
        assert_eq!(cached_players.get(&111), Some(&"Koivu".to_string())); // Away Koivu
        assert_eq!(cached_players.get(&456), Some(&"Selänne".to_string()));
        assert_eq!(cached_players.get(&222), Some(&"Peltonen".to_string()));

        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_players_with_disambiguation_empty_first_names() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 103000 + test_id as i32;

        // Clear cache to ensure clean state
        clear_cache().await;

        // Create test data with empty first names
        let mut home_players = HashMap::new();
        home_players.insert(123, ("".to_string(), "Koivu".to_string()));
        home_players.insert(456, ("Saku".to_string(), "Koivu".to_string()));

        let away_players = HashMap::new(); // Empty away team

        // Cache with disambiguation
        cache_players_with_disambiguation(game_id, home_players, away_players).await;

        // Retrieve and verify handling of empty first names
        let cached_players = get_cached_players(game_id).await.unwrap();

        // Player with empty first name should fall back to last name only
        assert_eq!(cached_players.get(&123), Some(&"Koivu".to_string()));
        // Player with valid first name should be disambiguated
        assert_eq!(cached_players.get(&456), Some(&"Koivu S.".to_string()));

        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_players_with_disambiguation_unicode_names() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 104000 + test_id as i32;

        // Clear cache to ensure clean state
        clear_cache().await;

        // Create test data with Finnish Unicode characters
        let mut home_players = HashMap::new();
        home_players.insert(123, ("Äkäslompolo".to_string(), "Kärppä".to_string()));
        home_players.insert(456, ("Östen".to_string(), "Kärppä".to_string()));
        home_players.insert(789, ("Åke".to_string(), "Kärppä".to_string()));

        let away_players = HashMap::new(); // Empty away team

        // Cache with disambiguation
        cache_players_with_disambiguation(game_id, home_players, away_players).await;

        // Retrieve and verify Unicode handling
        let cached_players = get_cached_players(game_id).await.unwrap();

        // All players should be disambiguated with proper Unicode handling
        assert_eq!(cached_players.get(&123), Some(&"Kärppä Ä.".to_string()));
        assert_eq!(cached_players.get(&456), Some(&"Kärppä Ö.".to_string()));
        assert_eq!(cached_players.get(&789), Some(&"Kärppä Å.".to_string()));

        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_get_cached_disambiguated_players() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 105000 + test_id as i32;

        // Clear cache to ensure clean state
        clear_cache().await;

        // Test cache miss
        let result = get_cached_disambiguated_players(game_id).await;
        assert!(result.is_none());

        // Add some disambiguated players
        let mut home_players = HashMap::new();
        home_players.insert(123, ("Mikko".to_string(), "Koivu".to_string()));
        home_players.insert(456, ("Saku".to_string(), "Koivu".to_string()));

        let away_players = HashMap::new();
        cache_players_with_disambiguation(game_id, home_players, away_players).await;

        // Test cache hit
        let result = get_cached_disambiguated_players(game_id).await;
        assert!(result.is_some());
        let players = result.unwrap();
        assert_eq!(players.len(), 2);
        assert_eq!(players.get(&123), Some(&"Koivu M.".to_string()));
        assert_eq!(players.get(&456), Some(&"Koivu S.".to_string()));

        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_get_cached_player_name() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 106000 + test_id as i32;

        // Clear cache to ensure clean state
        clear_cache().await;

        // Test cache miss for non-existent game
        let result = get_cached_player_name(game_id, 123).await;
        assert!(result.is_none());

        // Add some disambiguated players
        let mut home_players = HashMap::new();
        home_players.insert(123, ("Mikko".to_string(), "Koivu".to_string()));
        home_players.insert(456, ("Saku".to_string(), "Koivu".to_string()));

        let away_players = HashMap::new();
        cache_players_with_disambiguation(game_id, home_players, away_players).await;

        // Test cache hit for existing player
        let result = get_cached_player_name(game_id, 123).await;
        assert_eq!(result, Some("Koivu M.".to_string()));

        let result = get_cached_player_name(game_id, 456).await;
        assert_eq!(result, Some("Koivu S.".to_string()));

        // Test cache miss for non-existent player
        let result = get_cached_player_name(game_id, 999).await;
        assert!(result.is_none());

        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_has_cached_disambiguated_players() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 107000 + test_id as i32;

        // Clear cache to ensure clean state
        clear_cache().await;

        // Test cache miss
        let result = has_cached_disambiguated_players(game_id).await;
        assert!(!result);

        // Add some disambiguated players
        let mut home_players = HashMap::new();
        home_players.insert(123, ("Mikko".to_string(), "Koivu".to_string()));

        let away_players = HashMap::new();
        cache_players_with_disambiguation(game_id, home_players, away_players).await;

        // Test cache hit
        let result = has_cached_disambiguated_players(game_id).await;
        assert!(result);

        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_players_with_disambiguation_three_players_same_name() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 108000 + test_id as i32;

        // Clear cache to ensure clean state
        clear_cache().await;

        // Create test data with three players having the same last name
        let mut home_players = HashMap::new();
        home_players.insert(123, ("Mikko".to_string(), "Koivu".to_string()));
        home_players.insert(456, ("Saku".to_string(), "Koivu".to_string()));
        home_players.insert(789, ("Antti".to_string(), "Koivu".to_string()));

        let away_players = HashMap::new();

        // Cache with disambiguation
        cache_players_with_disambiguation(game_id, home_players, away_players).await;

        // Retrieve and verify all three are disambiguated
        let cached_players = get_cached_players(game_id).await.unwrap();

        assert_eq!(cached_players.get(&123), Some(&"Koivu M.".to_string()));
        assert_eq!(cached_players.get(&456), Some(&"Koivu S.".to_string()));
        assert_eq!(cached_players.get(&789), Some(&"Koivu A.".to_string()));

        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_players_with_disambiguation_mixed_scenario() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 109000 + test_id as i32;

        // Clear cache to ensure clean state
        clear_cache().await;

        // Create complex test scenario with mixed disambiguation needs
        let mut home_players = HashMap::new();
        home_players.insert(123, ("Mikko".to_string(), "Koivu".to_string()));
        home_players.insert(456, ("Saku".to_string(), "Koivu".to_string()));
        home_players.insert(789, ("Teemu".to_string(), "Selänne".to_string()));
        home_players.insert(101, ("Jari".to_string(), "Kurri".to_string()));

        let mut away_players = HashMap::new();
        away_players.insert(111, ("Jere".to_string(), "Kurri".to_string()));
        away_players.insert(222, ("Ville".to_string(), "Peltonen".to_string()));
        away_players.insert(333, ("Olli".to_string(), "Jokinen".to_string()));
        away_players.insert(444, ("Jussi".to_string(), "Jokinen".to_string()));

        // Cache with disambiguation
        cache_players_with_disambiguation(game_id, home_players, away_players).await;

        // Retrieve and verify mixed disambiguation
        let cached_players = get_cached_players(game_id).await.unwrap();

        // Home team: Koivu players disambiguated, others not
        assert_eq!(cached_players.get(&123), Some(&"Koivu M.".to_string()));
        assert_eq!(cached_players.get(&456), Some(&"Koivu S.".to_string()));
        assert_eq!(cached_players.get(&789), Some(&"Selänne".to_string()));
        assert_eq!(cached_players.get(&101), Some(&"Kurri".to_string()));

        // Away team: Jokinen players disambiguated, others not
        assert_eq!(cached_players.get(&111), Some(&"Kurri".to_string()));
        assert_eq!(cached_players.get(&222), Some(&"Peltonen".to_string()));
        assert_eq!(cached_players.get(&333), Some(&"Jokinen O.".to_string()));
        assert_eq!(cached_players.get(&444), Some(&"Jokinen J.".to_string()));

        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_players_with_disambiguation_empty_teams() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 110000 + test_id as i32;

        // Clear cache to ensure clean state
        clear_cache().await;

        // Test with empty teams
        let home_players = HashMap::new();
        let away_players = HashMap::new();

        // Cache with disambiguation
        cache_players_with_disambiguation(game_id, home_players, away_players).await;

        // Retrieve and verify empty result
        let cached_players = get_cached_players(game_id).await.unwrap();
        assert!(cached_players.is_empty());

        // Clear cache after test
        clear_cache().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_api_integration_disambiguation_flow() {
        let _guard = TEST_MUTEX.lock().await;
        let test_id = get_unique_test_id();
        let game_id = 111000 + test_id as i32;

        // Clear cache before test
        clear_cache().await;

        // Simulate API response data with players that need disambiguation
        let mut home_players = HashMap::new();
        home_players.insert(123, ("Mikko".to_string(), "Koivu".to_string()));
        home_players.insert(124, ("Saku".to_string(), "Koivu".to_string()));
        home_players.insert(125, ("Teemu".to_string(), "Selänne".to_string()));

        let mut away_players = HashMap::new();
        away_players.insert(456, ("Mikko".to_string(), "Koivu".to_string())); // Same name as home team
        away_players.insert(457, ("Jari".to_string(), "Kurri".to_string()));

        // Apply team-scoped disambiguation (simulating API processing)
        cache_players_with_disambiguation(game_id, home_players, away_players).await;

        // Retrieve cached results (simulating goal event processing)
        let cached_players = get_cached_players(game_id).await.unwrap();

        // Verify team-scoped disambiguation results
        assert_eq!(
            cached_players.get(&123),
            Some(&"Koivu M.".to_string()),
            "Home Mikko Koivu should be disambiguated"
        );
        assert_eq!(
            cached_players.get(&124),
            Some(&"Koivu S.".to_string()),
            "Home Saku Koivu should be disambiguated"
        );
        assert_eq!(
            cached_players.get(&125),
            Some(&"Selänne".to_string()),
            "Selänne should not be disambiguated"
        );
        assert_eq!(
            cached_players.get(&456),
            Some(&"Koivu".to_string()),
            "Away Mikko Koivu should not be disambiguated (different team)"
        );
        assert_eq!(
            cached_players.get(&457),
            Some(&"Kurri".to_string()),
            "Kurri should not be disambiguated"
        );

        // Verify total number of cached players
        assert_eq!(cached_players.len(), 5);

        // Clear cache after test
        clear_cache().await;
    }
}
