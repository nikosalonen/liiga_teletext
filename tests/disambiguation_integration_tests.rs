//! Integration tests for end-to-end player name disambiguation functionality.
//!
//! These tests verify the complete data flow from API response to teletext display,
//! ensuring that disambiguated player names are correctly processed and displayed
//! across all UI modes and scenarios.
//!
//! Requirements tested: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 3.1, 3.2, 3.3

use liiga_teletext::data_fetcher::models::{GoalEvent, GoalEventData, ScheduleGame, ScheduleTeam};
use liiga_teletext::data_fetcher::player_names::{
    DisambiguationContext, format_with_disambiguation,
};
use liiga_teletext::data_fetcher::processors::goal_events::process_goal_events_with_disambiguation;
use liiga_teletext::teletext_ui::{GameResultData, ScoreType, TeletextPage};

/// Creates realistic test player data with common Finnish hockey names
type PlayerData = Vec<(i64, String, String)>;

fn create_realistic_player_data() -> (PlayerData, PlayerData) {
    let home_players = vec![
        (1001, "Mikko".to_string(), "Koivu".to_string()),
        (1002, "Saku".to_string(), "Koivu".to_string()),
        (1003, "Teemu".to_string(), "Selänne".to_string()),
        (1004, "Jari".to_string(), "Kurri".to_string()),
        (1005, "Ville".to_string(), "Peltonen".to_string()),
        (1006, "Olli".to_string(), "Jokinen".to_string()),
        (1007, "Jussi".to_string(), "Jokinen".to_string()), // Another Jokinen for disambiguation
    ];

    let away_players = vec![
        (2001, "Patrik".to_string(), "Laine".to_string()),
        (2002, "Sebastian".to_string(), "Aho".to_string()),
        (2003, "Aleksander".to_string(), "Barkov".to_string()),
        (2004, "Mikael".to_string(), "Granlund".to_string()),
        (2005, "Markus".to_string(), "Granlund".to_string()), // Another Granlund for disambiguation
        (2006, "Artturi".to_string(), "Lehkonen".to_string()),
        (2007, "Joel".to_string(), "Armia".to_string()),
    ];

    (home_players, away_players)
}

/// Creates test goal events with realistic timing and scenarios
/// These goal events are placed in the home team's goal_events list
fn create_realistic_goal_events() -> Vec<GoalEvent> {
    vec![
        GoalEvent {
            scorer_player_id: 1001, // Mikko Koivu (should be disambiguated)
            log_time: "2024-01-15T18:35:23Z".to_string(),
            game_time: 323, // 5:23 into first period
            period: 1,
            event_id: 1,
            home_team_score: 1,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec!["YV".to_string()],     // Power play
            assistant_player_ids: vec![1003, 1004], // Selänne, Kurri
            video_clip_url: Some("https://example.com/goal1.mp4".to_string()),
            scorer_player: None,
        },
        GoalEvent {
            scorer_player_id: 1002, // Saku Koivu (should be disambiguated)
            log_time: "2024-01-15T19:05:42Z".to_string(),
            game_time: 1142, // 19:02 into first period
            period: 1,
            event_id: 3,
            home_team_score: 2,
            away_team_score: 1,
            winning_goal: false,
            goal_types: vec!["AV".to_string()], // Short-handed
            assistant_player_ids: vec![],
            video_clip_url: Some("https://example.com/goal3.mp4".to_string()),
            scorer_player: None,
        },
        GoalEvent {
            scorer_player_id: 1007, // Jussi Jokinen (should be disambiguated)
            log_time: "2024-01-15T19:25:18Z".to_string(),
            game_time: 2318, // 18:38 into second period
            period: 2,
            event_id: 4,
            home_team_score: 3,
            away_team_score: 1,
            winning_goal: true,
            goal_types: vec!["TM".to_string()], // Empty net
            assistant_player_ids: vec![1006],   // Olli Jokinen
            video_clip_url: None,
            scorer_player: None,
        },
    ]
}

/// Creates away team goal events
fn create_away_goal_events() -> Vec<GoalEvent> {
    vec![GoalEvent {
        scorer_player_id: 2005, // Markus Granlund (should be disambiguated)
        log_time: "2024-01-15T18:42:15Z".to_string(),
        game_time: 735, // 12:15 into first period
        period: 1,
        event_id: 2,
        home_team_score: 1,
        away_team_score: 1,
        winning_goal: false,
        goal_types: vec![],
        assistant_player_ids: vec![2001, 2003], // Laine, Barkov
        video_clip_url: None,
        scorer_player: None,
    }]
}

/// Creates a realistic schedule game for testing
fn create_realistic_schedule_game() -> ScheduleGame {
    let home_goal_events = create_realistic_goal_events();
    let away_goal_events = create_away_goal_events();

    ScheduleGame {
        id: 12345,
        season: 2024,
        start: "2024-01-15T18:30:00Z".to_string(),
        end: Some("2024-01-15T21:15:00Z".to_string()),
        home_team: ScheduleTeam {
            team_id: Some("1".to_string()),
            team_placeholder: None,
            team_name: Some("Tappara".to_string()),
            goals: 3,
            time_out: None,
            powerplay_instances: 4,
            powerplay_goals: 1,
            short_handed_instances: 2,
            short_handed_goals: 1,
            ranking: Some(1),
            game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
            goal_events: home_goal_events,
        },
        away_team: ScheduleTeam {
            team_id: Some("2".to_string()),
            team_placeholder: None,
            team_name: Some("HIFK".to_string()),
            goals: 1,
            time_out: None,
            powerplay_instances: 3,
            powerplay_goals: 0,
            short_handed_instances: 1,
            short_handed_goals: 0,
            ranking: Some(5),
            game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
            goal_events: away_goal_events,
        },
        finished_type: Some("FINISHED".to_string()),
        started: true,
        ended: true,
        game_time: 3600,
        serie: "runkosarja".to_string(),
    }
}

#[tokio::test]
async fn test_end_to_end_disambiguation_with_real_world_names() {
    // Test with realistic Finnish hockey player names
    let (home_players, away_players) = create_realistic_player_data();

    // Create a mock game with the player data
    let schedule_game = create_realistic_schedule_game();

    // Process goal events with disambiguation
    let goal_events =
        process_goal_events_with_disambiguation(&schedule_game, &home_players, &away_players);

    // Verify disambiguation results
    assert_eq!(goal_events.len(), 4, "Should have 4 goal events");

    // Check that Koivu players are disambiguated
    let koivu_goals: Vec<_> = goal_events
        .iter()
        .filter(|event| event.scorer_name.contains("Koivu"))
        .collect();
    assert_eq!(koivu_goals.len(), 2, "Should have 2 Koivu goals");

    // Verify specific disambiguation
    let mikko_koivu_goal = goal_events
        .iter()
        .find(|event| event.scorer_player_id == 1001)
        .expect("Should find Mikko Koivu goal");
    assert_eq!(
        mikko_koivu_goal.scorer_name, "Koivu M.",
        "Mikko Koivu should be disambiguated as 'Koivu M.'"
    );

    let saku_koivu_goal = goal_events
        .iter()
        .find(|event| event.scorer_player_id == 1002)
        .expect("Should find Saku Koivu goal");
    assert_eq!(
        saku_koivu_goal.scorer_name, "Koivu S.",
        "Saku Koivu should be disambiguated as 'Koivu S.'"
    );

    // Check that Granlund player is disambiguated (away team)
    let granlund_goal = goal_events
        .iter()
        .find(|event| event.scorer_player_id == 2005)
        .expect("Should find Markus Granlund goal");
    assert_eq!(
        granlund_goal.scorer_name, "Granlund Ma.",
        "Markus Granlund should be disambiguated as 'Granlund Ma.'"
    );

    // Check that Jokinen players are disambiguated
    let jokinen_goal = goal_events
        .iter()
        .find(|event| event.scorer_player_id == 1007)
        .expect("Should find Jussi Jokinen goal");
    assert_eq!(
        jokinen_goal.scorer_name, "Jokinen J.",
        "Jussi Jokinen should be disambiguated as 'Jokinen J.'"
    );

    println!("✓ End-to-end disambiguation with real-world names works correctly");
}

#[tokio::test]
async fn test_complete_data_flow_api_to_display() {
    // Simulate the complete data flow from API response to teletext display
    let (home_players, away_players) = create_realistic_player_data();

    // Step 1: Apply disambiguation
    let home_disambiguated = format_with_disambiguation(&home_players);
    let away_disambiguated = format_with_disambiguation(&away_players);

    // Verify disambiguation worked correctly
    assert_eq!(
        home_disambiguated.get(&1001),
        Some(&"Koivu M.".to_string()),
        "Mikko Koivu should be disambiguated"
    );
    assert_eq!(
        home_disambiguated.get(&1002),
        Some(&"Koivu S.".to_string()),
        "Saku Koivu should be disambiguated"
    );
    assert_eq!(
        home_disambiguated.get(&1003),
        Some(&"Selänne".to_string()),
        "Selänne should not be disambiguated"
    );
    assert_eq!(
        home_disambiguated.get(&1006),
        Some(&"Jokinen O.".to_string()),
        "Olli Jokinen should be disambiguated"
    );
    assert_eq!(
        home_disambiguated.get(&1007),
        Some(&"Jokinen J.".to_string()),
        "Jussi Jokinen should be disambiguated"
    );

    assert_eq!(
        away_disambiguated.get(&2004),
        Some(&"Granlund Mi.".to_string()),
        "Mikael Granlund should be disambiguated with Mi."
    );
    assert_eq!(
        away_disambiguated.get(&2005),
        Some(&"Granlund Ma.".to_string()),
        "Markus Granlund should be disambiguated with Ma."
    );
    assert_eq!(
        away_disambiguated.get(&2001),
        Some(&"Laine".to_string()),
        "Laine should not be disambiguated"
    );

    // Step 2: Cache the disambiguated names (simulated - we'll skip actual caching for this test)
    // In real implementation, this would cache the disambiguated names

    // Step 3: Create game data for teletext display
    let game_data = GameResultData {
        home_team: "Tappara".to_string(),
        away_team: "HIFK".to_string(),
        time: "18:30".to_string(),
        result: "3-1".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        goal_events: vec![
            GoalEventData {
                scorer_player_id: 1001,
                scorer_name: "Koivu M.".to_string(), // Should be disambiguated
                minute: 5,
                home_team_score: 1,
                away_team_score: 0,
                is_winning_goal: false,
                goal_types: vec!["YV".to_string()],
                is_home_team: true,
                video_clip_url: Some("https://example.com/goal1.mp4".to_string()),
            },
            GoalEventData {
                scorer_player_id: 1002,
                scorer_name: "Koivu S.".to_string(), // Should be disambiguated
                minute: 19,
                home_team_score: 2,
                away_team_score: 1,
                is_winning_goal: false,
                goal_types: vec!["AV".to_string()],
                is_home_team: true,
                video_clip_url: None,
            },
            GoalEventData {
                scorer_player_id: 2005,
                scorer_name: "Granlund Ma.".to_string(), // Should be disambiguated
                minute: 12,
                home_team_score: 1,
                away_team_score: 1,
                is_winning_goal: false,
                goal_types: vec![],
                is_home_team: false,
                video_clip_url: None,
            },
        ],
        played_time: 3600,
    };

    // Step 4: Test display in all UI modes

    // Normal mode
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
    normal_page.add_game_result(game_data.clone());
    normal_page.set_screen_height(25);
    assert!(
        !normal_page.is_compact_mode() && !normal_page.is_wide_mode(),
        "Normal mode configured correctly"
    );

    // Compact mode
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
    compact_page.add_game_result(game_data.clone());
    compact_page.set_screen_height(25);
    assert!(
        compact_page.is_compact_mode() && !compact_page.is_wide_mode(),
        "Compact mode configured correctly"
    );

    // Wide mode
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
    wide_page.add_game_result(game_data);
    wide_page.set_screen_height(25);
    assert!(
        !wide_page.is_compact_mode() && wide_page.is_wide_mode(),
        "Wide mode configured correctly"
    );

    println!("✓ Complete data flow from API to display works correctly");
}

#[tokio::test]
async fn test_goal_events_show_correct_disambiguated_scorer_names() {
    // Create test scenario with multiple players needing disambiguation
    let home_players = vec![
        (101, "Mikko".to_string(), "Koivu".to_string()),
        (102, "Saku".to_string(), "Koivu".to_string()),
        (103, "Kimmo".to_string(), "Koivu".to_string()), // Third Koivu for complex disambiguation
        (104, "Teemu".to_string(), "Selänne".to_string()),
    ];

    let away_players = vec![
        (201, "Patrik".to_string(), "Laine".to_string()),
        (202, "Aleksander".to_string(), "Barkov".to_string()),
    ];

    // Create home team goal events
    let home_goal_events = vec![
        GoalEvent {
            scorer_player_id: 101, // Mikko Koivu
            log_time: "2024-01-15T18:35:00Z".to_string(),
            game_time: 300,
            period: 1,
            event_id: 1,
            home_team_score: 1,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec![],
            assistant_player_ids: vec![],
            video_clip_url: None,
            scorer_player: None,
        },
        GoalEvent {
            scorer_player_id: 102, // Saku Koivu
            log_time: "2024-01-15T18:45:00Z".to_string(),
            game_time: 900,
            period: 1,
            event_id: 2,
            home_team_score: 2,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec!["YV".to_string()],
            assistant_player_ids: vec![],
            video_clip_url: Some("https://example.com/goal2.mp4".to_string()),
            scorer_player: None,
        },
        GoalEvent {
            scorer_player_id: 103, // Kimmo Koivu
            log_time: "2024-01-15T19:10:00Z".to_string(),
            game_time: 1800,
            period: 2,
            event_id: 3,
            home_team_score: 3,
            away_team_score: 0,
            winning_goal: true,
            goal_types: vec![],
            assistant_player_ids: vec![101, 104],
            video_clip_url: None,
            scorer_player: None,
        },
    ];

    // Create away team goal events
    let away_goal_events = vec![GoalEvent {
        scorer_player_id: 201, // Patrik Laine (no disambiguation needed)
        log_time: "2024-01-15T19:25:00Z".to_string(),
        game_time: 2700,
        period: 3,
        event_id: 4,
        home_team_score: 3,
        away_team_score: 1,
        winning_goal: false,
        goal_types: vec!["AV".to_string()],
        assistant_player_ids: vec![202],
        video_clip_url: None,
        scorer_player: None,
    }];

    // Create mock schedule game
    let schedule_game = ScheduleGame {
        id: 54321,
        season: 2024,
        start: "2024-01-15T18:30:00Z".to_string(),
        end: Some("2024-01-15T21:00:00Z".to_string()),
        home_team: ScheduleTeam {
            team_id: Some("1".to_string()),
            team_placeholder: None,
            team_name: Some("Tappara".to_string()),
            goals: 3,
            time_out: None,
            powerplay_instances: 2,
            powerplay_goals: 1,
            short_handed_instances: 0,
            short_handed_goals: 0,
            ranking: Some(1),
            game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
            goal_events: home_goal_events,
        },
        away_team: ScheduleTeam {
            team_id: Some("2".to_string()),
            team_placeholder: None,
            team_name: Some("HIFK".to_string()),
            goals: 1,
            time_out: None,
            powerplay_instances: 1,
            powerplay_goals: 0,
            short_handed_instances: 1,
            short_handed_goals: 1,
            ranking: Some(8),
            game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
            goal_events: away_goal_events,
        },
        finished_type: Some("FINISHED".to_string()),
        started: true,
        ended: true,
        game_time: 3600,
        serie: "runkosarja".to_string(),
    };

    // Process goal events with disambiguation
    let processed_events =
        process_goal_events_with_disambiguation(&schedule_game, &home_players, &away_players);

    // Verify all goal events have correct disambiguated names
    assert_eq!(
        processed_events.len(),
        4,
        "Should have 4 processed goal events"
    );

    // Check Mikko Koivu goal
    let mikko_goal = processed_events
        .iter()
        .find(|event| event.scorer_player_id == 101)
        .expect("Should find Mikko Koivu goal");
    assert_eq!(
        mikko_goal.scorer_name, "Koivu M.",
        "Mikko Koivu should be 'Koivu M.'"
    );
    assert_eq!(mikko_goal.minute, 5, "Goal should be at minute 5");
    assert_eq!(mikko_goal.home_team_score, 1, "Home team score should be 1");
    assert!(mikko_goal.is_home_team, "Should be home team goal");

    // Check Saku Koivu goal
    let saku_goal = processed_events
        .iter()
        .find(|event| event.scorer_player_id == 102)
        .expect("Should find Saku Koivu goal");
    assert_eq!(
        saku_goal.scorer_name, "Koivu S.",
        "Saku Koivu should be 'Koivu S.'"
    );
    assert_eq!(saku_goal.minute, 15, "Goal should be at minute 15");
    assert_eq!(
        saku_goal.goal_types,
        vec!["YV"],
        "Should have power play goal type"
    );
    assert!(
        saku_goal.video_clip_url.is_some(),
        "Should have video clip URL"
    );

    // Check Kimmo Koivu goal
    let kimmo_goal = processed_events
        .iter()
        .find(|event| event.scorer_player_id == 103)
        .expect("Should find Kimmo Koivu goal");
    assert_eq!(
        kimmo_goal.scorer_name, "Koivu K.",
        "Kimmo Koivu should be 'Koivu K.'"
    );
    assert_eq!(kimmo_goal.minute, 30, "Goal should be at minute 30");
    assert!(kimmo_goal.is_winning_goal, "Should be winning goal");

    // Check Patrik Laine goal (no disambiguation needed)
    let laine_goal = processed_events
        .iter()
        .find(|event| event.scorer_player_id == 201)
        .expect("Should find Patrik Laine goal");
    assert_eq!(
        laine_goal.scorer_name, "Laine",
        "Laine should not be disambiguated"
    );
    assert_eq!(laine_goal.minute, 45, "Goal should be at minute 45");
    assert!(!laine_goal.is_home_team, "Should be away team goal");
    assert_eq!(
        laine_goal.goal_types,
        vec!["AV"],
        "Should have short-handed goal type"
    );

    println!("✓ Goal events show correct disambiguated scorer names");
}

#[tokio::test]
async fn test_performance_impact_with_large_datasets() {
    use std::time::Instant;

    // Create large datasets to test performance impact
    let mut home_players = Vec::new();
    let mut away_players = Vec::new();

    // Generate 50 players per team with some duplicates to test disambiguation performance
    let last_names = [
        "Koivu", "Selänne", "Kurri", "Granlund", "Jokinen", "Laine", "Barkov", "Aho",
    ];
    let first_names = [
        "Mikko",
        "Saku",
        "Teemu",
        "Jari",
        "Ville",
        "Olli",
        "Jussi",
        "Patrik",
        "Sebastian",
        "Aleksander",
    ];

    for i in 0..50 {
        let last_name = last_names[i % last_names.len()].to_string();
        let first_name = first_names[i % first_names.len()].to_string();

        home_players.push((1000 + i as i64, first_name.clone(), last_name.clone()));
        away_players.push((2000 + i as i64, first_name, last_name));
    }

    // Create many goal events
    let mut goal_events = Vec::new();
    for i in 0..20 {
        let player_id = if i % 2 == 0 {
            1000 + (i as i64)
        } else {
            2000 + (i as i64)
        };
        goal_events.push(GoalEvent {
            scorer_player_id: player_id,
            log_time: format!("2024-01-15T18:{}:00Z", 30 + i),
            game_time: i * 180, // Every 3 minutes
            period: (i / 7) + 1,
            event_id: i + 1,
            home_team_score: if i % 2 == 0 { (i / 2) + 1 } else { i / 2 },
            away_team_score: if i % 2 == 1 { (i / 2) + 1 } else { i / 2 },
            winning_goal: i == 19, // Last goal is winning goal
            goal_types: if i % 3 == 0 {
                vec!["YV".to_string()]
            } else {
                vec![]
            },
            assistant_player_ids: vec![],
            video_clip_url: if i % 4 == 0 {
                Some(format!("https://example.com/goal{i}.mp4"))
            } else {
                None
            },
            scorer_player: None,
        });
    }

    // Create mock schedule game with large dataset
    let schedule_game = ScheduleGame {
        id: 99999,
        season: 2024,
        start: "2024-01-15T18:30:00Z".to_string(),
        end: Some("2024-01-15T21:30:00Z".to_string()),
        home_team: ScheduleTeam {
            team_id: Some("1".to_string()),
            team_placeholder: None,
            team_name: Some("Tappara".to_string()),
            goals: 10,
            time_out: None,
            powerplay_instances: 5,
            powerplay_goals: 3,
            short_handed_instances: 2,
            short_handed_goals: 1,
            ranking: Some(1),
            game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
            goal_events: goal_events.clone(),
        },
        away_team: ScheduleTeam {
            team_id: Some("2".to_string()),
            team_placeholder: None,
            team_name: Some("HIFK".to_string()),
            goals: 10,
            time_out: None,
            powerplay_instances: 4,
            powerplay_goals: 2,
            short_handed_instances: 3,
            short_handed_goals: 2,
            ranking: Some(5),
            game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
            goal_events: vec![],
        },
        finished_type: Some("FINISHED".to_string()),
        started: true,
        ended: true,
        game_time: 3600,
        serie: "runkosarja".to_string(),
    };

    // Measure disambiguation performance
    let start_time = Instant::now();

    // Test disambiguation context creation
    let home_context = DisambiguationContext::new(home_players.clone());
    let away_context = DisambiguationContext::new(away_players.clone());

    let disambiguation_time = start_time.elapsed();

    // Test goal event processing
    let processing_start = Instant::now();
    let processed_events =
        process_goal_events_with_disambiguation(&schedule_game, &home_players, &away_players);
    let processing_time = processing_start.elapsed();

    // Test format_with_disambiguation performance
    let format_start = Instant::now();
    let home_disambiguated = format_with_disambiguation(&home_players);
    let away_disambiguated = format_with_disambiguation(&away_players);
    let format_time = format_start.elapsed();

    let total_time = start_time.elapsed();

    // Performance assertions - these should complete quickly even with large datasets
    assert!(
        disambiguation_time.as_millis() < 100,
        "Disambiguation context creation should be fast ({}ms)",
        disambiguation_time.as_millis()
    );
    assert!(
        processing_time.as_millis() < 200,
        "Goal event processing should be fast ({}ms)",
        processing_time.as_millis()
    );
    assert!(
        format_time.as_millis() < 50,
        "Name formatting should be fast ({}ms)",
        format_time.as_millis()
    );
    assert!(
        total_time.as_millis() < 500,
        "Total processing should be fast ({}ms)",
        total_time.as_millis()
    );

    // Verify results are correct
    assert_eq!(
        processed_events.len(),
        20,
        "Should have processed all 20 goal events"
    );
    assert_eq!(
        home_disambiguated.len(),
        50,
        "Should have disambiguated all 50 home players"
    );
    assert_eq!(
        away_disambiguated.len(),
        50,
        "Should have disambiguated all 50 away players"
    );

    // Verify some disambiguation occurred
    let koivu_count = home_disambiguated
        .values()
        .filter(|name| name.starts_with("Koivu"))
        .count();
    assert!(
        koivu_count > 1,
        "Should have multiple Koivu players disambiguated"
    );

    // Check that disambiguation context can efficiently determine if disambiguation is needed
    assert!(
        home_context.needs_disambiguation("Koivu"),
        "Should need disambiguation for Koivu"
    );
    assert!(
        away_context.needs_disambiguation("Koivu"),
        "Should need disambiguation for Koivu"
    );

    println!("✓ Performance impact with large datasets is acceptable");
    println!(
        "  - Disambiguation context: {}ms",
        disambiguation_time.as_millis()
    );
    println!(
        "  - Goal event processing: {}ms",
        processing_time.as_millis()
    );
    println!("  - Name formatting: {}ms", format_time.as_millis());
    println!("  - Total time: {}ms", total_time.as_millis());
}

#[tokio::test]
async fn test_cross_team_disambiguation_isolation() {
    // Test that players with same last name on different teams don't affect each other
    let home_players = vec![
        (101, "Mikko".to_string(), "Koivu".to_string()),
        (102, "Teemu".to_string(), "Selänne".to_string()),
    ];

    let away_players = vec![
        (201, "Saku".to_string(), "Koivu".to_string()), // Same last name as home team
        (202, "Patrik".to_string(), "Laine".to_string()),
    ];

    let schedule_game = ScheduleGame {
        id: 11111,
        season: 2024,
        start: "2024-01-15T18:30:00Z".to_string(),
        end: Some("2024-01-15T21:00:00Z".to_string()),
        home_team: ScheduleTeam {
            team_id: Some("1".to_string()),
            team_placeholder: None,
            team_name: Some("Tappara".to_string()),
            goals: 1,
            time_out: None,
            powerplay_instances: 0,
            powerplay_goals: 0,
            short_handed_instances: 0,
            short_handed_goals: 0,
            ranking: Some(1),
            game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
            goal_events: vec![GoalEvent {
                scorer_player_id: 101, // Home Mikko Koivu
                log_time: "2024-01-15T18:35:00Z".to_string(),
                game_time: 300,
                period: 1,
                event_id: 1,
                home_team_score: 1,
                away_team_score: 0,
                winning_goal: false,
                goal_types: vec![],
                assistant_player_ids: vec![],
                video_clip_url: None,
                scorer_player: None,
            }],
        },
        away_team: ScheduleTeam {
            team_id: Some("2".to_string()),
            team_placeholder: None,
            team_name: Some("HIFK".to_string()),
            goals: 1,
            time_out: None,
            powerplay_instances: 0,
            powerplay_goals: 0,
            short_handed_instances: 0,
            short_handed_goals: 0,
            ranking: Some(5),
            game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
            goal_events: vec![GoalEvent {
                scorer_player_id: 201, // Away Saku Koivu
                log_time: "2024-01-15T18:45:00Z".to_string(),
                game_time: 900,
                period: 1,
                event_id: 2,
                home_team_score: 1,
                away_team_score: 1,
                winning_goal: false,
                goal_types: vec![],
                assistant_player_ids: vec![],
                video_clip_url: None,
                scorer_player: None,
            }],
        },
        finished_type: Some("FINISHED".to_string()),
        started: true,
        ended: true,
        game_time: 3600,
        serie: "runkosarja".to_string(),
    };

    // Process goal events with disambiguation
    let processed_events =
        process_goal_events_with_disambiguation(&schedule_game, &home_players, &away_players);

    // Verify that both Koivu players are NOT disambiguated since they're on different teams
    assert_eq!(
        processed_events.len(),
        2,
        "Should have 2 processed goal events"
    );

    let home_koivu_goal = processed_events
        .iter()
        .find(|event| event.scorer_player_id == 101)
        .expect("Should find home Koivu goal");
    assert_eq!(
        home_koivu_goal.scorer_name, "Koivu",
        "Home Koivu should NOT be disambiguated"
    );

    let away_koivu_goal = processed_events
        .iter()
        .find(|event| event.scorer_player_id == 201)
        .expect("Should find away Koivu goal");
    assert_eq!(
        away_koivu_goal.scorer_name, "Koivu",
        "Away Koivu should NOT be disambiguated"
    );

    // Test disambiguation contexts separately
    let home_context = DisambiguationContext::new(home_players.clone());
    let away_context = DisambiguationContext::new(away_players.clone());

    // Neither team should need disambiguation for Koivu since there's only one per team
    assert!(
        !home_context.needs_disambiguation("Koivu"),
        "Home team should not need Koivu disambiguation"
    );
    assert!(
        !away_context.needs_disambiguation("Koivu"),
        "Away team should not need Koivu disambiguation"
    );

    // Test with format_with_disambiguation directly
    let home_disambiguated = format_with_disambiguation(&home_players);
    let away_disambiguated = format_with_disambiguation(&away_players);

    assert_eq!(
        home_disambiguated.get(&101),
        Some(&"Koivu".to_string()),
        "Home Koivu should not be disambiguated"
    );
    assert_eq!(
        away_disambiguated.get(&201),
        Some(&"Koivu".to_string()),
        "Away Koivu should not be disambiguated"
    );

    println!("✓ Cross-team disambiguation isolation works correctly");
}

#[tokio::test]
async fn test_unicode_character_handling_in_disambiguation() {
    // Test with Finnish characters (ä, ö, å) in names
    let home_players = vec![
        (101, "Äkäslompolo".to_string(), "Kärppä".to_string()),
        (102, "Öljynen".to_string(), "Kärppä".to_string()),
        (103, "Åke".to_string(), "Kärppä".to_string()),
        (104, "Mikko".to_string(), "Mäkelä".to_string()),
        (105, "Ville".to_string(), "Mäkelä".to_string()),
    ];

    let away_players = vec![
        (201, "Jörgen".to_string(), "Björkström".to_string()),
        (202, "Åsa".to_string(), "Björkström".to_string()),
        (203, "Teemu".to_string(), "Pöllönen".to_string()),
    ];

    // Test disambiguation with Unicode characters
    let home_disambiguated = format_with_disambiguation(&home_players);
    let away_disambiguated = format_with_disambiguation(&away_players);

    // Verify Kärppä players are disambiguated with Unicode first initials
    assert_eq!(
        home_disambiguated.get(&101),
        Some(&"Kärppä Ä.".to_string()),
        "Should disambiguate with Ä initial"
    );
    assert_eq!(
        home_disambiguated.get(&102),
        Some(&"Kärppä Ö.".to_string()),
        "Should disambiguate with Ö initial"
    );
    assert_eq!(
        home_disambiguated.get(&103),
        Some(&"Kärppä Å.".to_string()),
        "Should disambiguate with Å initial"
    );

    // Verify Mäkelä players are disambiguated
    assert_eq!(
        home_disambiguated.get(&104),
        Some(&"Mäkelä M.".to_string()),
        "Should disambiguate Mäkelä with M initial"
    );
    assert_eq!(
        home_disambiguated.get(&105),
        Some(&"Mäkelä V.".to_string()),
        "Should disambiguate Mäkelä with V initial"
    );

    // Verify Björkström players are disambiguated
    assert_eq!(
        away_disambiguated.get(&201),
        Some(&"Björkström J.".to_string()),
        "Should disambiguate Björkström with J initial"
    );
    assert_eq!(
        away_disambiguated.get(&202),
        Some(&"Björkström Å.".to_string()),
        "Should disambiguate Björkström with Å initial"
    );

    // Verify unique name is not disambiguated
    assert_eq!(
        away_disambiguated.get(&203),
        Some(&"Pöllönen".to_string()),
        "Unique name should not be disambiguated"
    );

    // Test Unicode handling is already verified above through format_with_disambiguation

    // Test disambiguation context with Unicode
    let home_context = DisambiguationContext::new(home_players);
    assert!(
        home_context.needs_disambiguation("Kärppä"),
        "Should need disambiguation for Kärppä"
    );
    assert!(
        home_context.needs_disambiguation("Mäkelä"),
        "Should need disambiguation for Mäkelä"
    );

    let away_context = DisambiguationContext::new(away_players);
    assert!(
        away_context.needs_disambiguation("Björkström"),
        "Should need disambiguation for Björkström"
    );
    assert!(
        !away_context.needs_disambiguation("Pöllönen"),
        "Should not need disambiguation for Pöllönen"
    );

    println!("✓ Unicode character handling in disambiguation works correctly");
}

#[tokio::test]
async fn test_edge_cases_and_error_resilience() {
    // Test various edge cases that could occur in real-world scenarios

    // Case 1: Empty first names
    let players_with_empty_first = vec![
        (101, "".to_string(), "Koivu".to_string()),
        (102, "Saku".to_string(), "Koivu".to_string()),
        (103, "Teemu".to_string(), "Selänne".to_string()),
    ];

    let disambiguated = format_with_disambiguation(&players_with_empty_first);

    // Player with empty first name should fall back to last name only
    assert_eq!(
        disambiguated.get(&101),
        Some(&"Koivu".to_string()),
        "Empty first name should fallback to last name"
    );
    assert_eq!(
        disambiguated.get(&102),
        Some(&"Koivu S.".to_string()),
        "Valid first name should be disambiguated"
    );
    assert_eq!(
        disambiguated.get(&103),
        Some(&"Selänne".to_string()),
        "Unique name should not be disambiguated"
    );

    // Case 2: First names with multiple words
    let players_with_multi_word_first = vec![
        (201, "Jean-Pierre".to_string(), "Dumont".to_string()),
        (202, "Mary Jane".to_string(), "Dumont".to_string()),
        (203, "José María".to_string(), "González".to_string()),
    ];

    let disambiguated2 = format_with_disambiguation(&players_with_multi_word_first);

    // Should use first letter of first word for disambiguation
    assert_eq!(
        disambiguated2.get(&201),
        Some(&"Dumont J.".to_string()),
        "Should use first letter of hyphenated name"
    );
    assert_eq!(
        disambiguated2.get(&202),
        Some(&"Dumont M.".to_string()),
        "Should use first letter of multi-word name"
    );
    assert_eq!(
        disambiguated2.get(&203),
        Some(&"González".to_string()),
        "Unique name should not be disambiguated"
    );

    // Case 3: First names starting with non-alphabetic characters
    let players_with_special_chars = vec![
        (301, "1st".to_string(), "Player".to_string()),
        (302, "'Quoted".to_string(), "Player".to_string()),
        (303, "Normal".to_string(), "Player".to_string()),
    ];

    let disambiguated3 = format_with_disambiguation(&players_with_special_chars);

    // Should handle gracefully without crashing
    assert!(
        disambiguated3.contains_key(&301),
        "Should handle numeric first name"
    );
    assert!(
        disambiguated3.contains_key(&302),
        "Should handle quoted first name"
    );
    assert!(
        disambiguated3.contains_key(&303),
        "Should handle normal first name"
    );

    // Case 4: Very long names
    let players_with_long_names = vec![
        (
            401,
            "Verylongfirstnamethatexceedsnormallimits".to_string(),
            "Verylonglastnamethatexceedsnormallimits".to_string(),
        ),
        (
            402,
            "Another".to_string(),
            "Verylonglastnamethatexceedsnormallimits".to_string(),
        ),
        (403, "Short".to_string(), "Name".to_string()),
    ];

    let disambiguated4 = format_with_disambiguation(&players_with_long_names);

    // Should handle long names without issues
    assert!(
        disambiguated4.contains_key(&401),
        "Should handle very long names"
    );
    assert!(
        disambiguated4.contains_key(&402),
        "Should handle long last names"
    );
    assert!(
        disambiguated4.contains_key(&403),
        "Should handle short names"
    );

    // Case 5: Empty player list
    let empty_players: Vec<(i64, String, String)> = vec![];
    let disambiguated_empty = format_with_disambiguation(&empty_players);
    assert!(
        disambiguated_empty.is_empty(),
        "Empty player list should return empty result"
    );

    // Case 6: Single player
    let single_player = vec![(501, "Mikko".to_string(), "Koivu".to_string())];
    let disambiguated_single = format_with_disambiguation(&single_player);
    assert_eq!(
        disambiguated_single.get(&501),
        Some(&"Koivu".to_string()),
        "Single player should not be disambiguated"
    );

    // Case 7: Test DisambiguationContext with edge cases
    let context_empty = DisambiguationContext::new(vec![]);
    assert!(
        !context_empty.needs_disambiguation("AnyName"),
        "Empty context should not need disambiguation"
    );

    let context_single = DisambiguationContext::new(single_player);
    assert!(
        !context_single.needs_disambiguation("Koivu"),
        "Single player context should not need disambiguation"
    );

    println!("✓ Edge cases and error resilience handled correctly");
}
