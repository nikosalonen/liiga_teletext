// src/main.rs
mod config;
mod constants;
mod data_fetcher;
mod error;
mod teletext_ui;
mod ui;

use chrono::{Local, Utc};
use clap::Parser;
use config::Config;
use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use data_fetcher::{fetch_liiga_data, is_historical_date};
use error::AppError;
use semver::Version;
use std::io::stdout;
use std::path::Path;
use teletext_ui::TeletextPage;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use ui::{create_future_games_page, create_page, format_date_for_display};

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

    /// Set minimum refresh interval in seconds (default: auto-detect based on game count).
    /// Higher values reduce API calls but may miss updates. Use with caution.
    #[arg(long = "min-refresh-interval", help_heading = "Display Options")]
    min_refresh_interval: Option<u64>,
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
        SetForegroundColor(Color::AnsiValue(231)), // Authentic teletext white
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
                        SetForegroundColor(Color::AnsiValue(231)), // Authentic teletext white
                        Print(pre),
                        SetForegroundColor(*c),
                        Print(col),
                        SetForegroundColor(Color::AnsiValue(231)), // Authentic teletext white
                        Print(format!("{:pad$} ║\n", "", pad = pad)),
                    )
                    .ok();
                } else {
                    execute!(
                        stdout(),
                        SetForegroundColor(*c),
                        Print(padded),
                        SetForegroundColor(Color::AnsiValue(231)), // Authentic teletext white
                        Print("\n")
                    )
                    .ok();
                }
            }
            None => {
                execute!(
                    stdout(),
                    SetForegroundColor(Color::AnsiValue(231)), // Authentic teletext white
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
                Some(Color::AnsiValue(231)), // Authentic teletext white
            ),
            (
                format!("Latest Version:  {latest_version}"),
                Some(Color::AnsiValue(51)), // Authentic teletext cyan
            ),
            ("".to_string(), None),
            ("Update available! Run:".to_string(), None),
            (
                "cargo install liiga_teletext".to_string(),
                Some(Color::AnsiValue(51)), // Authentic teletext cyan
            ),
        ]);
    }
}

fn print_logo() {
    execute!(
        stdout(),
        SetForegroundColor(Color::AnsiValue(51)), // Authentic teletext cyan
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
                        .with_filter(
                            EnvFilter::from_default_env()
                                .add_directive("liiga_teletext=info".parse().unwrap()),
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
                        .with_filter(
                            EnvFilter::from_default_env()
                                .add_directive("liiga_teletext=info".parse().unwrap()),
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
                    .with_filter(
                        EnvFilter::from_default_env()
                            .add_directive("liiga_teletext=info".parse().unwrap()),
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
                    (
                        format!("Version: {CURRENT_VERSION}"),
                        Some(Color::AnsiValue(231)),
                    ), // Authentic teletext white
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

                error_page.render_buffered(&mut stdout())?;
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
                None,
            )
            .await
            {
                Some(page) => page,
                None => {
                    let mut page = create_page(
                        &games,
                        args.disable_links,
                        true,
                        true,
                        Some(fetched_date.clone()),
                        None,
                    )
                    .await;

                    // Disable auto-refresh for historical dates in --once mode too
                    if let Some(ref date) = args.date {
                        if is_historical_date(date) {
                            page.set_auto_refresh_disabled(true);
                        }
                    }

                    page
                }
            }
        };

        // Set terminal title for non-interactive mode
        execute!(stdout(), crossterm::terminal::SetTitle("SM-LIIGA 221"))?;

        page.render_buffered(&mut stdout())?;
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
    let result = ui::run_interactive_ui(
        args.date.clone(),
        args.disable_links,
        args.debug,
        args.min_refresh_interval,
    )
    .await;

    // Clean up terminal
    execute!(stdout, LeaveAlternateScreen)?;
    disable_raw_mode()?;

    // Show version info after UI closes if update is available
    if let Ok(Some(latest_version)) = version_check.await {
        print_version_info(&latest_version);
    }

    result
}
