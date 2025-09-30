// src/main.rs
mod cli;
mod config;
mod constants;
mod data_fetcher;
mod error;
mod teletext_ui;
mod ui;
mod version;

use chrono::{Local, Utc};
use clap::Parser;
use cli::{Args, is_noninteractive_mode};
use config::Config;
use crossterm::{execute, style::Color, terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode}};
use data_fetcher::{fetch_liiga_data, is_historical_date};
use error::AppError;
use std::io::stdout;
use std::path::Path;
use teletext_ui::TeletextPage;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use ui::{create_future_games_page, create_page, format_date_for_display};


#[tokio::main]
async fn main() -> Result<(), AppError> {
    let args = Args::parse();

    // Validate argument combinations
    if args.compact && args.wide {
        return Err(AppError::config_error(
            "Cannot use both compact (-c) and wide (-w) modes simultaneously",
        ));
    }

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
                        .with_writer(stdout)
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

        version::print_logo();

        // Check for updates and show version info
        if let Some(latest_version) = version::check_latest_version().await {
            let current = semver::Version::parse(env!("CARGO_PKG_VERSION")).map_err(AppError::VersionParse)?;
            let latest = semver::Version::parse(&latest_version).map_err(AppError::VersionParse)?;

            if latest > current {
                version::print_version_info(&latest_version);
            } else {
                println!();
                version::print_version_status_box(vec![
                    ("Liiga Teletext Status".to_string(), None),
                    ("".to_string(), None),
                    (
                        format!("Version: {}", env!("CARGO_PKG_VERSION")),
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

        version::print_logo();
        Config::display().await?;
        return Ok(());
    }

    // Handle configuration updates
    if args.new_api_domain.is_some() || args.new_log_file_path.is_some() || args.clear_log_file_path
    {
        let mut config = Config::load().await.unwrap_or_else(|_| Config {
            api_domain: String::new(),
            log_file_path: None,
            http_timeout_seconds: crate::constants::DEFAULT_HTTP_TIMEOUT_SECONDS,
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
    let version_check = tokio::spawn(version::check_latest_version());

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
                    args.compact,
                    args.wide,
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
                args.compact,
                args.wide,
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
                args.compact,
                args.wide,
                args.once || args.compact, // suppress_countdown when once or compact mode
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
                        args.compact,
                        args.wide,
                        args.once || args.compact, // suppress_countdown when once or compact mode
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
            version::print_version_info(&latest_version);
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
        args.compact,
        args.wide,
    )
    .await;

    // Clean up terminal
    execute!(stdout, LeaveAlternateScreen)?;
    disable_raw_mode()?;

    // Show version info after UI closes if update is available
    if let Ok(Some(latest_version)) = version_check.await {
        version::print_version_info(&latest_version);
    }

    result
}
