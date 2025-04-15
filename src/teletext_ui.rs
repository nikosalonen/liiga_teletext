// src/teletext_ui.rs - Updated with better display formatting

use crate::data_fetcher::GoalEventData;
use crossterm::{
    cursor::MoveTo,
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use std::io::{Stdout, Write};

// Constants for teletext appearance
fn header_bg() -> Color {
    Color::AnsiValue(21)
} // Bright blue
fn header_fg() -> Color {
    Color::AnsiValue(231)
} // Pure white
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
}

#[derive(Debug, Clone)]
pub enum ScoreType {
    Final,     // Final score
    Ongoing,   // Ongoing game with current score
    Scheduled, // Scheduled game with no score yet
}

/// Checks if there are any live/ongoing games in the provided game list.
///
/// # Arguments
/// * `games` - A slice of GameData containing game information
///
/// # Returns
/// * `bool` - true if there are any games with ScoreType::Ongoing, false otherwise
///
/// # Example
/// ```
/// let games = vec![game1, game2];
/// if has_live_games(&games) {
///     println!("There are live games in progress!");
/// }
/// ```
pub fn has_live_games(games: &[crate::data_fetcher::GameData]) -> bool {
    games
        .iter()
        .any(|game| matches!(game.score_type, ScoreType::Ongoing))
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
    /// let game_data = fetch_game_data();
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
        }
    }

    /// Updates the page layout when terminal size changes.
    /// Recalculates content positioning and pagination based on new dimensions.
    ///
    /// # Example
    /// ```
    /// // When terminal is resized
    /// page.handle_resize();
    /// page.render(&mut stdout)?;
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
    /// let game = GameResultData::new(&fetched_game);
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
    /// * `Result<(), Box<dyn std::error::Error>>` - Ok if rendering succeeded, Err otherwise
    ///
    /// # Example
    /// ```
    /// let mut stdout = stdout();
    /// page.render(&mut stdout)?;
    /// ```
    pub fn render(&self, stdout: &mut Stdout) -> Result<(), Box<dyn std::error::Error>> {
        // Clear the screen
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
            Print(format!("{:>width$}", format!("SM-LIIGA {}", self.page_number), width = (width as usize).saturating_sub(20))),
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
            Print(format!("{:>width$}", page_info, width = (width as usize).saturating_sub(20))),
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
                    for (i, line) in message.lines().enumerate() {
                        execute!(
                            stdout,
                            MoveTo(0, current_y),
                            SetForegroundColor(text_fg()),
                            Print(if i == 0 {
                                "Virhe haettaessa otteluita:"
                            } else {
                                line
                            }),
                            ResetColor
                        )?;
                        current_y += 1;
                    }
                }
            }
        }

        // Only render footer if show_footer is true
        if self.show_footer {
            let controls = if total_pages > 1 {
                "q=Lopeta ←→=Sivut"
            } else {
                "q=Lopeta"
            };

            execute!(
                stdout,
                MoveTo(0, self.screen_height.saturating_sub(1)),
                SetBackgroundColor(header_bg()),
                SetForegroundColor(Color::Blue),
                Print(if total_pages > 1 { "<<<" } else { "   " }),
                SetForegroundColor(Color::White),
                Print(format!("{:^width$}", controls, width = (width as usize).saturating_sub(6))),
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
        page.screen_height = 20; // Set fixed screen height for testing

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
                goal_events: goal_events,
                played_time: 1200,
                serie: "RUNKOSARJA".to_string(),
                finished_type: String::new(),
                log_time: String::new(),
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
        page.screen_height = 20; // Set fixed screen height for testing

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
                goal_events: goal_events,
                played_time: 1200,
                serie: "RUNKOSARJA".to_string(),
                finished_type: String::new(),
                log_time: String::new(),
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
            false,
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
            finished_type: String::new(),
            log_time: String::new(),
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
            finished_type: String::new(),
            log_time: String::new(),
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
            false,
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
            finished_type: String::new(),
            log_time: String::new(),
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
            finished_type: String::new(),
            log_time: String::new(),
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
            finished_type: String::new(),
            log_time: String::new(),
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
            finished_type: String::new(),
            log_time: String::new(),
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
            finished_type: String::new(),
            log_time: String::new(),
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
            goal_events: goal_events,
            played_time: 3600,
            serie: "RUNKOSARJA".to_string(),
            finished_type: String::new(),
            log_time: String::new(),
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
