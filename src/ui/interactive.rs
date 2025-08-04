//! Interactive UI module for the liiga_teletext application
//!
//! This module contains the main interactive UI loop and all UI-related helper functions.
//! It handles user input, screen updates, page creation, and the main application flow.

use crate::data_fetcher::cache::{
    has_live_games_from_game_data, invalidate_cache_for_games_near_start_time,
};
use crate::data_fetcher::{GameData, fetch_liiga_data, is_historical_date};
use crate::error::AppError;
use crate::teletext_ui::{GameResultData, ScoreType, TeletextPage};
use chrono::{Datelike, Local, NaiveDate, Utc};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::stdout;
use std::time::{Duration, Instant};
use tracing;

// Teletext page constants (removed unused constants)

// UI timing constants (removed unused constants)

/// Formats a date string for display in Finnish format (DD.MM.)
pub fn format_date_for_display(date_str: &str) -> String {
    // Parse the date using chrono for better error handling
    match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        Ok(date) => date.format("%d.%m.").to_string(),
        Err(_) => date_str.to_string(), // Fallback if parsing fails
    }
}

/// Gets the appropriate subheader based on the game series type
fn get_subheader(games: &[GameData]) -> String {
    if games.is_empty() {
        return "SM-LIIGA".to_string();
    }
    // Priority: PLAYOFFS > PLAYOUT-OTTELUT > LIIGAKARSINTA > HARJOITUSOTTELUT > RUNKOSARJA
    let mut priority = 4; // Default to RUNKOSARJA
    for game in games {
        let serie_lower = game.serie.to_ascii_lowercase();
        let current_priority = match serie_lower.as_str() {
            "playoffs" => 0,
            "playout" => 1,
            "qualifications" => 2,
            "valmistavat_ottelut" | "practice" => 3,
            _ => 4,
        };
        if current_priority < priority {
            priority = current_priority;
            if priority == 0 {
                break;
            } // Found highest priority
        }
    }

    match priority {
        0 => "PLAYOFFS".to_string(),
        1 => "PLAYOUT-OTTELUT".to_string(),
        2 => "LIIGAKARSINTA".to_string(),
        3 => "HARJOITUSOTTELUT".to_string(),
        _ => "RUNKOSARJA".to_string(),
    }
}

/// Calculates a hash of the games data for change detection
fn calculate_games_hash(games: &[GameData]) -> u64 {
    let mut hasher = DefaultHasher::new();

    for game in games {
        game.home_team.hash(&mut hasher);
        game.away_team.hash(&mut hasher);
        game.result.hash(&mut hasher);
        game.time.hash(&mut hasher);
        game.score_type.hash(&mut hasher);
        game.is_overtime.hash(&mut hasher);
        game.is_shootout.hash(&mut hasher);
        game.serie.hash(&mut hasher);
        game.played_time.hash(&mut hasher);
        game.start.hash(&mut hasher);

        // Hash goal events for change detection
        for goal in &game.goal_events {
            goal.scorer_player_id.hash(&mut hasher);
            goal.scorer_name.hash(&mut hasher);
            goal.minute.hash(&mut hasher);
            goal.home_team_score.hash(&mut hasher);
            goal.away_team_score.hash(&mut hasher);
            goal.is_winning_goal.hash(&mut hasher);
            goal.is_home_team.hash(&mut hasher);
            goal.goal_types.hash(&mut hasher);
        }
    }

    hasher.finish()
}

/// Creates a base TeletextPage with common initialization logic.
/// This helper function reduces code duplication between create_page and create_future_games_page.
async fn create_base_page(
    games: &[GameData],
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
    future_games_header: Option<String>,
    fetched_date: Option<String>,
    current_page: Option<usize>,
) -> TeletextPage {
    let subheader = get_subheader(games);
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        subheader,
        disable_video_links,
        show_footer,
        ignore_height_limit,
    );

    // Set the fetched date if provided
    if let Some(date) = fetched_date {
        page.set_fetched_date(date);
    }

    // Add future games header first if provided
    if let Some(header) = future_games_header {
        page.add_future_games_header(header);
    }

    for game in games {
        page.add_game_result(GameResultData::new(game));
    }

    // Set season countdown if regular season hasn't started yet
    page.set_show_season_countdown(games).await;

    // Set the current page AFTER content is added (so total_pages() is correct)
    if let Some(page_num) = current_page {
        page.set_current_page(page_num);
    }

    page
}

/// Creates a TeletextPage for regular games
pub async fn create_page(
    games: &[GameData],
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
    fetched_date: Option<String>,
    current_page: Option<usize>,
) -> TeletextPage {
    create_base_page(
        games,
        disable_video_links,
        show_footer,
        ignore_height_limit,
        None,
        fetched_date,
        current_page,
    )
    .await
}

/// Validates if a game is in the future by checking both time and start fields.
/// Returns true if the game has a non-empty time field and a valid future start date.
fn is_future_game(game: &GameData) -> bool {
    // Check if time field is non-empty (indicates scheduled game)
    if game.time.is_empty() {
        return false;
    }

    // Check if start field contains a valid future date
    if game.start.is_empty() {
        return false;
    }

    // Parse the start date to validate it's in the future
    // Expected format: YYYY-MM-DDThh:mm:ssZ
    match chrono::DateTime::parse_from_rfc3339(&game.start) {
        Ok(game_start) => {
            let now = chrono::Utc::now();
            let is_future = game_start > now;

            if !is_future {
                tracing::debug!(
                    "Game start time {} is not in the future (current: {})",
                    game_start,
                    now
                );
            }

            is_future
        }
        Err(e) => {
            tracing::warn!("Failed to parse game start time '{}': {}", game.start, e);
            false
        }
    }
}

/// Checks if a game is scheduled to start within the next few minutes or has recently started
fn is_game_near_start_time(game: &GameData) -> bool {
    if game.score_type != ScoreType::Scheduled || game.start.is_empty() {
        return false;
    }

    match chrono::DateTime::parse_from_rfc3339(&game.start) {
        Ok(game_start) => {
            let now = chrono::Utc::now();
            let time_diff = now.signed_duration_since(game_start);

            // Extended window: Check if game should start within the next 5 minutes or started within the last 10 minutes
            // This is more aggressive to catch games that should have started but haven't updated their status yet
            let is_near_start = time_diff >= chrono::Duration::minutes(-5)
                && time_diff <= chrono::Duration::minutes(10);

            if is_near_start {
                tracing::debug!(
                    "Game near start time: {} vs {} - start: {}, time_diff: {:?}",
                    game.home_team,
                    game.away_team,
                    game_start,
                    time_diff
                );
            }

            is_near_start
        }
        Err(e) => {
            tracing::warn!("Failed to parse game start time '{}': {}", game.start, e);
            false
        }
    }
}

/// Creates a TeletextPage for future games if the games are scheduled.
/// Returns Some(TeletextPage) if the games are future games, None otherwise.
pub async fn create_future_games_page(
    games: &[GameData],
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
    show_future_header: bool,
    fetched_date: Option<String>,
    current_page: Option<usize>,
) -> Option<TeletextPage> {
    // Check if these are future games by validating both time and start fields
    if !games.is_empty() && is_future_game(&games[0]) {
        // Extract date from the first game's start field (assuming format YYYY-MM-DDThh:mm:ssZ)
        let start_str = &games[0].start;
        let date_str = start_str.split('T').next().unwrap_or("");
        let formatted_date = format_date_for_display(date_str);

        tracing::debug!(
            "First game serie: '{}', subheader: '{}'",
            games[0].serie,
            get_subheader(games)
        );

        let future_games_header = if show_future_header {
            Some(format!("Seuraavat ottelut {formatted_date}"))
        } else {
            None
        };
        let mut page = create_base_page(
            games,
            disable_video_links,
            show_footer,
            ignore_height_limit,
            future_games_header,
            fetched_date, // Pass the fetched date to show it in the header
            current_page,
        )
        .await;

        // Set auto-refresh disabled for scheduled games
        page.set_auto_refresh_disabled(true);

        Some(page)
    } else {
        None
    }
}

/// Checks if the given key event matches the date navigation shortcut.
/// Uses Shift + Left/Right for all platforms (works reliably in all terminals)
fn is_date_navigation_key(key_event: &crossterm::event::KeyEvent, is_left: bool) -> bool {
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
/// This prevents navigation to previous season games via arrow keys.
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

    // If date is from a previous year, it's definitely previous season
    if date_year < current_year {
        return true;
    }

    // If same year, check hockey season logic
    if date_year == current_year {
        // Hockey season: September-February (regular), March-May (playoffs/playout)
        // Off-season: June-August

        // If we're in new regular season (September-December) and date is from previous season
        // (January-August), it's from the previous season
        if (9..=12).contains(&current_month) && date_month <= 8 {
            return true;
        }

        // If we're in early regular season (January-February) and date is from off-season
        // (June-August), it's from the previous season
        if (1..=2).contains(&current_month) && (6..=8).contains(&date_month) {
            return true;
        }
    }

    false
}

/// Finds the previous date with games by checking dates going backwards.
/// Returns None if no games are found within the current season or a reasonable time range.
/// Prevents navigation to previous season games for better UX.
async fn find_previous_date_with_games(current_date: &str) -> Option<String> {
    let current_parsed = match chrono::NaiveDate::parse_from_str(current_date, "%Y-%m-%d") {
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

            // Add timeout to the fetch operation (shorter timeout for faster navigation)
            let fetch_future = fetch_liiga_data(Some(date_string.clone()));
            let timeout_duration = tokio::time::Duration::from_secs(5);

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
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
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
    let current_parsed = match chrono::NaiveDate::parse_from_str(current_date, "%Y-%m-%d") {
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

            // Add timeout to the fetch operation (shorter timeout for faster navigation)
            let fetch_future = fetch_liiga_data(Some(date_string.clone()));
            let timeout_duration = tokio::time::Duration::from_secs(5);

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
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    }

    tracing::info!(
        "No next date with games found within search range from {}",
        current_date
    );
    None
}

/// Monitor cache usage and log statistics for long-running sessions
async fn monitor_cache_usage() {
    // The LRU cache automatically manages memory by evicting least recently used entries
    // when it reaches capacity. We just need to log the current state for monitoring.
    use crate::data_fetcher::cache::{get_all_cache_stats, get_detailed_cache_debug_info};

    let stats = get_all_cache_stats().await;

    tracing::debug!(
        "Cache status - Player: {}/{} ({}%), Tournament: {}/{} ({}%), Detailed Game: {}/{} ({}%), Goal Events: {}/{} ({}%), HTTP Response: {}/{} ({}%)",
        stats.player_cache.size,
        stats.player_cache.capacity,
        if stats.player_cache.capacity > 0 {
            (stats.player_cache.size * 100) / stats.player_cache.capacity
        } else {
            0
        },
        stats.tournament_cache.size,
        stats.tournament_cache.capacity,
        if stats.tournament_cache.capacity > 0 {
            (stats.tournament_cache.size * 100) / stats.tournament_cache.capacity
        } else {
            0
        },
        stats.detailed_game_cache.size,
        stats.detailed_game_cache.capacity,
        if stats.detailed_game_cache.capacity > 0 {
            (stats.detailed_game_cache.size * 100) / stats.detailed_game_cache.capacity
        } else {
            0
        },
        stats.goal_events_cache.size,
        stats.goal_events_cache.capacity,
        if stats.goal_events_cache.capacity > 0 {
            (stats.goal_events_cache.size * 100) / stats.goal_events_cache.capacity
        } else {
            0
        },
        stats.http_response_cache.size,
        stats.http_response_cache.capacity,
        if stats.http_response_cache.capacity > 0 {
            (stats.http_response_cache.size * 100) / stats.http_response_cache.capacity
        } else {
            0
        }
    );

    // Log detailed cache information for debugging if needed
    if tracing::enabled!(tracing::Level::TRACE) {
        let debug_info = get_detailed_cache_debug_info().await;
        tracing::trace!("Detailed cache debug info: {}", debug_info);
    }

    // The LRU cache implementation automatically handles memory management,
    // so we don't need manual cleanup logic anymore.
    // This ensures that the oldest/least recently used entries are always removed first.
}

/// Runs the interactive UI with adaptive polling and change detection
pub async fn run_interactive_ui(
    date: Option<String>,
    disable_links: bool,
    debug_mode: bool,
    min_refresh_interval: Option<u64>,
) -> Result<(), AppError> {
    let mut stdout = stdout();

    if !debug_mode {
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen)?;
    }
    // Timer management with adaptive intervals
    let mut last_manual_refresh = Instant::now()
        .checked_sub(Duration::from_secs(15))
        .unwrap_or_else(Instant::now);
    let mut last_auto_refresh = Instant::now()
        .checked_sub(Duration::from_secs(10))
        .unwrap_or_else(Instant::now);
    let mut last_page_change = Instant::now()
        .checked_sub(Duration::from_millis(200))
        .unwrap_or_else(Instant::now);
    let mut last_date_navigation = Instant::now()
        .checked_sub(Duration::from_millis(250))
        .unwrap_or_else(Instant::now);
    let mut last_resize = Instant::now()
        .checked_sub(Duration::from_millis(500))
        .unwrap_or_else(Instant::now);

    // State management
    let mut needs_refresh = true;
    let mut needs_render = false;
    let mut current_page: Option<TeletextPage> = None;
    let mut pending_resize = false;
    let mut resize_timer = Instant::now();
    // Preserved page number for restoration after refresh - initially None for first run
    #[allow(unused_assignments)]
    let mut preserved_page_for_restoration: Option<usize> = None;

    // Date navigation state - track the current date being displayed
    let mut current_date = date;

    // Change detection - track data changes to avoid unnecessary re-renders
    let mut last_games_hash = 0u64;
    let mut last_games = Vec::new();

    // Adaptive polling configuration
    let mut last_activity = Instant::now();

    // Cache monitoring tracking
    let mut cache_monitor_timer = Instant::now();
    const CACHE_MONITOR_INTERVAL: Duration = Duration::from_secs(300); // 5 minutes

    // Rate limiting protection
    let mut rate_limit_backoff = Duration::from_secs(0);
    let mut last_rate_limit_hit = Instant::now()
        .checked_sub(Duration::from_secs(60))
        .unwrap_or_else(Instant::now);

    loop {
        // Adaptive polling interval based on activity
        let time_since_activity = last_activity.elapsed();
        let poll_interval = if time_since_activity < Duration::from_secs(5) {
            Duration::from_millis(50) // Active: 50ms (smooth interaction)
        } else if time_since_activity < Duration::from_secs(30) {
            Duration::from_millis(200) // Semi-active: 200ms (good responsiveness)
        } else {
            Duration::from_millis(500) // Idle: 500ms (conserve CPU)
        };

        // Check for auto-refresh with better logic and rate limiting protection
        let auto_refresh_interval = if has_live_games_from_game_data(&last_games) {
            Duration::from_secs(15) // Increased from 8 to 15 seconds for live games
        } else if last_games.iter().any(is_game_near_start_time) {
            Duration::from_secs(30) // Increased from 10 to 30 seconds for games near start time
        } else {
            Duration::from_secs(60) // Standard interval for completed/scheduled games
        };

        // Rate limiting protection: don't refresh too frequently if we have many games
        let game_count = last_games.len();
        let min_interval_between_refreshes = if let Some(user_interval) = min_refresh_interval {
            Duration::from_secs(user_interval) // Use user-specified interval
        } else if game_count >= 6 {
            Duration::from_secs(30) // Minimum 30 seconds between refreshes for 6+ games
        } else if game_count >= 4 {
            Duration::from_secs(20) // Minimum 20 seconds between refreshes for 4-5 games
        } else {
            Duration::from_secs(10) // Minimum 10 seconds between refreshes for 1-3 games
        };

        // Debug logging for rate limit backoff enforcement
        if rate_limit_backoff > Duration::from_secs(0) {
            let backoff_remaining =
                rate_limit_backoff.saturating_sub(last_rate_limit_hit.elapsed());
            if backoff_remaining > Duration::from_secs(0) {
                tracing::debug!(
                    "Rate limit backoff active: {}s remaining (total backoff: {}s, elapsed since rate limit: {}s)",
                    backoff_remaining.as_secs(),
                    rate_limit_backoff.as_secs(),
                    last_rate_limit_hit.elapsed().as_secs()
                );
            }
        }

        if !needs_refresh
            && !last_games.is_empty()
            && last_auto_refresh.elapsed() >= auto_refresh_interval
            && last_auto_refresh.elapsed() >= min_interval_between_refreshes
            && last_rate_limit_hit.elapsed() >= rate_limit_backoff
        // Respect rate limit backoff
        {
            // Check if there are ongoing games - if so, always refresh
            let has_ongoing_games = has_live_games_from_game_data(&last_games);
            // Compute current state directly from last_games (don't rely on stale all_games_scheduled)
            let all_scheduled = !last_games.is_empty() && last_games.iter().all(is_future_game);
            let time_elapsed = last_auto_refresh.elapsed();

            // Enhanced logging for better debugging
            tracing::debug!(
                "Auto-refresh check: has_ongoing_games={}, all_scheduled={}, time_elapsed={:?}, games_count={}, auto_refresh_interval={:?}",
                has_ongoing_games,
                all_scheduled,
                time_elapsed,
                last_games.len(),
                auto_refresh_interval
            );

            // Log rate limit backoff status
            if rate_limit_backoff > Duration::from_secs(0) {
                let backoff_remaining =
                    rate_limit_backoff.saturating_sub(last_rate_limit_hit.elapsed());
                if backoff_remaining > Duration::from_secs(0) {
                    tracing::debug!(
                        "Auto-refresh skipped due to rate limit backoff: {}s remaining (total backoff: {}s)",
                        backoff_remaining.as_secs(),
                        rate_limit_backoff.as_secs()
                    );
                } else {
                    tracing::debug!(
                        "Rate limit backoff period completed: {}s elapsed since last rate limit hit",
                        last_rate_limit_hit.elapsed().as_secs()
                    );
                }
            }

            // Log individual game states for debugging
            for (i, game) in last_games.iter().enumerate() {
                tracing::debug!(
                    "Game {}: {} vs {} - score_type={:?}, time='{}', start='{}'",
                    i + 1,
                    game.home_team,
                    game.away_team,
                    game.score_type,
                    game.time,
                    game.start
                );
            }

            // Don't auto-refresh for historical dates
            if let Some(ref date) = current_date {
                if is_historical_date(date) {
                    tracing::debug!("Auto-refresh skipped for historical date: {}", date);
                } else if has_ongoing_games {
                    needs_refresh = true;
                    tracing::info!("Auto-refresh triggered for ongoing games");
                } else if !all_scheduled {
                    // Only refresh if not all games are scheduled (i.e., some are finished)
                    needs_refresh = true;
                    tracing::debug!(
                        "Auto-refresh triggered for non-scheduled games (mixed game states)"
                    );
                } else {
                    // Enhanced check for games that might have started
                    let has_recently_started_games = last_games.iter().any(is_game_near_start_time);

                    if has_recently_started_games {
                        // Aggressively invalidate cache for starting games to ensure fresh data
                        if let Some(ref date) = current_date {
                            invalidate_cache_for_games_near_start_time(date).await;
                        }
                        needs_refresh = true;
                        tracing::info!("Auto-refresh triggered for games that may have started");
                    } else {
                        tracing::debug!(
                            "Auto-refresh skipped - all games are scheduled for future"
                        );
                    }
                }
            } else if has_ongoing_games {
                needs_refresh = true;
                tracing::info!("Auto-refresh triggered for ongoing games");
            } else if !all_scheduled {
                needs_refresh = true;
                tracing::debug!(
                    "Auto-refresh triggered for non-scheduled games (mixed game states)"
                );
            } else {
                // Enhanced check for games that might have started (same logic as above)
                let has_recently_started_games = last_games.iter().any(is_game_near_start_time);

                if has_recently_started_games {
                    // Aggressively invalidate cache for starting games to ensure fresh data
                    if let Some(ref date) = current_date {
                        invalidate_cache_for_games_near_start_time(date).await;
                    }
                    needs_refresh = true;
                    tracing::info!("Auto-refresh triggered for games that may have started");
                } else {
                    tracing::debug!("Auto-refresh skipped - all games are scheduled for future");
                }
            }
        }

        // Data fetching with change detection
        if needs_refresh {
            tracing::debug!("Fetching new data");

            // Check if there are ongoing games to avoid showing loading screen during auto-refresh
            let has_ongoing_games = has_live_games_from_game_data(&last_games);

            // Show loading indicator only in specific cases:
            // 1. For historical dates (always show loading - these are slow)
            // 2. For initial load (when current_page is None)
            // Skip loading for all current date refreshes (both auto and manual)
            let should_show_loading = if let Some(ref date) = current_date {
                // Only show loading for historical dates
                is_historical_date(date)
            } else {
                // Show loading for initial load when no specific date is requested
                current_page.is_none()
            };

            // Show auto-refresh indicator whenever auto-refresh is active
            // This should match the auto-refresh logic above
            let all_scheduled = !last_games.is_empty() && last_games.iter().all(is_future_game);
            let should_show_indicator = if let Some(ref date) = current_date {
                !is_historical_date(date) && (has_ongoing_games || !all_scheduled)
            } else {
                has_ongoing_games || !all_scheduled
            };

            if should_show_indicator {
                if let Some(page) = &mut current_page {
                    page.show_auto_refresh_indicator();
                    needs_render = true;
                }
            }

            // Always preserve the current page number before refresh, regardless of loading screen
            let preserved_page = current_page
                .as_ref()
                .map(|existing_page| existing_page.get_current_page());
            preserved_page_for_restoration = preserved_page;

            if should_show_loading {
                let mut loading_page = TeletextPage::new(
                    221,
                    "JÄÄKIEKKO".to_string(),
                    "SM-LIIGA".to_string(),
                    disable_links,
                    true,
                    false,
                );

                if let Some(ref date) = current_date {
                    if is_historical_date(date) {
                        loading_page.add_error_message(&format!(
                            "Haetaan historiallista dataa päivälle {}...",
                            format_date_for_display(date)
                        ));
                        loading_page.add_error_message("Tämä voi kestää hetken, odotathan...");
                    } else {
                        loading_page.add_error_message(&format!(
                            "Haetaan otteluita päivälle {}...",
                            format_date_for_display(date)
                        ));
                    }
                } else {
                    loading_page.add_error_message("Haetaan päivän otteluita...");
                }

                current_page = Some(loading_page);
                needs_render = true;

                // Render the loading page immediately
                if needs_render {
                    if let Some(page) = &current_page {
                        page.render_buffered(&mut stdout)?;
                    }
                    needs_render = false;
                }
            } else {
                tracing::debug!("Skipping loading screen due to ongoing games");
            }

            // Add timeout for auto-refresh to prevent hanging
            let fetch_future = fetch_liiga_data(current_date.clone());
            let timeout_duration = tokio::time::Duration::from_secs(15); // Shorter timeout for auto-refresh

            let (games, had_error, fetched_date, should_retry) = match tokio::time::timeout(
                timeout_duration,
                fetch_future,
            )
            .await
            {
                Ok(fetch_result) => match fetch_result {
                    Ok((games, fetched_date)) => {
                        // Log successful recovery if we had previous errors
                        tracing::debug!("Auto-refresh successful: fetched {} games", games.len());
                        (games, false, fetched_date, false)
                    }
                    Err(e) => {
                        // Enhanced error logging with detailed information
                        tracing::error!("Auto-refresh failed: {}", e);
                        tracing::error!(
                            "Error details - Type: {}, Current date: {:?}, Has ongoing games: {}",
                            std::any::type_name_of_val(&e),
                            current_date,
                            has_live_games_from_game_data(&last_games)
                        );

                        // Log specific error context for debugging
                        match &e {
                            crate::error::AppError::NetworkTimeout { url } => {
                                tracing::warn!(
                                    "Auto-refresh timeout for URL: {}, will retry on next cycle",
                                    url
                                );
                            }
                            crate::error::AppError::NetworkConnection { url, message } => {
                                tracing::warn!(
                                    "Auto-refresh connection error for URL: {}, details: {}, will retry on next cycle",
                                    url,
                                    message
                                );
                            }
                            crate::error::AppError::ApiServerError {
                                status,
                                message,
                                url,
                            } => {
                                tracing::warn!(
                                    "Auto-refresh server error: HTTP {} - {} (URL: {}), will retry on next cycle",
                                    status,
                                    message,
                                    url
                                );
                            }
                            crate::error::AppError::ApiServiceUnavailable {
                                status,
                                message,
                                url,
                            } => {
                                tracing::warn!(
                                    "Auto-refresh service unavailable: HTTP {} - {} (URL: {}), will retry on next cycle",
                                    status,
                                    message,
                                    url
                                );
                            }
                            crate::error::AppError::ApiRateLimit { message, url } => {
                                tracing::warn!(
                                    "Auto-refresh rate limited: {} (URL: {}), will retry on next cycle",
                                    message,
                                    url
                                );

                                // Implement exponential backoff for rate limits
                                last_rate_limit_hit = Instant::now();
                                if rate_limit_backoff.is_zero() {
                                    rate_limit_backoff = Duration::from_secs(60); // Start with 1 minute
                                } else {
                                    // Double the backoff time, but cap at 10 minutes
                                    rate_limit_backoff = std::cmp::min(
                                        rate_limit_backoff * 2,
                                        Duration::from_secs(600),
                                    );
                                }
                                tracing::info!(
                                    "Rate limit backoff set to {:?} seconds",
                                    rate_limit_backoff.as_secs()
                                );
                            }
                            _ => {
                                tracing::warn!(
                                    "Auto-refresh error: {}, will retry on next cycle",
                                    e
                                );
                            }
                        }

                        // Graceful degradation: continue with existing data instead of showing error page
                        tracing::info!(
                            "Continuing with existing data ({} games) due to auto-refresh failure",
                            last_games.len()
                        );

                        // Return empty games but indicate we should retry (don't update last_auto_refresh)
                        (Vec::new(), true, String::new(), true)
                    }
                },
                Err(_) => {
                    // Timeout occurred during auto-refresh
                    tracing::warn!(
                        "Auto-refresh timeout after {:?}, continuing with existing data ({} games, {} live games)",
                        auto_refresh_interval,
                        last_games.len(),
                        last_games
                            .iter()
                            .filter(|g| g.score_type == ScoreType::Ongoing)
                            .count()
                    );
                    (Vec::new(), true, String::new(), true)
                }
            };

            // Reset rate limit backoff on successful refresh
            if !games.is_empty() && rate_limit_backoff > Duration::from_secs(0) {
                // Gradually reduce backoff on successful requests
                rate_limit_backoff = std::cmp::max(rate_limit_backoff / 2, Duration::from_secs(0));
                if rate_limit_backoff.is_zero() {
                    tracing::info!("Rate limit backoff reset to zero after successful request");
                } else {
                    tracing::debug!(
                        "Rate limit backoff reduced to {:?} seconds after successful request",
                        rate_limit_backoff.as_secs()
                    );
                }
            }

            // Update current_date to track the actual date being displayed
            if !had_error && !fetched_date.is_empty() {
                current_date = Some(fetched_date.clone());
                tracing::debug!("Updated current_date to: {:?}", current_date);
            }

            // Change detection using a simple hash of game data
            let games_hash = calculate_games_hash(&games);
            let data_changed = games_hash != last_games_hash;

            if data_changed {
                tracing::debug!("Data changed, updating UI");
                // Log specific changes for live games to help debug game clock updates
                if !last_games.is_empty() && games.len() == last_games.len() {
                    for (i, (new_game, old_game)) in games.iter().zip(last_games.iter()).enumerate()
                    {
                        if new_game.played_time != old_game.played_time
                            && new_game.score_type == ScoreType::Ongoing
                        {
                            tracing::info!(
                                "Game clock update detected: Game {} - {} vs {} - time changed from {}s to {}s",
                                i + 1,
                                new_game.home_team,
                                new_game.away_team,
                                old_game.played_time,
                                new_game.played_time
                            );
                        }
                    }
                }

                // Only create a new page if we didn't have an error and data changed
                if !had_error {
                    // Restore the preserved page number
                    if let Some(preserved_page_for_restoration) = preserved_page_for_restoration {
                        let mut page = create_page(
                            &games,
                            disable_links,
                            true,
                            false,
                            Some(fetched_date.clone()),
                            Some(preserved_page_for_restoration),
                        )
                        .await;

                        // Disable auto-refresh for historical dates
                        if let Some(ref date) = current_date {
                            if is_historical_date(date) {
                                page.set_auto_refresh_disabled(true);
                            }
                        }

                        current_page = Some(page);
                    } else {
                        let page = if games.is_empty() {
                            let mut error_page = TeletextPage::new(
                                221,
                                "JÄÄKIEKKO".to_string(),
                                "SM-LIIGA".to_string(),
                                disable_links,
                                true,
                                false,
                            );
                            // Use UTC internally, convert to local time for date formatting
                            let today = Utc::now()
                                .with_timezone(&Local)
                                .format("%Y-%m-%d")
                                .to_string();
                            if fetched_date == today {
                                error_page.add_error_message("Ei otteluita tänään");
                            } else {
                                error_page.add_error_message(&format!(
                                    "Ei otteluita {} päivälle",
                                    format_date_for_display(&fetched_date)
                                ));
                            }
                            error_page
                        } else {
                            // Try to create a future games page, fall back to regular page if not future games
                            let show_future_header = current_date.is_none();
                            match create_future_games_page(
                                &games,
                                disable_links,
                                true,
                                false,
                                show_future_header,
                                Some(fetched_date.clone()),
                                None,
                            )
                            .await
                            {
                                Some(page) => page,
                                None => {
                                    let mut page = create_page(
                                        &games,
                                        disable_links,
                                        true,
                                        false,
                                        Some(fetched_date.clone()),
                                        None,
                                    )
                                    .await;

                                    // Disable auto-refresh for historical dates
                                    if let Some(ref date) = current_date {
                                        if is_historical_date(date) {
                                            page.set_auto_refresh_disabled(true);
                                        }
                                    }

                                    page
                                }
                            }
                        };

                        current_page = Some(page);
                    }

                    needs_render = true;
                }
            } else if had_error {
                tracing::debug!(
                    "Auto-refresh failed but no data changes detected, continuing with existing UI"
                );
            } else {
                // Track ongoing games with static time to confirm API limitations
                let ongoing_games: Vec<_> = games
                    .iter()
                    .enumerate()
                    .filter(|(_, game)| game.score_type == ScoreType::Ongoing)
                    .collect();

                if !ongoing_games.is_empty() {
                    tracing::debug!(
                        "No data changes detected despite {} ongoing game(s): {}",
                        ongoing_games.len(),
                        ongoing_games
                            .iter()
                            .map(|(i, game)| format!(
                                "{}. {} vs {} ({}s)",
                                i + 1,
                                game.home_team,
                                game.away_team,
                                game.played_time
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }

                // Check if all games are scheduled (future games) - only relevant if no ongoing games
                let has_ongoing_games = has_live_games_from_game_data(&games);
                let all_scheduled = !games.is_empty() && games.iter().all(is_future_game);

                if all_scheduled && !has_ongoing_games {
                    tracing::info!("All games are scheduled - auto-refresh disabled");
                } else if has_ongoing_games {
                    tracing::info!("Ongoing games detected - auto-refresh enabled");
                }

                tracing::debug!("No data changes detected, skipping UI update");
            }

            // If we showed a loading screen but data didn't change, we still need to restore pagination
            if !data_changed && !had_error && preserved_page_for_restoration.is_some() {
                if let Some(ref current) = current_page {
                    // Check if current page is a loading page by checking if it has error messages
                    if current.has_error_messages() {
                        if let Some(preserved_page_for_restoration) = preserved_page_for_restoration
                        {
                            let games_to_use = if games.is_empty() {
                                &last_games
                            } else {
                                &games
                            };
                            let mut page = create_page(
                                games_to_use,
                                disable_links,
                                true,
                                false,
                                Some(fetched_date.clone()),
                                Some(preserved_page_for_restoration),
                            )
                            .await;

                            // Disable auto-refresh for historical dates
                            if let Some(ref date) = current_date {
                                if is_historical_date(date) {
                                    page.set_auto_refresh_disabled(true);
                                }
                            }

                            current_page = Some(page);
                            needs_render = true;
                        }
                    }
                }
            }

            // Hide auto-refresh indicator after data is fetched
            if let Some(page) = &mut current_page {
                page.hide_auto_refresh_indicator();
                needs_render = true;
            }

            // Update change detection variables
            last_games_hash = games_hash;
            last_games = games.clone();

            needs_refresh = false;

            // Only update last_auto_refresh if we shouldn't retry
            // This ensures that failed auto-refresh attempts will be retried on the next cycle
            if !should_retry {
                last_auto_refresh = Instant::now();
                tracing::trace!("Auto-refresh cycle completed successfully");
            } else {
                tracing::debug!(
                    "Auto-refresh failed, will retry on next cycle (not updating last_auto_refresh timer)"
                );
            }
        }

        // Handle pending resize with debouncing
        if pending_resize && resize_timer.elapsed() >= Duration::from_millis(500) {
            tracing::debug!("Handling resize");
            if let Some(page) = &mut current_page {
                page.handle_resize();
                needs_render = true;
            }
            pending_resize = false;
        }

        // Update auto-refresh indicator animation (only when active)
        if let Some(page) = &mut current_page {
            if page.is_auto_refresh_indicator_active() {
                page.update_auto_refresh_animation();
                needs_render = true;
            }
        }

        // Batched UI rendering - only render when necessary
        // Use buffered rendering to minimize flickering
        if needs_render {
            if let Some(page) = &current_page {
                page.render_buffered(&mut stdout)?;
                tracing::debug!("UI rendered with buffering");
            }
            needs_render = false;
        }

        // Event handling with adaptive polling
        if event::poll(poll_interval)? {
            last_activity = Instant::now(); // Reset activity timer

            match event::read()? {
                Event::Key(key_event) => {
                    tracing::debug!(
                        "Key event: {:?}, modifiers: {:?}",
                        key_event.code,
                        key_event.modifiers
                    );

                    // Check for date navigation first (Shift + Arrow keys)
                    if is_date_navigation_key(&key_event, true) {
                        // Shift + Left: Previous date with games
                        if last_date_navigation.elapsed() >= Duration::from_millis(250) {
                            tracing::info!("Previous date navigation requested");
                            tracing::debug!("Current date state: {:?}", current_date);
                            let target_date = get_target_date_for_navigation(&current_date);

                            // Show loading indicator
                            if let Some(page) = &mut current_page {
                                page.show_loading("Etsitään edellisiä otteluita...".to_string());
                                page.render_loading_indicator_only(&mut stdout)?;
                            }

                            tracing::info!(
                                "Searching for previous date with games from: {}",
                                target_date
                            );

                            // Create a task that will update animation while search runs
                            let target_date_clone = target_date.clone();
                            let mut search_task = tokio::spawn(async move {
                                find_previous_date_with_games(&target_date_clone).await
                            });
                            let mut animation_interval =
                                tokio::time::interval(Duration::from_millis(200));

                            let result = loop {
                                tokio::select! {
                                    search_result = &mut search_task => {
                                        match search_result {
                                            Ok(date_option) => {
                                                break date_option;
                                            }
                                            Err(join_error) => {
                                                tracing::error!(
                                                    "Previous date search task failed: {}",
                                                    join_error
                                                );
                                                break None;
                                            }
                                        }
                                    }
                                    _ = animation_interval.tick() => {
                                        if let Some(page) = &mut current_page {
                                            page.update_loading_animation();
                                            page.render_loading_indicator_only(&mut stdout)?;
                                        }
                                    }
                                }
                            };

                            if let Some(prev_date) = result {
                                current_date = Some(prev_date.clone());
                                needs_refresh = true;
                                tracing::info!("Navigated to previous date: {}", prev_date);
                            } else {
                                tracing::warn!("No previous date with games found");
                            }

                            // Hide loading indicator
                            if let Some(page) = &mut current_page {
                                page.hide_loading();
                            }
                            last_date_navigation = Instant::now();
                        }
                    } else if is_date_navigation_key(&key_event, false) {
                        // Shift + Right: Next date with games
                        if last_date_navigation.elapsed() >= Duration::from_millis(250) {
                            tracing::info!("Next date navigation requested");
                            tracing::debug!("Current date state: {:?}", current_date);
                            let target_date = get_target_date_for_navigation(&current_date);

                            // Show loading indicator
                            if let Some(page) = &mut current_page {
                                page.show_loading("Etsitään seuraavia otteluita...".to_string());
                                page.render_loading_indicator_only(&mut stdout)?;
                            }

                            tracing::info!(
                                "Searching for next date with games from: {}",
                                target_date
                            );

                            // Create a task that will update animation while search runs
                            let target_date_clone = target_date.clone();
                            let mut search_task = tokio::spawn(async move {
                                find_next_date_with_games(&target_date_clone).await
                            });
                            let mut animation_interval =
                                tokio::time::interval(Duration::from_millis(200));

                            let result = loop {
                                tokio::select! {
                                    search_result = &mut search_task => {
                                        match search_result {
                                            Ok(date_option) => {
                                                break date_option;
                                            }
                                            Err(join_error) => {
                                                tracing::error!(
                                                    "Next date search task failed: {}",
                                                    join_error
                                                );
                                                break None;
                                            }
                                        }
                                    }
                                    _ = animation_interval.tick() => {
                                        if let Some(page) = &mut current_page {
                                            page.update_loading_animation();
                                            page.render_loading_indicator_only(&mut stdout)?;
                                        }
                                    }
                                }
                            };

                            if let Some(next_date) = result {
                                current_date = Some(next_date.clone());
                                needs_refresh = true;
                                tracing::info!("Navigated to next date: {}", next_date);
                            } else {
                                tracing::warn!("No next date with games found");
                            }

                            // Hide loading indicator
                            if let Some(page) = &mut current_page {
                                page.hide_loading();
                            }
                            last_date_navigation = Instant::now();
                        }
                    } else {
                        // Handle regular key events (without modifiers)
                        match key_event.code {
                            KeyCode::Char('q') => {
                                tracing::info!("Quit requested");
                                if !debug_mode {
                                    disable_raw_mode()?;
                                    execute!(stdout, LeaveAlternateScreen)?;
                                }
                                return Ok(());
                            }
                            KeyCode::Char('r') => {
                                // Check if auto-refresh is disabled - ignore manual refresh too
                                if let Some(page) = &current_page {
                                    if page.is_auto_refresh_disabled() {
                                        tracing::info!(
                                            "Manual refresh ignored - auto-refresh is disabled"
                                        );
                                        continue; // Skip refresh when auto-refresh is disabled
                                    }
                                }

                                // Check if current date is historical - don't refresh historical data
                                if let Some(ref date) = current_date {
                                    if is_historical_date(date) {
                                        tracing::info!(
                                            "Manual refresh skipped for historical date: {}",
                                            date
                                        );
                                        continue; // Skip refresh for historical dates
                                    }
                                }

                                if last_manual_refresh.elapsed() >= Duration::from_secs(15) {
                                    tracing::info!("Manual refresh requested");
                                    needs_refresh = true;
                                    last_manual_refresh = Instant::now();
                                }
                            }
                            KeyCode::Left => {
                                if last_page_change.elapsed() >= Duration::from_millis(200) {
                                    if let Some(page) = &mut current_page {
                                        page.previous_page();
                                        needs_render = true;
                                    }
                                    last_page_change = Instant::now();
                                }
                            }
                            KeyCode::Right => {
                                if last_page_change.elapsed() >= Duration::from_millis(200) {
                                    if let Some(page) = &mut current_page {
                                        page.next_page();
                                        needs_render = true;
                                    }
                                    last_page_change = Instant::now();
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Event::Resize(_, _) => {
                    tracing::debug!("Resize event");
                    // Debounce resize events
                    if last_resize.elapsed() >= Duration::from_millis(500) {
                        resize_timer = Instant::now();
                        pending_resize = true;
                        last_resize = Instant::now();
                    }
                }
                _ => {}
            }
        }

        // Periodic cache monitoring for long-running sessions
        if cache_monitor_timer.elapsed() >= CACHE_MONITOR_INTERVAL {
            tracing::debug!("Monitoring cache usage");
            monitor_cache_usage().await;
            cache_monitor_timer = Instant::now();
        }

        // Only sleep if we're in idle mode to avoid unnecessary delays
        if poll_interval >= Duration::from_millis(200) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_game(home_team: &str, away_team: &str) -> GameData {
        GameData {
            home_team: home_team.to_string(),
            away_team: away_team.to_string(),
            time: "18:30".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T18:30:00Z".to_string(),
        }
    }

    #[test]
    fn test_calculate_games_hash() {
        let games1 = vec![
            create_test_game("Team A", "Team B"),
            create_test_game("Team C", "Team D"),
        ];

        let games2 = vec![
            create_test_game("Team A", "Team B"),
            create_test_game("Team C", "Team D"),
        ];

        let games3 = vec![
            create_test_game("Team A", "Team B"),
            create_test_game("Team E", "Team F"), // Different game
        ];

        let hash1 = calculate_games_hash(&games1);
        let hash2 = calculate_games_hash(&games2);
        let hash3 = calculate_games_hash(&games3);

        // Same games should have same hash
        assert_eq!(hash1, hash2);

        // Different games should have different hash
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_calculate_games_hash_empty() {
        let empty_games: Vec<GameData> = vec![];
        let _hash = calculate_games_hash(&empty_games);

        // Should not panic - any hash value is valid for empty games
    }

    #[test]
    fn test_format_date_for_display() {
        assert_eq!(format_date_for_display("2024-01-15"), "15.01.");
        assert_eq!(format_date_for_display("2024-12-31"), "31.12.");

        // Test invalid date - should return original string
        assert_eq!(format_date_for_display("invalid-date"), "invalid-date");
    }

    #[test]
    fn test_is_future_game() {
        // Create a future game
        let future_game = GameData {
            home_team: "Team A".to_string(),
            away_team: "Team B".to_string(),
            time: "18:30".to_string(),
            result: "".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 0,
            start: "2030-01-15T18:30:00Z".to_string(), // Future date
        };

        assert!(is_future_game(&future_game));

        // Create a past game
        let past_game = GameData {
            home_team: "Team A".to_string(),
            away_team: "Team B".to_string(),
            time: "18:30".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2020-01-15T18:30:00Z".to_string(), // Past date
        };

        assert!(!is_future_game(&past_game));
    }
}
