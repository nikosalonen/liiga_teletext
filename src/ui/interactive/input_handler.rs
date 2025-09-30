//! Keyboard input handling and date navigation for the interactive UI.
//!
//! This module handles:
//! - Keyboard event processing (quit, refresh, page navigation)
//! - Date navigation with Shift + Arrow keys
//! - Finding previous/next dates with games
//! - Season boundary checking

use crate::data_fetcher::{GameData, fetch_liiga_data, is_historical_date};
use crate::error::AppError;
use crate::teletext_ui::TeletextPage;
use chrono::{Datelike, Local, NaiveDate, Utc};
use crossterm::event::{self, KeyCode, KeyModifiers};
use std::io::stdout;
use std::time::{Duration, Instant};

/// Parameters for keyboard event handling
pub(super) struct KeyEventParams<'a> {
    pub key_event: &'a event::KeyEvent,
    pub current_page: &'a mut Option<TeletextPage>,
    pub needs_render: &'a mut bool,
    pub needs_refresh: &'a mut bool,
    pub current_date: &'a mut Option<String>,
    pub last_manual_refresh: &'a mut Instant,
    pub last_page_change: &'a mut Instant,
    pub last_date_navigation: &'a mut Instant,
}

/// Checks if the given key event matches the date navigation shortcut.
/// Uses Shift + Left/Right for all platforms (works reliably in all terminals)
fn is_date_navigation_key(key_event: &event::KeyEvent, is_left: bool) -> bool {
    let expected_code = if is_left {
        KeyCode::Left
    } else {
        KeyCode::Right
    };

    if key_event.code != expected_code {
        return false;
    }

    // Use Shift key for date navigation (works reliably in all terminals)
    let has_shift_modifier = key_event.modifiers.contains(KeyModifiers::SHIFT);

    if has_shift_modifier {
        tracing::debug!(
            "Date navigation key detected: Shift + {}",
            if is_left { "Left" } else { "Right" }
        );
        return true;
    }

    false
}

/// Gets the target date for navigation, using current_date if available,
/// otherwise determining the appropriate date based on current time.
fn get_target_date_for_navigation(current_date: &Option<String>) -> String {
    current_date.as_ref().cloned().unwrap_or_else(|| {
        // If no current date, use today/yesterday based on time
        if crate::data_fetcher::processors::should_show_todays_games() {
            Utc::now()
                .with_timezone(&Local)
                .format("%Y-%m-%d")
                .to_string()
        } else {
            let yesterday = Utc::now()
                .with_timezone(&Local)
                .date_naive()
                .pred_opt()
                .expect("Date underflow cannot happen");
            yesterday.format("%Y-%m-%d").to_string()
        }
    })
}

/// Checks if a date would require historical/schedule endpoint (from previous season).
/// This prevents navigation to very old games via arrow keys, but allows reasonable historical access.
fn would_be_previous_season(date: &str) -> bool {
    let now = Utc::now().with_timezone(&Local);

    let date_parts: Vec<&str> = date.split('-').collect();
    if date_parts.len() < 2 {
        return false;
    }

    let date_year = date_parts[0].parse::<i32>().unwrap_or(now.year());
    let date_month = date_parts[1].parse::<u32>().unwrap_or(now.month());

    let current_year = now.year();
    let current_month = now.month();

    // Allow navigation within the past 2 years for reasonable historical access
    // This covers the current season and the previous season
    if date_year < current_year - 1 {
        return true;
    }

    // For dates within the past 2 years, use more nuanced season logic
    if date_year == current_year {
        // Same year - check if we're trying to go to off-season of previous season
        // Hockey season: September-February (regular), March-May (playoffs/playout)
        // Off-season: June-August

        // If we're in new regular season (September-December) and date is from off-season
        // (June-August), it's from the previous season
        if (9..=12).contains(&current_month) && (6..=8).contains(&date_month) {
            return true;
        }
    } else if date_year == current_year - 1 {
        // Previous year - allow access to recent hockey season games
        // Only block if we're trying to access very old off-season games

        // If we're currently in the new season (September+) and trying to access
        // off-season games from the previous year (June-August), block it
        if current_month >= 9 && (6..=8).contains(&date_month) {
            return true;
        }
    }

    false
}

/// Finds the previous date with games by checking dates going backwards.
/// Returns None if no games are found within the current season or a reasonable time range.
/// Prevents navigation to previous season games for better UX.
async fn find_previous_date_with_games(current_date: &str) -> Option<String> {
    let current_parsed = match NaiveDate::parse_from_str(current_date, "%Y-%m-%d") {
        Ok(date) => date,
        Err(_) => return None,
    };

    tracing::info!(
        "Starting search for previous date with games from: {}",
        current_date
    );

    // Search up to 30 days in the past to stay within current season
    for days_back in 1..=30 {
        if let Some(check_date) = current_parsed.checked_sub_days(chrono::Days::new(days_back)) {
            let date_string = check_date.format("%Y-%m-%d").to_string();

            // Check if this date would be from the previous season
            if would_be_previous_season(&date_string) {
                tracing::info!(
                    "Reached previous season boundary at {}, stopping navigation (use -d flag for historical games)",
                    date_string
                );
                break;
            }

            // Log progress every 10 days
            if days_back % 10 == 0 {
                tracing::info!(
                    "Date navigation: checking {} ({} days back)",
                    date_string,
                    days_back
                );
            }

            // Add timeout to the fetch operation (allow enough time for detailed game data including goal scorers)
            let fetch_future = fetch_liiga_data(Some(date_string.clone()));
            let timeout_duration = Duration::from_secs(15);

            match tokio::time::timeout(timeout_duration, fetch_future).await {
                Ok(Ok((games, fetched_date))) if !games.is_empty() => {
                    // Ensure the fetched date matches the requested date
                    if fetched_date == date_string {
                        tracing::info!(
                            "Found previous date with games: {} (after {} days)",
                            date_string,
                            days_back
                        );
                        return Some(date_string);
                    } else {
                        tracing::debug!(
                            "Skipping date {} because fetcher returned different date: {} (after {} days)",
                            date_string,
                            fetched_date,
                            days_back
                        );
                    }
                }
                Ok(Ok(_)) => {
                    // No games found, continue searching
                }
                Ok(Err(e)) => {
                    tracing::warn!(
                        "Error fetching data for {}: {} (continuing search)",
                        date_string,
                        e
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        "Timeout fetching data for {} (continuing search)",
                        date_string
                    );
                }
            }

            // Small delay to prevent API spam
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    tracing::info!(
        "No previous date with games found within current season from {}",
        current_date
    );
    None
}

/// Finds the next date with games by checking dates going forwards.
/// Returns None if no games are found within a reasonable time range.
async fn find_next_date_with_games(current_date: &str) -> Option<String> {
    let current_parsed = match NaiveDate::parse_from_str(current_date, "%Y-%m-%d") {
        Ok(date) => date,
        Err(_) => return None,
    };

    tracing::info!(
        "Starting search for next date with games from: {}",
        current_date
    );

    // Search up to 60 days in the future (handles off-season periods)
    for days_ahead in 1..=60 {
        if let Some(check_date) = current_parsed.checked_add_days(chrono::Days::new(days_ahead)) {
            let date_string = check_date.format("%Y-%m-%d").to_string();

            // Log progress every 10 days
            if days_ahead % 10 == 0 {
                tracing::info!(
                    "Date navigation: checking {} ({} days ahead)",
                    date_string,
                    days_ahead
                );
            }

            // Add timeout to the fetch operation (allow enough time for detailed game data including goal scorers)
            let fetch_future = fetch_liiga_data(Some(date_string.clone()));
            let timeout_duration = Duration::from_secs(15);

            match tokio::time::timeout(timeout_duration, fetch_future).await {
                Ok(Ok((games, fetched_date))) if !games.is_empty() => {
                    // Ensure the fetched date matches the requested date
                    if fetched_date == date_string {
                        tracing::info!(
                            "Found next date with games: {} (after {} days)",
                            date_string,
                            days_ahead
                        );
                        return Some(date_string);
                    } else {
                        tracing::debug!(
                            "Skipping date {} because fetcher returned different date: {} (after {} days)",
                            date_string,
                            fetched_date,
                            days_ahead
                        );
                    }
                }
                Ok(Ok(_)) => {
                    // No games found, continue searching
                }
                Ok(Err(e)) => {
                    tracing::warn!(
                        "Error fetching data for {}: {} (continuing search)",
                        date_string,
                        e
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        "Timeout fetching data for {} (continuing search)",
                        date_string
                    );
                }
            }

            // Small delay to prevent API spam
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    tracing::info!(
        "No next date with games found within search range from {}",
        current_date
    );
    None
}

/// Handle keyboard events
pub(super) async fn handle_key_event(params: KeyEventParams<'_>) -> Result<bool, AppError> {
    tracing::debug!(
        "Key event: {:?}, modifiers: {:?}",
        params.key_event.code,
        params.key_event.modifiers
    );

    // Check for date navigation first (Shift + Arrow keys)
    if is_date_navigation_key(params.key_event, true) {
        // Shift + Left: Previous date with games
        if params.last_date_navigation.elapsed() >= Duration::from_millis(250) {
            tracing::info!("Previous date navigation requested");
            tracing::debug!("Current date state: {:?}", params.current_date);
            let target_date = get_target_date_for_navigation(params.current_date);

            // Show loading indicator
            if let Some(page) = params.current_page.as_mut() {
                page.show_loading("Etsitään edellisiä otteluita...".to_string());
                // Force immediate render to show loading indicator
                let mut stdout = stdout();
                let _ = page.render_buffered(&mut stdout);
                *params.needs_render = true;
            }

            tracing::info!(
                "Searching for previous date with games from: {}",
                target_date
            );

            let result = find_previous_date_with_games(&target_date).await;

            if let Some(prev_date) = result {
                *params.current_date = Some(prev_date.clone());
                *params.needs_refresh = true;
                tracing::info!("Navigated to previous date: {}", prev_date);
            } else {
                tracing::warn!("No previous date with games found");
            }

            // Hide loading indicator
            if let Some(page) = params.current_page.as_mut() {
                page.hide_loading();
            }
            *params.last_date_navigation = Instant::now();
        }
    } else if is_date_navigation_key(params.key_event, false) {
        // Shift + Right: Next date with games
        if params.last_date_navigation.elapsed() >= Duration::from_millis(250) {
            tracing::info!("Next date navigation requested");
            tracing::debug!("Current date state: {:?}", params.current_date);
            let target_date = get_target_date_for_navigation(params.current_date);

            // Show loading indicator
            if let Some(page) = params.current_page.as_mut() {
                page.show_loading("Etsitään seuraavia otteluita...".to_string());
                // Force immediate render to show loading indicator
                let mut stdout = stdout();
                let _ = page.render_buffered(&mut stdout);
                *params.needs_render = true;
            }

            tracing::info!("Searching for next date with games from: {}", target_date);

            let result = find_next_date_with_games(&target_date).await;

            if let Some(next_date) = result {
                *params.current_date = Some(next_date.clone());
                *params.needs_refresh = true;
                tracing::info!("Navigated to next date: {}", next_date);
            } else {
                tracing::warn!("No next date with games found");
            }

            // Hide loading indicator
            if let Some(page) = params.current_page.as_mut() {
                page.hide_loading();
            }
            *params.last_date_navigation = Instant::now();
        }
    } else {
        // Handle regular key events (without modifiers)
        match params.key_event.code {
            KeyCode::Char('q') => {
                tracing::info!("Quit requested");
                return Ok(true); // Signal to quit
            }
            KeyCode::Char('r') => {
                // Check if auto-refresh is disabled - ignore manual refresh too
                if let Some(page) = params.current_page.as_ref()
                    && page.is_auto_refresh_disabled()
                {
                    tracing::info!("Manual refresh ignored - auto-refresh is disabled");
                    return Ok(false); // Skip refresh when auto-refresh is disabled
                }

                // Check if current date is historical - don't refresh historical data
                if let Some(date) = params.current_date
                    && is_historical_date(date)
                {
                    tracing::info!("Manual refresh skipped for historical date: {}", date);
                    return Ok(false); // Skip refresh for historical dates
                }

                if params.last_manual_refresh.elapsed() >= Duration::from_secs(15) {
                    tracing::info!("Manual refresh requested");
                    *params.needs_refresh = true;
                    *params.last_manual_refresh = Instant::now();
                }
            }
            KeyCode::Left => {
                if params.last_page_change.elapsed() >= Duration::from_millis(200) {
                    if let Some(page) = params.current_page.as_mut() {
                        page.previous_page();
                        *params.needs_render = true;
                    }
                    *params.last_page_change = Instant::now();
                }
            }
            KeyCode::Right => {
                if params.last_page_change.elapsed() >= Duration::from_millis(200) {
                    if let Some(page) = params.current_page.as_mut() {
                        page.next_page();
                        *params.needs_render = true;
                    }
                    *params.last_page_change = Instant::now();
                }
            }
            _ => {}
        }
    }

    Ok(false) // Continue running
}