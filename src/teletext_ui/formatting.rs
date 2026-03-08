// src/teletext_ui/formatting.rs - Formatting utilities for TeletextPage display and layout

use super::core::{CompactDisplayConfig, TeletextPage, TeletextRow};
use crate::teletext_ui::ScoreType;

impl TeletextPage {
    /// Calculates the expected buffer size for rendering to avoid reallocations.
    /// Estimates size based on terminal width, content rows, and ANSI escape sequences.
    ///
    /// # Arguments
    /// * `width` - Terminal width in characters
    /// * `visible_rows` - The content rows that will be rendered
    ///
    /// # Returns
    /// * `usize` - Estimated buffer size in bytes
    pub fn calculate_buffer_size(&self, width: u16, visible_rows: &[&TeletextRow]) -> usize {
        let width = width as usize;

        // Base overhead for headers, ANSI escape sequences, and screen control
        let mut size = 500; // Header, subheader, screen clear sequences

        // Add terminal size as base (each line could be full width)
        size += width * 4; // Header + subheader + padding lines

        // Calculate content size
        for row in visible_rows {
            match row {
                TeletextRow::GameResult { goal_events, .. } => {
                    // Game line: ~80 chars + ANSI sequences (~50 chars)
                    size += 130;

                    // Goal events: estimate 2 lines per game on average
                    // Each goal line: ~40 chars + ANSI sequences (~30 chars)
                    size += goal_events.len() * 70;

                    // Extra spacing
                    size += 20;
                }
                TeletextRow::ErrorMessage(message) => {
                    // Error message: actual length + ANSI sequences
                    size += message.len() + 50;
                }
                TeletextRow::FutureGamesHeader(header) => {
                    // Header: actual length + ANSI sequences
                    size += header.len() + 30;
                }
            }
        }

        // Footer: ~100 chars + ANSI sequences
        if self.show_footer {
            size += 150;
            // Add space for season countdown if present
            if self.season_countdown.is_some() {
                size += 100;
            }
        }

        // Add 25% overhead for ANSI positioning sequences and safety margin
        size + (size / 4)
    }

    /// Groups rows into lines for compact display.
    /// Handles both games and headers properly for compact mode formatting.
    ///
    /// # Arguments
    /// * `rows` - List of rows to group
    /// * `config` - Compact display configuration
    /// * `terminal_width` - Current terminal width
    ///
    /// # Returns
    /// * `Vec<String>` - Lines of formatted content
    pub fn group_games_for_compact_display(
        &self,
        rows: &[&TeletextRow],
        config: &CompactDisplayConfig,
        terminal_width: usize,
    ) -> Vec<String> {
        let games_per_line = config.calculate_games_per_line(terminal_width);
        let mut lines = Vec::new();
        let mut current_line = String::new();
        let mut games_in_current_line = 0;

        for row in rows.iter() {
            let row_str = self.format_compact_game(row, config);

            // Skip empty strings (unsupported row types)
            if row_str.is_empty() {
                continue;
            }

            // Handle headers as separate lines
            if matches!(row, TeletextRow::FutureGamesHeader(_)) {
                // Finish current game line if not empty
                if !current_line.is_empty() {
                    lines.push(current_line.clone());
                    current_line.clear();
                    games_in_current_line = 0;
                }
                // Add header as its own line
                lines.push(row_str);
                continue;
            }

            // Handle games
            if current_line.is_empty() {
                current_line = row_str;
                games_in_current_line = 1;
            } else {
                current_line.push_str(config.game_separator);
                current_line.push_str(&row_str);
                games_in_current_line += 1;
            }

            // Start new line if we've reached the limit
            if games_in_current_line >= games_per_line {
                lines.push(current_line.clone());
                // Add empty line after each group of games for better readability
                lines.push(String::new());
                current_line.clear();
                games_in_current_line = 0;
            }
        }

        // Add remaining games if any
        if !current_line.is_empty() {
            lines.push(current_line);
            // Add empty line after the last group as well
            lines.push(String::new());
        }

        // Remove the final empty line if there are any lines (to avoid trailing empty space)
        if !lines.is_empty() && lines.last() == Some(&String::new()) {
            lines.pop();
        }

        lines
    }

    /// Formats a single game for compact display with proper teletext colors.
    /// Creates a condensed representation of the game suitable for multi-column layout.
    ///
    /// # Arguments
    /// * `game` - The game row to format
    /// * `config` - Compact display configuration
    ///
    /// # Returns
    /// * `String` - Formatted compact game string with ANSI color codes
    pub fn format_compact_game(&self, game: &TeletextRow, config: &CompactDisplayConfig) -> String {
        match game {
            TeletextRow::GameResult {
                home_team,
                away_team,
                time,
                result,
                score_type,
                is_overtime,
                is_shootout,
                ..
            } => {
                // Import color utilities
                use super::utils::get_ansi_code;
                use crate::ui::teletext::colors::*;

                let text_fg_code = get_ansi_code(text_fg(), 231);
                let result_fg_code = get_ansi_code(result_fg(), 46);

                // Use team abbreviations
                let home_abbr = get_team_abbreviation(home_team);
                let away_abbr = get_team_abbreviation(away_team);

                // Format team names with proper width and teletext white color
                let team_display = format!("{home_abbr}-{away_abbr}");
                let padded_team = format!(
                    "\x1b[38;5;{text_fg_code}m{:<width$}\x1b[0m",
                    team_display,
                    width = config.team_name_width
                );

                // Format score based on game state with appropriate colors
                let score_display = match score_type {
                    ScoreType::Scheduled => {
                        // Scheduled games show time in white
                        format!(
                            "\x1b[38;5;{text_fg_code}m{:<width$}\x1b[0m",
                            time,
                            width = config.score_width
                        )
                    }
                    ScoreType::Ongoing => {
                        // Ongoing games show score in white (like regular mode)
                        let mut score = result.clone();
                        if *is_shootout {
                            score.push_str(" rl");
                        } else if *is_overtime {
                            score.push_str(" ja");
                        }
                        format!(
                            "\x1b[38;5;{text_fg_code}m{:<width$}\x1b[0m",
                            score,
                            width = config.score_width
                        )
                    }
                    ScoreType::Final => {
                        // Final games show score in bright green (like regular mode)
                        let mut score = result.clone();
                        if *is_shootout {
                            score.push_str(" rl");
                        } else if *is_overtime {
                            score.push_str(" ja");
                        }
                        format!(
                            "\x1b[38;5;{result_fg_code}m{:<width$}\x1b[0m",
                            score,
                            width = config.score_width
                        )
                    }
                };

                format!("{padded_team}{score_display}")
            }
            TeletextRow::FutureGamesHeader(header_text) => {
                // Import color utilities
                use super::utils::get_ansi_code;
                use crate::ui::teletext::colors::*;

                let subheader_fg_code = get_ansi_code(subheader_fg(), 46);

                // Format future games header for compact mode - intelligently abbreviate to preserve date
                let abbreviated_header = if header_text.starts_with("Seuraavat ottelut ") {
                    // Special handling for "Seuraavat ottelut DD.MM." - abbreviate "Seuraavat" to preserve date
                    header_text.replace("Seuraavat ottelut ", "Seur. ottelut ")
                } else if header_text.len() > 30 {
                    // For other long headers, truncate at 30 characters (increased from 22)
                    format!("{}...", &header_text[..30])
                } else {
                    header_text.clone()
                };
                format!("\x1b[38;5;{subheader_fg_code}m>>> {abbreviated_header}\x1b[0m")
            }
            _ => String::new(),
        }
    }
}

// Re-export the canonical team abbreviation function from the single source of truth
pub use crate::ui::components::abbreviations::get_team_abbreviation;
