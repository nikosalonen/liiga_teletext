use crate::teletext_ui::ScoreType;
use chrono::Local;
use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Deserialize, Serialize)]
struct TeamInfo {
    #[serde(rename = "teamName")]
    team_name: String,
    goals: i32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Game {
    #[serde(rename = "homeTeam")]
    home_team: TeamInfo,
    #[serde(rename = "awayTeam")]
    away_team: TeamInfo,
    start: String,
    started: bool,
    ended: bool,
    #[serde(rename = "finishedType")]
    finished_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LiigaResponse {
    games: Vec<Game>,
}

#[derive(Debug, Clone)]
pub struct GameData {
    pub home_team: String,
    pub away_team: String,
    pub time: String,
    pub result: String,
    pub score_type: ScoreType,
}

pub(crate) fn fetch_liiga_data() -> Result<Vec<GameData>, Box<dyn Error>> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    //let debug_date = "2025-04-02";
    let tournaments = ["playoffs", "playout", "qualifications"];
    let client = Client::new();
    let mut all_games = Vec::new();

    for tournament in tournaments {
        let url = format!(
            "https://liiga.fi/api/v2/games?tournament={}&date={}",
            tournament, today
        );

        match client.get(&url).send() {
            Ok(response) => match response.text() {
                Ok(response_text) => match serde_json::from_str::<LiigaResponse>(&response_text) {
                    Ok(response) => {
                        let games = response
                            .games
                            .into_iter()
                            .map(|m| {
                                let time = format_time(&m.start)?;
                                let result = format!("{}-{}", m.home_team.goals, m.away_team.goals);

                                let score_type = if !m.started {
                                    ScoreType::Scheduled
                                } else if !m.ended {
                                    ScoreType::Ongoing
                                } else {
                                    ScoreType::Final
                                };

                                Ok(GameData {
                                    home_team: m.home_team.team_name,
                                    away_team: m.away_team.team_name,
                                    time,
                                    result,
                                    score_type,
                                })
                            })
                            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
                        all_games.extend(games);
                    }
                    Err(e) => eprintln!("Failed to parse JSON for {}: {}", tournament, e),
                },
                Err(e) => eprintln!("Failed to get response text for {}: {}", tournament, e),
            },
            Err(e) => eprintln!("Failed to send request for {}: {}", tournament, e),
        }
    }

    Ok(all_games)
}

fn format_time(timestamp: &str) -> Result<String, Box<dyn Error>> {
    let utc_time = timestamp.parse::<DateTime<Utc>>()?;
    let local_time = utc_time.with_timezone(&Local);
    Ok(local_time.format("%H.%M").to_string())
}
