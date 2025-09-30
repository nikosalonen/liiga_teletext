// src/teletext_ui/validation.rs - Validation utilities for TeletextPage mode compatibility and terminal width checking

use super::core::{TeletextPage, TeletextRow};
use crate::ui::teletext::compact_display::CompactModeValidation;

#[cfg(test)]
use crate::ui::teletext::compact_display::CompactDisplayConfig;

impl TeletextPage {
    /// Validates compact mode compatibility with current page settings.
    /// Checks for potential issues and provides warnings for crowded displays.
    ///
    /// # Returns
    /// * `CompactModeValidation` - Validation result with any issues found
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::TeletextPage;
    /// use liiga_teletext::ui::teletext::compact_display::CompactModeValidation;
    ///
    /// let page = TeletextPage::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    ///     false,
    ///     true,
    ///     false,
    ///     true, // compact_mode enabled
    ///     false,
    /// );
    ///
    /// let validation = page.validate_compact_mode_compatibility();
    /// match validation {
    ///     CompactModeValidation::Compatible => {
    ///         println!("Compact mode is fully compatible");
    ///     }
    ///     CompactModeValidation::CompatibleWithWarnings { warnings } => {
    ///         println!("Compact mode works but with warnings: {:?}", warnings);
    ///     }
    ///     CompactModeValidation::Incompatible { issues } => {
    ///         println!("Compact mode not compatible: {:?}", issues);
    ///     }
    /// }
    /// ```
    pub fn validate_compact_mode_compatibility(&self) -> CompactModeValidation {
        let issues: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        // Error messages are now properly handled in compact mode
        // No need for warning anymore

        // Loading indicators and auto-refresh indicators work fine in compact mode
        // No need for warnings anymore

        // Season countdown is now properly handled - suppressed when compact mode is enabled
        // No need for warning anymore

        // Future games headers are now properly supported in compact mode
        // No need for warning anymore

        // Check if we have many games (compact mode might be crowded)
        let game_count = self
            .content_rows
            .iter()
            .filter(|row| matches!(row, TeletextRow::GameResult { .. }))
            .count();

        if game_count > 20 {
            warnings.push("Many games detected - compact mode may be crowded".to_string());
        }

        if issues.is_empty() && warnings.is_empty() {
            CompactModeValidation::Compatible
        } else {
            CompactModeValidation::CompatibleWithWarnings { warnings }
        }
    }

    /// Calculates the optimal number of games per line for the current terminal width.
    /// This is primarily used in test scenarios to validate compact display behavior.
    ///
    /// # Arguments
    /// * `terminal_width` - Current terminal width in characters
    ///
    /// # Returns
    /// * `usize` - Optimal number of games per line based on terminal width and display config
    ///
    /// # Note
    /// This function is only available in test builds to support unit testing of compact mode layout.
    #[cfg(test)]
    pub fn calculate_compact_games_per_line(&self, terminal_width: usize) -> usize {
        let config = CompactDisplayConfig::default();
        config.calculate_games_per_line(terminal_width)
    }

    /// Checks if the current terminal width can accommodate compact mode display.
    /// This is primarily used in test scenarios to validate terminal width requirements.
    ///
    /// # Arguments
    /// * `terminal_width` - Current terminal width in characters
    ///
    /// # Returns
    /// * `bool` - True if terminal is wide enough for compact mode, false otherwise
    ///
    /// # Note
    /// This function is only available in test builds to support unit testing of compact mode compatibility.
    #[cfg(test)]
    pub fn is_terminal_suitable_for_compact(&self, terminal_width: usize) -> bool {
        let config = CompactDisplayConfig::default();
        config.is_terminal_width_sufficient(terminal_width)
    }
}
