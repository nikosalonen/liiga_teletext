// src/teletext_ui/rendering.rs - Rendering utilities for TeletextPage display operations

use super::core::{TeletextPage, TeletextRow};
use super::utils::get_ansi_code;
use crate::teletext_ui::{CONTENT_MARGIN, ScoreType};
use crate::ui::teletext::colors::*;

impl TeletextPage {
    /// Renders content in wide mode with two columns.
    /// Handles header/footer spanning full width and two-column layout rendering.
    ///
    /// # Arguments
    /// * `buffer` - The string buffer to append rendered content to
    /// * `visible_rows` - The rows to render
    /// * `width` - Terminal width
    /// * `current_line` - Current line position (mutable reference)
    /// * `text_fg_code` - Text foreground color code
    /// * `subheader_fg_code` - Subheader foreground color code
    pub fn render_wide_mode_content(
        &self,
        buffer: &mut String,
        visible_rows: &[&TeletextRow],
        width: u16,
        current_line: &mut usize,
        text_fg_code: u8,
        subheader_fg_code: u8,
    ) {
        // Check if we can actually fit two columns
        if !self.can_fit_two_pages() {
            // Show warning about insufficient width
            let required_width: usize = 122;
            let current_width: usize = width as usize;
            let shortfall = required_width.saturating_sub(current_width);

            let warning_message = format!(
                "Terminal too narrow for wide mode ({current_width} chars, need {required_width} chars, short {shortfall} chars)"
            );

            // Use optimized ANSI code generation for warning messages (requirement 4.3)
            let mut layout_manager = super::layout::ColumnLayoutManager::new(80, CONTENT_MARGIN);

            let warning_line = layout_manager.format_time_score(
                *current_line,
                CONTENT_MARGIN + 1,
                text_fg_code,
                &warning_message,
            );
            buffer.push_str(&warning_line);
            *current_line += 1;

            // Add suggestion for minimum terminal width
            let suggestion = format!(
                "Resize terminal to at least {} characters wide for wide mode",
                required_width
            );
            let suggestion_line = layout_manager.format_time_score(
                *current_line,
                CONTENT_MARGIN + 1,
                text_fg_code,
                &suggestion,
            );
            buffer.push_str(&suggestion_line);
            *current_line += 1;

            // Fallback to normal rendering
            self.render_normal_content(
                buffer,
                visible_rows,
                width,
                current_line,
                text_fg_code,
                subheader_fg_code,
            );
            return;
        }

        // Distribute visible rows between left and right columns using the shared distribution logic
        let (left_games, right_games) = self.distribute_games_for_wide_display();

        // Calculate column widths for wide mode using the layout system
        let left_column_start = 2;
        let gap_between_columns = 8; // Good separation between columns

        // Use the layout system to determine optimal column width
        let column_width = 60; // Standard wide mode column width

        // Create layout manager for wide mode columns
        use super::layout::ColumnLayoutManager;
        let mut wide_layout_manager =
            ColumnLayoutManager::new_for_wide_mode_column(column_width, 2);
        let games_for_layout = self.extract_games_for_layout(visible_rows);
        let wide_layout_config = wide_layout_manager.calculate_wide_mode_layout(&games_for_layout);

        // Render left column
        let mut left_line = *current_line;

        for (game_index, game) in left_games.iter().enumerate() {
            let formatted_game =
                self.format_game_for_wide_column(game, column_width, &wide_layout_config);
            let lines: Vec<&str> = formatted_game.lines().collect();

            // Use optimized ANSI code generation for left column (requirement 4.3)
            let mut layout_manager = super::layout::ColumnLayoutManager::new(80, CONTENT_MARGIN);

            for (line_index, line) in lines.iter().enumerate() {
                let position_code =
                    layout_manager.get_position_code(left_line + line_index, left_column_start);
                buffer.push_str(&format!("{}{}", position_code, line));
            }
            left_line += lines.len();

            // Add spacing between games (except after the last game)
            if game_index < left_games.len() - 1 {
                left_line += 1; // Extra blank line between games
            }
        }

        // Render right column
        let right_column_start = left_column_start + column_width + gap_between_columns;
        let mut right_line = *current_line;

        for (game_index, game) in right_games.iter().enumerate() {
            let formatted_game =
                self.format_game_for_wide_column(game, column_width, &wide_layout_config);
            let lines: Vec<&str> = formatted_game.lines().collect();

            // Use optimized ANSI code generation for right column (requirement 4.3)
            let mut layout_manager = super::layout::ColumnLayoutManager::new(80, CONTENT_MARGIN);

            for (line_index, line) in lines.iter().enumerate() {
                let position_code =
                    layout_manager.get_position_code(right_line + line_index, right_column_start);
                buffer.push_str(&format!("{}{}", position_code, line));
            }
            right_line += lines.len();

            // Add spacing between games (except after the last game)
            if game_index < right_games.len() - 1 {
                right_line += 1; // Extra blank line between games
            }
        }

        // Update current line to the maximum of left and right column heights
        *current_line = left_line.max(right_line);
    }

    /// Renders content in normal mode (fallback for wide mode when width insufficient).
    ///
    /// # Arguments
    /// * `buffer` - The string buffer to append rendered content to
    /// * `visible_rows` - The rows to render
    /// * `width` - Terminal width
    /// * `current_line` - Current line position (mutable reference)
    /// * `text_fg_code` - Text foreground color code
    /// * `subheader_fg_code` - Subheader foreground color code
    pub fn render_normal_content(
        &self,
        buffer: &mut String,
        visible_rows: &[&TeletextRow],
        _width: u16,
        current_line: &mut usize,
        text_fg_code: u8,
        subheader_fg_code: u8,
    ) {
        // Use the new layout system from game_display.rs
        // Convert color codes to match the new method signature
        let result_fg_code = get_ansi_code(result_fg(), 46);

        self.render_normal_mode_content(
            buffer,
            visible_rows,
            current_line,
            text_fg_code,
            result_fg_code,
            subheader_fg_code,
        );
    }

    /// Formats a game for display in a wide column with specified width constraints.
    /// Optimized for performance with pre-allocated buffers and reasonable goal limits.
    ///
    /// # Arguments
    /// * `game` - The game result to format
    /// * `column_width` - Maximum width for the column
    /// * `layout_config` - Layout configuration for wide mode columns
    ///
    /// # Returns
    /// * `String` - Formatted game string for wide column display with ANSI color codes
    pub fn format_game_for_wide_column(
        &self,
        game: &TeletextRow,
        _column_width: usize,
        layout_config: &super::layout::LayoutConfig,
    ) -> String {
        match game {
            TeletextRow::GameResult {
                home_team,
                away_team,
                time,
                result,
                score_type,
                is_overtime,
                is_shootout,
                goal_events,
                played_time,
                ..
            } => {
                let text_fg_code = get_ansi_code(text_fg(), 231);
                let result_fg_code = get_ansi_code(result_fg(), 46);
                let home_scorer_fg_code = get_ansi_code(home_scorer_fg(), 51);
                let away_scorer_fg_code = get_ansi_code(away_scorer_fg(), 51);
                let winning_goal_fg_code = get_ansi_code(winning_goal_fg(), 201);
                let goal_type_fg_code = get_ansi_code(goal_type_fg(), 226);

                // Format the main game line
                // Pre-allocate lines vector with estimated capacity (1 team line + potential goal lines)
                let estimated_goals = goal_events.len().min(30); // Cap estimate at 30 total goals
                let mut lines = Vec::with_capacity(1 + estimated_goals);

                // Team names and score line using proper teletext layout within column
                let team_score_line = {
                    // Format result with overtime/shootout indicator
                    let result_text = if *is_shootout {
                        format!("{result} rl")
                    } else if *is_overtime {
                        format!("{result} ja")
                    } else {
                        result.clone()
                    };

                    // Format time display based on game state
                    let (time_display, score_display) = match score_type {
                        ScoreType::Scheduled => (time.clone(), String::new()),
                        ScoreType::Ongoing => {
                            let formatted_time =
                                format!("{:02}:{:02}", played_time / 60, played_time % 60);
                            (formatted_time, result_text.clone())
                        }
                        ScoreType::Final => (String::new(), result_text.clone()),
                    };

                    let result_color = match score_type {
                        ScoreType::Final => result_fg_code,
                        _ => text_fg_code,
                    };

                    // Use proportional spacing within the column width
                    let display_text = if !time_display.is_empty() && !score_display.is_empty() {
                        // For ongoing games: show both time and score
                        format!("{time_display} {score_display}")
                    } else if !time_display.is_empty() {
                        time_display
                    } else {
                        score_display
                    };

                    // Format with layout-based character positions for wide mode
                    let mut line = String::new();

                    // Use layout config for team widths in wide mode
                    let home_text =
                        truncate_team_name_gracefully(home_team, layout_config.home_team_width);
                    line.push_str(&format!(
                        "{home_text:<width$}",
                        width = layout_config.home_team_width
                    ));

                    // Separator with layout-based width
                    let separator = match layout_config.separator_width {
                        3 => " - ",
                        _ => "   -   ", // Fallback for different separator widths
                    };
                    line.push_str(separator);

                    // Away team with layout-based width
                    let away_text =
                        truncate_team_name_gracefully(away_team, layout_config.away_team_width);
                    line.push_str(&format!(
                        "{away_text:<width$}",
                        width = layout_config.away_team_width
                    ));

                    // Score section
                    line.push_str(&format!(" \x1b[38;5;{result_color}m{display_text}\x1b[0m"));

                    format!("\x1b[38;5;{text_fg_code}m{line}\x1b[0m")
                };

                lines.push(team_score_line);

                // Goal events - position scorers under their respective teams like normal mode
                // Limit goal scorers for performance (max 15 per team to prevent excessive rendering)
                if !goal_events.is_empty() {
                    const MAX_SCORERS_PER_TEAM: usize = 15;

                    let home_scorers: Vec<_> = goal_events
                        .iter()
                        .filter(|e| e.is_home_team)
                        .take(MAX_SCORERS_PER_TEAM)
                        .collect();
                    let away_scorers: Vec<_> = goal_events
                        .iter()
                        .filter(|e| !e.is_home_team)
                        .take(MAX_SCORERS_PER_TEAM)
                        .collect();
                    let max_scorers = home_scorers.len().max(away_scorers.len());

                    // Pre-allocate lines vector with estimated capacity
                    lines.reserve(max_scorers + 1);

                    for i in 0..max_scorers {
                        let mut scorer_line = String::new();

                        // Build home side using layout-based widths
                        let home_side = if let Some(event) = home_scorers.get(i) {
                            let scorer_color = if (event.is_winning_goal
                                && (*is_overtime || *is_shootout))
                                || event.goal_types.contains(&"VL".to_string())
                            {
                                winning_goal_fg_code // Purple for game-winning goals only
                            } else {
                                home_scorer_fg_code // Regular home team color
                            };

                            let goal_type = event.get_goal_type_display();
                            let goal_type_str = if !goal_type.is_empty() {
                                let truncated_type: String = goal_type
                                    .chars()
                                    .take(layout_config.max_goal_types_width)
                                    .collect();
                                format!(" \x1b[38;5;{goal_type_fg_code}m{truncated_type}\x1b[0m")
                            } else {
                                String::new()
                            };

                            // Use layout config for player name width
                            let player_name_width = layout_config.max_player_name_width;
                            format!(
                                " \x1b[38;5;{}m{:2} {:<width$}\x1b[0m{}",
                                scorer_color,
                                event.minute,
                                event
                                    .scorer_name
                                    .chars()
                                    .take(player_name_width)
                                    .collect::<String>(),
                                goal_type_str,
                                width = player_name_width
                            )
                        } else {
                            // Calculate empty space based on layout config
                            let total_width = 1
                                + 2
                                + 1
                                + layout_config.max_player_name_width
                                + layout_config.max_goal_types_width
                                + 1;
                            " ".repeat(total_width)
                        };

                        scorer_line.push_str(&home_side);

                        // Build away side using layout-based widths
                        if let Some(event) = away_scorers.get(i) {
                            let scorer_color = if (event.is_winning_goal
                                && (*is_overtime || *is_shootout))
                                || event.goal_types.contains(&"VL".to_string())
                            {
                                winning_goal_fg_code // Purple for game-winning goals only
                            } else {
                                away_scorer_fg_code // Regular away team color
                            };

                            let goal_type = event.get_goal_type_display();
                            let goal_type_str = if !goal_type.is_empty() {
                                let truncated_type: String = goal_type
                                    .chars()
                                    .take(layout_config.max_goal_types_width)
                                    .collect();
                                format!(" \x1b[38;5;{goal_type_fg_code}m{truncated_type}\x1b[0m")
                            } else {
                                String::new()
                            };

                            // Use layout config for away team positioning and spacing
                            let away_start_spacing = layout_config.separator_width + 2; // Separator width plus some spacing
                            let away_player_name_width =
                                layout_config.max_player_name_width.min(15); // Cap for away side
                            scorer_line.push_str(&format!(
                                "{:width$}\x1b[38;5;{}m{:2} {:<name_width$}\x1b[0m{}",
                                "",
                                scorer_color,
                                event.minute,
                                event
                                    .scorer_name
                                    .chars()
                                    .take(away_player_name_width)
                                    .collect::<String>(),
                                goal_type_str,
                                width = away_start_spacing,
                                name_width = away_player_name_width
                            ));
                        }

                        if !scorer_line.trim().is_empty() {
                            lines.push(scorer_line);
                        }
                    }
                }

                lines.join("\n")
            }
            TeletextRow::ErrorMessage(message) => {
                let text_fg_code = get_ansi_code(text_fg(), 231);
                format!("\x1b[38;5;{text_fg_code}m{message}\x1b[0m")
            }
            TeletextRow::FutureGamesHeader(header_text) => {
                let subheader_fg_code = get_ansi_code(subheader_fg(), 46);
                format!("\x1b[38;5;{subheader_fg_code}m{header_text}\x1b[0m")
            }
        }
    }
}

/// Counts visible characters in text, excluding ANSI escape sequences.
///
/// # Arguments
/// * `text` - String that may contain ANSI escape sequences
///
/// # Returns
/// * `usize` - Number of visible characters (excluding ANSI sequences)
#[allow(dead_code)]
pub fn count_visible_chars(text: &str) -> usize {
    let mut visible_len = 0;
    let mut in_ansi = false;
    for c in text.chars() {
        if c == '\x1b' {
            in_ansi = true;
        } else if in_ansi && c == 'm' {
            in_ansi = false;
        } else if !in_ansi {
            visible_len += 1;
        }
    }
    visible_len
}

/// Truncates team names gracefully, preferring word boundaries when possible.
///
/// # Arguments
/// * `team_name` - Original team name
/// * `max_length` - Maximum allowed length
///
/// # Returns
/// * `String` - Truncated team name
pub fn truncate_team_name_gracefully(team_name: &str, max_length: usize) -> String {
    if team_name.len() <= max_length {
        return team_name.to_string();
    }

    // Try to find a good truncation point (space, hyphen, or vowel)
    let mut best_pos = max_length;
    for (i, c) in team_name.char_indices().take(max_length) {
        if c == ' ' || c == '-' {
            best_pos = i;
            break;
        }
    }

    team_name.chars().take(best_pos).collect()
}
