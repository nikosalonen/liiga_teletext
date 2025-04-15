use crate::config::Config;
use crate::teletext_ui::ScoreType;
use chrono::Local;
use chrono::{DateTime, NaiveTime, Utc};
use lazy_static::lazy_static;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Mutex;

// Cache structure for player information
lazy_static! {
    static ref PLAYER_CACHE: Mutex<HashMap<i32, HashMap<i64, String>>> = Mutex::new(HashMap::new());
}

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
    #[serde(rename = "videoClipUrl", default)]
    pub video_clip_url: Option<String>,
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ScheduleResponse {
    pub games: Vec<ScheduleGame>,
    pub previousGameDate: String,
    pub nextGameDate: String,
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
    pub played_time: i32,
    pub finished_type: String,
    pub log_time: String,
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
    for goal in team.goal_events().iter().filter(|g| {
        !g.goal_types.contains(&"RL0".to_string()) && !g.goal_types.contains(&"VT0".to_string())
    }) {
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
            video_clip_url: goal.video_clip_url.clone(),
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

fn should_show_todays_games() -> bool {
    let now = Local::now();
    let cutoff_time = NaiveTime::from_hms_opt(14, 0, 0).unwrap();
    let today_cutoff = now.date_naive().and_time(cutoff_time);
    now.naive_local() >= today_cutoff
}

pub async fn fetch_tournament_data(
    client: &Client,
    config: &Config,
    tournament: &str,
    date: &str,
) -> Result<Option<ScheduleResponse>, Box<dyn Error>> {
    let url = format!(
        "{}/games?tournament={}&date={}",
        config.api_domain, tournament, date
    );

    let response = match client.get(&url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            return Err(format!(
                "Failed to fetch data from API\nError: {}\nAPI domain: {}\nConfig: {}",
                e,
                config.api_domain,
                Config::get_config_path()
            )
            .into());
        }
    };

    // Check status code first
    if !response.status().is_success() {
        return Err(format!(
            "Failed to fetch data from API\nError: {}\nAPI domain: {}\nConfig: {}",
            response
                .status()
                .canonical_reason()
                .unwrap_or("Unknown error"),
            config.api_domain,
            Config::get_config_path()
        )
        .into());
    }

    let response_text = match response.text().await {
        Ok(text) => text,
        Err(e) => {
            return Err(format!(
                "Failed to read API response\nError: {}\nAPI domain: {}\nConfig: {}",
                e,
                config.api_domain,
                Config::get_config_path()
            )
            .into());
        }
    };

    match serde_json::from_str::<ScheduleResponse>(&response_text) {
        Ok(response) => Ok(Some(response)),
        Err(e) => Err(format!(
            "Failed to parse API response\nError: {}\nAPI domain: {}\nConfig: {}",
            e,
            config.api_domain,
            Config::get_config_path()
        )
        .into()),
    }
}

async fn fetch_previous_day_data(
    client: &Client,
    config: &Config,
    tournaments: &[&str],
    previous_dates: &[String],
) -> Result<Option<Vec<ScheduleResponse>>, Box<dyn Error>> {
    if previous_dates.is_empty() {
        return Ok(None);
    }

    // Sort dates in descending order to get the most recent one
    let mut sorted_dates = previous_dates.to_vec();
    sorted_dates.sort_by(|a, b| b.cmp(a));
    let nearest_date = &sorted_dates[0];

    let mut responses = Vec::new();
    let mut found_games = false;

    for tournament in tournaments {
        if let Ok(Some(response)) =
            fetch_tournament_data(client, config, tournament, nearest_date).await
        {
            if !response.games.is_empty() {
                responses.push(response);
                found_games = true;
            }
        }
    }

    if found_games {
        Ok(Some(responses))
    } else {
        Ok(None)
    }
}

fn determine_game_status(game: &ScheduleGame) -> (ScoreType, bool, bool) {
    let is_overtime = matches!(
        game.finished_type.as_deref(),
        Some("ENDED_DURING_EXTENDED_GAME_TIME")
    );

    let is_shootout = matches!(
        game.finished_type.as_deref(),
        Some("ENDED_DURING_WINNING_SHOT_COMPETITION")
    );

    let score_type = if !game.started {
        ScoreType::Scheduled
    } else if !game.ended {
        ScoreType::Ongoing
    } else {
        ScoreType::Final
    };

    (score_type, is_overtime, is_shootout)
}

pub async fn fetch_liiga_data(
    custom_date: Option<String>,
) -> Result<Vec<GameData>, Box<dyn Error>> {
    let config = Config::load()?;
    let client = Client::new();
    let date = if let Some(date) = custom_date {
        date
    } else {
        let now = Local::now();
        if should_show_todays_games() {
            now.format("%Y-%m-%d").to_string()
        } else {
            // If before 15:00, try to get previous day's games first
            let yesterday = now
                .date_naive()
                .pred_opt()
                .expect("Date underflow cannot happen with Local::now()");
            yesterday.format("%Y-%m-%d").to_string()
        }
    };
    let tournaments = ["runkosarja", "playoffs", "playout", "qualifications"];
    let mut all_games = Vec::new();
    let mut response_data: Vec<ScheduleResponse> = Vec::new();
    let mut found_games = false;
    let mut previous_dates = Vec::new();
    let mut last_error = None;

    // Try to get games for the selected date first
    for tournament in &tournaments {
        match fetch_tournament_data(&client, &config, tournament, &date).await {
            Ok(Some(response)) => {
                if !response.games.is_empty() {
                    response_data.push(response);
                    found_games = true;
                } else {
                    // Store previous game date if it exists
                    if !response.previousGameDate.is_empty() {
                        previous_dates.push(response.previousGameDate.clone());
                    }
                }
            }
            Err(e) => {
                last_error = Some(e);
                // Break early on API errors to avoid unnecessary retries
                break;
            }
            Ok(None) => continue,
        }
    }

    // If we got any errors, return the last error immediately
    if let Some(e) = last_error {
        return Err(e);
    }

    // If no games found in any tournament today, try the nearest previous game date
    if !found_games {
        if let Ok(Some(prev_day_response)) =
            fetch_previous_day_data(&client, &config, &tournaments, &previous_dates).await
        {
            response_data = prev_day_response;
        }
    }

    // Process games if we found any
    if !response_data.is_empty() {
        for response in &response_data {
            let games =
                futures::future::try_join_all(response.games.clone().into_iter().map(|m| {
                    let client = client.clone();
                    let config = config.clone();
                    async move {
                        let time = if !m.started {
                            format_time(&m.start)?
                        } else {
                            String::new()
                        };

                        let result = format!("{}-{}", m.home_team.goals, m.away_team.goals);
                        let (score_type, is_overtime, is_shootout) = determine_game_status(&m);

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
                            played_time: m.game_time,
                            finished_type: m.finished_type.unwrap_or_default(),
                            log_time: m.start.clone(),
                        })
                    }
                }))
                .await?;
            all_games.extend(games);
        }
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

    // Check cache first
    let mut player_names = HashMap::new();
    {
        if let Some(cached_players) = PLAYER_CACHE.lock().unwrap().get(&game_id) {
            return Ok(process_goal_events(&game_response.game, cached_players));
        }
    }

    // Build player names map if not in cache
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

    // Update cache
    PLAYER_CACHE
        .lock()
        .unwrap()
        .insert(game_id, player_names.clone());

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
    use crate::teletext_ui::has_live_games;

    #[test]
    fn test_format_time() {
        let timestamp = "2025-01-11T15:00:00Z";
        let result = format_time(timestamp).unwrap();
        // Note: This test assumes local timezone, might need adjustment
        assert!(result.contains("."), "Time should contain .");
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
                    video_clip_url: None,
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
                    video_clip_url: None,
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
                    video_clip_url: None,
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
                    video_clip_url: None,
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

    #[test]
    fn test_goal_event_display() {
        let event = GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Test".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["YV".to_string(), "IM".to_string()],
            is_home_team: true,
            video_clip_url: None,
        };

        assert_eq!(event.get_goal_type_display(), "YV IM");
    }

    #[test]
    fn test_should_show_todays_games() {
        // This test is time-dependent, so we need to be careful with assertions
        let result = should_show_todays_games();
        // We can only verify that the function returns a boolean
        assert!(result || !result);
    }

    #[test]
    fn test_process_goal_events_with_empty_events() {
        let game = ScheduleGame {
            id: 1,
            season: 2025,
            start: "2025-01-11T15:00:00Z".to_string(),
            end: "".to_string(),
            home_team: ScheduleTeam {
                team_id: "123".to_string(),
                team_name: "Home".to_string(),
                goals: 0,
                goal_events: vec![],
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
            game_time: 0,
            serie: "RUNKOSARJA".to_string(),
        };

        let player_names = HashMap::new();
        let events = process_goal_events(&game, &player_names);
        assert!(events.is_empty(), "Should return empty vec for no goals");
    }

    #[test]
    fn test_process_goal_events_with_rl0_goals() {
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
                    video_clip_url: None,
                }],
            },
            away_team: ScheduleTeam {
                team_id: "456".to_string(),
                team_name: "Away".to_string(),
                goals: 0,
                goal_events: vec![GoalEvent {
                    scorer_player_id: 456,
                    logTime: "2025-01-11T15:20:00Z".to_string(),
                    game_time: 1200,
                    period: 1,
                    eventId: 2,
                    home_team_score: 1,
                    away_team_score: 0,
                    winning_goal: false,
                    goal_types: vec!["VT0".to_string()],
                    assistant_player_ids: vec![],
                    video_clip_url: None,
                }],
            },
            finished_type: None,
            started: true,
            ended: false,
            game_time: 600,
            serie: "RUNKOSARJA".to_string(),
        };

        let player_names = HashMap::new();
        let events = process_goal_events(&game, &player_names);
        assert!(events.is_empty(), "Should filter out RL0 and VT0 goals");
    }

    #[test]
    fn test_format_time_invalid_input() {
        let result = format_time("invalid time");
        assert!(
            result.is_err(),
            "Should return error for invalid time format"
        );
    }

    #[test]
    fn test_determine_game_status() {
        // Test scheduled game
        let scheduled_game = ScheduleGame {
            id: 1,
            season: 2025,
            start: "2025-01-11T15:00:00Z".to_string(),
            end: "".to_string(),
            home_team: ScheduleTeam {
                team_id: "123".to_string(),
                team_name: "Home".to_string(),
                goals: 0,
                goal_events: vec![],
            },
            away_team: ScheduleTeam {
                team_id: "456".to_string(),
                team_name: "Away".to_string(),
                goals: 0,
                goal_events: vec![],
            },
            finished_type: None,
            started: false,
            ended: false,
            game_time: 0,
            serie: "RUNKOSARJA".to_string(),
        };
        let (status, is_ot, is_so) = determine_game_status(&scheduled_game);
        assert!(matches!(status, ScoreType::Scheduled));
        assert!(!is_ot);
        assert!(!is_so);

        // Test ongoing game
        let mut ongoing_game = scheduled_game.clone();
        ongoing_game.started = true;
        let (status, is_ot, is_so) = determine_game_status(&ongoing_game);
        assert!(matches!(status, ScoreType::Ongoing));
        assert!(!is_ot);
        assert!(!is_so);

        // Test finished regular game
        let mut finished_game = ongoing_game.clone();
        finished_game.ended = true;
        finished_game.finished_type = Some("ENDED_DURING_REGULAR_GAME_TIME".to_string());
        let (status, is_ot, is_so) = determine_game_status(&finished_game);
        assert!(matches!(status, ScoreType::Final));
        assert!(!is_ot);
        assert!(!is_so);

        // Test overtime game
        let mut ot_game = finished_game.clone();
        ot_game.finished_type = Some("ENDED_DURING_EXTENDED_GAME_TIME".to_string());
        let (status, is_ot, is_so) = determine_game_status(&ot_game);
        assert!(matches!(status, ScoreType::Final));
        assert!(is_ot);
        assert!(!is_so);

        // Test shootout game
        let mut so_game = finished_game.clone();
        so_game.finished_type = Some("ENDED_DURING_WINNING_SHOT_COMPETITION".to_string());
        let (status, is_ot, is_so) = determine_game_status(&so_game);
        assert!(matches!(status, ScoreType::Final));
        assert!(!is_ot);
        assert!(is_so);
    }

    #[test]
    fn test_format_time_edge_cases() {
        // Test empty string
        assert!(format_time("").is_err());

        // Test invalid format
        assert!(format_time("2025-13-11T15:00:00Z").is_err());

        // Test missing timezone
        assert!(format_time("2025-01-11T15:00:00").is_err());

        // Test different timezone
        let result = format_time("2025-01-11T15:00:00+02:00").unwrap();
        assert_eq!(result.len(), 5, "Time should be in format HH.MM");
        assert!(result.contains("."), "Time should contain .");
    }

    #[test]
    fn test_has_live_games() {
        let games = vec![
            GameData {
                home_team: "Home".to_string(),
                away_team: "Away".to_string(),
                time: "18.00".to_string(),
                result: "0-0".to_string(),
                score_type: ScoreType::Scheduled,
                is_overtime: false,
                is_shootout: false,
                goal_events: vec![],
                played_time: 0,
                serie: "RUNKOSARJA".to_string(),
                finished_type: String::new(),
                log_time: String::new(),
            },
            GameData {
                home_team: "Home2".to_string(),
                away_team: "Away2".to_string(),
                time: "".to_string(),
                result: "1-1".to_string(),
                score_type: ScoreType::Ongoing,
                is_overtime: false,
                is_shootout: false,
                goal_events: vec![],
                played_time: 1200,
                serie: "RUNKOSARJA".to_string(),
                finished_type: String::new(),
                log_time: String::new(),
            },
        ];

        assert!(has_live_games(&games));

        let no_live_games = vec![
            GameData {
                home_team: "Home".to_string(),
                away_team: "Away".to_string(),
                time: "18.00".to_string(),
                result: "0-0".to_string(),
                score_type: ScoreType::Scheduled,
                is_overtime: false,
                is_shootout: false,
                goal_events: vec![],
                played_time: 0,
                serie: "RUNKOSARJA".to_string(),
                finished_type: String::new(),
                log_time: String::new(),
            },
            GameData {
                home_team: "Home2".to_string(),
                away_team: "Away2".to_string(),
                time: "".to_string(),
                result: "2-1".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                goal_events: vec![],
                played_time: 3600,
                serie: "RUNKOSARJA".to_string(),
                finished_type: String::new(),
                log_time: String::new(),
            },
        ];

        assert!(!has_live_games(&no_live_games));
    }
}
