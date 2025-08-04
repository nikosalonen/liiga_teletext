//! Interactive UI module for the liiga_teletext application
//!
//! This module contains the main interactive UI loop and all UI-related helper functions.
//! It handles user input, screen updates, page creation, and the main application flow.

use crate::data_fetcher::cache::has_live_games_from_game_data;
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

/// Represents different tournament series types with explicit priority ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SeriesType {
    /// Highest priority - playoff games
    Playoffs,
    /// Playout games (relegation/promotion)
    Playout,
    /// Qualification tournament
    Qualifications,
    /// Practice/preseason games
    Practice,
    /// Regular season games (lowest priority)
    RegularSeason,
}

impl From<&str> for SeriesType {
    /// Converts a series string from the API to a SeriesType enum
    fn from(serie: &str) -> Self {
        match serie.to_ascii_lowercase().as_str() {
            "playoffs" => SeriesType::Playoffs,
            "playout" => SeriesType::Playout,
            "qualifications" => SeriesType::Qualifications,
            "valmistavat_ottelut" | "practice" => SeriesType::Practice,
            _ => SeriesType::RegularSeason,
        }
    }
}

impl std::fmt::Display for SeriesType {
    /// Returns the display text for the teletext UI subheader
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display_text = match self {
            SeriesType::Playoffs => "PLAYOFFS",
            SeriesType::Playout => "PLAYOUT-OTTELUT",
            SeriesType::Qualifications => "LIIGAKARSINTA",
            SeriesType::Practice => "HARJOITUSOTTELUT",
            SeriesType::RegularSeason => "RUNKOSARJA",
        };
        f.write_str(display_text)
    }
}

/// Gets the appropriate subheader based on the game series type with highest priority
fn get_subheader(games: &[GameData]) -> String {
    if games.is_empty() {
        return "SM-LIIGA".to_string();
    }

    // Find the series type with highest priority (lowest enum value due to Ord implementation)
    games
        .iter()
        .map(|game| SeriesType::from(game.serie.as_str()))
        .min() // Uses the Ord implementation where Playoffs < Playout < ... < RegularSeason
        .unwrap_or(SeriesType::RegularSeason)
        .to_string()
}

/// Determines whether to show loading indicator and auto-refresh indicator
fn determine_indicator_states(
    current_date: &Option<String>,
    last_games: &[GameData],
) -> (bool, bool) {
    let has_ongoing_games = has_live_games_from_game_data(last_games);

    // Show loading indicator only in specific cases
    let should_show_loading = if let Some(date) = current_date {
        // Only show loading for historical dates
        is_historical_date(date)
    } else {
        // Show loading for initial load when no specific date is requested
        true
    };

    // Show auto-refresh indicator whenever auto-refresh is active
    let all_scheduled = !last_games.is_empty() && last_games.iter().all(is_future_game);
    let should_show_indicator = if let Some(date) = current_date {
        !is_historical_date(date) && (has_ongoing_games || !all_scheduled)
    } else {
        has_ongoing_games || !all_scheduled
    };

    (should_show_loading, should_show_indicator)
}

/// Manages loading and auto-refresh indicators for the current page
fn manage_loading_indicators(
    current_page: &mut Option<TeletextPage>,
    should_show_loading: bool,
    should_show_indicator: bool,
    current_date: &Option<String>,
    disable_links: bool,
    compact_mode: bool,
) -> bool {
    let mut needs_render = false;

    if should_show_indicator {
        if let Some(page) = current_page {
            page.show_auto_refresh_indicator();
            needs_render = true;
        }
    }

    if should_show_loading {
        *current_page = Some(create_loading_page(current_date, disable_links, compact_mode));
        needs_render = true;
    } else {
        tracing::debug!("Skipping loading screen due to ongoing games");
    }

    needs_render
}

/// Calculates a hash of the games data for change detection
/// Optimized to focus on essential fields that indicate meaningful changes
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

        // Hash only essential goal event fields for efficient change detection
        // These fields capture the most important changes: new goals, score updates, and timing
        for goal in &game.goal_events {
            goal.scorer_player_id.hash(&mut hasher);
            goal.minute.hash(&mut hasher);
            goal.home_team_score.hash(&mut hasher);
            goal.away_team_score.hash(&mut hasher);
            // Omitted fields for performance:
            // - scorer_name: derived from scorer_player_id via players cache
            // - is_winning_goal: calculated field, can be derived
            // - is_home_team: derived from team comparison
            // - goal_types: less critical for change detection, rarely updated
        }
    }

    hasher.finish()
}

/// Performs change detection and logs detailed information about changes
fn detect_and_log_changes(games: &[GameData], last_games: &[GameData]) -> bool {
    let games_hash = calculate_games_hash(games);
    let last_games_hash = calculate_games_hash(last_games);
    let data_changed = games_hash != last_games_hash;

    if data_changed {
        tracing::debug!("Data changed, updating UI");

        // Log specific changes for live games to help debug game clock updates
        if !last_games.is_empty() && games.len() == last_games.len() {
            for (i, (new_game, old_game)) in games.iter().zip(last_games.iter()).enumerate() {
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
        let has_ongoing_games = has_live_games_from_game_data(games);
        let all_scheduled = !games.is_empty() && games.iter().all(is_future_game);

        if all_scheduled && !has_ongoing_games {
            tracing::info!("All games are scheduled - auto-refresh disabled");
        } else if has_ongoing_games {
            tracing::info!("Ongoing games detected - auto-refresh enabled");
        }

        tracing::debug!("No data changes detected, skipping UI update");
    }

    data_changed
}

/// Creates or restores a teletext page based on the current state and data
async fn create_or_restore_page(
    games: &[GameData],
    disable_links: bool,
    compact_mode: bool,
    fetched_date: &str,
    preserved_page_for_restoration: Option<usize>,
    current_date: &Option<String>,
    updated_current_date: &Option<String>,
) -> Option<TeletextPage> {
    // Restore the preserved page number
    if let Some(preserved_page_for_restoration) = preserved_page_for_restoration {
        let mut page = create_page(
            games,
            disable_links,
            true,
            false,
            compact_mode,
            false, // suppress_countdown - false for interactive mode
            Some(fetched_date.to_string()),
            Some(preserved_page_for_restoration),
        )
        .await;

        // Disable auto-refresh for historical dates
        if let Some(date) = updated_current_date {
            if is_historical_date(date) {
                page.set_auto_refresh_disabled(true);
            }
        }

        Some(page)
    } else {
        let page = if games.is_empty() {
            create_error_page(fetched_date, disable_links, compact_mode)
        } else {
            // Try to create a future games page, fall back to regular page if not future games
            let show_future_header = current_date.is_none();
            match create_future_games_page(
                games,
                disable_links,
                true,
                false,
                compact_mode,
                false, // suppress_countdown - false for interactive mode
                show_future_header,
                Some(fetched_date.to_string()),
                None,
            )
            .await
            {
                Some(page) => page,
                None => {
                    let mut page = create_page(
                        games,
                        disable_links,
                        true,
                        false,
                        compact_mode,
                        false, // suppress_countdown - false for interactive mode
                        Some(fetched_date.to_string()),
                        None,
                    )
                    .await;

                    // Disable auto-refresh for historical dates
                    if let Some(date) = updated_current_date {
                        if is_historical_date(date) {
                            page.set_auto_refresh_disabled(true);
                        }
                    }

                    page
                }
            }
        };

        Some(page)
    }
}

/// Parameters for page restoration
struct PageRestorationParams<'a> {
    current_page: &'a mut Option<TeletextPage>,
    data_changed: bool,
    had_error: bool,
    preserved_page_for_restoration: Option<usize>,
    games: &'a [GameData],
    last_games: &'a [GameData],
    disable_links: bool,
    fetched_date: &'a str,
    updated_current_date: &'a Option<String>,
    compact_mode: bool,
}

/// Handles page restoration when loading screen was shown but data didn't change
async fn handle_page_restoration(params: PageRestorationParams<'_>) -> bool {
    let mut needs_render = false;

    // If we showed a loading screen but data didn't change, we still need to restore pagination
    if !params.data_changed && !params.had_error && params.preserved_page_for_restoration.is_some()
    {
        if let Some(current) = params.current_page {
            // Check if current page is a loading page by checking if it has error messages
            if current.has_error_messages() {
                if let Some(preserved_page_for_restoration) = params.preserved_page_for_restoration
                {
                    let games_to_use = if params.games.is_empty() {
                        params.last_games
                    } else {
                        params.games
                    };
                    let mut page = create_page(
                        games_to_use,
                        params.disable_links,
                        true,
                        false,
                        params.compact_mode,
                        false, // suppress_countdown - false for interactive mode
                        Some(params.fetched_date.to_string()),
                        Some(preserved_page_for_restoration),
                    )
                    .await;

                    // Disable auto-refresh for historical dates
                    if let Some(date) = params.updated_current_date {
                        if is_historical_date(date) {
                            page.set_auto_refresh_disabled(true);
                        }
                    }

                    *params.current_page = Some(page);
                    needs_render = true;
                }
            }
        }
    }

    needs_render
}

/// Creates a base TeletextPage with common initialization logic.
/// This helper function reduces code duplication between create_page and create_future_games_page.
#[allow(clippy::too_many_arguments)]
async fn create_base_page(
    games: &[GameData],
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
    compact_mode: bool,
    suppress_countdown: bool,
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
        compact_mode,
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

    // Set season countdown if regular season hasn't started yet (unless suppressed)
    if !suppress_countdown {
        page.set_show_season_countdown(games).await;
    }

    // Set the current page AFTER content is added (so total_pages() is correct)
    if let Some(page_num) = current_page {
        page.set_current_page(page_num);
    }

    page
}

/// Creates a TeletextPage for regular games
#[allow(clippy::too_many_arguments)]
pub async fn create_page(
    games: &[GameData],
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
    compact_mode: bool,
    suppress_countdown: bool,
    fetched_date: Option<String>,
    current_page: Option<usize>,
) -> TeletextPage {
    create_base_page(
        games,
        disable_video_links,
        show_footer,
        ignore_height_limit,
        compact_mode,
        suppress_countdown,
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
#[allow(clippy::too_many_arguments)]
pub async fn create_future_games_page(
    games: &[GameData],
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
    compact_mode: bool,
    suppress_countdown: bool,
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
            compact_mode,
            suppress_countdown,
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

/// Initialize timer state for the interactive UI
fn initialize_timers() -> (
    Instant, // last_manual_refresh
    Instant, // last_auto_refresh
    Instant, // last_page_change
    Instant, // last_date_navigation
    Instant, // last_resize
    Instant, // last_activity
    Instant, // cache_monitor_timer
    Instant, // last_rate_limit_hit
) {
    (
        Instant::now()
            .checked_sub(Duration::from_secs(15))
            .unwrap_or_else(Instant::now),
        Instant::now()
            .checked_sub(Duration::from_secs(10))
            .unwrap_or_else(Instant::now),
        Instant::now()
            .checked_sub(Duration::from_millis(200))
            .unwrap_or_else(Instant::now),
        Instant::now()
            .checked_sub(Duration::from_millis(250))
            .unwrap_or_else(Instant::now),
        Instant::now()
            .checked_sub(Duration::from_millis(500))
            .unwrap_or_else(Instant::now),
        Instant::now(),
        Instant::now(),
        Instant::now()
            .checked_sub(Duration::from_secs(60))
            .unwrap_or_else(Instant::now),
    )
}

/// Calculate adaptive polling interval based on user activity
fn calculate_poll_interval(time_since_activity: Duration) -> Duration {
    if time_since_activity < Duration::from_secs(5) {
        Duration::from_millis(50) // Active: 50ms (smooth interaction)
    } else if time_since_activity < Duration::from_secs(30) {
        Duration::from_millis(200) // Semi-active: 200ms (good responsiveness)
    } else {
        Duration::from_millis(500) // Idle: 500ms (conserve CPU)
    }
}

/// Calculate auto-refresh interval based on game states
fn calculate_auto_refresh_interval(games: &[GameData]) -> Duration {
    if has_live_games_from_game_data(games) {
        Duration::from_secs(15) // Increased from 8 to 15 seconds for live games
    } else if games.iter().any(is_game_near_start_time) {
        Duration::from_secs(30) // Increased from 10 to 30 seconds for games near start time
    } else {
        Duration::from_secs(60) // Standard interval for completed/scheduled games
    }
}

/// Calculate minimum interval between refreshes based on game count
fn calculate_min_refresh_interval(
    game_count: usize,
    min_refresh_interval: Option<u64>,
) -> Duration {
    if let Some(user_interval) = min_refresh_interval {
        Duration::from_secs(user_interval) // Use user-specified interval
    } else if game_count >= 6 {
        Duration::from_secs(30) // Minimum 30 seconds between refreshes for 6+ games
    } else if game_count >= 4 {
        Duration::from_secs(20) // Minimum 20 seconds between refreshes for 4-5 games
    } else {
        Duration::from_secs(10) // Minimum 10 seconds between refreshes for 1-3 games
    }
}

/// Parameters for auto-refresh checking
struct AutoRefreshParams {
    needs_refresh: bool,
    games: Vec<GameData>,
    last_auto_refresh: Instant,
    auto_refresh_interval: Duration,
    min_interval_between_refreshes: Duration,
    last_rate_limit_hit: Instant,
    rate_limit_backoff: Duration,
    current_date: Option<String>,
}

/// Check if auto-refresh should be triggered
fn should_trigger_auto_refresh(params: AutoRefreshParams) -> bool {
    if params.needs_refresh || params.games.is_empty() {
        return false;
    }

    if params.last_auto_refresh.elapsed() < params.auto_refresh_interval {
        return false;
    }

    if params.last_auto_refresh.elapsed() < params.min_interval_between_refreshes {
        return false;
    }

    if params.last_rate_limit_hit.elapsed() < params.rate_limit_backoff {
        return false;
    }

    // Don't auto-refresh for historical dates
    if let Some(date) = &params.current_date {
        if is_historical_date(date) {
            tracing::debug!("Auto-refresh skipped for historical date: {}", date);
            return false;
        }
    }

    let has_ongoing_games = has_live_games_from_game_data(&params.games);
    let all_scheduled = !params.games.is_empty() && params.games.iter().all(is_future_game);

    if has_ongoing_games {
        tracing::info!("Auto-refresh triggered for ongoing games");
        true
    } else if !all_scheduled {
        tracing::debug!("Auto-refresh triggered for non-scheduled games (mixed game states)");
        true
    } else {
        // Enhanced check for games that might have started
        let has_recently_started_games = params.games.iter().any(is_game_near_start_time);
        if has_recently_started_games {
            tracing::info!("Auto-refresh triggered for games that may have started");
            true
        } else {
            tracing::debug!("Auto-refresh skipped - all games are scheduled for future");
            false
        }
    }
}

/// Handle data fetching with error handling and timeout
async fn fetch_data_with_timeout(
    current_date: Option<String>,
    timeout_duration: Duration,
) -> (Vec<GameData>, bool, String, bool) {
    let fetch_future = fetch_liiga_data(current_date.clone());

    match tokio::time::timeout(timeout_duration, fetch_future).await {
        Ok(fetch_result) => match fetch_result {
            Ok((games, fetched_date)) => {
                tracing::debug!("Auto-refresh successful: fetched {} games", games.len());
                (games, false, fetched_date, false)
            }
            Err(e) => {
                tracing::error!("Auto-refresh failed: {}", e);
                tracing::error!(
                    "Error details - Type: {}, Current date: {:?}",
                    std::any::type_name_of_val(&e),
                    current_date
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
                    }
                    _ => {
                        tracing::warn!("Auto-refresh error: {}, will retry on next cycle", e);
                    }
                }

                // Graceful degradation: continue with existing data instead of showing error page
                tracing::info!("Continuing with existing data due to auto-refresh failure");

                (Vec::new(), true, String::new(), true)
            }
        },
        Err(_) => {
            // Timeout occurred during auto-refresh
            tracing::warn!(
                "Auto-refresh timeout after {:?}, continuing with existing data",
                timeout_duration
            );
            (Vec::new(), true, String::new(), true)
        }
    }
}

/// Create loading page for data fetching
fn create_loading_page(current_date: &Option<String>, disable_links: bool, compact_mode: bool) -> TeletextPage {
    let mut loading_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        disable_links,
        true,
        false,
        compact_mode,
    );

    if let Some(date) = current_date {
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

    loading_page
}

/// Create error page for empty games
fn create_error_page(fetched_date: &str, disable_links: bool, compact_mode: bool) -> TeletextPage {
    let mut error_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        disable_links,
        true,
        false,
        compact_mode,
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
            format_date_for_display(fetched_date)
        ));
    }

    error_page
}

/// Handle data fetching and page creation
async fn handle_data_fetching(
    current_date: &Option<String>,
    last_games: &[GameData],
    disable_links: bool,
    compact_mode: bool,
    preserved_page_for_restoration: Option<usize>,
) -> Result<
    (
        Vec<GameData>,
        bool,
        String,
        bool,
        Option<TeletextPage>,
        bool,
    ),
    AppError,
> {
    // Determine indicator states
    let (should_show_loading, should_show_indicator) =
        determine_indicator_states(current_date, last_games);

    // Initialize page state
    let mut current_page: Option<TeletextPage> = None;
    let mut needs_render = manage_loading_indicators(
        &mut current_page,
        should_show_loading,
        should_show_indicator,
        current_date,
        disable_links,
        compact_mode,
    );

    // Fetch data with timeout
    let timeout_duration = tokio::time::Duration::from_secs(15);
    let (games, had_error, fetched_date, should_retry) =
        fetch_data_with_timeout(current_date.clone(), timeout_duration).await;

    // Update current_date to track the actual date being displayed
    let mut updated_current_date = current_date.clone();
    if !had_error && !fetched_date.is_empty() {
        updated_current_date = Some(fetched_date.clone());
        tracing::debug!("Updated current_date to: {:?}", updated_current_date);
    }

    // Perform change detection and logging
    let data_changed = detect_and_log_changes(&games, last_games);

    // Handle page creation/restoration based on data changes and errors
    if data_changed && !had_error {
        if let Some(page) = create_or_restore_page(
            &games,
            disable_links,
            compact_mode,
            &fetched_date,
            preserved_page_for_restoration,
            current_date,
            &updated_current_date,
        )
        .await
        {
            current_page = Some(page);
            needs_render = true;
        }
    } else if had_error {
        tracing::debug!(
            "Auto-refresh failed but no data changes detected, continuing with existing UI"
        );
    }

    // Handle page restoration when loading screen was shown but data didn't change
    let restoration_render = handle_page_restoration(PageRestorationParams {
        current_page: &mut current_page,
        data_changed,
        had_error,
        preserved_page_for_restoration,
        games: &games,
        last_games,
        disable_links,
        fetched_date: &fetched_date,
        updated_current_date: &updated_current_date,
        compact_mode,
    })
    .await;
    needs_render = needs_render || restoration_render;

    // Hide auto-refresh indicator after data is fetched
    if let Some(page) = &mut current_page {
        page.hide_auto_refresh_indicator();
        needs_render = true;
    }

    Ok((
        games,
        had_error,
        fetched_date,
        should_retry,
        current_page,
        needs_render,
    ))
}

/// Parameters for keyboard event handling
#[allow(dead_code)]
struct KeyEventParams<'a> {
    key_event: &'a crossterm::event::KeyEvent,
    current_date: &'a mut Option<String>,
    current_page: &'a mut Option<TeletextPage>,
    needs_refresh: &'a mut bool,
    needs_render: &'a mut bool,
    last_manual_refresh: &'a mut Instant,
    last_page_change: &'a mut Instant,
    last_date_navigation: &'a mut Instant,
    debug_mode: bool,
    disable_links: bool,
}

/// Handle keyboard events
async fn handle_key_event(params: KeyEventParams<'_>) -> Result<bool, AppError> {
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
                let mut stdout = std::io::stdout();
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
                let mut stdout = std::io::stdout();
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
                if let Some(page) = params.current_page.as_ref() {
                    if page.is_auto_refresh_disabled() {
                        tracing::info!("Manual refresh ignored - auto-refresh is disabled");
                        return Ok(false); // Skip refresh when auto-refresh is disabled
                    }
                }

                // Check if current date is historical - don't refresh historical data
                if let Some(date) = params.current_date {
                    if is_historical_date(date) {
                        tracing::info!("Manual refresh skipped for historical date: {}", date);
                        return Ok(false); // Skip refresh for historical dates
                    }
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

/// Handle resize events with debouncing
fn handle_resize_event(
    _current_page: &mut Option<TeletextPage>,
    _needs_render: &mut bool,
    pending_resize: &mut bool,
    resize_timer: &mut Instant,
    last_resize: &mut Instant,
) {
    tracing::debug!("Resize event");
    // Debounce resize events
    if last_resize.elapsed() >= Duration::from_millis(500) {
        *resize_timer = Instant::now();
        *pending_resize = true;
        *last_resize = Instant::now();
    }
}

/// Update auto-refresh indicator animation
fn update_auto_refresh_animation(current_page: &mut Option<TeletextPage>, needs_render: &mut bool) {
    if let Some(page) = current_page {
        if page.is_auto_refresh_indicator_active() {
            page.update_auto_refresh_animation();
            *needs_render = true;
        }
    }
}

/// Handle pending resize with debouncing
fn handle_pending_resize(
    current_page: &mut Option<TeletextPage>,
    needs_render: &mut bool,
    pending_resize: &mut bool,
    resize_timer: &Instant,
) {
    if *pending_resize && resize_timer.elapsed() >= Duration::from_millis(500) {
        tracing::debug!("Handling resize");
        if let Some(page) = current_page {
            page.handle_resize();
            *needs_render = true;
        }
        *pending_resize = false;
    }
}

/// Render UI with buffering
fn render_ui(
    current_page: &Option<TeletextPage>,
    needs_render: &mut bool,
    stdout: &mut std::io::Stdout,
) -> Result<(), AppError> {
    if *needs_render {
        if let Some(page) = current_page {
            page.render_buffered(stdout)?;
            tracing::debug!("UI rendered with buffering");
        }
        *needs_render = false;
    }
    Ok(())
}

/// Handle cache monitoring
async fn handle_cache_monitoring(cache_monitor_timer: &mut Instant) {
    const CACHE_MONITOR_INTERVAL: Duration = Duration::from_secs(300); // 5 minutes

    if cache_monitor_timer.elapsed() >= CACHE_MONITOR_INTERVAL {
        tracing::debug!("Monitoring cache usage");
        monitor_cache_usage().await;
        *cache_monitor_timer = Instant::now();
    }
}

/// Setup terminal for interactive mode
fn setup_terminal(debug_mode: bool) -> Result<std::io::Stdout, AppError> {
    let mut stdout = stdout();

    if !debug_mode {
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen)?;
    }

    Ok(stdout)
}

/// Cleanup terminal after interactive mode
fn cleanup_terminal(debug_mode: bool, mut stdout: std::io::Stdout) -> Result<(), AppError> {
    if !debug_mode {
        disable_raw_mode()?;
        execute!(stdout, LeaveAlternateScreen)?;
    }
    Ok(())
}

/// Runs the interactive UI with adaptive polling and change detection
pub async fn run_interactive_ui(
    date: Option<String>,
    disable_links: bool,
    debug_mode: bool,
    min_refresh_interval: Option<u64>,
    compact_mode: bool,
) -> Result<(), AppError> {
    // Setup terminal for interactive mode
    let mut stdout = setup_terminal(debug_mode)?;
    // Timer management with adaptive intervals
    let (
        mut last_manual_refresh,
        mut last_auto_refresh,
        mut last_page_change,
        mut last_date_navigation,
        mut last_resize,
        _last_activity,
        mut cache_monitor_timer,
        _last_rate_limit_hit,
    ) = initialize_timers();

    // State management
    let mut needs_refresh = true;
    let mut needs_render = false;
    let mut current_page: Option<TeletextPage> = None;
    let mut pending_resize = false;
    let mut resize_timer = Instant::now();
    // Preserved page number for restoration after refresh - will be set when needed
    let mut preserved_page_for_restoration: Option<usize>;

    // Date navigation state - track the current date being displayed
    let mut current_date = date;

    // Change detection - track data changes to avoid unnecessary re-renders
    let mut last_games_hash = 0u64;
    let mut last_games = Vec::new();

    // Adaptive polling configuration
    let mut last_activity = Instant::now();

    // Rate limiting protection
    let _rate_limit_backoff = Duration::from_secs(0);
    let _last_rate_limit_hit = Instant::now()
        .checked_sub(Duration::from_secs(60))
        .unwrap_or_else(Instant::now);

    loop {
        // Adaptive polling interval based on activity
        let time_since_activity = last_activity.elapsed();
        let poll_interval = calculate_poll_interval(time_since_activity);

        // Check for auto-refresh with better logic and rate limiting protection
        let auto_refresh_interval = calculate_auto_refresh_interval(&last_games);
        let min_interval_between_refreshes =
            calculate_min_refresh_interval(last_games.len(), min_refresh_interval);

        // Rate limiting protection: don't refresh too frequently if we have many games
        let _rate_limit_backoff = Duration::from_secs(0);

        // Debug logging for rate limit backoff enforcement
        if _rate_limit_backoff > Duration::from_secs(0) {
            let backoff_remaining =
                _rate_limit_backoff.saturating_sub(_last_rate_limit_hit.elapsed());
            if backoff_remaining > Duration::from_secs(0) {
                tracing::debug!(
                    "Rate limit backoff active: {}s remaining (total backoff: {}s, elapsed since rate limit: {}s)",
                    backoff_remaining.as_secs(),
                    _rate_limit_backoff.as_secs(),
                    _last_rate_limit_hit.elapsed().as_secs()
                );
            }
        }

        if should_trigger_auto_refresh(AutoRefreshParams {
            needs_refresh,
            games: last_games.clone(),
            last_auto_refresh,
            auto_refresh_interval,
            min_interval_between_refreshes,
            last_rate_limit_hit: _last_rate_limit_hit,
            rate_limit_backoff: _rate_limit_backoff,
            current_date: current_date.clone(),
        }) {
            needs_refresh = true;
        }

        // Data fetching with change detection
        if needs_refresh {
            tracing::debug!("Fetching new data");

            // Always preserve the current page number before refresh, regardless of loading screen
            preserved_page_for_restoration = current_page
                .as_ref()
                .map(|existing_page| existing_page.get_current_page());

            // Handle data fetching using the helper function
            let (games, had_error, fetched_date, should_retry, new_page, _needs_render_update) =
                handle_data_fetching(
                    &current_date,
                    &last_games,
                    disable_links,
                    compact_mode,
                    preserved_page_for_restoration,
                )
                .await?;

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

                // Update the current page if we have a new one
                if let Some(new_page) = new_page {
                    current_page = Some(new_page);
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
        handle_pending_resize(
            &mut current_page,
            &mut needs_render,
            &mut pending_resize,
            &resize_timer,
        );

        // Update auto-refresh indicator animation (only when active)
        update_auto_refresh_animation(&mut current_page, &mut needs_render);

        // Batched UI rendering - only render when necessary
        // Use buffered rendering to minimize flickering
        render_ui(&current_page, &mut needs_render, &mut stdout)?;

        // Event handling with adaptive polling
        if event::poll(poll_interval)? {
            last_activity = Instant::now(); // Reset activity timer

            match event::read()? {
                Event::Key(key_event) => {
                    if handle_key_event(KeyEventParams {
                        key_event: &key_event,
                        current_date: &mut current_date,
                        current_page: &mut current_page,
                        needs_refresh: &mut needs_refresh,
                        needs_render: &mut needs_render,
                        last_manual_refresh: &mut last_manual_refresh,
                        last_page_change: &mut last_page_change,
                        last_date_navigation: &mut last_date_navigation,
                        debug_mode,
                        disable_links,
                    })
                    .await?
                    {
                        break;
                    }
                }
                Event::Resize(_, _) => {
                    handle_resize_event(
                        &mut current_page,
                        &mut needs_render,
                        &mut pending_resize,
                        &mut resize_timer,
                        &mut last_resize,
                    );
                }
                _ => {}
            }
        }

        // Periodic cache monitoring for long-running sessions
        handle_cache_monitoring(&mut cache_monitor_timer).await;

        // Only sleep if we're in idle mode to avoid unnecessary delays
        if poll_interval >= Duration::from_millis(200) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    // Cleanup terminal
    cleanup_terminal(debug_mode, stdout)?;
    Ok(())
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

    #[test]
    fn test_series_type_from_string() {
        // Test all known series types
        assert_eq!(SeriesType::from("playoffs"), SeriesType::Playoffs);
        assert_eq!(SeriesType::from("PLAYOFFS"), SeriesType::Playoffs);
        assert_eq!(SeriesType::from("playout"), SeriesType::Playout);
        assert_eq!(SeriesType::from("PLAYOUT"), SeriesType::Playout);
        assert_eq!(
            SeriesType::from("qualifications"),
            SeriesType::Qualifications
        );
        assert_eq!(
            SeriesType::from("QUALIFICATIONS"),
            SeriesType::Qualifications
        );
        assert_eq!(SeriesType::from("practice"), SeriesType::Practice);
        assert_eq!(
            SeriesType::from("valmistavat_ottelut"),
            SeriesType::Practice
        );
        assert_eq!(SeriesType::from("PRACTICE"), SeriesType::Practice);

        // Test default fallback to RegularSeason
        assert_eq!(SeriesType::from("runkosarja"), SeriesType::RegularSeason);
        assert_eq!(SeriesType::from("unknown"), SeriesType::RegularSeason);
        assert_eq!(SeriesType::from(""), SeriesType::RegularSeason);
    }

    #[test]
    fn test_series_type_priority_ordering() {
        // Test that enum variants are ordered by priority (Playoffs highest, RegularSeason lowest)
        assert!(SeriesType::Playoffs < SeriesType::Playout);
        assert!(SeriesType::Playout < SeriesType::Qualifications);
        assert!(SeriesType::Qualifications < SeriesType::Practice);
        assert!(SeriesType::Practice < SeriesType::RegularSeason);

        // Test min() function picks highest priority
        let series_types = [
            SeriesType::RegularSeason,
            SeriesType::Playoffs,
            SeriesType::Practice,
        ];
        assert_eq!(series_types.iter().min(), Some(&SeriesType::Playoffs));
    }

    #[test]
    fn test_series_type_display() {
        assert_eq!(SeriesType::Playoffs.to_string(), "PLAYOFFS");
        assert_eq!(SeriesType::Playout.to_string(), "PLAYOUT-OTTELUT");
        assert_eq!(SeriesType::Qualifications.to_string(), "LIIGAKARSINTA");
        assert_eq!(SeriesType::Practice.to_string(), "HARJOITUSOTTELUT");
        assert_eq!(SeriesType::RegularSeason.to_string(), "RUNKOSARJA");
    }

    #[test]
    fn test_get_subheader_with_series_types() {
        // Test empty games
        assert_eq!(get_subheader(&[]), "SM-LIIGA");

        // Test single regular season game
        let regular_game = GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "19:00".to_string(),
            result: "3-2".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-01T19:00:00Z".to_string(),
        };
        assert_eq!(get_subheader(&[regular_game]), "RUNKOSARJA");

        // Test mixed games - should prioritize playoffs
        let playoff_game = GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "19:00".to_string(),
            result: "3-2".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "playoffs".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-01T19:00:00Z".to_string(),
        };

        let practice_game = GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "19:00".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "practice".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-01T19:00:00Z".to_string(),
        };

        // With mixed series types, should show highest priority (playoffs)
        assert_eq!(get_subheader(&[practice_game, playoff_game]), "PLAYOFFS");
    }
}
