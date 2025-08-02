pub mod content_adapter;
pub mod interactive;
pub mod layout;
pub mod resize;

pub use content_adapter::{
    AdaptedGameContent, ContentAdapter, DetailedTimeInfo, EnhancedGameDisplay, ExpandedGoalDetail,
    ExtendedTeamInfo,
};
pub use interactive::run_interactive_ui;
pub use layout::{ContentPositioning, DetailLevel, LayoutCalculator, LayoutConfig};
pub use resize::ResizeHandler;
