use crate::config::Config;
use crate::data_fetcher::models::ScheduleApiGame;
use crate::error::AppError;
use reqwest::Client;
use tracing::{error, info, instrument};

// Generic fetch utility available from sibling fetch_utils module
use super::fetch_utils::fetch;
// URL builders available from sibling urls module
use super::urls::build_schedule_url;

/// Fetches the regular season schedule to determine the season start date.
/// Returns the start date of the first regular season game.
#[instrument(skip(client, config))]
pub async fn fetch_regular_season_start_date(
    client: &Client,
    config: &Config,
    season: i32,
) -> Result<Option<String>, AppError> {
    info!("Fetching regular season schedule for season: {season}");
    let url = build_schedule_url(&config.api_domain, season);

    match fetch::<Vec<ScheduleApiGame>>(client, &url).await {
        Ok(games) => {
            if games.is_empty() {
                info!("No regular season games found for season: {season}");
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
