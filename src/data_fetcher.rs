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
    pub teamId: String,
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
    pub tournament: String,
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

    let home_team = game.home_team();
    let away_team = game.away_team();

    for goal in home_team.goal_events() {
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
                .unwrap_or_else(|| goal.scorer_player_id.to_string()),
            minute: goal.game_time / 60,
            home_team_score: goal.home_team_score,
            away_team_score: goal.away_team_score,
            is_winning_goal: goal.winning_goal,
            goal_types: goal.goal_types.clone(),
            is_home_team: true,
        });
    }

    for goal in away_team.goal_events() {
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
                .unwrap_or_else(|| goal.scorer_player_id.to_string()),
            minute: goal.game_time / 60,
            home_team_score: goal.home_team_score,
            away_team_score: goal.away_team_score,
            is_winning_goal: goal.winning_goal,
            goal_types: goal.goal_types.clone(),
            is_home_team: false,
        });
    }

    events
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
    // let mut date = Local::now().format("%Y-%m-%d").to_string();
    let mut date = "2025-01-11";
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
        println!("Fetching from URL: {}", url);

        match client.get(&url).send().await {
            Ok(response) => {
                println!("Response status: {}", response.status());
                match response.text().await {
                    Ok(response_text) => {
                        match serde_json::from_str::<ScheduleResponse>(&response_text) {
                            Ok(response) => {
                                println!(
                                    "Found {} games for tournament {}",
                                    response.games.len(),
                                    tournament
                                );
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
        println!(
            "No games today, fetching games from nearest previous game date: {}",
            nearest_date
        );

        let mut prev_day_response: Option<ScheduleResponse> = None;

        for tournament in &tournaments {
            let url = format!(
                "{}/games?tournament={}&date={}",
                config.api_domain, tournament, nearest_date
            );
            println!("Fetching from URL: {}", url);

            match client.get(&url).send().await {
                Ok(response) => match response.text().await {
                    Ok(response_text) => {
                        match serde_json::from_str::<ScheduleResponse>(&response_text) {
                            Ok(response) => {
                                println!(
                                    "Found {} games for tournament {}",
                                    response.games.len(),
                                    tournament
                                );
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

                let goal_events = if !m.started {
                    Vec::new()
                } else if !m.home_team.goal_events.is_empty() || !m.away_team.goal_events.is_empty()
                {
                    // Only fetch detailed game data if there are goals and game has started
                    match fetch_game_data(&client, &config, m.season, m.id).await {
                        Ok(detailed_data) => detailed_data.goal_events,
                        Err(e) => {
                            eprintln!("Failed to fetch detailed game data: {}", e);
                            process_goal_events(&m, &HashMap::new())
                        }
                    }
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
                    tournament: m.serie,
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

async fn fetch_game_data(
    client: &Client,
    config: &Config,
    season: i32,
    game_id: i32,
) -> Result<GameData, Box<dyn Error>> {
    let url = format!("{}/games/{}/{}", config.api_domain, season, game_id);
    println!("Fetching detailed game data from: {}", url);
    let response = client.get(&url).send().await?;
    let game_response = response.json::<DetailedGameResponse>().await?;

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

    let goal_events = process_goal_events(&game_response.game, &player_names);

    // Debug goal events
    for event in &goal_events {
        println!(
            "Goal at minute {}: Player ID {} -> Name '{}' (found in map: {})",
            event.minute,
            event.scorer_player_id,
            event.scorer_name,
            player_names.contains_key(&event.scorer_player_id)
        );
    }

    let game_time = game_response.game.game_time;
    let is_overtime = game_time > 3600;
    let is_shootout = game_response.game.finished_type.as_deref() == Some("ENDED_IN_SHOOTOUT");

    Ok(GameData {
        home_team: game_response.game.home_team.team_name,
        away_team: game_response.game.away_team.team_name,
        time: format!("{}:00", game_time / 60),
        result: format!(
            "{}-{}",
            game_response.game.home_team.goals, game_response.game.away_team.goals
        ),
        score_type: if !game_response.game.started {
            ScoreType::Scheduled
        } else if !game_response.game.ended {
            ScoreType::Ongoing
        } else {
            ScoreType::Final
        },
        is_overtime,
        is_shootout,
        tournament: game_response.game.serie,
        goal_events,
        finished_type: game_response.game.finished_type.unwrap_or_default(),
    })
}

fn format_time(timestamp: &str) -> Result<String, Box<dyn Error>> {
    let utc_time = timestamp.parse::<DateTime<Utc>>()?;
    let local_time = utc_time.with_timezone(&Local);
    Ok(local_time.format("%H.%M").to_string())
}
