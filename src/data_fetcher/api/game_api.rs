// Game-specific API operations
// This module contains functions for fetching, processing, and converting game data

use crate::config::Config;
use crate::data_fetcher::cache::{
    cache_detailed_game_data, cache_goal_events_data, cache_players,
    cache_players_with_disambiguation, get_cached_detailed_game_data, get_cached_goal_events_data,
    get_cached_players,
};
use crate::data_fetcher::models::{
    DetailedGame, DetailedGameResponse, DetailedTeam, GameData, GoalEvent, GoalEventData, Player,
    ScheduleApiGame, ScheduleGame, ScheduleResponse, ScheduleTeam,
};
use crate::data_fetcher::player_names::format_for_display;
use crate::data_fetcher::processors::{
    create_basic_goal_events, determine_game_status, format_time, process_goal_events,
};
use crate::error::AppError;
use crate::teletext_ui::ScoreType;
use futures;
use reqwest::Client;
use std::collections::HashMap;
use tracing::{debug, error, info, instrument, warn};

// Import from sibling modules
use super::date_logic::parse_date_and_season;
use super::fetch_utils::fetch;
use super::tournament_logic::{
    TournamentType, determine_tournaments_for_month, fetch_tournament_games,
};
use super::urls::build_game_url;

/// Helper function to extract team name from a ScheduleTeam, with fallback logic.
/// Returns the team_name if available, otherwise team_placeholder, or "Unknown" as last resort.
pub(super) fn get_team_name(team: &ScheduleTeam) -> &str {
    team.team_name
        .as_deref()
        .or(team.team_placeholder.as_deref())
        .unwrap_or("Unknown")
}

/// Determines if a game has actual goals (excluding RL0 goal types).
#[allow(dead_code)]
pub(super) fn has_actual_goals(game: &ScheduleGame) -> bool {
    game.home_team
        .goal_events
        .iter()
        .any(|g| !g.goal_types.contains(&"RL0".to_string()))
        || game
            .away_team
            .goal_events
            .iter()
            .any(|g| !g.goal_types.contains(&"RL0".to_string()))
}

/// Determines if detailed game data should be fetched based on game state.
#[allow(dead_code)]
pub(super) fn should_fetch_detailed_data(_game: &ScheduleGame) -> bool {
    false
}

/// Processes a single game and returns GameData.
pub(super) async fn process_single_game(
    _client: &Client,
    config: &Config,
    game: ScheduleGame,
    game_idx: usize,
    response_idx: usize,
) -> Result<GameData, AppError> {
    let home_team_name = get_team_name(&game.home_team);
    let away_team_name = get_team_name(&game.away_team);

    info!(
        "Processing game #{} in response #{}: {} vs {}",
        game_idx + 1,
        response_idx + 1,
        home_team_name,
        away_team_name
    );

    // Use enhanced game state detection for time formatting
    let (score_type, is_overtime, is_shootout) = determine_game_status(&game);
    debug!(
        "Game status: {:?}, overtime: {}, shootout: {}",
        score_type, is_overtime, is_shootout
    );

    let time = if matches!(score_type, ScoreType::Scheduled) {
        let formatted_time = format_time(&game.start).unwrap_or_default();
        debug!("Game scheduled, formatted time: {formatted_time}");
        formatted_time
    } else {
        debug!("Game ongoing or finished, no time to display");
        String::new()
    };

    let result = format!("{}-{}", game.home_team.goals, game.away_team.goals);
    debug!("Game result: {result}");

    // Always use schedule-provided goal events (with embedded names) to avoid per-game fetch
    let goal_events = create_basic_goal_events(&game, &config.api_domain).await;

    info!(
        "Successfully processed game #{} in response #{}",
        game_idx + 1,
        response_idx + 1
    );

    debug!("Game serie from API: '{}'", game.serie);
    Ok(GameData {
        home_team: home_team_name.to_string(),
        away_team: away_team_name.to_string(),
        time,
        result,
        score_type,
        is_overtime,
        is_shootout,
        serie: game.serie,
        goal_events,
        played_time: game.game_time,
        start: game.start.clone(),
    })
}

/// Processes all games in a single response.
pub(super) async fn process_response_games(
    client: &Client,
    config: &Config,
    response: &ScheduleResponse,
    response_idx: usize,
) -> Result<Vec<GameData>, AppError> {
    if response.games.is_empty() {
        info!("Response #{} has empty games array", response_idx + 1);
        return Ok(Vec::new());
    }

    info!(
        "Processing response #{} with {} games",
        response_idx + 1,
        response.games.len()
    );

    let games = futures::future::try_join_all(response.games.clone().into_iter().enumerate().map(
        |(game_idx, game)| {
            let client = client.clone();
            let config = config.clone();
            async move { process_single_game(&client, &config, game, game_idx, response_idx).await }
        },
    ))
    .await?;

    info!(
        "Successfully processed all games in response #{}, adding {} games to result",
        response_idx + 1,
        games.len()
    );

    Ok(games)
}

/// Processes games from all responses and returns a flat list of GameData.
pub(super) async fn process_games(
    client: &Client,
    config: &Config,
    response_data: Vec<ScheduleResponse>,
) -> Result<Vec<GameData>, AppError> {
    let mut all_games = Vec::new();

    if response_data.is_empty() {
        info!("No response data to process");
        return Ok(all_games);
    }

    info!(
        "Processing {} response(s) with game data",
        response_data.len()
    );

    for (i, response) in response_data.iter().enumerate() {
        let games = process_response_games(client, config, response, i).await?;
        all_games.extend(games);
    }

    info!("Total games processed: {}", all_games.len());
    Ok(all_games)
}

/// Fetches detailed game data including goal events for a specific game.
/// Uses caching to improve performance and reduce API calls.
/// This function is disabled in normal operation but preserved for testing.
#[instrument(skip(client, config))]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) async fn fetch_game_data(
    client: &Client,
    config: &Config,
    season: i32,
    game_id: i32,
) -> Result<Vec<GoalEventData>, AppError> {
    info!(
        "Fetching game data for game ID: {} (season: {})",
        game_id, season
    );

    // Check goal events cache first
    if let Some(cached_events) = get_cached_goal_events_data(season, game_id).await {
        info!(
            "Using cached goal events for game ID: {} ({} events)",
            game_id,
            cached_events.len()
        );
        return Ok(cached_events);
    }

    // Check detailed game cache
    if let Some(cached_response) = get_cached_detailed_game_data(season, game_id).await {
        info!(
            "Using cached detailed game response for game ID: {}",
            game_id
        );
        let events = process_game_response_with_cache(cached_response, game_id).await;
        return Ok(events);
    }

    let url = build_game_url(&config.api_domain, season, game_id);

    // Try to get detailed game response
    info!("Making API request to: {url}");
    let game_response: DetailedGameResponse = match fetch(client, &url).await {
        Ok(response) => {
            info!(
                "Successfully fetched detailed game response for game ID: {}",
                game_id
            );
            response
        }
        Err(e) => {
            error!(
                "Failed to fetch detailed game response for game ID {}: {}",
                game_id, e
            );

            // Transform API not found errors to game-specific errors
            return match &e {
                AppError::ApiNotFound { .. } => Err(AppError::api_game_not_found(game_id, season)),
                _ => Err(e),
            };
        }
    };

    // Cache the detailed game response
    let is_live_game = game_response.game.started && !game_response.game.ended;
    cache_detailed_game_data(season, game_id, game_response.clone(), is_live_game).await;

    // Process the response and cache the goal events
    let events = process_game_response_with_cache(game_response, game_id).await;
    cache_goal_events_data(season, game_id, events.clone(), is_live_game).await;

    Ok(events)
}

/// Helper function to process game response with player caching and team-scoped disambiguation
#[cfg_attr(not(test), allow(dead_code))]
pub(super) async fn process_game_response_with_cache(
    game_response: DetailedGameResponse,
    game_id: i32,
) -> Vec<GoalEventData> {
    // Check player cache first
    if let Some(cached_players) = get_cached_players(game_id).await {
        info!(
            "Using cached player data for game ID: {} ({} players)",
            game_id,
            cached_players.len()
        );
        let events = process_goal_events(&game_response.game, &cached_players);
        info!(
            "Processed {} goal events using cached player data",
            events.len()
        );
        return events;
    }

    // Build separate player data for home and away teams with proper error handling
    debug!("No cached player data found, building player data with disambiguation");

    let mut home_players = HashMap::new();
    let mut away_players = HashMap::new();

    info!(
        "Processing {} home team players for disambiguation",
        game_response.home_team_players.len()
    );

    // Process home team players with error handling for missing data
    for player in &game_response.home_team_players {
        if player.first_name.trim().is_empty() && player.last_name.trim().is_empty() {
            warn!(
                "Player {} has empty first and last name, skipping disambiguation",
                player.id
            );
            continue;
        }

        let first_name = if player.first_name.trim().is_empty() {
            debug!(
                "Player {} has empty first name, using empty string for disambiguation",
                player.id
            );
            String::new()
        } else {
            player.first_name.clone()
        };

        let last_name = if player.last_name.trim().is_empty() {
            warn!("Player {} has empty last name, using fallback", player.id);
            format!("Player{}", player.id)
        } else {
            player.last_name.clone()
        };

        home_players.insert(player.id, (first_name, last_name));
    }

    info!(
        "Processing {} away team players for disambiguation",
        game_response.away_team_players.len()
    );

    // Process away team players with error handling for missing data
    for player in &game_response.away_team_players {
        if player.first_name.trim().is_empty() && player.last_name.trim().is_empty() {
            warn!(
                "Player {} has empty first and last name, skipping disambiguation",
                player.id
            );
            continue;
        }

        let first_name = if player.first_name.trim().is_empty() {
            debug!(
                "Player {} has empty first name, using empty string for disambiguation",
                player.id
            );
            String::new()
        } else {
            player.first_name.clone()
        };

        let last_name = if player.last_name.trim().is_empty() {
            warn!("Player {} has empty last name, using fallback", player.id);
            format!("Player{}", player.id)
        } else {
            player.last_name.clone()
        };

        away_players.insert(player.id, (first_name, last_name));
    }

    info!(
        "Built player data: {} home players, {} away players",
        home_players.len(),
        away_players.len()
    );

    // Apply team-scoped disambiguation and cache the results
    debug!(
        "Applying team-scoped disambiguation for game ID: {}",
        game_id
    );
    cache_players_with_disambiguation(game_id, home_players, away_players).await;

    // Get the disambiguated names from cache for processing
    let disambiguated_players = match get_cached_players(game_id).await {
        Some(players) => players,
        None => {
            error!(
                "Failed to retrieve cached player data for game ID {} after disambiguation caching. This should not happen.",
                game_id
            );
            // Fallback: create basic player names without disambiguation
            let mut fallback_players = HashMap::new();
            for player in &game_response.home_team_players {
                if !player.last_name.trim().is_empty() {
                    fallback_players.insert(player.id, format_for_display(&player.last_name));
                } else {
                    fallback_players.insert(player.id, format!("Player{}", player.id));
                }
            }
            for player in &game_response.away_team_players {
                if !player.last_name.trim().is_empty() {
                    fallback_players.insert(player.id, format_for_display(&player.last_name));
                } else {
                    fallback_players.insert(player.id, format!("Player{}", player.id));
                }
            }
            fallback_players
        }
    };

    let events = process_goal_events(&game_response.game, &disambiguated_players);
    info!(
        "Processed {} goal events with team-scoped disambiguation for game ID: {}",
        events.len(),
        game_id
    );
    events
}

/// Filters games to match the requested date
pub(super) fn filter_games_by_date(
    games: Vec<ScheduleApiGame>,
    target_date: &str,
) -> Vec<ScheduleApiGame> {
    info!("Filtering games for target date: {target_date}");

    let matching_games: Vec<ScheduleApiGame> = games
        .into_iter()
        .filter(|game| {
            // Extract date part from the game start time (format: YYYY-MM-DDThh:mm:ssZ)
            if let Some(date_part) = game.start.split('T').next() {
                let matches = date_part == target_date;
                if matches {
                    let tournament = TournamentType::from_serie(game.serie);
                    info!(
                        "Found matching game: {} vs {} on {} (tournament: {})",
                        game.home_team_name,
                        game.away_team_name,
                        date_part,
                        tournament.as_str()
                    );
                }
                matches
            } else {
                false
            }
        })
        .collect();

    info!(
        "Found {} games matching date {}",
        matching_games.len(),
        target_date
    );

    matching_games
}

/// Converts a GoalEventData to GoalEvent with proper period and event_id handling
fn convert_goal_event_data_to_goal_event(
    event: &GoalEventData,
    detailed_game: &DetailedGame,
) -> GoalEvent {
    // Try to find the actual period and event_id from the detailed game data
    let (period, event_id) = find_period_and_event_id_for_goal(event, detailed_game);

    GoalEvent {
        scorer_player_id: event.scorer_player_id,
        log_time: format!("{:02}:{:02}:00", event.minute / 60, event.minute % 60),
        game_time: event.minute * 60, // Convert back to seconds
        period,
        event_id,
        home_team_score: event.home_team_score,
        away_team_score: event.away_team_score,
        winning_goal: event.is_winning_goal,
        goal_types: event.goal_types.clone(),
        assistant_player_ids: vec![], // TODO: Extract from detailed game data if available
        video_clip_url: event.video_clip_url.clone(),
        scorer_player: None,
    }
}

/// Finds the actual period and event_id for a goal event from detailed game data
/// Falls back to defaults if the information is not available
fn find_period_and_event_id_for_goal(
    event: &GoalEventData,
    detailed_game: &DetailedGame,
) -> (i32, i32) {
    let game_time_seconds = event.minute * 60;

    // Try to determine period from game time and periods data
    let period = if !detailed_game.periods.is_empty() {
        detailed_game
            .periods
            .iter()
            .find(|p| game_time_seconds >= p.start_time && game_time_seconds <= p.end_time)
            .map(|p| p.index)
            .unwrap_or(1) // Default to period 1 if not found
    } else {
        1 // Default period if no period data available
    };

    // Try to find the actual event_id from the detailed game goal events
    let event_id = detailed_game
        .home_team
        .goal_events
        .iter()
        .chain(detailed_game.away_team.goal_events.iter())
        .find(|ge| {
            ge.scorer_player_id == event.scorer_player_id
                && ge.game_time == game_time_seconds
                && ge.home_team_score == event.home_team_score
                && ge.away_team_score == event.away_team_score
        })
        .map(|ge| ge.event_id)
        .unwrap_or(0); // Default event ID if not found

    (period, event_id)
}

/// Helper to fetch and convert detailed game data
async fn fetch_and_convert_detailed_game_data(
    client: &Client,
    config: &Config,
    season: i32,
    game_id: i32,
) -> DetailedGameData {
    fetch_detailed_game_data_for_historical_game(client, config, season, game_id).await
}

/// Helper to convert goal events for a team
fn convert_goal_events_for_team(
    goal_events: &[GoalEventData],
    is_home_team: bool,
    detailed_game: &DetailedGame,
) -> Vec<GoalEvent> {
    goal_events
        .iter()
        .filter(|event| event.is_home_team == is_home_team)
        .map(|event| convert_goal_event_data_to_goal_event(event, detailed_game))
        .collect()
}

/// Helper to build a ScheduleTeam from API and detailed data
fn build_schedule_team_from_api_and_detailed(
    team_name: String,
    goals: i32,
    start_time: String,
    goal_events: Vec<GoalEvent>,
) -> ScheduleTeam {
    ScheduleTeam {
        team_id: None,
        team_placeholder: None,
        team_name: Some(team_name),
        goals,
        time_out: None,
        powerplay_instances: 0,
        powerplay_goals: 0,
        short_handed_instances: 0,
        short_handed_goals: 0,
        ranking: None,
        game_start_date_time: Some(start_time),
        goal_events,
    }
}

/// Converts a ScheduleApiGame to a ScheduleGame with full details
async fn convert_api_game_to_schedule_game(
    client: &Client,
    config: &Config,
    api_game: ScheduleApiGame,
    season: i32,
) -> Result<ScheduleGame, AppError> {
    let start_time = api_game.start.clone();

    // 1. Fetch detailed game data
    let detailed_game_data =
        fetch_and_convert_detailed_game_data(client, config, season, api_game.id).await;

    // 2. Create a player name mapping from the resolved goal events to preserve player names
    // Only cache if we don't already have player data (to avoid overwriting detailed disambiguation)
    if get_cached_players(api_game.id).await.is_none() {
        // Format the names properly for teletext display (last name only, properly capitalized)
        let mut player_names = HashMap::new();
        for event in &detailed_game_data.goal_events {
            let formatted_name = format_for_display(&event.scorer_name);
            player_names.insert(event.scorer_player_id, formatted_name);
        }

        // 3. Cache the player names for this game so they're available during later processing
        cache_players(api_game.id, player_names).await;
    }

    // 4. Convert goal events for home and away teams
    let home_goal_events = convert_goal_events_for_team(
        &detailed_game_data.goal_events,
        true,
        &detailed_game_data.detailed_game,
    );
    let away_goal_events = convert_goal_events_for_team(
        &detailed_game_data.goal_events,
        false,
        &detailed_game_data.detailed_game,
    );

    // 5. Build ScheduleTeam structs
    let home_team = build_schedule_team_from_api_and_detailed(
        api_game.home_team_name.clone(),
        detailed_game_data.home_goals,
        start_time.clone(),
        home_goal_events,
    );
    let away_team = build_schedule_team_from_api_and_detailed(
        api_game.away_team_name.clone(),
        detailed_game_data.away_goals,
        start_time.clone(),
        away_goal_events,
    );

    let tournament = TournamentType::from_serie(api_game.serie);

    Ok(ScheduleGame {
        id: api_game.id,
        season: api_game.season,
        start: start_time.clone(),
        end: None, // Not available in schedule API
        home_team,
        away_team,
        finished_type: api_game.finished_type,
        started: api_game.started,
        ended: api_game.ended,
        game_time: api_game.game_time.unwrap_or(0),
        serie: tournament.as_str().to_string(),
    })
}

/// Fetches games for a specific date from a historical season using the schedule endpoint.
/// This is used when the date-based games endpoint doesn't support historical data.
#[instrument(skip(client, config))]
pub(super) async fn fetch_historical_games(
    client: &Client,
    config: &Config,
    date: &str,
) -> Result<Vec<GameData>, AppError> {
    info!("Fetching historical games for date: {date}");

    // Parse the date and determine the season
    let (_, month, season) = parse_date_and_season(date);

    // Determine which tournaments to check based on the month
    let tournaments = determine_tournaments_for_month(month);

    // Fetch games from all relevant tournaments
    let all_schedule_games = fetch_tournament_games(client, config, &tournaments, season).await;

    if all_schedule_games.is_empty() {
        info!("No games found in any tournament for season {}", season);
        return Ok(Vec::new());
    }

    // Filter games to match the requested date
    let matching_games = filter_games_by_date(all_schedule_games, date);

    if matching_games.is_empty() {
        return Ok(Vec::new());
    }

    // Convert ScheduleApiGame to ScheduleGame format with detailed data
    use futures::future::join_all;
    let conversion_futures = matching_games
        .into_iter()
        .map(|api_game| convert_api_game_to_schedule_game(client, config, api_game, season));
    let results = join_all(conversion_futures).await;

    let mut schedule_games = Vec::new();
    let mut failed_games = 0;
    for result in results {
        match result {
            Ok(game) => schedule_games.push(game),
            Err(e) => {
                failed_games += 1;
                warn!("Failed to convert historical game: {e}");
            }
        }
    }
    if failed_games > 0 {
        warn!("{} games failed to convert and were skipped", failed_games);
    }

    // Create a ScheduleResponse with the filtered games
    let schedule_response = ScheduleResponse {
        games: schedule_games,
        previous_game_date: None,
        next_game_date: None,
    };

    // Process the games using the existing logic
    let response_data = vec![schedule_response];
    process_games(client, config, response_data).await
}

/// Fetches detailed game data for a historical game to get actual scores and goal events.
/// Returns a struct with home and away team goals and goal events.
async fn fetch_detailed_game_data_for_historical_game(
    client: &Client,
    config: &Config,
    season: i32,
    game_id: i32,
) -> DetailedGameData {
    let url = build_game_url(&config.api_domain, season, game_id);

    match fetch::<DetailedGameResponse>(client, &url).await {
        Ok(response) => {
            info!(
                "Successfully fetched detailed game data for game ID: {}",
                game_id
            );

            // Process goal events to get scorer information with player lookup
            let goal_events = process_goal_events_for_historical_game_with_players(
                &response.game,
                &response.home_team_players,
                &response.away_team_players,
            )
            .await;

            DetailedGameData {
                home_goals: response.game.home_team.goals,
                away_goals: response.game.away_team.goals,
                goal_events,
                detailed_game: response.game,
            }
        }
        Err(e) => {
            warn!(
                "Failed to fetch detailed game data for game ID {}: {}. Using default scores.",
                game_id, e
            );
            // Return default data if detailed data fetch fails
            // Create a minimal DetailedGame for fallback
            let fallback_game = DetailedGame {
                id: game_id,
                season,
                start: "".to_string(),
                end: None,
                home_team: DetailedTeam {
                    team_id: "".to_string(),
                    team_name: "".to_string(),
                    goals: 0,
                    goal_events: vec![],
                    penalty_events: vec![],
                },
                away_team: DetailedTeam {
                    team_id: "".to_string(),
                    team_name: "".to_string(),
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
            };

            DetailedGameData {
                home_goals: 0,
                away_goals: 0,
                goal_events: vec![],
                detailed_game: fallback_game,
            }
        }
    }
}

/// Process goal events for historical games to extract scorer information with player lookup
/// Uses the player data from the detailed game response to resolve actual player names
pub(super) async fn process_goal_events_for_historical_game_with_players(
    game: &DetailedGame,
    home_team_players: &[Player],
    away_team_players: &[Player],
) -> Vec<GoalEventData> {
    let mut all_goal_events = Vec::new();

    // Create player lookup maps for efficient name resolution
    let home_player_map: HashMap<i64, &Player> = home_team_players
        .iter()
        .map(|player| (player.id, player))
        .collect();
    let away_player_map: HashMap<i64, &Player> = away_team_players
        .iter()
        .map(|player| (player.id, player))
        .collect();

    // Helper function to get player name with fallback
    let get_player_name = |player_id: i64, player_map: &HashMap<i64, &Player>| {
        player_map
            .get(&player_id)
            .map(|player| format!("{} {}", player.first_name, player.last_name))
            .unwrap_or_else(|| format!("Player {player_id}"))
    };

    // Process home team goal events
    for event in &game.home_team.goal_events {
        let scorer_name = get_player_name(event.scorer_player_id, &home_player_map);
        let goal_event = GoalEventData {
            scorer_player_id: event.scorer_player_id,
            scorer_name,
            minute: event.game_time / 60, // Convert seconds to minutes
            home_team_score: event.home_team_score,
            away_team_score: event.away_team_score,
            is_winning_goal: event.winning_goal,
            goal_types: event.goal_types.clone(),
            is_home_team: true,
            video_clip_url: event.video_clip_url.clone(),
        };
        all_goal_events.push(goal_event);
    }

    // Process away team goal events
    for event in &game.away_team.goal_events {
        let scorer_name = get_player_name(event.scorer_player_id, &away_player_map);
        let goal_event = GoalEventData {
            scorer_player_id: event.scorer_player_id,
            scorer_name,
            minute: event.game_time / 60, // Convert seconds to minutes
            home_team_score: event.home_team_score,
            away_team_score: event.away_team_score,
            is_winning_goal: event.winning_goal,
            goal_types: event.goal_types.clone(),
            is_home_team: false,
            video_clip_url: event.video_clip_url.clone(),
        };
        all_goal_events.push(goal_event);
    }

    // Sort by game time
    all_goal_events.sort_by_key(|event| event.minute);
    all_goal_events
}

/// Enhanced struct to hold game data including goal events and detailed game information
struct DetailedGameData {
    home_goals: i32,
    away_goals: i32,
    goal_events: Vec<GoalEventData>,
    detailed_game: DetailedGame,
}
