// src/teletext_ui/mod.rs - Modular teletext UI system

pub mod core;
pub mod pagination;
pub mod mode_utils;
pub mod indicators;
pub mod content;
pub mod formatting;
pub mod rendering;
pub mod validation;
pub mod utils;
pub mod season_utils;
pub mod footer;
pub mod score_formatting;
pub mod wide_mode;

// Re-export all public types and functions for backward compatibility
pub use core::*;

// Re-export utilities
pub use utils::get_ansi_code;
pub use season_utils::calculate_days_until_regular_season;