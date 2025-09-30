use crate::cli::Args;
use crate::error::AppError;
use crate::ui;
use crate::version;
use crossterm::{
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, SetTitle, disable_raw_mode, enable_raw_mode,
    },
};
use std::io::stdout;

/// Run the interactive application flow.
///
/// - Sets up terminal raw mode and alternate screen
/// - Runs the interactive UI
/// - Cleans up terminal state
/// - After exit, prints version update info if available
pub async fn run_interactive(
    args: &Args,
    version_check: tokio::task::JoinHandle<Option<String>>,
) -> Result<(), AppError> {
    // Interactive mode
    enable_raw_mode()?;
    let mut out = stdout();

    // Set terminal title/header to show app name
    execute!(out, SetTitle("SM-LIIGA 221"))?;

    execute!(out, EnterAlternateScreen)?;

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
    execute!(out, LeaveAlternateScreen)?;
    disable_raw_mode()?;

    // Show version info after UI closes if update is available
    if let Ok(Some(latest_version)) = version_check.await {
        version::print_version_info(&latest_version);
    }

    result
}
