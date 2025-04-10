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
const HEADER_BG: Color = Color::Blue;
const HEADER_FG: Color = Color::White;
const SUBHEADER_FG: Color = Color::Green;
const RESULT_FG: Color = Color::Yellow;
const TEXT_FG: Color = Color::White;
const HOME_SCORER_FG: Color = Color::Cyan;
const AWAY_SCORER_FG: Color = Color::Cyan;
const WINNING_GOAL_FG: Color = Color::Magenta;
const TELETEXT_WIDTH: u16 = 50; // Increased from 40 to accommodate longer names
const TEAM_NAME_WIDTH: usize = 15; // Increased from 10 to fit longer team names
const TITLE_BG: Color = Color::Green;

pub struct TeletextPage {
    page_number: u16,
    title: String,
    subheader: String,
    content_rows: Vec<TeletextRow>,
    current_page: usize,
    screen_height: u16,
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
    },
    ErrorMessage(String),
}

#[derive(Debug, Clone)]
pub enum ScoreType {
    Final,     // Final score
    Ongoing,   // Ongoing game with current score
    Scheduled, // Scheduled game with no score yet
}

impl TeletextPage {
    pub fn new(page_number: u16, title: String, subheader: String) -> Self {
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
        }
    }

    pub fn add_game_result(
        &mut self,
        home_team: String,
        away_team: String,
        time: String,
        result: String,
        score_type: ScoreType,
        is_overtime: bool,
        is_shootout: bool,
        goal_events: Vec<GoalEventData>,
    ) {
        self.content_rows.push(TeletextRow::GameResult {
            home_team,
            away_team,
            time,
            result,
            score_type,
            is_overtime,
            is_shootout,
            goal_events,
        });
    }

    pub fn add_error_message(&mut self, message: &str) {
        self.content_rows
            .push(TeletextRow::ErrorMessage(message.to_string()));
    }

    fn calculate_game_height(game: &TeletextRow) -> u16 {
        match game {
            TeletextRow::GameResult { goal_events, .. } => {
                let base_height = 1; // Game result line
                let home_scorers = goal_events.iter().filter(|e| e.is_home_team).count();
                let away_scorers = goal_events.iter().filter(|e| !e.is_home_team).count();
                let scorer_lines = home_scorers.max(away_scorers);
                let spacer = 1; // Space between games
                (base_height + scorer_lines as u16 + spacer) as u16
            }
            TeletextRow::ErrorMessage(_) => 2u16, // Error message + spacer
        }
    }

    fn get_page_content(&self) -> (Vec<&TeletextRow>, bool) {
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
            } else {
                if !current_page_items.is_empty() {
                    items_per_page.push(current_page_items.len());
                    current_page_items = vec![game];
                    current_height = game_height;
                }
            }
        }
        if !current_page_items.is_empty() {
            items_per_page.push(current_page_items.len());
        }

        // Calculate the starting index for the current page
        let mut start_idx = 0;
        for (page_idx, &items) in items_per_page.iter().enumerate() {
            if page_idx as usize == self.current_page {
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

    pub fn next_page(&mut self) {
        let total = self.total_pages();
        if total <= 1 {
            return;
        }
        self.current_page = (self.current_page + 1) % total;
    }

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

    pub fn render(&self, stdout: &mut Stdout) -> Result<(), Box<dyn std::error::Error>> {
        // Clear the screen
        execute!(stdout, Clear(ClearType::All))?;

        // Draw header with title having green background and rest blue
        execute!(
            stdout,
            MoveTo(0, 0),
            SetBackgroundColor(TITLE_BG),
            SetForegroundColor(HEADER_FG),
            Print(format!("{:<20}", self.title)),
            SetBackgroundColor(HEADER_BG),
            Print(format!("{:>30}", format!("SM-LIIGA {}", self.page_number))),
            ResetColor
        )?;

        // Draw subheader with pagination info on the right
        let total_pages = self.total_pages();
        let page_info = if total_pages > 1 {
            format!("{}/{}", self.current_page + 1, total_pages)
        } else {
            String::new()
        };

        execute!(
            stdout,
            MoveTo(0, 1),
            SetForegroundColor(SUBHEADER_FG),
            Print(format!("{:<20}", self.subheader)),
            Print(format!("{:>30}", page_info)),
            ResetColor
        )?;

        // Get content for current page
        let (visible_rows, _) = self.get_page_content();

        // Draw content with exact positioning
        let mut current_y = 3; // Start content one line after subheader
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
                } => {
                    let formatted_home = format!("{:<width$}", home_team, width = TEAM_NAME_WIDTH);
                    let formatted_away = format!("{:<width$}", away_team, width = TEAM_NAME_WIDTH);

                    let display_result = match score_type {
                        ScoreType::Scheduled => time.clone(),
                        _ => {
                            if *is_overtime {
                                format!("{} JA", result)
                            } else if *is_shootout {
                                format!("{} RL", result)
                            } else {
                                result.clone()
                            }
                        }
                    };

                    execute!(
                        stdout,
                        MoveTo(0, current_y),
                        SetForegroundColor(TEXT_FG),
                        Print(formatted_home),
                        Print(" - "),
                        Print(formatted_away),
                        SetForegroundColor(RESULT_FG),
                        Print(format!(" {}", display_result)),
                        ResetColor
                    )?;
                    current_y += 1;

                    // Display scorers if game has started and has goal events
                    if matches!(score_type, ScoreType::Ongoing | ScoreType::Final)
                        && !goal_events.is_empty()
                    {
                        let mut home_scorers: Vec<_> =
                            goal_events.iter().filter(|e| e.is_home_team).collect();
                        let mut away_scorers: Vec<_> =
                            goal_events.iter().filter(|e| !e.is_home_team).collect();
                        let max_scorers = home_scorers.len().max(away_scorers.len());

                        for i in 0..max_scorers {
                            // Home team scorer
                            if let Some(event) = home_scorers.get(i) {
                                let scorer_color =
                                    if event.is_winning_goal && (*is_overtime || *is_shootout) {
                                        WINNING_GOAL_FG
                                    } else {
                                        HOME_SCORER_FG
                                    };
                                execute!(
                                    stdout,
                                    MoveTo(0, current_y),
                                    SetForegroundColor(scorer_color),
                                    Print(format!("{:2} {:<15}", event.minute, event.scorer_name)),
                                    ResetColor
                                )?;
                            } else {
                                // Print empty space to align away team scorers
                                execute!(
                                    stdout,
                                    MoveTo(0, current_y),
                                    Print(format!("{:18}", "")),
                                )?;
                            }

                            // Away team scorer
                            if let Some(event) = away_scorers.get(i) {
                                let scorer_color =
                                    if event.is_winning_goal && (*is_overtime || *is_shootout) {
                                        WINNING_GOAL_FG
                                    } else {
                                        AWAY_SCORER_FG
                                    };
                                execute!(
                                    stdout,
                                    MoveTo(TEAM_NAME_WIDTH as u16 + 3, current_y),
                                    SetForegroundColor(scorer_color),
                                    Print(format!("{:2} {}", event.minute, event.scorer_name)),
                                    ResetColor
                                )?;
                            }

                            current_y += 1;
                        }
                    }

                    current_y += 1; // Add a spacer line after each game
                }
                TeletextRow::ErrorMessage(message) => {
                    execute!(
                        stdout,
                        MoveTo(0, current_y),
                        SetForegroundColor(RESULT_FG),
                        Print(format!(
                            "{:^width$}",
                            message,
                            width = TELETEXT_WIDTH as usize
                        )),
                        ResetColor
                    )?;
                    current_y += 2; // Message + spacer
                }
            }
        }

        // Simplified footer with just controls
        let controls = if total_pages > 1 {
            "q=Lopeta r=Päivitä ←→=Sivut"
        } else {
            "q=Lopeta r=Päivitä"
        };

        execute!(
            stdout,
            MoveTo(0, self.screen_height.saturating_sub(1)),
            SetForegroundColor(Color::Blue),
            Print("<<<"),
            SetForegroundColor(Color::White),
            Print(format!("{:^44}", controls)),
            SetForegroundColor(Color::Blue),
            Print(">>>"),
            ResetColor
        )?;

        stdout.flush()?;
        Ok(())
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect()
    }
}
