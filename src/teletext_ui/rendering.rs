// src/teletext_ui/rendering.rs - Rendering utilities for TeletextPage display operations

use super::core::get_ansi_code;
use super::core::{TeletextPage, TeletextRow};
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

        // Shared ANSI position-code cache for both columns (requirement 4.3)
        let mut layout_manager = super::layout::ColumnLayoutManager::new(80, CONTENT_MARGIN);

        let right_column_start = left_column_start + column_width + gap_between_columns;
        let columns = [
            (&left_games, left_column_start),
            (&right_games, right_column_start),
        ];
        let mut bottom_line = *current_line;
        for (rows, column_start) in columns {
            let column_end = self.render_wide_column(
                buffer,
                &mut layout_manager,
                rows,
                *current_line,
                column_start,
                column_width,
                &wide_layout_config,
            );
            bottom_line = bottom_line.max(column_end);
        }

        // Update current line to the maximum of left and right column heights
        *current_line = bottom_line;
    }

    /// Draws one wide-mode column from `start_line` down and returns the line
    /// below the last one drawn.
    ///
    /// Pagination gives a game taller than the whole column a column of its
    /// own, so stop above the loading line and footer and use the last line
    /// that fits to say how many goals are not shown.
    #[allow(clippy::too_many_arguments)]
    fn render_wide_column(
        &self,
        buffer: &mut String,
        layout_manager: &mut super::layout::ColumnLayoutManager,
        rows: &[&TeletextRow],
        start_line: usize,
        column_start: usize,
        column_width: usize,
        layout_config: &super::layout::LayoutConfig,
    ) -> usize {
        let last_content_line = self.last_content_line();
        let mut line = start_line;

        for (row_index, row) in rows.iter().enumerate() {
            if line > last_content_line {
                break;
            }
            let formatted = self.format_game_for_wide_column(row, column_width, layout_config);
            let row_lines: Vec<&str> = formatted.lines().collect();

            let room = (last_content_line - line).saturating_add(1);
            let clipped = row_lines.len() > room;
            // The result line comes first, then scorer lines. With room for
            // only the result line there is no room for the marker either.
            let shown_lines = if clipped && room >= 2 {
                room - 1
            } else {
                row_lines.len().min(room)
            };

            for row_line in &row_lines[..shown_lines] {
                buffer.push_str(layout_manager.get_position_code(line, column_start));
                buffer.push_str(row_line);
                line += 1;
            }

            if clipped && room >= 2 {
                let shown_scorer_lines = shown_lines - 1;
                let hidden_goals = match row {
                    TeletextRow::GameResult { goal_events, .. } => {
                        let home = goal_events.iter().filter(|e| e.is_home_team).count();
                        let away = goal_events.len() - home;
                        goal_events.len()
                            - home.min(shown_scorer_lines)
                            - away.min(shown_scorer_lines)
                    }
                    _ => 0,
                };
                let goal_type_fg_code = get_ansi_code(goal_type_fg(), 226);
                buffer.push_str(layout_manager.get_position_code(line, column_start));
                buffer.push_str(&format!(
                    "\x1b[38;5;{goal_type_fg_code}m{}\x1b[0m",
                    super::game_display::hidden_goals_text(hidden_goals)
                ));
                line += 1;
            }

            // Blank line between rows (not after the last one)
            if row_index + 1 < rows.len() {
                line += 1;
            }
        }

        line
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
                series_score,
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
                            (formatted_time, result_text)
                        }
                        ScoreType::Final => (String::new(), result_text),
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
                        5 => "  -  ",
                        7 => "   -   ",
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

                // Add series win indicators for playoff games
                if let Some(score) = series_score
                    && score.req_wins > 1
                {
                    use super::game_display::format_team_series_indicator;
                    let home_indicator =
                        format_team_series_indicator(score.home_team_wins, score.req_wins);
                    let away_indicator =
                        format_team_series_indicator(score.away_team_wins, score.req_wins);
                    let series_line = format!(
                        "{team_score_line} \x1b[38;5;{goal_type_fg_code}m{home_indicator}  {away_indicator}\x1b[0m"
                    );
                    lines.push(series_line);
                } else {
                    lines.push(team_score_line);
                }

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
                            let has_video = !self.disable_video_links
                                && event
                                    .video_clip_url
                                    .as_ref()
                                    .is_some_and(|url| !url.trim().is_empty());
                            let video_icon = if has_video {
                                format!("\x1b[38;5;{home_scorer_fg_code}m▶\x1b[0m")
                            } else {
                                String::new()
                            };

                            // Use layout config for player name and goal type widths
                            let player_name_width = layout_config.max_player_name_width;
                            let goal_types_width = layout_config.max_goal_types_width;

                            // Fixed-width fields (name, icon, goal types) keep the
                            // away column aligned even when goal types differ per row
                            let icon_field = if has_video {
                                video_icon
                            } else {
                                " ".to_string()
                            };
                            let goal_type_field = if goal_type.is_empty() {
                                " ".repeat(goal_types_width)
                            } else {
                                format!(
                                    "\x1b[38;5;{goal_type_fg_code}m{goal_type:<goal_types_width$}\x1b[0m"
                                )
                            };
                            format!(
                                " \x1b[38;5;{}m{:2} {:<width$}\x1b[0m{} {}",
                                scorer_color,
                                event.minute,
                                event
                                    .scorer_name
                                    .chars()
                                    .take(player_name_width)
                                    .collect::<String>(),
                                icon_field,
                                goal_type_field,
                                width = player_name_width
                            )
                        } else {
                            // Match the fixed home-side width:
                            // space + minute (2) + space + name + icon + space + goal types
                            let total_width = 1
                                + 2
                                + 1
                                + layout_config.max_player_name_width
                                + 1
                                + 1
                                + layout_config.max_goal_types_width;
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
                            let has_video = !self.disable_video_links
                                && event
                                    .video_clip_url
                                    .as_ref()
                                    .is_some_and(|url| !url.trim().is_empty());
                            let video_icon = if has_video {
                                format!("\x1b[38;5;{away_scorer_fg_code}m▶\x1b[0m")
                            } else {
                                String::new()
                            };

                            // Use layout config for away team positioning and spacing
                            let away_start_spacing = layout_config.separator_width + 2; // Separator width plus some spacing
                            let away_player_name_width =
                                layout_config.max_player_name_width.min(15); // Cap for away side

                            // Create base content with fixed-width player area
                            let base_content = format!(
                                "{:width$}\x1b[38;5;{}m{:2} {:<name_width$}\x1b[0m{}",
                                "",
                                scorer_color,
                                event.minute,
                                event
                                    .scorer_name
                                    .chars()
                                    .take(away_player_name_width)
                                    .collect::<String>(),
                                video_icon,
                                width = away_start_spacing,
                                name_width = away_player_name_width
                            );

                            // Add goal type with consistent spacing
                            let final_content = if !goal_type.is_empty() {
                                format!(
                                    "{} \x1b[38;5;{goal_type_fg_code}m{goal_type}\x1b[0m",
                                    base_content
                                )
                            } else {
                                base_content
                            };

                            scorer_line.push_str(&final_content);
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
            TeletextRow::FutureGamesHeader(header_text)
            | TeletextRow::PlayoffPhaseHeader(header_text)
            | TeletextRow::SeriesHeader(header_text) => {
                let subheader_fg_code = get_ansi_code(subheader_fg(), 46);
                format!("\x1b[38;5;{subheader_fg_code}m{header_text}\x1b[0m")
            }
            TeletextRow::StandingsHeader | TeletextRow::StandingsRow { .. } => {
                // Standings rows are rendered in normal mode only (not wide column mode)
                String::new()
            }
            TeletextRow::BracketLine(line) => line.clone(),
            TeletextRow::BracketPageBreak => String::new(),
        }
    }
}

/// Truncates team names gracefully: cuts at the first space or hyphen within
/// the limit, or at the limit itself when there is none.
///
/// # Arguments
/// * `team_name` - Original team name
/// * `max_length` - Maximum allowed length
///
/// # Returns
/// * `String` - Truncated team name
pub fn truncate_team_name_gracefully(team_name: &str, max_length: usize) -> String {
    if team_name.chars().count() <= max_length {
        return team_name.to_string();
    }

    // Count characters, not bytes, so names with Ä/Ö cut at the right place.
    let best_pos = team_name
        .chars()
        .take(max_length)
        .position(|c| c == ' ' || c == '-')
        .unwrap_or(max_length);

    team_name.chars().take(best_pos).collect()
}

#[cfg(test)]
mod tests {
    use super::truncate_team_name_gracefully;

    #[test]
    fn truncation_cuts_finnish_names_at_the_space() {
        // The space in "Kärpät Oulu" is character 6 but byte 8; mixing the two
        // produced "Kärpät O".
        assert_eq!(truncate_team_name_gracefully("Kärpät Oulu", 9), "Kärpät");
    }

    #[test]
    fn truncation_keeps_names_that_fit_by_display_width() {
        // 11 columns but 13 bytes: measuring bytes cut a name that fits.
        assert_eq!(
            truncate_team_name_gracefully("Kärpät Oulu", 11),
            "Kärpät Oulu"
        );
    }
}
