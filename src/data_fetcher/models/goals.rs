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
    fn test_goal_event_data_goal_type_display() {
        // Test multiple goal types
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

        // Test single goal type
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
}
