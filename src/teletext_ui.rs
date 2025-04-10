// src/teletext_ui.rs - Updated with better display formatting

use crossterm::{
    cursor::MoveTo,
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType, size},
};
use std::io::{Stdout, Write};

// Constants for teletext appearance
const HEADER_BG: Color = Color::Blue;
const HEADER_FG: Color = Color::White;
const SUBHEADER_FG: Color = Color::Green;
const RESULT_FG: Color = Color::Yellow;
const TEXT_FG: Color = Color::White;
const TELETEXT_WIDTH: u16 = 40; // Standard teletext width
const TEAM_NAME_WIDTH: usize = 10; // Fixed width for team names

pub struct TeletextPage {
    page_number: u16,
    title: String,
    subheader: String,
    content_rows: Vec<TeletextRow>,
    current_page: u16,
    total_pages: u16,
}

pub enum TeletextRow {
    GameResult {
        home_team: String,
        away_team: String,
        time: String,
        result: String,
        score_type: ScoreType,
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
            current_page: 1,
            total_pages: 1,
        }
    }

    pub fn add_game_result(
        &mut self,
        home: String,
        away: String,
        time: String,
        result: String,
        score_type: ScoreType,
    ) {
        self.content_rows.push(TeletextRow::GameResult {
            home_team: home,
            away_team: away,
            time,
            result,
            score_type,
        });
    }

    pub fn add_spacer(&mut self) {
        self.content_rows.push(TeletextRow::Spacer);
    }

    pub fn add_error_message(&mut self, message: &str) {
        self.content_rows
            .push(TeletextRow::ErrorMessage(message.to_string()));
    }

    pub fn set_pagination(&mut self, current: u16, total: u16) {
        self.current_page = current;
        self.total_pages = total;
    }

    pub fn render(&self, stdout: &mut Stdout) -> Result<(), Box<dyn std::error::Error>> {
        // Clear the screen
        execute!(stdout, Clear(ClearType::All))?;

        // Draw header with full width blue background
        execute!(
            stdout,
            MoveTo(0, 0),
            SetBackgroundColor(HEADER_BG),
            SetForegroundColor(HEADER_FG),
        )?;

        let header_text = format!(
            "{:<15} {:>15} {:>8}",
            self.title,
            format!("SM-LIIGA {}", self.page_number),
            format!("{}/{}", self.current_page, self.total_pages)
        );

        execute!(
            stdout,
            Print(format!(
                "{:width$}",
                header_text,
                width = TELETEXT_WIDTH as usize
            )),
            ResetColor
        )?;

        // Draw subheader right under header
        execute!(
            stdout,
            MoveTo(0, 1),
            SetForegroundColor(SUBHEADER_FG),
            Print(format!(
                "{:^width$}",
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
                } => {
                    let formatted_home = format!("{:<width$}", home_team, width = TEAM_NAME_WIDTH);
                    let formatted_away = format!("{:<width$}", away_team, width = TEAM_NAME_WIDTH);

                    let display_result = match score_type {
                        ScoreType::Scheduled => time.clone(),
                        _ => result.clone(),
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

        // Footer at the bottom
        execute!(
            stdout,
            MoveTo(0, current_y + 1),
            SetForegroundColor(Color::Blue),
            Print("<<<  "),
            SetForegroundColor(Color::White),
            Print("q=Lopeta  ←→=Selaa  r=Päivitä"),
            SetForegroundColor(Color::Blue),
            Print("  >>>"),
            ResetColor
        )?;

        stdout.flush()?;
        Ok(())
    }
}
