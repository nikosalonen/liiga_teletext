use crate::constants::env_vars;
use crate::data_fetcher::models::{GoalEventData, HasGoalEvents, ScheduleGame};
use crate::data_fetcher::player_names::{
    DisambiguationContext, build_full_name, format_for_display,
};
use crate::error::AppError;
use crate::teletext_ui::ScoreType;
use chrono::{DateTime, Local, Utc};
use std::collections::{HashMap, HashSet};
use tracing;

// Import game status functions from game_status module
use super::game_status::{determine_game_status, format_time};
// Import goal event processing functions from goal_events module
use super::goal_events::{create_basic_goal_events, process_goal_events, process_goal_events_with_disambiguation, process_team_goals, process_team_goals_with_disambiguation};
// Import time formatting functions from time_formatting module
use super::time_formatting::{should_show_todays_games, should_show_todays_games_with_time};





/// Try to fetch player names for specific player IDs with a reduced timeout
/// This is used as a fallback when cached player names are missing
#[allow(dead_code)]
async fn try_fetch_player_names_for_game(
    api_domain: &str,
    season: i32,
    game_id: i32,
    player_ids: &[i64],
) -> Option<HashMap<i64, String>> {
    use crate::data_fetcher::api::build_game_url;
    use crate::data_fetcher::models::{DetailedGameResponse, Player};
    use reqwest::Client;
    use std::time::Duration;
    use tracing::{debug, warn};

    if player_ids.is_empty() {
        return Some(HashMap::new());
    }

    debug!(
        "Attempting to fetch player names for {} players in game ID {} (season {})",
        player_ids.len(),
        game_id,
        season
    );

    // Get timeout from env var with 5 second default, clamped to safe range (1-30 seconds)
    let timeout_secs = std::env::var(env_vars::API_FETCH_TIMEOUT)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5)
        .clamp(1, 30);

    // Create a client with a configurable timeout for this fallback attempt
    let client = match Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            warn!("Failed to create HTTP client for player name fetch: {}", e);
            return None;
        }
    };

    // Use the provided API domain and season
    let url = build_game_url(api_domain, season, game_id);

    match client.get(&url).send().await {
        Ok(response) => {
            // Check for HTTP errors before trying to parse JSON
            let response = match response.error_for_status() {
                Ok(response) => response,
                Err(e) => {
                    debug!(
                        "HTTP error for player name fetch (game ID {}): {}",
                        game_id, e
                    );
                    return None;
                }
            };

            match response.json::<DetailedGameResponse>().await {
                Ok(game_response) => {
                    debug!(
                        "Successfully fetched detailed game data for player name lookup (game ID: {})",
                        game_id
                    );

                    let mut player_names = HashMap::new();
                    // Convert player_ids to HashSet for O(1) lookup instead of O(n) contains()
                    let mut wanted_ids: HashSet<i64> = HashSet::with_capacity(player_ids.len());
                    wanted_ids.extend(player_ids.iter().copied());

                    // Helper to process players and extract names for the requested IDs
                    let mut process_players = |players: &[Player]| {
                        for player in players {
                            if wanted_ids.contains(&player.id) {
                                let full_name =
                                    build_full_name(&player.first_name, &player.last_name);
                                let display_name = format_for_display(&full_name);
                                player_names.insert(player.id, display_name);
                            }
                        }
                    };

                    // Process both home and away team players
                    process_players(&game_response.home_team_players);
                    process_players(&game_response.away_team_players);

                    if !player_names.is_empty() {
                        debug!(
                            "Successfully fetched {} player names for game ID {}",
                            player_names.len(),
                            game_id
                        );
                        Some(player_names)
                    } else {
                        debug!(
                            "No player names found for requested IDs in game ID {}",
                            game_id
                        );
                        None
                    }
                }
                Err(e) => {
                    debug!("Failed to parse game response for player names: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            debug!(
                "Failed to fetch game data for player names (game ID {}): {}",
                game_id, e
            );
            None
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::models::{GoalEvent, ScheduleGame, ScheduleTeam};

    fn create_test_goal_event(
        scorer_player_id: i64,
        game_time: i32,
        home_score: i32,
        away_score: i32,
        goal_types: Vec<String>,
    ) -> GoalEvent {
        GoalEvent {
            scorer_player_id,
            log_time: "18:30:00".to_string(),
            game_time,
            period: 1,
            event_id: 1,
            home_team_score: home_score,
            away_team_score: away_score,
            winning_goal: false,
            goal_types,
            assistant_player_ids: vec![],
            video_clip_url: Some("https://example.com/video.mp4".to_string()),
            scorer_player: None,
        }
    }

    fn create_test_team_with_goals(goals: Vec<GoalEvent>) -> ScheduleTeam {
        ScheduleTeam {
            goal_events: goals,
            ..Default::default()
        }
    }

    fn create_test_game(home_goals: Vec<GoalEvent>, away_goals: Vec<GoalEvent>) -> ScheduleGame {
        ScheduleGame {
            id: 1,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            end: None,
            home_team: create_test_team_with_goals(home_goals),
            away_team: create_test_team_with_goals(away_goals),
            finished_type: None,
            started: true,
            ended: false,
            game_time: 1200, // 20 minutes
            serie: "runkosarja".to_string(),
        }
    }

    #[test]
    fn test_process_goal_events_empty_game() {
        let game = create_test_game(vec![], vec![]);
        let player_names = HashMap::new();

        let events = process_goal_events(&game, &player_names);
        assert!(events.is_empty());
    }

    #[test]
    fn test_process_goal_events_with_goals() {
        let home_goal = create_test_goal_event(123, 900, 1, 0, vec!["EV".to_string()]);
        let away_goal = create_test_goal_event(456, 1200, 1, 1, vec!["YV".to_string()]);

        let game = create_test_game(vec![home_goal], vec![away_goal]);

        let mut player_names = HashMap::new();
        player_names.insert(123, "Koivu".to_string());
        player_names.insert(456, "Selänne".to_string());

        let events = process_goal_events(&game, &player_names);

        assert_eq!(events.len(), 2);

        // Check home goal
        let home_event = &events[0];
        assert_eq!(home_event.scorer_player_id, 123);
        assert_eq!(home_event.scorer_name, "Koivu");
        assert_eq!(home_event.minute, 15); // 900 seconds / 60
        assert_eq!(home_event.home_team_score, 1);
        assert_eq!(home_event.away_team_score, 0);
        assert!(home_event.is_home_team);
        assert_eq!(home_event.goal_types, vec!["EV"]);

        // Check away goal
        let away_event = &events[1];
        assert_eq!(away_event.scorer_player_id, 456);
        assert_eq!(away_event.scorer_name, "Selänne");
        assert_eq!(away_event.minute, 20); // 1200 seconds / 60
        assert_eq!(away_event.home_team_score, 1);
        assert_eq!(away_event.away_team_score, 1);
        assert!(!away_event.is_home_team);
        assert_eq!(away_event.goal_types, vec!["YV"]);
    }

    #[test]
    fn test_process_goal_events_with_fallback_names() {
        let home_goal = create_test_goal_event(999, 600, 1, 0, vec!["EV".to_string()]);
        let game = create_test_game(vec![home_goal], vec![]);

        // No player names provided - should use fallback
        let player_names = HashMap::new();

        let events = process_goal_events(&game, &player_names);

        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.scorer_player_id, 999);
        assert_eq!(event.scorer_name, "Pelaaja 999"); // Fallback name
    }

    #[test]
    fn test_process_team_goals_filters_cancelled_goals() {
        let valid_goal = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        let cancelled_goal_rl0 = create_test_goal_event(456, 900, 1, 0, vec!["RL0".to_string()]);
        let cancelled_goal_vt0 = create_test_goal_event(789, 1200, 1, 0, vec!["VT0".to_string()]);

        let team =
            create_test_team_with_goals(vec![valid_goal, cancelled_goal_rl0, cancelled_goal_vt0]);
        let mut player_names = HashMap::new();
        player_names.insert(123, "Koivu".to_string());
        player_names.insert(456, "Cancelled1".to_string());
        player_names.insert(789, "Cancelled2".to_string());

        let mut events = Vec::new();
        process_team_goals(&team, &player_names, true, &mut events);

        // Should only have the valid goal, cancelled goals filtered out
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].scorer_player_id, 123);
        assert_eq!(events[0].scorer_name, "Koivu");
    }


    #[test]
    fn test_determine_game_status_scheduled() {
        let mut game = create_test_game(vec![], vec![]);
        game.started = false;
        game.ended = false;
        game.finished_type = None;

        let (score_type, is_overtime, is_shootout) = determine_game_status(&game);

        assert!(matches!(score_type, ScoreType::Scheduled));
        assert!(!is_overtime);
        assert!(!is_shootout);
    }

    #[test]
    fn test_determine_game_status_ongoing() {
        let mut game = create_test_game(vec![], vec![]);
        game.started = true;
        game.ended = false;
        game.finished_type = None;

        let (score_type, is_overtime, is_shootout) = determine_game_status(&game);

        assert!(matches!(score_type, ScoreType::Ongoing));
        assert!(!is_overtime);
        assert!(!is_shootout);
    }

    #[test]
    fn test_determine_game_status_finished_regular() {
        let mut game = create_test_game(vec![], vec![]);
        game.started = true;
        game.ended = true;
        game.finished_type = Some("ENDED_DURING_REGULAR_TIME".to_string());

        let (score_type, is_overtime, is_shootout) = determine_game_status(&game);

        assert!(matches!(score_type, ScoreType::Final));
        assert!(!is_overtime);
        assert!(!is_shootout);
    }

    #[test]
    fn test_determine_game_status_overtime() {
        let mut game = create_test_game(vec![], vec![]);
        game.started = true;
        game.ended = true;
        game.finished_type = Some("ENDED_DURING_EXTENDED_GAME_TIME".to_string());

        let (score_type, is_overtime, is_shootout) = determine_game_status(&game);

        assert!(matches!(score_type, ScoreType::Final));
        assert!(is_overtime);
        assert!(!is_shootout);
    }

    #[test]
    fn test_determine_game_status_shootout() {
        let mut game = create_test_game(vec![], vec![]);
        game.started = true;
        game.ended = true;
        game.finished_type = Some("ENDED_DURING_WINNING_SHOT_COMPETITION".to_string());

        let (score_type, is_overtime, is_shootout) = determine_game_status(&game);

        assert!(matches!(score_type, ScoreType::Final));
        assert!(!is_overtime);
        assert!(is_shootout);
    }

    #[test]
    fn test_format_time_valid_utc() {
        let timestamp = "2024-01-15T18:30:00Z";
        let result = format_time(timestamp);

        assert!(result.is_ok());
        let formatted = result.unwrap();
        // Should be in HH.MM format
        assert!(formatted.contains('.'));
        assert_eq!(formatted.len(), 5); // HH.MM is 5 characters
    }

    #[test]
    fn test_format_time_valid_with_timezone() {
        let timestamp = "2024-01-15T18:30:00+02:00";
        let result = format_time(timestamp);

        assert!(result.is_ok());
        let formatted = result.unwrap();
        assert!(formatted.contains('.'));
        assert_eq!(formatted.len(), 5);
    }

    #[test]
    fn test_format_time_invalid_format() {
        let invalid_timestamp = "not a timestamp";
        let result = format_time(invalid_timestamp);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::DateTimeParse(_)));
    }

    #[test]
    fn test_format_time_empty_string() {
        let result = format_time("");

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::DateTimeParse(_)));
    }

    #[test]
    fn test_format_time_invalid_date() {
        let invalid_timestamp = "2024-13-45T25:70:90Z"; // Invalid date/time values
        let result = format_time(invalid_timestamp);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::DateTimeParse(_)));
    }

    #[tokio::test]
    async fn test_create_basic_goal_events() {
        let home_goal = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        let away_goal = create_test_goal_event(456, 900, 1, 1, vec!["YV".to_string()]);

        let game = create_test_game(vec![home_goal], vec![away_goal]);

        let events = create_basic_goal_events(&game, "test-api.example.com").await;

        assert_eq!(events.len(), 2);

        // Should use fallback names since no player names cache is provided
        assert_eq!(events[0].scorer_name, "Pelaaja 123");
        assert_eq!(events[1].scorer_name, "Pelaaja 456");
    }

    #[tokio::test]
    async fn test_create_basic_goal_events_empty_game() {
        let game = create_test_game(vec![], vec![]);
        let events = create_basic_goal_events(&game, "test-api.example.com").await;
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_create_basic_goal_events_with_scores_but_no_events() {
        // Test the new fallback logic for games with scores but no goal events
        let mut game = create_test_game(vec![], vec![]);

        // Set scores but keep goal_events empty (simulates schedule response)
        game.home_team.goals = 2;
        game.away_team.goals = 1;

        let events = create_basic_goal_events(&game, "test-api.example.com").await;

        // Should create placeholder events based on scores
        assert_eq!(events.len(), 3); // 2 home + 1 away

        // Check home team events
        let home_events: Vec<_> = events.iter().filter(|e| e.is_home_team).collect();
        assert_eq!(home_events.len(), 2);
        assert_eq!(home_events[0].scorer_name, "Tuntematon pelaaja");
        assert_eq!(home_events[0].home_team_score, 1);
        assert_eq!(home_events[1].home_team_score, 2);

        // Check away team events
        let away_events: Vec<_> = events.iter().filter(|e| !e.is_home_team).collect();
        assert_eq!(away_events.len(), 1);
        assert_eq!(away_events[0].scorer_name, "Tuntematon pelaaja");
        assert_eq!(away_events[0].away_team_score, 1);
    }

    #[test]
    fn test_goal_event_data_fields() {
        let goal = create_test_goal_event(123, 900, 2, 1, vec!["YV".to_string(), "MV".to_string()]);
        let game = create_test_game(vec![], vec![goal]);

        let mut player_names = HashMap::new();
        player_names.insert(123, "Test Player".to_string());

        let events = process_goal_events(&game, &player_names);
        assert_eq!(events.len(), 1);

        let event = &events[0];
        assert_eq!(event.scorer_player_id, 123);
        assert_eq!(event.scorer_name, "Test Player");
        assert_eq!(event.minute, 15); // 900 / 60
        assert_eq!(event.home_team_score, 2);
        assert_eq!(event.away_team_score, 1);
        assert!(!event.is_winning_goal);
        assert_eq!(event.goal_types, vec!["YV", "MV"]);
        assert!(!event.is_home_team); // Away team goal
        assert_eq!(
            event.video_clip_url,
            Some("https://example.com/video.mp4".to_string())
        );
    }

    #[test]
    fn test_process_goal_events_preserves_winning_goal_flag() {
        let mut winning_goal = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        winning_goal.winning_goal = true;

        let game = create_test_game(vec![winning_goal], vec![]);
        let mut player_names = HashMap::new();
        player_names.insert(123, "Winner".to_string());

        let events = process_goal_events(&game, &player_names);

        assert_eq!(events.len(), 1);
        assert!(events[0].is_winning_goal);
    }

    #[test]
    fn test_process_goal_events_multiple_goal_types() {
        let complex_goal = create_test_goal_event(
            123,
            600,
            1,
            0,
            vec!["YV".to_string(), "RV".to_string(), "MV".to_string()],
        );

        let game = create_test_game(vec![complex_goal], vec![]);
        let mut player_names = HashMap::new();
        player_names.insert(123, "Complex Scorer".to_string());

        let events = process_goal_events(&game, &player_names);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].goal_types, vec!["YV", "RV", "MV"]);
    }

    #[test]
    fn test_process_goal_events_no_video_url() {
        let mut goal_without_video = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        goal_without_video.video_clip_url = None;

        let game = create_test_game(vec![goal_without_video], vec![]);
        let mut player_names = HashMap::new();
        player_names.insert(123, "No Video".to_string());

        let events = process_goal_events(&game, &player_names);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].video_clip_url, None);
    }

    #[test]
    fn test_edge_cases_zero_game_time() {
        let zero_time_goal = create_test_goal_event(123, 0, 1, 0, vec!["EV".to_string()]);
        let game = create_test_game(vec![zero_time_goal], vec![]);
        let mut player_names = HashMap::new();
        player_names.insert(123, "Quick Goal".to_string());

        let events = process_goal_events(&game, &player_names);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].minute, 0); // 0 / 60 = 0
    }

    #[test]
    fn test_edge_cases_large_game_time() {
        let late_goal = create_test_goal_event(123, 7200, 1, 0, vec!["EV".to_string()]); // 2 hours
        let game = create_test_game(vec![late_goal], vec![]);
        let mut player_names = HashMap::new();
        player_names.insert(123, "Very Late Goal".to_string());

        let events = process_goal_events(&game, &player_names);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].minute, 120); // 7200 / 60 = 120 minutes
    }

    // Tests for process_goal_events_with_disambiguation
    #[test]
    fn test_process_goal_events_with_disambiguation_basic() {
        let home_goal = create_test_goal_event(123, 900, 1, 0, vec!["EV".to_string()]);
        let away_goal = create_test_goal_event(456, 1200, 1, 1, vec!["YV".to_string()]);

        let game = create_test_game(vec![home_goal], vec![away_goal]);

        let home_players = vec![
            (123, "Mikko".to_string(), "Koivu".to_string()),
            (124, "Saku".to_string(), "Koivu".to_string()),
        ];
        let away_players = vec![(456, "Teemu".to_string(), "Selänne".to_string())];

        let events = process_goal_events_with_disambiguation(&game, &home_players, &away_players);

        assert_eq!(events.len(), 2);

        // Check home goal - should be disambiguated because two Koivus on home team
        let home_event = &events[0];
        assert_eq!(home_event.scorer_player_id, 123);
        assert_eq!(home_event.scorer_name, "Koivu M.");
        assert!(home_event.is_home_team);

        // Check away goal - should not be disambiguated because only one Selänne on away team
        let away_event = &events[1];
        assert_eq!(away_event.scorer_player_id, 456);
        assert_eq!(away_event.scorer_name, "Selänne");
        assert!(!away_event.is_home_team);
    }

    #[test]
    fn test_process_goal_events_with_disambiguation_team_scoped() {
        // Both teams have a "Koivu" but they shouldn't affect each other's disambiguation
        let home_goal = create_test_goal_event(123, 900, 1, 0, vec!["EV".to_string()]);
        let away_goal = create_test_goal_event(456, 1200, 1, 1, vec!["YV".to_string()]);

        let game = create_test_game(vec![home_goal], vec![away_goal]);

        let home_players = vec![(123, "Mikko".to_string(), "Koivu".to_string())];
        let away_players = vec![(456, "Saku".to_string(), "Koivu".to_string())];

        let events = process_goal_events_with_disambiguation(&game, &home_players, &away_players);

        assert_eq!(events.len(), 2);

        // Both should show as "Koivu" without disambiguation since they're on different teams
        let home_event = &events[0];
        assert_eq!(home_event.scorer_name, "Koivu");
        assert!(home_event.is_home_team);

        let away_event = &events[1];
        assert_eq!(away_event.scorer_name, "Koivu");
        assert!(!away_event.is_home_team);
    }

    #[test]
    fn test_process_goal_events_with_disambiguation_multiple_same_name() {
        let home_goal1 = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        let home_goal2 = create_test_goal_event(124, 900, 2, 0, vec!["EV".to_string()]);
        let home_goal3 = create_test_goal_event(125, 1200, 3, 0, vec!["EV".to_string()]);

        let game = create_test_game(vec![home_goal1, home_goal2, home_goal3], vec![]);

        let home_players = vec![
            (123, "Mikko".to_string(), "Koivu".to_string()),
            (124, "Saku".to_string(), "Koivu".to_string()),
            (125, "Antti".to_string(), "Koivu".to_string()),
        ];
        let away_players = vec![];

        let events = process_goal_events_with_disambiguation(&game, &home_players, &away_players);

        assert_eq!(events.len(), 3);

        // All three should be disambiguated
        assert_eq!(events[0].scorer_name, "Koivu M.");
        assert_eq!(events[1].scorer_name, "Koivu S.");
        assert_eq!(events[2].scorer_name, "Koivu A.");
    }

    #[test]
    fn test_process_goal_events_with_disambiguation_mixed_scenario() {
        let home_goal1 = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        let home_goal2 = create_test_goal_event(124, 900, 2, 0, vec!["EV".to_string()]);
        let home_goal3 = create_test_goal_event(125, 1200, 3, 0, vec!["EV".to_string()]);

        let game = create_test_game(vec![home_goal1, home_goal2, home_goal3], vec![]);

        let home_players = vec![
            (123, "Mikko".to_string(), "Koivu".to_string()),
            (124, "Saku".to_string(), "Koivu".to_string()),
            (125, "Teemu".to_string(), "Selänne".to_string()),
        ];
        let away_players = vec![];

        let events = process_goal_events_with_disambiguation(&game, &home_players, &away_players);

        assert_eq!(events.len(), 3);

        // Koivus should be disambiguated, Selänne should not
        assert_eq!(events[0].scorer_name, "Koivu M.");
        assert_eq!(events[1].scorer_name, "Koivu S.");
        assert_eq!(events[2].scorer_name, "Selänne");
    }

    #[test]
    fn test_process_goal_events_with_disambiguation_missing_player() {
        let home_goal = create_test_goal_event(999, 600, 1, 0, vec!["EV".to_string()]);
        let game = create_test_game(vec![home_goal], vec![]);

        let home_players = vec![(123, "Mikko".to_string(), "Koivu".to_string())];
        let away_players = vec![];

        let events = process_goal_events_with_disambiguation(&game, &home_players, &away_players);

        assert_eq!(events.len(), 1);
        // Should use fallback name for missing player
        assert_eq!(events[0].scorer_name, "Pelaaja 999");
    }

    #[test]
    fn test_process_goal_events_with_disambiguation_empty_teams() {
        let game = create_test_game(vec![], vec![]);
        let home_players = vec![];
        let away_players = vec![];

        let events = process_goal_events_with_disambiguation(&game, &home_players, &away_players);

        assert!(events.is_empty());
    }

    #[test]
    fn test_process_goal_events_with_disambiguation_unicode_names() {
        let home_goal = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        let game = create_test_game(vec![home_goal], vec![]);

        let home_players = vec![
            (123, "Äkäslompolo".to_string(), "Kärppä".to_string()),
            (124, "Östen".to_string(), "Kärppä".to_string()),
        ];
        let away_players = vec![];

        let events = process_goal_events_with_disambiguation(&game, &home_players, &away_players);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].scorer_name, "Kärppä Ä.");
    }

    // Tests for process_team_goals_with_disambiguation
    #[test]
    fn test_process_team_goals_with_disambiguation() {
        let goal1 = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        let goal2 = create_test_goal_event(124, 900, 2, 0, vec!["YV".to_string()]);
        let team = create_test_team_with_goals(vec![goal1, goal2]);

        let players = vec![
            (123, "Mikko".to_string(), "Koivu".to_string()),
            (124, "Saku".to_string(), "Koivu".to_string()),
        ];
        let context = DisambiguationContext::new(players);

        let mut events = Vec::new();
        process_team_goals_with_disambiguation(&team, &context, true, &mut events);

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].scorer_name, "Koivu M.");
        assert_eq!(events[1].scorer_name, "Koivu S.");
        assert!(events[0].is_home_team);
        assert!(events[1].is_home_team);
    }

    #[test]
    fn test_process_team_goals_with_disambiguation_filters_cancelled() {
        let valid_goal = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        let cancelled_goal_rl0 = create_test_goal_event(124, 900, 1, 0, vec!["RL0".to_string()]);
        let cancelled_goal_vt0 = create_test_goal_event(125, 1200, 1, 0, vec!["VT0".to_string()]);

        let team =
            create_test_team_with_goals(vec![valid_goal, cancelled_goal_rl0, cancelled_goal_vt0]);

        let players = vec![
            (123, "Mikko".to_string(), "Koivu".to_string()),
            (124, "Saku".to_string(), "Koivu".to_string()),
            (125, "Antti".to_string(), "Koivu".to_string()),
        ];
        let context = DisambiguationContext::new(players);

        let mut events = Vec::new();
        process_team_goals_with_disambiguation(&team, &context, true, &mut events);

        // Should only have the valid goal, cancelled goals filtered out
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].scorer_player_id, 123);
        assert_eq!(events[0].scorer_name, "Koivu M.");
    }

    #[test]
    fn test_process_team_goals_with_disambiguation_missing_player() {
        let goal = create_test_goal_event(999, 600, 1, 0, vec!["EV".to_string()]);
        let team = create_test_team_with_goals(vec![goal]);

        let players = vec![(123, "Mikko".to_string(), "Koivu".to_string())];
        let context = DisambiguationContext::new(players);

        let mut events = Vec::new();
        process_team_goals_with_disambiguation(&team, &context, true, &mut events);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].scorer_name, "Pelaaja 999");
    }
}
