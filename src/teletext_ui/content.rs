// src/teletext_ui/content.rs - Content management utilities for TeletextPage

use super::core::{TeletextPage, TeletextRow};
use crate::ui::teletext::game_result::GameResultData;

impl TeletextPage {
    /// Adds a game result to the page content.
    /// The game will be displayed according to the page's current layout settings.
    ///
    /// # Arguments
    /// * `game_data` - The game result data to add to the page
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::{TeletextPage, GameResultData};
    ///
    /// let mut page = TeletextPage::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    ///     false,
    ///     true,
    ///     false,
    ///     false,
    ///     false, // wide_mode
    /// );
    ///
    /// // Create a sample game result
    /// let game = GameResultData::new(&liiga_teletext::data_fetcher::models::GameData {
    ///     home_team: "Tappara".to_string(),
    ///     away_team: "HIFK".to_string(),
    ///     time: "18:30".to_string(),
    ///     result: "3-2".to_string(),
    ///     score_type: liiga_teletext::teletext_ui::ScoreType::Final,
    ///     is_overtime: false,
    ///     is_shootout: false,
    ///     serie: "RUNKOSARJA".to_string(),
    ///     goal_events: vec![],
    ///     played_time: 60,
    ///     start: "2024-01-15T18:30:00Z".to_string(),
    /// });
    ///
    /// page.add_game_result(game);
    /// ```
    pub fn add_game_result(&mut self, game_data: GameResultData) {
        self.content_rows.push(TeletextRow::GameResult {
            home_team: game_data.home_team,
            away_team: game_data.away_team,
            time: game_data.time,
            result: game_data.result,
            score_type: game_data.score_type,
            is_overtime: game_data.is_overtime,
            is_shootout: game_data.is_shootout,
            goal_events: game_data.goal_events,
            played_time: game_data.played_time,
        });
    }

    /// Adds an error message to be displayed on the page.
    /// The message will be formatted and displayed prominently.
    ///
    /// # Arguments
    /// * `message` - The error message to display
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::TeletextPage;
    ///
    /// let mut page = TeletextPage::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    ///     false,
    ///     true,
    ///     false,
    ///     false,
    ///     false, // wide_mode
    /// );
    ///
    /// page.add_error_message("Failed to fetch game data");
    /// ```
    pub fn add_error_message(&mut self, message: &str) {
        // Split message into lines and format each line
        let formatted_message = message
            .lines()
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("\n"); // Remove the indentation
        self.content_rows
            .push(TeletextRow::ErrorMessage(formatted_message));
    }

    /// Adds a header row indicating future games with the specified text.
    /// Typically used to display "Seuraavat ottelut" (Next games) with a date.
    pub fn add_future_games_header(&mut self, header_text: String) {
        self.content_rows
            .push(TeletextRow::FutureGamesHeader(header_text));
    }
}