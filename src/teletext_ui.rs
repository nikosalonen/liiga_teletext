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
    Spacer,
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
        let mut result_text = result.clone();
        if is_overtime {
            result_text.push_str(" ja");
        } else if is_shootout {
            result_text.push_str(" rl");
        }

        let _line = format!(
            "{:<14} - {:<14} {} {}",
            truncate(&home_team, 14),
            truncate(&away_team, 14),
            time,
            result_text
        );
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

    pub fn add_spacer(&mut self) {
        self.content_rows.push(TeletextRow::Spacer);
    }

    pub fn add_error_message(&mut self, message: &str) {
        self.content_rows
            .push(TeletextRow::ErrorMessage(message.to_string()));
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

        // Draw content with exact positioning
        let mut current_y = 3; // Start content one line after subheader
        for row in self.content_rows.iter() {
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
                        for event in goal_events {
                            let indent = if event.is_home_team {
                                0 // Align directly under home team
                            } else {
                                TEAM_NAME_WIDTH + 3 // Align directly under away team (after " - ")
                            };

                            let scorer_color =
                                if event.is_winning_goal && (*is_overtime || *is_shootout) {
                                    WINNING_GOAL_FG
                                } else if event.is_home_team {
                                    HOME_SCORER_FG
                                } else {
                                    AWAY_SCORER_FG
                                };

                            execute!(
                                stdout,
                                MoveTo(indent as u16, current_y),
                                SetForegroundColor(scorer_color),
                                Print(format!("{:2} {}", event.minute, event.scorer_name)),
                                ResetColor
                            )?;
                            current_y += 1;
                        }
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
                TeletextRow::Spacer => {
                    current_y += 1;
                }
            }
        }

        // Footer right after content
        execute!(
            stdout,
            MoveTo(0, current_y),
            SetForegroundColor(Color::Blue),
            Print("<<<"),
            SetForegroundColor(Color::White),
            Print(format!("{:^34}", "q=Lopeta  r=Päivitä")),
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
