// src/teletext_ui/game_display.rs - Normal mode game rendering logic

use super::core::AWAY_TEAM_OFFSET;
use super::core::{TeletextPage, TeletextRow};
use super::layout::{ColumnLayoutManager, LayoutConfig};
use super::utils::get_ansi_code;
use crate::data_fetcher::models::GameData;
use crate::teletext_ui::{CONTENT_MARGIN, ScoreType};
use crate::ui::teletext::colors::*;

impl TeletextPage {
    /// Extracts GameData from TeletextRows for layout calculation
    pub(crate) fn extract_games_for_layout(&self, visible_rows: &[&TeletextRow]) -> Vec<GameData> {
        visible_rows
            .iter()
            .filter_map(|row| {
                if let TeletextRow::GameResult {
                    home_team,
                    away_team,
                    time,
                    result,
                    score_type,
                    is_overtime,
                    is_shootout,
                    goal_events,
                    played_time,
                } = row
                {
                    Some(GameData {
                        home_team: home_team.clone(),
                        away_team: away_team.clone(),
                        time: time.clone(),
                        result: result.clone(),
                        score_type: score_type.clone(),
                        is_overtime: *is_overtime,
                        is_shootout: *is_shootout,
                        serie: "RUNKOSARJA".to_string(), // Default value for layout calculation
                        goal_events: goal_events.clone(),
                        played_time: *played_time,
                        start: "".to_string(), // Not needed for layout calculation
                    })
                } else {
                    None
                }
            })
            .collect()
    }

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
        // Calculate layout configuration based on game content
        let games_for_layout = self.extract_games_for_layout(visible_rows);
        let terminal_width = if self.ignore_height_limit {
            if self.wide_mode { 136 } else { 80 }
        } else {
            crossterm::terminal::size()
                .map(|(w, _)| w as usize)
                .unwrap_or(80)
        };

        let mut layout_manager = ColumnLayoutManager::new(terminal_width, CONTENT_MARGIN);
        let layout_config = if self.wide_mode && self.can_fit_two_pages() {
            // In wide mode, calculate layout for individual columns (approximately 60 chars each)
            let wide_column_width = 60; // Typical wide mode column width
            let mut wide_layout_manager =
                ColumnLayoutManager::new_for_wide_mode_column(wide_column_width, CONTENT_MARGIN);
            wide_layout_manager.calculate_wide_mode_layout(&games_for_layout)
        } else {
            layout_manager.calculate_layout(&games_for_layout)
        };

        // Pre-calculate ANSI positioning codes for optimal performance (requirement 4.3)
        let estimated_lines = visible_rows.len() * 3; // Estimate 3 lines per game (game + goals + spacing)
        layout_manager.pre_calculate_ansi_codes(&layout_config, estimated_lines);

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
                        &layout_config,
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
    /// * `layout_config` - Layout configuration with calculated positions
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
        layout_config: &LayoutConfig,
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

        // Use optimized ANSI code generation for better performance (requirement 4.3)
        let mut layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN); // Use default for ANSI generation

        if !time_display.is_empty() && !score_display.is_empty() {
            // For ongoing games: show time on the left, score aligned with finished games on the right
            // Use batch ANSI code generation for optimal performance
            let home_pos = CONTENT_MARGIN + 1;
            let separator_pos = home_pos + layout_config.home_team_width;
            let away_pos = separator_pos + layout_config.separator_width;
            let time_pos = layout_config.time_column;
            let score_pos = layout_config.score_column;

            // Batch string operations to reduce allocations (requirement 4.3)
            let mut game_line = String::with_capacity(200); // Pre-allocate for better performance

            game_line.push_str(&layout_manager.format_team_name(
                *current_line,
                home_pos,
                text_fg_code,
                home_team,
                20,
            ));
            game_line.push_str(&layout_manager.format_separator(
                *current_line,
                separator_pos,
                text_fg_code,
            ));
            game_line.push_str(&layout_manager.format_team_name(
                *current_line,
                away_pos,
                text_fg_code,
                away_team,
                20,
            ));
            game_line.push_str(&layout_manager.format_time_score(
                *current_line,
                time_pos,
                text_fg_code,
                &time_display,
            ));
            game_line.push_str(&layout_manager.format_time_score(
                *current_line,
                score_pos,
                result_color,
                &score_display,
            ));

            buffer.push_str(&game_line);
        } else {
            // For scheduled/final games: show time or score on the right
            let display_text = if !time_display.is_empty() {
                time_display
            } else {
                score_display
            };

            // Use optimized complete game line formatting (requirement 4.3)
            let formatted_line = layout_manager.format_complete_game_line(
                *current_line,
                layout_config,
                home_team,
                away_team,
                &display_text,
                text_fg_code,
                result_color,
            );
            buffer.push_str(&formatted_line);
        }

        *current_line += 1;

        // Add goal events for finished/ongoing games
        if matches!(score_type, ScoreType::Ongoing | ScoreType::Final) && !goal_events.is_empty() {
            self.render_goal_events(
                buffer,
                goal_events,
                is_overtime,
                is_shootout,
                current_line,
                layout_config,
            );
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
    /// * `layout_config` - Layout configuration with calculated positions
    fn render_goal_events(
        &self,
        buffer: &mut String,
        goal_events: &[crate::data_fetcher::GoalEventData],
        is_overtime: bool,
        is_shootout: bool,
        current_line: &mut usize,
        layout_config: &LayoutConfig,
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
                    layout_config,
                );
            }

            // Away team scorer - align with away team name position
            if let Some(event) = away_scorers.get(i) {
                // Calculate away team position same as in game header
                let home_pos = CONTENT_MARGIN + 1;
                let separator_pos = home_pos + layout_config.home_team_width;
                let away_pos = separator_pos + layout_config.separator_width;

                self.render_goal_event(
                    buffer,
                    event,
                    is_overtime,
                    is_shootout,
                    *current_line,
                    away_pos,
                    away_scorer_fg_code,
                    winning_goal_fg_code,
                    goal_type_fg_code,
                    layout_config,
                );
            }

            if home_scorers.get(i).is_some() || away_scorers.get(i).is_some() {
                *current_line += 1;
            }
        }
    }

    /// Renders a single goal event (scorer name, minute, video link, goal type).
    /// Includes comprehensive safe fallbacks for missing data (requirement 4.1)
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
    /// * `layout_config` - Layout configuration with calculated positions
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
        layout_config: &LayoutConfig,
    ) {
        // Use safe rendering with comprehensive error handling (requirement 4.1)
        // Instead of panic handling, use Result-based error handling for safety
        match self.render_goal_event_safe(
            buffer,
            event,
            is_overtime,
            is_shootout,
            current_line,
            column_offset,
            scorer_fg_code,
            winning_goal_fg_code,
            goal_type_fg_code,
            layout_config,
        ) {
            Ok(()) => {
                // Rendering succeeded
            }
            Err(error_msg) => {
                tracing::error!(
                    "Goal event rendering failed: {}, using minimal fallback",
                    error_msg
                );
                // Minimal fallback rendering to ensure something appears
                let safe_minute = if event.minute >= 0 && event.minute <= 200 {
                    event.minute
                } else {
                    0
                };
                let safe_name = if event.scorer_name.trim().is_empty() {
                    "Unknown"
                } else {
                    &event.scorer_name
                };

                buffer.push_str(&format!(
                    "\x1b[{};{}H\x1b[38;5;{}m{:2} {}\x1b[0m",
                    current_line, column_offset, scorer_fg_code, safe_minute, safe_name
                ));
            }
        }
    }

    /// Internal safe rendering method for goal events
    /// This method contains the actual rendering logic with safe fallbacks
    /// Returns Result to handle errors gracefully without panics
    #[allow(clippy::too_many_arguments)]
    fn render_goal_event_safe(
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
        layout_config: &LayoutConfig,
    ) -> Result<(), String> {
        use super::layout::AlignmentCalculator;

        // Determine if this is a winning goal (overtime/shootout winner or "VL" penalty shot goal)
        let scorer_color = if (event.is_winning_goal && (is_overtime || is_shootout))
            || event.goal_types.contains(&"VL".to_string())
        {
            winning_goal_fg_code
        } else {
            scorer_fg_code
        };

        // Render goal minute with safe fallback for invalid data (requirement 4.1)
        let safe_minute = if event.minute < 0 || event.minute > 200 {
            tracing::debug!(
                "Invalid goal minute {} in event, using fallback: 0",
                event.minute
            );
            0
        } else {
            event.minute
        };

        // Use optimized ANSI code generation for goal minute (requirement 4.3)
        let mut layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
        let minute_code =
            layout_manager.get_color_position_code(current_line, column_offset, scorer_color);
        buffer.push_str(&format!("{}{:2} ", minute_code, safe_minute));

        // Calculate dynamic spacing for player name to maintain play icon alignment
        // Note: We create separate layout managers for each operation to avoid borrowing issues

        // Handle missing or corrupted player names safely (requirement 4.1)
        let safe_player_name = if event.scorer_name.trim().is_empty() {
            tracing::debug!("Empty player name in goal event, using fallback");
            "Unknown Player".to_string()
        } else {
            event.scorer_name.clone()
        };

        let player_name_length = safe_player_name.len();

        // Use intelligent truncation for player names (requirement 3.2)
        use super::layout::IntelligentTruncator;
        let truncator = IntelligentTruncator::new();

        let player_name_display = if player_name_length > layout_config.max_player_name_width {
            // Use intelligent truncation with ellipsis only as last resort (requirement 3.2)
            truncator.truncate_player_name(
                &safe_player_name,
                layout_config.max_player_name_width,
                Some(5),
            )
        } else {
            safe_player_name
        };

        // Render player name (make it clickable if there's a video link)
        let player_name_with_link = if let Some(url) = &event.video_clip_url {
            if !self.disable_video_links {
                // Validate URL safety before rendering (requirement 4.1)
                let safe_url = if url.trim().is_empty() {
                    tracing::debug!("Empty video URL in goal event, skipping video link");
                    None
                } else if url.len() > 500 {
                    // Reasonable URL length limit
                    tracing::warn!(
                        "Video URL too long ({} chars), truncating for safety",
                        url.len()
                    );
                    Some(url.chars().take(500).collect::<String>())
                } else {
                    Some(url.clone())
                };

                if let Some(validated_url) = safe_url {
                    // Wrap the player name in a clickable link while preserving colors
                    format!(
                        "\x1b[38;5;{}m\x1b]8;;{}\x07{}\x1b]8;;\x07",
                        scorer_color, validated_url, player_name_display
                    )
                } else {
                    format!("\x1b[38;5;{}m{}", scorer_color, player_name_display)
                }
            } else {
                format!("\x1b[38;5;{}m{}", scorer_color, player_name_display)
            }
        } else {
            format!("\x1b[38;5;{}m{}", scorer_color, player_name_display)
        };

        buffer.push_str(&player_name_with_link);

        // Use AlignmentCalculator to calculate goal type positions and prevent overflow
        let mut alignment_calculator = AlignmentCalculator::new();
        let goal_type_positions = alignment_calculator
            .calculate_goal_type_positions(std::slice::from_ref(event), layout_config);

        // Get goal type display with safe fallback for missing data (requirement 4.1)
        let goal_type = event.get_goal_type_display();

        // Calculate dynamic spacing based on AlignmentCalculator positioning
        let dynamic_spacing = if let Some(goal_position) = goal_type_positions.first() {
            // Validate that goal types won't overflow into away team area (requirement 3.2)
            if !alignment_calculator.validate_no_overflow(goal_position, layout_config) {
                // If overflow would occur, reduce spacing but never truncate goal types (requirement 3.4)
                let max_safe_position = 43_usize.saturating_sub(goal_type.len());
                let current_position = column_offset + 3 + player_name_display.len(); // 3 for minute + space
                if current_position < max_safe_position {
                    max_safe_position - current_position
                } else {
                    1 // Minimum spacing
                }
            } else {
                // Use calculated spacing from layout manager
                let spacing_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
                spacing_manager.calculate_dynamic_spacing(player_name_display.len(), layout_config)
            }
        } else {
            // Fallback to layout manager calculation
            let spacing_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
            spacing_manager.calculate_dynamic_spacing(player_name_display.len(), layout_config)
        };

        // Add the calculated spacing
        buffer.push_str(&" ".repeat(dynamic_spacing));

        // Video link functionality is now handled by making player names clickable above
        // This eliminates the need for separate play icons and creates a cleaner layout

        // Add goal type indicators with overflow prevention and validation (requirements 3.1, 3.2, 3.4)
        if !goal_type.is_empty() {
            // Validate that goal types can be displayed without truncation (requirement 3.4)
            if !truncator
                .validate_goal_types_no_truncation(&goal_type, layout_config.max_goal_types_width)
            {
                tracing::warn!(
                    "Goal types '{}' exceed allocated width {}. Goal types should never be truncated.",
                    goal_type,
                    layout_config.max_goal_types_width
                );
            }

            // Calculate safe position for goal types based on team (home vs away)
            let goal_type_start_position =
                column_offset + 3 + player_name_display.len() + dynamic_spacing;
            let goal_type_end_position = goal_type_start_position + goal_type.len();

            // Calculate the boundary based on whether this is home or away team
            let boundary_column = if event.is_home_team {
                // For home team: don't extend beyond the away team start position
                AWAY_TEAM_OFFSET + CONTENT_MARGIN - 1 // 30 + 2 - 1 = 31
            } else {
                // For away team: use the time column position minus some margin
                layout_config.time_column.saturating_sub(2) // Leave space before time column
            };

            if goal_type_end_position <= boundary_column {
                // Safe to render at current position - use optimized formatting (requirement 4.3)
                let formatted_goal_type =
                    format!("\x1b[38;5;{goal_type_fg_code}m{goal_type}\x1b[0m");
                buffer.push_str(&formatted_goal_type);
            } else {
                // Position goal types at safe location to prevent overflow (requirement 3.2)
                // Never truncate goal types - always find a safe position (requirement 3.4)
                let safe_position =
                    boundary_column.saturating_sub(goal_type.len().min(boundary_column));

                // Use optimized goal type formatting (requirement 4.3)
                let mut goal_layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
                let formatted_goal_type = goal_layout_manager.format_goal_types(
                    current_line,
                    safe_position,
                    goal_type_fg_code,
                    &goal_type,
                );
                buffer.push_str(&formatted_goal_type);
                tracing::debug!(
                    "Repositioned goal types '{}' to column {} to prevent overflow (boundary: {})",
                    goal_type,
                    safe_position,
                    boundary_column
                );
            }
        } else {
            // Always ensure proper ANSI reset even with empty goal types
            buffer.push_str("\x1b[0m");
        }

        Ok(())
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
        // Use optimized ANSI code generation for error messages (requirement 4.3)
        let mut layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);

        for line in message.lines() {
            let formatted_line = layout_manager.format_time_score(
                *current_line,
                CONTENT_MARGIN + 1,
                text_fg_code,
                line,
            );
            buffer.push_str(&formatted_line);
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
        // Use optimized ANSI code generation for headers (requirement 4.3)
        let mut layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
        let formatted_header = layout_manager.format_time_score(
            *current_line,
            CONTENT_MARGIN + 1,
            subheader_fg_code,
            header_text,
        );
        buffer.push_str(&formatted_header);
        *current_line += 1;
    }
}
