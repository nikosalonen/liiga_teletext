// src/teletext_ui.rs - Updated with better display formatting

use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor, Attribute, SetAttribute},
    terminal::{Clear, ClearType, size},
    cursor::{MoveTo},
};
use std::io::{Write, Stdout};

// Constants for teletext appearance
const HEADER_BG: Color = Color::Blue;
const HEADER_FG: Color = Color::White;
const SUBHEADER_FG: Color = Color::Green;
const RESULT_FG: Color = Color::Yellow;
const TEXT_FG: Color = Color::White;

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

pub enum ScoreType {
    Final,       // Final score
    Ongoing,     // Ongoing game with current score
    Scheduled,   // Scheduled game with no score yet
    SeriesScore, // Series score (like "voitot 0-1")
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

    pub fn add_series_result(&mut self, home: String, away: String, time: String, result: String) {
        self.add_game_result(home, away, time, result, ScoreType::SeriesScore);
    }

    pub fn add_spacer(&mut self) {
        self.content_rows.push(TeletextRow::Spacer);
    }

    pub fn add_error_message(&mut self, message: &str) {
        self.content_rows.push(TeletextRow::ErrorMessage(message.to_string()));
    }

    pub fn set_pagination(&mut self, current: u16, total: u16) {
        self.current_page = current;
        self.total_pages = total;
    }

    pub fn render(&self, stdout: &mut Stdout) -> Result<(), Box<dyn std::error::Error>> {
        // Clear the screen
        execute!(stdout, Clear(ClearType::All))?;

        // Draw header
        let header_text = format!("{:<15} {:>16} {}/{}",
                                  self.title,
                                  format!("SM-LIIGA {}", self.page_number),
                                  self.current_page,
                                  self.total_pages
        );

        execute!(
            stdout,
            SetBackgroundColor(HEADER_BG),
            SetForegroundColor(HEADER_FG),
            SetAttribute(Attribute::Bold),
            Print(format!("{}\n", header_text)),
            ResetColor
        )?;

        // Draw subheader
        execute!(
            stdout,
            SetForegroundColor(SUBHEADER_FG),
            SetAttribute(Attribute::Bold),
            Print(format!("{}\n", self.subheader)),
            ResetColor
        )?;

        // Draw content
        for row in &self.content_rows {
            match row {
                TeletextRow::GameResult { home_team, away_team, time, result, score_type } => {
                    // Team names and time in white (fixed width formatting for teletext look)
                    let teams_formatted = format!("{:<12} - {:<12} {}", home_team, away_team, time);
                    execute!(
                        stdout,
                        SetForegroundColor(TEXT_FG),
                        Print(format!("{}\n", teams_formatted)),
                    )?;

                    // Format the result based on the score type
                    let result_text = match score_type {
                        ScoreType::Final => format!("{}", result),
                        ScoreType::Ongoing => format!("{} *", result), // Asterisk for ongoing
                        ScoreType::Scheduled => format!("-"), // Dash for scheduled
                        ScoreType::SeriesScore => format!("voitot {}", result),
                    };

                    execute!(
                        stdout,
                        SetForegroundColor(RESULT_FG),
                        Print(format!("{}\n", result_text)),
                        ResetColor
                    )?;
                },
                TeletextRow::ErrorMessage(message) => {
                    // Error messages in yellow (standard teletext alert color)
                    execute!(
                        stdout,
                        SetForegroundColor(RESULT_FG),
                        Print(format!("{}\n", message)),
                        ResetColor
                    )?;
                },
                TeletextRow::Spacer => {
                    execute!(stdout, Print("\n"))?;
                },
            }
        }

        // Footer with page navigation instructions
        execute!(
            stdout,
            MoveTo(0, 22), // Position near bottom of screen
            SetForegroundColor(Color::Blue),
            Print("<<< "),
            SetForegroundColor(Color::White),
            Print("q=Lopeta ←→=Selaa r=Päivitä"),
            SetForegroundColor(Color::Blue),
            Print(" >>>"),
            ResetColor
        )?;

        stdout.flush()?;
        Ok(())
    }
}