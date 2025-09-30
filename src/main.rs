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
mod ui;
mod version;

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

    // Check for new version in the background for non-config operations
    let version_check = tokio::spawn(version::check_latest_version());

    // Load config first to fail early if there's an issue
    let _config = Config::load().await?;

    if args.once {
        return commands::handle_once_command(&args, version_check).await;
    }

    // Interactive mode
    app::run_interactive(&args, version_check).await
}
