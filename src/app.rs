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
use std::io::{Stdout, stdout};

/// RAII guard for terminal state cleanup.
///
/// Ensures that the terminal is always restored to its original state,
/// even if the application panics or returns early due to an error.
struct TerminalGuard {
    stdout: Stdout,
}

impl TerminalGuard {
    /// Creates a new terminal guard after successfully enabling raw mode and alternate screen.
    fn new(stdout: Stdout) -> Self {
        Self { stdout }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Best effort cleanup - we can't return errors from Drop
        let _ = execute!(self.stdout, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

/// Run the interactive application flow.
///
/// - Sets up terminal raw mode and alternate screen
/// - Runs the interactive UI
/// - Cleans up terminal state (via RAII guard)
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

    // Create RAII guard to ensure cleanup happens even on panic or early return
    let _guard = TerminalGuard::new(out);

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

    // Terminal cleanup happens automatically when _guard is dropped

    // Show version info after UI closes if update is available
    if let Ok(Some(latest_version)) = version_check.await {
        version::print_version_info(&latest_version);
    }

    result
}
