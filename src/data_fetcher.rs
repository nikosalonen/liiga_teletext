use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Deserialize, Serialize)]
struct LiigaMatch {
    home_team: String,
    away_team: String,
    start_time: String,
    score: Option<String>,
    status: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct LiigaResponse {
    matches: Vec<LiigaMatch>,
}

#[derive(Debug, Clone)]
pub struct GameData {
    pub home_team: String,
    pub away_team: String,
    pub time: String,
    pub result: String,
    pub score_type: String,
}

pub(crate) fn fetch_liiga_data() -> Result<Vec<GameData>, Box<dyn Error>> {
    let url = "https://liiga.fi/api/v2/games?tournament=playoffs&date=2025-04-02";

    let client = Client::new();
    let response = client.get(url).send()?.json::<LiigaResponse>()?;

    let games = response
        .matches
        .into_iter()
        .map(|m| {
            let score = m.score.unwrap_or_else(|| "-".to_string());
            let time = format_time(&m.start_time);

            GameData {
                home_team: m.home_team,
                away_team: m.away_team,
                time,
                result: score,
                score_type: "".to_string(),
            }
        })
        .collect();

    Ok(games)
}

fn format_time(timestamp: &str) -> String {
    // This would parse the API timestamp into the format "18.30"
    // Implementation depends on the actual format from the API
    "18.30".to_string() // Placeholder
}

// Replace get_mock_liiga_data with this in your main application
fn get_liiga_data() -> Vec<GameData> {
    fetch_liiga_data().unwrap_or_else(|e| {
        eprintln!("Error fetching data: {}", e);
        // Fall back to mock data if fetch fails
        get_mock_liiga_data()
    })
}

fn get_mock_liiga_data() -> Vec<GameData> {
    vec![
        GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "18.30".to_string(),
            result: "2-1".to_string(),
            score_type: "".to_string(),
        },
        GameData {
            home_team: "Kärpät".to_string(),
            away_team: "TPS".to_string(),
            time: "17.00".to_string(),
            result: "3-2".to_string(),
            score_type: "".to_string(),
        },
        GameData {
            home_team: "Ilves".to_string(),
            away_team: "Lukko".to_string(),
            time: "18.30".to_string(),
            result: "1-4".to_string(),
            score_type: "".to_string(),
        },
        GameData {
            home_team: "KalPa".to_string(),
            away_team: "Pelicans".to_string(),
            time: "17.00".to_string(),
            result: "0-2".to_string(),
            score_type: "".to_string(),
        },
        GameData {
            home_team: "JYP".to_string(),
            away_team: "HPK".to_string(),
            time: "18.30".to_string(),
            result: "-".to_string(),
            score_type: "".to_string(),
        },
    ]
}
