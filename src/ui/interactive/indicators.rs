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
