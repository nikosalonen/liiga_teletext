use lazy_static::lazy_static;
use std::collections::HashMap;
use tokio::sync::RwLock;

// Cache structure for player information
lazy_static! {
    pub static ref PLAYER_CACHE: RwLock<HashMap<i32, HashMap<i64, String>>> =
        RwLock::new(HashMap::new());
}

/// Retrieves cached player information for a specific game.
///
/// # Arguments
/// * `game_id` - The unique identifier of the game
///
/// # Returns
/// * `Option<HashMap<i64, String>>` - Some(HashMap) with player_id -> name mapping if found, None if not cached
///
/// # Example
/// ```
/// if let Some(players) = get_cached_players(12345).await {
///     println!("Found {} cached players", players.len());
/// }
/// ```
pub async fn get_cached_players(game_id: i32) -> Option<HashMap<i64, String>> {
    PLAYER_CACHE.read().await.get(&game_id).cloned()
}

/// Caches player information for a specific game.
/// Updates existing cache entry if game_id already exists.
///
/// # Arguments
/// * `game_id` - The unique identifier of the game
/// * `players` - HashMap mapping player IDs to their names
///
/// # Example
/// ```
/// let mut players = HashMap::new();
/// players.insert(123, "Player Name".to_string());
/// cache_players(12345, players).await;
/// ```
pub async fn cache_players(game_id: i32, players: HashMap<i64, String>) {
    PLAYER_CACHE.write().await.insert(game_id, players);
}
