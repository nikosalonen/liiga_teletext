use crate::config::Config;
use crate::teletext_ui::ScoreType;
use chrono::Local;
use chrono::{DateTime, Utc};
use futures;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalEvent {
    #[serde(rename = "scorerPlayerId")]
    pub scorer_player_id: i64,
    pub logTime: String,
    #[serde(rename = "gameTime")]
    pub game_time: i32,
    pub period: i32,
    pub eventId: i32,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleTeam {
    #[serde(rename = "teamId")]
    pub team_id: String,
    #[serde(rename = "teamName")]
    pub team_name: String,
    pub goals: i32,
    #[serde(rename = "goalEvents", default)]
    pub goal_events: Vec<GoalEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleGame {
    pub id: i32,
    pub season: i32,
    pub start: String,
    #[serde(default)]
    pub end: String,
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

#[derive(Debug, Deserialize, Serialize)]
struct ScheduleResponse {
    games: Vec<ScheduleGame>,
    previousGameDate: String,
    nextGameDate: String,
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
    pub logTime: String,
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
    pub teamId: String,
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
    pub end: String,
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
    pub lastName: String,
    pub firstName: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct DetailedGameResponse {
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
    pub finished_type: String,
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
}

fn process_goal_events<T>(game: &T, player_names: &HashMap<i64, String>) -> Vec<GoalEventData>
where
    T: HasTeams,
{
    let mut events = Vec::new();

    // Process home team goals
    process_team_goals(game.home_team(), player_names, true, &mut events);
    // Process away team goals
    process_team_goals(game.away_team(), player_names, false, &mut events);

    events
}

fn process_team_goals(
    team: &dyn HasGoalEvents,
    player_names: &HashMap<i64, String>,
    is_home_team: bool,
    events: &mut Vec<GoalEventData>,
) {
    for goal in team
        .goal_events()
        .iter()
        .filter(|g| !g.goal_types.contains(&"RL0".to_string()))
    {
        events.push(GoalEventData {
            scorer_player_id: goal.scorer_player_id,
            scorer_name: player_names
                .get(&goal.scorer_player_id)
                .map(|name| {
                    name.split_whitespace()
                        .last()
                        .unwrap_or("")
                        .chars()
                        .enumerate()
                        .map(|(i, c)| {
                            if i == 0 {
                                c.to_uppercase().next().unwrap_or(c)
                            } else {
                                c.to_lowercase().next().unwrap_or(c)
                            }
                        })
                        .collect::<String>()
                })
                .unwrap_or_else(|| format!("Pelaaja {}", goal.scorer_player_id)),
            minute: goal.game_time / 60,
            home_team_score: goal.home_team_score,
            away_team_score: goal.away_team_score,
            is_winning_goal: goal.winning_goal,
            goal_types: goal.goal_types.clone(),
            is_home_team,
        });
    }
}

trait HasTeams {
    fn home_team(&self) -> &dyn HasGoalEvents;
    fn away_team(&self) -> &dyn HasGoalEvents;
}

trait HasGoalEvents {
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

pub async fn fetch_liiga_data() -> Result<Vec<GameData>, Box<dyn Error>> {
    let config = Config::load()?;
    let client = Client::new();
    let date = Local::now().format("%Y-%m-%d").to_string();
    // let mut date = "2025-01-11";
    let tournaments = ["runkosarja", "playoffs", "playout", "qualifications"];
    let mut all_games = Vec::new();
    let mut response_data: Option<ScheduleResponse> = None;
    let mut found_games = false;
    let mut previous_dates = Vec::new();

    // Try to get games for today first
    for tournament in &tournaments {
        let url = format!(
            "{}/games?tournament={}&date={}",
            config.api_domain, tournament, date
        );

        match client.get(&url).send().await {
            Ok(response) => {
                match response.text().await {
                    Ok(response_text) => {
                        match serde_json::from_str::<ScheduleResponse>(&response_text) {
                            Ok(response) => {
                                if !response.games.is_empty() {
                                    response_data = Some(response);
                                    found_games = true;
                                    break;
                                }
                                // Store previous game date if it exists
                                if !response.previousGameDate.is_empty() {
                                    previous_dates.push(response.previousGameDate.clone());
                                }
                                // Only store the response if we haven't found any games yet and this is the last tournament
                                if response_data.is_none()
                                    && *tournament == tournaments[tournaments.len() - 1]
                                {
                                    response_data = Some(response);
                                }
                            }
                            Err(e) => eprintln!("Failed to parse JSON for {}: {}", tournament, e),
                        }
                    }
                    Err(e) => eprintln!("Failed to get response text for {}: {}", tournament, e),
                }
            }
            Err(e) => eprintln!("Failed to send request for {}: {}", tournament, e),
        }
    }

    // If no games found in any tournament today, try the nearest previous game date
    if !found_games && !previous_dates.is_empty() {
        // Sort dates in descending order to get the most recent one
        previous_dates.sort_by(|a, b| b.cmp(a));
        let nearest_date = &previous_dates[0];

        let mut prev_day_response: Option<ScheduleResponse> = None;

        for tournament in &tournaments {
            let url = format!(
                "{}/games?tournament={}&date={}",
                config.api_domain, tournament, nearest_date
            );

            match client.get(&url).send().await {
                Ok(response) => match response.text().await {
                    Ok(response_text) => {
                        match serde_json::from_str::<ScheduleResponse>(&response_text) {
                            Ok(response) => {
                                if !response.games.is_empty() {
                                    prev_day_response = Some(response);
                                    found_games = true;
                                    break;
                                }
                                // Only store the response if we haven't found any games yet and this is the last tournament
                                if prev_day_response.is_none()
                                    && *tournament == tournaments[tournaments.len() - 1]
                                {
                                    prev_day_response = Some(response);
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to parse JSON for {}: {}", tournament, e)
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to get response text for {}: {}", tournament, e)
                    }
                },
                Err(e) => eprintln!("Failed to send request for {}: {}", tournament, e),
            }
        }

        if found_games {
            response_data = prev_day_response;
        }
    }

    // Process games if we found any
    if let Some(response) = response_data {
        let games = futures::future::try_join_all(response.games.into_iter().map(|m| {
            let client = client.clone();
            let config = config.clone();
            async move {
                let time = format_time(&m.start)?;
                let result = format!("{}-{}", m.home_team.goals, m.away_team.goals);
                let is_overtime = matches!(
                    m.finished_type.as_deref(),
                    Some("ENDED_DURING_EXTENDED_GAME_TIME")
                );

                let is_shootout = matches!(
                    m.finished_type.as_deref(),
                    Some("ENDED_DURING_WINNING_SHOT_COMPETITION")
                );

                let score_type = if !m.started {
                    ScoreType::Scheduled
                } else if !m.ended {
                    ScoreType::Ongoing
                } else {
                    ScoreType::Final
                };

                let has_goals = m
                    .home_team
                    .goal_events
                    .iter()
                    .any(|g| !g.goal_types.contains(&"RL0".to_string()))
                    || m.away_team
                        .goal_events
                        .iter()
                        .any(|g| !g.goal_types.contains(&"RL0".to_string()));

                let goal_events = if !m.started {
                    Vec::new()
                } else if has_goals || !m.ended {
                    // Fetch detailed data if there are goals or game is ongoing
                    fetch_detailed_game_data(&client, &config, &m).await
                } else {
                    Vec::new()
                };

                Ok::<GameData, Box<dyn Error>>(GameData {
                    home_team: m.home_team.team_name,
                    away_team: m.away_team.team_name,
                    time,
                    result,
                    score_type,
                    is_overtime,
                    is_shootout,
                    serie: m.serie,
                    goal_events,
                    finished_type: m.finished_type.unwrap_or_default(),
                })
            }
        }))
        .await?;
        all_games.extend(games);
    }

    Ok(all_games)
}

async fn fetch_detailed_game_data(
    client: &Client,
    config: &Config,
    game: &ScheduleGame,
) -> Vec<GoalEventData> {
    match fetch_game_data(client, config, game.season, game.id).await {
        Ok(detailed_data) => detailed_data,
        Err(e) => {
            eprintln!(
                "Failed to fetch detailed game data: {}. Using basic game data.",
                e
            );
            create_basic_goal_events(game)
        }
    }
}

async fn fetch_game_data(
    client: &Client,
    config: &Config,
    season: i32,
    game_id: i32,
) -> Result<Vec<GoalEventData>, Box<dyn Error>> {
    let url = format!("{}/games/{}/{}", config.api_domain, season, game_id);
    let response = client.get(&url).send().await?;
    let response_text = response.text().await?;
    let game_response = serde_json::from_str::<DetailedGameResponse>(&response_text)?;

    let mut player_names: HashMap<i64, String> = HashMap::new();

    for player in &game_response.home_team_players {
        player_names.insert(
            player.id,
            format!("{} {}", player.firstName, player.lastName),
        );
    }
    for player in &game_response.away_team_players {
        player_names.insert(
            player.id,
            format!("{} {}", player.firstName, player.lastName),
        );
    }

    Ok(process_goal_events(&game_response.game, &player_names))
}

fn format_time(timestamp: &str) -> Result<String, Box<dyn Error>> {
    let utc_time = timestamp.parse::<DateTime<Utc>>()?;
    let local_time = utc_time.with_timezone(&Local);
    Ok(local_time.format("%H.%M").to_string())
}

fn create_basic_goal_events(game: &ScheduleGame) -> Vec<GoalEventData> {
    let mut basic_names = HashMap::new();
    for goal in &game.home_team.goal_events {
        basic_names.insert(
            goal.scorer_player_id,
            format!("Pelaaja {}", goal.scorer_player_id),
        );
    }
    for goal in &game.away_team.goal_events {
        basic_names.insert(
            goal.scorer_player_id,
            format!("Pelaaja {}", goal.scorer_player_id),
        );
    }
    process_goal_events(game, &basic_names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;

    #[test]
    fn test_format_time() {
        let timestamp = "2025-01-11T15:00:00Z";
        let result = format_time(timestamp).unwrap();
        // Note: This test assumes local timezone, might need adjustment
        assert!(result.contains(":"), "Time should contain :");
        assert_eq!(result.len(), 5, "Time should be in format HH.MM");
    }

    #[test]
    fn test_process_goal_events() {
        let game = ScheduleGame {
            id: 1,
            season: 2025,
            start: "2025-01-11T15:00:00Z".to_string(),
            end: "".to_string(),
            home_team: ScheduleTeam {
                team_id: "123".to_string(),
                team_name: "Home".to_string(),
                goals: 1,
                goal_events: vec![GoalEvent {
                    scorer_player_id: 123,
                    logTime: "2025-01-11T15:10:00Z".to_string(),
                    game_time: 600,
                    period: 1,
                    eventId: 1,
                    home_team_score: 1,
                    away_team_score: 0,
                    winning_goal: true,
                    goal_types: vec![],
                    assistant_player_ids: vec![],
                }],
            },
            away_team: ScheduleTeam {
                team_id: "456".to_string(),
                team_name: "Away".to_string(),
                goals: 0,
                goal_events: vec![],
            },
            finished_type: Some("ENDED_DURING_REGULAR_GAME_TIME".to_string()),
            started: true,
            ended: true,
            game_time: 3600,
            serie: "RUNKOSARJA".to_string(),
        };

        let mut player_names = HashMap::new();
        player_names.insert(123, "John Doe".to_string());

        let events = process_goal_events(&game, &player_names);

        assert_eq!(events.len(), 1, "Should process one goal event");
        let event = &events[0];
        assert_eq!(event.scorer_name, "Doe", "Should extract last name");
        assert_eq!(event.minute, 10, "Should convert game time to minutes");
        assert!(event.is_home_team, "Should be marked as home team goal");
    }

    #[test]
    fn test_create_basic_goal_events() {
        let game = ScheduleGame {
            id: 1,
            season: 2025,
            start: "2025-01-11T15:00:00Z".to_string(),
            end: "".to_string(),
            home_team: ScheduleTeam {
                team_id: "123".to_string(),
                team_name: "Home".to_string(),
                goals: 1,
                goal_events: vec![GoalEvent {
                    scorer_player_id: 123,
                    logTime: "2025-01-11T15:10:00Z".to_string(),
                    game_time: 600,
                    period: 1,
                    eventId: 1,
                    home_team_score: 1,
                    away_team_score: 0,
                    winning_goal: false,
                    goal_types: vec!["RL0".to_string()],
                    assistant_player_ids: vec![],
                }],
            },
            away_team: ScheduleTeam {
                team_id: "456".to_string(),
                team_name: "Away".to_string(),
                goals: 0,
                goal_events: vec![],
            },
            finished_type: None,
            started: true,
            ended: false,
            game_time: 600,
            serie: "RUNKOSARJA".to_string(),
        };

        let events = create_basic_goal_events(&game);
        assert!(events.is_empty(), "Should filter out RL0 goals");
    }

    #[test]
    fn test_process_team_goals() {
        let mut events = Vec::new();
        let team = ScheduleTeam {
            team_id: "123".to_string(),
            team_name: "Team".to_string(),
            goals: 2,
            goal_events: vec![
                GoalEvent {
                    scorer_player_id: 123,
                    logTime: "2025-01-11T15:10:00Z".to_string(),
                    game_time: 600,
                    period: 1,
                    eventId: 1,
                    home_team_score: 1,
                    away_team_score: 0,
                    winning_goal: false,
                    goal_types: vec![],
                    assistant_player_ids: vec![],
                },
                GoalEvent {
                    scorer_player_id: 456,
                    logTime: "2025-01-11T15:20:00Z".to_string(),
                    game_time: 1200,
                    period: 1,
                    eventId: 2,
                    home_team_score: 2,
                    away_team_score: 0,
                    winning_goal: false,
                    goal_types: vec!["RL0".to_string()],
                    assistant_player_ids: vec![],
                },
            ],
        };

        let mut player_names = HashMap::new();
        player_names.insert(123, "John Doe".to_string());
        player_names.insert(456, "Jane Smith".to_string());

        process_team_goals(&team, &player_names, true, &mut events);

        assert_eq!(events.len(), 1, "Should only process non-RL0 goals");
        assert_eq!(events[0].minute, 10, "Should convert time correctly");
        assert_eq!(events[0].scorer_name, "Doe", "Should format name correctly");
    }
}
