//! Player name formatting utilities for consistent handling across the application.
//!
//! This module provides functions for:
//! - Building full names from first/last name components
//! - Formatting full names for teletext display (last name only with proper capitalization)
//! - Creating fallback names for missing player data

/// Builds a full name from first and last name components.
///
/// This is used when processing API responses that provide separate name fields.
///
/// # Arguments
/// * `first_name` - The player's first name
/// * `last_name` - The player's last name
///
/// # Returns
/// * `String` - The formatted full name (e.g., "Mikko Koivu")
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::player_names::build_full_name;
///
/// let full_name = build_full_name("Mikko", "Koivu");
/// assert_eq!(full_name, "Mikko Koivu");
/// ```
pub fn build_full_name(first_name: &str, last_name: &str) -> String {
    format!("{first_name} {last_name}")
}

/// Formats a player's full name for teletext display by showing only the capitalized last name.
/// This follows the authentic YLE Teksti-TV formatting style for player names in goal lists.
///
/// # Arguments
/// * `full_name` - The player's full name (e.g., "Mikko Koivu")
///
/// # Returns
/// * `String` - The formatted display name (e.g., "Koivu")
///
/// # Examples
/// ```
/// use liiga_teletext::data_fetcher::player_names::format_for_display;
///
/// let display_name = format_for_display("Mikko Koivu");
/// assert_eq!(display_name, "Koivu");
///
/// let display_name = format_for_display("Teemu Selänne");
/// assert_eq!(display_name, "Selänne");
///
/// // Handles multiple names by taking the last one
/// let display_name = format_for_display("Mikko Ilmari Koivu");
/// assert_eq!(display_name, "Koivu");
///
/// // Handles hyphenated names
/// let display_name = format_for_display("Jean-Pierre Dumont");
/// assert_eq!(display_name, "Dumont");
/// ```
pub fn format_for_display(full_name: &str) -> String {
    full_name
        .split_whitespace()
        .last()
        .unwrap_or("")
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if i == 0 {
                c.to_uppercase().next().unwrap_or(c)
            } else {
                c.to_lowercase().next().unwrap_or(c)
            }
        })
        .collect::<String>()
}

/// Creates a fallback player name when the actual player name is not available.
/// This is used when player data is missing or cannot be retrieved from the API.
///
/// # Arguments
/// * `player_id` - The player's unique identifier
///
/// # Returns
/// * `String` - A formatted fallback name (e.g., "Pelaaja 123")
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::player_names::create_fallback_name;
///
/// let fallback_name = create_fallback_name(123);
/// assert_eq!(fallback_name, "Pelaaja 123");
/// ```
pub fn create_fallback_name(player_id: i64) -> String {
    format!("Pelaaja {player_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_full_name() {
        assert_eq!(build_full_name("Mikko", "Koivu"), "Mikko Koivu");
        assert_eq!(build_full_name("Teemu", "Selänne"), "Teemu Selänne");
        assert_eq!(build_full_name("John", "Smith"), "John Smith");
    }

    #[test]
    fn test_build_full_name_with_empty_strings() {
        assert_eq!(build_full_name("", "Koivu"), " Koivu");
        assert_eq!(build_full_name("Mikko", ""), "Mikko ");
        assert_eq!(build_full_name("", ""), " ");
    }

    #[test]
    fn test_format_for_display_simple() {
        assert_eq!(format_for_display("Mikko Koivu"), "Koivu");
        assert_eq!(format_for_display("Teemu Selänne"), "Selänne");
        assert_eq!(format_for_display("John Smith"), "Smith");
    }

    #[test]
    fn test_format_for_display_single_name() {
        assert_eq!(format_for_display("Koivu"), "Koivu");
        assert_eq!(format_for_display("Selänne"), "Selänne");
    }

    #[test]
    fn test_format_for_display_multiple_names() {
        assert_eq!(format_for_display("Mikko Ilmari Koivu"), "Koivu");
        assert_eq!(format_for_display("Teemu Ilmari Selänne"), "Selänne");
    }

    #[test]
    fn test_format_for_display_with_hyphens() {
        assert_eq!(format_for_display("Jean-Pierre Dumont"), "Dumont");
        assert_eq!(format_for_display("Pierre-Luc Dubois"), "Dubois");
    }

    #[test]
    fn test_format_for_display_empty() {
        assert_eq!(format_for_display(""), "");
    }

    #[test]
    fn test_format_for_display_whitespace() {
        assert_eq!(format_for_display("   Koivu   "), "Koivu");
        assert_eq!(format_for_display("  Mikko  Koivu  "), "Koivu");
    }

    #[test]
    fn test_format_for_display_capitalization() {
        assert_eq!(format_for_display("mikko koivu"), "Koivu");
        assert_eq!(format_for_display("MIKKO KOIVU"), "Koivu");
        assert_eq!(format_for_display("MiKkO kOiVu"), "Koivu");
    }

    #[test]
    fn test_create_fallback_name() {
        assert_eq!(create_fallback_name(123), "Pelaaja 123");
        assert_eq!(create_fallback_name(456), "Pelaaja 456");
        assert_eq!(create_fallback_name(0), "Pelaaja 0");
    }

    #[test]
    fn test_create_fallback_name_negative() {
        assert_eq!(create_fallback_name(-1), "Pelaaja -1");
    }
}
