// src/teletext_ui/mod.rs - Modular teletext UI system

pub mod compact_mode_rendering;
pub mod content;
pub mod core;
pub mod footer;
pub mod formatting;
pub mod game_display;
pub mod indicators;
pub mod layout;
pub mod mode_utils;
pub mod pagination;
pub mod rendering;
pub mod score_formatting;
pub mod season_utils;
pub mod utils;
pub mod validation;
pub mod wide_mode;

// Re-export all public types and functions for backward compatibility
pub use core::*;

// Re-export utilities
