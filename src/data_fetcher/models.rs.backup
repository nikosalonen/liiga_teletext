use crate::teletext_ui::ScoreType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub struct GoalEvent {
    #[serde(rename = "scorerPlayerId")]
    pub scorer_player_id: i64,
    #[serde(rename = "logTime")]
    pub log_time: String,
    #[serde(rename = "gameTime")]
    pub game_time: i32,
    pub period: i32,
    #[serde(rename = "eventId")]
    pub event_id: i32,
    #[serde(rename = "homeTeamScore")]
    pub home_team_score: i32,
    #[serde(rename = "awayTeamScore")]
    pub away_team_score: i32,
    #[serde(rename = "winningGoal", default)]
    pub winning_goal: bool,
    #[serde(rename = "goalTypes", default)]
    pub goal_types: Vec<String>,
    #[serde(rename = "assistantPlayerIds", default)]
    pub assistant_player_ids: Vec<i32>,
    #[serde(rename = "videoClipUrl", default)]
    pub video_clip_url: Option<String>,
    #[serde(rename = "scorerPlayer", default)]
    pub scorer_player: Option<EmbeddedPlayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct EmbeddedPlayer {
    #[serde(rename = "playerId")]
    pub player_id: i64,
    #[serde(rename = "lastName")]
    pub last_name: String,
    #[serde(rename = "firstName")]
    pub first_name: String,
}

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
pub struct Player {
    pub id: i64,
    #[serde(rename = "lastName")]
    pub last_name: String,
    #[serde(rename = "firstName")]
    pub first_name: String,
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

#[derive(Debug, Clone, Hash)]
pub struct GameData {
    pub home_team: String,
    pub away_team: String,
    pub time: String,
    pub result: String,
    pub score_type: ScoreType,
    pub is_overtime: bool,
    pub is_shootout: bool,
    pub serie: String,
    pub goal_events: Vec<GoalEventData>,
    pub played_time: i32,
    pub start: String,
}

#[derive(Debug, Clone, Hash)]
pub struct GoalEventData {
    pub scorer_player_id: i64,
    pub scorer_name: String,
    pub minute: i32,
    pub home_team_score: i32,
    pub away_team_score: i32,
    pub is_winning_goal: bool,
    pub goal_types: Vec<String>,
    pub is_home_team: bool,
    pub video_clip_url: Option<String>,
}

impl GoalEventData {
    pub fn get_goal_type_display(&self) -> String {
        let mut indicators = Vec::new();
        if self.goal_types.contains(&"YV".to_string()) {
            indicators.push("YV");
        }
        if self.goal_types.contains(&"YV2".to_string()) {
            indicators.push("YV2");
        }
        if self.goal_types.contains(&"IM".to_string()) {
            indicators.push("IM");
        }
        if self.goal_types.contains(&"VT".to_string()) {
            indicators.push("VT");
        }
        if self.goal_types.contains(&"AV".to_string()) {
            indicators.push("AV");
        }
        if self.goal_types.contains(&"TM".to_string()) {
            indicators.push("TM");
        }
        indicators.join(" ")
    }
}

pub trait HasTeams {
    fn home_team(&self) -> &dyn HasGoalEvents;
    fn away_team(&self) -> &dyn HasGoalEvents;
}

pub trait HasGoalEvents {
    fn goal_events(&self) -> &[GoalEvent];
}

impl HasTeams for ScheduleGame {
    fn home_team(&self) -> &dyn HasGoalEvents {
        &self.home_team
    }
    fn away_team(&self) -> &dyn HasGoalEvents {
        &self.away_team
    }
}

impl HasTeams for DetailedGame {
    fn home_team(&self) -> &dyn HasGoalEvents {
        &self.home_team
    }
    fn away_team(&self) -> &dyn HasGoalEvents {
        &self.away_team
    }
}

impl HasGoalEvents for ScheduleTeam {
    fn goal_events(&self) -> &[GoalEvent] {
        &self.goal_events
    }
}

impl HasGoalEvents for DetailedTeam {
    fn goal_events(&self) -> &[GoalEvent] {
        &self.goal_events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_goal_event_serialization() {
        let goal_event = create_test_goal_event();

        // Test serialization
        let json = serde_json::to_string(&goal_event).unwrap();
        assert!(json.contains("\"scorerPlayerId\":12345"));
        assert!(json.contains("\"logTime\":\"18:30:00\""));
        assert!(json.contains("\"gameTime\":900"));
        assert!(json.contains("\"goalTypes\":[\"EV\"]"));

        // Test deserialization
        let deserialized: GoalEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.scorer_player_id, 12345);
        assert_eq!(deserialized.log_time, "18:30:00");
        assert_eq!(deserialized.game_time, 900);
        assert_eq!(deserialized.goal_types, vec!["EV"]);
        assert_eq!(deserialized.assistant_player_ids, vec![67890, 11111]);
        assert_eq!(
            deserialized.video_clip_url,
            Some("https://example.com/video.mp4".to_string())
        );
    }

    #[test]
    fn test_goal_event_default_fields() {
        let json = r#"{
            "scorerPlayerId": 123,
            "logTime": "12:00:00",
            "gameTime": 600,
            "period": 2,
            "eventId": 1,
            "homeTeamScore": 1,
            "awayTeamScore": 1
        }"#;

        let goal_event: GoalEvent = serde_json::from_str(json).unwrap();

        // Test default values
        assert!(!goal_event.winning_goal);
        assert!(goal_event.goal_types.is_empty());
        assert!(goal_event.assistant_player_ids.is_empty());
        assert_eq!(goal_event.video_clip_url, None);
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
    fn test_schedule_team_default() {
        let team = ScheduleTeam::default();

        assert_eq!(team.team_id, None);
        assert_eq!(team.team_placeholder, None);
        assert_eq!(team.team_name, None);
        assert_eq!(team.goals, 0);
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
    fn test_schedule_game_serialization() {
        let game = create_test_schedule_game();

        // Test serialization
        let json = serde_json::to_string(&game).unwrap();
        assert!(json.contains("\"id\":12345"));
        assert!(json.contains("\"season\":2024"));
        assert!(json.contains("\"start\":\"2024-01-15T18:30:00Z\""));
        assert!(json.contains("\"homeTeam\":"));
        assert!(json.contains("\"awayTeam\":"));

        // Test deserialization
        let deserialized: ScheduleGame = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, 12345);
        assert_eq!(deserialized.season, 2024);
        assert_eq!(deserialized.start, "2024-01-15T18:30:00Z");
        assert_eq!(deserialized.end, Some("2024-01-15T21:00:00Z".to_string()));
        assert!(deserialized.started);
        assert!(deserialized.ended);
    }

    #[test]
    fn test_schedule_game_default_fields() {
        let json = r#"{
            "id": 123,
            "season": 2024,
            "start": "2024-01-15T18:30:00Z",
            "homeTeam": {"goals": 0},
            "awayTeam": {"goals": 0},
            "serie": "runkosarja"
        }"#;

        let game: ScheduleGame = serde_json::from_str(json).unwrap();

        // Test default values
        assert_eq!(game.end, None);
        assert_eq!(game.finished_type, None);
        assert!(!game.started);
        assert!(!game.ended);
        assert_eq!(game.game_time, 0);
    }

    #[test]
    fn test_schedule_api_game_serialization() {
        let api_game = ScheduleApiGame {
            id: 123,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            home_team_name: "HIFK".to_string(),
            away_team_name: "Tappara".to_string(),
            serie: 1, // Integer in schedule API
            finished_type: Some("ENDED_DURING_REGULAR_TIME".to_string()),
            started: true,
            ended: true,
            game_time: Some(3600),
        };

        // Test serialization
        let json = serde_json::to_string(&api_game).unwrap();
        assert!(json.contains("\"homeTeamName\":\"HIFK\""));
        assert!(json.contains("\"awayTeamName\":\"Tappara\""));
        assert!(json.contains("\"serie\":1"));
        assert!(json.contains("\"gameTime\":3600"));

        // Test deserialization
        let deserialized: ScheduleApiGame = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.home_team_name, "HIFK");
        assert_eq!(deserialized.away_team_name, "Tappara");
        assert_eq!(deserialized.serie, 1);
        assert_eq!(deserialized.game_time, Some(3600));
    }

    #[test]
    fn test_schedule_response_serialization() {
        let schedule_response = ScheduleResponse {
            games: vec![create_test_schedule_game()],
            previous_game_date: Some("2024-01-14".to_string()),
            next_game_date: Some("2024-01-16".to_string()),
        };

        // Test serialization
        let json = serde_json::to_string(&schedule_response).unwrap();
        assert!(json.contains("\"games\":["));
        assert!(json.contains("\"previousGameDate\":\"2024-01-14\""));
        assert!(json.contains("\"nextGameDate\":\"2024-01-16\""));

        // Test deserialization
        let deserialized: ScheduleResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.games.len(), 1);
        assert_eq!(
            deserialized.previous_game_date,
            Some("2024-01-14".to_string())
        );
        assert_eq!(deserialized.next_game_date, Some("2024-01-16".to_string()));
    }

    #[test]
    fn test_period_serialization() {
        let period = Period {
            index: 1,
            home_team_goals: 2,
            away_team_goals: 1,
            category: "REGULAR".to_string(),
            start_time: 0,
            end_time: 1200,
        };

        // Test serialization
        let json = serde_json::to_string(&period).unwrap();
        assert!(json.contains("\"index\":1"));
        assert!(json.contains("\"homeTeamGoals\":2"));
        assert!(json.contains("\"awayTeamGoals\":1"));
        assert!(json.contains("\"startTime\":0"));
        assert!(json.contains("\"endTime\":1200"));

        // Test deserialization
        let deserialized: Period = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.index, 1);
        assert_eq!(deserialized.home_team_goals, 2);
        assert_eq!(deserialized.away_team_goals, 1);
        assert_eq!(deserialized.category, "REGULAR");
    }

    #[test]
    fn test_penalty_event_serialization() {
        let penalty = PenaltyEvent {
            player_id: 123,
            sufferer_player_id: 456,
            log_time: "12:30:00".to_string(),
            game_time: 750,
            period: 2,
            penalty_begintime: 750,
            penalty_endtime: 870,
            penalty_fault_name: "Tripping".to_string(),
            penalty_fault_type: "Minor".to_string(),
            penalty_minutes: 2,
        };

        // Test serialization
        let json = serde_json::to_string(&penalty).unwrap();
        assert!(json.contains("\"playerId\":123"));
        assert!(json.contains("\"suffererPlayerId\":456"));
        assert!(json.contains("\"penaltyFaultName\":\"Tripping\""));
        assert!(json.contains("\"penaltyMinutes\":2"));

        // Test deserialization
        let deserialized: PenaltyEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.player_id, 123);
        assert_eq!(deserialized.sufferer_player_id, 456);
        assert_eq!(deserialized.penalty_fault_name, "Tripping");
        assert_eq!(deserialized.penalty_minutes, 2);
    }

    #[test]
    fn test_detailed_team_serialization() {
        let detailed_team = DetailedTeam {
            team_id: "HIFK".to_string(),
            team_name: "HIFK Helsinki".to_string(),
            goals: 3,
            goal_events: vec![create_test_goal_event()],
            penalty_events: vec![],
        };

        // Test serialization
        let json = serde_json::to_string(&detailed_team).unwrap();
        assert!(json.contains("\"teamId\":\"HIFK\""));
        assert!(json.contains("\"teamName\":\"HIFK Helsinki\""));
        assert!(json.contains("\"goals\":3"));
        assert!(json.contains("\"goalEvents\":["));
        assert!(json.contains("\"penaltyEvents\":[]"));

        // Test deserialization
        let deserialized: DetailedTeam = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.team_id, "HIFK");
        assert_eq!(deserialized.team_name, "HIFK Helsinki");
        assert_eq!(deserialized.goals, 3);
        assert_eq!(deserialized.goal_events.len(), 1);
        assert!(deserialized.penalty_events.is_empty());
    }

    #[test]
    fn test_detailed_game_serialization() {
        let detailed_game = DetailedGame {
            id: 123,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            end: Some("2024-01-15T21:00:00Z".to_string()),
            home_team: DetailedTeam {
                team_id: "HIFK".to_string(),
                team_name: "HIFK Helsinki".to_string(),
                goals: 3,
                goal_events: vec![],
                penalty_events: vec![],
            },
            away_team: DetailedTeam {
                team_id: "TPS".to_string(),
                team_name: "TPS Turku".to_string(),
                goals: 2,
                goal_events: vec![],
                penalty_events: vec![],
            },
            periods: vec![],
            finished_type: Some("ENDED_DURING_REGULAR_TIME".to_string()),
            started: true,
            ended: true,
            game_time: 3600,
            serie: "runkosarja".to_string(),
        };

        // Test serialization
        let json = serde_json::to_string(&detailed_game).unwrap();
        assert!(json.contains("\"id\":123"));
        assert!(json.contains("\"homeTeam\":"));
        assert!(json.contains("\"awayTeam\":"));
        assert!(json.contains("\"periods\":[]"));

        // Test deserialization
        let deserialized: DetailedGame = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, 123);
        assert_eq!(deserialized.home_team.team_name, "HIFK Helsinki");
        assert_eq!(deserialized.away_team.team_name, "TPS Turku");
    }

    #[test]
    fn test_player_serialization() {
        let player = Player {
            id: 12345,
            last_name: "Koivu".to_string(),
            first_name: "Mikko".to_string(),
        };

        // Test serialization
        let json = serde_json::to_string(&player).unwrap();
        assert!(json.contains("\"id\":12345"));
        assert!(json.contains("\"lastName\":\"Koivu\""));
        assert!(json.contains("\"firstName\":\"Mikko\""));

        // Test deserialization
        let deserialized: Player = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, 12345);
        assert_eq!(deserialized.last_name, "Koivu");
        assert_eq!(deserialized.first_name, "Mikko");
    }

    #[test]
    fn test_detailed_game_response_serialization() {
        let response = DetailedGameResponse {
            game: DetailedGame {
                id: 123,
                season: 2024,
                start: "2024-01-15T18:30:00Z".to_string(),
                end: None,
                home_team: DetailedTeam {
                    team_id: "HIFK".to_string(),
                    team_name: "HIFK Helsinki".to_string(),
                    goals: 0,
                    goal_events: vec![],
                    penalty_events: vec![],
                },
                away_team: DetailedTeam {
                    team_id: "TPS".to_string(),
                    team_name: "TPS Turku".to_string(),
                    goals: 0,
                    goal_events: vec![],
                    penalty_events: vec![],
                },
                periods: vec![],
                finished_type: None,
                started: false,
                ended: false,
                game_time: 0,
                serie: "runkosarja".to_string(),
            },
            awards: vec![],
            home_team_players: vec![Player {
                id: 123,
                last_name: "Koivu".to_string(),
                first_name: "Mikko".to_string(),
            }],
            away_team_players: vec![Player {
                id: 456,
                last_name: "Selänne".to_string(),
                first_name: "Teemu".to_string(),
            }],
        };

        // Test serialization
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"game\":"));
        assert!(json.contains("\"awards\":[]"));
        assert!(json.contains("\"homeTeamPlayers\":["));
        assert!(json.contains("\"awayTeamPlayers\":["));

        // Test deserialization
        let deserialized: DetailedGameResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.game.id, 123);
        assert_eq!(deserialized.home_team_players.len(), 1);
        assert_eq!(deserialized.away_team_players.len(), 1);
        assert_eq!(deserialized.home_team_players[0].first_name, "Mikko");
        assert_eq!(deserialized.away_team_players[0].first_name, "Teemu");
    }

    #[test]
    fn test_game_data_creation() {
        let game_data = GameData {
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

        assert_eq!(game_data.home_team, "HIFK");
        assert_eq!(game_data.away_team, "Tappara");
        assert_eq!(game_data.time, "18:30");
        assert_eq!(game_data.result, "3-2");
        assert!(matches!(game_data.score_type, ScoreType::Final));
        assert!(game_data.is_overtime);
        assert!(!game_data.is_shootout);
        assert_eq!(game_data.serie, "runkosarja");
        assert_eq!(game_data.played_time, 3900);
    }

    #[test]
    fn test_goal_event_data_creation() {
        let goal_event_data = GoalEventData {
            scorer_player_id: 12345,
            scorer_name: "Koivu".to_string(),
            minute: 15,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: true,
            goal_types: vec!["YV".to_string(), "MV".to_string()],
            is_home_team: true,
            video_clip_url: Some("https://example.com/video.mp4".to_string()),
        };

        assert_eq!(goal_event_data.scorer_player_id, 12345);
        assert_eq!(goal_event_data.scorer_name, "Koivu");
        assert_eq!(goal_event_data.minute, 15);
        assert_eq!(goal_event_data.home_team_score, 1);
        assert_eq!(goal_event_data.away_team_score, 0);
        assert!(goal_event_data.is_winning_goal);
        assert_eq!(goal_event_data.goal_types, vec!["YV", "MV"]);
        assert!(goal_event_data.is_home_team);
        assert_eq!(
            goal_event_data.video_clip_url,
            Some("https://example.com/video.mp4".to_string())
        );
    }

    #[test]
    fn test_goal_event_data_get_goal_type_display() {
        // Test single goal type
        let single_type = GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Player".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["YV".to_string()],
            is_home_team: true,
            video_clip_url: None,
        };
        assert_eq!(single_type.get_goal_type_display(), "YV");

        // Test multiple goal types (only YV, IM, VT are displayed)
        let multiple_types = GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Player".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["YV".to_string(), "IM".to_string(), "VT".to_string()],
            is_home_team: true,
            video_clip_url: None,
        };
        assert_eq!(multiple_types.get_goal_type_display(), "YV IM VT");

        // Test YV2 (2-man powerplay) goal type
        let yv2_type = GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Player".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["YV2".to_string()],
            is_home_team: true,
            video_clip_url: None,
        };
        assert_eq!(yv2_type.get_goal_type_display(), "YV2");

        // Test TM (empty-net) goal type
        let tm_type = GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Player".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["TM".to_string()],
            is_home_team: true,
            video_clip_url: None,
        };
        assert_eq!(tm_type.get_goal_type_display(), "TM");

        // Test combination including TM
        let combo_type = GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Player".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["YV".to_string(), "TM".to_string(), "VT".to_string()],
            is_home_team: true,
            video_clip_url: None,
        };
        assert_eq!(combo_type.get_goal_type_display(), "YV VT TM");

        // Test AV (short-handed) goal type
        let av_type = GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Player".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["AV".to_string()],
            is_home_team: true,
            video_clip_url: None,
        };
        assert_eq!(av_type.get_goal_type_display(), "AV");

        // Test empty goal types
        let no_types = GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Player".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec![],
            is_home_team: true,
            video_clip_url: None,
        };
        assert_eq!(no_types.get_goal_type_display(), "");
    }

    #[test]
    fn test_has_teams_trait_schedule_game() {
        let game = create_test_schedule_game();

        let home_team = game.home_team();
        let away_team = game.away_team();

        // Test that the trait returns the correct teams
        assert_eq!(home_team.goal_events().len(), 1);
        assert_eq!(away_team.goal_events().len(), 1);
    }

    #[test]
    fn test_has_teams_trait_detailed_game() {
        let detailed_game = DetailedGame {
            id: 123,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            end: None,
            home_team: DetailedTeam {
                team_id: "HIFK".to_string(),
                team_name: "HIFK Helsinki".to_string(),
                goals: 2,
                goal_events: vec![create_test_goal_event()],
                penalty_events: vec![],
            },
            away_team: DetailedTeam {
                team_id: "TPS".to_string(),
                team_name: "TPS Turku".to_string(),
                goals: 1,
                goal_events: vec![],
                penalty_events: vec![],
            },
            periods: vec![],
            finished_type: None,
            started: true,
            ended: false,
            game_time: 1200,
            serie: "runkosarja".to_string(),
        };

        let home_team = detailed_game.home_team();
        let away_team = detailed_game.away_team();

        assert_eq!(home_team.goal_events().len(), 1);
        assert_eq!(away_team.goal_events().len(), 0);
    }

    #[test]
    fn test_has_goal_events_trait_schedule_team() {
        let team = create_test_schedule_team();
        assert_eq!(team.goal_events().len(), 1);
        assert_eq!(team.goal_events()[0].scorer_player_id, 12345);
    }

    #[test]
    fn test_has_goal_events_trait_detailed_team() {
        let detailed_team = DetailedTeam {
            team_id: "HIFK".to_string(),
            team_name: "HIFK Helsinki".to_string(),
            goals: 1,
            goal_events: vec![create_test_goal_event()],
            penalty_events: vec![],
        };

        assert_eq!(detailed_team.goal_events().len(), 1);
        assert_eq!(detailed_team.goal_events()[0].scorer_player_id, 12345);
    }

    #[test]
    fn test_complex_goal_event_deserialization() {
        let json = r#"{
            "scorerPlayerId": 54321,
            "logTime": "14:23:45",
            "gameTime": 1463,
            "period": 2,
            "eventId": 7,
            "homeTeamScore": 2,
            "awayTeamScore": 1,
            "winningGoal": true,
            "goalTypes": ["YV", "MV", "RV"],
            "assistantPlayerIds": [11111, 22222, 33333],
            "videoClipUrl": "https://video.example.com/goal/54321.mp4"
        }"#;

        let goal_event: GoalEvent = serde_json::from_str(json).unwrap();

        assert_eq!(goal_event.scorer_player_id, 54321);
        assert_eq!(goal_event.log_time, "14:23:45");
        assert_eq!(goal_event.game_time, 1463);
        assert_eq!(goal_event.period, 2);
        assert_eq!(goal_event.event_id, 7);
        assert_eq!(goal_event.home_team_score, 2);
        assert_eq!(goal_event.away_team_score, 1);
        assert!(goal_event.winning_goal);
        assert_eq!(goal_event.goal_types, vec!["YV", "MV", "RV"]);
        assert_eq!(goal_event.assistant_player_ids, vec![11111, 22222, 33333]);
        assert_eq!(
            goal_event.video_clip_url,
            Some("https://video.example.com/goal/54321.mp4".to_string())
        );
    }

    #[test]
    fn test_clone_implementations() {
        let goal_event = create_test_goal_event();
        let cloned_goal = goal_event.clone();
        assert_eq!(goal_event.scorer_player_id, cloned_goal.scorer_player_id);
        assert_eq!(goal_event.log_time, cloned_goal.log_time);

        let team = create_test_schedule_team();
        let cloned_team = team.clone();
        assert_eq!(team.team_id, cloned_team.team_id);
        assert_eq!(team.goals, cloned_team.goals);

        let game = create_test_schedule_game();
        let cloned_game = game.clone();
        assert_eq!(game.id, cloned_game.id);
        assert_eq!(game.season, cloned_game.season);
    }

    #[test]
    fn test_debug_implementations() {
        let goal_event = create_test_goal_event();
        let debug_string = format!("{goal_event:?}");
        assert!(debug_string.contains("GoalEvent"));
        assert!(debug_string.contains("12345"));

        let team = create_test_schedule_team();
        let debug_string = format!("{team:?}");
        assert!(debug_string.contains("ScheduleTeam"));

        let game = create_test_schedule_game();
        let debug_string = format!("{game:?}");
        assert!(debug_string.contains("ScheduleGame"));
        assert!(debug_string.contains("12345"));
    }
}
