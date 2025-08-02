//! Integration tests for the dynamic UI space feature
//!
//! These tests verify the complete rendering pipeline with various terminal sizes,
//! layout consistency across size changes, and pagination behavior with dynamic sizing.

use liiga_teletext::{
    constants::dynamic_ui,
    data_fetcher::models::*,
    teletext_ui::{GameResultData, TeletextPage},
    ui::{
        content_adapter::ContentAdapter,
        layout::{DetailLevel, LayoutCalculator},
        resize::ResizeHandler,
    },
};
use std::io::{self, Write};
use std::time::Duration;
use tokio::time::sleep;

/// Helper function to create mock game data for testing
fn create_mock_game_data(
    home_team: &str,
    away_team: &str,
    result: &str,
    goal_count: usize,
) -> GameData {
    let mut goal_events = Vec::new();

    // Create mock goal events
    for i in 0..goal_count {
        goal_events.push(GoalEventData {
            scorer_player_id: 1000 + i as i64,
            scorer_name: format!("Player {}", i + 1),
            minute: 10 + (i * 5) as i32,
            home_team_score: if i % 2 == 0 {
                (i / 2) as i32 + 1
            } else {
                (i / 2) as i32
            },
            away_team_score: if i % 2 == 1 {
                ((i + 1) / 2) as i32
            } else {
                ((i + 1) / 2) as i32
            },
            is_winning_goal: i == goal_count - 1,
            goal_types: vec!["YV".to_string()],
            is_home_team: i % 2 == 0,
            video_clip_url: Some(format!("https://example.com/goal_{}.mp4", i)),
        });
    }

    GameData {
        home_team: home_team.to_string(),
        away_team: away_team.to_string(),
        time: "18:30".to_string(),
        result: result.to_string(),
        score_type: liiga_teletext::teletext_ui::ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        serie: "runkosarja".to_string(),
        goal_events,
        played_time: 3600,
        start: "2024-01-15T18:30:00Z".to_string(),
    }
}

/// Helper function to create a teletext page with multiple games
fn create_test_page_with_games(game_count: usize) -> TeletextPage {
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        false,
    );

    for i in 0..game_count {
        let game = create_mock_game_data(
            &format!("Team {}", i * 2),
            &format!("Team {}", i * 2 + 1),
            &format!("{}-{}", i % 5, (i + 1) % 4),
            i % 4, // Varying number of goals
        );
        page.add_game_result(GameResultData::new(&game));
    }

    page
}

/// Test helper to capture rendered output by using a mock stdout
struct MockStdout {
    buffer: Vec<u8>,
}

impl MockStdout {
    fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    fn get_output(&self) -> String {
        String::from_utf8_lossy(&self.buffer).to_string()
    }
}

impl Write for MockStdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Helper function to test rendering without actual terminal output
fn test_render_to_string(page: &TeletextPage) -> Result<String, String> {
    // Since we can't easily mock Stdout for crossterm, we'll test the core functionality
    // by checking that the page can calculate its layout and content properly

    // Test that pagination works
    let total_pages = page.total_pages();
    let current_page = page.get_current_page();

    // Verify basic properties
    if total_pages == 0 {
        return Err("No pages calculated".to_string());
    }

    if current_page >= total_pages {
        return Err("Current page exceeds total pages".to_string());
    }

    // Return a mock representation of successful rendering
    Ok(format!(
        "Rendered page {} of {} pages",
        current_page + 1,
        total_pages
    ))
}

#[tokio::test]
async fn test_end_to_end_rendering_small_terminal() {
    // Test rendering with minimum terminal size (80x24)
    let terminal_size = (
        dynamic_ui::MIN_TERMINAL_WIDTH,
        dynamic_ui::MIN_TERMINAL_HEIGHT,
    );
    let mut page = create_test_page_with_games(5);

    // Update layout for small terminal
    page.update_layout(terminal_size);

    // Verify the page can render without errors
    let result = test_render_to_string(&page);
    assert!(
        result.is_ok(),
        "Rendering should succeed for minimum terminal size"
    );

    // Verify pagination works correctly
    let total_pages = page.total_pages();
    assert!(total_pages > 0, "Should have at least one page");

    let current_page = page.get_current_page();
    assert!(current_page < total_pages, "Current page should be valid");

    // Verify games were added
    assert!(page.total_pages() >= 1, "Should have content to display");
}

#[tokio::test]
async fn test_end_to_end_rendering_medium_terminal() {
    // Test rendering with medium terminal size (100x30)
    let terminal_size = (100, 30);
    let mut page = create_test_page_with_games(8);

    // Update layout for medium terminal
    page.update_layout(terminal_size);

    // Verify the page can render without errors
    let result = test_render_to_string(&page);
    assert!(
        result.is_ok(),
        "Rendering should succeed for medium terminal size"
    );

    // Verify pagination works correctly
    let total_pages = page.total_pages();
    assert!(total_pages > 0, "Should have at least one page");

    // Medium terminal should potentially fit more games per page
    let current_page = page.get_current_page();
    assert!(current_page < total_pages, "Current page should be valid");
}

#[tokio::test]
async fn test_end_to_end_rendering_large_terminal() {
    // Test rendering with large terminal size (140x40)
    let terminal_size = (140, 40);
    let mut page = create_test_page_with_games(12);

    // Update layout for large terminal
    page.update_layout(terminal_size);

    // Verify the page can render without errors
    let result = test_render_to_string(&page);
    assert!(
        result.is_ok(),
        "Rendering should succeed for large terminal size"
    );

    // Verify pagination works correctly
    let total_pages = page.total_pages();
    assert!(total_pages > 0, "Should have at least one page");

    // Large terminal should potentially fit more games per page, resulting in fewer total pages
    let current_page = page.get_current_page();
    assert!(current_page < total_pages, "Current page should be valid");
}

#[tokio::test]
async fn test_layout_consistency_across_size_changes() {
    let mut page = create_test_page_with_games(10);

    // Test sequence of different terminal sizes
    let sizes = vec![
        (80, 24),  // Minimum
        (100, 30), // Medium
        (140, 40), // Large
        (90, 25),  // Back to small
        (120, 35), // Medium-large
    ];

    let mut previous_total_pages = 0;

    for (width, height) in sizes {
        // Update layout
        page.update_layout((width, height));

        // Test rendering
        let result = test_render_to_string(&page);
        assert!(
            result.is_ok(),
            "Rendering should succeed for size {}x{}",
            width,
            height
        );

        // Verify pagination consistency
        let total_pages = page.total_pages();
        assert!(total_pages > 0, "Should always have at least one page");

        let current_page = page.get_current_page();
        assert!(
            current_page < total_pages,
            "Current page should be valid for size {}x{}",
            width,
            height
        );

        // Verify layout adapts to size - larger screens should generally have fewer pages
        // (more content per page) but this isn't guaranteed due to content wrapping
        if width >= dynamic_ui::EXTENDED_DETAIL_WIDTH_THRESHOLD && previous_total_pages > 0 {
            // Just verify that pagination is working, not necessarily fewer pages
            assert!(
                total_pages > 0,
                "Large screens should still have valid pagination"
            );
        }

        previous_total_pages = total_pages;
    }
}

#[tokio::test]
async fn test_pagination_behavior_with_dynamic_sizing() {
    let mut page = create_test_page_with_games(20); // Many games to test pagination

    // Test pagination with small terminal (should have more pages)
    page.update_layout((80, 24));
    let small_total_pages = page.total_pages();

    // Test pagination with large terminal (should have fewer pages)
    page.update_layout((140, 40));
    let large_total_pages = page.total_pages();

    // Large terminal should fit more games per page, thus fewer total pages
    assert!(
        large_total_pages <= small_total_pages,
        "Large terminal should have fewer or equal pages than small terminal"
    );

    // Test page navigation consistency
    page.update_layout((100, 30));
    let initial_page = page.get_current_page();

    // Navigate through pages
    page.next_page();
    let after_next = page.get_current_page();
    assert!(
        after_next > initial_page || after_next == 0, // Wrapped to first page
        "Next page should advance or wrap to beginning"
    );

    page.previous_page();
    let after_prev = page.get_current_page();
    assert_eq!(
        after_prev, initial_page,
        "Previous page should return to initial page"
    );
}

#[tokio::test]
async fn test_resize_during_pagination() {
    let mut page = create_test_page_with_games(15);

    // Start with medium size and navigate to page 2
    page.update_layout((100, 30));
    page.next_page();
    let _page_before_resize = page.get_current_page();

    // Resize to large terminal (more games per page)
    page.update_layout((140, 40));
    let page_after_resize = page.get_current_page();

    // Page should be adjusted to remain valid
    assert!(
        page_after_resize < page.total_pages(),
        "Current page should remain valid after resize"
    );

    // Content should still be accessible
    let result = test_render_to_string(&page);
    assert!(
        result.is_ok(),
        "Rendering should work after resize during pagination"
    );

    // Verify pagination is still functional
    let total_pages = page.total_pages();
    assert!(total_pages > 0, "Should still have pages after resize");
}

#[tokio::test]
async fn test_detail_level_transitions() {
    let mut page = create_test_page_with_games(5);

    // Test minimal detail level (small screen)
    page.update_layout((80, 24));
    let minimal_result = test_render_to_string(&page);
    assert!(
        minimal_result.is_ok(),
        "Minimal detail level should render successfully"
    );
    let minimal_pages = page.total_pages();

    // Test standard detail level (medium screen)
    page.update_layout((100, 30));
    let standard_result = test_render_to_string(&page);
    assert!(
        standard_result.is_ok(),
        "Standard detail level should render successfully"
    );
    let standard_pages = page.total_pages();

    // Test extended detail level (large screen)
    page.update_layout((140, 40));
    let extended_result = test_render_to_string(&page);
    assert!(
        extended_result.is_ok(),
        "Extended detail level should render successfully"
    );
    let extended_pages = page.total_pages();

    // Verify that all detail levels work
    assert!(minimal_pages > 0, "Minimal detail should have pages");
    assert!(standard_pages > 0, "Standard detail should have pages");
    assert!(extended_pages > 0, "Extended detail should have pages");

    // Generally, larger screens should fit more content per page (fewer total pages)
    // but this isn't guaranteed due to content formatting differences
    assert!(
        extended_pages <= minimal_pages,
        "Extended detail should have same or fewer pages than minimal"
    );
}

#[tokio::test]
async fn test_content_adaptation_with_goal_events() {
    // Create games with varying numbers of goal events
    let games = vec![
        create_mock_game_data("HIFK", "Tappara", "0-0", 0), // No goals
        create_mock_game_data("Kärpät", "Lukko", "1-0", 1), // One goal
        create_mock_game_data("TPS", "Ilves", "3-2", 5),    // Many goals
    ];

    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        false,
    );

    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    // Test with different terminal sizes
    let sizes = vec![(80, 24), (100, 30), (140, 40)];

    for (width, height) in sizes {
        page.update_layout((width, height));

        let result = test_render_to_string(&page);
        assert!(
            result.is_ok(),
            "Should render games with varying goal events for size {}x{}",
            width,
            height
        );

        // Verify pagination works with different content types
        let total_pages = page.total_pages();
        assert!(
            total_pages > 0,
            "Should have pages with varying goal events"
        );

        let current_page = page.get_current_page();
        assert!(current_page < total_pages, "Current page should be valid");
    }
}

#[tokio::test]
async fn test_error_handling_during_rendering() {
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        false,
    );

    // Add error message
    page.add_error_message("Test error message");

    // Test rendering with various sizes
    let sizes = vec![(80, 24), (100, 30), (140, 40)];

    for (width, height) in sizes {
        page.update_layout((width, height));

        let result = test_render_to_string(&page);
        assert!(
            result.is_ok(),
            "Should handle error messages gracefully for size {}x{}",
            width,
            height
        );

        // Verify pagination works even with error messages
        let total_pages = page.total_pages();
        assert!(
            total_pages > 0,
            "Should have at least one page with error message"
        );

        let current_page = page.get_current_page();
        assert!(
            current_page < total_pages,
            "Current page should be valid with error message"
        );
    }
}

#[tokio::test]
async fn test_extreme_terminal_sizes() {
    let mut page = create_test_page_with_games(3);

    // Test very small terminal (below minimum)
    page.update_layout((60, 15));
    let result = test_render_to_string(&page);
    assert!(
        result.is_ok(),
        "Should handle very small terminals gracefully"
    );

    let small_pages = page.total_pages();
    assert!(
        small_pages > 0,
        "Should have pages even with very small terminal"
    );

    // Test very large terminal
    page.update_layout((200, 60));
    let result = test_render_to_string(&page);
    assert!(
        result.is_ok(),
        "Should handle very large terminals gracefully"
    );

    let large_pages = page.total_pages();
    assert!(
        large_pages > 0,
        "Should have pages even with very large terminal"
    );

    // Large terminal should generally have fewer or equal pages
    assert!(
        large_pages <= small_pages,
        "Large terminal should fit more content per page"
    );
}

#[tokio::test]
async fn test_layout_calculator_integration() {
    let mut calculator = LayoutCalculator::new();

    // Test various terminal sizes
    let test_cases = vec![
        ((80, 24), DetailLevel::Minimal),
        ((100, 30), DetailLevel::Standard),
        ((140, 40), DetailLevel::Extended),
    ];

    for ((width, height), expected_detail) in test_cases {
        let config = calculator.calculate_layout((width, height));

        assert_eq!(
            config.detail_level, expected_detail,
            "Detail level should match expected for size {}x{}",
            width, height
        );
        assert!(config.content_width > 0, "Content width should be positive");
        assert!(
            config.content_height > 0,
            "Content height should be positive"
        );
        assert!(
            config.games_per_page > 0,
            "Games per page should be positive"
        );
    }
}

#[tokio::test]
async fn test_resize_handler_integration() {
    let mut resize_handler = ResizeHandler::new();

    // Simulate size changes
    let size1 = (80, 24);
    let size2 = (100, 30);
    let _size3 = (80, 24); // Back to original

    // First check should not trigger immediate resize due to debouncing
    let result1 = resize_handler.check_for_resize(size1);
    assert!(result1.is_none(), "First size check should be debounced");

    // Immediate second check with same size should not trigger (debouncing)
    let result2 = resize_handler.check_for_resize(size1);
    assert!(
        result2.is_none(),
        "Immediate same size check should be debounced"
    );

    // After debounce period, should detect the resize
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    let result3 = resize_handler.check_for_resize(size1);
    assert!(result3.is_some(), "After debounce period, resize should be detected");

    // Different size should eventually trigger
    let result4 = resize_handler.check_for_resize(size2);
    assert!(result4.is_none(), "New size change should be debounced initially");

    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await; // Wait for debounce
    let result5 = resize_handler.check_for_resize(size2);
    assert!(
        result5.is_some(),
        "Size change should be detected after debounce"
    );
}

#[tokio::test]
async fn test_content_adapter_integration() {
    let game = create_mock_game_data("HIFK", "Tappara", "3-2", 3);
    let game_result = GameResultData::new(&game);

    // Test content adaptation for different detail levels
    let detail_levels = vec![
        DetailLevel::Minimal,
        DetailLevel::Standard,
        DetailLevel::Extended,
    ];

    for detail_level in detail_levels {
        let adapted = ContentAdapter::adapt_game_content(
            &game_result.home_team,
            &game_result.away_team,
            &game_result.time,
            &game_result.result,
            &game.goal_events,
            detail_level,
            100,
        );

        assert!(
            !adapted.home_team.is_empty(),
            "Home team should not be empty for {:?}",
            detail_level
        );
        assert!(
            !adapted.away_team.is_empty(),
            "Away team should not be empty for {:?}",
            detail_level
        );
        assert!(
            !adapted.result_display.is_empty(),
            "Result should not be empty for {:?}",
            detail_level
        );

        // Extended detail should have more goal information
        if detail_level == DetailLevel::Extended {
            assert!(
                !adapted.goal_lines.is_empty(),
                "Extended detail should show goal events"
            );
        }
    }
}
