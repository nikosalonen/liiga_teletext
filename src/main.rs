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
const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

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
#[command(author = "Niko Salonen", about, long_about = None)]
#[command(disable_version_flag = true)]
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

    /// Show version information
    #[arg(short = 'V', long = "version", help_heading = "Info")]
    version: bool,
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
    let crates_io_url = format!("https://crates.io/api/v1/crates/{}", CRATE_NAME);

    let client = reqwest::Client::new();
    let user_agent = format!("{}/{}", CRATE_NAME, CURRENT_VERSION);
    let response = match client
        .get(&crates_io_url)
        .header("User-Agent", user_agent)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Failed to check for updates: {}", e);
            return None;
        }
    };

    let json: serde_json::Value = match response.json::<serde_json::Value>().await {
        Ok(json) => json,
        Err(e) => {
            eprintln!("Failed to parse update response: {}", e);
            return None;
        }
    };

    // Try max_stable_version instead of newest_version
    json.get("crate")
        .and_then(|c| c.get("max_stable_version"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn print_version_info(latest_version: &str) {
    let current = Version::parse(CURRENT_VERSION).unwrap_or_else(|e| {
        eprintln!("Failed to parse current version: {}", e);
        Version::new(0, 0, 0)
    });
    let latest = Version::parse(latest_version).unwrap_or_else(|e| {
        eprintln!("Failed to parse latest version: {}", e);
        Version::new(0, 0, 0)
    });

    if latest > current {
        println!();
        execute!(
            stdout(),
            SetForegroundColor(Color::White),
            Print("╔════════════════════════════════════╗\n"),
            Print("║ Liiga Teletext Status              ║\n"),
            Print("╠════════════════════════════════════╣\n"),
            Print("║ Current Version: "),
            SetForegroundColor(Color::Yellow),
            Print(CURRENT_VERSION),
            SetForegroundColor(Color::White),
            Print("             ║\n"),
            Print("║ Latest Version:  "),
            SetForegroundColor(Color::Cyan),
            Print(latest_version),
            SetForegroundColor(Color::White),
            Print("             ║\n"),
            Print("╠════════════════════════════════════╣\n"),
            Print("║ Update available! Run:             ║\n"),
            Print("║ "),
            SetForegroundColor(Color::Cyan),
            Print("cargo install liiga_teletext"),
            SetForegroundColor(Color::White),
            Print("       ║\n"),
            Print("╚════════════════════════════════════╝\n"),
            ResetColor
        )
        .ok();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Handle version flag first
    if args.version {
        execute!(
            stdout(),
            SetForegroundColor(Color::Cyan),
            Print(format!(
                "\n{}",
                r#"
 _     _ _               _____    _      _            _
| |   (_|_) __ _  __ _  |_   _|__| | ___| |_ _____  _| |_
| |   | | |/ _` |/ _` |   | |/ _ \ |/ _ \ __/ _ \ \/ / __|
| |___| | | (_| | (_| |   | |  __/ |  __/ ||  __/>  <| |_
|_____|_|_|\__, |\__,_|   |_|\___|_|\___|\__\___/_/\_\\__|
           |___/
"#
            )),
            ResetColor
        )
        .ok();

        // Check for updates and show version info
        if let Some(latest_version) = check_latest_version().await {
            let current = Version::parse(CURRENT_VERSION).unwrap_or_else(|e| {
                eprintln!("Failed to parse current version: {}", e);
                Version::new(0, 0, 0)
            });
            let latest = Version::parse(&latest_version).unwrap_or_else(|e| {
                eprintln!("Failed to parse latest version: {}", e);
                Version::new(0, 0, 0)
            });

            if latest > current {
                print_version_info(&latest_version);
            } else {
                println!();
                execute!(
                    stdout(),
                    SetForegroundColor(Color::White),
                    Print("╔════════════════════════════════════╗\n"),
                    Print("║ Liiga Teletext Status              ║\n"),
                    Print("╠════════════════════════════════════╣\n"),
                    Print("║ Version: "),
                    SetForegroundColor(Color::Green),
                    Print(CURRENT_VERSION),
                    SetForegroundColor(Color::White),
                    Print("                     ║\n"),
                    Print("║ You're running the latest version! ║\n"),
                    Print("╚════════════════════════════════════╝\n"),
                    ResetColor
                )
                .ok();
            }
        }

        return Ok(());
    }

    // Check for new version in the background for other commands
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

        // Show version info after display if update is available
        if let Ok(Some(latest_version)) = version_check.await {
            print_version_info(&latest_version);
        }
        return Ok(());
    }

    // Interactive mode
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;

    // Run the interactive UI
    let result = run_interactive_ui(&mut stdout, &args).await;

    // Clean up terminal
    execute!(stdout, LeaveAlternateScreen)?;
    disable_raw_mode()?;

    // Show version info after UI closes if update is available
    if let Ok(Some(latest_version)) = version_check.await {
        print_version_info(&latest_version);
    }

    result
}

async fn run_interactive_ui(
    stdout: &mut std::io::Stdout,
    args: &Args,
) -> Result<(), Box<dyn std::error::Error>> {
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
        page.render(stdout)?;

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
                                page.render(stdout)?;
                            }
                        }
                        KeyCode::Right => {
                            let now = Instant::now();
                            if now.duration_since(last_page_change) >= Duration::from_millis(200) {
                                last_page_change = now;
                                page.next_page();
                                page.render(stdout)?;
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
