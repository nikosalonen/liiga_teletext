// src/main.rs
mod config;
mod data_fetcher;
mod teletext_ui;

use clap::Parser;
use config::Config;
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use data_fetcher::{GameData, fetch_liiga_data};
use semver::Version;
use std::io::{Write, stdout};
use std::path::Path;
use std::time::{Duration, Instant};
use teletext_ui::{GameResultData, TeletextPage, has_live_games};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Finnish Hockey League (Liiga) Teletext Viewer
///
/// A nostalgic teletext-style viewer for Finnish Hockey League scores and game information.
/// Displays game scores, goalscorers, and special situations (powerplay, overtime, shootout).
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
    #[arg(long = "plain", short = 'p', help_heading = "Display Options")]
    disable_links: bool,

    /// Update API domain in config. Will prompt for new domain if not provided.
    #[arg(long = "config", short = 'c', help_heading = "Configuration")]
    new_api_domain: Option<String>,

    /// Show games for a specific date in YYYY-MM-DD format.
    /// If not provided, shows today's or yesterday's games based on current time.
    #[arg(long = "date", short = 'd', help_heading = "Display Options")]
    date: Option<String>,
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

fn create_page(
    games: &[GameData],
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
) -> TeletextPage {
    let subheader = get_subheader(games);
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        subheader,
        disable_video_links,
        show_footer,
        ignore_height_limit,
    );

    for game in games {
        page.add_game_result(GameResultData::new(game));
    }

    page
}

/// Checks for the latest version of this crate on crates.io.
///
/// Returns `Some(version_string)` if a newer version is available,
/// or `None` if there was an error checking or if the current version is up to date.
async fn check_latest_version() -> Option<String> {
    const CRATES_IO_URL: &str = "https://crates.io/api/v1/crates/liiga_teletext";

    let client = reqwest::Client::new();
    let response = match client.get(CRATES_IO_URL).send().await {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Failed to check for updates: {}", e);
            return None;
        }
    };

    let json: serde_json::Value = match response.json().await {
        Ok(json) => json,
        Err(e) => {
            eprintln!("Failed to parse update response: {}", e);
            return None;
        }
    };

    json.get("versions")?
        .as_array()?
        .first()?
        .get("num")?
        .as_str()
        .map(String::from)
}

fn print_version_info(latest_version: &str) {
    let current = Version::parse(CURRENT_VERSION).unwrap_or_else(|_| Version::new(0, 0, 0));
    let latest = Version::parse(latest_version).unwrap_or_else(|_| Version::new(0, 0, 0));

    if latest > current {
        println!();
        execute!(
            stdout(),
            SetForegroundColor(Color::Yellow),
            Print(format!(
                "New version available: {} (current: {})\n",
                latest_version, CURRENT_VERSION
            )),
            Print("Update with: cargo install liiga_teletext\n"),
            ResetColor
        )
        .ok();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Check for new version in the background
    let version_check = tokio::spawn(check_latest_version());

    // Handle config update if requested
    if args.new_api_domain.is_some() {
        let config_path = Config::get_config_path();
        let mut config = if Path::new(&config_path).exists() {
            Config::load()?
        } else {
            Config {
                api_domain: String::new(),
            }
        };

        let new_domain = if let Some(domain) = args.new_api_domain {
            domain
        } else {
            print!("Please enter new API domain: ");
            std::io::stdout().flush()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            input.trim().to_string()
        };

        config.api_domain = new_domain;
        config.save()?;
        println!("Config updated successfully!");

        // Show version info before exiting
        if let Ok(Some(latest_version)) = version_check.await {
            print_version_info(&latest_version);
        }
        return Ok(());
    }

    // Load config first to fail early if there's an issue
    let _config = Config::load()?;

    if args.once {
        // Quick view mode - just show the data once and exit
        let games = fetch_liiga_data(args.date).await?;
        let page = if games.is_empty() {
            let mut error_page = TeletextPage::new(
                221,
                "JÄÄKIEKKO".to_string(),
                "SM-LIIGA".to_string(),
                args.disable_links,
                false, // Don't show footer in quick view mode
                true,  // Ignore height limit in quick view mode
            );
            error_page.add_error_message("Ei otteluita tänään");
            error_page
        } else {
            create_page(&games, args.disable_links, false, true) // Ignore height limit in quick view mode
        };

        let mut stdout = stdout();
        enable_raw_mode()?;
        page.render(&mut stdout)?;
        disable_raw_mode()?;
        println!(); // Add a newline at the end

        // Show version info before exiting
        if let Ok(Some(latest_version)) = version_check.await {
            print_version_info(&latest_version);
        }
        return Ok(());
    }

    // Interactive mode
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;

    // Show version info in interactive mode if available
    if let Ok(Some(latest_version)) = version_check.await {
        if let Ok(current) = Version::parse(CURRENT_VERSION) {
            if let Ok(latest) = Version::parse(&latest_version) {
                if latest > current {
                    execute!(
                        stdout,
                        MoveTo(0, 0),
                        SetForegroundColor(Color::Yellow),
                        Print(format!(
                            "New version {} available! Press 'q' and run: cargo install liiga_teletext",
                            latest_version
                        )),
                        ResetColor,
                    )?;
                    // Wait a moment to show the message
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    let mut last_manual_refresh = Instant::now()
        .checked_sub(Duration::from_secs(10))
        .unwrap_or_else(Instant::now);
    let mut last_page_change = Instant::now()
        .checked_sub(Duration::from_millis(200))
        .unwrap_or_else(Instant::now);

    loop {
        let games = fetch_liiga_data(args.date.clone()).await?;
        let mut page = if games.is_empty() {
            let mut error_page = TeletextPage::new(
                221,
                "JÄÄKIEKKO".to_string(),
                "SM-LIIGA".to_string(),
                args.disable_links,
                true,  // Show footer in interactive mode
                false, // Don't ignore height limit in interactive mode
            );
            error_page.add_error_message("Ei otteluita tänään");
            error_page
        } else {
            create_page(&games, args.disable_links, true, false) // Don't ignore height limit in interactive mode
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
                            let now = Instant::now();
                            if now.duration_since(last_page_change) >= Duration::from_millis(200) {
                                last_page_change = now;
                                page.previous_page();
                                page.render(&mut stdout)?;
                            }
                        }
                        KeyCode::Right => {
                            let now = Instant::now();
                            if now.duration_since(last_page_change) >= Duration::from_millis(200) {
                                last_page_change = now;
                                page.next_page();
                                page.render(&mut stdout)?;
                            }
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
