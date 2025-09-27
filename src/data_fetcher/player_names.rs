//! Player name formatting utilities for consistent handling across the application.
//!
//! This module provides functions for:
//! - Building full names from first/last name components
//! - Formatting full names for teletext display (last name only with proper capitalization)
//! - Creating fallback names for missing player data
//! - Player name disambiguation for teams with multiple players sharing the same last name

use crate::data_fetcher::models::{ScheduleGame, ScheduleTeam};
use std::collections::{HashMap, HashSet};

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

/// Groups players by last name and applies disambiguation rules for team-scoped display.
/// When multiple players on the same team have the same last name, their names include
/// the first letter of their first name to distinguish them.
///
/// # Arguments
/// * `players` - A slice of tuples containing (player_id, first_name, last_name)
///
/// # Returns
/// * `HashMap<i64, String>` - A mapping of player ID to disambiguated display name
///
/// # Examples
/// ```
/// use liiga_teletext::data_fetcher::player_names::format_with_disambiguation;
///
/// let players = vec![
///     (1, "Mikko".to_string(), "Koivu".to_string()),
///     (2, "Saku".to_string(), "Koivu".to_string()),
///     (3, "Teemu".to_string(), "Selänne".to_string()),
/// ];
///
/// let result = format_with_disambiguation(&players);
/// assert_eq!(result.get(&1), Some(&"Koivu M.".to_string()));
/// assert_eq!(result.get(&2), Some(&"Koivu S.".to_string()));
/// assert_eq!(result.get(&3), Some(&"Selänne".to_string()));
/// ```
pub fn format_with_disambiguation(players: &[(i64, String, String)]) -> HashMap<i64, String> {
    // Fast path: handle trivial cases without grouping overhead
    match players.len() {
        0 => return HashMap::new(),
        1 => {
            let (id, _, last_name) = &players[0];
            let display_name = format_for_display(&build_full_name("", last_name));
            return [(*id, display_name)].into_iter().collect();
        }
        2 => {
            // Fast path: if two players have different last names, no disambiguation needed
            let (_, _, last1) = &players[0];
            let (_, _, last2) = &players[1];
            if last1.to_lowercase() != last2.to_lowercase() {
                return players
                    .iter()
                    .map(|(id, _, last_name)| {
                        let display_name = format_for_display(&build_full_name("", last_name));
                        (*id, display_name)
                    })
                    .collect();
            }
            // Fall through to full algorithm if both players have same last name
        }
        _ => {} // Continue to full algorithm for 3+ players
    }

    let mut result = HashMap::new();
    let mut last_name_groups: HashMap<String, Vec<usize>> = HashMap::new();

    // Group players by last name (case-insensitive) using indices instead of cloning
    for (index, (_, _, last_name)) in players.iter().enumerate() {
        let normalized_last_name = last_name.to_lowercase();
        last_name_groups
            .entry(normalized_last_name)
            .or_default()
            .push(index);
    }

    // Apply disambiguation rules
    for (_, group_indices) in last_name_groups {
        if group_indices.len() > 1 {
            // Multiple players with same last name - apply progressive disambiguation
            let disambiguated_group =
                apply_progressive_disambiguation_by_indices(players, &group_indices);
            for (id, disambiguated_name) in disambiguated_group {
                result.insert(id, disambiguated_name);
            }
        } else {
            // Single player with this last name - use last name only
            let index = group_indices[0];
            let (id, _, last_name) = &players[index];
            let display_name = format_for_display(&build_full_name("", last_name));
            result.insert(*id, display_name);
        }
    }

    result
}

/// Check which players in a list need disambiguation.
/// This function efficiently determines which player IDs will be affected by disambiguation
/// without performing the actual disambiguation computation.
///
/// # Arguments
/// * `players` - A slice of tuples containing (player_id, first_name, last_name)
///
/// # Returns
/// * `HashSet<i64>` - Set of player IDs that will need disambiguation (have conflicting last names)
///
/// # Examples
/// ```
/// use liiga_teletext::data_fetcher::player_names::get_players_needing_disambiguation;
/// use std::collections::HashSet;
///
/// let players = vec![
///     (1, "Mikko".to_string(), "Koivu".to_string()),
///     (2, "Saku".to_string(), "Koivu".to_string()),
///     (3, "Teemu".to_string(), "Selänne".to_string()),
/// ];
///
/// let needing_disambiguation = get_players_needing_disambiguation(&players);
///
/// // Players 1 and 2 (both Koivu) need disambiguation
/// assert!(needing_disambiguation.contains(&1));
/// assert!(needing_disambiguation.contains(&2));
/// // Player 3 (Selänne) does not need disambiguation
/// assert!(!needing_disambiguation.contains(&3));
/// assert_eq!(needing_disambiguation.len(), 2);
/// ```
#[allow(dead_code)]
pub fn get_players_needing_disambiguation(players: &[(i64, String, String)]) -> HashSet<i64> {
    let mut result = HashSet::with_capacity(players.len());

    // Fast path: if 0-1 players, no disambiguation needed
    if players.len() <= 1 {
        return result;
    }

    // Fast path: if exactly 2 players with different last names, no disambiguation needed
    if players.len() == 2 {
        let (_, _, last1) = &players[0];
        let (_, _, last2) = &players[1];
        if last1.to_lowercase() != last2.to_lowercase() {
            return result;
        }
        // If both have same last name, both need disambiguation
        result.insert(players[0].0);
        result.insert(players[1].0);
        return result;
    }

    // Group players by last name (case-insensitive) using indices for efficiency
    let mut last_name_groups: HashMap<String, Vec<usize>> = HashMap::new();

    for (index, (_, _, last_name)) in players.iter().enumerate() {
        let normalized_last_name = last_name.to_lowercase();
        last_name_groups
            .entry(normalized_last_name)
            .or_default()
            .push(index);
    }

    // Add player IDs from groups that have conflicts (more than one player)
    for group_indices in last_name_groups.values() {
        if group_indices.len() > 1 {
            for &index in group_indices {
                result.insert(players[index].0);
            }
        }
    }

    result
}

/// Applies progressive disambiguation to a group of players with the same last name using indices.
/// This is an optimized version that avoids cloning strings by using indices into the original slice.
/// If single initials are sufficient, uses them. If not, extends to 2-3 characters as needed.
///
/// # Arguments
/// * `players` - The original slice of players: (player_id, first_name, last_name)
/// * `group_indices` - Indices of players with the same last name
///
/// # Returns
/// * `Vec<(i64, String)>` - A vector of (player_id, disambiguated_name) pairs
///
/// # Note
/// This is an internal function used by the optimized disambiguation system.
fn apply_progressive_disambiguation_by_indices(
    players: &[(i64, String, String)],
    group_indices: &[usize],
) -> Vec<(i64, String)> {
    let mut result = Vec::new();
    let first_index = group_indices[0];
    let formatted_last_name = format_for_display(&build_full_name("", &players[first_index].2));

    // Step 1: Try single initials - group by initial using indices
    let mut initial_groups: HashMap<String, Vec<usize>> = HashMap::new();

    for &index in group_indices {
        let (id, first_name, _) = &players[index];
        if let Some(initial) = extract_first_initial(first_name) {
            initial_groups.entry(initial).or_default().push(index);
        } else {
            // No valid initial - use last name only
            result.push((*id, formatted_last_name.clone()));
        }
    }

    // Step 2: Process each initial group
    for (initial, player_indices) in initial_groups {
        if player_indices.len() == 1 {
            // Single player with this initial - use single initial
            let index = player_indices[0];
            let (id, _, _) = &players[index];
            result.push((*id, format!("{formatted_last_name} {initial}.")));
        } else {
            // Multiple players with same initial - try extended disambiguation
            let extended_disambiguated = apply_extended_disambiguation_by_indices(
                players,
                &player_indices,
                &formatted_last_name,
            );

            // Check if extended disambiguation actually creates unique identifiers
            let mut unique_names: HashSet<String> =
                HashSet::with_capacity(extended_disambiguated.len());
            let mut all_unique = true;

            for (_, name) in &extended_disambiguated {
                if !unique_names.insert(name.clone()) {
                    all_unique = false;
                    break;
                }
            }

            if all_unique {
                // Extended disambiguation worked - use it
                result.extend(extended_disambiguated);
            } else {
                // Extended disambiguation didn't help - fall back to single initial
                for &index in &player_indices {
                    let (id, _, _) = &players[index];
                    result.push((*id, format!("{formatted_last_name} {initial}.")));
                }
            }
        }
    }

    result
}

/// Applies extended disambiguation when players share the same last name and first initial using indices.
/// This is an optimized version that avoids cloning strings by using indices into the original slice.
/// Uses 2-3 characters from the first name to create unique identifiers.
///
/// # Arguments
/// * `players` - The original slice of players
/// * `player_indices` - Indices of players with the same last name and first initial
/// * `formatted_last_name` - The already formatted last name
///
/// # Returns
/// * `Vec<(i64, String)>` - Disambiguated names using extended prefixes
fn apply_extended_disambiguation_by_indices(
    players: &[(i64, String, String)],
    player_indices: &[usize],
    formatted_last_name: &str,
) -> Vec<(i64, String)> {
    let mut result = Vec::new();

    // Try 2 characters first
    let mut char2_groups: HashMap<String, Vec<usize>> = HashMap::new();

    for &index in player_indices {
        let (id, first_name, _) = &players[index];
        if let Some(chars2) = extract_first_chars(first_name, 2) {
            char2_groups.entry(chars2).or_default().push(index);
        } else {
            // Fallback to single initial or last name only
            if let Some(initial) = extract_first_initial(first_name) {
                result.push((*id, format!("{formatted_last_name} {initial}.")));
            } else {
                result.push((*id, formatted_last_name.to_string()));
            }
        }
    }

    // Process 2-character groups
    for (chars2, indices_with_same_2chars) in char2_groups {
        if indices_with_same_2chars.len() == 1 {
            // Unique with 2 characters
            let index = indices_with_same_2chars[0];
            let (id, _, _) = &players[index];
            result.push((*id, format!("{formatted_last_name} {chars2}.")));
        } else {
            // Still conflicts, try 3 characters
            let mut char3_groups: HashMap<String, Vec<i64>> = HashMap::new();

            for &index in &indices_with_same_2chars {
                let (id, first_name, _) = &players[index];
                if let Some(chars3) = extract_first_chars(first_name, 3) {
                    char3_groups.entry(chars3).or_default().push(*id);
                } else {
                    // Fallback to 2 characters if 3 is not available
                    result.push((*id, format!("{formatted_last_name} {chars2}.")));
                }
            }

            // Process 3-character groups
            for (chars3, player_ids) in char3_groups {
                for id in player_ids {
                    result.push((id, format!("{formatted_last_name} {chars3}.")));
                }
            }
        }
    }

    result
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

/// Determines if disambiguation is needed for a given last name within a group of players.
/// This helper function checks if multiple players share the same last name (case-insensitive).
///
/// # Arguments
/// * `last_name` - The last name to check for duplicates
/// * `players` - A slice of tuples containing (player_id, first_name, last_name)
///
/// # Returns
/// * `bool` - True if multiple players share this last name
///
/// # Examples
/// ```
/// use liiga_teletext::data_fetcher::player_names::is_disambiguation_needed;
///
/// let players = vec![
///     (1, "Mikko".to_string(), "Koivu".to_string()),
///     (2, "Saku".to_string(), "Koivu".to_string()),
///     (3, "Teemu".to_string(), "Selänne".to_string()),
/// ];
///
/// assert!(is_disambiguation_needed("Koivu", &players));
/// assert!(!is_disambiguation_needed("Selänne", &players));
/// assert!(!is_disambiguation_needed("NonExistent", &players));
/// ```
#[allow(dead_code)]
pub fn is_disambiguation_needed(last_name: &str, players: &[(i64, String, String)]) -> bool {
    let normalized_last_name = last_name.to_lowercase();
    let count = players
        .iter()
        .filter(|(_, _, ln)| ln.to_lowercase() == normalized_last_name)
        .count();
    count > 1
}

/// Groups players by their last name within a team.
/// This helper function creates a mapping from normalized last names to lists of players
/// who share that last name.
///
/// # Arguments
/// * `players` - A slice of tuples containing (player_id, first_name, last_name)
///
/// # Returns
/// * `HashMap<String, Vec<(i64, String, String)>>` - A mapping from normalized last names to player groups
///
/// # Examples
/// ```
/// use liiga_teletext::data_fetcher::player_names::group_players_by_last_name;
///
/// let players = vec![
///     (1, "Mikko".to_string(), "Koivu".to_string()),
///     (2, "Saku".to_string(), "Koivu".to_string()),
///     (3, "Teemu".to_string(), "Selänne".to_string()),
/// ];
///
/// let groups = group_players_by_last_name(&players);
/// assert_eq!(groups.get("koivu").unwrap().len(), 2);
/// assert_eq!(groups.get("selänne").unwrap().len(), 1);
/// ```
#[allow(dead_code)]
pub fn group_players_by_last_name(
    players: &[(i64, String, String)],
) -> HashMap<String, Vec<(i64, String, String)>> {
    let mut groups: HashMap<String, Vec<(i64, String, String)>> = HashMap::new();

    for (id, first_name, last_name) in players {
        let normalized_last_name = last_name.to_lowercase();
        groups.entry(normalized_last_name).or_default().push((
            *id,
            first_name.clone(),
            last_name.clone(),
        ));
    }

    groups
}

/// Groups players by their last name within a team using indices to avoid cloning.
/// This optimized helper function creates a mapping from normalized last names to lists of indices
/// that reference the original player data.
///
/// # Arguments
/// * `players` - A slice of tuples containing (player_id, first_name, last_name)
///
/// # Returns
/// * `HashMap<String, Vec<usize>>` - A mapping from normalized last names to player indices
///
/// # Examples
/// ```
/// use liiga_teletext::data_fetcher::player_names::group_players_by_last_name_indices;
///
/// let players = vec![
///     (1, "Mikko".to_string(), "Koivu".to_string()),
///     (2, "Saku".to_string(), "Koivu".to_string()),
///     (3, "Teemu".to_string(), "Selänne".to_string()),
/// ];
///
/// let groups = group_players_by_last_name_indices(&players);
/// assert_eq!(groups.get("koivu").unwrap().len(), 2);
/// assert_eq!(groups.get("selänne").unwrap().len(), 1);
/// ```
#[allow(dead_code)]
pub fn group_players_by_last_name_indices(
    players: &[(i64, String, String)],
) -> HashMap<String, Vec<usize>> {
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();

    for (index, (_, _, last_name)) in players.iter().enumerate() {
        let normalized_last_name = last_name.to_lowercase();
        groups.entry(normalized_last_name).or_default().push(index);
    }

    groups
}

/// Context for managing team-scoped player name disambiguation.
/// This struct handles the disambiguation logic for a single team, ensuring that
/// players with the same last name are properly distinguished.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DisambiguationContext {
    /// The players in this team context
    pub players: Vec<(i64, String, String)>, // (id, first_name, last_name)
    /// The disambiguated names for each player
    pub disambiguated_names: HashMap<i64, String>,
}

impl DisambiguationContext {
    /// Creates a new disambiguation context for the given players.
    /// Automatically applies disambiguation rules during construction.
    ///
    /// # Arguments
    /// * `players` - A vector of tuples containing (player_id, first_name, last_name)
    ///
    /// # Returns
    /// * `DisambiguationContext` - A new context with disambiguation applied
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::data_fetcher::player_names::DisambiguationContext;
    ///
    /// let players = vec![
    ///     (1, "Mikko".to_string(), "Koivu".to_string()),
    ///     (2, "Saku".to_string(), "Koivu".to_string()),
    /// ];
    ///
    /// let context = DisambiguationContext::new(players);
    /// assert_eq!(context.get_disambiguated_name(1), Some(&"Koivu M.".to_string()));
    /// ```
    #[allow(dead_code)]
    pub fn new(players: Vec<(i64, String, String)>) -> Self {
        let disambiguated_names = format_with_disambiguation(&players);

        Self {
            players,
            disambiguated_names,
        }
    }

    /// Gets the disambiguated name for a specific player.
    ///
    /// # Arguments
    /// * `player_id` - The unique identifier for the player
    ///
    /// # Returns
    /// * `Option<&String>` - The disambiguated name if the player exists
    #[allow(dead_code)]
    pub fn get_disambiguated_name(&self, player_id: i64) -> Option<&String> {
        self.disambiguated_names.get(&player_id)
    }

    /// Checks if disambiguation is needed for players with the given last name.
    ///
    /// # Arguments
    /// * `last_name` - The last name to check
    ///
    /// # Returns
    /// * `bool` - True if multiple players share this last name
    #[allow(dead_code)]
    pub fn needs_disambiguation(&self, last_name: &str) -> bool {
        let normalized_last_name = last_name.to_lowercase();
        let count = self
            .players
            .iter()
            .filter(|(_, _, ln)| ln.to_lowercase() == normalized_last_name)
            .count();
        count > 1
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

/// Builds a disambiguation context from basic API response data for a single team.
/// This extracts player information from goal events in the team's schedule response.
///
/// # Arguments
/// * `team` - The schedule team containing goal events with embedded player data
///
/// # Returns
/// * `DisambiguationContext` - Context for player name disambiguation scoped to this team
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::player_names::build_disambiguation_from_team;
/// use liiga_teletext::data_fetcher::models::ScheduleTeam;
///
/// // Create a mock team with goal events containing player data
/// let team = ScheduleTeam::default();
///
/// let context = build_disambiguation_from_team(&team);
/// ```
pub fn build_disambiguation_from_team(team: &ScheduleTeam) -> DisambiguationContext {
    let mut players = Vec::new();

    // Extract players from team goal events
    for goal in &team.goal_events {
        if let Some(ref scorer_player) = goal.scorer_player {
            players.push((
                scorer_player.player_id,
                scorer_player.first_name.clone(),
                scorer_player.last_name.clone(),
            ));
        }
    }

    DisambiguationContext::new(players)
}

/// Builds a disambiguation context from basic API response data.
/// This extracts player information from goal events in the schedule response.
///
/// **Deprecated**: This function creates a global disambiguation context across both teams,
/// which can cause cross-team disambiguation. For team-scoped disambiguation,
/// use `build_disambiguation_from_team` instead.
///
/// # Arguments
/// * `game` - The schedule game containing goal events with embedded player data
///
/// # Returns
/// * `DisambiguationContext` - Context for player name disambiguation
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::player_names::build_disambiguation_from_basic_response;
/// use liiga_teletext::data_fetcher::models::ScheduleGame;
///
/// // Create a mock game with goal events containing player data
/// let game = ScheduleGame {
///     id: 1,
///     season: 2024,
///     start: "2024-01-15T18:30:00Z".to_string(),
///     end: Some("2024-01-15T21:00:00Z".to_string()),
///     home_team: liiga_teletext::data_fetcher::models::ScheduleTeam::default(),
///     away_team: liiga_teletext::data_fetcher::models::ScheduleTeam::default(),
///     finished_type: Some("FINISHED".to_string()),
///     started: true,
///     ended: true,
///     game_time: 3600,
///     serie: "runkosarja".to_string(),
/// };
///
/// let context = build_disambiguation_from_basic_response(&game);
/// ```
#[deprecated(
    since = "0.15.10",
    note = "Use build_disambiguation_from_team for team-scoped disambiguation"
)]
#[allow(dead_code)]
pub fn build_disambiguation_from_basic_response(game: &ScheduleGame) -> DisambiguationContext {
    let mut players = Vec::new();

    // Extract players from home team goal events
    for goal in &game.home_team.goal_events {
        if let Some(ref scorer_player) = goal.scorer_player {
            players.push((
                scorer_player.player_id,
                scorer_player.first_name.clone(),
                scorer_player.last_name.clone(),
            ));
        }
    }

    // Extract players from away team goal events
    for goal in &game.away_team.goal_events {
        if let Some(ref scorer_player) = goal.scorer_player {
            players.push((
                scorer_player.player_id,
                scorer_player.first_name.clone(),
                scorer_player.last_name.clone(),
            ));
        }
    }

    DisambiguationContext::new(players)
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

    // Tests for format_for_display_with_first_initial
    #[test]
    fn test_format_for_display_with_first_initial_basic() {
        assert_eq!(
            format_for_display_with_first_initial("Mikko", "Koivu"),
            "Koivu M."
        );
        assert_eq!(
            format_for_display_with_first_initial("Saku", "Koivu"),
            "Koivu S."
        );
        assert_eq!(
            format_for_display_with_first_initial("Teemu", "Selänne"),
            "Selänne T."
        );
    }

    #[test]
    fn test_format_for_display_with_first_initial_empty_first_name() {
        assert_eq!(format_for_display_with_first_initial("", "Koivu"), "Koivu");
        assert_eq!(
            format_for_display_with_first_initial("   ", "Koivu"),
            "Koivu"
        );
    }

    #[test]
    fn test_format_for_display_with_first_initial_unicode() {
        assert_eq!(
            format_for_display_with_first_initial("Äkäslompolo", "Koivu"),
            "Koivu Ä."
        );
        assert_eq!(
            format_for_display_with_first_initial("Östen", "Koivu"),
            "Koivu Ö."
        );
        assert_eq!(
            format_for_display_with_first_initial("Åke", "Koivu"),
            "Koivu Å."
        );
    }

    #[test]
    fn test_format_for_display_with_first_initial_multiple_words() {
        assert_eq!(
            format_for_display_with_first_initial("Jean-Pierre", "Dumont"),
            "Dumont J."
        );
        assert_eq!(
            format_for_display_with_first_initial("Mary Jane", "Watson"),
            "Watson M."
        );
    }

    #[test]
    fn test_format_for_display_with_first_initial_case_handling() {
        assert_eq!(
            format_for_display_with_first_initial("mikko", "koivu"),
            "Koivu M."
        );
        assert_eq!(
            format_for_display_with_first_initial("SAKU", "KOIVU"),
            "Koivu S."
        );
    }

    // Tests for format_with_disambiguation
    #[test]
    fn test_format_with_disambiguation_basic_two_players() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
        ];

        let result = format_with_disambiguation(&players);
        assert_eq!(result.get(&1), Some(&"Koivu M.".to_string()));
        assert_eq!(result.get(&2), Some(&"Koivu S.".to_string()));
    }

    #[test]
    fn test_format_with_disambiguation_no_disambiguation_needed() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Teemu".to_string(), "Selänne".to_string()),
            (3, "Jari".to_string(), "Kurri".to_string()),
        ];

        let result = format_with_disambiguation(&players);
        assert_eq!(result.get(&1), Some(&"Koivu".to_string()));
        assert_eq!(result.get(&2), Some(&"Selänne".to_string()));
        assert_eq!(result.get(&3), Some(&"Kurri".to_string()));
    }

    #[test]
    fn test_format_with_disambiguation_three_players_same_name() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
            (3, "Antti".to_string(), "Koivu".to_string()),
        ];

        let result = format_with_disambiguation(&players);
        assert_eq!(result.get(&1), Some(&"Koivu M.".to_string()));
        assert_eq!(result.get(&2), Some(&"Koivu S.".to_string()));
        assert_eq!(result.get(&3), Some(&"Koivu A.".to_string()));
    }

    #[test]
    fn test_format_with_disambiguation_mixed_scenario() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
            (3, "Teemu".to_string(), "Selänne".to_string()),
            (4, "Jari".to_string(), "Kurri".to_string()),
            (5, "Jere".to_string(), "Kurri".to_string()),
        ];

        let result = format_with_disambiguation(&players);
        assert_eq!(result.get(&1), Some(&"Koivu M.".to_string()));
        assert_eq!(result.get(&2), Some(&"Koivu S.".to_string()));
        assert_eq!(result.get(&3), Some(&"Selänne".to_string()));
        assert_eq!(result.get(&4), Some(&"Kurri Ja.".to_string()));
        assert_eq!(result.get(&5), Some(&"Kurri Je.".to_string()));
    }

    #[test]
    fn test_format_with_disambiguation_case_insensitive() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "KOIVU".to_string()),
            (3, "Antti".to_string(), "koivu".to_string()),
        ];

        let result = format_with_disambiguation(&players);
        assert_eq!(result.get(&1), Some(&"Koivu M.".to_string()));
        assert_eq!(result.get(&2), Some(&"Koivu S.".to_string()));
        assert_eq!(result.get(&3), Some(&"Koivu A.".to_string()));
    }

    #[test]
    fn test_format_with_disambiguation_empty_first_names() {
        let players = vec![
            (1, "".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
        ];

        let result = format_with_disambiguation(&players);
        assert_eq!(result.get(&1), Some(&"Koivu".to_string()));
        assert_eq!(result.get(&2), Some(&"Koivu S.".to_string()));
    }

    #[test]
    fn test_format_with_disambiguation_unicode_names() {
        let players = vec![
            (1, "Äkäslompolo".to_string(), "Koivu".to_string()),
            (2, "Östen".to_string(), "Koivu".to_string()),
            (3, "Åke".to_string(), "Koivu".to_string()),
        ];

        let result = format_with_disambiguation(&players);
        assert_eq!(result.get(&1), Some(&"Koivu Ä.".to_string()));
        assert_eq!(result.get(&2), Some(&"Koivu Ö.".to_string()));
        assert_eq!(result.get(&3), Some(&"Koivu Å.".to_string()));
    }

    #[test]
    fn test_format_with_disambiguation_empty_input() {
        let players = vec![];
        let result = format_with_disambiguation(&players);
        assert!(result.is_empty());
    }

    // Tests for DisambiguationContext
    #[test]
    fn test_disambiguation_context_new() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
            (3, "Teemu".to_string(), "Selänne".to_string()),
        ];

        let context = DisambiguationContext::new(players.clone());
        assert_eq!(context.players, players);
        assert_eq!(
            context.get_disambiguated_name(1),
            Some(&"Koivu M.".to_string())
        );
        assert_eq!(
            context.get_disambiguated_name(2),
            Some(&"Koivu S.".to_string())
        );
        assert_eq!(
            context.get_disambiguated_name(3),
            Some(&"Selänne".to_string())
        );
    }

    #[test]
    fn test_disambiguation_context_get_disambiguated_name() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Teemu".to_string(), "Selänne".to_string()),
        ];

        let context = DisambiguationContext::new(players);
        assert_eq!(
            context.get_disambiguated_name(1),
            Some(&"Koivu".to_string())
        );
        assert_eq!(
            context.get_disambiguated_name(2),
            Some(&"Selänne".to_string())
        );
        assert_eq!(context.get_disambiguated_name(999), None);
    }

    #[test]
    fn test_disambiguation_context_needs_disambiguation() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
            (3, "Teemu".to_string(), "Selänne".to_string()),
        ];

        let context = DisambiguationContext::new(players);
        assert!(context.needs_disambiguation("Koivu"));
        assert!(context.needs_disambiguation("koivu")); // Case insensitive
        assert!(context.needs_disambiguation("KOIVU")); // Case insensitive
        assert!(!context.needs_disambiguation("Selänne"));
        assert!(!context.needs_disambiguation("NonExistent"));
    }

    #[test]
    fn test_disambiguation_context_empty() {
        let players = vec![];
        let context = DisambiguationContext::new(players);
        assert_eq!(context.get_disambiguated_name(1), None);
        assert!(!context.needs_disambiguation("Koivu"));
    }

    #[test]
    fn test_disambiguation_context_single_player() {
        let players = vec![(1, "Mikko".to_string(), "Koivu".to_string())];

        let context = DisambiguationContext::new(players);
        assert_eq!(
            context.get_disambiguated_name(1),
            Some(&"Koivu".to_string())
        );
        assert!(!context.needs_disambiguation("Koivu"));
    }

    // Tests for extract_first_initial
    #[test]
    fn test_extract_first_initial_basic() {
        assert_eq!(extract_first_initial("Mikko"), Some("M".to_string()));
        assert_eq!(extract_first_initial("Saku"), Some("S".to_string()));
        assert_eq!(extract_first_initial("Teemu"), Some("T".to_string()));
    }

    #[test]
    fn test_extract_first_initial_unicode() {
        assert_eq!(extract_first_initial("Äkäslompolo"), Some("Ä".to_string()));
        assert_eq!(extract_first_initial("Östen"), Some("Ö".to_string()));
        assert_eq!(extract_first_initial("Åke"), Some("Å".to_string()));
    }

    #[test]
    fn test_extract_first_initial_multiple_words() {
        assert_eq!(extract_first_initial("Jean-Pierre"), Some("J".to_string()));
        assert_eq!(extract_first_initial("Mary Jane"), Some("M".to_string()));
        assert_eq!(extract_first_initial("Van Der Berg"), Some("V".to_string()));
    }

    #[test]
    fn test_extract_first_initial_case_handling() {
        assert_eq!(extract_first_initial("mikko"), Some("M".to_string()));
        assert_eq!(extract_first_initial("sAKU"), Some("S".to_string()));
        assert_eq!(extract_first_initial("tEEMU"), Some("T".to_string()));
    }

    #[test]
    fn test_extract_first_initial_empty_and_whitespace() {
        assert_eq!(extract_first_initial(""), None);
        assert_eq!(extract_first_initial("   "), None);
        assert_eq!(extract_first_initial("\t\n"), None);
    }

    #[test]
    fn test_extract_first_initial_non_alphabetic() {
        assert_eq!(extract_first_initial("123John"), None);
        assert_eq!(extract_first_initial("-Pierre"), None);
        assert_eq!(extract_first_initial("'Connor"), None);
        assert_eq!(extract_first_initial("@username"), None);
    }

    #[test]
    fn test_extract_first_initial_with_leading_whitespace() {
        assert_eq!(extract_first_initial("  Mikko"), Some("M".to_string()));
        assert_eq!(extract_first_initial("\tSaku"), Some("S".to_string()));
        assert_eq!(extract_first_initial("\n Teemu"), Some("T".to_string()));
    }

    // Tests for is_disambiguation_needed
    #[test]
    fn test_is_disambiguation_needed_basic() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
            (3, "Teemu".to_string(), "Selänne".to_string()),
        ];

        assert!(is_disambiguation_needed("Koivu", &players));
        assert!(!is_disambiguation_needed("Selänne", &players));
        assert!(!is_disambiguation_needed("NonExistent", &players));
    }

    #[test]
    fn test_is_disambiguation_needed_case_insensitive() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "KOIVU".to_string()),
            (3, "Antti".to_string(), "koivu".to_string()),
        ];

        assert!(is_disambiguation_needed("Koivu", &players));
        assert!(is_disambiguation_needed("koivu", &players));
        assert!(is_disambiguation_needed("KOIVU", &players));
        assert!(is_disambiguation_needed("KoIvU", &players));
    }

    #[test]
    fn test_is_disambiguation_needed_single_player() {
        let players = vec![(1, "Mikko".to_string(), "Koivu".to_string())];

        assert!(!is_disambiguation_needed("Koivu", &players));
        assert!(!is_disambiguation_needed("NonExistent", &players));
    }

    #[test]
    fn test_is_disambiguation_needed_empty_players() {
        let players = vec![];

        assert!(!is_disambiguation_needed("Koivu", &players));
        assert!(!is_disambiguation_needed("", &players));
    }

    #[test]
    fn test_is_disambiguation_needed_three_players_same_name() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
            (3, "Antti".to_string(), "Koivu".to_string()),
        ];

        assert!(is_disambiguation_needed("Koivu", &players));
    }

    #[test]
    fn test_is_disambiguation_needed_unicode_names() {
        let players = vec![
            (1, "Äkäslompolo".to_string(), "Kärppä".to_string()),
            (2, "Östen".to_string(), "Kärppä".to_string()),
        ];

        assert!(is_disambiguation_needed("Kärppä", &players));
        assert!(is_disambiguation_needed("kärppä", &players));
    }

    // Tests for group_players_by_last_name
    #[test]
    fn test_group_players_by_last_name_basic() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
            (3, "Teemu".to_string(), "Selänne".to_string()),
        ];

        let groups = group_players_by_last_name(&players);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups.get("koivu").unwrap().len(), 2);
        assert_eq!(groups.get("selänne").unwrap().len(), 1);

        let koivu_group = groups.get("koivu").unwrap();
        assert!(koivu_group.contains(&(1, "Mikko".to_string(), "Koivu".to_string())));
        assert!(koivu_group.contains(&(2, "Saku".to_string(), "Koivu".to_string())));
    }

    #[test]
    fn test_group_players_by_last_name_case_insensitive() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "KOIVU".to_string()),
            (3, "Antti".to_string(), "koivu".to_string()),
        ];

        let groups = group_players_by_last_name(&players);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups.get("koivu").unwrap().len(), 3);
    }

    #[test]
    fn test_group_players_by_last_name_all_unique() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Teemu".to_string(), "Selänne".to_string()),
            (3, "Jari".to_string(), "Kurri".to_string()),
        ];

        let groups = group_players_by_last_name(&players);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups.get("koivu").unwrap().len(), 1);
        assert_eq!(groups.get("selänne").unwrap().len(), 1);
        assert_eq!(groups.get("kurri").unwrap().len(), 1);
    }

    #[test]
    fn test_group_players_by_last_name_empty() {
        let players = vec![];
        let groups = group_players_by_last_name(&players);

        assert!(groups.is_empty());
    }

    #[test]
    fn test_group_players_by_last_name_single_player() {
        let players = vec![(1, "Mikko".to_string(), "Koivu".to_string())];
        let groups = group_players_by_last_name(&players);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups.get("koivu").unwrap().len(), 1);
    }

    #[test]
    fn test_group_players_by_last_name_unicode() {
        let players = vec![
            (1, "Äkäslompolo".to_string(), "Kärppä".to_string()),
            (2, "Östen".to_string(), "Kärppä".to_string()),
            (3, "Åke".to_string(), "Björklund".to_string()),
        ];

        let groups = group_players_by_last_name(&players);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups.get("kärppä").unwrap().len(), 2);
        assert_eq!(groups.get("björklund").unwrap().len(), 1);
    }

    #[test]
    fn test_group_players_by_last_name_complex_scenario() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
            (3, "Antti".to_string(), "Koivu".to_string()),
            (4, "Jari".to_string(), "Kurri".to_string()),
            (5, "Jere".to_string(), "Kurri".to_string()),
            (6, "Teemu".to_string(), "Selänne".to_string()),
        ];

        let groups = group_players_by_last_name(&players);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups.get("koivu").unwrap().len(), 3);
        assert_eq!(groups.get("kurri").unwrap().len(), 2);
        assert_eq!(groups.get("selänne").unwrap().len(), 1);
    }

    #[test]
    fn test_group_players_by_last_name_preserves_original_case() {
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "KOIVU".to_string()),
        ];

        let groups = group_players_by_last_name(&players);
        let koivu_group = groups.get("koivu").unwrap();

        // Check that original case is preserved in the stored data
        assert!(koivu_group.contains(&(1, "Mikko".to_string(), "Koivu".to_string())));
        assert!(koivu_group.contains(&(2, "Saku".to_string(), "KOIVU".to_string())));
    }

    // Comprehensive disambiguation logic tests for task 6

    #[test]
    fn test_comprehensive_basic_two_player_disambiguation() {
        // Test basic two-player disambiguation scenario (e.g., "Koivu M." and "Koivu S.")
        // Requirements: 1.1, 1.2, 2.1, 2.2
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
        ];

        let result = format_with_disambiguation(&players);

        // Both players should be disambiguated with first initials
        assert_eq!(result.get(&1), Some(&"Koivu M.".to_string()));
        assert_eq!(result.get(&2), Some(&"Koivu S.".to_string()));
        assert_eq!(result.len(), 2);

        // Verify the disambiguation context also works correctly
        let context = DisambiguationContext::new(players);
        assert_eq!(
            context.get_disambiguated_name(1),
            Some(&"Koivu M.".to_string())
        );
        assert_eq!(
            context.get_disambiguated_name(2),
            Some(&"Koivu S.".to_string())
        );
        assert!(context.needs_disambiguation("Koivu"));
    }

    #[test]
    fn test_comprehensive_no_disambiguation_needed_unique_names() {
        // Test no disambiguation needed when all last names are unique
        // Requirements: 1.1, 1.2, 2.1, 2.2
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Teemu".to_string(), "Selänne".to_string()),
            (3, "Jari".to_string(), "Kurri".to_string()),
            (4, "Sami".to_string(), "Kapanen".to_string()),
        ];

        let result = format_with_disambiguation(&players);

        // All players should display with last name only (no first initial)
        assert_eq!(result.get(&1), Some(&"Koivu".to_string()));
        assert_eq!(result.get(&2), Some(&"Selänne".to_string()));
        assert_eq!(result.get(&3), Some(&"Kurri".to_string()));
        assert_eq!(result.get(&4), Some(&"Kapanen".to_string()));
        assert_eq!(result.len(), 4);

        // Verify the disambiguation context
        let context = DisambiguationContext::new(players);
        assert_eq!(
            context.get_disambiguated_name(1),
            Some(&"Koivu".to_string())
        );
        assert_eq!(
            context.get_disambiguated_name(2),
            Some(&"Selänne".to_string())
        );
        assert_eq!(
            context.get_disambiguated_name(3),
            Some(&"Kurri".to_string())
        );
        assert_eq!(
            context.get_disambiguated_name(4),
            Some(&"Kapanen".to_string())
        );

        // None of the names should need disambiguation
        assert!(!context.needs_disambiguation("Koivu"));
        assert!(!context.needs_disambiguation("Selänne"));
        assert!(!context.needs_disambiguation("Kurri"));
        assert!(!context.needs_disambiguation("Kapanen"));
    }

    #[test]
    fn test_comprehensive_multiple_players_same_name_three_plus() {
        // Test multiple players with same last name (3+ players)
        // Requirements: 1.1, 1.2, 2.1, 2.2
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
            (3, "Antti".to_string(), "Koivu".to_string()),
            (4, "Petri".to_string(), "Koivu".to_string()),
        ];

        let result = format_with_disambiguation(&players);

        // All four players should be disambiguated with first initials
        assert_eq!(result.get(&1), Some(&"Koivu M.".to_string()));
        assert_eq!(result.get(&2), Some(&"Koivu S.".to_string()));
        assert_eq!(result.get(&3), Some(&"Koivu A.".to_string()));
        assert_eq!(result.get(&4), Some(&"Koivu P.".to_string()));
        assert_eq!(result.len(), 4);

        // Verify the disambiguation context
        let context = DisambiguationContext::new(players);
        assert_eq!(
            context.get_disambiguated_name(1),
            Some(&"Koivu M.".to_string())
        );
        assert_eq!(
            context.get_disambiguated_name(2),
            Some(&"Koivu S.".to_string())
        );
        assert_eq!(
            context.get_disambiguated_name(3),
            Some(&"Koivu A.".to_string())
        );
        assert_eq!(
            context.get_disambiguated_name(4),
            Some(&"Koivu P.".to_string())
        );
        assert!(context.needs_disambiguation("Koivu"));
    }

    #[test]
    fn test_comprehensive_cross_team_scenarios_no_disambiguation() {
        // Test cross-team scenarios where same last names on different teams don't disambiguate
        // This simulates the team-scoped disambiguation requirement
        // Requirements: 1.1, 1.2, 2.1, 2.2

        // Simulate home team with one Koivu
        let home_team_players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Teemu".to_string(), "Selänne".to_string()),
        ];

        // Simulate away team with one Koivu
        let away_team_players = vec![
            (3, "Saku".to_string(), "Koivu".to_string()),
            (4, "Jari".to_string(), "Kurri".to_string()),
        ];

        // Process each team separately (as would happen in real team-scoped disambiguation)
        let home_result = format_with_disambiguation(&home_team_players);
        let away_result = format_with_disambiguation(&away_team_players);

        // Both Koivu players should display without disambiguation since they're on different teams
        assert_eq!(home_result.get(&1), Some(&"Koivu".to_string())); // Home team Koivu
        assert_eq!(home_result.get(&2), Some(&"Selänne".to_string()));
        assert_eq!(away_result.get(&3), Some(&"Koivu".to_string())); // Away team Koivu
        assert_eq!(away_result.get(&4), Some(&"Kurri".to_string()));

        // Verify disambiguation contexts for each team
        let home_context = DisambiguationContext::new(home_team_players);
        let away_context = DisambiguationContext::new(away_team_players);

        // Neither team should need disambiguation for Koivu since there's only one per team
        assert!(!home_context.needs_disambiguation("Koivu"));
        assert!(!away_context.needs_disambiguation("Koivu"));

        assert_eq!(
            home_context.get_disambiguated_name(1),
            Some(&"Koivu".to_string())
        );
        assert_eq!(
            away_context.get_disambiguated_name(3),
            Some(&"Koivu".to_string())
        );
    }

    #[test]
    fn test_comprehensive_mixed_team_scenario_with_cross_team_same_names() {
        // Test a more complex scenario with multiple same names within teams and across teams
        // Requirements: 1.1, 1.2, 2.1, 2.2

        // Home team: 2 Koivus, 1 Selänne
        let home_team_players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Antti".to_string(), "Koivu".to_string()),
            (3, "Teemu".to_string(), "Selänne".to_string()),
        ];

        // Away team: 1 Koivu, 2 Kurris
        let away_team_players = vec![
            (4, "Saku".to_string(), "Koivu".to_string()),
            (5, "Jari".to_string(), "Kurri".to_string()),
            (6, "Jere".to_string(), "Kurri".to_string()),
        ];

        let home_result = format_with_disambiguation(&home_team_players);
        let away_result = format_with_disambiguation(&away_team_players);

        // Home team: Koivus should be disambiguated, Selänne should not
        assert_eq!(home_result.get(&1), Some(&"Koivu M.".to_string()));
        assert_eq!(home_result.get(&2), Some(&"Koivu A.".to_string()));
        assert_eq!(home_result.get(&3), Some(&"Selänne".to_string()));

        // Away team: Kurris should be disambiguated, Koivu should not (only one on this team)
        assert_eq!(away_result.get(&4), Some(&"Koivu".to_string()));
        assert_eq!(away_result.get(&5), Some(&"Kurri Ja.".to_string()));
        assert_eq!(away_result.get(&6), Some(&"Kurri Je.".to_string()));

        // Verify disambiguation contexts
        let home_context = DisambiguationContext::new(home_team_players);
        let away_context = DisambiguationContext::new(away_team_players);

        // Home team should need disambiguation for Koivu but not Selänne
        assert!(home_context.needs_disambiguation("Koivu"));
        assert!(!home_context.needs_disambiguation("Selänne"));

        // Away team should need disambiguation for Kurri but not Koivu
        assert!(!away_context.needs_disambiguation("Koivu"));
        assert!(away_context.needs_disambiguation("Kurri"));
    }

    #[test]
    fn test_comprehensive_edge_cases_in_disambiguation() {
        // Test edge cases that might occur in real disambiguation scenarios
        // Requirements: 1.1, 1.2, 2.1, 2.2

        let players = vec![
            // Two players with same last name, one with empty first name
            (1, "".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
            // Two players with same last name and same first initial
            (3, "Jari".to_string(), "Kurri".to_string()),
            (4, "Jere".to_string(), "Kurri".to_string()),
            // Player with unique name
            (5, "Teemu".to_string(), "Selänne".to_string()),
        ];

        let result = format_with_disambiguation(&players);

        // Koivu with empty first name should fall back to last name only
        // Koivu with first name should get first initial
        assert_eq!(result.get(&1), Some(&"Koivu".to_string()));
        assert_eq!(result.get(&2), Some(&"Koivu S.".to_string()));

        // Both Kurris should get extended disambiguation (Ja. and Je.)
        assert_eq!(result.get(&3), Some(&"Kurri Ja.".to_string()));
        assert_eq!(result.get(&4), Some(&"Kurri Je.".to_string()));

        // Selänne should remain unique
        assert_eq!(result.get(&5), Some(&"Selänne".to_string()));

        // Verify disambiguation context
        let context = DisambiguationContext::new(players);
        assert!(context.needs_disambiguation("Koivu"));
        assert!(context.needs_disambiguation("Kurri"));
        assert!(!context.needs_disambiguation("Selänne"));
    }

    #[test]
    fn test_comprehensive_case_insensitive_disambiguation() {
        // Test that disambiguation works correctly with different case variations
        // Requirements: 1.1, 1.2, 2.1, 2.2

        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "KOIVU".to_string()),
            (3, "Antti".to_string(), "koivu".to_string()),
            (4, "Teemu".to_string(), "Selänne".to_string()),
        ];

        let result = format_with_disambiguation(&players);

        // All three Koivu variants should be disambiguated
        assert_eq!(result.get(&1), Some(&"Koivu M.".to_string()));
        assert_eq!(result.get(&2), Some(&"Koivu S.".to_string()));
        assert_eq!(result.get(&3), Some(&"Koivu A.".to_string()));
        assert_eq!(result.get(&4), Some(&"Selänne".to_string()));

        // Verify disambiguation context recognizes case-insensitive matches
        let context = DisambiguationContext::new(players);
        assert!(context.needs_disambiguation("Koivu"));
        assert!(context.needs_disambiguation("koivu"));
        assert!(context.needs_disambiguation("KOIVU"));
        assert!(context.needs_disambiguation("KoIvU"));
        assert!(!context.needs_disambiguation("Selänne"));
    }

    #[test]
    fn test_comprehensive_unicode_character_disambiguation() {
        // Test disambiguation with Finnish characters (ä, ö, å)
        // Requirements: 1.1, 1.2, 2.1, 2.2

        let players = vec![
            (1, "Äkäslompolo".to_string(), "Kärppä".to_string()),
            (2, "Östen".to_string(), "Kärppä".to_string()),
            (3, "Åke".to_string(), "Kärppä".to_string()),
            (4, "Mikko".to_string(), "Selänne".to_string()),
        ];

        let result = format_with_disambiguation(&players);

        // All three Kärppä players should be disambiguated with proper Unicode initials
        assert_eq!(result.get(&1), Some(&"Kärppä Ä.".to_string()));
        assert_eq!(result.get(&2), Some(&"Kärppä Ö.".to_string()));
        assert_eq!(result.get(&3), Some(&"Kärppä Å.".to_string()));
        assert_eq!(result.get(&4), Some(&"Selänne".to_string()));

        // Verify disambiguation context
        let context = DisambiguationContext::new(players);
        assert!(context.needs_disambiguation("Kärppä"));
        assert!(!context.needs_disambiguation("Selänne"));

        // Test case-insensitive matching with Unicode
        assert!(context.needs_disambiguation("kärppä"));
        assert!(context.needs_disambiguation("KÄRPPÄ"));
    }

    #[test]
    fn test_comprehensive_empty_and_single_player_scenarios() {
        // Test edge cases with empty player lists and single players
        // Requirements: 1.1, 1.2, 2.1, 2.2

        // Test empty player list
        let empty_players = vec![];
        let empty_result = format_with_disambiguation(&empty_players);
        assert!(empty_result.is_empty());

        let empty_context = DisambiguationContext::new(empty_players);
        assert_eq!(empty_context.get_disambiguated_name(1), None);
        assert!(!empty_context.needs_disambiguation("Koivu"));

        // Test single player
        let single_player = vec![(1, "Mikko".to_string(), "Koivu".to_string())];
        let single_result = format_with_disambiguation(&single_player);
        assert_eq!(single_result.get(&1), Some(&"Koivu".to_string()));
        assert_eq!(single_result.len(), 1);

        let single_context = DisambiguationContext::new(single_player);
        assert_eq!(
            single_context.get_disambiguated_name(1),
            Some(&"Koivu".to_string())
        );
        assert!(!single_context.needs_disambiguation("Koivu"));
    }

    // Edge Case Tests for Task 7: Add edge case handling and error resilience tests
    // Requirements: 1.4, 4.1, 4.2, 4.3, 4.4

    #[test]
    fn test_edge_case_empty_and_missing_first_names() {
        // Test handling of empty or missing first names
        // Requirements: 4.1 - When a player's first name is missing or empty THEN the system SHALL fall back to displaying only the last name

        let players = vec![
            // Empty first name
            (1, "".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
            // Whitespace-only first name
            (3, "   ".to_string(), "Lindström".to_string()),
            (4, "Erik".to_string(), "Lindström".to_string()),
            // Tab and newline characters
            (5, "\t\n".to_string(), "Granlund".to_string()),
            (6, "Mikael".to_string(), "Granlund".to_string()),
            // Single player with empty first name (no disambiguation needed)
            (7, "".to_string(), "Selänne".to_string()),
        ];

        let result = format_with_disambiguation(&players);

        // Players with empty first names should fall back to last name only
        // while their teammates with valid first names get disambiguation
        assert_eq!(result.get(&1), Some(&"Koivu".to_string()));
        assert_eq!(result.get(&2), Some(&"Koivu S.".to_string()));

        assert_eq!(result.get(&3), Some(&"Lindström".to_string()));
        assert_eq!(result.get(&4), Some(&"Lindström E.".to_string()));

        assert_eq!(result.get(&5), Some(&"Granlund".to_string()));
        assert_eq!(result.get(&6), Some(&"Granlund M.".to_string()));

        // Single player with empty first name should just show last name
        assert_eq!(result.get(&7), Some(&"Selänne".to_string()));

        // Test individual function behavior
        assert_eq!(format_for_display_with_first_initial("", "Koivu"), "Koivu");
        assert_eq!(
            format_for_display_with_first_initial("   ", "Koivu"),
            "Koivu"
        );
        assert_eq!(
            format_for_display_with_first_initial("\t\n", "Koivu"),
            "Koivu"
        );

        // Test extract_first_initial with empty inputs
        assert_eq!(extract_first_initial(""), None);
        assert_eq!(extract_first_initial("   "), None);
        assert_eq!(extract_first_initial("\t\n\r "), None);
    }

    #[test]
    fn test_edge_case_unicode_finnish_characters() {
        // Test Unicode character support for Finnish names (ä, ö, å)
        // Requirements: 1.4 - When processing player names THEN the system SHALL handle Finnish characters (ä, ö, å) correctly

        let players = vec![
            // Finnish characters in first names
            (1, "Äkäslompolo".to_string(), "Kärppä".to_string()),
            (2, "Östen".to_string(), "Kärppä".to_string()),
            (3, "Åke".to_string(), "Kärppä".to_string()),
            // Finnish characters in last names
            (4, "Mikko".to_string(), "Kärppä".to_string()),
            (5, "Saku".to_string(), "Kärppä".to_string()),
            // Mixed case Finnish characters
            (6, "äkäslompolo".to_string(), "Lönnberg".to_string()),
            (7, "ÖSTEN".to_string(), "Lönnberg".to_string()),
            // Complex Finnish names
            (8, "Väinö".to_string(), "Kääriäinen".to_string()),
            (9, "Yrjö".to_string(), "Kääriäinen".to_string()),
        ];

        let result = format_with_disambiguation(&players);

        // Test proper Unicode handling in disambiguation
        assert_eq!(result.get(&1), Some(&"Kärppä Ä.".to_string()));
        assert_eq!(result.get(&2), Some(&"Kärppä Ö.".to_string()));
        assert_eq!(result.get(&3), Some(&"Kärppä Å.".to_string()));
        assert_eq!(result.get(&4), Some(&"Kärppä M.".to_string()));
        assert_eq!(result.get(&5), Some(&"Kärppä S.".to_string()));

        // Test case handling with Finnish characters
        assert_eq!(result.get(&6), Some(&"Lönnberg Ä.".to_string()));
        assert_eq!(result.get(&7), Some(&"Lönnberg Ö.".to_string()));

        // Test complex Finnish names
        assert_eq!(result.get(&8), Some(&"Kääriäinen V.".to_string()));
        assert_eq!(result.get(&9), Some(&"Kääriäinen Y.".to_string()));

        // Test individual function behavior with Unicode
        assert_eq!(
            format_for_display_with_first_initial("Äkäslompolo", "Kärppä"),
            "Kärppä Ä."
        );
        assert_eq!(
            format_for_display_with_first_initial("östen", "lönnberg"),
            "Lönnberg Ö."
        );
        assert_eq!(
            format_for_display_with_first_initial("ÅKE", "KÄRPPÄ"),
            "Kärppä Å."
        );

        // Test extract_first_initial with Finnish characters
        assert_eq!(extract_first_initial("Äkäslompolo"), Some("Ä".to_string()));
        assert_eq!(extract_first_initial("östen"), Some("Ö".to_string()));
        assert_eq!(extract_first_initial("ÅKE"), Some("Å".to_string()));
        assert_eq!(extract_first_initial("väinö"), Some("V".to_string()));

        // Test case-insensitive grouping with Finnish characters
        let context = DisambiguationContext::new(players);
        assert!(context.needs_disambiguation("Kärppä"));
        assert!(context.needs_disambiguation("kärppä"));
        assert!(context.needs_disambiguation("KÄRPPÄ"));
    }

    #[test]
    fn test_edge_case_multiple_words_and_hyphens() {
        // Test handling of first names with multiple words or hyphens
        // Requirements: 4.2 - When a player's first name contains multiple words THEN the system SHALL use the first letter of the first word

        let players = vec![
            // Hyphenated first names
            (1, "Jean-Pierre".to_string(), "Dumont".to_string()),
            (2, "Jean-Luc".to_string(), "Dumont".to_string()),
            (3, "Marie-Claire".to_string(), "Dubois".to_string()),
            (4, "Anne-Marie".to_string(), "Dubois".to_string()),
            // Multiple word first names (space separated)
            (5, "Mary Jane".to_string(), "Watson".to_string()),
            (6, "Mary Lou".to_string(), "Watson".to_string()),
            (7, "Van Der".to_string(), "Berg".to_string()),
            (8, "Van Den".to_string(), "Berg".to_string()),
            // Mixed hyphen and space combinations
            (9, "Jean-Pierre Louis".to_string(), "Martin".to_string()),
            (10, "Jean-Claude Van".to_string(), "Martin".to_string()),
            // Names with apostrophes
            (11, "O'Connor".to_string(), "Smith".to_string()),
            (12, "O'Brien".to_string(), "Smith".to_string()),
        ];

        let result = format_with_disambiguation(&players);

        // Test hyphenated names - should use first letter of first part
        assert_eq!(result.get(&1), Some(&"Dumont J.".to_string()));
        assert_eq!(result.get(&2), Some(&"Dumont J.".to_string()));
        assert_eq!(result.get(&3), Some(&"Dubois M.".to_string()));
        assert_eq!(result.get(&4), Some(&"Dubois A.".to_string()));

        // Test multiple word names - should use first letter of first word
        assert_eq!(result.get(&5), Some(&"Watson M.".to_string()));
        assert_eq!(result.get(&6), Some(&"Watson M.".to_string()));
        assert_eq!(result.get(&7), Some(&"Berg V.".to_string()));
        assert_eq!(result.get(&8), Some(&"Berg V.".to_string()));

        // Test complex combinations
        assert_eq!(result.get(&9), Some(&"Martin J.".to_string()));
        assert_eq!(result.get(&10), Some(&"Martin J.".to_string()));

        // Test apostrophe names
        assert_eq!(result.get(&11), Some(&"Smith O.".to_string()));
        assert_eq!(result.get(&12), Some(&"Smith O.".to_string()));

        // Test individual function behavior
        assert_eq!(
            format_for_display_with_first_initial("Jean-Pierre", "Dumont"),
            "Dumont J."
        );
        assert_eq!(
            format_for_display_with_first_initial("Mary Jane", "Watson"),
            "Watson M."
        );
        assert_eq!(
            format_for_display_with_first_initial("Van Der", "Berg"),
            "Berg V."
        );
        assert_eq!(
            format_for_display_with_first_initial("O'Connor", "Smith"),
            "Smith O."
        );

        // Test extract_first_initial with complex names
        assert_eq!(extract_first_initial("Jean-Pierre"), Some("J".to_string()));
        assert_eq!(extract_first_initial("Mary Jane"), Some("M".to_string()));
        assert_eq!(extract_first_initial("Van Der Berg"), Some("V".to_string()));
        assert_eq!(extract_first_initial("O'Connor"), Some("O".to_string()));
    }

    #[test]
    fn test_edge_case_non_alphabetic_first_characters() {
        // Test handling of first names starting with non-alphabetic characters
        // Requirements: 4.3 - When a player's first name starts with a non-alphabetic character THEN the system SHALL handle it gracefully

        let players = vec![
            // Names starting with numbers
            (1, "123John".to_string(), "Smith".to_string()),
            (2, "456Jane".to_string(), "Smith".to_string()),
            // Names starting with symbols
            (3, "-Pierre".to_string(), "Dubois".to_string()),
            (4, "'Connor".to_string(), "Dubois".to_string()),
            (5, "@username".to_string(), "Johnson".to_string()),
            (6, "#hashtag".to_string(), "Johnson".to_string()),
            // Names with leading punctuation
            (7, ".hidden".to_string(), "Brown".to_string()),
            (8, "!exclaim".to_string(), "Brown".to_string()),
            // Mixed valid and invalid starting characters
            (9, "Normal".to_string(), "Wilson".to_string()),
            (10, "123Invalid".to_string(), "Wilson".to_string()),
        ];

        let result = format_with_disambiguation(&players);

        // Players with non-alphabetic first characters should fall back to last name only
        // while players with valid first names get disambiguation
        assert_eq!(result.get(&1), Some(&"Smith".to_string()));
        assert_eq!(result.get(&2), Some(&"Smith".to_string()));

        assert_eq!(result.get(&3), Some(&"Dubois".to_string()));
        assert_eq!(result.get(&4), Some(&"Dubois".to_string()));

        assert_eq!(result.get(&5), Some(&"Johnson".to_string()));
        assert_eq!(result.get(&6), Some(&"Johnson".to_string()));

        assert_eq!(result.get(&7), Some(&"Brown".to_string()));
        assert_eq!(result.get(&8), Some(&"Brown".to_string()));

        // Mixed scenario - valid name gets initial, invalid falls back
        assert_eq!(result.get(&9), Some(&"Wilson N.".to_string()));
        assert_eq!(result.get(&10), Some(&"Wilson".to_string()));

        // Test individual function behavior
        assert_eq!(
            format_for_display_with_first_initial("123John", "Smith"),
            "Smith"
        );
        assert_eq!(
            format_for_display_with_first_initial("-Pierre", "Dubois"),
            "Dubois"
        );
        assert_eq!(
            format_for_display_with_first_initial("@username", "Johnson"),
            "Johnson"
        );
        assert_eq!(
            format_for_display_with_first_initial("Normal", "Wilson"),
            "Wilson N."
        );

        // Test extract_first_initial with non-alphabetic characters
        assert_eq!(extract_first_initial("123John"), None);
        assert_eq!(extract_first_initial("-Pierre"), None);
        assert_eq!(extract_first_initial("@username"), None);
        assert_eq!(extract_first_initial("!exclaim"), None);
        assert_eq!(extract_first_initial(".hidden"), None);
        assert_eq!(extract_first_initial("Normal"), Some("N".to_string()));
    }

    #[test]
    fn test_edge_case_incomplete_player_data() {
        // Test graceful degradation when player data is incomplete
        // Requirements: 4.4 - When player data is incomplete THEN the system SHALL not break the disambiguation logic for other players

        let players = vec![
            // Complete data
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
            // Empty first name but valid last name
            (3, "".to_string(), "Lindström".to_string()),
            (4, "Erik".to_string(), "Lindström".to_string()),
            // Empty last name (edge case)
            (5, "Teemu".to_string(), "".to_string()),
            (6, "Jari".to_string(), "".to_string()),
            // Both names empty
            (7, "".to_string(), "".to_string()),
            // Whitespace-only names
            (8, "   ".to_string(), "Granlund".to_string()),
            (9, "Mikael".to_string(), "   ".to_string()),
            // Valid player that should work normally
            (10, "Teemu".to_string(), "Selänne".to_string()),
        ];

        let result = format_with_disambiguation(&players);

        // Complete data should work normally
        assert_eq!(result.get(&1), Some(&"Koivu M.".to_string()));
        assert_eq!(result.get(&2), Some(&"Koivu S.".to_string()));

        // Empty first name should fall back gracefully
        assert_eq!(result.get(&3), Some(&"Lindström".to_string()));
        assert_eq!(result.get(&4), Some(&"Lindström E.".to_string()));

        // Empty last names should be handled (though unusual)
        assert!(result.contains_key(&5));
        assert!(result.contains_key(&6));

        // Both names empty should be handled
        assert!(result.contains_key(&7));

        // Whitespace-only should be handled
        assert!(result.contains_key(&8));
        assert!(result.contains_key(&9));

        // Normal player should work fine despite other incomplete data
        assert_eq!(result.get(&10), Some(&"Selänne".to_string()));

        // Test that the system doesn't crash with incomplete data
        let context = DisambiguationContext::new(players);

        // Should be able to query for any player without crashing
        assert!(context.get_disambiguated_name(1).is_some());
        assert!(context.get_disambiguated_name(7).is_some());
        assert!(context.get_disambiguated_name(999).is_none());

        // Should handle disambiguation queries gracefully
        assert!(context.needs_disambiguation("Koivu"));
        assert!(!context.needs_disambiguation("Selänne"));
        // Empty string will match empty last names, and we have 2 players with empty last names
        assert!(context.needs_disambiguation(""));
    }

    #[test]
    fn test_edge_case_extreme_unicode_and_special_characters() {
        // Test handling of extreme Unicode cases and special characters
        // Requirements: 1.4, 4.1, 4.2, 4.3

        let players = vec![
            // Emoji in names (should be handled gracefully)
            (1, "😀John".to_string(), "Smith".to_string()),
            (2, "Jane😀".to_string(), "Smith".to_string()),
            // Extended Unicode characters
            (3, "Žofia".to_string(), "Novák".to_string()),
            (4, "Łukasz".to_string(), "Novák".to_string()),
            // Combining characters
            (5, "José".to_string(), "García".to_string()),
            (6, "María".to_string(), "García".to_string()),
            // Right-to-left script characters (Arabic)
            (7, "محمد".to_string(), "Johnson".to_string()),
            (8, "أحمد".to_string(), "Johnson".to_string()),
            // Mixed scripts
            (9, "Иван".to_string(), "Petrov".to_string()),
            (10, "Владимир".to_string(), "Petrov".to_string()),
        ];

        let result = format_with_disambiguation(&players);

        // System should handle all cases without crashing
        assert_eq!(result.len(), 10);

        // Emoji characters should not be considered alphabetic
        assert_eq!(result.get(&1), Some(&"Smith".to_string()));
        assert_eq!(result.get(&2), Some(&"Smith J.".to_string()));

        // Extended Unicode should work
        assert_eq!(result.get(&3), Some(&"Novák Ž.".to_string()));
        assert_eq!(result.get(&4), Some(&"Novák Ł.".to_string()));

        // Accented characters should work
        assert_eq!(result.get(&5), Some(&"García J.".to_string()));
        assert_eq!(result.get(&6), Some(&"García M.".to_string()));

        // Non-Latin scripts should work
        assert!(result.contains_key(&7));
        assert!(result.contains_key(&8));
        assert!(result.contains_key(&9));
        assert!(result.contains_key(&10));

        // Test extract_first_initial with various Unicode
        assert_eq!(extract_first_initial("😀John"), None); // Emoji not alphabetic
        assert_eq!(extract_first_initial("Žofia"), Some("Ž".to_string()));
        assert_eq!(extract_first_initial("Łukasz"), Some("Ł".to_string()));
        assert_eq!(extract_first_initial("José"), Some("J".to_string()));
        assert_eq!(extract_first_initial("María"), Some("M".to_string()));

        // Test that the system remains stable with extreme inputs
        let context = DisambiguationContext::new(players);
        assert!(context.needs_disambiguation("Smith"));
        assert!(context.needs_disambiguation("Novák"));
        assert!(context.needs_disambiguation("García"));
    }

    #[test]
    fn test_extended_disambiguation_same_initial() {
        // Test that players with same last name and first initial get extended disambiguation
        let players = vec![
            (1, "Mikael".to_string(), "Granlund".to_string()),
            (2, "Markus".to_string(), "Granlund".to_string()),
            (3, "Teemu".to_string(), "Selänne".to_string()),
        ];

        let result = format_with_disambiguation(&players);

        // Granlund players should get extended disambiguation (Mi., Ma.)
        assert_eq!(result.get(&1), Some(&"Granlund Mi.".to_string()));
        assert_eq!(result.get(&2), Some(&"Granlund Ma.".to_string()));
        // Selänne should remain unique
        assert_eq!(result.get(&3), Some(&"Selänne".to_string()));
    }

    #[test]
    fn test_extended_disambiguation_three_characters() {
        // Test extreme case where 2 characters are still not enough
        let players = vec![
            (1, "Michael".to_string(), "Smith".to_string()),
            (2, "Michelle".to_string(), "Smith".to_string()),
            (3, "Mikhail".to_string(), "Smith".to_string()),
        ];

        let result = format_with_disambiguation(&players);

        // Should fall back to single initial since 3 characters don't help
        assert_eq!(result.get(&1), Some(&"Smith M.".to_string()));
        assert_eq!(result.get(&2), Some(&"Smith M.".to_string())); // Same as Michael - they can't be uniquely disambiguated
        assert_eq!(result.get(&3), Some(&"Smith M.".to_string()));
    }

    #[test]
    fn test_mixed_disambiguation_levels() {
        // Test mix of single initial and extended disambiguation
        let players = vec![
            (1, "Mikael".to_string(), "Granlund".to_string()),
            (2, "Markus".to_string(), "Granlund".to_string()),
            (3, "Jari".to_string(), "Granlund".to_string()),
            (4, "Teemu".to_string(), "Selänne".to_string()),
        ];

        let result = format_with_disambiguation(&players);

        // M* players should get extended disambiguation
        assert_eq!(result.get(&1), Some(&"Granlund Mi.".to_string()));
        assert_eq!(result.get(&2), Some(&"Granlund Ma.".to_string()));
        // J player should get single initial
        assert_eq!(result.get(&3), Some(&"Granlund J.".to_string()));
        // Unique player should not be disambiguated
        assert_eq!(result.get(&4), Some(&"Selänne".to_string()));
    }

    #[test]
    fn test_extract_first_chars() {
        // Test the new helper function
        assert_eq!(extract_first_chars("Mikael", 2), Some("Mi".to_string()));
        assert_eq!(extract_first_chars("Markus", 2), Some("Ma".to_string()));
        assert_eq!(
            extract_first_chars("Äkäslompolo", 3),
            Some("Äkä".to_string())
        );
        assert_eq!(
            extract_first_chars("Jean-Pierre", 3),
            Some("Jea".to_string())
        );
        assert_eq!(extract_first_chars("M", 2), Some("M".to_string())); // Short name
        assert_eq!(extract_first_chars("", 2), None); // Empty name
        assert_eq!(extract_first_chars("123John", 2), Some("Jo".to_string())); // Skip non-alphabetic
    }

    // Fast path optimization tests
    #[test]
    fn test_fast_path_empty_players() {
        // Test fast path for empty input
        let players = vec![];
        let result = format_with_disambiguation(&players);
        assert!(result.is_empty());
    }

    #[test]
    fn test_fast_path_single_player() {
        // Test fast path for single player
        let players = vec![(1, "Mikko".to_string(), "Koivu".to_string())];
        let result = format_with_disambiguation(&players);
        assert_eq!(result.len(), 1);
        assert_eq!(result.get(&1), Some(&"Koivu".to_string()));
    }

    #[test]
    fn test_fast_path_two_different_players() {
        // Test fast path for two players with different last names
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Teemu".to_string(), "Selänne".to_string()),
        ];
        let result = format_with_disambiguation(&players);
        assert_eq!(result.len(), 2);
        assert_eq!(result.get(&1), Some(&"Koivu".to_string()));
        assert_eq!(result.get(&2), Some(&"Selänne".to_string()));
    }

    #[test]
    fn test_fast_path_two_same_players_falls_through() {
        // Test that two players with same last name fall through to full algorithm
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
        ];
        let result = format_with_disambiguation(&players);
        assert_eq!(result.len(), 2);
        // Should get disambiguation (not fast path)
        assert_eq!(result.get(&1), Some(&"Koivu M.".to_string()));
        assert_eq!(result.get(&2), Some(&"Koivu S.".to_string()));
    }

    #[test]
    fn test_fast_path_case_insensitive_matching() {
        // Test that fast path correctly handles case-insensitive last name comparison
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "KOIVU".to_string()),
        ];
        let result = format_with_disambiguation(&players);
        assert_eq!(result.len(), 2);
        // Should fall through to full algorithm and get disambiguation
        assert_eq!(result.get(&1), Some(&"Koivu M.".to_string()));
        assert_eq!(result.get(&2), Some(&"Koivu S.".to_string()));
    }

    #[test]
    fn test_fast_path_unicode_last_names() {
        // Test fast path with Unicode last names
        let players = vec![
            (1, "Mikko".to_string(), "Kärppä".to_string()),
            (2, "Teemu".to_string(), "Selänne".to_string()),
        ];
        let result = format_with_disambiguation(&players);
        assert_eq!(result.len(), 2);
        assert_eq!(result.get(&1), Some(&"Kärppä".to_string()));
        assert_eq!(result.get(&2), Some(&"Selänne".to_string()));
    }

    #[test]
    fn test_fast_path_vs_full_algorithm_consistency() {
        // Verify fast path produces identical results to full algorithm for edge cases

        // Single player case
        let single_player = vec![(1, "Mikko".to_string(), "Koivu".to_string())];
        let fast_result = format_with_disambiguation(&single_player);

        // Manually compute what full algorithm would produce
        let mut expected = HashMap::new();
        expected.insert(1, "Koivu".to_string());
        assert_eq!(fast_result, expected);

        // Two different players case
        let two_different = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Teemu".to_string(), "Selänne".to_string()),
        ];
        let fast_result = format_with_disambiguation(&two_different);

        let mut expected = HashMap::new();
        expected.insert(1, "Koivu".to_string());
        expected.insert(2, "Selänne".to_string());
        assert_eq!(fast_result, expected);
    }

    // Batch disambiguation check tests
    #[test]
    fn test_get_players_needing_disambiguation_empty() {
        // Test empty input
        let players = vec![];
        let result = get_players_needing_disambiguation(&players);
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_players_needing_disambiguation_single_player() {
        // Test single player - no disambiguation needed
        let players = vec![(1, "Mikko".to_string(), "Koivu".to_string())];
        let result = get_players_needing_disambiguation(&players);
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_players_needing_disambiguation_two_different() {
        // Test two players with different last names - no disambiguation needed
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Teemu".to_string(), "Selänne".to_string()),
        ];
        let result = get_players_needing_disambiguation(&players);
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_players_needing_disambiguation_two_same() {
        // Test two players with same last name - both need disambiguation
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
        ];
        let result = get_players_needing_disambiguation(&players);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&1));
        assert!(result.contains(&2));
    }

    #[test]
    fn test_get_players_needing_disambiguation_mixed_scenario() {
        // Test mixed scenario: some conflicts, some unique
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
            (3, "Teemu".to_string(), "Selänne".to_string()),
            (4, "Jari".to_string(), "Kurri".to_string()),
            (5, "Jere".to_string(), "Kurri".to_string()),
        ];
        let result = get_players_needing_disambiguation(&players);
        assert_eq!(result.len(), 4);
        // Koivu players need disambiguation
        assert!(result.contains(&1));
        assert!(result.contains(&2));
        // Selänne does not need disambiguation (unique)
        assert!(!result.contains(&3));
        // Kurri players need disambiguation
        assert!(result.contains(&4));
        assert!(result.contains(&5));
    }

    #[test]
    fn test_get_players_needing_disambiguation_case_insensitive() {
        // Test case-insensitive last name matching
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "KOIVU".to_string()),
            (3, "Antti".to_string(), "koivu".to_string()),
        ];
        let result = get_players_needing_disambiguation(&players);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&1));
        assert!(result.contains(&2));
        assert!(result.contains(&3));
    }

    #[test]
    fn test_get_players_needing_disambiguation_unicode() {
        // Test Unicode characters in last names
        let players = vec![
            (1, "Mikko".to_string(), "Kärppä".to_string()),
            (2, "Saku".to_string(), "Kärppä".to_string()),
            (3, "Teemu".to_string(), "Björklund".to_string()),
        ];
        let result = get_players_needing_disambiguation(&players);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&1));
        assert!(result.contains(&2));
        assert!(!result.contains(&3));
    }

    #[test]
    fn test_get_players_needing_disambiguation_all_unique() {
        // Test all players with unique last names
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Teemu".to_string(), "Selänne".to_string()),
            (3, "Jari".to_string(), "Kurri".to_string()),
            (4, "Sami".to_string(), "Kapanen".to_string()),
        ];
        let result = get_players_needing_disambiguation(&players);
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_players_needing_disambiguation_all_same() {
        // Test all players with same last name
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
            (3, "Antti".to_string(), "Koivu".to_string()),
            (4, "Petri".to_string(), "Koivu".to_string()),
        ];
        let result = get_players_needing_disambiguation(&players);
        assert_eq!(result.len(), 4);
        assert!(result.contains(&1));
        assert!(result.contains(&2));
        assert!(result.contains(&3));
        assert!(result.contains(&4));
    }

    #[test]
    fn test_get_players_needing_disambiguation_large_dataset() {
        // Test with larger dataset for performance
        let mut players = Vec::new();

        // Add 50 players: 10 Koivus, 10 Selännes, 30 unique names
        for i in 0..10 {
            players.push((i, format!("First{i}"), "Koivu".to_string()));
        }
        for i in 10..20 {
            players.push((i, format!("First{i}"), "Selänne".to_string()));
        }
        for i in 20..50 {
            players.push((i, format!("First{i}"), format!("Unique{i}")));
        }

        let result = get_players_needing_disambiguation(&players);
        assert_eq!(result.len(), 20); // 10 Koivus + 10 Selännes

        // Check that all Koivus and Selännes are marked as needing disambiguation
        for i in 0..20 {
            assert!(result.contains(&i));
        }
        // Check that unique names are not marked
        for i in 20..50 {
            assert!(!result.contains(&i));
        }
    }

    #[test]
    fn test_get_players_needing_disambiguation_consistency_with_format_function() {
        // Test that this function is consistent with actual disambiguation behavior
        let players = vec![
            (1, "Mikko".to_string(), "Koivu".to_string()),
            (2, "Saku".to_string(), "Koivu".to_string()),
            (3, "Teemu".to_string(), "Selänne".to_string()),
            (4, "Jari".to_string(), "Kurri".to_string()),
            (5, "Jere".to_string(), "Kurri".to_string()),
        ];

        let needing_disambiguation = get_players_needing_disambiguation(&players);
        let actual_disambiguation = format_with_disambiguation(&players);

        // Every player that needs disambiguation should appear in the actual result
        for &player_id in &needing_disambiguation {
            assert!(actual_disambiguation.contains_key(&player_id));
            // And their name should include disambiguation (contain ".")
            let name = actual_disambiguation.get(&player_id).unwrap();
            assert!(
                name.contains('.'),
                "Player {player_id} name '{name}' should be disambiguated"
            );
        }

        // Every player that doesn't need disambiguation should not have "." in their name
        for (player_id, name) in &actual_disambiguation {
            if !needing_disambiguation.contains(player_id) {
                assert!(
                    !name.contains('.'),
                    "Player {player_id} name '{name}' should not be disambiguated"
                );
            }
        }

        // The total number of players should match
        assert_eq!(actual_disambiguation.len(), players.len());
    }

    #[test]
    fn test_edge_case_performance_with_large_datasets() {
        // Test that disambiguation performs well with larger datasets
        // Requirements: 4.4 - System should handle large numbers of players efficiently

        let mut players = Vec::new();

        // Create 100 players with various name patterns
        for i in 0..100 {
            let first_name = match i % 10 {
                0 => "Mikko".to_string(),
                1 => "Saku".to_string(),
                2 => "Teemu".to_string(),
                3 => "Jari".to_string(),
                4 => "Antti".to_string(),
                5 => "".to_string(),            // Empty first name
                6 => "Jean-Pierre".to_string(), // Hyphenated
                7 => "Mary Jane".to_string(),   // Multiple words
                8 => "Äkäslompolo".to_string(), // Finnish characters
                _ => format!("Player{i}"),
            };

            let last_name = match i % 5 {
                0 => "Koivu".to_string(),
                1 => "Selänne".to_string(),
                2 => "Kurri".to_string(),
                3 => "Lindström".to_string(),
                _ => format!("Lastname{i}"),
            };

            players.push((i as i64, first_name, last_name));
        }

        // This should complete without performance issues
        let result = format_with_disambiguation(&players);
        assert_eq!(result.len(), 100);

        // Test context creation with large dataset
        let context = DisambiguationContext::new(players);

        // Should be able to query efficiently
        assert!(context.get_disambiguated_name(0).is_some());
        assert!(context.get_disambiguated_name(99).is_some());
        assert!(context.get_disambiguated_name(1000).is_none());

        // Should handle disambiguation queries efficiently
        assert!(context.needs_disambiguation("Koivu"));
        assert!(context.needs_disambiguation("Selänne"));
    }
}
