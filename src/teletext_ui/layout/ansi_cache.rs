// src/teletext_ui/layout/ansi_cache.rs - ANSI code caching for optimized rendering

use std::collections::HashMap;

use super::config::LayoutConfig;

/// Pre-calculated ANSI positioning codes for optimized rendering
#[derive(Debug, Clone)]
pub struct AnsiCodeCache {
    /// Cache for positioning codes (line, column) -> ANSI code
    position_codes: HashMap<(usize, usize), String>,
    /// Cache for color codes with positioning
    color_position_codes: HashMap<(usize, usize, u8), String>,
}

impl AnsiCodeCache {
    /// Creates a new ANSI code cache
    pub fn new() -> Self {
        Self {
            position_codes: HashMap::new(),
            color_position_codes: HashMap::new(),
        }
    }

    /// Pre-calculates positioning codes for common positions
    /// This optimizes repeated ANSI code generation (requirement 4.3)
    pub fn pre_calculate_positions(&mut self, layout_config: &LayoutConfig, max_lines: usize) {
        // Pre-calculate common positioning codes
        let common_columns = vec![
            1,                                                                 // Start of line
            layout_config.home_team_width + 1,                                 // Home team position
            layout_config.home_team_width + layout_config.separator_width + 1, // Separator position
            layout_config.home_team_width
                + layout_config.separator_width
                + layout_config.away_team_width
                + 1, // Away team position
            layout_config.time_column,
            layout_config.score_column,
            layout_config.play_icon_column,
        ];

        for line in 1..=max_lines {
            for &column in &common_columns {
                let position_code = format!("\x1b[{};{}H", line, column);
                self.position_codes.insert((line, column), position_code);
            }
        }

        tracing::debug!(
            "Pre-calculated {} positioning codes for {} lines and {} columns",
            self.position_codes.len(),
            max_lines,
            common_columns.len()
        );
    }

    /// Gets or generates a positioning code
    pub fn get_position_code(&mut self, line: usize, column: usize) -> &str {
        self.position_codes
            .entry((line, column))
            .or_insert_with(|| format!("\x1b[{};{}H", line, column))
    }

    /// Gets or generates a positioning code with color
    pub fn get_color_position_code(&mut self, line: usize, column: usize, color: u8) -> &str {
        self.color_position_codes
            .entry((line, column, color))
            .or_insert_with(|| format!("\x1b[{};{}H\x1b[38;5;{}m", line, column, color))
    }

    /// Clears the cache to free memory
    pub fn clear(&mut self) {
        let total_entries = self.position_codes.len() + self.color_position_codes.len();
        self.position_codes.clear();
        self.color_position_codes.clear();

        tracing::debug!(
            "Cleared ANSI code cache with {} total entries",
            total_entries
        );
    }

    /// Gets cache statistics
    pub fn get_cache_stats(&self) -> AnsiCacheStats {
        AnsiCacheStats {
            position_codes: self.position_codes.len(),
            color_position_codes: self.color_position_codes.len(),
        }
    }
}

/// Statistics for ANSI code cache
#[derive(Debug)]
pub struct AnsiCacheStats {
    pub position_codes: usize,
    pub color_position_codes: usize,
}

impl Default for AnsiCodeCache {
    fn default() -> Self {
        Self::new()
    }
}
