use liiga_teletext::data_fetcher::models::{GoalEvent, ScheduleGame, ScheduleTeam};
use liiga_teletext::data_fetcher::processors::process_goal_events_with_disambiguation;

#[test]
fn test_no_redundant_disambiguation_for_same_player() {
    // Create test scenario where the same player scores multiple goals
    let home_players = vec![
        (101, "Mikko".to_string(), "Koivu".to_string()),
        (102, "Saku".to_string(), "Koivu".to_string()), // Needs disambiguation
        (103, "Teemu".to_string(), "Selänne".to_string()), // Unique
    ];

    let away_players = vec![
        (201, "Patrik".to_string(), "Laine".to_string()),
        (202, "Aleksander".to_string(), "Barkov".to_string()),
    ];

    // Create goal events where Mikko Koivu scores 3 goals
    let home_goal_events = vec![
        GoalEvent {
            scorer_player_id: 101, // Mikko Koivu - first goal
            scorer_player: None,
            log_time: "18:35:00".to_string(),
            game_time: 300, // 5 minutes
            period: 1,
            event_id: 1,
            home_team_score: 1,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec![],
            assistant_player_ids: vec![],
            assistant_players: vec![],
            video_clip_url: None,
        },
        GoalEvent {
            scorer_player_id: 102, // Saku Koivu - different player
            scorer_player: None,
            log_time: "18:42:00".to_string(),
            game_time: 720, // 12 minutes
            period: 1,
            event_id: 2,
            home_team_score: 2,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec![],
            assistant_player_ids: vec![],
            assistant_players: vec![],
            video_clip_url: None,
        },
        GoalEvent {
            scorer_player_id: 101, // Mikko Koivu - second goal (should not show disambiguation)
            scorer_player: None,
            log_time: "19:15:00".to_string(),
            game_time: 1500, // 25 minutes
            period: 2,
            event_id: 3,
            home_team_score: 3,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec!["YV".to_string()],
            assistant_player_ids: vec![],
            assistant_players: vec![],
            video_clip_url: None,
        },
        GoalEvent {
            scorer_player_id: 101, // Mikko Koivu - third goal (should not show disambiguation)
            scorer_player: None,
            log_time: "19:45:00".to_string(),
            game_time: 2700, // 45 minutes
            period: 3,
            event_id: 4,
            home_team_score: 4,
            away_team_score: 0,
            winning_goal: true,
            goal_types: vec![],
            assistant_player_ids: vec![],
            assistant_players: vec![],
            video_clip_url: None,
        },
    ];

    let away_goal_events = vec![GoalEvent {
        scorer_player_id: 201, // Patrik Laine
        scorer_player: None,
        log_time: "19:30:00".to_string(),
        game_time: 2400, // 40 minutes
        period: 3,
        event_id: 5,
        home_team_score: 4,
        away_team_score: 1,
        winning_goal: false,
        goal_types: vec![],
        assistant_player_ids: vec![],
        assistant_players: vec![],
        video_clip_url: None,
    }];

    let home_team = ScheduleTeam {
        team_id: Some("1".to_string()),
        team_name: Some("Tappara".to_string()),
        goal_events: home_goal_events,
        ..Default::default()
    };

    let away_team = ScheduleTeam {
        team_id: Some("2".to_string()),
        team_name: Some("HIFK".to_string()),
        goal_events: away_goal_events,
        ..Default::default()
    };

    let schedule_game = ScheduleGame {
        id: 12345,
        season: 2024,
        start: "2024-01-15T18:30:00Z".to_string(),
        end: Some("2024-01-15T21:00:00Z".to_string()),
        home_team,
        away_team,
        finished_type: Some("FINISHED".to_string()),
        started: true,
        ended: true,
        game_time: 3600,
        serie: "runkosarja".to_string(),
    };

    // Process the goal events with disambiguation
    let goal_events =
        process_goal_events_with_disambiguation(&schedule_game, &home_players, &away_players);

    // Verify we have 5 goal events total
    assert_eq!(goal_events.len(), 5, "Should have 5 goal events");

    // Find all Mikko Koivu goals (player ID 101)
    let mikko_goals: Vec<_> = goal_events
        .iter()
        .filter(|event| event.scorer_player_id == 101)
        .collect();

    assert_eq!(mikko_goals.len(), 3, "Should have 3 goals by Mikko Koivu");

    // First goal should show disambiguation "Koivu M."
    assert_eq!(
        mikko_goals[0].scorer_name, "Koivu M.",
        "First goal by Mikko Koivu should show disambiguation 'Koivu M.'"
    );

    // Second and third goals should show just "Koivu"
    assert_eq!(
        mikko_goals[1].scorer_name, "Koivu",
        "Second goal by Mikko Koivu should show just 'Koivu'"
    );
    assert_eq!(
        mikko_goals[2].scorer_name, "Koivu",
        "Third goal by Mikko Koivu should show just 'Koivu'"
    );

    // Find Saku Koivu goal (player ID 102)
    let saku_goal = goal_events
        .iter()
        .find(|event| event.scorer_player_id == 102)
        .expect("Should find Saku Koivu goal");

    // Saku's goal should still show disambiguation since it's his first (and only) goal
    assert_eq!(
        saku_goal.scorer_name, "Koivu S.",
        "Saku Koivu should show disambiguation 'Koivu S.'"
    );

    // Find Patrik Laine goal (player ID 201) - no disambiguation needed
    let laine_goal = goal_events
        .iter()
        .find(|event| event.scorer_player_id == 201)
        .expect("Should find Patrik Laine goal");

    assert_eq!(
        laine_goal.scorer_name, "Laine",
        "Patrik Laine should show just 'Laine' (no disambiguation needed)"
    );

    println!("✓ No redundant disambiguation test passed");
    println!(
        "  - Mikko Koivu first goal: '{}'",
        mikko_goals[0].scorer_name
    );
    println!(
        "  - Mikko Koivu second goal: '{}'",
        mikko_goals[1].scorer_name
    );
    println!(
        "  - Mikko Koivu third goal: '{}'",
        mikko_goals[2].scorer_name
    );
    println!("  - Saku Koivu goal: '{}'", saku_goal.scorer_name);
    println!("  - Patrik Laine goal: '{}'", laine_goal.scorer_name);
}

#[test]
fn test_cross_team_no_redundant_disambiguation() {
    // Test that the same logic works across teams
    let home_players = vec![
        (101, "Mikko".to_string(), "Koivu".to_string()), // Only one Koivu on home team
    ];

    let away_players = vec![
        (201, "Saku".to_string(), "Koivu".to_string()), // Only one Koivu on away team
        (202, "Patrik".to_string(), "Laine".to_string()),
        (203, "Sebastian".to_string(), "Laine".to_string()), // Different first name, same last name - needs disambiguation
    ];

    // Create goal events where both Laines score multiple goals
    let away_goal_events = vec![
        GoalEvent {
            scorer_player_id: 202, // Patrik Laine - first goal
            scorer_player: None,
            log_time: "18:35:00".to_string(),
            game_time: 300,
            period: 1,
            event_id: 1,
            home_team_score: 0,
            away_team_score: 1,
            winning_goal: false,
            goal_types: vec![],
            assistant_player_ids: vec![],
            assistant_players: vec![],
            video_clip_url: None,
        },
        GoalEvent {
            scorer_player_id: 203, // Sebastian Laine - first goal
            scorer_player: None,
            log_time: "18:42:00".to_string(),
            game_time: 720,
            period: 1,
            event_id: 2,
            home_team_score: 0,
            away_team_score: 2,
            winning_goal: false,
            goal_types: vec![],
            assistant_player_ids: vec![],
            assistant_players: vec![],
            video_clip_url: None,
        },
        GoalEvent {
            scorer_player_id: 202, // Patrik Laine - second goal (should not show disambiguation)
            scorer_player: None,
            log_time: "19:15:00".to_string(),
            game_time: 1500,
            period: 2,
            event_id: 3,
            home_team_score: 0,
            away_team_score: 3,
            winning_goal: false,
            goal_types: vec!["YV".to_string()],
            assistant_player_ids: vec![],
            assistant_players: vec![],
            video_clip_url: None,
        },
    ];

    let home_goal_events = vec![GoalEvent {
        scorer_player_id: 101, // Mikko Koivu - no disambiguation needed on home team
        scorer_player: None,
        log_time: "19:30:00".to_string(),
        game_time: 2400,
        period: 3,
        event_id: 4,
        home_team_score: 1,
        away_team_score: 3,
        winning_goal: false,
        goal_types: vec![],
        assistant_player_ids: vec![],
        assistant_players: vec![],
        video_clip_url: None,
    }];

    let home_team = ScheduleTeam {
        team_id: Some("1".to_string()),
        team_name: Some("Tappara".to_string()),
        goal_events: home_goal_events,
        ..Default::default()
    };

    let away_team = ScheduleTeam {
        team_id: Some("2".to_string()),
        team_name: Some("HIFK".to_string()),
        goal_events: away_goal_events,
        ..Default::default()
    };

    let schedule_game = ScheduleGame {
        id: 12346,
        season: 2024,
        start: "2024-01-15T18:30:00Z".to_string(),
        end: Some("2024-01-15T21:00:00Z".to_string()),
        home_team,
        away_team,
        finished_type: Some("FINISHED".to_string()),
        started: true,
        ended: true,
        game_time: 3600,
        serie: "runkosarja".to_string(),
    };

    // Process the goal events with disambiguation
    let goal_events =
        process_goal_events_with_disambiguation(&schedule_game, &home_players, &away_players);

    // Verify we have 4 goal events total
    assert_eq!(goal_events.len(), 4, "Should have 4 goal events");

    // Find Patrik Laine goals (player ID 202)
    let laine_goals: Vec<_> = goal_events
        .iter()
        .filter(|event| event.scorer_player_id == 202)
        .collect();

    assert_eq!(laine_goals.len(), 2, "Should have 2 goals by Patrik Laine");

    // First goal should show disambiguation "Laine P."
    assert_eq!(
        laine_goals[0].scorer_name, "Laine P.",
        "First goal by Patrik Laine should show disambiguation 'Laine P.'"
    );

    // Second goal should show just "Laine"
    assert_eq!(
        laine_goals[1].scorer_name, "Laine",
        "Second goal by Patrik Laine should show just 'Laine'"
    );

    // Find Sebastian Laine goal (player ID 203)
    let sebastian_goal = goal_events
        .iter()
        .find(|event| event.scorer_player_id == 203)
        .expect("Should find Sebastian Laine goal");

    // Sebastian's goal should show disambiguation since it's his first goal
    assert_eq!(
        sebastian_goal.scorer_name, "Laine S.",
        "Sebastian Laine should show disambiguation 'Laine S.'"
    );

    // Find Mikko Koivu goal (player ID 101) - no disambiguation needed
    let koivu_goal = goal_events
        .iter()
        .find(|event| event.scorer_player_id == 101)
        .expect("Should find Mikko Koivu goal");

    assert_eq!(
        koivu_goal.scorer_name, "Koivu",
        "Mikko Koivu should show just 'Koivu' (no disambiguation needed on home team)"
    );

    println!("✓ Cross-team no redundant disambiguation test passed");
    println!(
        "  - Patrik Laine first goal: '{}'",
        laine_goals[0].scorer_name
    );
    println!(
        "  - Patrik Laine second goal: '{}'",
        laine_goals[1].scorer_name
    );
    println!("  - Sebastian Laine goal: '{}'", sebastian_goal.scorer_name);
    println!("  - Mikko Koivu goal: '{}'", koivu_goal.scorer_name);
}
