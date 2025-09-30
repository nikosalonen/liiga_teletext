// src/main.rs
mod cli;
mod commands;
mod config;
mod constants;
mod data_fetcher;
mod error;
mod logging;
mod teletext_ui;
mod ui;
mod version;

use clap::Parser;
use cli::Args;
use config::Config;
use crossterm::{execute, terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode}};
use error::AppError;
use std::io::stdout;


#[tokio::main]
async fn main() -> Result<(), AppError> {
    let args = Args::parse();

    // Validate argument combinations
    commands::validate_args(&args)?;

    // Set up logging configuration
    let (log_file_path, _guard) = logging::setup_logging(&args).await?;
    tracing::info!("Logs are being written to: {log_file_path}");

    // Handle version flag first
    if args.version {
        return commands::handle_version_command().await;
    }

    // Handle configuration operations without version check
    if args.list_config {
        return commands::handle_list_config_command().await;
    }

    // Handle configuration updates
    if args.new_api_domain.is_some() || args.new_log_file_path.is_some() || args.clear_log_file_path
    {
        return commands::handle_config_update_command(&args).await;
    }

    // Check for new version in the background for non-config operations
    let version_check = tokio::spawn(version::check_latest_version());

    // Load config first to fail early if there's an issue
    let _config = Config::load().await?;

    if args.once {
        return commands::handle_once_command(&args, version_check).await;
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
