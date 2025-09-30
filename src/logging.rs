use crate::cli::Args;
use crate::config::Config;
use crate::error::AppError;
use std::io::stdout;
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Sets up logging configuration for the application.
///
/// Configures logging based on the provided arguments and config:
/// - Interactive mode: logs only to file
/// - Once mode without debug: logs only to file
/// - Other non-interactive modes: logs to both stdout and file
/// - Creates log directory if it doesn't exist
/// - Uses daily rolling file appender
///
/// Returns the path to the log file and the guard that must be kept alive
/// for the duration of the program to ensure proper log flushing.
pub async fn setup_logging(args: &Args) -> Result<(String, WorkerGuard), AppError> {
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
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Set up the subscriber with appropriate outputs based on mode
    let registry = tracing_subscriber::registry();
    let is_noninteractive = crate::cli::is_noninteractive_mode(args);

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

    // Return the log file path and guard
    let log_file_path = format!("{log_dir}/{log_file_name}");
    Ok((log_file_path, guard))
}
