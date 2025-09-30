pub mod core;
pub mod game_status;

// Re-export all public items from core for backward compatibility
pub use core::*;

// Re-export game status functions
pub use game_status::{determine_game_status, format_time};