use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Mutex;

// Cache structure for player information
lazy_static! {
    pub static ref PLAYER_CACHE: Mutex<HashMap<i32, HashMap<i64, String>>> =
        Mutex::new(HashMap::new());
}

pub fn get_cached_players(game_id: i32) -> Option<HashMap<i64, String>> {
    PLAYER_CACHE.lock().unwrap().get(&game_id).cloned()
}

pub fn cache_players(game_id: i32, players: HashMap<i64, String>) {
    PLAYER_CACHE.lock().unwrap().insert(game_id, players);
}
