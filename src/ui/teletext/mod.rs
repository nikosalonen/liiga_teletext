pub mod colors;
pub mod compact_display;

// Re-export for backward compatibility
#[allow(unused_imports)]
pub use compact_display::{CompactDisplayConfig, CompactModeValidation, TerminalWidthValidation};
