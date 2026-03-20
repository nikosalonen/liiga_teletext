//! Player cache operations with LRU caching and disambiguation support

use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::LazyLock;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

use crate::data_fetcher::player_names::format_for_display;

// LRU cache structure for formatted player information
// Using LRU ensures that when we need to evict entries, we remove the least recently used ones
pub static PLAYER_CACHE: LazyLock<RwLock<LruCache<i32, HashMap<i64, String>>>> =
    LazyLock::new(|| RwLock::new(LruCache::new(NonZeroUsize::new(100).unwrap())));

/// Retrieves cached formatted player information for a specific game.
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
        debug!("Cache miss for players: game_id={game_id}");
        None
    }
}

/// Caches formatted player information for a specific game.
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
#[allow(dead_code)]
pub async fn cache_players_with_formatting(game_id: i32, raw_players: HashMap<i64, String>) {
    let formatted_players: HashMap<i64, String> = raw_players
        .into_iter()
        .map(|(id, full_name)| (id, format_for_display(&full_name)))
        .collect();
    cache_players(game_id, formatted_players).await;
}

/// Caches player information with team-scoped disambiguation for a specific game.
#[instrument(skip(game_id, home_players, away_players), fields(game_id = %game_id))]
pub async fn cache_players_with_disambiguation(
    game_id: i32,
    home_players: HashMap<i64, (String, String)>, // (first_name, last_name)
    away_players: HashMap<i64, (String, String)>, // (first_name, last_name)
) {
    use crate::data_fetcher::player_names::format_with_disambiguation;

    let home_count = home_players.len();
    let away_count = away_players.len();
    debug!(
        "Caching players with disambiguation: game_id={}, home_players={}, away_players={}",
        game_id, home_count, away_count
    );

    // Convert home players to the format expected by disambiguation function
    let home_player_data: Vec<(i64, String, String)> = home_players
        .into_iter()
        .map(|(id, (first_name, last_name))| (id, first_name, last_name))
        .collect();

    // Convert away players to the format expected by disambiguation function
    let away_player_data: Vec<(i64, String, String)> = away_players
        .into_iter()
        .map(|(id, (first_name, last_name))| (id, first_name, last_name))
        .collect();

    // Apply team-scoped disambiguation
    let home_disambiguated = format_with_disambiguation(&home_player_data);
    let away_disambiguated = format_with_disambiguation(&away_player_data);

    // Combine both teams' disambiguated names
    let mut all_players = HashMap::new();
    all_players.extend(home_disambiguated);
    all_players.extend(away_disambiguated);

    let total_players = all_players.len();
    debug!(
        "Disambiguation complete: game_id={}, total_disambiguated_players={}",
        game_id, total_players
    );

    // Cache the combined disambiguated names
    cache_players(game_id, all_players).await;

    info!(
        "Successfully cached players with disambiguation: game_id={}, home_players={}, away_players={}, total_players={}",
        game_id, home_count, away_count, total_players
    );
}

/// Retrieves cached disambiguated player information for a specific game.
#[instrument(skip(game_id), fields(game_id = %game_id))]
#[allow(dead_code)]
pub async fn get_cached_disambiguated_players(game_id: i32) -> Option<HashMap<i64, String>> {
    debug!(
        "Attempting to retrieve cached disambiguated players for game_id: {}",
        game_id
    );

    let mut cache = PLAYER_CACHE.write().await;

    if let Some(players) = cache.get(&game_id) {
        let player_count = players.len();
        debug!(
            "Cache hit for disambiguated players: game_id={}, player_count={}",
            game_id, player_count
        );
        Some(players.clone())
    } else {
        debug!("Cache miss for disambiguated players: game_id={game_id}");
        None
    }
}

/// Retrieves a specific player's disambiguated name from the cache.
#[instrument(skip(game_id, player_id), fields(game_id = %game_id, player_id = %player_id))]
#[allow(dead_code)]
pub async fn get_cached_player_name(game_id: i32, player_id: i64) -> Option<String> {
    debug!(
        "Attempting to retrieve cached player name: game_id={}, player_id={}",
        game_id, player_id
    );

    if let Some(players) = get_cached_disambiguated_players(game_id).await {
        if let Some(name) = players.get(&player_id) {
            debug!(
                "Found cached player name: game_id={}, player_id={}, name={}",
                game_id, player_id, name
            );
            Some(name.clone())
        } else {
            debug!(
                "Player not found in cache: game_id={}, player_id={}",
                game_id, player_id
            );
            None
        }
    } else {
        debug!(
            "No cached players found for game: game_id={}, player_id={}",
            game_id, player_id
        );
        None
    }
}

/// Checks if disambiguated player data exists in cache for a specific game.
#[instrument(skip(game_id), fields(game_id = %game_id))]
#[allow(dead_code)]
pub async fn has_cached_disambiguated_players(game_id: i32) -> bool {
    debug!(
        "Checking if disambiguated players exist in cache: game_id={}",
        game_id
    );

    let cache = PLAYER_CACHE.read().await;
    let exists = cache.peek(&game_id).is_some();

    debug!("Cache check result: game_id={game_id}, exists={exists}");

    exists
}

/// Gets the current cache size for monitoring purposes
#[allow(dead_code)]
pub async fn get_cache_size() -> usize {
    PLAYER_CACHE.read().await.len()
}

/// Clears all entries from the cache
#[allow(dead_code)]
pub async fn clear_cache() {
    PLAYER_CACHE.write().await.clear();
}
