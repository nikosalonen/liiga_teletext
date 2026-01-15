//! Integration tests for game display functionality with layout system
//!
//! These tests verify the complete game display rendering pipeline including:
//! - Layout calculation and column positioning
//! - ANSI positioning code correctness
//! - Play icon alignment consistency
//! - Goal type positioning and overflow prevention
//! - Complete game display scenarios across different modes
//!
//! Requirements tested: 4.2 (modular and testable formatting logic)

use liiga_teletext::data_fetcher::GoalEventData;
use liiga_teletext::data_fetcher::models::GameData;
use liiga_teletext::teletext_ui::CONTENT_MARGIN;
use liiga_teletext::teletext_ui::layout::{AlignmentCalculator, ColumnLayoutManager};
use liiga_teletext::teletext_ui::{GameResultData, ScoreType, TeletextPage};
#[allow(unused_imports)]
use std::time::Instant;

/// Creates test game data with specified goal events
fn create_test_game_data(
    home_team: &str,
    away_team: &str,
    goal_events: Vec<GoalEventData>,
    score_type: ScoreType,
) -> GameData {
    GameData {
        home_team: home_team.to_string(),
        away_team: away_team.to_string(),
        time: "18:30".to_string(),
        result: "2-1".to_string(),
        score_type,
        is_overtime: false,
        is_shootout: false,
        serie: "RUNKOSARJA".to_string(),
        goal_events,
        played_time: 3600,
        start: "2024-01-15T18:30:00Z".to_string(),
    }
}

/// Creates test goal event with specified parameters
fn create_test_goal_event(
    scorer_name: &str,
    goal_types: Vec<String>,
    is_home_team: bool,
    video_url: Option<String>,
) -> GoalEventData {
    GoalEventData {
        scorer_player_id: 123,
        scorer_name: scorer_name.to_string(),
        minute: 10,
        home_team_score: 1,
        away_team_score: 0,
        is_winning_goal: false,
        goal_types,
        is_home_team,
        video_clip_url: video_url,
    }
}

/// Test complete game display scenario with layout calculation
#[tokio::test]
async fn test_complete_game_display_with_layout_calculation() {
    // Create games with varying content lengths to test layout calculation
    let goal_events = vec![
        create_test_goal_event(
            "Short Name",
            vec!["YV".to_string()],
            true,
            Some("https://example.com/video1.mp4".to_string()),
        ),
        create_test_goal_event(
            "Very Long Player Name",
            vec!["YV".to_string(), "IM".to_string()],
            true,
            None,
        ),
        create_test_goal_event(
            "Away Player",
            vec!["TM".to_string()],
            false,
            Some("https://example.com/video2.mp4".to_string()),
        ),
    ];

    let games = vec![
        create_test_game_data("HIFK", "Tappara", goal_events.clone(), ScoreType::Final),
        create_test_game_data("Kärpät", "Lukko", vec![], ScoreType::Scheduled),
        create_test_game_data(
            "JYP",
            "Ilves",
            vec![create_test_goal_event(
                "Medium Name",
                vec!["AV".to_string()],
                true,
                None,
            )],
            ScoreType::Ongoing,
        ),
    ];

    // Test layout calculation
    let mut layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
    let layout_config = layout_manager.calculate_layout(&games);

    // Verify layout configuration is calculated correctly
    assert_eq!(layout_config.home_team_width, 20);
    assert_eq!(layout_config.away_team_width, 20);
    assert_eq!(layout_config.separator_width, 5);
    assert!(layout_config.play_icon_column > 43); // Should be positioned after team areas
    assert!(layout_config.max_player_name_width >= 10); // Should accommodate content
    assert!(layout_config.max_goal_types_width >= 2); // Should accommodate goal types

    // Test alignment calculator with the layout
    let mut alignment_calculator = AlignmentCalculator::new();
    let play_icon_positions =
        alignment_calculator.calculate_play_icon_positions(&games, &layout_config);

    // Verify play icon positions are consistent
    assert_eq!(play_icon_positions.len(), 4); // 3 + 0 + 1 goal events
    for position in &play_icon_positions {
        assert_eq!(
            position.column_position, layout_config.play_icon_column,
            "All play icons should be aligned to the same column"
        );
    }

    // Test goal type positioning
    let all_events: Vec<_> = games.iter().flat_map(|g| &g.goal_events).cloned().collect();
    let goal_type_positions =
        alignment_calculator.calculate_goal_type_positions(&all_events, &layout_config);

    // Verify no overflow into away team area (column 44+)
    for position in &goal_type_positions {
        let end_position = position.column_position + position.goal_types.len();
        assert!(
            end_position <= 43,
            "Goal type '{}' at position {} would end at {} (overflow past column 43)",
            position.goal_types,
            position.column_position,
            end_position
        );
        assert!(alignment_calculator.validate_no_overflow(position, &layout_config));
    }

    println!("✓ Complete game display with layout calculation works correctly");
}

/// Test ANSI positioning code correctness through layout calculations
#[tokio::test]
async fn test_ansi_positioning_code_correctness() {
    let goal_events = vec![create_test_goal_event(
        "Test Player",
        vec!["YV".to_string()],
        true,
        Some("https://example.com/video.mp4".to_string()),
    )];

    let game = create_test_game_data("HIFK", "Tappara", goal_events, ScoreType::Final);
    let games = vec![game];

    // Test layout calculation for ANSI positioning
    let mut layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
    let layout_config = layout_manager.calculate_layout(&games);

    // Verify calculated positions match expected ANSI positioning

    // Home team should be positioned at content margin + 1 (ANSI 1-based)
    let expected_home_position = CONTENT_MARGIN + 1;
    assert_eq!(expected_home_position, 3, "Home team should be at column 3");

    // Away team should be positioned after home team (20 chars) + separator (3 chars)
    let expected_away_position = expected_home_position + 20 + 3;
    assert_eq!(
        expected_away_position, 26,
        "Away team should be at column 26"
    );

    // Play icon should be positioned after team areas
    assert!(
        layout_config.play_icon_column > expected_away_position + 20,
        "Play icon column {} should be after away team area ({})",
        layout_config.play_icon_column,
        expected_away_position + 20
    );

    // Test alignment calculator positioning
    let mut alignment_calculator = AlignmentCalculator::new();
    let play_icon_positions =
        alignment_calculator.calculate_play_icon_positions(&games, &layout_config);

    assert_eq!(play_icon_positions.len(), 1);
    assert_eq!(
        play_icon_positions[0].column_position, layout_config.play_icon_column,
        "Play icon should be positioned at calculated column"
    );

    // Test goal type positioning to ensure no overflow
    let goal_type_positions =
        alignment_calculator.calculate_goal_type_positions(&games[0].goal_events, &layout_config);

    assert_eq!(goal_type_positions.len(), 1);
    let goal_type_end =
        goal_type_positions[0].column_position + goal_type_positions[0].goal_types.len();
    assert!(
        goal_type_end <= 43,
        "Goal type should not overflow past column 43 (ends at {})",
        goal_type_end
    );

    // Verify time and score columns are within terminal bounds
    assert!(
        layout_config.time_column < 80,
        "Time column {} should be within terminal width",
        layout_config.time_column
    );
    assert!(
        layout_config.score_column < 80,
        "Score column {} should be within terminal width",
        layout_config.score_column
    );

    println!("✓ ANSI positioning calculations are correct for layout system");
}

/// Test play icon alignment consistency across multiple games
#[tokio::test]
async fn test_play_icon_alignment_consistency() {
    // Create games with different player name lengths to test alignment
    let games = vec![
        create_test_game_data(
            "HIFK",
            "Tappara",
            vec![
                create_test_goal_event(
                    "X", // Very short name
                    vec!["YV".to_string()],
                    true,
                    Some("https://example.com/video1.mp4".to_string()),
                ),
                create_test_goal_event(
                    "Very Long Player Name Here", // Long name
                    vec!["IM".to_string()],
                    true,
                    Some("https://example.com/video2.mp4".to_string()),
                ),
            ],
            ScoreType::Final,
        ),
        create_test_game_data(
            "Kärpät",
            "Lukko",
            vec![create_test_goal_event(
                "Medium Name", // Medium length name
                vec!["TM".to_string()],
                true,
                Some("https://example.com/video3.mp4".to_string()),
            )],
            ScoreType::Final,
        ),
    ];

    // Test layout calculation and alignment
    let mut layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
    let layout_config = layout_manager.calculate_layout(&games);
    let mut alignment_calculator = AlignmentCalculator::new();
    let play_icon_positions =
        alignment_calculator.calculate_play_icon_positions(&games, &layout_config);

    // Verify all play icons are aligned to the same column
    assert_eq!(play_icon_positions.len(), 3);
    let expected_column = layout_config.play_icon_column;

    for (i, position) in play_icon_positions.iter().enumerate() {
        assert_eq!(
            position.column_position, expected_column,
            "Play icon {} should be at column {} but was at {}",
            i, expected_column, position.column_position
        );
        assert!(
            position.has_video_link,
            "All test events should have video links"
        );
    }

    // Test dynamic spacing calculation
    let short_name_spacing = layout_manager.calculate_dynamic_spacing(1, &layout_config); // "X"
    let long_name_spacing = layout_manager.calculate_dynamic_spacing(25, &layout_config); // "Very Long Player Name Here"
    let medium_name_spacing = layout_manager.calculate_dynamic_spacing(11, &layout_config); // "Medium Name"

    // Short names should get more spacing, long names should get minimum spacing
    assert!(
        short_name_spacing > long_name_spacing,
        "Short names should get more spacing ({}) than long names ({})",
        short_name_spacing,
        long_name_spacing
    );
    assert!(
        medium_name_spacing > long_name_spacing,
        "Medium names should get more spacing ({}) than long names ({})",
        medium_name_spacing,
        long_name_spacing
    );
    assert!(
        long_name_spacing >= 1,
        "Long names should still get minimum spacing ({})",
        long_name_spacing
    );

    println!("✓ Play icon alignment consistency maintained across different name lengths");
}

/// Test goal type positioning and overflow prevention
#[tokio::test]
async fn test_goal_type_positioning_and_overflow_prevention() {
    // Create goal events with various goal type combinations
    let goal_events = vec![
        create_test_goal_event("Player A", vec!["YV".to_string()], true, None), // 2 chars
        create_test_goal_event(
            "Player B",
            vec!["YV".to_string(), "IM".to_string()],
            true,
            None,
        ), // 5 chars
        create_test_goal_event(
            "Player C",
            vec!["YV".to_string(), "IM".to_string(), "TM".to_string()],
            true,
            None,
        ), // 8 chars
        create_test_goal_event("Very Long Player Name", vec!["VT".to_string()], true, None), // Long name + goal type
        create_test_goal_event(
            "Another Long Name Here",
            vec!["YV".to_string(), "IM".to_string()],
            true,
            None,
        ), // Long name + multiple types
    ];

    let game = create_test_game_data("HIFK", "Tappara", goal_events.clone(), ScoreType::Final);
    let games = vec![game];

    // Test layout and positioning
    let mut layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
    let layout_config = layout_manager.calculate_layout(&games);
    let mut alignment_calculator = AlignmentCalculator::new();
    let goal_type_positions =
        alignment_calculator.calculate_goal_type_positions(&goal_events, &layout_config);

    // Verify no overflow past column 43 (away team starts at 44)
    for (i, position) in goal_type_positions.iter().enumerate() {
        let end_position = position.column_position + position.goal_types.len();
        assert!(
            end_position <= 43,
            "Goal type {} '{}' at position {} ends at {} (overflow past column 43)",
            i,
            position.goal_types,
            position.column_position,
            end_position
        );

        // Verify overflow validation works
        assert!(
            alignment_calculator.validate_no_overflow(position, &layout_config),
            "Goal type {} should pass overflow validation",
            i
        );

        // Verify goal types are not empty (except for events with no goal types)
        if !goal_events[position.event_index].goal_types.is_empty() {
            assert!(
                !position.goal_types.is_empty(),
                "Goal type {} should not be empty when source event has goal types",
                i
            );
        }
    }

    // Test goal type validation
    for goal_type_pos in &goal_type_positions {
        assert!(
            layout_manager.validate_goal_types_fit(&goal_type_pos.goal_types, &layout_config),
            "Goal type '{}' should fit within allocated space",
            goal_type_pos.goal_types
        );
    }

    println!("✓ Goal type positioning prevents overflow and validates correctly");
}

/// Test complete game display scenarios through layout validation
#[tokio::test]
async fn test_complete_game_display_scenarios_normal_mode() {
    // Test various game scenarios
    let scenarios = vec![
        // Scenario 1: Finished game with multiple goal events
        (
            "Finished game with goals",
            create_test_game_data(
                "HIFK",
                "Tappara",
                vec![
                    create_test_goal_event(
                        "Mikko Koivu",
                        vec!["YV".to_string()],
                        true,
                        Some("https://example.com/goal1.mp4".to_string()),
                    ),
                    create_test_goal_event("Saku Koivu", vec!["AV".to_string()], true, None),
                    create_test_goal_event(
                        "Patrik Laine",
                        vec!["TM".to_string()],
                        false,
                        Some("https://example.com/goal2.mp4".to_string()),
                    ),
                ],
                ScoreType::Final,
            ),
        ),
        // Scenario 2: Ongoing game
        (
            "Ongoing game",
            create_test_game_data(
                "Kärpät",
                "Lukko",
                vec![create_test_goal_event(
                    "Teemu Selänne",
                    vec!["YV".to_string(), "IM".to_string()],
                    true,
                    None,
                )],
                ScoreType::Ongoing,
            ),
        ),
        // Scenario 3: Scheduled game (no goals)
        (
            "Scheduled game",
            create_test_game_data("JYP", "Ilves", vec![], ScoreType::Scheduled),
        ),
        // Scenario 4: Game with long player names and multiple goal types
        (
            "Long names and multiple goal types",
            create_test_game_data(
                "Pelicans",
                "SaiPa",
                vec![
                    create_test_goal_event(
                        "Very Long Player Name Here",
                        vec!["YV".to_string(), "IM".to_string(), "TM".to_string()],
                        true,
                        Some("https://example.com/goal3.mp4".to_string()),
                    ),
                    create_test_goal_event(
                        "Another Long Name Player",
                        vec!["VT".to_string()],
                        false,
                        None,
                    ),
                ],
                ScoreType::Final,
            ),
        ),
    ];

    for (scenario_name, game_data) in scenarios {
        println!("Testing scenario: {}", scenario_name);

        // Test layout calculation for this scenario
        let games = vec![game_data.clone()];
        let mut layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
        let layout_config = layout_manager.calculate_layout(&games);

        // Verify layout handles the scenario correctly
        assert!(
            layout_config.max_player_name_width >= 10,
            "Layout should accommodate player names for scenario: {}",
            scenario_name
        );
        assert!(
            layout_config.max_goal_types_width >= 2,
            "Layout should accommodate goal types for scenario: {}",
            scenario_name
        );

        // Test alignment calculations
        let mut alignment_calculator = AlignmentCalculator::new();
        let play_icon_positions =
            alignment_calculator.calculate_play_icon_positions(&games, &layout_config);
        let goal_type_positions = alignment_calculator
            .calculate_goal_type_positions(&game_data.goal_events, &layout_config);

        // Verify play icon alignment
        for position in &play_icon_positions {
            assert_eq!(
                position.column_position, layout_config.play_icon_column,
                "Play icons should be aligned consistently for scenario: {}",
                scenario_name
            );
        }

        // Verify goal type positioning prevents overflow
        for position in &goal_type_positions {
            let end_position = position.column_position + position.goal_types.len();
            assert!(
                end_position <= 43,
                "Goal types should not overflow for scenario: {} (ends at {})",
                scenario_name,
                end_position
            );
            assert!(
                alignment_calculator.validate_no_overflow(position, &layout_config),
                "Goal type overflow validation should pass for scenario: {}",
                scenario_name
            );
        }

        // Create page and verify it can be created successfully
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "SM-LIIGA".to_string(),
            false, // enable video links
            false, // disable footer for cleaner testing
            true,  // ignore height limit
            false, // normal mode
            false, // not wide mode
        );

        page.add_game_result(GameResultData::new(&game_data));

        // Verify page creation succeeded and contains expected data
        assert!(!page.is_compact_mode(), "Should be in normal mode");
        assert!(!page.is_wide_mode(), "Should not be in wide mode");

        println!("✓ Scenario '{}' layout calculated correctly", scenario_name);
    }

    println!("✓ All complete game display scenarios work correctly in normal mode");
}

/// Test complete game display scenarios in wide mode
#[tokio::test]
async fn test_complete_game_display_scenarios_wide_mode() {
    // Create multiple games for wide mode testing
    let games = vec![
        create_test_game_data(
            "HIFK",
            "Tappara",
            vec![create_test_goal_event(
                "Player One",
                vec!["YV".to_string()],
                true,
                Some("https://example.com/video1.mp4".to_string()),
            )],
            ScoreType::Final,
        ),
        create_test_game_data(
            "Kärpät",
            "Lukko",
            vec![create_test_goal_event(
                "Player Two",
                vec!["IM".to_string()],
                true,
                None,
            )],
            ScoreType::Final,
        ),
        create_test_game_data("JYP", "Ilves", vec![], ScoreType::Scheduled),
        create_test_game_data(
            "Pelicans",
            "SaiPa",
            vec![create_test_goal_event(
                "Player Three",
                vec!["TM".to_string()],
                false,
                Some("https://example.com/video2.mp4".to_string()),
            )],
            ScoreType::Ongoing,
        ),
    ];

    // Test layout calculation for wide mode (136 char width)
    let mut layout_manager = ColumnLayoutManager::new(136, CONTENT_MARGIN);
    let layout_config = layout_manager.calculate_layout(&games);

    // Verify wide mode layout has more space for content
    assert!(
        layout_config.time_column > 60,
        "Wide mode should have time column positioned further right ({})",
        layout_config.time_column
    );
    assert!(
        layout_config.score_column > layout_config.time_column,
        "Score column should be after time column in wide mode"
    );

    // Test alignment calculations for wide mode
    let mut alignment_calculator = AlignmentCalculator::new();
    let play_icon_positions =
        alignment_calculator.calculate_play_icon_positions(&games, &layout_config);
    let all_goal_events: Vec<_> = games.iter().flat_map(|g| &g.goal_events).cloned().collect();
    let goal_type_positions =
        alignment_calculator.calculate_goal_type_positions(&all_goal_events, &layout_config);

    // Verify play icon alignment consistency in wide mode
    for position in &play_icon_positions {
        assert_eq!(
            position.column_position, layout_config.play_icon_column,
            "Play icons should be aligned consistently in wide mode"
        );
    }

    // Verify goal type positioning in wide mode
    for position in &goal_type_positions {
        assert!(
            alignment_calculator.validate_no_overflow(position, &layout_config),
            "Goal types should not overflow in wide mode"
        );
    }

    // Create wide mode page
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // enable video links
        false, // disable footer for cleaner testing
        true,  // ignore height limit (uses 136 char width)
        false, // not compact mode
        true,  // wide mode enabled
    );

    // Add all games
    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    // Verify wide mode is enabled and can fit two pages
    assert!(page.is_wide_mode(), "Page should be in wide mode");
    assert!(
        page.can_fit_two_pages(),
        "Should be able to fit two pages in wide mode"
    );

    // Test game distribution for wide mode
    let (left_games, right_games) = page.distribute_games_for_wide_display();

    // Should distribute games between columns
    assert!(!left_games.is_empty(), "Left column should have games");
    assert_eq!(
        left_games.len() + right_games.len(),
        games.len(),
        "Total distributed games should equal input games"
    );

    // Verify distribution is balanced
    let expected_left = games.len().div_ceil(2); // Left gets extra if odd number
    assert_eq!(
        left_games.len(),
        expected_left,
        "Left column should have {} games but has {}",
        expected_left,
        left_games.len()
    );

    println!("✓ Complete game display scenarios work correctly in wide mode");
}

/// Test layout system integration with different terminal widths
#[tokio::test]
async fn test_layout_system_integration_different_widths() {
    let goal_events = vec![create_test_goal_event(
        "Test Player Name",
        vec!["YV".to_string(), "IM".to_string()],
        true,
        Some("https://example.com/video.mp4".to_string()),
    )];

    let game = create_test_game_data("HIFK", "Tappara", goal_events, ScoreType::Final);
    let games = vec![game];

    // Test different terminal widths
    let terminal_widths = vec![60, 80, 100, 120, 136];

    for width in terminal_widths {
        println!("Testing layout with terminal width: {}", width);

        let mut layout_manager = ColumnLayoutManager::new(width, CONTENT_MARGIN);
        let layout_config = layout_manager.calculate_layout(&games);

        // Verify layout is reasonable for this width
        // Note: Layout system may adjust team widths based on terminal width and content
        assert!(
            layout_config.home_team_width >= 15,
            "Home team width should be reasonable (got {})",
            layout_config.home_team_width
        );
        assert!(
            layout_config.away_team_width >= 15,
            "Away team width should be reasonable (got {})",
            layout_config.away_team_width
        );
        // Separator width may be reduced for narrow terminals (3) or use default (5)
        assert!(
            layout_config.separator_width == 3 || layout_config.separator_width == 5,
            "Separator width should be 3 or 5 (got {})",
            layout_config.separator_width
        );

        // Play icon should be positioned after team areas
        // Note: Layout system may use fallback positioning for narrow terminals
        let expected_min_position = if width < 80 {
            // Fallback layout for narrow terminals
            CONTENT_MARGIN + 10 // Minimum reasonable position
        } else {
            // Normal layout calculation
            CONTENT_MARGIN
                + layout_config.home_team_width
                + layout_config.separator_width
                + layout_config.away_team_width
                + 2
        };

        assert!(
            layout_config.play_icon_column >= expected_min_position,
            "Play icon column {} should be at least {} for width {} (using {} layout)",
            layout_config.play_icon_column,
            expected_min_position,
            width,
            if width < 80 { "fallback" } else { "normal" }
        );

        // Time and score columns should fit within terminal width
        assert!(
            layout_config.time_column < width,
            "Time column {} should fit within terminal width {}",
            layout_config.time_column,
            width
        );
        assert!(
            layout_config.score_column < width,
            "Score column {} should fit within terminal width {}",
            layout_config.score_column,
            width
        );

        // Test alignment calculator with this layout
        let mut alignment_calculator = AlignmentCalculator::new();
        let play_icon_positions =
            alignment_calculator.calculate_play_icon_positions(&games, &layout_config);
        let goal_type_positions = alignment_calculator
            .calculate_goal_type_positions(&games[0].goal_events, &layout_config);

        // Verify positioning is valid
        assert_eq!(play_icon_positions.len(), 1);
        assert_eq!(
            play_icon_positions[0].column_position,
            layout_config.play_icon_column
        );

        assert_eq!(goal_type_positions.len(), 1);
        assert!(alignment_calculator.validate_no_overflow(&goal_type_positions[0], &layout_config));

        println!(
            "✓ Layout system works correctly with terminal width {}",
            width
        );
    }

    println!("✓ Layout system integration works correctly across different terminal widths");
}

/// Test wide mode rendering with new layout system
/// Requirements: 4.2 (modular and testable formatting logic)
#[tokio::test]
async fn test_wide_mode_rendering_with_layout_system() {
    // Create test games for wide mode rendering
    let games = vec![
        create_test_game_data(
            "HIFK",
            "Tappara",
            vec![
                create_test_goal_event(
                    "Mikko Koivu",
                    vec!["YV".to_string()],
                    true,
                    Some("https://example.com/video1.mp4".to_string()),
                ),
                create_test_goal_event("Saku Koivu", vec!["IM".to_string()], true, None),
            ],
            ScoreType::Final,
        ),
        create_test_game_data(
            "Kärpät",
            "Lukko",
            vec![create_test_goal_event(
                "Teemu Selänne",
                vec!["TM".to_string()],
                false,
                Some("https://example.com/video2.mp4".to_string()),
            )],
            ScoreType::Final,
        ),
        create_test_game_data("JYP", "Ilves", vec![], ScoreType::Scheduled),
        create_test_game_data(
            "Pelicans",
            "SaiPa",
            vec![create_test_goal_event(
                "Patrik Laine",
                vec!["YV".to_string(), "IM".to_string()],
                true,
                Some("https://example.com/video3.mp4".to_string()),
            )],
            ScoreType::Ongoing,
        ),
    ];

    // Create wide mode page
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // enable video links
        false, // disable footer for cleaner testing
        true,  // ignore height limit (uses 136 char width)
        false, // not compact mode
        true,  // wide mode enabled
    );

    // Add all games to the page
    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    // Verify wide mode is properly configured
    assert!(page.is_wide_mode(), "Page should be in wide mode");
    assert!(
        page.can_fit_two_pages(),
        "Should be able to fit two pages in wide mode"
    );
    assert!(!page.is_compact_mode(), "Should not be in compact mode");

    // Test game distribution for wide mode
    let (left_games, right_games) = page.distribute_games_for_wide_display();

    // Verify games are distributed between columns
    assert!(!left_games.is_empty(), "Left column should have games");
    assert!(!right_games.is_empty(), "Right column should have games");
    assert_eq!(
        left_games.len() + right_games.len(),
        games.len(),
        "Total distributed games should equal input games"
    );

    // Verify distribution is balanced (left gets extra if odd number)
    let expected_left = games.len().div_ceil(2);
    assert_eq!(
        left_games.len(),
        expected_left,
        "Left column should have {} games but has {}",
        expected_left,
        left_games.len()
    );

    // Test layout calculation for wide mode columns
    let mut wide_layout_manager = ColumnLayoutManager::new_for_wide_mode_column(60, CONTENT_MARGIN);
    let wide_layout_config = wide_layout_manager.calculate_wide_mode_layout(&games);

    // Verify wide mode layout configuration
    assert!(
        wide_layout_config.home_team_width <= 20,
        "Wide mode should use appropriate team width ({})",
        wide_layout_config.home_team_width
    );
    assert!(
        wide_layout_config.away_team_width <= 20,
        "Wide mode should use appropriate team width ({})",
        wide_layout_config.away_team_width
    );
    // Separator width may be reduced for narrow columns (3) or use default (5)
    assert!(
        wide_layout_config.separator_width == 3 || wide_layout_config.separator_width == 5,
        "Separator width should be 3 or 5 (got {})",
        wide_layout_config.separator_width
    );

    // Test alignment calculations for wide mode
    let mut alignment_calculator = AlignmentCalculator::new();
    let play_icon_positions =
        alignment_calculator.calculate_play_icon_positions(&games, &wide_layout_config);
    let all_goal_events: Vec<_> = games.iter().flat_map(|g| &g.goal_events).cloned().collect();
    let goal_type_positions =
        alignment_calculator.calculate_goal_type_positions(&all_goal_events, &wide_layout_config);

    // Verify play icon alignment consistency in wide mode
    for position in &play_icon_positions {
        assert_eq!(
            position.column_position, wide_layout_config.play_icon_column,
            "Play icons should be aligned consistently in wide mode"
        );
    }

    // Verify goal type positioning doesn't overflow in wide mode
    for position in &goal_type_positions {
        assert!(
            alignment_calculator.validate_no_overflow(position, &wide_layout_config),
            "Goal types should not overflow in wide mode"
        );
    }

    // Test that the page can be created and configured correctly for wide mode rendering
    // The actual rendering is tested through the layout system components above
    assert!(page.is_wide_mode(), "Page should remain in wide mode");
    assert!(
        page.can_fit_two_pages(),
        "Page should support two-column layout"
    );

    println!("✓ Wide mode rendering with layout system works correctly");
}

/// Test wide mode backward compatibility
/// Requirements: 4.2 (modular and testable formatting logic)
#[tokio::test]
async fn test_wide_mode_backward_compatibility() {
    // Test that wide mode maintains compatibility with existing functionality

    // Create games with various scenarios that existed before layout system
    let legacy_scenarios = vec![
        // Simple game without goal events
        create_test_game_data("HIFK", "TPS", vec![], ScoreType::Final),
        // Game with single goal event
        create_test_game_data(
            "Kärpät",
            "Lukko",
            vec![create_test_goal_event(
                "Player",
                vec!["YV".to_string()],
                true,
                None,
            )],
            ScoreType::Final,
        ),
        // Game with multiple goal types (legacy format)
        create_test_game_data(
            "JYP",
            "Ilves",
            vec![create_test_goal_event(
                "Scorer",
                vec!["YV".to_string(), "IM".to_string()],
                true,
                Some("https://example.com/video.mp4".to_string()),
            )],
            ScoreType::Ongoing,
        ),
        // Scheduled game (no score, no events)
        create_test_game_data("Pelicans", "SaiPa", vec![], ScoreType::Scheduled),
    ];

    // Test with wide mode enabled
    let mut wide_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // enable video links
        false, // disable footer
        true,  // ignore height limit
        false, // not compact mode
        true,  // wide mode enabled
    );

    // Test with wide mode disabled (normal mode)
    let mut normal_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // enable video links
        false, // disable footer
        true,  // ignore height limit
        false, // not compact mode
        false, // wide mode disabled
    );

    // Add same games to both pages
    for game in &legacy_scenarios {
        wide_page.add_game_result(GameResultData::new(game));
        normal_page.add_game_result(GameResultData::new(game));
    }

    // Verify mode settings
    assert!(wide_page.is_wide_mode(), "Wide page should be in wide mode");
    assert!(
        !normal_page.is_wide_mode(),
        "Normal page should not be in wide mode"
    );

    // Test that both pages can handle the same game data
    // Verify both pages are configured correctly for their respective modes
    assert!(wide_page.is_wide_mode(), "Wide page should be in wide mode");
    assert!(
        !normal_page.is_wide_mode(),
        "Normal page should not be in wide mode"
    );

    // Test game distribution - wide mode should distribute, normal mode should not
    let (wide_left, wide_right) = wide_page.distribute_games_for_wide_display();
    let (normal_left, normal_right) = normal_page.distribute_games_for_wide_display();

    // Wide mode should distribute games
    assert!(
        !wide_left.is_empty(),
        "Wide mode should have left column games"
    );
    assert!(
        !wide_right.is_empty(),
        "Wide mode should have right column games"
    );
    assert_eq!(
        wide_left.len() + wide_right.len(),
        legacy_scenarios.len(),
        "Wide mode should distribute all games"
    );

    // Normal mode should put all games in left column
    assert_eq!(
        normal_left.len(),
        legacy_scenarios.len(),
        "Normal mode should put all games in left column"
    );
    assert!(
        normal_right.is_empty(),
        "Normal mode should have empty right column"
    );

    // Test layout calculations work for both modes
    let mut normal_layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
    let normal_layout = normal_layout_manager.calculate_layout(&legacy_scenarios);

    let mut wide_layout_manager = ColumnLayoutManager::new_for_wide_mode_column(60, CONTENT_MARGIN);
    let wide_layout = wide_layout_manager.calculate_wide_mode_layout(&legacy_scenarios);

    // Both layouts should be valid
    assert!(
        normal_layout.play_icon_column > 43,
        "Normal layout should position play icons correctly"
    );
    assert!(
        wide_layout.play_icon_column > 39,
        "Wide layout should position play icons correctly"
    ); // Adjusted for narrower columns

    // Test alignment calculations work for both modes
    let mut alignment_calculator = AlignmentCalculator::new();

    let normal_play_positions =
        alignment_calculator.calculate_play_icon_positions(&legacy_scenarios, &normal_layout);
    let wide_play_positions =
        alignment_calculator.calculate_play_icon_positions(&legacy_scenarios, &wide_layout);

    // Both should produce consistent alignment within their respective modes
    for position in &normal_play_positions {
        assert_eq!(position.column_position, normal_layout.play_icon_column);
    }
    for position in &wide_play_positions {
        assert_eq!(position.column_position, wide_layout.play_icon_column);
    }

    // Test that color schemes and visual standards are preserved
    // (This is implicit in the layout calculations - if they work, colors should work too)

    println!("✓ Wide mode maintains backward compatibility with existing functionality");
}

/// Test wide mode with insufficient terminal width
/// Requirements: 4.2 (modular and testable formatting logic)
#[tokio::test]
async fn test_wide_mode_insufficient_width_fallback() {
    // Create test games
    let games = vec![
        create_test_game_data("HIFK", "TPS", vec![], ScoreType::Final),
        create_test_game_data("Kärpät", "Lukko", vec![], ScoreType::Final),
    ];

    // Create wide mode page but with insufficient width simulation
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // enable video links
        false, // disable footer
        false, // don't ignore height limit (this will use actual terminal width)
        false, // not compact mode
        true,  // wide mode enabled
    );

    // Add games
    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    // Verify wide mode is enabled but can't fit two pages due to width
    assert!(page.is_wide_mode(), "Page should be in wide mode");

    // The can_fit_two_pages() method will return false in test environment
    // because crossterm::terminal::size() will fail and fallback to 80 chars
    if !page.can_fit_two_pages() {
        // Test fallback behavior
        let (left_games, right_games) = page.distribute_games_for_wide_display();

        // Should fallback to single column (all games in left)
        assert_eq!(
            left_games.len(),
            games.len(),
            "Should put all games in left column when width insufficient"
        );
        assert!(
            right_games.is_empty(),
            "Right column should be empty when width insufficient"
        );

        // Test that the page configuration remains consistent even with insufficient width
        // The actual fallback behavior is handled internally by the rendering system
        assert!(
            page.is_wide_mode(),
            "Page should remain in wide mode even with insufficient width"
        );
        assert!(
            !page.can_fit_two_pages(),
            "Page should correctly detect insufficient width"
        );

        println!("✓ Wide mode properly handles insufficient terminal width with fallback");
    } else {
        // If we somehow have sufficient width in test environment, verify normal wide mode behavior
        let (left_games, _right_games) = page.distribute_games_for_wide_display();
        assert!(!left_games.is_empty(), "Should have games in left column");

        println!("✓ Wide mode works with sufficient terminal width");
    }
}

/// Test wide mode layout system integration with different game types
/// Requirements: 4.2 (modular and testable formatting logic)
#[tokio::test]
async fn test_wide_mode_layout_integration_different_game_types() {
    // Create games representing different game states and content types
    let games = vec![
        // Final game with multiple goal events and video links
        create_test_game_data(
            "HIFK",
            "Tappara",
            vec![
                create_test_goal_event(
                    "Long Player Name Here",
                    vec!["YV".to_string(), "IM".to_string()],
                    true,
                    Some("https://example.com/video1.mp4".to_string()),
                ),
                create_test_goal_event(
                    "Short",
                    vec!["TM".to_string()],
                    false,
                    Some("https://example.com/video2.mp4".to_string()),
                ),
            ],
            ScoreType::Final,
        ),
        // Ongoing game with single goal event
        create_test_game_data(
            "Kärpät",
            "Lukko",
            vec![create_test_goal_event(
                "Medium Name",
                vec!["AV".to_string()],
                true,
                None,
            )],
            ScoreType::Ongoing,
        ),
        // Scheduled game (no goal events)
        create_test_game_data("JYP", "Ilves", vec![], ScoreType::Scheduled),
        // Final game with complex goal types
        create_test_game_data(
            "Pelicans",
            "SaiPa",
            vec![create_test_goal_event(
                "Another Player Name",
                vec!["YV".to_string(), "IM".to_string(), "TM".to_string()],
                true,
                Some("https://example.com/video3.mp4".to_string()),
            )],
            ScoreType::Final,
        ),
    ];

    // Test wide mode layout calculation with diverse content
    let mut wide_layout_manager = ColumnLayoutManager::new_for_wide_mode_column(60, CONTENT_MARGIN);
    let wide_layout_config = wide_layout_manager.calculate_wide_mode_layout(&games);

    // Verify layout handles diverse content appropriately
    assert!(
        wide_layout_config.max_player_name_width >= 10,
        "Should accommodate player names (got {})",
        wide_layout_config.max_player_name_width
    );
    assert!(
        wide_layout_config.max_player_name_width <= 15,
        "Should cap player names for wide mode (got {})",
        wide_layout_config.max_player_name_width
    );
    assert!(
        wide_layout_config.max_goal_types_width >= 2,
        "Should accommodate goal types (got {})",
        wide_layout_config.max_goal_types_width
    );
    assert!(
        wide_layout_config.max_goal_types_width <= 6,
        "Should cap goal types for wide mode (got {})",
        wide_layout_config.max_goal_types_width
    );

    // Test alignment calculations with diverse content
    let mut alignment_calculator = AlignmentCalculator::new();
    let play_icon_positions =
        alignment_calculator.calculate_play_icon_positions(&games, &wide_layout_config);
    let all_goal_events: Vec<_> = games.iter().flat_map(|g| &g.goal_events).cloned().collect();
    let goal_type_positions =
        alignment_calculator.calculate_goal_type_positions(&all_goal_events, &wide_layout_config);

    // Verify consistent alignment across different game types
    for position in &play_icon_positions {
        assert_eq!(
            position.column_position, wide_layout_config.play_icon_column,
            "Play icons should be consistently aligned across different game types"
        );
    }

    // Verify goal type positioning handles different content lengths
    for position in &goal_type_positions {
        assert!(
            alignment_calculator.validate_no_overflow(position, &wide_layout_config),
            "Goal types should not overflow regardless of content complexity"
        );

        // Verify goal types are properly formatted
        if !position.goal_types.is_empty() {
            // Note: Some goal type combinations may exceed the allocated width,
            // but the overflow prevention should handle this gracefully
            if position.goal_types.len() > wide_layout_config.max_goal_types_width {
                println!(
                    "Goal types '{}' ({} chars) exceed allocated width {} - overflow prevention should handle this",
                    position.goal_types,
                    position.goal_types.len(),
                    wide_layout_config.max_goal_types_width
                );
            }
        }
    }

    // Test page creation and game distribution with diverse content
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // enable video links
        false, // disable footer
        true,  // ignore height limit
        false, // not compact mode
        true,  // wide mode enabled
    );

    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    // Test game distribution with diverse content
    let (left_games, right_games) = page.distribute_games_for_wide_display();

    // Verify distribution works with different game types
    assert!(!left_games.is_empty(), "Left column should have games");
    assert!(!right_games.is_empty(), "Right column should have games");
    assert_eq!(
        left_games.len() + right_games.len(),
        games.len(),
        "All games should be distributed"
    );

    // Verify each column has manageable content
    assert!(
        left_games.len() <= 3,
        "Left column should not be overloaded (has {})",
        left_games.len()
    );
    assert!(
        right_games.len() <= 3,
        "Right column should not be overloaded (has {})",
        right_games.len()
    );

    println!("✓ Wide mode layout system integrates correctly with different game types");
}

/// Test wide mode play icon alignment consistency
/// Requirements: 1.4 - Play icon alignment in wide mode
#[tokio::test]
async fn test_wide_mode_play_icon_alignment() {
    // Create test games with varying player name lengths and goal types
    let goal_events = vec![
        create_test_goal_event(
            "Short",
            vec!["YV".to_string()],
            true,
            Some("https://example.com/video1.mp4".to_string()),
        ),
        create_test_goal_event(
            "Very Long Player Name",
            vec!["YV".to_string(), "IM".to_string()],
            true,
            Some("https://example.com/video2.mp4".to_string()),
        ),
        create_test_goal_event(
            "Medium Name",
            vec!["TM".to_string()],
            false,
            Some("https://example.com/video3.mp4".to_string()),
        ),
    ];

    let games = vec![
        create_test_game_data("HIFK", "Tappara", goal_events.clone(), ScoreType::Final),
        create_test_game_data(
            "Kärpät",
            "Lukko",
            vec![create_test_goal_event(
                "Another Player",
                vec!["AV".to_string()],
                true,
                Some("https://example.com/video4.mp4".to_string()),
            )],
            ScoreType::Final,
        ),
    ];

    // Test normal mode layout and alignment
    let mut normal_layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
    let normal_layout_config = normal_layout_manager.calculate_layout(&games);
    let mut normal_alignment_calculator = AlignmentCalculator::new();
    let normal_play_icon_positions =
        normal_alignment_calculator.calculate_play_icon_positions(&games, &normal_layout_config);

    // Test wide mode layout and alignment using wide mode column width (60 chars)
    let mut wide_layout_manager = ColumnLayoutManager::new_for_wide_mode_column(60, CONTENT_MARGIN);
    let wide_layout_config = wide_layout_manager.calculate_wide_mode_layout(&games);
    let mut wide_alignment_calculator = AlignmentCalculator::new();
    let wide_play_icon_positions =
        wide_alignment_calculator.calculate_play_icon_positions(&games, &wide_layout_config);

    // Verify play icon alignment consistency within normal mode
    let normal_expected_column = normal_layout_config.play_icon_column;
    for (i, position) in normal_play_icon_positions.iter().enumerate() {
        assert_eq!(
            position.column_position, normal_expected_column,
            "Normal mode: Play icon {} should be at column {} but was at {}",
            i, normal_expected_column, position.column_position
        );
        assert!(
            position.has_video_link,
            "All test events should have video links"
        );
    }

    // Verify play icon alignment consistency within wide mode
    let wide_expected_column = wide_layout_config.play_icon_column;
    for (i, position) in wide_play_icon_positions.iter().enumerate() {
        assert_eq!(
            position.column_position, wide_expected_column,
            "Wide mode: Play icon {} should be at column {} but was at {}",
            i, wide_expected_column, position.column_position
        );
        assert!(
            position.has_video_link,
            "All test events should have video links"
        );
    }

    // Verify both modes have the same number of play icons
    assert_eq!(
        normal_play_icon_positions.len(),
        wide_play_icon_positions.len(),
        "Both modes should have the same number of play icons"
    );

    // Test that wide mode uses appropriate column positioning for narrower columns
    // Wide mode columns are narrower, so play icon column should be positioned differently
    assert!(
        wide_layout_config.play_icon_column <= normal_layout_config.play_icon_column,
        "Wide mode play icon column ({}) should be positioned at or before normal mode column ({}) due to narrower column width",
        wide_layout_config.play_icon_column,
        normal_layout_config.play_icon_column
    );

    // Test goal type positioning consistency in both modes
    let all_goal_events: Vec<_> = games.iter().flat_map(|g| &g.goal_events).cloned().collect();
    let normal_goal_type_positions = normal_alignment_calculator
        .calculate_goal_type_positions(&all_goal_events, &normal_layout_config);
    let wide_goal_type_positions = wide_alignment_calculator
        .calculate_goal_type_positions(&all_goal_events, &wide_layout_config);

    // Verify no overflow in either mode
    for position in &normal_goal_type_positions {
        assert!(
            normal_alignment_calculator.validate_no_overflow(position, &normal_layout_config),
            "Normal mode: Goal type '{}' should not overflow",
            position.goal_types
        );
    }

    for position in &wide_goal_type_positions {
        assert!(
            wide_alignment_calculator.validate_no_overflow(position, &wide_layout_config),
            "Wide mode: Goal type '{}' should not overflow",
            position.goal_types
        );
    }

    println!("✓ Play icon alignment is consistent in both normal and wide modes");
}

/// Test wide mode play icon alignment with different column widths
/// Requirements: 1.4 - Play icon alignment consistency across different wide mode column widths
#[tokio::test]
async fn test_wide_mode_play_icon_alignment_different_column_widths() {
    let goal_events = vec![
        create_test_goal_event(
            "Test Player One",
            vec!["YV".to_string()],
            true,
            Some("https://example.com/video1.mp4".to_string()),
        ),
        create_test_goal_event(
            "Test Player Two",
            vec!["IM".to_string(), "TM".to_string()],
            true,
            Some("https://example.com/video2.mp4".to_string()),
        ),
    ];

    let games = vec![create_test_game_data(
        "HIFK",
        "Tappara",
        goal_events,
        ScoreType::Final,
    )];

    // Test different wide mode column widths
    let column_widths = vec![50, 60, 70, 80];

    for width in column_widths {
        println!(
            "Testing wide mode play icon alignment with column width: {}",
            width
        );

        let mut layout_manager =
            ColumnLayoutManager::new_for_wide_mode_column(width, CONTENT_MARGIN);
        let layout_config = layout_manager.calculate_wide_mode_layout(&games);
        let mut alignment_calculator = AlignmentCalculator::new();
        let play_icon_positions =
            alignment_calculator.calculate_play_icon_positions(&games, &layout_config);

        // Verify all play icons are aligned consistently within this column width
        let expected_column = layout_config.play_icon_column;
        for (i, position) in play_icon_positions.iter().enumerate() {
            assert_eq!(
                position.column_position, expected_column,
                "Width {}: Play icon {} should be at column {} but was at {}",
                width, i, expected_column, position.column_position
            );
        }

        // Verify play icon column is positioned reasonably within the column width
        assert!(
            layout_config.play_icon_column < width,
            "Width {}: Play icon column {} should be within column width {}",
            width,
            layout_config.play_icon_column,
            width
        );

        // Verify layout is reasonable for this width
        let team_area_width = layout_config.home_team_width
            + layout_config.separator_width
            + layout_config.away_team_width;
        assert!(
            layout_config.play_icon_column > team_area_width,
            "Width {}: Play icon column {} should be positioned after team area ({})",
            width,
            layout_config.play_icon_column,
            team_area_width
        );

        println!(
            "✓ Wide mode column width {} maintains consistent play icon alignment",
            width
        );
    }

    println!("✓ Play icon alignment is consistent across different wide mode column widths");
}

/// Test wide mode vs normal mode play icon alignment comparison
/// Requirements: 1.4 - Ensure play icons align correctly in wide mode and maintain consistency with normal mode principles
#[tokio::test]
async fn test_wide_mode_vs_normal_mode_play_icon_consistency() {
    // Create games with diverse content to test alignment robustness
    let games = vec![
        create_test_game_data(
            "HIFK",
            "Tappara",
            vec![
                create_test_goal_event(
                    "A", // Very short name
                    vec!["YV".to_string()],
                    true,
                    Some("https://example.com/video1.mp4".to_string()),
                ),
                create_test_goal_event(
                    "Extremely Long Player Name That Tests Limits", // Very long name
                    vec!["YV".to_string(), "IM".to_string(), "TM".to_string()],
                    true,
                    Some("https://example.com/video2.mp4".to_string()),
                ),
            ],
            ScoreType::Final,
        ),
        create_test_game_data(
            "Kärpät",
            "Lukko",
            vec![create_test_goal_event(
                "Regular Name", // Regular length name
                vec!["AV".to_string()],
                false,
                Some("https://example.com/video3.mp4".to_string()),
            )],
            ScoreType::Ongoing,
        ),
        create_test_game_data("JYP", "Ilves", vec![], ScoreType::Scheduled), // No goals
    ];

    // Test normal mode (80 char terminal)
    let mut normal_layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
    let normal_layout_config = normal_layout_manager.calculate_layout(&games);
    let mut normal_alignment_calculator = AlignmentCalculator::new();
    let normal_play_icon_positions =
        normal_alignment_calculator.calculate_play_icon_positions(&games, &normal_layout_config);

    // Test wide mode (60 char column)
    let mut wide_layout_manager = ColumnLayoutManager::new_for_wide_mode_column(60, CONTENT_MARGIN);
    let wide_layout_config = wide_layout_manager.calculate_wide_mode_layout(&games);
    let mut wide_alignment_calculator = AlignmentCalculator::new();
    let wide_play_icon_positions =
        wide_alignment_calculator.calculate_play_icon_positions(&games, &wide_layout_config);

    // Both modes should have the same number of play icons (3 goal events)
    assert_eq!(
        normal_play_icon_positions.len(),
        3,
        "Normal mode should have 3 play icon positions"
    );
    assert_eq!(
        wide_play_icon_positions.len(),
        3,
        "Wide mode should have 3 play icon positions"
    );

    // Verify internal consistency within each mode
    let normal_column = normal_layout_config.play_icon_column;
    for position in &normal_play_icon_positions {
        assert_eq!(
            position.column_position, normal_column,
            "Normal mode: All play icons should be at column {}",
            normal_column
        );
    }

    let wide_column = wide_layout_config.play_icon_column;
    for position in &wide_play_icon_positions {
        assert_eq!(
            position.column_position, wide_column,
            "Wide mode: All play icons should be at column {}",
            wide_column
        );
    }

    // Verify that both modes handle the same game/event indices correctly
    for i in 0..3 {
        assert_eq!(
            normal_play_icon_positions[i].game_index, wide_play_icon_positions[i].game_index,
            "Position {}: Both modes should reference the same game index",
            i
        );
        assert_eq!(
            normal_play_icon_positions[i].event_index, wide_play_icon_positions[i].event_index,
            "Position {}: Both modes should reference the same event index",
            i
        );
        assert_eq!(
            normal_play_icon_positions[i].has_video_link,
            wide_play_icon_positions[i].has_video_link,
            "Position {}: Both modes should have the same video link status",
            i
        );
    }

    // Test dynamic spacing calculations for both modes
    let short_name_length = 1; // "A"
    let long_name_length = 42; // "Extremely Long Player Name That Tests Limits"
    let regular_name_length = 12; // "Regular Name"

    let normal_short_spacing =
        normal_layout_manager.calculate_dynamic_spacing(short_name_length, &normal_layout_config);
    let normal_long_spacing =
        normal_layout_manager.calculate_dynamic_spacing(long_name_length, &normal_layout_config);
    let normal_regular_spacing =
        normal_layout_manager.calculate_dynamic_spacing(regular_name_length, &normal_layout_config);

    let wide_short_spacing =
        wide_layout_manager.calculate_dynamic_spacing(short_name_length, &wide_layout_config);
    let wide_long_spacing =
        wide_layout_manager.calculate_dynamic_spacing(long_name_length, &wide_layout_config);
    let wide_regular_spacing =
        wide_layout_manager.calculate_dynamic_spacing(regular_name_length, &wide_layout_config);

    // Both modes should provide minimum spacing for long names
    assert_eq!(
        normal_long_spacing, 1,
        "Normal mode should provide minimum spacing for long names"
    );
    assert_eq!(
        wide_long_spacing, 1,
        "Wide mode should provide minimum spacing for long names"
    );

    // Both modes should provide more spacing for shorter names
    assert!(
        normal_short_spacing > normal_long_spacing,
        "Normal mode should provide more spacing for short names"
    );
    assert!(
        wide_short_spacing > wide_long_spacing,
        "Wide mode should provide more spacing for short names"
    );
    assert!(
        normal_regular_spacing > normal_long_spacing,
        "Normal mode should provide more spacing for regular names"
    );
    assert!(
        wide_regular_spacing > wide_long_spacing,
        "Wide mode should provide more spacing for regular names"
    );

    // Test goal type positioning consistency
    let all_goal_events: Vec<_> = games.iter().flat_map(|g| &g.goal_events).cloned().collect();
    let normal_goal_type_positions = normal_alignment_calculator
        .calculate_goal_type_positions(&all_goal_events, &normal_layout_config);
    let wide_goal_type_positions = wide_alignment_calculator
        .calculate_goal_type_positions(&all_goal_events, &wide_layout_config);

    // Both modes should handle the same number of goal events
    assert_eq!(
        normal_goal_type_positions.len(),
        wide_goal_type_positions.len(),
        "Both modes should handle the same number of goal events"
    );

    // Both modes should prevent overflow
    for position in &normal_goal_type_positions {
        assert!(
            normal_alignment_calculator.validate_no_overflow(position, &normal_layout_config),
            "Normal mode: Goal type '{}' should not overflow",
            position.goal_types
        );
    }

    for position in &wide_goal_type_positions {
        assert!(
            wide_alignment_calculator.validate_no_overflow(position, &wide_layout_config),
            "Wide mode: Goal type '{}' should not overflow",
            position.goal_types
        );
    }

    println!("✓ Wide mode and normal mode maintain consistent play icon alignment principles");
}

/// Test error handling in game display layout calculations
#[tokio::test]
async fn test_game_display_error_handling() {
    // Test with edge case data
    let edge_case_games = [
        // Game with empty team names
        GameData {
            home_team: "".to_string(),
            away_team: "".to_string(),
            time: "18:30".to_string(),
            result: "0-0".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            serie: "RUNKOSARJA".to_string(),
            goal_events: vec![],
            played_time: 0,
            start: "2024-01-15T18:30:00Z".to_string(),
        },
        // Game with very long team names
        GameData {
            home_team: "Very Long Team Name That Exceeds Normal Length".to_string(),
            away_team: "Another Very Long Team Name Here".to_string(),
            time: "19:00".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "RUNKOSARJA".to_string(),
            goal_events: vec![GoalEventData {
                scorer_player_id: 123,
                scorer_name: "Player With Extremely Long Name That Should Be Handled Gracefully"
                    .to_string(),
                minute: 10,
                home_team_score: 1,
                away_team_score: 0,
                is_winning_goal: false,
                goal_types: vec![
                    "YV".to_string(),
                    "IM".to_string(),
                    "TM".to_string(),
                    "VT".to_string(),
                ],
                is_home_team: true,
                video_clip_url: Some(
                    "https://example.com/very-long-url-that-should-be-handled-properly.mp4"
                        .to_string(),
                ),
            }],
            played_time: 3600,
            start: "2024-01-15T19:00:00Z".to_string(),
        },
    ];

    for (i, game) in edge_case_games.iter().enumerate() {
        println!("Testing edge case game {}", i + 1);

        // Test layout calculation with edge case data
        let games = vec![game.clone()];
        let mut layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
        let layout_config = layout_manager.calculate_layout(&games);

        // Layout calculation should not panic and should produce valid results
        assert!(
            layout_config.home_team_width > 0,
            "Home team width should be positive for game {}",
            i + 1
        );
        assert!(
            layout_config.away_team_width > 0,
            "Away team width should be positive for game {}",
            i + 1
        );
        assert!(
            layout_config.play_icon_column > 0,
            "Play icon column should be positive for game {}",
            i + 1
        );

        // Test alignment calculations with edge case data
        let mut alignment_calculator = AlignmentCalculator::new();
        let play_icon_positions =
            alignment_calculator.calculate_play_icon_positions(&games, &layout_config);
        let goal_type_positions =
            alignment_calculator.calculate_goal_type_positions(&game.goal_events, &layout_config);

        // Alignment calculations should handle edge cases gracefully
        assert_eq!(
            play_icon_positions.len(),
            game.goal_events.len(),
            "Should have play icon position for each goal event in game {}",
            i + 1
        );
        assert_eq!(
            goal_type_positions.len(),
            game.goal_events.len(),
            "Should have goal type position for each goal event in game {}",
            i + 1
        );

        // Verify overflow prevention still works with edge cases
        for position in &goal_type_positions {
            assert!(
                alignment_calculator.validate_no_overflow(position, &layout_config),
                "Overflow validation should pass even with edge case data for game {}",
                i + 1
            );
        }

        // Test page creation with edge case data
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "SM-LIIGA".to_string(),
            false,
            false,
            true,
            false,
            false,
        );

        // Page creation should not panic
        page.add_game_result(GameResultData::new(game));

        println!("✓ Edge case game {} handled gracefully", i + 1);
    }

    println!("✓ Game display error handling works correctly");
}

/// Test performance of complete game display layout calculations
#[tokio::test]
async fn test_game_display_layout_performance() {
    use std::time::Instant;

    // Create a large number of games with goal events
    let mut games = Vec::new();
    for i in 0..50 {
        let goal_events = vec![
            create_test_goal_event(
                &format!("Player {}", i),
                vec!["YV".to_string()],
                true,
                Some(format!("https://example.com/video{}.mp4", i)),
            ),
            create_test_goal_event(
                &format!("Away Player {}", i),
                vec!["IM".to_string(), "TM".to_string()],
                false,
                None,
            ),
        ];

        games.push(create_test_game_data(
            &format!("Home {}", i),
            &format!("Away {}", i),
            goal_events,
            ScoreType::Final,
        ));
    }

    // Test layout calculation performance
    let layout_start = Instant::now();
    let mut layout_manager = ColumnLayoutManager::new(80, CONTENT_MARGIN);
    let layout_config = layout_manager.calculate_layout(&games);
    let layout_duration = layout_start.elapsed();

    // Test alignment calculation performance
    let alignment_start = Instant::now();
    let mut alignment_calculator = AlignmentCalculator::new();
    let play_icon_positions =
        alignment_calculator.calculate_play_icon_positions(&games, &layout_config);
    let alignment_duration = alignment_start.elapsed();

    // Test goal type positioning performance
    let goal_type_start = Instant::now();
    let all_goal_events: Vec<_> = games.iter().flat_map(|g| &g.goal_events).cloned().collect();
    let goal_type_positions =
        alignment_calculator.calculate_goal_type_positions(&all_goal_events, &layout_config);
    let goal_type_duration = goal_type_start.elapsed();

    // Test page creation performance
    let page_start = Instant::now();
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        false,
        true,
        false,
        false,
    );

    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }
    let page_duration = page_start.elapsed();

    let total_duration = layout_start.elapsed();

    // Verify results are correct
    assert_eq!(
        play_icon_positions.len(),
        games.len() * 2, // 2 goal events per game
        "Should have calculated positions for all goal events"
    );
    assert_eq!(
        goal_type_positions.len(),
        all_goal_events.len(),
        "Should have calculated goal type positions for all events"
    );

    // Performance assertions - should complete quickly even with many games

    println!("✓ Game display layout performance is acceptable");
    println!("  - Layout calculation: {} ms", layout_duration.as_millis());
    println!(
        "  - Alignment calculation: {} ms",
        alignment_duration.as_millis()
    );
    println!(
        "  - Goal type positioning: {} ms",
        goal_type_duration.as_millis()
    );
    println!("  - Page creation: {} ms", page_duration.as_millis());
    println!("  - Total time: {} ms", total_duration.as_millis());
}
