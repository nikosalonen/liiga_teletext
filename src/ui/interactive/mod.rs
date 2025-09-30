//! Interactive UI module for the liiga_teletext application
//!
//! This module is organized into focused submodules:
//! - `series_utils`: Tournament series type classification and display
//! - `change_detection`: Game data change tracking and hashing
//! - `indicators`: Loading and auto-refresh indicator management
//! - `refresh_manager`: Auto-refresh timing and logic
//! - `input_handler`: Keyboard input and date navigation
//! - `core`: Main interactive UI loop and orchestration

mod change_detection;
mod core;
mod indicators;
mod input_handler;
mod refresh_manager;
mod series_utils;

// Re-export all public items from core for backward compatibility
pub use core::*;
