//! Basic player name formatting utilities.
//!
//! This module provides functions for:
//! - Building full names from first/last name components
//! - Formatting full names for teletext display (last name only with proper capitalization)
//! - Extracting initials and character prefixes for disambiguation
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

/// Formats a player name for display with first initial when disambiguation is needed.
/// This follows the hockey scoring convention of showing "LastName F." when multiple
/// players on the same team share the same last name.
///
/// # Arguments
/// * `first_name` - The player's first name
/// * `last_name` - The player's last name
///
/// # Returns
/// * `String` - The formatted display name with first initial (e.g., "Koivu M.")
///
/// # Examples
/// ```
/// use liiga_teletext::data_fetcher::player_names::format_for_display_with_first_initial;
///
/// let display_name = format_for_display_with_first_initial("Mikko", "Koivu");
/// assert_eq!(display_name, "Koivu M.");
///
/// let display_name = format_for_display_with_first_initial("Saku", "Koivu");
/// assert_eq!(display_name, "Koivu S.");
/// ```
#[allow(dead_code)]
pub fn format_for_display_with_first_initial(first_name: &str, last_name: &str) -> String {
    let formatted_last_name = format_for_display(&build_full_name("", last_name));

    // Use the extract_first_initial helper to get the first alphabetic character
    match extract_first_initial(first_name) {
        Some(initial) => format!("{formatted_last_name} {initial}."),
        None => formatted_last_name,
    }
}

/// Extracts the first initial from a first name with proper Unicode support.
/// This helper function handles edge cases like empty names, multiple words, and special characters.
///
/// # Arguments
/// * `first_name` - The player's first name
///
/// # Returns
/// * `Option<String>` - The first initial as an uppercase string, or None if no valid initial found
///
/// # Examples
/// ```
/// use liiga_teletext::data_fetcher::player_names::extract_first_initial;
///
/// assert_eq!(extract_first_initial("Mikko"), Some("M".to_string()));
/// assert_eq!(extract_first_initial("Äkäslompolo"), Some("Ä".to_string()));
/// assert_eq!(extract_first_initial("Jean-Pierre"), Some("J".to_string()));
/// assert_eq!(extract_first_initial(""), None);
/// assert_eq!(extract_first_initial("   "), None);
/// ```
pub fn extract_first_initial(first_name: &str) -> Option<String> {
    first_name
        .trim()
        .chars()
        .next()
        .filter(|c| c.is_alphabetic())
        .map(|c| c.to_uppercase().to_string())
}

/// Extracts the first N characters from a first name for extended disambiguation.
/// This function is used when single initials are not sufficient to distinguish players.
///
/// # Arguments
/// * `first_name` - The player's first name
/// * `length` - Number of characters to extract (minimum 1, maximum 3)
///
/// # Returns
/// * `Option<String>` - The first N characters as uppercase, or None if no valid characters found
///
/// # Examples
/// ```
/// use liiga_teletext::data_fetcher::player_names::extract_first_chars;
///
/// assert_eq!(extract_first_chars("Mikael", 2), Some("Mi".to_string()));
/// assert_eq!(extract_first_chars("Markus", 2), Some("Ma".to_string()));
/// assert_eq!(extract_first_chars("Äkäslompolo", 3), Some("Äkä".to_string()));
/// assert_eq!(extract_first_chars("", 2), None);
/// ```
pub fn extract_first_chars(first_name: &str, length: usize) -> Option<String> {
    let length = length.clamp(1, 3); // Limit to reasonable range

    // Extract only the first word/part before any separator (space, hyphen, apostrophe)
    let first_part = first_name
        .trim()
        .split(&[' ', '-', '\''][..])
        .next()
        .unwrap_or("");

    let alphabetic_chars: Vec<char> = first_part
        .chars()
        .filter(|c| c.is_alphabetic())
        .take(length)
        .collect();

    if alphabetic_chars.is_empty() {
        None
    } else {
        // First character uppercase, rest lowercase
        let mut result = String::new();
        for (i, c) in alphabetic_chars.iter().enumerate() {
            if i == 0 {
                result.push(c.to_uppercase().next().unwrap_or(*c));
            } else {
                result.push(c.to_lowercase().next().unwrap_or(*c));
            }
        }
        Some(result)
    }
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