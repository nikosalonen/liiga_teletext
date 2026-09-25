use crate::cli::Args;
use crate::config::Config;
use crate::error::AppError;
use std::io::stdout;
use std::path::{Path, PathBuf};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

const DEFAULT_LOG_FILE_NAME: &str = "liiga_teletext.log";

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

    let custom_log_path = args.log_file.as_deref().or(config_log_path.as_deref());
    let (log_dir, log_file_name) = log_location(custom_log_path, &Config::get_log_dir_path());

    // Create log directory if it doesn't exist
    if !log_dir.exists() {
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

    Ok((describe_log_path(&log_dir, &log_file_name), guard))
}

/// Splits a custom log path into its directory and file name.
/// A bare file name like `app.log` has an empty parent, which means the
/// current directory, not the filesystem root.
fn log_location(custom_path: Option<&str>, default_dir: &str) -> (PathBuf, String) {
    let Some(custom_path) = custom_path else {
        return (
            PathBuf::from(default_dir),
            DEFAULT_LOG_FILE_NAME.to_string(),
        );
    };
    let path = Path::new(custom_path);
    let dir = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(DEFAULT_LOG_FILE_NAME);
    (dir, file_name.to_string())
}

/// The daily rolling appender adds a date suffix to the file name,
/// so the reported path shows that pattern.
fn describe_log_path(log_dir: &Path, log_file_name: &str) -> String {
    format!("{}.YYYY-MM-DD", log_dir.join(log_file_name).display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_file_name_logs_to_current_directory() {
        let (dir, file_name) = log_location(Some("test12"), "/default/logs");
        assert_eq!(dir, PathBuf::from("."));
        assert_eq!(file_name, "test12");
    }

    #[test]
    fn custom_path_keeps_its_directory() {
        let (dir, file_name) = log_location(Some("/var/log/liiga/app.log"), "/default/logs");
        assert_eq!(dir, PathBuf::from("/var/log/liiga"));
        assert_eq!(file_name, "app.log");
    }

    #[test]
    fn no_custom_path_uses_default_directory() {
        let (dir, file_name) = log_location(None, "/default/logs");
        assert_eq!(dir, PathBuf::from("/default/logs"));
        assert_eq!(file_name, "liiga_teletext.log");
    }

    #[test]
    fn reported_path_names_the_daily_file_pattern() {
        let reported = describe_log_path(&PathBuf::from("/var/log/liiga"), "app.log");
        assert_eq!(
            reported,
            format!(
                "{}.YYYY-MM-DD",
                Path::new("/var/log/liiga/app.log").display()
            )
        );
    }
}
