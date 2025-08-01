//! Layout calculation module for dynamic UI sizing
//!
//! This module provides functionality for calculating optimal layouts based on terminal
//! dimensions, determining appropriate detail levels, and managing content positioning.

use crate::constants::dynamic_ui;

/// Represents different levels of detail for content display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailLevel {
    /// Minimal detail for small screens (current behavior)
    Minimal,
    /// Standard detail for medium screens with enhanced information
    Standard,
    /// Extended detail for large screens with full information
    Extended,
}

/// Configuration for layout calculations
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    /// Available width for content display
    pub content_width: u16,
    /// Available height for content display
    pub content_height: u16,
    /// Number of games that can fit per page
    pub games_per_page: usize,
    /// Detail level to use for content rendering
    pub detail_level: DetailLevel,
    /// Horizontal padding on each side
    pub horizontal_padding: u16,
}

/// Positioning information for UI elements
#[derive(Debug, Clone)]
pub struct ContentPositioning {
    /// Y position for header
    pub header_y: u16,
    /// Y position where content starts
    pub content_start_y: u16,
    /// Y position where content ends
    pub content_end_y: u16,
    /// Y position for footer
    pub footer_y: u16,
    /// Left margin for content
    pub left_margin: u16,
    /// Right margin for content
    pub right_margin: u16,
}

/// Handles layout calculations for dynamic UI sizing
#[derive(Debug)]
pub struct LayoutCalculator {
    min_width: u16,
    min_height: u16,
    current_dimensions: (u16, u16),
}

impl LayoutCalculator {
    /// Creates a new LayoutCalculator with default minimum dimensions
    pub fn new() -> Self {
        Self {
            min_width: dynamic_ui::MIN_TERMINAL_WIDTH,
            min_height: dynamic_ui::MIN_TERMINAL_HEIGHT,
            current_dimensions: (
                dynamic_ui::MIN_TERMINAL_WIDTH,
                dynamic_ui::MIN_TERMINAL_HEIGHT,
            ),
        }
    }

    /// Calculates optimal layout configuration for given terminal size
    pub fn calculate_layout(&mut self, terminal_size: (u16, u16)) -> LayoutConfig {
        self.current_dimensions = terminal_size;
        let (width, height) = terminal_size;

        let detail_level = self.determine_detail_level(width);
        let horizontal_padding = self.calculate_horizontal_padding(width);
        let content_width = width.saturating_sub(horizontal_padding * 2);
        let content_height = height.saturating_sub(4); // Reserve space for header/footer
        let games_per_page = self.get_optimal_games_per_page(content_height, detail_level);

        LayoutConfig {
            content_width,
            content_height,
            games_per_page,
            detail_level,
            horizontal_padding,
        }
    }

    /// Determines the appropriate detail level based on available width
    pub fn determine_detail_level(&self, available_width: u16) -> DetailLevel {
        if available_width >= dynamic_ui::EXTENDED_DETAIL_WIDTH_THRESHOLD {
            DetailLevel::Extended
        } else if available_width >= dynamic_ui::STANDARD_DETAIL_WIDTH_THRESHOLD {
            DetailLevel::Standard
        } else {
            DetailLevel::Minimal
        }
    }

    /// Calculates optimal number of games per page based on available height
    pub fn get_optimal_games_per_page(
        &self,
        available_height: u16,
        detail_level: DetailLevel,
    ) -> usize {
        let height_per_game = match detail_level {
            DetailLevel::Minimal => dynamic_ui::BASE_GAME_HEIGHT,
            DetailLevel::Standard => dynamic_ui::BASE_GAME_HEIGHT + 1,
            DetailLevel::Extended => {
                dynamic_ui::BASE_GAME_HEIGHT + dynamic_ui::EXTENDED_GAME_HEIGHT_BONUS
            }
        };

        let max_games = available_height / height_per_game;
        std::cmp::max(1, max_games as usize) // Ensure at least 1 game per page
    }

    /// Calculates horizontal padding based on terminal width
    fn calculate_horizontal_padding(&self, width: u16) -> u16 {
        let max_padding = (width as f32 * dynamic_ui::MAX_HORIZONTAL_PADDING_PERCENT) as u16;
        std::cmp::min(max_padding, 10) // Cap at 10 characters padding
    }

    /// Gets current terminal dimensions
    pub fn current_dimensions(&self) -> (u16, u16) {
        self.current_dimensions
    }

    /// Calculates content positioning for UI elements
    pub fn calculate_content_positioning(
        &self,
        layout_config: &LayoutConfig,
    ) -> ContentPositioning {
        let (_, height) = self.current_dimensions;

        ContentPositioning {
            header_y: 0,
            content_start_y: 2, // Leave space for header and subheader
            content_end_y: height.saturating_sub(2), // Leave space for footer
            footer_y: height.saturating_sub(1),
            left_margin: layout_config.horizontal_padding,
            right_margin: layout_config.horizontal_padding,
        }
    }
}

impl Default for LayoutCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_calculator_creation() {
        let calculator = LayoutCalculator::new();
        assert_eq!(calculator.min_width, dynamic_ui::MIN_TERMINAL_WIDTH);
        assert_eq!(calculator.min_height, dynamic_ui::MIN_TERMINAL_HEIGHT);
    }

    #[test]
    fn test_detail_level_determination() {
        let calculator = LayoutCalculator::new();

        // Test minimal detail level
        assert_eq!(calculator.determine_detail_level(79), DetailLevel::Minimal);
        assert_eq!(calculator.determine_detail_level(80), DetailLevel::Minimal);

        // Test standard detail level
        assert_eq!(
            calculator.determine_detail_level(100),
            DetailLevel::Standard
        );
        assert_eq!(
            calculator.determine_detail_level(119),
            DetailLevel::Standard
        );

        // Test extended detail level
        assert_eq!(
            calculator.determine_detail_level(120),
            DetailLevel::Extended
        );
        assert_eq!(
            calculator.determine_detail_level(200),
            DetailLevel::Extended
        );
    }

    #[test]
    fn test_games_per_page_calculation() {
        let calculator = LayoutCalculator::new();

        // Test minimal detail level
        let games_minimal = calculator.get_optimal_games_per_page(24, DetailLevel::Minimal);
        assert_eq!(games_minimal, 8); // 24 / 3 = 8

        // Test standard detail level
        let games_standard = calculator.get_optimal_games_per_page(24, DetailLevel::Standard);
        assert_eq!(games_standard, 6); // 24 / 4 = 6

        // Test extended detail level
        let games_extended = calculator.get_optimal_games_per_page(24, DetailLevel::Extended);
        assert_eq!(games_extended, 4); // 24 / 5 = 4 (3 + 2 bonus)

        // Test minimum games per page
        let games_small = calculator.get_optimal_games_per_page(2, DetailLevel::Extended);
        assert_eq!(games_small, 1); // Should be at least 1
    }

    #[test]
    fn test_layout_calculation() {
        let mut calculator = LayoutCalculator::new();

        // Test with minimal terminal size
        let layout = calculator.calculate_layout((80, 24));
        assert_eq!(layout.detail_level, DetailLevel::Minimal);
        assert!(layout.content_width <= 80);
        assert!(layout.content_height <= 20); // 24 - 4 for header/footer
        assert!(layout.games_per_page > 0);

        // Test with large terminal size
        let layout_large = calculator.calculate_layout((150, 40));
        assert_eq!(layout_large.detail_level, DetailLevel::Extended);
        assert!(layout_large.content_width <= 150);
        assert!(layout_large.content_height <= 36); // 40 - 4 for header/footer
        assert!(layout_large.games_per_page > 0);
    }

    #[test]
    fn test_horizontal_padding_calculation() {
        let calculator = LayoutCalculator::new();

        // Test padding calculation
        let padding_small = calculator.calculate_horizontal_padding(80);
        assert!(padding_small <= 8); // 10% of 80 = 8

        let padding_large = calculator.calculate_horizontal_padding(200);
        assert_eq!(padding_large, 10); // Capped at 10
    }

    #[test]
    fn test_content_positioning_calculation() {
        let mut calculator = LayoutCalculator::new();
        let layout = calculator.calculate_layout((100, 30));
        let positioning = calculator.calculate_content_positioning(&layout);

        // Test positioning values
        assert_eq!(positioning.header_y, 0);
        assert_eq!(positioning.content_start_y, 2);
        assert_eq!(positioning.content_end_y, 28); // 30 - 2
        assert_eq!(positioning.footer_y, 29); // 30 - 1
        assert_eq!(positioning.left_margin, layout.horizontal_padding);
        assert_eq!(positioning.right_margin, layout.horizontal_padding);
    }

    #[test]
    fn test_content_positioning_with_small_terminal() {
        let mut calculator = LayoutCalculator::new();
        let layout = calculator.calculate_layout((80, 24));
        let positioning = calculator.calculate_content_positioning(&layout);

        // Test positioning values for minimum size
        assert_eq!(positioning.header_y, 0);
        assert_eq!(positioning.content_start_y, 2);
        assert_eq!(positioning.content_end_y, 22); // 24 - 2
        assert_eq!(positioning.footer_y, 23); // 24 - 1
        assert!(positioning.left_margin > 0);
        assert!(positioning.right_margin > 0);
    }

    #[test]
    fn test_various_terminal_size_scenarios() {
        let mut calculator = LayoutCalculator::new();

        // Test very small terminal (edge case)
        let layout_tiny = calculator.calculate_layout((60, 15));
        assert_eq!(layout_tiny.detail_level, DetailLevel::Minimal);
        assert!(layout_tiny.games_per_page >= 1);
        assert!(layout_tiny.content_width > 0);

        // Test medium terminal
        let layout_medium = calculator.calculate_layout((110, 35));
        assert_eq!(layout_medium.detail_level, DetailLevel::Standard);
        assert!(layout_medium.games_per_page > layout_tiny.games_per_page);

        // Test very large terminal
        let layout_large = calculator.calculate_layout((200, 60));
        assert_eq!(layout_large.detail_level, DetailLevel::Extended);
        assert!(layout_large.games_per_page > layout_medium.games_per_page);

        // Test ultra-wide terminal
        let layout_wide = calculator.calculate_layout((300, 30));
        assert_eq!(layout_wide.detail_level, DetailLevel::Extended);
        assert_eq!(layout_wide.horizontal_padding, 10); // Should be capped
    }

    #[test]
    fn test_detail_level_boundary_conditions() {
        let calculator = LayoutCalculator::new();

        // Test exact boundary values
        assert_eq!(calculator.determine_detail_level(99), DetailLevel::Minimal);
        assert_eq!(
            calculator.determine_detail_level(100),
            DetailLevel::Standard
        );
        assert_eq!(
            calculator.determine_detail_level(119),
            DetailLevel::Standard
        );
        assert_eq!(
            calculator.determine_detail_level(120),
            DetailLevel::Extended
        );

        // Test extreme values
        assert_eq!(calculator.determine_detail_level(1), DetailLevel::Minimal);
        assert_eq!(
            calculator.determine_detail_level(u16::MAX),
            DetailLevel::Extended
        );
    }

    #[test]
    fn test_games_per_page_edge_cases() {
        let calculator = LayoutCalculator::new();

        // Test with very small height
        assert_eq!(
            calculator.get_optimal_games_per_page(1, DetailLevel::Minimal),
            1
        );
        assert_eq!(
            calculator.get_optimal_games_per_page(2, DetailLevel::Standard),
            1
        );
        assert_eq!(
            calculator.get_optimal_games_per_page(4, DetailLevel::Extended),
            1
        );

        // Test with zero height (edge case)
        assert_eq!(
            calculator.get_optimal_games_per_page(0, DetailLevel::Minimal),
            1
        );

        // Test with large height
        let large_height_games = calculator.get_optimal_games_per_page(100, DetailLevel::Minimal);
        assert_eq!(large_height_games, 33); // 100 / 3 = 33

        let large_height_extended =
            calculator.get_optimal_games_per_page(100, DetailLevel::Extended);
        assert_eq!(large_height_extended, 20); // 100 / 5 = 20
    }

    #[test]
    fn test_layout_consistency_across_size_changes() {
        let mut calculator = LayoutCalculator::new();

        // Test that layout calculations are consistent
        let layout1 = calculator.calculate_layout((100, 30));
        let layout2 = calculator.calculate_layout((100, 30));

        assert_eq!(layout1.detail_level, layout2.detail_level);
        assert_eq!(layout1.games_per_page, layout2.games_per_page);
        assert_eq!(layout1.content_width, layout2.content_width);
        assert_eq!(layout1.content_height, layout2.content_height);

        // Test that dimensions are properly updated
        calculator.calculate_layout((150, 40));
        assert_eq!(calculator.current_dimensions(), (150, 40));
    }

    #[test]
    fn test_content_width_calculation() {
        let mut calculator = LayoutCalculator::new();

        // Test that content width accounts for padding
        let layout = calculator.calculate_layout((100, 30));
        let expected_content_width = 100 - (layout.horizontal_padding * 2);
        assert_eq!(layout.content_width, expected_content_width);

        // Test with minimum width
        let layout_min = calculator.calculate_layout((80, 24));
        assert!(layout_min.content_width > 0);
        assert!(layout_min.content_width <= 80);
    }

    #[test]
    fn test_content_height_calculation() {
        let mut calculator = LayoutCalculator::new();

        // Test that content height accounts for header/footer
        let layout = calculator.calculate_layout((100, 30));
        assert_eq!(layout.content_height, 26); // 30 - 4 for header/footer

        // Test with minimum height
        let layout_min = calculator.calculate_layout((80, 24));
        assert_eq!(layout_min.content_height, 20); // 24 - 4

        // Test with very small height
        let layout_tiny = calculator.calculate_layout((80, 5));
        assert_eq!(layout_tiny.content_height, 1); // 5 - 4 = 1 (saturating_sub)
    }

    #[test]
    fn test_positioning_edge_cases() {
        let mut calculator = LayoutCalculator::new();

        // Test with minimum terminal size
        let layout_min = calculator.calculate_layout((80, 24));
        let pos_min = calculator.calculate_content_positioning(&layout_min);

        assert!(pos_min.content_start_y < pos_min.content_end_y);
        assert!(pos_min.content_end_y < pos_min.footer_y);
        assert_eq!(pos_min.header_y, 0);

        // Test with very small terminal
        let layout_tiny = calculator.calculate_layout((60, 10));
        let pos_tiny = calculator.calculate_content_positioning(&layout_tiny);

        assert!(pos_tiny.content_start_y <= pos_tiny.content_end_y);
        assert_eq!(pos_tiny.footer_y, 9); // 10 - 1
    }
}
