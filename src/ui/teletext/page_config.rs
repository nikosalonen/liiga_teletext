//! Teletext page configuration

/// Configuration for creating a TeletextPage.
/// Provides a more ergonomic API for functions with many parameters.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Used in tests
pub struct TeletextPageConfig {
    pub page_number: u16,
    pub title: String,
    pub subheader: String,
    pub disable_video_links: bool,
    pub show_footer: bool,
    pub ignore_height_limit: bool,
    pub compact_mode: bool,
    pub wide_mode: bool,
}

impl TeletextPageConfig {
    #[allow(dead_code)] // Used in tests
    pub fn new(page_number: u16, title: String, subheader: String) -> Self {
        Self {
            page_number,
            title,
            subheader,
            disable_video_links: false,
            show_footer: true,
            ignore_height_limit: false,
            compact_mode: false,
            wide_mode: false,
        }
    }

    /// Sets compact mode, automatically disabling wide mode if both were enabled.
    /// Compact mode and wide mode are mutually exclusive.
    ///
    /// # Arguments
    /// * `compact` - Whether to enable compact mode
    #[allow(dead_code)] // Used in tests
    pub fn set_compact_mode(&mut self, compact: bool) {
        self.compact_mode = compact;
        if compact && self.wide_mode {
            self.wide_mode = false;
        }
    }

    /// Sets wide mode, automatically disabling compact mode if both were enabled.
    /// Compact mode and wide mode are mutually exclusive.
    ///
    /// # Arguments
    /// * `wide` - Whether to enable wide mode
    #[allow(dead_code)] // Used in tests
    pub fn set_wide_mode(&mut self, wide: bool) {
        self.wide_mode = wide;
        if wide && self.compact_mode {
            self.compact_mode = false;
        }
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
}
