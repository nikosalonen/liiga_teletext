mod core;
pub mod ttl_cache;

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

use tracing::{debug, info};
use ttl_cache::TtlCache;

use crate::constants::cache_ttl;
use crate::data_fetcher::models::{
    DetailedGameResponse, GameData, GoalEventData, ScheduleResponse,
};
use crate::data_fetcher::player_names::{format_for_display, format_with_disambiguation};
use crate::teletext_ui::ScoreType;

// Re-export core cache functions
pub use core::*;

// --- Player cache (backed by generic TtlCache) ---

/// Effectively-infinite TTL for player data (LRU eviction is the primary cleanup mechanism)
const PLAYER_CACHE_TTL: Duration = Duration::from_secs(86400 * 365);

pub static PLAYER_CACHE: LazyLock<TtlCache<i32, HashMap<i64, String>>> =
    LazyLock::new(|| TtlCache::new(100));

/// Retrieves cached formatted player information for a specific game.
pub async fn get_cached_players(game_id: i32) -> Option<HashMap<i64, String>> {
    PLAYER_CACHE.get(&game_id).await
}

/// Caches formatted player information for a specific game.
pub async fn cache_players(game_id: i32, players: HashMap<i64, String>) {
    let player_count = players.len();
    PLAYER_CACHE
        .insert(game_id, players, PLAYER_CACHE_TTL)
        .await;
    debug!("Cached {player_count} players for game_id={game_id}");
}

/// Caches player information with automatic formatting for a specific game.
#[allow(dead_code)]
pub async fn cache_players_with_formatting(game_id: i32, raw_players: HashMap<i64, String>) {
    let formatted_players: HashMap<i64, String> = raw_players
        .into_iter()
        .map(|(id, full_name)| (id, format_for_display(&full_name)))
        .collect();
    cache_players(game_id, formatted_players).await;
}

/// Caches player information with team-scoped disambiguation for a specific game.
pub async fn cache_players_with_disambiguation(
    game_id: i32,
    home_players: HashMap<i64, (String, String)>,
    away_players: HashMap<i64, (String, String)>,
) {
    let home_player_data: Vec<(i64, String, String)> = home_players
        .into_iter()
        .map(|(id, (first_name, last_name))| (id, first_name, last_name))
        .collect();

    let away_player_data: Vec<(i64, String, String)> = away_players
        .into_iter()
        .map(|(id, (first_name, last_name))| (id, first_name, last_name))
        .collect();

    let home_disambiguated = format_with_disambiguation(&home_player_data);
    let away_disambiguated = format_with_disambiguation(&away_player_data);

    let mut all_players = HashMap::new();
    all_players.extend(home_disambiguated);
    all_players.extend(away_disambiguated);

    cache_players(game_id, all_players).await;
}

/// Retrieves cached disambiguated player information for a specific game.
#[allow(dead_code)]
pub async fn get_cached_disambiguated_players(game_id: i32) -> Option<HashMap<i64, String>> {
    get_cached_players(game_id).await
}

/// Retrieves a specific player's disambiguated name from the cache.
#[allow(dead_code)]
pub async fn get_cached_player_name(game_id: i32, player_id: i64) -> Option<String> {
    get_cached_players(game_id)
        .await
        .and_then(|players| players.get(&player_id).cloned())
}

/// Checks if disambiguated player data exists in cache for a specific game.
#[allow(dead_code)]
pub async fn has_cached_disambiguated_players(game_id: i32) -> bool {
    get_cached_players(game_id).await.is_some()
}

/// Gets the current player cache size for monitoring purposes.
#[allow(dead_code)]
pub async fn get_cache_size() -> usize {
    PLAYER_CACHE.len().await
}

/// Clears all entries from the player cache.
#[allow(dead_code)]
pub async fn clear_cache() {
    PLAYER_CACHE.clear().await;
}

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

// --- Goal events cache (backed by generic TtlCache) ---

pub static GOAL_EVENTS_CACHE: LazyLock<TtlCache<String, Vec<GoalEventData>>> =
    LazyLock::new(|| TtlCache::new(300));

/// Creates a cache key for goal events data.
pub fn create_goal_events_key(season: i32, game_id: i32) -> String {
    format!("goal_events_{season}_{game_id}")
}

/// Caches processed goal events data with a TTL that depends on game liveness.
pub async fn cache_goal_events_data(
    season: i32,
    game_id: i32,
    data: Vec<GoalEventData>,
    is_live_game: bool,
) {
    let key = create_goal_events_key(season, game_id);
    GOAL_EVENTS_CACHE
        .insert(key, data, game_state_ttl(is_live_game))
        .await;
}

/// Retrieves cached goal events data if it has not expired.
pub async fn get_cached_goal_events_data(season: i32, game_id: i32) -> Option<Vec<GoalEventData>> {
    let key = create_goal_events_key(season, game_id);
    GOAL_EVENTS_CACHE.get(&key).await
}

/// Gets the current goal events cache size for monitoring purposes.
#[allow(dead_code)]
pub async fn get_goal_events_cache_size() -> usize {
    GOAL_EVENTS_CACHE.len().await
}

/// Clears all goal events cache entries.
#[allow(dead_code)]
pub async fn clear_goal_events_cache() {
    GOAL_EVENTS_CACHE.clear().await;
}

// --- Tournament cache (backed by generic TtlCache) ---

pub static TOURNAMENT_CACHE: LazyLock<TtlCache<String, ScheduleResponse>> =
    LazyLock::new(|| TtlCache::new(50));

/// Determines if a ScheduleResponse contains live games.
pub fn has_live_games(response: &ScheduleResponse) -> bool {
    response
        .games
        .iter()
        .any(|game| game.started && !game.ended)
}

/// Determines whether the cache should be completely bypassed for games near their start time.
pub fn should_bypass_cache_for_starting_games(current_games: &[GameData]) -> bool {
    current_games.iter().any(|game| {
        if game.score_type != ScoreType::Scheduled || game.start.is_empty() {
            return false;
        }

        match chrono::DateTime::parse_from_rfc3339(&game.start) {
            Ok(game_start) => {
                let now = chrono::Utc::now();
                let time_diff = now.signed_duration_since(game_start);

                // Extended window: game should start within 5 min or started within last 10 min
                let is_near_start = time_diff >= chrono::Duration::minutes(-5)
                    && time_diff <= chrono::Duration::minutes(10);

                if is_near_start {
                    info!(
                        "Cache bypass for game near start: {} vs {} (time_diff: {time_diff:?})",
                        game.home_team, game.away_team
                    );
                }

                is_near_start
            }
            Err(_) => false,
        }
    })
}

/// Caches tournament data with automatic live game detection.
pub async fn cache_tournament_data(key: String, data: ScheduleResponse) {
    let has_live = has_live_games(&data);

    TOURNAMENT_CACHE
        .insert(key.clone(), data, game_state_ttl(has_live))
        .await;

    if has_live {
        info!(
            "Live game cache entry: key={key}, ttl={}s",
            cache_ttl::LIVE_GAMES_SECONDS
        );
    }
}

/// Retrieves cached tournament data if it has not expired.
#[allow(dead_code)]
pub async fn get_cached_tournament_data(key: &str) -> Option<ScheduleResponse> {
    TOURNAMENT_CACHE.get(&key.to_string()).await
}

/// Enhanced cache retrieval that applies aggressive TTL when games are about to start.
pub async fn get_cached_tournament_data_with_start_check(
    key: &str,
    current_games: &[GameData],
) -> Option<ScheduleResponse> {
    // Use the proper start-time window check (±5/10 minutes) instead of
    // treating any scheduled game with a start time as "starting"
    let has_starting = should_bypass_cache_for_starting_games(current_games);

    if has_starting {
        let aggressive_ttl = Duration::from_secs(cache_ttl::STARTING_GAMES_SECONDS);
        TOURNAMENT_CACHE
            .get_if(&key.to_string(), |cached_at| {
                cached_at.elapsed() <= aggressive_ttl
            })
            .await
    } else {
        TOURNAMENT_CACHE.get(&key.to_string()).await
    }
}

/// Gets the current tournament cache size for monitoring purposes.
#[allow(dead_code)]
pub async fn get_tournament_cache_size() -> usize {
    TOURNAMENT_CACHE.len().await
}

/// Clears all tournament cache entries.
#[allow(dead_code)]
pub async fn clear_tournament_cache() {
    TOURNAMENT_CACHE.clear().await
}
