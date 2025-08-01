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
async fn calculate_days_until_regular_season() -> Option<i64> {
    // Try to fetch the actual season start date from the API
    let config = match Config::load().await {
        Ok(config) => config,
        Err(_) => {
            // If config loading fails, we can't determine the start date
            return None;
        }
    };

    let client = Client::new();
    // Use UTC for consistent year calculation, convert to local for display logic
    let current_year = Utc::now().with_timezone(&Local).year();

    // Try current year first
    match fetch_regular_season_start_date(&client, &config, current_year).await {
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
    match fetch_regular_season_start_date(&client, &config, current_year + 1).await {
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
}

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
    ///     false
    /// );
    /// ```
    pub fn new(
        page_number: u16,
        title: String,
        subheader: String,
        disable_video_links: bool,
        show_footer: bool,
        ignore_height_limit: bool,
    ) -> Self {
        // Get terminal size, fallback to reasonable default if can't get size
        let screen_height = crossterm::terminal::size()
            .map(|(_, height)| height)
            .unwrap_or(24);

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
        }
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
    ///     false
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
    ///     false
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
    ///     false
    /// );
    ///
    /// page.add_error_message("Failed to fetch game data");
    /// ```
    pub fn add_error_message(&mut self, message: &str) {
        // Split message into lines and format each line
        let formatted_message = message
            .lines()
            .map(|line| line.trim())
            .collect::<Vec<_>>()
            .join("\n"); // Remove the indentation
        self.content_rows
            .push(TeletextRow::ErrorMessage(formatted_message));
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
    #[allow(dead_code)]
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

    /// Renders only the loading indicator area without redrawing the entire screen
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
                SetForegroundColor(Color::Yellow),
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
        if let Some(days_until_season) = calculate_days_until_regular_season().await {
            let countdown_text = format!("Runkosarjan alkuun {days_until_season} päivää");
            self.season_countdown = Some(countdown_text);
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
            let game_height = Self::calculate_game_height(game);

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
            let game_height = Self::calculate_game_height(game);
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
    ///     false
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
    ///     false
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
    ///     false
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
        // Hide cursor to prevent visual artifacts during rendering
        execute!(stdout, crossterm::cursor::Hide)?;

        // Get terminal dimensions
        let (width, _) = crossterm::terminal::size()?;

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
        let mut current_line = 4;
        let text_fg_code = get_ansi_code(text_fg(), 231);
        let result_fg_code = get_ansi_code(result_fg(), 46);

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
                                if let Some(url) = &event.video_clip_url {
                                    if !self.disable_video_links {
                                        buffer.push_str(&format!(
                                            "\x1b[38;5;{}m{:<12}\x1B]8;;{}\x07▶\x1B]8;;\x07",
                                            scorer_color, event.scorer_name, url
                                        ));
                                    } else {
                                        buffer.push_str(&format!(
                                            "\x1b[38;5;{}m{:<12}",
                                            scorer_color, event.scorer_name
                                        ));
                                    }
                                } else {
                                    buffer.push_str(&format!(
                                        "\x1b[38;5;{}m{:<12}",
                                        scorer_color, event.scorer_name
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
                                if let Some(url) = &event.video_clip_url {
                                    if !self.disable_video_links {
                                        buffer.push_str(&format!(
                                            "\x1b[38;5;{}m{:<12}\x1B]8;;{}\x07▶\x1B]8;;\x07",
                                            scorer_color, event.scorer_name, url
                                        ));
                                    } else {
                                        buffer.push_str(&format!(
                                            "\x1b[38;5;{}m{:<12}",
                                            scorer_color, event.scorer_name
                                        ));
                                    }
                                } else {
                                    buffer.push_str(&format!(
                                        "\x1b[38;5;{}m{:<12}",
                                        scorer_color, event.scorer_name
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

        // Add footer if enabled
        if self.show_footer {
            let footer_y = if self.ignore_height_limit {
                current_line + 1
            } else {
                self.screen_height.saturating_sub(1)
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

            // Add auto-refresh indicator if active
            let footer_text = if let Some(ref indicator) = self.auto_refresh_indicator {
                let indicator_frame = indicator.current_frame();
                format!("{controls} {indicator_frame}")
            } else {
                controls.to_string()
            };

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

    #[test]
    fn test_loading_indicator() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
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

            page.add_game_result(GameResultData::new(&crate::data_fetcher::GameData {
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

            page.add_game_result(GameResultData::new(&crate::data_fetcher::GameData {
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
        );

        // Test game without goals
        page.add_game_result(GameResultData::new(&crate::data_fetcher::GameData {
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

        page.add_game_result(GameResultData::new(&crate::data_fetcher::GameData {
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
        );

        // Test scheduled game display
        page.add_game_result(GameResultData::new(&crate::data_fetcher::GameData {
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

        page.add_game_result(GameResultData::new(&crate::data_fetcher::GameData {
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
        page.add_game_result(GameResultData::new(&crate::data_fetcher::GameData {
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
        page.add_game_result(GameResultData::new(&crate::data_fetcher::GameData {
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

        page.add_game_result(GameResultData::new(&crate::data_fetcher::GameData {
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
        );

        page_no_video.add_game_result(GameResultData::new(&crate::data_fetcher::GameData {
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
}
