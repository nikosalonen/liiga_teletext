use crate::cli::Args;
use crate::config::Config;
use crate::data_fetcher::{fetch_liiga_data, is_historical_date};
use crate::error::AppError;
use crate::teletext_ui::TeletextPage;
use crate::ui::{NavigationManager, format_date_for_display};
use crate::version;
use chrono::{Local, Utc};
use crossterm::{execute, style::Color, terminal::SetTitle};
use std::io::stdout;

/// Validates command line argument combinations.
///
/// Returns an error if incompatible arguments are used together.
pub fn validate_args(args: &Args) -> Result<(), AppError> {
    if args.compact && args.wide {
        return Err(AppError::config_error(
            "Cannot use both compact (-c) and wide (-w) modes simultaneously",
        ));
    }
    Ok(())
}

/// Handles the --version command.
///
/// Displays version information, logo, and checks for updates.
/// Sets appropriate terminal title and handles version comparison logic.
pub async fn handle_version_command() -> Result<(), AppError> {
    // Set terminal title for version display
    execute!(stdout(), SetTitle("SM-LIIGA 221"))?;

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

    Ok(())
}

/// Handles the --list-config command.
///
/// Displays current configuration settings with logo.
/// Sets appropriate terminal title.
pub async fn handle_list_config_command() -> Result<(), AppError> {
    // Set terminal title for config display
    execute!(stdout(), SetTitle("SM-LIIGA 221"))?;

    version::print_logo();
    Config::display().await?;
    
    Ok(())
}

/// Handles configuration update commands (--config, --set-log-file, --clear-log-file).
///
/// Updates configuration based on the provided arguments and saves changes.
/// Handles domain updates, log file path changes, and clearing log file paths.
pub async fn handle_config_update_command(args: &Args) -> Result<(), AppError> {
    let mut config = Config::load().await.unwrap_or_else(|_| Config {
        api_domain: String::new(),
        log_file_path: None,
        http_timeout_seconds: crate::constants::DEFAULT_HTTP_TIMEOUT_SECONDS,
    });

    if let Some(new_domain) = &args.new_api_domain {
        config.api_domain = new_domain.clone();
    }

    if let Some(new_log_path) = &args.new_log_file_path {
        config.log_file_path = Some(new_log_path.clone());
    } else if args.clear_log_file_path {
        config.log_file_path = None;
        println!("Custom log file path cleared. Using default location.");
    }

    config.save().await?;
    println!("Config updated successfully!");
    
    Ok(())
}

/// Handles the --once command (quick view mode).
///
/// Fetches and displays game data once, then exits.
/// Handles error cases, empty games, and different page types.
/// Shows version info after display if update is available.
pub async fn handle_once_command(args: &Args, version_check: tokio::task::JoinHandle<Option<String>>) -> Result<(), AppError> {
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
            execute!(stdout(), SetTitle("SM-LIIGA 221"))?;

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
        // Create navigation manager
        let nav_manager = NavigationManager::new();
        
        // Try to create a future games page, fall back to regular page if not future games
        // Only show future games header if no specific date was requested
        let show_future_header = args.date.is_none();
        match nav_manager.create_future_games_page(
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
                let mut page = nav_manager.create_page(
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
    execute!(stdout(), SetTitle("SM-LIIGA 221"))?;

    page.render_buffered(&mut stdout())?;
    println!(); // Add a newline at the end

    // Show version info after display if update is available
    if let Ok(Some(latest_version)) = version_check.await {
        version::print_version_info(&latest_version);
    }

    Ok(())
}