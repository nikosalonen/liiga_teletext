//! Interactive UI module for the liiga_teletext application
//!
//! This module contains the main interactive UI loop and all UI-related helper functions.
//! It handles user input, screen updates, page creation, and the main application flow.

use crate::data_fetcher::has_live_games_from_game_data;
use crate::data_fetcher::{GameData, fetch_liiga_data, is_historical_date};
use crate::error::AppError;
use crate::teletext_ui::{GameResultData, ScoreType, TeletextPage};
use chrono::{Datelike, Local, NaiveDate, Utc};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::stdout;
use std::time::{Duration, Instant};
use tracing;

// Import utilities from sibling modules
use super::series_utils::get_subheader;
use super::change_detection::{calculate_games_hash, detect_and_log_changes};
use super::indicators::determine_indicator_states;
use super::refresh_manager::{
    calculate_auto_refresh_interval, calculate_min_refresh_interval,
    should_trigger_auto_refresh, AutoRefreshParams,
};
use super::state_manager::InteractiveState;
use super::event_handler::{EventHandler, EventHandlerBuilder, EventResult};
use super::navigation_manager::{NavigationManager, PageCreationConfig, PageRestorationParams, LoadingIndicatorConfig};

// Teletext page constants (removed unused constants)

// UI timing constants (removed unused constants)








/// Gets the target date for navigation, using current_date if available,
/// otherwise determining the appropriate date based on current time.
/// Checks if a date would require historical/schedule endpoint (from previous season).
/// This prevents navigation to very old games via arrow keys, but allows reasonable historical access.
/// Finds the previous date with games by checking dates going backwards.
/// Returns None if no games are found within the current season or a reasonable time range.
/// Prevents navigation to previous season games for better UX.
/// Finds the next date with games by checking dates going forwards.
/// Returns None if no games are found within a reasonable time range.
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
                    AppError::NetworkTimeout { url } => {
                        tracing::warn!(
                            "Auto-refresh timeout for URL: {}, will retry on next cycle",
                            url
                        );
                    }
                    AppError::NetworkConnection { url, message } => {
                        tracing::warn!(
                            "Auto-refresh connection error for URL: {}, details: {}, will retry on next cycle",
                            url,
                            message
                        );
                    }
                    AppError::ApiServerError {
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
                    AppError::ApiServiceUnavailable {
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
                    AppError::ApiRateLimit { message, url } => {
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


/// Handle data fetching and page creation using NavigationManager
async fn handle_data_fetching(
    current_date: &Option<String>,
    last_games: &[GameData],
    disable_links: bool,
    compact_mode: bool,
    wide_mode: bool,
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
    // Create navigation manager
    let nav_manager = NavigationManager::new();

    // Determine indicator states
    let (should_show_loading, should_show_indicator) =
        determine_indicator_states(current_date, last_games);

    // Initialize page state
    let mut current_page: Option<TeletextPage> = None;
    let mut needs_render = nav_manager.manage_loading_indicators(
        &mut current_page,
        LoadingIndicatorConfig {
            should_show_loading,
            should_show_indicator,
            current_date,
            disable_links,
            compact_mode,
            wide_mode,
        },
    );

    // Fetch data with timeout
    let timeout_duration = Duration::from_secs(15);
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
    // Always create a page if we have no games (to show the error message with navigation hints)
    // or if data changed and there was no error
    if (data_changed || games.is_empty()) && !had_error {
        if let Some(page) = nav_manager.create_or_restore_page(PageCreationConfig {
            games: &games,
            disable_links,
            compact_mode,
            wide_mode,
            fetched_date: &fetched_date,
            preserved_page_for_restoration,
            current_date,
            updated_current_date: &updated_current_date,
        })
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
    let restoration_render = nav_manager.handle_page_restoration(PageRestorationParams {
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
        wide_mode,
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
/// Handle keyboard events

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
    wide_mode: bool,
) -> Result<(), AppError> {
    // Setup terminal for interactive mode
    let mut stdout = setup_terminal(debug_mode)?;
    
    // Initialize all state through the state manager
    let mut state = InteractiveState::new(date);
    
    // Create event handler with appropriate configuration
    let event_handler = if debug_mode {
        EventHandler::for_debug()
    } else {
        EventHandler::new()
    };

    loop {
        // Check for auto-refresh with better logic and rate limiting protection
        let auto_refresh_interval = calculate_auto_refresh_interval(state.change_detection.last_games());
        let min_interval_between_refreshes =
            calculate_min_refresh_interval(state.change_detection.last_games().len(), min_refresh_interval);

        // Debug logging for backoff enforcement
        if state.adaptive_polling.retry_backoff() > Duration::from_secs(0) {
            let backoff_remaining = state.adaptive_polling.backoff_remaining();
            if backoff_remaining > Duration::from_secs(0) {
                tracing::trace!(
                    "Retry backoff active: {}s remaining (total backoff: {}s, elapsed since error: {}s)",
                    backoff_remaining.as_secs(),
                    state.adaptive_polling.retry_backoff().as_secs(),
                    state.adaptive_polling.last_backoff_hit().elapsed().as_secs()
                );
            }
        }

        if should_trigger_auto_refresh(AutoRefreshParams {
            needs_refresh: state.needs_refresh(),
            games: state.change_detection.last_games(),
            last_auto_refresh: state.timers.last_auto_refresh,
            auto_refresh_interval,
            min_interval_between_refreshes,
            last_rate_limit_hit: state.adaptive_polling.last_backoff_hit(),
            rate_limit_backoff: state.adaptive_polling.retry_backoff(),
            current_date: state.current_date(),
        }) {
            state.request_refresh();
        }

        // Data fetching with change detection
        if state.needs_refresh() {
            tracing::debug!("Fetching new data");

            // Always preserve the current page number before refresh, regardless of loading screen
            if let Some(page) = state.current_page() {
                state.preserve_page(page.get_current_page());
            }

            // Handle data fetching using the helper function
            let (games, had_error, fetched_date, should_retry, new_page, _needs_render_update) =
                handle_data_fetching(
                    state.current_date(),
                    state.change_detection.last_games(),
                    disable_links,
                    compact_mode,
                    wide_mode,
                    state.preserved_page(),
                )
                .await?;

            // Update current_date to track the actual date being displayed
            if !had_error && !fetched_date.is_empty() {
                state.set_current_date(Some(fetched_date.clone()));
                tracing::debug!("Updated current_date to: {:?}", state.current_date());
            }

            // Change detection using a simple hash of game data
            let games_hash = calculate_games_hash(&games);
            let data_changed = state.change_detection.update_and_check_changes(&games, games_hash);

            if data_changed {
                tracing::debug!("Data changed, updating UI");

                // Log specific changes for live games to help debug game clock updates
                if !state.change_detection.last_games().is_empty() && games.len() == state.change_detection.last_games().len() {
                    for (i, (new_game, old_game)) in games.iter().zip(state.change_detection.last_games().iter()).enumerate()
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
                    state.set_current_page(new_page);
                }
            } else if had_error {
                tracing::debug!(
                    "Auto-refresh failed but no data changes detected, continuing with existing UI"
                );
                if let Some(page) = state.current_page_mut() {
                    page.show_error_warning();
                    state.request_render();
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
                let nav_manager = NavigationManager::new();
                let has_ongoing_games = has_live_games_from_game_data(&games);
                let all_scheduled = !games.is_empty() && games.iter().all(|game| nav_manager.is_future_game(game));

                if all_scheduled && !has_ongoing_games {
                    tracing::info!("All games are scheduled - auto-refresh disabled");
                } else if has_ongoing_games {
                    tracing::info!("Ongoing games detected - auto-refresh enabled");
                }

                tracing::debug!("No data changes detected, skipping UI update");
            }

            // Update change detection variables only on successful fetch
            if !had_error {
                if let Some(page) = state.current_page_mut()
                    && page.is_error_warning_active()
                {
                    page.hide_error_warning();
                    state.request_render();
                }
                state.change_detection.update_state(games, games_hash);
            } else {
                tracing::debug!(
                    "Preserving last_games due to fetch error; will retry without clearing state"
                );
            }

            state.clear_refresh_flag();

            // Only update last_auto_refresh if we shouldn't retry
            // This ensures that failed auto-refresh attempts will be retried on the next cycle
            if !should_retry {
                state.timers.update_auto_refresh();
                // Reset backoff window after a successful cycle
                if state.adaptive_polling.retry_backoff() > Duration::from_secs(0) {
                    tracing::debug!("Resetting retry backoff after successful refresh");
                }
                state.adaptive_polling.reset_backoff();
                tracing::trace!("Auto-refresh cycle completed successfully");
            } else {
                state.adaptive_polling.apply_backoff();
                let jittered_secs = state.adaptive_polling.retry_backoff().as_secs_f64();
                tracing::debug!(
                    "Auto-refresh failed; applying retry backoff of {jittered_secs:.2}s"
                );
            }
        }

        // Update auto-refresh indicator animation (only when active)
        if let Some(page) = state.current_page_mut()
            && page.is_auto_refresh_indicator_active()
        {
            page.update_auto_refresh_animation();
            state.request_render();
        }

        // Batched UI rendering - only render when necessary
        // Use buffered rendering to minimize flickering
        if state.needs_render() {
            if let Some(page) = state.current_page() {
                page.render_buffered(&mut stdout)?;
                tracing::debug!("UI rendered with buffering");
            }
            state.clear_render_flag();
        }

        // Process events using the event handler
        match event_handler.process_events(&mut state).await? {
            EventResult::Exit => {
                tracing::info!("Exit requested through event handler");
                break;
            }
            EventResult::Handled | EventResult::Continue => {
                // Continue with the loop
            }
        }

        // Periodic cache monitoring for long-running sessions
        const CACHE_MONITOR_INTERVAL: Duration = Duration::from_secs(300); // 5 minutes
        if state.timers.cache_monitor_timer.elapsed() >= CACHE_MONITOR_INTERVAL {
            tracing::debug!("Monitoring cache usage");
            monitor_cache_usage().await;
            state.timers.update_cache_monitor();
        }

        // Small sleep to prevent tight loops when not processing events
        tokio::time::sleep(Duration::from_millis(50)).await;
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

}
