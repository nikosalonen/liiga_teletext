// src/teletext_ui/mod.rs - Modular teletext UI system

pub mod utils;
pub mod season_utils;
pub mod core;

// Re-export all public types and functions for backward compatibility
pub use core::*;

// Re-export utilities
pub use utils::get_ansi_code;
pub use season_utils::calculate_days_until_regular_season;