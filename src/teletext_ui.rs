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
    Color::AnsiValue(226)
} // Bright yellow
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
fn title_bg() -> Color {
    Color::AnsiValue(46)
} // Bright green

const TELETEXT_WIDTH: u16 = 55;
const TEAM_NAME_WIDTH: usize = 15;
const AWAY_TEAM_OFFSET: usize = 25; // Reduced from 30 to bring teams closer
const SEPARATOR_OFFSET: usize = 23; // New constant for separator position
const VIDEO_ICON: &str = " ▶";

pub struct TeletextPage {
    page_number: u16,
    title: String,
    subheader: String,
    content_rows: Vec<TeletextRow>,
    current_page: usize,
    screen_height: u16,
    disable_video_links: bool,
    show_footer: bool,
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
    pub fn new(
        page_number: u16,
        title: String,
        subheader: String,
        disable_video_links: bool,
        show_footer: bool,
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
            SetBackgroundColor(title_bg()),
            SetForegroundColor(header_fg()),
            Print(format!("{:<20}", self.title)),
            SetBackgroundColor(header_bg()),
            Print(format!("{:>35}", format!("SM-LIIGA {}", self.page_number))),
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
            SetForegroundColor(subheader_fg()),
            Print(format!("{:<20}", self.subheader)),
            Print(format!("{:>30}", page_info)),
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
                } => {
                    // Format result with overtime/shootout indicator
                    let result_text = if *is_shootout {
                        format!("{} rl", result)
                    } else if *is_overtime {
                        format!("{} ja", result)
                    } else {
                        result.clone()
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
                        SetForegroundColor(result_fg()),
                        MoveTo(45, current_y),
                        Print(match score_type {
                            ScoreType::Scheduled => time.as_str(),
                            _ => result_text.as_str(),
                        }),
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
                                let scorer_color =
                                    if event.is_winning_goal && (*is_overtime || *is_shootout) {
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
                                            Print(&event.scorer_name),
                                            Print(" "),
                                            Print("\x1B]8;;"),
                                            Print(url),
                                            Print("\x1B\\"),
                                            Print("▶"),
                                            Print("\x1B]8;;\x1B\\"),
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
                                        SetForegroundColor(result_fg()),
                                        Print(goal_type),
                                        ResetColor
                                    )?;
                                }
                            } else {
                                // Print empty space to align away team scorers
                                execute!(stdout, Print(format!("{:16}", "")),)?;
                            }

                            // Away team scorer
                            if let Some(event) = away_scorers.get(i) {
                                let scorer_color =
                                    if event.is_winning_goal && (*is_overtime || *is_shootout) {
                                        winning_goal_fg()
                                    } else {
                                        away_scorer_fg()
                                    };
                                execute!(
                                    stdout,
                                    MoveTo(AWAY_TEAM_OFFSET as u16, current_y), // Use new offset for away team
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
                                            Print(&event.scorer_name),
                                            Print(" "),
                                            Print("\x1B]8;;"),
                                            Print(url),
                                            Print("\x1B\\"),
                                            Print("▶"),
                                            Print("\x1B]8;;\x1B\\"),
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
                                        SetForegroundColor(result_fg()),
                                        Print(goal_type),
                                        ResetColor
                                    )?;
                                }
                            }

                            current_y += 1;
                        }
                    }

                    // Reduce space between games from 1 to 0
                }
                TeletextRow::ErrorMessage(message) => {
                    execute!(
                        stdout,
                        MoveTo(0, current_y),
                        SetForegroundColor(text_fg()),
                        Print(message),
                        ResetColor
                    )?;
                    current_y += 1; // Reduced from 2 to 1
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
                Print(format!("{:^49}", controls)),
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
        let mut page = TeletextPage::new(221, "TEST".to_string(), "TEST".to_string(), false, true);
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

            page.add_game_result(
                format!("Home {}", i),
                format!("Away {}", i),
                "18.00".to_string(),
                "1-1".to_string(),
                ScoreType::Final,
                false,
                false,
                goal_events,
            );
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
        let mut page = TeletextPage::new(221, "TEST".to_string(), "TEST".to_string(), false, true);
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

            page.add_game_result(
                format!("Home {}", i),
                format!("Away {}", i),
                "18.00".to_string(),
                "1-1".to_string(),
                ScoreType::Final,
                false,
                false,
                goal_events,
            );
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
        let mut page = TeletextPage::new(221, "TEST".to_string(), "TEST".to_string(), false, true);

        // Test game without goals
        page.add_game_result(
            "Home".to_string(),
            "Away".to_string(),
            "18.00".to_string(),
            "0-0".to_string(),
            ScoreType::Scheduled,
            false,
            false,
            vec![],
        );

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

        page.add_game_result(
            "Home".to_string(),
            "Away".to_string(),
            "18.00".to_string(),
            "1-0".to_string(),
            ScoreType::Final,
            false,
            false,
            goals,
        );

        let (content, _) = page.get_page_content();
        assert_eq!(content.len(), 2, "Should show both games");
    }

    #[test]
    fn test_error_message_display() {
        let mut page = TeletextPage::new(221, "TEST".to_string(), "TEST".to_string(), false, true);
        let error_msg = "Test Error";
        page.add_error_message(error_msg);

        let (content, _) = page.get_page_content();
        assert_eq!(content.len(), 1, "Should have one row");
        match &content[0] {
            TeletextRow::ErrorMessage(msg) => assert_eq!(msg, error_msg),
            _ => panic!("Should be an error message"),
        }
    }
}
