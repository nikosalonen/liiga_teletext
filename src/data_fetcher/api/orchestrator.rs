// src/data_fetcher/api/orchestrator.rs - Main API orchestration logic extracted from core.rs

use crate::config::Config;
use crate::data_fetcher::models::GameData;
use crate::error::AppError;
use tracing::{info, instrument, warn};

// HTTP client utilities available from sibling http_client module
use super::http_client::create_http_client_with_timeout;
// Date and season logic available from sibling date_logic module
use super::date_logic::determine_fetch_date;
// Season utilities available from sibling season_utils module
use super::season_utils::{is_historical_date, should_use_schedule_for_playoffs};
// Game-specific API operations available from sibling game_api module
use super::game_api::{fetch_historical_games, process_games};
// Tournament logic available from sibling tournament_logic module
use super::tournament_logic::build_tournament_list;
// Tournament-specific API operations available from sibling tournament_api module
use super::tournament_api::{determine_return_date, fetch_day_data, handle_no_games_found};

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