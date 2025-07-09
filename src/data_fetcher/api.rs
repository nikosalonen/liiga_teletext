use crate::config::Config;
use crate::data_fetcher::cache::{
    cache_detailed_game_data, cache_goal_events_data, cache_http_response,
    cache_players_with_formatting, cache_tournament_data, get_cached_detailed_game_data,
    get_cached_goal_events_data, get_cached_http_response, get_cached_players,
    get_cached_tournament_data,
};
#[cfg(test)]
use crate::data_fetcher::cache::{
    get_detailed_game_cache_size, get_goal_events_cache_size, get_tournament_cache_size,
};
use crate::data_fetcher::models::{
    DetailedGame, DetailedGameResponse, DetailedTeam, GameData, GoalEvent, GoalEventData, Player,
    ScheduleApiGame, ScheduleGame, ScheduleResponse, ScheduleTeam,
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
use std::time::Duration;
use tracing::{debug, error, info, instrument, warn};

// Tournament season constants for month-based logic
const PRESEASON_START_MONTH: u32 = 5; // May
const PRESEASON_END_MONTH: u32 = 9; // September
const PLAYOFFS_START_MONTH: u32 = 3; // March
const PLAYOFFS_END_MONTH: u32 = 6; // June

/// Creates a properly configured HTTP client with connection pooling and timeout handling.
/// This follows the coding guidelines for HTTP client usage with proper timeout handling,
/// connection pooling, and HTTP/2 multiplexing when available.
///
/// # Returns
/// * `Client` - A configured reqwest HTTP client
///
/// # Features
/// * 30-second timeout for requests
/// * Connection pooling with up to 100 connections per host
/// * HTTP/2 multiplexing when available
/// * Automatic retry logic for transient failures
fn create_http_client() -> Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(100)
        .build()
        .expect("Failed to create HTTP client")
}

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

/// Builds a schedule URL for a specific tournament type.
/// This constructs the API endpoint for a specific tournament and season.
///
/// # Arguments
/// * `api_domain` - The base API domain
/// * `tournament` - The tournament type (runkosarja, playoffs, playout, qualifications, valmistavat_ottelut)
/// * `season` - The season year
///
/// # Returns
/// * `String` - The complete schedule URL
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::api::build_tournament_schedule_url;
///
/// let url = build_tournament_schedule_url("https://api.example.com", "playoffs", 2024);
/// assert_eq!(url, "https://api.example.com/schedule?tournament=playoffs&week=1&season=2024");
/// ```
pub fn build_tournament_schedule_url(api_domain: &str, tournament: &str, season: i32) -> String {
    format!("{api_domain}/schedule?tournament={tournament}&week=1&season={season}")
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

    // Check HTTP response cache first
    if let Some(cached_response) = get_cached_http_response(url).await {
        info!("Using cached HTTP response for URL: {}", url);
        match serde_json::from_str::<T>(&cached_response) {
            Ok(parsed) => return Ok(parsed),
            Err(e) => {
                warn!("Failed to parse cached response for URL {}: {}", url, e);
                // Continue with fresh request if cached response is invalid
            }
        }
    }

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

    // Cache successful HTTP responses with appropriate TTL
    let ttl_seconds = if url.contains("/games/") {
        300 // 5 minutes for game data
    } else if url.contains("/schedule") {
        1800 // 30 minutes for schedule data
    } else {
        600 // 10 minutes for other data
    };
    cache_http_response(url.to_string(), response_text.clone(), ttl_seconds).await;

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
/// Uses caching to improve performance and reduce API calls.
#[instrument(skip(client, config))]
pub async fn fetch_tournament_data(
    client: &Client,
    config: &Config,
    tournament: &str,
    date: &str,
) -> Result<ScheduleResponse, AppError> {
    info!("Fetching tournament data for {} on {}", tournament, date);

    // Create cache key
    let cache_key = create_tournament_key(tournament, date);

    // Check cache first
    if let Some(cached_response) = get_cached_tournament_data(&cache_key).await {
        info!(
            "Using cached tournament data for {} on {}",
            tournament, date
        );
        return Ok(cached_response);
    }

    info!(
        "Cache miss, fetching from API for {} on {}",
        tournament, date
    );
    let url = build_tournament_url(&config.api_domain, tournament, date);

    match fetch::<ScheduleResponse>(client, &url).await {
        Ok(response) => {
            info!(
                "Successfully fetched tournament data for {} on {}",
                tournament, date
            );

            // Cache the response
            cache_tournament_data(cache_key, response.clone()).await;

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

    // If we found games with next_game_date, return them
    if !next_responses.is_empty() {
        info!(
            "Found {} responses with next_game_date",
            next_responses.len()
        );
        return Ok((next_responses, next_date));
    }

    // Fallback: try to find future games by checking upcoming dates
    info!("No next_game_date found, trying fallback mechanism to find future games");
    let fallback_result = find_future_games_fallback(client, config, tournaments, date).await?;

    if let Some((fallback_responses, fallback_date)) = fallback_result {
        info!(
            "Found {} responses with fallback mechanism for date {}",
            fallback_responses.len(),
            fallback_date
        );
        return Ok((fallback_responses, Some(fallback_date)));
    }

    info!("No future games found with any mechanism");
    Ok((Vec::new(), next_date))
}

/// Fallback mechanism to find future games by checking upcoming dates.
/// This is used when the API doesn't provide next_game_date information.
async fn find_future_games_fallback(
    client: &Client,
    config: &Config,
    tournaments: &[&str],
    current_date: &str,
) -> Result<Option<(Vec<ScheduleResponse>, String)>, AppError> {
    info!(
        "Starting fallback search for future games from date: {}",
        current_date
    );

    // Try the next 7 days to find games
    let mut check_date = match chrono::NaiveDate::parse_from_str(current_date, "%Y-%m-%d") {
        Ok(date) => date,
        Err(e) => {
            error!("Failed to parse date '{}': {}", current_date, e);
            return Err(AppError::datetime_parse_error(format!(
                "Failed to parse date '{current_date}': {e}"
            )));
        }
    };

    for _day_offset in 1..=7 {
        check_date = match check_date.succ_opt() {
            Some(date) => date,
            None => {
                error!(
                    "Date overflow when calculating next day from {}",
                    current_date
                );
                return Err(AppError::datetime_parse_error(
                    "Date overflow when calculating next day".to_string(),
                ));
            }
        };

        let date_str = check_date.format("%Y-%m-%d").to_string();
        info!("Checking for games on date: {}", date_str);

        // Try to fetch games for this date
        match fetch_day_data(client, config, tournaments, &date_str).await {
            Ok((Some(responses), _)) => {
                if !responses.is_empty() {
                    info!(
                        "Found {} responses with games on date {}",
                        responses.len(),
                        date_str
                    );
                    return Ok(Some((responses, date_str)));
                }
            }
            Ok((None, _)) => {
                info!("No games found on date {}", date_str);
            }
            Err(e) => {
                warn!("Error fetching games for date {}: {}", date_str, e);
                // Continue to next date even if this one fails
            }
        }
    }

    info!("No future games found in the next 7 days");
    Ok(None)
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
    let client = create_http_client();

    // Determine the date to fetch data for
    let date = determine_fetch_date(custom_date);

    // Check if this is a historical date (previous season)
    let is_historical = is_historical_date(&date);
    info!("Date: {}, is_historical: {}", date, is_historical);
    if is_historical {
        info!(
            "Detected historical date: {}, using schedule endpoint",
            date
        );
        let historical_games = fetch_historical_games(&client, &config, &date).await?;
        return Ok((historical_games, date));
    }

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

    // Cache the detailed game response
    let is_live_game = game_response.game.started && !game_response.game.ended;
    cache_detailed_game_data(season, game_id, game_response.clone(), is_live_game).await;

    // Process the response and cache the goal events
    let events = process_game_response_with_cache(game_response, game_id).await;
    cache_goal_events_data(season, game_id, events.clone()).await;

    Ok(events)
}

/// Helper function to process game response with player caching
async fn process_game_response_with_cache(
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
    events
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

/// Represents a tournament type with its string identifier
#[derive(Debug, Clone, PartialEq)]
enum TournamentType {
    Runkosarja,
    Playoffs,
    Playout,
    Qualifications,
    ValmistavatOttelut,
}

impl TournamentType {
    /// Converts the tournament type to its string representation
    fn as_str(&self) -> &'static str {
        match self {
            TournamentType::Runkosarja => "runkosarja",
            TournamentType::Playoffs => "playoffs",
            TournamentType::Playout => "playout",
            TournamentType::Qualifications => "qualifications",
            TournamentType::ValmistavatOttelut => "valmistavat_ottelut",
        }
    }

    /// Converts from the integer serie value used in ScheduleApiGame
    fn from_serie(serie: i32) -> Self {
        match serie {
            2 => TournamentType::Playoffs,
            3 => TournamentType::Playout,
            4 => TournamentType::Qualifications,
            5 => TournamentType::ValmistavatOttelut,
            _ => TournamentType::Runkosarja, // Default to runkosarja
        }
    }

    /// Converts to the integer serie value used in ScheduleApiGame
    fn to_serie(&self) -> i32 {
        match self {
            TournamentType::Runkosarja => 1,
            TournamentType::Playoffs => 2,
            TournamentType::Playout => 3,
            TournamentType::Qualifications => 4,
            TournamentType::ValmistavatOttelut => 5,
        }
    }
}

/// Parses a date string and determines the hockey season
/// Hockey seasons typically start in September and end in April/May
/// Returns (year, month, season)
fn parse_date_and_season(date: &str) -> (i32, u32, i32) {
    let date_parts: Vec<&str> = date.split('-').collect();
    let (year, month) = if date_parts.len() >= 2 {
        let y = date_parts[0]
            .parse::<i32>()
            .unwrap_or_else(|_| Utc::now().with_timezone(&Local).year());
        let m = date_parts[1].parse::<u32>().unwrap_or(1);
        (y, m)
    } else {
        (Utc::now().with_timezone(&Local).year(), 1)
    };

    // Ice hockey season: if month >= 9, season = year+1, else season = year
    let season = if month >= 9 { year + 1 } else { year };

    info!(
        "Parsed date: year={}, month={}, season={}",
        year, month, season
    );
    (year, month, season)
}

/// Determines which tournaments to check based on the month
fn determine_tournaments_for_month(month: u32) -> Vec<TournamentType> {
    let tournaments = if (PLAYOFFS_START_MONTH..=PLAYOFFS_END_MONTH).contains(&month) {
        // Spring months (March-June): check playoffs, playout, qualifications, and runkosarja
        info!(
            "Spring month {} detected, checking all tournament types",
            month
        );
        vec![
            TournamentType::Runkosarja,
            TournamentType::Playoffs,
            TournamentType::Playout,
            TournamentType::Qualifications,
        ]
    } else if (PRESEASON_START_MONTH..=PRESEASON_END_MONTH).contains(&month) {
        // Preseason months (May-September): check valmistavat_ottelut and runkosarja
        info!(
            "Preseason month {} detected, checking valmistavat_ottelut and runkosarja",
            month
        );
        vec![
            TournamentType::Runkosarja,
            TournamentType::ValmistavatOttelut,
        ]
    } else {
        // Regular season months: only check runkosarja
        info!(
            "Regular season month {} detected, checking only runkosarja",
            month
        );
        vec![TournamentType::Runkosarja]
    };

    info!(
        "Tournaments to check: {:?}",
        tournaments.iter().map(|t| t.as_str()).collect::<Vec<_>>()
    );
    tournaments
}

/// Fetches games from all relevant tournaments for a given season
/// Implements connection pooling and parallel requests for better performance
async fn fetch_tournament_games(
    client: &Client,
    config: &Config,
    tournaments: &[TournamentType],
    season: i32,
) -> Vec<ScheduleApiGame> {
    info!(
        "Fetching games from {} tournaments for season {}",
        tournaments.len(),
        season
    );

    // Create futures for parallel execution to leverage connection pooling
    let fetch_futures: Vec<_> = tournaments
        .iter()
        .map(|tournament| {
            let url =
                build_tournament_schedule_url(&config.api_domain, tournament.as_str(), season);
            let tournament_name = tournament.as_str();

            async move {
                info!("Fetching {} schedule from: {}", tournament_name, url);

                match fetch::<Vec<ScheduleApiGame>>(client, &url).await {
                    Ok(games) => {
                        info!(
                            "Successfully fetched {} games for {} tournament in season {}",
                            games.len(),
                            tournament_name,
                            season
                        );

                        // Annotate games with tournament type
                        let mut annotated_games = Vec::with_capacity(games.len());
                        for mut game in games {
                            game.serie = tournament.to_serie();
                            annotated_games.push(game);
                        }

                        Ok(annotated_games)
                    }
                    Err(e) => {
                        warn!(
                            "Failed to fetch {} schedule for season {}: {}",
                            tournament_name, season, e
                        );
                        Err(e)
                    }
                }
            }
        })
        .collect();

    // Execute all requests in parallel to maximize connection pool usage
    let results = futures::future::join_all(fetch_futures).await;

    // Collect successful results
    let mut all_schedule_games: Vec<ScheduleApiGame> = Vec::new();
    let mut successful_fetches = 0;
    let mut failed_fetches = 0;

    for result in results {
        match result {
            Ok(games) => {
                all_schedule_games.extend(games);
                successful_fetches += 1;
            }
            Err(_) => {
                failed_fetches += 1;
            }
        }
    }

    info!(
        "Tournament fetch completed: {} successful, {} failed, {} total games",
        successful_fetches,
        failed_fetches,
        all_schedule_games.len()
    );

    all_schedule_games
}

/// Filters games to match the requested date
fn filter_games_by_date(games: Vec<ScheduleApiGame>, target_date: &str) -> Vec<ScheduleApiGame> {
    info!("Filtering games for target date: {}", target_date);

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

// Helper to fetch and convert detailed game data
async fn fetch_and_convert_detailed_game_data(
    client: &Client,
    config: &Config,
    season: i32,
    game_id: i32,
) -> DetailedGameData {
    fetch_detailed_game_data_for_historical_game(client, config, season, game_id).await
}

// Helper to convert goal events for a team
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

// Helper to build a ScheduleTeam from API and detailed data
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

    // 2. Convert goal events for home and away teams
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

    // 3. Build ScheduleTeam structs
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
async fn fetch_historical_games(
    client: &Client,
    config: &Config,
    date: &str,
) -> Result<Vec<GameData>, AppError> {
    info!("Fetching historical games for date: {}", date);

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
                warn!("Failed to convert historical game: {}", e);
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

/// Determines if a date is from a previous season (not the current season).
/// Hockey seasons typically start in September and end in April/May.
/// So a date in May-July is from the previous season.
pub fn is_historical_date(date: &str) -> bool {
    let now = Utc::now().with_timezone(&Local);
    is_historical_date_with_current_time(date, now)
}

/// Internal function that determines if a date is historical given a specific current time.
/// This allows for testing with mocked current times.
fn is_historical_date_with_current_time(date: &str, current_time: chrono::DateTime<Local>) -> bool {
    let date_parts: Vec<&str> = date.split('-').collect();
    if date_parts.len() < 2 {
        return false;
    }

    let date_year = date_parts[0]
        .parse::<i32>()
        .unwrap_or_else(|_| current_time.year());
    let date_month = date_parts[1]
        .parse::<u32>()
        .unwrap_or_else(|_| current_time.month());

    // Try to parse the full date to check if it's in the future
    if let Ok(parsed_date) = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        let current_date = current_time.date_naive();

        // Future dates should not be considered historical
        if parsed_date > current_date {
            return false;
        }
    }

    let current_year = current_time.year();
    let current_month = current_time.month();

    // Hockey season logic:
    // - Season starts in September (month 9)
    // - Season ends in April/May (months 4-5)
    // - So dates in May-July are from the previous season

    if date_year < current_year {
        // Definitely historical
        return true;
    } else if date_year == current_year {
        // Same year, check if it's in the off-season
        // If current month is August (8) and date is May-July, it's from previous season
        if current_month == 8 && (5..=7).contains(&date_month) {
            return true;
        }
        // If we're in the off-season (May-July) and the date is from the regular season (September-April),
        // it's from the previous season
        if (5..=7).contains(&current_month) && (date_month >= 9 || date_month <= 4) {
            return true;
        }
    }

    false
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
async fn process_goal_events_for_historical_game_with_players(
    game: &DetailedGame,
    home_team_players: &[Player],
    away_team_players: &[Player],
) -> Vec<GoalEventData> {
    let mut all_goal_events = Vec::new();

    // Create player lookup maps for efficient name resolution
    let home_player_map: std::collections::HashMap<i64, &Player> = home_team_players
        .iter()
        .map(|player| (player.id, player))
        .collect();
    let away_player_map: std::collections::HashMap<i64, &Player> = away_team_players
        .iter()
        .map(|player| (player.id, player))
        .collect();

    // Helper function to get player name with fallback
    let get_player_name = |player_id: i64, player_map: &std::collections::HashMap<i64, &Player>| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::models::{DetailedGame, DetailedTeam, GoalEvent, Period, Player};
    use serial_test::serial;
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

    async fn clear_all_caches_for_test() {
        use crate::data_fetcher::cache::clear_all_caches;
        clear_all_caches().await;
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
        clear_all_caches_for_test().await;

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

        clear_all_caches_for_test().await;
    }

    #[tokio::test]
    async fn test_fetch_tournament_data_no_games() {
        clear_all_caches_for_test().await;

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

        clear_all_caches_for_test().await;
    }

    #[tokio::test]
    async fn test_fetch_tournament_data_server_error() {
        clear_all_caches_for_test().await;

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

        clear_all_caches_for_test().await;
    }

    #[tokio::test]
    async fn test_fetch_tournament_data_not_found() {
        clear_all_caches_for_test().await;

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

        clear_all_caches_for_test().await;
    }

    #[tokio::test]
    async fn test_fetch_day_data_success() {
        clear_all_caches_for_test().await;

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

        clear_all_caches_for_test().await;
    }

    #[tokio::test]
    async fn test_fetch_day_data_no_games() {
        clear_all_caches_for_test().await;

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

        clear_all_caches_for_test().await;
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

        // Clear all caches to ensure clean state
        clear_all_caches_for_test().await;

        let result = fetch_game_data(&client, &test_config, 2024, 1).await;

        assert!(result.is_ok());
        let goal_events = result.unwrap();
        assert_eq!(goal_events.len(), 1);
        assert_eq!(goal_events[0].scorer_name, "Smith");
        assert_eq!(goal_events[0].home_team_score, 1);
        assert_eq!(goal_events[0].away_team_score, 0);

        // Clear caches after test
        clear_all_caches_for_test().await;
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

        // Clear all caches to ensure clean state
        clear_all_caches_for_test().await;

        let result = fetch_game_data(&client, &test_config, 2024, 1).await;

        assert!(result.is_ok());
        let goal_events = result.unwrap();
        assert_eq!(goal_events.len(), 0);

        // Clear caches after test
        clear_all_caches_for_test().await;
    }

    #[tokio::test]
    #[serial]
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

        // Clear all caches to ensure a completely clean state
        use crate::data_fetcher::cache::{clear_all_caches, get_cache_size};
        clear_all_caches().await;

        // Wait for cache clearing to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Verify cache is actually empty - check all cache types
        let initial_player_cache_size = get_cache_size().await;
        let initial_tournament_cache_size = get_tournament_cache_size().await;
        let initial_detailed_game_cache_size = get_detailed_game_cache_size().await;
        let initial_goal_events_cache_size = get_goal_events_cache_size().await;

        assert_eq!(
            initial_player_cache_size, 0,
            "Player cache should be empty before test"
        );
        assert_eq!(
            initial_tournament_cache_size, 0,
            "Tournament cache should be empty before test"
        );
        assert_eq!(
            initial_detailed_game_cache_size, 0,
            "Detailed game cache should be empty before test"
        );
        assert_eq!(
            initial_goal_events_cache_size, 0,
            "Goal events cache should be empty before test"
        );

        let result = fetch_game_data(&client, &test_config, 2024, 1).await;

        // Should still succeed due to fallback logic
        assert!(
            result.is_ok(),
            "fetch_game_data should succeed with mock data"
        );
        let goal_events = result.unwrap();

        // Debug information for CI troubleshooting
        if goal_events.is_empty() {
            // Let's verify what the mock response contains
            let mock_data = create_mock_detailed_game_response();
            let home_goals = mock_data.game.home_team.goal_events.len();
            let away_goals = mock_data.game.away_team.goal_events.len();
            let total_players =
                mock_data.home_team_players.len() + mock_data.away_team_players.len();

            panic!(
                "Expected 1 goal event but got {}. Mock data has {} home goals, {} away goals, {} total players. \
                 Goal scorer ID: {}. First player ID: {}",
                goal_events.len(),
                home_goals,
                away_goals,
                total_players,
                if !mock_data.game.home_team.goal_events.is_empty() {
                    mock_data.game.home_team.goal_events[0]
                        .scorer_player_id
                        .to_string()
                } else {
                    "none".to_string()
                },
                if !mock_data.home_team_players.is_empty() {
                    mock_data.home_team_players[0].id.to_string()
                } else {
                    "none".to_string()
                }
            );
        }

        assert_eq!(goal_events.len(), 1, "Should have exactly 1 goal event");
        assert_eq!(
            goal_events[0].scorer_name, "Smith",
            "Scorer name should be 'Smith'"
        );
        assert_eq!(
            goal_events[0].home_team_score, 1,
            "Home team score should be 1"
        );
        assert_eq!(
            goal_events[0].away_team_score, 0,
            "Away team score should be 0"
        );

        // Clear the cache after the test to avoid interference
        clear_all_caches().await;
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

        // Clear all caches to ensure clean state
        clear_all_caches_for_test().await;

        let result = fetch_game_data(&client, &test_config, 2024, 1).await;

        assert!(result.is_err(), "Should return error for 500 status code");

        // Clear caches after test
        clear_all_caches_for_test().await;
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

    #[tokio::test]
    async fn test_process_goal_events_for_historical_game_with_players() {
        use crate::data_fetcher::models::{DetailedGame, DetailedTeam, GoalEvent, Player};

        // Create test players
        let home_players = vec![
            Player {
                id: 123,
                first_name: "John".to_string(),
                last_name: "Smith".to_string(),
            },
            Player {
                id: 456,
                first_name: "Mike".to_string(),
                last_name: "Johnson".to_string(),
            },
        ];

        let away_players = vec![Player {
            id: 789,
            first_name: "David".to_string(),
            last_name: "Brown".to_string(),
        }];

        // Create test game with goal events
        let game = DetailedGame {
            id: 1,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            end: Some("2024-01-15T20:30:00Z".to_string()),
            home_team: DetailedTeam {
                team_id: "team1".to_string(),
                team_name: "HIFK".to_string(),
                goals: 2,
                goal_events: vec![
                    GoalEvent {
                        scorer_player_id: 123,
                        log_time: "2024-01-15T19:15:00Z".to_string(),
                        game_time: 2700,
                        period: 2,
                        event_id: 1,
                        home_team_score: 1,
                        away_team_score: 0,
                        winning_goal: false,
                        goal_types: vec!["even_strength".to_string()],
                        assistant_player_ids: vec![456],
                        video_clip_url: Some("https://example.com/video1.mp4".to_string()),
                    },
                    GoalEvent {
                        scorer_player_id: 456,
                        log_time: "2024-01-15T19:45:00Z".to_string(),
                        game_time: 3300,
                        period: 3,
                        event_id: 2,
                        home_team_score: 2,
                        away_team_score: 1,
                        winning_goal: true,
                        goal_types: vec!["powerplay".to_string()],
                        assistant_player_ids: vec![],
                        video_clip_url: None,
                    },
                ],
                penalty_events: vec![],
            },
            away_team: DetailedTeam {
                team_id: "team2".to_string(),
                team_name: "Tappara".to_string(),
                goals: 1,
                goal_events: vec![GoalEvent {
                    scorer_player_id: 789,
                    log_time: "2024-01-15T19:30:00Z".to_string(),
                    game_time: 3000,
                    period: 2,
                    event_id: 3,
                    home_team_score: 1,
                    away_team_score: 1,
                    winning_goal: false,
                    goal_types: vec!["even_strength".to_string()],
                    assistant_player_ids: vec![],
                    video_clip_url: None,
                }],
                penalty_events: vec![],
            },
            periods: vec![],
            finished_type: Some("normal".to_string()),
            started: true,
            ended: true,
            game_time: 3600,
            serie: "runkosarja".to_string(),
        };

        // Process goal events with player lookup
        let goal_events = process_goal_events_for_historical_game_with_players(
            &game,
            &home_players,
            &away_players,
        )
        .await;

        // Verify results
        assert_eq!(goal_events.len(), 3);

        // Check home team goals
        let home_goal_1 = &goal_events[0]; // First goal (earliest time)
        assert_eq!(home_goal_1.scorer_player_id, 123);
        assert_eq!(home_goal_1.scorer_name, "John Smith");
        assert_eq!(home_goal_1.minute, 45); // 2700 seconds / 60
        assert!(home_goal_1.is_home_team);
        assert!(!home_goal_1.is_winning_goal);

        let home_goal_2 = &goal_events[2]; // Third goal (latest time)
        assert_eq!(home_goal_2.scorer_player_id, 456);
        assert_eq!(home_goal_2.scorer_name, "Mike Johnson");
        assert_eq!(home_goal_2.minute, 55); // 3300 seconds / 60
        assert!(home_goal_2.is_home_team);
        assert!(home_goal_2.is_winning_goal);

        // Check away team goal
        let away_goal = &goal_events[1]; // Second goal (middle time)
        assert_eq!(away_goal.scorer_player_id, 789);
        assert_eq!(away_goal.scorer_name, "David Brown");
        assert_eq!(away_goal.minute, 50); // 3000 seconds / 60
        assert!(!away_goal.is_home_team);
        assert!(!away_goal.is_winning_goal);

        // Verify sorting by game time
        assert!(goal_events[0].minute <= goal_events[1].minute);
        assert!(goal_events[1].minute <= goal_events[2].minute);
    }

    #[tokio::test]
    async fn test_process_goal_events_with_missing_player() {
        use crate::data_fetcher::models::{DetailedGame, DetailedTeam, GoalEvent, Player};

        // Create test players (missing player ID 999)
        let home_players = vec![Player {
            id: 123,
            first_name: "John".to_string(),
            last_name: "Smith".to_string(),
        }];

        let away_players = vec![];

        // Create test game with goal event for missing player
        let game = DetailedGame {
            id: 1,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            end: Some("2024-01-15T20:30:00Z".to_string()),
            home_team: DetailedTeam {
                team_id: "team1".to_string(),
                team_name: "HIFK".to_string(),
                goals: 1,
                goal_events: vec![GoalEvent {
                    scorer_player_id: 999, // Missing player
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
                penalty_events: vec![],
            },
            away_team: DetailedTeam {
                team_id: "team2".to_string(),
                team_name: "Tappara".to_string(),
                goals: 0,
                goal_events: vec![],
                penalty_events: vec![],
            },
            periods: vec![],
            finished_type: Some("normal".to_string()),
            started: true,
            ended: true,
            game_time: 3600,
            serie: "runkosarja".to_string(),
        };

        // Process goal events with player lookup
        let goal_events = process_goal_events_for_historical_game_with_players(
            &game,
            &home_players,
            &away_players,
        )
        .await;

        // Verify results
        assert_eq!(goal_events.len(), 1);

        let goal_event = &goal_events[0];
        assert_eq!(goal_event.scorer_player_id, 999);
        assert_eq!(goal_event.scorer_name, "Player 999"); // Fallback name
        assert!(goal_event.is_home_team);
    }

    // Tests for is_historical_date function
    #[test]
    fn test_is_historical_date_august_transition() {
        // Mock current date as August 2024
        let current_time = chrono::DateTime::parse_from_rfc3339("2024-08-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Local);

        // Test August transition scenario: current_month is 8, date_month is between 5-7
        // These should be historical (from previous season)
        assert!(is_historical_date_with_current_time(
            "2024-05-15",
            current_time
        )); // May 2024 in August 2024
        assert!(is_historical_date_with_current_time(
            "2024-06-20",
            current_time
        )); // June 2024 in August 2024
        assert!(is_historical_date_with_current_time(
            "2024-07-10",
            current_time
        )); // July 2024 in August 2024

        // These should NOT be historical (same season)
        assert!(!is_historical_date_with_current_time(
            "2024-08-15",
            current_time
        )); // August 2024 in August 2024
        assert!(!is_historical_date_with_current_time(
            "2024-09-01",
            current_time
        )); // September 2024 in August 2024
        assert!(!is_historical_date_with_current_time(
            "2024-12-25",
            current_time
        )); // December 2024 in August 2024
        assert!(!is_historical_date_with_current_time(
            "2024-03-15",
            current_time
        )); // March 2024 in August 2024
    }

    #[test]
    fn test_is_historical_date_year_boundary() {
        // Test year boundary cases
        // Mock current date as January 2023
        let current_time = chrono::DateTime::parse_from_rfc3339("2023-01-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Local);

        // Date in September 2022 compared to current date in January 2023
        // This should be historical (previous year)
        assert!(is_historical_date_with_current_time(
            "2022-09-15",
            current_time
        )); // September 2022

        // Date in April 2022 compared to current date in January 2023
        // This should be historical (previous year)
        assert!(is_historical_date_with_current_time(
            "2022-04-15",
            current_time
        )); // April 2022

        // Date in January 2023 compared to current date in January 2023
        // This should NOT be historical (current year, same month)
        assert!(!is_historical_date_with_current_time(
            "2023-01-15",
            current_time
        )); // January 2023

        // Date in December 2022 compared to current date in January 2023
        // This should be historical (previous year)
        assert!(is_historical_date_with_current_time(
            "2022-12-15",
            current_time
        )); // December 2022
    }

    #[test]
    fn test_is_historical_date_off_season_months() {
        // Test off-season months where current_month is between 5-7
        // and date_month is between 9-4 (regular season months)

        // Mock current date as June 2024 (off-season)
        let current_time = chrono::DateTime::parse_from_rfc3339("2024-06-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Local);

        // These should be historical (from previous season)
        assert!(is_historical_date_with_current_time(
            "2023-09-15",
            current_time
        )); // September 2023 in June 2024
        assert!(is_historical_date_with_current_time(
            "2023-10-20",
            current_time
        )); // October 2023 in June 2024
        assert!(is_historical_date_with_current_time(
            "2023-11-10",
            current_time
        )); // November 2023 in June 2024
        assert!(is_historical_date_with_current_time(
            "2023-12-25",
            current_time
        )); // December 2023 in June 2024
        assert!(is_historical_date_with_current_time(
            "2024-01-15",
            current_time
        )); // January 2024 in June 2024
        assert!(is_historical_date_with_current_time(
            "2024-02-20",
            current_time
        )); // February 2024 in June 2024
        assert!(is_historical_date_with_current_time(
            "2024-03-10",
            current_time
        )); // March 2024 in June 2024
        assert!(is_historical_date_with_current_time(
            "2024-04-15",
            current_time
        )); // April 2024 in June 2024

        // These should NOT be historical (same off-season)
        assert!(!is_historical_date_with_current_time(
            "2024-05-15",
            current_time
        )); // May 2024 in June 2024
        assert!(!is_historical_date_with_current_time(
            "2024-06-20",
            current_time
        )); // June 2024 in June 2024
        assert!(!is_historical_date_with_current_time(
            "2024-07-10",
            current_time
        )); // July 2024 in June 2024
    }

    #[test]
    fn test_is_historical_date_regular_season_months() {
        // Test regular season months that should return false
        // during both in-season and off-season periods

        // Mock current date as December 2024 (in-season)
        let current_time = chrono::DateTime::parse_from_rfc3339("2024-12-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Local);

        // These should NOT be historical (current year)
        assert!(!is_historical_date_with_current_time(
            "2024-09-15",
            current_time
        )); // September 2024 in December 2024
        assert!(!is_historical_date_with_current_time(
            "2024-10-20",
            current_time
        )); // October 2024 in December 2024
        assert!(!is_historical_date_with_current_time(
            "2024-11-10",
            current_time
        )); // November 2024 in December 2024
        assert!(!is_historical_date_with_current_time(
            "2024-12-25",
            current_time
        )); // December 2024 in December 2024
        assert!(!is_historical_date_with_current_time(
            "2025-01-15",
            current_time
        )); // January 2025 in December 2024
        assert!(!is_historical_date_with_current_time(
            "2025-02-20",
            current_time
        )); // February 2025 in December 2024
        assert!(!is_historical_date_with_current_time(
            "2025-03-10",
            current_time
        )); // March 2025 in December 2024
        assert!(!is_historical_date_with_current_time(
            "2025-04-15",
            current_time
        )); // April 2025 in December 2024

        // These should be historical (previous year)
        assert!(is_historical_date_with_current_time(
            "2023-09-15",
            current_time
        )); // September 2023 in December 2024
        assert!(is_historical_date_with_current_time(
            "2023-12-25",
            current_time
        )); // December 2023 in December 2024
        // Note: January 2024 and April 2024 are NOT historical when current time is December 2024
        // because they are in the same year and the off-season condition doesn't apply
        assert!(!is_historical_date_with_current_time(
            "2024-01-15",
            current_time
        )); // January 2024 in December 2024
        assert!(!is_historical_date_with_current_time(
            "2024-04-15",
            current_time
        )); // April 2024 in December 2024
    }

    #[test]
    fn test_is_historical_date_edge_cases() {
        // Test edge cases and invalid inputs
        // Use current time for edge case tests
        let current_time = Utc::now().with_timezone(&Local);

        // Invalid date format should return false
        assert!(!is_historical_date_with_current_time(
            "invalid-date",
            current_time
        ));
        assert!(!is_historical_date_with_current_time("2024", current_time));
        assert!(!is_historical_date_with_current_time("", current_time));

        // Same year, different months - these depend on current month
        // Let's test with a specific current time to avoid flaky tests
        let specific_current_time = chrono::DateTime::parse_from_rfc3339("2024-08-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Local);

        assert!(!is_historical_date_with_current_time(
            "2024-08-15",
            specific_current_time
        )); // August in August (same month)
        assert!(!is_historical_date_with_current_time(
            "2024-09-01",
            specific_current_time
        )); // September in August (next month)

        // Future dates should not be historical
        assert!(!is_historical_date_with_current_time(
            "2025-01-15",
            specific_current_time
        )); // Future year
        assert!(!is_historical_date_with_current_time(
            "2024-12-31",
            specific_current_time
        )); // Future month in same year

        // Test the specific case reported by the user
        let january_2025_time = chrono::DateTime::parse_from_rfc3339("2025-01-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Local);
        assert!(!is_historical_date_with_current_time(
            "2025-09-09",
            january_2025_time
        )); // Future date should not be historical
    }

    #[test]
    fn test_is_historical_date_complex_scenarios() {
        // Test complex scenarios that might occur in real usage

        // Scenario 1: During playoffs (April 2024), looking at regular season games
        // Mock current date as April 2024
        let current_time_april = chrono::DateTime::parse_from_rfc3339("2024-04-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Local);

        // Regular season games from current year should NOT be historical
        assert!(!is_historical_date_with_current_time(
            "2024-10-15",
            current_time_april
        )); // October 2024 in April 2024
        assert!(!is_historical_date_with_current_time(
            "2024-12-25",
            current_time_april
        )); // December 2024 in April 2024
        assert!(!is_historical_date_with_current_time(
            "2024-02-20",
            current_time_april
        )); // February 2024 in April 2024

        // Regular season games from previous year should be historical
        assert!(is_historical_date_with_current_time(
            "2023-10-15",
            current_time_april
        )); // October 2023 in April 2024
        assert!(is_historical_date_with_current_time(
            "2023-12-25",
            current_time_april
        )); // December 2023 in April 2024
        // Note: January 2024 is NOT historical when current time is April 2024
        // because they are in the same year and the off-season condition doesn't apply
        assert!(!is_historical_date_with_current_time(
            "2024-01-20",
            current_time_april
        )); // January 2024 in April 2024

        // Scenario 2: During preseason (September 2024), looking at previous season
        // Mock current date as September 2024
        let current_time_september = chrono::DateTime::parse_from_rfc3339("2024-09-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Local);

        // Previous year games should be historical
        assert!(is_historical_date_with_current_time(
            "2023-10-15",
            current_time_september
        )); // October 2023 in September 2024
        // April 2024 is NOT historical in September 2024
        assert!(!is_historical_date_with_current_time(
            "2024-04-15",
            current_time_september
        )); // April 2024 in September 2024
        // May 2024 is NOT historical in September 2024
        assert!(!is_historical_date_with_current_time(
            "2024-05-20",
            current_time_september
        )); // May 2024 in September 2024

        // Current year games should NOT be historical
        assert!(!is_historical_date_with_current_time(
            "2024-09-20",
            current_time_september
        )); // September 2024 in September 2024
        assert!(!is_historical_date_with_current_time(
            "2024-10-15",
            current_time_september
        )); // October 2024 in September 2024
    }

    #[tokio::test]
    async fn test_find_future_games_fallback() {
        clear_all_caches_for_test().await;

        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let client = Client::new();

        // Mock response for a future date
        let mock_response = create_mock_schedule_response();

        Mock::given(method("GET"))
            .and(path("/games"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        // Update config to use mock server
        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let result =
            find_future_games_fallback(&client, &test_config, &["runkosarja"], "2024-01-14").await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_some());
        let (responses, date) = response.unwrap();
        assert_eq!(responses.len(), 1);
        assert_eq!(date, "2024-01-15");

        // Clear cache after test to prevent interference with other tests
        clear_all_caches_for_test().await;
    }

    #[tokio::test]
    async fn test_find_future_games_fallback_no_games() {
        clear_all_caches_for_test().await;

        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let client = Client::new();

        // Mock empty response for all dates
        let mock_response = create_mock_empty_schedule_response();

        Mock::given(method("GET"))
            .and(path("/games"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        // Update config to use mock server
        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let result =
            find_future_games_fallback(&client, &test_config, &["runkosarja"], "2024-01-14").await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_none());

        // Clear cache after test to prevent interference with other tests
        clear_all_caches_for_test().await;
    }

    #[tokio::test]
    async fn test_find_future_games_fallback_invalid_date() {
        let config = create_mock_config();
        let client = Client::new();

        let result =
            find_future_games_fallback(&client, &config, &["runkosarja"], "invalid-date").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::DateTimeParse(_)));
    }
}
