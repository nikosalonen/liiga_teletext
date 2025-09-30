use crate::constants::env_vars;
use crate::data_fetcher::player_names::{build_full_name, format_for_display};
use reqwest::Client;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tracing::{debug, warn};

/// Try to fetch player names for specific player IDs with a reduced timeout.
/// This is used as a fallback when cached player names are missing.
///
/// # Arguments
///
/// * `api_domain` - The API domain to fetch from
/// * `season` - The season year
/// * `game_id` - The game ID to fetch player data for
/// * `player_ids` - List of player IDs to fetch names for
///
/// # Returns
///
/// `Some(HashMap<i64, String>)` with player ID -> formatted name mapping,
/// or `None` if the fetch fails or no players are found.
///
/// # Examples
///
/// ```
/// use liiga_teletext::data_fetcher::processors::try_fetch_player_names_for_game;
///
/// async fn example() {
///     let player_ids = vec![123, 456];
///     let names = try_fetch_player_names_for_game(
///         "api.example.com",
///         2024,
///         12345,
///         &player_ids
///     ).await;
///     
///     if let Some(name_map) = names {
///         println!("Found {} player names", name_map.len());
///     }
/// }
/// ```
#[allow(dead_code)]
pub async fn try_fetch_player_names_for_game(
    api_domain: &str,
    season: i32,
    game_id: i32,
    player_ids: &[i64],
) -> Option<HashMap<i64, String>> {
    use crate::data_fetcher::api::build_game_url;
    use crate::data_fetcher::models::{DetailedGameResponse, Player};

    if player_ids.is_empty() {
        return Some(HashMap::new());
    }

    debug!(
        "Attempting to fetch player names for {} players in game ID {} (season {})",
        player_ids.len(),
        game_id,
        season
    );

    // Get timeout from env var with 5 second default, clamped to safe range (1-30 seconds)
    let timeout_secs = std::env::var(env_vars::API_FETCH_TIMEOUT)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5)
        .clamp(1, 30);

    // Create a client with a configurable timeout for this fallback attempt
    let client = match Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            warn!("Failed to create HTTP client for player name fetch: {}", e);
            return None;
        }
    };

    // Use the provided API domain and season
    let url = build_game_url(api_domain, season, game_id);

    match client.get(&url).send().await {
        Ok(response) => {
            // Check for HTTP errors before trying to parse JSON
            let response = match response.error_for_status() {
                Ok(response) => response,
                Err(e) => {
                    debug!(
                        "HTTP error for player name fetch (game ID {}): {}",
                        game_id, e
                    );
                    return None;
                }
            };

            match response.json::<DetailedGameResponse>().await {
                Ok(game_response) => {
                    debug!(
                        "Successfully fetched detailed game data for player name lookup (game ID: {})",
                        game_id
                    );

                    let mut player_names = HashMap::new();
                    // Convert player_ids to HashSet for O(1) lookup instead of O(n) contains()
                    let mut wanted_ids: HashSet<i64> = HashSet::with_capacity(player_ids.len());
                    wanted_ids.extend(player_ids.iter().copied());

                    // Helper to process players and extract names for the requested IDs
                    let mut process_players = |players: &[Player]| {
                        for player in players {
                            if wanted_ids.contains(&player.id) {
                                let full_name =
                                    build_full_name(&player.first_name, &player.last_name);
                                let display_name = format_for_display(&full_name);
                                player_names.insert(player.id, display_name);
                            }
                        }
                    };

                    // Process both home and away team players
                    process_players(&game_response.home_team_players);
                    process_players(&game_response.away_team_players);

                    if !player_names.is_empty() {
                        debug!(
                            "Successfully fetched {} player names for game ID {}",
                            player_names.len(),
                            game_id
                        );
                        Some(player_names)
                    } else {
                        debug!(
                            "No player names found for requested IDs in game ID {}",
                            game_id
                        );
                        None
                    }
                }
                Err(e) => {
                    debug!("Failed to parse game response for player names: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            debug!(
                "Failed to fetch game data for player names (game ID {}): {}",
                game_id, e
            );
            None
        }
    }
}
