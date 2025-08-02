//! Backward compatibility tests for dynamic UI feature
//!
//! These tests ensure that the dynamic UI enhancements don't break existing functionality
//! and that the behavior with minimum terminal sizes matches the original implementation.

use liiga_teletext::{
    constants::dynamic_ui,
    data_fetcher::models::*,
    teletext_ui::{GameResultData, ScoreType, TeletextPage},
    ui::layout::{DetailLevel, LayoutCalculator},
};

/// Helper function to create mock game data for testing
fn create_mock_game_data(home_team: &str, away_team: &str, result: &str) -> GameData {
    GameData {
        home_team: home_team.to_string(),
        away_team: away_team.to_string(),
        time: "18:30".to_string(),
        result: result.to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        serie: "runkosarja".to_string(),
        goal_events: vec![],
        played_time: 3900,
        start: "2024-01-15T18:30:00Z".to_string(),
    }
}

#[test]
fn test_minimum_terminal_size_behavior() {
    let mut calculator = LayoutCalculator::new();

    // Test with exact minimum terminal size (80x24)
    let layout = calculator.calculate_layout((
        dynamic_ui::MIN_TERMINAL_WIDTH,
        dynamic_ui::MIN_TERMINAL_HEIGHT,
    ));

    // Should use minimal detail level
    assert_eq!(layout.detail_level, DetailLevel::Minimal);

    // Should not be in emergency mode
    assert!(!layout.is_emergency_mode);

    // Should have reasonable content dimensions
    assert!(layout.content_width > 0);
    assert!(layout.content_height > 0);
    assert!(layout.games_per_page > 0);

    // Content width should be less than terminal width (accounting for padding)
    assert!(layout.content_width <= dynamic_ui::MIN_TERMINAL_WIDTH);

    // Content height should be less than terminal height (accounting for header/footer)
    assert!(layout.content_height <= dynamic_ui::MIN_TERMINAL_HEIGHT - 4);
}

#[test]
fn test_teletext_page_creation_compatibility() {
    // Test that TeletextPage can still be created with the same interface
    let games = vec![
        create_mock_game_data("TPS", "HIFK", "2-1"),
        create_mock_game_data("Ilves", "Lukko", "0-3"),
    ];

    let game_results: Vec<GameResultData> = games
        .into_iter()
        .map(|game| GameResultData {
            home_team: game.home_team,
            away_team: game.away_team,
            time: game.time,
            result: game.result,
            score_type: game.score_type,
            is_overtime: game.is_overtime,
            is_shootout: game.is_shootout,
            goal_events: game.goal_events,
            played_time: game.played_time,
        })
        .collect();

    // Create TeletextPage with correct interface
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        false, // ignore_height_limit
    );

    // Add the games to the page
    for game_result in game_results {
        page.add_game_result(game_result);
    }

    // Should be able to set current page
    page.set_current_page(0);
    assert_eq!(page.get_current_page(), 0);

    // Should be able to get total pages
    assert!(page.total_pages() > 0);

    // Should be able to navigate pages
    if page.total_pages() > 1 {
        page.next_page();
        assert_eq!(page.get_current_page(), 1);

        page.previous_page();
        assert_eq!(page.get_current_page(), 0);
    }
}

#[test]
fn test_layout_calculation_consistency() {
    let mut calculator = LayoutCalculator::new();

    // Test that repeated calculations with same size give same results
    let size = (100, 30);
    let layout1 = calculator.calculate_layout(size);
    let layout2 = calculator.calculate_layout(size);

    assert_eq!(layout1.content_width, layout2.content_width);
    assert_eq!(layout1.content_height, layout2.content_height);
    assert_eq!(layout1.games_per_page, layout2.games_per_page);
    assert_eq!(layout1.detail_level, layout2.detail_level);
    assert_eq!(layout1.horizontal_padding, layout2.horizontal_padding);
    assert_eq!(layout1.is_emergency_mode, layout2.is_emergency_mode);
}

#[test]
fn test_detail_level_thresholds() {
    let calculator = LayoutCalculator::new();

    // Test that detail level thresholds work as expected
    assert_eq!(
        calculator.determine_detail_level(dynamic_ui::MIN_TERMINAL_WIDTH),
        DetailLevel::Minimal
    );

    assert_eq!(
        calculator.determine_detail_level(dynamic_ui::STANDARD_DETAIL_WIDTH_THRESHOLD),
        DetailLevel::Standard
    );

    assert_eq!(
        calculator.determine_detail_level(dynamic_ui::EXTENDED_DETAIL_WIDTH_THRESHOLD),
        DetailLevel::Extended
    );
}

#[test]
fn test_games_per_page_calculation() {
    let calculator = LayoutCalculator::new();

    // Test that games per page calculation is reasonable
    let minimal_games = calculator.get_optimal_games_per_page(20, DetailLevel::Minimal);
    let standard_games = calculator.get_optimal_games_per_page(20, DetailLevel::Standard);
    let extended_games = calculator.get_optimal_games_per_page(20, DetailLevel::Extended);

    // Extended should show fewer games per page than standard, which should show fewer than minimal
    assert!(extended_games <= standard_games);
    assert!(standard_games <= minimal_games);

    // All should be at least 1
    assert!(minimal_games >= 1);
    assert!(standard_games >= 1);
    assert!(extended_games >= 1);
}

#[test]
fn test_emergency_mode_activation() {
    let mut calculator = LayoutCalculator::new();

    // Test with terminal size below minimum
    let layout = calculator.calculate_layout((60, 15));

    // Should activate emergency mode
    assert!(layout.is_emergency_mode);

    // Should have a degradation warning
    assert!(layout.degradation_warning.is_some());

    // Should still provide usable layout
    assert!(layout.content_width > 0);
    assert!(layout.content_height > 0);
    assert!(layout.games_per_page >= 1);

    // Should use minimal detail level
    assert_eq!(layout.detail_level, DetailLevel::Minimal);
}

#[test]
fn test_content_positioning_calculation() {
    let mut calculator = LayoutCalculator::new();
    let layout = calculator.calculate_layout((100, 30));
    let positioning = calculator.calculate_content_positioning(&layout);

    // Test that positioning values are reasonable
    assert_eq!(positioning.header_y, 0);
    assert!(positioning.content_start_y > positioning.header_y);
    assert!(positioning.content_end_y > positioning.content_start_y);
    assert!(positioning.footer_y > positioning.content_end_y);

    // Test that margins are symmetric
    assert_eq!(positioning.left_margin, positioning.right_margin);
    assert_eq!(positioning.left_margin, layout.horizontal_padding);
}

#[test]
fn test_cache_functionality() {
    let mut calculator = LayoutCalculator::new();

    // Test that cache statistics work
    let stats_initial = calculator.get_cache_stats();
    assert_eq!(stats_initial.total_entries, 0);

    // Calculate a layout (should cache it)
    calculator.calculate_layout((100, 30));
    let stats_after = calculator.get_cache_stats();
    assert_eq!(stats_after.total_entries, 1);

    // Clear cache
    calculator.clear_cache();
    let stats_cleared = calculator.get_cache_stats();
    assert_eq!(stats_cleared.total_entries, 0);
}

#[test]
fn test_string_buffer_pool() {
    let mut calculator = LayoutCalculator::new();

    // Test buffer pool functionality
    let buffer = calculator.get_string_buffer(1024);
    assert_eq!(buffer.len(), 0);
    assert!(buffer.capacity() >= 1024);

    // Return buffer to pool
    calculator.return_string_buffer(buffer);

    // Get another buffer (should reuse from pool)
    let buffer2 = calculator.get_string_buffer(512);
    assert_eq!(buffer2.len(), 0);

    calculator.return_string_buffer(buffer2);
}

#[test]
fn test_layout_validation() {
    let calculator = LayoutCalculator::new();

    // Test valid terminal sizes
    assert!(calculator.validate_terminal_size((80, 24)).is_ok());
    assert!(calculator.validate_terminal_size((120, 40)).is_ok());

    // Test invalid terminal sizes
    assert!(calculator.validate_terminal_size((0, 24)).is_err());
    assert!(calculator.validate_terminal_size((80, 0)).is_err());
    assert!(calculator.validate_terminal_size((30, 5)).is_err());
}
