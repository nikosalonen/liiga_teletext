use crate::config::Config;
use crate::data_fetcher::cache::{cache_players, get_cached_players};
use crate::data_fetcher::models::{
    DetailedGameResponse, GameData, GoalEventData, ScheduleGame, ScheduleResponse,
};
use crate::data_fetcher::processors::{
    create_basic_goal_events, determine_game_status, format_time, process_goal_events,
    should_show_todays_games,
};
use crate::error::AppError;
use chrono::Local;
use reqwest::Client;
use std::collections::HashMap;

/// Fetches game data for a specific tournament and date from the API.
///
/// # Arguments
/// * `client` - HTTP client for making API requests
/// * `config` - Application configuration containing API domain
/// * `tournament` - Tournament identifier
/// * `date` - Date string in YYYY-MM-DD format
///
/// # Returns
/// * `Ok(Option<ScheduleResponse>)` - Successfully fetched schedule data, None if no data found
/// * `Err(AppError)` - Error occurred during fetch with detailed error message
///
/// # Notes
/// - Handles API connection errors with detailed error messages
/// - Validates response status code
/// - Includes config path in error messages for troubleshooting
/// - Returns structured error messages for common failure cases
pub async fn fetch_tournament_data(
    client: &Client,
    config: &Config,
    tournament: &str,
    date: &str,
) -> Result<Option<ScheduleResponse>, AppError> {
    let url = format!(
        "{}/games?tournament={}&date={}",
        config.api_domain, tournament, date
    );

    let response = client.get(&url).send().await?;

    // Check status code first
    if !response.status().is_success() {
        return Err(AppError::Custom(format!(
            "Failed to fetch data from API: {}\nAPI domain: {}\nConfig: {}",
            response
                .status()
                .canonical_reason()
                .unwrap_or("Unknown error"),
            config.api_domain,
            Config::get_config_path()
        )));
    }

    let response_text = response.text().await?;
    let schedule_response = serde_json::from_str::<ScheduleResponse>(&response_text)?;
    Ok(Some(schedule_response))
}

async fn fetch_previous_day_data(
    client: &Client,
    config: &Config,
    tournaments: &[&str],
    previous_dates: &[String],
) -> Result<Option<Vec<ScheduleResponse>>, AppError> {
    if previous_dates.is_empty() {
        return Ok(None);
    }

    // Sort dates in descending order to get the most recent one
    let mut sorted_dates = previous_dates.to_vec();
    sorted_dates.sort_by(|a, b| b.cmp(a));
    let nearest_date = &sorted_dates[0];

    let mut responses = Vec::new();
    let mut found_games = false;

    for tournament in tournaments {
        if let Ok(Some(response)) =
            fetch_tournament_data(client, config, tournament, nearest_date).await
        {
            if !response.games.is_empty() {
                responses.push(response);
                found_games = true;
            }
        }
    }

    if found_games {
        Ok(Some(responses))
    } else {
        Ok(None)
    }
}

pub async fn fetch_liiga_data(custom_date: Option<String>) -> Result<Vec<GameData>, AppError> {
    let config = Config::load()?;
    let client = Client::new();
    let date = if let Some(date) = custom_date {
        date
    } else {
        let now = Local::now();
        if should_show_todays_games() {
            now.format("%Y-%m-%d").to_string()
        } else {
            // If before 15:00, try to get previous day's games first
            let yesterday = now
                .date_naive()
                .pred_opt()
                .expect("Date underflow cannot happen with Local::now()");
            yesterday.format("%Y-%m-%d").to_string()
        }
    };
    let tournaments = ["runkosarja", "playoffs", "playout", "qualifications"];
    let mut all_games = Vec::new();
    let mut response_data: Vec<ScheduleResponse> = Vec::new();
    let mut found_games = false;
    let mut previous_dates = Vec::new();

    // Try to get games for the selected date first
    for tournament in &tournaments {
        match fetch_tournament_data(&client, &config, tournament, &date).await {
            Ok(Some(response)) => {
                if !response.games.is_empty() {
                    response_data.push(response);
                    found_games = true;
                } else {
                    // Store previous game date if it exists
                    if !response.previous_game_date.is_empty() {
                        previous_dates.push(response.previous_game_date.clone());
                    }
                }
            }
            Err(e) => return Err(e),
            Ok(None) => continue,
        }
    }

    // If no games found in any tournament today, try the nearest previous game date
    if !found_games {
        if let Ok(Some(prev_day_response)) =
            fetch_previous_day_data(&client, &config, &tournaments, &previous_dates).await
        {
            response_data = prev_day_response;
        }
    }

    // Process games if we found any
    if !response_data.is_empty() {
        for response in &response_data {
            let games = futures::future::try_join_all(response.games.clone().into_iter().map(
                |m| {
                    let client = client.clone();
                    let config = config.clone();
                    async move {
                        let time = if !m.started {
                            format_time(&m.start).unwrap_or_default()
                        } else {
                            String::new()
                        };

                        let result = format!("{}-{}", m.home_team.goals, m.away_team.goals);
                        let (score_type, is_overtime, is_shootout) = determine_game_status(&m);

                        let has_goals = m
                            .home_team
                            .goal_events
                            .iter()
                            .any(|g| !g.goal_types.contains(&"RL0".to_string()))
                            || m.away_team
                                .goal_events
                                .iter()
                                .any(|g| !g.goal_types.contains(&"RL0".to_string()));

                        let goal_events = if !m.started {
                            Vec::new()
                        } else if has_goals || !m.ended {
                            // Fetch detailed data if there are goals or game is ongoing
                            fetch_detailed_game_data(&client, &config, &m).await
                        } else {
                            Vec::new()
                        };

                        Ok::<GameData, AppError>(GameData {
                            home_team: m.home_team.team_name,
                            away_team: m.away_team.team_name,
                            time,
                            result,
                            score_type,
                            is_overtime,
                            is_shootout,
                            serie: m.serie,
                            goal_events,
                            played_time: m.game_time,
                        })
                    }
                },
            ))
            .await?;
            all_games.extend(games);
        }
    }

    Ok(all_games)
}

async fn fetch_detailed_game_data(
    client: &Client,
    config: &Config,
    game: &ScheduleGame,
) -> Vec<GoalEventData> {
    match fetch_game_data(client, config, game.season, game.id).await {
        Ok(detailed_data) => detailed_data,
        Err(e) => {
            eprintln!(
                "Failed to fetch detailed game data: {}. Using basic game data.",
                e
            );
            create_basic_goal_events(game)
        }
    }
}

async fn fetch_game_data(
    client: &Client,
    config: &Config,
    season: i32,
    game_id: i32,
) -> Result<Vec<GoalEventData>, AppError> {
    let url = format!("{}/games/{}/{}", config.api_domain, season, game_id);
    let response = client.get(&url).send().await?;
    let response_text = response.text().await?;
    let game_response = serde_json::from_str::<DetailedGameResponse>(&response_text)?;

    // Check cache first
    if let Some(cached_players) = get_cached_players(game_id).await {
        return Ok(process_goal_events(&game_response.game, &cached_players));
    }

    // Build player names map if not in cache
    let mut player_names = HashMap::new();
    for player in &game_response.home_team_players {
        player_names.insert(
            player.id,
            format!("{} {}", player.first_name, player.last_name),
        );
    }
    for player in &game_response.away_team_players {
        player_names.insert(
            player.id,
            format!("{} {}", player.first_name, player.last_name),
        );
    }

    // Update cache
    cache_players(game_id, player_names.clone()).await;

    Ok(process_goal_events(&game_response.game, &player_names))
}
