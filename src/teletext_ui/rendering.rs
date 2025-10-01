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

            buffer.push_str(&format!(
                "\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                *current_line,
                CONTENT_MARGIN + 1,
                text_fg_code,
                warning_message
            ));
            *current_line += 1;

            // Add suggestion for minimum terminal width
            buffer.push_str(&format!(
                "\x1b[{};{}H\x1b[38;5;{}mResize terminal to at least {} characters wide for wide mode\x1b[0m",
                *current_line,
                CONTENT_MARGIN + 1,
                text_fg_code,
                required_width
            ));
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

        // Calculate column widths for wide mode - based on normal mode layout
        let left_column_start = 2;
        let gap_between_columns = 8; // Good separation between columns

        // Each column should accommodate the normal teletext layout with extra width
        // Normal mode uses positions up to ~55 chars, but we can make columns wider for better readability
        let normal_content_width = 60; // Wider columns for better spacing and readability
        let column_width = normal_content_width;

        // Render left column
        let mut left_line = *current_line;

        for (game_index, game) in left_games.iter().enumerate() {
            let formatted_game = self.format_game_for_wide_column(game, column_width);
            let lines: Vec<&str> = formatted_game.lines().collect();

            for (line_index, line) in lines.iter().enumerate() {
                buffer.push_str(&format!(
                    "\x1b[{};{}H{}",
                    left_line + line_index,
                    left_column_start,
                    line
                ));
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
            let formatted_game = self.format_game_for_wide_column(game, column_width);
            let lines: Vec<&str> = formatted_game.lines().collect();

            for (line_index, line) in lines.iter().enumerate() {
                buffer.push_str(&format!(
                    "\x1b[{};{}H{}",
                    right_line + line_index,
                    right_column_start,
                    line
                ));
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
        width: u16,
        current_line: &mut usize,
        text_fg_code: u8,
        subheader_fg_code: u8,
    ) {
        for row in visible_rows {
            match row {
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
                } => {
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
                        ScoreType::Final => get_ansi_code(result_fg(), 46),
                        _ => text_fg_code,
                    };

                    // Build game line with flexible positioning based on terminal width
                    let available_width = width as usize - (CONTENT_MARGIN * 2);
                    let home_team_width = (available_width * 3) / 8; // 3/8 of available width
                    let away_team_width = (available_width * 3) / 8; // 3/8 of available width
                    let time_score_width = available_width - home_team_width - away_team_width - 3; // Remaining space minus separator

                    let home_pos = CONTENT_MARGIN + 1;
                    let separator_pos = home_pos + home_team_width;
                    let away_pos = separator_pos + 3; // 3 chars for " - "
                    let score_pos = away_pos + away_team_width;

                    // Build game line with precise positioning (using 1-based ANSI coordinates)
                    if !time_display.is_empty() && !score_display.is_empty() {
                        // For ongoing games: show time on the left, score on the right
                        buffer.push_str(&format!(
                            "\x1b[{};{}H\x1b[38;5;{}m{:<width1$}\x1b[{};{}H\x1b[38;5;{}m- \x1b[{};{}H\x1b[38;5;{}m{:<width2$}\x1b[{};{}H\x1b[38;5;{}m{:<width3$}\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                            *current_line, home_pos,
                            text_fg_code,
                            home_team.chars().take(home_team_width).collect::<String>(),
                            *current_line, separator_pos,
                            text_fg_code,
                            *current_line, away_pos,
                            text_fg_code,
                            away_team.chars().take(away_team_width).collect::<String>(),
                            *current_line, score_pos,
                            text_fg_code,
                            time_display,
                            *current_line, score_pos + time_display.len() + 1,
                            result_color,
                            score_display,
                            width1 = home_team_width,
                            width2 = away_team_width,
                            width3 = time_score_width,
                        ));
                    } else if !time_display.is_empty() {
                        // For scheduled games: show time on the right
                        buffer.push_str(&format!(
                            "\x1b[{};{}H\x1b[38;5;{}m{:<width1$}\x1b[{};{}H\x1b[38;5;{}m- \x1b[{};{}H\x1b[38;5;{}m{:<width2$}\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                            *current_line, home_pos,
                            text_fg_code,
                            home_team.chars().take(home_team_width).collect::<String>(),
                            *current_line, separator_pos,
                            text_fg_code,
                            *current_line, away_pos,
                            text_fg_code,
                            away_team.chars().take(away_team_width).collect::<String>(),
                            *current_line, score_pos,
                            text_fg_code,
                            time_display,
                            width1 = home_team_width,
                            width2 = away_team_width,
                        ));
                    } else {
                        // For finished games: show score on the right
                        buffer.push_str(&format!(
                            "\x1b[{};{}H\x1b[38;5;{}m{:<width1$}\x1b[{};{}H\x1b[38;5;{}m- \x1b[{};{}H\x1b[38;5;{}m{:<width2$}\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                            *current_line, home_pos,
                            text_fg_code,
                            home_team.chars().take(home_team_width).collect::<String>(),
                            *current_line, separator_pos,
                            text_fg_code,
                            *current_line, away_pos,
                            text_fg_code,
                            away_team.chars().take(away_team_width).collect::<String>(),
                            *current_line, score_pos,
                            result_color,
                            score_display,
                            width1 = home_team_width,
                            width2 = away_team_width,
                        ));
                    }

                    *current_line += 1;

                    // Goal events - position scorers under their respective teams
                    if !goal_events.is_empty() {
                        let home_scorer_fg_code = get_ansi_code(home_scorer_fg(), 51);
                        let away_scorer_fg_code = get_ansi_code(away_scorer_fg(), 51);
                        let winning_goal_fg_code = get_ansi_code(winning_goal_fg(), 201);
                        let goal_type_fg_code = get_ansi_code(goal_type_fg(), 226);

                        let home_scorers: Vec<_> =
                            goal_events.iter().filter(|e| e.is_home_team).collect();
                        let away_scorers: Vec<_> =
                            goal_events.iter().filter(|e| !e.is_home_team).collect();
                        let max_scorers = home_scorers.len().max(away_scorers.len());

                        for i in 0..max_scorers {
                            if let Some(event) = home_scorers.get(i) {
                                let scorer_color = if (event.is_winning_goal
                                    && (*is_overtime || *is_shootout))
                                    || event.goal_types.contains(&"VL".to_string())
                                {
                                    winning_goal_fg_code // Purple for game-winning goals only
                                } else {
                                    home_scorer_fg_code // Regular home team color
                                };

                                // Position under home team
                                buffer.push_str(&format!(
                                    "\x1b[{};{}H\x1b[38;5;{}m {:2} ",
                                    *current_line,
                                    home_pos + 1,
                                    scorer_color,
                                    event.minute
                                ));

                                // Add clickable URL only if videos are enabled and URL exists
                                if !self.disable_video_links {
                                    if let Some(ref url) = event.video_clip_url {
                                        buffer.push_str(&format!(
                                            "\x1b[38;5;{}m{:<17}\x1b]8;;{}\x07▶\x1b]8;;\x07",
                                            scorer_color, event.scorer_name, url
                                        ));
                                    } else {
                                        buffer.push_str(&format!(
                                            "\x1b[38;5;{}m{:<17}",
                                            scorer_color, event.scorer_name
                                        ));
                                    }
                                } else {
                                    buffer.push_str(&format!(
                                        "\x1b[38;5;{}m{:<17}",
                                        scorer_color, event.scorer_name
                                    ));
                                }

                                // Add goal type indicators
                                // Truncate to 3 characters max to prevent overflow into away column
                                let goal_type = event.get_goal_type_display();
                                if !goal_type.is_empty() {
                                    let truncated_type: String =
                                        goal_type.chars().take(3).collect();
                                    buffer.push_str(&format!(
                                        " \x1b[38;5;{goal_type_fg_code}m{truncated_type}\x1b[0m"
                                    ));
                                } else {
                                    buffer.push_str("\x1b[0m");
                                }
                            }

                            if let Some(event) = away_scorers.get(i) {
                                let scorer_color = if (event.is_winning_goal
                                    && (*is_overtime || *is_shootout))
                                    || event.goal_types.contains(&"VL".to_string())
                                {
                                    winning_goal_fg_code // Purple for game-winning goals only
                                } else {
                                    away_scorer_fg_code // Regular away team color
                                };

                                // Position under away team
                                buffer.push_str(&format!(
                                    "\x1b[{};{}H\x1b[38;5;{}m {:2} ",
                                    *current_line,
                                    away_pos + 1,
                                    scorer_color,
                                    event.minute
                                ));

                                // Add clickable URL only if videos are enabled and URL exists
                                if !self.disable_video_links {
                                    if let Some(ref url) = event.video_clip_url {
                                        buffer.push_str(&format!(
                                            "\x1b[38;5;{}m{:<17}\x1b]8;;{}\x07▶\x1b]8;;\x07",
                                            scorer_color, event.scorer_name, url
                                        ));
                                    } else {
                                        buffer.push_str(&format!(
                                            "\x1b[38;5;{}m{:<17}",
                                            scorer_color, event.scorer_name
                                        ));
                                    }
                                } else {
                                    buffer.push_str(&format!(
                                        "\x1b[38;5;{}m{:<17}",
                                        scorer_color, event.scorer_name
                                    ));
                                }

                                // Add goal type indicators
                                // Truncate to 3 characters max to prevent overflow into away column
                                let goal_type = event.get_goal_type_display();
                                if !goal_type.is_empty() {
                                    let truncated_type: String =
                                        goal_type.chars().take(3).collect();
                                    buffer.push_str(&format!(
                                        " \x1b[38;5;{goal_type_fg_code}m{truncated_type}\x1b[0m"
                                    ));
                                } else {
                                    buffer.push_str("\x1b[0m");
                                }
                            }

                            if home_scorers.get(i).is_some() || away_scorers.get(i).is_some() {
                                *current_line += 1;
                            }
                        }
                    }

                    // Add spacing between games in interactive mode
                    if !self.ignore_height_limit {
                        *current_line += 1;
                    }
                }
                TeletextRow::ErrorMessage(message) => {
                    for line in message.lines() {
                        buffer.push_str(&format!(
                            "\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                            *current_line,
                            CONTENT_MARGIN + 1,
                            text_fg_code,
                            line
                        ));
                        *current_line += 1;
                    }
                }
                TeletextRow::FutureGamesHeader(header_text) => {
                    buffer.push_str(&format!(
                        "\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                        *current_line,
                        CONTENT_MARGIN + 1,
                        subheader_fg_code,
                        header_text
                    ));
                    *current_line += 1;
                }
            }
        }
    }

    /// Formats a game for display in a wide column with specified width constraints.
    /// Optimized for performance with pre-allocated buffers and reasonable goal limits.
    ///
    /// # Arguments
    /// * `game` - The game result to format
    /// * `column_width` - Maximum width for the column
    ///
    /// # Returns
    /// * `String` - Formatted game string for wide column display with ANSI color codes
    pub fn format_game_for_wide_column(&self, game: &TeletextRow, _column_width: usize) -> String {
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

                    // Format with fixed character positions - away team always starts at position 27
                    let mut line = String::new();

                    // Position 0-19: Home team (up to 20 chars) - use graceful truncation
                    let home_text = truncate_team_name_gracefully(home_team, 20);
                    line.push_str(&format!("{home_text:<20}"));

                    // Position 20-26: Spacing and dash (7 chars total)
                    line.push_str("    - ");

                    // Position 27+: Away team (up to 17 chars) - use graceful truncation
                    let away_text = truncate_team_name_gracefully(away_team, 17);
                    line.push_str(&format!("{away_text:<20}"));

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

                        // Build home side (always exactly 22 characters total)
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
                                format!(" \x1b[38;5;{goal_type_fg_code}m{goal_type}\x1b[0m")
                            } else {
                                String::new()
                            };

                            format!(
                                " \x1b[38;5;{}m{:2} {:<17}\x1b[0m{}",
                                scorer_color,
                                event.minute,
                                event.scorer_name.chars().take(17).collect::<String>(),
                                goal_type_str
                            )
                        } else {
                            "                         ".to_string() // 25 spaces: 1 + 2 (minute) + 1 + 17 (name) + 4 (margin)
                        };

                        scorer_line.push_str(&home_side);

                        // Build away side
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
                                let truncated_type: String = goal_type.chars().take(3).collect();
                                format!(" \x1b[38;5;{goal_type_fg_code}m{truncated_type}\x1b[0m")
                            } else {
                                String::new()
                            };

                            scorer_line.push_str(&format!(
                                "     \x1b[38;5;{}m{:2} {:<15}\x1b[0m{}",
                                scorer_color,
                                event.minute,
                                event.scorer_name.chars().take(15).collect::<String>(),
                                goal_type_str
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
