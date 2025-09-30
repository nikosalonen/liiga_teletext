use super::goals::GoalEvent;
use super::players::Player;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Period {
    pub index: i32,
    #[serde(rename = "homeTeamGoals")]
    pub home_team_goals: i32,
    #[serde(rename = "awayTeamGoals")]
    pub away_team_goals: i32,
    pub category: String,
    #[serde(rename = "startTime")]
    pub start_time: i32,
    #[serde(rename = "endTime")]
    pub end_time: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PenaltyEvent {
    #[serde(rename = "playerId")]
    pub player_id: i32,
    #[serde(rename = "suffererPlayerId")]
    pub sufferer_player_id: i32,
    #[serde(rename = "logTime")]
    pub log_time: String,
    #[serde(rename = "gameTime")]
    pub game_time: i32,
    pub period: i32,
    #[serde(rename = "penaltyBegintime")]
    pub penalty_begintime: i32,
    #[serde(rename = "penaltyEndtime")]
    pub penalty_endtime: i32,
    #[serde(rename = "penaltyFaultName")]
    pub penalty_fault_name: String,
    #[serde(rename = "penaltyFaultType")]
    pub penalty_fault_type: String,
    #[serde(rename = "penaltyMinutes")]
    pub penalty_minutes: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedTeam {
    #[serde(rename = "teamId")]
    pub team_id: String,
    #[serde(rename = "teamName")]
    pub team_name: String,
    pub goals: i32,
    #[serde(rename = "goalEvents")]
    pub goal_events: Vec<GoalEvent>,
    #[serde(rename = "penaltyEvents")]
    pub penalty_events: Vec<PenaltyEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedGame {
    pub id: i32,
    pub season: i32,
    pub start: String,
    #[serde(default)]
    pub end: Option<String>,
    #[serde(rename = "homeTeam")]
    pub home_team: DetailedTeam,
    #[serde(rename = "awayTeam")]
    pub away_team: DetailedTeam,
    pub periods: Vec<Period>,
    #[serde(rename = "finishedType")]
    pub finished_type: Option<String>,
    pub started: bool,
    pub ended: bool,
    #[serde(rename = "gameTime")]
    pub game_time: i32,
    pub serie: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DetailedGameResponse {
    pub game: DetailedGame,
    pub awards: Vec<serde_json::Value>,
    #[serde(rename = "homeTeamPlayers")]
    pub home_team_players: Vec<Player>,
    #[serde(rename = "awayTeamPlayers")]
    pub away_team_players: Vec<Player>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::models::goals::{EmbeddedPlayer, GoalEvent};
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

    fn create_test_penalty_event() -> PenaltyEvent {
        PenaltyEvent {
            player_id: 11111,
            sufferer_player_id: 22222,
            log_time: "19:15:30".to_string(),
            game_time: 1155,
            period: 1,
            penalty_begintime: 1155,
            penalty_endtime: 1275,
            penalty_fault_name: "Cross-checking".to_string(),
            penalty_fault_type: "Minor".to_string(),
            penalty_minutes: 2,
        }
    }

    fn create_test_period() -> Period {
        Period {
            index: 1,
            home_team_goals: 1,
            away_team_goals: 0,
            category: "REGULAR".to_string(),
            start_time: 0,
            end_time: 1200,
        }
    }

    fn create_test_detailed_team() -> DetailedTeam {
        DetailedTeam {
            team_id: "HIFK".to_string(),
            team_name: "HIFK Helsinki".to_string(),
            goals: 2,
            goal_events: vec![create_test_goal_event()],
            penalty_events: vec![create_test_penalty_event()],
        }
    }

    fn create_test_detailed_game() -> DetailedGame {
        DetailedGame {
            id: 54321,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            end: Some("2024-01-15T21:00:00Z".to_string()),
            home_team: create_test_detailed_team(),
            away_team: create_test_detailed_team(),
            periods: vec![create_test_period()],
            finished_type: Some("ENDED_DURING_REGULAR_TIME".to_string()),
            started: true,
            ended: true,
            game_time: 3600,
            serie: "runkosarja".to_string(),
        }
    }

    #[test]
    fn test_period_serialization() {
        let period = create_test_period();

        // Test serialization
        let json = serde_json::to_string(&period).unwrap();
        assert!(json.contains("\"index\":1"));
        assert!(json.contains("\"homeTeamGoals\":1"));
        assert!(json.contains("\"awayTeamGoals\":0"));

        // Test deserialization
        let deserialized: Period = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.index, 1);
        assert_eq!(deserialized.home_team_goals, 1);
        assert_eq!(deserialized.away_team_goals, 0);
        assert_eq!(deserialized.category, "REGULAR");
    }

    #[test]
    fn test_penalty_event_serialization() {
        let penalty = create_test_penalty_event();

        // Test serialization
        let json = serde_json::to_string(&penalty).unwrap();
        assert!(json.contains("\"playerId\":11111"));
        assert!(json.contains("\"penaltyFaultName\":\"Cross-checking\""));
        assert!(json.contains("\"penaltyMinutes\":2"));

        // Test deserialization
        let deserialized: PenaltyEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.player_id, 11111);
        assert_eq!(deserialized.penalty_fault_name, "Cross-checking");
        assert_eq!(deserialized.penalty_minutes, 2);
    }

    #[test]
    fn test_detailed_team_serialization() {
        let team = create_test_detailed_team();

        // Test serialization
        let json = serde_json::to_string(&team).unwrap();
        assert!(json.contains("\"teamId\":\"HIFK\""));
        assert!(json.contains("\"teamName\":\"HIFK Helsinki\""));
        assert!(json.contains("\"goalEvents\""));
        assert!(json.contains("\"penaltyEvents\""));

        // Test deserialization
        let deserialized: DetailedTeam = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.team_id, "HIFK");
        assert_eq!(deserialized.team_name, "HIFK Helsinki");
        assert_eq!(deserialized.goals, 2);
        assert_eq!(deserialized.goal_events.len(), 1);
        assert_eq!(deserialized.penalty_events.len(), 1);
    }

    #[test]
    fn test_detailed_game_serialization() {
        let game = create_test_detailed_game();

        // Test serialization
        let json = serde_json::to_string(&game).unwrap();
        assert!(json.contains("\"id\":54321"));
        assert!(json.contains("\"homeTeam\""));
        assert!(json.contains("\"awayTeam\""));
        assert!(json.contains("\"periods\""));

        // Test deserialization
        let deserialized: DetailedGame = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, 54321);
        assert_eq!(deserialized.season, 2024);
        assert_eq!(deserialized.home_team.team_id, "HIFK");
        assert_eq!(deserialized.periods.len(), 1);
    }

    #[test]
    fn test_detailed_game_default_fields() {
        let json = r#"{
            "id": 123,
            "season": 2024,
            "start": "2024-01-15T18:30:00Z",
            "homeTeam": {
                "teamId": "HIFK",
                "teamName": "HIFK Helsinki",
                "goals": 1,
                "goalEvents": [],
                "penaltyEvents": []
            },
            "awayTeam": {
                "teamId": "TPS",
                "teamName": "TPS Turku",
                "goals": 0,
                "goalEvents": [],
                "penaltyEvents": []
            },
            "periods": [],
            "started": true,
            "ended": false,
            "gameTime": 1200,
            "serie": "runkosarja"
        }"#;

        let game: DetailedGame = serde_json::from_str(json).unwrap();

        // Test default values
        assert_eq!(game.end, None);
        assert_eq!(game.finished_type, None);
        assert!(game.started);
        assert!(!game.ended);
    }

    #[test]
    fn test_detailed_game_response_serialization() {
        let player = Player {
            id: 12345,
            last_name: "Koivu".to_string(),
            first_name: "Mikko".to_string(),
        };

        let response = DetailedGameResponse {
            game: create_test_detailed_game(),
            awards: vec![],
            home_team_players: vec![player.clone()],
            away_team_players: vec![player],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"homeTeamPlayers\""));
        assert!(json.contains("\"awayTeamPlayers\""));
        assert!(json.contains("\"game\""));

        let deserialized: DetailedGameResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.home_team_players.len(), 1);
        assert_eq!(deserialized.away_team_players.len(), 1);
        assert_eq!(deserialized.game.id, 54321);
    }
}
