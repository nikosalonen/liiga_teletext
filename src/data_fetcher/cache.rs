use lazy_static::lazy_static;
use std::collections::HashMap;
use tokio::sync::RwLock;

// Cache structure for formatted player information
lazy_static! {
    pub static ref PLAYER_CACHE: RwLock<HashMap<i32, HashMap<i64, String>>> =
        RwLock::new(HashMap::new());
}

/// Formats a player's full name to show only the capitalized last name.
/// This is the same formatting logic used in process_team_goals.
///
/// # Arguments
/// * `full_name` - The player's full name (e.g., "Mikko Koivu")
///
/// # Returns
/// * `String` - The formatted name (e.g., "Koivu")
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::cache::format_player_name;
///
/// let formatted = format_player_name("Mikko Koivu");
/// assert_eq!(formatted, "Koivu");
///
/// let formatted = format_player_name("Teemu Selänne");
/// assert_eq!(formatted, "Selänne");
/// ```
pub fn format_player_name(full_name: &str) -> String {
    full_name
        .split_whitespace()
        .last()
        .unwrap_or("")
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if i == 0 {
                c.to_uppercase().next().unwrap_or(c)
            } else {
                c.to_lowercase().next().unwrap_or(c)
            }
        })
        .collect::<String>()
}

/// Retrieves cached formatted player information for a specific game.
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
    PLAYER_CACHE.read().await.get(&game_id).cloned()
}

/// Caches formatted player information for a specific game.
/// Updates existing cache entry if game_id already exists.
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
    PLAYER_CACHE.write().await.insert(game_id, players);
}

/// Caches player information with automatic formatting for a specific game.
/// This function takes raw player data and formats the names before caching.
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
        .map(|(id, full_name)| (id, format_player_name(&full_name)))
        .collect();
    cache_players(game_id, formatted_players).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_player_name_simple() {
        assert_eq!(format_player_name("Mikko Koivu"), "Koivu");
        assert_eq!(format_player_name("Teemu Selänne"), "Selänne");
        assert_eq!(format_player_name("John Smith"), "Smith");
    }

    #[test]
    fn test_format_player_name_single_name() {
        assert_eq!(format_player_name("Koivu"), "Koivu");
        assert_eq!(format_player_name("Selänne"), "Selänne");
    }

    #[test]
    fn test_format_player_name_multiple_names() {
        assert_eq!(format_player_name("Mikko Ilmari Koivu"), "Koivu");
        assert_eq!(format_player_name("Teemu Ilmari Selänne"), "Selänne");
    }

    #[test]
    fn test_format_player_name_with_hyphens() {
        assert_eq!(format_player_name("Jean-Pierre Dumont"), "Dumont");
        assert_eq!(format_player_name("Pierre-Luc Dubois"), "Dubois");
    }

    #[test]
    fn test_format_player_name_empty() {
        assert_eq!(format_player_name(""), "");
    }

    #[test]
    fn test_format_player_name_whitespace() {
        assert_eq!(format_player_name("   Koivu   "), "Koivu");
        assert_eq!(format_player_name("  Mikko  Koivu  "), "Koivu");
    }

    #[tokio::test]
    async fn test_cache_players_with_formatting() {
        let mut raw_players = HashMap::new();
        raw_players.insert(123, "Mikko Koivu".to_string());
        raw_players.insert(456, "Teemu Selänne".to_string());
        raw_players.insert(789, "John Smith".to_string());

        cache_players_with_formatting(999, raw_players).await;

        let cached_players = get_cached_players(999).await.unwrap();
        assert_eq!(cached_players.get(&123), Some(&"Koivu".to_string()));
        assert_eq!(cached_players.get(&456), Some(&"Selänne".to_string()));
        assert_eq!(cached_players.get(&789), Some(&"Smith".to_string()));
    }
}
