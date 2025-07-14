use crate::data_fetcher::models::{GameData, GoalEventData};
use crate::teletext_ui::ScoreType;

/// Test utilities for creating mock data and testing scenarios
pub struct TestDataBuilder;

impl TestDataBuilder {
    /// Creates a basic game data for testing
    pub fn create_basic_game(home_team: &str, away_team: &str) -> GameData {
        GameData {
            home_team: home_team.to_string(),
            away_team: away_team.to_string(),
            time: "18:30".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T18:30:00Z".to_string(),
        }
    }

    /// Creates a game with overtime
    pub fn create_overtime_game(home_team: &str, away_team: &str) -> GameData {
        GameData {
            home_team: home_team.to_string(),
            away_team: away_team.to_string(),
            time: "18:30".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: true,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3900, // 65 minutes
            start: "2024-01-15T18:30:00Z".to_string(),
        }
    }

    /// Creates a game with shootout
    pub fn create_shootout_game(home_team: &str, away_team: &str) -> GameData {
        GameData {
            home_team: home_team.to_string(),
            away_team: away_team.to_string(),
            time: "18:30".to_string(),
            result: "3-2".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: true,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3900, // 65 minutes
            start: "2024-01-15T18:30:00Z".to_string(),
        }
    }

    /// Creates a live game
    pub fn create_live_game(home_team: &str, away_team: &str, current_score: &str) -> GameData {
        GameData {
            home_team: home_team.to_string(),
            away_team: away_team.to_string(),
            time: "18:30".to_string(),
            result: current_score.to_string(),
            score_type: ScoreType::Ongoing,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 2400, // 40 minutes
            start: "2024-01-15T18:30:00Z".to_string(),
        }
    }

    /// Creates a goal event for testing
    pub fn create_goal_event(
        scorer_name: &str,
        minute: i32,
        home_score: i32,
        away_score: i32,
        is_home_team: bool,
    ) -> GoalEventData {
        GoalEventData {
            scorer_player_id: 12345,
            scorer_name: scorer_name.to_string(),
            minute,
            home_team_score: home_score,
            away_team_score: away_score,
            is_winning_goal: false,
            goal_types: vec![],
            is_home_team,
            video_clip_url: None,
        }
    }

    /// Creates a power play goal event
    pub fn create_powerplay_goal(
        scorer_name: &str,
        minute: i32,
        home_score: i32,
        away_score: i32,
        is_home_team: bool,
    ) -> GoalEventData {
        GoalEventData {
            scorer_player_id: 12345,
            scorer_name: scorer_name.to_string(),
            minute,
            home_team_score: home_score,
            away_team_score: away_score,
            is_winning_goal: false,
            goal_types: vec!["YV".to_string()], // Power play
            is_home_team,
            video_clip_url: Some("https://example.com/video.mp4".to_string()),
        }
    }

    /// Creates a winning goal event
    pub fn create_winning_goal(
        scorer_name: &str,
        minute: i32,
        home_score: i32,
        away_score: i32,
        is_home_team: bool,
    ) -> GoalEventData {
        GoalEventData {
            scorer_player_id: 12345,
            scorer_name: scorer_name.to_string(),
            minute,
            home_team_score: home_score,
            away_team_score: away_score,
            is_winning_goal: true,
            goal_types: vec![],
            is_home_team,
            video_clip_url: None,
        }
    }

    /// Creates multiple games for pagination testing
    pub fn create_multiple_games(count: usize) -> Vec<GameData> {
        (0..count)
            .map(|i| {
                Self::create_basic_game(&format!("Team {}", i * 2), &format!("Team {}", i * 2 + 1))
            })
            .collect()
    }

    /// Creates games with different tournament types
    pub fn create_tournament_games() -> Vec<GameData> {
        let tournaments = vec!["runkosarja", "playoffs", "playout", "qualifications"];
        tournaments
            .into_iter()
            .enumerate()
            .map(|(i, tournament)| {
                let mut game = Self::create_basic_game(
                    &format!("Team {}", i * 2),
                    &format!("Team {}", i * 2 + 1),
                );
                game.serie = tournament.to_string();
                game
            })
            .collect()
    }
}

/// Property-based testing utilities
pub struct PropertyTesting;

impl PropertyTesting {
    /// Validates that a game data structure is consistent
    pub fn validate_game_data(game: &GameData) -> Result<(), String> {
        // Check that team names are not empty
        if game.home_team.is_empty() {
            return Err("Home team name cannot be empty".to_string());
        }
        if game.away_team.is_empty() {
            return Err("Away team name cannot be empty".to_string());
        }

        // Check that time format is reasonable
        if game.time.is_empty() {
            return Err("Game time cannot be empty".to_string());
        }

        // Check that result format is reasonable for finished games
        if matches!(game.score_type, ScoreType::Final) && game.result.is_empty() {
            return Err("Final game must have a result".to_string());
        }

        // Check that overtime and shootout flags are mutually exclusive
        if game.is_overtime && game.is_shootout {
            return Err("Game cannot be both overtime and shootout".to_string());
        }

        // Check that played time is reasonable
        if game.played_time < 0 {
            return Err("Played time cannot be negative".to_string());
        }

        // For overtime games, played time should be more than 60 minutes
        if game.is_overtime && game.played_time <= 3600 {
            return Err("Overtime game should have more than 60 minutes played".to_string());
        }

        // Check that start time is a valid ISO 8601 format
        if chrono::DateTime::parse_from_rfc3339(&game.start).is_err() {
            return Err("Start time must be valid ISO 8601 format".to_string());
        }

        Ok(())
    }

    /// Validates that goal events are consistent with game data
    pub fn validate_goal_events(game: &GameData) -> Result<(), String> {
        for (i, goal) in game.goal_events.iter().enumerate() {
            // Check that scorer name is not empty
            if goal.scorer_name.is_empty() {
                return Err(format!("Goal {} scorer name cannot be empty", i));
            }

            // Check that minute is reasonable
            if goal.minute < 0 || goal.minute > 200 {
                return Err(format!("Goal {} minute {} is unreasonable", i, goal.minute));
            }

            // Check that scores are non-negative
            if goal.home_team_score < 0 || goal.away_team_score < 0 {
                return Err(format!("Goal {} scores cannot be negative", i));
            }

            // Check that scores are progressing logically
            if i > 0 {
                let prev_goal = &game.goal_events[i - 1];
                let total_prev = prev_goal.home_team_score + prev_goal.away_team_score;
                let total_current = goal.home_team_score + goal.away_team_score;

                if total_current != total_prev + 1 {
                    return Err(format!(
                        "Goal {} score progression is invalid: {} -> {}",
                        i, total_prev, total_current
                    ));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_basic_game() {
        let game = TestDataBuilder::create_basic_game("HIFK", "Jokerit");
        assert_eq!(game.home_team, "HIFK");
        assert_eq!(game.away_team, "Jokerit");
        assert_eq!(game.score_type, ScoreType::Final);
        assert!(!game.is_overtime);
        assert!(!game.is_shootout);
    }

    #[test]
    fn test_create_overtime_game() {
        let game = TestDataBuilder::create_overtime_game("HIFK", "Jokerit");
        assert!(game.is_overtime);
        assert!(!game.is_shootout);
        assert!(game.played_time > 3600);
    }

    #[test]
    fn test_create_shootout_game() {
        let game = TestDataBuilder::create_shootout_game("HIFK", "Jokerit");
        assert!(!game.is_overtime);
        assert!(game.is_shootout);
    }

    #[test]
    fn test_create_live_game() {
        let game = TestDataBuilder::create_live_game("HIFK", "Jokerit", "1-1");
        assert_eq!(game.score_type, ScoreType::Ongoing);
        assert_eq!(game.result, "1-1");
    }

    #[test]
    fn test_create_goal_event() {
        let goal = TestDataBuilder::create_goal_event("Koivu", 15, 1, 0, true);
        assert_eq!(goal.scorer_name, "Koivu");
        assert_eq!(goal.minute, 15);
        assert_eq!(goal.home_team_score, 1);
        assert_eq!(goal.away_team_score, 0);
        assert!(goal.is_home_team);
    }

    #[test]
    fn test_create_powerplay_goal() {
        let goal = TestDataBuilder::create_powerplay_goal("Koivu", 15, 1, 0, true);
        assert_eq!(goal.goal_types, vec!["YV"]);
        assert!(goal.video_clip_url.is_some());
    }

    #[test]
    fn test_create_winning_goal() {
        let goal = TestDataBuilder::create_winning_goal("Koivu", 58, 2, 1, true);
        assert!(goal.is_winning_goal);
    }

    #[test]
    fn test_create_multiple_games() {
        let games = TestDataBuilder::create_multiple_games(5);
        assert_eq!(games.len(), 5);
        assert_eq!(games[0].home_team, "Team 0");
        assert_eq!(games[0].away_team, "Team 1");
        assert_eq!(games[4].home_team, "Team 8");
        assert_eq!(games[4].away_team, "Team 9");
    }

    #[test]
    fn test_create_tournament_games() {
        let games = TestDataBuilder::create_tournament_games();
        assert_eq!(games.len(), 4);
        assert_eq!(games[0].serie, "runkosarja");
        assert_eq!(games[1].serie, "playoffs");
        assert_eq!(games[2].serie, "playout");
        assert_eq!(games[3].serie, "qualifications");
    }

    #[test]
    fn test_validate_game_data_valid() {
        let game = TestDataBuilder::create_basic_game("HIFK", "Jokerit");
        assert!(PropertyTesting::validate_game_data(&game).is_ok());
    }

    #[test]
    fn test_validate_game_data_invalid_empty_team() {
        let mut game = TestDataBuilder::create_basic_game("", "Jokerit");
        let result = PropertyTesting::validate_game_data(&game);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Home team name cannot be empty")
        );

        game.home_team = "HIFK".to_string();
        game.away_team = "".to_string();
        let result = PropertyTesting::validate_game_data(&game);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Away team name cannot be empty")
        );
    }

    #[test]
    fn test_validate_game_data_invalid_overtime_and_shootout() {
        let mut game = TestDataBuilder::create_basic_game("HIFK", "Jokerit");
        game.is_overtime = true;
        game.is_shootout = true;
        let result = PropertyTesting::validate_game_data(&game);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("cannot be both overtime and shootout")
        );
    }

    #[test]
    fn test_validate_goal_events_valid() {
        let game = TestDataBuilder::create_basic_game("HIFK", "Jokerit");
        assert!(PropertyTesting::validate_goal_events(&game).is_ok());
    }

    #[test]
    fn test_validate_goal_events_invalid_empty_scorer() {
        let mut game = TestDataBuilder::create_basic_game("HIFK", "Jokerit");
        let goal = TestDataBuilder::create_goal_event("", 15, 1, 0, true);
        game.goal_events.push(goal);
        let result = PropertyTesting::validate_goal_events(&game);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("scorer name cannot be empty"));
    }

    #[test]
    fn test_validate_goal_events_invalid_minute() {
        let mut game = TestDataBuilder::create_basic_game("HIFK", "Jokerit");
        let goal = TestDataBuilder::create_goal_event("Koivu", -5, 1, 0, true);
        game.goal_events.push(goal);
        let result = PropertyTesting::validate_goal_events(&game);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("minute -5 is unreasonable"));
    }
}
