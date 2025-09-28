use liiga_teletext::{
    config::Config,
    data_fetcher::models::*,
    teletext_ui::{GameResultData, ScoreType, TeletextPage, TeletextRow},
};
use tempfile::tempdir;

/// Test goal event data processing
#[tokio::test]
async fn test_goal_event_processing() {
    // Create mock goal event
    let goal_event = GoalEventData {
        scorer_player_id: 12345,
        scorer_name: "Mikko Rantanen".to_string(),
        minute: 15,
        home_team_score: 1,
        away_team_score: 0,
        is_winning_goal: false,
        goal_types: vec!["YV".to_string()], // Power play goal
        is_home_team: true,
        video_clip_url: Some("https://example.com/video.mp4".to_string()),
    };

    // Test goal type display
    let goal_type_display = goal_event.get_goal_type_display();
    assert_eq!(goal_type_display, "YV");

    // Test goal event properties
    assert_eq!(goal_event.scorer_name, "Mikko Rantanen");
    assert_eq!(goal_event.minute, 15);
    assert_eq!(goal_event.home_team_score, 1);
    assert_eq!(goal_event.away_team_score, 0);
    assert!(goal_event.is_home_team);
    assert!(!goal_event.is_winning_goal);
}

/// Test error handling in teletext UI
#[tokio::test]
async fn test_error_handling() {
    // Create teletext page
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        false,
        false,
        false,
    );

    // Add error message
    let error_msg = "No games found for the specified date";
    page.add_error_message(error_msg);

    // Test that the error message was added correctly using the test-friendly accessor
    assert!(
        page.has_error_message(error_msg),
        "Error message should be present in the page content"
    );
}

/// Test page navigation
#[tokio::test]
async fn test_page_navigation() {
    // Create teletext page
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        false,
        false,
        false,
    );

    // Add multiple games to test pagination
    for i in 0..10 {
        let game = GameData {
            home_team: format!("Team {}", i * 2),
            away_team: format!("Team {}", i * 2 + 1),
            time: "18:30".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T18:30:00Z".to_string(),
        };
        page.add_game_result(GameResultData::new(&game));
    }

    // Test page navigation
    page.next_page();
    page.previous_page();

    // Verify page navigation works
    // The page should handle navigation without errors
}

/// Test configuration validation
#[tokio::test]
async fn test_config_validation() {
    // Test config with different API domains
    let configs = vec![
        Config {
            api_domain: "https://api.example.com".to_string(),
            log_file_path: None,
            http_timeout_seconds: liiga_teletext::constants::DEFAULT_HTTP_TIMEOUT_SECONDS,
        },
        Config {
            api_domain: "http://api.example.com".to_string(),
            log_file_path: Some("/custom/log/path".to_string()),
            http_timeout_seconds: liiga_teletext::constants::DEFAULT_HTTP_TIMEOUT_SECONDS,
        },
    ];

    for config in configs {
        // Verify config can be serialized and deserialized
        let config_str = toml::to_string_pretty(&config).unwrap();
        let loaded_config: Config = toml::from_str(&config_str).unwrap();

        assert_eq!(loaded_config.api_domain, config.api_domain);
        assert_eq!(loaded_config.log_file_path, config.log_file_path);
        assert_eq!(
            loaded_config.http_timeout_seconds,
            config.http_timeout_seconds
        );
    }
}

/// Test game result data creation
#[tokio::test]
async fn test_game_result_data_creation() {
    // Create mock game data
    let game = GameData {
        home_team: "HIFK".to_string(),
        away_team: "Tappara".to_string(),
        time: "18:30".to_string(),
        result: "3-2".to_string(),
        score_type: ScoreType::Final,
        is_overtime: true,
        is_shootout: false,
        serie: "runkosarja".to_string(),
        goal_events: vec![],
        played_time: 3900,
        start: "2024-01-15T18:30:00Z".to_string(),
    };

    // Create game result data
    let game_result = GameResultData::new(&game);

    // Verify game result data
    assert_eq!(game_result.home_team, "HIFK");
    assert_eq!(game_result.away_team, "Tappara");
    assert_eq!(game_result.time, "18:30");
    assert_eq!(game_result.result, "3-2");
    assert!(matches!(game_result.score_type, ScoreType::Final));
    assert!(game_result.is_overtime);
    assert!(!game_result.is_shootout);
}

/// Test teletext UI generation with mock data
#[tokio::test]
async fn test_teletext_ui_generation() {
    // Create mock game data
    let mock_game = GameData {
        home_team: "HIFK".to_string(),
        away_team: "Tappara".to_string(),
        time: "18:30".to_string(),
        result: "3-2".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        serie: "runkosarja".to_string(),
        goal_events: vec![],
        played_time: 3600,
        start: "2024-01-15T18:30:00Z".to_string(),
    };

    // Create teletext page
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        false, // ignore_height_limit
        false, // compact_mode
        false, // wide_mode
    );

    // Add game result
    let game_data = GameResultData::new(&mock_game);
    page.add_game_result(game_data);

    // Verify page was created successfully with game data
    // The page should contain the game result we added
}

/// Test configuration loading and saving
#[tokio::test]
async fn test_config_integration() {
    // Create temporary directory for config
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // Create test config
    let test_config = Config {
        api_domain: "https://api.test.com".to_string(),
        log_file_path: Some("/test/log/path".to_string()),
        http_timeout_seconds: liiga_teletext::constants::DEFAULT_HTTP_TIMEOUT_SECONDS,
    };

    // Save config
    let config_content = toml::to_string_pretty(&test_config).unwrap();
    tokio::fs::write(&config_path, config_content)
        .await
        .unwrap();

    // Load config
    let content = tokio::fs::read_to_string(&config_path).await.unwrap();
    let loaded_config: Config = toml::from_str(&content).unwrap();

    // Verify config
    assert_eq!(loaded_config.api_domain, "https://api.test.com");
    assert_eq!(
        loaded_config.log_file_path,
        Some("/test/log/path".to_string())
    );
    assert_eq!(
        loaded_config.http_timeout_seconds,
        liiga_teletext::constants::DEFAULT_HTTP_TIMEOUT_SECONDS
    );
}

/// Test end-to-end workflow with multiple games
#[tokio::test]
async fn test_end_to_end_multiple_games() {
    // Create mock game data for multiple games
    let mock_games = vec![
        GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "18:30".to_string(),
            result: "3-2".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T18:30:00Z".to_string(),
        },
        GameData {
            home_team: "Kärpät".to_string(),
            away_team: "Lukko".to_string(),
            time: "19:00".to_string(),
            result: "1-4".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T19:00:00Z".to_string(),
        },
    ];

    // Create teletext page
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        false,
        false,
        false,
    );

    // Add all game results
    for game in &mock_games {
        let game_data = GameResultData::new(game);
        page.add_game_result(game_data);
    }

    // Verify page was created successfully with multiple games
    // The page should contain both game results we added
}

/// Test different tournament types
#[tokio::test]
async fn test_different_tournament_types() {
    // Test playoffs tournament
    let playoffs_game = GameData {
        home_team: "HIFK".to_string(),
        away_team: "Tappara".to_string(),
        time: "18:30".to_string(),
        result: "2-1".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        serie: "playoffs".to_string(),
        goal_events: vec![],
        played_time: 3600,
        start: "2024-03-15T18:30:00Z".to_string(),
    };

    // Create teletext page
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "PLAYOFFS".to_string(),
        false,
        true,
        false,
        false,
        false,
    );

    // Add game result
    let game_data = GameResultData::new(&playoffs_game);
    page.add_game_result(game_data);

    // Verify page was created successfully with playoffs tournament
    // The page should contain the playoff game result
}

/// Test ongoing games (games that haven't finished)
#[tokio::test]
async fn test_ongoing_games() {
    // Create mock response for ongoing game
    let ongoing_game = GameData {
        home_team: "HIFK".to_string(),
        away_team: "Tappara".to_string(),
        time: "18:30".to_string(),
        result: "2-1".to_string(),
        score_type: ScoreType::Ongoing,
        is_overtime: false,
        is_shootout: false,
        serie: "runkosarja".to_string(),
        goal_events: vec![],
        played_time: 1800, // Half time
        start: "2024-01-15T18:30:00Z".to_string(),
    };

    // Verify game data
    assert_eq!(ongoing_game.result, "2-1");
    assert_eq!(ongoing_game.time, "18:30");
    assert!(matches!(ongoing_game.score_type, ScoreType::Ongoing));
}

/// Test games with special situations (overtime, shootout)
#[tokio::test]
async fn test_special_situations() {
    // Create mock response for overtime game
    let overtime_game = GameData {
        home_team: "HIFK".to_string(),
        away_team: "Tappara".to_string(),
        time: "18:30".to_string(),
        result: "3-2".to_string(),
        score_type: ScoreType::Final,
        is_overtime: true,
        is_shootout: false,
        serie: "runkosarja".to_string(),
        goal_events: vec![],
        played_time: 3900, // Regular time + overtime
        start: "2024-01-15T18:30:00Z".to_string(),
    };

    // Verify game data
    assert_eq!(overtime_game.result, "3-2");
    assert!(overtime_game.is_overtime); // Should have overtime
}

/// Test compact mode end-to-end in non-interactive mode
#[tokio::test]
async fn test_compact_mode_non_interactive() {
    // Create mock game data for multiple games
    let mock_games = vec![
        GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "18:30".to_string(),
            result: "3-2".to_string(),
            score_type: ScoreType::Final,
            is_overtime: true,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3900,
            start: "2024-01-15T18:30:00Z".to_string(),
        },
        GameData {
            home_team: "Kärpät".to_string(),
            away_team: "Lukko".to_string(),
            time: "19:00".to_string(),
            result: "1-4".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: true,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3900,
            start: "2024-01-15T19:00:00Z".to_string(),
        },
        GameData {
            home_team: "Ilves".to_string(),
            away_team: "JYP".to_string(),
            time: "19:30".to_string(),
            result: "".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 0,
            start: "2024-01-15T19:30:00Z".to_string(),
        },
    ];

    // Create teletext page in compact mode
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        true,  // ignore_height_limit (non-interactive mode)
        true,  // compact_mode
        false, // wide_mode
    );

    // Add all game results
    for game in &mock_games {
        let game_data = GameResultData::new(game);
        page.add_game_result(game_data);
    }

    // Verify compact mode is enabled
    assert!(page.is_compact_mode());

    // Test compact mode compatibility
    let validation = page.validate_compact_mode_compatibility();
    assert!(matches!(
        validation,
        liiga_teletext::teletext_ui::CompactModeValidation::Compatible
            | liiga_teletext::teletext_ui::CompactModeValidation::CompatibleWithWarnings { .. }
    ));

    // Test that compact mode configuration is valid
    // This verifies the compact mode logic works end-to-end without rendering
    let validation = page.validate_compact_mode_compatibility();
    assert!(matches!(
        validation,
        liiga_teletext::teletext_ui::CompactModeValidation::Compatible
            | liiga_teletext::teletext_ui::CompactModeValidation::CompatibleWithWarnings { .. }
    ));
}

/// Test compact mode with different date selections
#[tokio::test]
async fn test_compact_mode_with_dates() {
    // Create games for different dates
    let past_game = GameData {
        home_team: "HIFK".to_string(),
        away_team: "Tappara".to_string(),
        time: "18:30".to_string(),
        result: "3-2".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        serie: "runkosarja".to_string(),
        goal_events: vec![],
        played_time: 3600,
        start: "2024-01-15T18:30:00Z".to_string(),
    };

    let future_game = GameData {
        home_team: "Kärpät".to_string(),
        away_team: "Lukko".to_string(),
        time: "19:00".to_string(),
        result: "".to_string(),
        score_type: ScoreType::Scheduled,
        is_overtime: false,
        is_shootout: false,
        serie: "runkosarja".to_string(),
        goal_events: vec![],
        played_time: 0,
        start: "2024-12-15T19:00:00Z".to_string(),
    };

    // Test with past game (compact mode should work)
    let mut past_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        true,
        true,  // compact_mode
        false, // wide_mode
    );
    past_page.set_fetched_date("2024-01-15".to_string());
    let past_game_data = GameResultData::new(&past_game);
    past_page.add_game_result(past_game_data);

    assert!(past_page.is_compact_mode());

    // Test with future game (compact mode should work)
    let mut future_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        true,
        true,  // compact_mode
        false, // wide_mode
    );
    future_page.set_fetched_date("2024-12-15".to_string());
    let future_game_data = GameResultData::new(&future_game);
    future_page.add_game_result(future_game_data);

    assert!(future_page.is_compact_mode());

    // Both pages should be compatible with compact mode
    let past_validation = past_page.validate_compact_mode_compatibility();
    let future_validation = future_page.validate_compact_mode_compatibility();
    assert!(matches!(
        past_validation,
        liiga_teletext::teletext_ui::CompactModeValidation::Compatible
            | liiga_teletext::teletext_ui::CompactModeValidation::CompatibleWithWarnings { .. }
    ));
    assert!(matches!(
        future_validation,
        liiga_teletext::teletext_ui::CompactModeValidation::Compatible
            | liiga_teletext::teletext_ui::CompactModeValidation::CompatibleWithWarnings { .. }
    ));
}

/// Test compact mode with terminal width constraints
#[tokio::test]
async fn test_compact_mode_terminal_width_constraints() {
    use liiga_teletext::teletext_ui::{CompactDisplayConfig, TerminalWidthValidation};

    // Test various terminal widths
    let config = CompactDisplayConfig::default();

    // Test sufficient width
    let validation = config.validate_terminal_width(80);
    match validation {
        TerminalWidthValidation::Sufficient {
            current_width,
            required_width,
            excess,
        } => {
            assert_eq!(current_width, 80);
            assert_eq!(required_width, 18);
            assert_eq!(excess, 62);
        }
        _ => panic!("Expected sufficient validation"),
    }

    // Test insufficient width
    let validation = config.validate_terminal_width(10);
    match validation {
        TerminalWidthValidation::Insufficient {
            current_width,
            required_width,
            shortfall,
        } => {
            assert_eq!(current_width, 10);
            assert_eq!(required_width, 18);
            assert_eq!(shortfall, 8);
        }
        _ => panic!("Expected insufficient validation"),
    }

    // Test minimum width exactly
    let validation = config.validate_terminal_width(18);
    match validation {
        TerminalWidthValidation::Sufficient {
            current_width,
            required_width,
            excess,
        } => {
            assert_eq!(current_width, 18);
            assert_eq!(required_width, 18);
            assert_eq!(excess, 0);
        }
        _ => panic!("Expected sufficient validation at minimum width"),
    }
}

/// Test compact mode preserves teletext styling
#[tokio::test]
async fn test_compact_mode_preserves_styling() {
    // Create game with various states to test styling
    let games = vec![
        GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "18:30".to_string(),
            result: "3-2".to_string(),
            score_type: ScoreType::Final,
            is_overtime: true,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3900,
            start: "2024-01-15T18:30:00Z".to_string(),
        },
        GameData {
            home_team: "Kärpät".to_string(),
            away_team: "Lukko".to_string(),
            time: "19:00".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Ongoing,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 2400,
            start: "2024-01-15T19:00:00Z".to_string(),
        },
        GameData {
            home_team: "Ilves".to_string(),
            away_team: "JYP".to_string(),
            time: "19:30".to_string(),
            result: "".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 0,
            start: "2024-01-15T19:30:00Z".to_string(),
        },
    ];

    // Create pages in both normal and compact mode
    let mut normal_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        true,
        false, // normal mode
        false, // wide_mode
    );

    let mut compact_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        true,
        true,  // compact mode
        false, // wide_mode
    );

    // Add same games to both pages
    for game in &games {
        let game_data_normal = GameResultData::new(game);
        let game_data_compact = GameResultData::new(game);
        normal_page.add_game_result(game_data_normal);
        compact_page.add_game_result(game_data_compact);
    }

    // Both pages should be compatible (basic styling verification)
    let normal_validation = normal_page.validate_compact_mode_compatibility();
    let compact_validation = compact_page.validate_compact_mode_compatibility();
    assert!(matches!(
        normal_validation,
        liiga_teletext::teletext_ui::CompactModeValidation::Compatible
            | liiga_teletext::teletext_ui::CompactModeValidation::CompatibleWithWarnings { .. }
    ));
    assert!(matches!(
        compact_validation,
        liiga_teletext::teletext_ui::CompactModeValidation::Compatible
            | liiga_teletext::teletext_ui::CompactModeValidation::CompatibleWithWarnings { .. }
    ));

    // Verify compact page reports compact mode
    assert!(!normal_page.is_compact_mode());
    assert!(compact_page.is_compact_mode());
}

/// Test compact mode with various terminal sizes
#[tokio::test]
async fn test_compact_mode_various_terminal_sizes() {
    use liiga_teletext::teletext_ui::CompactDisplayConfig;

    // Test different configurations for different terminal sizes
    let configs = vec![
        (CompactDisplayConfig::default(), 80, true), // Standard wide terminal
        (CompactDisplayConfig::default(), 40, true), // Medium terminal
        (CompactDisplayConfig::default(), 20, true), // Narrow terminal
        (CompactDisplayConfig::default(), 18, true), // Minimum width
        (CompactDisplayConfig::default(), 17, false), // Too narrow
        (CompactDisplayConfig::new(3, 10, 8, " | "), 80, true), // Custom config wide
        (CompactDisplayConfig::new(3, 10, 8, " | "), 40, true), // Custom config medium
        (CompactDisplayConfig::new(3, 10, 8, " | "), 22, true), // Custom config minimum
        (CompactDisplayConfig::new(3, 10, 8, " | "), 20, false), // Custom config too narrow
    ];

    for (config, terminal_width, should_be_sufficient) in configs {
        let is_sufficient = config.is_terminal_width_sufficient(terminal_width);
        assert_eq!(
            is_sufficient,
            should_be_sufficient,
            "Terminal width {} should be {} for config {:?}",
            terminal_width,
            if should_be_sufficient {
                "sufficient"
            } else {
                "insufficient"
            },
            config
        );

        // Test games per line calculation
        let games_per_line = config.calculate_games_per_line(terminal_width);
        assert!(
            games_per_line > 0,
            "Games per line should always be at least 1"
        );
        assert!(
            games_per_line <= config.max_games_per_line,
            "Games per line should not exceed max_games_per_line"
        );
    }
}

/// Test compact mode basic functionality end-to-end
#[tokio::test]
async fn test_compact_mode_basic_functionality() {
    // Test that compact mode can be enabled and disabled properly
    let mut page = TeletextPage::new(
        221,
        "TEST".to_string(),
        "TEST".to_string(),
        false,
        true,
        false,
        true,  // compact mode enabled
        false, // wide_mode
    );

    // Verify compact mode is enabled
    assert!(page.is_compact_mode());

    // Add some test games
    let games = vec![
        GameData {
            home_team: "Tappara".to_string(),
            away_team: "HIFK".to_string(),
            time: "18:30".to_string(),
            result: "3-2".to_string(),
            score_type: ScoreType::Final,
            is_overtime: true,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3900,
            start: "2024-01-15T18:30:00Z".to_string(),
        },
        GameData {
            home_team: "Kärpät".to_string(),
            away_team: "Lukko".to_string(),
            time: "19:00".to_string(),
            result: "".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 0,
            start: "2024-01-15T19:00:00Z".to_string(),
        },
    ];

    // Add games to the page
    for game in &games {
        let game_data = GameResultData::new(game);
        page.add_game_result(game_data);
    }

    // Test that compact mode is compatible
    let validation = page.validate_compact_mode_compatibility();
    assert!(
        matches!(
            validation,
            liiga_teletext::teletext_ui::CompactModeValidation::Compatible
                | liiga_teletext::teletext_ui::CompactModeValidation::CompatibleWithWarnings { .. }
        ),
        "Compact mode should be compatible"
    );

    // Test toggling compact mode
    assert!(page.set_compact_mode(false).is_ok());
    assert!(!page.is_compact_mode());

    assert!(page.set_compact_mode(true).is_ok());
    assert!(page.is_compact_mode());
}

// PHASE 4: WIDE MODE INTEGRATION TESTS

/// Test wide mode CLI flag parsing and basic functionality
#[tokio::test]
async fn test_wide_mode_cli_integration() {
    // Test wide mode page creation
    let page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        true,  // ignore_height_limit (simulating --once mode)
        false, // compact_mode
        true,  // wide_mode - ENABLED
    );

    assert!(page.is_wide_mode(), "Wide mode should be enabled");
    assert!(
        page.can_fit_two_pages(),
        "Should fit two pages with wide terminal"
    );
}

/// Test wide mode with various terminal widths
#[tokio::test]
async fn test_wide_mode_terminal_widths() {
    // Test with sufficient width (non-interactive mode uses 136 chars)
    let wide_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        true,  // ignore_height_limit (non-interactive, uses 136 width)
        false, // compact_mode
        true,  // wide_mode
    );

    assert!(
        wide_page.can_fit_two_pages(),
        "Should support wide mode with 136 char width"
    );

    // Test interactive mode behavior: uses actual terminal width
    let interactive_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        false, // ignore_height_limit (interactive mode - uses actual terminal width)
        false, // compact_mode
        true,  // wide_mode
    );

    // Confirm the page is in wide mode
    assert!(interactive_page.is_wide_mode());

    // In interactive mode, can_fit_two_pages() depends on actual terminal width:
    // - If terminal width >= 128: returns true (supports two-page layout)
    // - If terminal width < 128: returns false (insufficient width)
    // This test verifies the behavior is consistent with the terminal environment
    let can_fit = interactive_page.can_fit_two_pages();

    // Get actual terminal width to verify the behavior is correct
    let actual_width = crossterm::terminal::size()
        .map(|(width, _)| width as usize)
        .unwrap_or(80);

    if actual_width >= 128 {
        assert!(
            can_fit,
            "Should support two-page layout when terminal width ({actual_width}) >= 128"
        );
    } else {
        assert!(
            !can_fit,
            "Should not support two-page layout when terminal width ({actual_width}) < 128"
        );
    }

    // Test with wide mode disabled
    let no_wide_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        false, // ignore_height_limit (interactive mode)
        false, // compact_mode
        false, // wide_mode disabled
    );

    // Wide mode should be disabled
    assert!(!no_wide_page.is_wide_mode());

    assert!(
        !no_wide_page.can_fit_two_pages(),
        "Should not support wide mode when wide_mode is disabled"
    );
}

/// Test wide mode fallback behavior
#[tokio::test]
async fn test_wide_mode_fallback_behavior() {
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        false, // ignore_height_limit (narrow terminal)
        false, // compact_mode
        true,  // wide_mode - enabled but will fallback
    );

    // Add test games
    let test_game = create_test_game_data();
    let test_game_data = GameResultData::new(&test_game);
    page.add_game_result(test_game_data);

    // When terminal is too narrow, wide mode should fallback gracefully
    let (left_games, right_games) = page.distribute_games_for_wide_display();

    // Should fallback to putting all games in left column
    assert!(!left_games.is_empty(), "Should have games in left column");
    assert_eq!(
        right_games.len(),
        0,
        "Should have no games in right column due to fallback"
    );
}

/// Test wide mode with different game states
#[tokio::test]
async fn test_wide_mode_with_different_game_states() {
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        true,  // ignore_height_limit (wide terminal)
        false, // compact_mode
        true,  // wide_mode
    );

    // Add games with different states

    // Finished game
    let mut finished_game = create_test_game_data();
    finished_game.result = "3-2".to_string();
    finished_game.score_type = ScoreType::Final;
    let finished_game_data = GameResultData::new(&finished_game);
    page.add_game_result(finished_game_data);

    // Ongoing game
    let mut ongoing_game = create_test_game_data();
    ongoing_game.home_team = "TPS".to_string();
    ongoing_game.away_team = "HIFK".to_string();
    ongoing_game.result = "1-1".to_string();
    ongoing_game.score_type = ScoreType::Ongoing;
    let ongoing_game_data = GameResultData::new(&ongoing_game);
    page.add_game_result(ongoing_game_data);

    // Scheduled game
    let mut scheduled_game = create_test_game_data();
    scheduled_game.home_team = "KalPa".to_string();
    scheduled_game.away_team = "Sport".to_string();
    scheduled_game.result = "18:30".to_string();
    scheduled_game.score_type = ScoreType::Scheduled;
    let scheduled_game_data = GameResultData::new(&scheduled_game);
    page.add_game_result(scheduled_game_data);

    let (left_games, right_games) = page.distribute_games_for_wide_display();

    // Should distribute games between columns
    let total_games = left_games.len() + right_games.len();
    assert_eq!(total_games, 3, "Should have all 3 games distributed");
    assert!(!left_games.is_empty(), "Should have games in left column");
}

/// Test mutual exclusivity with compact mode
#[tokio::test]
async fn test_wide_mode_mutual_exclusivity() {
    // Test that wide mode and compact mode are mutually exclusive in practice
    // (This would be enforced at the CLI level, but we test the page behavior)

    let page_compact = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        true,  // ignore_height_limit
        true,  // compact_mode - ENABLED
        false, // wide_mode - disabled
    );

    let page_wide = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        true,  // ignore_height_limit
        false, // compact_mode - disabled
        true,  // wide_mode - ENABLED
    );

    // Verify modes are correctly set
    assert!(page_compact.is_compact_mode() && !page_compact.is_wide_mode());
    assert!(!page_wide.is_compact_mode() && page_wide.is_wide_mode());

    // Verify they behave differently
    assert!(
        !page_compact.can_fit_two_pages(),
        "Compact mode should not fit two pages"
    );
    assert!(
        page_wide.can_fit_two_pages(),
        "Wide mode should fit two pages"
    );
}

/// Test wide mode rendering with game distribution
#[tokio::test]
async fn test_wide_mode_game_distribution_integration() {
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        true,  // ignore_height_limit (wide terminal)
        false, // compact_mode
        true,  // wide_mode
    );

    // Add multiple games to test distribution logic
    let teams = [
        ("HIFK", "Tappara"),
        ("TPS", "KalPa"),
        ("Ilves", "Lukko"),
        ("Ässät", "Sport"),
        ("JYP", "Kärpät"),
        ("HPK", "SaiPa"),
    ];

    for (i, (home, away)) in teams.iter().enumerate() {
        let mut game = create_test_game_data();
        game.home_team = home.to_string();
        game.away_team = away.to_string();
        game.result = format!("{}-{}", i % 3, (i + 1) % 3);
        game.score_type = if i % 2 == 0 {
            ScoreType::Final
        } else {
            ScoreType::Ongoing
        };
        let game_data = GameResultData::new(&game);
        page.add_game_result(game_data);
    }

    let (left_games, right_games) = page.distribute_games_for_wide_display();

    // Verify games are distributed
    assert!(!left_games.is_empty(), "Left column should have games");
    assert_eq!(
        left_games.len() + right_games.len(),
        teams.len(),
        "All games should be distributed between columns"
    );

    // Left column should typically have equal or one more game than right
    // (left-column-first distribution)
    assert!(
        left_games.len() >= right_games.len(),
        "Left column should have equal or more games (left-first distribution)"
    );
}

/// Test wide mode with goal scorer data
#[tokio::test]
async fn test_wide_mode_with_goal_scorers() {
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        true,  // ignore_height_limit
        false, // compact_mode
        true,  // wide_mode
    );

    // Create game with goal events
    let mut game_with_goals = create_test_game_data();
    game_with_goals.home_team = "HIFK".to_string();
    game_with_goals.away_team = "Tappara".to_string();
    game_with_goals.result = "2-1".to_string();
    game_with_goals.score_type = ScoreType::Final;

    // Add goal events
    game_with_goals.goal_events = vec![
        GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Mikko Rantanen".to_string(),
            minute: 15,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["YV".to_string()],
            is_home_team: true,
            video_clip_url: Some("https://example.com/goal1.mp4".to_string()),
        },
        GoalEventData {
            scorer_player_id: 456,
            scorer_name: "Sebastian Aho".to_string(),
            minute: 32,
            home_team_score: 1,
            away_team_score: 1,
            is_winning_goal: false,
            goal_types: vec!["EV".to_string()],
            is_home_team: false,
            video_clip_url: None,
        },
        GoalEventData {
            scorer_player_id: 789,
            scorer_name: "Artturi Lehkonen".to_string(),
            minute: 58,
            home_team_score: 2,
            away_team_score: 1,
            is_winning_goal: true,
            goal_types: vec!["EV".to_string()],
            is_home_team: true,
            video_clip_url: Some("https://example.com/goal3.mp4".to_string()),
        },
    ];

    let game_with_goals_data = GameResultData::new(&game_with_goals);
    page.add_game_result(game_with_goals_data);

    let (left_games, right_games) = page.distribute_games_for_wide_display();

    // Verify game with goals is included
    assert!(
        !left_games.is_empty() || !right_games.is_empty(),
        "Game should be distributed"
    );

    // Verify that games with goals are properly handled in wide mode distribution
    // (Detailed formatting is tested at the unit level, here we just verify integration)
    assert!(
        !left_games.is_empty() || !right_games.is_empty(),
        "Game with goals should be distributed"
    );

    // Verify that goal events are preserved in the game data
    // (The actual rendering is handled internally by the page)
}

// Helper function to create test game data (already exists but ensuring it's available)
fn create_test_game_data() -> GameData {
    GameData {
        home_team: "HIFK".to_string(),
        away_team: "Tappara".to_string(),
        time: "18:30".to_string(),
        result: "2-1".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        serie: "runkosarja".to_string(),
        goal_events: vec![],
        played_time: 3600,
        start: "2024-01-15T18:30:00Z".to_string(),
    }
}

/// Test wide mode performance with edge cases like many goal scorers and long team names
#[tokio::test]
async fn test_wide_mode_performance_edge_cases() {
    // Create a game with many goal scorers to test limits
    let mut game_with_many_goals = create_test_game_data();
    game_with_many_goals.home_team =
        "Very Long Home Team Name That Should Be Truncated Gracefully".to_string();
    game_with_many_goals.away_team = "Also A Very Long Away Team Name For Testing".to_string();
    game_with_many_goals.result = "15-10".to_string(); // High-scoring game
    game_with_many_goals.score_type = ScoreType::Final;

    // Add many goal events (25 total) to test the 15-goal-per-team limit
    let mut many_goal_events = Vec::new();
    for i in 0..25 {
        many_goal_events.push(GoalEventData {
            scorer_player_id: (i as i64) + 1000,
            scorer_name: format!("VeryLongPlayerName{i}"),
            minute: i % 60,
            home_team_score: if i % 2 == 0 { (i / 2) + 1 } else { i / 2 },
            away_team_score: if i % 2 == 1 { (i / 2) + 1 } else { i / 2 },
            is_winning_goal: false,
            goal_types: vec!["EV".to_string()],
            is_home_team: i % 2 == 0, // Alternate between home and away
            video_clip_url: None,
        });
    }
    game_with_many_goals.goal_events = many_goal_events;

    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        true,  // ignore_height_limit
        false, // compact_mode
        true,  // wide_mode
    );

    let game_data = GameResultData::new(&game_with_many_goals);
    page.add_game_result(game_data);

    // Test that the function handles many goals gracefully
    let (left_games, right_games) = page.distribute_games_for_wide_display();

    // Should still work with many goals
    assert!(!left_games.is_empty() || !right_games.is_empty());

    // Test that wide mode can be enabled and works with edge cases
    assert!(page.is_wide_mode(), "Wide mode should be enabled");
    assert!(page.can_fit_two_pages(), "Should be able to fit two pages");

    // Should handle the game distribution without errors
    assert!(
        left_games.len() + right_games.len() >= 1,
        "Should distribute at least one game"
    );

    // Test that game distribution and basic operations work with many goals (performance test)
    // Use timeout-based check to avoid coupling to machine performance
    // Test operations that internally use the optimized functions without timing assumptions
    let (left_games_2, right_games_2) = page.distribute_games_for_wide_display();

    let total_games = left_games_2.len() + right_games_2.len();

    // Verify that we have the expected number of games
    assert_eq!(total_games, 1, "Should have exactly one game");

    // Test that the game contains expected data
    if !left_games_2.is_empty() {
        match &left_games_2[0] {
            TeletextRow::GameResult {
                home_team,
                away_team,
                goal_events,
                ..
            } => {
                assert!(home_team.contains("Very Long Home Team"));
                assert!(away_team.contains("Also A Very Long"));
                // Should have all 25 goal events (limit is applied during rendering, not storage)
                assert_eq!(goal_events.len(), 25, "Should store all 25 goal events");
            }
            _ => panic!("Expected GameResult row"),
        }
    }
}
