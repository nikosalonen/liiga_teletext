use liiga_teletext::data_fetcher::models::{GoalEvent, ScheduleGame, ScheduleTeam};
use liiga_teletext::data_fetcher::processors::process_goal_events_with_disambiguation;

#[test]
fn test_different_players_same_lastname_both_show_disambiguation() {
    // Test scenario like in the screenshot: two different Erholtz players
    let home_players = vec![
        (13, "Erik".to_string(), "Erholtz".to_string()), // Player 13 Erholtz E.
        (56, "Anton".to_string(), "Erholtz".to_string()), // Player 56 Erholtz A. (different first name)
        (42, "Teemu".to_string(), "Selänne".to_string()), // Unique player
    ];

    let away_players = vec![(201, "Patrik".to_string(), "Laine".to_string())];

    // Create goal events where both Erholtz players score
    let home_goal_events = vec![
        GoalEvent {
            scorer_player_id: 13, // Erik Erholtz - first goal
            scorer_player: None,
            log_time: "18:35:00".to_string(),
            game_time: 300, // 5 minutes
            period: 1,
            event_id: 1,
            home_team_score: 1,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec!["AV".to_string()],
            assistant_player_ids: vec![],
            assistant_players: vec![],
            video_clip_url: None,
        },
        GoalEvent {
            scorer_player_id: 56, // Anton Erholtz - different player, same last name
            scorer_player: None,
            log_time: "18:42:00".to_string(),
            game_time: 720, // 12 minutes
            period: 1,
            event_id: 2,
            home_team_score: 2,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec!["YV".to_string(), "VT".to_string()],
            assistant_player_ids: vec![],
            assistant_players: vec![],
            video_clip_url: None,
        },
        GoalEvent {
            scorer_player_id: 13, // Erik Erholtz - second goal by same player
            scorer_player: None,
            log_time: "19:15:00".to_string(),
            game_time: 1500, // 25 minutes
            period: 2,
            event_id: 3,
            home_team_score: 3,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec!["EV".to_string()],
            assistant_player_ids: vec![],
            assistant_players: vec![],
            video_clip_url: None,
        },
    ];

    let away_goal_events = vec![GoalEvent {
        scorer_player_id: 201, // Patrik Laine - no disambiguation needed
        scorer_player: None,
        log_time: "19:30:00".to_string(),
        game_time: 2400, // 40 minutes
        period: 3,
        event_id: 4,
        home_team_score: 3,
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
        id: 12347,
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

    // Find Erik Erholtz goals (player ID 13)
    let erik_goals: Vec<_> = goal_events
        .iter()
        .filter(|event| event.scorer_player_id == 13)
        .collect();

    assert_eq!(erik_goals.len(), 2, "Should have 2 goals by Erik Erholtz");

    // Erik's first goal should show disambiguation "Erholtz E."
    assert_eq!(
        erik_goals[0].scorer_name, "Erholtz E.",
        "Erik Erholtz first goal should show disambiguation 'Erholtz E.'"
    );

    // Erik's second goal should show just "Erholtz" (no disambiguation for same player)
    assert_eq!(
        erik_goals[1].scorer_name, "Erholtz",
        "Erik Erholtz second goal should show just 'Erholtz'"
    );

    // Find Anton Erholtz goal (player ID 56)
    let anton_goal = goal_events
        .iter()
        .find(|event| event.scorer_player_id == 56)
        .expect("Should find Anton Erholtz goal");

    // Anton's goal should show disambiguation "Erholtz A." since it's a different player
    assert_eq!(
        anton_goal.scorer_name, "Erholtz A.",
        "Anton Erholtz should show disambiguation 'Erholtz A.'"
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

    println!("✓ Same lastname different players test passed");
    println!(
        "  - Erik Erholtz first goal: '{}'",
        erik_goals[0].scorer_name
    );
    println!("  - Anton Erholtz goal: '{}'", anton_goal.scorer_name);
    println!(
        "  - Erik Erholtz second goal: '{}'",
        erik_goals[1].scorer_name
    );
    println!("  - Patrik Laine goal: '{}'", laine_goal.scorer_name);
}

#[test]
fn test_same_player_multiple_goals_no_disambiguation_needed() {
    // Test scenario where a player scores multiple goals but doesn't need disambiguation
    let home_players = vec![
        (13, "Erik".to_string(), "Erholtz".to_string()), // Only Erholtz on team
        (42, "Teemu".to_string(), "Selänne".to_string()), // Unique player
    ];

    let away_players = vec![(201, "Patrik".to_string(), "Laine".to_string())];

    // Create goal events where Erholtz scores multiple goals
    let home_goal_events = vec![
        GoalEvent {
            scorer_player_id: 13, // Erik Erholtz - first goal
            scorer_player: None,
            log_time: "18:35:00".to_string(),
            game_time: 300,
            period: 1,
            event_id: 1,
            home_team_score: 1,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec!["AV".to_string()],
            assistant_player_ids: vec![],
            assistant_players: vec![],
            video_clip_url: None,
        },
        GoalEvent {
            scorer_player_id: 13, // Erik Erholtz - second goal by same player
            scorer_player: None,
            log_time: "19:15:00".to_string(),
            game_time: 1500,
            period: 2,
            event_id: 2,
            home_team_score: 2,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec!["YV".to_string(), "VT".to_string()],
            assistant_player_ids: vec![],
            assistant_players: vec![],
            video_clip_url: None,
        },
    ];

    let home_team = ScheduleTeam {
        team_id: Some("1".to_string()),
        team_name: Some("Tappara".to_string()),
        goal_events: home_goal_events,
        ..Default::default()
    };

    let away_team = ScheduleTeam {
        team_id: Some("2".to_string()),
        team_name: Some("HIFK".to_string()),
        goal_events: vec![],
        ..Default::default()
    };

    let schedule_game = ScheduleGame {
        id: 12348,
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

    // Verify we have 2 goal events total
    assert_eq!(goal_events.len(), 2, "Should have 2 goal events");

    // Find Erik Erholtz goals (player ID 13)
    let erik_goals: Vec<_> = goal_events
        .iter()
        .filter(|event| event.scorer_player_id == 13)
        .collect();

    assert_eq!(erik_goals.len(), 2, "Should have 2 goals by Erik Erholtz");

    // Both goals should show just "Erholtz" since no disambiguation is needed
    assert_eq!(
        erik_goals[0].scorer_name, "Erholtz",
        "Erik Erholtz first goal should show just 'Erholtz' (no disambiguation needed)"
    );

    assert_eq!(
        erik_goals[1].scorer_name, "Erholtz",
        "Erik Erholtz second goal should show just 'Erholtz' (no disambiguation needed)"
    );

    println!("✓ Same player multiple goals no disambiguation test passed");
    println!(
        "  - Erik Erholtz first goal: '{}'",
        erik_goals[0].scorer_name
    );
    println!(
        "  - Erik Erholtz second goal: '{}'",
        erik_goals[1].scorer_name
    );
}
