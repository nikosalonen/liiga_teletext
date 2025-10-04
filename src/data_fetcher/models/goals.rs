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
    /// Gets the display string for goal types with safe fallbacks for missing data
    /// Ensures rendering continues even with problematic goal type data (Requirement 4.1)
    pub fn get_goal_type_display(&self) -> String {
        // Handle missing or empty goal types safely
        if self.goal_types.is_empty() {
            return String::new();
        }

        let mut indicators = Vec::new();
        let valid_goal_types = ["EV", "YV", "YV2", "IM", "VT", "AV", "TM", "VL", "MV", "RV"];

        // Process goal types in fixed priority order to maintain consistent display
        // This preserves the original behavior while adding safe fallbacks

        // First, validate and clean the goal types
        let mut valid_goal_types_set = std::collections::HashSet::new();
        for goal_type in &self.goal_types {
            let goal_type_str = goal_type.trim();
            if !goal_type_str.is_empty() && valid_goal_types.contains(&goal_type_str) {
                valid_goal_types_set.insert(goal_type_str);
            } else if !goal_type_str.is_empty() {
                tracing::debug!(
                    "Invalid goal type '{}' found, excluding from display",
                    goal_type_str
                );
            }
        }

        // Add goal types in fixed priority order (maintains original behavior)
        if valid_goal_types_set.contains("YV") {
            indicators.push("YV");
        }
        if valid_goal_types_set.contains("YV2") {
            indicators.push("YV2");
        }
        if valid_goal_types_set.contains("IM") {
            indicators.push("IM");
        }
        if valid_goal_types_set.contains("VT") {
            indicators.push("VT");
        }
        if valid_goal_types_set.contains("AV") {
            indicators.push("AV");
        }
        if valid_goal_types_set.contains("TM") {
            indicators.push("TM");
        }
        if valid_goal_types_set.contains("VL") {
            indicators.push("VL");
        }
        if valid_goal_types_set.contains("MV") {
            indicators.push("MV");
        }
        if valid_goal_types_set.contains("RV") {
            indicators.push("RV");
        }
        // EV (Even strength) is the default, only show if no other types
        if valid_goal_types_set.contains("EV") && self.goal_types.len() == 1 {
            indicators.push("EV");
        }

        // Join with space separator, ensuring safe string operations
        let result = indicators.join(" ");

        // Validate result length to prevent layout issues
        if result.len() > 20 {
            // Reasonable maximum for goal type display
            tracing::warn!(
                "Goal type display '{}' is unusually long ({}), may cause layout issues",
                result,
                result.len()
            );
        }

        result
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
    fn test_goal_type_display_safe_fallbacks() {
        // Test invalid goal types are filtered out
        let invalid_types = GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Player".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec![
                "INVALID".to_string(),
                "YV".to_string(),
                "BADTYPE".to_string(),
            ],
            is_home_team: true,
            video_clip_url: None,
        };
        assert_eq!(invalid_types.get_goal_type_display(), "YV");

        // Test empty strings in goal types are handled
        let empty_strings = GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Player".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["".to_string(), "IM".to_string(), "   ".to_string()],
            is_home_team: true,
            video_clip_url: None,
        };
        assert_eq!(empty_strings.get_goal_type_display(), "IM");

        // Test duplicate goal types are removed
        let duplicates = GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Player".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["YV".to_string(), "IM".to_string(), "YV".to_string()],
            is_home_team: true,
            video_clip_url: None,
        };
        assert_eq!(duplicates.get_goal_type_display(), "YV IM");

        // Test EV (even strength) is only shown when it's the only type
        let ev_only = GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Player".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["EV".to_string()],
            is_home_team: true,
            video_clip_url: None,
        };
        assert_eq!(ev_only.get_goal_type_display(), "EV");

        // Test EV is not shown when other types are present
        let ev_with_others = GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Player".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["EV".to_string(), "YV".to_string()],
            is_home_team: true,
            video_clip_url: None,
        };
        assert_eq!(ev_with_others.get_goal_type_display(), "YV");
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
