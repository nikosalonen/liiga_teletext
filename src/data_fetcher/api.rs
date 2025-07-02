use crate::config::Config;
use crate::data_fetcher::cache::{cache_players_with_formatting, get_cached_players};
use crate::data_fetcher::models::{
    DetailedGameResponse, GameData, GoalEventData, ScheduleApiGame, ScheduleGame, ScheduleResponse,
    ScheduleTeam,
};
use crate::data_fetcher::player_names::{build_full_name, format_for_display};
use crate::data_fetcher::processors::{
    create_basic_goal_events, determine_game_status, format_time, process_goal_events,
    should_show_todays_games,
};
use crate::error::AppError;
use chrono::{Datelike, Local, Utc};
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use tracing::{debug, error, info, instrument};

// Tournament season constants for month-based logic
const PRESEASON_START_MONTH: u32 = 5; // May
const PRESEASON_END_MONTH: u32 = 9; // September
const PLAYOFFS_START_MONTH: u32 = 3; // March
const PLAYOFFS_END_MONTH: u32 = 6; // June

/// Builds a tournament URL for fetching game data.
/// This constructs the API endpoint for a specific tournament and date.
///
/// # Arguments
/// * `api_domain` - The base API domain
/// * `tournament` - The tournament identifier
/// * `date` - The date in YYYY-MM-DD format
///
/// # Returns
/// * `String` - The complete tournament URL
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::api::build_tournament_url;
///
/// let url = build_tournament_url("https://api.example.com", "runkosarja", "2024-01-15");
/// assert_eq!(url, "https://api.example.com/games?tournament=runkosarja&date=2024-01-15");
/// ```
pub fn build_tournament_url(api_domain: &str, tournament: &str, date: &str) -> String {
    format!("{api_domain}/games?tournament={tournament}&date={date}")
}

/// Builds a game URL for fetching detailed game data.
/// This constructs the API endpoint for a specific game by season and game ID.
///
/// # Arguments
/// * `api_domain` - The base API domain
/// * `season` - The season year
/// * `game_id` - The unique game identifier
///
/// # Returns
/// * `String` - The complete game URL
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::api::build_game_url;
///
/// let url = build_game_url("https://api.example.com", 2024, 12345);
/// assert_eq!(url, "https://api.example.com/games/2024/12345");
/// ```
pub fn build_game_url(api_domain: &str, season: i32, game_id: i32) -> String {
    format!("{api_domain}/games/{season}/{game_id}")
}

/// Builds a schedule URL for fetching season schedule data.
/// This constructs the API endpoint for a specific tournament and season.
///
/// # Arguments
/// * `api_domain` - The base API domain
/// * `season` - The season year
///
/// # Returns
/// * `String` - The complete schedule URL
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::api::build_schedule_url;
///
/// let url = build_schedule_url("https://api.example.com", 2024);
/// assert_eq!(url, "https://api.example.com/schedule?tournament=runkosarja&week=1&season=2024");
/// ```
pub fn build_schedule_url(api_domain: &str, season: i32) -> String {
    format!("{api_domain}/schedule?tournament=runkosarja&week=1&season={season}")
}

/// Creates a tournament key for caching and identification purposes.
/// This combines tournament name and date into a unique identifier.
///
/// # Arguments
/// * `tournament` - The tournament identifier
/// * `date` - The date in YYYY-MM-DD format
///
/// # Returns
/// * `String` - The tournament key (e.g., "runkosarja-2024-01-15")
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::api::create_tournament_key;
///
/// let key = create_tournament_key("runkosarja", "2024-01-15");
/// assert_eq!(key, "runkosarja-2024-01-15");
/// ```
pub fn create_tournament_key(tournament: &str, date: &str) -> String {
    format!("{tournament}-{date}")
}

/// Helper function to extract team name from a ScheduleTeam, with fallback logic.
/// Returns the team_name if available, otherwise team_placeholder, or "Unknown" as last resort.
fn get_team_name(team: &ScheduleTeam) -> &str {
    team.team_name
        .as_deref()
        .or(team.team_placeholder.as_deref())
        .unwrap_or("Unknown")
}

/// Determines the date to fetch data for based on custom date or current time.
/// Returns today's date if games should be shown today, otherwise yesterday's date.
/// Uses UTC internally for consistent calculations, formats as local date for display.
fn determine_fetch_date(custom_date: Option<String>) -> String {
    custom_date.unwrap_or_else(|| {
        // Use UTC for internal calculations to avoid DST issues
        let now_utc = Utc::now();
        // Convert to local time for the date decision logic
        let now_local = now_utc.with_timezone(&Local);

        if should_show_todays_games() {
            let date_str = now_local.format("%Y-%m-%d").to_string();
            info!("Using today's date: {}", date_str);
            date_str
        } else {
            let yesterday = now_local
                .date_naive()
                .pred_opt()
                .expect("Date underflow cannot happen with valid date");
            let date_str = yesterday.format("%Y-%m-%d").to_string();
            info!("Using yesterday's date: {}", date_str);
            date_str
        }
    })
}

/// Builds the list of tournaments to fetch based on the month.
/// Different tournaments are active during different parts of the season.
fn build_tournament_list(date: &str) -> Vec<&'static str> {
    // Parse the date to get the month
    let date_parts: Vec<&str> = date.split('-').collect();
    let month = if date_parts.len() >= 2 {
        date_parts[1].parse::<u32>().unwrap_or(0)
    } else {
        // Default to current month if date parsing fails
        // Use UTC for consistency, convert to local time for month extraction
        Utc::now().with_timezone(&Local).month()
    };

    let mut tournaments = Vec::new();

    // Only include valmistavat_ottelut during preseason (May-September)
    if (PRESEASON_START_MONTH..=PRESEASON_END_MONTH).contains(&month) {
        info!(
            "Including valmistavat_ottelut (month is {} - May<->Sep)",
            month
        );
        tournaments.push("valmistavat_ottelut");
    }

    // Always include runkosarja
    tournaments.push("runkosarja");

    // Only include playoffs, playout, and qualifications during playoff season (March-June)
    if (PLAYOFFS_START_MONTH..=PLAYOFFS_END_MONTH).contains(&month) {
        info!(
            "Including playoffs, playout, and qualifications (month is {} >= 3)",
            month
        );
        tournaments.push("playoffs");
        tournaments.push("playout");
        tournaments.push("qualifications");
    } else {
        info!(
            "Excluding playoffs, playout, and qualifications (month is {} < 3)",
            month
        );
    }

    tournaments
}

/// Processes next game dates when no games are found for the current date.
/// Returns the earliest next game date and tournaments that have games on that date.
async fn process_next_game_dates(
    client: &Client,
    config: &Config,
    tournaments: &[&str],
    date: &str,
    tournament_responses: HashMap<String, ScheduleResponse>,
) -> Result<(Option<String>, Vec<ScheduleResponse>), AppError> {
    let mut tournament_next_dates: HashMap<&str, String> = HashMap::new();
    let mut earliest_date: Option<String> = None;

    // Check for next game dates using the tournament responses we already have
    for tournament in tournaments {
        let tournament_key = create_tournament_key(tournament, date);

        if let Some(response) = tournament_responses.get(&tournament_key) {
            if let Some(next_date) = &response.next_game_date {
                // Update earliest date if this is the first date or earlier than current earliest
                if earliest_date.is_none() || next_date < earliest_date.as_ref().unwrap() {
                    earliest_date = Some(next_date.clone());
                    info!("Updated earliest date to: {}", next_date);
                }
                tournament_next_dates.insert(*tournament, next_date.clone());
                info!(
                    "Tournament {} has next game date: {}",
                    tournament, next_date
                );
            } else {
                info!("Tournament {} has no next game date", tournament);
            }
        } else {
            info!("No response found for tournament key: {}", tournament_key);
        }
    }

    if let Some(next_date) = earliest_date.clone() {
        info!("Found earliest next game date: {}", next_date);
        // Only fetch tournaments that have games on the earliest date
        let tournaments_to_fetch: Vec<&str> = tournament_next_dates
            .iter()
            .filter_map(|(tournament, date)| {
                if date == &next_date {
                    info!("Tournament {} has games on the earliest date", tournament);
                    Some(*tournament)
                } else {
                    info!(
                        "Tournament {} has games on a later date: {}",
                        tournament, date
                    );
                    None
                }
            })
            .collect();

        if !tournaments_to_fetch.is_empty() {
            info!(
                "Fetching games for next date: {} for tournaments: {:?}",
                next_date, tournaments_to_fetch
            );

            let mut response_data = Vec::new();

            // Directly fetch tournament data for each tournament
            for tournament in &tournaments_to_fetch {
                info!(
                    "Fetching data for tournament {} on date {}",
                    tournament, next_date
                );
                match fetch_tournament_data(client, config, tournament, &next_date).await {
                    Ok(response) => {
                        if !response.games.is_empty() {
                            info!(
                                "Found {} games for tournament {} on date {}",
                                response.games.len(),
                                tournament,
                                next_date
                            );
                            response_data.push(response);
                        } else {
                            info!(
                                "No games found for tournament {} on date {} despite next_game_date indicating games should exist",
                                tournament, next_date
                            );
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to fetch tournament data for {} on date {}: {}",
                            tournament, next_date, e
                        );
                    }
                }
            }

            // If we didn't find any games with direct fetching, try the regular fetch_day_data
            if response_data.is_empty() {
                info!("No games found with direct tournament fetching, trying fetch_day_data");
                match fetch_day_data(client, config, &tournaments_to_fetch, &next_date).await {
                    Ok((next_games_option, _)) => {
                        if let Some(responses) = next_games_option {
                            info!("Found {} responses with fetch_day_data", responses.len());
                            response_data = responses;
                        } else {
                            info!("No games found with fetch_day_data either");
                        }
                    }
                    Err(e) => {
                        // Log the error but continue with empty response_data
                        error!(
                            "Failed to fetch next games with fetch_day_data: {}. Continuing with empty data.",
                            e
                        );
                    }
                }
            }

            Ok((Some(next_date), response_data))
        } else {
            info!(
                "No tournaments have games on the earliest date: {}",
                next_date
            );
            Ok((Some(next_date), Vec::new()))
        }
    } else {
        info!("No next game date found for any tournament");
        Ok((None, Vec::new()))
    }
}

/// Determines if a game has actual goals (excluding RL0 goal types).
fn has_actual_goals(game: &ScheduleGame) -> bool {
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
fn should_fetch_detailed_data(game: &ScheduleGame) -> bool {
    if game.started {
        has_actual_goals(game) || !game.ended
    } else {
        false
    }
}

/// Processes a single game and returns GameData.
async fn process_single_game(
    client: &Client,
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

    let time = if !game.started {
        let formatted_time = format_time(&game.start).unwrap_or_default();
        info!("Game not started, formatted time: {}", formatted_time);
        formatted_time
    } else {
        info!("Game already started, no time to display");
        String::new()
    };

    let result = format!("{}-{}", game.home_team.goals, game.away_team.goals);
    info!("Game result: {}", result);

    let (score_type, is_overtime, is_shootout) = determine_game_status(&game);
    info!(
        "Game status: {:?}, overtime: {}, shootout: {}",
        score_type, is_overtime, is_shootout
    );

    let goal_events = if should_fetch_detailed_data(&game) {
        info!("Fetching detailed game data");
        fetch_detailed_game_data(client, config, &game).await
    } else {
        info!("No detailed data needed for this game");
        Vec::new()
    };

    info!(
        "Successfully processed game #{} in response #{}",
        game_idx + 1,
        response_idx + 1
    );

    info!("Game serie from API: '{}'", game.serie);
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
async fn process_response_games(
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

async fn process_games(
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

#[instrument(skip(client))]
async fn fetch<T: DeserializeOwned>(client: &Client, url: &str) -> Result<T, AppError> {
    info!("Fetching data from URL: {}", url);

    // Handle reqwest errors with specific error types
    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(e) => {
            error!("Request failed for URL {}: {}", url, e);

            // Categorize reqwest errors into specific types
            if e.is_timeout() {
                return Err(AppError::network_timeout(url));
            } else if e.is_connect() {
                return Err(AppError::network_connection(url, e.to_string()));
            } else {
                // For other reqwest errors, keep the original behavior
                return Err(AppError::ApiFetch(e));
            }
        }
    };

    let status = response.status();
    let headers = response.headers().clone();

    info!("Response status: {}", status);
    debug!("Response headers: {:?}", headers);

    if !status.is_success() {
        let status_code = status.as_u16();
        let reason = status.canonical_reason().unwrap_or("Unknown error");

        error!("HTTP {} - {} (URL: {})", status_code, reason, url);

        // Return specific error types based on HTTP status code
        return Err(match status_code {
            404 => AppError::api_not_found(url),
            429 => AppError::api_rate_limit(reason, url),
            400..=499 => AppError::api_client_error(status_code, reason, url),
            500..=599 => {
                if status_code == 502 || status_code == 503 {
                    AppError::api_service_unavailable(status_code, reason, url)
                } else {
                    AppError::api_server_error(status_code, reason, url)
                }
            }
            _ => AppError::api_server_error(status_code, reason, url),
        });
    }

    let response_text = match response.text().await {
        Ok(text) => text,
        Err(e) => {
            error!("Failed to read response text from URL {}: {}", url, e);
            return Err(AppError::ApiFetch(e));
        }
    };

    info!("Response length: {} bytes", response_text.len());
    debug!("Response text: {}", response_text);

    // Enhanced JSON parsing with more specific error handling
    match serde_json::from_str::<T>(&response_text) {
        Ok(parsed) => Ok(parsed),
        Err(e) => {
            error!("Failed to parse API response: {} (URL: {})", e, url);
            error!(
                "Response text (first 200 chars): {}",
                &response_text.chars().take(200).collect::<String>()
            );

            // Check if it's malformed JSON vs unexpected structure
            if response_text.trim().is_empty() {
                Err(AppError::api_no_data("Response body is empty", url))
            } else if !response_text.trim_start().starts_with('{')
                && !response_text.trim_start().starts_with('[')
            {
                Err(AppError::api_malformed_json(
                    "Response is not valid JSON",
                    url,
                ))
            } else {
                // Valid JSON but unexpected structure
                Err(AppError::api_unexpected_structure(e.to_string(), url))
            }
        }
    }
}

/// Fetches game data for a specific tournament and date from the API.
#[instrument(skip(client, config))]
pub async fn fetch_tournament_data(
    client: &Client,
    config: &Config,
    tournament: &str,
    date: &str,
) -> Result<ScheduleResponse, AppError> {
    info!("Fetching tournament data for {} on {}", tournament, date);
    let url = build_tournament_url(&config.api_domain, tournament, date);

    match fetch(client, &url).await {
        Ok(response) => {
            info!(
                "Successfully fetched tournament data for {} on {}",
                tournament, date
            );
            Ok(response)
        }
        Err(e) => {
            error!(
                "Failed to fetch tournament data for {} on {}: {}",
                tournament, date, e
            );

            // Transform API not found errors to tournament-specific errors
            match &e {
                AppError::ApiNotFound { .. } => {
                    Err(AppError::api_tournament_not_found(tournament, date))
                }
                _ => Err(e),
            }
        }
    }
}

#[allow(clippy::type_complexity)]
async fn fetch_day_data(
    client: &Client,
    config: &Config,
    tournaments: &[&str],
    date: &str,
) -> Result<
    (
        Option<Vec<ScheduleResponse>>,
        HashMap<String, ScheduleResponse>,
    ),
    AppError,
> {
    let mut responses = Vec::new();
    let mut found_games = false;
    let mut tournament_responses = HashMap::new();

    // Process tournaments sequentially to respect priority order
    for tournament in tournaments {
        if let Ok(response) = fetch_tournament_data(client, config, tournament, date).await {
            // Store all responses in the HashMap for potential reuse
            let tournament_key = create_tournament_key(tournament, date);
            tournament_responses.insert(tournament_key, response.clone());

            if !response.games.is_empty() {
                responses.push(response);
                found_games = true;
            }
        }
    }

    if found_games {
        Ok((Some(responses), tournament_responses))
    } else {
        Ok((None, tournament_responses))
    }
}

/// Handles the case when no games are found for the current date.
/// Returns the response data and earliest date for next games.
async fn handle_no_games_found(
    client: &Client,
    config: &Config,
    tournaments: &[&str],
    date: &str,
    tournament_responses: HashMap<String, ScheduleResponse>,
) -> Result<(Vec<ScheduleResponse>, Option<String>), AppError> {
    info!("No games found for the current date, checking for next game dates");
    let (next_date, next_responses) =
        process_next_game_dates(client, config, tournaments, date, tournament_responses).await?;
    Ok((next_responses, next_date))
}

/// Determines the appropriate date to return based on whether games were found.
fn determine_return_date(
    games: &[GameData],
    earliest_date: Option<String>,
    original_date: &str,
) -> String {
    if games.is_empty() {
        earliest_date.unwrap_or_else(|| original_date.to_string())
    } else {
        original_date.to_string()
    }
}

#[instrument(skip(custom_date))]
pub async fn fetch_liiga_data(
    custom_date: Option<String>,
) -> Result<(Vec<GameData>, String), AppError> {
    info!("Starting to fetch Liiga data");
    let config = Config::load().await?;
    info!("Config loaded successfully");
    let client = Client::new();

    // Determine the date to fetch data for
    let date = determine_fetch_date(custom_date);

    // Build the list of tournaments to fetch based on the month
    let tournaments = build_tournament_list(&date);

    // First try to fetch data for the current date
    info!(
        "Fetching data for date: {} with tournaments: {:?}",
        date, tournaments
    );
    let (games_option, tournament_responses) =
        fetch_day_data(&client, &config, &tournaments, &date).await?;

    let (response_data, earliest_date) = if let Some(responses) = games_option {
        info!(
            "Found games for the current date. Number of responses: {}",
            responses.len()
        );
        (responses, None)
    } else {
        handle_no_games_found(&client, &config, &tournaments, &date, tournament_responses).await?
    };

    // Process games if we found any
    let all_games = process_games(&client, &config, response_data).await?;

    // Determine the appropriate date to return
    let return_date = determine_return_date(&all_games, earliest_date.clone(), &date);

    if all_games.is_empty() {
        info!("No games found after processing all data");
        if earliest_date.is_some() {
            info!("Returning empty games list with next date: {}", return_date);
        } else {
            info!(
                "Returning empty games list with original date: {}",
                return_date
            );
        }
    } else {
        info!(
            "Returning {} games with date: {}",
            all_games.len(),
            return_date
        );
    }

    Ok((all_games, return_date))
}

#[instrument(skip(client, config))]
async fn fetch_detailed_game_data(
    client: &Client,
    config: &Config,
    game: &ScheduleGame,
) -> Vec<GoalEventData> {
    info!(
        "Fetching detailed game data for game ID: {} (season: {})",
        game.id, game.season
    );
    match fetch_game_data(client, config, game.season, game.id).await {
        Ok(detailed_data) => {
            info!(
                "Successfully fetched detailed game data: {} goal events",
                detailed_data.len()
            );
            detailed_data
        }
        Err(e) => {
            error!(
                "Failed to fetch detailed game data for game ID {}: {}. Using basic game data.",
                game.id, e
            );
            let basic_events = create_basic_goal_events(game);
            info!(
                "Created {} basic goal events as fallback",
                basic_events.len()
            );
            basic_events
        }
    }
}

#[instrument(skip(client, config))]
async fn fetch_game_data(
    client: &Client,
    config: &Config,
    season: i32,
    game_id: i32,
) -> Result<Vec<GoalEventData>, AppError> {
    info!(
        "Fetching game data for game ID: {} (season: {})",
        game_id, season
    );
    let url = build_game_url(&config.api_domain, season, game_id);

    // Try to get detailed game response
    info!("Making API request to: {}", url);
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
            match &e {
                AppError::ApiNotFound { .. } => {
                    return Err(AppError::api_game_not_found(game_id, season));
                }
                _ => return Err(e),
            }
        }
    };

    // Check cache first
    info!("Checking player cache for game ID: {}", game_id);
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
        return Ok(events);
    }

    // Build player names map if not in cache
    info!("No cached player data found, building player names map");
    let mut player_names = HashMap::new();
    info!(
        "Processing {} home team players",
        game_response.home_team_players.len()
    );
    for player in &game_response.home_team_players {
        player_names.insert(
            player.id,
            build_full_name(&player.first_name, &player.last_name),
        );
    }

    info!(
        "Processing {} away team players",
        game_response.away_team_players.len()
    );
    for player in &game_response.away_team_players {
        player_names.insert(
            player.id,
            build_full_name(&player.first_name, &player.last_name),
        );
    }
    info!("Built player names map with {} players", player_names.len());

    // Update cache with formatted names
    info!("Updating player cache for game ID: {}", game_id);
    cache_players_with_formatting(game_id, player_names.clone()).await;

    // Get the formatted names from cache for processing
    let formatted_players = match get_cached_players(game_id).await {
        Some(players) => players,
        None => {
            error!(
                "Failed to retrieve cached player data for game ID {} after caching. This should not happen.",
                game_id
            );
            // Fallback: use the raw player names and format them on-the-fly
            let fallback_players: HashMap<i64, String> = player_names
                .into_iter()
                .map(|(id, full_name)| (id, format_for_display(&full_name)))
                .collect();
            fallback_players
        }
    };
    let events = process_goal_events(&game_response.game, &formatted_players);
    info!(
        "Processed {} goal events for game ID: {}",
        events.len(),
        game_id
    );
    Ok(events)
}

/// Fetches the regular season schedule to determine the season start date.
/// Returns the start date of the first regular season game.
#[instrument(skip(client, config))]
pub async fn fetch_regular_season_start_date(
    client: &Client,
    config: &Config,
    season: i32,
) -> Result<Option<String>, AppError> {
    info!("Fetching regular season schedule for season: {}", season);
    let url = build_schedule_url(&config.api_domain, season);

    match fetch::<Vec<ScheduleApiGame>>(client, &url).await {
        Ok(games) => {
            if games.is_empty() {
                info!("No regular season games found for season: {}", season);
                Ok(None)
            } else {
                // Get the earliest start date from the games
                let earliest_game = games
                    .iter()
                    .min_by_key(|game| &game.start)
                    .expect("We already checked that games is not empty");

                info!(
                    "Found regular season start date: {} for season: {}",
                    earliest_game.start, season
                );
                Ok(Some(earliest_game.start.clone()))
            }
        }
        Err(e) => {
            error!(
                "Failed to fetch regular season schedule for season {}: {}",
                season, e
            );

            // Transform API not found errors to season-specific errors
            match &e {
                AppError::ApiNotFound { .. } => Err(AppError::api_season_not_found(season)),
                AppError::ApiNoData { .. } => {
                    info!("Season {} schedule exists but contains no games", season);
                    Ok(None)
                }
                _ => Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::models::{DetailedGame, DetailedTeam, GoalEvent, Period, Player};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    fn create_mock_config() -> Config {
        Config {
            api_domain: "http://localhost:8080".to_string(),
            log_file_path: None,
        }
    }

    fn create_mock_schedule_response() -> ScheduleResponse {
        ScheduleResponse {
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
                    powerplay_instances: 2,
                    powerplay_goals: 1,
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
                    powerplay_instances: 1,
                    powerplay_goals: 0,
                    short_handed_instances: 0,
                    short_handed_goals: 0,
                    ranking: Some(2),
                    game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
                    goal_events: vec![],
                },
                finished_type: Some("normal".to_string()),
                started: true,
                ended: true,
                game_time: 3600,
                serie: "runkosarja".to_string(),
            }],
            previous_game_date: Some("2024-01-14".to_string()),
            next_game_date: Some("2024-01-16".to_string()),
        }
    }

    fn create_mock_empty_schedule_response() -> ScheduleResponse {
        ScheduleResponse {
            games: vec![],
            previous_game_date: Some("2024-01-14".to_string()),
            next_game_date: Some("2024-01-16".to_string()),
        }
    }

    fn create_mock_detailed_game_response() -> DetailedGameResponse {
        DetailedGameResponse {
            game: DetailedGame {
                id: 1,
                season: 2024,
                start: "2024-01-15T18:30:00Z".to_string(),
                end: Some("2024-01-15T20:30:00Z".to_string()),
                home_team: DetailedTeam {
                    team_id: "team1".to_string(),
                    team_name: "HIFK".to_string(),
                    goals: 3,
                    goal_events: vec![GoalEvent {
                        scorer_player_id: 123,
                        log_time: "2024-01-15T19:15:00Z".to_string(),
                        game_time: 2700,
                        period: 2,
                        event_id: 1,
                        home_team_score: 1,
                        away_team_score: 0,
                        winning_goal: false,
                        goal_types: vec!["even_strength".to_string()],
                        assistant_player_ids: vec![456, 789],
                        video_clip_url: Some("https://example.com/video1.mp4".to_string()),
                    }],
                    penalty_events: vec![],
                },
                away_team: DetailedTeam {
                    team_id: "team2".to_string(),
                    team_name: "Tappara".to_string(),
                    goals: 2,
                    goal_events: vec![],
                    penalty_events: vec![],
                },
                periods: vec![
                    Period {
                        index: 1,
                        home_team_goals: 1,
                        away_team_goals: 0,
                        category: "regular".to_string(),
                        start_time: 0,
                        end_time: 1200,
                    },
                    Period {
                        index: 2,
                        home_team_goals: 1,
                        away_team_goals: 1,
                        category: "regular".to_string(),
                        start_time: 1200,
                        end_time: 2400,
                    },
                    Period {
                        index: 3,
                        home_team_goals: 1,
                        away_team_goals: 1,
                        category: "regular".to_string(),
                        start_time: 2400,
                        end_time: 3600,
                    },
                ],
                finished_type: Some("normal".to_string()),
                started: true,
                ended: true,
                game_time: 3600,
                serie: "runkosarja".to_string(),
            },
            awards: vec![],
            home_team_players: vec![Player {
                id: 123,
                last_name: "Smith".to_string(),
                first_name: "John".to_string(),
            }],
            away_team_players: vec![Player {
                id: 456,
                last_name: "Johnson".to_string(),
                first_name: "Mike".to_string(),
            }],
        }
    }

    #[tokio::test]
    async fn test_fetch_tournament_data_success() {
        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let client = Client::new();

        let mock_response = create_mock_schedule_response();

        Mock::given(method("GET"))
            .and(path("/games"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        // Update config to use mock server
        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let result = fetch_tournament_data(&client, &test_config, "runkosarja", "2024-01-15").await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.games.len(), 1);
        assert_eq!(
            response.games[0].home_team.team_name.as_deref(),
            Some("HIFK")
        );
        assert_eq!(
            response.games[0].away_team.team_name.as_deref(),
            Some("Tappara")
        );
    }

    #[tokio::test]
    async fn test_fetch_tournament_data_no_games() {
        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let client = Client::new();

        let mock_response = create_mock_empty_schedule_response();

        Mock::given(method("GET"))
            .and(path("/games"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let result = fetch_tournament_data(&client, &test_config, "runkosarja", "2024-01-15").await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.games.len(), 0);
        assert_eq!(response.next_game_date, Some("2024-01-16".to_string()));
    }

    #[tokio::test]
    async fn test_fetch_tournament_data_server_error() {
        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let client = Client::new();

        Mock::given(method("GET"))
            .and(path("/games"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let result = fetch_tournament_data(&client, &test_config, "runkosarja", "2024-01-15").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_tournament_data_not_found() {
        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let client = Client::new();

        Mock::given(method("GET"))
            .and(path("/games"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let result = fetch_tournament_data(&client, &test_config, "runkosarja", "2024-01-15").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_day_data_success() {
        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let client = Client::new();

        let mock_response = create_mock_schedule_response();

        Mock::given(method("GET"))
            .and(path("/games"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let tournaments = vec!["runkosarja"];
        let result = fetch_day_data(&client, &test_config, &tournaments, "2024-01-15").await;

        assert!(result.is_ok());
        let (responses, _) = result.unwrap();
        assert!(responses.is_some());
        let responses = responses.unwrap();
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].games.len(), 1);
    }

    #[tokio::test]
    async fn test_fetch_day_data_no_games() {
        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let client = Client::new();

        let mock_response = create_mock_empty_schedule_response();

        Mock::given(method("GET"))
            .and(path("/games"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let tournaments = vec!["runkosarja"];
        let result = fetch_day_data(&client, &test_config, &tournaments, "2024-01-15").await;

        assert!(result.is_ok());
        let (responses, _) = result.unwrap();
        assert!(responses.is_none());
    }

    #[tokio::test]
    async fn test_fetch_game_data_success() {
        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let client = Client::new();

        let mock_response = create_mock_detailed_game_response();

        Mock::given(method("GET"))
            .and(path("/games/2024/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let result = fetch_game_data(&client, &test_config, 2024, 1).await;

        assert!(result.is_ok());
        let goal_events = result.unwrap();
        assert_eq!(goal_events.len(), 1);
        assert_eq!(goal_events[0].scorer_name, "Smith");
        assert_eq!(goal_events[0].home_team_score, 1);
        assert_eq!(goal_events[0].away_team_score, 0);
    }

    #[tokio::test]
    async fn test_fetch_game_data_no_goals() {
        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let client = Client::new();

        let mut mock_response = create_mock_detailed_game_response();
        mock_response.game.home_team.goal_events = vec![];
        mock_response.game.away_team.goal_events = vec![];

        Mock::given(method("GET"))
            .and(path("/games/2024/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let result = fetch_game_data(&client, &test_config, 2024, 1).await;

        assert!(result.is_ok());
        let goal_events = result.unwrap();
        assert_eq!(goal_events.len(), 0);
    }

    #[tokio::test]
    async fn test_fetch_game_data_cache_fallback() {
        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let client = Client::new();

        let mock_response = create_mock_detailed_game_response();

        Mock::given(method("GET"))
            .and(path("/games/2024/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        // Clear the cache to simulate a cache miss after caching
        use crate::data_fetcher::cache::PLAYER_CACHE;
        PLAYER_CACHE.write().await.clear();

        let result = fetch_game_data(&client, &test_config, 2024, 1).await;

        // Should still succeed due to fallback logic
        assert!(result.is_ok());
        let goal_events = result.unwrap();
        assert_eq!(goal_events.len(), 1);
        assert_eq!(goal_events[0].scorer_name, "Smith");
        assert_eq!(goal_events[0].home_team_score, 1);
        assert_eq!(goal_events[0].away_team_score, 0);
    }

    #[tokio::test]
    async fn test_fetch_game_data_server_error() {
        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let client = Client::new();

        Mock::given(method("GET"))
            .and(path("/games/2024/1"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let result = fetch_game_data(&client, &test_config, 2024, 1).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_regular_season_start_date_success() {
        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let client = Client::new();

        let mock_response = vec![ScheduleApiGame {
            id: 1,
            season: 2024,
            start: "2024-09-15T18:30:00Z".to_string(),
            home_team_name: "HIFK".to_string(),
            away_team_name: "Tappara".to_string(),
            serie: 1,
            finished_type: None,
            started: false,
            ended: false,
            game_time: None,
        }];

        Mock::given(method("GET"))
            .and(path("/schedule"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let result = fetch_regular_season_start_date(&client, &test_config, 2024).await;

        assert!(result.is_ok());
        let start_date = result.unwrap();
        assert_eq!(start_date, Some("2024-09-15T18:30:00Z".to_string()));
    }

    #[tokio::test]
    async fn test_fetch_regular_season_start_date_not_found() {
        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let client = Client::new();

        Mock::given(method("GET"))
            .and(path("/schedule"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let result = fetch_regular_season_start_date(&client, &test_config, 2024).await;

        assert!(result.is_err());
    }

    #[test]
    fn test_build_tournament_list_preseason() {
        let tournaments = build_tournament_list("2024-08-15");
        assert!(tournaments.contains(&"valmistavat_ottelut"));
        assert!(tournaments.contains(&"runkosarja"));
        assert!(!tournaments.contains(&"playoffs"));
        assert!(!tournaments.contains(&"playout"));
        assert!(!tournaments.contains(&"qualifications"));
    }

    #[test]
    fn test_build_tournament_list_regular_season() {
        let tournaments = build_tournament_list("2024-12-15");
        assert!(!tournaments.contains(&"valmistavat_ottelut"));
        assert!(tournaments.contains(&"runkosarja"));
        assert!(!tournaments.contains(&"playoffs"));
        assert!(!tournaments.contains(&"playout"));
        assert!(!tournaments.contains(&"qualifications"));
    }

    #[test]
    fn test_build_tournament_list_playoffs() {
        let tournaments = build_tournament_list("2024-04-15");
        assert!(!tournaments.contains(&"valmistavat_ottelut"));
        assert!(tournaments.contains(&"runkosarja"));
        assert!(tournaments.contains(&"playoffs"));
        assert!(tournaments.contains(&"playout"));
        assert!(tournaments.contains(&"qualifications"));
    }

    #[test]
    fn test_get_team_name_with_team_name() {
        let team = ScheduleTeam {
            team_id: Some("team1".to_string()),
            team_placeholder: Some("Placeholder".to_string()),
            team_name: Some("HIFK".to_string()),
            goals: 3,
            time_out: None,
            powerplay_instances: 0,
            powerplay_goals: 0,
            short_handed_instances: 0,
            short_handed_goals: 0,
            ranking: None,
            game_start_date_time: None,
            goal_events: vec![],
        };
        assert_eq!(get_team_name(&team), "HIFK");
    }

    #[test]
    fn test_get_team_name_with_placeholder() {
        let team = ScheduleTeam {
            team_id: Some("team1".to_string()),
            team_placeholder: Some("Placeholder".to_string()),
            team_name: None,
            goals: 3,
            time_out: None,
            powerplay_instances: 0,
            powerplay_goals: 0,
            short_handed_instances: 0,
            short_handed_goals: 0,
            ranking: None,
            game_start_date_time: None,
            goal_events: vec![],
        };
        assert_eq!(get_team_name(&team), "Placeholder");
    }

    #[test]
    fn test_get_team_name_unknown() {
        let team = ScheduleTeam {
            team_id: Some("team1".to_string()),
            team_placeholder: None,
            team_name: None,
            goals: 3,
            time_out: None,
            powerplay_instances: 0,
            powerplay_goals: 0,
            short_handed_instances: 0,
            short_handed_goals: 0,
            ranking: None,
            game_start_date_time: None,
            goal_events: vec![],
        };
        assert_eq!(get_team_name(&team), "Unknown");
    }

    #[test]
    fn test_has_actual_goals_with_goals() {
        let game = ScheduleGame {
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
                ranking: None,
                game_start_date_time: None,
                goal_events: vec![GoalEvent {
                    scorer_player_id: 123,
                    log_time: "2024-01-15T19:15:00Z".to_string(),
                    game_time: 2700,
                    period: 2,
                    event_id: 1,
                    home_team_score: 1,
                    away_team_score: 0,
                    winning_goal: false,
                    goal_types: vec!["even_strength".to_string()],
                    assistant_player_ids: vec![],
                    video_clip_url: None,
                }],
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
                ranking: None,
                game_start_date_time: None,
                goal_events: vec![],
            },
            finished_type: Some("normal".to_string()),
            started: true,
            ended: true,
            game_time: 3600,
            serie: "runkosarja".to_string(),
        };
        assert!(has_actual_goals(&game));
    }

    #[test]
    fn test_has_actual_goals_no_goals() {
        let game = ScheduleGame {
            id: 1,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            end: Some("2024-01-15T20:30:00Z".to_string()),
            home_team: ScheduleTeam {
                team_id: Some("team1".to_string()),
                team_placeholder: None,
                team_name: Some("HIFK".to_string()),
                goals: 0,
                time_out: None,
                powerplay_instances: 0,
                powerplay_goals: 0,
                short_handed_instances: 0,
                short_handed_goals: 0,
                ranking: None,
                game_start_date_time: None,
                goal_events: vec![],
            },
            away_team: ScheduleTeam {
                team_id: Some("team2".to_string()),
                team_placeholder: None,
                team_name: Some("Tappara".to_string()),
                goals: 0,
                time_out: None,
                powerplay_instances: 0,
                powerplay_goals: 0,
                short_handed_instances: 0,
                short_handed_goals: 0,
                ranking: None,
                game_start_date_time: None,
                goal_events: vec![],
            },
            finished_type: Some("normal".to_string()),
            started: true,
            ended: true,
            game_time: 3600,
            serie: "runkosarja".to_string(),
        };
        assert!(!has_actual_goals(&game));
    }

    #[test]
    fn test_should_fetch_detailed_data_finished_game() {
        let game = ScheduleGame {
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
                ranking: None,
                game_start_date_time: None,
                goal_events: vec![GoalEvent {
                    scorer_player_id: 123,
                    log_time: "2024-01-15T19:15:00Z".to_string(),
                    game_time: 2700,
                    period: 2,
                    event_id: 1,
                    home_team_score: 1,
                    away_team_score: 0,
                    winning_goal: false,
                    goal_types: vec!["even_strength".to_string()],
                    assistant_player_ids: vec![],
                    video_clip_url: None,
                }],
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
                ranking: None,
                game_start_date_time: None,
                goal_events: vec![],
            },
            finished_type: Some("normal".to_string()),
            started: true,
            ended: true,
            game_time: 3600,
            serie: "runkosarja".to_string(),
        };
        assert!(should_fetch_detailed_data(&game));
    }

    #[test]
    fn test_should_fetch_detailed_data_not_finished() {
        let game = ScheduleGame {
            id: 1,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            end: None,
            home_team: ScheduleTeam {
                team_id: Some("team1".to_string()),
                team_placeholder: None,
                team_name: Some("HIFK".to_string()),
                goals: 0,
                time_out: None,
                powerplay_instances: 0,
                powerplay_goals: 0,
                short_handed_instances: 0,
                short_handed_goals: 0,
                ranking: None,
                game_start_date_time: None,
                goal_events: vec![],
            },
            away_team: ScheduleTeam {
                team_id: Some("team2".to_string()),
                team_placeholder: None,
                team_name: Some("Tappara".to_string()),
                goals: 0,
                time_out: None,
                powerplay_instances: 0,
                powerplay_goals: 0,
                short_handed_instances: 0,
                short_handed_goals: 0,
                ranking: None,
                game_start_date_time: None,
                goal_events: vec![],
            },
            finished_type: None,
            started: false,
            ended: false,
            game_time: 0,
            serie: "runkosarja".to_string(),
        };
        assert!(!should_fetch_detailed_data(&game));
    }
}
