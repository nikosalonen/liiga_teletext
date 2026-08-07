// src/main.rs
mod app;
mod cli;
mod commands;
mod config;
mod constants;
mod data_fetcher;
mod error;
mod logging;
mod teletext_ui;
mod timezone_check;
mod ui;
mod version;

#[cfg(test)]
#[allow(dead_code)]
mod testing_utils;

use clap::Parser;
use cli::Args;
use config::Config;
use error::AppError;

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

    // Every game time is rendered through chrono::Local, which silently falls
    // back to UTC when it cannot resolve the system zone. Catch that here so a
    // whole page of times shifted by the local UTC offset is reported rather
    // than displayed as if it were correct.
    let timezone_problem = timezone_check::check();
    if let Some(problem) = &timezone_problem {
        tracing::warn!("Local timezone resolution failed: {}", problem.message());
    }

    // Check for new version in the background for non-config operations
    let version_check = tokio::spawn(version::check_latest_version());

    // Load config first to fail early if there's an issue
    let _config = Config::load().await?;

    if args.reset_cache {
        let count = data_fetcher::cache::clear_all_cache_files().await;
        println!("Cleared {count} player cache file(s).");
    }

    if args.once {
        // Safe to print now: --once never takes over the terminal.
        if let Some(problem) = &timezone_problem {
            eprintln!("WARNING: {}", problem.message());
        }
        return commands::handle_once_command(&args, version_check).await;
    }

    // Interactive mode. The warning is handed over rather than printed here
    // because the alternate screen would wipe anything written before it opens.
    app::run_interactive(&args, version_check, timezone_problem).await
}
