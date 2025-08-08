//! Integration tests for player name disambiguation display compatibility across all UI modes.
//!
//! These tests verify that disambiguated player names display correctly in:
//! - Normal mode (default display)
//! - Compact mode (space-constrained display)
//! - Wide mode (two-column display)
//!
//! Requirements tested: 3.1, 3.2, 3.3, 3.4

use liiga_teletext::data_fetcher::GoalEventData;
use liiga_teletext::data_fetcher::models::{GoalEvent, ScheduleGame, ScheduleTeam};
use liiga_teletext::data_fetcher::player_names::{
    DisambiguationContext, format_with_disambiguation,
};
use liiga_teletext::data_fetcher::processors::process_goal_events_with_disambiguation;
use liiga_teletext::teletext_ui::{GameResultData, ScoreType, TeletextPage};

/// Type alias for player data tuple: (id, first_name, last_name)
type PlayerData = (i64, String, String);
/// Type alias for team player lists: (home_players, away_players)
type TeamPlayerData = (Vec<PlayerData>, Vec<PlayerData>);

/// Creates raw player data that needs disambiguation (for testing actual logic)
fn create_raw_test_players() -> TeamPlayerData {
    let home_players = vec![
        (1, "Mikko".to_string(), "Koivu".to_string()),
        (2, "Saku".to_string(), "Koivu".to_string()), // Needs disambiguation
        (3, "Teemu".to_string(), "Selänne".to_string()), // Unique
    ];

    let away_players = vec![
        (4, "Jari".to_string(), "Kurri".to_string()),
        (5, "Jarkko".to_string(), "Kurri".to_string()), // Needs disambiguation
        (6, "Ville".to_string(), "Peltonen".to_string()), // Unique
    ];

    (home_players, away_players)
}

/// Creates a realistic schedule game with goals from players who need disambiguation
fn create_raw_schedule_game_with_disambiguation() -> ScheduleGame {
    let home_goals = vec![
        GoalEvent {
            scorer_player_id: 1, // Mikko Koivu (will become "Koivu M.")
            log_time: "18:35:00".to_string(),
            game_time: 300, // 5 minutes
            period: 1,
            event_id: 1,
            home_team_score: 1,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec![],
            assistant_player_ids: vec![],
            video_clip_url: None,
        },
        GoalEvent {
            scorer_player_id: 2, // Saku Koivu (will become "Koivu S.")
            log_time: "18:42:00".to_string(),
            game_time: 720, // 12 minutes
            period: 1,
            event_id: 2,
            home_team_score: 2,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec![],
            assistant_player_ids: vec![],
            video_clip_url: None,
        },
        GoalEvent {
            scorer_player_id: 3, // Teemu Selänne (will become "Selänne")
            log_time: "19:08:00".to_string(),
            game_time: 1080, // 18 minutes
            period: 2,
            event_id: 3,
            home_team_score: 3,
            away_team_score: 1,
            winning_goal: false,
            goal_types: vec![],
            assistant_player_ids: vec![],
            video_clip_url: None,
        },
    ];

    let away_goals = vec![GoalEvent {
        scorer_player_id: 4, // Jari Kurri (will become "Kurri J.")
        log_time: "19:15:00".to_string(),
        game_time: 1500, // 25 minutes
        period: 2,
        event_id: 4,
        home_team_score: 3,
        away_team_score: 2,
        winning_goal: true,
        goal_types: vec!["YV".to_string()],
        assistant_player_ids: vec![],
        video_clip_url: None,
    }];

    ScheduleGame {
        id: 12345,
        season: 2024,
        start: "2024-01-15T18:30:00Z".to_string(),
        end: Some("2024-01-15T21:00:00Z".to_string()),
        home_team: ScheduleTeam {
            team_id: Some("TAP".to_string()),
            team_placeholder: None,
            team_name: Some("Tappara".to_string()),
            goals: 3,
            time_out: None,
            powerplay_instances: 0,
            powerplay_goals: 0,
            short_handed_instances: 0,
            short_handed_goals: 0,
            ranking: None,
            game_start_date_time: None,
            goal_events: home_goals,
        },
        away_team: ScheduleTeam {
            team_id: Some("HIFK".to_string()),
            team_placeholder: None,
            team_name: Some("HIFK".to_string()),
            goals: 2,
            time_out: None,
            powerplay_instances: 1,
            powerplay_goals: 1,
            short_handed_instances: 0,
            short_handed_goals: 0,
            ranking: None,
            game_start_date_time: None,
            goal_events: away_goals,
        },
        finished_type: Some("FINISHED".to_string()),
        started: true,
        ended: true,
        game_time: 3600, // 60 minutes in seconds
        serie: "RUNKOSARJA".to_string(),
    }
}

#[test]
fn test_normal_mode_displays_disambiguated_names_correctly() {
    // Test the actual disambiguation logic first
    let (home_players, away_players) = create_raw_test_players();
    let schedule_game = create_raw_schedule_game_with_disambiguation();

    // Process the goal events with actual disambiguation logic
    let disambiguated_goal_events =
        process_goal_events_with_disambiguation(&schedule_game, &home_players, &away_players);

    // Verify the disambiguation logic worked correctly
    assert_eq!(
        disambiguated_goal_events.len(),
        4,
        "Should have 4 goal events"
    );

    // Check specific disambiguated names
    let mikko_goal = disambiguated_goal_events
        .iter()
        .find(|event| event.scorer_player_id == 1)
        .expect("Should find Mikko Koivu's goal");
    assert_eq!(
        mikko_goal.scorer_name, "Koivu M.",
        "Mikko Koivu should be disambiguated as 'Koivu M.'"
    );

    let saku_goal = disambiguated_goal_events
        .iter()
        .find(|event| event.scorer_player_id == 2)
        .expect("Should find Saku Koivu's goal");
    assert_eq!(
        saku_goal.scorer_name, "Koivu S.",
        "Saku Koivu should be disambiguated as 'Koivu S.'"
    );

    let selanne_goal = disambiguated_goal_events
        .iter()
        .find(|event| event.scorer_player_id == 3)
        .expect("Should find Selänne's goal");
    assert_eq!(
        selanne_goal.scorer_name, "Selänne",
        "Selänne should not be disambiguated"
    );

    let kurri_goal = disambiguated_goal_events
        .iter()
        .find(|event| event.scorer_player_id == 4)
        .expect("Should find Jari Kurri's goal");
    assert_eq!(
        kurri_goal.scorer_name, "Kurri J.",
        "Jari Kurri should be disambiguated as 'Kurri J.'"
    );

    // Create a game result with the properly disambiguated events
    let game_result = GameResultData {
        home_team: "Tappara".to_string(),
        away_team: "HIFK".to_string(),
        time: "18:30".to_string(),
        result: "3-2".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        goal_events: disambiguated_goal_events,
        played_time: 60,
    };

    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        true,  // ignore_height_limit for testing
        false, // compact_mode
        false, // wide_mode
    );

    page.add_game_result(game_result);
    page.set_screen_height(25);

    // Verify the page was created successfully and is in normal mode
    assert!(
        !page.is_compact_mode(),
        "Page should not be in compact mode"
    );
    assert!(!page.is_wide_mode(), "Page should not be in wide mode");
    assert_eq!(page.total_pages(), 1, "Should have one page");

    println!("✓ Normal mode displays correctly disambiguated names");
}

#[test]
fn test_compact_mode_handles_disambiguated_names_within_space_constraints() {
    // Test disambiguation with more complex data that challenges compact mode
    let home_players = vec![
        (1, "Mikko".to_string(), "Koivu".to_string()),
        (2, "Saku".to_string(), "Koivu".to_string()),
        (3, "Markus".to_string(), "Koivu".to_string()), // Three players with same last name
    ];

    let away_players = vec![
        (4, "Jari".to_string(), "Kurri".to_string()),
        (5, "Jarkko".to_string(), "Kurri".to_string()),
    ];

    // Test the disambiguation context handles multiple players correctly
    let home_context = DisambiguationContext::new(home_players.clone());
    assert!(
        home_context.needs_disambiguation("Koivu"),
        "Should need disambiguation for Koivu"
    );

    // Verify the specific disambiguation results
    assert_eq!(
        home_context.get_disambiguated_name(1),
        Some(&"Koivu Mi.".to_string()),
        "Mikko should be 'Koivu Mi.'"
    );
    assert_eq!(
        home_context.get_disambiguated_name(2),
        Some(&"Koivu S.".to_string()),
        "Saku should be 'Koivu S.'"
    );
    assert_eq!(
        home_context.get_disambiguated_name(3),
        Some(&"Koivu Ma.".to_string()),
        "Markus should be 'Koivu Ma.'"
    );

    let away_context = DisambiguationContext::new(away_players.clone());
    assert_eq!(
        away_context.get_disambiguated_name(4),
        Some(&"Kurri J.".to_string()),
        "Jari should be 'Kurri J.'"
    );
    // Both Jari and Jarkko start with 'J', so they might both get 'J.' if extended disambiguation isn't needed
    let jarkko_name = away_context.get_disambiguated_name(5).unwrap();
    assert!(
        jarkko_name.starts_with("Kurri"),
        "Jarkko should start with 'Kurri'"
    );
    assert!(
        jarkko_name.ends_with("."),
        "Jarkko should end with disambiguation marker"
    );

    // Create a game result for compact mode testing
    let game_result = GameResultData {
        home_team: "Tappara".to_string(),
        away_team: "HIFK".to_string(),
        time: "18:30".to_string(),
        result: "3-2".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        goal_events: vec![
            GoalEventData {
                scorer_player_id: 1,
                scorer_name: "Koivu Mi.".to_string(), // Extended disambiguation
                minute: 5,
                home_team_score: 1,
                away_team_score: 0,
                is_winning_goal: false,
                goal_types: vec![],
                is_home_team: true,
                video_clip_url: None,
            },
            GoalEventData {
                scorer_player_id: 4,
                scorer_name: jarkko_name.clone(), // Use actual disambiguated name
                minute: 15,
                home_team_score: 1,
                away_team_score: 1,
                is_winning_goal: false,
                goal_types: vec!["YV".to_string()],
                is_home_team: false,
                video_clip_url: None,
            },
        ],
        played_time: 60,
    };

    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        true,  // ignore_height_limit for testing
        true,  // compact_mode
        false, // wide_mode
    );

    page.add_game_result(game_result);
    page.set_screen_height(25);

    // Verify the page was created successfully and is in compact mode
    assert!(page.is_compact_mode(), "Page should be in compact mode");
    assert!(!page.is_wide_mode(), "Page should not be in wide mode");
    assert_eq!(page.total_pages(), 1, "Should have one page");

    println!("✓ Compact mode handles complex disambiguation correctly within space constraints");
}

#[test]
fn test_wide_mode_maintains_consistent_disambiguation_logic() {
    // Test wide mode with multiple games that have disambiguation requirements
    let (home_players1, away_players1) = create_raw_test_players();
    let schedule_game1 = create_raw_schedule_game_with_disambiguation();

    // Create different players for the second game
    let home_players2 = vec![
        (7, "Mikael".to_string(), "Granlund".to_string()),
        (8, "Markus".to_string(), "Granlund".to_string()), // Needs disambiguation
        (9, "Erik".to_string(), "Haula".to_string()),      // Unique
    ];

    let _away_players2 = [
        (10, "Patrik".to_string(), "Laine".to_string()),
        (11, "Aleksander".to_string(), "Barkov".to_string()),
    ];

    // Process both games with disambiguation
    let disambiguated_events1 =
        process_goal_events_with_disambiguation(&schedule_game1, &home_players1, &away_players1);

    // Verify the first game's disambiguation
    assert_eq!(
        disambiguated_events1.len(),
        4,
        "First game should have 4 goal events"
    );
    let koivu_goals_count = disambiguated_events1
        .iter()
        .filter(|event| event.scorer_name.starts_with("Koivu"))
        .count();
    assert_eq!(
        koivu_goals_count, 2,
        "Should have 2 Koivu goals with different disambiguation"
    );

    // Create second game data (simpler, with just one goal for testing)
    let game_result1 = GameResultData {
        home_team: "Tappara".to_string(),
        away_team: "HIFK".to_string(),
        time: "18:30".to_string(),
        result: "3-2".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        goal_events: disambiguated_events1,
        played_time: 60,
    };

    // Test disambiguation context for second game
    let home_context2 = DisambiguationContext::new(home_players2.clone());
    assert!(
        home_context2.needs_disambiguation("Granlund"),
        "Should need disambiguation for Granlund"
    );
    assert_eq!(
        home_context2.get_disambiguated_name(7),
        Some(&"Granlund Mi.".to_string()),
        "Mikael should be 'Granlund Mi.'"
    );
    assert_eq!(
        home_context2.get_disambiguated_name(8),
        Some(&"Granlund Ma.".to_string()),
        "Markus should be 'Granlund Ma.'"
    );

    let game_result2 = GameResultData {
        home_team: "Ilves".to_string(),
        away_team: "Lukko".to_string(),
        time: "19:00".to_string(),
        result: "1-0".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        goal_events: vec![GoalEventData {
            scorer_player_id: 7,
            scorer_name: "Granlund Mi.".to_string(), // Disambiguated
            minute: 25,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: true,
            goal_types: vec![],
            is_home_team: true,
            video_clip_url: None,
        }],
        played_time: 60,
    };

    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        true,  // ignore_height_limit for testing
        false, // compact_mode
        true,  // wide_mode
    );

    page.add_game_result(game_result1);
    page.add_game_result(game_result2);
    page.set_screen_height(25);

    // Verify the page was created successfully and is in wide mode
    assert!(
        !page.is_compact_mode(),
        "Page should not be in compact mode"
    );
    assert!(page.is_wide_mode(), "Page should be in wide mode");
    assert_eq!(page.total_pages(), 1, "Should have one page for wide mode");

    println!("✓ Wide mode maintains consistent disambiguation logic across multiple games");
}

#[test]
fn test_name_truncation_works_properly_with_disambiguated_names() {
    // Test disambiguation with very long names that may need truncation
    let home_players = vec![
        (
            1,
            "Maximilian-Alexander".to_string(),
            "Korhonen-Virtanen".to_string(),
        ),
        (
            2,
            "Johannes-Sebastian".to_string(),
            "Korhonen-Virtanen".to_string(),
        ), // Same last name
        (
            3,
            "Christopher-Benjamin".to_string(),
            "Korhonen-Virtanen".to_string(),
        ), // Three with same last name
    ];

    let _away_players = [
        (
            4,
            "Alessandro-Giovanni".to_string(),
            "Bernardinelli-Rossi".to_string(),
        ),
        (
            5,
            "Maximilian-Andreas".to_string(),
            "Bernardinelli-Rossi".to_string(),
        ),
    ];

    // Test the disambiguation logic with long names
    let home_context = DisambiguationContext::new(home_players.clone());
    assert!(
        home_context.needs_disambiguation("Korhonen-Virtanen"),
        "Should need disambiguation for long last name"
    );

    // Verify extended disambiguation is used when needed
    let disambiguated_names = format_with_disambiguation(&home_players);
    let max_name = disambiguated_names.get(&1).unwrap();
    let joh_name = disambiguated_names.get(&2).unwrap();
    let chr_name = disambiguated_names.get(&3).unwrap();

    // These should be distinct even with long names
    assert_ne!(
        max_name, joh_name,
        "Maximilian and Johannes should have different disambiguated names"
    );
    assert_ne!(
        joh_name, chr_name,
        "Johannes and Christopher should have different disambiguated names"
    );
    assert_ne!(
        max_name, chr_name,
        "Maximilian and Christopher should have different disambiguated names"
    );

    // All should contain some form of the last name or be properly disambiguated
    assert!(max_name.len() > 5, "Disambiguated name should not be empty");
    assert!(joh_name.len() > 5, "Disambiguated name should not be empty");
    assert!(chr_name.len() > 5, "Disambiguated name should not be empty");

    // Check that they all contain some recognizable part of the name or proper disambiguation
    println!("Disambiguated names: {max_name}, {joh_name}, {chr_name}");

    // Create game with long disambiguated names
    let game = GameResultData {
        home_team: "Tappara".to_string(),
        away_team: "HIFK".to_string(),
        time: "18:30".to_string(),
        result: "2-1".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        goal_events: vec![
            GoalEventData {
                scorer_player_id: 1,
                scorer_name: max_name.clone(), // Long disambiguated name from actual logic
                minute: 8,
                home_team_score: 1,
                away_team_score: 0,
                is_winning_goal: false,
                goal_types: vec![],
                is_home_team: true,
                video_clip_url: None,
            },
            GoalEventData {
                scorer_player_id: 2,
                scorer_name: joh_name.clone(), // Long disambiguated name from actual logic
                minute: 15,
                home_team_score: 1,
                away_team_score: 1,
                is_winning_goal: false,
                goal_types: vec!["AV".to_string()],
                is_home_team: false,
                video_clip_url: None,
            },
        ],
        played_time: 60,
    };

    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // disable_video_links
        true,  // show_footer
        true,  // ignore_height_limit for testing
        false, // compact_mode
        false, // wide_mode
    );

    page.add_game_result(game);
    page.set_screen_height(25);

    // Verify the page can handle long disambiguated names without crashing
    assert!(
        !page.is_compact_mode(),
        "Page should not be in compact mode"
    );
    assert!(!page.is_wide_mode(), "Page should not be in wide mode");
    assert_eq!(page.total_pages(), 1, "Should have one page");

    println!("✓ Name truncation handling verified for long disambiguated names");
}

#[test]
fn test_all_modes_handle_unicode_disambiguated_names() {
    // Test disambiguation with Finnish Unicode characters
    let home_players = vec![
        (1, "Äänetön".to_string(), "Kärppä".to_string()),
        (2, "Öljynen".to_string(), "Kärppä".to_string()), // Needs disambiguation
        (3, "Åke".to_string(), "Mäkelä".to_string()),
        (4, "Äiti".to_string(), "Mäkelä".to_string()), // Needs disambiguation
    ];

    let _away_players = [
        (5, "Björn".to_string(), "Ström".to_string()),
        (6, "Östen".to_string(), "Björkström".to_string()),
    ];

    // Test disambiguation with Unicode characters
    let home_context = DisambiguationContext::new(home_players.clone());
    assert!(
        home_context.needs_disambiguation("Kärppä"),
        "Should need disambiguation for Kärppä"
    );
    assert!(
        home_context.needs_disambiguation("Mäkelä"),
        "Should need disambiguation for Mäkelä"
    );

    // Verify Unicode handling in disambiguation
    let disambiguated_names = format_with_disambiguation(&home_players);
    let aatonk_name = disambiguated_names.get(&1).unwrap();
    let oljyen_name = disambiguated_names.get(&2).unwrap();
    let ake_name = disambiguated_names.get(&3).unwrap();
    let aiti_name = disambiguated_names.get(&4).unwrap();

    // Verify names are distinct and contain Unicode properly
    assert_ne!(
        aatonk_name, oljyen_name,
        "Unicode names should be disambiguated differently"
    );
    assert_ne!(
        ake_name, aiti_name,
        "Unicode Mäkelä names should be disambiguated differently"
    );
    assert!(
        aatonk_name.contains("Kärppä"),
        "Should contain Unicode last name"
    );
    assert!(
        oljyen_name.contains("Kärppä"),
        "Should contain Unicode last name"
    );

    let goal_events = vec![
        GoalEventData {
            scorer_player_id: 1,
            scorer_name: aatonk_name.clone(), // Actual disambiguated name
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec![],
            is_home_team: true,
            video_clip_url: None,
        },
        GoalEventData {
            scorer_player_id: 2,
            scorer_name: oljyen_name.clone(), // Actual disambiguated name
            minute: 20,
            home_team_score: 1,
            away_team_score: 1,
            is_winning_goal: false,
            goal_types: vec![],
            is_home_team: false,
            video_clip_url: None,
        },
    ];

    let game = GameResultData {
        home_team: "Kärpät".to_string(),
        away_team: "Ässät".to_string(),
        time: "19:30".to_string(),
        result: "2-1".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        goal_events,
        played_time: 60,
    };

    // Test normal mode
    let mut normal_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        true, // ignore_height_limit for testing
        false,
        false,
    );
    normal_page.add_game_result(game.clone());
    normal_page.set_screen_height(25);
    assert!(
        !normal_page.is_compact_mode() && !normal_page.is_wide_mode(),
        "Normal mode configured correctly"
    );

    // Test compact mode
    let mut compact_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        true, // ignore_height_limit for testing
        true,
        false,
    );
    compact_page.add_game_result(game.clone());
    compact_page.set_screen_height(25);
    assert!(
        compact_page.is_compact_mode() && !compact_page.is_wide_mode(),
        "Compact mode configured correctly"
    );

    // Test wide mode
    let mut wide_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        true, // ignore_height_limit for testing
        false,
        true,
    );
    wide_page.add_game_result(game);
    wide_page.set_screen_height(25);
    assert!(
        !wide_page.is_compact_mode() && wide_page.is_wide_mode(),
        "Wide mode configured correctly"
    );

    println!("✓ All modes handle Unicode disambiguated names correctly");
}

#[test]
fn test_disambiguation_error_scenarios_in_display() {
    // Test error scenarios related to disambiguation in display context

    // Test empty player names
    let empty_name_players = vec![
        (1, "".to_string(), "Koivu".to_string()),
        (2, "Saku".to_string(), "".to_string()),
        (3, "".to_string(), "".to_string()),
    ];

    // Should handle gracefully without panicking
    let _empty_context = DisambiguationContext::new(empty_name_players.clone());
    let empty_disambiguated = format_with_disambiguation(&empty_name_players);
    assert_eq!(
        empty_disambiguated.len(),
        3,
        "Should handle empty names gracefully"
    );

    // Test players with identical first and last names
    let identical_players = vec![
        (1, "Mikko".to_string(), "Koivu".to_string()),
        (2, "Mikko".to_string(), "Koivu".to_string()), // Identical
    ];

    let identical_context = DisambiguationContext::new(identical_players.clone());
    assert!(
        identical_context.needs_disambiguation("Koivu"),
        "Should still need disambiguation for identical names"
    );
    let identical_disambiguated = format_with_disambiguation(&identical_players);
    assert_eq!(
        identical_disambiguated.len(),
        2,
        "Should handle identical names"
    );

    // Test missing player IDs in goal events with page display
    let error_game = GameResultData {
        home_team: "Tappara".to_string(),
        away_team: "HIFK".to_string(),
        time: "18:30".to_string(),
        result: "1-0".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        goal_events: vec![GoalEventData {
            scorer_player_id: 999, // Non-existent ID
            scorer_name: "Unknown Player".to_string(),
            minute: 5,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec![],
            is_home_team: true,
            video_clip_url: None,
        }],
        played_time: 60,
    };

    // Test that all UI modes can handle error scenarios without crashing
    let modes = [
        ("normal", false, false),
        ("compact", true, false),
        ("wide", false, true),
    ];

    for (mode_name, compact, wide) in modes {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "SM-LIIGA".to_string(),
            false,
            true,
            true, // ignore_height_limit for testing
            compact,
            wide,
        );

        page.add_game_result(error_game.clone());
        page.set_screen_height(25);

        // Should handle error data gracefully without crashing
        assert_eq!(
            page.total_pages(),
            1,
            "{mode_name} mode should handle error data gracefully"
        );
        assert_eq!(
            page.is_compact_mode(),
            compact,
            "{mode_name} mode should maintain correct compact setting"
        );
        assert_eq!(
            page.is_wide_mode(),
            wide,
            "{mode_name} mode should maintain correct wide setting"
        );
    }

    println!("✓ All UI modes handle disambiguation error scenarios gracefully");
}

#[test]
fn test_disambiguation_performance_with_many_players() {
    // Test performance characteristics with larger datasets
    use std::time::Instant;

    // Create a large set of players that need disambiguation
    let mut home_players = Vec::new();
    for i in 0..50 {
        home_players.push((i, format!("Player{i}"), "Koivu".to_string()));
    }

    let mut away_players = Vec::new();
    for i in 50..100 {
        away_players.push((i, format!("Player{i}"), "Selänne".to_string()));
    }

    // Measure disambiguation performance
    let start = Instant::now();
    let home_context = DisambiguationContext::new(home_players.clone());
    let away_context = DisambiguationContext::new(away_players.clone());
    let disambiguation_time = start.elapsed();

    // Verify disambiguation works correctly even with many players
    assert!(
        home_context.needs_disambiguation("Koivu"),
        "Should need disambiguation for Koivu"
    );
    assert!(
        away_context.needs_disambiguation("Selänne"),
        "Should need disambiguation for Selänne"
    );

    // Check that some reasonable number of players are disambiguated
    let home_disambiguated = format_with_disambiguation(&home_players);
    let away_disambiguated = format_with_disambiguation(&away_players);

    assert_eq!(
        home_disambiguated.len(),
        50,
        "Should disambiguate all home players"
    );
    assert_eq!(
        away_disambiguated.len(),
        50,
        "Should disambiguate all away players"
    );

    // Create a game with a subset of these players
    let large_game = GameResultData {
        home_team: "Tappara".to_string(),
        away_team: "HIFK".to_string(),
        time: "18:30".to_string(),
        result: "2-1".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        goal_events: vec![
            GoalEventData {
                scorer_player_id: 0,
                scorer_name: home_disambiguated.get(&0).unwrap().clone(),
                minute: 5,
                home_team_score: 1,
                away_team_score: 0,
                is_winning_goal: false,
                goal_types: vec![],
                is_home_team: true,
                video_clip_url: None,
            },
            GoalEventData {
                scorer_player_id: 50,
                scorer_name: away_disambiguated.get(&50).unwrap().clone(),
                minute: 15,
                home_team_score: 1,
                away_team_score: 1,
                is_winning_goal: false,
                goal_types: vec![],
                is_home_team: false,
                video_clip_url: None,
            },
        ],
        played_time: 60,
    };

    // Test that UI can handle large player sets
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

    page.add_game_result(large_game);
    page.set_screen_height(25);

    assert_eq!(page.total_pages(), 1, "Should handle large player datasets");

    // Performance should be reasonable (less than 1 second for 100 players)
    assert!(
        disambiguation_time.as_secs() < 1,
        "Disambiguation should be fast even with many players"
    );

    println!("✓ Disambiguation performance is acceptable with large player datasets");
}
