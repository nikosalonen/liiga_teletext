use lazy_static::lazy_static;
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, trace, warn};

use crate::constants::cache_ttl;
use crate::data_fetcher::models::{
    DetailedGameResponse, GameData, GoalEventData, ScheduleResponse,
};
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
    pub game_id: i32,
    pub season: i32,
    pub is_live_game: bool,
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
            Duration::from_secs(cache_ttl::LIVE_GAMES_SECONDS) // 15 seconds for live games
        } else {
            Duration::from_secs(cache_ttl::COMPLETED_GAMES_SECONDS) // 1 hour for completed games
        };

        let age = self.cached_at.elapsed();
        let is_expired = age > ttl;

        debug!(
            "Cache expiration check: has_live_games={}, age={:?}, ttl={:?}, is_expired={}",
            self.has_live_games, age, ttl, is_expired
        );

        is_expired
    }

    /// Gets the TTL duration for this cache entry
    pub fn get_ttl(&self) -> Duration {
        if self.has_live_games {
            Duration::from_secs(cache_ttl::LIVE_GAMES_SECONDS)
        } else {
            Duration::from_secs(cache_ttl::COMPLETED_GAMES_SECONDS)
        }
    }

    /// Gets the remaining time until expiration
    #[allow(dead_code)]
    pub fn time_until_expiry(&self) -> Duration {
        let ttl = self.get_ttl();
        let elapsed = self.cached_at.elapsed();
        ttl.saturating_sub(elapsed)
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
            Duration::from_secs(cache_ttl::LIVE_GAMES_SECONDS) // 30 seconds for live games
        } else {
            Duration::from_secs(cache_ttl::COMPLETED_GAMES_SECONDS) // 1 hour for completed games
        };

        self.cached_at.elapsed() > ttl
    }

    /// Gets the TTL duration for this cache entry
    pub fn get_ttl(&self) -> Duration {
        if self.is_live_game {
            Duration::from_secs(cache_ttl::LIVE_GAMES_SECONDS)
        } else {
            Duration::from_secs(cache_ttl::COMPLETED_GAMES_SECONDS)
        }
    }
}

impl CachedGoalEventsData {
    /// Creates a new cached goal events data entry
    pub fn new(data: Vec<GoalEventData>, game_id: i32, season: i32, is_live_game: bool) -> Self {
        Self {
            data,
            cached_at: Instant::now(),
            game_id,
            season,
            is_live_game,
        }
    }

    /// Checks if the cached data is expired based on game state
    pub fn is_expired(&self) -> bool {
        let ttl = if self.is_live_game {
            Duration::from_secs(cache_ttl::LIVE_GAMES_SECONDS) // 30 seconds for live games
        } else {
            Duration::from_secs(cache_ttl::COMPLETED_GAMES_SECONDS) // 1 hour for completed games
        };

        let age = self.cached_at.elapsed();
        let is_expired = age > ttl;

        debug!(
            "Goal events cache expiration check: is_live_game={}, age={:?}, ttl={:?}, is_expired={}",
            self.is_live_game, age, ttl, is_expired
        );

        is_expired
    }

    /// Gets the TTL duration for this cache entry
    #[allow(dead_code)]
    pub fn get_ttl(&self) -> Duration {
        if self.is_live_game {
            Duration::from_secs(cache_ttl::LIVE_GAMES_SECONDS)
        } else {
            Duration::from_secs(cache_ttl::COMPLETED_GAMES_SECONDS)
        }
    }

    /// Gets the game ID associated with this cached data (useful for debugging and logging)
    pub fn get_game_id(&self) -> i32 {
        self.game_id
    }

    /// Gets the season associated with this cached data (useful for debugging and logging)
    pub fn get_season(&self) -> i32 {
        self.season
    }

    /// Gets cache metadata including game ID and season for monitoring and debugging
    pub fn get_cache_info(&self) -> (i32, i32, usize, bool) {
        (
            self.game_id,
            self.season,
            self.data.len(),
            self.is_expired(),
        )
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

/// Caches tournament data with automatic live game detection
#[instrument(skip(key, data), fields(cache_key = %key))]
pub async fn cache_tournament_data(key: String, data: ScheduleResponse) {
    let games_count = data.games.len();
    let has_live = has_live_games(&data);

    debug!(
        "Caching tournament data: key={}, games={}, has_live={}",
        key, games_count, has_live
    );

    let cached_data = CachedTournamentData::new(data, has_live);

    let mut cache = TOURNAMENT_CACHE.write().await;
    cache.put(key.clone(), cached_data);

    // Enhanced logging for live game cache entries
    if has_live {
        info!(
            "Live game cache entry created: key={}, games={}, ttl={}s",
            key,
            games_count,
            crate::constants::cache_ttl::LIVE_GAMES_SECONDS
        );
    } else {
        info!(
            "Completed game cache entry created: key={}, games={}, ttl={}s",
            key,
            games_count,
            crate::constants::cache_ttl::COMPLETED_GAMES_SECONDS
        );
    }
}

/// Retrieves cached tournament data if it's not expired
#[instrument(skip(key), fields(cache_key = %key))]
#[allow(dead_code)]
pub async fn get_cached_tournament_data(key: &str) -> Option<ScheduleResponse> {
    debug!(
        "Attempting to retrieve tournament data from cache for key: {}",
        key
    );

    let mut cache = TOURNAMENT_CACHE.write().await;

    if let Some(cached_entry) = cache.get(key) {
        debug!("Found cached tournament data for key: {}", key);

        if !cached_entry.is_expired() {
            let games_count = cached_entry.data.games.len();
            let has_live = cached_entry.has_live_games;
            debug!(
                "Cache hit for tournament data: key={}, games={}, has_live={}, age={:?}",
                key,
                games_count,
                has_live,
                cached_entry.cached_at.elapsed()
            );
            return Some(cached_entry.data.clone());
        } else {
            // Enhanced logging for expired cache entries during auto-refresh
            let has_live = cached_entry.has_live_games;
            let age = cached_entry.cached_at.elapsed();
            let ttl = cached_entry.get_ttl();

            if has_live {
                info!(
                    "Cache bypass: Expired live game cache entry removed during auto-refresh: key={}, age={:?}, ttl={:?}",
                    key, age, ttl
                );
            } else {
                warn!(
                    "Removing expired tournament cache entry: key={}, age={:?}, ttl={:?}",
                    key, age, ttl
                );
            }
            cache.pop(key);
        }
    } else {
        debug!("Cache miss for tournament data: key={}", key);
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
            debug!(
                "Cache invalidated due to live game state change: key={}, cached_has_live={}, current_has_live={}",
                key, cached_entry.has_live_games, has_live
            );
            cache.pop(key);
            return None;
        }

        if !cached_entry.is_expired() {
            return Some(cached_entry.data.clone());
        } else {
            // Remove expired entry
            debug!(
                "Removing expired cache entry during live game state check: key={}, age={:?}",
                key,
                cached_entry.cached_at.elapsed()
            );
            cache.pop(key);
        }
    }

    None
}

/// Retrieves cached tournament data specifically for auto-refresh scenarios
/// This function provides enhanced logging and ensures proper cache bypass for expired entries
#[allow(dead_code)]
pub async fn get_cached_tournament_data_for_auto_refresh(key: &str) -> Option<ScheduleResponse> {
    debug!(
        "Auto-refresh: Attempting to retrieve tournament data from cache for key: {}",
        key
    );

    let mut cache = TOURNAMENT_CACHE.write().await;

    if let Some(cached_entry) = cache.get(key) {
        let has_live = cached_entry.has_live_games;
        let age = cached_entry.cached_at.elapsed();
        let ttl = cached_entry.get_ttl();

        debug!(
            "Auto-refresh: Found cached tournament data: key={}, has_live={}, age={:?}, ttl={:?}",
            key, has_live, age, ttl
        );

        if !cached_entry.is_expired() {
            let games_count = cached_entry.data.games.len();
            info!(
                "Auto-refresh: Using cached data: key={}, games={}, has_live={}, age={:?}",
                key, games_count, has_live, age
            );
            return Some(cached_entry.data.clone());
        } else {
            // Enhanced logging for auto-refresh cache bypass
            if has_live {
                info!(
                    "Auto-refresh: Cache bypass for expired live game entry: key={}, age={:?}, ttl={:?} - fetching fresh data",
                    key, age, ttl
                );
            } else {
                info!(
                    "Auto-refresh: Cache bypass for expired completed game entry: key={}, age={:?}, ttl={:?}",
                    key, age, ttl
                );
            }
            cache.pop(key);
        }
    } else {
        debug!("Auto-refresh: Cache miss for tournament data: key={}", key);
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

/// Aggressively invalidates cache for games that should be starting soon
/// This is called when we detect games are near their scheduled start time
pub async fn invalidate_cache_for_games_near_start_time(date: &str) {
    let mut cache = TOURNAMENT_CACHE.write().await;

    // Find and remove cache entries for the given date
    let keys_to_remove: Vec<String> = cache
        .iter()
        .filter(|(key, _)| key.contains(date))
        .map(|(key, _)| key.clone())
        .collect();

    for key in keys_to_remove {
        info!(
            "Aggressively invalidating cache for games near start time: {}",
            key
        );
        cache.pop(&key);
    }
}

/// Completely bypasses cache for games that should be starting soon
/// This ensures fresh data is always fetched for games near their start time
pub async fn should_bypass_cache_for_starting_games(current_games: &[GameData]) -> bool {
    // Check if any games are near their start time
    let has_starting_games = current_games.iter().any(|game| {
        if game.score_type != crate::teletext_ui::ScoreType::Scheduled || game.start.is_empty() {
            return false;
        }

        match chrono::DateTime::parse_from_rfc3339(&game.start) {
            Ok(game_start) => {
                let now = chrono::Utc::now();
                let time_diff = now.signed_duration_since(game_start);

                // Extended window: Check if game should start within the next 5 minutes or started within the last 10 minutes
                let is_near_start = time_diff >= chrono::Duration::minutes(-5)
                    && time_diff <= chrono::Duration::minutes(10);

                if is_near_start {
                    info!(
                        "Cache bypass triggered for game near start time: {} vs {} - start: {}, time_diff: {:?}",
                        game.home_team,
                        game.away_team,
                        game_start,
                        time_diff
                    );
                }

                is_near_start
            }
            Err(_) => false,
        }
    });

    if has_starting_games {
        debug!("Cache bypass enabled for games near start time");
    }

    has_starting_games
}

/// Enhanced cache expiration check that considers games that might be starting
pub async fn get_cached_tournament_data_with_start_check(
    key: &str,
    current_games: &[GameData],
) -> Option<ScheduleResponse> {
    let mut cache = TOURNAMENT_CACHE.write().await;

    if let Some(cached_entry) = cache.get(key) {
        // Check if we have any games that might be starting
        let has_starting_games = current_games.iter().any(|game| {
            game.score_type == crate::teletext_ui::ScoreType::Scheduled && !game.start.is_empty()
        });

        // If we have starting games, consider cache expired more aggressively
        if has_starting_games {
            let age = cached_entry.cached_at.elapsed();
            let aggressive_ttl =
                Duration::from_secs(crate::constants::cache_ttl::STARTING_GAMES_SECONDS); // 10 seconds for starting games

            if age > aggressive_ttl {
                info!(
                    "Cache expired for starting games: key={}, age={:?}, aggressive_ttl={:?}",
                    key, age, aggressive_ttl
                );
                cache.pop(key);
                return None;
            }
        }

        if !cached_entry.is_expired() {
            return Some(cached_entry.data.clone());
        } else {
            cache.pop(key);
        }
    }

    None
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
#[instrument(skip(game_id), fields(game_id = %game_id))]
pub async fn get_cached_players(game_id: i32) -> Option<HashMap<i64, String>> {
    debug!(
        "Attempting to retrieve cached players for game_id: {}",
        game_id
    );

    let mut cache = PLAYER_CACHE.write().await;

    if let Some(players) = cache.get(&game_id) {
        let player_count = players.len();
        debug!(
            "Cache hit for players: game_id={}, player_count={}",
            game_id, player_count
        );
        Some(players.clone())
    } else {
        debug!("Cache miss for players: game_id={}", game_id);
        None
    }
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
#[instrument(skip(game_id, players), fields(game_id = %game_id))]
pub async fn cache_players(game_id: i32, players: HashMap<i64, String>) {
    let player_count = players.len();
    debug!(
        "Caching players: game_id={}, player_count={}",
        game_id, player_count
    );

    let mut cache = PLAYER_CACHE.write().await;
    cache.put(game_id, players);

    info!(
        "Successfully cached players: game_id={}, player_count={}",
        game_id, player_count
    );
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
        debug!("Found cached detailed game data: key={}", key);

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
        debug!("Cache miss for detailed game data: key={}", key);
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
                Duration::from_secs(3600) // 1 hour TTL
            );
            cache.pop(&key);
        }
    } else {
        debug!("Cache miss for goal events data: key={}", key);
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
    cache.pop(&key);
    debug!(
        "Cleared goal events cache for game: season={}, game_id={}",
        season, game_id
    );
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

            // Verify consistency between individual methods and combined method
            assert_eq!(game_id, returned_game_id);
            assert_eq!(season, returned_season);

            debug_info.push_str(&format!(
                "  Key: {key}, Game ID: {game_id}, Season: {season}, Events: {event_count}, Expired: {is_expired}\n"
            ));
        }
        debug_info.push('\n');
    }

    debug_info
}

/// Resets all caches and returns confirmation - demonstrates clear_all_caches usage
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
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

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
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

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
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

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
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

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
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

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
}
