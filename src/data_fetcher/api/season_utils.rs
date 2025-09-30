//! Season detection and historical date utility functions

use chrono::{Datelike, Local, Utc};

/// Determines if a date is from a previous season (not the current season).
/// Hockey seasons typically start in September and end in April/May.
/// So a date in May-July is from the previous season.
pub fn is_historical_date(date: &str) -> bool {
    let now = Utc::now().with_timezone(&Local);
    is_historical_date_with_current_time(date, now)
}

/// Internal function that determines if a date is historical given a specific current time.
/// This allows for testing with mocked current times.
pub fn is_historical_date_with_current_time(
    date: &str,
    current_time: chrono::DateTime<Local>,
) -> bool {
    let date_parts: Vec<&str> = date.split('-').collect();
    if date_parts.len() < 2 {
        return false;
    }

    let date_year = date_parts[0]
        .parse::<i32>()
        .unwrap_or_else(|_| current_time.year());
    let date_month = date_parts[1]
        .parse::<u32>()
        .unwrap_or_else(|_| current_time.month());

    // Try to parse the full date to check if it's in the future
    if let Ok(parsed_date) = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        let current_date = current_time.date_naive();

        // Future dates should not be considered historical
        if parsed_date > current_date {
            return false;
        }
    }

    let current_year = current_time.year();
    let current_month = current_time.month();

    // Hockey season logic:
    // - Season starts in September (month 9)
    // - Season ends in April/May (months 4-5)
    // - So dates in May-July are from the previous season

    if date_year < current_year {
        // Definitely historical
        return true;
    } else if date_year == current_year {
        // Same year, check if it's in the off-season
        // If current month is August (8) and date is May-July, it's from previous season
        if current_month == 8 && (5..=7).contains(&date_month) {
            return true;
        }
        // If we're in the off-season (May-July) and the date is from the regular season (September-April),
        // it's from the previous season
        if (5..=7).contains(&current_month) && (date_month >= 9 || date_month <= 4) {
            return true;
        }
    }

    false
}

/// Determines if a date requires the schedule endpoint for playoff games.
/// This handles the specific case where playoff games from the previous season
/// need to be fetched using the schedule endpoint instead of the games endpoint.
pub fn should_use_schedule_for_playoffs(date: &str) -> bool {
    let now = Utc::now().with_timezone(&Local);
    should_use_schedule_for_playoffs_with_current_time(date, now)
}

/// Internal function to check if a date requires the schedule endpoint for playoffs.
pub fn should_use_schedule_for_playoffs_with_current_time(
    date: &str,
    current_time: chrono::DateTime<Local>,
) -> bool {
    let date_parts: Vec<&str> = date.split('-').collect();
    if date_parts.len() < 2 {
        return false;
    }

    let date_year = date_parts[0]
        .parse::<i32>()
        .unwrap_or_else(|_| current_time.year());
    let date_month = date_parts[1]
        .parse::<u32>()
        .unwrap_or_else(|_| current_time.month());

    // Check if date is in the future
    if let Ok(parsed_date) = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        let current_date = current_time.date_naive();
        if parsed_date > current_date {
            return false;
        }
    }

    let current_year = current_time.year();
    let current_month = current_time.month();

    // Only check for playoff games in the same year
    if date_year == current_year {
        // If we're in the off-season (June-August) and looking at playoff months (March-May)
        // from the same year, we need to use the schedule endpoint
        if (6..=8).contains(&current_month) && (3..=5).contains(&date_month) {
            return true;
        }
    }

    false
}
