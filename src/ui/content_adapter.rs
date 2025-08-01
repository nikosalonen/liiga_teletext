//! Content adaptation module for dynamic content formatting
//!
//! This module provides functionality for adapting game content to different detail levels
//! based on available screen space, including team name formatting, time display formatting,
//! and goal event formatting.

use crate::data_fetcher::models::GoalEventData;
use crate::ui::layout::DetailLevel;

/// Enhanced game display data with extended information for different detail levels
#[derive(Debug, Clone)]
pub struct EnhancedGameDisplay {
    /// Base game content from the data fetcher
    pub base_content: crate::teletext_ui::GameResultData,
    /// Extended team information for larger screens
    pub extended_team_info: Option<ExtendedTeamInfo>,
    /// Detailed time information for enhanced display
    pub detailed_time_info: Option<DetailedTimeInfo>,
    /// Expanded goal details with additional information
    pub expanded_goal_details: Vec<ExpandedGoalDetail>,
}

/// Extended team information for enhanced display
#[derive(Debug, Clone)]
pub struct ExtendedTeamInfo {
    /// Full home team name without truncation
    pub full_home_name: String,
    /// Full away team name without truncation
    pub full_away_name: String,
    /// Home team record (optional)
    pub home_record: Option<String>,
    /// Away team record (optional)
    pub away_record: Option<String>,
}

/// Detailed time information for enhanced display
#[derive(Debug, Clone)]
pub struct DetailedTimeInfo {
    /// Precise timestamp with seconds
    pub precise_timestamp: String,
    /// Game duration information
    pub game_duration: Option<String>,
    /// Period information (regulation, overtime, etc.)
    pub period_info: Option<String>,
}

/// Expanded goal detail with additional information
#[derive(Debug, Clone)]
pub struct ExpandedGoalDetail {
    /// Goal scorer name
    pub scorer: String,
    /// First assist (optional)
    pub assist1: Option<String>,
    /// Second assist (optional)
    pub assist2: Option<String>,
    /// Goal time
    pub time: String,
    /// Goal situation (powerplay, shorthanded, etc.)
    pub situation: Option<String>,
}

/// Adapted content for a single game display
#[derive(Debug, Clone)]
pub struct AdaptedGameContent {
    /// Formatted home team name
    pub home_team: String,
    /// Formatted away team name
    pub away_team: String,
    /// Formatted time display
    pub time_display: String,
    /// Formatted result display
    pub result_display: String,
    /// Formatted goal event lines
    pub goal_lines: Vec<String>,
    /// Estimated height this content will take
    pub estimated_height: u16,
}

/// Content adapter for formatting game content based on detail level and available space
pub struct ContentAdapter;

impl ContentAdapter {
    /// Creates an enhanced game display from base game data
    ///
    /// # Arguments
    /// * `game_data` - Base game result data
    /// * `detail_level` - Target detail level for enhancement
    ///
    /// # Returns
    /// * `EnhancedGameDisplay` - Enhanced game display with additional information
    pub fn create_enhanced_game_display(
        game_data: crate::teletext_ui::GameResultData,
        detail_level: DetailLevel,
    ) -> EnhancedGameDisplay {
        let extended_team_info = match detail_level {
            DetailLevel::Extended => Some(ExtendedTeamInfo {
                full_home_name: game_data.home_team.clone(),
                full_away_name: game_data.away_team.clone(),
                home_record: None, // Could be populated from additional data sources
                away_record: None,
            }),
            _ => None,
        };

        let detailed_time_info = match detail_level {
            DetailLevel::Standard | DetailLevel::Extended => Some(DetailedTimeInfo {
                precise_timestamp: game_data.time.clone(),
                game_duration: Self::calculate_game_duration(game_data.played_time),
                period_info: Self::determine_period_info(&game_data),
            }),
            _ => None,
        };

        let expanded_goal_details = game_data
            .goal_events
            .iter()
            .map(|event| Self::create_expanded_goal_detail(event, detail_level))
            .collect();

        EnhancedGameDisplay {
            base_content: game_data,
            extended_team_info,
            detailed_time_info,
            expanded_goal_details,
        }
    }

    /// Adapts enhanced game display to the specified constraints
    ///
    /// # Arguments
    /// * `enhanced_game` - Enhanced game display data
    /// * `detail_level` - Target detail level for formatting
    /// * `available_width` - Available width for content display
    ///
    /// # Returns
    /// * `AdaptedGameContent` - Formatted content adapted to the specified constraints
    pub fn adapt_enhanced_game_content(
        enhanced_game: &EnhancedGameDisplay,
        detail_level: DetailLevel,
        available_width: u16,
    ) -> AdaptedGameContent {
        let game_data = &enhanced_game.base_content;

        // Use extended team info if available and appropriate for detail level
        let (home_team, away_team) = if let (Some(extended_info), DetailLevel::Extended) =
            (&enhanced_game.extended_team_info, detail_level)
        {
            (
                extended_info.full_home_name.clone(),
                extended_info.full_away_name.clone(),
            )
        } else {
            (game_data.home_team.clone(), game_data.away_team.clone())
        };

        // Use detailed time info if available
        let time_display = if let Some(detailed_time) = &enhanced_game.detailed_time_info {
            match detail_level {
                DetailLevel::Extended => {
                    if let Some(duration) = &detailed_time.game_duration {
                        format!("{} ({})", detailed_time.precise_timestamp, duration)
                    } else {
                        detailed_time.precise_timestamp.clone()
                    }
                }
                DetailLevel::Standard => detailed_time.precise_timestamp.clone(),
                DetailLevel::Minimal => game_data.time.clone(),
            }
        } else {
            game_data.time.clone()
        };

        Self::adapt_game_content(
            &home_team,
            &away_team,
            &time_display,
            &game_data.result,
            &game_data.goal_events,
            detail_level,
            available_width,
        )
    }

    /// Calculates game duration from played time in seconds
    fn calculate_game_duration(played_time: i32) -> Option<String> {
        if played_time <= 0 {
            return None;
        }

        let minutes = played_time / 60;
        let seconds = played_time % 60;

        if minutes >= 60 {
            let hours = minutes / 60;
            let remaining_minutes = minutes % 60;
            Some(format!("{}:{:02}:{:02}", hours, remaining_minutes, seconds))
        } else {
            Some(format!("{}:{:02}", minutes, seconds))
        }
    }

    /// Determines period information from game data
    fn determine_period_info(game_data: &crate::teletext_ui::GameResultData) -> Option<String> {
        if game_data.is_shootout {
            Some("Ratkaisu".to_string())
        } else if game_data.is_overtime {
            Some("Jatkoaika".to_string())
        } else {
            match game_data.score_type {
                crate::teletext_ui::ScoreType::Final => Some("Päättynyt".to_string()),
                crate::teletext_ui::ScoreType::Ongoing => Some("Käynnissä".to_string()),
                crate::teletext_ui::ScoreType::Scheduled => Some("Tulossa".to_string()),
            }
        }
    }

    /// Creates expanded goal detail from goal event data
    fn create_expanded_goal_detail(
        event: &GoalEventData,
        detail_level: DetailLevel,
    ) -> ExpandedGoalDetail {
        let situation = if !event.goal_types.is_empty() {
            Some(Self::translate_goal_types(&event.goal_types))
        } else {
            None
        };

        ExpandedGoalDetail {
            scorer: event.scorer_name.clone(),
            assist1: None, // Would need additional data from API
            assist2: None, // Would need additional data from API
            time: format!("{}.", event.minute),
            situation,
        }
    }

    /// Translates goal types to Finnish descriptions
    fn translate_goal_types(goal_types: &[String]) -> String {
        let translations: Vec<String> = goal_types
            .iter()
            .map(|gt| match gt.as_str() {
                "YV" => "Ylivoimamaali".to_string(),
                "YV2" => "2 miehen ylivoima".to_string(),
                "IM" => "Irtomaalin".to_string(),
                "VT" => "Vajaamiehinen".to_string(),
                "RL" => "Rangaistuslyönti".to_string(),
                _ => gt.clone(),
            })
            .collect();

        translations.join(", ")
    }

    /// Adapts game content to the specified detail level and available width
    ///
    /// # Arguments
    /// * `home_team` - Home team name
    /// * `away_team` - Away team name
    /// * `time` - Game time string
    /// * `result` - Game result string
    /// * `goal_events` - Vector of goal events
    /// * `detail_level` - Target detail level for formatting
    /// * `available_width` - Available width for content display
    ///
    /// # Returns
    /// * `AdaptedGameContent` - Formatted content adapted to the specified constraints
    pub fn adapt_game_content(
        home_team: &str,
        away_team: &str,
        time: &str,
        result: &str,
        goal_events: &[GoalEventData],
        detail_level: DetailLevel,
        available_width: u16,
    ) -> AdaptedGameContent {
        let (formatted_home, formatted_away) =
            Self::format_team_names(home_team, away_team, detail_level, available_width);
        let formatted_time = Self::format_time_display(time, detail_level);
        let formatted_result = Self::format_result_display(result, detail_level);
        let goal_lines = Self::format_goal_events(goal_events, detail_level, available_width);

        // Calculate estimated height
        let base_height = 1; // Game result line
        let goal_height = goal_lines.len() as u16;
        let spacer_height = if goal_lines.is_empty() { 1 } else { 1 }; // Space between games
        let estimated_height = base_height + goal_height + spacer_height;

        AdaptedGameContent {
            home_team: formatted_home,
            away_team: formatted_away,
            time_display: formatted_time,
            result_display: formatted_result,
            goal_lines,
            estimated_height,
        }
    }

    /// Formats team names based on detail level and available width
    ///
    /// # Arguments
    /// * `home` - Home team name
    /// * `away` - Away team name
    /// * `detail_level` - Target detail level
    /// * `available_width` - Available width for display
    ///
    /// # Returns
    /// * `(String, String)` - Tuple of formatted (home, away) team names
    pub fn format_team_names(
        home: &str,
        away: &str,
        detail_level: DetailLevel,
        available_width: u16,
    ) -> (String, String) {
        let max_team_name_width = match detail_level {
            DetailLevel::Minimal => Self::calculate_minimal_team_width(available_width),
            DetailLevel::Standard => Self::calculate_standard_team_width(available_width),
            DetailLevel::Extended => Self::calculate_extended_team_width(available_width),
        };

        let formatted_home = Self::truncate_team_name(home, max_team_name_width);
        let formatted_away = Self::truncate_team_name(away, max_team_name_width);

        (formatted_home, formatted_away)
    }

    /// Formats time display based on detail level
    ///
    /// # Arguments
    /// * `time` - Original time string
    /// * `detail_level` - Target detail level
    ///
    /// # Returns
    /// * `String` - Formatted time display
    pub fn format_time_display(time: &str, detail_level: DetailLevel) -> String {
        match detail_level {
            DetailLevel::Minimal => {
                // Keep original format for minimal
                time.to_string()
            }
            DetailLevel::Standard => {
                // Add slight enhancement for standard
                if time.len() <= 5 {
                    format!("{:>6}", time) // Right-align with padding
                } else {
                    time.to_string()
                }
            }
            DetailLevel::Extended => {
                // Enhanced format for extended
                if time.contains(':') {
                    format!("{:>8}", time) // More padding for extended
                } else {
                    format!("{:>8}", time)
                }
            }
        }
    }

    /// Formats result display based on detail level
    ///
    /// # Arguments
    /// * `result` - Original result string
    /// * `detail_level` - Target detail level
    ///
    /// # Returns
    /// * `String` - Formatted result display
    pub fn format_result_display(result: &str, detail_level: DetailLevel) -> String {
        match detail_level {
            DetailLevel::Minimal => {
                // Keep original format
                result.to_string()
            }
            DetailLevel::Standard => {
                // Center-align result for standard
                format!("{:^7}", result)
            }
            DetailLevel::Extended => {
                // Enhanced result display for extended
                if result.contains('-') {
                    format!("{:^9}", result) // More space for extended
                } else {
                    format!("{:^9}", result)
                }
            }
        }
    }

    /// Formats goal events based on detail level and available width
    ///
    /// # Arguments
    /// * `events` - Vector of goal events
    /// * `detail_level` - Target detail level
    /// * `available_width` - Available width for display
    ///
    /// # Returns
    /// * `Vec<String>` - Vector of formatted goal event lines
    pub fn format_goal_events(
        events: &[GoalEventData],
        detail_level: DetailLevel,
        available_width: u16,
    ) -> Vec<String> {
        if events.is_empty() {
            return Vec::new();
        }

        match detail_level {
            DetailLevel::Minimal => Self::format_minimal_goal_events(events, available_width),
            DetailLevel::Standard => Self::format_standard_goal_events(events, available_width),
            DetailLevel::Extended => Self::format_extended_goal_events(events, available_width),
        }
    }

    /// Calculates maximum team name width for minimal detail level
    fn calculate_minimal_team_width(available_width: u16) -> usize {
        // Reserve space for time, result, and separators
        let reserved_space = 20; // Approximate space for " 18:30  2-1 "
        let remaining = available_width.saturating_sub(reserved_space);
        let team_space = remaining / 2; // Split between home and away
        std::cmp::max(8, std::cmp::min(15, team_space as usize)) // Min 8, max 15 chars
    }

    /// Calculates maximum team name width for standard detail level
    fn calculate_standard_team_width(available_width: u16) -> usize {
        let reserved_space = 25; // More space for enhanced formatting
        let remaining = available_width.saturating_sub(reserved_space);
        let team_space = remaining / 2;
        std::cmp::max(10, std::cmp::min(20, team_space as usize)) // Min 10, max 20 chars
    }

    /// Calculates maximum team name width for extended detail level
    fn calculate_extended_team_width(available_width: u16) -> usize {
        let reserved_space = 30; // Even more space for extended formatting
        let remaining = available_width.saturating_sub(reserved_space);
        let team_space = remaining / 2;
        std::cmp::max(12, std::cmp::min(25, team_space as usize)) // Min 12, max 25 chars
    }

    /// Truncates team name to fit within specified width
    fn truncate_team_name(name: &str, max_width: usize) -> String {
        if max_width == 0 {
            return String::new();
        }

        if name.chars().count() <= max_width {
            name.to_string()
        } else if max_width == 1 {
            "…".to_string()
        } else {
            let truncated: String = name.chars().take(max_width - 1).collect();
            format!("{}…", truncated)
        }
    }

    /// Formats goal events for minimal detail level
    fn format_minimal_goal_events(events: &[GoalEventData], available_width: u16) -> Vec<String> {
        let mut lines = Vec::new();
        let home_scorers: Vec<_> = events.iter().filter(|e| e.is_home_team).collect();
        let away_scorers: Vec<_> = events.iter().filter(|e| !e.is_home_team).collect();

        let max_lines = std::cmp::max(home_scorers.len(), away_scorers.len());
        let scorer_width = (available_width as usize).saturating_sub(10) / 2; // Reserve space for formatting

        for i in 0..max_lines {
            let home_scorer = home_scorers
                .get(i)
                .map(|e| Self::format_minimal_scorer(&e.scorer_name, e.minute, scorer_width))
                .unwrap_or_else(|| " ".repeat(scorer_width));

            let away_scorer = away_scorers
                .get(i)
                .map(|e| Self::format_minimal_scorer(&e.scorer_name, e.minute, scorer_width))
                .unwrap_or_else(|| " ".repeat(scorer_width));

            lines.push(format!(
                "{:<width$} {}",
                home_scorer,
                away_scorer,
                width = scorer_width
            ));
        }

        lines
    }

    /// Formats goal events for standard detail level
    fn format_standard_goal_events(events: &[GoalEventData], available_width: u16) -> Vec<String> {
        let mut lines = Vec::new();
        let home_scorers: Vec<_> = events.iter().filter(|e| e.is_home_team).collect();
        let away_scorers: Vec<_> = events.iter().filter(|e| !e.is_home_team).collect();

        let max_lines = std::cmp::max(home_scorers.len(), away_scorers.len());
        let scorer_width = (available_width as usize).saturating_sub(12) / 2; // More space for enhanced formatting

        for i in 0..max_lines {
            let home_scorer = home_scorers
                .get(i)
                .map(|e| {
                    Self::format_standard_scorer(
                        &e.scorer_name,
                        e.minute,
                        &e.goal_types,
                        scorer_width,
                    )
                })
                .unwrap_or_else(|| " ".repeat(scorer_width));

            let away_scorer = away_scorers
                .get(i)
                .map(|e| {
                    Self::format_standard_scorer(
                        &e.scorer_name,
                        e.minute,
                        &e.goal_types,
                        scorer_width,
                    )
                })
                .unwrap_or_else(|| " ".repeat(scorer_width));

            lines.push(format!(
                "{:<width$}  {}",
                home_scorer,
                away_scorer,
                width = scorer_width
            ));
        }

        lines
    }

    /// Formats goal events for extended detail level
    fn format_extended_goal_events(events: &[GoalEventData], available_width: u16) -> Vec<String> {
        let mut lines = Vec::new();
        let home_scorers: Vec<_> = events.iter().filter(|e| e.is_home_team).collect();
        let away_scorers: Vec<_> = events.iter().filter(|e| !e.is_home_team).collect();

        let max_lines = std::cmp::max(home_scorers.len(), away_scorers.len());
        let scorer_width = (available_width as usize).saturating_sub(15) / 2; // Maximum space for extended

        for i in 0..max_lines {
            let home_scorer = home_scorers
                .get(i)
                .map(|e| {
                    Self::format_extended_scorer(
                        &e.scorer_name,
                        e.minute,
                        &e.goal_types,
                        e.is_winning_goal,
                        scorer_width,
                    )
                })
                .unwrap_or_else(|| " ".repeat(scorer_width));

            let away_scorer = away_scorers
                .get(i)
                .map(|e| {
                    Self::format_extended_scorer(
                        &e.scorer_name,
                        e.minute,
                        &e.goal_types,
                        e.is_winning_goal,
                        scorer_width,
                    )
                })
                .unwrap_or_else(|| " ".repeat(scorer_width));

            lines.push(format!(
                "{:<width$}   {}",
                home_scorer,
                away_scorer,
                width = scorer_width
            ));
        }

        lines
    }

    /// Formats a single scorer for minimal detail level
    fn format_minimal_scorer(name: &str, minute: i32, max_width: usize) -> String {
        let time_str = format!("{}.", minute);
        let name_width = max_width.saturating_sub(time_str.len() + 1);
        let truncated_name = Self::truncate_text(name, name_width);
        format!("{} {}", truncated_name, time_str)
    }

    /// Formats a single scorer for standard detail level
    fn format_standard_scorer(
        name: &str,
        minute: i32,
        goal_types: &[String],
        max_width: usize,
    ) -> String {
        let time_str = format!("{}.", minute);
        let type_str = if goal_types.is_empty() {
            String::new()
        } else {
            format!(" ({})", goal_types.join(","))
        };

        let reserved = time_str.len() + type_str.len() + 1;
        let name_width = max_width.saturating_sub(reserved);
        let truncated_name = Self::truncate_text(name, name_width);

        format!("{} {}{}", truncated_name, time_str, type_str)
    }

    /// Formats a single scorer for extended detail level
    fn format_extended_scorer(
        name: &str,
        minute: i32,
        goal_types: &[String],
        is_winning: bool,
        max_width: usize,
    ) -> String {
        let time_str = format!("{}.", minute);
        let mut indicators = Vec::new();

        if !goal_types.is_empty() {
            indicators.push(goal_types.join(","));
        }
        if is_winning {
            indicators.push("VM".to_string()); // Voittomaalin merkki
        }

        let type_str = if indicators.is_empty() {
            String::new()
        } else {
            format!(" ({})", indicators.join(" "))
        };

        let reserved = time_str.len() + type_str.len() + 1;
        let name_width = max_width.saturating_sub(reserved);
        let truncated_name = Self::truncate_text(name, name_width);

        format!("{} {}{}", truncated_name, time_str, type_str)
    }

    /// Truncates text to fit within specified width, adding ellipsis if needed
    fn truncate_text(text: &str, max_width: usize) -> String {
        if max_width == 0 {
            return String::new();
        }

        if text.chars().count() <= max_width {
            text.to_string()
        } else if max_width == 1 {
            "…".to_string()
        } else {
            let truncated: String = text.chars().take(max_width - 1).collect();
            format!("{}…", truncated)
        }
    }

    /// Wraps text to fit within specified width, breaking at word boundaries when possible
    ///
    /// # Arguments
    /// * `text` - Text to wrap
    /// * `max_width` - Maximum width per line
    /// * `max_lines` - Maximum number of lines
    ///
    /// # Returns
    /// * `Vec<String>` - Vector of wrapped lines
    pub fn wrap_text(text: &str, max_width: usize, max_lines: usize) -> Vec<String> {
        if max_width == 0 || max_lines == 0 {
            return Vec::new();
        }

        let words: Vec<&str> = text.split_whitespace().collect();
        let mut lines = Vec::new();
        let mut current_line = String::new();

        for word in words {
            if lines.len() >= max_lines {
                break;
            }

            // If adding this word would exceed the width
            if !current_line.is_empty() && current_line.len() + 1 + word.len() > max_width {
                lines.push(current_line.clone());
                current_line.clear();

                if lines.len() >= max_lines {
                    break;
                }
            }

            // If the word itself is too long, truncate it
            if word.len() > max_width {
                let truncated_word = Self::truncate_text(word, max_width);
                if current_line.is_empty() {
                    lines.push(truncated_word);
                } else {
                    lines.push(current_line.clone());
                    if lines.len() < max_lines {
                        lines.push(truncated_word);
                    }
                    current_line.clear();
                }
            } else {
                if !current_line.is_empty() {
                    current_line.push(' ');
                }
                current_line.push_str(word);
            }
        }

        // Add the last line if it's not empty and we haven't exceeded max_lines
        if !current_line.is_empty() && lines.len() < max_lines {
            lines.push(current_line);
        }

        lines
    }

    /// Pads text to center it within the specified width
    ///
    /// # Arguments
    /// * `text` - Text to center
    /// * `width` - Target width
    ///
    /// # Returns
    /// * `String` - Centered text with padding
    pub fn center_text(text: &str, width: usize) -> String {
        let text_len = text.chars().count();
        if text_len >= width {
            return text.to_string();
        }

        let padding = width - text_len;
        let left_padding = padding / 2;
        let right_padding = padding - left_padding;

        format!(
            "{}{}{}",
            " ".repeat(left_padding),
            text,
            " ".repeat(right_padding)
        )
    }

    /// Right-aligns text within the specified width
    ///
    /// # Arguments
    /// * `text` - Text to align
    /// * `width` - Target width
    ///
    /// # Returns
    /// * `String` - Right-aligned text with padding
    pub fn right_align_text(text: &str, width: usize) -> String {
        let text_len = text.chars().count();
        if text_len >= width {
            return text.to_string();
        }

        let padding = width - text_len;
        format!("{}{}", " ".repeat(padding), text)
    }

    /// Left-aligns text within the specified width
    ///
    /// # Arguments
    /// * `text` - Text to align
    /// * `width` - Target width
    ///
    /// # Returns
    /// * `String` - Left-aligned text with padding
    pub fn left_align_text(text: &str, width: usize) -> String {
        let text_len = text.chars().count();
        if text_len >= width {
            return text.to_string();
        }

        let padding = width - text_len;
        format!("{}{}", text, " ".repeat(padding))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_goal_event(
        scorer_name: &str,
        minute: i32,
        is_home: bool,
        goal_types: Vec<String>,
        is_winning: bool,
    ) -> GoalEventData {
        GoalEventData {
            scorer_player_id: 123,
            scorer_name: scorer_name.to_string(),
            minute,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: is_winning,
            goal_types,
            is_home_team: is_home,
            video_clip_url: None,
        }
    }

    #[test]
    fn test_adapt_game_content_minimal() {
        let goal_events = vec![
            create_test_goal_event("Koivu", 15, true, vec![], false),
            create_test_goal_event("Selänne", 25, false, vec!["YV".to_string()], true),
        ];

        let content = ContentAdapter::adapt_game_content(
            "HIFK Helsinki",
            "Tappara Tampere",
            "18:30",
            "2-1",
            &goal_events,
            DetailLevel::Minimal,
            80,
        );

        assert!(!content.home_team.is_empty());
        assert!(!content.away_team.is_empty());
        assert_eq!(content.time_display, "18:30");
        assert_eq!(content.result_display, "2-1");
        assert_eq!(content.goal_lines.len(), 1); // One line for both scorers
        assert!(content.estimated_height > 0);
    }

    #[test]
    fn test_adapt_game_content_standard() {
        let goal_events = vec![create_test_goal_event(
            "Koivu",
            15,
            true,
            vec!["YV".to_string()],
            false,
        )];

        let content = ContentAdapter::adapt_game_content(
            "HIFK",
            "Tappara",
            "18:30",
            "1-0",
            &goal_events,
            DetailLevel::Standard,
            100,
        );

        assert_eq!(content.time_display, " 18:30"); // Right-aligned with padding
        assert_eq!(content.result_display, "  1-0  "); // Center-aligned
        assert!(!content.goal_lines.is_empty());
    }

    #[test]
    fn test_adapt_game_content_extended() {
        let goal_events = vec![create_test_goal_event(
            "Koivu",
            15,
            true,
            vec!["YV".to_string()],
            true,
        )];

        let content = ContentAdapter::adapt_game_content(
            "HIFK Helsinki",
            "Tappara Tampere",
            "18:30",
            "1-0",
            &goal_events,
            DetailLevel::Extended,
            120,
        );

        assert_eq!(content.time_display, "   18:30"); // Extended padding
        assert_eq!(content.result_display, "   1-0   "); // Extended center-align
        assert!(!content.goal_lines.is_empty());
        // Should include winning goal indicator
        assert!(content.goal_lines[0].contains("VM") || content.goal_lines[0].contains("YV"));
    }

    #[test]
    fn test_format_team_names_truncation() {
        let (home, away) = ContentAdapter::format_team_names(
            "Very Long Team Name That Should Be Truncated",
            "Another Very Long Team Name",
            DetailLevel::Minimal,
            80,
        );

        // Calculate expected max width for minimal level with 80 width
        let expected_max_width = ContentAdapter::calculate_minimal_team_width(80);

        // Should be truncated with ellipsis
        assert!(home.chars().count() <= expected_max_width);
        assert!(away.chars().count() <= expected_max_width);
        assert!(home.ends_with('…') || home.chars().count() < expected_max_width);
        assert!(away.ends_with('…') || away.chars().count() < expected_max_width);
    }

    #[test]
    fn test_format_team_names_no_truncation() {
        let (home, away) =
            ContentAdapter::format_team_names("HIFK", "TPS", DetailLevel::Extended, 120);

        assert_eq!(home, "HIFK");
        assert_eq!(away, "TPS");
    }

    #[test]
    fn test_format_time_display_levels() {
        let time = "18:30";

        let minimal = ContentAdapter::format_time_display(time, DetailLevel::Minimal);
        let standard = ContentAdapter::format_time_display(time, DetailLevel::Standard);
        let extended = ContentAdapter::format_time_display(time, DetailLevel::Extended);

        assert_eq!(minimal, "18:30");
        assert_eq!(standard, " 18:30"); // Right-aligned with padding
        assert_eq!(extended, "   18:30"); // More padding for extended
    }

    #[test]
    fn test_format_result_display_levels() {
        let result = "2-1";

        let minimal = ContentAdapter::format_result_display(result, DetailLevel::Minimal);
        let standard = ContentAdapter::format_result_display(result, DetailLevel::Standard);
        let extended = ContentAdapter::format_result_display(result, DetailLevel::Extended);

        assert_eq!(minimal, "2-1");
        assert_eq!(standard, "  2-1  "); // Center-aligned
        assert_eq!(extended, "   2-1   "); // More space for extended
    }

    #[test]
    fn test_format_goal_events_empty() {
        let events = vec![];
        let lines = ContentAdapter::format_goal_events(&events, DetailLevel::Minimal, 80);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_format_goal_events_single_home_scorer() {
        let events = vec![create_test_goal_event("Koivu", 15, true, vec![], false)];

        let lines = ContentAdapter::format_goal_events(&events, DetailLevel::Minimal, 80);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("Koivu"));
        assert!(lines[0].contains("15."));
    }

    #[test]
    fn test_format_goal_events_both_teams() {
        let events = vec![
            create_test_goal_event("Koivu", 15, true, vec![], false),
            create_test_goal_event("Selänne", 25, false, vec!["YV".to_string()], false),
        ];

        let lines = ContentAdapter::format_goal_events(&events, DetailLevel::Standard, 100);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("Koivu"));
        assert!(lines[0].contains("Selänne"));
        assert!(lines[0].contains("15."));
        assert!(lines[0].contains("25."));
        assert!(lines[0].contains("YV"));
    }

    #[test]
    fn test_format_goal_events_extended_with_winning_goal() {
        let events = vec![create_test_goal_event(
            "Koivu",
            15,
            true,
            vec!["YV".to_string()],
            true,
        )];

        let lines = ContentAdapter::format_goal_events(&events, DetailLevel::Extended, 120);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("Koivu"));
        assert!(lines[0].contains("15."));
        assert!(lines[0].contains("YV"));
        assert!(lines[0].contains("VM")); // Winning goal indicator
    }

    #[test]
    fn test_calculate_team_width_levels() {
        let minimal_width = ContentAdapter::calculate_minimal_team_width(80);
        let standard_width = ContentAdapter::calculate_standard_team_width(100);
        let extended_width = ContentAdapter::calculate_extended_team_width(120);

        // Extended should allow longer names than standard, standard longer than minimal
        assert!(extended_width >= standard_width);
        assert!(standard_width >= minimal_width);

        // All should be within reasonable bounds
        assert!(minimal_width >= 8);
        assert!(standard_width >= 10);
        assert!(extended_width >= 12);
    }

    #[test]
    fn test_truncate_team_name() {
        let long_name = "Very Long Team Name";
        let truncated = ContentAdapter::truncate_team_name(long_name, 10);

        assert!(truncated.chars().count() <= 10);
        assert!(truncated.ends_with('…'));

        let short_name = "HIFK";
        let not_truncated = ContentAdapter::truncate_team_name(short_name, 10);
        assert_eq!(not_truncated, "HIFK");
    }

    #[test]
    fn test_truncate_text() {
        let text = "Long text that needs truncation";
        let truncated = ContentAdapter::truncate_text(text, 10);

        assert!(truncated.chars().count() <= 10);
        assert!(truncated.ends_with('…'));

        let short_text = "Short";
        let not_truncated = ContentAdapter::truncate_text(short_text, 10);
        assert_eq!(not_truncated, "Short");

        // Test edge case with zero width
        let empty = ContentAdapter::truncate_text("test", 0);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_format_minimal_scorer() {
        let formatted = ContentAdapter::format_minimal_scorer("Koivu", 15, 20);
        assert!(formatted.contains("Koivu"));
        assert!(formatted.contains("15."));
        assert!(formatted.len() <= 20);
    }

    #[test]
    fn test_format_standard_scorer() {
        let goal_types = vec!["YV".to_string()];
        let formatted = ContentAdapter::format_standard_scorer("Koivu", 15, &goal_types, 25);

        assert!(formatted.contains("Koivu"));
        assert!(formatted.contains("15."));
        assert!(formatted.contains("YV"));
        assert!(formatted.len() <= 25);
    }

    #[test]
    fn test_format_extended_scorer() {
        let goal_types = vec!["YV".to_string()];
        let formatted = ContentAdapter::format_extended_scorer("Koivu", 15, &goal_types, true, 30);

        assert!(formatted.contains("Koivu"));
        assert!(formatted.contains("15."));
        assert!(formatted.contains("YV"));
        assert!(formatted.contains("VM")); // Winning goal indicator
        assert!(formatted.len() <= 30);
    }

    #[test]
    fn test_multiple_goal_events_formatting() {
        let events = vec![
            create_test_goal_event("Koivu", 15, true, vec![], false),
            create_test_goal_event("Kurri", 25, true, vec!["YV".to_string()], false),
            create_test_goal_event("Selänne", 35, false, vec![], true),
        ];

        let lines = ContentAdapter::format_goal_events(&events, DetailLevel::Standard, 100);

        // Should have 2 lines (2 home scorers, 1 away scorer)
        assert_eq!(lines.len(), 2);

        // First line should have both home scorers
        assert!(lines[0].contains("Koivu"));
        assert!(lines[1].contains("Kurri"));

        // Away scorer should be on first line
        assert!(lines[0].contains("Selänne"));
    }

    #[test]
    fn test_width_constraints() {
        // Test with very narrow width
        let content = ContentAdapter::adapt_game_content(
            "Very Long Team Name",
            "Another Long Name",
            "18:30",
            "2-1",
            &[],
            DetailLevel::Minimal,
            40, // Very narrow
        );

        // Should still produce valid content
        assert!(!content.home_team.is_empty());
        assert!(!content.away_team.is_empty());
        assert!(content.home_team.len() <= 15); // Should be truncated
        assert!(content.away_team.len() <= 15);
    }

    #[test]
    fn test_estimated_height_calculation() {
        let no_goals = ContentAdapter::adapt_game_content(
            "HIFK",
            "TPS",
            "18:30",
            "0-0",
            &[],
            DetailLevel::Minimal,
            80,
        );
        assert_eq!(no_goals.estimated_height, 2); // Game line + spacer

        let with_goals = vec![
            create_test_goal_event("Koivu", 15, true, vec![], false),
            create_test_goal_event("Selänne", 25, false, vec![], false),
        ];
        let content_with_goals = ContentAdapter::adapt_game_content(
            "HIFK",
            "TPS",
            "18:30",
            "1-1",
            &with_goals,
            DetailLevel::Minimal,
            80,
        );
        assert!(content_with_goals.estimated_height > no_goals.estimated_height);
    }

    #[test]
    fn test_create_enhanced_game_display() {
        use crate::teletext_ui::{GameResultData, ScoreType};

        let goal_events = vec![create_test_goal_event(
            "Koivu",
            15,
            true,
            vec!["YV".to_string()],
            false,
        )];

        let game_data = GameResultData {
            home_team: "HIFK Helsinki".to_string(),
            away_team: "Tappara Tampere".to_string(),
            time: "18:30".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            goal_events,
            played_time: 3600,
        };

        let enhanced =
            ContentAdapter::create_enhanced_game_display(game_data, DetailLevel::Extended);

        assert!(enhanced.extended_team_info.is_some());
        assert!(enhanced.detailed_time_info.is_some());
        assert_eq!(enhanced.expanded_goal_details.len(), 1);

        let team_info = enhanced.extended_team_info.unwrap();
        assert_eq!(team_info.full_home_name, "HIFK Helsinki");
        assert_eq!(team_info.full_away_name, "Tappara Tampere");
    }

    #[test]
    fn test_create_enhanced_game_display_minimal() {
        use crate::teletext_ui::{GameResultData, ScoreType};

        let game_data = GameResultData {
            home_team: "HIFK".to_string(),
            away_team: "TPS".to_string(),
            time: "18:30".to_string(),
            result: "0-0".to_string(),
            score_type: ScoreType::Ongoing,
            is_overtime: false,
            is_shootout: false,
            goal_events: vec![],
            played_time: 1800,
        };

        let enhanced =
            ContentAdapter::create_enhanced_game_display(game_data, DetailLevel::Minimal);

        assert!(enhanced.extended_team_info.is_none());
        assert!(enhanced.detailed_time_info.is_none());
        assert!(enhanced.expanded_goal_details.is_empty());
    }

    #[test]
    fn test_calculate_game_duration() {
        assert_eq!(ContentAdapter::calculate_game_duration(0), None);
        assert_eq!(ContentAdapter::calculate_game_duration(-10), None);
        assert_eq!(
            ContentAdapter::calculate_game_duration(90),
            Some("1:30".to_string())
        );
        assert_eq!(
            ContentAdapter::calculate_game_duration(3665),
            Some("1:01:05".to_string())
        );
        assert_eq!(
            ContentAdapter::calculate_game_duration(45),
            Some("0:45".to_string())
        );
    }

    #[test]
    fn test_determine_period_info() {
        use crate::teletext_ui::{GameResultData, ScoreType};

        let mut game_data = GameResultData {
            home_team: "HIFK".to_string(),
            away_team: "TPS".to_string(),
            time: "18:30".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            goal_events: vec![],
            played_time: 3600,
        };

        assert_eq!(
            ContentAdapter::determine_period_info(&game_data),
            Some("Päättynyt".to_string())
        );

        game_data.is_overtime = true;
        assert_eq!(
            ContentAdapter::determine_period_info(&game_data),
            Some("Jatkoaika".to_string())
        );

        game_data.is_shootout = true;
        assert_eq!(
            ContentAdapter::determine_period_info(&game_data),
            Some("Ratkaisu".to_string())
        );

        game_data.is_overtime = false;
        game_data.is_shootout = false;
        game_data.score_type = ScoreType::Ongoing;
        assert_eq!(
            ContentAdapter::determine_period_info(&game_data),
            Some("Käynnissä".to_string())
        );
    }

    #[test]
    fn test_translate_goal_types() {
        assert_eq!(
            ContentAdapter::translate_goal_types(&["YV".to_string()]),
            "Ylivoimamaali"
        );
        assert_eq!(
            ContentAdapter::translate_goal_types(&["YV2".to_string()]),
            "2 miehen ylivoima"
        );
        assert_eq!(
            ContentAdapter::translate_goal_types(&["IM".to_string()]),
            "Irtomaalin"
        );
        assert_eq!(
            ContentAdapter::translate_goal_types(&["VT".to_string()]),
            "Vajaamiehinen"
        );
        assert_eq!(
            ContentAdapter::translate_goal_types(&["RL".to_string()]),
            "Rangaistuslyönti"
        );

        let multiple = vec!["YV".to_string(), "IM".to_string()];
        assert_eq!(
            ContentAdapter::translate_goal_types(&multiple),
            "Ylivoimamaali, Irtomaalin"
        );

        assert_eq!(
            ContentAdapter::translate_goal_types(&["UNKNOWN".to_string()]),
            "UNKNOWN"
        );
    }

    #[test]
    fn test_wrap_text() {
        let text = "This is a long text that needs to be wrapped";
        let wrapped = ContentAdapter::wrap_text(text, 10, 3);

        assert!(wrapped.len() <= 3);
        for line in &wrapped {
            assert!(line.len() <= 10);
        }

        // Test with very long word
        let long_word = "Supercalifragilisticexpialidocious";
        let wrapped_long = ContentAdapter::wrap_text(long_word, 10, 2);
        assert!(wrapped_long.len() <= 2);
        assert!(wrapped_long[0].ends_with('…'));
    }

    #[test]
    fn test_text_alignment() {
        let text = "Test";

        let centered = ContentAdapter::center_text(text, 10);
        assert_eq!(centered, "   Test   ");

        let right_aligned = ContentAdapter::right_align_text(text, 10);
        assert_eq!(right_aligned, "      Test");

        let left_aligned = ContentAdapter::left_align_text(text, 10);
        assert_eq!(left_aligned, "Test      ");

        // Test with text longer than width
        let long_text = "Very long text";
        assert_eq!(ContentAdapter::center_text(long_text, 5), long_text);
        assert_eq!(ContentAdapter::right_align_text(long_text, 5), long_text);
        assert_eq!(ContentAdapter::left_align_text(long_text, 5), long_text);
    }

    #[test]
    fn test_adapt_enhanced_game_content() {
        use crate::teletext_ui::{GameResultData, ScoreType};

        let goal_events = vec![create_test_goal_event(
            "Koivu",
            15,
            true,
            vec!["YV".to_string()],
            false,
        )];

        let game_data = GameResultData {
            home_team: "HIFK Helsinki".to_string(),
            away_team: "Tappara Tampere".to_string(),
            time: "18:30".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            goal_events,
            played_time: 3600,
        };

        let enhanced =
            ContentAdapter::create_enhanced_game_display(game_data, DetailLevel::Extended);
        let adapted =
            ContentAdapter::adapt_enhanced_game_content(&enhanced, DetailLevel::Extended, 120);

        assert_eq!(adapted.home_team, "HIFK Helsinki");
        assert_eq!(adapted.away_team, "Tappara Tampere");
        assert!(adapted.time_display.contains("18:30"));
        assert!(!adapted.goal_lines.is_empty());
    }
}
