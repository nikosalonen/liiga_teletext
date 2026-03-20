// src/teletext_ui/layout/config.rs - Layout configuration types and validation

use crate::data_fetcher::GoalEventData;
use crate::data_fetcher::models::GameData;

/// Configuration for column layout calculations
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    /// Width allocated for home team display
    pub home_team_width: usize,
    /// Width of the separator between teams (" - ")
    pub separator_width: usize,
    /// Width allocated for away team display
    pub away_team_width: usize,
    /// Column position for time display
    pub time_column: usize,
    /// Column position for score display
    pub score_column: usize,
    /// Column position for play icon alignment
    pub play_icon_column: usize,
    /// Maximum width needed for player names in goal events
    pub max_player_name_width: usize,
    /// Maximum width needed for goal type indicators (YV, IM, TM, etc.)
    pub max_goal_types_width: usize,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            home_team_width: 20,
            separator_width: 5, // Balanced separator width for better spacing without overflow
            away_team_width: 20,
            time_column: 51,
            score_column: 62,
            play_icon_column: 51,
            max_player_name_width: 17,
            max_goal_types_width: 8,
        }
    }
}

/// Terminal width validation results
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalWidthValidation {
    /// Terminal width is adequate for optimal display
    Adequate { current_width: usize },
    /// Terminal width is suboptimal but usable
    Suboptimal {
        current_width: usize,
        recommended_width: usize,
    },
    /// Terminal width is too narrow for proper display
    TooNarrow {
        current_width: usize,
        minimum_required: usize,
    },
}

/// Results of content analysis for layout calculations
#[derive(Debug, Clone)]
pub(super) struct ContentAnalysis {
    /// Maximum player name width found in the content
    pub(super) max_player_name_width: usize,
    /// Maximum goal types width found in the content
    pub(super) max_goal_types_width: usize,
}

/// Cache key for layout calculations based on terminal configuration and content signature
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct LayoutCacheKey {
    /// Terminal width
    pub(super) terminal_width: usize,
    /// Content margin
    pub(super) content_margin: usize,
    /// Content signature (hash of game data relevant for layout)
    pub(super) content_signature: u64,
    /// Whether this is for wide mode
    pub(super) is_wide_mode: bool,
}

/// Cache key for content analysis based on game data signature
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ContentCacheKey {
    /// Content signature (hash of relevant game data)
    pub(super) content_signature: u64,
    /// Whether this is for fallback analysis
    pub(super) is_fallback: bool,
    /// Terminal width (affects fallback analysis)
    pub(super) terminal_width: Option<usize>,
}

/// Cache statistics for monitoring performance
#[derive(Debug, Clone)]
#[allow(dead_code)] // Used in tests
pub struct CacheStats {
    /// Number of entries in layout cache
    pub layout_cache_size: usize,
    /// Number of entries in content analysis cache
    pub content_analysis_cache_size: usize,
    /// Number of entries in string cache
    pub string_cache_size: usize,
}

/// Cache statistics for alignment calculations
#[derive(Debug, Clone)]
#[allow(dead_code)] // Used in tests
pub struct AlignmentCacheStats {
    /// Number of entries in play icon cache
    pub play_icon_cache_size: usize,
    /// Number of entries in goal type cache
    pub goal_type_cache_size: usize,
}

/// Game data validation results
#[derive(Debug, Clone)]
pub struct GameDataValidation {
    /// Whether the game data is valid for layout calculations
    #[allow(dead_code)] // Read in tests
    pub is_valid: bool,
    /// List of validation issues found
    pub issues: Vec<ValidationIssue>,
    /// Sanitized game data with fallbacks applied
    pub sanitized_game: Option<GameData>,
}

/// Individual validation issue
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Type of validation issue
    pub issue_type: ValidationIssueType,
    /// Human-readable description of the issue
    #[allow(dead_code)] // Read in tests
    pub description: String,
    /// Whether this issue was automatically fixed
    pub auto_fixed: bool,
}

/// Types of validation issues that can occur
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationIssueType {
    /// Missing or empty team name
    MissingTeamName,
    /// Missing or empty player name in goal events
    MissingPlayerName,
    /// Invalid goal event data
    InvalidGoalEvent,
    /// Missing time information
    MissingTimeInfo,
    /// Invalid score format
    InvalidScore,
    /// Missing or invalid goal types
    InvalidGoalTypes,
}

/// Game data validator for ensuring data integrity before layout calculations
#[derive(Debug)]
pub struct GameDataValidator;

impl GameDataValidator {
    /// Creates a new game data validator
    pub fn new() -> Self {
        Self
    }

    /// Validates a single game's data and returns validation results with sanitized data
    ///
    /// # Arguments
    /// * `game` - The game data to validate
    ///
    /// # Returns
    /// * `GameDataValidation` - Validation results with sanitized data if fixable
    pub fn validate_game(&self, game: &GameData) -> GameDataValidation {
        let mut issues = Vec::new();
        let mut sanitized_game = game.clone();
        let mut is_valid = true;

        // Validate team names
        if game.home_team.trim().is_empty() {
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::MissingTeamName,
                description: "Home team name is missing or empty".to_string(),
                auto_fixed: true,
            });
            sanitized_game.home_team = "Unknown Team".to_string();
            tracing::warn!("Missing home team name, using fallback: 'Unknown Team'");
        }

        if game.away_team.trim().is_empty() {
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::MissingTeamName,
                description: "Away team name is missing or empty".to_string(),
                auto_fixed: true,
            });
            sanitized_game.away_team = "Unknown Team".to_string();
            tracing::warn!("Missing away team name, using fallback: 'Unknown Team'");
        }

        // Validate time information
        if game.time.trim().is_empty() && game.result.trim().is_empty() {
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::MissingTimeInfo,
                description: "Both time and result are missing".to_string(),
                auto_fixed: true,
            });
            sanitized_game.time = "TBD".to_string();
            tracing::warn!("Missing time and result information, using fallback time: 'TBD'");
        }

        // Validate score format for finished games
        if !game.result.trim().is_empty() && !self.is_valid_score_format(&game.result) {
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::InvalidScore,
                description: format!("Invalid score format: '{}'", game.result),
                auto_fixed: true,
            });
            sanitized_game.result = "0-0".to_string();
            tracing::warn!(
                "Invalid score format '{}', using fallback: '0-0'",
                game.result
            );
        }

        // Validate goal events
        let mut sanitized_goal_events = Vec::new();
        for (index, event) in game.goal_events.iter().enumerate() {
            let (sanitized_event, goal_issues) = self.validate_goal_event(event, index);
            issues.extend(goal_issues);
            sanitized_goal_events.push(sanitized_event);
        }
        sanitized_game.goal_events = sanitized_goal_events;

        // Determine overall validity
        let critical_issues = issues.iter().any(|issue| {
            matches!(
                issue.issue_type,
                ValidationIssueType::MissingTeamName | ValidationIssueType::MissingTimeInfo
            ) && !issue.auto_fixed
        });

        if critical_issues {
            is_valid = false;
            tracing::error!("Game data has critical validation issues that cannot be auto-fixed");
        }

        GameDataValidation {
            is_valid,
            issues,
            sanitized_game: if is_valid { Some(sanitized_game) } else { None },
        }
    }

    /// Validates multiple games and returns validation results
    ///
    /// # Arguments
    /// * `games` - Slice of game data to validate
    ///
    /// # Returns
    /// * `Vec<GameDataValidation>` - Validation results for each game
    pub fn validate_games(&self, games: &[GameData]) -> Vec<GameDataValidation> {
        games.iter().map(|game| self.validate_game(game)).collect()
    }

    /// Validates and sanitizes games, returning only valid games with fallbacks applied
    ///
    /// # Arguments
    /// * `games` - Slice of game data to validate and sanitize
    ///
    /// # Returns
    /// * `Vec<GameData>` - Vector of valid, sanitized game data
    pub fn sanitize_games(&self, games: &[GameData]) -> Vec<GameData> {
        let validations = self.validate_games(games);
        let mut sanitized_games = Vec::new();
        let mut excluded_count = 0;

        for (index, validation) in validations.into_iter().enumerate() {
            if let Some(sanitized_game) = validation.sanitized_game {
                sanitized_games.push(sanitized_game);

                // Log validation issues if any
                if !validation.issues.is_empty() {
                    tracing::debug!(
                        "Game at index {} had {} validation issues (all auto-fixed)",
                        index,
                        validation.issues.len()
                    );
                }
            } else {
                excluded_count += 1;
                tracing::warn!(
                    "Excluding game at index {} from layout calculation due to validation failures: {:?}",
                    index,
                    validation.issues
                );
            }
        }

        if excluded_count > 0 {
            tracing::warn!(
                "Excluded {} games from layout calculation due to validation failures. {} games remain.",
                excluded_count,
                sanitized_games.len()
            );
        }

        sanitized_games
    }

    /// Validates a single goal event
    ///
    /// # Arguments
    /// * `event` - The goal event to validate
    /// * `index` - Index of the event for logging purposes
    ///
    /// # Returns
    /// * `(GoalEventData, Vec<ValidationIssue>)` - Sanitized event and any validation issues found
    fn validate_goal_event(
        &self,
        event: &GoalEventData,
        index: usize,
    ) -> (GoalEventData, Vec<ValidationIssue>) {
        let mut issues = Vec::new();
        let mut sanitized_event = event.clone();

        // Validate player name (requirement: handle missing player names gracefully)
        if event.scorer_name.trim().is_empty() {
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::MissingPlayerName,
                description: format!("Goal event at index {} has missing player name", index),
                auto_fixed: true,
            });
            sanitized_event.scorer_name = "Unknown Player".to_string();
            tracing::debug!(
                "Goal event at index {} missing player name, using fallback: 'Unknown Player'",
                index
            );
        }

        // Validate goal minute
        if event.minute < 0 || event.minute > 200 {
            // Allow up to 200 minutes for extreme overtime
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::InvalidGoalEvent,
                description: format!(
                    "Goal event at index {} has invalid minute: {}",
                    index, event.minute
                ),
                auto_fixed: true,
            });
            sanitized_event.minute = 0;
            tracing::debug!(
                "Goal event at index {} has invalid minute {}, using fallback: 0",
                index,
                event.minute
            );
        }

        // Validate goal types with enhanced safe fallbacks (requirement 4.1)
        let valid_goal_types = ["EV", "YV", "YV2", "IM", "VT", "AV", "TM", "VL", "MV", "RV"];
        let mut sanitized_goal_types = Vec::new();

        for goal_type in &event.goal_types {
            // Handle null, empty, or whitespace-only goal types safely
            let goal_type_trimmed = goal_type.trim();
            if goal_type_trimmed.is_empty() {
                issues.push(ValidationIssue {
                    issue_type: ValidationIssueType::InvalidGoalTypes,
                    description: format!("Goal event at index {} has empty goal type", index),
                    auto_fixed: true,
                });
                tracing::debug!(
                    "Goal event at index {} has empty goal type, excluding from display",
                    index
                );
                continue;
            }

            // Validate against known goal types
            if valid_goal_types.contains(&goal_type_trimmed) {
                // Avoid duplicates in sanitized list
                if !sanitized_goal_types.contains(&goal_type_trimmed.to_string()) {
                    sanitized_goal_types.push(goal_type_trimmed.to_string());
                }
            } else {
                issues.push(ValidationIssue {
                    issue_type: ValidationIssueType::InvalidGoalTypes,
                    description: format!(
                        "Goal event at index {} has invalid goal type: '{}'",
                        index, goal_type_trimmed
                    ),
                    auto_fixed: true,
                });
                tracing::debug!(
                    "Goal event at index {} has invalid goal type '{}', excluding from display",
                    index,
                    goal_type_trimmed
                );
            }
        }

        // Always assign sanitized goal types, even if empty (safe fallback)
        sanitized_event.goal_types = sanitized_goal_types;

        // Validate scores (should be non-negative)
        if event.home_team_score < 0 {
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::InvalidGoalEvent,
                description: format!(
                    "Goal event at index {} has negative home team score: {}",
                    index, event.home_team_score
                ),
                auto_fixed: true,
            });
            sanitized_event.home_team_score = 0;
            tracing::debug!(
                "Goal event at index {} has negative home team score, using fallback: 0",
                index
            );
        }

        if event.away_team_score < 0 {
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::InvalidGoalEvent,
                description: format!(
                    "Goal event at index {} has negative away team score: {}",
                    index, event.away_team_score
                ),
                auto_fixed: true,
            });
            sanitized_event.away_team_score = 0;
            tracing::debug!(
                "Goal event at index {} has negative away team score, using fallback: 0",
                index
            );
        }

        (sanitized_event, issues)
    }

    /// Validates score format (should be in format like "2-1", "0-0", etc.)
    ///
    /// # Arguments
    /// * `score` - The score string to validate
    ///
    /// # Returns
    /// * `bool` - True if the score format is valid
    // pub(super) for test access from mod.rs tests
    pub(super) fn is_valid_score_format(&self, score: &str) -> bool {
        let score = score.trim();

        // Allow empty scores
        if score.is_empty() {
            return true;
        }

        // Check for basic score format: number-number (with optional suffixes like "ja", "rl")
        let parts: Vec<&str> = score.split_whitespace().collect();
        if parts.is_empty() {
            return false;
        }

        let score_part = parts[0];
        if let Some(dash_pos) = score_part.find('-') {
            let home_score = &score_part[..dash_pos];
            let away_score = &score_part[dash_pos + 1..];

            // Both parts should be valid numbers
            home_score.parse::<u32>().is_ok() && away_score.parse::<u32>().is_ok()
        } else {
            false
        }
    }
}

impl Default for GameDataValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Intelligent truncation utilities for handling extreme content cases
#[derive(Debug, Default)]
pub struct IntelligentTruncator;

impl IntelligentTruncator {
    /// Creates a new IntelligentTruncator
    pub fn new() -> Self {
        Self
    }

    /// Intelligently truncates a player name to fit within available space
    /// Only uses ellipsis as a last resort (Requirement 3.2)
    ///
    /// # Arguments
    /// * `player_name` - The original player name
    /// * `max_width` - Maximum width available for the player name
    /// * `preserve_critical_chars` - Minimum characters to preserve (default: 5)
    ///
    /// # Returns
    /// * `String` - Truncated player name with ellipsis if necessary
    pub fn truncate_player_name(
        &self,
        player_name: &str,
        max_width: usize,
        preserve_critical_chars: Option<usize>,
    ) -> String {
        let preserve_chars = preserve_critical_chars.unwrap_or(5);

        // If name fits within max width, return as-is
        if player_name.len() <= max_width {
            tracing::debug!(
                "Player name '{}' (length: {}) fits within max_width: {}",
                player_name,
                player_name.len(),
                max_width
            );
            return player_name.to_string();
        }

        tracing::warn!(
            "Player name truncation required: '{}' (length: {}) exceeds max_width: {}",
            player_name,
            player_name.len(),
            max_width
        );

        // If max_width is too small to preserve critical information, use minimum viable truncation
        if max_width < preserve_chars + 3 {
            // 3 for "..."
            if max_width >= 4 {
                // Use first character + "..."
                let result = format!("{}...", player_name.chars().next().unwrap_or('?'));
                tracing::warn!(
                    "Extreme truncation with ellipsis: '{}' -> '{}' (max_width: {})",
                    player_name,
                    result,
                    max_width
                );
                return result;
            } else {
                // Extreme case: just use first few characters without ellipsis
                let result: String = player_name.chars().take(max_width).collect();
                tracing::error!(
                    "Critical truncation without ellipsis: '{}' -> '{}' (max_width: {} too small for ellipsis)",
                    player_name,
                    result,
                    max_width
                );
                return result;
            }
        }

        // Standard truncation with ellipsis (last resort per requirement 3.2)
        let truncate_to = max_width.saturating_sub(3); // Reserve 3 chars for "..."
        let truncated: String = player_name.chars().take(truncate_to).collect();

        // Ensure we preserve at least the minimum critical characters
        if truncated.len() >= preserve_chars {
            let result = format!("{}...", truncated);
            tracing::warn!(
                "Standard truncation with ellipsis: '{}' -> '{}' (preserved {} chars)",
                player_name,
                result,
                truncated.len()
            );
            result
        } else {
            // If we can't preserve enough characters, use the full available width
            let result: String = player_name.chars().take(max_width).collect();
            tracing::warn!(
                "Fallback truncation without ellipsis: '{}' -> '{}' (couldn't preserve {} critical chars)",
                player_name,
                result,
                preserve_chars
            );
            result
        }
    }

    /// Validates that goal types can be displayed without truncation
    /// Goal types should never be truncated (Requirement 3.4)
    ///
    /// # Arguments
    /// * `goal_types` - The goal types string to validate
    /// * `available_width` - Available width for goal types
    ///
    /// # Returns
    /// * `bool` - True if goal types fit, false if they would need truncation
    pub fn validate_goal_types_no_truncation(
        &self,
        goal_types: &str,
        available_width: usize,
    ) -> bool {
        let fits = goal_types.len() <= available_width;

        if !fits {
            tracing::error!(
                "Goal types validation failed: '{}' (length: {}) exceeds available_width: {}. Goal types should never be truncated per requirement 3.4",
                goal_types,
                goal_types.len(),
                available_width
            );
        } else {
            tracing::debug!(
                "Goal types validation passed: '{}' (length: {}) fits within available_width: {}",
                goal_types,
                goal_types.len(),
                available_width
            );
        }

        fits
    }

    /// Calculates optimal spacing reduction to avoid truncation
    /// Reduces spacing before resorting to truncation
    ///
    /// # Arguments
    /// * `content_length` - Length of content that needs to fit
    /// * `available_width` - Total available width
    /// * `min_spacing` - Minimum spacing to maintain (default: 1)
    ///
    /// # Returns
    /// * `(usize, bool)` - (optimal_spacing, needs_truncation)
    #[allow(dead_code)] // Used in tests
    pub fn calculate_spacing_reduction(
        &self,
        content_length: usize,
        available_width: usize,
        min_spacing: Option<usize>,
    ) -> (usize, bool) {
        let min_space = min_spacing.unwrap_or(1);

        if content_length + min_space <= available_width {
            // Content fits with minimum spacing
            let optimal_spacing = available_width - content_length;
            (optimal_spacing, false)
        } else {
            // Content doesn't fit even with minimum spacing - truncation needed
            (min_space, true)
        }
    }

    /// Handles extreme terminal width cases with intelligent fallbacks
    /// Ensures critical information remains visible even in very narrow terminals
    ///
    /// # Arguments
    /// * `terminal_width` - Current terminal width
    /// * `critical_content_width` - Width needed for critical content
    ///
    /// # Returns
    /// * `TruncationStrategy` - Strategy to use for this terminal width
    pub fn determine_truncation_strategy(
        &self,
        terminal_width: usize,
        critical_content_width: usize,
    ) -> TruncationStrategy {
        let strategy = if terminal_width >= critical_content_width + 20 {
            TruncationStrategy::NoTruncation
        } else if terminal_width >= critical_content_width + 10 {
            TruncationStrategy::ReduceSpacing
        } else if terminal_width >= critical_content_width + 5 {
            TruncationStrategy::MinimalSpacing
        } else {
            TruncationStrategy::AggressiveTruncation
        };

        tracing::debug!(
            "Determined truncation strategy: {:?} (terminal_width: {}, critical_content_width: {}, available_extra: {})",
            strategy,
            terminal_width,
            critical_content_width,
            terminal_width.saturating_sub(critical_content_width)
        );

        match strategy {
            TruncationStrategy::NoTruncation => {
                tracing::debug!("No truncation needed - sufficient space available");
            }
            TruncationStrategy::ReduceSpacing => {
                tracing::info!("Reducing spacing to fit content within terminal width");
            }
            TruncationStrategy::MinimalSpacing => {
                tracing::warn!("Using minimal spacing - layout will be cramped");
            }
            TruncationStrategy::AggressiveTruncation => {
                tracing::error!(
                    "Aggressive truncation required - some content may be severely limited"
                );
            }
        }

        strategy
    }
}

/// Strategy for handling content that doesn't fit in available space
#[derive(Debug, Clone, PartialEq)]
pub enum TruncationStrategy {
    /// No truncation needed - content fits comfortably
    NoTruncation,
    /// Reduce spacing between elements but don't truncate content
    ReduceSpacing,
    /// Use minimal spacing (1 character) between elements
    MinimalSpacing,
    /// Aggressively truncate non-critical content to preserve critical information
    AggressiveTruncation,
}
