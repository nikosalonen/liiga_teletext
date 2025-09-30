//! Terminal management for interactive UI
//!
//! This module handles all terminal setup and cleanup operations including:
//! - Raw mode enabling/disabling
//! - Alternate screen management
//! - Terminal configuration for interactive mode
//! - Error handling and recovery for terminal operations

use crate::error::AppError;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::io::stdout;

/// Configuration for terminal management operations
#[derive(Debug, Clone, Default)]
pub struct TerminalConfig {
    pub debug_mode: bool,
}

/// Terminal manager responsible for setup and cleanup operations
pub struct TerminalManager {
    config: TerminalConfig,
}

impl TerminalManager {
    /// Create a new terminal manager with default configuration
    pub fn new() -> Self {
        Self {
            config: TerminalConfig::default(),
        }
    }

    /// Create a new terminal manager with custom configuration
    pub fn with_config(config: TerminalConfig) -> Self {
        Self { config }
    }

    /// Setup terminal for interactive mode
    /// Returns a handle to stdout that can be used for rendering
    pub fn setup_terminal(&self) -> Result<std::io::Stdout, AppError> {
        let mut stdout = stdout();

        if !self.config.debug_mode {
            // Enable raw mode for immediate key processing
            enable_raw_mode()?;

            // Enter alternate screen to preserve terminal content
            execute!(stdout, EnterAlternateScreen)?;
        }

        Ok(stdout)
    }

    /// Cleanup terminal after interactive mode
    /// Restores terminal to its original state
    pub fn cleanup_terminal(&self, mut stdout: std::io::Stdout) -> Result<(), AppError> {
        if !self.config.debug_mode {
            // Disable raw mode to restore normal terminal behavior
            disable_raw_mode()?;

            // Leave alternate screen to restore original content
            execute!(stdout, LeaveAlternateScreen)?;
        }
        Ok(())
    }

    /// Get the terminal configuration
    #[allow(dead_code)]
    pub fn config(&self) -> &TerminalConfig {
        &self.config
    }
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_manager_creation() {
        let manager = TerminalManager::new();
        assert!(!manager.config().debug_mode);
    }

    #[test]
    fn test_terminal_manager_default() {
        let manager = TerminalManager::default();
        assert!(!manager.config().debug_mode);
    }

    #[test]
    fn test_terminal_manager_with_config() {
        let config = TerminalConfig { debug_mode: true };
        let manager = TerminalManager::with_config(config);
        assert!(manager.config().debug_mode);
    }

    #[test]
    fn test_terminal_config_default() {
        let config = TerminalConfig::default();
        assert!(!config.debug_mode);
    }

    #[test]
    fn test_terminal_config_debug_mode() {
        let config = TerminalConfig { debug_mode: true };
        assert!(config.debug_mode);
    }
}
