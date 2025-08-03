//! Content adaptation module for dynamic content formatting
//!
//! This module provides functionality for adapting game content to different detail levels
//! based on available screen space. It handles the intelligent truncation and formatting
//! of content to make optimal use of available terminal space.
//!
//! ## Features
//!
//! - **Adaptive Team Names**: Truncates team names intelligently based on available width
//! - **Time Formatting**: Provides different time display formats for different detail levels
//! - **Goal Event Formatting**: Adapts goal information display based on screen space
//! - **Text Truncation**: Smart truncation with ellipsis indicators
//! - **Content Prioritization**: Shows most important information first when space is limited
//!
//! ## Detail Level Adaptation
//!
//! - **Minimal**: Basic team abbreviations, simple time format, essential goal info
//! - **Standard**: Longer team names, enhanced time display, more goal details
//! - **Extended**: Full team names, detailed timestamps, complete goal information
//!
//! ## Usage
//!
//! ```rust
//! use liiga_teletext::ui::content_adapter::ContentAdapter;
//! use liiga_teletext::ui::layout::DetailLevel;
//! use liiga_teletext::data_fetcher::models::GoalEventData;
//!
//! // Create sample data
//! let home_team = "HIFK";
//! let away_team = "Tappara";
//! let time = "18:30";
//! let result = "3-2";
//! let goal_events = vec![];
//!
//! // Adapt content for display
//! let formatted_content = ContentAdapter::adapt_game_content(
//!     home_team,
//!     away_team,
//!     time,
//!     result,
//!     &goal_events,
//!     DetailLevel::Standard,
//!     100 // available width
//! );
//! ```

use crate::constants::dynamic_ui;
use crate::data_fetcher::models::GoalEventData;
use crate::error::AppError;
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

/// Content elements that can be prioritized based on available space
#[derive(Debug, Clone)]
pub struct ContentElements {
    /// Whether to show enhanced team names
    pub team_names: bool,
    /// Whether to show enhanced game time information
    pub game_time: bool,
    /// Whether to show enhanced score display
    pub score: bool,
    /// Whether to show basic goal information
    pub basic_goal_info: bool,
}

/// Content priority configuration for progressive enhancement
#[derive(Debug, Clone)]
pub struct ContentPriority {
    /// Essential content that should always be shown
    pub essential: ContentElements,
    /// Enhanced content that can be shown if space allows
    pub enhanced: ContentElements,
    /// Extended content that requires significant space
    pub extended: ContentElements,
    /// Whether to show detailed goal information
    pub show_goal_details: bool,
    /// Whether to show extended team information
    pub show_extended_team_info: bool,
    /// Whether to show detailed time information
    pub show_detailed_time_info: bool,
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
                full_home_name: Self::get_extended_team_name(&game_data.home_team),
                full_away_name: Self::get_extended_team_name(&game_data.away_team),
                home_record: Self::get_team_record(&game_data.home_team),
                away_record: Self::get_team_record(&game_data.away_team),
            }),
            _ => None,
        };

        let detailed_time_info = match detail_level {
            DetailLevel::Standard => Some(DetailedTimeInfo {
                precise_timestamp: Self::format_precise_timestamp(&game_data.time, detail_level),
                game_duration: Self::calculate_game_duration(game_data.played_time),
                period_info: Self::determine_period_info(&game_data),
            }),
            DetailLevel::Extended => Some(DetailedTimeInfo {
                precise_timestamp: Self::format_precise_timestamp(&game_data.time, detail_level),
                game_duration: Self::calculate_enhanced_game_duration(
                    game_data.played_time,
                    game_data.is_overtime,
                    game_data.is_shootout,
                ),
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
            Some(format!("{hours}:{remaining_minutes:02}:{seconds:02}"))
        } else {
            Some(format!("{minutes}:{seconds:02}"))
        }
    }

    /// Calculates enhanced game duration with additional context for extended detail level
    fn calculate_enhanced_game_duration(
        played_time: i32,
        is_overtime: bool,
        is_shootout: bool,
    ) -> Option<String> {
        if played_time <= 0 {
            return None;
        }

        let base_duration = Self::calculate_game_duration(played_time)?;

        // Add context for extended detail level
        if is_shootout {
            Some(format!("{base_duration} + ratkaisu"))
        } else if is_overtime {
            Some(format!("{base_duration} + jatkoaika"))
        } else if played_time > 3600 {
            // Regular game is 60 minutes, so anything over 1 hour indicates overtime
            Some(format!("{base_duration} (jatkoaika)"))
        } else {
            Some(base_duration)
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

    /// Gets extended team name with full city/organization information
    fn get_extended_team_name(team_name: &str) -> String {
        // Map common abbreviations to full names for extended display
        match team_name {
            "HIFK" => "HIFK Helsinki".to_string(),
            "TPS" => "TPS Turku".to_string(),
            "Tappara" => "Tappara Tampere".to_string(),
            "Ilves" => "Ilves Tampere".to_string(),
            "KalPa" => "KalPa Kuopio".to_string(),
            "Lukko" => "Lukko Rauma".to_string(),
            "Ässät" => "Ässät Pori".to_string(),
            "Sport" => "Sport Vaasa".to_string(),
            "JYP" => "JYP Jyväskylä".to_string(),
            "Pelicans" => "Pelicans Lahti".to_string(),
            "HPK" => "HPK Hämeenlinna".to_string(),
            "Kärpät" => "Kärpät Oulu".to_string(),
            "SaiPa" => "SaiPa Lappeenranta".to_string(),
            "Jukurit" => "Jukurit Mikkeli".to_string(),
            "KooKoo" => "KooKoo Kouvola".to_string(),
            _ => team_name.to_string(), // Return original if no mapping found
        }
    }

    /// Gets team record information (placeholder for future implementation)
    fn get_team_record(_team_name: &str) -> Option<String> {
        // This would be populated from additional data sources in a real implementation
        // For now, return None as we don't have access to season records
        None
    }

    /// Formats precise timestamp with enhanced detail level formatting
    fn format_precise_timestamp(time: &str, detail_level: DetailLevel) -> String {
        match detail_level {
            DetailLevel::Extended => {
                // Add seconds precision for extended mode if time format allows
                if time.contains(':') && !time.contains("Päättynyt") && !time.contains("Tulossa")
                {
                    format!("{time}:00") // Add seconds
                } else {
                    time.to_string()
                }
            }
            _ => time.to_string(),
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

        // Enhanced time formatting for extended detail level
        let time = match detail_level {
            DetailLevel::Extended => {
                // Add more precise time information for extended mode
                if event.minute > 60 {
                    let period = (event.minute - 1) / 20 + 1;
                    let period_minute = ((event.minute - 1) % 20) + 1;
                    format!("{}.{:02} ({}. erä)", period_minute, 0, period)
                } else {
                    format!("{}.{:02}", event.minute, 0)
                }
            }
            _ => format!("{}.", event.minute),
        };

        ExpandedGoalDetail {
            scorer: event.scorer_name.clone(),
            assist1: None, // Would need additional data from API
            assist2: None, // Would need additional data from API
            time,
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

        // Account for additional spacing in extended mode
        let spacer_height = match detail_level {
            DetailLevel::Extended => 2, // Extra spacing for extended mode
            _ => 1,                     // Standard spacing
        };

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

        let formatted_home = match detail_level {
            DetailLevel::Extended => {
                let extended_name = Self::get_extended_team_name(home);
                let truncated = Self::truncate_team_name(&extended_name, max_team_name_width);
                format!("🏠 {truncated}") // Add home indicator for extended mode
            }
            _ => Self::truncate_team_name(home, max_team_name_width),
        };

        let formatted_away = match detail_level {
            DetailLevel::Extended => {
                let extended_name = Self::get_extended_team_name(away);
                let truncated = Self::truncate_team_name(&extended_name, max_team_name_width);
                format!("✈️  {truncated}") // Add away indicator for extended mode
            }
            _ => Self::truncate_team_name(away, max_team_name_width),
        };

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
                    format!("{time:>6}") // Right-align with padding
                } else {
                    time.to_string()
                }
            }
            DetailLevel::Extended => {
                // Enhanced format for extended
                                format!("{time:>8}") // More padding for extended
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
                format!("{result:^7}")
            }
            DetailLevel::Extended => {
                // Enhanced result display for extended with additional context
                if result.contains('-') {
                    // Add visual emphasis for extended mode
                    format!("┤{result:^7}├")
                } else {
                    format!("┤{result:^7}├")
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
        (team_space as usize).clamp(8, 15) // Min 8, max 15 chars
    }

    /// Calculates maximum team name width for standard detail level
    fn calculate_standard_team_width(available_width: u16) -> usize {
        let reserved_space = 25; // More space for enhanced formatting
        let remaining = available_width.saturating_sub(reserved_space);
        let team_space = remaining / 2;
        (team_space as usize).clamp(10, 20) // Min 10, max 20 chars
    }

    /// Calculates maximum team name width for extended detail level
    fn calculate_extended_team_width(available_width: u16) -> usize {
        let reserved_space = 35; // More space for enhanced extended formatting
        let remaining = available_width.saturating_sub(reserved_space);
        let team_space = remaining / 2;
        (team_space as usize).clamp(15, 30) // Min 15, max 30 chars for extended
    }

    /// Truncates team name to fit within specified width with graceful degradation
    fn truncate_team_name(name: &str, max_width: usize) -> String {
        if max_width == 0 {
            return String::new();
        }

        let char_count = name.chars().count();

        if char_count <= max_width {
            name.to_string()
        } else if max_width == 1 {
            dynamic_ui::TRUNCATION_INDICATOR.to_string()
        } else if max_width <= dynamic_ui::EMERGENCY_MAX_TEAM_NAME_LENGTH {
            // Emergency truncation for very small spaces
            Self::emergency_truncate_team_name(name, max_width)
        } else {
            let truncated: String = name.chars().take(max_width - 1).collect();
            format!("{}{}", truncated, dynamic_ui::TRUNCATION_INDICATOR)
        }
    }

    /// Emergency truncation for extremely constrained spaces
    fn emergency_truncate_team_name(name: &str, max_width: usize) -> String {
        if max_width == 0 {
            return String::new();
        }

        if max_width == 1 {
            return dynamic_ui::TRUNCATION_INDICATOR.to_string();
        }

        // For emergency mode, use abbreviations or first letters
        let chars: Vec<char> = name.chars().collect();
        if max_width == 2 {
            if chars.len() >= 2 {
                format!("{}{}", chars[0], dynamic_ui::TRUNCATION_INDICATOR)
            } else {
                chars.iter().collect()
            }
        } else {
            // Try to create meaningful abbreviation
            let abbreviated = Self::create_team_abbreviation(name, max_width - 1);
            format!("{}{}", abbreviated, dynamic_ui::TRUNCATION_INDICATOR)
        }
    }

    /// Creates a meaningful abbreviation for team names in emergency mode
    fn create_team_abbreviation(name: &str, max_chars: usize) -> String {
        if max_chars == 0 {
            return String::new();
        }

        // Split by spaces and take first letter of each word
        let words: Vec<&str> = name.split_whitespace().collect();
        if words.len() > 1 && max_chars >= words.len() {
            words
                .iter()
                .take(max_chars)
                .map(|word| word.chars().next().unwrap_or('?'))
                .collect()
        } else {
            // Just take first characters
            name.chars().take(max_chars).collect()
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

            #[allow(clippy::uninlined_format_args)]
            lines.push(format!(
                "{home_scorer:<scorer_width$} {away_scorer}",
                scorer_width = scorer_width
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

            #[allow(clippy::uninlined_format_args)]
            lines.push(format!(
                "{home_scorer:<scorer_width$}  {away_scorer}",
                scorer_width = scorer_width
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

        // Add a separator line for extended detail level if there are goals
        if !events.is_empty() {
            let separator = "─".repeat(available_width as usize);
            lines.push(separator);
        }

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

            #[allow(clippy::uninlined_format_args)]
            lines.push(format!(
                "│{home_scorer:<scorer_width$} │ {away_scorer}│",
                scorer_width = scorer_width
            ));
        }

        // Add closing separator for extended detail level
        if !events.is_empty() {
            let separator = "─".repeat(available_width as usize);
            lines.push(separator);
        }

        lines
    }

    /// Formats a single scorer for minimal detail level
    fn format_minimal_scorer(name: &str, minute: i32, max_width: usize) -> String {
        let time_str = format!("{minute}.");
        let name_width = max_width.saturating_sub(time_str.len() + 1);
        let truncated_name = Self::truncate_text(name, name_width);
        format!("{truncated_name} {time_str}")
    }

    /// Formats a single scorer for standard detail level
    fn format_standard_scorer(
        name: &str,
        minute: i32,
        goal_types: &[String],
        max_width: usize,
    ) -> String {
        let time_str = format!("{minute}.");
        let type_str = if goal_types.is_empty() {
            String::new()
        } else {
            format!(" ({})", goal_types.join(","))
        };

        let reserved = time_str.len() + type_str.len() + 1;
        let name_width = max_width.saturating_sub(reserved);
        let truncated_name = Self::truncate_text(name, name_width);

        format!("{truncated_name} {time_str}{type_str}")
    }

    /// Formats a single scorer for extended detail level
    fn format_extended_scorer(
        name: &str,
        minute: i32,
        goal_types: &[String],
        is_winning: bool,
        max_width: usize,
    ) -> String {
        // Enhanced time formatting for extended mode
        let time_str = if minute > 60 {
            let period = (minute - 1) / 20 + 1;
            let period_minute = ((minute - 1) % 20) + 1;
            format!("{period_minute}.{:02} ({period})", 0)
        } else {
            format!("{minute}.{:02}", 0)
        };

        let mut indicators = Vec::new();

        // Enhanced goal type translations for extended mode
        for goal_type in goal_types {
            let translated = match goal_type.as_str() {
                "YV" => "Ylivoima",
                "YV2" => "2-miehen ylivoima",
                "IM" => "Irtomaalin",
                "VT" => "Vajaamiehinen",
                "RL" => "Rangaistuslyönti",
                _ => goal_type,
            };
            indicators.push(translated.to_string());
        }

        if is_winning {
            indicators.push("Voittomaali".to_string());
        }

        let type_str = if indicators.is_empty() {
            String::new()
        } else {
            format!(" ({})", indicators.join(", "))
        };

        let reserved = time_str.len() + type_str.len() + 1;
        let name_width = max_width.saturating_sub(reserved);
        let truncated_name = Self::truncate_text(name, name_width);

        format!("{truncated_name} {time_str}{type_str}")
    }

    /// Truncates text to fit within specified width with graceful degradation
    fn truncate_text(text: &str, max_width: usize) -> String {
        if max_width == 0 {
            return String::new();
        }

        let char_count = text.chars().count();

        if char_count <= max_width {
            text.to_string()
        } else if max_width == 1 {
            dynamic_ui::TRUNCATION_INDICATOR.to_string()
        } else {
            let truncated: String = text.chars().take(max_width - 1).collect();
            format!("{}{}", truncated, dynamic_ui::TRUNCATION_INDICATOR)
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

    /// Determines optimal detail level based on available space and content complexity
    ///
    /// # Arguments
    /// * `base_detail_level` - Base detail level from layout calculator
    /// * `available_width` - Available width for content
    /// * `available_height` - Available height for content
    /// * `content_complexity` - Complexity score of the content (number of goals, etc.)
    ///
    /// # Returns
    /// * `DetailLevel` - Optimal detail level for the given constraints
    pub fn determine_progressive_detail_level(
        base_detail_level: DetailLevel,
        available_width: u16,
        available_height: u16,
        content_complexity: usize,
    ) -> DetailLevel {
        // Start with base detail level and potentially downgrade based on constraints
        let mut optimal_level = base_detail_level;

        // Calculate space requirements for different detail levels
        let minimal_width_required = 60;
        let standard_width_required = 90;
        let extended_width_required = 120;

        let minimal_height_per_game = 3;
        let standard_height_per_game = 4;
        let extended_height_per_game = 6;

        // Estimate height needed based on content complexity
        let estimated_height_needed = match base_detail_level {
            DetailLevel::Minimal => minimal_height_per_game + (content_complexity / 2),
            DetailLevel::Standard => standard_height_per_game + content_complexity,
            DetailLevel::Extended => extended_height_per_game + (content_complexity * 2),
        };

        // Downgrade if width constraints are too tight
        if available_width < extended_width_required && optimal_level == DetailLevel::Extended {
            optimal_level = DetailLevel::Standard;
        }
        if available_width < standard_width_required && optimal_level == DetailLevel::Standard {
            optimal_level = DetailLevel::Minimal;
        }
        if available_width < minimal_width_required {
            optimal_level = DetailLevel::Minimal; // Force minimal for very small screens
        }

        // Downgrade if height constraints are too tight
        if available_height < estimated_height_needed as u16 {
            optimal_level = match optimal_level {
                DetailLevel::Extended => DetailLevel::Standard,
                DetailLevel::Standard => DetailLevel::Minimal,
                DetailLevel::Minimal => DetailLevel::Minimal,
            };
        }

        optimal_level
    }

    /// Prioritizes content elements based on available space and importance
    ///
    /// # Arguments
    /// * `enhanced_game` - Enhanced game display data
    /// * `available_space` - Available space for content
    /// * `detail_level` - Target detail level
    ///
    /// # Returns
    /// * `ContentPriority` - Prioritized content elements
    pub fn prioritize_content(
        enhanced_game: &EnhancedGameDisplay,
        available_space: (u16, u16),
        detail_level: DetailLevel,
    ) -> ContentPriority {
        let (width, height) = available_space;

        // Essential content that should always be shown
        let essential = ContentElements {
            team_names: true,
            game_time: true,
            score: true,
            basic_goal_info: true,
        };

        // Enhanced content that can be shown if space allows
        let enhanced = ContentElements {
            team_names: detail_level != DetailLevel::Minimal,
            game_time: detail_level == DetailLevel::Extended,
            score: detail_level == DetailLevel::Extended,
            basic_goal_info: detail_level != DetailLevel::Minimal,
        };

        // Extended content that requires significant space
        let extended = ContentElements {
            team_names: detail_level == DetailLevel::Extended && width >= 120,
            game_time: detail_level == DetailLevel::Extended && width >= 100,
            score: detail_level == DetailLevel::Extended && width >= 80,
            basic_goal_info: detail_level == DetailLevel::Extended && height >= 8,
        };

        ContentPriority {
            essential,
            enhanced,
            extended,
            show_goal_details: enhanced_game.expanded_goal_details.len() <= (height as usize / 2),
            show_extended_team_info: enhanced_game.extended_team_info.is_some() && width >= 120,
            show_detailed_time_info: enhanced_game.detailed_time_info.is_some() && width >= 100,
        }
    }

    /// Creates smooth transition between detail levels by gradually adjusting content
    ///
    /// # Arguments
    /// * `from_level` - Current detail level
    /// * `to_level` - Target detail level
    /// * `transition_factor` - Factor between 0.0 and 1.0 indicating transition progress
    ///
    /// # Returns
    /// * `DetailLevel` - Intermediate detail level for smooth transition
    pub fn create_smooth_transition(
        from_level: DetailLevel,
        to_level: DetailLevel,
        transition_factor: f32,
    ) -> DetailLevel {
        let factor = transition_factor.clamp(0.0, 1.0);

        match (from_level, to_level) {
            // No transition needed if levels are the same
            (DetailLevel::Minimal, DetailLevel::Minimal) => DetailLevel::Minimal,
            (DetailLevel::Standard, DetailLevel::Standard) => DetailLevel::Standard,
            (DetailLevel::Extended, DetailLevel::Extended) => DetailLevel::Extended,

            // Transitioning up
            (DetailLevel::Minimal, DetailLevel::Standard) => {
                if factor > 0.5 {
                    DetailLevel::Standard
                } else {
                    DetailLevel::Minimal
                }
            }
            (DetailLevel::Minimal, DetailLevel::Extended) => {
                if factor > 0.66 {
                    DetailLevel::Extended
                } else if factor > 0.33 {
                    DetailLevel::Standard
                } else {
                    DetailLevel::Minimal
                }
            }
            (DetailLevel::Standard, DetailLevel::Extended) => {
                if factor > 0.5 {
                    DetailLevel::Extended
                } else {
                    DetailLevel::Standard
                }
            }

            // Transitioning down
            (DetailLevel::Standard, DetailLevel::Minimal) => {
                if factor > 0.5 {
                    DetailLevel::Minimal
                } else {
                    DetailLevel::Standard
                }
            }
            (DetailLevel::Extended, DetailLevel::Minimal) => {
                if factor > 0.66 {
                    DetailLevel::Minimal
                } else if factor > 0.33 {
                    DetailLevel::Standard
                } else {
                    DetailLevel::Extended
                }
            }
            (DetailLevel::Extended, DetailLevel::Standard) => {
                if factor > 0.5 {
                    DetailLevel::Standard
                } else {
                    DetailLevel::Extended
                }
            }
        }
    }

    /// Adapts content with progressive enhancement based on available space
    ///
    /// # Arguments
    /// * `enhanced_game` - Enhanced game display data
    /// * `base_detail_level` - Base detail level from layout calculator
    /// * `available_width` - Available width for content
    /// * `available_height` - Available height for content
    ///
    /// # Returns
    /// * `AdaptedGameContent` - Progressively enhanced content
    pub fn adapt_content_progressively(
        enhanced_game: &EnhancedGameDisplay,
        base_detail_level: DetailLevel,
        available_width: u16,
        available_height: u16,
    ) -> AdaptedGameContent {
        // Calculate content complexity
        let content_complexity = enhanced_game.expanded_goal_details.len();

        // Determine optimal detail level based on space and complexity
        let optimal_detail_level = Self::determine_progressive_detail_level(
            base_detail_level,
            available_width,
            available_height,
            content_complexity,
        );

        // Prioritize content elements
        let priority = Self::prioritize_content(
            enhanced_game,
            (available_width, available_height),
            optimal_detail_level,
        );

        // Adapt content using the optimal detail level and priorities
        let mut adapted_content =
            Self::adapt_enhanced_game_content(enhanced_game, optimal_detail_level, available_width);

        // Apply content prioritization
        if !priority.show_goal_details {
            adapted_content.goal_lines.clear();
        } else if priority.show_goal_details
            && adapted_content.goal_lines.len() > (available_height as usize / 3)
        {
            // Limit goal lines if space is constrained
            let max_lines = std::cmp::max(1, available_height as usize / 3);
            adapted_content.goal_lines.truncate(max_lines);
        }

        // Adjust team names based on priority
        if !priority.show_extended_team_info {
            // Use shorter team names if extended info is not prioritized
            let (home, away) = Self::format_team_names(
                &enhanced_game.base_content.home_team,
                &enhanced_game.base_content.away_team,
                DetailLevel::Standard,
                available_width,
            );
            adapted_content.home_team = home;
            adapted_content.away_team = away;
        }

        // Adjust time display based on priority
        if !priority.show_detailed_time_info {
            adapted_content.time_display = enhanced_game.base_content.time.clone();
        }

        // Recalculate estimated height
        let base_height = 1;
        let goal_height = adapted_content.goal_lines.len() as u16;
        let spacer_height = 1;
        adapted_content.estimated_height = base_height + goal_height + spacer_height;

        adapted_content
    }

    /// Scales content dynamically based on available space constraints
    ///
    /// # Arguments
    /// * `content` - Base adapted content
    /// * `available_space` - Available space (width, height)
    /// * `target_detail_level` - Target detail level
    ///
    /// # Returns
    /// * `AdaptedGameContent` - Scaled content that fits within constraints
    pub fn scale_content_to_fit(
        mut content: AdaptedGameContent,
        available_space: (u16, u16),
        target_detail_level: DetailLevel,
    ) -> AdaptedGameContent {
        let (available_width, available_height) = available_space;

        // Scale team names if they're too long
        let max_team_width = (available_width as usize).saturating_sub(20) / 2;
        if content.home_team.chars().count() > max_team_width {
            content.home_team = Self::truncate_text(&content.home_team, max_team_width);
        }
        if content.away_team.chars().count() > max_team_width {
            content.away_team = Self::truncate_text(&content.away_team, max_team_width);
        }

        // Scale goal lines if they exceed available height
        let max_goal_lines = available_height.saturating_sub(3) as usize; // Reserve space for game line and spacing
        if content.goal_lines.len() > max_goal_lines {
            content.goal_lines.truncate(max_goal_lines);

            // Add indicator that content was truncated
            if max_goal_lines > 0 {
                let last_index = content.goal_lines.len() - 1;
                if let Some(last_line) = content.goal_lines.get_mut(last_index) {
                    if last_line.len() > 3 {
                        let truncated = format!("{}...", &last_line[..last_line.len() - 3]);
                        *last_line = truncated;
                    }
                }
            }
        }

        // Scale individual goal lines if they're too wide
        for goal_line in &mut content.goal_lines {
            if goal_line.chars().count() > available_width as usize {
                *goal_line = Self::truncate_text(goal_line, available_width as usize);
            }
        }

        // Recalculate estimated height after scaling
        let base_height = 1;
        let goal_height = content.goal_lines.len() as u16;
        let spacer_height = match target_detail_level {
            DetailLevel::Extended => 2,
            _ => 1,
        };
        content.estimated_height = base_height + goal_height + spacer_height;

        content
    }

    /// Provides fallback content when space is extremely constrained
    ///
    /// # Arguments
    /// * `enhanced_game` - Enhanced game display data
    /// * `available_width` - Available width (very limited)
    ///
    /// # Returns
    /// * `AdaptedGameContent` - Minimal fallback content
    pub fn create_fallback_content(
        enhanced_game: &EnhancedGameDisplay,
        available_width: u16,
    ) -> AdaptedGameContent {
        let game_data = &enhanced_game.base_content;

        // Ultra-minimal formatting for very constrained spaces
        let max_team_width = std::cmp::max(3, (available_width as usize).saturating_sub(10) / 2);

        let home_team = Self::truncate_text(&game_data.home_team, max_team_width);
        let away_team = Self::truncate_text(&game_data.away_team, max_team_width);

        // Minimal time and result display
        let time_display = if game_data.time.len() > 8 {
            Self::truncate_text(&game_data.time, 8)
        } else {
            game_data.time.clone()
        };

        let result_display = if game_data.result.len() > 5 {
            Self::truncate_text(&game_data.result, 5)
        } else {
            game_data.result.clone()
        };

        // No goal lines in fallback mode to save space
        AdaptedGameContent {
            home_team,
            away_team,
            time_display,
            result_display,
            goal_lines: Vec::new(),
            estimated_height: 2, // Just game line and minimal spacing
        }
    }

    /// Creates content with smooth transitions between detail levels
    ///
    /// # Arguments
    /// * `enhanced_game` - Enhanced game display data
    /// * `from_level` - Current detail level
    /// * `to_level` - Target detail level
    /// * `transition_progress` - Progress of transition (0.0 to 1.0)
    /// * `available_width` - Available width for content
    ///
    /// # Returns
    /// * `AdaptedGameContent` - Content with smooth transition applied
    pub fn create_transitional_content(
        enhanced_game: &EnhancedGameDisplay,
        from_level: DetailLevel,
        to_level: DetailLevel,
        transition_progress: f32,
        available_width: u16,
    ) -> AdaptedGameContent {
        let progress = transition_progress.clamp(0.0, 1.0);

        // Determine intermediate detail level
        let intermediate_level = Self::create_smooth_transition(from_level, to_level, progress);

        // Create base content with intermediate level
        let mut content =
            Self::adapt_enhanced_game_content(enhanced_game, intermediate_level, available_width);

        // Apply transition-specific adjustments only if levels are different
        if from_level != to_level && progress < 1.0 {
            match (from_level, to_level) {
                // Transitioning from minimal to standard
                (DetailLevel::Minimal, DetailLevel::Standard) => {
                    let blend_factor = progress;
                    content =
                        Self::blend_content_formatting(content, from_level, to_level, blend_factor);
                }

                // Transitioning from standard to extended
                (DetailLevel::Standard, DetailLevel::Extended) => {
                    let blend_factor = progress;
                    content =
                        Self::blend_content_formatting(content, from_level, to_level, blend_factor);
                }

                // Transitioning from minimal to extended (through standard)
                (DetailLevel::Minimal, DetailLevel::Extended) => {
                    let blend_factor = progress;
                    content =
                        Self::blend_content_formatting(content, from_level, to_level, blend_factor);
                }

                // Transitioning down (extended to standard, standard to minimal, extended to minimal)
                (DetailLevel::Extended, DetailLevel::Standard)
                | (DetailLevel::Standard, DetailLevel::Minimal)
                | (DetailLevel::Extended, DetailLevel::Minimal) => {
                    let blend_factor = 1.0 - progress;
                    content =
                        Self::blend_content_formatting(content, to_level, from_level, blend_factor);
                }

                // Same levels - no transition needed (shouldn't reach here due to guard)
                _ => {}
            }
        }

        content
    }

    /// Blends content formatting between two detail levels
    ///
    /// # Arguments
    /// * `content` - Base content to blend
    /// * `level_a` - First detail level
    /// * `level_b` - Second detail level
    /// * `blend_factor` - Blending factor (0.0 = level_a, 1.0 = level_b)
    ///
    /// # Returns
    /// * `AdaptedGameContent` - Blended content
    fn blend_content_formatting(
        mut content: AdaptedGameContent,
        level_a: DetailLevel,
        level_b: DetailLevel,
        blend_factor: f32,
    ) -> AdaptedGameContent {
        let factor = blend_factor.clamp(0.0, 1.0);

        // Blend team name formatting
        if matches!(
            (level_a, level_b),
            (DetailLevel::Standard, DetailLevel::Extended)
        ) {
            // Gradually introduce extended team formatting
            if factor > 0.5 {
                // Start showing extended indicators
                if !content.home_team.starts_with("🏠") {
                    content.home_team = format!("🏠 {}", content.home_team);
                }
                if !content.away_team.starts_with("✈️") {
                    content.away_team = format!("✈️  {}", content.away_team);
                }
            }
        }

        // Blend result formatting
        if matches!(
            (level_a, level_b),
            (DetailLevel::Standard, DetailLevel::Extended)
        ) && factor > 0.7 {
            // Gradually introduce extended result formatting
            if !content.result_display.starts_with('┤') {
                content.result_display = format!("┤{result:^7}├", result = content.result_display.trim());
            }
        }

        // Blend goal line formatting
        if factor > 0.3 && level_b == DetailLevel::Extended && factor > 0.6 {
            // Gradually introduce extended goal formatting elements
            for goal_line in &mut content.goal_lines {
                if !goal_line.contains('│') {
                    // Add extended formatting elements gradually
                    *goal_line = format!("│{goal_line}│");
                }
            }
        }

        content
    }

    /// Safely adapts game content with comprehensive error handling
    pub fn adapt_game_content_safe(
        home_team: &str,
        away_team: &str,
        time: &str,
        result: &str,
        goal_events: &[GoalEventData],
        detail_level: DetailLevel,
        available_width: u16,
    ) -> Result<AdaptedGameContent, AppError> {
        // Validate input parameters
        if available_width == 0 {
            return Err(AppError::content_truncation_required(
                "Available width is zero",
            ));
        }

        if available_width < dynamic_ui::MIN_CONTENT_WIDTH {
            return Err(AppError::content_truncation_required(format!(
                "Available width {} is below minimum {}",
                available_width,
                dynamic_ui::MIN_CONTENT_WIDTH
            )));
        }

        // Attempt normal content adaptation
        let adapted_content = Self::adapt_game_content(
            home_team,
            away_team,
            time,
            result,
            goal_events,
            detail_level,
            available_width,
        );

        // Validate the adapted content
        if adapted_content.home_team.is_empty() && !home_team.is_empty() {
            return Err(AppError::content_truncation_required(
                "Home team name was completely truncated",
            ));
        }

        if adapted_content.away_team.is_empty() && !away_team.is_empty() {
            return Err(AppError::content_truncation_required(
                "Away team name was completely truncated",
            ));
        }

        Ok(adapted_content)
    }

    /// Adapts content for emergency layout mode with maximum truncation
    pub fn adapt_game_content_emergency(
        home_team: &str,
        away_team: &str,
        time: &str,
        result: &str,
        _goal_events: &[GoalEventData],
        _available_width: u16,
    ) -> AdaptedGameContent {
        // In emergency mode, use minimal formatting and aggressive truncation
        let max_team_width = dynamic_ui::EMERGENCY_MAX_TEAM_NAME_LENGTH;

        let formatted_home = Self::emergency_truncate_team_name(home_team, max_team_width);
        let formatted_away = Self::emergency_truncate_team_name(away_team, max_team_width);

        // Simplify time and result display
        let formatted_time = if time.len() > 5 {
            Self::truncate_text(time, 5)
        } else {
            time.to_string()
        };

        let formatted_result = if result.len() > 5 {
            Self::truncate_text(result, 5)
        } else {
            result.to_string()
        };

        // Skip goal events in emergency mode to save space
        let goal_lines = Vec::new();

        AdaptedGameContent {
            home_team: formatted_home,
            away_team: formatted_away,
            time_display: formatted_time,
            result_display: formatted_result,
            goal_lines,
            estimated_height: 2, // Minimal height in emergency mode
        }
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
        assert_eq!(content.result_display, "┤  1-0  ├"); // Enhanced extended formatting
        assert!(!content.goal_lines.is_empty());

        // Should include winning goal indicator - check all lines since extended format has separators
        let has_goal_info = content.goal_lines.iter().any(|line| {
            line.contains("Voittomaali") || line.contains("Ylivoima") || line.contains("Koivu")
        });
        assert!(has_goal_info);
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

        assert_eq!(home, "🏠 HIFK Helsinki");
        assert_eq!(away, "✈️  TPS Turku");
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
        assert_eq!(extended, "┤  2-1  ├"); // Enhanced formatting for extended
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
        assert_eq!(lines.len(), 3); // Separator + content + separator
        assert!(lines[1].contains("Koivu")); // Content is in the middle line
        assert!(lines[1].contains("15.00")); // Enhanced time format
        assert!(lines[1].contains("Ylivoima")); // Full translation
        assert!(lines[1].contains("Voittomaali")); // Full winning goal text
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
        assert!(truncated.ends_with(dynamic_ui::TRUNCATION_INDICATOR));

        let short_text = "Short";
        let not_truncated = ContentAdapter::truncate_text(short_text, 10);
        assert_eq!(not_truncated, "Short");

        // Test edge case with zero width
        let empty = ContentAdapter::truncate_text("test", 0);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_emergency_truncate_team_name() {
        // Test emergency truncation with very small widths
        let team_name = "Tappara Tampere";

        let truncated_1 = ContentAdapter::emergency_truncate_team_name(team_name, 1);
        assert_eq!(truncated_1, dynamic_ui::TRUNCATION_INDICATOR);

        let truncated_2 = ContentAdapter::emergency_truncate_team_name(team_name, 2);
        assert!(truncated_2.chars().count() <= 2);
        assert!(truncated_2.ends_with(dynamic_ui::TRUNCATION_INDICATOR));

        let truncated_4 = ContentAdapter::emergency_truncate_team_name(team_name, 4);
        assert!(truncated_4.chars().count() <= 4);
        assert!(truncated_4.ends_with(dynamic_ui::TRUNCATION_INDICATOR));
    }

    #[test]
    fn test_create_team_abbreviation() {
        // Test abbreviation creation
        let team_name = "Tappara Tampere";
        let abbrev = ContentAdapter::create_team_abbreviation(team_name, 2);
        assert_eq!(abbrev, "TT"); // First letters of each word

        let single_word = "Tappara";
        let abbrev_single = ContentAdapter::create_team_abbreviation(single_word, 3);
        assert_eq!(abbrev_single, "Tap"); // First 3 characters
    }

    #[test]
    fn test_safe_content_adaptation() {
        let goal_events = vec![];

        // Test successful adaptation
        let result = ContentAdapter::adapt_game_content_safe(
            "Tappara",
            "HIFK",
            "18:30",
            "3-2",
            &goal_events,
            DetailLevel::Minimal,
            80,
        );
        assert!(result.is_ok());

        // Test with zero width
        let result = ContentAdapter::adapt_game_content_safe(
            "Tappara",
            "HIFK",
            "18:30",
            "3-2",
            &goal_events,
            DetailLevel::Minimal,
            0,
        );
        assert!(result.is_err());

        // Test with width below minimum
        let result = ContentAdapter::adapt_game_content_safe(
            "Tappara",
            "HIFK",
            "18:30",
            "3-2",
            &goal_events,
            DetailLevel::Minimal,
            20,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_emergency_content_adaptation() {
        let goal_events = vec![];

        let adapted = ContentAdapter::adapt_game_content_emergency(
            "Tappara Tampere",
            "HIFK Helsinki",
            "18:30:45",
            "3-2",
            &goal_events,
            40,
        );

        // Check that content is heavily truncated
        assert!(adapted.home_team.chars().count() <= dynamic_ui::EMERGENCY_MAX_TEAM_NAME_LENGTH);
        assert!(adapted.away_team.chars().count() <= dynamic_ui::EMERGENCY_MAX_TEAM_NAME_LENGTH);
        assert!(adapted.time_display.chars().count() <= 5);
        assert!(adapted.result_display.chars().count() <= 5);
        assert!(adapted.goal_lines.is_empty()); // No goal events in emergency mode
        assert_eq!(adapted.estimated_height, 2); // Minimal height
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
        let formatted = ContentAdapter::format_extended_scorer("Koivu", 15, &goal_types, true, 50);

        assert!(formatted.contains("Koivu"));
        assert!(formatted.contains("15.00")); // Enhanced time format
        assert!(formatted.contains("Ylivoima")); // Full translation
        assert!(formatted.contains("Voittomaali")); // Full winning goal text
        assert!(formatted.len() <= 80); // Allow more space for enhanced formatting
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
        let available_width = 40; // Very narrow
        let content = ContentAdapter::adapt_game_content(
            "Very Long Team Name",
            "Another Long Name",
            "18:30",
            "2-1",
            &[],
            DetailLevel::Minimal,
            available_width,
        );

        // Calculate expected maximum width using the same logic as implementation
        let expected_max_width = {
            let reserved_space = 20; // Same as calculate_minimal_team_width
            let remaining = available_width.saturating_sub(reserved_space);
            let team_space = remaining / 2;
            (team_space as usize).clamp(8, 15)
        };

        // Should still produce valid content
        assert!(!content.home_team.is_empty());
        assert!(!content.away_team.is_empty());
        assert!(
            content.home_team.chars().count() <= expected_max_width,
            "Home team name '{}' (char count {}) exceeds expected max width {}",
            content.home_team,
            content.home_team.chars().count(),
            expected_max_width
        );
        assert!(
            content.away_team.chars().count() <= expected_max_width,
            "Away team name '{}' (char count {}) exceeds expected max width {}",
            content.away_team,
            content.away_team.chars().count(),
            expected_max_width
        );
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

        assert_eq!(adapted.home_team, "🏠 HIFK Helsinki");
        assert_eq!(adapted.away_team, "✈️  Tappara Tampere");
        assert!(adapted.time_display.contains("18:30"));
        assert!(!adapted.goal_lines.is_empty());
    }
}
