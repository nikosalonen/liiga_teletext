// src/main.rs
mod config;
mod data_fetcher;
mod error;
mod teletext_ui;

use chrono::{Datelike, Local, NaiveDate, Utc};
use clap::Parser;
use config::Config;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use data_fetcher::{GameData, fetch_liiga_data, is_historical_date};
use error::AppError;
use semver::Version;
use std::io::stdout;
use std::path::Path;
use std::time::{Duration, Instant};
use teletext_ui::{GameResultData, TeletextPage};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, format::FmtSpan},
    prelude::*,
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
/// - Use Shift+←/→ to navigate between dates with games
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
    // Priority: PLAYOFFS > PLAYOUT-OTTELUT > LIIGAKARSINTA > HARJOITUSOTTELUT > RUNKOSARJA
    let mut priority = 4; // Default to RUNKOSARJA
    for game in games {
        let serie_lower = game.serie.to_ascii_lowercase();
        let current_priority = match serie_lower.as_str() {
            "playoffs" => 0,
            "playout" => 1,
            "qualifications" => 2,
            "valmistavat_ottelut" | "practice" => 3,
            _ => 4,
        };
        if current_priority < priority {
            priority = current_priority;
            if priority == 0 {
                break;
            } // Found highest priority
        }
    }

    match priority {
        0 => "PLAYOFFS".to_string(),
        1 => "PLAYOUT-OTTELUT".to_string(),
        2 => "LIIGAKARSINTA".to_string(),
        3 => "HARJOITUSOTTELUT".to_string(),
        _ => "RUNKOSARJA".to_string(),
    }
}

/// Creates a base TeletextPage with common initialization logic.
/// This helper function reduces code duplication between create_page and create_future_games_page.
async fn create_base_page(
    games: &[GameData],
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
    future_games_header: Option<String>,
    fetched_date: Option<String>,
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

    // Set the fetched date if provided
    if let Some(date) = fetched_date {
        page.set_fetched_date(date);
    }

    // Add future games header first if provided
    if let Some(header) = future_games_header {
        page.add_future_games_header(header);
    }

    for game in games {
        page.add_game_result(GameResultData::new(game));
    }

    // Set season countdown if regular season hasn't started yet
    page.set_show_season_countdown(games).await;

    page
}

async fn create_page(
    games: &[GameData],
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
    fetched_date: Option<String>,
) -> TeletextPage {
    create_base_page(
        games,
        disable_video_links,
        show_footer,
        ignore_height_limit,
        None,
        fetched_date,
    )
    .await
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
                tracing::debug!(
                    "Game start time {} is not in the future (current: {})",
                    game_start,
                    now
                );
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
async fn create_future_games_page(
    games: &[GameData],
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
    show_future_header: bool,
    fetched_date: Option<String>,
) -> Option<TeletextPage> {
    // Check if these are future games by validating both time and start fields
    if !games.is_empty() && is_future_game(&games[0]) {
        // Extract date from the first game's start field (assuming format YYYY-MM-DDThh:mm:ssZ)
        let start_str = &games[0].start;
        let date_str = start_str.split('T').next().unwrap_or("");
        let formatted_date = format_date_for_display(date_str);

        tracing::debug!(
            "First game serie: '{}', subheader: '{}'",
            games[0].serie,
            get_subheader(games)
        );

        let future_games_header = if show_future_header {
            Some(format!("Seuraavat ottelut {formatted_date}"))
        } else {
            None
        };
        let mut page = create_base_page(
            games,
            disable_video_links,
            show_footer,
            ignore_height_limit,
            future_games_header,
            fetched_date, // Pass the fetched date to show it in the header
        )
        .await;

        // Set auto-refresh disabled for scheduled games
        page.set_auto_refresh_disabled(true);

        Some(page)
    } else {
        None
    }
}

/// Checks if the given key event matches the date navigation shortcut.
/// Uses Shift + Left/Right for all platforms (works reliably in all terminals)
fn is_date_navigation_key(key_event: &crossterm::event::KeyEvent, is_left: bool) -> bool {
    let expected_code = if is_left {
        KeyCode::Left
    } else {
        KeyCode::Right
    };

    if key_event.code != expected_code {
        return false;
    }

    // Use Shift key for date navigation (works reliably in all terminals)
    let has_shift_modifier = key_event.modifiers.contains(KeyModifiers::SHIFT);

    if has_shift_modifier {
        tracing::debug!(
            "Date navigation key detected: Shift + {}",
            if is_left { "Left" } else { "Right" }
        );
        return true;
    }

    false
}

/// Gets the target date for navigation, using current_date if available,
/// otherwise determining the appropriate date based on current time.
fn get_target_date_for_navigation(current_date: &Option<String>) -> String {
    current_date.as_ref().cloned().unwrap_or_else(|| {
        // If no current date, use today/yesterday based on time
        if crate::data_fetcher::processors::should_show_todays_games() {
            Utc::now()
                .with_timezone(&Local)
                .format("%Y-%m-%d")
                .to_string()
        } else {
            let yesterday = Utc::now()
                .with_timezone(&Local)
                .date_naive()
                .pred_opt()
                .expect("Date underflow cannot happen");
            yesterday.format("%Y-%m-%d").to_string()
        }
    })
}

/// Checks if a date would require historical/schedule endpoint (from previous season).
/// This prevents navigation to previous season games via arrow keys.
fn would_be_previous_season(date: &str) -> bool {
    use chrono::{Local, Utc};
    let now = Utc::now().with_timezone(&Local);

    let date_parts: Vec<&str> = date.split('-').collect();
    if date_parts.len() < 2 {
        return false;
    }

    let date_year = date_parts[0].parse::<i32>().unwrap_or(now.year());
    let date_month = date_parts[1].parse::<u32>().unwrap_or(now.month());

    let current_year = now.year();
    let current_month = now.month();

    // If date is from a previous year, it's definitely previous season
    if date_year < current_year {
        return true;
    }

    // If same year, check hockey season logic
    if date_year == current_year {
        // Hockey season: September-February (regular), March-May (playoffs/playout)
        // Off-season: June-August

        // If we're in new regular season (September-December) and date is from previous season
        // (January-August), it's from the previous season
        if (9..=12).contains(&current_month) && date_month <= 8 {
            return true;
        }

        // If we're in early regular season (January-February) and date is from off-season
        // (June-August), it's from the previous season
        if (1..=2).contains(&current_month) && (6..=8).contains(&date_month) {
            return true;
        }
    }

    false
}

/// Finds the previous date with games by checking dates going backwards.
/// Returns None if no games are found within the current season or a reasonable time range.
/// Prevents navigation to previous season games for better UX.
async fn find_previous_date_with_games(current_date: &str) -> Option<String> {
    let current_parsed = match chrono::NaiveDate::parse_from_str(current_date, "%Y-%m-%d") {
        Ok(date) => date,
        Err(_) => return None,
    };

    tracing::info!(
        "Starting search for previous date with games from: {}",
        current_date
    );

    // Search up to 30 days in the past to stay within current season
    for days_back in 1..=30 {
        if let Some(check_date) = current_parsed.checked_sub_days(chrono::Days::new(days_back)) {
            let date_string = check_date.format("%Y-%m-%d").to_string();

            // Check if this date would be from the previous season
            if would_be_previous_season(&date_string) {
                tracing::info!(
                    "Reached previous season boundary at {}, stopping navigation (use -d flag for historical games)",
                    date_string
                );
                break;
            }

            // Log progress every 10 days
            if days_back % 10 == 0 {
                tracing::info!(
                    "Date navigation: checking {} ({} days back)",
                    date_string,
                    days_back
                );
            }

            // Add timeout to the fetch operation (shorter timeout for faster navigation)
            let fetch_future = fetch_liiga_data(Some(date_string.clone()));
            let timeout_duration = tokio::time::Duration::from_secs(5);

            match tokio::time::timeout(timeout_duration, fetch_future).await {
                Ok(Ok((games, fetched_date))) if !games.is_empty() => {
                    // Ensure the fetched date matches the requested date
                    if fetched_date == date_string {
                        tracing::info!(
                            "Found previous date with games: {} (after {} days)",
                            date_string,
                            days_back
                        );
                        return Some(date_string);
                    } else {
                        tracing::debug!(
                            "Skipping date {} because fetcher returned different date: {} (after {} days)",
                            date_string,
                            fetched_date,
                            days_back
                        );
                    }
                }
                Ok(Ok(_)) => {
                    // No games found, continue searching
                }
                Ok(Err(e)) => {
                    tracing::warn!(
                        "Error fetching data for {}: {} (continuing search)",
                        date_string,
                        e
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        "Timeout fetching data for {} (continuing search)",
                        date_string
                    );
                }
            }

            // Small delay to prevent API spam
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    }

    tracing::info!(
        "No previous date with games found within current season from {}",
        current_date
    );
    None
}

/// Finds the next date with games by checking dates going forwards.
/// Returns None if no games are found within a reasonable time range.
async fn find_next_date_with_games(current_date: &str) -> Option<String> {
    let current_parsed = match chrono::NaiveDate::parse_from_str(current_date, "%Y-%m-%d") {
        Ok(date) => date,
        Err(_) => return None,
    };

    tracing::info!(
        "Starting search for next date with games from: {}",
        current_date
    );

    // Search up to 60 days in the future (handles off-season periods)
    for days_ahead in 1..=60 {
        if let Some(check_date) = current_parsed.checked_add_days(chrono::Days::new(days_ahead)) {
            let date_string = check_date.format("%Y-%m-%d").to_string();

            // Log progress every 10 days
            if days_ahead % 10 == 0 {
                tracing::info!(
                    "Date navigation: checking {} ({} days ahead)",
                    date_string,
                    days_ahead
                );
            }

            // Add timeout to the fetch operation (shorter timeout for faster navigation)
            let fetch_future = fetch_liiga_data(Some(date_string.clone()));
            let timeout_duration = tokio::time::Duration::from_secs(5);

            match tokio::time::timeout(timeout_duration, fetch_future).await {
                Ok(Ok((games, fetched_date))) if !games.is_empty() => {
                    // Ensure the fetched date matches the requested date
                    if fetched_date == date_string {
                        tracing::info!(
                            "Found next date with games: {} (after {} days)",
                            date_string,
                            days_ahead
                        );
                        return Some(date_string);
                    } else {
                        tracing::debug!(
                            "Skipping date {} because fetcher returned different date: {} (after {} days)",
                            date_string,
                            fetched_date,
                            days_ahead
                        );
                    }
                }
                Ok(Ok(_)) => {
                    // No games found, continue searching
                }
                Ok(Err(e)) => {
                    tracing::warn!(
                        "Error fetching data for {}: {} (continuing search)",
                        date_string,
                        e
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        "Timeout fetching data for {} (continuing search)",
                        date_string
                    );
                }
            }

            // Small delay to prevent API spam
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    }

    tracing::info!(
        "No next date with games found within search range from {}",
        current_date
    );
    None
}

/// Checks for the latest version of this crate on crates.io.
///
/// Returns `Some(version_string)` if a newer version is available,
/// or `None` if there was an error checking or if the current version is up to date.
async fn check_latest_version() -> Option<String> {
    let crates_io_url = format!("https://crates.io/api/v1/crates/{CRATE_NAME}");

    // Create a properly configured HTTP client with timeout handling
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10)) // Shorter timeout for update checks
        .build()
        .unwrap_or_else(|_| reqwest::Client::new()); // Fallback to default client if builder fails

    let user_agent = format!("{CRATE_NAME}/{CURRENT_VERSION}");
    let response = match client
        .get(&crates_io_url)
        .header("User-Agent", user_agent)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Failed to check for updates: {e}");
            return None;
        }
    };

    let json: serde_json::Value = match response.json::<serde_json::Value>().await {
        Ok(json) => json,
        Err(e) => {
            eprintln!("Failed to parse update response: {e}");
            return None;
        }
    };

    // Try max_stable_version instead of newest_version
    json.get("crate")
        .and_then(|c| c.get("max_stable_version"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Helper to print a dynamic-width version status box with optional color highlights
fn print_version_status_box(lines: Vec<(String, Option<Color>)>) {
    // Compute max content width
    let max_content_width = lines
        .iter()
        .map(|(l, _)| l.chars().count())
        .max()
        .unwrap_or(0);
    let box_width = max_content_width + 4; // 2 for borders, 2 for padding
    let border = format!("╔{:═<width$}╗", "", width = box_width - 2);
    let sep = format!("╠{:═<width$}╣", "", width = box_width - 2);
    let bottom = format!("╚{:═<width$}╝", "", width = box_width - 2);
    // Print top border
    execute!(
        stdout(),
        SetForegroundColor(Color::White),
        Print(format!("{border}\n"))
    )
    .ok();
    // Print lines
    for (i, (line, color)) in lines.iter().enumerate() {
        let padded = format!("║ {line:<max_content_width$} ║");
        match color {
            Some(c) => {
                // Print up to the colored part, then color, then reset
                if let Some((pre, col)) = line.split_once(':') {
                    let pre = format!("║ {pre}:");
                    let col = col.trim_start();
                    let pad = max_content_width - (pre.chars().count() - 2 + col.chars().count());
                    execute!(
                        stdout(),
                        SetForegroundColor(Color::White),
                        Print(pre),
                        SetForegroundColor(*c),
                        Print(col),
                        SetForegroundColor(Color::White),
                        Print(format!("{:pad$} ║\n", "", pad = pad)),
                    )
                    .ok();
                } else {
                    execute!(
                        stdout(),
                        SetForegroundColor(*c),
                        Print(padded),
                        SetForegroundColor(Color::White),
                        Print("\n")
                    )
                    .ok();
                }
            }
            None => {
                execute!(
                    stdout(),
                    SetForegroundColor(Color::White),
                    Print(padded),
                    Print("\n")
                )
                .ok();
            }
        }
        // Separator after first or second line if needed
        if i == 0 && lines.len() > 2 {
            execute!(stdout(), Print(format!("{sep}\n"))).ok();
        }
    }
    // Print bottom border
    execute!(stdout(), Print(format!("{bottom}\n")), ResetColor).ok();
}

fn print_version_info(latest_version: &str) {
    let current = match Version::parse(CURRENT_VERSION) {
        Ok(v) => v,
        Err(_) => {
            // If we can't parse the current version, just show a generic message
            println!("Update available! Latest version: {latest_version}");
            return;
        }
    };
    let latest = match Version::parse(latest_version) {
        Ok(v) => v,
        Err(_) => {
            // If we can't parse the latest version, don't show anything
            return;
        }
    };

    if latest > current {
        println!();
        print_version_status_box(vec![
            ("Liiga Teletext Status".to_string(), None),
            ("".to_string(), None),
            (
                format!("Current Version: {CURRENT_VERSION}"),
                Some(Color::White),
            ),
            (
                format!("Latest Version:  {latest_version}"),
                Some(Color::Cyan),
            ),
            ("".to_string(), None),
            ("Update available! Run:".to_string(), None),
            (
                "cargo install liiga_teletext".to_string(),
                Some(Color::Cyan),
            ),
        ]);
    }
}

fn print_logo() {
    execute!(
        stdout(),
        SetForegroundColor(Color::Cyan),
        Print(format!(
            "\n{}",
            r#"

██╗░░░░░██╗██╗░██████╗░░█████╗░  ██████╗░██████╗░░░███╗░░
██║░░░░░██║██║██╔════╝░██╔══██╗  ╚════██╗╚════██╗░████║░░
██║░░░░░██║██║██║░░██╗░███████║  ░░███╔═╝░░███╔═╝██╔██║░░
██║░░░░░██║██║██║░░╚██╗██╔══██║  ██╔══╝░░██╔══╝░░╚═╝██║░░
███████╗██║██║╚██████╔╝██║░░██║  ███████╗███████╗███████╗
╚══════╝╚═╝╚═╝░╚═════╝░╚═╝░░╚═╝  ╚══════╝╚══════╝╚══════╝
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
    let config_log_path = Config::load()
        .await
        .ok()
        .and_then(|config| config.log_file_path);

    // Set up logging to both console and file
    let custom_log_path = args.log_file.as_ref().or(config_log_path.as_ref());
    let (log_dir, log_file_name) = match custom_log_path {
        Some(custom_path) => {
            let path = Path::new(custom_path);
            let parent = path.parent().unwrap_or(Path::new("."));
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("liiga_teletext.log");
            (parent.to_string_lossy().to_string(), file_name.to_string())
        }
        None => (Config::get_log_dir_path(), "liiga_teletext.log".to_string()),
    };

    // Create log directory if it doesn't exist
    if !Path::new(&log_dir).exists() {
        tokio::fs::create_dir_all(&log_dir).await.map_err(|e| {
            AppError::log_setup_error(format!("Failed to create log directory: {e}"))
        })?;
    }

    // Set up a rolling file appender that creates a new log file each day
    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, &log_file_name);

    // Create a non-blocking writer for the file appender
    // The guard must be kept alive for the duration of the program
    // to ensure logs are flushed properly
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Set up the subscriber with appropriate outputs based on mode
    let registry = tracing_subscriber::registry();
    let is_noninteractive = is_noninteractive_mode(&args);

    if is_noninteractive {
        if args.once && !args.debug {
            // Once mode without debug: log only to file, not to stdout
            registry
                .with(
                    fmt::Layer::new()
                        .with_writer(non_blocking)
                        .with_ansi(false)
                        .with_span_events(FmtSpan::CLOSE)
                        .with_filter(
                            EnvFilter::from_default_env()
                                .add_directive("liiga_teletext=debug".parse().unwrap()),
                        ),
                )
                .init();
        } else {
            // Other non-interactive modes: log to both stdout and file
            registry
                .with(
                    fmt::Layer::new()
                        .with_writer(std::io::stdout)
                        .with_ansi(true)
                        .with_filter(
                            EnvFilter::from_default_env()
                                .add_directive("liiga_teletext=info".parse().unwrap()),
                        ),
                )
                .with(
                    fmt::Layer::new()
                        .with_writer(non_blocking)
                        .with_ansi(false)
                        .with_span_events(FmtSpan::CLOSE)
                        .with_filter(
                            EnvFilter::from_default_env()
                                .add_directive("liiga_teletext=debug".parse().unwrap()),
                        ),
                )
                .init();
        }
    } else {
        // Interactive: log only to file
        registry
            .with(
                fmt::Layer::new()
                    .with_writer(non_blocking)
                    .with_ansi(false)
                    .with_span_events(FmtSpan::CLOSE)
                    .with_filter(
                        EnvFilter::from_default_env()
                            .add_directive("liiga_teletext=debug".parse().unwrap()),
                    ),
            )
            .init();
    }

    // Log the location of the log file
    let log_file_path = format!("{log_dir}/{log_file_name}");
    tracing::info!("Logs are being written to: {log_file_path}");

    // Handle version flag first
    if args.version {
        // Set terminal title for version display
        execute!(stdout(), crossterm::terminal::SetTitle("SM-LIIGA 221"))?;

        print_logo();

        // Check for updates and show version info
        if let Some(latest_version) = check_latest_version().await {
            let current = Version::parse(CURRENT_VERSION).map_err(AppError::VersionParse)?;
            let latest = Version::parse(&latest_version).map_err(AppError::VersionParse)?;

            if latest > current {
                print_version_info(&latest_version);
            } else {
                println!();
                print_version_status_box(vec![
                    ("Liiga Teletext Status".to_string(), None),
                    ("".to_string(), None),
                    (format!("Version: {CURRENT_VERSION}"), Some(Color::White)),
                    ("You're running the latest version!".to_string(), None),
                ]);
            }
        }

        return Ok(());
    }

    // Handle configuration operations without version check
    if args.list_config {
        // Set terminal title for config display
        execute!(stdout(), crossterm::terminal::SetTitle("SM-LIIGA 221"))?;

        print_logo();
        Config::display().await?;
        return Ok(());
    }

    // Handle configuration updates
    if args.new_api_domain.is_some() || args.new_log_file_path.is_some() || args.clear_log_file_path
    {
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

        // In --once mode, don't show loading messages (only show in interactive mode)

        let (games, fetched_date) = match fetch_liiga_data(args.date.clone()).await {
            Ok((games, fetched_date)) => (games, fetched_date),
            Err(e) => {
                let mut error_page = TeletextPage::new(
                    221,
                    "JÄÄKIEKKO".to_string(),
                    "SM-LIIGA".to_string(),
                    args.disable_links,
                    true,
                    false,
                );
                error_page.add_error_message(&format!("Virhe haettaessa otteluita: {e}"));
                // Set terminal title for non-interactive mode (error case)
                execute!(stdout(), crossterm::terminal::SetTitle("SM-LIIGA 221"))?;

                error_page.render(&mut stdout())?;
                println!();
                return Ok(());
            }
        };

        let page = if games.is_empty() {
            let mut no_games_page = TeletextPage::new(
                221,
                "JÄÄKIEKKO".to_string(),
                "SM-LIIGA".to_string(),
                args.disable_links,
                false, // Don't show footer in quick view mode
                true,  // Ignore height limit in quick view mode
            );
            // Use UTC internally, convert to local time for date formatting
            let today = Utc::now()
                .with_timezone(&Local)
                .format("%Y-%m-%d")
                .to_string();
            if fetched_date == today {
                no_games_page.add_error_message("Ei otteluita tänään");
            } else {
                no_games_page.add_error_message(&format!(
                    "Ei otteluita {} päivälle",
                    format_date_for_display(&fetched_date)
                ));
            }
            no_games_page
        } else {
            // Try to create a future games page, fall back to regular page if not future games
            // Only show future games header if no specific date was requested
            let show_future_header = args.date.is_none();
            match create_future_games_page(
                &games,
                args.disable_links,
                true,
                true,
                show_future_header,
                Some(fetched_date.clone()),
            )
            .await
            {
                Some(page) => page,
                None => {
                    create_page(
                        &games,
                        args.disable_links,
                        true,
                        true,
                        Some(fetched_date.clone()),
                    )
                    .await
                }
            }
        };

        // Set terminal title for non-interactive mode
        execute!(stdout(), crossterm::terminal::SetTitle("SM-LIIGA 221"))?;

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

    // Set terminal title/header to show app name
    execute!(stdout, crossterm::terminal::SetTitle("SM-LIIGA 221"))?;

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

/// Optimized interactive UI with change detection and adaptive polling
/// Performance improvements:
/// - Change detection prevents unnecessary UI re-renders
/// - Adaptive polling intervals reduce CPU usage
/// - Batched UI updates reduce flickering
/// - Memory cleanup for long-running sessions
async fn run_interactive_ui(stdout: &mut std::io::Stdout, args: &Args) -> Result<(), AppError> {
    // Timer management with adaptive intervals
    let mut last_manual_refresh = Instant::now()
        .checked_sub(Duration::from_secs(15))
        .unwrap_or_else(Instant::now);
    let mut last_auto_refresh = Instant::now()
        .checked_sub(Duration::from_secs(10))
        .unwrap_or_else(Instant::now);
    let mut last_page_change = Instant::now()
        .checked_sub(Duration::from_millis(200))
        .unwrap_or_else(Instant::now);
    let mut last_date_navigation = Instant::now()
        .checked_sub(Duration::from_millis(250))
        .unwrap_or_else(Instant::now);
    let mut last_resize = Instant::now()
        .checked_sub(Duration::from_millis(500))
        .unwrap_or_else(Instant::now);

    // State management
    let mut needs_refresh = true;
    let mut needs_render = false;
    let mut current_page: Option<TeletextPage> = None;
    let mut pending_resize = false;
    let mut resize_timer = Instant::now();

    // Date navigation state - track the current date being displayed
    let mut current_date = args.date.clone();

    // Change detection - track data changes to avoid unnecessary re-renders
    let mut last_games_hash = 0u64;
    let mut last_games = Vec::new();
    let mut all_games_scheduled = false;

    // Adaptive polling configuration
    let mut last_activity = Instant::now();

    // Cache monitoring tracking
    let mut cache_monitor_timer = Instant::now();
    const CACHE_MONITOR_INTERVAL: Duration = Duration::from_secs(300); // 5 minutes

    loop {
        // Adaptive polling interval based on activity
        let time_since_activity = last_activity.elapsed();
        let poll_interval = if time_since_activity < Duration::from_secs(5) {
            Duration::from_millis(50) // Active: 50ms (smooth interaction)
        } else if time_since_activity < Duration::from_secs(30) {
            Duration::from_millis(200) // Semi-active: 200ms (good responsiveness)
        } else {
            Duration::from_millis(500) // Idle: 500ms (conserve CPU)
        };

        // Check for auto-refresh with better logic
        if !needs_refresh
            && !last_games.is_empty()
            && !all_games_scheduled
            && last_auto_refresh.elapsed() >= Duration::from_secs(60)
        {
            needs_refresh = true;
            tracing::debug!("Auto-refresh triggered");
        }

        // Data fetching with change detection
        if needs_refresh {
            tracing::debug!("Fetching new data");

            // Show loading indicator for historical dates or when specific date is requested
            if let Some(ref date) = current_date {
                let mut loading_page = TeletextPage::new(
                    221,
                    "JÄÄKIEKKO".to_string(),
                    "SM-LIIGA".to_string(),
                    args.disable_links,
                    true,
                    false,
                );

                if is_historical_date(date) {
                    loading_page.add_error_message(&format!(
                        "Haetaan historiallista dataa päivälle {}...",
                        format_date_for_display(date)
                    ));
                    loading_page.add_error_message("Tämä voi kestää hetken, odotathan...");
                } else {
                    loading_page.add_error_message(&format!(
                        "Haetaan otteluita päivälle {}...",
                        format_date_for_display(date)
                    ));
                }

                current_page = Some(loading_page);
                needs_render = true;
            } else if current_page.is_none() {
                // Show loading for initial load when no specific date is requested
                let mut loading_page = TeletextPage::new(
                    221,
                    "JÄÄKIEKKO".to_string(),
                    "SM-LIIGA".to_string(),
                    args.disable_links,
                    true,
                    false,
                );
                loading_page.add_error_message("Haetaan päivän otteluita...");
                current_page = Some(loading_page);
                needs_render = true;
            }

            // Render the loading page immediately
            if needs_render {
                if let Some(page) = &current_page {
                    page.render(stdout)?;
                }
                needs_render = false;
            }

            let (games, had_error, fetched_date) =
                match fetch_liiga_data(current_date.clone()).await {
                    Ok((games, fetched_date)) => (games, false, fetched_date),
                    Err(e) => {
                        tracing::error!("Error fetching data: {}", e);
                        let mut error_page = TeletextPage::new(
                            221,
                            "JÄÄKIEKKO".to_string(),
                            "SM-LIIGA".to_string(),
                            args.disable_links,
                            true,
                            false,
                        );
                        error_page.add_error_message(&format!("Virhe haettaessa otteluita: {e}"));
                        current_page = Some(error_page);
                        needs_render = true;
                        (Vec::new(), true, String::new())
                    }
                };

            // Update current_date to track the actual date being displayed
            if !had_error && !fetched_date.is_empty() {
                current_date = Some(fetched_date.clone());
                tracing::debug!("Updated current_date to: {:?}", current_date);
            }

            // Change detection using a simple hash of game data
            let games_hash = calculate_games_hash(&games);
            let data_changed = games_hash != last_games_hash;

            if data_changed {
                tracing::debug!("Data changed, updating UI");
                last_games = games.clone();
                last_games_hash = games_hash;

                // Check if all games are scheduled (future games)
                all_games_scheduled = !games.is_empty() && games.iter().all(is_future_game);

                if all_games_scheduled {
                    tracing::info!("All games are scheduled - auto-refresh disabled");
                }

                // Only create a new page if we didn't have an error and data changed
                if !had_error {
                    let page = if games.is_empty() {
                        let mut error_page = TeletextPage::new(
                            221,
                            "JÄÄKIEKKO".to_string(),
                            "SM-LIIGA".to_string(),
                            args.disable_links,
                            true,
                            false,
                        );
                        // Use UTC internally, convert to local time for date formatting
                        let today = Utc::now()
                            .with_timezone(&Local)
                            .format("%Y-%m-%d")
                            .to_string();
                        if fetched_date == today {
                            error_page.add_error_message("Ei otteluita tänään");
                        } else {
                            error_page.add_error_message(&format!(
                                "Ei otteluita {} päivälle",
                                format_date_for_display(&fetched_date)
                            ));
                        }
                        error_page
                    } else {
                        // Try to create a future games page, fall back to regular page if not future games
                        let show_future_header = current_date.is_none();
                        match create_future_games_page(
                            &games,
                            args.disable_links,
                            true,
                            false,
                            show_future_header,
                            Some(fetched_date.clone()),
                        )
                        .await
                        {
                            Some(page) => page,
                            None => {
                                create_page(
                                    &games,
                                    args.disable_links,
                                    true,
                                    false,
                                    Some(fetched_date.clone()),
                                )
                                .await
                            }
                        }
                    };

                    current_page = Some(page);
                    needs_render = true;
                }
            } else {
                tracing::debug!("No data changes detected, skipping UI update");
            }

            needs_refresh = false;
            last_auto_refresh = Instant::now();
        }

        // Handle pending resize with debouncing
        if pending_resize && resize_timer.elapsed() >= Duration::from_millis(500) {
            tracing::debug!("Handling resize");
            if let Some(page) = &mut current_page {
                page.handle_resize();
                needs_render = true;
            }
            pending_resize = false;
        }

        // Batched UI rendering - only render when necessary
        if needs_render {
            if let Some(page) = &current_page {
                page.render(stdout)?;
                tracing::debug!("UI rendered");
            }
            needs_render = false;
        }

        // Event handling with adaptive polling
        if event::poll(poll_interval)? {
            last_activity = Instant::now(); // Reset activity timer

            match event::read()? {
                Event::Key(key_event) => {
                    tracing::debug!(
                        "Key event: {:?}, modifiers: {:?}",
                        key_event.code,
                        key_event.modifiers
                    );

                    // Check for date navigation first (Shift + Arrow keys)
                    if is_date_navigation_key(&key_event, true) {
                        // Shift + Left: Previous date with games
                        if last_date_navigation.elapsed() >= Duration::from_millis(250) {
                            tracing::info!("Previous date navigation requested");
                            tracing::debug!("Current date state: {:?}", current_date);
                            let target_date = get_target_date_for_navigation(&current_date);

                            // Show loading indicator
                            if let Some(page) = &mut current_page {
                                page.show_loading("Etsitään edellisiä otteluita...".to_string());
                                page.render_loading_indicator_only(stdout)?;
                            }

                            tracing::info!(
                                "Searching for previous date with games from: {}",
                                target_date
                            );

                            // Create a task that will update animation while search runs
                            let target_date_clone = target_date.clone();
                            let mut search_task = tokio::spawn(async move {
                                find_previous_date_with_games(&target_date_clone).await
                            });
                            let mut animation_interval =
                                tokio::time::interval(Duration::from_millis(200));

                            let result = loop {
                                tokio::select! {
                                    search_result = &mut search_task => {
                                        match search_result {
                                            Ok(date_option) => {
                                                break date_option;
                                            }
                                            Err(join_error) => {
                                                tracing::error!(
                                                    "Previous date search task failed: {}",
                                                    join_error
                                                );
                                                break None;
                                            }
                                        }
                                    }
                                    _ = animation_interval.tick() => {
                                        if let Some(page) = &mut current_page {
                                            page.update_loading_animation();
                                            page.render_loading_indicator_only(stdout)?;
                                        }
                                    }
                                }
                            };

                            if let Some(prev_date) = result {
                                current_date = Some(prev_date.clone());
                                needs_refresh = true;
                                tracing::info!("Navigated to previous date: {}", prev_date);
                            } else {
                                tracing::warn!("No previous date with games found");
                            }

                            // Hide loading indicator
                            if let Some(page) = &mut current_page {
                                page.hide_loading();
                            }
                            last_date_navigation = Instant::now();
                        }
                    } else if is_date_navigation_key(&key_event, false) {
                        // Shift + Right: Next date with games
                        if last_date_navigation.elapsed() >= Duration::from_millis(250) {
                            tracing::info!("Next date navigation requested");
                            tracing::debug!("Current date state: {:?}", current_date);
                            let target_date = get_target_date_for_navigation(&current_date);

                            // Show loading indicator
                            if let Some(page) = &mut current_page {
                                page.show_loading("Etsitään seuraavia otteluita...".to_string());
                                page.render_loading_indicator_only(stdout)?;
                            }

                            tracing::info!(
                                "Searching for next date with games from: {}",
                                target_date
                            );

                            // Create a task that will update animation while search runs
                            let target_date_clone = target_date.clone();
                            let mut search_task = tokio::spawn(async move {
                                find_next_date_with_games(&target_date_clone).await
                            });
                            let mut animation_interval =
                                tokio::time::interval(Duration::from_millis(200));

                            let result = loop {
                                tokio::select! {
                                    search_result = &mut search_task => {
                                        match search_result {
                                            Ok(date_option) => {
                                                break date_option;
                                            }
                                            Err(join_error) => {
                                                tracing::error!(
                                                    "Next date search task failed: {}",
                                                    join_error
                                                );
                                                break None;
                                            }
                                        }
                                    }
                                    _ = animation_interval.tick() => {
                                        if let Some(page) = &mut current_page {
                                            page.update_loading_animation();
                                            page.render_loading_indicator_only(stdout)?;
                                        }
                                    }
                                }
                            };

                            if let Some(next_date) = result {
                                current_date = Some(next_date.clone());
                                needs_refresh = true;
                                tracing::info!("Navigated to next date: {}", next_date);
                            } else {
                                tracing::warn!("No next date with games found");
                            }

                            // Hide loading indicator
                            if let Some(page) = &mut current_page {
                                page.hide_loading();
                            }
                            last_date_navigation = Instant::now();
                        }
                    } else {
                        // Handle regular key events (without modifiers)
                        match key_event.code {
                            KeyCode::Char('q') => {
                                tracing::info!("Quit requested");
                                return Ok(());
                            }
                            KeyCode::Char('r') => {
                                if last_manual_refresh.elapsed() >= Duration::from_secs(15) {
                                    tracing::info!("Manual refresh requested");
                                    needs_refresh = true;
                                    last_manual_refresh = Instant::now();
                                }
                            }
                            KeyCode::Left => {
                                if last_page_change.elapsed() >= Duration::from_millis(200) {
                                    if let Some(page) = &mut current_page {
                                        page.previous_page();
                                        needs_render = true;
                                    }
                                    last_page_change = Instant::now();
                                }
                            }
                            KeyCode::Right => {
                                if last_page_change.elapsed() >= Duration::from_millis(200) {
                                    if let Some(page) = &mut current_page {
                                        page.next_page();
                                        needs_render = true;
                                    }
                                    last_page_change = Instant::now();
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Event::Resize(_, _) => {
                    tracing::debug!("Resize event");
                    // Debounce resize events
                    if last_resize.elapsed() >= Duration::from_millis(500) {
                        resize_timer = Instant::now();
                        pending_resize = true;
                        last_resize = Instant::now();
                    }
                }
                _ => {}
            }
        }

        // Periodic cache monitoring for long-running sessions
        if cache_monitor_timer.elapsed() >= CACHE_MONITOR_INTERVAL {
            tracing::debug!("Monitoring cache usage");
            monitor_cache_usage().await;
            cache_monitor_timer = Instant::now();
        }

        // Only sleep if we're in idle mode to avoid unnecessary delays
        if poll_interval >= Duration::from_millis(200) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

/// Calculate a simple hash of game data for change detection
fn calculate_games_hash(games: &[GameData]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();

    for game in games {
        game.home_team.hash(&mut hasher);
        game.away_team.hash(&mut hasher);
        game.result.hash(&mut hasher);
        game.time.hash(&mut hasher);
        game.score_type.hash(&mut hasher);
        game.is_overtime.hash(&mut hasher);
        game.is_shootout.hash(&mut hasher);
        game.serie.hash(&mut hasher);
        game.played_time.hash(&mut hasher);
        game.start.hash(&mut hasher);

        // Hash goal events for change detection
        for goal in &game.goal_events {
            goal.scorer_player_id.hash(&mut hasher);
            goal.scorer_name.hash(&mut hasher);
            goal.minute.hash(&mut hasher);
            goal.home_team_score.hash(&mut hasher);
            goal.away_team_score.hash(&mut hasher);
            goal.is_winning_goal.hash(&mut hasher);
            goal.is_home_team.hash(&mut hasher);
            goal.goal_types.hash(&mut hasher);
        }
    }

    hasher.finish()
}

/// Monitor cache usage and log statistics for long-running sessions
async fn monitor_cache_usage() {
    // The LRU cache automatically manages memory by evicting least recently used entries
    // when it reaches capacity. We just need to log the current state for monitoring.
    use crate::data_fetcher::cache::{get_all_cache_stats, get_detailed_cache_debug_info};

    let stats = get_all_cache_stats().await;

    tracing::debug!(
        "Cache status - Player: {}/{} ({}%), Tournament: {}/{} ({}%), Detailed Game: {}/{} ({}%), Goal Events: {}/{} ({}%), HTTP Response: {}/{} ({}%)",
        stats.player_cache.size,
        stats.player_cache.capacity,
        if stats.player_cache.capacity > 0 {
            (stats.player_cache.size * 100) / stats.player_cache.capacity
        } else {
            0
        },
        stats.tournament_cache.size,
        stats.tournament_cache.capacity,
        if stats.tournament_cache.capacity > 0 {
            (stats.tournament_cache.size * 100) / stats.tournament_cache.capacity
        } else {
            0
        },
        stats.detailed_game_cache.size,
        stats.detailed_game_cache.capacity,
        if stats.detailed_game_cache.capacity > 0 {
            (stats.detailed_game_cache.size * 100) / stats.detailed_game_cache.capacity
        } else {
            0
        },
        stats.goal_events_cache.size,
        stats.goal_events_cache.capacity,
        if stats.goal_events_cache.capacity > 0 {
            (stats.goal_events_cache.size * 100) / stats.goal_events_cache.capacity
        } else {
            0
        },
        stats.http_response_cache.size,
        stats.http_response_cache.capacity,
        if stats.http_response_cache.capacity > 0 {
            (stats.http_response_cache.size * 100) / stats.http_response_cache.capacity
        } else {
            0
        }
    );

    // Get detailed debug information including goal events cache details
    let detailed_debug = get_detailed_cache_debug_info().await;
    if !detailed_debug.is_empty() {
        tracing::trace!("Detailed cache debug info: {}", detailed_debug);
    }

    // The LRU cache automatically evicts entries when it reaches capacity,
    // so we don't need manual cleanup logic anymore.
    // This ensures that the oldest/least recently used entries are always removed first.
}

/// Emergency cache management function for debugging and troubleshooting
/// This ensures cache management functions are available for diagnostic purposes
#[allow(dead_code)]
async fn emergency_cache_management() -> Result<String, AppError> {
    use crate::data_fetcher::cache::{clear_all_caches, reset_all_caches_with_confirmation};

    // In case of cache corruption or memory issues, this function provides
    // emergency cache clearing capabilities for debugging and troubleshooting
    tracing::warn!("Emergency cache clearing initiated");

    // Use the monitoring function to demonstrate comprehensive cache management
    let result = reset_all_caches_with_confirmation().await;
    tracing::info!("Emergency cache reset completed: {}", result);

    // Alternative direct clearing method
    clear_all_caches().await;
    tracing::info!("All caches cleared directly");

    Ok(result)
}
