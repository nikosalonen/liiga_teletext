//! Player name disambiguation utilities.
//!
//! This module provides functions for:
//! - Disambiguating players with the same last name on a team
//! - Progressive disambiguation (single initial → 2 chars → 3 chars)
//! - Checking which players need disambiguation
//! - Grouping players by last name
//! - Managing team-scoped disambiguation contexts

use std::collections::{HashMap, HashSet};

use super::formatting::{
    build_full_name, extract_first_chars, extract_first_initial, format_for_display,
};

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