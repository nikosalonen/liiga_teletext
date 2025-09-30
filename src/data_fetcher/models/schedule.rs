use super::goals::GoalEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScheduleTeam {
    #[serde(rename = "teamId")]
    pub team_id: Option<String>,
    #[serde(rename = "teamPlaceholder")]
    pub team_placeholder: Option<String>,
    #[serde(rename = "teamName")]
    pub team_name: Option<String>,
    pub goals: i32,
    #[serde(rename = "timeOut", default)]
    pub time_out: Option<i32>,
    #[serde(rename = "powerplayInstances", default)]
    pub powerplay_instances: i32,
    #[serde(rename = "powerplayGoals", default)]
    pub powerplay_goals: i32,
    #[serde(rename = "shortHandedInstances", default)]
    pub short_handed_instances: i32,
    #[serde(rename = "shortHandedGoals", default)]
    pub short_handed_goals: i32,
    #[serde(rename = "ranking", default)]
    pub ranking: Option<i32>,
    #[serde(rename = "gameStartDateTime", default)]
    pub game_start_date_time: Option<String>,
    #[serde(rename = "goalEvents", default)]
    pub goal_events: Vec<GoalEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleGame {
    pub id: i32,
    pub season: i32,
    pub start: String,
    #[serde(default)]
    pub end: Option<String>,
    #[serde(rename = "homeTeam")]
    pub home_team: ScheduleTeam,
    #[serde(rename = "awayTeam")]
    pub away_team: ScheduleTeam,
    #[serde(rename = "finishedType")]
    pub finished_type: Option<String>,
    #[serde(default)]
    pub started: bool,
    #[serde(default)]
    pub ended: bool,
    #[serde(rename = "gameTime", default)]
    pub game_time: i32,
    pub serie: String,
}

/// Model for the schedule API response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleApiGame {
    pub id: i32,
    pub season: i32,
    pub start: String,
    #[serde(rename = "homeTeamName")]
    pub home_team_name: String,
    #[serde(rename = "awayTeamName")]
    pub away_team_name: String,
    pub serie: i32, // This is an integer in the schedule API
    #[serde(rename = "finishedType")]
    pub finished_type: Option<String>,
    pub started: bool,
    pub ended: bool,
    #[serde(rename = "gameTime")]
    pub game_time: Option<i32>, // Can be null in the API response
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ScheduleResponse {
    pub games: Vec<ScheduleGame>,
    #[serde(rename = "previousGameDate")]
    pub previous_game_date: Option<String>,
    #[serde(rename = "nextGameDate")]
    pub next_game_date: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::models::goals::GoalEvent;
    use serde_json;

    fn create_test_goal_event() -> GoalEvent {
        GoalEvent {
            scorer_player_id: 12345,
            log_time: "18:30:00".to_string(),
            game_time: 900,
            period: 1,
            event_id: 1,
            home_team_score: 1,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec!["EV".to_string()],
            assistant_player_ids: vec![67890, 11111],
            video_clip_url: Some("https://example.com/video.mp4".to_string()),
            scorer_player: None,
        }
    }

    fn create_test_schedule_team() -> ScheduleTeam {
        ScheduleTeam {
            team_id: Some("HIFK".to_string()),
            team_placeholder: None,
            team_name: Some("HIFK Helsinki".to_string()),
            goals: 2,
            time_out: None,
            powerplay_instances: 3,
            powerplay_goals: 1,
            short_handed_instances: 1,
            short_handed_goals: 0,
            ranking: Some(5),
            game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
            goal_events: vec![create_test_goal_event()],
        }
    }

    fn create_test_schedule_game() -> ScheduleGame {
        ScheduleGame {
            id: 12345,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            end: Some("2024-01-15T21:00:00Z".to_string()),
            home_team: create_test_schedule_team(),
            away_team: create_test_schedule_team(),
            finished_type: Some("ENDED_DURING_REGULAR_TIME".to_string()),
            started: true,
            ended: true,
            game_time: 3600,
            serie: "runkosarja".to_string(),
        }
    }

    #[test]
    fn test_schedule_team_serialization() {
        let team = create_test_schedule_team();

        // Test serialization
        let json = serde_json::to_string(&team).unwrap();
        assert!(json.contains("\"teamId\":\"HIFK\""));
        assert!(json.contains("\"teamName\":\"HIFK Helsinki\""));
        assert!(json.contains("\"goals\":2"));
        assert!(json.contains("\"goalEvents\""));

        // Test deserialization
        let deserialized: ScheduleTeam = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.team_id, Some("HIFK".to_string()));
        assert_eq!(deserialized.team_name, Some("HIFK Helsinki".to_string()));
        assert_eq!(deserialized.goals, 2);
        assert_eq!(deserialized.goal_events.len(), 1);
    }

    #[test]
    fn test_schedule_game_serialization() {
        let game = create_test_schedule_game();

        // Test serialization
        let json = serde_json::to_string(&game).unwrap();
        assert!(json.contains("\"id\":12345"));
        assert!(json.contains("\"season\":2024"));
        assert!(json.contains("\"homeTeam\""));
        assert!(json.contains("\"awayTeam\""));

        // Test deserialization
        let deserialized: ScheduleGame = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, 12345);
        assert_eq!(deserialized.season, 2024);
        assert_eq!(deserialized.home_team.team_id, Some("HIFK".to_string()));
        assert_eq!(deserialized.away_team.team_id, Some("HIFK".to_string()));
    }

    #[test]
    fn test_schedule_team_default_fields() {
        let json = r#"{
            "goals": 1
        }"#;

        let team: ScheduleTeam = serde_json::from_str(json).unwrap();

        // Test default values
        assert_eq!(team.team_id, None);
        assert_eq!(team.team_placeholder, None);
        assert_eq!(team.team_name, None);
        assert_eq!(team.goals, 1);
        assert_eq!(team.time_out, None);
        assert_eq!(team.powerplay_instances, 0);
        assert_eq!(team.powerplay_goals, 0);
        assert_eq!(team.short_handed_instances, 0);
        assert_eq!(team.short_handed_goals, 0);
        assert_eq!(team.ranking, None);
        assert_eq!(team.game_start_date_time, None);
        assert!(team.goal_events.is_empty());
    }

    #[test]
    fn test_schedule_game_default_fields() {
        let json = r#"{
            "id": 123,
            "season": 2024,
            "start": "2024-01-15T18:30:00Z",
            "homeTeam": {
                "goals": 1
            },
            "awayTeam": {
                "goals": 0
            },
            "serie": "runkosarja"
        }"#;

        let game: ScheduleGame = serde_json::from_str(json).unwrap();

        // Test default values
        assert_eq!(game.end, None);
        assert!(!game.started);
        assert!(!game.ended);
        assert_eq!(game.game_time, 0);
        assert_eq!(game.finished_type, None);
    }

    #[test]
    fn test_schedule_api_game_serialization() {
        let api_game = ScheduleApiGame {
            id: 456,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            home_team_name: "HIFK".to_string(),
            away_team_name: "TPS".to_string(),
            serie: 1,
            finished_type: None,
            started: false,
            ended: false,
            game_time: Some(0),
        };

        let json = serde_json::to_string(&api_game).unwrap();
        assert!(json.contains("\"homeTeamName\":\"HIFK\""));
        assert!(json.contains("\"awayTeamName\":\"TPS\""));
        assert!(json.contains("\"serie\":1"));

        let deserialized: ScheduleApiGame = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.home_team_name, "HIFK");
        assert_eq!(deserialized.away_team_name, "TPS");
        assert_eq!(deserialized.serie, 1);
    }

    #[test]
    fn test_schedule_response_serialization() {
        let response = ScheduleResponse {
            games: vec![create_test_schedule_game()],
            previous_game_date: Some("2024-01-14".to_string()),
            next_game_date: Some("2024-01-16".to_string()),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"previousGameDate\":\"2024-01-14\""));
        assert!(json.contains("\"nextGameDate\":\"2024-01-16\""));

        let deserialized: ScheduleResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.games.len(), 1);
        assert_eq!(
            deserialized.previous_game_date,
            Some("2024-01-14".to_string())
        );
        assert_eq!(deserialized.next_game_date, Some("2024-01-16".to_string()));
    }
}
