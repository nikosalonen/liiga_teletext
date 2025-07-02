// src/teletext_ui.rs - Updated with better display formatting

use crate::config::Config;
use crate::data_fetcher::GoalEventData;
use crate::data_fetcher::api::fetch_regular_season_start_date;
use crate::error::AppError;
use chrono::{DateTime, Datelike, Local, Utc};
use crossterm::{
    cursor::MoveTo,
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType},
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

/// Creates a September date string for the given year and day.
/// This is used for generating season start date candidates.
///
/// # Arguments
/// * `year` - The year (e.g., 2024)
/// * `day` - The day of September (1-30)
///
/// # Returns
/// * `String` - The formatted date string (e.g., "2024-09-01")
///
/// # Example
/// ```
/// use liiga_teletext::teletext_ui::create_september_date;
///
/// let date = create_september_date(2024, 1);
/// assert_eq!(date, "2024-09-01");
/// ```
pub fn create_september_date(year: i32, day: u32) -> String {
    format!("{}-09-{:02}", year, day)
}

/// Calculates the number of days until the regular season starts.
/// Returns None if the regular season has already started or if we can't determine the start date.
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
    let current_year = Local::now().year();

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

#[derive(Debug, Clone)]
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
    ///     page.render(&mut stdout)?;
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

    /// Sets the screen height for testing purposes.
    /// This method is primarily used in tests to avoid terminal size detection issues.
    #[allow(dead_code)]
    pub fn set_screen_height(&mut self, height: u16) {
        self.screen_height = height;
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
            let countdown_text = format!("Runkosarjan alkuun {} päivää", days_until_season);
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

    fn total_pages(&self) -> usize {
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
    /// // Only render if we have a proper terminal (skip in CI)
    /// if std::env::var("CI").is_err() {
    ///     let mut stdout = stdout();
    ///     page.render(&mut stdout)?;
    /// }
    /// # Ok::<(), liiga_teletext::AppError>(())
    /// ```
    pub fn render(&self, stdout: &mut Stdout) -> Result<(), AppError> {
        // Always clear the screen to ensure proper rendering
        execute!(stdout, Clear(ClearType::All))?;

        // Draw header with title having green background and rest blue
        let (width, _) = crossterm::terminal::size()?;
        execute!(
            stdout,
            MoveTo(0, 0),
            SetBackgroundColor(title_bg()),
            SetForegroundColor(header_fg()),
            Print(format!("{:<20}", self.title)),
            SetBackgroundColor(header_bg()),
            SetForegroundColor(Color::AnsiValue(231)), // Pure white
            Print(format!(
                "{:>width$}",
                format!("SM-LIIGA {}", self.page_number),
                width = (width as usize).saturating_sub(20)
            )),
            ResetColor
        )?;

        // Draw subheader with pagination info on the right
        let total_pages = self.total_pages();
        let page_info = if total_pages > 1 && !self.ignore_height_limit {
            format!("{}/{}", self.current_page + 1, total_pages)
        } else {
            String::new()
        };

        execute!(
            stdout,
            MoveTo(0, 1),
            SetForegroundColor(subheader_fg()),
            Print(format!("{:<20}", self.subheader)),
            Print(format!(
                "{:>width$}",
                page_info,
                width = (width as usize).saturating_sub(20)
            )),
            ResetColor
        )?;

        // Get content for current page
        let (visible_rows, _) = self.get_page_content();

        // Draw content with exact positioning
        let mut current_y = 3; // Start content after one row space from subheader

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
                        format!("{} rl", result)
                    } else if *is_overtime {
                        format!("{} ja", result)
                    } else {
                        result.clone()
                    };

                    // Format played time for ongoing games
                    let formatted_time = format!("{:02}:{:02}", played_time / 60, played_time % 60);
                    let ongoing_display = format!("{} {}", formatted_time, result_text);
                    let time_display = match score_type {
                        ScoreType::Scheduled => time.as_str(),
                        ScoreType::Ongoing => ongoing_display.as_str(),
                        ScoreType::Final => result_text.as_str(),
                    };

                    // Draw game result line
                    execute!(
                        stdout,
                        MoveTo(0, current_y),
                        SetForegroundColor(text_fg()),
                        Print(format!(
                            "{:<20}",
                            home_team.chars().take(20).collect::<String>()
                        )),
                        MoveTo(SEPARATOR_OFFSET as u16, current_y),
                        Print("- "),
                        MoveTo(AWAY_TEAM_OFFSET as u16, current_y),
                        Print(format!(
                            "{:<20}",
                            away_team.chars().take(20).collect::<String>()
                        )),
                        SetForegroundColor(match score_type {
                            ScoreType::Final => result_fg(),
                            ScoreType::Ongoing => text_fg(),
                            ScoreType::Scheduled => text_fg(),
                        }),
                        MoveTo(45, current_y),
                        Print(time_display),
                        ResetColor
                    )?;

                    current_y += 1;

                    // Draw goal events if game has started
                    if matches!(score_type, ScoreType::Ongoing | ScoreType::Final)
                        && !goal_events.is_empty()
                    {
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
                                    winning_goal_fg()
                                } else {
                                    home_scorer_fg()
                                };
                                execute!(
                                    stdout,
                                    MoveTo(0, current_y),
                                    SetForegroundColor(scorer_color),
                                    Print(format!("{:2}", event.minute)),
                                )?;

                                // If there's a video clip and video links are not disabled, make the scorer name a clickable link
                                if let Some(url) = &event.video_clip_url {
                                    if !self.disable_video_links {
                                        execute!(
                                            stdout,
                                            Print(" "),
                                            SetForegroundColor(scorer_color),
                                            Print(format!("{:<12}", event.scorer_name)),
                                            Print("\x1B]8;;"),
                                            Print(url),
                                            Print("\x07"),
                                            Print("▶"),
                                            Print("\x1B]8;;\x07"),
                                            ResetColor
                                        )?;
                                    } else {
                                        execute!(
                                            stdout,
                                            Print(" "),
                                            SetForegroundColor(scorer_color),
                                            Print(format!("{:<12}", event.scorer_name)),
                                            ResetColor
                                        )?;
                                    }
                                } else {
                                    execute!(
                                        stdout,
                                        Print(" "),
                                        SetForegroundColor(scorer_color),
                                        Print(format!("{:<12}", event.scorer_name)),
                                        ResetColor
                                    )?;
                                }

                                // Add goal type indicators if present
                                let goal_type = event.get_goal_type_display();
                                if !goal_type.is_empty() {
                                    execute!(
                                        stdout,
                                        Print(" "),
                                        SetForegroundColor(goal_type_fg()),
                                        Print(goal_type),
                                        ResetColor
                                    )?;
                                }
                            }

                            // Away team scorer
                            if let Some(event) = away_scorers.get(i) {
                                let scorer_color = if (event.is_winning_goal
                                    && (*is_overtime || *is_shootout))
                                    || event.goal_types.contains(&"VL".to_string())
                                {
                                    winning_goal_fg()
                                } else {
                                    away_scorer_fg()
                                };
                                execute!(
                                    stdout,
                                    MoveTo(AWAY_TEAM_OFFSET as u16, current_y),
                                    SetForegroundColor(scorer_color),
                                    Print(format!("{:2}", event.minute)),
                                )?;

                                // If there's a video clip and video links are not disabled, make the scorer name a clickable link
                                if let Some(url) = &event.video_clip_url {
                                    if !self.disable_video_links {
                                        execute!(
                                            stdout,
                                            Print(" "),
                                            SetForegroundColor(scorer_color),
                                            Print(format!("{:<12}", event.scorer_name)),
                                            Print("\x1B]8;;"),
                                            Print(url),
                                            Print("\x07"),
                                            Print("▶"),
                                            Print("\x1B]8;;\x07"),
                                            ResetColor
                                        )?;
                                    } else {
                                        execute!(
                                            stdout,
                                            Print(" "),
                                            SetForegroundColor(scorer_color),
                                            Print(format!("{:<12}", event.scorer_name)),
                                            ResetColor
                                        )?;
                                    }
                                } else {
                                    execute!(
                                        stdout,
                                        Print(" "),
                                        SetForegroundColor(scorer_color),
                                        Print(format!("{:<12}", event.scorer_name)),
                                        ResetColor
                                    )?;
                                }

                                // Add goal type indicators if present
                                let goal_type = event.get_goal_type_display();
                                if !goal_type.is_empty() {
                                    execute!(
                                        stdout,
                                        Print(" "),
                                        SetForegroundColor(goal_type_fg()),
                                        Print(goal_type),
                                        ResetColor
                                    )?;
                                }
                            }

                            current_y += 1;
                        }
                    }

                    // Add a blank line between games, but only if not the last game and not in single-view mode
                    if !self.ignore_height_limit {
                        current_y += 1;
                    }
                }
                TeletextRow::ErrorMessage(message) => {
                    execute!(
                        stdout,
                        MoveTo(0, current_y),
                        SetForegroundColor(text_fg()),
                        Print("Virhe haettaessa otteluita:"),
                        ResetColor
                    )?;
                    current_y += 1;
                    for line in message.lines() {
                        execute!(
                            stdout,
                            MoveTo(0, current_y),
                            SetForegroundColor(text_fg()),
                            Print(line),
                            ResetColor
                        )?;
                        current_y += 1;
                    }
                }
                TeletextRow::FutureGamesHeader(header_text) => {
                    execute!(
                        stdout,
                        MoveTo(0, current_y),
                        SetForegroundColor(subheader_fg()),
                        Print(header_text),
                        ResetColor
                    )?;
                    current_y += 1;
                }
            }
        }

        // Render footer area if show_footer is true
        if self.show_footer {
            let footer_y = self.screen_height.saturating_sub(1);
            let countdown_y = footer_y.saturating_sub(2);
            let empty_y = footer_y.saturating_sub(1);

            // Show countdown two lines above the blue bar, if available
            if let Some(ref countdown_text) = self.season_countdown {
                // Center the countdown text
                let countdown_width = countdown_text.chars().count();
                let left_padding = if width as usize > countdown_width {
                    (width as usize - countdown_width) / 2
                } else {
                    0
                };
                execute!(
                    stdout,
                    MoveTo(0, countdown_y),
                    SetForegroundColor(subheader_fg()),
                    Print(format!(
                        "{space:>pad$}{text}",
                        space = "",
                        pad = left_padding,
                        text = countdown_text
                    )),
                    ResetColor
                )?;
            }

            // Always print an empty line above the blue bar
            execute!(stdout, MoveTo(0, empty_y), Print(""))?;

            let mut controls = if total_pages > 1 {
                "q=Lopeta ←→=Sivut"
            } else {
                "q=Lopeta"
            };

            // Add auto-refresh status if disabled
            if self.auto_refresh_disabled {
                controls = if total_pages > 1 {
                    "q=Lopeta ←→=Sivut (Ei päivity)"
                } else {
                    "q=Lopeta (Ei päivity)"
                };
            }

            execute!(
                stdout,
                MoveTo(0, footer_y),
                SetBackgroundColor(header_bg()),
                SetForegroundColor(Color::Blue),
                Print(if total_pages > 1 { "<<<" } else { "   " }),
                SetForegroundColor(Color::White),
                Print(format!(
                    "{:^width$}",
                    controls,
                    width = (width as usize).saturating_sub(6)
                )),
                SetForegroundColor(Color::Blue),
                Print(if total_pages > 1 { ">>>" } else { "   " }),
                ResetColor
            )?;
        }

        stdout.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::GoalEventData;

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
                    scorer_player_id: i as i64,
                    scorer_name: format!("Scorer {}", i),
                    minute: 10,
                    home_team_score: 1,
                    away_team_score: 0,
                    is_winning_goal: false,
                    goal_types: vec![],
                    is_home_team: true,
                    video_clip_url: None,
                },
                GoalEventData {
                    scorer_player_id: (i + 100) as i64,
                    scorer_name: format!("Scorer {}", i + 100),
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
                home_team: format!("Home {}", i),
                away_team: format!("Away {}", i),
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
                    scorer_player_id: i as i64,
                    scorer_name: format!("Scorer {}", i),
                    minute: 10,
                    home_team_score: 1,
                    away_team_score: 0,
                    is_winning_goal: false,
                    goal_types: vec![],
                    is_home_team: true,
                    video_clip_url: None,
                },
                GoalEventData {
                    scorer_player_id: (i + 100) as i64,
                    scorer_name: format!("Scorer {}", i + 100),
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
                home_team: format!("Home {}", i),
                away_team: format!("Away {}", i),
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
}
