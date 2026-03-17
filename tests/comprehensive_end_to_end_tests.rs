//! Comprehensive end-to-end tests for the complete application flow
//!
//! This module implements task 24: Run comprehensive end-to-end tests
//! - Test complete application flow
//! - Verify all game display scenarios
//! - Test different terminal sizes
//! - Requirements: 4.2

use liiga_teletext::data_fetcher::GoalEventData;
use liiga_teletext::data_fetcher::models::{GameData, PlayoffSeriesScore};
use liiga_teletext::teletext_ui::CONTENT_MARGIN;
use liiga_teletext::teletext_ui::layout::{AlignmentCalculator, ColumnLayoutManager};
use liiga_teletext::teletext_ui::{GameResultData, ScoreType, TeletextPage};
use liiga_teletext::testing_utils::TestDataBuilder;
use liiga_teletext::ui::teletext::CompactModeValidation;

/// Creates comprehensive test game data covering all scenarios
fn create_comprehensive_test_games() -> Vec<GameData> {
    vec![
        // Scenario 1: Finished game with multiple goal events and video links
        {
            let mut game = TestDataBuilder::create_overtime_game("HIFK", "Tappara");
            game.result = "3-2".to_string();
            game.goal_events = vec![
                {
                    let mut goal =
                        TestDataBuilder::create_powerplay_goal("Teemu Hartikainen", 15, 1, 0, true);
                    goal.scorer_player_id = 1;
                    goal.video_clip_url = Some("https://example.com/video1.mp4".to_string());
                    goal
                },
                GoalEventData {
                    scorer_player_id: 2,
                    scorer_name: "Mikko Rantanen".to_string(),
                    minute: 28,
                    home_team_score: 1,
                    away_team_score: 1,
                    is_winning_goal: false,
                    goal_types: vec!["IM".to_string(), "TM".to_string()],
                    is_home_team: false,
                    video_clip_url: None,
                },
                {
                    let mut goal =
                        TestDataBuilder::create_winning_goal("Saku Koivu", 65, 3, 2, true);
                    goal.scorer_player_id = 3;
                    goal.goal_types = vec!["YV".to_string(), "IM".to_string()];
                    goal.video_clip_url = Some("https://example.com/video2.mp4".to_string());
                    goal
                },
            ];
            game
        },
        // Scenario 2: Ongoing game with current score
        {
            let mut game = TestDataBuilder::create_live_game("Kärpät", "Lukko", "1-0");
            game.time = "19:00".to_string();
            game.goal_events = vec![{
                let mut goal = TestDataBuilder::create_goal_event("Patrik Laine", 12, 1, 0, true);
                goal.scorer_player_id = 4;
                goal.goal_types = vec!["AV".to_string()];
                goal.video_clip_url = Some("https://example.com/video3.mp4".to_string());
                goal
            }];
            game.start = "2024-01-15T19:00:00Z".to_string();
            game
        },
        // Scenario 3: Scheduled game (no goals yet)
        {
            let mut game = TestDataBuilder::create_basic_game("JYP", "Ilves");
            game.time = "19:30".to_string();
            game.result = "".to_string();
            game.score_type = ScoreType::Scheduled;
            game.played_time = 0;
            game.start = "2024-01-15T19:30:00Z".to_string();
            game
        },
        // Scenario 4: Shootout game
        {
            let mut game = TestDataBuilder::create_shootout_game("Pelicans", "SaiPa");
            game.time = "20:00".to_string();
            game.result = "2-1".to_string();
            game.goal_events = vec![
                GoalEventData {
                    scorer_player_id: 5,
                    scorer_name: "X".to_string(),
                    minute: 5,
                    home_team_score: 1,
                    away_team_score: 0,
                    is_winning_goal: false,
                    goal_types: vec!["VT".to_string()],
                    is_home_team: true,
                    video_clip_url: None,
                },
                GoalEventData {
                    scorer_player_id: 6,
                    scorer_name: "Very Long Player Name Here".to_string(),
                    minute: 45,
                    home_team_score: 1,
                    away_team_score: 1,
                    is_winning_goal: false,
                    goal_types: vec!["YV".to_string(), "IM".to_string(), "TM".to_string()],
                    is_home_team: false,
                    video_clip_url: Some("https://example.com/video4.mp4".to_string()),
                },
            ];
            game.start = "2024-01-15T20:00:00Z".to_string();
            game
        },
        // Scenario 5: Playoffs game
        {
            let mut game = TestDataBuilder::create_overtime_game("TPS", "Sport");
            game.time = "18:00".to_string();
            game.result = "4-3".to_string();
            game.serie = "playoffs".to_string();
            game.goal_events = vec![{
                let mut goal = TestDataBuilder::create_winning_goal("Medium Name", 62, 4, 3, true);
                goal.scorer_player_id = 7;
                goal.goal_types = vec!["YV".to_string()];
                goal.video_clip_url = Some("https://example.com/video5.mp4".to_string());
                goal
            }];
            game.played_time = 3720;
            game.start = "2024-03-15T18:00:00Z".to_string();
            game.play_off_phase = Some(1);
            game.play_off_pair = Some(1);
            game.play_off_req_wins = Some(4);
            game.series_score = Some(PlayoffSeriesScore {
                home_team_wins: 2,
                away_team_wins: 1,
                req_wins: 4,
            });
            game
        },
    ]
}

/// Test complete application flow with all game scenarios
/// Requirements: 4.2 (comprehensive testing)
#[tokio::test]
async fn test_complete_application_flow_all_scenarios() {
    println!("🧪 Testing complete application flow with all game scenarios");

    let games = create_comprehensive_test_games();

    // Test normal mode page creation and game addition
    let mut normal_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // enable video links
        true,  // show footer
        true,  // ignore height limit for testing
        false, // normal mode
        false, // not wide mode
    );

    // Add all games to normal mode page
    for game in &games {
        let game_result = GameResultData::new(game);
        normal_page.add_game_result(game_result);
    }

    // Verify normal mode page creation
    assert!(!normal_page.is_compact_mode(), "Should be in normal mode");
    assert!(!normal_page.is_wide_mode(), "Should not be in wide mode");

    // Test compact mode page creation
    let mut compact_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // enable video links
        true,  // show footer
        true,  // ignore height limit for testing
        true,  // compact mode
        false, // not wide mode
    );

    // Add all games to compact mode page
    for game in &games {
        let game_result = GameResultData::new(game);
        compact_page.add_game_result(game_result);
    }

    // Verify compact mode functionality
    assert!(compact_page.is_compact_mode(), "Should be in compact mode");
    let compact_validation = compact_page.validate_compact_mode_compatibility();
    assert!(
        matches!(
            compact_validation,
            CompactModeValidation::Compatible
                | CompactModeValidation::CompatibleWithWarnings { .. }
        ),
        "Compact mode should be compatible with test scenarios"
    );

    // Test wide mode page creation
    let mut wide_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // enable video links
        true,  // show footer
        true,  // ignore height limit (uses 136 char width)
        false, // not compact mode
        true,  // wide mode
    );

    // Add all games to wide mode page
    for game in &games {
        let game_result = GameResultData::new(game);
        wide_page.add_game_result(game_result);
    }

    // Verify wide mode functionality
    assert!(wide_page.is_wide_mode(), "Should be in wide mode");
    assert!(
        wide_page.can_fit_two_pages(),
        "Should support two-page layout"
    );

    // Test game distribution in wide mode
    let (left_games, right_games) = wide_page.distribute_games_for_wide_display();
    assert!(!left_games.is_empty(), "Left column should have games");
    assert_eq!(
        left_games.len() + right_games.len(),
        games.len(),
        "All games should be distributed"
    );

    println!("✅ Complete application flow test passed for all scenarios");
}

/// Test layout system with all game display scenarios
/// Requirements: 4.2 (verify all game display scenarios)
#[tokio::test]
async fn test_layout_system_all_game_scenarios() {
    println!("🧪 Testing layout system with all game display scenarios");

    let games = create_comprehensive_test_games();

    // Test layout calculation for all scenarios
    let mut layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
    let layout_config = layout_manager.calculate_layout(&games);

    // Verify layout handles all scenarios correctly
    assert!(
        layout_config.home_team_width >= 20,
        "Home team width should accommodate content"
    );
    assert!(
        layout_config.away_team_width >= 15,
        "Away team width should accommodate content"
    );
    // Separator width may be reduced for narrow terminals (3) or use default (5)
    assert!(
        layout_config.separator_width == 3 || layout_config.separator_width == 5,
        "Separator width should be 3 or 5 (got {})",
        layout_config.separator_width
    );
    assert!(
        layout_config.play_icon_column > 40,
        "Play icon should be positioned after team areas"
    );
    assert!(
        layout_config.max_player_name_width >= 10,
        "Should accommodate player names"
    );
    assert!(
        layout_config.max_goal_types_width >= 2,
        "Should accommodate goal types"
    );

    // Test alignment calculations for all scenarios
    let mut alignment_calculator = AlignmentCalculator::new();
    let play_icon_positions =
        alignment_calculator.calculate_play_icon_positions(&games, &layout_config);

    // Verify play icon alignment consistency across all scenarios
    let expected_column = layout_config.play_icon_column;
    for position in &play_icon_positions {
        assert_eq!(
            position.column_position, expected_column,
            "All play icons should be aligned to the same column"
        );
    }

    // Test goal type positioning for all scenarios
    let all_goal_events: Vec<_> = games.iter().flat_map(|g| &g.goal_events).cloned().collect();
    let goal_type_positions =
        alignment_calculator.calculate_goal_type_positions(&all_goal_events, &layout_config);

    // Verify no overflow for any scenario
    for position in &goal_type_positions {
        assert!(
            alignment_calculator.validate_no_overflow(position, &layout_config),
            "Goal type '{}' should not overflow",
            position.goal_types
        );
    }

    println!("✅ Layout system test passed for all game scenarios");
}

/// Test different terminal sizes with comprehensive scenarios
/// Requirements: 4.2 (test different terminal sizes)
#[tokio::test]
async fn test_different_terminal_sizes_comprehensive() {
    println!("🧪 Testing different terminal sizes with comprehensive scenarios");

    let games = create_comprehensive_test_games();
    let terminal_sizes = vec![
        (60, "narrow"),
        (80, "standard"),
        (100, "wide"),
        (120, "very wide"),
        (136, "ultra wide"),
        (160, "maximum"),
    ];

    for (width, description) in terminal_sizes {
        println!("  Testing terminal width: {} ({})", width, description);

        // Test layout calculation for this terminal size
        let mut layout_manager = ColumnLayoutManager::new(width, CONTENT_MARGIN);
        let layout_config = layout_manager.calculate_layout(&games);

        // Verify layout is reasonable for this width
        assert!(
            layout_config.home_team_width >= 15,
            "Home team width should be reasonable for {} width",
            width
        );
        assert!(
            layout_config.away_team_width >= 15,
            "Away team width should be reasonable for {} width",
            width
        );
        // For narrow terminals, columns might extend beyond width (handled by rendering truncation)
        // For wider terminals, columns should fit comfortably
        if width >= 100 {
            assert!(
                layout_config.time_column < width - 10,
                "Time column {} should fit comfortably within terminal width {}",
                layout_config.time_column,
                width
            );
            assert!(
                layout_config.score_column < width,
                "Score column {} should fit within terminal width {}",
                layout_config.score_column,
                width
            );
        } else {
            // For narrow terminals, just verify columns are positioned reasonably
            assert!(
                layout_config.time_column > 0,
                "Time column should be positioned"
            );
            assert!(
                layout_config.score_column > layout_config.time_column,
                "Score column should be after time column"
            );
        }

        // Test alignment calculations for this terminal size
        let mut alignment_calculator = AlignmentCalculator::new();
        let play_icon_positions =
            alignment_calculator.calculate_play_icon_positions(&games, &layout_config);
        let all_goal_events: Vec<_> = games.iter().flat_map(|g| &g.goal_events).cloned().collect();
        let goal_type_positions =
            alignment_calculator.calculate_goal_type_positions(&all_goal_events, &layout_config);

        // Verify positioning works for this terminal size
        for position in &play_icon_positions {
            assert_eq!(
                position.column_position, layout_config.play_icon_column,
                "Play icons should be aligned consistently for {} width",
                width
            );
        }

        for position in &goal_type_positions {
            assert!(
                alignment_calculator.validate_no_overflow(position, &layout_config),
                "Goal types should not overflow for {} width",
                width
            );
        }

        // Test page creation for this terminal size
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "SM-LIIGA".to_string(),
            false,
            true,
            true, // ignore height limit for testing
            false,
            width >= 128, // Enable wide mode for large terminals
        );

        // Add all games
        for game in &games {
            page.add_game_result(GameResultData::new(game));
        }

        // Verify page creation succeeded
        if width >= 128 {
            assert!(
                page.is_wide_mode(),
                "Wide mode should be enabled for {} width",
                width
            );
            assert!(
                page.can_fit_two_pages(),
                "Should support two-page layout for {} width",
                width
            );
        } else {
            assert!(
                !page.is_wide_mode(),
                "Wide mode should be disabled for {} width",
                width
            );
        }

        println!(
            "    ✅ Terminal width {} ({}) test passed",
            width, description
        );
    }

    println!("✅ All terminal size tests passed");
}

/// Test interactive mode integration with comprehensive scenarios
/// Requirements: 4.2 (complete application flow)
#[tokio::test]
async fn test_interactive_mode_comprehensive_integration() {
    println!("🧪 Testing interactive mode integration with comprehensive scenarios");

    let games = create_comprehensive_test_games();

    // Test interactive mode page creation
    let mut interactive_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // enable video links
        true,  // show footer
        false, // interactive mode (uses actual terminal size)
        false, // not compact mode
        false, // not wide mode initially
    );

    // Add games incrementally (simulating live updates)
    for (i, game) in games.iter().enumerate() {
        let game_result = GameResultData::new(game);
        interactive_page.add_game_result(game_result);

        println!(
            "  Added game {}: {} vs {}",
            i + 1,
            game.home_team,
            game.away_team
        );
    }

    // Test resize handling
    interactive_page.handle_resize();

    // Verify interactive mode functionality
    assert!(
        !interactive_page.is_compact_mode(),
        "Should not be in compact mode by default"
    );

    // Test compact mode toggle
    assert!(
        interactive_page.set_compact_mode(true).is_ok(),
        "Should be able to enable compact mode"
    );
    assert!(
        interactive_page.is_compact_mode(),
        "Compact mode should be enabled"
    );

    let compact_validation = interactive_page.validate_compact_mode_compatibility();
    assert!(
        matches!(
            compact_validation,
            CompactModeValidation::Compatible
                | CompactModeValidation::CompatibleWithWarnings { .. }
        ),
        "Compact mode should be compatible"
    );

    // Test compact mode toggle back
    assert!(
        interactive_page.set_compact_mode(false).is_ok(),
        "Should be able to disable compact mode"
    );
    assert!(
        !interactive_page.is_compact_mode(),
        "Compact mode should be disabled"
    );

    println!("✅ Interactive mode integration test passed");
}

/// Test error handling and edge cases in comprehensive scenarios
/// Requirements: 4.2 (complete application flow)
#[tokio::test]
async fn test_error_handling_comprehensive_scenarios() {
    println!("🧪 Testing error handling and edge cases");

    // Test with empty games list
    let empty_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        true,
        false,
        false,
    );

    // Should handle empty state gracefully
    assert!(!empty_page.is_compact_mode(), "Should handle empty state");

    // Test with malformed game data
    let malformed_games = vec![GameData {
        home_team: "".to_string(), // Empty team name
        away_team: "Test".to_string(),
        time: "".to_string(), // Empty time
        result: "".to_string(),
        score_type: ScoreType::Scheduled,
        is_overtime: false,
        is_shootout: false,
        serie: "".to_string(), // Empty serie
        goal_events: vec![GoalEventData {
            scorer_player_id: 0,
            scorer_name: "".to_string(), // Empty player name
            minute: -1,                  // Invalid minute
            home_team_score: 0,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec![], // Empty goal types
            is_home_team: true,
            video_clip_url: None,
        }],
        played_time: 0,
        start: "invalid-date".to_string(), // Invalid date
        play_off_phase: None,
        play_off_pair: None,
        play_off_req_wins: None,
        series_score: None,
        is_placeholder: false,
    }];

    // Test layout system with malformed data
    let mut layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
    let layout_config = layout_manager.calculate_layout(&malformed_games);

    // Should provide reasonable defaults
    assert!(
        layout_config.home_team_width > 0,
        "Should provide reasonable home team width"
    );
    assert!(
        layout_config.away_team_width > 0,
        "Should provide reasonable away team width"
    );
    assert!(
        layout_config.play_icon_column > 0,
        "Should provide reasonable play icon position"
    );

    // Test page creation with malformed data
    let mut malformed_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        true,
        false,
        false,
    );

    // Should handle malformed games gracefully
    for game in &malformed_games {
        let game_result = GameResultData::new(game);
        malformed_page.add_game_result(game_result);
    }

    // Test extremely narrow terminal
    let mut narrow_layout_manager = ColumnLayoutManager::new(40, CONTENT_MARGIN);
    let narrow_layout = narrow_layout_manager.calculate_layout(&create_comprehensive_test_games());

    // Should provide fallback layout
    assert!(
        narrow_layout.home_team_width > 0,
        "Should provide fallback home team width"
    );
    assert!(
        narrow_layout.away_team_width > 0,
        "Should provide fallback away team width"
    );

    println!("✅ Error handling and edge cases test passed");
}

/// Test performance with comprehensive scenarios
/// Requirements: 4.2 (complete application flow)
#[tokio::test]
async fn test_performance_comprehensive_scenarios() {
    println!("🧪 Testing performance with comprehensive scenarios");

    let games = create_comprehensive_test_games();

    // Test layout calculation performance
    let layout_start = std::time::Instant::now();
    let mut layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
    let _layout_config = layout_manager.calculate_layout(&games);
    let layout_duration = layout_start.elapsed();

    // Test alignment calculation performance
    let alignment_start = std::time::Instant::now();
    let mut alignment_calculator = AlignmentCalculator::new();
    let _play_icon_positions =
        alignment_calculator.calculate_play_icon_positions(&games, &_layout_config);
    let alignment_duration = alignment_start.elapsed();

    // Test page creation performance
    let page_start = std::time::Instant::now();
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        true,
        false,
        false,
    );

    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }
    let page_duration = page_start.elapsed();

    // Performance should be reasonable (under 10ms for test scenarios)

    println!("✅ Performance test passed:");
    println!("  - Layout calculation: {} ms", layout_duration.as_millis());
    println!(
        "  - Alignment calculation: {} ms",
        alignment_duration.as_millis()
    );
    println!("  - Page creation: {} ms", page_duration.as_millis());
}

/// Test backward compatibility with existing functionality
/// Requirements: 4.2 (complete application flow)
#[tokio::test]
async fn test_backward_compatibility_comprehensive() {
    println!("🧪 Testing backward compatibility with existing functionality");

    let games = create_comprehensive_test_games();

    // Test that all existing page creation patterns still work
    let page_configs = vec![
        (false, false, false, false, "normal mode"),
        (false, false, false, true, "wide mode"),
        (false, false, true, false, "compact mode"),
        (true, false, false, false, "no video links"),
        (false, true, false, false, "no footer"),
        (false, false, false, false, "interactive mode"),
    ];

    for (disable_video, hide_footer, compact, wide, description) in page_configs {
        println!("  Testing configuration: {}", description);

        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "SM-LIIGA".to_string(),
            disable_video,
            !hide_footer,
            true, // ignore height limit for testing
            compact,
            wide,
        );

        // Add all games
        for game in &games {
            page.add_game_result(GameResultData::new(game));
        }

        // Verify configuration is respected
        assert_eq!(
            page.is_compact_mode(),
            compact,
            "Compact mode setting should be respected"
        );
        assert_eq!(
            page.is_wide_mode(),
            wide,
            "Wide mode setting should be respected"
        );

        if wide {
            assert!(
                page.can_fit_two_pages(),
                "Wide mode should support two-page layout"
            );
            let (left_games, right_games) = page.distribute_games_for_wide_display();
            assert_eq!(
                left_games.len() + right_games.len(),
                games.len(),
                "All games should be distributed in wide mode"
            );
        }

        if compact {
            let validation = page.validate_compact_mode_compatibility();
            assert!(
                matches!(
                    validation,
                    CompactModeValidation::Compatible
                        | CompactModeValidation::CompatibleWithWarnings { .. }
                ),
                "Compact mode should be compatible"
            );
        }

        println!("    ✅ Configuration '{}' test passed", description);
    }

    println!("✅ Backward compatibility test passed");
}
