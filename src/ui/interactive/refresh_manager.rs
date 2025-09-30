//! Auto-refresh timing and logic for the interactive UI.
//!
//! This module handles all auto-refresh logic including:
//! - Calculating adaptive polling intervals based on user activity
//! - Determining auto-refresh intervals based on game states
//! - Deciding when to trigger auto-refresh
//! - Rate limiting and backoff logic

use crate::data_fetcher::{GameData, has_live_games_from_game_data, is_historical_date};
use crate::teletext_ui::ScoreType;
use std::time::{Duration, Instant};

/// Helper function to check if a game is in the future (scheduled)
fn is_future_game(game: &GameData) -> bool {
    game.score_type == ScoreType::Scheduled
}

/// Checks if a game is scheduled to start within the next few minutes or has recently started
fn is_game_near_start_time(game: &GameData) -> bool {
    use chrono::Utc;

    if game.score_type != ScoreType::Scheduled || game.start.is_empty() {
        return false;
    }

    match chrono::DateTime::parse_from_rfc3339(&game.start) {
        Ok(game_start) => {
            let time_diff = Utc::now().signed_duration_since(game_start.with_timezone(&Utc));

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

/// Calculate adaptive polling interval based on user activity
pub(super) fn calculate_poll_interval(time_since_activity: Duration) -> Duration {
    if time_since_activity < Duration::from_secs(5) {
        Duration::from_millis(50) // Active: 50ms (smooth interaction)
    } else if time_since_activity < Duration::from_secs(30) {
        Duration::from_millis(200) // Semi-active: 200ms (good responsiveness)
    } else {
        Duration::from_millis(500) // Idle: 500ms (conserve CPU)
    }
}

/// Calculate auto-refresh interval based on game states
pub(super) fn calculate_auto_refresh_interval(games: &[GameData]) -> Duration {
    if has_live_games_from_game_data(games) {
        Duration::from_secs(15) // Increased from 8 to 15 seconds for live games
    } else if games.iter().any(is_game_near_start_time) {
        Duration::from_secs(30) // Increased from 10 to 30 seconds for games near start time
    } else {
        Duration::from_secs(60) // Standard interval for completed/scheduled games
    }
}

/// Calculate minimum interval between refreshes based on game count
pub(super) fn calculate_min_refresh_interval(
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
pub(super) struct AutoRefreshParams<'a> {
    pub needs_refresh: bool,
    pub games: &'a [GameData],
    pub last_auto_refresh: Instant,
    pub auto_refresh_interval: Duration,
    pub min_interval_between_refreshes: Duration,
    pub last_rate_limit_hit: Instant,
    pub rate_limit_backoff: Duration,
    pub current_date: &'a Option<String>,
}

/// Check if auto-refresh should be triggered
pub(super) fn should_trigger_auto_refresh(params: AutoRefreshParams<'_>) -> bool {
    if params.needs_refresh {
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
    if let Some(date) = params.current_date.as_deref()
        && is_historical_date(date)
    {
        tracing::debug!("Auto-refresh skipped for historical date: {}", date);
        return false;
    }

    // After respecting timing/backoff/historical checks, recover from empty state
    if params.games.is_empty() {
        tracing::debug!("Auto-refresh triggered: games list empty (after guards)");
        return true;
    }

    let has_ongoing_games = has_live_games_from_game_data(params.games);
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
