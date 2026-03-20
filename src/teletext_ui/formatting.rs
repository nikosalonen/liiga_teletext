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
                TeletextRow::FutureGamesHeader(header)
                | TeletextRow::PlayoffPhaseHeader(header) => {
                    // Header: actual length + ANSI sequences
                    size += header.len() + 30;
                }
                TeletextRow::StandingsHeader => {
                    size += 100;
                }
                TeletextRow::StandingsRow { .. } => {
                    // Standings row: ~80 chars + ANSI sequences
                    size += 150;
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
            if matches!(
                row,
                TeletextRow::FutureGamesHeader(_) | TeletextRow::PlayoffPhaseHeader(_)
            ) {
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
                use super::core::get_ansi_code;
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
            TeletextRow::FutureGamesHeader(header_text)
            | TeletextRow::PlayoffPhaseHeader(header_text) => {
                // Import color utilities
                use super::core::get_ansi_code;
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

// --- Score formatting utilities (moved from score_formatting.rs) ---

/// Formats a game result with appropriate styling and indicators
#[allow(dead_code)]
pub fn format_score_with_indicators(
    result: &str,
    score_type: &ScoreType,
    is_overtime: bool,
    is_shootout: bool,
    played_time: i32,
) -> String {
    match score_type {
        ScoreType::Final => {
            let mut formatted_score = result.to_string();
            if is_overtime {
                formatted_score.push_str(" JA");
            } else if is_shootout {
                formatted_score.push_str(" RL");
            }
            formatted_score
        }
        ScoreType::Ongoing => {
            let time_display = format_playing_time(played_time);
            format!("{result} {time_display}")
        }
        ScoreType::Scheduled => "-".to_string(),
    }
}

/// Formats the playing time for ongoing games
#[allow(dead_code)]
pub fn format_playing_time(played_time: i32) -> String {
    if played_time <= 0 {
        return "0:00".to_string();
    }

    let minutes = played_time / 60;
    let seconds = played_time % 60;

    if minutes >= 20 {
        let period = (minutes / 20) + 1;
        let period_minutes = minutes % 20;

        if period > 3 {
            let ot_minutes = minutes - 60;
            format!("JA {ot_minutes}:{seconds:02}")
        } else {
            format!("{period}. {period_minutes}:{seconds:02}")
        }
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// Gets the appropriate color code for a score based on game status
#[allow(dead_code)]
pub fn get_score_color(score_type: &ScoreType, is_overtime: bool, is_shootout: bool) -> u8 {
    match score_type {
        ScoreType::Final => {
            if is_overtime || is_shootout {
                226
            } else {
                46
            }
        }
        ScoreType::Ongoing => 201,
        ScoreType::Scheduled => 231,
    }
}

/// Formats game time display for scheduled games
#[allow(dead_code)]
pub fn format_game_time(time: &str) -> String {
    if time.is_empty() {
        return "TBD".to_string();
    }

    // Extract only the HH:MM portion to avoid capturing date prefixes or seconds.
    // Scan backwards through the string looking for a D{1,2}:DD pattern.
    let bytes = time.as_bytes();
    for i in (0..bytes.len().saturating_sub(2)).rev() {
        if bytes[i + 1] == b':'
            && bytes[i].is_ascii_digit()
            && i + 3 < bytes.len()
            && bytes[i + 2].is_ascii_digit()
            && bytes[i + 3].is_ascii_digit()
        {
            // Check for a second hour digit before position i
            let start = if i > 0 && bytes[i - 1].is_ascii_digit() {
                // Ensure we don't grab a third digit (e.g. from "123:45")
                if i >= 2 && bytes[i - 2].is_ascii_digit() {
                    continue;
                }
                i - 1
            } else {
                i
            };
            // Skip if preceded by ':' — this is likely `:MM:SS`, not `HH:MM`
            if start > 0 && bytes[start - 1] == b':' {
                continue;
            }
            // Ensure the minute part is exactly 2 digits (no trailing digit)
            if i + 4 < bytes.len() && bytes[i + 4].is_ascii_digit() {
                continue;
            }
            return time[start..i + 4].to_string();
        }
    }

    time.to_string()
}

/// Formats a complete score line with colors and indicators
#[allow(dead_code)]
pub fn format_complete_score_line(
    result: &str,
    time: &str,
    score_type: &ScoreType,
    is_overtime: bool,
    is_shootout: bool,
    played_time: i32,
) -> String {
    let score_color = get_score_color(score_type, is_overtime, is_shootout);
    let formatted_score =
        format_score_with_indicators(result, score_type, is_overtime, is_shootout, played_time);

    match score_type {
        ScoreType::Scheduled => {
            let formatted_time = format_game_time(time);
            format!("\x1b[38;5;{score_color}m{formatted_time:>6}\x1b[0m")
        }
        _ => {
            format!("\x1b[38;5;{score_color}m{formatted_score:>6}\x1b[0m")
        }
    }
}

/// Determines if a game result should be highlighted
#[allow(dead_code)]
pub fn should_highlight_score(
    score_type: &ScoreType,
    is_overtime: bool,
    is_shootout: bool,
) -> bool {
    match score_type {
        ScoreType::Ongoing => true,
        ScoreType::Final => is_overtime || is_shootout,
        ScoreType::Scheduled => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_score_with_indicators_final() {
        let result = format_score_with_indicators("2-1", &ScoreType::Final, false, false, 0);
        assert_eq!(result, "2-1");

        let result_ot = format_score_with_indicators("2-1", &ScoreType::Final, true, false, 0);
        assert_eq!(result_ot, "2-1 JA");

        let result_so = format_score_with_indicators("2-1", &ScoreType::Final, false, true, 0);
        assert_eq!(result_so, "2-1 RL");
    }

    #[test]
    fn test_format_score_with_indicators_ongoing() {
        let result = format_score_with_indicators("1-0", &ScoreType::Ongoing, false, false, 900);
        assert_eq!(result, "1-0 15:00");
    }

    #[test]
    fn test_format_score_with_indicators_scheduled() {
        let result = format_score_with_indicators("0-0", &ScoreType::Scheduled, false, false, 0);
        assert_eq!(result, "-");
    }

    #[test]
    fn test_format_playing_time() {
        assert_eq!(format_playing_time(0), "0:00");
        assert_eq!(format_playing_time(65), "1:05");
        assert_eq!(format_playing_time(900), "15:00");
        assert_eq!(format_playing_time(1200), "2. 0:00");
        assert_eq!(format_playing_time(2400), "3. 0:00");
        assert_eq!(format_playing_time(3900), "JA 5:00");
    }

    #[test]
    fn test_format_game_time() {
        assert_eq!(format_game_time("18:30"), "18:30");
        assert_eq!(format_game_time("18:30:00"), "18:30"); // Strips seconds
        assert_eq!(format_game_time(""), "TBD");
        // Handles date prefixes by extracting only HH:MM
        assert_eq!(format_game_time("2024-01-15T18:30:00Z"), "18:30");
        assert_eq!(format_game_time("9:05"), "9:05");
    }

    #[test]
    fn test_should_highlight_score() {
        assert!(should_highlight_score(&ScoreType::Ongoing, false, false));
        assert!(should_highlight_score(&ScoreType::Final, true, false));
        assert!(should_highlight_score(&ScoreType::Final, false, true));
        assert!(!should_highlight_score(&ScoreType::Final, false, false));
        assert!(!should_highlight_score(&ScoreType::Scheduled, false, false));
    }
}
