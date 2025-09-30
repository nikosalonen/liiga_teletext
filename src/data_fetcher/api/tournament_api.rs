// Tournament-specific API operations
// This module contains functions for fetching tournament data, handling date selection,
// and managing fallback mechanisms when games are not found

use crate::config::Config;
use crate::data_fetcher::cache::{
    cache_tournament_data, get_cached_tournament_data_with_start_check,
    should_bypass_cache_for_starting_games,
};
use crate::data_fetcher::models::{GameData, ScheduleResponse};
use crate::error::AppError;
use reqwest::Client;
use std::collections::HashMap;
use tracing::{debug, error, info, instrument, warn};

// Import from sibling modules
use super::fetch_utils::fetch;
use super::urls::{build_tournament_url, create_tournament_key};

/// Determines if a candidate date should be used as the best date for showing games.
/// Prioritizes future games over past games, and regular season over preseason when close to season start.
pub(super) fn should_use_this_date(
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

/// Determines the appropriate date to return based on whether games were found.
/// If games were found on a different date than the original (earliest_date is set),
/// returns that date. Otherwise returns the original date.
pub(super) fn determine_return_date(
    _games: &[GameData],
    earliest_date: Option<String>,
    original_date: &str,
) -> String {
    earliest_date.unwrap_or_else(|| original_date.to_string())
}

/// Fetches game data for a specific tournament and date from the API.
/// Uses caching to improve performance and reduce API calls.
#[instrument(skip(client, config))]
pub(super) async fn fetch_tournament_data(
    client: &Client,
    config: &Config,
    tournament: &str,
    date: &str,
) -> Result<ScheduleResponse, AppError> {
    fetch_tournament_data_with_cache_check(client, config, tournament, date, &[]).await
}

/// Enhanced version of fetch_tournament_data that can use current games for cache validation
pub(super) async fn fetch_tournament_data_with_cache_check(
    client: &Client,
    config: &Config,
    tournament: &str,
    date: &str,
    current_games: &[GameData],
) -> Result<ScheduleResponse, AppError> {
    info!("Fetching tournament data for {tournament} on {date}");

    // Create cache key
    let cache_key = create_tournament_key(tournament, date);

    // Check if we should completely bypass cache for starting games
    if should_bypass_cache_for_starting_games(current_games) {
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

/// Fetches game data for multiple tournaments on a specific date.
/// Returns responses for tournaments that have games and a map of all tournament responses.
#[allow(clippy::type_complexity)]
pub(super) async fn fetch_day_data(
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

/// Processes next game dates when no games are found for the current date.
/// Returns the best next game date and tournaments that have games on that date.
/// Uses simple date comparison logic to find the best upcoming games.
pub(super) async fn process_next_game_dates(
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
                info!("Tournament {tournament} has no next game date");
            }
        } else {
            info!("No response found for tournament key: {tournament_key}");
        }
    }

    if let Some(next_date) = best_date.clone() {
        info!("Found best next game date: {next_date}");
        // Only fetch tournaments that have games on the best date
        let tournaments_to_fetch: Vec<&str> = tournament_next_dates
            .iter()
            .filter_map(|(tournament, date)| {
                if date == &next_date {
                    info!("Tournament {tournament} has games on the best date");
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

/// Fallback mechanism to find future games by checking upcoming dates.
/// This is used when the API doesn't provide next_game_date information.
pub(super) async fn find_future_games_fallback(
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
            error!("Failed to parse date '{current_date}': {e}");
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
        info!("Checking for games on date: {date_str}");

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
                info!("No games found on date {date_str}");
            }
            Err(e) => {
                warn!("Error fetching games for date {date_str}: {e}");
                // Continue to next date even if this one fails
            }
        }
    }

    info!("No future games found in the next 7 days");
    Ok(None)
}

/// Handles the case when no games are found for the current date.
/// Returns the response data and earliest date for next games.
pub(super) async fn handle_no_games_found(
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
