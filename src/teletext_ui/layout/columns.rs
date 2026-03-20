// src/teletext_ui/layout/columns.rs - Column width calculations, alignment, and positioning

use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

use lru::LruCache;

use crate::data_fetcher::GoalEventData;
use crate::data_fetcher::models::GameData;

use super::config::{AlignmentCacheStats, LayoutConfig};

const ALIGNMENT_CACHE_CAPACITY: usize = 50;

/// Generates a content signature for caching purposes
/// This creates a hash based on the layout-relevant aspects of game data
pub(super) fn generate_content_signature(games: &[GameData]) -> u64 {
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();

    // Hash the number of games
    games.len().hash(&mut hasher);

    // Hash layout-relevant data from each game
    for game in games {
        // Hash team names (affects display width)
        game.home_team.hash(&mut hasher);
        game.away_team.hash(&mut hasher);

        // Hash goal events data that affects layout
        game.goal_events.len().hash(&mut hasher);
        for event in &game.goal_events {
            // Hash player name length (affects layout calculations)
            event.scorer_name.len().hash(&mut hasher);
            // Hash goal types (affects layout calculations)
            event.goal_types.hash(&mut hasher);
            // Hash video link presence (affects play icon positioning)
            event.video_clip_url.is_some().hash(&mut hasher);
        }
    }

    hasher.finish()
}

/// Generates a signature for goal events for caching purposes
#[allow(dead_code)] // Used in tests via AlignmentCalculator
fn generate_events_signature(events: &[GoalEventData]) -> u64 {
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();

    events.len().hash(&mut hasher);
    for event in events {
        event.scorer_name.len().hash(&mut hasher);
        event.goal_types.hash(&mut hasher);
        event.video_clip_url.is_some().hash(&mut hasher);
        event.minute.hash(&mut hasher);
    }

    hasher.finish()
}

/// Generates a signature for layout configuration for caching purposes
#[allow(dead_code)] // Used in tests via AlignmentCalculator
fn generate_layout_signature(layout: &LayoutConfig) -> u64 {
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();

    layout.play_icon_column.hash(&mut hasher);
    layout.max_player_name_width.hash(&mut hasher);
    layout.max_goal_types_width.hash(&mut hasher);

    hasher.finish()
}

/// Position tracking for play icons
#[derive(Debug, Clone)]
#[allow(dead_code)] // Used in tests
pub struct PlayIconPosition {
    /// Index of the game this position relates to
    pub game_index: usize,
    /// Index of the goal event within the game
    pub event_index: usize,
    /// Column position for the play icon
    pub column_position: usize,
    /// Whether this event has a video link
    pub has_video_link: bool,
}

/// Position tracking for goal type indicators
#[derive(Debug, Clone)]
#[allow(dead_code)] // Used in tests
pub struct GoalTypePosition {
    /// Index of the goal event
    pub event_index: usize,
    /// Column position for goal type display
    pub column_position: usize,
    /// Goal types string to display
    pub goal_types: String,
    /// Available width for goal types
    pub available_width: usize,
}

/// Cache key for play icon position calculations
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(dead_code)] // Used in tests via AlignmentCalculator
struct PlayIconCacheKey {
    /// Content signature of games
    content_signature: u64,
    /// Play icon column position
    play_icon_column: usize,
}

/// Cache key for goal type position calculations
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(dead_code)] // Used in tests via AlignmentCalculator
struct GoalTypeCacheKey {
    /// Content signature of events
    events_signature: u64,
    /// Layout configuration signature
    layout_signature: u64,
}

/// Calculates alignment positions for play icons and goal type indicators
#[derive(Debug)]
#[allow(dead_code)] // Used in tests
pub struct AlignmentCalculator {
    /// LRU cache for play icon position calculations
    play_icon_cache: LruCache<PlayIconCacheKey, Vec<PlayIconPosition>>,
    /// LRU cache for goal type position calculations
    goal_type_cache: LruCache<GoalTypeCacheKey, Vec<GoalTypePosition>>,
}

#[allow(dead_code)] // Used in tests
impl AlignmentCalculator {
    /// Creates a new AlignmentCalculator
    pub fn new() -> Self {
        let cap = NonZeroUsize::new(ALIGNMENT_CACHE_CAPACITY).unwrap();
        Self {
            play_icon_cache: LruCache::new(cap),
            goal_type_cache: LruCache::new(cap),
        }
    }

    /// Clears all caches to free memory
    pub fn clear_caches(&mut self) {
        let play_icon_cache_size = self.play_icon_cache.len();
        let goal_type_cache_size = self.goal_type_cache.len();

        self.play_icon_cache.clear();
        self.goal_type_cache.clear();

        tracing::debug!(
            "Cleared alignment caches: {} play icon entries, {} goal type entries",
            play_icon_cache_size,
            goal_type_cache_size
        );
    }

    /// Gets cache statistics for monitoring performance
    pub fn get_cache_stats(&self) -> AlignmentCacheStats {
        AlignmentCacheStats {
            play_icon_cache_size: self.play_icon_cache.len(),
            goal_type_cache_size: self.goal_type_cache.len(),
        }
    }

    /// Calculates play icon positions for consistent vertical alignment
    /// Uses caching to optimize repeated calculations (requirement 4.3)
    ///
    /// # Arguments
    /// * `games` - Slice of game data containing goal events
    /// * `layout` - Layout configuration with column positions
    ///
    /// # Returns
    /// * `Vec<PlayIconPosition>` - Vector of calculated play icon positions
    pub fn calculate_play_icon_positions(
        &mut self,
        games: &[GameData],
        layout: &LayoutConfig,
    ) -> Vec<PlayIconPosition> {
        // Check cache first (requirement 4.3)
        let content_signature = generate_content_signature(games);
        let cache_key = PlayIconCacheKey {
            content_signature,
            play_icon_column: layout.play_icon_column,
        };

        if let Some(cached_positions) = self.play_icon_cache.get(&cache_key) {
            tracing::debug!(
                "Play icon positions cache hit for signature {}, returning {} cached positions",
                content_signature,
                cached_positions.len()
            );
            return cached_positions.clone();
        }

        tracing::debug!(
            "Play icon positions cache miss for signature {}, performing calculation",
            content_signature
        );

        let mut positions = Vec::new();
        let mut total_events = 0;
        let mut video_link_count = 0;

        for (game_index, game) in games.iter().enumerate() {
            for (event_index, event) in game.goal_events.iter().enumerate() {
                total_events += 1;
                let has_video_link = event.video_clip_url.is_some();

                if has_video_link {
                    video_link_count += 1;
                }

                positions.push(PlayIconPosition {
                    game_index,
                    event_index,
                    column_position: layout.play_icon_column,
                    has_video_link,
                });
            }
        }

        tracing::debug!(
            "Calculated {} play icon positions at column {} ({} with video links)",
            total_events,
            layout.play_icon_column,
            video_link_count
        );

        if layout.play_icon_column > 60 {
            tracing::warn!(
                "Play icon column position {} may be too far right for optimal display",
                layout.play_icon_column
            );
        }

        // Cache the calculated positions (LRU eviction handles size bounds)
        self.play_icon_cache.put(cache_key, positions.clone());

        positions
    }

    /// Calculates goal type positions ensuring no overflow into away team area
    /// Uses caching to optimize repeated calculations (requirement 4.3)
    ///
    /// # Arguments
    /// * `events` - Slice of goal event data
    /// * `layout` - Layout configuration with column positions
    ///
    /// # Returns
    /// * `Vec<GoalTypePosition>` - Vector of calculated goal type positions
    pub fn calculate_goal_type_positions(
        &mut self,
        events: &[GoalEventData],
        layout: &LayoutConfig,
    ) -> Vec<GoalTypePosition> {
        // Check cache first (requirement 4.3)
        let events_signature = generate_events_signature(events);
        let layout_signature = generate_layout_signature(layout);
        let cache_key = GoalTypeCacheKey {
            events_signature,
            layout_signature,
        };

        if let Some(cached_positions) = self.goal_type_cache.get(&cache_key) {
            tracing::debug!(
                "Goal type positions cache hit for events signature {}, returning {} cached positions",
                events_signature,
                cached_positions.len()
            );
            return cached_positions.clone();
        }

        tracing::debug!(
            "Goal type positions cache miss for events signature {}, performing calculation",
            events_signature
        );

        let mut positions = Vec::new();
        let mut overflow_adjustments = 0;
        let mut max_goal_types_length = 0;

        for (event_index, event) in events.iter().enumerate() {
            let goal_types = event.get_goal_type_display();
            let available_width = layout.max_goal_types_width;

            max_goal_types_length = max_goal_types_length.max(goal_types.len());

            // Calculate position ensuring no overflow past the away team area boundary
            let away_area_end =
                layout.home_team_width + layout.separator_width + layout.away_team_width - 2;
            let max_allowed_column = away_area_end.saturating_sub(goal_types.len());
            let column_position = layout.play_icon_column + layout.max_player_name_width + 1;
            let safe_column_position = column_position.min(max_allowed_column);

            if safe_column_position < column_position {
                overflow_adjustments += 1;
                tracing::debug!(
                    "Goal type position adjusted to prevent overflow: '{}' moved from column {} to {}",
                    goal_types,
                    column_position,
                    safe_column_position
                );
            }

            positions.push(GoalTypePosition {
                event_index,
                column_position: safe_column_position,
                goal_types,
                available_width,
            });
        }

        tracing::debug!(
            "Calculated {} goal type positions, {} required overflow adjustments, max goal types length: {}",
            events.len(),
            overflow_adjustments,
            max_goal_types_length
        );

        if overflow_adjustments > 0 {
            tracing::warn!(
                "{} goal type positions required adjustment to prevent overflow into away team area",
                overflow_adjustments
            );
        }

        if max_goal_types_length > layout.max_goal_types_width {
            tracing::warn!(
                "Some goal types (max length: {}) exceed allocated width ({}). May cause display issues.",
                max_goal_types_length,
                layout.max_goal_types_width
            );
        }

        // Cache the calculated positions (LRU eviction handles size bounds)
        self.goal_type_cache.put(cache_key, positions.clone());

        positions
    }

    /// Gets the consistent column position for play icons
    ///
    /// # Arguments
    /// * `layout` - Layout configuration
    ///
    /// # Returns
    /// * `usize` - Column position for play icon alignment
    pub fn get_play_icon_column_position(&self, layout: &LayoutConfig) -> usize {
        layout.play_icon_column
    }

    /// Validates that a goal type position doesn't overflow into away team area
    ///
    /// # Arguments
    /// * `position` - Goal type position to validate
    /// * `layout` - Layout configuration used to derive column boundaries
    ///
    /// # Returns
    /// * `bool` - True if position is safe, false if it would overflow
    pub fn validate_no_overflow(&self, position: &GoalTypePosition, layout: &LayoutConfig) -> bool {
        let away_area_end =
            layout.home_team_width + layout.separator_width + layout.away_team_width - 2;
        let end_position = position.column_position + position.goal_types.len();
        let is_safe = end_position <= away_area_end;

        if !is_safe {
            tracing::error!(
                "Goal type overflow detected: '{}' at column {} would end at column {} (exceeds limit of {})",
                position.goal_types,
                position.column_position,
                end_position,
                away_area_end
            );
        } else {
            tracing::debug!(
                "Goal type position validated: '{}' at column {} ends at column {} (within limit of {})",
                position.goal_types,
                position.column_position,
                end_position,
                away_area_end
            );
        }

        is_safe
    }
}

impl Default for AlignmentCalculator {
    fn default() -> Self {
        Self::new()
    }
}
