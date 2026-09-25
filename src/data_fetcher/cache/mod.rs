mod core;
pub mod persistence;
pub mod ttl_cache;

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

use tracing::{debug, info, warn};
use ttl_cache::TtlCache;

use crate::constants::cache_ttl;
use crate::data_fetcher::models::{
    DetailedGameResponse, GameData, GoalEventData, ScheduleResponse,
};
#[cfg(test)]
use crate::data_fetcher::player_names::format_for_display;
use crate::data_fetcher::player_names::format_with_disambiguation;
use crate::teletext_ui::ScoreType;

// Re-export core cache functions
pub use core::*;
pub use persistence::clear_all_cache_files;

// --- Player cache (backed by generic TtlCache) ---

/// Player data never expires by TTL — LRU eviction is the sole cleanup mechanism.
const PLAYER_CACHE_TTL: Duration = Duration::from_secs(u64::MAX / 2);

pub(crate) static PLAYER_CACHE: LazyLock<TtlCache<i32, HashMap<i64, String>>> =
    LazyLock::new(|| TtlCache::new("player", 100));

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
#[cfg(test)]
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
#[cfg(test)]
pub async fn get_cached_disambiguated_players(game_id: i32) -> Option<HashMap<i64, String>> {
    get_cached_players(game_id).await
}

/// Retrieves a specific player's disambiguated name from the cache.
#[cfg(test)]
pub async fn get_cached_player_name(game_id: i32, player_id: i64) -> Option<String> {
    get_cached_players(game_id)
        .await
        .and_then(|players| players.get(&player_id).cloned())
}

/// Checks if disambiguated player data exists in cache for a specific game.
#[cfg(test)]
pub async fn has_cached_disambiguated_players(game_id: i32) -> bool {
    get_cached_players(game_id).await.is_some()
}

/// Gets the current player cache size for monitoring purposes.
#[cfg(test)]
pub async fn get_cache_size() -> usize {
    PLAYER_CACHE.len().await
}

/// Clears all entries from the player cache.
#[allow(dead_code)]
pub async fn clear_cache() {
    PLAYER_CACHE.clear().await;
}

// --- HTTP response cache (backed by generic TtlCache) ---

pub(crate) static HTTP_RESPONSE_CACHE: LazyLock<TtlCache<String, String>> =
    LazyLock::new(|| TtlCache::new("http_response", 100));

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

pub(crate) static DETAILED_GAME_CACHE: LazyLock<TtlCache<String, DetailedGameResponse>> =
    LazyLock::new(|| TtlCache::new("detailed_game", 200));

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
#[cfg(test)]
pub async fn get_detailed_game_cache_size() -> usize {
    DETAILED_GAME_CACHE.len().await
}

/// Clears all detailed game cache entries.
#[allow(dead_code)]
pub async fn clear_detailed_game_cache() {
    DETAILED_GAME_CACHE.clear().await;
}

// --- Goal events cache (backed by generic TtlCache) ---

pub(crate) static GOAL_EVENTS_CACHE: LazyLock<TtlCache<String, Vec<GoalEventData>>> =
    LazyLock::new(|| TtlCache::new("goal_events", 300));

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
#[cfg(test)]
pub async fn get_goal_events_cache_size() -> usize {
    GOAL_EVENTS_CACHE.len().await
}

/// Clears all goal events cache entries.
#[allow(dead_code)]
pub async fn clear_goal_events_cache() {
    GOAL_EVENTS_CACHE.clear().await;
}

// --- Tournament cache (backed by generic TtlCache) ---

pub(crate) static TOURNAMENT_CACHE: LazyLock<TtlCache<String, ScheduleResponse>> =
    LazyLock::new(|| TtlCache::new("tournament", 50));

/// Determines if a ScheduleResponse contains live games.
pub fn has_live_games(response: &ScheduleResponse) -> bool {
    response
        .games
        .iter()
        .any(|game| game.started && !game.ended)
}

/// How long before a game's scheduled start its day's data counts as "starting soon".
const STARTING_SOON_LEAD_MINUTES: i64 = 5;

/// How long after the scheduled start a game the API still marks as not started
/// keeps its day's data on the short TTL. Covers late puck drops without keeping
/// a postponed game's date on the short TTL forever.
const LATE_START_GRACE_MINUTES: i64 = 60;

/// Returns a short cache TTL (in seconds) for a schedule response whose data is
/// about to change: `LIVE_GAMES_SECONDS` while a game is in progress, and
/// `STARTING_GAMES_SECONDS` around a game's scheduled start. Returns `None`
/// when nothing is about to change, so callers keep their normal TTL.
///
/// Both cache layers (HTTP responses and parsed tournament data) use this.
/// Without it, a response fetched just before puck drop was cached for
/// minutes and hid the game start and early goals.
pub fn short_schedule_ttl(
    response: &ScheduleResponse,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<u64> {
    if has_live_games(response) {
        return Some(cache_ttl::LIVE_GAMES_SECONDS);
    }

    let starting_soon = response.games.iter().any(|game| {
        if game.started {
            return false;
        }
        let Ok(start) = chrono::DateTime::parse_from_rfc3339(&game.start) else {
            return false;
        };
        let minutes_since_start = now.signed_duration_since(start).num_minutes();
        (-STARTING_SOON_LEAD_MINUTES..=LATE_START_GRACE_MINUTES).contains(&minutes_since_start)
    });

    starting_soon.then_some(cache_ttl::STARTING_GAMES_SECONDS)
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
            Err(e) => {
                warn!(
                    "Failed to parse start time '{}' for {} vs {}: {e}",
                    game.start, game.home_team, game.away_team
                );
                false
            }
        }
    })
}

/// TTL for a parsed tournament response: short while a game is live or about
/// to start, otherwise the completed-games TTL.
fn tournament_cache_ttl(
    response: &ScheduleResponse,
    now: chrono::DateTime<chrono::Utc>,
) -> Duration {
    short_schedule_ttl(response, now)
        .map(Duration::from_secs)
        .unwrap_or_else(|| game_state_ttl(false))
}

/// Caches tournament data with automatic live and starting-soon game detection.
pub async fn cache_tournament_data(key: String, data: ScheduleResponse) {
    let ttl = tournament_cache_ttl(&data, chrono::Utc::now());
    if ttl < game_state_ttl(false) {
        info!(
            "Short-lived tournament cache entry: key={key}, ttl={}s",
            ttl.as_secs()
        );
    }

    TOURNAMENT_CACHE.insert(key, data, ttl).await;
}

/// Retrieves cached tournament data if it has not expired.
#[cfg(test)]
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
#[cfg(test)]
pub async fn get_tournament_cache_size() -> usize {
    TOURNAMENT_CACHE.len().await
}

/// Clears all tournament cache entries.
#[allow(dead_code)]
pub async fn clear_tournament_cache() {
    TOURNAMENT_CACHE.clear().await
}

#[cfg(test)]
mod schedule_ttl_tests {
    use super::*;
    use chrono::{DateTime, TimeZone, Utc};

    fn response_with_game(start: &str, started: bool, ended: bool) -> ScheduleResponse {
        let json = format!(
            r#"{{"games":[{{"id":1,"season":2026,"start":"{start}",
                "homeTeam":{{"teamId":"a","teamName":"A","goals":0,"goalEvents":[]}},
                "awayTeam":{{"teamId":"b","teamName":"B","goals":0,"goalEvents":[]}},
                "finishedType":null,"started":{started},"ended":{ended},
                "serie":"RUNKOSARJA"}}],
                "previousGameDate":null,"nextGameDate":null}}"#
        );
        serde_json::from_str(&json).expect("test schedule JSON should parse")
    }

    fn at(hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 1, hour, minute, 0).unwrap()
    }

    const START: &str = "2026-10-01T15:30:00Z";

    #[test]
    fn live_game_gets_live_ttl() {
        let response = response_with_game(START, true, false);
        assert_eq!(
            short_schedule_ttl(&response, at(15, 45)),
            Some(cache_ttl::LIVE_GAMES_SECONDS)
        );
    }

    #[test]
    fn game_starting_within_five_minutes_gets_starting_ttl() {
        let response = response_with_game(START, false, false);
        assert_eq!(
            short_schedule_ttl(&response, at(15, 26)),
            Some(cache_ttl::STARTING_GAMES_SECONDS)
        );
    }

    #[test]
    fn game_past_its_start_time_but_not_started_gets_starting_ttl() {
        // Late puck drop: the API still says not started 20 minutes after the
        // scheduled start. Caching this for minutes would hide the start.
        let response = response_with_game(START, false, false);
        assert_eq!(
            short_schedule_ttl(&response, at(15, 50)),
            Some(cache_ttl::STARTING_GAMES_SECONDS)
        );
    }

    #[test]
    fn game_hours_away_gets_no_short_ttl() {
        let response = response_with_game(START, false, false);
        assert_eq!(short_schedule_ttl(&response, at(12, 0)), None);
    }

    #[test]
    fn finished_game_gets_no_short_ttl() {
        let response = response_with_game(START, true, true);
        assert_eq!(short_schedule_ttl(&response, at(18, 0)), None);
    }

    #[test]
    fn tournament_cache_uses_starting_ttl_before_puck_drop() {
        let response = response_with_game(START, false, false);
        assert_eq!(
            tournament_cache_ttl(&response, at(15, 28)),
            Duration::from_secs(cache_ttl::STARTING_GAMES_SECONDS)
        );
    }

    #[test]
    fn tournament_cache_keeps_long_ttl_for_finished_day() {
        let response = response_with_game(START, true, true);
        assert_eq!(
            tournament_cache_ttl(&response, at(20, 0)),
            Duration::from_secs(cache_ttl::COMPLETED_GAMES_SECONDS)
        );
    }

    #[test]
    fn game_that_never_started_long_ago_gets_no_short_ttl() {
        // A postponed game on a past date must not keep that day on a 30s TTL forever.
        let response = response_with_game(START, false, false);
        assert_eq!(short_schedule_ttl(&response, at(23, 0)), None);
    }
}
