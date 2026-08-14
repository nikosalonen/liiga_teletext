// src/data_fetcher/api/orchestrator.rs - Main API orchestration logic extracted from core.rs

use crate::config::Config;
use crate::data_fetcher::cache::persistence::PLAYER_NAME_STORE;
use crate::data_fetcher::models::GameData;
use crate::error::AppError;
use tracing::{info, instrument, warn};

// HTTP client utilities available from sibling http_client module
use super::http_client::create_http_client_with_timeout;
// Date and season logic available from sibling date_logic module
use super::date_logic::{determine_fetch_date, parse_date_and_season};
// Season utilities available from sibling season_utils module
use super::season_utils::{is_historical_date, should_use_schedule_for_playoffs};
// Game-specific API operations available from sibling game_api module
use super::game_api::{fetch_historical_games, process_games};
// Tournament logic available from sibling tournament_logic module
use super::tournament_logic::build_tournament_list;
// Tournament-specific API operations available from sibling tournament_api module
use super::tournament_api::{
    best_next_game_date, best_previous_game_date, determine_return_date, fetch_day_data,
    handle_no_games_found,
};

/// Early check shared by every network entry point in this module: prevents
/// network calls if the API domain is not properly configured. This prevents
/// CI hangs when LIIGA_API_DOMAIN is unset or invalid.
fn ensure_api_domain_allows_network() -> Result<(), AppError> {
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
    Ok(())
}

/// Main API entry point that orchestrates the fetching of Liiga game data.
///
/// This function coordinates between multiple specialized modules to:
/// - Determine the appropriate date to fetch
/// - Route to historical vs current data endpoints
/// - Build tournament lists and fetch data
/// - Process and return the final game data
///
/// # Arguments
/// * `custom_date` - Optional date override in "YYYY-MM-DD" format
///
/// # Returns
/// * `Result<(Vec<GameData>, String), AppError>` - Tuple of games and the date they represent
///
/// # Example
/// ```rust,no_run
/// use liiga_teletext::data_fetcher::api::fetch_liiga_data;
///
/// #[tokio::main]
/// async fn main() -> Result<(), liiga_teletext::AppError> {
///     // Fetch data for today
///     let (games, date) = fetch_liiga_data(None).await?;
///
///     // Fetch data for a specific date
///     let (games, date) = fetch_liiga_data(Some("2024-01-15".to_string())).await?;
///
///     Ok(())
/// }
/// ```
#[instrument(skip(custom_date))]
pub async fn fetch_liiga_data(
    custom_date: Option<String>,
) -> Result<(Vec<GameData>, String), AppError> {
    info!("Starting to fetch Liiga data");

    ensure_api_domain_allows_network()?;

    let config = Config::load().await?;
    info!("Config loaded successfully");
    let client = create_http_client_with_timeout(config.http_timeout_seconds)?;

    // Determine the date to fetch data for
    let (date, is_pre_noon_cutoff) = determine_fetch_date(custom_date);

    // Load persistent player name cache for the current season
    let (_, _, season) = parse_date_and_season(&date);
    PLAYER_NAME_STORE.load_from_disk(season).await;

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
        PLAYER_NAME_STORE.save_to_disk().await;
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
            info!("Returning empty games list with next date: {return_date}");
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

    // Persist any newly cached player names to disk
    PLAYER_NAME_STORE.save_to_disk().await;

    Ok((all_games, return_date))
}

/// Returns the API's `previousGameDate` hint for the given date: the latest
/// date strictly before it on which any active tournament has games.
///
/// Reads the same per-tournament day responses `fetch_liiga_data` uses, so for
/// a date the user is currently viewing this is typically served entirely from
/// cache. Errors are logged and mapped to `None` because callers treat the
/// hint as an optimization with a slower search as fallback. Always `None`
/// for historical/playoff dates, which are served by the schedule endpoint
/// that carries no hints.
pub async fn fetch_previous_game_date_hint(date: &str) -> Option<String> {
    let (client, config) = hint_network_setup(date).await?;
    fetch_previous_game_date_hint_with(&client, &config, date).await
}

/// Returns the API's `nextGameDate` hint for the given date: the earliest
/// date strictly after it on which any active tournament has games.
/// Same caching and error semantics as [`fetch_previous_game_date_hint`].
pub async fn fetch_next_game_date_hint(date: &str) -> Option<String> {
    let (client, config) = hint_network_setup(date).await?;
    fetch_next_game_date_hint_with(&client, &config, date).await
}

/// [`fetch_previous_game_date_hint`] with injected client/config, so the
/// direction wiring is testable against a mock server.
pub(super) async fn fetch_previous_game_date_hint_with(
    client: &reqwest::Client,
    config: &Config,
    date: &str,
) -> Option<String> {
    let responses = fetch_day_responses_for_hint(client, config, date).await?;
    best_previous_game_date(&responses, date)
}

/// [`fetch_next_game_date_hint`] with injected client/config, so the
/// direction wiring is testable against a mock server.
pub(super) async fn fetch_next_game_date_hint_with(
    client: &reqwest::Client,
    config: &Config,
    date: &str,
) -> Option<String> {
    let responses = fetch_day_responses_for_hint(client, config, date).await?;
    if let Some(hint) = best_next_game_date(&responses, date) {
        return Some(hint);
    }
    regular_season_start_hint(client, config, date).await
}

/// Fallback next-date hint for the preseason boundary: near the end of the
/// practice-game schedule the API stops providing a usable `nextGameDate`
/// (it can even return past dates there), but the regular season opener IS
/// the next game date. Asks the runkosarja day endpoint first — the request
/// the July-August practice-game short-circuit deliberately skipped — and
/// falls back to the season schedule (typically already cached for the
/// footer's season countdown, but unavailable until the API publishes it).
async fn regular_season_start_hint(
    client: &reqwest::Client,
    config: &Config,
    date: &str,
) -> Option<String> {
    let (year, month, _) = parse_date_and_season(date);
    if !super::date_logic::is_preseason_only_month(month) {
        return None;
    }

    match super::tournament_api::fetch_tournament_data(client, config, "runkosarja", date).await {
        Ok(response) => {
            // The strict `> date` filter also drops the garbage past dates the
            // API is known to return in these hint fields
            if let Some(hint) = response.next_game_date.filter(|next| next.as_str() > date) {
                info!("Using runkosarja nextGameDate {hint} as next game date hint");
                return Some(hint);
            }
        }
        Err(e) => {
            warn!("Failed to fetch runkosarja day response for date hint: {e}");
        }
    }

    // The season starting in September of `year` is season `year + 1`
    match super::season_schedule::fetch_regular_season_start_date(client, config, year + 1).await {
        Ok(Some(start)) => {
            let start_date = chrono::DateTime::parse_from_rfc3339(&start)
                .ok()?
                .date_naive()
                .format("%Y-%m-%d")
                .to_string();
            if start_date.as_str() > date {
                info!("Using regular season start {start_date} as next game date hint");
                Some(start_date)
            } else {
                None
            }
        }
        Ok(None) => None,
        Err(e) => {
            warn!("Failed to fetch regular season start for date hint: {e}");
            None
        }
    }
}

/// Prepares the client and config for a hint fetch. Returns `None` without
/// touching the network when hints cannot exist for the date or when the
/// environment forbids network calls.
async fn hint_network_setup(date: &str) -> Option<(reqwest::Client, Config)> {
    if is_historical_date(date) || should_use_schedule_for_playoffs(date) {
        // Historical/playoff dates are served by the schedule endpoint,
        // which carries no previous/nextGameDate hints
        return None;
    }

    ensure_api_domain_allows_network().ok()?;

    let config = match Config::load().await {
        Ok(config) => config,
        Err(e) => {
            warn!("Failed to load config for date hint: {e}");
            return None;
        }
    };
    let client = match create_http_client_with_timeout(config.http_timeout_seconds) {
        Ok(client) => client,
        Err(e) => {
            warn!("Failed to create HTTP client for date hint: {e}");
            return None;
        }
    };
    Some((client, config))
}

/// Fetches the per-tournament day responses used for previous/next date hints.
async fn fetch_day_responses_for_hint(
    client: &reqwest::Client,
    config: &Config,
    date: &str,
) -> Option<std::collections::HashMap<String, crate::data_fetcher::models::ScheduleResponse>> {
    let (tournaments, cached_responses) = match build_tournament_list(client, config, date).await {
        Ok(result) => result,
        Err(e) => {
            warn!("Failed to build tournament list for date hint: {e}");
            return None;
        }
    };

    match fetch_day_data(client, config, &tournaments, date, &[], &cached_responses).await {
        Ok((_, tournament_responses)) => Some(tournament_responses),
        Err(e) => {
            warn!("Failed to fetch day data for date hint: {e}");
            None
        }
    }
}
