// src/teletext_ui/layout/mod.rs - Layout management infrastructure for dynamic column width calculations

mod ansi_cache;
mod columns;
mod config;

pub use ansi_cache::*;
#[allow(unused_imports)] // Items used by integration tests
pub use columns::*;
pub use config::*;

use crate::data_fetcher::models::GameData;
use std::collections::HashMap;

use columns::generate_content_signature;
use config::{ContentAnalysis, ContentCacheKey, LayoutCacheKey};

/// Manages column layout calculations for dynamic width determination
#[derive(Debug)]
pub struct ColumnLayoutManager {
    /// Terminal width available for content
    terminal_width: usize,
    /// Content margin from terminal border
    content_margin: usize,
    /// Cache for layout calculations to avoid repeated computation
    layout_cache: HashMap<LayoutCacheKey, LayoutConfig>,
    /// Cache for content analysis results
    content_analysis_cache: HashMap<ContentCacheKey, ContentAnalysis>,
    /// Cache for string operations (goal type displays)
    string_cache: HashMap<Vec<String>, String>,
    /// Cache for pre-calculated ANSI positioning codes
    ansi_cache: AnsiCodeCache,
}

/// Minimum terminal width constants for layout validation
impl ColumnLayoutManager {
    /// Absolute minimum terminal width required for basic functionality
    /// This allows for: margin(2) + home(15) + sep(3) + away(15) + time(8) + score(6) = 49 chars
    const ABSOLUTE_MINIMUM_WIDTH: usize = 50;

    /// Recommended minimum terminal width for optimal display
    /// This allows for: margin(2) + home(20) + sep(3) + away(20) + play_icon + goal_types + time + score = 70 chars
    const RECOMMENDED_MINIMUM_WIDTH: usize = 70;

    /// Minimum columns reserved for the time/score area: time (5) + space (1) + score (4) = 10
    const MIN_TIME_SCORE_SPACE: usize = 10;
}

impl ColumnLayoutManager {
    /// Creates a new ColumnLayoutManager with specified terminal width and margin
    ///
    /// # Arguments
    /// * `terminal_width` - Available terminal width
    /// * `content_margin` - Margin from terminal border (typically 2)
    pub fn new(terminal_width: usize, content_margin: usize) -> Self {
        Self {
            terminal_width,
            content_margin,
            layout_cache: HashMap::new(),
            content_analysis_cache: HashMap::new(),
            string_cache: HashMap::new(),
            ansi_cache: AnsiCodeCache::new(),
        }
    }

    /// Creates a new ColumnLayoutManager specifically for wide mode column calculations
    ///
    /// # Arguments
    /// * `column_width` - Available width for a single column in wide mode
    /// * `content_margin` - Margin from column border (typically 2)
    ///
    /// # Returns
    /// * `ColumnLayoutManager` - Manager configured for wide mode column constraints
    pub fn new_for_wide_mode_column(column_width: usize, content_margin: usize) -> Self {
        Self {
            terminal_width: column_width,
            content_margin,
            layout_cache: HashMap::new(),
            content_analysis_cache: HashMap::new(),
            string_cache: HashMap::new(),
            ansi_cache: AnsiCodeCache::new(),
        }
    }

    /// Clears all caches to free memory
    /// Should be called periodically or when memory usage is a concern
    pub fn clear_caches(&mut self) {
        let layout_cache_size = self.layout_cache.len();
        let content_cache_size = self.content_analysis_cache.len();
        let string_cache_size = self.string_cache.len();
        let ansi_cache_stats = self.ansi_cache.get_cache_stats();

        self.layout_cache.clear();
        self.content_analysis_cache.clear();
        self.string_cache.clear();
        self.ansi_cache.clear();

        tracing::debug!(
            "Cleared layout caches: {} layout entries, {} content analysis entries, {} string entries, {} ANSI codes",
            layout_cache_size,
            content_cache_size,
            string_cache_size,
            ansi_cache_stats.position_codes + ansi_cache_stats.color_position_codes
        );
    }

    /// Gets cache statistics for monitoring performance
    #[allow(dead_code)] // Used in tests
    pub fn get_cache_stats(&self) -> CacheStats {
        CacheStats {
            layout_cache_size: self.layout_cache.len(),
            content_analysis_cache_size: self.content_analysis_cache.len(),
            string_cache_size: self.string_cache.len(),
        }
    }

    /// Pre-calculates ANSI positioning codes for optimal performance
    /// This should be called after layout calculation to optimize rendering (requirement 4.3)
    ///
    /// # Arguments
    /// * `layout_config` - The layout configuration to pre-calculate codes for
    /// * `max_lines` - Maximum number of lines that will be rendered
    pub fn pre_calculate_ansi_codes(&mut self, layout_config: &LayoutConfig, max_lines: usize) {
        self.ansi_cache
            .pre_calculate_positions(layout_config, max_lines);
        tracing::debug!(
            "Pre-calculated ANSI positioning codes for {} lines",
            max_lines
        );
    }

    /// Gets an optimized positioning code for the given line and column
    /// Uses pre-calculated codes when available (requirement 4.3)
    ///
    /// # Arguments
    /// * `line` - Line number (1-based for ANSI)
    /// * `column` - Column number (1-based for ANSI)
    ///
    /// # Returns
    /// * `&str` - The ANSI positioning code
    pub fn get_position_code(&mut self, line: usize, column: usize) -> &str {
        self.ansi_cache.get_position_code(line, column)
    }

    /// Gets an optimized positioning code with color for the given line, column, and color
    /// Uses pre-calculated codes when available (requirement 4.3)
    ///
    /// # Arguments
    /// * `line` - Line number (1-based for ANSI)
    /// * `column` - Column number (1-based for ANSI)
    /// * `color` - ANSI color code
    ///
    /// # Returns
    /// * `&str` - The ANSI positioning code with color
    pub fn get_color_position_code(&mut self, line: usize, column: usize, color: u8) -> &str {
        self.ansi_cache.get_color_position_code(line, column, color)
    }

    /// Generates optimized ANSI codes for team name display
    /// Batches positioning, color, and formatting operations (requirement 4.3)
    ///
    /// # Arguments
    /// * `line` - Line number
    /// * `column` - Column position
    /// * `color` - Text color
    /// * `team_name` - Team name to display
    /// * `width` - Display width for padding
    ///
    /// # Returns
    /// * `String` - Complete formatted ANSI string
    pub fn format_team_name(
        &mut self,
        line: usize,
        column: usize,
        color: u8,
        team_name: &str,
        width: usize,
    ) -> String {
        let position_code = self.get_color_position_code(line, column, color);
        format!(
            "{}{:<width$}\x1b[0m",
            position_code,
            team_name.chars().take(width).collect::<String>(),
            width = width
        )
    }

    /// Generates optimized ANSI codes for separator display
    /// Pre-formats common separator patterns (requirement 4.3)
    ///
    /// # Arguments
    /// * `line` - Line number
    /// * `column` - Column position
    /// * `color` - Text color
    ///
    /// # Returns
    /// * `String` - Complete formatted ANSI string for separator
    pub fn format_separator(&mut self, line: usize, column: usize, color: u8) -> String {
        let position_code = self.get_color_position_code(line, column, color);
        format!("{}  -  \x1b[0m", position_code) // Balanced separator: "  -  " (5 chars total)
    }

    /// Generates optimized ANSI codes for time/score display
    /// Batches positioning and color operations (requirement 4.3)
    ///
    /// # Arguments
    /// * `line` - Line number
    /// * `column` - Column position
    /// * `color` - Text color
    /// * `text` - Text to display
    ///
    /// # Returns
    /// * `String` - Complete formatted ANSI string
    pub fn format_time_score(
        &mut self,
        line: usize,
        column: usize,
        color: u8,
        text: &str,
    ) -> String {
        let position_code = self.get_color_position_code(line, column, color);
        format!("{}{}\x1b[0m", position_code, text)
    }

    /// Generates optimized ANSI codes for goal type display
    /// Uses cached formatting for common goal type combinations (requirement 4.3)
    ///
    /// # Arguments
    /// * `line` - Line number
    /// * `column` - Column position
    /// * `color` - Text color
    /// * `goal_types` - Goal type string
    ///
    /// # Returns
    /// * `String` - Complete formatted ANSI string
    pub fn format_goal_types(
        &mut self,
        line: usize,
        column: usize,
        color: u8,
        goal_types: &str,
    ) -> String {
        let position_code = self.get_color_position_code(line, column, color);
        format!("{}{}\x1b[0m", position_code, goal_types)
    }

    /// Batch generates multiple ANSI codes for a complete game line
    /// Optimizes the most common rendering pattern (requirement 4.3)
    ///
    /// # Arguments
    /// * `line` - Line number
    /// * `layout_config` - Layout configuration
    /// * `home_team` - Home team name
    /// * `away_team` - Away team name
    /// * `time_score` - Time or score text
    /// * `text_color` - Text color code
    /// * `result_color` - Result color code
    ///
    /// # Returns
    /// * `String` - Complete formatted game line with all ANSI codes
    #[allow(clippy::too_many_arguments)]
    pub fn format_complete_game_line(
        &mut self,
        line: usize,
        layout_config: &LayoutConfig,
        home_team: &str,
        away_team: &str,
        time_score: &str,
        text_color: u8,
        result_color: u8,
    ) -> String {
        // Pre-calculate all positions
        let home_pos = self.content_margin + 1;
        let separator_pos = home_pos + layout_config.home_team_width;
        let away_pos = separator_pos + layout_config.separator_width;
        let time_pos = layout_config.time_column;

        // Pre-calculate all ANSI codes to avoid multiple mutable borrows
        let home_code = self
            .get_color_position_code(line, home_pos, text_color)
            .to_string();
        let separator_code = self.format_separator(line, separator_pos, text_color);
        let away_code = self
            .get_color_position_code(line, away_pos, text_color)
            .to_string();
        let time_code = self
            .get_color_position_code(line, time_pos, result_color)
            .to_string();

        // Batch all ANSI code generation into a single string operation
        let home_width = layout_config.home_team_width;
        let away_width = layout_config.away_team_width;
        format!(
            "{}{:<home_width$}{}{}{:<away_width$}{}{}\x1b[0m",
            home_code,
            home_team.chars().take(home_width).collect::<String>(),
            separator_code,
            away_code,
            away_team.chars().take(away_width).collect::<String>(),
            time_code,
            time_score
        )
    }

    /// Optimizes string operations by caching goal type display generation
    /// This reduces repeated string allocations and concatenations
    #[allow(dead_code)]
    fn get_cached_goal_type_display(&mut self, goal_types: &[String]) -> String {
        // Check cache first
        if let Some(cached_display) = self.string_cache.get(goal_types) {
            return cached_display.clone();
        }

        // Generate display string (optimized version of GoalEventData::get_goal_type_display)
        let display = if goal_types.is_empty() {
            String::new()
        } else {
            // Pre-allocate string with estimated capacity to reduce reallocations
            let estimated_capacity = goal_types.len() * 3; // Assume average 2 chars per type + space
            let mut result = String::with_capacity(estimated_capacity);

            for (i, goal_type) in goal_types.iter().enumerate() {
                if i > 0 {
                    result.push(' ');
                }
                result.push_str(goal_type);
            }
            result
        };

        // Cache the result for future use
        self.string_cache
            .insert(goal_types.to_vec(), display.clone());

        // Limit cache size to prevent unbounded growth
        if self.string_cache.len() > 1000 {
            tracing::debug!("String cache size exceeded 1000 entries, clearing oldest entries");
            // Keep only the most recent 500 entries (simple LRU approximation)
            let keys_to_remove: Vec<_> = self.string_cache.keys().take(500).cloned().collect();
            for key in keys_to_remove {
                self.string_cache.remove(&key);
            }
        }

        display
    }

    /// Calculates optimal layout configuration based on game content
    /// Includes safe fallbacks for missing or corrupted data (requirement 4.1)
    /// Uses caching to optimize repeated calculations (requirement 4.3)
    ///
    /// # Arguments
    /// * `games` - Slice of game data to analyze for content requirements
    ///
    /// # Returns
    /// * `LayoutConfig` - Calculated layout configuration
    pub fn calculate_layout(&mut self, games: &[GameData]) -> LayoutConfig {
        tracing::debug!(
            "Starting layout calculation for {} games with terminal width {}",
            games.len(),
            self.terminal_width
        );

        // Handle empty games list safely (requirement 4.1)
        if games.is_empty() {
            tracing::info!("No games provided for layout calculation, using default configuration");
            return LayoutConfig::default();
        }

        // Check cache first to avoid repeated calculations (requirement 4.3)
        let content_signature = generate_content_signature(games);
        let cache_key = LayoutCacheKey {
            terminal_width: self.terminal_width,
            content_margin: self.content_margin,
            content_signature,
            is_wide_mode: false,
        };

        if let Some(cached_layout) = self.layout_cache.get(&cache_key) {
            tracing::debug!(
                "Layout calculation cache hit for signature {}, returning cached result",
                content_signature
            );
            return cached_layout.clone();
        }

        tracing::debug!(
            "Layout calculation cache miss for signature {}, performing calculation",
            content_signature
        );

        // Validate and sanitize game data before layout calculations (requirement 4.1)
        let validator = GameDataValidator::new();
        let sanitized_games = validator.sanitize_games(games);

        // Handle case where all games were filtered out due to validation issues
        if sanitized_games.is_empty() {
            tracing::warn!(
                "All games were filtered out during validation, using default configuration"
            );
            return LayoutConfig::default();
        }

        if sanitized_games.len() != games.len() {
            tracing::info!(
                "Game data validation completed: {} out of {} games passed validation",
                sanitized_games.len(),
                games.len()
            );
        }

        // Validate terminal width and use fallback if necessary
        match self.validate_terminal_width() {
            TerminalWidthValidation::TooNarrow {
                current_width,
                minimum_required,
            } => {
                tracing::warn!(
                    "Terminal width {} is below absolute minimum of {}. Layout may be severely compromised. Using fallback layout.",
                    current_width,
                    minimum_required
                );
                return self.create_fallback_layout(&sanitized_games);
            }
            TerminalWidthValidation::Suboptimal {
                current_width,
                recommended_width,
            } => {
                tracing::warn!(
                    "Terminal width {} is below recommended minimum of {}. Using reduced spacing layout with fallback.",
                    current_width,
                    recommended_width
                );
                return self.create_fallback_layout(&sanitized_games);
            }
            TerminalWidthValidation::Adequate { current_width } => {
                tracing::debug!(
                    "Terminal width {} is adequate for optimal layout calculation",
                    current_width
                );
            }
        }

        let mut config = LayoutConfig::default();

        // Analyze content to determine space requirements using sanitized game data (with caching)
        let content_analysis = self.analyze_content_cached(&sanitized_games, false);

        tracing::debug!(
            "Content analysis results: max_player_name_width={}, max_goal_types_width={}",
            content_analysis.max_player_name_width,
            content_analysis.max_goal_types_width
        );

        // Calculate available width for dynamic content
        let fixed_width = self.content_margin
            + config.home_team_width
            + config.separator_width
            + config.away_team_width;
        let available_dynamic_width = self.terminal_width.saturating_sub(fixed_width + 10); // Reserve 10 chars for time/score

        tracing::debug!(
            "Layout space calculation: fixed_width={}, available_dynamic_width={}, terminal_width={}",
            fixed_width,
            available_dynamic_width,
            self.terminal_width
        );

        // Update config with analyzed content requirements
        config.max_player_name_width = content_analysis.max_player_name_width;
        config.max_goal_types_width = content_analysis.max_goal_types_width;

        // Calculate play icon column position to ensure proper alignment
        // Position it after the longest expected content in the home team area
        let home_content_end = self.content_margin
            + config.home_team_width
            + config.separator_width
            + config.away_team_width;
        config.play_icon_column = home_content_end + 2; // Add small buffer

        // Adjust time and score columns based on available space
        if available_dynamic_width >= 20 {
            // Position time column closer to the teams, score column further right for alignment
            config.time_column = config.play_icon_column
                + content_analysis.max_player_name_width
                + content_analysis.max_goal_types_width
                + 1;
            config.score_column = config.time_column + 8; // More space between time and score

            tracing::debug!(
                "Using optimal layout: play_icon_column={}, time_column={}, score_column={}",
                config.play_icon_column,
                config.time_column,
                config.score_column
            );
        } else {
            // Fallback for narrow terminals
            config.time_column = self.terminal_width.saturating_sub(18); // Move time further left
            config.score_column = self.terminal_width.saturating_sub(8);

            tracing::warn!(
                "Insufficient space for optimal layout (available_dynamic_width={}). Using fallback positioning: time_column={}, score_column={}",
                available_dynamic_width,
                config.time_column,
                config.score_column
            );
        }

        tracing::debug!(
            "Final layout configuration: home_team_width={}, away_team_width={}, play_icon_column={}, time_column={}, score_column={}",
            config.home_team_width,
            config.away_team_width,
            config.play_icon_column,
            config.time_column,
            config.score_column
        );

        // Cache the calculated layout for future use (requirement 4.3)
        self.layout_cache.insert(cache_key, config.clone());

        // Limit cache size to prevent unbounded memory growth
        if self.layout_cache.len() > 100 {
            tracing::debug!("Layout cache size exceeded 100 entries, clearing oldest entries");
            // Keep only the most recent 50 entries (simple LRU approximation)
            let keys_to_remove: Vec<_> = self.layout_cache.keys().take(50).cloned().collect();
            for key in keys_to_remove {
                self.layout_cache.remove(&key);
            }
        }

        config
    }

    /// Calculates layout configuration optimized for wide mode columns
    /// Maintains proportional spacing while adapting to reduced column width
    /// Uses caching to optimize repeated calculations (requirement 4.3)
    ///
    /// # Arguments
    /// * `games` - Slice of game data to analyze for content requirements
    ///
    /// # Returns
    /// * `LayoutConfig` - Layout configuration adapted for wide mode column constraints
    pub fn calculate_wide_mode_layout(&mut self, games: &[GameData]) -> LayoutConfig {
        tracing::debug!(
            "Starting wide mode layout calculation for {} games with column width {}",
            games.len(),
            self.terminal_width
        );

        // Check cache first for wide mode layouts (requirement 4.3)
        let content_signature = generate_content_signature(games);
        let cache_key = LayoutCacheKey {
            terminal_width: self.terminal_width,
            content_margin: self.content_margin,
            content_signature,
            is_wide_mode: true,
        };

        if let Some(cached_layout) = self.layout_cache.get(&cache_key) {
            tracing::debug!(
                "Wide mode layout calculation cache hit for signature {}, returning cached result",
                content_signature
            );
            return cached_layout.clone();
        }

        tracing::debug!(
            "Wide mode layout calculation cache miss for signature {}, performing calculation",
            content_signature
        );

        // Validate and sanitize game data before layout calculations (requirement 4.1)
        let validator = GameDataValidator::new();
        let sanitized_games = validator.sanitize_games(games);

        if sanitized_games.len() != games.len() {
            tracing::info!(
                "Wide mode game data validation completed: {} out of {} games passed validation",
                sanitized_games.len(),
                games.len()
            );
        }

        let mut config = LayoutConfig::default();

        // Analyze content to determine space requirements using sanitized game data (with caching)
        let content_analysis = self.analyze_content_cached(&sanitized_games, false);

        tracing::debug!(
            "Wide mode content analysis: max_player_name_width={}, max_goal_types_width={}",
            content_analysis.max_player_name_width,
            content_analysis.max_goal_types_width
        );

        // Wide mode columns are narrower, so we need to be more conservative with spacing
        // Typical wide mode column width is around 60-64 characters
        let is_narrow_column = self.terminal_width <= 70;

        if is_narrow_column {
            // Adjust team widths for narrow wide mode columns
            config.home_team_width = 18; // Slightly reduced from 20
            config.away_team_width = 18; // Slightly reduced from 20
            config.separator_width = 3; // Reduced separator for narrow terminals

            tracing::debug!(
                "Using narrow column layout for wide mode: home_team_width={}, away_team_width={}",
                config.home_team_width,
                config.away_team_width
            );
        } else {
            tracing::debug!(
                "Using standard column layout for wide mode: home_team_width={}, away_team_width={}",
                config.home_team_width,
                config.away_team_width
            );
        }

        // Calculate available width for dynamic content in wide mode
        let fixed_width = self.content_margin
            + config.home_team_width
            + config.separator_width
            + config.away_team_width;
        let available_dynamic_width = self.terminal_width.saturating_sub(fixed_width + 8); // Reserve 8 chars for time/score (reduced from 10)

        // Update config with analyzed content requirements, but be more conservative
        let original_player_width = content_analysis.max_player_name_width;
        let original_goal_types_width = content_analysis.max_goal_types_width;

        config.max_player_name_width = content_analysis.max_player_name_width.min(15); // Cap at 15 for wide mode
        config.max_goal_types_width = content_analysis.max_goal_types_width.min(6); // Cap at 6 for wide mode

        if original_player_width > config.max_player_name_width {
            tracing::debug!(
                "Capping player name width for wide mode: {} -> {}",
                original_player_width,
                config.max_player_name_width
            );
        }

        if original_goal_types_width > config.max_goal_types_width {
            tracing::debug!(
                "Capping goal types width for wide mode: {} -> {}",
                original_goal_types_width,
                config.max_goal_types_width
            );
        }

        // Calculate play icon column position for wide mode
        let home_content_end = self.content_margin
            + config.home_team_width
            + config.separator_width
            + config.away_team_width;
        config.play_icon_column = home_content_end + 1; // Reduced buffer for wide mode

        // Adjust time and score columns for wide mode constraints
        if available_dynamic_width >= 15 {
            // Position time column closer to teams, score column further right for alignment
            config.time_column = config.play_icon_column
                + config.max_player_name_width
                + config.max_goal_types_width
                + 1;
            config.score_column = config.time_column + 7; // More space between time and score for alignment

            tracing::debug!(
                "Wide mode optimal layout: play_icon_column={}, time_column={}, score_column={}",
                config.play_icon_column,
                config.time_column,
                config.score_column
            );
        } else {
            // Fallback for very narrow wide mode columns
            config.time_column = self.terminal_width.saturating_sub(15); // Move time further left
            config.score_column = self.terminal_width.saturating_sub(6);

            tracing::warn!(
                "Very narrow wide mode column (available_dynamic_width={}). Using fallback positioning: time_column={}, score_column={}",
                available_dynamic_width,
                config.time_column,
                config.score_column
            );
        }

        tracing::debug!(
            "Final wide mode layout: column_width={}, max_player_name_width={}, max_goal_types_width={}, play_icon_column={}, time_column={}, score_column={}",
            self.terminal_width,
            config.max_player_name_width,
            config.max_goal_types_width,
            config.play_icon_column,
            config.time_column,
            config.score_column
        );

        // Cache the calculated wide mode layout for future use (requirement 4.3)
        self.layout_cache.insert(cache_key, config.clone());

        config
    }

    /// Analyzes game content to determine space requirements
    /// Includes safe fallbacks for corrupted or missing data (requirement 4.1)
    ///
    /// # Arguments
    /// * `games` - Slice of game data to analyze
    ///
    /// # Returns
    /// * `ContentAnalysis` - Analysis results with maximum widths needed
    fn analyze_content(&self, games: &[GameData]) -> ContentAnalysis {
        let mut max_player_name_width = 0;
        let mut max_goal_types_width = 0;
        let mut total_events = 0;

        for game in games {
            // Safely analyze goal events for player name and goal type requirements
            for event in &game.goal_events {
                total_events += 1;

                // Track longest player name with safe fallbacks
                let safe_player_name = if event.scorer_name.trim().is_empty() {
                    "Unknown Player".to_string()
                } else {
                    event.scorer_name.clone()
                };

                let player_name_len = safe_player_name.len();
                if player_name_len > max_player_name_width {
                    max_player_name_width = player_name_len;
                    tracing::debug!(
                        "New longest player name found: '{}' (length: {})",
                        safe_player_name,
                        player_name_len
                    );
                }

                // Track longest goal type combination with safe fallbacks
                let goal_type_display = event.get_goal_type_display();

                let goal_type_len = goal_type_display.len();
                if goal_type_len > max_goal_types_width {
                    max_goal_types_width = goal_type_len;
                    tracing::debug!(
                        "New longest goal type combination found: '{}' (length: {})",
                        goal_type_display,
                        goal_type_len
                    );
                }
            }
        }

        tracing::debug!(
            "Content analysis completed: {} events analyzed, raw max_player_name_width={}, raw max_goal_types_width={}",
            total_events,
            max_player_name_width,
            max_goal_types_width
        );

        // Apply reasonable limits to prevent excessive spacing
        let original_player_width = max_player_name_width;
        let original_goal_types_width = max_goal_types_width;

        max_player_name_width = max_player_name_width.clamp(10, 20); // Between 10-20 chars
        max_goal_types_width = max_goal_types_width.clamp(2, 8); // Between 2-8 chars

        if original_player_width != max_player_name_width {
            tracing::debug!(
                "Applied player name width limits: {} -> {} (min: 10, max: 20)",
                original_player_width,
                max_player_name_width
            );
        }

        if original_goal_types_width != max_goal_types_width {
            tracing::debug!(
                "Applied goal types width limits: {} -> {} (min: 2, max: 8)",
                original_goal_types_width,
                max_goal_types_width
            );
        }

        ContentAnalysis {
            max_player_name_width,
            max_goal_types_width,
        }
    }

    /// Cached version of content analysis to optimize repeated calculations
    ///
    /// # Arguments
    /// * `games` - Slice of game data to analyze
    /// * `is_fallback` - Whether this is for fallback analysis
    ///
    /// # Returns
    /// * `ContentAnalysis` - Analysis results (cached or freshly calculated)
    fn analyze_content_cached(&mut self, games: &[GameData], is_fallback: bool) -> ContentAnalysis {
        let content_signature = generate_content_signature(games);
        let cache_key = ContentCacheKey {
            content_signature,
            is_fallback,
            terminal_width: if is_fallback {
                Some(self.terminal_width)
            } else {
                None
            },
        };

        // Check cache first
        if let Some(cached_analysis) = self.content_analysis_cache.get(&cache_key) {
            tracing::debug!(
                "Content analysis cache hit for signature {} (fallback: {})",
                content_signature,
                is_fallback
            );
            return cached_analysis.clone();
        }

        tracing::debug!(
            "Content analysis cache miss for signature {} (fallback: {}), performing analysis",
            content_signature,
            is_fallback
        );

        // Perform analysis
        let analysis = if is_fallback {
            self.analyze_content_for_fallback(games)
        } else {
            self.analyze_content(games)
        };

        // Cache the result
        self.content_analysis_cache
            .insert(cache_key, analysis.clone());

        // Limit cache size to prevent unbounded memory growth
        if self.content_analysis_cache.len() > 200 {
            tracing::debug!(
                "Content analysis cache size exceeded 200 entries, clearing oldest entries"
            );
            // Keep only the most recent 100 entries (simple LRU approximation)
            let keys_to_remove: Vec<_> = self
                .content_analysis_cache
                .keys()
                .take(100)
                .cloned()
                .collect();
            for key in keys_to_remove {
                self.content_analysis_cache.remove(&key);
            }
        }

        analysis
    }

    /// Gets the calculated width for home team display area
    ///
    /// # Arguments
    /// * `layout` - Layout configuration
    ///
    /// # Returns
    /// * `usize` - Width allocated for home team
    #[allow(dead_code)] // Used in tests
    pub fn get_home_team_width(&self, layout: &LayoutConfig) -> usize {
        layout.home_team_width
    }

    /// Gets the calculated width for away team display area
    ///
    /// # Arguments
    /// * `layout` - Layout configuration
    ///
    /// # Returns
    /// * `usize` - Width allocated for away team
    #[allow(dead_code)] // Used in tests
    pub fn get_away_team_width(&self, layout: &LayoutConfig) -> usize {
        layout.away_team_width
    }

    /// Gets the calculated column position for play icon alignment
    ///
    /// # Arguments
    /// * `layout` - Layout configuration
    ///
    /// # Returns
    /// * `usize` - Column position for play icons
    #[allow(dead_code)] // Used in tests
    pub fn get_play_icon_column(&self, layout: &LayoutConfig) -> usize {
        layout.play_icon_column
    }

    /// Calculates dynamic spacing after player names to maintain alignment
    ///
    /// # Arguments
    /// * `player_name_length` - Length of the current player name
    /// * `layout` - Layout configuration
    ///
    /// # Returns
    /// * `usize` - Number of spaces to add after player name
    #[allow(dead_code)] // Used in tests
    pub fn calculate_dynamic_spacing(
        &self,
        player_name_length: usize,
        layout: &LayoutConfig,
    ) -> usize {
        if player_name_length >= layout.max_player_name_width {
            1 // Minimum spacing
        } else {
            layout.max_player_name_width - player_name_length + 1
        }
    }

    /// Validates that goal types will fit within allocated space
    ///
    /// # Arguments
    /// * `goal_types` - Goal type string to validate
    /// * `layout` - Layout configuration
    ///
    /// # Returns
    /// * `bool` - True if goal types fit within allocated space
    #[allow(dead_code)] // Used in tests
    pub fn validate_goal_types_fit(&self, goal_types: &str, layout: &LayoutConfig) -> bool {
        let fits = goal_types.len() <= layout.max_goal_types_width;

        if !fits {
            tracing::warn!(
                "Goal types '{}' (length: {}) exceed allocated width of {}. May cause layout issues.",
                goal_types,
                goal_types.len(),
                layout.max_goal_types_width
            );
        } else {
            tracing::debug!(
                "Goal types '{}' (length: {}) fit within allocated width of {}",
                goal_types,
                goal_types.len(),
                layout.max_goal_types_width
            );
        }

        fits
    }

    /// Validates if the terminal width is sufficient for proper layout
    ///
    /// # Returns
    /// * `TerminalWidthValidation` - Validation result with recommendations
    pub fn validate_terminal_width(&self) -> TerminalWidthValidation {
        if self.terminal_width < Self::ABSOLUTE_MINIMUM_WIDTH {
            TerminalWidthValidation::TooNarrow {
                current_width: self.terminal_width,
                minimum_required: Self::ABSOLUTE_MINIMUM_WIDTH,
            }
        } else if self.terminal_width < Self::RECOMMENDED_MINIMUM_WIDTH {
            TerminalWidthValidation::Suboptimal {
                current_width: self.terminal_width,
                recommended_width: Self::RECOMMENDED_MINIMUM_WIDTH,
            }
        } else {
            TerminalWidthValidation::Adequate {
                current_width: self.terminal_width,
            }
        }
    }

    /// Creates a fallback layout configuration for narrow terminals
    /// This layout sacrifices some visual appeal for basic functionality
    /// Uses intelligent truncation to preserve critical information
    ///
    /// # Arguments
    /// * `games` - Slice of game data to analyze for content requirements
    ///
    /// # Returns
    /// * `LayoutConfig` - Minimal viable layout configuration
    pub fn create_fallback_layout(&mut self, games: &[GameData]) -> LayoutConfig {
        tracing::warn!(
            "Terminal width {} is below recommended minimum of {}. Using fallback layout with intelligent truncation.",
            self.terminal_width,
            Self::RECOMMENDED_MINIMUM_WIDTH
        );

        let mut config = LayoutConfig::default();
        let truncator = IntelligentTruncator::new();

        // Use reduced team widths for narrow terminals
        config.home_team_width = if self.terminal_width < Self::ABSOLUTE_MINIMUM_WIDTH {
            12 // Absolute minimum
        } else {
            15 // Reduced but still readable
        };

        config.away_team_width = config.home_team_width;
        config.separator_width = 3; // Reduced separator for fallback layout

        // Analyze content but with stricter limits and intelligent truncation (with caching)
        let content_analysis =
            self.analyze_content_for_fallback_with_truncation_cached(games, &truncator);
        config.max_player_name_width = content_analysis.max_player_name_width;
        config.max_goal_types_width = content_analysis.max_goal_types_width;

        // Calculate positions with minimal spacing
        let teams_width = config.home_team_width + config.separator_width + config.away_team_width;
        config.play_icon_column = self.content_margin + teams_width + 1; // Minimal buffer

        // Calculate minimum required space for play icon area
        let play_icon_area_width = config.max_player_name_width + config.max_goal_types_width + 2;
        let play_icon_area_end = config.play_icon_column + play_icon_area_width;

        let available_width_after_play_area =
            self.terminal_width.saturating_sub(play_icon_area_end);

        if available_width_after_play_area >= Self::MIN_TIME_SCORE_SPACE {
            // Enough space after play area
            config.time_column = play_icon_area_end + 1;
            config.score_column = config.time_column + 6;
        } else {
            // Not enough space - use intelligent truncation strategy
            let critical_content_width = teams_width + Self::MIN_TIME_SCORE_SPACE;
            let strategy = truncator
                .determine_truncation_strategy(self.terminal_width, critical_content_width);

            match strategy {
                TruncationStrategy::NoTruncation | TruncationStrategy::ReduceSpacing => {
                    // Try positioning time and score at the very end
                    config.score_column = self.terminal_width.saturating_sub(4);
                    config.time_column = config.score_column.saturating_sub(6);
                }
                TruncationStrategy::MinimalSpacing => {
                    // Use minimal spacing throughout
                    config.score_column = self.terminal_width.saturating_sub(4);
                    config.time_column = config.score_column.saturating_sub(6);

                    // Recalculate play icon area end with current settings
                    let current_play_icon_area_end = config.play_icon_column
                        + config.max_player_name_width
                        + config.max_goal_types_width
                        + 2;

                    // Reduce play icon area if needed to prevent overlap
                    if config.time_column <= current_play_icon_area_end {
                        // Calculate how much space is actually available for the play area
                        let available_for_play_area =
                            if config.time_column > config.play_icon_column {
                                config
                                    .time_column
                                    .saturating_sub(config.play_icon_column)
                                    .saturating_sub(1) // Reserve 1 space buffer
                            } else {
                                // Time column is too close, use minimal space
                                3 // Minimum for player name
                            };

                        // We need space for: player_name + goal_types + 2 (standard spacing)
                        // Total must fit within available_for_play_area
                        if available_for_play_area >= 6 {
                            // Enough space for reasonable content: 3 chars player + 1 char goal types + 2 spacing = 6
                            config.max_player_name_width = 3;
                            config.max_goal_types_width = (available_for_play_area - 3 - 2).max(1); // available - player - spacing

                            tracing::warn!(
                                "Minimal spacing fallback: available_for_play_area={}, using player_name_width={}, goal_types_width={}",
                                available_for_play_area,
                                config.max_player_name_width,
                                config.max_goal_types_width
                            );
                        } else if available_for_play_area >= 5 {
                            // Minimal space: 2 chars player + 1 char goal types + 2 spacing = 5
                            config.max_player_name_width = 2;
                            config.max_goal_types_width = 1;

                            tracing::warn!(
                                "Extreme minimal spacing fallback: available_for_play_area={}, using player_name_width={}, goal_types_width={}",
                                available_for_play_area,
                                config.max_player_name_width,
                                config.max_goal_types_width
                            );
                        } else {
                            // Extreme case: reduce everything to fit
                            config.max_player_name_width = (available_for_play_area - 2).max(1); // Reserve 2 for spacing
                            config.max_goal_types_width = 0; // Will be handled specially in rendering

                            tracing::error!(
                                "Critical layout fallback: available_for_play_area={} is extremely limited. Using player_name_width={}, goal_types_width={}. Goal types may not display properly.",
                                available_for_play_area,
                                config.max_player_name_width,
                                config.max_goal_types_width
                            );
                        }
                    }
                }
                TruncationStrategy::AggressiveTruncation => {
                    // Aggressive truncation to preserve critical information
                    tracing::warn!(
                        "Terminal width {} requires aggressive truncation. Some content may be severely limited.",
                        self.terminal_width
                    );

                    // Position time and score at absolute minimum positions
                    config.score_column = self.terminal_width.saturating_sub(4);
                    config.time_column = config.score_column.saturating_sub(6);

                    // Calculate maximum available space for play area
                    let max_play_area = config.time_column.saturating_sub(config.play_icon_column);

                    // Preserve goal types at all costs (requirement 3.4), truncate player names aggressively
                    if max_play_area >= config.max_goal_types_width + 3 {
                        // 3 for minimum player name
                        let new_player_width = max_play_area
                            .saturating_sub(config.max_goal_types_width + 1)
                            .max(3);

                        tracing::warn!(
                            "Aggressive truncation: preserving goal types (width={}), reducing player names to width={}",
                            config.max_goal_types_width,
                            new_player_width
                        );

                        config.max_player_name_width = new_player_width;
                    } else {
                        // Extreme case: very limited space - preserve goal types at all costs (requirement 3.4)
                        let original_goal_types_width = config.max_goal_types_width;
                        config.max_player_name_width = 3; // Minimum viable
                        // Never reduce goal_types_width below 2 (requirement 3.4)
                        config.max_goal_types_width = config
                            .max_goal_types_width
                            .min(max_play_area.saturating_sub(4))
                            .max(2);

                        tracing::error!(
                            "Critical aggressive truncation: max_play_area={} is extremely limited. Player names reduced to minimum ({}), goal types preserved but limited ({} -> {})",
                            max_play_area,
                            config.max_player_name_width,
                            original_goal_types_width,
                            config.max_goal_types_width
                        );
                    }
                }
            }
        }

        // Final safety check - ensure all positions are within terminal bounds
        let original_score_column = config.score_column;
        let original_time_column = config.time_column;

        if config.score_column >= self.terminal_width {
            config.score_column = self.terminal_width.saturating_sub(1);
            tracing::warn!(
                "Score column position {} exceeded terminal width {}. Adjusted to {}",
                original_score_column,
                self.terminal_width,
                config.score_column
            );
        }

        if config.time_column >= config.score_column {
            config.time_column = config.score_column.saturating_sub(4);
            tracing::warn!(
                "Time column position {} would overlap with score column {}. Adjusted to {}",
                original_time_column,
                config.score_column,
                config.time_column
            );
        }

        // Log the final layout decisions for debugging
        tracing::debug!(
            "Fallback layout: terminal_width={}, max_player_name_width={}, max_goal_types_width={}, play_icon_column={}, time_column={}, score_column={}",
            self.terminal_width,
            config.max_player_name_width,
            config.max_goal_types_width,
            config.play_icon_column,
            config.time_column,
            config.score_column
        );

        config
    }

    /// Analyzes content with stricter limits for fallback layouts
    ///
    /// # Arguments
    /// * `games` - Slice of game data to analyze
    ///
    /// # Returns
    /// * `ContentAnalysis` - Analysis results with conservative limits
    fn analyze_content_for_fallback(&self, games: &[GameData]) -> ContentAnalysis {
        let mut max_player_name_width = 0;
        let mut max_goal_types_width = 0;

        for game in games {
            for event in &game.goal_events {
                max_player_name_width = max_player_name_width.max(event.scorer_name.len());
                let goal_type_display = event.get_goal_type_display();
                max_goal_types_width = max_goal_types_width.max(goal_type_display.len());
            }
        }

        // Apply stricter limits for fallback layout
        max_player_name_width = if self.terminal_width < Self::ABSOLUTE_MINIMUM_WIDTH {
            max_player_name_width.clamp(5, 8) // Very tight limits
        } else {
            max_player_name_width.clamp(8, 12) // Reduced but reasonable
        };

        max_goal_types_width = max_goal_types_width.clamp(2, 4); // Minimal goal type space

        ContentAnalysis {
            max_player_name_width,
            max_goal_types_width,
        }
    }

    /// Analyzes content with intelligent truncation for extreme fallback layouts
    /// Uses IntelligentTruncator to determine optimal content widths
    ///
    /// # Arguments
    /// * `games` - Slice of game data to analyze
    /// * `truncator` - IntelligentTruncator for handling extreme cases
    ///
    /// # Returns
    /// * `ContentAnalysis` - Analysis results with intelligent truncation applied
    fn analyze_content_for_fallback_with_truncation(
        &self,
        games: &[GameData],
        truncator: &IntelligentTruncator,
    ) -> ContentAnalysis {
        let mut max_player_name_width = 0;
        let mut max_goal_types_width = 0;

        for game in games {
            for event in &game.goal_events {
                max_player_name_width = max_player_name_width.max(event.scorer_name.len());
                let goal_type_display = event.get_goal_type_display();
                max_goal_types_width = max_goal_types_width.max(goal_type_display.len());
            }
        }

        // Determine truncation strategy based on terminal width
        let critical_content_width = self.content_margin + 30 + Self::MIN_TIME_SCORE_SPACE; // Basic teams + time/score
        let strategy =
            truncator.determine_truncation_strategy(self.terminal_width, critical_content_width);

        // Apply intelligent limits based on strategy
        max_player_name_width = match strategy {
            TruncationStrategy::NoTruncation => max_player_name_width.clamp(10, 20),
            TruncationStrategy::ReduceSpacing => max_player_name_width.clamp(8, 15),
            TruncationStrategy::MinimalSpacing => max_player_name_width.clamp(6, 12),
            TruncationStrategy::AggressiveTruncation => {
                if self.terminal_width < Self::ABSOLUTE_MINIMUM_WIDTH {
                    max_player_name_width.clamp(3, 6) // Very aggressive truncation
                } else {
                    max_player_name_width.clamp(5, 8) // Aggressive but readable
                }
            }
        };

        // Goal types should never be truncated (requirement 3.4), but we can limit space allocation
        max_goal_types_width = match strategy {
            TruncationStrategy::NoTruncation => max_goal_types_width.clamp(2, 8),
            TruncationStrategy::ReduceSpacing => max_goal_types_width.clamp(2, 6),
            TruncationStrategy::MinimalSpacing => max_goal_types_width.clamp(2, 5),
            TruncationStrategy::AggressiveTruncation => max_goal_types_width.clamp(2, 4),
        };

        // Log truncation decisions for debugging
        tracing::debug!(
            "Content analysis with truncation: strategy={:?}, player_name_width={}, goal_types_width={}",
            strategy,
            max_player_name_width,
            max_goal_types_width
        );

        ContentAnalysis {
            max_player_name_width,
            max_goal_types_width,
        }
    }

    /// Cached version of content analysis with intelligent truncation for extreme fallback layouts
    ///
    /// # Arguments
    /// * `games` - Slice of game data to analyze
    /// * `truncator` - IntelligentTruncator for handling extreme cases
    ///
    /// # Returns
    /// * `ContentAnalysis` - Analysis results with intelligent truncation applied (cached or freshly calculated)
    fn analyze_content_for_fallback_with_truncation_cached(
        &mut self,
        games: &[GameData],
        truncator: &IntelligentTruncator,
    ) -> ContentAnalysis {
        let content_signature = generate_content_signature(games);
        let cache_key = ContentCacheKey {
            content_signature,
            is_fallback: true,
            terminal_width: Some(self.terminal_width),
        };

        // Check cache first
        if let Some(cached_analysis) = self.content_analysis_cache.get(&cache_key) {
            tracing::debug!(
                "Fallback content analysis cache hit for signature {} (terminal_width: {})",
                content_signature,
                self.terminal_width
            );
            return cached_analysis.clone();
        }

        tracing::debug!(
            "Fallback content analysis cache miss for signature {} (terminal_width: {}), performing analysis",
            content_signature,
            self.terminal_width
        );

        // Perform analysis
        let analysis = self.analyze_content_for_fallback_with_truncation(games, truncator);

        // Cache the result
        self.content_analysis_cache
            .insert(cache_key, analysis.clone());

        analysis
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::GoalEventData;
    use crate::teletext_ui::ScoreType;

    fn create_test_game_data(
        home_team: &str,
        away_team: &str,
        goal_events: Vec<GoalEventData>,
    ) -> GameData {
        GameData {
            home_team: home_team.to_string(),
            away_team: away_team.to_string(),
            time: "18:30".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "RUNKOSARJA".to_string(),
            goal_events,
            played_time: 3600,
            start: "2024-01-15T18:30:00Z".to_string(),
            play_off_phase: None,
            play_off_pair: None,
            play_off_req_wins: None,
            series_score: None,
            is_placeholder: false,
        }
    }

    fn create_test_goal_event(scorer_name: &str, goal_types: Vec<String>) -> GoalEventData {
        GoalEventData {
            scorer_player_id: 123,
            scorer_name: scorer_name.to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types,
            is_home_team: true,
            video_clip_url: None,
        }
    }

    #[test]
    fn test_layout_config_default() {
        let config = LayoutConfig::default();
        assert_eq!(config.home_team_width, 20);
        assert_eq!(config.separator_width, 5);
        assert_eq!(config.away_team_width, 20);
        assert_eq!(config.time_column, 51); // Updated to match new default
        assert_eq!(config.score_column, 62); // Updated to match new default
        assert_eq!(config.play_icon_column, 51); // Updated to match new default
        assert_eq!(config.max_player_name_width, 17);
        assert_eq!(config.max_goal_types_width, 8); // Updated to match new default
    }

    #[test]
    fn test_column_layout_manager_creation() {
        let manager = ColumnLayoutManager::new(80, 2);
        assert_eq!(manager.terminal_width, 80);
        assert_eq!(manager.content_margin, 2);
    }

    #[test]
    fn test_content_analysis_empty_games() {
        let manager = ColumnLayoutManager::new(80, 2);
        let games = vec![];
        let analysis = manager.analyze_content(&games);

        // Should use minimum values when no content
        assert_eq!(analysis.max_player_name_width, 10);
        assert_eq!(analysis.max_goal_types_width, 2);
    }

    #[test]
    fn test_content_analysis_with_goal_events() {
        let manager = ColumnLayoutManager::new(80, 2);

        let goal_events = vec![
            create_test_goal_event("Short", vec!["YV".to_string()]),
            create_test_goal_event(
                "Very Long Player Name",
                vec!["YV".to_string(), "IM".to_string()],
            ),
            create_test_goal_event("Medium Name", vec!["TM".to_string()]),
        ];

        let games = vec![create_test_game_data("HIFK", "TPS", goal_events)];
        let analysis = manager.analyze_content(&games);

        // Should find the longest player name ("Very Long Player Name" = 20 chars)
        assert_eq!(analysis.max_player_name_width, 20);
        // Should find the longest goal type combination ("YV IM" = 5 chars)
        assert_eq!(analysis.max_goal_types_width, 5);
    }

    #[test]
    fn test_content_analysis_limits() {
        let manager = ColumnLayoutManager::new(80, 2);

        let goal_events = vec![create_test_goal_event(
            "Extremely Long Player Name That Exceeds Normal Limits",
            vec![
                "YV".to_string(),
                "IM".to_string(),
                "TM".to_string(),
                "VT".to_string(),
            ],
        )];

        let games = vec![create_test_game_data("HIFK", "TPS", goal_events)];
        let analysis = manager.analyze_content(&games);

        // Should be capped at maximum limits
        assert_eq!(analysis.max_player_name_width, 20);
        assert_eq!(analysis.max_goal_types_width, 8);
    }

    #[test]
    fn test_layout_calculation() {
        let mut manager = ColumnLayoutManager::new(80, 2);

        let goal_events = vec![create_test_goal_event(
            "Player Name",
            vec!["YV".to_string(), "IM".to_string()],
        )];

        let games = vec![create_test_game_data("HIFK", "TPS", goal_events)];
        let layout = manager.calculate_layout(&games);

        // Should maintain default team widths
        assert_eq!(layout.home_team_width, 20);
        assert_eq!(layout.away_team_width, 20);
        assert_eq!(layout.separator_width, 5);

        // Should update content-based values
        assert_eq!(layout.max_player_name_width, 11); // "Player Name" length
        assert_eq!(layout.max_goal_types_width, 5); // "YV IM" length

        // Play icon column should be positioned after team areas
        assert!(
            layout.play_icon_column
                > layout.home_team_width + layout.separator_width + layout.away_team_width
        );
    }

    #[test]
    fn test_dynamic_spacing_calculation() {
        let manager = ColumnLayoutManager::new(80, 2);
        let layout = LayoutConfig {
            max_player_name_width: 15,
            ..Default::default()
        };

        // Short name should get more spacing
        assert_eq!(manager.calculate_dynamic_spacing(5, &layout), 11); // 15 - 5 + 1

        // Name at max width should get minimum spacing
        assert_eq!(manager.calculate_dynamic_spacing(15, &layout), 1);

        // Name longer than max should get minimum spacing
        assert_eq!(manager.calculate_dynamic_spacing(20, &layout), 1);
    }

    #[test]
    fn test_goal_types_validation() {
        let manager = ColumnLayoutManager::new(80, 2);
        let layout = LayoutConfig {
            max_goal_types_width: 6,
            ..Default::default()
        };

        // Short goal types should fit
        assert!(manager.validate_goal_types_fit("YV", &layout));
        assert!(manager.validate_goal_types_fit("YV IM", &layout));

        // Goal types at limit should fit
        assert!(manager.validate_goal_types_fit("YV IM ", &layout)); // 6 chars

        // Goal types exceeding limit should not fit
        assert!(!manager.validate_goal_types_fit("YV IM TM", &layout)); // 8 chars
    }

    #[test]
    fn test_layout_accessors() {
        let manager = ColumnLayoutManager::new(80, 2);
        let layout = LayoutConfig::default();

        assert_eq!(manager.get_home_team_width(&layout), 20);
        assert_eq!(manager.get_away_team_width(&layout), 20);
        assert_eq!(manager.get_play_icon_column(&layout), 51);
    }

    #[test]
    fn test_narrow_terminal_layout() {
        let mut manager = ColumnLayoutManager::new(60, 2); // Narrow terminal

        let games = vec![create_test_game_data("HIFK", "TPS", vec![])];
        let layout = manager.calculate_layout(&games);

        // Should use fallback positioning for narrow terminals
        assert!(layout.time_column <= 60);
        assert!(layout.score_column <= 60);
        assert!(layout.time_column < layout.score_column);
    }

    #[test]
    fn test_wide_mode_column_layout_manager_creation() {
        let manager = ColumnLayoutManager::new_for_wide_mode_column(60, 2);
        assert_eq!(manager.terminal_width, 60);
        assert_eq!(manager.content_margin, 2);
    }

    #[test]
    fn test_wide_mode_layout_calculation() {
        let mut manager = ColumnLayoutManager::new_for_wide_mode_column(60, 2);

        let goal_events = vec![create_test_goal_event(
            "Long Player Name",
            vec!["YV".to_string(), "IM".to_string()],
        )];

        let games = vec![create_test_game_data("HIFK", "TPS", goal_events)];
        let layout = manager.calculate_wide_mode_layout(&games);

        // Should use reduced team widths for narrow wide mode columns
        assert_eq!(layout.home_team_width, 18);
        assert_eq!(layout.away_team_width, 18);
        assert_eq!(layout.separator_width, 3);
    }

    #[test]
    fn test_layout_calculation_caching() {
        let mut manager = ColumnLayoutManager::new(80, 2);

        let goal_events = vec![create_test_goal_event(
            "Player Name",
            vec!["YV".to_string(), "IM".to_string()],
        )];

        let games = vec![create_test_game_data("HIFK", "TPS", goal_events)];

        // First calculation should populate cache
        let layout1 = manager.calculate_layout(&games);
        let stats1 = manager.get_cache_stats();
        assert_eq!(stats1.layout_cache_size, 1);

        // Second calculation with same data should use cache
        let layout2 = manager.calculate_layout(&games);
        let stats2 = manager.get_cache_stats();
        assert_eq!(stats2.layout_cache_size, 1); // Should still be 1

        // Results should be identical
        assert_eq!(layout1.max_player_name_width, layout2.max_player_name_width);
        assert_eq!(layout1.max_goal_types_width, layout2.max_goal_types_width);
        assert_eq!(layout1.play_icon_column, layout2.play_icon_column);
    }

    #[test]
    fn test_string_caching() {
        let mut manager = ColumnLayoutManager::new(80, 2);

        let goal_types1 = vec!["YV".to_string(), "IM".to_string()];
        let goal_types2 = vec!["YV".to_string(), "IM".to_string()]; // Same content
        let goal_types3 = vec!["TM".to_string()]; // Different content

        // First call should populate cache
        let display1 = manager.get_cached_goal_type_display(&goal_types1);
        assert_eq!(display1, "YV IM");

        // Second call with same content should use cache
        let display2 = manager.get_cached_goal_type_display(&goal_types2);
        assert_eq!(display2, "YV IM");

        // Third call with different content should create new cache entry
        let display3 = manager.get_cached_goal_type_display(&goal_types3);
        assert_eq!(display3, "TM");

        let stats = manager.get_cache_stats();
        assert_eq!(stats.string_cache_size, 2); // Two unique entries
    }

    #[test]
    fn test_alignment_calculator_caching() {
        let mut calculator = AlignmentCalculator::new();
        let layout = LayoutConfig::default();

        let goal_events = vec![create_test_goal_event(
            "Player Name",
            vec!["YV".to_string()],
        )];
        let games = vec![create_test_game_data("HIFK", "TPS", goal_events.clone())];

        // First calculation should populate cache
        let positions1 = calculator.calculate_play_icon_positions(&games, &layout);
        let stats1 = calculator.get_cache_stats();
        assert_eq!(stats1.play_icon_cache_size, 1);

        // Second calculation with same data should use cache
        let positions2 = calculator.calculate_play_icon_positions(&games, &layout);
        let stats2 = calculator.get_cache_stats();
        assert_eq!(stats2.play_icon_cache_size, 1); // Should still be 1

        // Results should be identical
        assert_eq!(positions1.len(), positions2.len());
        assert_eq!(positions1[0].column_position, positions2[0].column_position);
    }

    #[test]
    fn test_cache_clearing() {
        let mut manager = ColumnLayoutManager::new(80, 2);
        let mut calculator = AlignmentCalculator::new();

        // Populate caches
        let games = vec![create_test_game_data("HIFK", "TPS", vec![])];
        let layout = manager.calculate_layout(&games);
        let _positions = calculator.calculate_play_icon_positions(&games, &layout);

        // Verify caches have entries
        assert!(manager.get_cache_stats().layout_cache_size > 0);
        assert!(calculator.get_cache_stats().play_icon_cache_size > 0);

        // Clear caches
        manager.clear_caches();
        calculator.clear_caches();

        // Verify caches are empty
        assert_eq!(manager.get_cache_stats().layout_cache_size, 0);
        assert_eq!(manager.get_cache_stats().content_analysis_cache_size, 0);
        assert_eq!(manager.get_cache_stats().string_cache_size, 0);
        assert_eq!(calculator.get_cache_stats().play_icon_cache_size, 0);
        assert_eq!(calculator.get_cache_stats().goal_type_cache_size, 0);

        // Should use reasonable content widths for normal mode
        assert!(layout.max_player_name_width <= 20);
        assert!(layout.max_goal_types_width <= 8);

        // Play icon column should be positioned after team areas
        assert!(
            layout.play_icon_column
                > layout.home_team_width + layout.separator_width + layout.away_team_width
        );

        // Time and score columns should fit within the terminal width
        assert!(layout.time_column <= 80);
        assert!(layout.score_column <= 80);
    }

    #[test]
    fn test_wide_mode_layout_with_normal_column_width() {
        let mut manager = ColumnLayoutManager::new_for_wide_mode_column(80, 2); // Wider column

        let goal_events = vec![create_test_goal_event("Player", vec!["YV".to_string()])];

        let games = vec![create_test_game_data("HIFK", "TPS", goal_events)];
        let layout = manager.calculate_wide_mode_layout(&games);

        // Should use normal team widths for wider columns
        assert_eq!(layout.home_team_width, 20);
        assert_eq!(layout.away_team_width, 20);
        assert_eq!(layout.separator_width, 5);

        // Should still cap content widths for wide mode
        assert!(layout.max_player_name_width <= 15);
        assert!(layout.max_goal_types_width <= 6);
    }

    #[test]
    fn test_wide_mode_layout_proportional_spacing() {
        let mut narrow_manager = ColumnLayoutManager::new_for_wide_mode_column(60, 2);
        let mut wide_manager = ColumnLayoutManager::new_for_wide_mode_column(80, 2);

        let games = vec![create_test_game_data("HIFK", "TPS", vec![])];
        let narrow_layout = narrow_manager.calculate_wide_mode_layout(&games);
        let wide_layout = wide_manager.calculate_wide_mode_layout(&games);

        // Narrow layout should have smaller team widths
        assert!(narrow_layout.home_team_width <= wide_layout.home_team_width);
        assert!(narrow_layout.away_team_width <= wide_layout.away_team_width);

        // Both should maintain proportional spacing
        let narrow_total_teams = narrow_layout.home_team_width
            + narrow_layout.separator_width
            + narrow_layout.away_team_width;
        let wide_total_teams =
            wide_layout.home_team_width + wide_layout.separator_width + wide_layout.away_team_width;

        // Narrow layout should use less space for teams
        assert!(narrow_total_teams <= wide_total_teams);
    }

    #[test]
    fn test_terminal_width_validation_adequate() {
        let manager = ColumnLayoutManager::new(80, 2);
        let validation = manager.validate_terminal_width();

        match validation {
            TerminalWidthValidation::Adequate { current_width } => {
                assert_eq!(current_width, 80);
            }
            _ => panic!("Expected Adequate validation for width 80"),
        }
    }

    #[test]
    fn test_terminal_width_validation_suboptimal() {
        let manager = ColumnLayoutManager::new(60, 2);
        let validation = manager.validate_terminal_width();

        match validation {
            TerminalWidthValidation::Suboptimal {
                current_width,
                recommended_width,
            } => {
                assert_eq!(current_width, 60);
                assert_eq!(recommended_width, 70);
            }
            _ => panic!("Expected Suboptimal validation for width 60"),
        }
    }

    #[test]
    fn test_terminal_width_validation_too_narrow() {
        let manager = ColumnLayoutManager::new(40, 2);
        let validation = manager.validate_terminal_width();

        match validation {
            TerminalWidthValidation::TooNarrow {
                current_width,
                minimum_required,
            } => {
                assert_eq!(current_width, 40);
                assert_eq!(minimum_required, 50);
            }
            _ => panic!("Expected TooNarrow validation for width 40"),
        }
    }

    #[test]
    fn test_fallback_layout_for_narrow_terminal() {
        let mut manager = ColumnLayoutManager::new(45, 2); // Below absolute minimum

        let goal_events = vec![create_test_goal_event(
            "Long Player Name",
            vec!["YV".to_string(), "IM".to_string()],
        )];

        let games = vec![create_test_game_data("HIFK", "TPS", goal_events)];
        let layout = manager.create_fallback_layout(&games);

        // Should use reduced team widths
        assert_eq!(layout.home_team_width, 12);
        assert_eq!(layout.away_team_width, 12);
        assert_eq!(layout.separator_width, 3);

        // Should have stricter content limits
        assert!(layout.max_player_name_width <= 8);
        assert!(layout.max_goal_types_width <= 4);

        // All positions should fit within terminal width
        assert!(layout.play_icon_column < 45);
        assert!(layout.time_column < 45);
        assert!(layout.score_column < 45);
    }

    #[test]
    fn test_fallback_layout_for_suboptimal_terminal() {
        let mut manager = ColumnLayoutManager::new(60, 2); // Below recommended minimum

        let goal_events = vec![create_test_goal_event(
            "Player Name",
            vec!["YV".to_string()],
        )];

        let games = vec![create_test_game_data("HIFK", "TPS", goal_events)];
        let layout = manager.create_fallback_layout(&games);

        // Should use reduced but reasonable team widths
        assert_eq!(layout.home_team_width, 15);
        assert_eq!(layout.away_team_width, 15);
        assert_eq!(layout.separator_width, 3);

        // Should have moderate content limits
        assert!(layout.max_player_name_width <= 12);
        assert!(layout.max_goal_types_width <= 4);

        // All positions should fit within terminal width
        assert!(layout.play_icon_column < 60);
        assert!(layout.time_column < 60);
        assert!(layout.score_column < 60);
    }

    #[test]
    fn test_calculate_layout_uses_fallback_for_narrow_terminal() {
        let mut manager = ColumnLayoutManager::new(45, 2); // Below absolute minimum

        let games = vec![create_test_game_data("HIFK", "TPS", vec![])];
        let layout = manager.calculate_layout(&games);

        // Should automatically use fallback layout
        assert_eq!(layout.home_team_width, 12);
        assert_eq!(layout.away_team_width, 12);
    }

    #[test]
    fn test_calculate_layout_uses_fallback_for_suboptimal_terminal() {
        let mut manager = ColumnLayoutManager::new(60, 2); // Below recommended minimum

        let games = vec![create_test_game_data("HIFK", "TPS", vec![])];
        let layout = manager.calculate_layout(&games);

        // Should automatically use fallback layout
        assert_eq!(layout.home_team_width, 15);
        assert_eq!(layout.away_team_width, 15);
    }

    #[test]
    fn test_calculate_layout_uses_normal_for_adequate_terminal() {
        let mut manager = ColumnLayoutManager::new(80, 2); // Adequate width

        let games = vec![create_test_game_data("HIFK", "TPS", vec![])];
        let layout = manager.calculate_layout(&games);

        // Should use normal layout
        assert_eq!(layout.home_team_width, 20);
        assert_eq!(layout.away_team_width, 20);
    }

    #[test]
    fn test_analyze_content_for_fallback_strict_limits() {
        let manager = ColumnLayoutManager::new(45, 2); // Very narrow

        let goal_events = vec![create_test_goal_event(
            "Extremely Long Player Name That Should Be Limited",
            vec!["YV".to_string(), "IM".to_string(), "TM".to_string()],
        )];

        let games = vec![create_test_game_data("HIFK", "TPS", goal_events)];
        let analysis = manager.analyze_content_for_fallback(&games);

        // Should apply very strict limits for narrow terminals
        assert!(analysis.max_player_name_width <= 8);
        assert!(analysis.max_player_name_width >= 5);
        assert!(analysis.max_goal_types_width <= 4);
        assert!(analysis.max_goal_types_width >= 2);
    }

    #[test]
    fn test_analyze_content_for_fallback_moderate_limits() {
        let manager = ColumnLayoutManager::new(60, 2); // Suboptimal but usable

        let goal_events = vec![create_test_goal_event(
            "Long Player Name",
            vec!["YV".to_string(), "IM".to_string()],
        )];

        let games = vec![create_test_game_data("HIFK", "TPS", goal_events)];
        let analysis = manager.analyze_content_for_fallback(&games);

        // Should apply moderate limits for suboptimal terminals
        assert!(analysis.max_player_name_width <= 12);
        assert!(analysis.max_player_name_width >= 8);
        assert!(analysis.max_goal_types_width <= 4);
        assert!(analysis.max_goal_types_width >= 2);
    }

    #[test]
    fn test_fallback_layout_positions_dont_overlap() {
        let mut manager = ColumnLayoutManager::new(50, 2); // Minimum width

        let goal_events = vec![create_test_goal_event(
            "Player",
            vec!["YV".to_string(), "IM".to_string()],
        )];

        let games = vec![create_test_game_data("HIFK", "TPS", goal_events)];
        let layout = manager.create_fallback_layout(&games);

        // Ensure positions don't overlap
        let play_icon_end = layout.play_icon_column
            + layout.max_player_name_width
            + layout.max_goal_types_width
            + 2;
        assert!(
            play_icon_end <= layout.time_column,
            "Play icon area (ends at {}) overlaps with time column ({})",
            play_icon_end,
            layout.time_column
        );

        assert!(
            layout.time_column < layout.score_column,
            "Time column ({}) should be before score column ({})",
            layout.time_column,
            layout.score_column
        );
    }

    #[test]
    fn test_alignment_calculator_creation() {
        let _calculator = AlignmentCalculator::new();
        // Should create successfully (no specific state to verify)
    }

    #[test]
    fn test_play_icon_positions_calculation() {
        let mut calculator = AlignmentCalculator::new();
        let layout = LayoutConfig::default();

        let goal_events = vec![
            create_test_goal_event("Player One", vec!["YV".to_string()]),
            create_test_goal_event("Player Two", vec!["IM".to_string()]),
        ];

        let games = vec![create_test_game_data("HIFK", "TPS", goal_events)];
        let positions = calculator.calculate_play_icon_positions(&games, &layout);

        assert_eq!(positions.len(), 2);

        // All positions should use the same column for alignment
        assert_eq!(positions[0].column_position, layout.play_icon_column);
        assert_eq!(positions[1].column_position, layout.play_icon_column);

        // Should track game and event indices correctly
        assert_eq!(positions[0].game_index, 0);
        assert_eq!(positions[0].event_index, 0);
        assert_eq!(positions[1].game_index, 0);
        assert_eq!(positions[1].event_index, 1);
    }

    #[test]
    fn test_play_icon_positions_with_video_links() {
        let mut calculator = AlignmentCalculator::new();
        let layout = LayoutConfig::default();

        let mut goal_event_with_video = create_test_goal_event("Player", vec!["YV".to_string()]);
        goal_event_with_video.video_clip_url = Some("http://example.com/video".to_string());

        let goal_event_without_video = create_test_goal_event("Player", vec!["YV".to_string()]);

        let games = vec![create_test_game_data(
            "HIFK",
            "TPS",
            vec![goal_event_with_video, goal_event_without_video],
        )];
        let positions = calculator.calculate_play_icon_positions(&games, &layout);

        assert_eq!(positions.len(), 2);
        assert!(positions[0].has_video_link);
        assert!(!positions[1].has_video_link);
    }

    #[test]
    fn test_goal_type_positions_calculation() {
        let mut calculator = AlignmentCalculator::new();
        let layout = LayoutConfig::default();

        let events = vec![
            create_test_goal_event("Player", vec!["YV".to_string()]),
            create_test_goal_event("Player", vec!["YV".to_string(), "IM".to_string()]),
        ];

        let positions = calculator.calculate_goal_type_positions(&events, &layout);

        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0].goal_types, "YV");
        assert_eq!(positions[1].goal_types, "YV IM");

        // Should have available width from layout
        assert_eq!(positions[0].available_width, layout.max_goal_types_width);
        assert_eq!(positions[1].available_width, layout.max_goal_types_width);
    }

    #[test]
    fn test_goal_type_overflow_prevention() {
        let mut calculator = AlignmentCalculator::new();
        let layout = LayoutConfig {
            play_icon_column: 40, // Position close to overflow boundary
            max_player_name_width: 10,
            ..Default::default()
        };

        let events = vec![create_test_goal_event(
            "Player",
            vec!["YV".to_string(), "IM".to_string(), "TM".to_string()],
        )];

        let positions = calculator.calculate_goal_type_positions(&events, &layout);

        assert_eq!(positions.len(), 1);

        // Should prevent overflow past column 43
        let end_position = positions[0].column_position + positions[0].goal_types.len();
        assert!(
            end_position <= 43,
            "Goal type position {} + length {} = {} exceeds column 43",
            positions[0].column_position,
            positions[0].goal_types.len(),
            end_position
        );
    }

    #[test]
    fn test_get_play_icon_column_position() {
        let calculator = AlignmentCalculator::new();
        let layout = LayoutConfig::default();

        assert_eq!(
            calculator.get_play_icon_column_position(&layout),
            layout.play_icon_column
        );
    }

    #[test]
    fn test_validate_no_overflow() {
        let calculator = AlignmentCalculator::new();
        let layout = LayoutConfig::default();

        // Position that fits within bounds
        let safe_position = GoalTypePosition {
            event_index: 0,
            column_position: 40,
            goal_types: "YV".to_string(), // 2 chars, ends at column 42
            available_width: 6,
        };

        assert!(calculator.validate_no_overflow(&safe_position, &layout));

        // Position that would overflow
        let overflow_position = GoalTypePosition {
            event_index: 0,
            column_position: 42,
            goal_types: "YV IM".to_string(), // 5 chars, ends at column 47 (overflow!)
            available_width: 6,
        };

        assert!(!calculator.validate_no_overflow(&overflow_position, &layout));

        // Position exactly at boundary
        let boundary_position = GoalTypePosition {
            event_index: 0,
            column_position: 41,
            goal_types: "YV".to_string(), // 2 chars, ends at column 43 (exactly at boundary)
            available_width: 6,
        };

        assert!(calculator.validate_no_overflow(&boundary_position, &layout));
    }

    // Additional comprehensive AlignmentCalculator unit tests for task 5

    #[test]
    fn test_play_icon_alignment_consistency_multiple_games() {
        let mut calculator = AlignmentCalculator::new();
        let layout = LayoutConfig::default();

        // Create multiple games with different content lengths
        let games = vec![
            create_test_game_data(
                "HIFK",
                "TPS",
                vec![
                    create_test_goal_event("Short", vec!["YV".to_string()]),
                    create_test_goal_event("Medium Name", vec!["IM".to_string()]),
                ],
            ),
            create_test_game_data(
                "Jokerit",
                "Blues",
                vec![create_test_goal_event(
                    "Very Long Player Name",
                    vec!["YV".to_string(), "IM".to_string()],
                )],
            ),
            create_test_game_data(
                "KalPa",
                "Ilves",
                vec![
                    create_test_goal_event("X", vec!["TM".to_string()]),
                    create_test_goal_event("Another Player", vec!["VT".to_string()]),
                    create_test_goal_event("Third Player Name", vec!["AV".to_string()]),
                ],
            ),
        ];

        let positions = calculator.calculate_play_icon_positions(&games, &layout);

        // Should have 6 total positions (2 + 1 + 3)
        assert_eq!(positions.len(), 6);

        // All play icons should be aligned to the same column regardless of content
        let expected_column = layout.play_icon_column;
        for position in &positions {
            assert_eq!(
                position.column_position,
                expected_column,
                "Play icon at game {} event {} should be at column {} but was at {}",
                position.game_index,
                position.event_index,
                expected_column,
                position.column_position
            );
        }

        // Verify game and event indices are correctly tracked
        assert_eq!(positions[0].game_index, 0);
        assert_eq!(positions[0].event_index, 0);
        assert_eq!(positions[1].game_index, 0);
        assert_eq!(positions[1].event_index, 1);
        assert_eq!(positions[2].game_index, 1);
        assert_eq!(positions[2].event_index, 0);
        assert_eq!(positions[3].game_index, 2);
        assert_eq!(positions[3].event_index, 0);
        assert_eq!(positions[4].game_index, 2);
        assert_eq!(positions[4].event_index, 1);
        assert_eq!(positions[5].game_index, 2);
        assert_eq!(positions[5].event_index, 2);
    }

    #[test]
    fn test_play_icon_alignment_consistency_empty_games() {
        let mut calculator = AlignmentCalculator::new();
        let layout = LayoutConfig::default();

        // Test with games that have no goal events
        let games = vec![
            create_test_game_data("HIFK", "TPS", vec![]),
            create_test_game_data("Jokerit", "Blues", vec![]),
        ];

        let positions = calculator.calculate_play_icon_positions(&games, &layout);

        // Should have no positions for games without goal events
        assert_eq!(positions.len(), 0);
    }

    #[test]
    fn test_play_icon_alignment_consistency_mixed_video_links() {
        let mut calculator = AlignmentCalculator::new();
        let layout = LayoutConfig::default();

        let mut event_with_video = create_test_goal_event("Player One", vec!["YV".to_string()]);
        event_with_video.video_clip_url = Some("http://example.com/video1".to_string());

        let event_without_video = create_test_goal_event("Player Two", vec!["IM".to_string()]);

        let mut event_with_video2 = create_test_goal_event("Player Three", vec!["TM".to_string()]);
        event_with_video2.video_clip_url = Some("http://example.com/video2".to_string());

        let games = vec![create_test_game_data(
            "HIFK",
            "TPS",
            vec![event_with_video, event_without_video, event_with_video2],
        )];

        let positions = calculator.calculate_play_icon_positions(&games, &layout);

        assert_eq!(positions.len(), 3);

        // All should use same column position regardless of video link presence
        let expected_column = layout.play_icon_column;
        for position in &positions {
            assert_eq!(position.column_position, expected_column);
        }

        // Verify video link tracking
        assert!(positions[0].has_video_link);
        assert!(!positions[1].has_video_link);
        assert!(positions[2].has_video_link);
    }

    #[test]
    fn test_goal_type_positioning_accuracy_various_lengths() {
        let mut calculator = AlignmentCalculator::new();
        let layout = LayoutConfig::default();

        let events = vec![
            create_test_goal_event("Player", vec!["YV".to_string()]), // 2 chars
            create_test_goal_event("Player", vec!["YV".to_string(), "IM".to_string()]), // 5 chars
            create_test_goal_event(
                "Player",
                vec!["YV".to_string(), "IM".to_string(), "TM".to_string()],
            ), // 8 chars
            create_test_goal_event("Player", vec!["VT".to_string()]), // 2 chars
            create_test_goal_event("Player", vec![]),                 // 0 chars (empty)
        ];

        let positions = calculator.calculate_goal_type_positions(&events, &layout);

        assert_eq!(positions.len(), 5);

        // Verify goal type strings are correctly formatted
        assert_eq!(positions[0].goal_types, "YV");
        assert_eq!(positions[1].goal_types, "YV IM");
        assert_eq!(positions[2].goal_types, "YV IM TM");
        assert_eq!(positions[3].goal_types, "VT");
        assert_eq!(positions[4].goal_types, "");

        // All should have the same available width from layout
        for position in &positions {
            assert_eq!(position.available_width, layout.max_goal_types_width);
        }

        // Verify all positions are safe (no overflow)
        for position in &positions {
            assert!(
                calculator.validate_no_overflow(position, &layout),
                "Goal type '{}' at position {} should not overflow",
                position.goal_types,
                position.column_position
            );
        }
    }

    #[test]
    fn test_goal_type_positioning_accuracy_with_custom_layout() {
        let mut calculator = AlignmentCalculator::new();
        let layout = LayoutConfig {
            play_icon_column: 30,
            max_player_name_width: 15,
            max_goal_types_width: 8,
            ..Default::default()
        };

        let events = vec![create_test_goal_event(
            "Player",
            vec!["YV".to_string(), "IM".to_string()],
        )];

        let positions = calculator.calculate_goal_type_positions(&events, &layout);

        assert_eq!(positions.len(), 1);

        // Position should be calculated based on play_icon_column + max_player_name_width + 1
        let expected_base_position = layout.play_icon_column + layout.max_player_name_width + 1;

        // But should be adjusted to prevent overflow past column 43
        let goal_types_length = positions[0].goal_types.len();
        let max_allowed_position = 43_usize.saturating_sub(goal_types_length);
        let expected_position = expected_base_position.min(max_allowed_position);

        assert_eq!(positions[0].column_position, expected_position);
        assert_eq!(positions[0].available_width, 8);
    }

    #[test]
    fn test_overflow_prevention_edge_cases() {
        let mut calculator = AlignmentCalculator::new();
        // Test case 1: Layout that would cause overflow
        let mut layout = LayoutConfig {
            play_icon_column: 42,
            max_player_name_width: 10,
            ..Default::default()
        };

        let events = vec![
            create_test_goal_event(
                "Player",
                vec!["YV".to_string(), "IM".to_string(), "TM".to_string()],
            ), // "YV IM TM" = 8 chars
        ];

        let positions = calculator.calculate_goal_type_positions(&events, &layout);
        assert_eq!(positions.len(), 1);

        // Should prevent overflow by adjusting position
        let end_position = positions[0].column_position + positions[0].goal_types.len();
        assert!(
            end_position <= 43,
            "End position {} should not exceed column 43",
            end_position
        );

        // Test case 2: Extreme overflow scenario
        layout.play_icon_column = 50; // Way past safe zone
        let positions2 = calculator.calculate_goal_type_positions(&events, &layout);
        assert_eq!(positions2.len(), 1);

        let end_position2 = positions2[0].column_position + positions2[0].goal_types.len();
        assert!(
            end_position2 <= 43,
            "End position {} should not exceed column 43 even with extreme layout",
            end_position2
        );
    }

    #[test]
    fn test_overflow_prevention_boundary_conditions() {
        let calculator = AlignmentCalculator::new();
        let layout = LayoutConfig::default();

        // Test positions that are exactly at the boundary
        let boundary_cases = [
            GoalTypePosition {
                event_index: 0,
                column_position: 43,
                goal_types: "".to_string(), // Empty string at column 43 should be valid
                available_width: 6,
            },
            GoalTypePosition {
                event_index: 1,
                column_position: 42,
                goal_types: "Y".to_string(), // 1 char ending at column 43 should be valid
                available_width: 6,
            },
            GoalTypePosition {
                event_index: 2,
                column_position: 41,
                goal_types: "YV".to_string(), // 2 chars ending at column 43 should be valid
                available_width: 6,
            },
            GoalTypePosition {
                event_index: 3,
                column_position: 44,
                goal_types: "".to_string(), // Empty string past boundary should be invalid
                available_width: 6,
            },
            GoalTypePosition {
                event_index: 4,
                column_position: 43,
                goal_types: "Y".to_string(), // 1 char starting at column 43 should be invalid
                available_width: 6,
            },
        ];

        // Test each boundary case
        assert!(calculator.validate_no_overflow(&boundary_cases[0], &layout)); // Empty at 43: valid
        assert!(calculator.validate_no_overflow(&boundary_cases[1], &layout)); // "Y" at 42: valid (ends at 43)
        assert!(calculator.validate_no_overflow(&boundary_cases[2], &layout)); // "YV" at 41: valid (ends at 43)
        assert!(!calculator.validate_no_overflow(&boundary_cases[3], &layout)); // Empty at 44: invalid
        assert!(!calculator.validate_no_overflow(&boundary_cases[4], &layout)); // "Y" at 43: invalid (ends at 44)
    }

    #[test]
    fn test_overflow_prevention_with_long_goal_types() {
        let mut calculator = AlignmentCalculator::new();
        let layout = LayoutConfig {
            play_icon_column: 35,
            max_player_name_width: 5,
            ..Default::default()
        };

        // Create events with progressively longer goal type combinations
        let events = vec![
            create_test_goal_event(
                "Player",
                vec![
                    "YV".to_string(),
                    "IM".to_string(),
                    "TM".to_string(),
                    "VT".to_string(),
                ],
            ), // Very long combination
        ];

        let positions = calculator.calculate_goal_type_positions(&events, &layout);
        assert_eq!(positions.len(), 1);

        // Even with very long goal types, should not overflow
        let end_position = positions[0].column_position + positions[0].goal_types.len();
        assert!(
            end_position <= 43,
            "Long goal types should not cause overflow: end position {}",
            end_position
        );

        // Verify the goal types are still correctly formatted
        assert_eq!(positions[0].goal_types, "YV IM VT TM");
    }

    #[test]
    fn test_alignment_calculator_consistency_across_different_layouts() {
        let mut calculator = AlignmentCalculator::new();

        // Test with different layout configurations
        let layouts = vec![
            LayoutConfig {
                play_icon_column: 30,
                max_player_name_width: 10,
                ..Default::default()
            },
            LayoutConfig {
                play_icon_column: 40,
                max_player_name_width: 15,
                ..Default::default()
            },
            LayoutConfig {
                play_icon_column: 50,
                max_player_name_width: 20,
                ..Default::default()
            },
        ];

        let events = vec![
            create_test_goal_event("Player", vec!["YV".to_string()]),
            create_test_goal_event("Player", vec!["IM".to_string()]),
        ];

        for layout in &layouts {
            let play_positions = calculator.calculate_play_icon_positions(
                &[create_test_game_data("HIFK", "TPS", events.clone())],
                layout,
            );

            let goal_positions = calculator.calculate_goal_type_positions(&events, layout);

            // All play icons should use the layout's play_icon_column consistently
            for position in &play_positions {
                assert_eq!(position.column_position, layout.play_icon_column);
            }

            // All goal type positions should respect overflow prevention
            for position in &goal_positions {
                assert!(calculator.validate_no_overflow(position, layout));
            }

            // Play icon column should match the accessor method
            assert_eq!(
                calculator.get_play_icon_column_position(layout),
                layout.play_icon_column
            );
        }
    }

    #[test]
    fn test_intelligent_truncator_player_name_truncation() {
        let truncator = IntelligentTruncator::new();

        // Test normal case - name fits
        assert_eq!(truncator.truncate_player_name("Short", 10, None), "Short");

        // Test truncation with ellipsis
        assert_eq!(
            truncator.truncate_player_name("Very Long Player Name", 10, None),
            "Very Lo..."
        );

        // Test preserving critical characters
        assert_eq!(
            truncator.truncate_player_name("Player", 8, Some(5)),
            "Player"
        );
        assert_eq!(
            truncator.truncate_player_name("Very Long Name", 8, Some(5)),
            "Very ..."
        );

        // Test extreme case - very small width
        assert_eq!(truncator.truncate_player_name("Player", 4, None), "P...");
        assert_eq!(truncator.truncate_player_name("Player", 3, None), "Pla");
        assert_eq!(truncator.truncate_player_name("Player", 2, None), "Pl");
    }

    #[test]
    fn test_intelligent_truncator_goal_types_validation() {
        let truncator = IntelligentTruncator::new();

        // Test goal types that fit
        assert!(truncator.validate_goal_types_no_truncation("YV", 5));
        assert!(truncator.validate_goal_types_no_truncation("YV IM", 6));

        // Test goal types that don't fit
        assert!(!truncator.validate_goal_types_no_truncation("YV IM TM", 6));
        assert!(!truncator.validate_goal_types_no_truncation("Very Long Goal Type", 10));
    }

    #[test]
    fn test_intelligent_truncator_spacing_reduction() {
        let truncator = IntelligentTruncator::new();

        // Test content that fits with optimal spacing
        let (spacing, needs_truncation) = truncator.calculate_spacing_reduction(10, 20, None);
        assert_eq!(spacing, 10);
        assert!(!needs_truncation);

        // Test content that fits with minimal spacing
        let (spacing, needs_truncation) = truncator.calculate_spacing_reduction(10, 11, None);
        assert_eq!(spacing, 1);
        assert!(!needs_truncation);

        // Test content that doesn't fit
        let (spacing, needs_truncation) = truncator.calculate_spacing_reduction(15, 10, None);
        assert_eq!(spacing, 1);
        assert!(needs_truncation);

        // Test with custom minimum spacing that fits
        let (spacing, needs_truncation) = truncator.calculate_spacing_reduction(6, 10, Some(3));
        assert_eq!(spacing, 4); // available_width - content_length = 10 - 6 = 4
        assert!(!needs_truncation);

        // Test with custom minimum spacing that doesn't fit
        let (spacing, needs_truncation) = truncator.calculate_spacing_reduction(8, 10, Some(3));
        assert_eq!(spacing, 3); // Should return min_spacing when truncation needed
        assert!(needs_truncation);
    }

    #[test]
    fn test_intelligent_truncator_strategy_determination() {
        let truncator = IntelligentTruncator::new();

        // Test adequate width
        assert_eq!(
            truncator.determine_truncation_strategy(100, 50),
            TruncationStrategy::NoTruncation
        );

        // Test reduced spacing needed
        assert_eq!(
            truncator.determine_truncation_strategy(65, 50),
            TruncationStrategy::ReduceSpacing
        );

        // Test minimal spacing needed
        assert_eq!(
            truncator.determine_truncation_strategy(58, 50),
            TruncationStrategy::MinimalSpacing
        );

        // Test aggressive truncation needed
        assert_eq!(
            truncator.determine_truncation_strategy(52, 50),
            TruncationStrategy::AggressiveTruncation
        );
    }

    #[test]
    fn test_fallback_layout_with_intelligent_truncation() {
        let mut manager = ColumnLayoutManager::new(45, 2); // Very narrow terminal

        let goal_events = vec![create_test_goal_event(
            "Very Long Player Name That Exceeds Limits",
            vec!["YV".to_string(), "IM".to_string()],
        )];

        let games = vec![create_test_game_data("HIFK", "TPS", goal_events)];
        let layout = manager.create_fallback_layout(&games);

        // Should use reduced team widths
        assert!(layout.home_team_width < 20);
        assert!(layout.away_team_width < 20);

        // Should have reasonable player name width (not zero) - may be reduced in extreme cases
        assert!(layout.max_player_name_width >= 1);
        assert!(layout.max_player_name_width <= 12);

        // Goal types space is preserved during rendering even if layout allocation is reduced

        // All positions should be within terminal bounds
        assert!(layout.score_column < 45);
        assert!(layout.time_column < layout.score_column);
        assert!(layout.play_icon_column < layout.time_column);
    }

    #[test]
    fn test_content_analysis_with_truncation_strategies() {
        let manager = ColumnLayoutManager::new(50, 2); // Narrow terminal
        let truncator = IntelligentTruncator::new();

        let goal_events = vec![create_test_goal_event(
            "Extremely Long Player Name",
            vec!["YV".to_string(), "IM".to_string(), "TM".to_string()],
        )];

        let games = vec![create_test_game_data("HIFK", "TPS", goal_events)];
        let analysis = manager.analyze_content_for_fallback_with_truncation(&games, &truncator);

        // Should apply intelligent limits based on terminal width
        assert!(analysis.max_player_name_width <= 12);
        assert!(analysis.max_player_name_width >= 3);

        // Goal types should be preserved but space-limited
        assert!(analysis.max_goal_types_width >= 2);
        assert!(analysis.max_goal_types_width <= 6);
    }

    // Game Data Validation Tests

    #[test]
    fn test_game_data_validator_valid_game() {
        let validator = GameDataValidator::new();
        let game = create_test_game_data(
            "HIFK",
            "TPS",
            vec![create_test_goal_event("John Doe", vec!["EV".to_string()])],
        );

        let validation = validator.validate_game(&game);

        assert!(validation.is_valid);
        assert!(validation.issues.is_empty());
        assert!(validation.sanitized_game.is_some());
    }

    #[test]
    fn test_game_data_validator_missing_team_names() {
        let validator = GameDataValidator::new();
        let game = create_test_game_data("", "  ", vec![]);

        let validation = validator.validate_game(&game);

        assert!(validation.is_valid); // Should be valid after auto-fix
        assert_eq!(validation.issues.len(), 2); // Two missing team name issues

        let sanitized = validation.sanitized_game.unwrap();
        assert_eq!(sanitized.home_team, "Unknown Team");
        assert_eq!(sanitized.away_team, "Unknown Team");

        // Check issue types
        assert!(validation.issues.iter().any(|issue| issue.issue_type
            == ValidationIssueType::MissingTeamName
            && issue.auto_fixed));
    }

    #[test]
    fn test_game_data_validator_missing_player_names() {
        let validator = GameDataValidator::new();
        let goal_events = vec![
            create_test_goal_event("", vec!["EV".to_string()]),
            create_test_goal_event("  ", vec!["YV".to_string()]),
            create_test_goal_event("Valid Player", vec!["IM".to_string()]),
        ];
        let game = create_test_game_data("HIFK", "TPS", goal_events);

        let validation = validator.validate_game(&game);

        assert!(validation.is_valid);
        let sanitized = validation.sanitized_game.unwrap();

        // First two events should have fallback names
        assert_eq!(sanitized.goal_events[0].scorer_name, "Unknown Player");
        assert_eq!(sanitized.goal_events[1].scorer_name, "Unknown Player");
        assert_eq!(sanitized.goal_events[2].scorer_name, "Valid Player");

        // Should have issues for missing player names
        let player_name_issues: Vec<_> = validation
            .issues
            .iter()
            .filter(|issue| issue.issue_type == ValidationIssueType::MissingPlayerName)
            .collect();
        assert_eq!(player_name_issues.len(), 2);
    }

    #[test]
    fn test_game_data_validator_invalid_goal_types() {
        let validator = GameDataValidator::new();
        let goal_events = vec![
            create_test_goal_event(
                "Player 1",
                vec!["EV".to_string(), "INVALID".to_string(), "YV".to_string()],
            ),
            create_test_goal_event(
                "Player 2",
                vec!["BADTYPE".to_string(), "ANOTHER_BAD".to_string()],
            ),
        ];
        let game = create_test_game_data("HIFK", "TPS", goal_events);

        let validation = validator.validate_game(&game);

        assert!(validation.is_valid);
        let sanitized = validation.sanitized_game.unwrap();

        // First event should keep valid goal types, remove invalid ones
        assert_eq!(sanitized.goal_events[0].goal_types, vec!["EV", "YV"]);
        // Second event should have no goal types (all were invalid)
        assert!(sanitized.goal_events[1].goal_types.is_empty());

        // Should have issues for invalid goal types
        let goal_type_issues: Vec<_> = validation
            .issues
            .iter()
            .filter(|issue| issue.issue_type == ValidationIssueType::InvalidGoalTypes)
            .collect();
        assert_eq!(goal_type_issues.len(), 3); // INVALID, BADTYPE, ANOTHER_BAD
    }

    #[test]
    fn test_game_data_validator_invalid_scores() {
        let validator = GameDataValidator::new();
        let mut game = create_test_game_data("HIFK", "TPS", vec![]);
        game.result = "invalid-score".to_string();

        let validation = validator.validate_game(&game);

        assert!(validation.is_valid); // Should be valid after auto-fix
        let sanitized = validation.sanitized_game.unwrap();
        assert_eq!(sanitized.result, "0-0");

        // Should have issue for invalid score
        assert!(validation.issues.iter().any(|issue| issue.issue_type
            == ValidationIssueType::InvalidScore
            && issue.auto_fixed));
    }

    #[test]
    fn test_game_data_validator_missing_time_and_result() {
        let validator = GameDataValidator::new();
        let mut game = create_test_game_data("HIFK", "TPS", vec![]);
        game.time = "".to_string();
        game.result = "".to_string();

        let validation = validator.validate_game(&game);

        assert!(validation.is_valid); // Should be valid after auto-fix
        let sanitized = validation.sanitized_game.unwrap();
        assert_eq!(sanitized.time, "TBD");

        // Should have issue for missing time info
        assert!(validation.issues.iter().any(|issue| issue.issue_type
            == ValidationIssueType::MissingTimeInfo
            && issue.auto_fixed));
    }

    #[test]
    fn test_game_data_validator_sanitize_games() {
        let validator = GameDataValidator::new();
        let games = vec![
            create_test_game_data("HIFK", "TPS", vec![]), // Valid game
            create_test_game_data("", "", vec![]),        // Missing team names (auto-fixable)
            create_test_game_data(
                "Team A",
                "Team B",
                vec![
                    create_test_goal_event("", vec!["EV".to_string()]), // Missing player name (auto-fixable)
                ],
            ),
        ];

        let sanitized = validator.sanitize_games(&games);

        // All games should be included since all issues are auto-fixable
        assert_eq!(sanitized.len(), 3);

        // Check that fallbacks were applied
        assert_eq!(sanitized[1].home_team, "Unknown Team");
        assert_eq!(sanitized[1].away_team, "Unknown Team");
        assert_eq!(sanitized[2].goal_events[0].scorer_name, "Unknown Player");
    }

    #[test]
    fn test_score_format_validation() {
        let validator = GameDataValidator::new();

        // Valid score formats
        assert!(validator.is_valid_score_format("2-1"));
        assert!(validator.is_valid_score_format("0-0"));
        assert!(validator.is_valid_score_format("10-5"));
        assert!(validator.is_valid_score_format("2-1 ja")); // With overtime
        assert!(validator.is_valid_score_format("3-2 rl")); // With shootout
        assert!(validator.is_valid_score_format("")); // Empty is valid
        assert!(validator.is_valid_score_format("  ")); // Whitespace only is valid

        // Invalid score formats
        assert!(!validator.is_valid_score_format("2"));
        assert!(!validator.is_valid_score_format("2-"));
        assert!(!validator.is_valid_score_format("-1"));
        assert!(!validator.is_valid_score_format("a-b"));
        assert!(!validator.is_valid_score_format("2:1"));
        assert!(!validator.is_valid_score_format("invalid"));
        assert!(!validator.is_valid_score_format("2-1 garbage")); // Trailing unknown suffix
        assert!(!validator.is_valid_score_format("2-1 ja extra")); // Too many tokens
    }

    #[test]
    fn test_layout_calculation_with_validation() {
        let mut manager = ColumnLayoutManager::new(80, 2);

        // Create games with some validation issues
        let games = vec![
            create_test_game_data(
                "HIFK",
                "TPS",
                vec![
                    create_test_goal_event("", vec!["EV".to_string()]), // Missing player name
                ],
            ),
            create_test_game_data("", "Team B", vec![]), // Missing home team name
        ];

        // Layout calculation should handle validation internally
        let layout = manager.calculate_layout(&games);

        // Should still produce a valid layout
        assert!(layout.home_team_width > 0);
        assert!(layout.away_team_width > 0);
        assert!(layout.play_icon_column > 0);
    }
}
