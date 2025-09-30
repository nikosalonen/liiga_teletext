//! Player name formatting and disambiguation utilities.
//!
//! This module provides comprehensive player name handling:
//! - Basic formatting for teletext display
//! - Name disambiguation for players with the same last name
//! - Fallback name generation for missing player data
//!
//! The module is organized into two main components:
//! - `formatting`: Basic name formatting, display helpers, and initial extraction
//! - `disambiguation`: Advanced name disambiguation for teams with duplicate last names

// Submodules
mod disambiguation;
mod formatting;

// Re-export public items from formatting
#[allow(unused_imports)]
pub use formatting::{
    build_full_name, create_fallback_name, extract_first_chars, extract_first_initial,
    format_for_display, format_for_display_with_first_initial,
};

// Re-export public items from disambiguation
#[allow(unused_imports)]
pub use disambiguation::{
    DisambiguationContext, format_with_disambiguation, get_players_needing_disambiguation,
    group_players_by_last_name, group_players_by_last_name_indices, is_disambiguation_needed,
};
