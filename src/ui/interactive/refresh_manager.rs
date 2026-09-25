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
            tracing::warn!("Failed to parse game start time '{}': {e}", game.start);
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
        Duration::from_secs(crate::constants::refresh::LIVE_GAMES_INTERVAL_SECONDS)
    } else if games.iter().any(is_game_near_start_time) {
        Duration::from_secs(30) // Games near start time
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
    /// The last refresh failed and has not been retried successfully yet.
    pub retry_pending: bool,
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

    // A failed refresh keeps the last good games on screen. Retry it even when
    // those games are all scheduled for later, or for a historical date (whose
    // data never changes, but has not been fetched yet), or it would never run.
    if params.retry_pending {
        tracing::debug!("Auto-refresh triggered: retrying a failed refresh");
        return true;
    }

    // Don't auto-refresh for historical dates
    if let Some(date) = params.current_date.as_deref()
        && is_historical_date(date)
    {
        tracing::debug!("Auto-refresh skipped for historical date: {date}");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing_utils::TestDataBuilder;

    /// A game scheduled three hours from now, well outside the near-start window.
    fn game_later_today() -> GameData {
        let mut game = TestDataBuilder::create_basic_game("TPS", "HIFK");
        game.score_type = ScoreType::Scheduled;
        game.start = (chrono::Utc::now() + chrono::Duration::hours(3)).to_rfc3339();
        game
    }

    fn params_for(games: &[GameData], retry_pending: bool) -> AutoRefreshParams<'_> {
        let long_ago = Instant::now()
            .checked_sub(Duration::from_secs(600))
            .unwrap_or_else(Instant::now);
        AutoRefreshParams {
            needs_refresh: false,
            games,
            last_auto_refresh: long_ago,
            auto_refresh_interval: Duration::from_secs(60),
            min_interval_between_refreshes: Duration::from_secs(10),
            last_rate_limit_hit: long_ago,
            rate_limit_backoff: Duration::from_secs(2),
            retry_pending,
            current_date: &None,
        }
    }

    #[test]
    fn all_scheduled_day_does_not_auto_refresh() {
        let games = [game_later_today()];
        assert!(!should_trigger_auto_refresh(params_for(&games, false)));
    }

    #[test]
    fn failed_refresh_is_retried_even_when_all_games_are_scheduled() {
        // A failed refresh keeps the last good games. If those are all scheduled
        // for later, the retry must still run once the backoff has passed.
        let games = [game_later_today()];
        assert!(should_trigger_auto_refresh(params_for(&games, true)));
    }

    #[test]
    fn failed_refresh_still_waits_for_backoff() {
        let games = [game_later_today()];
        let mut params = params_for(&games, true);
        params.last_rate_limit_hit = Instant::now();
        assert!(!should_trigger_auto_refresh(params));
    }

    #[test]
    fn failed_fetch_of_historical_date_is_retried() {
        // The error page promises an automatic retry, so a historical date
        // whose first fetch failed must be retried too.
        let historical_date = Some("2024-01-15".to_string());
        let mut params = params_for(&[], true);
        params.current_date = &historical_date;
        assert!(should_trigger_auto_refresh(params));
    }

    #[test]
    fn historical_date_is_not_auto_refreshed_after_success() {
        let historical_date = Some("2024-01-15".to_string());
        let mut params = params_for(&[], false);
        params.current_date = &historical_date;
        assert!(!should_trigger_auto_refresh(params));
    }
}
