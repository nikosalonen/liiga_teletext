// src/teletext_ui.rs - Updated with better display formatting

use crate::config::Config;
use crate::data_fetcher::GoalEventData;
use crate::data_fetcher::api::fetch_regular_season_start_date;
use crate::error::AppError;
use chrono::{DateTime, Datelike, Local, Utc};
use crossterm::{
    cursor::MoveTo,
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use reqwest::Client;
use std::io::{Stdout, Write};
use tracing::debug;

// Constants for teletext appearance
fn header_bg() -> Color {
    Color::AnsiValue(21)
} // Bright blue
fn header_fg() -> Color {
    Color::AnsiValue(21)
} // Bright blue
fn subheader_fg() -> Color {
    Color::AnsiValue(46)
} // Bright green
fn result_fg() -> Color {
    Color::AnsiValue(46)
} // Bright green
fn text_fg() -> Color {
    Color::AnsiValue(231)
} // Pure white
fn home_scorer_fg() -> Color {
    Color::AnsiValue(51)
} // Bright cyan
fn away_scorer_fg() -> Color {
    Color::AnsiValue(51)
} // Bright cyan
fn winning_goal_fg() -> Color {
    Color::AnsiValue(201)
} // Bright magenta
fn goal_type_fg() -> Color {
    Color::AnsiValue(226)
} // Bright yellow
fn title_bg() -> Color {
    Color::AnsiValue(46)
} // Bright green

const AWAY_TEAM_OFFSET: usize = 25; // Reduced from 30 to bring teams closer
const SEPARATOR_OFFSET: usize = 23; // New constant for separator position
const CONTENT_MARGIN: usize = 2; // Small margin for game content from terminal border

/// Returns the abbreviated form of a team name for compact display.
///
/// This function maps full team names to their 3-4 character abbreviations
/// commonly used in Finnish hockey. For unknown team names, it generates
/// a fallback by taking letters only, converting to uppercase, and using
/// the first 3-4 characters.
///
/// # Arguments
/// * `team_name` - The full team name to abbreviate
///
/// # Returns
/// * `String` - The abbreviated team name
///
/// # Examples
/// ```
/// use liiga_teletext::get_team_abbreviation;
///
/// assert_eq!(get_team_abbreviation("Tappara"), "TAP");
/// assert_eq!(get_team_abbreviation("HIFK"), "IFK");
/// assert_eq!(get_team_abbreviation("HC Blues"), "HCB");
/// assert_eq!(get_team_abbreviation("K-Espoo"), "KES");
/// ```
pub fn get_team_abbreviation(team_name: &str) -> String {
    match team_name {
        // Current Liiga teams (2024-25 season)
        "Tappara" => "TAP".to_string(),
        "HIFK" => "IFK".to_string(),
        "TPS" => "TPS".to_string(),
        "JYP" => "JYP".to_string(),
        "Ilves" => "ILV".to_string(),
        "KalPa" => "KAL".to_string(),
        "Kärpät" => "KÄR".to_string(),
        "Lukko" => "LUK".to_string(),
        "Pelicans" => "PEL".to_string(),
        "SaiPa" => "SAI".to_string(),
        "Sport" => "SPO".to_string(),
        "HPK" => "HPK".to_string(),
        "Jukurit" => "JUK".to_string(),
        "Ässät" => "ÄSS".to_string(),
        "KooKoo" => "KOO".to_string(),
        "K-Espoo" => "KES".to_string(),

        // Alternative team name formats that might appear in API
        "HIFK Helsinki" => "IFK".to_string(),
        "TPS Turku" => "TPS".to_string(),
        "Tampereen Tappara" => "TAP".to_string(),
        "Tampereen Ilves" => "ILV".to_string(),
        "Jyväskylän JYP" => "JYP".to_string(),
        "Kuopion KalPa" => "KAL".to_string(),
        "Oulun Kärpät" => "KÄR".to_string(),
        "Rauman Lukko" => "LUK".to_string(),
        "Lahden Pelicans" => "PEL".to_string(),
        "Lappeenrannan SaiPa" => "SAI".to_string(),
        "Vaasan Sport" => "SPO".to_string(),
        "Hämeenlinnan HPK" => "HPK".to_string(),
        "Mikkelin Jukurit" => "JUK".to_string(),
        "Porin Ässät" => "ÄSS".to_string(),
        "Kouvolan KooKoo" => "KOO".to_string(),

        // Fallback for unknown team names - extract letters only, uppercase, take first 3-4 chars
        _ => {
            let letters_only: String = team_name
                .chars()
                .filter(|c| c.is_alphabetic())
                .collect::<String>()
                .to_uppercase();

            if letters_only.len() >= 3 {
                letters_only[..3.min(letters_only.len())].to_string()
            } else if !letters_only.is_empty() {
                letters_only
            } else {
                // If no letters found, use original team name as last resort
                team_name.to_string()
            }
        }
    }
}

/// Configuration for compact display mode layout parameters.
///
/// This struct defines the layout parameters used when rendering games
/// in compact mode, including spacing, width constraints, and formatting options.
#[derive(Debug, Clone)]
pub struct CompactDisplayConfig {
    /// Maximum number of games to display per line
    pub max_games_per_line: usize,
    /// Width allocated for team name display (e.g., "TAP-HIK")
    pub team_name_width: usize,
    /// Width allocated for score display (e.g., " 3-2 ")
    pub score_width: usize,
    /// String used to separate games on the same line
    pub game_separator: &'static str,
}

impl Default for CompactDisplayConfig {
    /// Creates a default compact display configuration optimized for multi-column layout.
    ///
    /// The default configuration supports up to 3 columns on wide terminals,
    /// falling back to 2 columns on medium terminals, and 1 column on narrow terminals.
    fn default() -> Self {
        Self {
            max_games_per_line: 3, // Up to 3 games per line for efficient space usage
            team_name_width: 8,    // "TAP-IFK" = 7 characters
            score_width: 6,        // " 3-2  " = 6 characters with padding
            game_separator: "  ",  // Two spaces between games
        }
    }
}

impl CompactDisplayConfig {
    /// Creates a new compact display configuration with custom parameters.
    ///
    /// # Arguments
    /// * `max_games_per_line` - Maximum games to show per line
    /// * `team_name_width` - Width for team name display
    /// * `score_width` - Width for score display
    /// * `game_separator` - String to separate games
    ///
    /// # Returns
    /// * `CompactDisplayConfig` - New configuration instance
    #[allow(dead_code)] // Used in tests
    pub fn new(
        max_games_per_line: usize,
        team_name_width: usize,
        score_width: usize,
        game_separator: &'static str,
    ) -> Self {
        Self {
            max_games_per_line,
            team_name_width,
            score_width,
            game_separator,
        }
    }

    /// Calculates the optimal number of games per line based on terminal width.
    ///
    /// This method adapts the display to the current terminal width while
    /// respecting the maximum games per line setting. It accounts for content
    /// margins and proper separator spacing.
    ///
    /// # Arguments
    /// * `terminal_width` - Current terminal width in characters
    ///
    /// # Returns
    /// * `usize` - Optimal number of games that can fit per line
    pub fn calculate_games_per_line(&self, terminal_width: usize) -> usize {
        if terminal_width == 0 {
            return 1;
        }

        // Account for content margins (2 chars on each side)
        let available_width = terminal_width.saturating_sub(CONTENT_MARGIN * 2);

        if available_width == 0 {
            return 1;
        }

        // Calculate space needed for one game: team names + score
        let single_game_width = self.team_name_width + self.score_width;

        // Try to fit multiple games with separators
        // For n games, we need: n * game_width + (n-1) * separator_width
        for games_count in (1..=self.max_games_per_line).rev() {
            let total_width = if games_count == 1 {
                single_game_width
            } else {
                games_count * single_game_width + (games_count - 1) * self.game_separator.len()
            };

            if total_width <= available_width {
                return games_count;
            }
        }

        // Fallback to 1 game if nothing fits
        1
    }

    /// Checks if the current terminal width can accommodate compact mode.
    ///
    /// # Arguments
    /// * `terminal_width` - Current terminal width in characters
    ///
    /// # Returns
    /// * `bool` - True if terminal is wide enough for compact mode
    #[allow(dead_code)] // Used in tests
    pub fn is_terminal_width_sufficient(&self, terminal_width: usize) -> bool {
        terminal_width >= self.get_minimum_terminal_width()
    }

    /// Gets the minimum terminal width required for compact mode (including margins)
    pub fn get_minimum_terminal_width(&self) -> usize {
        self.team_name_width + self.score_width + CONTENT_MARGIN * 2
    }

    /// Validates terminal width and returns detailed error information
    pub fn validate_terminal_width(&self, terminal_width: usize) -> TerminalWidthValidation {
        let min_width = self.get_minimum_terminal_width();

        if terminal_width < min_width {
            TerminalWidthValidation::Insufficient {
                current_width: terminal_width,
                required_width: min_width,
                shortfall: min_width - terminal_width,
            }
        } else {
            TerminalWidthValidation::Sufficient {
                current_width: terminal_width,
                required_width: min_width,
                excess: terminal_width - min_width,
            }
        }
    }
}

/// Terminal width validation result
#[derive(Debug, Clone)]
pub enum TerminalWidthValidation {
    /// Terminal width is sufficient for compact mode
    Sufficient {
        #[allow(dead_code)] // Used in tests for validation
        current_width: usize,
        #[allow(dead_code)] // Used in tests for validation
        required_width: usize,
        #[allow(dead_code)] // Used in tests for validation
        excess: usize,
    },
    /// Terminal width is insufficient for compact mode
    Insufficient {
        current_width: usize,
        required_width: usize,
        shortfall: usize,
    },
}

/// Compact mode compatibility validation result
#[derive(Debug, Clone)]
pub enum CompactModeValidation {
    /// Compact mode is fully compatible
    Compatible,
    /// Compact mode is compatible but with warnings
    CompatibleWithWarnings { warnings: Vec<String> },
    /// Compact mode is incompatible
    #[allow(dead_code)] // For future compatibility validation
    Incompatible { issues: Vec<String> },
}

/// Helper function to extract ANSI color code from crossterm Color enum.
/// Provides a fallback value for non-ANSI colors.
fn get_ansi_code(color: Color, fallback: u8) -> u8 {
    match color {
        Color::AnsiValue(val) => val,
        _ => fallback,
    }
}

/// Calculates the number of days until the regular season starts.
/// Returns None if the regular season has already started or if we can't determine the start date.
/// Uses UTC internally for consistent calculations across timezone changes.
///
/// # Arguments
/// * `client` - HTTP client for API requests
/// * `config` - Configuration containing API domain
/// * `current_year` - Optional current year (defaults to current UTC year)
async fn calculate_days_until_regular_season(
    client: &Client,
    config: &Config,
    current_year: Option<i32>,
) -> Option<i64> {
    // Use UTC for consistent year calculation, convert to local for display logic
    let current_year = current_year.unwrap_or_else(|| Utc::now().with_timezone(&Local).year());

    // Try current year first
    match fetch_regular_season_start_date(client, config, current_year).await {
        Ok(Some(start_date)) => {
            // Parse the ISO 8601 date from the API
            if let Ok(season_start) = DateTime::parse_from_rfc3339(&start_date) {
                let today = Utc::now();
                let days_until =
                    (season_start.naive_utc().date() - today.naive_utc().date()).num_days();

                if days_until > 0 {
                    return Some(days_until);
                }
            }
        }
        Ok(None) => {
            // No games found for current year, try next year
        }
        Err(_) => {
            // API call failed, we can't determine the start date
            return None;
        }
    }

    // Try next year if current year failed or no games found
    match fetch_regular_season_start_date(client, config, current_year + 1).await {
        Ok(Some(start_date)) => {
            // Parse the ISO 8601 date from the API
            if let Ok(season_start) = DateTime::parse_from_rfc3339(&start_date) {
                let today = Utc::now();
                let days_until =
                    (season_start.naive_utc().date() - today.naive_utc().date()).num_days();

                if days_until > 0 {
                    return Some(days_until);
                }
            }
        }
        Ok(None) => {
            // No games found for next year either
        }
        Err(_) => {
            // API call failed, we can't determine the start date
            return None;
        }
    }

    // If all API calls failed or no valid dates found, return None
    None
}

/// Simple ASCII loading indicator with rotating animation
#[derive(Debug, Clone)]
pub struct LoadingIndicator {
    message: String,
    frame: usize,
    frames: Vec<&'static str>,
}

impl LoadingIndicator {
    /// Creates a new loading indicator with the specified message
    pub fn new(message: String) -> Self {
        Self {
            message,
            frame: 0,
            frames: vec!["|", "/", "-", "\\"],
        }
    }

    /// Gets the current animation frame character
    pub fn current_frame(&self) -> &str {
        self.frames[self.frame]
    }

    /// Gets the loading message
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Advances to the next animation frame
    pub fn next_frame(&mut self) {
        self.frame = (self.frame + 1) % self.frames.len();
    }
}

/// Configuration for creating a TeletextPage.
/// Provides a more ergonomic API for functions with many parameters.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Used in tests
pub struct TeletextPageConfig {
    pub page_number: u16,
    pub title: String,
    pub subheader: String,
    pub disable_video_links: bool,
    pub show_footer: bool,
    pub ignore_height_limit: bool,
    pub compact_mode: bool,
    pub wide_mode: bool,
}

impl TeletextPageConfig {
    #[allow(dead_code)] // Used in tests
    pub fn new(page_number: u16, title: String, subheader: String) -> Self {
        Self {
            page_number,
            title,
            subheader,
            disable_video_links: false,
            show_footer: true,
            ignore_height_limit: false,
            compact_mode: false,
            wide_mode: false,
        }
    }

    /// Sets compact mode, automatically disabling wide mode if both were enabled.
    /// Compact mode and wide mode are mutually exclusive.
    ///
    /// # Arguments
    /// * `compact` - Whether to enable compact mode
    #[allow(dead_code)] // Used in tests
    pub fn set_compact_mode(&mut self, compact: bool) {
        self.compact_mode = compact;
        if compact && self.wide_mode {
            self.wide_mode = false;
        }
    }

    /// Sets wide mode, automatically disabling compact mode if both were enabled.
    /// Compact mode and wide mode are mutually exclusive.
    ///
    /// # Arguments
    /// * `wide` - Whether to enable wide mode
    #[allow(dead_code)] // Used in tests
    pub fn set_wide_mode(&mut self, wide: bool) {
        self.wide_mode = wide;
        if wide && self.compact_mode {
            self.compact_mode = false;
        }
    }

    /// Validates that compact mode and wide mode are not both enabled.
    /// This method should be called after manual field modifications to ensure consistency.
    ///
    /// # Returns
    /// * `Result<(), &'static str>` - Ok if valid, Err with message if invalid
    #[allow(dead_code)] // Used in tests
    pub fn validate_mode_exclusivity(&self) -> Result<(), &'static str> {
        if self.compact_mode && self.wide_mode {
            Err("compact_mode and wide_mode cannot be enabled simultaneously")
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
pub struct TeletextPage {
    page_number: u16,
    title: String,
    subheader: String,
    content_rows: Vec<TeletextRow>,
    current_page: usize,
    screen_height: u16,
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
    auto_refresh_disabled: bool,
    season_countdown: Option<String>,
    fetched_date: Option<String>, // Date for which data was fetched
    loading_indicator: Option<LoadingIndicator>,
    auto_refresh_indicator: Option<LoadingIndicator>, // Subtle indicator for auto-refresh
    error_warning_active: bool,                       // Show footer warning when true
    compact_mode: bool,                               // Enable compact display mode
    wide_mode: bool,                                  // Enable wide display mode
}

#[derive(Debug)]
pub enum TeletextRow {
    GameResult {
        home_team: String,
        away_team: String,
        time: String,
        result: String,
        score_type: ScoreType,
        is_overtime: bool,
        is_shootout: bool,
        goal_events: Vec<GoalEventData>,
        played_time: i32,
    },
    ErrorMessage(String),
    FutureGamesHeader(String), // For "Seuraavat ottelut {date}" line
}

#[derive(Debug, Clone, Hash, PartialEq)]
pub enum ScoreType {
    Final,     // Final score
    Ongoing,   // Ongoing game with current score
    Scheduled, // Scheduled game with no score yet
}

/// Represents a game result with all relevant information for display.
/// This struct acts as a data transfer object between the data fetcher and UI components.
#[derive(Debug, Clone)]
pub struct GameResultData {
    pub home_team: String,
    pub away_team: String,
    pub time: String,
    pub result: String,
    pub score_type: ScoreType,
    pub is_overtime: bool,
    pub is_shootout: bool,
    pub goal_events: Vec<GoalEventData>,
    pub played_time: i32,
}

impl GameResultData {
    /// Creates a new GameResultData instance from a GameData object.
    ///
    /// # Arguments
    /// * `game_data` - Reference to a GameData object containing raw game information
    ///
    /// # Returns
    /// * `GameResultData` - A new instance containing formatted game result data
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::GameResultData;
    /// let game_data = liiga_teletext::data_fetcher::models::GameData {
    ///     home_team: "Tappara".to_string(),
    ///     away_team: "HIFK".to_string(),
    ///     time: "18:30".to_string(),
    ///     result: "3-2".to_string(),
    ///     score_type: liiga_teletext::teletext_ui::ScoreType::Final,
    ///     is_overtime: false,
    ///     is_shootout: false,
    ///     serie: "RUNKOSARJA".to_string(),
    ///     goal_events: vec![],
    ///     played_time: 60,
    ///     start: "2024-01-15T18:30:00Z".to_string(),
    /// };
    /// let result = GameResultData::new(&game_data);
    /// ```
    pub fn new(game_data: &crate::data_fetcher::GameData) -> Self {
        Self {
            home_team: game_data.home_team.clone(),
            away_team: game_data.away_team.clone(),
            time: game_data.time.clone(),
            result: game_data.result.clone(),
            score_type: game_data.score_type.clone(),
            is_overtime: game_data.is_overtime,
            is_shootout: game_data.is_shootout,
            goal_events: game_data.goal_events.clone(),
            played_time: game_data.played_time,
        }
    }
}

impl TeletextPage {
    /// Helper function to format scorer name for display, handling empty names gracefully.
    fn format_scorer_name(scorer_name: &str) -> String {
        if scorer_name.is_empty() {
            "Maali".to_string() // "Goal" in Finnish
        } else {
            scorer_name.to_string()
        }
    }

    /// Creates a new TeletextPage instance with the specified parameters.
    ///
    /// # Arguments
    /// * `page_number` - The teletext page number (e.g., 221 for sports)
    /// * `title` - The title displayed at the top of the page
    /// * `subheader` - The subtitle displayed below the title
    /// * `disable_video_links` - Whether to disable clickable video links
    /// * `show_footer` - Whether to show the control footer
    /// * `ignore_height_limit` - Whether to ignore terminal height limits
    ///
    /// # Returns
    /// * `TeletextPage` - A new instance configured with the provided parameters
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::TeletextPage;
    ///
    /// let page = TeletextPage::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    ///     false,
    ///     true,
    ///     false,
    ///     false,
    ///     false, // wide_mode
    /// );
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        page_number: u16,
        title: String,
        subheader: String,
        disable_video_links: bool,
        show_footer: bool,
        ignore_height_limit: bool,
        compact_mode: bool,
        wide_mode: bool,
    ) -> Self {
        // Get terminal size, fallback to reasonable default if can't get size
        let screen_height = if ignore_height_limit {
            // Use a reasonable default for non-interactive mode
            24
        } else {
            crossterm::terminal::size()
                .map(|(_, height)| height)
                .unwrap_or(24)
        };

        TeletextPage {
            page_number,
            title,
            subheader,
            content_rows: Vec::new(),
            current_page: 0,
            screen_height,
            disable_video_links,
            show_footer,
            ignore_height_limit,
            auto_refresh_disabled: false,
            season_countdown: None,
            fetched_date: None,
            loading_indicator: None,
            auto_refresh_indicator: None,
            error_warning_active: false,
            compact_mode,
            wide_mode,
        }
    }

    /// Creates a new TeletextPage from a configuration struct.
    /// This provides a more ergonomic API compared to the many-parameter constructor.
    /// Validates that compact_mode and wide_mode are not both enabled.
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::{TeletextPage, TeletextPageConfig};
    ///
    /// let config = TeletextPageConfig::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    /// );
    /// let page = TeletextPage::from_config(config)?;
    /// # Ok::<(), liiga_teletext::AppError>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if both compact_mode and wide_mode are enabled in the configuration.
    #[allow(dead_code)] // Used in tests
    pub fn from_config(config: TeletextPageConfig) -> Result<Self, AppError> {
        // Validate mode exclusivity before creating the page
        if let Err(msg) = config.validate_mode_exclusivity() {
            return Err(AppError::config_error(format!(
                "Invalid TeletextPageConfig: {msg}"
            )));
        }

        Ok(Self::new(
            config.page_number,
            config.title,
            config.subheader,
            config.disable_video_links,
            config.show_footer,
            config.ignore_height_limit,
            config.compact_mode,
            config.wide_mode,
        ))
    }

    /// Updates the page layout when terminal size changes.
    /// Recalculates content positioning and pagination based on new dimensions.
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::TeletextPage;
    /// use std::io::stdout;
    ///
    /// let mut page = TeletextPage::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    ///     false,
    ///     true,
    ///     false,
    ///     false,
    ///     false, // wide_mode
    /// );
    ///
    /// // Set a fixed screen height for testing to avoid terminal size issues
    /// page.set_screen_height(20);
    ///
    /// // When terminal is resized
    /// page.handle_resize();
    ///
    /// // Only render if we have a proper terminal (skip in CI)
    /// if std::env::var("CI").is_err() {
    ///     let mut stdout = stdout();
    ///     page.render_buffered(&mut stdout)?;
    /// }
    /// # Ok::<(), liiga_teletext::AppError>(())
    /// ```
    pub fn handle_resize(&mut self) {
        // Update screen height
        if let Ok((_, height)) = crossterm::terminal::size() {
            self.screen_height = height;

            // Recalculate current page to ensure content fits
            let available_height = self.screen_height.saturating_sub(5); // Reserve space for header, subheader, and footer
            let mut current_height = 0u16;
            let mut current_page = 0;

            for game in &self.content_rows {
                let game_height = Self::calculate_game_height(game);

                if current_height + game_height > available_height {
                    current_page += 1;
                    current_height = game_height;
                } else {
                    current_height += game_height;
                }
            }

            // Ensure current_page is within bounds
            self.current_page = self.current_page.min(current_page);
        }
    }

    /// Adds a game result to the page content.
    /// The game will be displayed according to the page's current layout settings.
    ///
    /// # Arguments
    /// * `game_data` - The game result data to add to the page
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::{TeletextPage, GameResultData};
    ///
    /// let mut page = TeletextPage::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    ///     false,
    ///     true,
    ///     false,
    ///     false,
    ///     false, // wide_mode
    /// );
    ///
    /// // Create a sample game result
    /// let game = GameResultData::new(&liiga_teletext::data_fetcher::models::GameData {
    ///     home_team: "Tappara".to_string(),
    ///     away_team: "HIFK".to_string(),
    ///     time: "18:30".to_string(),
    ///     result: "3-2".to_string(),
    ///     score_type: liiga_teletext::teletext_ui::ScoreType::Final,
    ///     is_overtime: false,
    ///     is_shootout: false,
    ///     serie: "RUNKOSARJA".to_string(),
    ///     goal_events: vec![],
    ///     played_time: 60,
    ///     start: "2024-01-15T18:30:00Z".to_string(),
    /// });
    ///
    /// page.add_game_result(game);
    /// ```
    pub fn add_game_result(&mut self, game_data: GameResultData) {
        self.content_rows.push(TeletextRow::GameResult {
            home_team: game_data.home_team,
            away_team: game_data.away_team,
            time: game_data.time,
            result: game_data.result,
            score_type: game_data.score_type,
            is_overtime: game_data.is_overtime,
            is_shootout: game_data.is_shootout,
            goal_events: game_data.goal_events,
            played_time: game_data.played_time,
        });
    }

    /// Adds an error message to be displayed on the page.
    /// The message will be formatted and displayed prominently.
    ///
    /// # Arguments
    /// * `message` - The error message to display
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::TeletextPage;
    ///
    /// let mut page = TeletextPage::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    ///     false,
    ///     true,
    ///     false,
    ///     false,
    ///     false, // wide_mode
    /// );
    ///
    /// page.add_error_message("Failed to fetch game data");
    /// ```
    pub fn add_error_message(&mut self, message: &str) {
        // Split message into lines and format each line
        let formatted_message = message
            .lines()
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("\n"); // Remove the indentation
        self.content_rows
            .push(TeletextRow::ErrorMessage(formatted_message));
    }

    /// Clears all error messages from the page.
    /// This removes all ErrorMessage rows from the content, useful for preventing
    /// accumulation of repeated error messages like rate-limit notifications.
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::TeletextPage;
    ///
    /// let mut page = TeletextPage::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    ///     false,
    ///     true,
    ///     false,
    ///     false,
    ///     false, // wide_mode
    /// );
    ///
    /// page.add_error_message("Error 1");
    /// page.add_error_message("Error 2");
    /// page.clear_error_messages(); // Removes both error messages
    /// ```
    pub fn clear_error_messages(&mut self) {
        self.content_rows
            .retain(|row| !matches!(row, TeletextRow::ErrorMessage(_)));
    }

    /// Adds a header row indicating future games with the specified text.
    /// Typically used to display "Seuraavat ottelut" (Next games) with a date.
    pub fn add_future_games_header(&mut self, header_text: String) {
        self.content_rows
            .push(TeletextRow::FutureGamesHeader(header_text));
    }

    /// Sets whether auto-refresh should be disabled for this page.
    /// Useful for pages showing only future/scheduled games that don't need frequent updates.
    pub fn set_auto_refresh_disabled(&mut self, disabled: bool) {
        self.auto_refresh_disabled = disabled;
    }

    /// Gets whether auto-refresh is disabled for this page.
    /// Returns true if automatic updates are disabled.
    pub fn is_auto_refresh_disabled(&self) -> bool {
        self.auto_refresh_disabled
    }

    /// Checks if this page contains any error messages.
    /// Used to identify loading pages or error pages that need restoration.
    pub fn has_error_messages(&self) -> bool {
        self.content_rows
            .iter()
            .any(|row| matches!(row, TeletextRow::ErrorMessage(_)))
    }

    /// Sets the screen height for testing purposes.
    /// This method is primarily used in tests to avoid terminal size detection issues.
    #[allow(dead_code)] // Used in integration tests
    pub fn set_screen_height(&mut self, height: u16) {
        self.screen_height = height;
    }

    /// Sets the fetched date to display in the header.
    /// This helps users understand which date's data they're viewing.
    pub fn set_fetched_date(&mut self, date: String) {
        self.fetched_date = Some(date);
    }

    /// Shows a loading indicator with the specified message
    pub fn show_loading(&mut self, message: String) {
        self.loading_indicator = Some(LoadingIndicator::new(message));
    }

    /// Hides the loading indicator
    pub fn hide_loading(&mut self) {
        self.loading_indicator = None;
    }

    /// Updates the loading indicator animation frame
    #[allow(dead_code)] // Used in tests and future UI updates
    pub fn update_loading_animation(&mut self) {
        if let Some(ref mut indicator) = self.loading_indicator {
            indicator.next_frame();
        }
    }

    /// Shows a subtle auto-refresh indicator in the footer
    pub fn show_auto_refresh_indicator(&mut self) {
        self.auto_refresh_indicator = Some(LoadingIndicator::new("Päivitetään...".to_string()));
    }

    /// Hides the auto-refresh indicator
    pub fn hide_auto_refresh_indicator(&mut self) {
        self.auto_refresh_indicator = None;
    }

    /// Updates the auto-refresh indicator animation
    pub fn update_auto_refresh_animation(&mut self) {
        if let Some(ref mut indicator) = self.auto_refresh_indicator {
            indicator.next_frame();
        }
    }

    /// Checks if the auto-refresh indicator is active
    pub fn is_auto_refresh_indicator_active(&self) -> bool {
        self.auto_refresh_indicator.is_some()
    }

    /// Shows an error warning indicator in the footer
    pub fn show_error_warning(&mut self) {
        self.error_warning_active = true;
    }

    /// Hides the error warning indicator in the footer
    pub fn hide_error_warning(&mut self) {
        self.error_warning_active = false;
    }

    /// Returns whether the error warning indicator is active
    #[allow(dead_code)] // Reserved for future use/tests
    pub fn is_error_warning_active(&self) -> bool {
        self.error_warning_active
    }

    /// Returns whether compact mode is enabled.
    ///
    /// # Returns
    /// * `bool` - True if compact mode is enabled, false otherwise
    #[allow(dead_code)] // Used in tests
    pub fn is_compact_mode(&self) -> bool {
        self.compact_mode
    }

    /// Sets the compact mode state.
    /// Compact mode and wide mode are mutually exclusive.
    ///
    /// # Arguments
    /// * `compact` - Whether to enable compact mode
    ///
    /// # Returns
    /// * `Result<(), &'static str>` - Ok if successful, Err with message if there's a conflict
    #[allow(dead_code)] // Used in tests
    pub fn set_compact_mode(&mut self, compact: bool) -> Result<(), &'static str> {
        if compact && self.wide_mode {
            // Automatically disable wide mode
            self.wide_mode = false;
        }

        self.compact_mode = compact;
        Ok(())
    }

    /// Returns whether wide mode is enabled.
    ///
    /// # Returns
    /// * `bool` - True if wide mode is enabled, false otherwise
    #[allow(dead_code)] // Used in tests
    pub fn is_wide_mode(&self) -> bool {
        self.wide_mode
    }

    /// Test-friendly accessor to check if the page contains an error message with specific text.
    /// This method is primarily intended for testing to avoid exposing private content_rows.
    ///
    /// # Arguments
    /// * `message` - The error message text to search for
    ///
    /// # Returns
    /// * `bool` - True if an error message containing the specified text is found
    #[allow(dead_code)]
    pub fn has_error_message(&self, message: &str) -> bool {
        self.content_rows.iter().any(|row| match row {
            TeletextRow::ErrorMessage(msg) => msg.contains(message),
            _ => false,
        })
    }

    /// Sets the wide mode state.
    /// Compact mode and wide mode are mutually exclusive.
    ///
    /// # Arguments
    /// * `wide` - Whether to enable wide mode
    ///
    /// # Returns
    /// * `Result<(), &'static str>` - Ok if successful, Err with message if there's a conflict
    #[allow(dead_code)] // Used in tests
    pub fn set_wide_mode(&mut self, wide: bool) -> Result<(), &'static str> {
        if wide && self.compact_mode {
            // Automatically disable compact mode
            self.compact_mode = false;
        }

        self.wide_mode = wide;
        Ok(())
    }

    /// Validates that compact mode and wide mode are not both enabled.
    /// This method should be called after manual field modifications to ensure consistency.
    ///
    /// # Returns
    /// * `Result<(), &'static str>` - Ok if valid, Err with message if invalid
    #[allow(dead_code)] // Used in tests
    pub fn validate_mode_exclusivity(&self) -> Result<(), &'static str> {
        if self.compact_mode && self.wide_mode {
            Err("compact_mode and wide_mode cannot be enabled simultaneously")
        } else {
            Ok(())
        }
    }

    /// Checks if the terminal width is sufficient for wide mode display.
    /// Wide mode requires at least 100 characters to display two full-width columns effectively.
    ///
    /// # Returns
    /// * `bool` - True if terminal width supports wide mode, false otherwise
    pub fn can_fit_two_pages(&self) -> bool {
        if !self.wide_mode {
            return false;
        }

        // Get terminal width, fallback to reasonable default if can't get size
        let terminal_width = if self.ignore_height_limit {
            // In non-interactive mode, use appropriate width for wide mode
            if self.wide_mode { 136 } else { 80 }
        } else {
            crossterm::terminal::size()
                .map(|(width, _)| width as usize)
                .unwrap_or(80)
        };

        // Wide mode requires minimum width for two normal-width columns plus gap
        // Each column: 60 chars, gap: 8 chars, margins: 4 chars = 128 chars total
        terminal_width >= 128
    }

    /// Distributes games between left and right columns for wide mode display.
    /// Uses left-column-first filling logic similar to pagination.
    ///
    /// # Returns
    /// * `(Vec<&TeletextRow>, Vec<&TeletextRow>)` - Left and right column games
    pub fn distribute_games_for_wide_display(&self) -> (Vec<&TeletextRow>, Vec<&TeletextRow>) {
        if !self.wide_mode || !self.can_fit_two_pages() {
            // If not in wide mode or can't fit two columns, return all games in left column
            let all_games: Vec<&TeletextRow> = self.content_rows.iter().collect();
            return (all_games, Vec::new());
        }

        let (visible_rows, _) = self.get_page_content();
        if visible_rows.is_empty() {
            return (Vec::new(), Vec::new());
        }

        // Split games roughly evenly between columns using balanced distribution
        // Left column gets the extra game if there's an odd number
        let total_games = visible_rows.len();
        let games_per_column = total_games.div_ceil(2);

        let mut left_games: Vec<&TeletextRow> = Vec::new();
        let mut right_games: Vec<&TeletextRow> = Vec::new();

        for (i, game) in visible_rows.iter().enumerate() {
            if i < games_per_column {
                left_games.push(game);
            } else {
                right_games.push(game);
            }
        }

        (left_games, right_games)
    }

    /// Renders content in wide mode with two columns.
    /// Handles header/footer spanning full width and two-column layout rendering.
    ///
    /// # Arguments
    /// * `buffer` - The string buffer to append rendered content to
    /// * `visible_rows` - The rows to render
    /// * `width` - Terminal width
    /// * `current_line` - Current line position (mutable reference)
    /// * `text_fg_code` - Text foreground color code
    /// * `subheader_fg_code` - Subheader foreground color code
    fn render_wide_mode_content(
        &self,
        buffer: &mut String,
        visible_rows: &[&TeletextRow],
        width: u16,
        current_line: &mut usize,
        text_fg_code: u8,
        subheader_fg_code: u8,
    ) {
        // Check if we can actually fit two columns
        if !self.can_fit_two_pages() {
            // Show warning about insufficient width
            let required_width: usize = 122;
            let current_width: usize = width as usize;
            let shortfall = required_width.saturating_sub(current_width);

            let warning_message = format!(
                "Terminal too narrow for wide mode ({current_width} chars, need {required_width} chars, short {shortfall} chars)"
            );

            buffer.push_str(&format!(
                "\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                *current_line,
                CONTENT_MARGIN + 1,
                text_fg_code,
                warning_message
            ));
            *current_line += 1;

            // Add suggestion for minimum terminal width
            buffer.push_str(&format!(
                "\x1b[{};{}H\x1b[38;5;{}mResize terminal to at least {} characters wide for wide mode\x1b[0m",
                *current_line,
                CONTENT_MARGIN + 1,
                text_fg_code,
                required_width
            ));
            *current_line += 1;

            // Fallback to normal rendering
            self.render_normal_content(
                buffer,
                visible_rows,
                width,
                current_line,
                text_fg_code,
                subheader_fg_code,
            );
            return;
        }

        // Distribute visible rows between left and right columns using the shared distribution logic
        let (left_games, right_games) = self.distribute_games_for_wide_display();

        // Calculate column widths for wide mode - based on normal mode layout
        let left_column_start = 2;
        let gap_between_columns = 8; // Good separation between columns

        // Each column should accommodate the normal teletext layout with extra width
        // Normal mode uses positions up to ~55 chars, but we can make columns wider for better readability
        let normal_content_width = 60; // Wider columns for better spacing and readability
        let column_width = normal_content_width;

        // Render left column
        let mut left_line = *current_line;

        for (game_index, game) in left_games.iter().enumerate() {
            let formatted_game = self.format_game_for_wide_column(game, column_width);
            let lines: Vec<&str> = formatted_game.lines().collect();

            for (line_index, line) in lines.iter().enumerate() {
                buffer.push_str(&format!(
                    "\x1b[{};{}H{}",
                    left_line + line_index,
                    left_column_start,
                    line
                ));
            }
            left_line += lines.len();

            // Add spacing between games (except after the last game)
            if game_index < left_games.len() - 1 {
                left_line += 1; // Extra blank line between games
            }
        }

        // Render right column
        let right_column_start = left_column_start + column_width + gap_between_columns;
        let mut right_line = *current_line;

        for (game_index, game) in right_games.iter().enumerate() {
            let formatted_game = self.format_game_for_wide_column(game, column_width);
            let lines: Vec<&str> = formatted_game.lines().collect();

            for (line_index, line) in lines.iter().enumerate() {
                buffer.push_str(&format!(
                    "\x1b[{};{}H{}",
                    right_line + line_index,
                    right_column_start,
                    line
                ));
            }
            right_line += lines.len();

            // Add spacing between games (except after the last game)
            if game_index < right_games.len() - 1 {
                right_line += 1; // Extra blank line between games
            }
        }

        // Update current line to the maximum of left and right column heights
        *current_line = left_line.max(right_line);
    }

    /// Renders content in normal mode (fallback for wide mode when width insufficient).
    ///
    /// # Arguments
    /// * `buffer` - The string buffer to append rendered content to
    /// * `visible_rows` - The rows to render
    /// * `width` - Terminal width
    /// * `current_line` - Current line position (mutable reference)
    /// * `text_fg_code` - Text foreground color code
    /// * `subheader_fg_code` - Subheader foreground color code
    fn render_normal_content(
        &self,
        buffer: &mut String,
        visible_rows: &[&TeletextRow],
        width: u16,
        current_line: &mut usize,
        text_fg_code: u8,
        subheader_fg_code: u8,
    ) {
        for row in visible_rows {
            match row {
                TeletextRow::GameResult {
                    home_team,
                    away_team,
                    time,
                    result,
                    score_type,
                    is_overtime,
                    is_shootout,
                    goal_events,
                    played_time,
                } => {
                    // Format result with overtime/shootout indicator
                    let result_text = if *is_shootout {
                        format!("{result} rl")
                    } else if *is_overtime {
                        format!("{result} ja")
                    } else {
                        result.clone()
                    };

                    // Format time display based on game state
                    let (time_display, score_display) = match score_type {
                        ScoreType::Scheduled => (time.clone(), String::new()),
                        ScoreType::Ongoing => {
                            let formatted_time =
                                format!("{:02}:{:02}", played_time / 60, played_time % 60);
                            (formatted_time, result_text.clone())
                        }
                        ScoreType::Final => (String::new(), result_text.clone()),
                    };

                    let result_color = match score_type {
                        ScoreType::Final => get_ansi_code(result_fg(), 46),
                        _ => text_fg_code,
                    };

                    // Build game line with flexible positioning based on terminal width
                    let available_width = width as usize - (CONTENT_MARGIN * 2);
                    let home_team_width = (available_width * 3) / 8; // 3/8 of available width
                    let away_team_width = (available_width * 3) / 8; // 3/8 of available width
                    let time_score_width = available_width - home_team_width - away_team_width - 3; // Remaining space minus separator

                    let home_pos = CONTENT_MARGIN + 1;
                    let separator_pos = home_pos + home_team_width;
                    let away_pos = separator_pos + 3; // 3 chars for " - "
                    let time_score_pos = away_pos + away_team_width;

                    if !time_display.is_empty() && !score_display.is_empty() {
                        // For ongoing games: show time on the left, score on the right
                        let home_team_text =
                            home_team.chars().take(home_team_width).collect::<String>();
                        let away_team_text =
                            away_team.chars().take(away_team_width).collect::<String>();

                        buffer.push_str(&format!(
                                "\x1b[{};{}H\x1b[38;5;{}m{:<home_width$}\x1b[{};{}H\x1b[38;5;{}m- \x1b[{};{}H\x1b[38;5;{}m{:<away_width$}\x1b[{};{}H\x1b[38;5;{}m{:<time_width$}\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                                *current_line, home_pos,
                                text_fg_code,
                                home_team_text,
                                *current_line, separator_pos,
                                text_fg_code,
                                *current_line, away_pos,
                                text_fg_code,
                                away_team_text,
                                *current_line, time_score_pos,
                                text_fg_code,
                                time_display,
                                *current_line, time_score_pos + time_display.len(),
                                result_color,
                                score_display,
                                home_width = home_team_width,
                                away_width = away_team_width,
                                time_width = time_score_width
                            ));
                    } else {
                        // For scheduled/final games: show time or score on the right
                        let display_text = if !time_display.is_empty() {
                            time_display
                        } else {
                            score_display
                        };
                        let home_team_text =
                            home_team.chars().take(home_team_width).collect::<String>();
                        let away_team_text =
                            away_team.chars().take(away_team_width).collect::<String>();

                        buffer.push_str(&format!(
                                "\x1b[{};{}H\x1b[38;5;{}m{:<home_width$}\x1b[{};{}H\x1b[38;5;{}m- \x1b[{};{}H\x1b[38;5;{}m{:<away_width$}\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                                *current_line, home_pos,
                                text_fg_code,
                                home_team_text,
                                *current_line, separator_pos,
                                text_fg_code,
                                *current_line, away_pos,
                                text_fg_code,
                                away_team_text,
                                *current_line, time_score_pos,
                                result_color,
                                display_text,
                                home_width = home_team_width,
                                away_width = away_team_width
                            ));
                    }

                    *current_line += 1;

                    // Add goal events for finished/ongoing games
                    if matches!(score_type, ScoreType::Ongoing | ScoreType::Final)
                        && !goal_events.is_empty()
                    {
                        let home_scorer_fg_code = get_ansi_code(home_scorer_fg(), 51);
                        let away_scorer_fg_code = get_ansi_code(away_scorer_fg(), 51);
                        let winning_goal_fg_code = get_ansi_code(winning_goal_fg(), 201);
                        let goal_type_fg_code = get_ansi_code(goal_type_fg(), 226);

                        let home_scorers: Vec<_> =
                            goal_events.iter().filter(|e| e.is_home_team).collect();
                        let away_scorers: Vec<_> =
                            goal_events.iter().filter(|e| !e.is_home_team).collect();
                        let max_scorers = home_scorers.len().max(away_scorers.len());

                        for i in 0..max_scorers {
                            // Home team scorer
                            if let Some(event) = home_scorers.get(i) {
                                let scorer_color = if (event.is_winning_goal
                                    && (*is_overtime || *is_shootout))
                                    || event.goal_types.contains(&"VL".to_string())
                                {
                                    winning_goal_fg_code
                                } else {
                                    home_scorer_fg_code
                                };

                                buffer.push_str(&format!(
                                    "\x1b[{};{}H\x1b[38;5;{}m{:2} ",
                                    *current_line, home_pos, scorer_color, event.minute
                                ));

                                // Add video link functionality if there's a video clip and links are enabled
                                let display_name = Self::format_scorer_name(&event.scorer_name);
                                if let Some(url) = &event.video_clip_url {
                                    if !self.disable_video_links {
                                        buffer.push_str(&format!(
                                            "\x1b[38;5;{}m{:<12}\x1B]8;;{}\x07▶\x1B]8;;\x07",
                                            scorer_color, display_name, url
                                        ));
                                    } else {
                                        buffer.push_str(&format!(
                                            "\x1b[38;5;{}m{:<12}",
                                            scorer_color, display_name
                                        ));
                                    }
                                } else {
                                    buffer.push_str(&format!(
                                        "\x1b[38;5;{}m{:<12}",
                                        scorer_color, display_name
                                    ));
                                }

                                // Add goal type indicators
                                let goal_type = event.get_goal_type_display();
                                if !goal_type.is_empty() {
                                    buffer.push_str(&format!(
                                        " \x1b[38;5;{goal_type_fg_code}m{goal_type}\x1b[0m"
                                    ));
                                } else {
                                    buffer.push_str("\x1b[0m");
                                }
                            }

                            // Away team scorer
                            if let Some(event) = away_scorers.get(i) {
                                let scorer_color = if (event.is_winning_goal
                                    && (*is_overtime || *is_shootout))
                                    || event.goal_types.contains(&"VL".to_string())
                                {
                                    winning_goal_fg_code
                                } else {
                                    away_scorer_fg_code
                                };

                                buffer.push_str(&format!(
                                    "\x1b[{};{}H\x1b[38;5;{}m{:2} ",
                                    *current_line, away_pos, scorer_color, event.minute
                                ));

                                // Add video link functionality if there's a video clip and links are enabled
                                let display_name = Self::format_scorer_name(&event.scorer_name);
                                if let Some(url) = &event.video_clip_url {
                                    if !self.disable_video_links {
                                        buffer.push_str(&format!(
                                            "\x1b[38;5;{}m{:<12}\x1B]8;;{}\x07▶\x1B]8;;\x07",
                                            scorer_color, display_name, url
                                        ));
                                    } else {
                                        buffer.push_str(&format!(
                                            "\x1b[38;5;{}m{:<12}",
                                            scorer_color, display_name
                                        ));
                                    }
                                } else {
                                    buffer.push_str(&format!(
                                        "\x1b[38;5;{}m{:<12}",
                                        scorer_color, display_name
                                    ));
                                }

                                // Add goal type indicators
                                let goal_type = event.get_goal_type_display();
                                if !goal_type.is_empty() {
                                    buffer.push_str(&format!(
                                        " \x1b[38;5;{goal_type_fg_code}m{goal_type}\x1b[0m"
                                    ));
                                } else {
                                    buffer.push_str("\x1b[0m");
                                }
                            }

                            if home_scorers.get(i).is_some() || away_scorers.get(i).is_some() {
                                *current_line += 1;
                            }
                        }
                    }

                    // Add spacing between games in interactive mode
                    if !self.ignore_height_limit {
                        *current_line += 1;
                    }
                }
                TeletextRow::ErrorMessage(message) => {
                    for line in message.lines() {
                        buffer.push_str(&format!(
                            "\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                            *current_line,
                            CONTENT_MARGIN + 1,
                            text_fg_code,
                            line
                        ));
                        *current_line += 1;
                    }
                }
                TeletextRow::FutureGamesHeader(header_text) => {
                    buffer.push_str(&format!(
                        "\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                        *current_line,
                        CONTENT_MARGIN + 1,
                        subheader_fg_code,
                        header_text
                    ));
                    *current_line += 1;
                }
            }
        }
    }

    /// Formats a single game for wide column display.
    /// Preserves all game details while constraining output to column width.
    ///
    /// # Arguments
    /// * `text` - String that may contain ANSI escape sequences
    ///
    /// # Returns
    /// * `usize` - Number of visible characters (excluding ANSI sequences)
    fn count_visible_chars(text: &str) -> usize {
        let mut visible_len = 0;
        let mut in_ansi = false;
        for c in text.chars() {
            if c == '\x1b' {
                in_ansi = true;
            } else if in_ansi && c == 'm' {
                in_ansi = false;
            } else if !in_ansi {
                visible_len += 1;
            }
        }
        visible_len
    }

    /// Truncates team names gracefully, preferring word boundaries when possible.
    ///
    /// # Arguments
    /// * `team_name` - Original team name
    /// * `max_length` - Maximum allowed length
    ///
    /// # Returns
    /// * `String` - Truncated team name
    fn truncate_team_name_gracefully(team_name: &str, max_length: usize) -> String {
        if team_name.len() <= max_length {
            return team_name.to_string();
        }

        // Try to find a good truncation point (space, hyphen, or vowel)
        let mut best_pos = max_length;
        for (i, c) in team_name.char_indices().take(max_length) {
            if c == ' ' || c == '-' {
                best_pos = i;
                break;
            }
        }

        team_name.chars().take(best_pos).collect()
    }

    /// Formats a game for display in a wide column with specified width constraints.
    /// Optimized for performance with pre-allocated buffers and reasonable goal limits.
    ///
    /// # Arguments
    /// * `game` - The game result to format
    /// * `column_width` - Maximum width for the column
    ///
    /// # Returns
    /// * `String` - Formatted game string for wide column display with ANSI color codes
    fn format_game_for_wide_column(&self, game: &TeletextRow, column_width: usize) -> String {
        match game {
            TeletextRow::GameResult {
                home_team,
                away_team,
                time,
                result,
                score_type,
                is_overtime,
                is_shootout,
                goal_events,
                played_time,
                ..
            } => {
                let text_fg_code = get_ansi_code(text_fg(), 231);
                let result_fg_code = get_ansi_code(result_fg(), 46);
                let home_scorer_fg_code = get_ansi_code(home_scorer_fg(), 51);
                let away_scorer_fg_code = get_ansi_code(away_scorer_fg(), 51);
                let winning_goal_fg_code = get_ansi_code(winning_goal_fg(), 201);
                let goal_type_fg_code = get_ansi_code(goal_type_fg(), 226);

                // Format the main game line
                // Pre-allocate lines vector with estimated capacity (1 team line + potential goal lines)
                let estimated_goals = goal_events.len().min(30); // Cap estimate at 30 total goals
                let mut lines = Vec::with_capacity(1 + estimated_goals);

                // Team names and score line using proper teletext layout within column
                let team_score_line = {
                    // Format result with overtime/shootout indicator
                    let result_text = if *is_shootout {
                        format!("{result} rl")
                    } else if *is_overtime {
                        format!("{result} ja")
                    } else {
                        result.clone()
                    };

                    // Format time display based on game state
                    let (time_display, score_display) = match score_type {
                        ScoreType::Scheduled => (time.clone(), String::new()),
                        ScoreType::Ongoing => {
                            let formatted_time =
                                format!("{:02}:{:02}", played_time / 60, played_time % 60);
                            (formatted_time, result_text.clone())
                        }
                        ScoreType::Final => (String::new(), result_text.clone()),
                    };

                    let result_color = match score_type {
                        ScoreType::Final => result_fg_code,
                        _ => text_fg_code,
                    };

                    // Use proportional spacing within the 48-character column width
                    let display_text = if !time_display.is_empty() && !score_display.is_empty() {
                        // For ongoing games: show both time and score
                        format!("{time_display} {score_display}")
                    } else if !time_display.is_empty() {
                        time_display
                    } else {
                        score_display
                    };

                    // Format with fixed character positions - away team always starts at position 27
                    let mut line = String::new();

                    // Position 0-19: Home team (up to 20 chars) - use graceful truncation
                    let home_text = Self::truncate_team_name_gracefully(home_team, 20);
                    line.push_str(&format!("{home_text:<20}"));

                    // Position 20-26: Spacing and dash (7 chars total)
                    line.push_str("    - ");

                    // Position 27+: Away team (up to 17 chars) - use graceful truncation
                    let away_text = Self::truncate_team_name_gracefully(away_team, 17);
                    line.push_str(&format!("{away_text:<20}"));

                    // Score section
                    line.push_str(&format!(" \x1b[38;5;{result_color}m{display_text}\x1b[0m"));

                    format!("\x1b[38;5;{text_fg_code}m{line}\x1b[0m")
                };

                lines.push(team_score_line);

                // Goal events - position scorers under their respective teams like normal mode
                // Limit goal scorers for performance (max 15 per team to prevent excessive rendering)
                if !goal_events.is_empty() {
                    const MAX_SCORERS_PER_TEAM: usize = 15;

                    let home_scorers: Vec<_> = goal_events
                        .iter()
                        .filter(|e| e.is_home_team)
                        .take(MAX_SCORERS_PER_TEAM)
                        .collect();
                    let away_scorers: Vec<_> = goal_events
                        .iter()
                        .filter(|e| !e.is_home_team)
                        .take(MAX_SCORERS_PER_TEAM)
                        .collect();
                    let max_scorers = home_scorers.len().max(away_scorers.len());

                    // Pre-allocate lines vector with estimated capacity
                    lines.reserve(max_scorers + 1);

                    for i in 0..max_scorers {
                        let mut scorer_line = String::new();

                        // Build home side (always exactly 22 characters total)
                        let home_side = if let Some(event) = home_scorers.get(i) {
                            let scorer_color = if (event.is_winning_goal
                                && (*is_overtime || *is_shootout))
                                || event.goal_types.contains(&"VL".to_string())
                            {
                                winning_goal_fg_code // Purple for game-winning goals only
                            } else {
                                home_scorer_fg_code // Regular home team color
                            };

                            let goal_type = event.get_goal_type_display();
                            let goal_type_str = if !goal_type.is_empty() {
                                format!(" \x1b[38;5;{goal_type_fg_code}m{goal_type}\x1b[0m")
                            } else {
                                String::new()
                            };

                            format!(
                                " \x1b[38;5;{}m{:2} {:<12}\x1b[0m{}",
                                scorer_color,
                                event.minute,
                                Self::format_scorer_name(&event.scorer_name)
                                    .chars()
                                    .take(12)
                                    .collect::<String>(),
                                goal_type_str
                            )
                        } else {
                            String::new()
                        };

                        // Create away scorer content
                        let away_content = if let Some(event) = away_scorers.get(i) {
                            let scorer_color = if (event.is_winning_goal
                                && (*is_overtime || *is_shootout))
                                || event.goal_types.contains(&"VL".to_string())
                            {
                                winning_goal_fg_code // Purple for game-winning goals only
                            } else {
                                away_scorer_fg_code // Regular away team color
                            };

                            let goal_type = event.get_goal_type_display();
                            let goal_type_str = if !goal_type.is_empty() {
                                format!(" \x1b[38;5;{goal_type_fg_code}m{goal_type}\x1b[0m")
                            } else {
                                String::new()
                            };

                            format!(
                                "\x1b[38;5;{}m{:2} {:<12}\x1b[0m{}",
                                scorer_color,
                                event.minute,
                                Self::format_scorer_name(&event.scorer_name)
                                    .chars()
                                    .take(12)
                                    .collect::<String>(),
                                goal_type_str
                            )
                        } else {
                            String::new()
                        };

                        // Format complete line with ANSI-aware padding using optimized helper
                        let visible_len = Self::count_visible_chars(&home_side);

                        // Pre-allocate scorer_line with estimated capacity
                        scorer_line.reserve(60);

                        // Build line: home_side + padding to reach 27 chars + away_content
                        scorer_line.push_str(&home_side);
                        for _ in visible_len..27 {
                            scorer_line.push(' ');
                        }
                        scorer_line.push_str(&away_content);

                        lines.push(scorer_line);
                    }
                }

                // Join lines
                lines.join("\n")
            }
            TeletextRow::ErrorMessage(message) => {
                let text_fg_code = get_ansi_code(text_fg(), 231);
                format!("\x1b[38;5;{text_fg_code}m{message}\x1b[0m")
            }
            TeletextRow::FutureGamesHeader(header_text) => {
                let subheader_fg_code = get_ansi_code(subheader_fg(), 46);
                let formatted = format!("\x1b[38;5;{subheader_fg_code}m{header_text}\x1b[0m");

                if formatted.len() > column_width {
                    let truncated = &formatted[..column_width];
                    format!("{truncated}...")
                } else {
                    formatted
                }
            }
        }
    }

    /// Formats a single game in compact mode with proper teletext colors.
    ///
    /// # Arguments
    /// * `game` - The game result to format
    /// * `config` - Compact display configuration
    ///
    /// # Returns
    /// * `String` - Formatted game string for compact display with ANSI color codes
    fn format_compact_game(&self, game: &TeletextRow, config: &CompactDisplayConfig) -> String {
        match game {
            TeletextRow::GameResult {
                home_team,
                away_team,
                time,
                result,
                score_type,
                is_overtime,
                is_shootout,
                ..
            } => {
                let text_fg_code = get_ansi_code(text_fg(), 231);
                let result_fg_code = get_ansi_code(result_fg(), 46);

                // Use team abbreviations
                let home_abbr = get_team_abbreviation(home_team);
                let away_abbr = get_team_abbreviation(away_team);

                // Format team names with proper width and teletext white color
                let team_display = format!("{home_abbr}-{away_abbr}");
                let padded_team = format!(
                    "\x1b[38;5;{text_fg_code}m{:<width$}\x1b[0m",
                    team_display,
                    width = config.team_name_width
                );

                // Format score based on game state with appropriate colors
                let score_display = match score_type {
                    ScoreType::Scheduled => {
                        // Scheduled games show time in white
                        format!(
                            "\x1b[38;5;{text_fg_code}m{:<width$}\x1b[0m",
                            time,
                            width = config.score_width
                        )
                    }
                    ScoreType::Ongoing => {
                        // Ongoing games show score in white (like regular mode)
                        let mut score = result.clone();
                        if *is_shootout {
                            score.push_str(" rl");
                        } else if *is_overtime {
                            score.push_str(" ja");
                        }
                        format!(
                            "\x1b[38;5;{text_fg_code}m{:<width$}\x1b[0m",
                            score,
                            width = config.score_width
                        )
                    }
                    ScoreType::Final => {
                        // Final games show score in bright green (like regular mode)
                        let mut score = result.clone();
                        if *is_shootout {
                            score.push_str(" rl");
                        } else if *is_overtime {
                            score.push_str(" ja");
                        }
                        format!(
                            "\x1b[38;5;{result_fg_code}m{:<width$}\x1b[0m",
                            score,
                            width = config.score_width
                        )
                    }
                };

                format!("{padded_team}{score_display}")
            }
            TeletextRow::FutureGamesHeader(header_text) => {
                let subheader_fg_code = get_ansi_code(subheader_fg(), 46);

                // Format future games header for compact mode - intelligently abbreviate to preserve date
                let abbreviated_header = if header_text.starts_with("Seuraavat ottelut ") {
                    // Special handling for "Seuraavat ottelut DD.MM." - abbreviate "Seuraavat" to preserve date
                    header_text.replace("Seuraavat ottelut ", "Seur. ottelut ")
                } else if header_text.len() > 30 {
                    // For other long headers, truncate at 30 characters (increased from 22)
                    format!("{}...", &header_text[..30])
                } else {
                    header_text.clone()
                };
                format!("\x1b[38;5;{subheader_fg_code}m>>> {abbreviated_header}\x1b[0m")
            }
            _ => String::new(),
        }
    }

    /// Groups rows into lines for compact display.
    ///
    /// # Arguments
    /// * `rows` - List of rows to group
    /// * `config` - Compact display configuration
    /// * `terminal_width` - Current terminal width
    ///
    /// # Returns
    /// * `Vec<String>` - Lines of formatted content
    fn group_games_for_compact_display(
        &self,
        rows: &[&TeletextRow],
        config: &CompactDisplayConfig,
        terminal_width: usize,
    ) -> Vec<String> {
        let games_per_line = config.calculate_games_per_line(terminal_width);
        let mut lines = Vec::new();
        let mut current_line = String::new();
        let mut games_in_current_line = 0;

        for row in rows.iter() {
            let row_str = self.format_compact_game(row, config);

            // Skip empty strings (unsupported row types)
            if row_str.is_empty() {
                continue;
            }

            // Handle headers as separate lines
            if matches!(row, TeletextRow::FutureGamesHeader(_)) {
                // Finish current game line if not empty
                if !current_line.is_empty() {
                    lines.push(current_line.clone());
                    current_line.clear();
                    games_in_current_line = 0;
                }
                // Add header as its own line
                lines.push(row_str);
                continue;
            }

            // Handle games
            if current_line.is_empty() {
                current_line = row_str;
                games_in_current_line = 1;
            } else {
                current_line.push_str(config.game_separator);
                current_line.push_str(&row_str);
                games_in_current_line += 1;
            }

            // Start new line if we've reached the limit
            if games_in_current_line >= games_per_line {
                lines.push(current_line.clone());
                // Add empty line after each group of games for better readability
                lines.push(String::new());
                current_line.clear();
                games_in_current_line = 0;
            }
        }

        // Add remaining games if any
        if !current_line.is_empty() {
            lines.push(current_line);
            // Add empty line after the last group as well
            lines.push(String::new());
        }

        // Remove the final empty line if there are any lines (to avoid trailing empty space)
        if !lines.is_empty() && lines.last() == Some(&String::new()) {
            lines.pop();
        }

        lines
    }

    /// Calculates the optimal number of games per line for the current terminal width.
    ///
    /// # Arguments
    /// * `terminal_width` - Current terminal width in characters
    ///
    /// # Returns
    /// * `usize` - Optimal number of games per line
    #[cfg(test)]
    fn calculate_compact_games_per_line(&self, terminal_width: usize) -> usize {
        let config = CompactDisplayConfig::default();
        config.calculate_games_per_line(terminal_width)
    }

    /// Checks if the current terminal width can accommodate compact mode.
    ///
    /// # Arguments
    /// * `terminal_width` - Current terminal width in characters
    ///
    /// # Returns
    /// * `bool` - True if terminal is wide enough for compact mode
    #[cfg(test)]
    fn is_terminal_suitable_for_compact(&self, terminal_width: usize) -> bool {
        let config = CompactDisplayConfig::default();
        config.is_terminal_width_sufficient(terminal_width)
    }

    /// Validates compact mode compatibility with current page settings
    ///
    /// # Returns
    /// * `CompactModeValidation` - Validation result with any issues found
    pub fn validate_compact_mode_compatibility(&self) -> CompactModeValidation {
        let issues: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        // Error messages are now properly handled in compact mode
        // No need for warning anymore

        // Loading indicators and auto-refresh indicators work fine in compact mode
        // No need for warnings anymore

        // Season countdown is now properly handled - suppressed when compact mode is enabled
        // No need for warning anymore

        // Future games headers are now properly supported in compact mode
        // No need for warning anymore

        // Check if we have many games (compact mode might be crowded)
        let game_count = self
            .content_rows
            .iter()
            .filter(|row| matches!(row, TeletextRow::GameResult { .. }))
            .count();

        if game_count > 20 {
            warnings.push("Many games detected - compact mode may be crowded".to_string());
        }

        if issues.is_empty() && warnings.is_empty() {
            CompactModeValidation::Compatible
        } else {
            CompactModeValidation::CompatibleWithWarnings { warnings }
        }
    }

    /// Renders only the loading indicator area without redrawing the entire screen
    #[allow(dead_code)] // Method for future use
    pub fn render_loading_indicator_only(&self, stdout: &mut Stdout) -> Result<(), AppError> {
        if !self.show_footer {
            return Ok(());
        }

        let (width, _) = crossterm::terminal::size()?;
        let footer_y = if self.ignore_height_limit {
            // In --once mode, we don't update loading indicators
            return Ok(());
        } else {
            // In interactive mode, position footer at bottom of screen
            self.screen_height.saturating_sub(1)
        };
        let empty_y = footer_y.saturating_sub(1);

        // Clear the loading indicator line first
        execute!(
            stdout,
            MoveTo(0, empty_y),
            Print(" ".repeat(width as usize))
        )?;

        // Show loading indicator if active
        if let Some(ref loading) = self.loading_indicator {
            let loading_text = format!("{} {}", loading.current_frame(), loading.message());
            let loading_width = loading_text.chars().count();
            let left_padding = if width as usize > loading_width {
                (width as usize - loading_width) / 2
            } else {
                0
            };
            execute!(
                stdout,
                MoveTo(0, empty_y),
                SetForegroundColor(goal_type_fg()), // Use existing color function for consistency
                Print(format!(
                    "{space:>pad$}{text}",
                    space = "",
                    pad = left_padding,
                    text = loading_text
                )),
                ResetColor
            )?;
        }

        stdout.flush()?;
        Ok(())
    }

    /// Sets whether to show the season countdown in the footer.
    /// The countdown will be displayed in the footer area if enabled and regular season hasn't started.
    pub async fn set_show_season_countdown(&mut self, games: &[crate::data_fetcher::GameData]) {
        // Only show countdown if we have games and they're not from the regular season yet
        if games.is_empty() {
            return;
        }

        // Check if any game is from the regular season (runkosarja)
        let has_regular_season_games = games.iter().any(|game| game.serie == "RUNKOSARJA");

        if has_regular_season_games {
            // Regular season has started, don't show countdown
            return;
        }

        // Find the earliest regular season game by fetching future regular season games
        // Only attempt countdown calculation if we can load config without user interaction
        match tokio::task::spawn_blocking(|| std::env::var("LIIGA_API_DOMAIN")).await {
            Ok(Ok(api_domain))
                if !api_domain.is_empty()
                    && api_domain != "placeholder"
                    && api_domain != "test"
                    && api_domain != "unset" =>
            {
                // Config is available via environment variable, safe to calculate countdown
                let client = reqwest::Client::new();
                let config = match Config::load().await {
                    Ok(config) => config,
                    Err(_) => return, // Skip countdown if config loading fails
                };

                if let Some(days_until_season) =
                    calculate_days_until_regular_season(&client, &config, None).await
                {
                    let countdown_text = format!("Runkosarjan alkuun {days_until_season} päivää");
                    self.season_countdown = Some(countdown_text);
                }
            }
            _ => {
                // No valid API domain available, skip countdown to prevent interactive config reads
                debug!("Skipping season countdown due to missing or invalid LIIGA_API_DOMAIN");
            }
        }
    }

    fn calculate_game_height(game: &TeletextRow) -> u16 {
        match game {
            TeletextRow::GameResult { goal_events, .. } => {
                let base_height = 1; // Game result line
                let home_scorers = goal_events.iter().filter(|e| e.is_home_team).count();
                let away_scorers = goal_events.iter().filter(|e| !e.is_home_team).count();
                let scorer_lines = home_scorers.max(away_scorers);
                let spacer = 1; // Space between games
                base_height + scorer_lines as u16 + spacer
            }
            TeletextRow::ErrorMessage(_) => 2u16, // Error message + spacer
            TeletextRow::FutureGamesHeader(_) => 1u16, // Single line for future games header
        }
    }

    /// Calculates the effective game height considering wide mode.
    /// In wide mode, we can fit two games side by side, effectively halving the height usage.
    fn calculate_effective_game_height(&self, game: &TeletextRow) -> u16 {
        let base_height = Self::calculate_game_height(game);
        if self.wide_mode && self.can_fit_two_pages() {
            // In wide mode, we can fit two games in the same vertical space
            // Add spacing between games (1 extra line per game except the last)
            let height_with_spacing = base_height + 1; // Add space between games
            // So each game effectively uses half the height
            height_with_spacing.div_ceil(2) // Round up to ensure we don't underestimate
        } else {
            base_height
        }
    }

    /// Calculates and returns the content that should be displayed on the current page.
    /// Handles pagination based on available screen height and content size.
    ///
    /// # Returns
    /// A tuple containing:
    /// * Vec<&TeletextRow> - Content rows that should be displayed on the current page
    /// * bool - Whether there are more pages after the current one
    ///
    /// # Notes
    /// - When ignore_height_limit is true, returns all content in a single page
    /// - Otherwise, calculates how many items fit on each page based on screen height
    /// - Reserves 5 lines for header, subheader, and footer
    /// - Maintains consistent item grouping across pages
    fn get_page_content(&self) -> (Vec<&TeletextRow>, bool) {
        if self.ignore_height_limit {
            // When ignoring height limit, return all content in one page
            return (self.content_rows.iter().collect(), false);
        }

        let available_height = self.screen_height.saturating_sub(5); // Reserve space for header, subheader, and footer
        let mut current_height = 0u16;
        let mut page_content = Vec::new();
        let mut has_more = false;
        let mut items_per_page = Vec::new();
        let mut current_page_items = Vec::new();

        // First, calculate how many items fit on each page
        for game in self.content_rows.iter() {
            let game_height = self.calculate_effective_game_height(game);

            if current_height + game_height <= available_height {
                current_page_items.push(game);
                current_height += game_height;
            } else if !current_page_items.is_empty() {
                items_per_page.push(current_page_items.len());
                current_page_items = vec![game];
                current_height = game_height;
            }
        }
        if !current_page_items.is_empty() {
            items_per_page.push(current_page_items.len());
        }

        // Calculate the starting index for the current page
        let mut start_idx = 0;
        for (page_idx, &items) in items_per_page.iter().enumerate() {
            if page_idx == self.current_page {
                break;
            }
            start_idx += items;
        }

        // Get the items for the current page
        if let Some(&items_in_current_page) = items_per_page.get(self.current_page) {
            let end_idx = (start_idx + items_in_current_page).min(self.content_rows.len());
            page_content = self.content_rows[start_idx..end_idx].iter().collect();
            has_more = end_idx < self.content_rows.len();
        }

        (page_content, has_more)
    }

    pub fn total_pages(&self) -> usize {
        let mut total_pages = 1;
        let mut current_height = 0u16;
        let available_height = self.screen_height.saturating_sub(5);
        let mut current_page_items = 0;

        for game in &self.content_rows {
            let game_height = self.calculate_effective_game_height(game);
            if current_height + game_height > available_height {
                if current_page_items > 0 {
                    total_pages += 1;
                    current_height = game_height;
                    current_page_items = 1;
                }
            } else {
                current_height += game_height;
                current_page_items += 1;
            }
        }

        total_pages
    }

    /// Gets the current page number (0-based index)
    pub fn get_current_page(&self) -> usize {
        self.current_page
    }

    /// Sets the current page number (0-based index)
    /// Ensures the page number is within valid bounds
    pub fn set_current_page(&mut self, page: usize) {
        let total_pages = self.total_pages();
        if total_pages > 0 {
            self.current_page = page.min(total_pages - 1);
        } else {
            self.current_page = 0;
        }
    }

    /// Moves to the next page of content if available.
    /// Wraps around to the first page when at the end.
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::TeletextPage;
    /// use crossterm::event::KeyCode;
    ///
    /// let mut page = TeletextPage::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    ///     false,
    ///     true,
    ///     false,
    ///     false,
    ///     false, // wide_mode
    /// );
    ///
    /// let event = KeyCode::Right;
    /// if event == KeyCode::Right {
    ///     page.next_page();
    /// }
    /// ```
    pub fn next_page(&mut self) {
        let total = self.total_pages();
        if total <= 1 {
            return;
        }
        self.current_page = (self.current_page + 1) % total;
    }

    /// Moves to the previous page of content if available.
    /// Wraps around to the last page when at the beginning.
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::TeletextPage;
    /// use crossterm::event::KeyCode;
    ///
    /// let mut page = TeletextPage::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    ///     false,
    ///     true,
    ///     false,
    ///     false,
    ///     false, // wide_mode
    /// );
    ///
    /// let event = KeyCode::Left;
    /// if event == KeyCode::Left {
    ///     page.previous_page();
    /// }
    /// ```
    pub fn previous_page(&mut self) {
        let total = self.total_pages();
        if total <= 1 {
            return;
        }
        self.current_page = if self.current_page == 0 {
            total - 1
        } else {
            self.current_page - 1
        };
    }

    /// Renders the page content to the provided stdout.
    /// Handles all formatting, colors, and layout according to current settings.
    /// Optimized to reduce flickering by minimizing screen clears and using cursor positioning.
    ///
    /// # Arguments
    /// * `stdout` - Mutable reference to stdout for writing
    ///
    /// # Returns
    /// * `Result<(), AppError>` - Ok if rendering succeeded, Err otherwise
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::TeletextPage;
    /// use std::io::stdout;
    ///
    /// let mut page = TeletextPage::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    ///     false,
    ///     true,
    ///     false,
    ///     false,
    ///     false, // wide_mode
    /// );
    ///
    /// // Set a fixed screen height for testing to avoid terminal size issues
    /// page.set_screen_height(20);
    ///
    /// // When terminal is resized
    /// page.handle_resize();
    ///
    /// // Only render if we have a proper terminal (skip in CI)
    /// if std::env::var("CI").is_err() {
    ///     let mut stdout = stdout();
    ///     page.render_buffered(&mut stdout)?;
    /// }
    /// # Ok::<(), liiga_teletext::AppError>(())
    /// ```
    /// Calculates the expected buffer size for rendering to avoid reallocations.
    /// Estimates size based on terminal width, content rows, and ANSI escape sequences.
    ///
    /// # Arguments
    /// * `width` - Terminal width in characters
    /// * `visible_rows` - The content rows that will be rendered
    ///
    /// # Returns
    /// * `usize` - Estimated buffer size in bytes
    fn calculate_buffer_size(&self, width: u16, visible_rows: &[&TeletextRow]) -> usize {
        let width = width as usize;

        // Base overhead for headers, ANSI escape sequences, and screen control
        let mut size = 500; // Header, subheader, screen clear sequences

        // Add terminal size as base (each line could be full width)
        size += width * 4; // Header + subheader + padding lines

        // Calculate content size
        for row in visible_rows {
            match row {
                TeletextRow::GameResult { goal_events, .. } => {
                    // Game line: ~80 chars + ANSI sequences (~50 chars)
                    size += 130;

                    // Goal events: estimate 2 lines per game on average
                    // Each goal line: ~40 chars + ANSI sequences (~30 chars)
                    size += goal_events.len() * 70;

                    // Extra spacing
                    size += 20;
                }
                TeletextRow::ErrorMessage(message) => {
                    // Error message: actual length + ANSI sequences
                    size += message.len() + 50;
                }
                TeletextRow::FutureGamesHeader(header) => {
                    // Header: actual length + ANSI sequences
                    size += header.len() + 30;
                }
            }
        }

        // Footer: ~100 chars + ANSI sequences
        if self.show_footer {
            size += 150;
            // Add space for season countdown if present
            if self.season_countdown.is_some() {
                size += 100;
            }
        }

        // Add 25% overhead for ANSI positioning sequences and safety margin
        size + (size / 4)
    }

    /// Renders the page content using double buffering for reduced flickering.
    /// This method builds all terminal escape sequences and content in a buffer first,
    /// then writes everything in a single operation.
    pub fn render_buffered(&self, stdout: &mut Stdout) -> Result<(), AppError> {
        // Get terminal dimensions - use appropriate width in non-interactive mode
        let width = if self.ignore_height_limit {
            // Use wider default width for non-interactive mode when wide mode is enabled
            if self.wide_mode {
                136u16 // Wide enough to accommodate wide mode (128+ required)
            } else {
                80u16 // Standard width for normal mode
            }
        } else {
            // Hide cursor to prevent visual artifacts during rendering
            execute!(stdout, crossterm::cursor::Hide)?;

            // Get terminal dimensions
            let (width, _) = crossterm::terminal::size()?;
            width
        };

        // Get content for current page to calculate buffer size
        let (visible_rows, _) = self.get_page_content();

        // Calculate expected buffer size to avoid reallocations
        let expected_size = self.calculate_buffer_size(width, &visible_rows);

        // Build the entire screen content in a string buffer (double buffering)
        let mut buffer = String::with_capacity(expected_size);

        // Only clear the screen in interactive mode using more efficient method
        if !self.ignore_height_limit {
            buffer.push_str("\x1b[H"); // Move to home position
            buffer.push_str("\x1b[0J"); // Clear from cursor down
        }

        // Format the header text with date if available
        let header_text = if let Some(ref date) = self.fetched_date {
            let formatted_date = match chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
                Ok(date) => date.format("%d.%m.%Y").to_string(),
                Err(_) => date.clone(),
            };
            format!("SM-LIIGA {} {}", self.page_number, formatted_date)
        } else {
            format!("SM-LIIGA {}", self.page_number)
        };

        // Build header with proper ANSI escape codes
        let title_bg_code = get_ansi_code(title_bg(), 46);
        let header_fg_code = get_ansi_code(header_fg(), 21);
        let header_bg_code = get_ansi_code(header_bg(), 21);

        buffer.push_str(&format!(
            "\x1b[1;1H\x1b[48;5;{}m\x1b[38;5;{}m{:<20}\x1b[48;5;{}m\x1b[38;5;231m{:>width$}\x1b[0m",
            title_bg_code,
            header_fg_code,
            self.title,
            header_bg_code,
            header_text,
            width = (width as usize).saturating_sub(20)
        ));

        // Build subheader with pagination info
        let total_pages = self.total_pages();
        let page_info = if total_pages > 1 && !self.ignore_height_limit {
            format!("{}/{}", self.current_page + 1, total_pages)
        } else {
            String::new()
        };

        let subheader_fg_code = get_ansi_code(subheader_fg(), 46);

        buffer.push_str(&format!(
            "\x1b[2;1H\x1b[38;5;{}m{:<20}{:>width$}\x1b[0m",
            subheader_fg_code,
            self.subheader,
            page_info,
            width = (width as usize).saturating_sub(20)
        ));

        // Build content starting at line 4 (1-based ANSI positioning)
        let mut current_line: usize = 4;
        let text_fg_code = get_ansi_code(text_fg(), 231);
        let result_fg_code = get_ansi_code(result_fg(), 46);

        // Handle rendering modes

        if self.wide_mode && self.can_fit_two_pages() {
            // Wide mode rendering - two columns
            self.render_wide_mode_content(
                &mut buffer,
                &visible_rows,
                width,
                &mut current_line,
                text_fg_code,
                subheader_fg_code,
            );
        } else if self.compact_mode {
            // Compact mode rendering
            let config = CompactDisplayConfig::default();
            let validation = config.validate_terminal_width(width as usize);

            match validation {
                TerminalWidthValidation::Sufficient {
                    current_width: _,
                    required_width: _,
                    excess: _,
                } => {
                    // Terminal is wide enough for compact mode
                    let compact_lines = self.group_games_for_compact_display(
                        &visible_rows,
                        &config,
                        width as usize,
                    );

                    // Check for compatibility warnings
                    let compatibility = self.validate_compact_mode_compatibility();
                    if let CompactModeValidation::CompatibleWithWarnings { warnings } =
                        compatibility
                    {
                        // Display warnings at the top of compact content
                        for (warning_index, warning) in warnings.iter().enumerate() {
                            buffer.push_str(&format!(
                                "\x1b[{};{}H\x1b[38;5;{}m⚠ {} (compact mode)\x1b[0m",
                                current_line + warning_index,
                                CONTENT_MARGIN + 1,
                                text_fg_code,
                                warning
                            ));
                        }
                        current_line += warnings.len();
                    }

                    for (line_index, compact_line) in compact_lines.iter().enumerate() {
                        buffer.push_str(&format!(
                            "\x1b[{};{}H{}",
                            current_line + line_index,
                            CONTENT_MARGIN + 1,
                            compact_line
                        ));
                    }
                    current_line += compact_lines.len();
                }
                TerminalWidthValidation::Insufficient {
                    current_width,
                    required_width,
                    shortfall,
                } => {
                    // Terminal is too narrow for compact mode - show detailed error message
                    let error_message = format!(
                        "Terminal too narrow for compact mode ({current_width} chars, need {required_width} chars, short {shortfall} chars)"
                    );

                    buffer.push_str(&format!(
                        "\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                        current_line,
                        CONTENT_MARGIN + 1,
                        text_fg_code,
                        error_message
                    ));
                    current_line += 1;

                    // Add suggestion for minimum terminal width
                    buffer.push_str(&format!(
                        "\x1b[{};{}H\x1b[38;5;{}mResize terminal to at least {} characters wide\x1b[0m",
                        current_line,
                        CONTENT_MARGIN + 1,
                        text_fg_code,
                        required_width
                    ));
                    current_line += 1;
                }
            }
        } else {
            // Normal rendering mode
            for row in visible_rows {
                match row {
                    TeletextRow::GameResult {
                        home_team,
                        away_team,
                        time,
                        result,
                        score_type,
                        is_overtime,
                        is_shootout,
                        goal_events,
                        played_time,
                    } => {
                        // Format result with overtime/shootout indicator
                        let result_text = if *is_shootout {
                            format!("{result} rl")
                        } else if *is_overtime {
                            format!("{result} ja")
                        } else {
                            result.clone()
                        };

                        // Format time display based on game state
                        let (time_display, score_display) = match score_type {
                            ScoreType::Scheduled => (time.clone(), String::new()),
                            ScoreType::Ongoing => {
                                let formatted_time =
                                    format!("{:02}:{:02}", played_time / 60, played_time % 60);
                                (formatted_time, result_text.clone())
                            }
                            ScoreType::Final => (String::new(), result_text.clone()),
                        };

                        let result_color = match score_type {
                            ScoreType::Final => result_fg_code,
                            _ => text_fg_code,
                        };

                        // Build game line with precise positioning (using 1-based ANSI coordinates)
                        if !time_display.is_empty() && !score_display.is_empty() {
                            // For ongoing games: show time on the left, score on the right
                            buffer.push_str(&format!(
                            "\x1b[{};{}H\x1b[38;5;{}m{:<20}\x1b[{};{}H\x1b[38;5;{}m- \x1b[{};{}H\x1b[38;5;{}m{:<20}\x1b[{};{}H\x1b[38;5;{}m{:<10}\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                            current_line, CONTENT_MARGIN + 1,
                            text_fg_code,
                            home_team.chars().take(20).collect::<String>(),
                            current_line, SEPARATOR_OFFSET + CONTENT_MARGIN + 1,
                            text_fg_code,
                            current_line, AWAY_TEAM_OFFSET + CONTENT_MARGIN + 1,
                            text_fg_code,
                            away_team.chars().take(20).collect::<String>(),
                            current_line, 35 + CONTENT_MARGIN + 1,
                            text_fg_code,
                            time_display,
                            current_line, 45 + CONTENT_MARGIN + 1,
                            result_color,
                            score_display
                        ));
                        } else {
                            // For scheduled/final games: show time or score on the right
                            let display_text = if !time_display.is_empty() {
                                time_display
                            } else {
                                score_display
                            };
                            buffer.push_str(&format!(
                            "\x1b[{};{}H\x1b[38;5;{}m{:<20}\x1b[{};{}H\x1b[38;5;{}m- \x1b[{};{}H\x1b[38;5;{}m{:<20}\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                            current_line, CONTENT_MARGIN + 1,
                            text_fg_code,
                            home_team.chars().take(20).collect::<String>(),
                            current_line, SEPARATOR_OFFSET + CONTENT_MARGIN + 1,
                            text_fg_code,
                            current_line, AWAY_TEAM_OFFSET + CONTENT_MARGIN + 1,
                            text_fg_code,
                            away_team.chars().take(20).collect::<String>(),
                            current_line, 45 + CONTENT_MARGIN + 1,
                            result_color,
                            display_text
                        ));
                        }

                        current_line += 1;

                        // Add goal events for finished/ongoing games
                        if matches!(score_type, ScoreType::Ongoing | ScoreType::Final)
                            && !goal_events.is_empty()
                        {
                            let home_scorer_fg_code = get_ansi_code(home_scorer_fg(), 51);
                            let away_scorer_fg_code = get_ansi_code(away_scorer_fg(), 51);
                            let winning_goal_fg_code = get_ansi_code(winning_goal_fg(), 201);
                            let goal_type_fg_code = get_ansi_code(goal_type_fg(), 226);

                            let home_scorers: Vec<_> =
                                goal_events.iter().filter(|e| e.is_home_team).collect();
                            let away_scorers: Vec<_> =
                                goal_events.iter().filter(|e| !e.is_home_team).collect();
                            let max_scorers = home_scorers.len().max(away_scorers.len());

                            for i in 0..max_scorers {
                                // Home team scorer
                                if let Some(event) = home_scorers.get(i) {
                                    let scorer_color = if (event.is_winning_goal
                                        && (*is_overtime || *is_shootout))
                                        || event.goal_types.contains(&"VL".to_string())
                                    {
                                        winning_goal_fg_code
                                    } else {
                                        home_scorer_fg_code
                                    };

                                    buffer.push_str(&format!(
                                        "\x1b[{};{}H\x1b[38;5;{}m{:2} ",
                                        current_line,
                                        CONTENT_MARGIN + 1,
                                        scorer_color,
                                        event.minute
                                    ));

                                    // Add video link functionality if there's a video clip and links are enabled
                                    let display_name = Self::format_scorer_name(&event.scorer_name);
                                    if let Some(url) = &event.video_clip_url {
                                        if !self.disable_video_links {
                                            buffer.push_str(&format!(
                                                "\x1b[38;5;{}m{:<12}\x1B]8;;{}\x07▶\x1B]8;;\x07",
                                                scorer_color, display_name, url
                                            ));
                                        } else {
                                            buffer.push_str(&format!(
                                                "\x1b[38;5;{}m{:<12}",
                                                scorer_color, display_name
                                            ));
                                        }
                                    } else {
                                        buffer.push_str(&format!(
                                            "\x1b[38;5;{}m{:<12}",
                                            scorer_color, display_name
                                        ));
                                    }

                                    // Add goal type indicators
                                    let goal_type = event.get_goal_type_display();
                                    if !goal_type.is_empty() {
                                        buffer.push_str(&format!(
                                            " \x1b[38;5;{goal_type_fg_code}m{goal_type}\x1b[0m"
                                        ));
                                    } else {
                                        buffer.push_str("\x1b[0m");
                                    }
                                }

                                // Away team scorer
                                if let Some(event) = away_scorers.get(i) {
                                    let scorer_color = if (event.is_winning_goal
                                        && (*is_overtime || *is_shootout))
                                        || event.goal_types.contains(&"VL".to_string())
                                    {
                                        winning_goal_fg_code
                                    } else {
                                        away_scorer_fg_code
                                    };

                                    buffer.push_str(&format!(
                                        "\x1b[{};{}H\x1b[38;5;{}m{:2} ",
                                        current_line,
                                        AWAY_TEAM_OFFSET + CONTENT_MARGIN + 1,
                                        scorer_color,
                                        event.minute
                                    ));

                                    // Add video link functionality if there's a video clip and links are enabled
                                    let display_name = Self::format_scorer_name(&event.scorer_name);
                                    if let Some(url) = &event.video_clip_url {
                                        if !self.disable_video_links {
                                            buffer.push_str(&format!(
                                                "\x1b[38;5;{}m{:<12}\x1B]8;;{}\x07▶\x1B]8;;\x07",
                                                scorer_color, display_name, url
                                            ));
                                        } else {
                                            buffer.push_str(&format!(
                                                "\x1b[38;5;{}m{:<12}",
                                                scorer_color, display_name
                                            ));
                                        }
                                    } else {
                                        buffer.push_str(&format!(
                                            "\x1b[38;5;{}m{:<12}",
                                            scorer_color, display_name
                                        ));
                                    }

                                    // Add goal type indicators
                                    let goal_type = event.get_goal_type_display();
                                    if !goal_type.is_empty() {
                                        buffer.push_str(&format!(
                                            " \x1b[38;5;{goal_type_fg_code}m{goal_type}\x1b[0m"
                                        ));
                                    } else {
                                        buffer.push_str("\x1b[0m");
                                    }
                                }

                                if home_scorers.get(i).is_some() || away_scorers.get(i).is_some() {
                                    current_line += 1;
                                }
                            }
                        }

                        // Add spacing between games in interactive mode
                        if !self.ignore_height_limit {
                            current_line += 1;
                        }
                    }
                    TeletextRow::ErrorMessage(message) => {
                        for line in message.lines() {
                            buffer.push_str(&format!(
                                "\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                                current_line,
                                CONTENT_MARGIN + 1,
                                text_fg_code,
                                line
                            ));
                            current_line += 1;
                        }
                    }
                    TeletextRow::FutureGamesHeader(header_text) => {
                        buffer.push_str(&format!(
                            "\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
                            current_line,
                            CONTENT_MARGIN + 1,
                            subheader_fg_code,
                            header_text
                        ));
                        current_line += 1;
                    }
                }
            }
        }

        // Add footer if enabled
        if self.show_footer {
            let footer_y = if self.ignore_height_limit {
                current_line + 1
            } else {
                self.screen_height.saturating_sub(1) as usize
            };

            let controls = if total_pages > 1 {
                "q=Lopeta ←→=Sivut"
            } else {
                "q=Lopeta"
            };

            let controls = if self.auto_refresh_disabled {
                if total_pages > 1 {
                    "q=Lopeta ←→=Sivut (Ei päivity)"
                } else {
                    "q=Lopeta (Ei päivity)"
                }
            } else {
                controls
            };

            // Add season countdown above the footer if available
            if let Some(ref countdown) = self.season_countdown {
                let countdown_y = footer_y.saturating_sub(1);
                buffer.push_str(&format!(
                    "\x1b[{};1H\x1b[38;5;{}m{:^width$}\x1b[0m",
                    countdown_y,
                    get_ansi_code(Color::AnsiValue(226), 226), // Bright yellow
                    countdown,
                    width = width as usize
                ));
            }

            // Add loading indicator or auto-refresh indicator if active
            let mut footer_text = if let Some(ref loading) = self.loading_indicator {
                let loading_frame = loading.current_frame();
                format!("{controls} {} {}", loading_frame, loading.message())
            } else if let Some(ref indicator) = self.auto_refresh_indicator {
                let indicator_frame = indicator.current_frame();
                format!("{controls} {indicator_frame}")
            } else {
                controls.to_string()
            };

            // Append error warning if active
            if self.error_warning_active {
                footer_text.push_str("  ⚠️");
            }

            buffer.push_str(&format!(
                "\x1b[{};1H\x1b[48;5;{}m\x1b[38;5;21m{}\x1b[38;5;231m{:^width$}\x1b[38;5;21m{}\x1b[0m",
                footer_y,
                get_ansi_code(header_bg(), 21),
                "   ",
                footer_text,
                "   ",
                width = (width as usize).saturating_sub(6)
            ));
        }

        // Write entire buffer in one operation (minimizes flicker)
        execute!(stdout, Print(buffer))?;

        // Show cursor again
        execute!(stdout, crossterm::cursor::Show)?;

        stdout.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::GoalEventData;
    use crate::data_fetcher::models::GameData;

    #[test]
    fn test_team_abbreviation() {
        // Test current Liiga teams
        assert_eq!(get_team_abbreviation("Tappara"), "TAP");
        assert_eq!(get_team_abbreviation("HIFK"), "IFK");
        assert_eq!(get_team_abbreviation("TPS"), "TPS");
        assert_eq!(get_team_abbreviation("JYP"), "JYP");
        assert_eq!(get_team_abbreviation("Ilves"), "ILV");
        assert_eq!(get_team_abbreviation("KalPa"), "KAL");
        assert_eq!(get_team_abbreviation("Kärpät"), "KÄR");
        assert_eq!(get_team_abbreviation("Lukko"), "LUK");
        assert_eq!(get_team_abbreviation("Pelicans"), "PEL");
        assert_eq!(get_team_abbreviation("SaiPa"), "SAI");
        assert_eq!(get_team_abbreviation("Sport"), "SPO");
        assert_eq!(get_team_abbreviation("HPK"), "HPK");
        assert_eq!(get_team_abbreviation("Jukurit"), "JUK");
        assert_eq!(get_team_abbreviation("Ässät"), "ÄSS");
        assert_eq!(get_team_abbreviation("KooKoo"), "KOO");

        // Test alternative team name formats
        assert_eq!(get_team_abbreviation("HIFK Helsinki"), "IFK");
        assert_eq!(get_team_abbreviation("TPS Turku"), "TPS");
        assert_eq!(get_team_abbreviation("Tampereen Tappara"), "TAP");
        assert_eq!(get_team_abbreviation("Tampereen Ilves"), "ILV");
        assert_eq!(get_team_abbreviation("Jyväskylän JYP"), "JYP");
        assert_eq!(get_team_abbreviation("Kuopion KalPa"), "KAL");
        assert_eq!(get_team_abbreviation("Oulun Kärpät"), "KÄR");
        assert_eq!(get_team_abbreviation("Rauman Lukko"), "LUK");
        assert_eq!(get_team_abbreviation("Lahden Pelicans"), "PEL");
        assert_eq!(get_team_abbreviation("Lappeenrannan SaiPa"), "SAI");
        assert_eq!(get_team_abbreviation("Vaasan Sport"), "SPO");
        assert_eq!(get_team_abbreviation("Hämeenlinnan HPK"), "HPK");
        assert_eq!(get_team_abbreviation("Mikkelin Jukurit"), "JUK");
        assert_eq!(get_team_abbreviation("Porin Ässät"), "ÄSS");
        assert_eq!(get_team_abbreviation("Kouvolan KooKoo"), "KOO");

        // Test fallback for unknown team names (letters only, uppercase)
        assert_eq!(get_team_abbreviation("Unknown Team"), "UNK"); // "UnknownTeam" -> "UNK"
        assert_eq!(get_team_abbreviation("New Team"), "NEW"); // "NewTeam" -> "NEW"
        assert_eq!(get_team_abbreviation("AB"), "AB"); // Short name
        assert_eq!(get_team_abbreviation("A"), "A"); // Very short name
    }

    #[test]
    fn test_compact_display_config() {
        // Test default configuration
        let config = CompactDisplayConfig::default();
        assert_eq!(config.max_games_per_line, 3);
        assert_eq!(config.team_name_width, 8);
        assert_eq!(config.score_width, 6);
        assert_eq!(config.game_separator, "  ");

        // Test custom configuration
        let custom_config = CompactDisplayConfig::new(3, 10, 8, " | ");
        assert_eq!(custom_config.max_games_per_line, 3);
        assert_eq!(custom_config.team_name_width, 10);
        assert_eq!(custom_config.score_width, 8);
        assert_eq!(custom_config.game_separator, " | ");

        // Test terminal width adaptation
        assert_eq!(config.calculate_games_per_line(80), 3);
        assert_eq!(config.calculate_games_per_line(100), 3);
        assert_eq!(config.calculate_games_per_line(0), 1);

        // Test terminal width sufficiency
        assert!(config.is_terminal_width_sufficient(20));
        assert!(config.is_terminal_width_sufficient(18));
        assert!(!config.is_terminal_width_sufficient(17));
    }

    #[test]
    fn test_loading_indicator() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            false,
            false,
        );

        // Test showing loading indicator
        page.show_loading("Etsitään otteluita...".to_string());
        assert!(page.loading_indicator.is_some());

        if let Some(ref indicator) = page.loading_indicator {
            assert_eq!(indicator.message(), "Etsitään otteluita...");
            assert_eq!(indicator.current_frame(), "|"); // First frame
        }

        // Test updating animation
        page.update_loading_animation();
        if let Some(ref indicator) = page.loading_indicator {
            assert_eq!(indicator.current_frame(), "/"); // Second frame
        }

        // Test hiding loading indicator
        page.hide_loading();
        assert!(page.loading_indicator.is_none());
    }

    #[test]
    fn test_page_navigation() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            false,
            false,
        );
        page.set_screen_height(20); // Set fixed screen height for testing

        // Add enough games with goal events to create multiple pages
        for i in 0..10 {
            let goal_events = vec![
                GoalEventData {
                    scorer_player_id: i64::from(i),
                    scorer_name: format!("Scorer {i}"),
                    minute: 10,
                    home_team_score: 1,
                    away_team_score: 0,
                    is_winning_goal: false,
                    goal_types: vec![],
                    is_home_team: true,
                    video_clip_url: None,
                },
                GoalEventData {
                    scorer_player_id: i64::from(i + 100),
                    scorer_name: format!("Scorer {val}", val = i + 100),
                    minute: 20,
                    home_team_score: 1,
                    away_team_score: 1,
                    is_winning_goal: false,
                    goal_types: vec![],
                    is_home_team: false,
                    video_clip_url: None,
                },
            ];

            page.add_game_result(GameResultData::new(&GameData {
                home_team: format!("Home {i}"),
                away_team: format!("Away {i}"),
                time: "18.00".to_string(),
                result: "1-1".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                goal_events,
                played_time: 1200,
                serie: "RUNKOSARJA".to_string(),
                start: "2025-01-01T00:00:00Z".to_string(),
            }));
        }

        let initial_page = page.current_page;
        page.next_page();
        assert!(page.current_page > initial_page, "Should move to next page");

        page.previous_page();
        assert_eq!(
            page.current_page, initial_page,
            "Should return to initial page"
        );
    }

    #[test]
    fn test_page_wrapping() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            false,
            false,
        );
        page.set_screen_height(20); // Set fixed screen height for testing

        // Add enough games with goal events to create multiple pages
        for i in 0..10 {
            let goal_events = vec![
                GoalEventData {
                    scorer_player_id: i64::from(i),
                    scorer_name: format!("Scorer {i}"),
                    minute: 10,
                    home_team_score: 1,
                    away_team_score: 0,
                    is_winning_goal: false,
                    goal_types: vec![],
                    is_home_team: true,
                    video_clip_url: None,
                },
                GoalEventData {
                    scorer_player_id: i64::from(i + 100),
                    scorer_name: format!("Scorer {val}", val = i + 100),
                    minute: 20,
                    home_team_score: 1,
                    away_team_score: 1,
                    is_winning_goal: false,
                    goal_types: vec![],
                    is_home_team: false,
                    video_clip_url: None,
                },
            ];

            page.add_game_result(GameResultData::new(&GameData {
                home_team: format!("Home {i}"),
                away_team: format!("Away {i}"),
                time: "18.00".to_string(),
                result: "1-1".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                goal_events,
                played_time: 1200,
                serie: "RUNKOSARJA".to_string(),
                start: "2025-01-01T00:00:00Z".to_string(),
            }));
        }

        let total_pages = page.total_pages();
        assert!(total_pages > 1, "Should have multiple pages");

        // Test wrapping from last to first page
        page.current_page = total_pages - 1;
        page.next_page();
        assert_eq!(page.current_page, 0, "Should wrap to first page");

        // Test wrapping from first to last page
        page.current_page = 0;
        page.previous_page();
        assert_eq!(
            page.current_page,
            total_pages - 1,
            "Should wrap to last page"
        );
    }

    #[test]
    fn test_game_height_calculation() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            true, // ignore_height_limit = true to show all games
            false,
            false,
        );

        // Test game without goals
        page.add_game_result(GameResultData::new(&GameData {
            home_team: "Home".to_string(),
            away_team: "Away".to_string(),
            time: "18.00".to_string(),
            result: "0-0".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            goal_events: vec![],
            played_time: 0,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        // Test game with goals
        let goals = vec![GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Scorer".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec![],
            is_home_team: true,
            video_clip_url: None,
        }];

        page.add_game_result(GameResultData::new(&GameData {
            home_team: "Home".to_string(),
            away_team: "Away".to_string(),
            time: "18.00".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            goal_events: goals,
            played_time: 600,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        let (content, _) = page.get_page_content();
        assert_eq!(content.len(), 2, "Should show both games");
    }

    #[test]
    fn test_error_message_display() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            true, // ignore_height_limit = true to show all content
            false,
            false,
        );
        let error_msg = "Test Error";
        page.add_error_message(error_msg);

        let (content, _) = page.get_page_content();
        assert_eq!(content.len(), 1, "Should have one row");
        match &content[0] {
            TeletextRow::ErrorMessage(msg) => assert_eq!(msg, error_msg),
            _ => panic!("Should be an error message"),
        }
    }

    #[test]
    fn test_game_result_display() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            true, // ignore_height_limit = true to show all games
            false,
            false,
        );

        // Test scheduled game display
        page.add_game_result(GameResultData::new(&GameData {
            home_team: "Home".to_string(),
            away_team: "Away".to_string(),
            time: "18.00".to_string(),
            result: "0-0".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            goal_events: vec![],
            played_time: 0,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        // Test ongoing game with goals
        let goal_events = vec![
            GoalEventData {
                scorer_player_id: 123,
                scorer_name: "Scorer".to_string(),
                minute: 10,
                home_team_score: 1,
                away_team_score: 0,
                is_winning_goal: false,
                goal_types: vec!["YV".to_string()],
                is_home_team: true,
                video_clip_url: Some("http://example.com".to_string()),
            },
            GoalEventData {
                scorer_player_id: 456,
                scorer_name: "Away Scorer".to_string(),
                minute: 25,
                home_team_score: 1,
                away_team_score: 1,
                is_winning_goal: false,
                goal_types: vec![],
                is_home_team: false,
                video_clip_url: None,
            },
        ];

        page.add_game_result(GameResultData::new(&GameData {
            home_team: "Home".to_string(),
            away_team: "Away".to_string(),
            time: "".to_string(),
            result: "1-1".to_string(),
            score_type: ScoreType::Ongoing,
            is_overtime: false,
            is_shootout: false,
            goal_events: goal_events.clone(),
            played_time: 1500,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        // Test finished game with overtime
        page.add_game_result(GameResultData::new(&GameData {
            home_team: "Home OT".to_string(),
            away_team: "Away OT".to_string(),
            time: "".to_string(),
            result: "3-2".to_string(),
            score_type: ScoreType::Final,
            is_overtime: true,
            is_shootout: false,
            goal_events: vec![],
            played_time: 3900,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        // Test finished game with shootout
        page.add_game_result(GameResultData::new(&GameData {
            home_team: "Home SO".to_string(),
            away_team: "Away SO".to_string(),
            time: "".to_string(),
            result: "4-3".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: true,
            goal_events: vec![],
            played_time: 3600,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        let (content, _) = page.get_page_content();
        assert_eq!(content.len(), 4, "Should show all games");

        // Verify each game type is present
        let mut found_scheduled = false;
        let mut found_ongoing = false;
        let mut found_overtime = false;
        let mut found_shootout = false;

        for row in content {
            if let TeletextRow::GameResult {
                score_type,
                is_overtime,
                is_shootout,
                ..
            } = row
            {
                match score_type {
                    ScoreType::Scheduled => found_scheduled = true,
                    ScoreType::Ongoing => found_ongoing = true,
                    ScoreType::Final => {
                        if *is_overtime {
                            found_overtime = true;
                        } else if *is_shootout {
                            found_shootout = true;
                        }
                    }
                }
            }
        }

        assert!(found_scheduled, "Should contain scheduled game");
        assert!(found_ongoing, "Should contain ongoing game");
        assert!(found_overtime, "Should contain overtime game");
        assert!(found_shootout, "Should contain shootout game");
    }

    #[test]
    fn test_video_link_display() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false, // video links enabled
            true,
            false,
            false,
            false,
        );

        let goal_events = vec![GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Scorer".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec![],
            is_home_team: true,
            video_clip_url: Some("http://example.com".to_string()),
        }];

        page.add_game_result(GameResultData::new(&GameData {
            home_team: "Home".to_string(),
            away_team: "Away".to_string(),
            time: "".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            goal_events: goal_events.clone(),
            played_time: 3600,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        // Create another page with video links disabled
        let mut page_no_video = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            true, // video links disabled
            true,
            false,
            false,
            false,
        );

        page_no_video.add_game_result(GameResultData::new(&GameData {
            home_team: "Home".to_string(),
            away_team: "Away".to_string(),
            time: "".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            goal_events,
            played_time: 3600,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        let (content, _) = page.get_page_content();
        let (content_no_video, _) = page_no_video.get_page_content();

        assert_eq!(
            content.len(),
            content_no_video.len(),
            "Should have same number of games"
        );
    }

    #[test]
    fn test_buffer_size_calculation() {
        let page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            false,
            false,
        );

        // Test with empty content
        let empty_rows: Vec<&TeletextRow> = vec![];
        let empty_size = page.calculate_buffer_size(80, &empty_rows);

        // Should have base overhead + terminal width overhead (500 + 80*4 = 820, +25% = 1025)
        assert!(empty_size > 500); // Base overhead
        assert!(empty_size < 1500); // Should be reasonable for empty content

        // Test with game content
        let goal_events = vec![
            GoalEventData {
                scorer_name: "Player 1".to_string(),
                scorer_player_id: 1001,
                minute: 10,
                home_team_score: 1,
                away_team_score: 0,
                is_home_team: true,
                is_winning_goal: false,
                goal_types: vec![],
                video_clip_url: None,
            },
            GoalEventData {
                scorer_name: "Player 2".to_string(),
                scorer_player_id: 1002,
                minute: 25,
                home_team_score: 1,
                away_team_score: 1,
                is_home_team: false,
                is_winning_goal: false,
                goal_types: vec![],
                video_clip_url: None,
            },
        ];

        let game_row = TeletextRow::GameResult {
            home_team: "Team A".to_string(),
            away_team: "Team B".to_string(),
            time: "20:00".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            goal_events,
            played_time: 3600,
        };

        let game_rows = vec![&game_row];
        let game_size = page.calculate_buffer_size(80, &game_rows);

        // Should be larger than empty content
        assert!(game_size > empty_size);

        // Should account for game content + goal events
        // Game: 130 bytes + 2 goals * 70 bytes = 270 bytes content overhead
        assert!(game_size > empty_size + 200);

        // Test with error message
        let error_row = TeletextRow::ErrorMessage("Test error message".to_string());
        let error_rows = vec![&error_row];
        let error_size = page.calculate_buffer_size(80, &error_rows);

        // Should be appropriately sized for error message
        assert!(error_size > empty_size);
        assert!(error_size < game_size); // Smaller than game with goals

        // Test scaling with terminal width
        let wide_size = page.calculate_buffer_size(160, &empty_rows);
        assert!(wide_size > empty_size); // Larger terminal should need more buffer
    }

    #[test]
    fn test_get_ansi_code_helper() {
        // Test with AnsiValue color
        let ansi_color = Color::AnsiValue(42);
        assert_eq!(get_ansi_code(ansi_color, 100), 42);

        // Test with non-AnsiValue color (should use fallback)
        let rgb_color = Color::Rgb { r: 255, g: 0, b: 0 };
        assert_eq!(get_ansi_code(rgb_color, 100), 100);

        // Test with different fallback values
        let reset_color = Color::Reset;
        assert_eq!(get_ansi_code(reset_color, 231), 231);
        assert_eq!(get_ansi_code(reset_color, 46), 46);

        // Test actual teletext colors
        assert_eq!(get_ansi_code(text_fg(), 231), 231); // Should be 231 (white)
        assert_eq!(get_ansi_code(header_bg(), 21), 21); // Should be 21 (blue)
        assert_eq!(get_ansi_code(result_fg(), 46), 46); // Should be 46 (green)
    }

    #[test]
    fn test_page_preservation() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "SM-LIIGA".to_string(),
            false,
            true,
            false,
            false,
            false,
        );

        // Set a small screen height to ensure multiple pages
        page.set_screen_height(10);

        // Add enough content to create multiple pages
        for i in 0..20 {
            page.add_error_message(&format!("Test message {i}"));
        }

        // Navigate to page 2
        page.next_page();
        assert_eq!(page.get_current_page(), 1);

        // Create a new page with the same content and preserved page number
        let mut new_page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "SM-LIIGA".to_string(),
            false,
            true,
            false,
            false,
            false,
        );

        // Set the same small screen height
        new_page.set_screen_height(10);

        // Add the same content
        for i in 0..20 {
            new_page.add_error_message(&format!("Test message {i}"));
        }

        // Set the current page to match the original
        new_page.set_current_page(1);
        assert_eq!(new_page.get_current_page(), 1);

        // Verify both pages show the same content
        let (original_content, _) = page.get_page_content();
        let (new_content, _) = new_page.get_page_content();
        assert_eq!(original_content.len(), new_content.len());
    }

    #[test]
    fn test_compact_mode_getter_setter() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            false,
            false,
        );

        // Test initial state
        assert!(!page.is_compact_mode());

        // Test setting compact mode to true
        assert!(page.set_compact_mode(true).is_ok());
        assert!(page.is_compact_mode());

        // Test setting compact mode to false
        assert!(page.set_compact_mode(false).is_ok());
        assert!(!page.is_compact_mode());
    }

    #[test]
    fn test_format_compact_game() {
        let page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            true,
            false,
        );

        let config = CompactDisplayConfig::default();

        // Test scheduled game
        let scheduled_game = TeletextRow::GameResult {
            home_team: "Tappara".to_string(),
            away_team: "HIFK".to_string(),
            time: "18:30".to_string(),
            result: "".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            goal_events: vec![],
            played_time: 0,
        };

        let formatted = page.format_compact_game(&scheduled_game, &config);
        assert!(formatted.contains("TAP-IFK"));
        assert!(formatted.contains("18:30"));

        // Test final game
        let final_game = TeletextRow::GameResult {
            home_team: "Tappara".to_string(),
            away_team: "HIFK".to_string(),
            time: "18:30".to_string(),
            result: "3-2".to_string(),
            score_type: ScoreType::Final,
            is_overtime: true,
            is_shootout: false,
            goal_events: vec![],
            played_time: 3900,
        };

        let formatted = page.format_compact_game(&final_game, &config);
        assert!(formatted.contains("TAP-IFK"));
        assert!(formatted.contains("3-2 ja"));
    }

    #[test]
    fn test_group_games_for_compact_display() {
        let page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            true,
            false,
        );

        let config = CompactDisplayConfig::new(2, 10, 8, " | ");

        let games = vec![
            TeletextRow::GameResult {
                home_team: "Tappara".to_string(),
                away_team: "HIFK".to_string(),
                time: "18:30".to_string(),
                result: "3-2".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                goal_events: vec![],
                played_time: 3900,
            },
            TeletextRow::GameResult {
                home_team: "HIFK".to_string(),
                away_team: "Tappara".to_string(),
                time: "19:00".to_string(),
                result: "1-4".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                goal_events: vec![],
                played_time: 3900,
            },
        ];

        let game_refs: Vec<&TeletextRow> = games.iter().collect();
        let lines = page.group_games_for_compact_display(&game_refs, &config, 80);

        assert_eq!(lines.len(), 1); // Should fit on one line with 2 games per line
        assert!(lines[0].contains("TAP-IFK"));
        assert!(lines[0].contains("IFK-TAP"));
        assert!(lines[0].contains(" | ")); // Should contain separator
    }

    #[test]
    fn test_terminal_width_adaptation() {
        let page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            true,
            false,
        );

        // Test wide terminal
        assert!(page.is_terminal_suitable_for_compact(120));
        assert_eq!(page.calculate_compact_games_per_line(120), 3); // Default config allows up to 3

        // Test narrow terminal
        assert!(page.is_terminal_suitable_for_compact(30)); // 30 >= 18 (8+6+4)
        assert_eq!(page.calculate_compact_games_per_line(30), 1);

        // Test very narrow terminal
        assert!(!page.is_terminal_suitable_for_compact(10)); // 10 < 18
        assert_eq!(page.calculate_compact_games_per_line(10), 1);

        // Test with custom config that allows multiple games per line
        let custom_config = CompactDisplayConfig::new(3, 8, 6, "  ");
        assert_eq!(custom_config.calculate_games_per_line(120), 3);
        assert_eq!(custom_config.calculate_games_per_line(30), 1);
    }

    #[test]
    fn test_terminal_width_validation() {
        let config = CompactDisplayConfig::default();

        // Test sufficient width
        let validation = config.validate_terminal_width(80);
        match validation {
            TerminalWidthValidation::Sufficient {
                current_width,
                required_width,
                excess,
            } => {
                assert_eq!(current_width, 80);
                assert_eq!(required_width, 18); // team_name_width + score_width + margins
                assert_eq!(excess, 62);
            }
            _ => panic!("Expected sufficient validation"),
        }

        // Test insufficient width
        let validation = config.validate_terminal_width(10);
        match validation {
            TerminalWidthValidation::Insufficient {
                current_width,
                required_width,
                shortfall,
            } => {
                assert_eq!(current_width, 10);
                assert_eq!(required_width, 18);
                assert_eq!(shortfall, 8);
            }
            _ => panic!("Expected insufficient validation"),
        }
    }

    #[test]
    fn test_compact_mode_compatibility_validation() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            true,
            false,
        );

        // Test compatible page (no warnings)
        let validation = page.validate_compact_mode_compatibility();
        match validation {
            CompactModeValidation::Compatible => {
                // Expected for empty page
            }
            _ => panic!("Expected compatible validation for empty page"),
        }

        // Test page with error messages - now properly handled in compact mode
        page.add_error_message("Test error");
        let validation = page.validate_compact_mode_compatibility();
        match validation {
            CompactModeValidation::Compatible => {
                // Expected - error messages are now properly handled in compact mode
            }
            CompactModeValidation::CompatibleWithWarnings { warnings } => {
                // If there are warnings, they should be about other things, not error messages
                assert!(!warnings.iter().any(|w| w.contains("error messages")));
            }
            CompactModeValidation::Incompatible { .. } => {
                panic!("Unexpected incompatible validation for page with error messages");
            }
        }

        // Reset page for next test
        let mut many_games_page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            true,
            false,
        );

        // Test page with many games (manually create games to avoid testing_utils dependency)
        for i in 0..25 {
            let away_team_index = i + 1;
            let game = GameData {
                home_team: format!("Team{i}"),
                away_team: format!("Team{away_team_index}"),
                time: "18:30".to_string(),
                result: "1-0".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 3600,
                start: "2024-01-15T18:30:00Z".to_string(),
            };
            let game_data = GameResultData::new(&game);
            many_games_page.add_game_result(game_data);
        }

        let validation = many_games_page.validate_compact_mode_compatibility();
        match validation {
            CompactModeValidation::CompatibleWithWarnings { warnings } => {
                assert!(warnings.iter().any(|w| w.contains("Many games detected")));
            }
            _ => panic!("Expected warnings for page with many games"),
        }
    }

    #[test]
    fn test_team_abbreviation_comprehensive() {
        // Test all current Liiga teams
        assert_eq!(get_team_abbreviation("Tappara"), "TAP");
        assert_eq!(get_team_abbreviation("HIFK"), "IFK");
        assert_eq!(get_team_abbreviation("TPS"), "TPS");
        assert_eq!(get_team_abbreviation("JYP"), "JYP");
        assert_eq!(get_team_abbreviation("Ilves"), "ILV");
        assert_eq!(get_team_abbreviation("KalPa"), "KAL");
        assert_eq!(get_team_abbreviation("Kärpät"), "KÄR");
        assert_eq!(get_team_abbreviation("Lukko"), "LUK");
        assert_eq!(get_team_abbreviation("Pelicans"), "PEL");
        assert_eq!(get_team_abbreviation("SaiPa"), "SAI");
        assert_eq!(get_team_abbreviation("Sport"), "SPO");
        assert_eq!(get_team_abbreviation("HPK"), "HPK");
        assert_eq!(get_team_abbreviation("Jukurit"), "JUK");
        assert_eq!(get_team_abbreviation("Ässät"), "ÄSS");
        assert_eq!(get_team_abbreviation("KooKoo"), "KOO");

        // Test full team names
        assert_eq!(get_team_abbreviation("HIFK Helsinki"), "IFK");
        assert_eq!(get_team_abbreviation("TPS Turku"), "TPS");
        assert_eq!(get_team_abbreviation("Tampereen Tappara"), "TAP");
        assert_eq!(get_team_abbreviation("Tampereen Ilves"), "ILV");
        assert_eq!(get_team_abbreviation("Jyväskylän JYP"), "JYP");

        // Test case sensitivity (fallback extracts letters and uppercases)
        assert_eq!(get_team_abbreviation("tappara"), "TAP"); // "tappara" -> "TAP"
        assert_eq!(get_team_abbreviation("TAPPARA"), "TAP"); // "TAPPARA" -> "TAP"
        assert_eq!(get_team_abbreviation("TapPaRa"), "TAP"); // "TapPaRa" -> "TAP"

        // Test fallback for unknown teams (letters only, uppercase)
        assert_eq!(get_team_abbreviation("Unknown Team"), "UNK"); // "UnknownTeam" -> "UNK"
        assert_eq!(get_team_abbreviation("New Team"), "NEW"); // "NewTeam" -> "NEW"
        assert_eq!(get_team_abbreviation("Future Club"), "FUT"); // "FutureClub" -> "FUT"

        // Test special character handling (non-letters removed, uppercase)
        assert_eq!(get_team_abbreviation("HC Blues"), "HCB"); // "HCBlues" -> "HCB"
        assert_eq!(get_team_abbreviation("K-Espoo"), "KES"); // Known team (exact match)
        assert_eq!(get_team_abbreviation("HC-Jokers"), "HCJ"); // "HCJokers" -> "HCJ"
        assert_eq!(get_team_abbreviation("Team #1"), "TEA"); // "Team" -> "TEA"
        assert_eq!(get_team_abbreviation("123 ABC 456"), "ABC"); // "ABC" -> "ABC"
        assert_eq!(get_team_abbreviation("!@#$%"), "!@#$%"); // No letters -> return original

        // Test edge cases
        assert_eq!(get_team_abbreviation(""), ""); // No letters
        assert_eq!(get_team_abbreviation("A"), "A"); // Single letter
        assert_eq!(get_team_abbreviation("AB"), "AB"); // Two letters
        assert_eq!(get_team_abbreviation("ABC"), "ABC"); // Three letters
        assert_eq!(get_team_abbreviation("ABCD"), "ABC"); // Four letters -> truncate to 3
        assert_eq!(get_team_abbreviation("ABCDE"), "ABC"); // Five letters -> truncate to 3
    }

    #[test]
    fn test_compact_display_config_comprehensive() {
        // Test default configuration
        let config = CompactDisplayConfig::default();
        assert_eq!(config.team_name_width, 8);
        assert_eq!(config.score_width, 6);
        assert_eq!(config.max_games_per_line, 3);
        assert_eq!(config.game_separator, "  ");

        // Test custom configuration
        let custom_config = CompactDisplayConfig::new(3, 10, 8, " | ");
        assert_eq!(custom_config.max_games_per_line, 3);
        assert_eq!(custom_config.team_name_width, 10);
        assert_eq!(custom_config.score_width, 8);
        assert_eq!(custom_config.game_separator, " | ");

        // Test terminal width calculations (includes CONTENT_MARGIN * 2 = 4)
        assert_eq!(config.get_minimum_terminal_width(), 18); // 8 + 6 + 4
        assert_eq!(custom_config.get_minimum_terminal_width(), 22); // 10 + 8 + 4

        // Test games per line calculation with different terminal widths
        assert_eq!(config.calculate_games_per_line(80), 3); // Default max is 3, fits easily
        assert_eq!(custom_config.calculate_games_per_line(80), 3); // Can fit 3 games
        assert_eq!(custom_config.calculate_games_per_line(40), 1); // Can fit 1 game (corrected expectation)
        assert_eq!(custom_config.calculate_games_per_line(20), 1); // Can fit 1 game
        assert_eq!(custom_config.calculate_games_per_line(10), 1); // Too narrow but return 1

        // Test terminal width sufficiency
        assert!(config.is_terminal_width_sufficient(80));
        assert!(config.is_terminal_width_sufficient(18)); // Exactly minimum
        assert!(!config.is_terminal_width_sufficient(17)); // Below minimum
        assert!(!config.is_terminal_width_sufficient(0));
    }

    #[test]
    fn test_multi_column_compact_layout() {
        let config = CompactDisplayConfig::default();

        // Test 1 column layout (narrow terminal)
        assert_eq!(config.calculate_games_per_line(20), 1); // 20 chars total, 18 needed minimum

        // Test 2 column layout (medium terminal)
        // Each game: 8 (team) + 6 (score) = 14 chars
        // Two games: 14 + 2 (separator) + 14 = 30 chars content + 4 (margins) = 34 chars total
        assert_eq!(config.calculate_games_per_line(34), 2);
        assert_eq!(config.calculate_games_per_line(40), 2);

        // Test 3 column layout (wide terminal)
        // Three games: 14 + 2 + 14 + 2 + 14 = 46 chars content + 4 (margins) = 50 chars total
        assert_eq!(config.calculate_games_per_line(50), 3);
        assert_eq!(config.calculate_games_per_line(80), 3);
        assert_eq!(config.calculate_games_per_line(120), 3); // Max is 3, even on very wide terminals

        // Test edge cases
        assert_eq!(config.calculate_games_per_line(0), 1); // Always return at least 1
        assert_eq!(config.calculate_games_per_line(18), 1); // Exactly minimum width
        assert_eq!(config.calculate_games_per_line(33), 1); // Just under 2-column threshold
    }

    #[test]
    fn test_compact_mode_spacing() {
        let page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            false,
            false,
            true,
            false,
        );

        let config = CompactDisplayConfig::new(2, 8, 6, "  "); // 2 games per line

        // Create test rows
        let game1 = TeletextRow::GameResult {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "18:30".to_string(),
            result: "3-2".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            goal_events: Vec::new(),
            played_time: 0,
        };

        let game2 = TeletextRow::GameResult {
            home_team: "Blues".to_string(),
            away_team: "Jokerit".to_string(),
            time: "19:00".to_string(),
            result: "1-4".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            goal_events: Vec::new(),
            played_time: 0,
        };

        let game3 = TeletextRow::GameResult {
            home_team: "Lukko".to_string(),
            away_team: "KalPa".to_string(),
            time: "19:30".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: true,
            is_shootout: false,
            goal_events: Vec::new(),
            played_time: 0,
        };

        let rows = vec![&game1, &game2, &game3];
        let result = page.group_games_for_compact_display(&rows, &config, 80);

        // Should have:
        // Line 1: game1 + game2 (2 games per line)
        // Line 2: empty line for spacing
        // Line 3: game3 (remaining game)
        assert_eq!(result.len(), 3);
        assert!(!result[0].is_empty()); // First line with games
        assert!(result[1].is_empty()); // Empty spacing line
        assert!(!result[2].is_empty()); // Second line with remaining game

        // Verify the content contains expected teams (using correct abbreviations)
        assert!(result[0].contains("IFK"));
        assert!(result[0].contains("TAP")); // "Tappara" -> "TAP"
        assert!(result[0].contains("BLU")); // "Blues" -> "BLU" (fallback rule, uppercase)
        assert!(result[2].contains("LUK")); // "Lukko" -> "LUK"
    }

    #[test]
    fn test_compact_formatting_various_game_states() {
        let page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            true,
            false,
        );

        let config = CompactDisplayConfig::default();

        // Test scheduled game
        let scheduled_game = TeletextRow::GameResult {
            home_team: "Tappara".to_string(),
            away_team: "HIFK".to_string(),
            time: "18:30".to_string(),
            result: "".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            goal_events: vec![],
            played_time: 0,
        };

        let formatted = page.format_compact_game(&scheduled_game, &config);
        assert!(formatted.contains("TAP-IFK"));
        assert!(formatted.contains("18:30"));

        // Test ongoing game
        let ongoing_game = TeletextRow::GameResult {
            home_team: "Tappara".to_string(),
            away_team: "HIFK".to_string(),
            time: "18:30".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Ongoing,
            is_overtime: false,
            is_shootout: false,
            goal_events: vec![],
            played_time: 2400, // 40 minutes
        };

        let formatted = page.format_compact_game(&ongoing_game, &config);
        assert!(formatted.contains("TAP-IFK"));
        assert!(formatted.contains("1-0"));

        // Test final game with overtime
        let overtime_game = TeletextRow::GameResult {
            home_team: "Tappara".to_string(),
            away_team: "HIFK".to_string(),
            time: "18:30".to_string(),
            result: "3-2".to_string(),
            score_type: ScoreType::Final,
            is_overtime: true,
            is_shootout: false,
            goal_events: vec![],
            played_time: 3900,
        };

        let formatted = page.format_compact_game(&overtime_game, &config);
        assert!(formatted.contains("TAP-IFK"));
        assert!(formatted.contains("3-2 ja"));

        // Test final game with shootout
        let shootout_game = TeletextRow::GameResult {
            home_team: "Tappara".to_string(),
            away_team: "HIFK".to_string(),
            time: "18:30".to_string(),
            result: "4-3".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: true,
            goal_events: vec![],
            played_time: 3900,
        };

        let formatted = page.format_compact_game(&shootout_game, &config);
        assert!(formatted.contains("TAP-IFK"));
        assert!(formatted.contains("4-3 rl"));

        // Test non-game row (should return empty string)
        let error_row = TeletextRow::ErrorMessage("Error".to_string());

        let formatted = page.format_compact_game(&error_row, &config);
        assert_eq!(formatted, "");
    }

    #[test]
    fn test_header_truncation_logic() {
        let page = TeletextPage::new(
            1,
            "Test Title".to_string(),
            "Test Subheader".to_string(),
            false,
            false,
            false,
            true,
            false,
        );
        let config = CompactDisplayConfig::default();

        // Test short header - should not be truncated
        let short_header = TeletextRow::FutureGamesHeader("Short Header".to_string());
        let formatted = page.format_compact_game(&short_header, &config);
        assert!(formatted.contains("Short Header"));
        assert!(!formatted.contains("..."));

        // Test "Seuraavat ottelut" header - should be abbreviated to preserve date
        let future_games_header =
            TeletextRow::FutureGamesHeader("Seuraavat ottelut 07.08.".to_string());
        let formatted = page.format_compact_game(&future_games_header, &config);
        assert!(formatted.contains("Seur. ottelut 07.08."));
        assert!(!formatted.contains("Seuraavat ottelut"));
        assert!(!formatted.contains("..."));

        // Test header under 30 characters - should not be truncated
        let medium_header = TeletextRow::FutureGamesHeader("Medium length header text".to_string());
        let formatted = page.format_compact_game(&medium_header, &config);
        assert!(formatted.contains("Medium length header text"));
        assert!(!formatted.contains("..."));

        // Test header exactly 30 characters - should not be truncated
        let exact_header =
            TeletextRow::FutureGamesHeader("123456789012345678901234567890".to_string());
        let formatted = page.format_compact_game(&exact_header, &config);
        assert!(formatted.contains("123456789012345678901234567890"));
        assert!(!formatted.contains("..."));

        // Test header over 30 characters - should be truncated
        let very_long_header = TeletextRow::FutureGamesHeader(
            "This is a very long header that should be truncated at thirty chars".to_string(),
        );
        let formatted = page.format_compact_game(&very_long_header, &config);
        assert!(formatted.contains("This is a very long header tha..."));
        assert!(
            !formatted
                .contains("This is a very long header that should be truncated at thirty chars")
        );

        // Verify the truncated part length is 33 characters (30 + 3 for "...")
        let truncated_part = "This is a very long header tha...";
        assert_eq!(truncated_part.len(), 33);
    }

    // PHASE 4: WIDE MODE UNIT TESTS

    #[test]
    fn test_wide_mode_getter_setter() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // show_videos
            true,  // ignore_height_limit
            false, // compact_mode
            false, // wide_mode
            false, // enable_colors
        );

        // Test initial state
        assert!(!page.is_wide_mode());

        // Test setter
        assert!(page.set_wide_mode(true).is_ok());
        assert!(page.is_wide_mode());

        // Test setter again
        assert!(page.set_wide_mode(false).is_ok());
        assert!(!page.is_wide_mode());
    }

    #[test]
    fn test_can_fit_two_pages_false_when_wide_mode_disabled() {
        let page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // show_videos
            true,  // ignore_height_limit (non-interactive mode)
            false, // compact_mode
            false, // wide_mode - DISABLED
            false, // enable_colors
        );

        // Should return false when wide_mode is disabled, regardless of terminal width
        assert!(!page.can_fit_two_pages());
    }

    #[test]
    fn test_can_fit_two_pages_with_sufficient_width() {
        let page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // disable_video_links
            true,  // show_footer
            true,  // ignore_height_limit (non-interactive mode - uses 136 width)
            false, // compact_mode
            true,  // wide_mode - ENABLED
        );

        // Should return true when wide_mode is enabled and in non-interactive mode (136 >= 128)
        assert!(page.can_fit_two_pages());
    }

    #[test]
    fn test_can_fit_two_pages_with_insufficient_width() {
        // This test simulates a narrow terminal by using interactive mode
        // In interactive mode, crossterm::terminal::size() would be called,
        // but it will fallback to 80 chars if it can't get the size
        let page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // show_videos
            false, // ignore_height_limit (interactive mode - uses crossterm or fallback to 80)
            false, // compact_mode
            true,  // wide_mode - ENABLED
            false, // enable_colors
        );

        // Should return false when width is insufficient (80 < 128)
        // Note: This test relies on the fallback width of 80 when crossterm can't get terminal size
        assert!(!page.can_fit_two_pages());
    }

    #[test]
    fn test_distribute_games_for_wide_display_disabled() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // show_videos
            true,  // ignore_height_limit
            false, // compact_mode
            false, // wide_mode - DISABLED
            false, // enable_colors
        );

        // Add some test games using GameData -> GameResultData
        let test_game = GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "18:30".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T18:30:00Z".to_string(),
        };
        let test_game_data = GameResultData::new(&test_game);
        page.add_game_result(test_game_data);

        let (left_games, right_games) = page.distribute_games_for_wide_display();

        // When wide mode is disabled, all games should go to left column
        assert_eq!(left_games.len(), 1);
        assert_eq!(right_games.len(), 0);
    }

    #[test]
    fn test_distribute_games_for_wide_display_insufficient_width() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // show_videos
            false, // ignore_height_limit (interactive mode - narrow terminal)
            false, // compact_mode
            true,  // wide_mode - ENABLED but insufficient width
            false, // enable_colors
        );

        // Add some test games
        let test_game = GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "18:30".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T18:30:00Z".to_string(),
        };
        let test_game_data = GameResultData::new(&test_game);
        page.add_game_result(test_game_data);

        let (left_games, right_games) = page.distribute_games_for_wide_display();

        // When terminal width is insufficient, all games should go to left column
        assert_eq!(left_games.len(), 1);
        assert_eq!(right_games.len(), 0);
    }

    #[test]
    fn test_distribute_games_for_wide_display_enabled() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // disable_video_links
            true,  // show_footer
            true,  // ignore_height_limit (non-interactive mode - wide terminal)
            false, // compact_mode
            true,  // wide_mode - ENABLED
        );

        // Add multiple test games to test distribution
        for i in 0..4 {
            let test_game = GameData {
                home_team: format!("Team{i}A"),
                away_team: format!("Team{i}B"),
                time: "18:30".to_string(),
                result: "2-1".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 3600,
                start: "2024-01-15T18:30:00Z".to_string(),
            };
            let test_game_data = GameResultData::new(&test_game);
            page.add_game_result(test_game_data);
        }

        let (left_games, right_games) = page.distribute_games_for_wide_display();

        // With 4 games, balanced distribution should put 2 in left, 2 in right
        assert_eq!(left_games.len(), 2, "Left column should have 2 games");
        assert_eq!(right_games.len(), 2, "Right column should have 2 games");
        assert_eq!(
            left_games.len() + right_games.len(),
            4,
            "Total games should equal 4"
        );
    }

    #[test]
    fn test_distribute_games_for_wide_display_odd_number() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // disable_video_links
            true,  // show_footer
            true,  // ignore_height_limit (non-interactive mode - wide terminal)
            false, // compact_mode
            true,  // wide_mode - ENABLED
        );

        // Add 3 test games (odd number)
        for i in 0..3 {
            let test_game = GameData {
                home_team: format!("Team{i}A"),
                away_team: format!("Team{i}B"),
                time: "18:30".to_string(),
                result: "2-1".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 3600,
                start: "2024-01-15T18:30:00Z".to_string(),
            };
            let test_game_data = GameResultData::new(&test_game);
            page.add_game_result(test_game_data);
        }

        let (left_games, right_games) = page.distribute_games_for_wide_display();

        // With 3 games, balanced distribution should put 2 in left, 1 in right
        // (left column gets the extra game if odd number)
        assert_eq!(left_games.len(), 2, "Left column should have 2 games");
        assert_eq!(right_games.len(), 1, "Right column should have 1 game");
        assert_eq!(
            left_games.len() + right_games.len(),
            3,
            "Total games should equal 3"
        );
    }

    #[test]
    fn test_wide_mode_with_test_games() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // show_videos
            true,  // ignore_height_limit (wide terminal)
            false, // compact_mode
            true,  // wide_mode
            false, // enable_colors
        );

        // Add test games
        let test_game1 = GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "18:30".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T18:30:00Z".to_string(),
        };

        let test_game2 = GameData {
            home_team: "TPS".to_string(),
            away_team: "KalPa".to_string(),
            time: "19:00".to_string(),
            result: "1-3".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T19:00:00Z".to_string(),
        };

        page.add_game_result(GameResultData::new(&test_game1));
        page.add_game_result(GameResultData::new(&test_game2));

        // Test that games are properly distributed
        let (left_games, right_games) = page.distribute_games_for_wide_display();

        // Should have games distributed (exact distribution depends on content size)
        assert_eq!(
            left_games.len() + right_games.len(),
            2,
            "Should have both games distributed"
        );
        assert!(
            !left_games.is_empty(),
            "Should have at least one game in left column"
        );
    }

    #[test]
    fn test_teletext_page_config_mode_exclusivity() {
        // Test that new config has both modes disabled by default
        let config = TeletextPageConfig::new(221, "Test".to_string(), "Test".to_string());
        assert!(!config.compact_mode);
        assert!(!config.wide_mode);
        assert!(config.validate_mode_exclusivity().is_ok());

        // Test setter methods enforce mutual exclusivity
        let mut config = TeletextPageConfig::new(221, "Test".to_string(), "Test".to_string());

        // Enable compact mode
        config.set_compact_mode(true);
        assert!(config.compact_mode);
        assert!(!config.wide_mode);
        assert!(config.validate_mode_exclusivity().is_ok());

        // Enable wide mode - should disable compact mode
        config.set_wide_mode(true);
        assert!(!config.compact_mode);
        assert!(config.wide_mode);
        assert!(config.validate_mode_exclusivity().is_ok());

        // Enable compact mode again - should disable wide mode
        config.set_compact_mode(true);
        assert!(config.compact_mode);
        assert!(!config.wide_mode);
        assert!(config.validate_mode_exclusivity().is_ok());
    }

    #[test]
    fn test_teletext_page_config_validation() {
        // Test valid configurations
        let mut config = TeletextPageConfig::new(221, "Test".to_string(), "Test".to_string());
        assert!(config.validate_mode_exclusivity().is_ok());

        config.set_compact_mode(true);
        assert!(config.validate_mode_exclusivity().is_ok());

        config.set_wide_mode(true);
        assert!(config.validate_mode_exclusivity().is_ok());

        // Test invalid configuration (both modes enabled)
        let mut config = TeletextPageConfig::new(221, "Test".to_string(), "Test".to_string());
        config.compact_mode = true;
        config.wide_mode = true;
        assert!(config.validate_mode_exclusivity().is_err());
        assert_eq!(
            config.validate_mode_exclusivity().unwrap_err(),
            "compact_mode and wide_mode cannot be enabled simultaneously"
        );
    }

    #[test]
    fn test_teletext_page_mode_exclusivity() {
        let mut page = TeletextPage::new(
            221,
            "Test".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            false, // compact_mode
            false, // wide_mode
        );

        // Test initial state
        assert!(!page.is_compact_mode());
        assert!(!page.is_wide_mode());
        assert!(page.validate_mode_exclusivity().is_ok());

        // Test setter methods enforce mutual exclusivity
        assert!(page.set_compact_mode(true).is_ok());
        assert!(page.is_compact_mode());
        assert!(!page.is_wide_mode());
        assert!(page.validate_mode_exclusivity().is_ok());

        // Enable wide mode - should disable compact mode
        assert!(page.set_wide_mode(true).is_ok());
        assert!(!page.is_compact_mode());
        assert!(page.is_wide_mode());
        assert!(page.validate_mode_exclusivity().is_ok());

        // Enable compact mode again - should disable wide mode
        assert!(page.set_compact_mode(true).is_ok());
        assert!(page.is_compact_mode());
        assert!(!page.is_wide_mode());
        assert!(page.validate_mode_exclusivity().is_ok());
    }

    #[test]
    fn test_teletext_page_validation() {
        let mut page = TeletextPage::new(
            221,
            "Test".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            false,
            false,
        );

        // Test valid configurations
        assert!(page.validate_mode_exclusivity().is_ok());

        assert!(page.set_compact_mode(true).is_ok());
        assert!(page.validate_mode_exclusivity().is_ok());

        assert!(page.set_wide_mode(true).is_ok());
        assert!(page.validate_mode_exclusivity().is_ok());

        // Test invalid configuration (both modes enabled)
        let page = TeletextPage::new(
            221,
            "Test".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            true, // compact_mode
            true, // wide_mode
        );
        assert!(page.validate_mode_exclusivity().is_err());
        assert_eq!(
            page.validate_mode_exclusivity().unwrap_err(),
            "compact_mode and wide_mode cannot be enabled simultaneously"
        );
    }

    #[test]
    fn test_from_config_returns_error_with_invalid_config() {
        let mut config = TeletextPageConfig::new(221, "Test".to_string(), "Test".to_string());
        config.compact_mode = true;
        config.wide_mode = true;

        // This should return an error
        let result = TeletextPage::from_config(config);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(matches!(error, AppError::Config(_)));
        assert!(
            error
                .to_string()
                .contains("compact_mode and wide_mode cannot be enabled simultaneously")
        );
    }

    #[test]
    fn test_from_config_succeeds_with_valid_config() {
        let config = TeletextPageConfig::new(221, "Test".to_string(), "Test".to_string());

        // This should succeed
        let result = TeletextPage::from_config(config);
        assert!(result.is_ok());

        let page = result.unwrap();
        assert_eq!(page.page_number, 221);
        assert_eq!(page.title, "Test");
        assert_eq!(page.subheader, "Test");
        assert!(!page.is_compact_mode());
        assert!(!page.is_wide_mode());
    }

    #[test]
    fn test_setter_validation_conflicts() {
        let mut page = TeletextPage::new(
            221,
            "Test".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            false, // compact_mode
            false, // wide_mode
        );

        // Test that enabling compact mode when wide mode is active automatically disables wide mode
        page.set_wide_mode(true).unwrap();
        let result = page.set_compact_mode(true);
        assert!(result.is_ok());
        // Verify that wide mode was automatically disabled
        assert!(!page.is_wide_mode());
        assert!(page.is_compact_mode());

        // Test that enabling wide mode when compact mode is active automatically disables compact mode
        let mut page = TeletextPage::new(
            221,
            "Test".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            false, // compact_mode
            false, // wide_mode
        );
        page.set_compact_mode(true).unwrap();
        let result = page.set_wide_mode(true);
        assert!(result.is_ok());
        // Verify that compact mode was automatically disabled
        assert!(!page.is_compact_mode());
        assert!(page.is_wide_mode());

        // Test that disabling modes doesn't cause conflicts
        let mut page = TeletextPage::new(
            221,
            "Test".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            true,  // compact_mode
            false, // wide_mode
        );
        assert!(page.set_compact_mode(false).is_ok());
        assert!(page.set_wide_mode(false).is_ok());
    }
}
