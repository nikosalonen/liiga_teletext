//! Date determination and season logic utilities

use chrono::{Datelike, Local, Utc};
use tracing::info;

// Tournament season constants for month-based logic
pub const PRESEASON_START_MONTH: u32 = 5; // May
pub const PRESEASON_END_MONTH: u32 = 9; // September
pub const PLAYOFFS_START_MONTH: u32 = 3; // March
pub const PLAYOFFS_END_MONTH: u32 = 6; // June

/// Determines the date to fetch data for based on custom date or current time.
/// Returns today's date if games should be shown today, otherwise yesterday's date.
/// Uses UTC internally for consistent calculations, formats as local date for display.
/// Also returns whether this date was chosen due to pre-noon cutoff logic.
pub fn determine_fetch_date(custom_date: Option<String>) -> (String, bool) {
    // Use UTC for internal calculations to avoid DST issues
    let now_utc = Utc::now();
    // Convert to local time for the date decision logic
    let now_local = now_utc.with_timezone(&Local);

    determine_fetch_date_with_time(custom_date, now_local)
}

/// Internal helper function for determining fetch date with injected time.
/// This allows for deterministic testing by accepting a specific time instead of using the current time.
///
/// # Arguments
/// * `custom_date` - Optional custom date to use instead of time-based logic
/// * `now_local` - The local time to use for cutoff decisions
///
/// # Returns
/// * `(String, bool)` - Tuple of (date_string, is_pre_noon_cutoff)
pub fn determine_fetch_date_with_time(
    custom_date: Option<String>,
    now_local: chrono::DateTime<chrono::Local>,
) -> (String, bool) {
    use crate::data_fetcher::processors::should_show_todays_games_with_time;

    match custom_date {
        Some(date) => (date, false), // Custom date provided, not due to cutoff
        None => {
            if should_show_todays_games_with_time(now_local) {
                let date_str = now_local.format("%Y-%m-%d").to_string();
                info!("Using today's date: {date_str}");
                (date_str, false)
            } else {
                let yesterday = now_local
                    .date_naive()
                    .pred_opt()
                    .expect("Date underflow cannot happen with valid date");
                let date_str = yesterday.format("%Y-%m-%d").to_string();
                info!(
                    "Using yesterday's date due to pre-noon cutoff: {}",
                    date_str
                );
                (date_str, true) // This was chosen due to pre-noon cutoff
            }
        }
    }
}

/// Parses a date string and determines the hockey season
/// Hockey seasons typically start in September and end in April/May
/// Returns (year, month, season)
pub fn parse_date_and_season(date: &str) -> (i32, u32, i32) {
    let date_parts: Vec<&str> = date.split('-').collect();
    let (year, month) = if date_parts.len() >= 2 {
        let y = date_parts[0]
            .parse::<i32>()
            .unwrap_or_else(|_| Utc::now().year());
        let m = date_parts[1].parse::<u32>().unwrap_or(1);
        (y, m)
    } else {
        (Utc::now().year(), 1)
    };

    // Ice hockey season: if month >= 9, season = year+1, else season = year
    let season = if month >= 9 { year + 1 } else { year };

    info!(
        "Parsed date: year={}, month={}, season={}",
        year, month, season
    );
    (year, month, season)
}
