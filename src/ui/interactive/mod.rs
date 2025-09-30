//! Interactive UI module for the liiga_teletext application
//!
//! This module is organized into focused submodules:
//! - `series_utils`: Tournament series type classification and display
//! - `change_detection`: Game data change tracking and hashing
//! - `indicators`: Loading and auto-refresh indicator management
//! - `refresh_manager`: Auto-refresh timing and logic
//! - `input_handler`: Keyboard input and date navigation
//! - `state_manager`: State management and organization
//! - `event_handler`: Event processing and coordination
//! - `navigation_manager`: Page navigation and creation management
//! - `refresh_coordinator`: Auto-refresh operations and data fetching coordination
//! - `core`: Main interactive UI loop and orchestration

mod change_detection;
mod core;
mod event_handler;
mod indicators;
mod input_handler;
pub mod navigation_manager;
mod refresh_coordinator;
mod refresh_manager;
mod series_utils;
mod state_manager;

// Re-export all public items from core for backward compatibility
pub use core::*;
