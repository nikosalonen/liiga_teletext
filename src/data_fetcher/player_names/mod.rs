//! Player name formatting and disambiguation utilities.
//!
//! This module provides comprehensive player name handling:
//! - Basic formatting for teletext display
//! - Name disambiguation for players with the same last name
//! - Fallback name generation for missing player data

mod core;

// Re-export all public items from core for backward compatibility
pub use core::*;