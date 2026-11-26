// src/teletext_ui/game_display.rs - Normal mode game rendering logic

use super::core::get_ansi_code;
use super::core::{TeletextPage, TeletextRow};
use super::layout::{ColumnLayoutManager, IntelligentTruncator, LayoutConfig, ansi};
use crate::data_fetcher::GoalEventData;
use crate::data_fetcher::models::GameData;
use crate::teletext_ui::{CONTENT_MARGIN, ScoreType};
use crate::ui::teletext::colors::*;

pub(crate) fn format_team_series_indicator(wins: u8, req_wins: u8) -> String {
    let filled = "●".repeat(wins as usize);
    let empty = "○".repeat(req_wins.saturating_sub(wins) as usize);
    format!("{filled}{empty}")
}

/// Web player for goal clips, served from `pages/play.html` on GitHub Pages.
/// Liiga's clips are HLS streams, which Firefox and other browsers without
/// built-in HLS support download instead of playing. The page plays them with hls.js.
const VIDEO_PLAYER_URL: &str = "https://nikosalonen.github.io/liiga_teletext/play.html";
/// Only clips under this prefix go through the player page. Keep it in sync
/// with `ALLOWED_HOST` in `pages/play.html`, which rejects every other host.
const LIIGA_VIDEO_PREFIX: &str = "https://videos.liiga.fi/";

/// The URL a goal's video link opens. Liiga clips go through the player page,
/// with the clip URL after the `#` so it is never sent to the page's server.
/// Other URLs stay as they are, because the page only plays Liiga clips.
fn video_link_url(clip_url: &str) -> String {
    if clip_url.starts_with(LIIGA_VIDEO_PREFIX) {
        format!("{VIDEO_PLAYER_URL}#{clip_url}")
    } else {
        clip_url.to_string()
    }
}

/// Line shown in place of scorer lines cut off at the bottom of the page.
pub(super) fn hidden_goals_text(hidden_goals: usize) -> String {
    if hidden_goals == 1 {
        "+1 maali".to_string()
    } else {
        format!("+{hidden_goals} maalia")
    }
}

/// The score with its shootout ("rl") or overtime ("ja") suffix.
pub(super) fn result_with_suffix(result: &str, is_overtime: bool, is_shootout: bool) -> String {
    if is_shootout {
        format!("{result} rl")
    } else if is_overtime {
        format!("{result} ja")
    } else {
        result.to_string()
    }
}

/// What a game line shows as `(time, score)`: a scheduled game its start
/// time, an ongoing game the game clock and the score, a finished game the
/// score. `score` already carries the overtime or shootout suffix.
pub(super) fn time_and_score_display(
    score_type: &ScoreType,
    time: &str,
    score: String,
    played_time: i32,
) -> (String, String) {
    match score_type {
        ScoreType::Scheduled => (time.to_string(), String::new()),
        ScoreType::Ongoing => (
            format!("{:02}:{:02}", played_time / 60, played_time % 60),
            score,
        ),
        ScoreType::Final => (String::new(), score),
    }
}

/// Goals drawn in the winning-goal color: the overtime or shootout winner,
/// and penalty shot goals ("VL").
pub(super) fn is_highlighted_goal(
    event: &GoalEventData,
    is_overtime: bool,
    is_shootout: bool,
) -> bool {
    (event.is_winning_goal && (is_overtime || is_shootout))
        || event.goal_types.iter().any(|goal_type| goal_type == "VL")
}

impl TeletextPage {
    /// True when the goal has a video clip and video links are turned on.
    pub(super) fn shows_video_link(&self, event: &GoalEventData) -> bool {
        !self.disable_video_links
            && event
                .video_clip_url
                .as_ref()
                .is_some_and(|url| !url.trim().is_empty())
    }

    /// Last screen row game content may use. The two rows below it hold the
    /// loading line and the footer. Unlimited in non-interactive mode, where
    /// the page grows to fit its content.
    pub(super) fn last_content_line(&self) -> usize {
        if self.ignore_height_limit {
            usize::MAX
        } else {
            self.screen_height.saturating_sub(2) as usize
        }
    }

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
                    ..
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
                        play_off_phase: None,
                        play_off_pair: None,
                        play_off_req_wins: None,
                        series_score: None,
                        is_placeholder: false,
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
        let terminal_width = self.layout_width() as usize;

        let layout_config = if self.wide_mode && self.can_fit_two_pages() {
            // In wide mode, calculate layout for individual columns (approximately 60 chars each)
            let wide_column_width = 60; // Typical wide mode column width
            ColumnLayoutManager::new_for_wide_mode_column(wide_column_width, CONTENT_MARGIN)
                .calculate_wide_mode_layout(&games_for_layout)
        } else {
            ColumnLayoutManager::new(terminal_width, CONTENT_MARGIN)
                .calculate_layout(&games_for_layout)
        };

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
                    series_score,
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
                        series_score.as_ref(),
                        current_line,
                        text_fg_code,
                        result_fg_code,
                        &layout_config,
                    );
                }
                TeletextRow::ErrorMessage(message) => {
                    self.render_error_message(buffer, message, current_line, text_fg_code);
                }
                TeletextRow::FutureGamesHeader(header_text)
                | TeletextRow::PlayoffPhaseHeader(header_text)
                | TeletextRow::SeriesHeader(header_text) => {
                    self.render_future_games_header(
                        buffer,
                        header_text,
                        current_line,
                        subheader_fg_code,
                    );
                }
                TeletextRow::StandingsHeader => {
                    self.render_standings_header(buffer, current_line, text_fg_code);
                }
                TeletextRow::StandingsRow {
                    position,
                    team_name,
                    games_played,
                    wins,
                    ot_wins,
                    ot_losses,
                    losses,
                    goals_for,
                    goals_against,
                    points,
                    live_goals_for,
                    live_goals_against,
                    live_points_delta,
                    live_position_change,
                    live_game_active,
                } => {
                    self.render_standings_row(
                        buffer,
                        *position,
                        team_name,
                        *games_played,
                        *wins,
                        *ot_wins,
                        *ot_losses,
                        *losses,
                        *goals_for,
                        *goals_against,
                        *points,
                        *live_goals_for,
                        *live_goals_against,
                        live_points_delta,
                        live_position_change,
                        *live_game_active,
                        current_line,
                    );
                }
                TeletextRow::BracketLine(line) => {
                    let line_code =
                        format!("\x1b[{};{}H{}", *current_line + 1, CONTENT_MARGIN + 1, line);
                    buffer.push_str(&line_code);
                    *current_line += 1;
                }
                TeletextRow::BracketPageBreak => {}
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
        series_score: Option<&crate::data_fetcher::models::PlayoffSeriesScore>,
        current_line: &mut usize,
        text_fg_code: u8,
        result_fg_code: u8,
        layout_config: &LayoutConfig,
    ) {
        let (time_display, score_display) = time_and_score_display(
            score_type,
            time,
            result_with_suffix(result, is_overtime, is_shootout),
            played_time,
        );

        let result_color = match score_type {
            ScoreType::Final => result_fg_code,
            ScoreType::Scheduled => get_ansi_code(scheduled_time_fg(), 51),
            ScoreType::Ongoing => text_fg_code,
        };

        if !time_display.is_empty() && !score_display.is_empty() {
            // For ongoing games: show time on the left, score aligned with finished games on the right
            let line = *current_line;
            let home_pos = CONTENT_MARGIN + 1;
            let separator_pos = home_pos + layout_config.home_team_width;
            let away_pos = separator_pos + layout_config.separator_width;

            buffer.push_str(&ansi::padded_text_at(
                line,
                home_pos,
                text_fg_code,
                home_team,
                layout_config.home_team_width,
            ));
            buffer.push_str(&ansi::separator_at(line, separator_pos, text_fg_code));
            buffer.push_str(&ansi::padded_text_at(
                line,
                away_pos,
                text_fg_code,
                away_team,
                layout_config.away_team_width,
            ));
            buffer.push_str(&ansi::text_at(
                line,
                layout_config.time_column,
                text_fg_code,
                &time_display,
            ));
            buffer.push_str(&ansi::text_at(
                line,
                layout_config.score_column,
                result_color,
                &score_display,
            ));
        } else {
            // For scheduled/final games: show time or score on the right
            let display_text = if !time_display.is_empty() {
                time_display
            } else {
                score_display
            };

            buffer.push_str(&ansi::game_line_at(
                *current_line,
                layout_config,
                home_team,
                away_team,
                &display_text,
                text_fg_code,
                result_color,
            ));
        }

        // Render series win indicators after each team name
        if let Some(score) = series_score
            && score.req_wins > 1
        {
            let goal_type_fg_code = get_ansi_code(goal_type_fg(), 226);
            let home_pos = CONTENT_MARGIN + 1;

            // Home team indicator: after home team name
            let home_indicator = format_team_series_indicator(score.home_team_wins, score.req_wins);
            let home_indicator_pos = home_pos + home_team.chars().count() + 1;
            let home_code = ansi::position_code(*current_line, home_indicator_pos);
            buffer.push_str(&format!(
                "{home_code}\x1b[38;5;{goal_type_fg_code}m{home_indicator}\x1b[0m"
            ));

            // Away team indicator: after away team name
            let away_indicator = format_team_series_indicator(score.away_team_wins, score.req_wins);
            let separator_pos = home_pos + layout_config.home_team_width;
            let away_pos = separator_pos + layout_config.separator_width;
            let away_indicator_pos = away_pos + away_team.chars().count() + 1;
            let away_code = ansi::position_code(*current_line, away_indicator_pos);
            buffer.push_str(&format!(
                "{away_code}\x1b[38;5;{goal_type_fg_code}m{away_indicator}\x1b[0m"
            ));
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

        let (home_scorers, away_scorers): (Vec<_>, Vec<_>) =
            goal_events.iter().partition(|e| e.is_home_team);
        let max_scorers = home_scorers.len().max(away_scorers.len());

        // Pagination places a game taller than the whole page on its own page;
        // stop above the loading line and footer, and use the last line that
        // fits to say how many goals are not shown.
        let last_content_line = self.last_content_line();

        for i in 0..max_scorers {
            if *current_line > last_content_line {
                break;
            }
            if *current_line == last_content_line && i + 1 < max_scorers {
                let hidden_goals =
                    home_scorers.len().saturating_sub(i) + away_scorers.len().saturating_sub(i);
                buffer.push_str(&format!(
                    "\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                    *current_line,
                    CONTENT_MARGIN + 1,
                    goal_type_fg_code,
                    hidden_goals_text(hidden_goals)
                ));
                *current_line += 1;
                break;
            }
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
    /// Includes comprehensive safe fallbacks for missing data
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
        // Use safe rendering with comprehensive error handling
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
        let scorer_color = if is_highlighted_goal(event, is_overtime, is_shootout) {
            winning_goal_fg_code
        } else {
            scorer_fg_code
        };

        // Render goal minute with safe fallback for invalid data
        let safe_minute = if event.minute < 0 || event.minute > 200 {
            tracing::debug!(
                "Invalid goal minute {} in event, using fallback: 0",
                event.minute
            );
            0
        } else {
            event.minute
        };

        let minute_code = ansi::color_position_code(current_line, column_offset, scorer_color);
        buffer.push_str(&format!("{minute_code}{safe_minute:2} "));

        // Calculate dynamic spacing for player name to maintain play icon alignment

        // Handle missing or corrupted player names safely
        let safe_player_name = if event.scorer_name.trim().is_empty() {
            tracing::debug!("Empty player name in goal event, using fallback");
            "Unknown Player".to_string()
        } else {
            event.scorer_name.clone()
        };

        // Store original length before truncation for spacing calculation
        let original_player_name_length = safe_player_name.chars().count();

        // Use intelligent truncation for player names
        let player_name_display =
            if original_player_name_length > layout_config.max_player_name_width {
                // Use intelligent truncation with ellipsis only as last resort
                IntelligentTruncator::truncate_player_name(
                    &safe_player_name,
                    layout_config.max_player_name_width,
                    Some(5),
                )
            } else {
                safe_player_name
            };

        // Calculate available space for player name based on team position.
        // Reserve room for the longest goal-type combination in the game
        // (not just this event's) so the ▶ icons and goal-type indicators
        // stay vertically aligned across all scorer rows.
        let goal_type_reservation = if layout_config.max_goal_types_width > 0 {
            layout_config.max_goal_types_width + 1 // +1 for space before goal types
        } else {
            0
        };
        let available_space_for_name = if event.is_home_team {
            // For home team: content (name + icon + goal types) may use the
            // blank gutter of the separator up to (but never under) the "-"
            // divider, which sits at the center of the separator field.
            let home_pos = CONTENT_MARGIN + 1;
            let separator_start = home_pos + layout_config.home_team_width;
            let divider_column = separator_start + layout_config.separator_width / 2;
            let name_start = column_offset + 3; // minute (2) + space (1)
            divider_column
                .saturating_sub(name_start)
                .saturating_sub(goal_type_reservation)
        } else {
            // For away team: calculate space until time column
            let name_start = column_offset + 3;
            layout_config
                .time_column
                .saturating_sub(name_start)
                .saturating_sub(goal_type_reservation)
                .saturating_sub(1)
        };

        // Pad player name to available space, but not more than max_player_name_width
        // Reserve 1 character for play icon (▶) when video link is available
        let has_video = self.shows_video_link(event);
        let play_icon_width = if has_video { 1 } else { 0 };
        let padding_width = available_space_for_name
            .saturating_sub(play_icon_width)
            .min(layout_config.max_player_name_width);
        // Truncate the player name gracefully (ellipsis) when it doesn't fit,
        // so the play icon never overflows the column
        let truncated_player_name: String = if player_name_display.chars().count() > padding_width {
            IntelligentTruncator::truncate_player_name(&player_name_display, padding_width, Some(5))
        } else {
            player_name_display
        };
        let padded_player_name =
            format!("{:<width$}", truncated_player_name, width = padding_width);

        // Render player name (make it clickable if there's a video link)
        let player_name_with_link = if let Some(url) = &event.video_clip_url {
            if !self.disable_video_links {
                // Validate URL safety before rendering
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
                        scorer_color,
                        video_link_url(&validated_url),
                        padded_player_name
                    )
                } else {
                    format!("\x1b[38;5;{}m{}", scorer_color, padded_player_name)
                }
            } else {
                format!("\x1b[38;5;{}m{}", scorer_color, padded_player_name)
            }
        } else {
            format!("\x1b[38;5;{}m{}", scorer_color, padded_player_name)
        };

        buffer.push_str(&player_name_with_link);

        // Show play icon when goal has a video clip link
        if has_video {
            let play_icon_fg_code = get_ansi_code(home_scorer_fg(), 51);
            buffer.push_str(&format!("\x1b[38;5;{play_icon_fg_code}m▶\x1b[0m"));
        }

        // Get goal type display with safe fallback for missing data
        let goal_type = event.get_goal_type_display();

        // Add single space before goal type (player name is already padded to fixed width)
        buffer.push(' ');

        // Add goal type indicators with overflow prevention and validation
        if !goal_type.is_empty() {
            // Validate that goal types can be displayed without truncation
            if !IntelligentTruncator::validate_goal_types_no_truncation(
                &goal_type,
                layout_config.max_goal_types_width,
            ) {
                tracing::warn!(
                    "Goal types '{}' exceed allocated width {}. Goal types should never be truncated.",
                    goal_type,
                    layout_config.max_goal_types_width
                );
            }

            // Calculate the actual rendered position of the goal types:
            // minute (2) + space (1) + padded name + play icon + space
            let goal_type_start_position = column_offset + 3 + padding_width + play_icon_width + 1;
            let goal_type_end_position = goal_type_start_position + goal_type.len();

            // Calculate the boundary based on whether this is home or away team
            let boundary_column = if event.is_home_team {
                // For home team: goal types may use the separator's blank
                // gutter but must never render under the "-" divider
                let home_pos = CONTENT_MARGIN + 1;
                let separator_start = home_pos + layout_config.home_team_width;
                separator_start + layout_config.separator_width / 2
            } else {
                // For away team: use the time column position minus some margin
                layout_config.time_column.saturating_sub(2) // Leave space before time column
            };

            if goal_type_end_position <= boundary_column {
                // Safe to render at current position - use optimized formatting
                let formatted_goal_type =
                    format!("\x1b[38;5;{goal_type_fg_code}m{goal_type}\x1b[0m");
                buffer.push_str(&formatted_goal_type);
            } else {
                // Position goal types at safe location to prevent overflow
                // Never truncate goal types - always find a safe position
                let safe_position =
                    boundary_column.saturating_sub(goal_type.len().min(boundary_column));

                let formatted_goal_type =
                    ansi::text_at(current_line, safe_position, goal_type_fg_code, &goal_type);
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
        for line in message.lines() {
            buffer.push_str(&ansi::text_at(
                *current_line,
                CONTENT_MARGIN + 1,
                text_fg_code,
                line,
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
        buffer.push_str(&ansi::text_at(
            *current_line,
            CONTENT_MARGIN + 1,
            subheader_fg_code,
            header_text,
        ));
        *current_line += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::GoalEventData;

    /// Renders one finished playoff game (with series dots and a scorer for
    /// each team) the way the normal-mode page draws it, at a fixed 80-column
    /// width.
    fn render_playoff_game(home_team: &str, away_team: &str, scorer: &str) -> String {
        use crate::data_fetcher::models::PlayoffSeriesScore;
        use crate::teletext_ui::GameResultData;

        let mut game =
            crate::testing_utils::TestDataBuilder::create_basic_game(home_team, away_team);
        game.score_type = ScoreType::Final;
        game.result = "1-1".to_string();
        game.goal_events = vec![
            crate::testing_utils::TestDataBuilder::create_goal_event(scorer, 12, 1, 0, true),
            crate::testing_utils::TestDataBuilder::create_goal_event(scorer, 40, 1, 1, false),
        ];
        game.play_off_phase = Some(1);
        game.play_off_pair = Some(1);
        game.play_off_req_wins = Some(4);
        game.series_score = Some(PlayoffSeriesScore {
            home_team_wins: 2,
            away_team_wins: 1,
            req_wins: 4,
        });

        let mut page = TeletextPage::new(221, "JÄÄKIEKKO", "PLAYOFFS")
            .with_video_links_disabled(true)
            .with_footer(false)
            .with_ignore_height_limit(true);
        page.add_game_result(GameResultData::new(&game));

        let (rows, _) = page.get_page_content();
        let mut buffer = String::new();
        let mut current_line = 4;
        page.render_normal_mode_content(&mut buffer, &rows, &mut current_line, 231, 46, 46);
        buffer
    }

    #[test]
    fn liiga_clip_links_open_the_player_page() {
        assert_eq!(
            video_link_url("https://videos.liiga.fi/videos/abc/playlist.m3u8"),
            "https://nikosalonen.github.io/liiga_teletext/play.html#https://videos.liiga.fi/videos/abc/playlist.m3u8"
        );
    }

    #[test]
    fn other_video_links_stay_unchanged() {
        // The player page only plays clips from Liiga's own video host over https.
        for url in [
            "https://example.com/video.mp4",
            "https://videos.liiga.fi.example.com/videos/abc/playlist.m3u8",
            "http://videos.liiga.fi/videos/abc/playlist.m3u8",
        ] {
            assert_eq!(video_link_url(url), url);
        }
    }

    #[test]
    fn scorer_link_opens_the_player_page() {
        let mut game = crate::testing_utils::TestDataBuilder::create_basic_game("Tappara", "HIFK");
        game.score_type = ScoreType::Final;
        game.result = "1-0".to_string();
        let mut goal =
            crate::testing_utils::TestDataBuilder::create_goal_event("Puutio", 3, 1, 0, true);
        goal.video_clip_url = Some("https://videos.liiga.fi/videos/abc/playlist.m3u8".to_string());
        game.goal_events = vec![goal];

        let mut page = TeletextPage::new(221, "JÄÄKIEKKO", "RUNKOSARJA")
            .with_footer(false)
            .with_ignore_height_limit(true);
        page.add_game_result(crate::teletext_ui::GameResultData::new(&game));
        let (rows, _) = page.get_page_content();
        let mut buffer = String::new();
        let mut current_line = 4;
        page.render_normal_mode_content(&mut buffer, &rows, &mut current_line, 231, 46, 46);

        assert!(buffer.contains(
            "\x1b]8;;https://nikosalonen.github.io/liiga_teletext/play.html#https://videos.liiga.fi/videos/abc/playlist.m3u8\x07"
        ));
    }

    #[test]
    fn test_finnish_names_are_positioned_by_display_width() {
        // Ä/Ö are one column wide but two bytes. Positions computed from byte
        // length put the series dots after "Kärpät" two columns too far right.
        let finnish = render_playoff_game("Kärpät", "Ässät", "Hämäläinen");
        let ascii = render_playoff_game("Karpat", "Assat", "Hamalainen");

        assert_eq!(
            finnish
                .replace("Kärpät", "Karpat")
                .replace("Ässät", "Assat")
                .replace("Hämäläinen", "Hamalainen"),
            ascii
        );
    }

    /// Strips ANSI escape sequences (CSI and OSC) to recover visible text
    fn strip_ansi(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c != '\x1b' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('[') => {
                    for n in chars.by_ref() {
                        if n.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                Some(']') => {
                    for n in chars.by_ref() {
                        if n == '\x07' {
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    fn make_home_event(name: &str, goal_types: &[&str]) -> GoalEventData {
        GoalEventData {
            scorer_player_id: 1,
            scorer_name: name.to_string(),
            minute: 4,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: goal_types.iter().map(|s| s.to_string()).collect(),
            is_home_team: true,
            video_clip_url: None,
        }
    }

    fn render_home_event_with_config(
        name: &str,
        goal_types: &[&str],
        layout_config: &LayoutConfig,
    ) -> String {
        let page = TeletextPage::new(221, "JÄÄKIEKKO", "SM-LIIGA")
            .with_video_links_disabled(true)
            .with_footer(false)
            .with_ignore_height_limit(true);
        let event = make_home_event(name, goal_types);
        let mut buffer = String::new();
        page.render_goal_event_safe(
            &mut buffer,
            &event,
            false,
            false,
            5,
            CONTENT_MARGIN + 1,
            51,
            201,
            226,
            layout_config,
        )
        .unwrap();
        strip_ansi(&buffer)
    }

    fn render_home_event(goal_types: &[&str]) -> String {
        render_home_event_with_config("Borchardt", goal_types, &LayoutConfig::default())
    }

    #[test]
    fn test_long_player_name_not_clipped() {
        // Regression test: an 11-character name must render in full when a
        // game's longest goal-type combination is "YV VT" (5 chars wide).
        // The name field may use the separator's blank gutter for this.
        let layout_config = LayoutConfig {
            max_goal_types_width: 5,
            ..LayoutConfig::default()
        };
        let visible = render_home_event_with_config("Valkeejärvi", &[], &layout_config);
        assert!(
            visible.contains("Valkeejärvi"),
            "long player name was clipped: {visible:?}"
        );
    }

    #[test]
    fn test_home_goal_types_stay_within_home_column() {
        // Regression test: with two goal types (e.g. "YV VT") the goal type
        // column must not spill under the team separator ("-" divider).
        // Content may use the separator's blank gutter up to (but never
        // under) the divider at the separator's center.
        let layout_config = LayoutConfig::default();
        let max_content_width = layout_config.home_team_width + layout_config.separator_width / 2;
        for types in [&["YV"][..], &["YV", "VT"][..]] {
            let visible = render_home_event(types);
            let visible_len = visible.trim_end().chars().count();
            assert!(
                visible_len <= max_content_width,
                "home scorer row {visible:?} ({visible_len} cols) would overlap the divider (limit {max_content_width})"
            );
        }
    }

    #[test]
    fn test_goal_type_column_aligned_across_rows() {
        // Goal types must start at the same column whether a row has one
        // or two goal type indicators
        let single = render_home_event(&["YV"]);
        let double = render_home_event(&["YV", "VT"]);
        let single_idx = single.find("YV").unwrap();
        let double_idx = double.find("YV VT").unwrap();
        assert_eq!(
            single_idx, double_idx,
            "goal type column misaligned: {single:?} vs {double:?}"
        );
    }

    #[test]
    fn test_result_with_suffix() {
        assert_eq!(result_with_suffix("3-2", false, false), "3-2");
        assert_eq!(result_with_suffix("3-2", true, false), "3-2 ja");
        assert_eq!(result_with_suffix("3-2", false, true), "3-2 rl");
        // A shootout also went through overtime; the shootout wins
        assert_eq!(result_with_suffix("3-2", true, true), "3-2 rl");
    }

    #[test]
    fn test_time_and_score_display() {
        let scheduled =
            time_and_score_display(&ScoreType::Scheduled, "18:30", "0-0".to_string(), 0);
        assert_eq!(scheduled, ("18:30".to_string(), String::new()));

        let ongoing = time_and_score_display(&ScoreType::Ongoing, "18:30", "1-0".to_string(), 754);
        assert_eq!(ongoing, ("12:34".to_string(), "1-0".to_string()));

        let finished =
            time_and_score_display(&ScoreType::Final, "18:30", "3-2 ja".to_string(), 3900);
        assert_eq!(finished, (String::new(), "3-2 ja".to_string()));
    }

    #[test]
    fn test_is_highlighted_goal() {
        let mut winner = make_home_event("Koivu", &["YV"]);
        winner.is_winning_goal = true;
        assert!(is_highlighted_goal(&winner, true, false));
        assert!(is_highlighted_goal(&winner, false, true));
        // A regulation-time winner is not highlighted
        assert!(!is_highlighted_goal(&winner, false, false));

        let penalty_shot = make_home_event("Selänne", &["VL"]);
        assert!(is_highlighted_goal(&penalty_shot, false, false));

        let regular = make_home_event("Kurri", &["YV", "AV"]);
        assert!(!is_highlighted_goal(&regular, true, false));
    }
}
