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
pub use formatting::{
    build_full_name, create_fallback_name,
    format_for_display,
};

// Re-export public items from disambiguation
pub use disambiguation::{
    DisambiguationContext, format_with_disambiguation,
};
