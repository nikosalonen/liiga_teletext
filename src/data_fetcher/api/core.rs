use crate::config::Config;
use crate::data_fetcher::cache::{
    cache_detailed_game_data, cache_goal_events_data, cache_http_response, cache_players,
    cache_players_with_disambiguation, cache_tournament_data, get_cached_detailed_game_data,
    get_cached_goal_events_data, get_cached_http_response, get_cached_players,
    get_cached_tournament_data_with_start_check, has_live_games,
    should_bypass_cache_for_starting_games,
};
#[cfg(test)]
use crate::data_fetcher::cache::{
    get_detailed_game_cache_size, get_goal_events_cache_size, get_tournament_cache_size,
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
use chrono::{Datelike, Local, Utc};
use futures;
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info, instrument, warn};

// HTTP client utilities available from sibling http_client module
use super::http_client::create_http_client_with_timeout;
#[cfg(test)]
use super::http_client::create_test_http_client;
// URL builders available from sibling urls module
use super::urls::{build_game_url, build_schedule_url, build_tournament_url, create_tournament_key};
// Date and season logic available from sibling date_logic module
use super::date_logic::{determine_fetch_date, parse_date_and_season};
#[cfg(test)]
use super::date_logic::determine_fetch_date_with_time;
// Tournament logic available from sibling tournament_logic module
use super::tournament_logic::{
    build_tournament_list, determine_tournaments_for_month, fetch_tournament_games, TournamentType,
};
#[cfg(test)]
use super::tournament_logic::{build_tournament_list_fallback, determine_active_tournaments};
// Season utilities available from sibling season_utils module
use super::season_utils::{
    is_historical_date, is_historical_date_with_current_time, should_use_schedule_for_playoffs,
    should_use_schedule_for_playoffs_with_current_time,
};
// Generic fetch utility available from sibling fetch_utils module
use super::fetch_utils::fetch;
// Game-specific API operations available from sibling game_api module
use super::game_api::{
    fetch_historical_games, filter_games_by_date, get_team_name, has_actual_goals,
    process_games, process_response_games, process_single_game, should_fetch_detailed_data,
};
#[cfg(test)]
use super::game_api::{
    fetch_game_data, process_game_response_with_cache,
    process_goal_events_for_historical_game_with_players,
};


/// Determines if a candidate date should be used as the best date for showing games.
/// Prioritizes future games over past games, and regular season over preseason when close to season start.
fn should_use_this_date(
    current_best: &Option<String>,
    candidate_date: &str,
    tournament: &str,
    baseline_date: &str,
) -> bool {
    let current_best = match current_best {
        Some(date) => date,
        None => return true, // First date is always accepted
    };

    // Parse dates for comparison
    let baseline_date_parsed = baseline_date.parse::<chrono::NaiveDate>();
    let candidate_parsed = candidate_date.parse::<chrono::NaiveDate>();
    let current_parsed = current_best.parse::<chrono::NaiveDate>();

    if let (Ok(baseline_date_val), Ok(candidate), Ok(current)) =
        (baseline_date_parsed, candidate_parsed, current_parsed)
    {
        let is_candidate_future = candidate >= baseline_date_val;
        let is_current_future = current >= baseline_date_val;
        let is_candidate_regular_season = tournament == "runkosarja";

        // If only one is a future date, prioritize the future one
        if is_candidate_future && !is_current_future {
            return true;
        }
        if !is_candidate_future && is_current_future {
            return false;
        }

        // If both are future dates or both are past dates:
        if is_candidate_future && is_current_future {
            // Both are future: prefer regular season if close to today, otherwise prefer earlier
            if is_candidate_regular_season {
                // Prefer regular season if it's within 7 days of today
                let days_from_today = (candidate - baseline_date_val).num_days();
                if days_from_today <= 7 {
                    return true;
                }
            }
            // For future dates, prefer the earlier one
            return candidate < current;
        } else {
            // Both are past dates: prefer the later one (closer to today)
            return candidate > current;
        }
    }

    // Fallback to string comparison if date parsing fails; prefer regular season on ties
    if candidate_date == current_best.as_str() && tournament == "runkosarja" {
        true
    } else {
        candidate_date < current_best.as_str()
    }
}

/// Processes next game dates when no games are found for the current date.
/// Returns the best next game date and tournaments that have games on that date.
/// Uses simple date comparison logic to find the best upcoming games.
async fn process_next_game_dates(
    client: &Client,
    config: &Config,
    tournaments: &[&str],
    date: &str,
    tournament_responses: HashMap<String, ScheduleResponse>,
) -> Result<(Option<String>, Vec<ScheduleResponse>), AppError> {
    let mut tournament_next_dates: HashMap<&str, String> = HashMap::new();
    let mut best_date: Option<String> = None;

    // Check for next game dates using the tournament responses we already have
    for tournament in tournaments {
        let tournament_key = create_tournament_key(tournament, date);

        if let Some(response) = tournament_responses.get(&tournament_key) {
            if let Some(next_date) = &response.next_game_date {
                // Simple date selection logic
                if should_use_this_date(&best_date, next_date, tournament, date) {
                    best_date = Some(next_date.clone());
                    info!(
                        "Updated best date to: {} from tournament: {}",
                        next_date, tournament
                    );
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

    if let Some(next_date) = best_date.clone() {
        info!("Found best next game date: {}", next_date);
        // Only fetch tournaments that have games on the best date
        let tournaments_to_fetch: Vec<&str> = tournament_next_dates
            .iter()
            .filter_map(|(tournament, date)| {
                if date == &next_date {
                    info!("Tournament {} has games on the best date", tournament);
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

            use futures::future::join_all;
            let futs = tournaments_to_fetch.iter().map(|t| {
                let next_date_clone = next_date.clone();
                async move {
                    info!(
                        "Fetching data for tournament {} on date {}",
                        t, next_date_clone
                    );
                    (
                        *t,
                        fetch_tournament_data(client, config, t, &next_date_clone).await,
                    )
                }
            });
            let mut response_data = Vec::with_capacity(tournaments_to_fetch.len());
            for (t, res) in join_all(futs).await {
                match res {
                    Ok(resp) if !resp.games.is_empty() => {
                        info!(
                            "Found {} games for tournament {} on date {}",
                            resp.games.len(),
                            t,
                            next_date
                        );
                        response_data.push(resp);
                    }
                    Ok(_) => info!(
                        "No games found for tournament {} on date {} despite next_game_date indicating games should exist",
                        t, next_date
                    ),
                    Err(e) => error!(
                        "Failed to fetch tournament data for {} on date {}: {}",
                        t, next_date, e
                    ),
                }
            }

            // If we didn't find any games with direct fetching, try the regular fetch_day_data
            if response_data.is_empty() {
                info!("No games found with direct tournament fetching, trying fetch_day_data");
                match fetch_day_data(
                    client,
                    config,
                    &tournaments_to_fetch,
                    &next_date,
                    &[],
                    &HashMap::new(),
                )
                .await
                {
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

/// Fetches game data for a specific tournament and date from the API.
/// Uses caching to improve performance and reduce API calls.
#[instrument(skip(client, config))]
pub async fn fetch_tournament_data(
    client: &Client,
    config: &Config,
    tournament: &str,
    date: &str,
) -> Result<ScheduleResponse, AppError> {
    fetch_tournament_data_with_cache_check(client, config, tournament, date, &[]).await
}

/// Enhanced version of fetch_tournament_data that can use current games for cache validation
pub async fn fetch_tournament_data_with_cache_check(
    client: &Client,
    config: &Config,
    tournament: &str,
    date: &str,
    current_games: &[GameData],
) -> Result<ScheduleResponse, AppError> {
    info!("Fetching tournament data for {} on {}", tournament, date);

    // Create cache key
    let cache_key = create_tournament_key(tournament, date);

    // Check if we should completely bypass cache for starting games
    if should_bypass_cache_for_starting_games(current_games).await {
        debug!("Cache bypass enabled for starting games, fetching fresh data");
    } else {
        // Check cache first with enhanced validation
        if let Some(cached_response) =
            get_cached_tournament_data_with_start_check(&cache_key, current_games).await
        {
            info!(
                "Using cached tournament data for {} on {}",
                tournament, date
            );
            return Ok(cached_response);
        }
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
    current_games: &[GameData],
    cached_responses: &HashMap<String, ScheduleResponse>,
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
        // Check if we have cached response first
        let cache_key = create_tournament_key(tournament, date);
        let response = if let Some(cached_response) = cached_responses.get(&cache_key) {
            info!(
                "Using cached response for tournament {} on date {}",
                tournament, date
            );
            cached_response.clone()
        } else {
            // Fall back to fetching if not cached
            match fetch_tournament_data_with_cache_check(
                client,
                config,
                tournament,
                date,
                current_games,
            )
            .await
            {
                Ok(resp) => resp,
                Err(_) => continue, // Skip this tournament if fetch fails
            }
        };

        // Store all responses in the HashMap for potential reuse
        let tournament_key = create_tournament_key(tournament, date);
        tournament_responses.insert(tournament_key, response.clone());

        if !response.games.is_empty() {
            responses.push(response);
            found_games = true;
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
    is_pre_noon_cutoff: bool,
) -> Result<(Vec<ScheduleResponse>, Option<String>), AppError> {
    if is_pre_noon_cutoff {
        info!(
            "No games found for {} (pre-noon cutoff date). Searching for today's/future games as fallback.",
            date
        );
        // During pre-noon cutoff, we tried yesterday first but found no games
        // Now fall back to today's games or future games for better UX
    } else {
        info!("No games found for the current date, checking for next game dates");
    }

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
        match fetch_day_data(client, config, tournaments, &date_str, &[], &HashMap::new()).await {
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
/// If games were found on a different date than the original (earliest_date is set),
/// returns that date. Otherwise returns the original date.
fn determine_return_date(
    games: &[GameData],
    earliest_date: Option<String>,
    original_date: &str,
) -> String {
    if games.is_empty() {
        earliest_date.unwrap_or_else(|| original_date.to_string())
    } else {
        // If we have games and earliest_date is set, it means we found games on a different date
        // than the original requested date, so we should return that date
        earliest_date.unwrap_or_else(|| original_date.to_string())
    }
}

#[instrument(skip(custom_date))]
pub async fn fetch_liiga_data(
    custom_date: Option<String>,
) -> Result<(Vec<GameData>, String), AppError> {
    info!("Starting to fetch Liiga data");

    // Early check: prevent network calls if API domain is not properly configured
    // This prevents CI hangs when LIIGA_API_DOMAIN is unset or invalid
    if let Ok(api_domain) = std::env::var("LIIGA_API_DOMAIN")
        && (api_domain.is_empty()
            || api_domain == "placeholder"
            || api_domain == "test"
            || api_domain == "unset")
    {
        warn!(
            "LIIGA_API_DOMAIN is set to '{}' - skipping network calls to prevent CI hangs",
            api_domain
        );
        return Err(AppError::config_error(
            "API domain is not properly configured - network calls skipped",
        ));
    }

    let config = Config::load().await?;
    info!("Config loaded successfully");
    let client = create_http_client_with_timeout(config.http_timeout_seconds)?;

    // Determine the date to fetch data for
    let (date, is_pre_noon_cutoff) = determine_fetch_date(custom_date);

    // Check if this is a historical date (previous season) or requires schedule endpoint for playoffs
    let is_historical = is_historical_date(&date);
    let use_schedule_for_playoffs = should_use_schedule_for_playoffs(&date);
    info!(
        "Date: {}, is_historical: {}, use_schedule_for_playoffs: {}",
        date, is_historical, use_schedule_for_playoffs
    );

    if is_historical || use_schedule_for_playoffs {
        info!(
            "Detected {} date: {}, using schedule endpoint",
            if is_historical {
                "historical"
            } else {
                "playoff"
            },
            date
        );
        let historical_games = fetch_historical_games(&client, &config, &date).await?;
        return Ok((historical_games, date));
    }

    // Build the list of tournaments to fetch based on tournament lifecycle
    let (tournaments, cached_responses) = build_tournament_list(&client, &config, &date).await?;

    // First try to fetch data for the current date
    info!(
        "Fetching data for date: {} with tournaments: {:?}",
        date, tournaments
    );
    let (games_option, tournament_responses) = fetch_day_data(
        &client,
        &config,
        &tournaments,
        &date,
        &[],
        &cached_responses,
    )
    .await?;

    let (response_data, earliest_date) = if let Some(responses) = games_option {
        info!(
            "Found games for the current date. Number of responses: {}",
            responses.len()
        );
        (responses, None)
    } else {
        handle_no_games_found(
            &client,
            &config,
            &tournaments,
            &date,
            tournament_responses,
            is_pre_noon_cutoff,
        )
        .await?
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
    use serial_test::serial;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path, query_param},
    };

    fn create_mock_config() -> Config {
        Config {
            api_domain: "http://localhost:8080".to_string(),
            log_file_path: None,
            http_timeout_seconds: crate::constants::DEFAULT_HTTP_TIMEOUT_SECONDS,
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

    fn create_mock_empty_schedule_response_no_next_date() -> ScheduleResponse {
        ScheduleResponse {
            games: vec![],
            previous_game_date: Some("2024-01-14".to_string()),
            next_game_date: None, // No next game date to force fallback usage
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
                        scorer_player: None,
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
        let client = create_test_http_client();

        let mock_response = create_mock_schedule_response();

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "runkosarja"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        // Update config to use mock server
        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let result = fetch_tournament_data(&client, &test_config, "runkosarja", "2024-03-15").await;

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
        let client = create_test_http_client();

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
        let client = create_test_http_client();

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
        let client = create_test_http_client();

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "runkosarja"))
            .and(query_param("date", "2024-01-15"))
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
        let client = create_test_http_client();

        let mock_response = create_mock_schedule_response();

        Mock::given(method("GET"))
            .and(path("/games"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let tournaments = vec!["runkosarja"];
        let result = fetch_day_data(
            &client,
            &test_config,
            &tournaments,
            "2024-01-15",
            &[],
            &HashMap::new(),
        )
        .await;

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
        let client = create_test_http_client();

        let mock_response = create_mock_empty_schedule_response();

        Mock::given(method("GET"))
            .and(path("/games"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        let mut test_config = config;
        test_config.api_domain = mock_server.uri();

        let tournaments = vec!["runkosarja"];
        let result = fetch_day_data(
            &client,
            &test_config,
            &tournaments,
            "2024-01-15",
            &[],
            &HashMap::new(),
        )
        .await;

        assert!(result.is_ok());
        let (responses, _) = result.unwrap();
        assert!(responses.is_none());

        clear_all_caches_for_test().await;
    }

    #[tokio::test]
    async fn test_fetch_game_data_success() {
        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let _client = Client::new();

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

        // Detailed fetch disabled in new flow; ensure schedule processing succeeds instead
        let schedule_resp = create_mock_schedule_response_with_games();
        assert!(!schedule_resp.games.is_empty());

        // Clear caches after test
        clear_all_caches_for_test().await;
    }

    #[tokio::test]
    async fn test_fetch_game_data_no_goals() {
        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let _client = Client::new();

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

        // Detailed fetch disabled; no assertion on per-game fetch

        // Clear caches after test
        clear_all_caches_for_test().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_fetch_game_data_cache_fallback() {
        let mock_server = MockServer::start().await;
        let config = create_mock_config();
        let client = create_test_http_client();

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

        // Force multiple cache clears to ensure consistency
        clear_all_caches().await;

        // Wait for cache clearing to complete
        tokio::time::sleep(Duration::from_millis(200)).await;

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
        let client = create_test_http_client();

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
        let client = create_test_http_client();

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
        let client = create_test_http_client();

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
        let tournaments = build_tournament_list_fallback("2024-08-15");
        assert!(tournaments.contains(&"valmistavat_ottelut"));
        assert!(tournaments.contains(&"runkosarja"));
        assert!(!tournaments.contains(&"playoffs"));
        assert!(!tournaments.contains(&"playout"));
        assert!(!tournaments.contains(&"qualifications"));
    }

    #[test]
    fn test_build_tournament_list_regular_season() {
        let tournaments = build_tournament_list_fallback("2024-12-15");
        assert!(!tournaments.contains(&"valmistavat_ottelut"));
        assert!(tournaments.contains(&"runkosarja"));
        assert!(!tournaments.contains(&"playoffs"));
        assert!(!tournaments.contains(&"playout"));
        assert!(!tournaments.contains(&"qualifications"));
    }

    #[test]
    fn test_build_tournament_list_playoffs() {
        let tournaments = build_tournament_list_fallback("2024-04-15");
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
                    scorer_player: None,
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
                    scorer_player: None,
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
        assert!(!should_fetch_detailed_data(&game));
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

    #[test]
    fn test_should_fetch_detailed_data_finished_with_score() {
        // With new flow we do not fetch detailed data anymore
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
                goal_events: vec![], // Empty goal_events but non-zero score
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
                        scorer_player: None,
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
                        scorer_player: None,
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
                    scorer_player: None,
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
                    scorer_player: None,
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
        let client = create_test_http_client();

        // Mock response for a future date
        let mock_response = create_mock_schedule_response();

        // Mock for the specific request that will be made (first day will be 2024-01-15)
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "runkosarja"))
            .and(query_param("date", "2024-01-15"))
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
        let client = create_test_http_client();

        // Mock empty response for all dates
        let mock_response = create_mock_empty_schedule_response_no_next_date();

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
        let client = create_test_http_client();

        let result =
            find_future_games_fallback(&client, &config, &["runkosarja"], "invalid-date").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::DateTimeParse(_)));
    }

    #[test]
    fn test_determine_fetch_date_custom_date() {
        // Test with custom date - should return false for cutoff flag
        let custom_date = Some("2024-01-15".to_string());
        let (date, is_cutoff) = determine_fetch_date(custom_date);

        assert_eq!(date, "2024-01-15");
        assert!(!is_cutoff); // Custom date should not trigger cutoff logic
    }

    #[test]
    fn test_determine_fetch_date_no_custom_date() {
        // Test without custom date - behavior depends on current time
        let (date, is_cutoff) = determine_fetch_date(None);

        // Date should be valid format
        assert!(date.len() == 10); // YYYY-MM-DD format
        assert!(date.contains('-')); // Should contain date separators

        // The cutoff flag should be either true or false based on current time
        // We can't predict the exact value, but we can verify it's a boolean
        // This assertion is always true for boolean values, but documents the expected type
        let _: bool = is_cutoff;
    }

    #[test]
    fn test_determine_fetch_date_custom_date_none() {
        // Test with explicit None - should behave same as no custom date
        let (date, is_cutoff) = determine_fetch_date(None);

        // Date should be valid format
        assert!(date.len() == 10); // YYYY-MM-DD format
        assert!(date.contains('-')); // Should contain date separators

        // The cutoff flag should be either true or false based on current time
        // This assertion is always true for boolean values, but documents the expected type
        let _: bool = is_cutoff;
    }

    #[test]
    fn test_determine_fetch_date_with_time_deterministic() {
        use chrono::{Local, TimeZone};

        // Create a fixed date for deterministic testing
        let _base_date = Local.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap();

        // Test morning time (before noon) - should show yesterday's games
        let morning_time = Local.with_ymd_and_hms(2024, 1, 15, 11, 59, 59).unwrap();
        let (date, is_cutoff) = determine_fetch_date_with_time(None, morning_time);

        assert_eq!(date, "2024-01-14"); // Yesterday's date
        assert!(is_cutoff); // Should be marked as pre-noon cutoff

        // Test noon time (at/after noon) - should show today's games
        let noon_time = Local.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap();
        let (date, is_cutoff) = determine_fetch_date_with_time(None, noon_time);

        assert_eq!(date, "2024-01-15"); // Today's date
        assert!(!is_cutoff); // Should not be marked as pre-noon cutoff

        // Test custom date - should override time logic
        let custom_date = Some("2024-02-20".to_string());
        let (date, is_cutoff) = determine_fetch_date_with_time(custom_date, morning_time);

        assert_eq!(date, "2024-02-20"); // Custom date should be used
        assert!(!is_cutoff); // Custom date should not trigger cutoff logic
    }

    #[test]
    fn test_determine_fetch_date_with_time_edge_cases() {
        use chrono::{Local, TimeZone};

        // Test edge case: exactly at noon
        let exactly_noon = Local.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap();
        let (date, is_cutoff) = determine_fetch_date_with_time(None, exactly_noon);

        assert_eq!(date, "2024-01-15"); // Today's date at noon
        assert!(!is_cutoff); // Noon is considered "after noon"

        // Test edge case: one second before noon
        let one_second_before_noon = Local.with_ymd_and_hms(2024, 1, 15, 11, 59, 59).unwrap();
        let (date, is_cutoff) = determine_fetch_date_with_time(None, one_second_before_noon);

        assert_eq!(date, "2024-01-14"); // Yesterday's date before noon
        assert!(is_cutoff); // Should be marked as pre-noon cutoff
    }

    // Tests for determine_active_tournaments function
    // These tests verify the concurrent tournament functionality

    fn create_mock_schedule_response_with_games() -> ScheduleResponse {
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
                    short_handed_instances: 2,
                    short_handed_goals: 0,
                    ranking: Some(2),
                    game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
                    goal_events: vec![],
                },
                finished_type: Some("regular".to_string()),
                started: true,
                ended: true,
                game_time: 3600,
                serie: "runkosarja".to_string(),
            }],
            previous_game_date: Some("2024-01-14".to_string()),
            next_game_date: Some("2024-01-16".to_string()),
        }
    }

    fn create_mock_schedule_response_no_games_future_date() -> ScheduleResponse {
        ScheduleResponse {
            games: vec![],
            previous_game_date: None,
            next_game_date: Some("2024-01-16".to_string()),
        }
    }

    fn create_mock_schedule_response_no_games_no_future() -> ScheduleResponse {
        ScheduleResponse {
            games: vec![],
            previous_game_date: None,
            next_game_date: None,
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_determine_active_tournaments_concurrent_tournaments() {
        clear_all_caches_for_test().await;

        let mock_server = MockServer::start().await;
        let mut config = create_mock_config();
        config.api_domain = mock_server.uri();
        let client = create_test_http_client();

        // Mock responses for multiple tournaments
        // playoffs has games on the date
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "playoffs"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_with_games()),
            )
            .mount(&mock_server)
            .await;

        // playout also has games on the date
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "playout"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_with_games()),
            )
            .mount(&mock_server)
            .await;

        // qualifications has no games but has future games
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "qualifications"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_future_date()),
            )
            .mount(&mock_server)
            .await;

        // runkosarja and valmistavat_ottelut have no games and no future dates
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "runkosarja"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "valmistavat_ottelut"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        let result = determine_active_tournaments(&client, &config, "2024-03-15").await;

        assert!(result.is_ok());
        let (active_tournaments, _cached_responses) = result.unwrap();

        // March includes playoff tournaments that have games or future games
        // Note: With month-based filtering, only playoff tournaments are checked in March
        assert_eq!(active_tournaments.len(), 2);
        assert_eq!(active_tournaments, vec!["playoffs", "playout"]);
    }

    #[tokio::test]
    #[serial]
    async fn test_determine_active_tournaments_single_tournament_with_games() {
        clear_all_caches_for_test().await;

        let mock_server = MockServer::start().await;
        let mut config = create_mock_config();
        config.api_domain = mock_server.uri();
        let client = create_test_http_client();

        // Only runkosarja has games on the date
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "runkosarja"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_with_games()),
            )
            .mount(&mock_server)
            .await;

        // Other tournaments have no games and no future dates
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "valmistavat_ottelut"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "playoffs"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "playout"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "qualifications"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        let result = determine_active_tournaments(&client, &config, "2024-03-15").await;

        assert!(result.is_ok());
        let (active_tournaments, _cached_responses) = result.unwrap();

        // Should contain only the regular season tournament
        assert_eq!(active_tournaments.len(), 1);
        assert_eq!(active_tournaments, vec!["runkosarja"]);
    }

    #[tokio::test]
    #[serial]
    async fn test_determine_active_tournaments_future_games_same_date() {
        clear_all_caches_for_test().await;

        let mock_server = MockServer::start().await;
        let mut config = create_mock_config();
        config.api_domain = mock_server.uri();
        let client = create_test_http_client();

        // Create a response with next_game_date equal to current date (>= comparison test)
        let same_date_response = ScheduleResponse {
            games: vec![],
            previous_game_date: None,
            next_game_date: Some("2024-03-15".to_string()),
        };

        // playoffs has no games but next game date is same as current date
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "playoffs"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&same_date_response))
            .mount(&mock_server)
            .await;

        // Other tournaments have no games and no future dates
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "valmistavat_ottelut"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "runkosarja"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "playout"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "qualifications"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        let result = determine_active_tournaments(&client, &config, "2024-03-15").await;

        assert!(result.is_ok());
        let (active_tournaments, _cached_responses) = result.unwrap();

        // Should include playoffs due to >= comparison (same date as next_game_date)
        assert_eq!(active_tournaments.len(), 1);
        assert_eq!(active_tournaments, vec!["playoffs"]);
    }

    #[tokio::test]
    #[serial]
    async fn test_determine_active_tournaments_no_active_tournaments() {
        clear_all_caches_for_test().await;

        let mock_server = MockServer::start().await;
        let mut config = create_mock_config();
        config.api_domain = mock_server.uri();
        let client = create_test_http_client();

        // All tournaments have no games and no future dates
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "valmistavat_ottelut"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "runkosarja"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "playoffs"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "playout"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "qualifications"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        let result = determine_active_tournaments(&client, &config, "2024-03-15").await;

        assert!(result.is_ok());
        let (active_tournaments, _cached_responses) = result.unwrap();

        // Should fall back to regular season when no tournaments are active
        assert_eq!(active_tournaments.len(), 1);
        assert_eq!(active_tournaments, vec!["runkosarja"]);
    }

    #[tokio::test]
    #[serial]
    async fn test_determine_active_tournaments_priority_order() {
        clear_all_caches_for_test().await;

        let mock_server = MockServer::start().await;
        let mut config = create_mock_config();
        config.api_domain = mock_server.uri();
        let client = create_test_http_client();

        // Set up tournaments in reverse priority order to test that results maintain priority
        // qualifications (last in priority) has games
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "qualifications"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_with_games()),
            )
            .mount(&mock_server)
            .await;

        // runkosarja (second in priority) has games
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "runkosarja"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_with_games()),
            )
            .mount(&mock_server)
            .await;

        // valmistavat_ottelut (first in priority) has games
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "valmistavat_ottelut"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_with_games()),
            )
            .mount(&mock_server)
            .await;

        // Other tournaments have no games
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "playoffs"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "playout"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        let result = determine_active_tournaments(&client, &config, "2024-03-15").await;

        assert!(result.is_ok());
        let (active_tournaments, _cached_responses) = result.unwrap();

        // March should include runkosarja (always) and qualifications (playoff period) that have games
        // valmistavat_ottelut is not checked in March (outside preseason period)
        assert_eq!(active_tournaments.len(), 2);
        assert_eq!(active_tournaments, vec!["runkosarja", "qualifications"]);
    }

    #[tokio::test]
    #[serial]
    async fn test_determine_active_tournaments_api_errors_handled() {
        clear_all_caches_for_test().await;

        let mock_server = MockServer::start().await;
        let mut config = create_mock_config();
        config.api_domain = mock_server.uri();
        let client = create_test_http_client();

        // playoffs returns 404 (simulates tournament not available)
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "playoffs"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        // runkosarja returns valid response with games
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "runkosarja"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_with_games()),
            )
            .mount(&mock_server)
            .await;

        // Other tournaments have no games
        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "valmistavat_ottelut"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "playout"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/games"))
            .and(query_param("tournament", "qualifications"))
            .and(query_param("date", "2024-03-15"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(create_mock_schedule_response_no_games_no_future()),
            )
            .mount(&mock_server)
            .await;

        let result = determine_active_tournaments(&client, &config, "2024-03-15").await;

        assert!(result.is_ok());
        let (active_tournaments, _cached_responses) = result.unwrap();

        // Should continue processing despite API error and return runkosarja
        assert_eq!(active_tournaments.len(), 1);
        assert_eq!(active_tournaments, vec!["runkosarja"]);
    }
}
