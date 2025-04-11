// src/main.rs
mod config;
mod data_fetcher;
mod teletext_ui;

use clap::Parser;
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

/// Finnish Hockey League (Liiga) Teletext Viewer
///
/// A nostalgic teletext-style viewer for Finnish Hockey League scores and game information.
/// Displays game scores, goal scorers, and special situations (powerplay, overtime, shootout).
///
/// In interactive mode (default):
/// - Use arrow keys (←/→) to navigate between pages
/// - Press 'r' to refresh data (10s cooldown between refreshes)
/// - Press 'q' to quit
///
/// The viewer automatically refreshes:
/// - Every minute when there are ongoing games
/// - Every hour when showing only completed games
#[derive(Parser, Debug)]
#[command(author = "Niko Salonen", version, about, long_about = None)]
struct Args {
    /// Show scores once and exit immediately. Useful for scripts or quick score checks.
    /// The output stays visible in terminal history.
    #[arg(short, long)]
    once: bool,

    /// Disable clickable video links in the output.
    /// Useful for terminals that don't support links or for plain text output.
    #[arg(long = "plain", help_heading = "Display Options")]
    disable_links: bool,
}

fn get_subheader(games: &[GameData]) -> String {
    if games.is_empty() {
        return "SM-LIIGA".to_string();
    }

    // Use the tournament type from the first game as they should all be from same tournament
    match games[0].serie.as_str() {
        "PLAYOFFS" => "PLAYOFFS".to_string(),
        "PLAYOUT" => "PLAYOUT-OTTELUT".to_string(),
        "QUALIFICATIONS" => "LIIGAKARSINTA".to_string(),
        _ => "RUNKOSARJA".to_string(),
    }
}

fn create_page(games: &[GameData], disable_video_links: bool, show_footer: bool) -> TeletextPage {
    let subheader = get_subheader(games);
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        subheader,
        disable_video_links,
        show_footer,
    );

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
    let args = Args::parse();

    // Load config first to fail early if there's an issue
    let config = Config::load()?;

    if args.once {
        // Quick view mode - just show the data once and exit
        let games = fetch_liiga_data().await?;
        let page = if games.is_empty() {
            let mut error_page = TeletextPage::new(
                221,
                "JÄÄKIEKKO".to_string(),
                "SM-LIIGA".to_string(),
                args.disable_links,
                false, // Don't show footer in quick view mode
            );
            error_page.add_error_message("Ei otteluita tänään");
            error_page
        } else {
            create_page(&games, args.disable_links, false)
        };

        let mut stdout = stdout();
        enable_raw_mode()?;
        page.render(&mut stdout)?;
        disable_raw_mode()?;
        println!(); // Add a newline at the end
        return Ok(());
    }

    // Interactive mode
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let mut last_manual_refresh = Instant::now()
        .checked_sub(Duration::from_secs(10))
        .unwrap_or_else(Instant::now);

    loop {
        let games = fetch_liiga_data().await?;
        let mut page = if games.is_empty() {
            let mut error_page = TeletextPage::new(
                221,
                "JÄÄKIEKKO".to_string(),
                "SM-LIIGA".to_string(),
                args.disable_links,
                true, // Show footer in interactive mode
            );
            error_page.add_error_message("Ei otteluita tänään");
            error_page
        } else {
            create_page(&games, args.disable_links, true)
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
                            let now = Instant::now();
                            if now.duration_since(last_manual_refresh) >= Duration::from_secs(10) {
                                last_manual_refresh = now;
                                break; // Break inner loop to refresh data
                            }
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
