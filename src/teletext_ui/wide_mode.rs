//! Wide mode display logic for teletext UI
//!
//! This module handles wide-screen display functionality, including:
//! - Two-column layout management
//! - Wide terminal detection and validation
//! - Column-based content rendering

use crossterm::terminal;

/// Minimum terminal width required for wide mode display
#[allow(dead_code)]
const MIN_WIDE_TERMINAL_WIDTH: u16 = 128;

/// Column width for each side in wide mode
#[allow(dead_code)]
const WIDE_MODE_COLUMN_WIDTH: u16 = 64;

/// Configuration for wide mode display
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct WideModeConfig {
    pub enabled: bool,
    pub min_terminal_width: u16,
    pub column_width: u16,
    pub column_separator_width: u16,
}

impl Default for WideModeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_terminal_width: MIN_WIDE_TERMINAL_WIDTH,
            column_width: WIDE_MODE_COLUMN_WIDTH,
            column_separator_width: 4,
        }
    }
}

/// Manages wide mode display logic and layout
#[allow(dead_code)]
pub struct WideModeManager {
    config: WideModeConfig,
}

#[allow(dead_code)]
impl WideModeManager {
    /// Create a new wide mode manager
    pub fn new(config: WideModeConfig) -> Self {
        Self { config }
    }

    /// Create a wide mode manager with default configuration
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new(WideModeConfig::default())
    }

    /// Check if the terminal can accommodate wide mode display
    ///
    /// # Arguments
    /// * `ignore_height_limit` - Whether to ignore terminal height limits (non-interactive mode)
    ///
    /// # Returns
    /// * `bool` - Whether wide mode can be used
    pub fn can_fit_two_pages(&self, ignore_height_limit: bool) -> bool {
        if !self.config.enabled {
            return false;
        }

        let terminal_width = if ignore_height_limit {
            // In non-interactive mode, assume a wide terminal (like 136 columns)
            136
        } else {
            // In interactive mode, get actual terminal size or fallback to 80
            terminal::size().map(|(width, _)| width).unwrap_or(80)
        };

        terminal_width >= self.config.min_terminal_width
    }

    /// Check if the terminal can accommodate wide mode display with a specific terminal width
    /// This method is primarily for testing purposes to avoid terminal state dependencies
    ///
    /// # Arguments
    /// * `terminal_width` - The terminal width to check against
    ///
    /// # Returns
    /// * `bool` - Whether wide mode can be used
    #[cfg(test)]
    pub fn can_fit_two_pages_with_width(&self, terminal_width: u16) -> bool {
        if !self.config.enabled {
            return false;
        }

        terminal_width >= self.config.min_terminal_width
    }

    /// Calculate the starting column position for the right column
    ///
    /// # Returns
    /// * `usize` - The x-coordinate where the right column should start
    pub fn get_right_column_start(&self) -> usize {
        (self.config.column_width + self.config.column_separator_width) as usize
    }

    /// Get the effective width for content in each column
    ///
    /// # Returns
    /// * `usize` - The usable width for content in each column
    pub fn get_column_content_width(&self) -> usize {
        self.config.column_width as usize - 4 // Account for margins
    }

    /// Check if wide mode is enabled
    ///
    /// # Returns
    /// * `bool` - Whether wide mode is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Enable or disable wide mode
    ///
    /// # Arguments
    /// * `enabled` - Whether to enable wide mode
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    /// Get the current configuration
    ///
    /// # Returns
    /// * `&WideModeConfig` - Reference to the current configuration
    pub fn config(&self) -> &WideModeConfig {
        &self.config
    }

    /// Validate terminal capabilities for wide mode
    ///
    /// # Arguments
    /// * `ignore_height_limit` - Whether to ignore terminal height limits
    ///
    /// # Returns
    /// * `WideModeValidation` - Validation result with details
    pub fn validate_terminal_for_wide_mode(&self, ignore_height_limit: bool) -> WideModeValidation {
        let terminal_width = if ignore_height_limit {
            136 // Assume wide terminal in non-interactive mode
        } else {
            terminal::size().map(|(width, _)| width).unwrap_or(80)
        };

        if !self.config.enabled {
            WideModeValidation::Disabled
        } else if terminal_width >= self.config.min_terminal_width {
            WideModeValidation::Suitable {
                terminal_width,
                required_width: self.config.min_terminal_width,
                excess_width: terminal_width - self.config.min_terminal_width,
            }
        } else {
            WideModeValidation::TooNarrow {
                terminal_width,
                required_width: self.config.min_terminal_width,
                shortfall: self.config.min_terminal_width - terminal_width,
            }
        }
    }

    /// Validate terminal capabilities for wide mode with a specific terminal width
    /// This method is primarily for testing purposes to avoid terminal state dependencies
    ///
    /// # Arguments
    /// * `terminal_width` - The terminal width to validate against
    ///
    /// # Returns
    /// * `WideModeValidation` - Validation result with details
    #[cfg(test)]
    pub fn validate_terminal_for_wide_mode_with_width(
        &self,
        terminal_width: u16,
    ) -> WideModeValidation {
        if !self.config.enabled {
            WideModeValidation::Disabled
        } else if terminal_width >= self.config.min_terminal_width {
            WideModeValidation::Suitable {
                terminal_width,
                required_width: self.config.min_terminal_width,
                excess_width: terminal_width - self.config.min_terminal_width,
            }
        } else {
            WideModeValidation::TooNarrow {
                terminal_width,
                required_width: self.config.min_terminal_width,
                shortfall: self.config.min_terminal_width - terminal_width,
            }
        }
    }
}

/// Result of wide mode terminal validation
#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum WideModeValidation {
    /// Wide mode is disabled
    Disabled,
    /// Terminal is suitable for wide mode
    Suitable {
        terminal_width: u16,
        required_width: u16,
        excess_width: u16,
    },
    /// Terminal is too narrow for wide mode
    TooNarrow {
        terminal_width: u16,
        required_width: u16,
        shortfall: u16,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_wide_mode_manager_creation() {
        let config = WideModeConfig {
            enabled: true,
            ..WideModeConfig::default()
        };
        let manager = WideModeManager::new(config);

        assert!(manager.is_enabled());
    }

    #[test]
    fn test_can_fit_two_pages_disabled() {
        let config = WideModeConfig {
            enabled: false,
            ..WideModeConfig::default()
        };
        let manager = WideModeManager::new(config);

        // Should return false when disabled, regardless of terminal width
        assert!(!manager.can_fit_two_pages(true));
        assert!(!manager.can_fit_two_pages(false));
    }

    #[test]
    fn test_can_fit_two_pages_enabled() {
        let config = WideModeConfig {
            enabled: true,
            ..WideModeConfig::default()
        };
        let manager = WideModeManager::new(config);

        // In non-interactive mode (136 columns), should fit
        assert!(manager.can_fit_two_pages(true));

        // Test with deterministic terminal width to avoid test isolation issues
        // Wide terminal (136 columns) should fit
        assert!(manager.can_fit_two_pages_with_width(136));

        // Narrow terminal (80 columns) should not fit
        assert!(!manager.can_fit_two_pages_with_width(80));
    }

    #[test]
    fn test_get_right_column_start() {
        let manager = WideModeManager::default();
        let start_pos = manager.get_right_column_start();

        // Should be column_width + separator_width
        assert_eq!(start_pos, 68); // 64 + 4
    }

    #[test]
    fn test_get_column_content_width() {
        let manager = WideModeManager::default();
        let content_width = manager.get_column_content_width();

        // Should be column_width - margins
        assert_eq!(content_width, 60); // 64 - 4
    }

    #[test]
    fn test_validate_terminal_disabled() {
        let config = WideModeConfig {
            enabled: false,
            ..WideModeConfig::default()
        };
        let manager = WideModeManager::new(config);

        let validation = manager.validate_terminal_for_wide_mode(true);
        assert_eq!(validation, WideModeValidation::Disabled);
    }

    #[test]
    fn test_validate_terminal_suitable() {
        let config = WideModeConfig {
            enabled: true,
            ..WideModeConfig::default()
        };
        let manager = WideModeManager::new(config);

        let validation = manager.validate_terminal_for_wide_mode(true);
        if let WideModeValidation::Suitable {
            terminal_width,
            required_width,
            excess_width,
        } = validation
        {
            assert_eq!(terminal_width, 136);
            assert_eq!(required_width, 128);
            assert_eq!(excess_width, 8);
        } else {
            panic!("Expected suitable validation");
        }
    }

    #[test]
    fn test_validate_terminal_too_narrow() {
        let config = WideModeConfig {
            enabled: true,
            ..WideModeConfig::default()
        };
        let manager = WideModeManager::new(config);

        // Test with deterministic terminal width to avoid test isolation issues
        let validation = manager.validate_terminal_for_wide_mode_with_width(80);
        if let WideModeValidation::TooNarrow {
            terminal_width,
            required_width,
            shortfall,
        } = validation
        {
            assert_eq!(terminal_width, 80);
            assert_eq!(required_width, 128);
            assert_eq!(shortfall, 48);
        } else {
            panic!("Expected too narrow validation");
        }
    }
}
