pub mod colors;
pub mod compact_display;
pub mod page_config;

// Re-export for backward compatibility
#[allow(unused_imports)]
pub use compact_display::{CompactDisplayConfig, CompactModeValidation, TerminalWidthValidation};
#[allow(unused_imports)]
pub use page_config::TeletextPageConfig;
