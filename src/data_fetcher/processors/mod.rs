pub mod core;
pub mod game_status;
pub mod goal_events;
pub mod player_fetching;
pub mod time_formatting;

// Re-export all public items from core for backward compatibility
pub use core::*;

// Re-export game status functions
pub use game_status::{determine_game_status, format_time};

// Re-export goal event processing functions
pub use goal_events::{
    create_basic_goal_events, process_goal_events, process_goal_events_with_disambiguation,
    process_team_goals, process_team_goals_with_disambiguation,
};

// Re-export time formatting functions
pub use time_formatting::{should_show_todays_games, should_show_todays_games_with_time};

// Re-export player fetching functions
pub use player_fetching::try_fetch_player_names_for_game;
