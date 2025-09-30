pub mod components;
pub mod interactive;
pub mod teletext;

pub use interactive::navigation_manager::format_date_for_display;
pub use interactive::run_interactive_ui;

// Re-export NavigationManager for external use
pub use interactive::navigation_manager::NavigationManager;
