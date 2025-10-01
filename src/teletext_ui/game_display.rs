// src/teletext_ui/game_display.rs - Normal mode game rendering logic

use super::core::{AWAY_TEAM_OFFSET, SEPARATOR_OFFSET};
use super::core::{TeletextPage, TeletextRow};
use super::utils::get_ansi_code;
use crate::teletext_ui::{CONTENT_MARGIN, ScoreType};
use crate::ui::teletext::colors::*;

impl TeletextPage {
    /// Renders game content in normal (single-column) mode.
    /// This is the default rendering mode for standard terminal widths.
    ///
    /// # Arguments
    /// * `buffer` - The string buffer to append rendered content to
    /// * `visible_rows` - The rows to render (game results, error messages, headers)
    /// * `current_line` - Current line position (mutable reference)
    /// * `text_fg_code` - Text foreground color code
    /// * `result_fg_code` - Result foreground color code (for final scores)
    /// * `subheader_fg_code` - Subheader foreground color code (for section headers)
    pub fn render_normal_mode_content(
        &self,
        buffer: &mut String,
        visible_rows: &[&TeletextRow],
        current_line: &mut usize,
        text_fg_code: u8,
        result_fg_code: u8,
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
                    self.render_game_result_row(
                        buffer,
                        home_team,
                        away_team,
                        time,
                        result,
                        score_type,
                        *is_overtime,
                        *is_shootout,
                        goal_events,
                        *played_time,
                        current_line,
                        text_fg_code,
                        result_fg_code,
                    );
                }
                TeletextRow::ErrorMessage(message) => {
                    self.render_error_message(buffer, message, current_line, text_fg_code);
                }
                TeletextRow::FutureGamesHeader(header_text) => {
                    self.render_future_games_header(
                        buffer,
                        header_text,
                        current_line,
                        subheader_fg_code,
                    );
                }
            }
        }
    }

    /// Renders a single game result row with team names, time/score, and goal events.
    ///
    /// # Arguments
    /// * `buffer` - The string buffer to append to
    /// * `home_team` - Home team name
    /// * `away_team` - Away team name
    /// * `time` - Game time (for scheduled games)
    /// * `result` - Game score (for ongoing/finished games)
    /// * `score_type` - Type of score (Scheduled, Ongoing, Final)
    /// * `is_overtime` - Whether game went to overtime
    /// * `is_shootout` - Whether game went to shootout
    /// * `goal_events` - List of goal events to display
    /// * `played_time` - Current game time in seconds (for ongoing games)
    /// * `current_line` - Current line position
    /// * `text_fg_code` - Text color code
    /// * `result_fg_code` - Result color code
    #[allow(clippy::too_many_arguments)]
    fn render_game_result_row(
        &self,
        buffer: &mut String,
        home_team: &str,
        away_team: &str,
        time: &str,
        result: &str,
        score_type: &ScoreType,
        is_overtime: bool,
        is_shootout: bool,
        goal_events: &[crate::data_fetcher::GoalEventData],
        played_time: i32,
        current_line: &mut usize,
        text_fg_code: u8,
        result_fg_code: u8,
    ) {
        // Format result with overtime/shootout indicator
        let result_text = if is_shootout {
            format!("{result} rl")
        } else if is_overtime {
            format!("{result} ja")
        } else {
            result.to_string()
        };

        // Format time display based on game state
        let (time_display, score_display) = match score_type {
            ScoreType::Scheduled => (time.to_string(), String::new()),
            ScoreType::Ongoing => {
                let formatted_time = format!("{:02}:{:02}", played_time / 60, played_time % 60);
                (formatted_time, result_text.clone())
            }
            ScoreType::Final => (String::new(), result_text.clone()),
        };

        let result_color = match score_type {
            ScoreType::Final => result_fg_code,
            _ => text_fg_code,
        };

        // Build game line with precise positioning (using 1-based ANSI coordinates)
        if !time_display.is_empty() && !score_display.is_empty() {
            // For ongoing games: show time on the left, score on the right
            buffer.push_str(&format!(
                "\x1b[{};{}H\x1b[38;5;{}m{:<20}\x1b[{};{}H\x1b[38;5;{}m- \x1b[{};{}H\x1b[38;5;{}m{:<20}\x1b[{};{}H\x1b[38;5;{}m{:<10}\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                current_line, CONTENT_MARGIN + 1,
                text_fg_code,
                home_team.chars().take(20).collect::<String>(),
                current_line, SEPARATOR_OFFSET + CONTENT_MARGIN + 1,
                text_fg_code,
                current_line, AWAY_TEAM_OFFSET + CONTENT_MARGIN + 1,
                text_fg_code,
                away_team.chars().take(20).collect::<String>(),
                current_line, 35 + CONTENT_MARGIN + 1,
                text_fg_code,
                time_display,
                current_line, 45 + CONTENT_MARGIN + 1,
                result_color,
                score_display
            ));
        } else {
            // For scheduled/final games: show time or score on the right
            let display_text = if !time_display.is_empty() {
                time_display
            } else {
                score_display
            };
            buffer.push_str(&format!(
                "\x1b[{};{}H\x1b[38;5;{}m{:<20}\x1b[{};{}H\x1b[38;5;{}m- \x1b[{};{}H\x1b[38;5;{}m{:<20}\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                current_line, CONTENT_MARGIN + 1,
                text_fg_code,
                home_team.chars().take(20).collect::<String>(),
                current_line, SEPARATOR_OFFSET + CONTENT_MARGIN + 1,
                text_fg_code,
                current_line, AWAY_TEAM_OFFSET + CONTENT_MARGIN + 1,
                text_fg_code,
                away_team.chars().take(20).collect::<String>(),
                current_line, 45 + CONTENT_MARGIN + 1,
                result_color,
                display_text
            ));
        }

        *current_line += 1;

        // Add goal events for finished/ongoing games
        if matches!(score_type, ScoreType::Ongoing | ScoreType::Final) && !goal_events.is_empty() {
            self.render_goal_events(buffer, goal_events, is_overtime, is_shootout, current_line);
        }

        // Add spacing between games in interactive mode
        if !self.ignore_height_limit {
            *current_line += 1;
        }
    }

    /// Renders goal events for a game, showing scorers for home and away teams.
    ///
    /// # Arguments
    /// * `buffer` - The string buffer to append to
    /// * `goal_events` - List of goal events to display
    /// * `is_overtime` - Whether game went to overtime
    /// * `is_shootout` - Whether game went to shootout
    /// * `current_line` - Current line position
    fn render_goal_events(
        &self,
        buffer: &mut String,
        goal_events: &[crate::data_fetcher::GoalEventData],
        is_overtime: bool,
        is_shootout: bool,
        current_line: &mut usize,
    ) {
        let home_scorer_fg_code = get_ansi_code(home_scorer_fg(), 51);
        let away_scorer_fg_code = get_ansi_code(away_scorer_fg(), 51);
        let winning_goal_fg_code = get_ansi_code(winning_goal_fg(), 201);
        let goal_type_fg_code = get_ansi_code(goal_type_fg(), 226);

        let home_scorers: Vec<_> = goal_events.iter().filter(|e| e.is_home_team).collect();
        let away_scorers: Vec<_> = goal_events.iter().filter(|e| !e.is_home_team).collect();
        let max_scorers = home_scorers.len().max(away_scorers.len());

        for i in 0..max_scorers {
            // Home team scorer
            if let Some(event) = home_scorers.get(i) {
                self.render_goal_event(
                    buffer,
                    event,
                    is_overtime,
                    is_shootout,
                    *current_line,
                    CONTENT_MARGIN + 1,
                    home_scorer_fg_code,
                    winning_goal_fg_code,
                    goal_type_fg_code,
                );
            }

            // Away team scorer
            if let Some(event) = away_scorers.get(i) {
                self.render_goal_event(
                    buffer,
                    event,
                    is_overtime,
                    is_shootout,
                    *current_line,
                    AWAY_TEAM_OFFSET + CONTENT_MARGIN + 1,
                    away_scorer_fg_code,
                    winning_goal_fg_code,
                    goal_type_fg_code,
                );
            }

            if home_scorers.get(i).is_some() || away_scorers.get(i).is_some() {
                *current_line += 1;
            }
        }
    }

    /// Renders a single goal event (scorer name, minute, video link, goal type).
    ///
    /// # Arguments
    /// * `buffer` - The string buffer to append to
    /// * `event` - The goal event to render
    /// * `is_overtime` - Whether game went to overtime
    /// * `is_shootout` - Whether game went to shootout
    /// * `current_line` - Current line position
    /// * `column_offset` - Column offset for positioning
    /// * `scorer_fg_code` - Default scorer color code
    /// * `winning_goal_fg_code` - Winning goal color code
    /// * `goal_type_fg_code` - Goal type indicator color code
    #[allow(clippy::too_many_arguments)]
    fn render_goal_event(
        &self,
        buffer: &mut String,
        event: &crate::data_fetcher::GoalEventData,
        is_overtime: bool,
        is_shootout: bool,
        current_line: usize,
        column_offset: usize,
        scorer_fg_code: u8,
        winning_goal_fg_code: u8,
        goal_type_fg_code: u8,
    ) {
        // Determine if this is a winning goal (overtime/shootout winner or "VL" penalty shot goal)
        let scorer_color = if (event.is_winning_goal && (is_overtime || is_shootout))
            || event.goal_types.contains(&"VL".to_string())
        {
            winning_goal_fg_code
        } else {
            scorer_fg_code
        };

        // Render goal minute
        buffer.push_str(&format!(
            "\x1b[{};{}H\x1b[38;5;{}m{:2} ",
            current_line, column_offset, scorer_color, event.minute
        ));

        // Add video link functionality if there's a video clip and links are enabled
        if let Some(url) = &event.video_clip_url {
            if !self.disable_video_links {
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

        // Add goal type indicators (e.g., "YV" for overtime, "RL" for shootout)
        let goal_type = event.get_goal_type_display();
        if !goal_type.is_empty() {
            buffer.push_str(&format!(
                " \x1b[38;5;{goal_type_fg_code}m{goal_type}\x1b[0m"
            ));
        } else {
            buffer.push_str("\x1b[0m");
        }
    }

    /// Renders an error message row.
    ///
    /// # Arguments
    /// * `buffer` - The string buffer to append to
    /// * `message` - The error message text
    /// * `current_line` - Current line position
    /// * `text_fg_code` - Text color code
    fn render_error_message(
        &self,
        buffer: &mut String,
        message: &str,
        current_line: &mut usize,
        text_fg_code: u8,
    ) {
        for line in message.lines() {
            buffer.push_str(&format!(
                "\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                current_line,
                CONTENT_MARGIN + 1,
                text_fg_code,
                line
            ));
            *current_line += 1;
        }
    }

    /// Renders a future games header (e.g., "Seuraavat ottelut 15.01.2024").
    ///
    /// # Arguments
    /// * `buffer` - The string buffer to append to
    /// * `header_text` - The header text to display
    /// * `current_line` - Current line position
    /// * `subheader_fg_code` - Subheader color code
    fn render_future_games_header(
        &self,
        buffer: &mut String,
        header_text: &str,
        current_line: &mut usize,
        subheader_fg_code: u8,
    ) {
        buffer.push_str(&format!(
            "\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
            current_line,
            CONTENT_MARGIN + 1,
            subheader_fg_code,
            header_text
        ));
        *current_line += 1;
    }
}
