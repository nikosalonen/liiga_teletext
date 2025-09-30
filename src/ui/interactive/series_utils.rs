//! Tournament series type utilities for the interactive UI.
//!
//! This module handles series type classification and display formatting
//! for different types of Liiga games (playoffs, regular season, etc.).

use crate::data_fetcher::GameData;

/// Represents different tournament series types with explicit priority ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum SeriesType {
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
pub(super) fn get_subheader(games: &[GameData]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_series_type_from_string() {
        assert_eq!(SeriesType::from("playoffs"), SeriesType::Playoffs);
        assert_eq!(SeriesType::from("PLAYOFFS"), SeriesType::Playoffs);
        assert_eq!(SeriesType::from("playout"), SeriesType::Playout);
        assert_eq!(SeriesType::from("qualifications"), SeriesType::Qualifications);
        assert_eq!(SeriesType::from("valmistavat_ottelut"), SeriesType::Practice);
        assert_eq!(SeriesType::from("practice"), SeriesType::Practice);
        assert_eq!(SeriesType::from("runkosarja"), SeriesType::RegularSeason);
        assert_eq!(SeriesType::from("unknown"), SeriesType::RegularSeason);
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
    fn test_series_type_priority_ordering() {
        // Playoffs has highest priority (lowest value in Ord)
        assert!(SeriesType::Playoffs < SeriesType::Playout);
        assert!(SeriesType::Playout < SeriesType::Qualifications);
        assert!(SeriesType::Qualifications < SeriesType::Practice);
        assert!(SeriesType::Practice < SeriesType::RegularSeason);
    }

    // TODO: Re-enable when testing_utils import issue is resolved
    // #[test]
    // fn test_get_subheader_with_series_types() {
    //     use super::super::super::testing_utils::create_basic_game;
    //
    //     // Test with playoff games
    //     let playoff_games = vec![create_basic_game(1, "TPS", "HIFK", "3-2", "playoffs")];
    //     assert_eq!(get_subheader(&playoff_games), "PLAYOFFS");
    //
    //     // Test with mixed series types - should return highest priority (Playoffs)
    //     let mixed_games = vec![
    //         create_basic_game(1, "TPS", "HIFK", "3-2", "runkosarja"),
    //         create_basic_game(2, "Kärpät", "Tappara", "2-1", "playoffs"),
    //     ];
    //     assert_eq!(get_subheader(&mixed_games), "PLAYOFFS");
    //
    //     // Test with regular season only
    //     let regular_games = vec![create_basic_game(1, "TPS", "HIFK", "3-2", "runkosarja")];
    //     assert_eq!(get_subheader(&regular_games), "RUNKOSARJA");
    //
    //     // Test with empty games list
    //     let empty_games: Vec<GameData> = vec![];
    //     assert_eq!(get_subheader(&empty_games), "SM-LIIGA");
    // }
}