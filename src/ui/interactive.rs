//! Interactive UI module for the liiga_teletext application
//!
//! This module contains the main interactive UI loop that was previously in main.rs.
//! It handles user input, screen updates, and the main application flow.

use crate::data_fetcher::{fetch_liiga_data, GameData};
use crate::error::AppError;
use crate::teletext_ui::{GameResultData, TeletextPage};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::stdout;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Calculates a hash of the games data for change detection
fn calculate_games_hash(games: &[GameData]) -> u64 {
    let mut hasher = DefaultHasher::new();
    games.hash(&mut hasher);
    hasher.finish()
}

/// Runs the interactive UI with adaptive polling and change detection
pub async fn run_interactive_ui(
    date: Option<String>,
    disable_video_links: bool,
    debug_mode: bool,
) -> Result<(), AppError> {
    if !debug_mode {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen)?;
    }

    let mut current_page = 0usize;
    let mut pages: Vec<TeletextPage>;
    let mut last_refresh = Instant::now();
    let mut last_games_hash = 0u64;
    let mut last_activity = Instant::now();
    let mut current_date = date;
    let mut needs_render = true;

    // Initial data fetch
    match fetch_liiga_data(current_date.clone()).await {
        Ok((games, fetched_date)) => {
            current_date = Some(fetched_date.clone());
            let games_hash = calculate_games_hash(&games);
            last_games_hash = games_hash;

            if games.is_empty() {
                let mut page = TeletextPage::new(
                    221,
                    "JÄÄKIEKKO".to_string(),
                    "SM-LIIGA".to_string(),
                    disable_video_links,
                    true,
                    false,
                );
                page.add_error_message("Ei otteluita tälle päivälle");
                page.set_fetched_date(fetched_date);
                pages = vec![page];
            } else {
                // Create pages from games
                let mut page = TeletextPage::new(
                    221,
                    "JÄÄKIEKKO".to_string(),
                    "SM-LIIGA".to_string(),
                    disable_video_links,
                    true,
                    false,
                );
                page.set_fetched_date(fetched_date);

                for game in &games {
                    page.add_game_result(GameResultData::new(game));
                }

                pages = vec![page];
            }
        }
        Err(e) => {
            warn!("Failed to fetch initial data: {}", e);
            let mut page = TeletextPage::new(
                221,
                "JÄÄKIEKKO".to_string(),
                "SM-LIIGA".to_string(),
                disable_video_links,
                true,
                false,
            );
            page.add_error_message(&format!("Virhe tietojen haussa: {}", e));
            pages = vec![page];
        }
    }

    loop {
        // Adaptive polling based on user activity
        let idle_duration = last_activity.elapsed();
        let poll_interval = if idle_duration < Duration::from_secs(5) {
            Duration::from_millis(50) // Active use
        } else if idle_duration < Duration::from_secs(30) {
            Duration::from_millis(200) // Semi-active
        } else {
            Duration::from_millis(500) // Idle
        };

        // Check for user input
        if event::poll(poll_interval)? {
            if let Event::Key(key_event) = event::read()? {
                last_activity = Instant::now();

                match key_event.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => break,
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        // Manual refresh with cooldown
                        if last_refresh.elapsed() >= Duration::from_secs(10) {
                            info!("Manual refresh requested");
                            match fetch_liiga_data(current_date.clone()).await {
                                Ok((games, fetched_date)) => {
                                    let games_hash = calculate_games_hash(&games);
                                    if games_hash != last_games_hash {
                                        last_games_hash = games_hash;
                                        current_date = Some(fetched_date.clone());

                                        // Rebuild pages
                                        if games.is_empty() {
                                            let mut page = TeletextPage::new(
                                                221,
                                                "JÄÄKIEKKO".to_string(),
                                                "SM-LIIGA".to_string(),
                                                disable_video_links,
                                                true,
                                                false,
                                            );
                                            page.add_error_message("Ei otteluita tälle päivälle");
                                            page.set_fetched_date(fetched_date);
                                            pages = vec![page];
                                        } else {
                                            let mut page = TeletextPage::new(
                                                221,
                                                "JÄÄKIEKKO".to_string(),
                                                "SM-LIIGA".to_string(),
                                                disable_video_links,
                                                true,
                                                false,
                                            );
                                            page.set_fetched_date(fetched_date);

                                            for game in &games {
                                                page.add_game_result(GameResultData::new(game));
                                            }

                                            pages = vec![page];
                                        }

                                        current_page = 0;
                                        needs_render = true;
                                    }
                                    last_refresh = Instant::now();
                                }
                                Err(e) => {
                                    warn!("Manual refresh failed: {}", e);
                                    last_refresh = Instant::now(); // Maintain cooldown even on failure
                                }
                            }
                        } else {
                            debug!("Manual refresh ignored due to cooldown");
                        }
                    }
                    KeyCode::Left => {
                        if current_page > 0 {
                            current_page -= 1;
                            needs_render = true;
                        }
                    }
                    KeyCode::Right => {
                        if current_page + 1 < pages.len() {
                            current_page += 1;
                            needs_render = true;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Auto-refresh logic
        let should_refresh = last_refresh.elapsed() >= Duration::from_secs(60);
        if should_refresh {
            debug!("Auto-refresh triggered");
            match fetch_liiga_data(current_date.clone()).await {
                Ok((games, fetched_date)) => {
                    let games_hash = calculate_games_hash(&games);
                    if games_hash != last_games_hash {
                        last_games_hash = games_hash;
                        current_date = Some(fetched_date.clone());

                        // Rebuild pages
                        if games.is_empty() {
                            let mut page = TeletextPage::new(
                                221,
                                "JÄÄKIEKKO".to_string(),
                                "SM-LIIGA".to_string(),
                                disable_video_links,
                                true,
                                false,
                            );
                            page.add_error_message("Ei otteluita tälle päivälle");
                            page.set_fetched_date(fetched_date);
                            pages = vec![page];
                        } else {
                            let mut page = TeletextPage::new(
                                221,
                                "JÄÄKIEKKO".to_string(),
                                "SM-LIIGA".to_string(),
                                disable_video_links,
                                true,
                                false,
                            );
                            page.set_fetched_date(fetched_date);

                            for game in &games {
                                page.add_game_result(GameResultData::new(game));
                            }

                            pages = vec![page];
                        }

                        needs_render = true;
                    }
                    last_refresh = Instant::now();
                }
                Err(e) => {
                    warn!("Auto-refresh failed: {}", e);
                    last_refresh = Instant::now(); // Still update to avoid spam
                }
            }
        }

        // Render UI only when needed
        if needs_render {
            if !debug_mode {
                execute!(stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;
            }

            if !pages.is_empty() && current_page < pages.len() {
                pages[current_page].render(&mut stdout())?;
            }

            needs_render = false;
        }
    }

    if !debug_mode {
        disable_raw_mode()?;
        execute!(stdout(), LeaveAlternateScreen)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing_utils::TestDataBuilder;

    #[test]
    fn test_calculate_games_hash() {
        let games1 = vec![
            TestDataBuilder::create_basic_game("Team A", "Team B"),
            TestDataBuilder::create_basic_game("Team C", "Team D"),
        ];

        let games2 = vec![
            TestDataBuilder::create_basic_game("Team A", "Team B"),
            TestDataBuilder::create_basic_game("Team C", "Team D"),
        ];

        let games3 = vec![
            TestDataBuilder::create_basic_game("Team A", "Team B"),
            TestDataBuilder::create_basic_game("Team E", "Team F"), // Different game
        ];

        let hash1 = calculate_games_hash(&games1);
        let hash2 = calculate_games_hash(&games2);
        let hash3 = calculate_games_hash(&games3);

        // Same games should have same hash
        assert_eq!(hash1, hash2);

        // Different games should have different hash
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_calculate_games_hash_empty() {
        let empty_games: Vec<GameData> = vec![];
        let _hash = calculate_games_hash(&empty_games);

        // Should not panic - any hash value is valid for empty games
    }

    #[test]
    fn test_calculate_games_hash_single_game() {
        let single_game = vec![TestDataBuilder::create_basic_game("Team A", "Team B")];
        let _hash = calculate_games_hash(&single_game);

        // Should not panic - any hash value is valid
    }
}
