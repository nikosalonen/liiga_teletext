use crate::teletext_ui::ScoreType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct ScheduleTeam {
    #[serde(rename = "teamId")]
    pub team_id: Option<String>,
    #[serde(rename = "teamPlaceholder")]
    pub team_placeholder: Option<String>,
    #[serde(rename = "teamName")]
    pub team_name: Option<String>,
    pub goals: i32,
    #[serde(rename = "timeOut", default)]
    pub time_out: Option<String>,
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

#[derive(Debug, Deserialize, Serialize)]
pub struct Player {
    pub id: i64,
    #[serde(rename = "lastName")]
    pub last_name: String,
    #[serde(rename = "firstName")]
    pub first_name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DetailedGameResponse {
    pub game: DetailedGame,
    pub awards: Vec<serde_json::Value>,
    #[serde(rename = "homeTeamPlayers")]
    pub home_team_players: Vec<Player>,
    #[serde(rename = "awayTeamPlayers")]
    pub away_team_players: Vec<Player>,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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
        if self.goal_types.contains(&"IM".to_string()) {
            indicators.push("IM");
        }
        if self.goal_types.contains(&"VT".to_string()) {
            indicators.push("VT");
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
