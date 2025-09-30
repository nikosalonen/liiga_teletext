// src/teletext_ui/indicators.rs - Loading indicators, error warnings, and state management utilities

use super::core::TeletextPage;
use crate::ui::teletext::loading_indicator::LoadingIndicator;

impl TeletextPage {
    /// Shows a loading indicator with the specified message
    pub fn show_loading(&mut self, message: String) {
        self.loading_indicator = Some(LoadingIndicator::new(message));
    }

    /// Hides the loading indicator
    pub fn hide_loading(&mut self) {
        self.loading_indicator = None;
    }

    /// Updates the loading indicator animation frame
    #[allow(dead_code)] // Used in tests and future UI updates
    pub fn update_loading_animation(&mut self) {
        if let Some(ref mut indicator) = self.loading_indicator {
            indicator.next_frame();
        }
    }

    /// Shows a subtle auto-refresh indicator in the footer
    pub fn show_auto_refresh_indicator(&mut self) {
        self.auto_refresh_indicator = Some(LoadingIndicator::new("Päivitetään...".to_string()));
    }

    /// Hides the auto-refresh indicator
    pub fn hide_auto_refresh_indicator(&mut self) {
        self.auto_refresh_indicator = None;
    }

    /// Updates the auto-refresh indicator animation
    pub fn update_auto_refresh_animation(&mut self) {
        if let Some(ref mut indicator) = self.auto_refresh_indicator {
            indicator.next_frame();
        }
    }

    /// Checks if the auto-refresh indicator is active
    pub fn is_auto_refresh_indicator_active(&self) -> bool {
        self.auto_refresh_indicator.is_some()
    }

    /// Shows an error warning indicator in the footer
    pub fn show_error_warning(&mut self) {
        self.error_warning_active = true;
    }

    /// Hides the error warning indicator in the footer
    pub fn hide_error_warning(&mut self) {
        self.error_warning_active = false;
    }

    /// Returns whether the error warning indicator is active
    #[allow(dead_code)] // Reserved for future use/tests
    pub fn is_error_warning_active(&self) -> bool {
        self.error_warning_active
    }

    /// Sets whether auto-refresh should be disabled for this page.
    /// Useful for pages showing only future/scheduled games that don't need frequent updates.
    pub fn set_auto_refresh_disabled(&mut self, disabled: bool) {
        self.auto_refresh_disabled = disabled;
    }

    /// Gets whether auto-refresh is disabled for this page.
    /// Returns true if automatic updates are disabled.
    pub fn is_auto_refresh_disabled(&self) -> bool {
        self.auto_refresh_disabled
    }

    /// Sets the screen height for testing purposes.
    /// This method is primarily used in tests to avoid terminal size detection issues.
    #[allow(dead_code)] // Used in integration tests
    pub fn set_screen_height(&mut self, height: u16) {
        self.screen_height = height;
    }

    /// Sets the fetched date to display in the header.
    /// This helps users understand which date's data they're viewing.
    pub fn set_fetched_date(&mut self, date: String) {
        self.fetched_date = Some(date);
    }
}