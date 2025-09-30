//! Game result data structures and types

use crate::data_fetcher::GoalEventData;

#[derive(Debug, Clone, Hash, PartialEq)]
pub enum ScoreType {
    Final,     // Final score
    Ongoing,   // Ongoing game with current score
    Scheduled, // Scheduled game with no score yet
}

/// Represents a game result with all relevant information for display.
/// This struct acts as a data transfer object between the data fetcher and UI components.
#[derive(Debug, Clone)]
pub struct GameResultData {
    pub home_team: String,
    pub away_team: String,
    pub time: String,
    pub result: String,
    pub score_type: ScoreType,
    pub is_overtime: bool,
    pub is_shootout: bool,
    pub goal_events: Vec<GoalEventData>,
    pub played_time: i32,
}

impl GameResultData {
    /// Creates a new GameResultData instance from a GameData object.
    ///
    /// # Arguments
    /// * `game_data` - Reference to a GameData object containing raw game information
    ///
    /// # Returns
    /// * `GameResultData` - A new instance containing formatted game result data
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::GameResultData;
    /// let game_data = liiga_teletext::data_fetcher::models::GameData {
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
    /// };
    /// let result = GameResultData::new(&game_data);
    /// ```
    pub fn new(game_data: &crate::data_fetcher::GameData) -> Self {
        Self {
            home_team: game_data.home_team.clone(),
            away_team: game_data.away_team.clone(),
            time: game_data.time.clone(),
            result: game_data.result.clone(),
            score_type: game_data.score_type.clone(),
            is_overtime: game_data.is_overtime,
            is_shootout: game_data.is_shootout,
            goal_events: game_data.goal_events.clone(),
            played_time: game_data.played_time,
        }
    }
}
