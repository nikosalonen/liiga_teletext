use crate::config::Config;
use crate::data_fetcher::cache::{cache_players, get_cached_players};
use crate::data_fetcher::models::{
    DetailedGameResponse, GameData, GoalEventData, ScheduleApiGame, ScheduleGame, ScheduleResponse,
    ScheduleTeam,
};
use crate::data_fetcher::processors::{
    create_basic_goal_events, determine_game_status, format_time, process_goal_events,
    should_show_todays_games,
};
use crate::error::AppError;
use chrono::{Datelike, Local};
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use tracing::{debug, error, info, instrument};

// Tournament season constants for month-based logic
const PRESEASON_START_MONTH: u32 = 5; // May
const PRESEASON_END_MONTH: u32 = 9; // September
const PLAYOFFS_START_MONTH: u32 = 3; // March
const PLAYOFFS_END_MONTH: u32 = 6; // June

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
fn determine_fetch_date(custom_date: Option<String>) -> String {
    custom_date.unwrap_or_else(|| {
        let now = Local::now();
        if should_show_todays_games() {
            let date_str = now.format("%Y-%m-%d").to_string();
            info!("Using today's date: {}", date_str);
            date_str
        } else {
            let yesterday = now
                .date_naive()
                .pred_opt()
                .expect("Date underflow cannot happen with Local::now()");
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
        Local::now().month()
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
        let tournament_key = format!("{}-{}", tournament, date);

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

/// Processes game data from API responses and converts them to GameData objects.
/// Handles team names, game times, results, and detailed goal events.
async fn process_games(
    client: &Client,
    config: &Config,
    response_data: Vec<ScheduleResponse>,
) -> Result<Vec<GameData>, AppError> {
    let mut all_games = Vec::new();

    if !response_data.is_empty() {
        info!(
            "Processing {} response(s) with game data",
            response_data.len()
        );
        for (i, response) in response_data.iter().enumerate() {
            // Check if the games array is actually empty
            if response.games.is_empty() {
                info!("Response #{} has empty games array", i + 1);
                continue;
            }

            info!(
                "Processing response #{} with {} games",
                i + 1,
                response.games.len()
            );
            let games =
                futures::future::try_join_all(response.games.clone().into_iter().enumerate().map(
                    |(game_idx, m)| {
                        let client = client.clone();
                        let config = config.clone();
                        let response_idx = i;
                        async move {
                            let home_team_name = get_team_name(&m.home_team);
                            let away_team_name = get_team_name(&m.away_team);
                            info!(
                                "Processing game #{} in response #{}: {} vs {}",
                                game_idx + 1,
                                response_idx + 1,
                                home_team_name,
                                away_team_name
                            );

                            let time = if !m.started {
                                let formatted_time = format_time(&m.start).unwrap_or_default();
                                info!("Game not started, formatted time: {}", formatted_time);
                                formatted_time
                            } else {
                                info!("Game already started, no time to display");
                                String::new()
                            };

                            let result = format!("{}-{}", m.home_team.goals, m.away_team.goals);
                            info!("Game result: {}", result);

                            let (score_type, is_overtime, is_shootout) = determine_game_status(&m);
                            info!(
                                "Game status: {:?}, overtime: {}, shootout: {}",
                                score_type, is_overtime, is_shootout
                            );

                            let has_goals = m
                                .home_team
                                .goal_events
                                .iter()
                                .any(|g| !g.goal_types.contains(&"RL0".to_string()))
                                || m.away_team
                                    .goal_events
                                    .iter()
                                    .any(|g| !g.goal_types.contains(&"RL0".to_string()));

                            info!("Game has goals: {}", has_goals);

                            let goal_events = if !m.started {
                                info!("Game not started, no goal events to fetch");
                                Vec::new()
                            } else if has_goals || !m.ended {
                                // Fetch detailed data if there are goals or game is ongoing
                                info!(
                                    "Fetching detailed game data (has_goals: {}, ended: {})",
                                    has_goals, m.ended
                                );
                                let events = fetch_detailed_game_data(&client, &config, &m).await;
                                info!("Fetched {} goal events", events.len());
                                events
                            } else {
                                info!("Game ended with no goals, no need to fetch detailed data");
                                Vec::new()
                            };

                            info!(
                                "Successfully processed game #{} in response #{}",
                                game_idx + 1,
                                response_idx + 1
                            );

                            info!("Game serie from API: '{}'", m.serie);
                            Ok::<GameData, AppError>(GameData {
                                home_team: home_team_name.to_string(),
                                away_team: away_team_name.to_string(),
                                time,
                                result,
                                score_type,
                                is_overtime,
                                is_shootout,
                                serie: m.serie,
                                goal_events,
                                played_time: m.game_time,
                                start: m.start.clone(),
                            })
                        }
                    },
                ))
                .await?;

            info!(
                "Successfully processed all games in response #{}, adding {} games to result",
                i + 1,
                games.len()
            );
            all_games.extend(games);
        }
        info!("Total games processed: {}", all_games.len());
    } else {
        info!("No response data to process");
    }

    Ok(all_games)
}

#[instrument(skip(client))]
async fn fetch<T: DeserializeOwned>(client: &Client, url: &str) -> Result<T, AppError> {
    info!("Fetching data from URL: {}", url);
    let response = client.get(url).send().await?;
    let status = response.status();
    let headers = response.headers().clone();

    info!("Response status: {}", status);
    debug!("Response headers: {:?}", headers);

    if !status.is_success() {
        let error_message = format!(
            "Failed to fetch data from API: {} (URL: {})",
            status.canonical_reason().unwrap_or("Unknown error"),
            url
        );
        error!("{}", error_message);
        return Err(AppError::Custom(error_message));
    }

    let response_text = response.text().await?;
    info!("Response length: {} bytes", response_text.len());
    debug!("Response text: {}", response_text);

    match serde_json::from_str::<T>(&response_text) {
        Ok(parsed) => Ok(parsed),
        Err(e) => {
            error!("Failed to parse API response: {} (URL: {})", e, url);
            error!(
                "Response text (first 200 chars): {}",
                &response_text.chars().take(200).collect::<String>()
            );
            Err(AppError::ApiParse(e))
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
    info!("Fetching tournament data");
    let url = format!(
        "{}/games?tournament={}&date={}",
        config.api_domain, tournament, date
    );
    fetch(client, &url).await
}

#[instrument(skip(client, config))]
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
            let tournament_key = format!("{}-{}", tournament, date);
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
        info!("No games found for the current date, checking for next game dates");
        // Process next game dates when no games are found for the current date
        let (next_date, next_responses) =
            process_next_game_dates(&client, &config, &tournaments, &date, tournament_responses)
                .await?;
        (next_responses, next_date)
    };

    // Process games if we found any
    let all_games = process_games(&client, &config, response_data).await?;

    // Return results with appropriate date
    if all_games.is_empty() {
        info!("No games found after processing all data");
        if let Some(next_date) = earliest_date {
            info!("Returning empty games list with next date: {}", next_date);
            Ok((all_games, next_date))
        } else {
            info!("Returning empty games list with original date: {}", date);
            Ok((all_games, date))
        }
    } else {
        info!("Returning {} games with date: {}", all_games.len(), date);
        Ok((all_games, date))
    }
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
    let url = format!("{}/games/{}/{}", config.api_domain, season, game_id);

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
            return Err(e);
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
            format!("{} {}", player.first_name, player.last_name),
        );
    }

    info!(
        "Processing {} away team players",
        game_response.away_team_players.len()
    );
    for player in &game_response.away_team_players {
        player_names.insert(
            player.id,
            format!("{} {}", player.first_name, player.last_name),
        );
    }
    info!("Built player names map with {} players", player_names.len());

    // Update cache
    info!("Updating player cache for game ID: {}", game_id);
    cache_players(game_id, player_names.clone()).await;

    let events = process_goal_events(&game_response.game, &player_names);
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
    let url = format!(
        "{}/schedule?tournament=runkosarja&season={}",
        config.api_domain, season
    );

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
            Err(e)
        }
    }
}
