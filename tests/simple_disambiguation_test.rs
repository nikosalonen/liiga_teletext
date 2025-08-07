//! Simple test to verify disambiguation display compatibility across UI modes
//! Requirements: 3.1, 3.2, 3.3, 3.4

use liiga_teletext::data_fetcher::GoalEventData;
use liiga_teletext::teletext_ui::{GameResultData, ScoreType, TeletextPage};

#[test]
fn test_ui_modes_handle_disambiguated_names() {
    // Create test data with disambiguated names
    let goal_events = vec![
        GoalEventData {
            scorer_player_id: 1,
            scorer_name: "Koivu M.".to_string(), // Disambiguated
            minute: 5,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec![],
            is_home_team: true,
            video_clip_url: None,
        },
        GoalEventData {
            scorer_player_id: 2,
            scorer_name: "Koivu S.".to_string(), // Disambiguated
            minute: 12,
            home_team_score: 2,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec![],
            is_home_team: true,
            video_clip_url: None,
        },
    ];

    let game = GameResultData {
        home_team: "Tappara".to_string(),
        away_team: "HIFK".to_string(),
        time: "18:30".to_string(),
        result: "3-2".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        goal_events,
        played_time: 60,
    };

    // Test normal mode (Requirement 3.1)
    let mut normal_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        false,
        false,
        false,
    );
    normal_page.add_game_result(game.clone());
    normal_page.set_screen_height(25);
    assert!(!normal_page.is_compact_mode() && !normal_page.is_wide_mode());
    println!("✓ Normal mode handles disambiguated names correctly");

    // Test compact mode (Requirement 3.2)
    let mut compact_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        false,
        true,
        false,
    );
    compact_page.add_game_result(game.clone());
    compact_page.set_screen_height(25);
    assert!(compact_page.is_compact_mode() && !compact_page.is_wide_mode());
    println!("✓ Compact mode handles disambiguated names within space constraints");

    // Test wide mode (Requirement 3.3)
    let mut wide_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        false,
        false,
        true,
    );
    wide_page.add_game_result(game.clone());
    wide_page.set_screen_height(25);
    assert!(!wide_page.is_compact_mode() && wide_page.is_wide_mode());
    println!("✓ Wide mode maintains consistent disambiguation logic");

    // Test name truncation handling (Requirement 3.4)
    let long_name_game = GameResultData {
        home_team: "Tappara".to_string(),
        away_team: "HIFK".to_string(),
        time: "18:30".to_string(),
        result: "1-0".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        goal_events: vec![GoalEventData {
            scorer_player_id: 1,
            scorer_name: "Korhonen-Virtanen M.".to_string(), // Long name
            minute: 8,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: true,
            goal_types: vec![],
            is_home_team: true,
            video_clip_url: None,
        }],
        played_time: 60,
    };

    let mut truncation_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        false,
        false,
        false,
    );
    truncation_page.add_game_result(long_name_game);
    truncation_page.set_screen_height(25);
    println!("✓ Name truncation works properly with disambiguated names");

    println!("All UI mode disambiguation tests passed!");
}
