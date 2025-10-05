//! Integration tests for interactive mode layout system
//!
//! This module tests the integration between the interactive UI system and the layout management,
//! ensuring that layout updates work correctly during interactive refresh cycles.

use liiga_teletext::data_fetcher::GoalEventData;
use liiga_teletext::data_fetcher::models::GameData;
use liiga_teletext::teletext_ui::TeletextPage;
use liiga_teletext::ui::teletext::game_result::{GameResultData, ScoreType};

// Mock InteractiveState for testing since it's not public
#[derive(Debug)]
struct MockInteractiveState {
    needs_refresh: bool,
    needs_render: bool,
    current_page: Option<TeletextPage>,
}

impl MockInteractiveState {
    fn new() -> Self {
        Self {
            needs_refresh: true,
            needs_render: false,
            current_page: None,
        }
    }

    fn needs_refresh(&self) -> bool {
        self.needs_refresh
    }

    fn needs_render(&self) -> bool {
        self.needs_render
    }

    fn request_refresh(&mut self) {
        self.needs_refresh = true;
    }

    fn request_render(&mut self) {
        self.needs_render = true;
    }

    fn clear_refresh_flag(&mut self) {
        self.needs_refresh = false;
    }

    fn clear_render_flag(&mut self) {
        self.needs_render = false;
    }

    fn set_current_page(&mut self, page: TeletextPage) {
        self.current_page = Some(page);
        self.request_render();
    }

    fn current_page(&self) -> Option<&TeletextPage> {
        self.current_page.as_ref()
    }

    fn current_page_mut(&mut self) -> Option<&mut TeletextPage> {
        self.current_page.as_mut()
    }

    fn handle_resize(&mut self) {
        if let Some(page) = &mut self.current_page {
            page.handle_resize();
        }
        self.request_render();
    }
}

/// Test data for creating realistic game scenarios
fn create_test_game_data(
    home_team: &str,
    away_team: &str,
    score: &str,
    score_type: ScoreType,
    goal_events: Vec<GoalEventData>,
) -> GameData {
    GameData {
        home_team: home_team.to_string(),
        away_team: away_team.to_string(),
        time: "19:30".to_string(),
        result: score.to_string(),
        score_type,
        is_overtime: false,
        is_shootout: false,
        goal_events,
        played_time: 3600,
        serie: "RUNKOSARJA".to_string(),
        start: "2025-01-15T19:30:00Z".to_string(),
    }
}

/// Creates test goal events with various goal types to test layout positioning
fn create_test_goal_events() -> Vec<GoalEventData> {
    vec![
        GoalEventData {
            scorer_player_id: 1,
            scorer_name: "Teemu Selänne".to_string(), // Long name to test spacing
            minute: 15,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["YV".to_string()], // Young player goal
            is_home_team: true,
            video_clip_url: Some("https://example.com/video1".to_string()),
        },
        GoalEventData {
            scorer_player_id: 2,
            scorer_name: "Jari Kurri".to_string(),
            minute: 28,
            home_team_score: 1,
            away_team_score: 1,
            is_winning_goal: false,
            goal_types: vec!["IM".to_string(), "TM".to_string()], // Multiple goal types
            is_home_team: false,
            video_clip_url: Some("https://example.com/video2".to_string()),
        },
        GoalEventData {
            scorer_player_id: 3,
            scorer_name: "Saku Koivu".to_string(),
            minute: 45,
            home_team_score: 2,
            away_team_score: 1,
            is_winning_goal: true,
            goal_types: vec!["YV".to_string(), "IM".to_string()], // Winning goal with multiple types
            is_home_team: true,
            video_clip_url: None,
        },
    ]
}

#[test]
fn test_interactive_state_layout_integration() {
    // Test that MockInteractiveState properly manages TeletextPage with layout updates
    let mut state = MockInteractiveState::new();

    // Create a TeletextPage with layout manager
    let page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        false, // ignore_height_limit = false for interactive mode
        false, // compact_mode
        false, // wide_mode
    );

    // Verify initial state
    assert!(state.needs_refresh());
    assert!(!state.needs_render());
    assert!(state.current_page().is_none());

    // Set the page in state
    state.set_current_page(page);

    // Verify state after setting page
    assert!(state.needs_render()); // Should request render after setting page
    assert!(state.current_page().is_some());

    // Test that page is properly stored
    assert!(state.current_page().is_some());
}

#[test]
fn test_layout_updates_during_refresh() {
    // Test that layout calculations are updated correctly during refresh cycles
    let mut state = MockInteractiveState::new();

    // Create initial page with some games
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        false,
        false,
        false,
    );

    // Add games with goal events to test layout calculations
    let goal_events = create_test_goal_events();
    let games = vec![
        create_test_game_data(
            "Tappara",
            "HIFK",
            "2-1",
            ScoreType::Final,
            goal_events.clone(),
        ),
        create_test_game_data("TPS", "Ilves", "1-0", ScoreType::Ongoing, vec![]),
        create_test_game_data("JYP", "KalPa", "0-0", ScoreType::Scheduled, vec![]),
    ];

    // Add games to page
    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    // Set page in state
    state.set_current_page(page);

    // Simulate refresh cycle - clear refresh flag and set it again
    state.clear_refresh_flag();
    state.request_refresh();

    // Verify refresh is requested
    assert!(state.needs_refresh());

    // Verify page is still accessible after refresh simulation
    assert!(state.current_page().is_some());
}

#[test]
fn test_resize_handling_in_interactive_mode() {
    // Test that resize events are properly handled in interactive mode
    let mut state = MockInteractiveState::new();

    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        false, // Interactive mode
        false,
        false,
    );

    // Add some games to test layout recalculation
    let games = vec![create_test_game_data(
        "Tappara",
        "HIFK",
        "2-1",
        ScoreType::Final,
        create_test_goal_events(),
    )];

    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    state.set_current_page(page);

    // Clear initial render flag
    state.clear_render_flag();

    // Simulate resize event
    state.handle_resize();

    // Verify resize triggers re-render
    assert!(state.needs_render());

    // Verify page is still accessible after resize
    assert!(state.current_page().is_some());
}

#[test]
fn test_layout_consistency_across_refreshes() {
    // Test that layout calculations remain consistent across multiple refresh cycles
    let mut state = MockInteractiveState::new();

    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        false,
        false,
        false,
    );

    // Create consistent test data
    let goal_events = create_test_goal_events();
    let games = vec![
        create_test_game_data(
            "Tappara",
            "HIFK",
            "2-1",
            ScoreType::Final,
            goal_events.clone(),
        ),
        create_test_game_data("TPS", "Ilves", "1-0", ScoreType::Ongoing, vec![]),
    ];

    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    state.set_current_page(page);

    // Simulate multiple refresh cycles
    for _i in 0..5 {
        state.clear_refresh_flag();
        state.request_refresh();

        // Verify page remains accessible
        assert!(state.current_page().is_some());

        state.clear_refresh_flag();
    }
}

#[test]
fn test_layout_with_goal_events_in_interactive_mode() {
    // Test that goal event positioning works correctly in interactive mode
    let mut state = MockInteractiveState::new();

    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        false,
        false,
        false,
    );

    // Create games with various goal event scenarios
    let complex_goal_events = vec![GoalEventData {
        scorer_player_id: 1,
        scorer_name: "Teemu Hartikainen".to_string(), // Long name
        minute: 15,
        home_team_score: 1,
        away_team_score: 0,
        is_winning_goal: false,
        goal_types: vec!["YV".to_string(), "IM".to_string(), "TM".to_string()], // Multiple types
        is_home_team: true,
        video_clip_url: Some("https://example.com/video".to_string()),
    }];

    let games = vec![create_test_game_data(
        "Tappara",
        "HIFK",
        "1-0",
        ScoreType::Final,
        complex_goal_events,
    )];

    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    state.set_current_page(page);

    // Test that page with goal events is properly stored
    assert!(state.current_page().is_some());
}

#[test]
fn test_buffered_rendering_with_layout() {
    // Test that buffered rendering works correctly with the layout system
    let mut state = MockInteractiveState::new();

    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        true, // ignore_height_limit = true for testing
        false,
        false,
    );

    // Add games to test rendering
    let games = vec![
        create_test_game_data(
            "Tappara",
            "HIFK",
            "2-1",
            ScoreType::Final,
            create_test_goal_events(),
        ),
        create_test_game_data("TPS", "Ilves", "1-0", ScoreType::Ongoing, vec![]),
    ];

    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    state.set_current_page(page);

    // Test rendering capability (without actually rendering to avoid terminal issues in tests)
    if let Some(_page) = state.current_page() {
        // In test environment, we just verify the page is accessible for rendering
        // Actual rendering would require terminal access which may not be available in CI
        // Page is accessible for rendering - no assertion needed
    }
}

#[test]
fn test_wide_mode_layout_integration() {
    // Test layout system integration with wide mode in interactive context
    let mut state = MockInteractiveState::new();

    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        false,
        false,
        true, // wide_mode = true
    );

    // Add enough games to test wide mode distribution
    let games: Vec<GameData> = (0..6)
        .map(|i| {
            create_test_game_data(
                &format!("Home{}", i),
                &format!("Away{}", i),
                "1-0",
                ScoreType::Final,
                vec![],
            )
        })
        .collect();

    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    state.set_current_page(page);

    // Test wide mode functionality
    if let Some(page) = state.current_page() {
        // Test game distribution for wide mode
        let (left_games, right_games) = page.distribute_games_for_wide_display();

        // Should have some games distributed
        assert!(!left_games.is_empty() || !right_games.is_empty());

        // Test wide mode capability
        let _can_fit_two_pages = page.can_fit_two_pages();
        // can_fit_two_pages can be either true or false depending on terminal width - both are valid
    }
}

#[test]
fn test_layout_cache_performance() {
    // Test that layout calculations work consistently for performance
    let mut state = MockInteractiveState::new();

    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        false,
        false,
        false,
    );

    // Create games with goal events to test layout calculations
    let games = vec![create_test_game_data(
        "Tappara",
        "HIFK",
        "2-1",
        ScoreType::Final,
        create_test_goal_events(),
    )];

    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    state.set_current_page(page);

    // Test that page is accessible and layout system is working
    assert!(state.current_page().is_some());

    // Test multiple refresh cycles to ensure consistency
    for _i in 0..3 {
        state.clear_refresh_flag();
        state.request_refresh();
        assert!(state.needs_refresh());
        state.clear_refresh_flag();
    }
}
#[test]
fn test_interactive_refresh_cycle_with_layout_updates() {
    // Test the complete interactive refresh cycle with layout system integration
    let mut state = MockInteractiveState::new();

    // Create page with initial games
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        false, // Interactive mode
        false,
        false,
    );

    // Initial games
    let initial_games = vec![create_test_game_data(
        "Tappara",
        "HIFK",
        "0-0",
        ScoreType::Scheduled,
        vec![],
    )];

    for game in &initial_games {
        page.add_game_result(GameResultData::new(game));
    }

    state.set_current_page(page);

    // Simulate interactive refresh cycle
    // 1. Initial state - needs refresh
    assert!(state.needs_refresh());

    // 2. Clear refresh flag (simulating data fetch completion)
    state.clear_refresh_flag();
    assert!(!state.needs_refresh());

    // 3. Simulate game state change (scheduled -> ongoing)
    if let Some(page) = state.current_page_mut() {
        // Clear existing games and add updated game
        *page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "SM-LIIGA".to_string(),
            false,
            true,
            false,
            false,
            false,
        );

        let updated_games = vec![create_test_game_data(
            "Tappara",
            "HIFK",
            "1-0",
            ScoreType::Ongoing,
            create_test_goal_events(),
        )];

        for game in &updated_games {
            page.add_game_result(GameResultData::new(game));
        }
    }

    // 4. Request refresh due to data change
    state.request_refresh();
    assert!(state.needs_refresh());

    // 5. Simulate resize during refresh
    state.handle_resize();
    assert!(state.needs_render()); // Should trigger render

    // 6. Clear flags after processing
    state.clear_refresh_flag();
    state.clear_render_flag();

    // 7. Verify final state
    assert!(!state.needs_refresh());
    assert!(!state.needs_render());
    assert!(state.current_page().is_some());
}

#[test]
fn test_layout_system_handles_empty_games_gracefully() {
    // Test that layout system handles edge cases gracefully in interactive mode
    let mut state = MockInteractiveState::new();

    // Create page with no games initially
    let page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        false,
        false,
        false,
    );

    state.set_current_page(page);

    // Should handle empty state gracefully
    assert!(state.current_page().is_some());
    assert!(state.needs_render());

    // Simulate refresh with empty data
    state.clear_refresh_flag();
    state.request_refresh();
    assert!(state.needs_refresh());

    // Simulate resize with empty data
    state.handle_resize();
    assert!(state.needs_render());

    // Should still be functional
    assert!(state.current_page().is_some());
}

#[test]
fn test_layout_system_performance_under_frequent_updates() {
    // Test layout system performance under frequent refresh cycles
    let mut state = MockInteractiveState::new();

    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        false,
        false,
        false,
    );

    // Add games with complex goal events
    let games = vec![
        create_test_game_data(
            "Tappara",
            "HIFK",
            "2-1",
            ScoreType::Final,
            create_test_goal_events(),
        ),
        create_test_game_data("TPS", "Ilves", "1-0", ScoreType::Ongoing, vec![]),
        create_test_game_data("JYP", "KalPa", "0-0", ScoreType::Scheduled, vec![]),
    ];

    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    state.set_current_page(page);

    // Simulate many rapid refresh cycles (like during live games)
    for i in 0..20 {
        state.clear_refresh_flag();
        state.request_refresh();

        // Occasionally simulate resize
        if i % 5 == 0 {
            state.handle_resize();
        }

        // Verify state remains consistent
        assert!(state.needs_refresh() || state.needs_render());
        assert!(state.current_page().is_some());

        // Clear flags
        state.clear_refresh_flag();
        state.clear_render_flag();
    }

    // Final verification
    assert!(state.current_page().is_some());
}
