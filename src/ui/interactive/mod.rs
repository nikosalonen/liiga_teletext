//! Interactive UI module for the liiga_teletext application
//!
//! This module is organized into focused submodules:
//! - `series_utils`: Tournament series type classification and display
//! - `change_detection`: Game data change tracking and hashing
//! - `core`: Main interactive UI loop and orchestration

mod change_detection;
mod core;
mod series_utils;

// Re-export all public items from core for backward compatibility
pub use core::*;
