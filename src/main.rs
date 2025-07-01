// src/main.rs
mod config;
mod data_fetcher;
mod error;
mod teletext_ui;

use clap::Parser;
use config::Config;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use data_fetcher::{GameData, fetch_liiga_data};
use error::AppError;
use semver::Version;
use chrono::{Local, NaiveDate};
use std::io::stdout;
use std::path::Path;
use std::time::{Duration, Instant};
use teletext_ui::{GameResultData, TeletextPage};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    prelude::*,
    EnvFilter,
};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Determines if the application should run in non-interactive mode
/// based on the provided command line arguments.
fn is_noninteractive_mode(args: &Args) -> bool {
    args.once
        || args.list_config
        || args.version
        || args.new_api_domain.is_some()
        || args.new_log_file_path.is_some()
        || args.clear_log_file_path
}

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

    /// Update log file path in config. This sets a persistent custom log file location.
    #[arg(long = "set-log-file", help_heading = "Configuration")]
    new_log_file_path: Option<String>,

    /// Clear the custom log file path from config. This reverts to using the default log location.
    #[arg(long = "clear-log-file", help_heading = "Configuration")]
    clear_log_file_path: bool,

    /// List current configuration settings
    #[arg(long = "list-config", short = 'l', help_heading = "Configuration")]
    list_config: bool,

    /// Show games for a specific date in YYYY-MM-DD format.
    /// If not provided, shows today's or yesterday's games based on current time.
    #[arg(long = "date", short = 'd', help_heading = "Display Options")]
    date: Option<String>,

    /// Show version information
    #[arg(short = 'V', long = "version", help_heading = "Info")]
    version: bool,

    /// Enable debug mode which doesn't clear the terminal before drawing the UI.
    /// In this mode, info logs are written to the log file instead of being displayed in the terminal.
    /// The log file is created if it doesn't exist.
    #[arg(long = "debug", help_heading = "Debug")]
    debug: bool,

    /// Specify a custom log file path. If not provided, logs will be written to the default location.
    #[arg(long = "log-file", help_heading = "Debug")]
    log_file: Option<String>,
}

fn format_date_for_display(date_str: &str) -> String {
    // Parse the date using chrono for better error handling
    match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        Ok(date) => date.format("%d.%m.").to_string(),
        Err(_) => date_str.to_string(), // Fallback if parsing fails
    }
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
        "valmistavat_ottelut" => "HARJOITUSOTTELUT".to_string(),
        "PRACTICE" => "HARJOITUSOTTELUT".to_string(),
        _ => "RUNKOSARJA".to_string(),
    }
}

fn create_page(
    games: &[GameData],
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
    debug_mode: bool,
) -> TeletextPage {
    let subheader = get_subheader(games);
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        subheader,
        disable_video_links,
        show_footer,
        ignore_height_limit,
        debug_mode,
    );

    for game in games {
        page.add_game_result(GameResultData::new(game));
    }

    page
}

/// Validates if a game is in the future by checking both time and start fields.
/// Returns true if the game has a non-empty time field and a valid future start date.
fn is_future_game(game: &GameData) -> bool {
    // Check if time field is non-empty (indicates scheduled game)
    if game.time.is_empty() {
        return false;
    }

    // Check if start field contains a valid future date
    if game.start.is_empty() {
        return false;
    }

    // Parse the start date to validate it's in the future
    // Expected format: YYYY-MM-DDThh:mm:ssZ
    match chrono::DateTime::parse_from_rfc3339(&game.start) {
        Ok(game_start) => {
            let now = chrono::Utc::now();
            let is_future = game_start > now;

            if !is_future {
                tracing::debug!("Game start time {} is not in the future (current: {})",
                    game_start, now);
            }

            is_future
        }
        Err(e) => {
            tracing::warn!("Failed to parse game start time '{}': {}", game.start, e);
            false
        }
    }
}

/// Creates a TeletextPage for future games if the games are scheduled.
/// Returns Some(TeletextPage) if the games are future games, None otherwise.
fn create_future_games_page(
    games: &[GameData],
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
    debug_mode: bool,
) -> Option<TeletextPage> {
    // Check if these are future games by validating both time and start fields
    if !games.is_empty() && is_future_game(&games[0]) {
        // Extract date from the first game's start field (assuming format YYYY-MM-DDThh:mm:ssZ)
        let start_str = &games[0].start;
        let date_str = start_str.split('T').next().unwrap_or("");
        let formatted_date = format_date_for_display(date_str);

        let subheader = get_subheader(games);
        tracing::debug!("First game serie: '{}', subheader: '{}'", games[0].serie, subheader);

        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            subheader,
            disable_video_links,
            show_footer,
            ignore_height_limit,
            debug_mode,
        );

        // Add the "Seuraavat ottelut" line
        page.add_future_games_header(format!("Seuraavat ottelut {}", formatted_date));

        // Set auto-refresh disabled for scheduled games
        page.set_auto_refresh_disabled(true);

        for game in games {
            page.add_game_result(GameResultData::new(game));
        }

        Some(page)
    } else {
        None
    }
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

fn print_logo() {
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
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let args = Args::parse();

    // Try to load config to get log file path if specified
    let config_log_path = Config::load().await.ok().and_then(|config| config.log_file_path);

    // Set up logging to both console and file
    let custom_log_path = args.log_file.as_ref().or(config_log_path.as_ref());
    let (log_dir, log_file_name) = match custom_log_path {
        Some(custom_path) => {
            let path = Path::new(custom_path);
            let parent = path.parent().unwrap_or(Path::new("."));
            let file_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("liiga_teletext.log");
            (parent.to_string_lossy().to_string(), file_name.to_string())
        }
        None => (Config::get_log_dir_path(), "liiga_teletext.log".to_string())
    };

    // Create log directory if it doesn't exist
    if !Path::new(&log_dir).exists() {
        std::fs::create_dir_all(&log_dir).map_err(|e| {
            eprintln!("Failed to create log directory: {}", e);
            e
        })?;
    }

    // Set up a rolling file appender that creates a new log file each day
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        &log_dir,
        &log_file_name,
    );

    // Create a non-blocking writer for the file appender
    // The guard must be kept alive for the duration of the program
    // to ensure logs are flushed properly
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Set up the subscriber with appropriate outputs based on mode
    let registry = tracing_subscriber::registry();
    let is_noninteractive = is_noninteractive_mode(&args);

    if is_noninteractive {
        // Non-interactive: log to both stdout and file
        registry
            .with(
                fmt::Layer::new()
                    .with_writer(std::io::stdout)
                    .with_ansi(true)
                    .with_filter(EnvFilter::from_default_env().add_directive("liiga_teletext=info".parse().unwrap()))
            )
            .with(
                fmt::Layer::new()
                    .with_writer(non_blocking)
                    .with_ansi(false)
                    .with_span_events(FmtSpan::CLOSE)
                    .with_filter(EnvFilter::from_default_env().add_directive("liiga_teletext=debug".parse().unwrap()))
            )
            .init();
    } else {
        // Interactive: log only to file
        registry
            .with(
                fmt::Layer::new()
                    .with_writer(non_blocking)
                    .with_ansi(false)
                    .with_span_events(FmtSpan::CLOSE)
                    .with_filter(EnvFilter::from_default_env().add_directive("liiga_teletext=debug".parse().unwrap()))
            )
            .init();
    }

    // Log the location of the log file
    let log_file_path = format!("{}/{}", log_dir, log_file_name);
    tracing::info!("Logs are being written to: {}", log_file_path);

    // Handle version flag first
    if args.version {
        print_logo();

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

    // Handle configuration operations without version check
    if args.list_config {
        print_logo();
        Config::display().await?;
        return Ok(());
    }

    // Handle configuration updates
    if args.new_api_domain.is_some() || args.new_log_file_path.is_some() || args.clear_log_file_path {
        let mut config = Config::load().await.unwrap_or_else(|_| Config {
            api_domain: String::new(),
            log_file_path: None,
        });

        if let Some(new_domain) = args.new_api_domain {
            config.api_domain = new_domain;
        }

        if let Some(new_log_path) = args.new_log_file_path {
            config.log_file_path = Some(new_log_path);
        } else if args.clear_log_file_path {
            config.log_file_path = None;
            println!("Custom log file path cleared. Using default location.");
        }

        config.save().await?;
        println!("Config updated successfully!");
        return Ok(());
    }

    // Check for new version in the background for non-config operations
    let version_check = tokio::spawn(check_latest_version());

    // Load config first to fail early if there's an issue
    let _config = Config::load().await?;

    if args.once {
        // Quick view mode - just show the data once and exit
        let (games, fetched_date) = match fetch_liiga_data(args.date.clone()).await {
            Ok((games, fetched_date)) => (games, fetched_date),
            Err(e) => {
                let mut error_page = TeletextPage::new(
                    221,
                    "JÄÄKIEKKO".to_string(),
                    "SM-LIIGA".to_string(),
                    args.disable_links,
                    false,
                    true,
                    args.debug,
                );
                error_page.add_error_message(&e.to_string());
                error_page.render(&mut stdout())?;
                println!();
                return Ok(());
            }
        };
        let page = if games.is_empty() {
            let mut error_page = TeletextPage::new(
                221,
                "JÄÄKIEKKO".to_string(),
                "SM-LIIGA".to_string(),
                args.disable_links,
                false, // Don't show footer in quick view mode
                true,  // Ignore height limit in quick view mode
                args.debug,
            );
            let today = Local::now().format("%Y-%m-%d").to_string();
            if fetched_date == today {
                error_page.add_error_message("Ei otteluita tänään");
            } else {
                error_page.add_error_message(&format!("Ei otteluita {} päivälle", format_date_for_display(&fetched_date)));
            }
            error_page
        } else {
            // Try to create a future games page, fall back to regular page if not future games
            create_future_games_page(&games, args.disable_links, false, true, args.debug)
                .unwrap_or_else(|| create_page(&games, args.disable_links, false, true, args.debug))
        };

        page.render(&mut stdout())?;
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
) -> Result<(), AppError> {
    let mut last_manual_refresh = Instant::now()
        .checked_sub(Duration::from_secs(15))
        .unwrap_or_else(Instant::now);
    let mut last_auto_refresh = Instant::now()
        .checked_sub(Duration::from_secs(10))
        .unwrap_or_else(Instant::now);
    let mut last_page_change = Instant::now()
        .checked_sub(Duration::from_millis(200))
        .unwrap_or_else(Instant::now);
    let mut needs_refresh = true;
    let mut current_page: Option<TeletextPage> = None;
    let mut pending_resize = false;
    let mut resize_timer = Instant::now();
    let mut last_resize = Instant::now()
        .checked_sub(Duration::from_millis(500))
        .unwrap_or_else(Instant::now);
    let mut last_games = Vec::new();
    let mut all_games_scheduled = false;

    loop {
        // Check for auto-refresh first
        // Don't auto-refresh if all games are scheduled (future games)
        if !needs_refresh && !last_games.is_empty() && !all_games_scheduled {
            if last_auto_refresh.elapsed() >= Duration::from_secs(60) {
                needs_refresh = true;
            }
        }

        if needs_refresh {
            let (games, had_error, fetched_date) = match fetch_liiga_data(args.date.clone()).await {
                Ok((games, fetched_date)) => (games, false, fetched_date),
                Err(e) => {
                    let mut error_page = TeletextPage::new(
                        221,
                        "JÄÄKIEKKO".to_string(),
                        "SM-LIIGA".to_string(),
                        args.disable_links,
                        true,
                        false,
                        args.debug,
                    );
                    error_page.add_error_message(&e.to_string());
                    current_page = Some(error_page);
                    if let Some(page) = &current_page {
                        page.render(stdout)?;
                    }
                    (Vec::new(), true, String::new())
                }
            };
            last_games = games.clone();

            // Check if all games are scheduled (future games)
            all_games_scheduled = !games.is_empty() && games.iter().all(|game| is_future_game(game));

            if all_games_scheduled {
                tracing::info!("All games are scheduled - auto-refresh disabled");
            }

            // Only create a new page if we didn't have an error
            if !had_error {
                let page = if games.is_empty() {
                    let mut error_page = TeletextPage::new(
                        221,
                        "JÄÄKIEKKO".to_string(),
                        "SM-LIIGA".to_string(),
                        args.disable_links,
                        true,
                        false,
                        args.debug,
                    );
                    let today = Local::now().format("%Y-%m-%d").to_string();
                    if fetched_date == today {
                        error_page.add_error_message("Ei otteluita tänään");
                    } else {
                        error_page.add_error_message(&format!("Ei otteluita {} päivälle", format_date_for_display(&fetched_date)));
                    }
                    error_page
                } else {
                    // Try to create a future games page, fall back to regular page if not future games
                    create_future_games_page(&games, args.disable_links, true, false, args.debug)
                        .unwrap_or_else(|| create_page(&games, args.disable_links, true, false, args.debug))
                };

                // Store the current page state
                current_page = Some(page);

                // Render only when we have new data
                if let Some(page) = &current_page {
                    page.render(stdout)?;
                }
            }
            needs_refresh = false;
            last_auto_refresh = Instant::now();
        }

        // Handle pending resize after a longer delay
        if pending_resize && resize_timer.elapsed() >= Duration::from_millis(500) {
            if let Some(page) = &mut current_page {
                page.handle_resize();
                page.render(stdout)?;
            }
            pending_resize = false;
        }

        // Event loop with shorter timeout
        if event::poll(Duration::from_millis(20))? {
            match event::read()? {
                Event::Key(key_event) => match key_event.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('r') => {
                        if last_manual_refresh.elapsed() >= Duration::from_secs(15) {
                            needs_refresh = true;
                            last_manual_refresh = Instant::now();
                        }
                    }
                    KeyCode::Left => {
                        if last_page_change.elapsed() >= Duration::from_millis(200) {
                            if let Some(page) = &mut current_page {
                                page.previous_page();
                                page.render(stdout)?;
                            }
                            last_page_change = Instant::now();
                        }
                    }
                    KeyCode::Right => {
                        if last_page_change.elapsed() >= Duration::from_millis(200) {
                            if let Some(page) = &mut current_page {
                                page.next_page();
                                page.render(stdout)?;
                            }
                            last_page_change = Instant::now();
                        }
                    }
                    _ => {}
                },
                Event::Resize(_, _) => {
                    // Only set pending resize if enough time has passed since last resize
                    if last_resize.elapsed() >= Duration::from_millis(500) {
                        resize_timer = Instant::now();
                        pending_resize = true;
                        last_resize = Instant::now();
                    }
                }
                _ => {}
            }
        }

        // Add a smaller delay between polls
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
