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

/// Whether a serie value is one of the playoff-style series that the API
/// annotates with a meaningful `playOffPhase`. Preseason series set the field
/// to 0 on every game, so phase-based ordering and headers must not apply.
pub(super) fn is_playoff_type(serie: &str) -> bool {
    matches!(
        serie.to_ascii_lowercase().as_str(),
        "playoffs" | "playout" | "qualifications"
    )
}

/// Display label for a single serie value.
///
/// Known series map to their established Finnish name. Anything else is a
/// tournament the API names directly (e.g. the preseason `PITSITURNAUS`) and is
/// shown verbatim in upper case rather than falling back to "RUNKOSARJA".
pub(super) fn series_group_label(serie: &str) -> String {
    match serie.to_ascii_lowercase().as_str() {
        "playoffs"
        | "playout"
        | "qualifications"
        | "valmistavat_ottelut"
        | "practice"
        | "runkosarja" => SeriesType::from(serie).to_string(),
        other if other.trim().is_empty() => SeriesType::RegularSeason.to_string(),
        _ => serie.to_uppercase(),
    }
}

/// Gets the appropriate subheader based on the game series type with highest priority
pub(super) fn get_subheader(games: &[GameData]) -> String {
    let Some(first) = games.first() else {
        return "SM-LIIGA".to_string();
    };

    // When every game shares one serie, name it directly. This keeps
    // API-named tournaments (e.g. PITSITURNAUS) out of the RegularSeason
    // fallback bucket on days where they are the only games.
    if games
        .iter()
        .all(|game| game.serie.eq_ignore_ascii_case(&first.serie))
    {
        return series_group_label(&first.serie);
    }

    // Mixed series: find the type with highest priority (lowest enum value due to Ord implementation)
    games
        .iter()
        .map(|game| SeriesType::from(game.serie.as_str()))
        .min() // Uses the Ord implementation where Playoffs < Playout < ... < RegularSeason
        .unwrap_or(SeriesType::RegularSeason)
        .to_string()
}

/// Returns the Finnish name for a playoff phase based on phase number and serie type.
pub fn playoff_phase_name(phase: i32, serie: &str) -> &'static str {
    match serie.to_ascii_lowercase().as_str() {
        "playoffs" => match phase {
            1 => "1. KIERROS",
            2 => "PUOLIVÄLIERÄT",
            3 => "VÄLIERÄT",
            4 => "PRONSSIOTTELU",
            5 => "FINAALI",
            _ => "PLAYOFFS",
        },
        "playout" => "PLAYOUT",
        "qualifications" => "LIIGAKARSINTA",
        _ => "OTTELUT",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_series_type_from_string() {
        assert_eq!(SeriesType::from("playoffs"), SeriesType::Playoffs);
        assert_eq!(SeriesType::from("PLAYOFFS"), SeriesType::Playoffs);
        assert_eq!(SeriesType::from("playout"), SeriesType::Playout);
        assert_eq!(
            SeriesType::from("qualifications"),
            SeriesType::Qualifications
        );
        assert_eq!(
            SeriesType::from("valmistavat_ottelut"),
            SeriesType::Practice
        );
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

    fn make_game(home: &str, away: &str, result: &str, serie: &str) -> GameData {
        crate::testing_utils::TestDataBuilder::create_custom_game(0, home, away, result, serie)
    }

    #[test]
    fn test_get_subheader_with_series_types() {
        // Test with playoff games
        let playoff_games = vec![make_game("TPS", "HIFK", "3-2", "playoffs")];
        assert_eq!(get_subheader(&playoff_games), "PLAYOFFS");

        // Test with mixed series types - should return highest priority (Playoffs)
        let mixed_games = vec![
            make_game("TPS", "HIFK", "3-2", "runkosarja"),
            make_game("Kärpät", "Tappara", "2-1", "playoffs"),
        ];
        assert_eq!(get_subheader(&mixed_games), "PLAYOFFS");

        // Test with regular season only
        let regular_games = vec![make_game("TPS", "HIFK", "3-2", "runkosarja")];
        assert_eq!(get_subheader(&regular_games), "RUNKOSARJA");

        // Test with empty games list
        let empty_games: Vec<GameData> = vec![];
        assert_eq!(get_subheader(&empty_games), "SM-LIIGA");
    }

    #[test]
    fn test_series_group_label() {
        assert_eq!(series_group_label("playoffs"), "PLAYOFFS");
        assert_eq!(series_group_label("runkosarja"), "RUNKOSARJA");
        assert_eq!(series_group_label("PRACTICE"), "HARJOITUSOTTELUT");
        assert_eq!(
            series_group_label("valmistavat_ottelut"),
            "HARJOITUSOTTELUT"
        );
        // Tournaments the API names directly keep their own name
        assert_eq!(series_group_label("PITSITURNAUS"), "PITSITURNAUS");
        assert_eq!(series_group_label("pitsiturnaus"), "PITSITURNAUS");
        // Empty serie falls back rather than rendering a blank header
        assert_eq!(series_group_label(""), "RUNKOSARJA");
    }

    #[test]
    fn test_is_playoff_type() {
        assert!(is_playoff_type("playoffs"));
        assert!(is_playoff_type("PLAYOUT"));
        assert!(is_playoff_type("qualifications"));
        assert!(!is_playoff_type("runkosarja"));
        assert!(!is_playoff_type("PITSITURNAUS"));
        assert!(!is_playoff_type("PRACTICE"));
    }

    #[test]
    fn test_get_subheader_names_a_single_api_named_tournament() {
        let games = vec![
            make_game("Sport", "TPS", "", "PITSITURNAUS"),
            make_game("Lukko", "Ässät", "", "PITSITURNAUS"),
        ];
        assert_eq!(get_subheader(&games), "PITSITURNAUS");
    }

    #[test]
    fn test_get_subheader_uses_umbrella_term_for_mixed_preseason() {
        let games = vec![
            make_game("Sport", "TPS", "", "PITSITURNAUS"),
            make_game("HIFK", "JYP", "", "PRACTICE"),
        ];
        assert_eq!(get_subheader(&games), "HARJOITUSOTTELUT");
    }

    #[test]
    fn test_playoff_phase_name() {
        assert_eq!(playoff_phase_name(1, "playoffs"), "1. KIERROS");
        assert_eq!(playoff_phase_name(2, "playoffs"), "PUOLIVÄLIERÄT");
        assert_eq!(playoff_phase_name(3, "playoffs"), "VÄLIERÄT");
        assert_eq!(playoff_phase_name(4, "playoffs"), "PRONSSIOTTELU");
        assert_eq!(playoff_phase_name(5, "playoffs"), "FINAALI");
        assert_eq!(playoff_phase_name(99, "playoffs"), "PLAYOFFS");
        assert_eq!(playoff_phase_name(1, "playout"), "PLAYOUT");
        assert_eq!(playoff_phase_name(1, "qualifications"), "LIIGAKARSINTA");
        assert_eq!(playoff_phase_name(1, "runkosarja"), "OTTELUT");
    }
}
