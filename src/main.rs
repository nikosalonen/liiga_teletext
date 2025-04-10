// src/main.rs
mod config;
mod data_fetcher;
mod teletext_ui;

use config::Config;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use data_fetcher::{GameData, fetch_liiga_data};
use std::io::stdout;
use std::time::{Duration, Instant};
use teletext_ui::{ScoreType, TeletextPage};

fn get_subheader(games: &[GameData]) -> String {
    if games.is_empty() {
        return "SM-LIIGA".to_string();
    }

    // Use the tournament type from the first game as they should all be from same tournament
    match games[0].tournament.as_str() {
        "runkosarja" => "RUNKOSARJA".to_string(),
        "playoffs" => "PLAYOFFS".to_string(),
        "playout" => "PLAYOUT-OTTELUT".to_string(),
        "qualifications" => "LIIGAKARSINTA".to_string(),
        _ => "SM-LIIGA".to_string(),
    }
}

fn create_page(games: &[GameData]) -> TeletextPage {
    let subheader = get_subheader(games);
    let mut page = TeletextPage::new(221, "JÄÄKIEKKO".to_string(), subheader);

    for game in games {
        page.add_game_result(
            game.home_team.clone(),
            game.away_team.clone(),
            game.time.clone(),
            game.result.clone(),
            game.score_type.clone(),
            game.is_overtime,
            game.is_shootout,
            game.goal_events.clone(),
        );
    }

    page
}

fn has_live_games(games: &[GameData]) -> bool {
    games
        .iter()
        .any(|game| matches!(game.score_type, ScoreType::Ongoing))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load config first to fail early if there's an issue
    Config::load()?;

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;

    loop {
        let games = fetch_liiga_data().await?;
        let mut page = if games.is_empty() {
            let mut error_page =
                TeletextPage::new(221, "JÄÄKIEKKO".to_string(), "SM-LIIGA".to_string());
            error_page.add_error_message("Ei otteluita tänään");
            error_page
        } else {
            create_page(&games)
        };

        // Initial render
        page.render(&mut stdout)?;

        // Check if we need to update more frequently due to live games
        let update_interval = if has_live_games(&games) {
            Duration::from_secs(60) // 1 minute for live games
        } else {
            Duration::from_secs(3600) // 1 hour for non-live games
        };

        // Wait for key press or timeout
        let last_update = Instant::now();
        loop {
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => {
                            execute!(stdout, LeaveAlternateScreen)?;
                            disable_raw_mode()?;
                            return Ok(());
                        }
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            break; // Break inner loop to refresh data
                        }
                        KeyCode::Left => {
                            page.previous_page();
                            page.render(&mut stdout)?;
                        }
                        KeyCode::Right => {
                            page.next_page();
                            page.render(&mut stdout)?;
                        }
                        _ => {}
                    }
                }
            }

            // Check if it's time to update for live games
            if last_update.elapsed() >= update_interval {
                break; // Break inner loop to refresh data
            }

            // Small sleep to prevent CPU hogging
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}
