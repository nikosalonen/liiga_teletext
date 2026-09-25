//! Tournament selection and fetching logic

use crate::config::Config;
use crate::data_fetcher::models::{ScheduleApiGame, ScheduleResponse};
use crate::error::AppError;
use chrono::{Datelike, Utc};
use futures;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
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

/// Negative cache for tournament availability checks. When a secondary
/// tournament endpoint fails with 502/503/404 (typically an unpublished
/// tournament, e.g. preseason fixtures before they are announced), it is
/// remembered here and skipped until the entry expires. Keyed by
/// "{api_domain}:{tournament}" so tests with separate mock servers don't
/// interfere with each other.
static UNAVAILABLE_TOURNAMENTS: LazyLock<RwLock<HashMap<String, Instant>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn unavailable_key(api_domain: &str, tournament: &str) -> String {
    format!("{api_domain}:{tournament}")
}

/// Whether the error indicates a long-lived "tournament not published"
/// condition worth negative-caching (as opposed to a transient failure).
fn is_unavailability_error(error: &AppError) -> bool {
    matches!(
        error,
        AppError::ApiServiceUnavailable { .. } | AppError::ApiNotFound { .. }
    )
}

async fn is_marked_unavailable(api_domain: &str, tournament: &str) -> bool {
    UNAVAILABLE_TOURNAMENTS
        .read()
        .await
        .get(&unavailable_key(api_domain, tournament))
        .is_some_and(|expiry| *expiry > Instant::now())
}

async fn mark_unavailable(api_domain: &str, tournament: &str) {
    let now = Instant::now();
    let expiry =
        now + Duration::from_secs(crate::constants::cache_ttl::TOURNAMENT_UNAVAILABLE_SECONDS);
    let mut map = UNAVAILABLE_TOURNAMENTS.write().await;
    map.retain(|_, e| *e > now); // Drop expired entries while we hold the lock
    map.insert(unavailable_key(api_domain, tournament), expiry);
}

/// Clears the tournament unavailability cache. Test isolation only: mock
/// server ports are reused across tests, so entries from one test could
/// otherwise leak into another test's domain.
#[cfg(test)]
pub(super) async fn clear_unavailable_tournaments_cache() {
    UNAVAILABLE_TOURNAMENTS.write().await.clear();
}

/// The tournaments that can have games in a month, in priority order:
/// preseason, regular season, playoffs, playout, qualifications.
/// Every tournament list (live, historical and date search) comes from here.
pub fn determine_tournaments_for_month(month: u32) -> Vec<TournamentType> {
    let mut tournaments = Vec::new();
    if (PRESEASON_START_MONTH..=PRESEASON_END_MONTH).contains(&month) {
        tournaments.push(TournamentType::ValmistavatOttelut);
    }
    tournaments.push(TournamentType::Runkosarja);
    if (PLAYOFFS_START_MONTH..=PLAYOFFS_END_MONTH).contains(&month) {
        tournaments.extend([
            TournamentType::Playoffs,
            TournamentType::Playout,
            TournamentType::Qualifications,
        ]);
    }
    tournaments
}

/// Same as [`determine_tournaments_for_month`], as API names.
fn tournament_names_for_month(month: u32) -> Vec<&'static str> {
    determine_tournaments_for_month(month)
        .iter()
        .map(TournamentType::as_str)
        .collect()
}

/// Month of a `YYYY-MM-DD` date, or the current month if the date can't be read.
fn month_of_date(date: &str) -> u32 {
    date.split('-')
        .nth(1)
        .and_then(|month| month.parse().ok())
        .unwrap_or_else(|| Utc::now().month())
}

/// Fetches games from all relevant tournaments for a given season
/// Implements connection pooling and parallel requests for better performance
///
/// Returns an error if `runkosarja` (the main data source) failed, or if no
/// tournament returned a response and at least one fetch failed. A tournament
/// that is not found just has no games, so its failure is not an error here.
pub async fn fetch_tournament_games(
    client: &Client,
    config: &Config,
    tournaments: &[TournamentType],
    season: i32,
) -> Result<Vec<ScheduleApiGame>, AppError> {
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
                        Err((*tournament == TournamentType::Runkosarja, e))
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
    let mut primary_fetch_error = None;
    let mut secondary_fetch_error = None;

    for result in results {
        match result {
            Ok(games) => {
                all_schedule_games.extend(games);
                successful_fetches += 1;
            }
            Err((is_primary, e)) => {
                failed_fetches += 1;
                if !e.is_not_found() {
                    let slot = if is_primary {
                        &mut primary_fetch_error
                    } else {
                        &mut secondary_fetch_error
                    };
                    slot.get_or_insert(e);
                }
            }
        }
    }

    info!(
        "Tournament fetch completed: {} successful, {} failed, {} total games",
        successful_fetches,
        failed_fetches,
        all_schedule_games.len()
    );

    if let Some(error) = primary_fetch_error {
        return Err(error);
    }
    if successful_fetches == 0
        && let Some(error) = secondary_fetch_error
    {
        return Err(error);
    }
    Ok(all_schedule_games)
}

/// Tournament candidates for a date, based on the calendar month only.
pub fn build_tournament_list_fallback(date: &str) -> Vec<&'static str> {
    tournament_names_for_month(month_of_date(date))
}

/// Fetches one tournament's day response, applying the per-tournament retry
/// budget and negative-caching long-lived unavailability errors for
/// secondary tournaments.
async fn check_tournament(
    client: &Client,
    config: &Config,
    tournament: &'static str,
    date: &str,
) -> Result<(&'static str, ScheduleResponse), AppError> {
    use super::fetch_utils::fetch_with_retries;

    let url = build_tournament_url(&config.api_domain, tournament, date);

    // Secondary tournament endpoints return 502 until published;
    // retrying those aggressively only risks rate limiting.
    let max_retries = if tournament == "runkosarja" {
        crate::constants::retry::MAX_ATTEMPTS
    } else {
        crate::constants::retry::SECONDARY_TOURNAMENT_MAX_ATTEMPTS
    };

    info!("Checking tournament: {tournament}");
    match fetch_with_retries::<ScheduleResponse>(client, &url, max_retries).await {
        Ok(response) => Ok((tournament, response)),
        Err(e) => {
            if tournament != "runkosarja" && is_unavailability_error(&e) {
                let cooldown_minutes =
                    crate::constants::cache_ttl::TOURNAMENT_UNAVAILABLE_SECONDS / 60;
                warn!(
                    "Tournament {tournament} endpoint unavailable ({e}), skipping it for the next {cooldown_minutes} minutes"
                );
                mark_unavailable(&config.api_domain, tournament).await;
            } else {
                info!("Failed to fetch tournament {tournament}: {e}, will skip this tournament");
            }
            Err(e)
        }
    }
}

/// Determines which tournaments are active by checking all tournament types in parallel.
/// Uses the API's nextGameDate to determine when tournaments transition.
/// Returns both the active tournaments and cached API responses to avoid double-fetching.
/// - During July-August, checks practice games first and skips the regular
///   season request when practice games exist on the date
/// - Otherwise fetches all tournament data simultaneously for better performance
/// - Processes results in priority order (preseason -> regular -> playoffs -> playout -> qualifications)
/// - This naturally handles tournament transitions using API data
pub async fn determine_active_tournaments(
    client: &Client,
    config: &Config,
    date: &str,
) -> Result<(Vec<&'static str>, HashMap<String, ScheduleResponse>), AppError> {
    info!(
        "Determining active tournaments for date: {} using API nextGameDate logic",
        date
    );

    // Only ask for tournaments that can have games this month
    let month = month_of_date(date);
    let tournament_candidates = tournament_names_for_month(month);

    info!(
        "Tournament candidates for month {}: {:?}",
        month, tournament_candidates
    );

    // Skip secondary tournaments whose endpoint recently failed with a
    // long-lived unavailability error (e.g. unpublished preseason fixtures).
    // runkosarja is always re-checked: it is the primary data source and a
    // transient failure must not suppress it.
    let mut checkable_candidates = Vec::with_capacity(tournament_candidates.len());
    for &tournament in &tournament_candidates {
        if tournament != "runkosarja" && is_marked_unavailable(&config.api_domain, tournament).await
        {
            info!(
                "Skipping tournament {} - endpoint recently unavailable, will re-check after cooldown",
                tournament
            );
        } else {
            checkable_candidates.push(tournament);
        }
    }

    let mut results: Vec<Result<(&'static str, ScheduleResponse), AppError>> =
        Vec::with_capacity(checkable_candidates.len());

    // July-August: practice games are the only games being played (the
    // regular season starts in September, playoffs ended in spring), so check
    // valmistavat_ottelut first and skip the runkosarja request entirely when
    // practice games exist on this date. Not applied in September, when the
    // regular season starts while the last practice games may still be
    // played, nor in May-June, when the playoff tournaments are candidates.
    if super::date_logic::is_preseason_only_month(month)
        && checkable_candidates.contains(&"valmistavat_ottelut")
    {
        let practice_result = check_tournament(client, config, "valmistavat_ottelut", date).await;
        let has_practice_games = matches!(&practice_result, Ok((_, r)) if !r.games.is_empty());
        results.push(practice_result);
        checkable_candidates.retain(|&t| t != "valmistavat_ottelut");
        if has_practice_games {
            info!(
                "Practice games found on {date} (month {month} is preseason-only), skipping regular season check"
            );
            checkable_candidates.clear();
        }
    }

    // Check the remaining tournaments in parallel
    let fetch_futures: Vec<_> = checkable_candidates
        .iter()
        .map(|&tournament| check_tournament(client, config, tournament, date))
        .collect();
    results.extend(futures::future::join_all(fetch_futures).await);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unavailable_cache_is_domain_scoped() {
        mark_unavailable("https://domain-a.test", "valmistavat_ottelut").await;

        assert!(is_marked_unavailable("https://domain-a.test", "valmistavat_ottelut").await);
        // Different domain (e.g. another mock server) is unaffected
        assert!(!is_marked_unavailable("https://domain-b.test", "valmistavat_ottelut").await);
        // Different tournament on the same domain is unaffected
        assert!(!is_marked_unavailable("https://domain-a.test", "playoffs").await);
    }

    #[test]
    fn test_month_windows_follow_liiga_calendar() {
        // Practice games (valmistavat_ottelut) are played in August and can
        // spill into early September, where the regular season also starts:
        // both months must poll both tournaments
        for month in [8, 9] {
            let tournaments = determine_tournaments_for_month(month);
            assert!(
                tournaments.contains(&TournamentType::ValmistavatOttelut),
                "month {month} must include valmistavat_ottelut"
            );
            assert!(
                tournaments.contains(&TournamentType::Runkosarja),
                "month {month} must include runkosarja"
            );
        }

        // October-February: the regular season is the only ongoing tournament
        for month in [10, 11, 12, 1, 2] {
            assert_eq!(
                determine_tournaments_for_month(month),
                vec![TournamentType::Runkosarja],
                "month {month} must poll only runkosarja"
            );
        }

        // Spring: playoffs, playout, and qualifications join runkosarja
        for month in [3, 4] {
            let tournaments = determine_tournaments_for_month(month);
            assert!(
                tournaments.contains(&TournamentType::Playoffs),
                "month {month} must include playoffs"
            );
            assert!(
                tournaments.contains(&TournamentType::Runkosarja),
                "month {month} must include runkosarja"
            );
        }
    }

    #[test]
    fn test_live_and_historical_tournament_lists_agree() {
        for month in 1..=12 {
            let historical: Vec<&str> = determine_tournaments_for_month(month)
                .iter()
                .map(TournamentType::as_str)
                .collect();
            let live = build_tournament_list_fallback(&format!("2026-{month:02}-15"));
            assert_eq!(historical, live, "month {month}");
        }
    }

    #[test]
    fn test_fallback_tournament_list_follows_liiga_calendar() {
        // September: preseason may still trickle in while the regular season starts
        let september = build_tournament_list_fallback("2026-09-05");
        assert!(september.contains(&"valmistavat_ottelut"));
        assert!(september.contains(&"runkosarja"));

        // August: practice games plus the (not yet started) regular season
        let august = build_tournament_list_fallback("2026-08-14");
        assert!(august.contains(&"valmistavat_ottelut"));
        assert!(august.contains(&"runkosarja"));
        assert!(!august.contains(&"playoffs"));

        // October: regular season only
        assert_eq!(
            build_tournament_list_fallback("2026-10-14"),
            vec!["runkosarja"]
        );
    }

    #[test]
    fn test_unavailability_error_classification() {
        assert!(is_unavailability_error(&AppError::api_service_unavailable(
            502,
            "Bad Gateway",
            "http://test"
        )));
        assert!(is_unavailability_error(&AppError::api_not_found(
            "http://test"
        )));
        // Transient/network errors must not be negative-cached
        assert!(!is_unavailability_error(&AppError::network_timeout(
            "http://test"
        )));
        assert!(!is_unavailability_error(&AppError::api_server_error(
            500,
            "Internal Server Error",
            "http://test"
        )));
    }
}
