use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::Parser;

fn get_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
        .usage(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
        .literal(AnsiColor::Green.on_default())
        .placeholder(AnsiColor::Yellow.on_default())
        .error(AnsiColor::Red.on_default().effects(Effects::BOLD))
        .valid(AnsiColor::Green.on_default())
        .invalid(AnsiColor::Red.on_default())
}

/// Determines if the application should run in non-interactive mode
/// Non-interactive mode is used when any of these conditions are met:
/// - --once flag is set (run once and exit)
/// - --compact flag is set (display games in compact format)
/// - config operations are requested
/// - --version flag is set
/// - --debug mode is enabled (debug mode always runs once and exits)
pub fn is_noninteractive_mode(args: &Args) -> bool {
    args.once
        || args.compact
        || args.new_api_domain.is_some()
        || args.new_log_file_path.is_some()
        || args.clear_log_file_path
        || args.list_config
        || args.version
        || args.debug
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
#[command(styles = get_styles())]
pub struct Args {
    /// Show scores once and exit immediately. Useful for scripts or quick score checks.
    /// The output stays visible in terminal history.
    #[arg(short, long)]
    pub once: bool,

    /// Disable clickable video links in the output.
    /// Useful for terminals that don't support links or for plain text output.
    #[arg(long = "plain", short = 'p', help_heading = "Display Options")]
    pub disable_links: bool,

    /// Display games in compact format showing only team identifiers and scores.
    /// Removes goal scorer details, timestamps, and video links for a condensed view.
    #[arg(short = 'c', long = "compact", help_heading = "Display Options")]
    pub compact: bool,

    /// Display games in wide format with two columns side by side.
    /// Shows full game details in a two-column layout when terminal width is 128+ characters.
    /// Each column displays 60 characters width with full teletext layout.
    /// Falls back to normal single-column display on narrow terminals.
    #[arg(short = 'w', long = "wide", help_heading = "Display Options")]
    pub wide: bool,

    /// Update API domain in config. Will prompt for new domain if not provided.
    #[arg(
        long = "config",
        help_heading = "Configuration",
        value_name = "API_DOMAIN",
        num_args = 0..=1,
        default_missing_value = ""
    )]
    pub new_api_domain: Option<String>,

    /// Update log file path in config. This sets a persistent custom log file location.
    #[arg(long = "set-log-file", help_heading = "Configuration")]
    pub new_log_file_path: Option<String>,

    /// Clear the custom log file path from config. This reverts to using the default log location.
    #[arg(long = "clear-log-file", help_heading = "Configuration")]
    pub clear_log_file_path: bool,

    /// List current configuration settings
    #[arg(long = "list-config", short = 'l', help_heading = "Configuration")]
    pub list_config: bool,

    /// Show games for a specific date in YYYY-MM-DD format.
    /// If not provided, shows today's or yesterday's games based on current time.
    #[arg(long = "date", short = 'd', help_heading = "Display Options")]
    pub date: Option<String>,

    /// Show version information
    #[arg(short = 'V', long = "version", help_heading = "Info")]
    pub version: bool,

    /// Enable debug mode which doesn't clear the terminal before drawing the UI.
    /// In this mode, info logs are written to the log file instead of being displayed in the terminal.
    /// The log file is created if it doesn't exist.
    #[arg(long = "debug", help_heading = "Debug")]
    pub debug: bool,

    /// Specify a custom log file path. If not provided, logs will be written to the default location.
    #[arg(long = "log-file", help_heading = "Debug")]
    pub log_file: Option<String>,

    /// Set minimum refresh interval in seconds (default: auto-detect based on game count).
    /// Higher values reduce API calls but may miss updates. Use with caution.
    #[arg(long = "min-refresh-interval", help_heading = "Display Options")]
    pub min_refresh_interval: Option<u64>,
}
