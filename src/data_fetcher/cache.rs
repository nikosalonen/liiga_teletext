use lazy_static::lazy_static;
use std::collections::HashMap;
use tokio::sync::RwLock;

// Cache structure for player information
lazy_static! {
    pub static ref PLAYER_CACHE: RwLock<HashMap<i32, HashMap<i64, String>>> =
        RwLock::new(HashMap::new());
}

pub async fn get_cached_players(game_id: i32) -> Option<HashMap<i64, String>> {
    PLAYER_CACHE.read().await.get(&game_id).cloned()
}

pub async fn cache_players(game_id: i32, players: HashMap<i64, String>) {
    PLAYER_CACHE.write().await.insert(game_id, players);
}
