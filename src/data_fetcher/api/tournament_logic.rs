//! Tournament selection and fetching logic

use crate::config::Config;
use crate::data_fetcher::models::{ScheduleApiGame, ScheduleResponse};
use crate::error::AppError;
use chrono::{Datelike, Utc};
use futures;
use reqwest::Client;
use std::collections::HashMap;
use tracing::{info, warn};

use super::date_logic::{
    PLAYOFFS_END_MONTH, PLAYOFFS_START_MONTH, PRESEASON_END_MONTH, PRESEASON_START_MONTH,
};
use super::urls::{build_tournament_schedule_url, build_tournament_url, create_tournament_key};

/// Represents a tournament type with its string identifier
#[derive(Debug, Clone, PartialEq)]
pub enum TournamentType {
    Runkosarja,
    Playoffs,
    Playout,
    Qualifications,
    ValmistavatOttelut,
}

impl TournamentType {
    /// Converts the tournament type to its string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            TournamentType::Runkosarja => "runkosarja",
            TournamentType::Playoffs => "playoffs",
            TournamentType::Playout => "playout",
            TournamentType::Qualifications => "qualifications",
            TournamentType::ValmistavatOttelut => "valmistavat_ottelut",
        }
    }

    /// Converts from the integer serie value used in ScheduleApiGame
    pub fn from_serie(serie: i32) -> Self {
        match serie {
            2 => TournamentType::Playoffs,
            3 => TournamentType::Playout,
            4 => TournamentType::Qualifications,
            5 => TournamentType::ValmistavatOttelut,
            _ => TournamentType::Runkosarja, // Default to runkosarja
        }
    }

    /// Converts to the integer serie value used in ScheduleApiGame
    pub fn to_serie(&self) -> i32 {
        match self {
            TournamentType::Runkosarja => 1,
            TournamentType::Playoffs => 2,
            TournamentType::Playout => 3,
            TournamentType::Qualifications => 4,
            TournamentType::ValmistavatOttelut => 5,
        }
    }
}

/// Determines which tournaments to check based on the month
pub fn determine_tournaments_for_month(month: u32) -> Vec<TournamentType> {
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
        tournaments
            .iter()
            .map(TournamentType::as_str)
            .collect::<Vec<_>>()
    );
    tournaments
}

/// Fetches games from all relevant tournaments for a given season
/// Implements connection pooling and parallel requests for better performance
pub async fn fetch_tournament_games(
    client: &Client,
    config: &Config,
    tournaments: &[TournamentType],
    season: i32,
) -> Vec<ScheduleApiGame> {
    // Import fetch function from core module
    use super::fetch_utils::fetch;

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

/// Fallback tournament selection based on calendar months when API data is not available.
/// This is the old logic preserved as a fallback.
pub fn build_tournament_list_fallback(date: &str) -> Vec<&'static str> {
    // Parse the date to get the month
    let date_parts: Vec<&str> = date.split('-').collect();
    let month = if date_parts.len() >= 2 {
        date_parts[1].parse::<u32>().unwrap_or(0)
    } else {
        // Default to current month if date parsing fails
        // Use UTC for consistency
        Utc::now().month()
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

/// Determines which tournaments are active by checking all tournament types in parallel.
/// Uses the API's nextGameDate to determine when tournaments transition.
/// Returns both the active tournaments and cached API responses to avoid double-fetching.
/// - Fetches all tournament data simultaneously for better performance
/// - Processes results in priority order (preseason -> regular -> playoffs -> playout -> qualifications)
/// - This naturally handles tournament transitions using API data
pub async fn determine_active_tournaments(
    client: &Client,
    config: &Config,
    date: &str,
) -> Result<(Vec<&'static str>, HashMap<String, ScheduleResponse>), AppError> {
    // Import fetch function from core module
    use super::fetch_utils::fetch;

    info!(
        "Determining active tournaments for date: {} using API nextGameDate logic",
        date
    );

    // Parse the date to get the month for tournament filtering
    let date_parts: Vec<&str> = date.split('-').collect();
    let month = if date_parts.len() >= 2 {
        date_parts[1].parse::<u32>().unwrap_or(0)
    } else {
        // Default to current month if date parsing fails
        Utc::now().month()
    };

    // Filter tournament candidates based on season (avoid unnecessary API calls)
    // Maintain original priority order: preseason -> regular -> playoffs -> playout -> qualifications
    let mut tournament_candidates = Vec::new();

    // Only include preseason during May-September
    if (PRESEASON_START_MONTH..=PRESEASON_END_MONTH).contains(&month) {
        info!(
            "Including valmistavat_ottelut (month {} is in preseason period)",
            month
        );
        tournament_candidates.push("valmistavat_ottelut");
    }

    // Always include regular season
    tournament_candidates.push("runkosarja");

    // Only include playoffs/playout/qualifications during March-June
    if (PLAYOFFS_START_MONTH..=PLAYOFFS_END_MONTH).contains(&month) {
        info!(
            "Including playoffs, playout, and qualifications (month {} is in playoff period)",
            month
        );
        tournament_candidates.push("playoffs");
        tournament_candidates.push("playout");
        tournament_candidates.push("qualifications");
    } else {
        info!(
            "Skipping playoffs, playout, and qualifications (month {} is outside playoff period)",
            month
        );
    }

    info!(
        "Tournament candidates for month {}: {:?}",
        month, tournament_candidates
    );

    // Create parallel futures for filtered tournament checks to improve performance
    let fetch_futures: Vec<_> = tournament_candidates
        .iter()
        .map(|&tournament| {
            let url = build_tournament_url(&config.api_domain, tournament, date);
            let tournament_name = tournament;

            async move {
                info!("Checking tournament: {}", tournament_name);
                match fetch::<ScheduleResponse>(client, &url).await {
                    Ok(response) => Ok((tournament_name, response)),
                    Err(e) => {
                        info!(
                            "Failed to fetch tournament {}: {}, will skip this tournament",
                            tournament_name, e
                        );
                        Err(e)
                    }
                }
            }
        })
        .collect();

    // Execute all tournament checks in parallel
    let results = futures::future::join_all(fetch_futures).await;

    let mut active: Vec<&'static str> = Vec::with_capacity(tournament_candidates.len());
    let mut cached_responses: HashMap<String, ScheduleResponse> = HashMap::new();

    // Process results in original order to maintain priority
    for (tournament, response) in results.into_iter().filter_map(Result::ok) {
        // Cache the response for downstream reuse
        let cache_key = create_tournament_key(tournament, date);
        cached_responses.insert(cache_key, response.clone());

        // If there are games on this date, mark this tournament active
        if !response.games.is_empty() {
            info!(
                "Found {} games for tournament {} on date {}",
                response.games.len(),
                tournament,
                date
            );
            active.push(tournament);
            continue;
        }

        // If no games but has a future nextGameDate, use this tournament
        if let Some(next_date) = &response.next_game_date {
            if let (Ok(next_parsed), Ok(current_parsed)) = (
                chrono::NaiveDate::parse_from_str(next_date, "%Y-%m-%d"),
                chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d"),
            ) {
                if next_parsed >= current_parsed {
                    info!(
                        "Tournament {} has future games on {}, using this tournament",
                        tournament, next_date
                    );
                    active.push(tournament);
                } else {
                    info!(
                        "Tournament {} nextGameDate {} is in the past, trying next tournament type",
                        tournament, next_date
                    );
                }
            }
        } else {
            info!(
                "Tournament {} has no nextGameDate, trying next tournament type",
                tournament
            );
        }
    }

    if active.is_empty() {
        warn!("No tournaments have future games, falling back to regular season");
        Ok((vec!["runkosarja"], cached_responses))
    } else {
        info!("Active tournaments selected: {:?}", active);
        Ok((active, cached_responses))
    }
}

/// Builds the list of tournaments to fetch based on the month.
/// Different tournaments are active during different parts of the season.
/// Returns both the active tournaments and cached API responses to avoid double-fetching.
/// This is now a wrapper around the lifecycle-based logic with fallback to month-based logic.
pub async fn build_tournament_list(
    client: &Client,
    config: &Config,
    date: &str,
) -> Result<(Vec<&'static str>, HashMap<String, ScheduleResponse>), AppError> {
    match determine_active_tournaments(client, config, date).await {
        Ok((tournaments, cached_responses)) => Ok((tournaments, cached_responses)),
        Err(e) => {
            warn!(
                "Failed to determine active tournaments via API, falling back to month-based selection: {}",
                e
            );
            let fallback_tournaments = build_tournament_list_fallback(date);
            Ok((fallback_tournaments, HashMap::new()))
        }
    }
}
