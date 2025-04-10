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
    items_per_page: usize,
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
        TeletextPage {
            page_number,
            title,
            subheader,
            content_rows: Vec::new(),
            current_page: 0,
            items_per_page: 3,
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

    pub fn next_page(&mut self) {
        if (self.current_page + 1) * self.items_per_page < self.content_rows.len() {
            self.current_page += 1;
        }
    }

    pub fn previous_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
        }
    }

    fn total_pages(&self) -> usize {
        (self.content_rows.len() + self.items_per_page - 1) / self.items_per_page
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

        // Draw subheader right under header
        execute!(
            stdout,
            MoveTo(0, 1),
            SetForegroundColor(SUBHEADER_FG),
            Print(format!(
                "{:<width$}",
                self.subheader,
                width = TELETEXT_WIDTH as usize
            )),
            ResetColor
        )?;

        // Calculate page bounds
        let start_idx = self.current_page * self.items_per_page;
        let end_idx = (start_idx + self.items_per_page).min(self.content_rows.len());
        let visible_rows = &self.content_rows[start_idx..end_idx];

        // Draw content with exact positioning
        let mut current_y = 3; // Start content one line after subheader
        for (index, row) in visible_rows.iter().enumerate() {
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

                    // Add a spacer line after each game except the last one
                    if index < visible_rows.len() - 1 {
                        current_y += 1;
                    }
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
                    current_y += 1;
                }
            }
        }

        // Add pagination info and controls to footer
        let total_pages = self.total_pages();
        let page_info = if total_pages > 1 {
            format!("Sivu {}/{}", self.current_page + 1, total_pages)
        } else {
            String::new()
        };

        let controls = if total_pages > 1 {
            "q=Lopeta r=Päivitä ←→=Sivut"
        } else {
            "q=Lopeta r=Päivitä"
        };

        execute!(
            stdout,
            MoveTo(0, current_y),
            SetForegroundColor(Color::Blue),
            Print("<<<"),
            SetForegroundColor(Color::White),
            Print(format!("{:^10}{:^24}{:^10}", "", controls, page_info)),
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
