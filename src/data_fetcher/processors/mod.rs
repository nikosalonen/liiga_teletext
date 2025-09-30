pub mod core;
pub mod game_status;
pub mod goal_events;

// Re-export all public items from core for backward compatibility
pub use core::*;

// Re-export game status functions
pub use game_status::{determine_game_status, format_time};

// Re-export goal event processing functions
pub use goal_events::{
    create_basic_goal_events, process_goal_events, process_goal_events_with_disambiguation,
    process_team_goals, process_team_goals_with_disambiguation,
};
