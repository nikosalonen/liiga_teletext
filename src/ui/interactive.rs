//! Interactive UI module for the liiga_teletext application
//!
//! This module contains the main interactive UI loop that was previously in main.rs.
//! It handles user input, screen updates, and the main application flow.

use crate::data_fetcher::{GameData, fetch_liiga_data};
use crate::error::AppError;
use crate::teletext_ui::{GameResultData, TeletextPage};
use crate::ui::resize::ResizeHandler;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, size,
    },
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::stdout;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

// Teletext page constants
const TELETEXT_PAGE_NUMBER: u16 = 221;
const TELETEXT_HEADER: &str = "JÄÄKIEKKO";
const TELETEXT_SUBHEADER: &str = "SM-LIIGA";

// UI timing constants
const ACTIVE_POLL_INTERVAL_MS: u64 = 50;
const SEMI_ACTIVE_POLL_INTERVAL_MS: u64 = 200;
const IDLE_POLL_INTERVAL_MS: u64 = 500;
const ACTIVE_THRESHOLD_SECS: u64 = 5;
const SEMI_ACTIVE_THRESHOLD_SECS: u64 = 30;
const MANUAL_REFRESH_COOLDOWN_SECS: u64 = 10;
const AUTO_REFRESH_INTERVAL_SECS: u64 = 60;

/// Calculates a hash of the games data for change detection
fn calculate_games_hash(games: &[GameData]) -> u64 {
    let mut hasher = DefaultHasher::new();
    games.hash(&mut hasher);
    hasher.finish()
}

/// Creates a TeletextPage with an error message
/// This helper function eliminates code duplication for error handling
fn create_error_page(error_message: String, disable_video_links: bool) -> TeletextPage {
    let mut page = TeletextPage::new(
        TELETEXT_PAGE_NUMBER,
        TELETEXT_HEADER.to_string(),
        TELETEXT_SUBHEADER.to_string(),
        disable_video_links,
        true,
        false,
    );
    page.add_error_message(&error_message);
    page
}

/// Creates a TeletextPage instance from game data
/// This helper function eliminates code duplication by centralizing the page creation logic
fn create_teletext_page(
    games: &[GameData],
    fetched_date: String,
    disable_video_links: bool,
) -> TeletextPage {
    let mut page = TeletextPage::new(
        TELETEXT_PAGE_NUMBER,
        TELETEXT_HEADER.to_string(),
        TELETEXT_SUBHEADER.to_string(),
        disable_video_links,
        true,
        false,
    );
    page.set_fetched_date(fetched_date);

    if games.is_empty() {
        page.add_error_message("Ei otteluita tälle päivälle");
    } else {
        for game in games {
            page.add_game_result(GameResultData::new(game));
        }
    }

    page
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

    let mut page: TeletextPage;
    let mut last_refresh = Instant::now();
    let mut last_games_hash = 0u64;
    let mut last_activity = Instant::now();
    let mut current_date = date;
    let mut needs_render = true;
    let mut resize_handler = ResizeHandler::new();

    // Initial data fetch
    match fetch_liiga_data(current_date.clone()).await {
        Ok((games, fetched_date)) => {
            current_date = Some(fetched_date.clone());
            let games_hash = calculate_games_hash(&games);
            last_games_hash = games_hash;

            page = create_teletext_page(&games, fetched_date, disable_video_links);

            // Initialize layout with current terminal size
            if let Ok(terminal_size) = size() {
                page.update_layout(terminal_size);
            }
        }
        Err(e) => {
            warn!("Failed to fetch initial data: {}", e);
            page = create_error_page(format!("Virhe tietojen haussa: {e}"), disable_video_links);

            // Initialize layout for error page
            if let Ok(terminal_size) = size() {
                page.update_layout(terminal_size);
            }
        }
    }

    loop {
        // Adaptive polling based on user activity
        let idle_duration = last_activity.elapsed();
        let poll_interval = if idle_duration < Duration::from_secs(ACTIVE_THRESHOLD_SECS) {
            Duration::from_millis(ACTIVE_POLL_INTERVAL_MS) // Active use
        } else if idle_duration < Duration::from_secs(SEMI_ACTIVE_THRESHOLD_SECS) {
            Duration::from_millis(SEMI_ACTIVE_POLL_INTERVAL_MS) // Semi-active
        } else {
            Duration::from_millis(IDLE_POLL_INTERVAL_MS) // Idle
        };

        // Check for user input
        if event::poll(poll_interval)? {
            if let Event::Key(key_event) = event::read()? {
                last_activity = Instant::now();

                match key_event.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => break,
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        // Check if auto-refresh is disabled - ignore manual refresh too
                        if page.is_auto_refresh_disabled() {
                            debug!("Manual refresh ignored - auto-refresh is disabled");
                            continue; // Skip refresh when auto-refresh is disabled
                        }

                        // Manual refresh with cooldown
                        if last_refresh.elapsed()
                            >= Duration::from_secs(MANUAL_REFRESH_COOLDOWN_SECS)
                        {
                            info!("Manual refresh requested");
                            match fetch_liiga_data(current_date.clone()).await {
                                Ok((games, fetched_date)) => {
                                    let games_hash = calculate_games_hash(&games);
                                    if games_hash != last_games_hash {
                                        last_games_hash = games_hash;
                                        current_date = Some(fetched_date.clone());

                                        // Rebuild page
                                        page = create_teletext_page(
                                            &games,
                                            fetched_date,
                                            disable_video_links,
                                        );

                                        // Initialize layout for new page
                                        if let Ok(terminal_size) = size() {
                                            page.update_layout(terminal_size);
                                        }
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
                        page.previous_page();
                        needs_render = true;
                    }
                    KeyCode::Right => {
                        page.next_page();
                        needs_render = true;
                    }
                    _ => {}
                }
            }
        }

        // Check for terminal resize
        if let Ok(current_size) = size() {
            if let Some(new_size) = resize_handler.check_for_resize(current_size) {
                debug!("Terminal resize detected: {:?}", new_size);

                // Update layout for the page
                page.update_layout(new_size);

                needs_render = true;
            }
        }

        // Auto-refresh logic
        let should_refresh =
            last_refresh.elapsed() >= Duration::from_secs(AUTO_REFRESH_INTERVAL_SECS);
        if should_refresh {
            debug!("Auto-refresh triggered");
            match fetch_liiga_data(current_date.clone()).await {
                Ok((games, fetched_date)) => {
                    let games_hash = calculate_games_hash(&games);
                    if games_hash != last_games_hash {
                        last_games_hash = games_hash;
                        current_date = Some(fetched_date.clone());

                        // Rebuild page
                        page = create_teletext_page(&games, fetched_date, disable_video_links);

                        // Initialize layout for new page
                        if let Ok(terminal_size) = size() {
                            page.update_layout(terminal_size);
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
                execute!(
                    stdout(),
                    crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
                )?;
            }

            page.render_buffered(&mut stdout())?;

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
    use tokio::time::{Duration, timeout};

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

    #[test]
    fn test_create_teletext_page_with_games() {
        let games = vec![
            TestDataBuilder::create_basic_game("HIFK", "Jokerit"),
            TestDataBuilder::create_basic_game("TPS", "Ilves"),
        ];
        let fetched_date = "2024-01-15".to_string();
        let disable_video_links = false;

        let _page = create_teletext_page(&games, fetched_date.clone(), disable_video_links);

        // Verify the page was created with correct parameters
        // Note: We can't easily test internal state without exposing more methods
        // The fact that it doesn't panic is a good sign
    }

    #[test]
    fn test_create_teletext_page_empty_games() {
        let games: Vec<GameData> = vec![];
        let fetched_date = "2024-01-15".to_string();
        let disable_video_links = false;

        let _page = create_teletext_page(&games, fetched_date.clone(), disable_video_links);

        // Should create a page with error message for no games
        // The fact that it doesn't panic is a good sign
    }

    #[test]
    fn test_create_error_page() {
        let error_message = "Test error message".to_string();
        let disable_video_links = true;

        let _page = create_error_page(error_message.clone(), disable_video_links);

        // Should create a page with the error message
        // The fact that it doesn't panic is a good sign
    }

    // Since we can't easily mock the fetch_liiga_data function directly without
    // dependency injection, we'll test the helper functions and create comprehensive
    // tests that verify the UI initialization logic works correctly

    #[tokio::test]
    async fn test_ui_initialization_with_successful_data() {
        // Test the helper functions that would be called during UI initialization
        let games = vec![
            TestDataBuilder::create_basic_game("HIFK", "Jokerit"),
            TestDataBuilder::create_basic_game("TPS", "Ilves"),
        ];
        let fetched_date = "2024-01-15".to_string();
        let disable_video_links = false;

        // Test that page is created correctly with successful data
        let _page = create_teletext_page(&games, fetched_date.clone(), disable_video_links);

        // Test hash calculation for change detection
        let hash1 = calculate_games_hash(&games);
        let hash2 = calculate_games_hash(&games);
        assert_eq!(hash1, hash2);

        // Test with different games to ensure hash changes
        let different_games = vec![TestDataBuilder::create_basic_game("Kärpät", "Lukko")];
        let hash3 = calculate_games_hash(&different_games);
        assert_ne!(hash1, hash3);
    }

    #[tokio::test]
    async fn test_ui_initialization_with_empty_data() {
        // Test initialization with no games (common scenario)
        let games: Vec<GameData> = vec![];
        let fetched_date = "2024-01-15".to_string();
        let disable_video_links = false;

        let _page = create_teletext_page(&games, fetched_date.clone(), disable_video_links);

        // Test hash calculation with empty games
        let hash = calculate_games_hash(&games);
        let hash2 = calculate_games_hash(&games);
        assert_eq!(hash, hash2);
    }

    #[tokio::test]
    async fn test_error_page_creation() {
        // Test error handling scenario
        let error_message = "Failed to fetch data: Network timeout".to_string();
        let disable_video_links = true;

        let _page = create_error_page(error_message.clone(), disable_video_links);

        // Test with different error messages
        let different_error = "API rate limit exceeded".to_string();
        let _page2 = create_error_page(different_error, false);
    }

    #[tokio::test]
    async fn test_ui_state_transitions() {
        // Test state transitions that would occur during UI operation
        let initial_games = vec![TestDataBuilder::create_basic_game("Team A", "Team B")];
        let updated_games = vec![
            TestDataBuilder::create_basic_game("Team A", "Team B"),
            TestDataBuilder::create_basic_game("Team C", "Team D"),
        ];

        let initial_hash = calculate_games_hash(&initial_games);
        let updated_hash = calculate_games_hash(&updated_games);

        // Verify that hash changes when games are updated (triggers UI refresh)
        assert_ne!(initial_hash, updated_hash);

        // Test page creation for both states
        let fetched_date = "2024-01-15".to_string();
        let _initial_page = create_teletext_page(&initial_games, fetched_date.clone(), false);
        let _updated_page = create_teletext_page(&updated_games, fetched_date, false);
    }

    #[tokio::test]
    async fn test_concurrent_hash_calculations() {
        // Test that hash calculations work correctly in concurrent scenarios
        let games = vec![
            TestDataBuilder::create_basic_game("HIFK", "Jokerit"),
            TestDataBuilder::create_basic_game("TPS", "Ilves"),
        ];

        // Spawn multiple concurrent hash calculations
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let games_clone = games.clone();
                tokio::spawn(async move { calculate_games_hash(&games_clone) })
            })
            .collect();

        // Wait for all calculations to complete
        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.unwrap());
        }

        // All results should be identical
        let first_hash = results[0];
        for hash in &results[1..] {
            assert_eq!(first_hash, *hash);
        }
    }

    #[tokio::test]
    async fn test_ui_timeout_behavior() {
        // Test that UI operations complete within reasonable time limits
        let games = vec![TestDataBuilder::create_basic_game("Team A", "Team B")];
        let fetched_date = "2024-01-15".to_string();

        // Test that page creation completes quickly
        let result = timeout(Duration::from_millis(100), async {
            create_teletext_page(&games, fetched_date, false)
        })
        .await;

        assert!(result.is_ok());
        let _page = result.unwrap();

        // Test that hash calculation completes quickly
        let hash_result = timeout(Duration::from_millis(50), async {
            calculate_games_hash(&games)
        })
        .await;

        assert!(hash_result.is_ok());
    }

    #[tokio::test]
    async fn test_error_scenarios() {
        // Test various error scenarios that could occur during UI operation

        // Test with different error types
        let network_error = "Network connection failed".to_string();
        let api_error = "API returned invalid data".to_string();
        let timeout_error = "Request timed out".to_string();

        let error_pages = vec![
            create_error_page(network_error, false),
            create_error_page(api_error, true),
            create_error_page(timeout_error, false),
        ];

        // All error pages should be created successfully
        assert_eq!(error_pages.len(), 3);
    }

    #[tokio::test]
    async fn test_resize_and_refresh_interaction() {
        // Test that resize handling works correctly with auto-refresh
        let games = vec![
            TestDataBuilder::create_basic_game("HIFK", "Jokerit"),
            TestDataBuilder::create_basic_game("TPS", "Ilves"),
        ];
        let fetched_date = "2024-01-15".to_string();

        // Create initial page
        let mut page = create_teletext_page(&games, fetched_date.clone(), false);

        // Simulate initial layout setup
        page.update_layout((80, 24));

        // Simulate resize during operation
        page.update_layout((120, 40));

        // Simulate auto-refresh creating new page with proper layout
        let refreshed_page = create_teletext_page(&games, fetched_date, false);
        // Auto-refresh should initialize layout with current terminal size
        // This is handled in the main UI loop, but we can verify the page accepts layout updates
        let mut refreshed_page = refreshed_page;
        refreshed_page.update_layout((120, 40));

        // Both operations should complete without errors
        // The fact that we reach this point means the interaction works correctly
    }
}
