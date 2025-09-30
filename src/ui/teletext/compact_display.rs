//! Compact display mode configuration and utilities

use crate::teletext_ui::CONTENT_MARGIN;

/// Configuration for compact display mode layout parameters.
///
/// This struct defines the layout parameters used when rendering games
/// in compact mode, including spacing, width constraints, and formatting options.
#[derive(Debug, Clone)]
pub struct CompactDisplayConfig {
    /// Maximum number of games to display per line
    pub max_games_per_line: usize,
    /// Width allocated for team name display (e.g., "TAP-HIK")
    pub team_name_width: usize,
    /// Width allocated for score display (e.g., " 3-2 ")
    pub score_width: usize,
    /// String used to separate games on the same line
    pub game_separator: &'static str,
}

impl Default for CompactDisplayConfig {
    /// Creates a default compact display configuration optimized for multi-column layout.
    ///
    /// The default configuration supports up to 3 columns on wide terminals,
    /// falling back to 2 columns on medium terminals, and 1 column on narrow terminals.
    fn default() -> Self {
        Self {
            max_games_per_line: 3, // Up to 3 games per line for efficient space usage
            team_name_width: 8,    // "TAP-IFK" = 7 characters
            score_width: 6,        // " 3-2  " = 6 characters with padding
            game_separator: "  ",  // Two spaces between games
        }
    }
}

impl CompactDisplayConfig {
    /// Creates a new compact display configuration with custom parameters.
    ///
    /// # Arguments
    /// * `max_games_per_line` - Maximum games to show per line
    /// * `team_name_width` - Width for team name display
    /// * `score_width` - Width for score display
    /// * `game_separator` - String to separate games
    ///
    /// # Returns
    /// * `CompactDisplayConfig` - New configuration instance
    #[allow(dead_code)] // Used in tests
    pub fn new(
        max_games_per_line: usize,
        team_name_width: usize,
        score_width: usize,
        game_separator: &'static str,
    ) -> Self {
        Self {
            max_games_per_line,
            team_name_width,
            score_width,
            game_separator,
        }
    }

    /// Calculates the optimal number of games per line based on terminal width.
    ///
    /// This method adapts the display to the current terminal width while
    /// respecting the maximum games per line setting. It accounts for content
    /// margins and proper separator spacing.
    ///
    /// # Arguments
    /// * `terminal_width` - Current terminal width in characters
    ///
    /// # Returns
    /// * `usize` - Optimal number of games that can fit per line
    pub fn calculate_games_per_line(&self, terminal_width: usize) -> usize {
        if terminal_width == 0 {
            return 1;
        }

        // Account for content margins (2 chars on each side)
        let available_width = terminal_width.saturating_sub(CONTENT_MARGIN * 2);

        if available_width == 0 {
            return 1;
        }

        // Calculate space needed for one game: team names + score
        let single_game_width = self.team_name_width + self.score_width;

        // Try to fit multiple games with separators
        // For n games, we need: n * game_width + (n-1) * separator_width
        for games_count in (1..=self.max_games_per_line).rev() {
            let total_width = if games_count == 1 {
                single_game_width
            } else {
                games_count * single_game_width + (games_count - 1) * self.game_separator.len()
            };

            if total_width <= available_width {
                return games_count;
            }
        }

        // Fallback to 1 game if nothing fits
        1
    }

    /// Checks if the current terminal width can accommodate compact mode.
    ///
    /// # Arguments
    /// * `terminal_width` - Current terminal width in characters
    ///
    /// # Returns
    /// * `bool` - True if terminal is wide enough for compact mode
    #[allow(dead_code)] // Used in tests
    pub fn is_terminal_width_sufficient(&self, terminal_width: usize) -> bool {
        terminal_width >= self.get_minimum_terminal_width()
    }

    /// Gets the minimum terminal width required for compact mode (including margins)
    pub fn get_minimum_terminal_width(&self) -> usize {
        self.team_name_width + self.score_width + CONTENT_MARGIN * 2
    }

    /// Validates terminal width and returns detailed error information
    pub fn validate_terminal_width(&self, terminal_width: usize) -> TerminalWidthValidation {
        let min_width = self.get_minimum_terminal_width();

        if terminal_width < min_width {
            TerminalWidthValidation::Insufficient {
                current_width: terminal_width,
                required_width: min_width,
                shortfall: min_width - terminal_width,
            }
        } else {
            TerminalWidthValidation::Sufficient {
                current_width: terminal_width,
                required_width: min_width,
                excess: terminal_width - min_width,
            }
        }
    }
}

/// Terminal width validation result
#[derive(Debug, Clone)]
pub enum TerminalWidthValidation {
    /// Terminal width is sufficient for compact mode
    Sufficient {
        #[allow(dead_code)] // Used in tests for validation
        current_width: usize,
        #[allow(dead_code)] // Used in tests for validation
        required_width: usize,
        #[allow(dead_code)] // Used in tests for validation
        excess: usize,
    },
    /// Terminal width is insufficient for compact mode
    Insufficient {
        current_width: usize,
        required_width: usize,
        shortfall: usize,
    },
}

/// Compact mode compatibility validation result
#[derive(Debug, Clone)]
pub enum CompactModeValidation {
    /// Compact mode is fully compatible
    Compatible,
    /// Compact mode is compatible but with warnings
    CompatibleWithWarnings { warnings: Vec<String> },
    /// Compact mode is incompatible
    #[allow(dead_code)] // For future compatibility validation
    Incompatible { issues: Vec<String> },
}
