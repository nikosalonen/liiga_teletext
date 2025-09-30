use crate::config::Config;
use crate::data_fetcher::api::fetch_regular_season_start_date;
use chrono::{DateTime, Datelike, Local, Utc};
use reqwest::Client;

/// Calculates the number of days until the regular season starts.
/// Returns None if the regular season has already started or if we can't determine the start date.
/// Uses UTC internally for consistent calculations across timezone changes.
///
/// # Arguments
/// * `client` - HTTP client for API requests
/// * `config` - Configuration containing API domain
/// * `current_year` - Optional current year (defaults to current UTC year)
pub async fn calculate_days_until_regular_season(
    client: &Client,
    config: &Config,
    current_year: Option<i32>,
) -> Option<i64> {
    // Use UTC for consistent year calculation, convert to local for display logic
    let current_year = current_year.unwrap_or_else(|| Utc::now().with_timezone(&Local).year());

    // Try current year first
    match fetch_regular_season_start_date(client, config, current_year).await {
        Ok(Some(start_date)) => {
            // Parse the ISO 8601 date from the API
            if let Ok(season_start) = DateTime::parse_from_rfc3339(&start_date) {
                let today = Utc::now();
                let days_until =
                    (season_start.naive_utc().date() - today.naive_utc().date()).num_days();

                if days_until > 0 {
                    return Some(days_until);
                }
            }
        }
        Ok(None) => {
            // No games found for current year, try next year
        }
        Err(_) => {
            // API call failed, we can't determine the start date
            return None;
        }
    }

    // Try next year if current year failed or no games found
    match fetch_regular_season_start_date(client, config, current_year + 1).await {
        Ok(Some(start_date)) => {
            // Parse the ISO 8601 date from the API
            if let Ok(season_start) = DateTime::parse_from_rfc3339(&start_date) {
                let today = Utc::now();
                let days_until =
                    (season_start.naive_utc().date() - today.naive_utc().date()).num_days();

                if days_until > 0 {
                    return Some(days_until);
                }
            }
        }
        Ok(None) => {
            // No games found for next year either
        }
        Err(_) => {
            // API call failed, we can't determine the start date
            return None;
        }
    }

    // If all API calls failed or no valid dates found, return None
    None
}