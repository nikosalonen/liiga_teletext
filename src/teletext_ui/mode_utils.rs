// src/teletext_ui/mode_utils.rs - Mode validation and utility functions

use super::core::{TeletextPage, TeletextRow};

impl TeletextPage {
    /// Returns whether compact mode is enabled.
    ///
    /// # Returns
    /// * `bool` - True if compact mode is enabled, false otherwise
    #[allow(dead_code)] // Used in tests
    pub fn is_compact_mode(&self) -> bool {
        self.compact_mode
    }

    /// Sets the compact mode state.
    /// Compact mode and wide mode are mutually exclusive.
    ///
    /// # Arguments
    /// * `compact` - Whether to enable compact mode
    ///
    /// # Returns
    /// * `Result<(), &'static str>` - Ok if successful, Err with message if there's a conflict
    #[allow(dead_code)] // Used in tests
    pub fn set_compact_mode(&mut self, compact: bool) -> Result<(), &'static str> {
        if compact && self.wide_mode {
            // Automatically disable wide mode
            self.wide_mode = false;
        }

        self.compact_mode = compact;
        Ok(())
    }

    /// Returns whether wide mode is enabled.
    ///
    /// # Returns
    /// * `bool` - True if wide mode is enabled, false otherwise
    #[allow(dead_code)] // Used in tests
    pub fn is_wide_mode(&self) -> bool {
        self.wide_mode
    }

    /// Sets the wide mode state.
    /// Compact mode and wide mode are mutually exclusive.
    ///
    /// # Arguments
    /// * `wide` - Whether to enable wide mode
    ///
    /// # Returns
    /// * `Result<(), &'static str>` - Ok if successful, Err with message if there's a conflict
    #[allow(dead_code)] // Used in tests
    pub fn set_wide_mode(&mut self, wide: bool) -> Result<(), &'static str> {
        if wide && self.compact_mode {
            // Automatically disable compact mode
            self.compact_mode = false;
        }

        self.wide_mode = wide;
        Ok(())
    }

    /// Validates that compact mode and wide mode are not both enabled.
    /// This method should be called after manual field modifications to ensure consistency.
    ///
    /// # Returns
    /// * `Result<(), &'static str>` - Ok if valid, Err with message if invalid
    #[allow(dead_code)] // Used in tests
    pub fn validate_mode_exclusivity(&self) -> Result<(), &'static str> {
        if self.compact_mode && self.wide_mode {
            Err("compact_mode and wide_mode cannot be enabled simultaneously")
        } else {
            Ok(())
        }
    }

    /// Checks if the terminal width is sufficient for wide mode display.
    /// Wide mode requires at least 100 characters to display two full-width columns effectively.
    ///
    /// # Returns
    /// * `bool` - True if terminal width supports wide mode, false otherwise
    pub fn can_fit_two_pages(&self) -> bool {
        if !self.wide_mode {
            return false;
        }

        // Get terminal width, fallback to reasonable default if can't get size
        let terminal_width = if self.ignore_height_limit {
            // In non-interactive mode, use appropriate width for wide mode
            if self.wide_mode { 136 } else { 80 }
        } else {
            crossterm::terminal::size()
                .map(|(width, _)| width as usize)
                .unwrap_or(80)
        };

        // Wide mode requires minimum width for two normal-width columns plus gap
        // Each column: 60 chars, gap: 8 chars, margins: 4 chars = 128 chars total
        terminal_width >= 128
    }

    /// Checks if this page contains any error messages.
    /// Used to identify loading pages or error pages that need restoration.
    pub fn has_error_messages(&self) -> bool {
        self.content_rows
            .iter()
            .any(|row| matches!(row, TeletextRow::ErrorMessage(_)))
    }

    /// Test-friendly accessor to check if the page contains an error message with specific text.
    /// This method is primarily intended for testing to avoid exposing private content_rows.
    ///
    /// # Arguments
    /// * `message` - The error message text to search for
    ///
    /// # Returns
    /// * `bool` - True if an error message containing the specified text is found
    #[allow(dead_code)]
    pub fn has_error_message(&self, message: &str) -> bool {
        self.content_rows.iter().any(|row| match row {
            TeletextRow::ErrorMessage(msg) => msg.contains(message),
            _ => false,
        })
    }
}
