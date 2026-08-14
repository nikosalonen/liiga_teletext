//! Loading and auto-refresh indicator management for the interactive UI.
//!
//! This module handles the logic for showing/hiding loading screens and
//! auto-refresh indicators based on game state and date selection.

use crate::data_fetcher::{GameData, has_live_games_from_game_data, is_historical_date};
use crate::teletext_ui::{ScoreType, TeletextPage};

/// Helper function to check if a game is in the future (scheduled)
fn is_future_game(game: &GameData) -> bool {
    game.score_type == ScoreType::Scheduled
}

/// Determines whether to show loading indicator and auto-refresh indicator
pub(super) fn determine_indicator_states(
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

/// How long a background task may run before a deferred loading indicator is
/// shown. Fast fetches (cache hits, quick responses) finish without the loader
/// ever flashing on screen.
const LOADING_INDICATOR_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

/// Grace period for Shift+arrow date searches. A search always costs at least
/// one network round trip and blocks the event loop, so feedback has to come
/// much sooner than for a possibly-cached refresh.
pub(super) const SEARCH_LOADING_GRACE: std::time::Duration = std::time::Duration::from_millis(300);

/// Runs a spawned background task to completion while animating any active
/// loading/auto-refresh spinner on the given page every 100ms.
///
/// `deferred_message`, when set, shows a loading indicator with that message
/// only once the task has run longer than the grace period, so quick fetches
/// don't flash a loader.
pub(super) async fn animate_page_during_task<T>(
    page: &mut Option<TeletextPage>,
    handle: tokio::task::JoinHandle<T>,
    deferred_message: Option<&str>,
) -> Result<T, tokio::task::JoinError> {
    animate_page_during_task_with_grace(page, handle, deferred_message, LOADING_INDICATOR_GRACE)
        .await
}

pub(super) async fn animate_page_during_task_with_grace<T>(
    page: &mut Option<TeletextPage>,
    mut handle: tokio::task::JoinHandle<T>,
    deferred_message: Option<&str>,
    grace: std::time::Duration,
) -> Result<T, tokio::task::JoinError> {
    use std::io::Write;

    let started = tokio::time::Instant::now();
    let mut loader_pending = deferred_message;
    loop {
        tokio::select! {
            result = &mut handle => return result,
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                let Some(page) = page.as_mut() else { continue };
                if let Some(message) = loader_pending
                    && started.elapsed() >= grace
                {
                    page.show_loading(message.to_string());
                    loader_pending = None;
                }
                if page.tick_loading_animations()
                    && let Ok((width, height)) = crossterm::terminal::size()
                {
                    // Redraw only the indicator rows: a full-page repaint on
                    // every tick makes the whole screen flicker in some
                    // terminals (e.g. Ghostty)
                    let frame = page.build_animation_frame(width, height);
                    if !frame.is_empty() {
                        let mut stdout = std::io::stdout();
                        let _ = write!(stdout, "{frame}");
                        let _ = stdout.flush();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_page() -> TeletextPage {
        TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            false,
            false,
        )
    }

    #[tokio::test]
    async fn test_animate_page_during_task_returns_task_result() {
        let handle = tokio::task::spawn(async {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            42
        });
        let mut page = None;
        let result = animate_page_during_task(&mut page, handle, None).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_deferred_loader_not_shown_for_fast_task() {
        let handle = tokio::task::spawn(async {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            1
        });
        let mut page = Some(test_page());
        let result = animate_page_during_task_with_grace(
            &mut page,
            handle,
            Some("Haetaan..."),
            std::time::Duration::from_secs(2),
        )
        .await;
        assert_eq!(result.unwrap(), 1);
        assert!(!page.unwrap().is_loading_indicator_active());
    }

    #[tokio::test]
    async fn test_deferred_loader_shown_after_grace_period() {
        let handle = tokio::task::spawn(async {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            1
        });
        let mut page = Some(test_page());
        let result = animate_page_during_task_with_grace(
            &mut page,
            handle,
            Some("Haetaan..."),
            std::time::Duration::from_millis(50),
        )
        .await;
        assert_eq!(result.unwrap(), 1);
        assert!(page.unwrap().is_loading_indicator_active());
    }
}
