// src/main.rs
mod config;
mod data_fetcher;
mod teletext_ui;

use crossterm::{
    cursor::{Hide, Show},
    event::{Event, KeyCode, read},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use data_fetcher::{GameData, fetch_liiga_data};
use std::io::stdout;
use std::thread;
use std::time::Duration;
use teletext_ui::TeletextPage;

fn get_subheader(games: &[GameData]) -> String {
    if games.is_empty() {
        return "SM-LIIGA".to_string();
    }

    // Use the tournament type from the first game as they should all be from same tournament
    match games[0].tournament.as_str() {
        "runkosarja" => "RUNKOSARJA".to_string(),
        "playoffs" => "PLAYOFFS".to_string(),
        "playout" => "PLAYOUT-OTTELUT".to_string(),
        "qualifications" => "LIIGAKARSINTA".to_string(),
        _ => "SM-LIIGA".to_string(),
    }
}

// Function to create multiple pages if there are many games
fn create_pages(games: &[GameData], page_size: usize) -> Vec<TeletextPage> {
    let mut pages = Vec::new();
    let chunks = games.chunks(page_size);
    let total_pages = (games.len() as f32 / page_size as f32).ceil() as u16;
    let subheader = get_subheader(games);

    for (i, chunk) in chunks.enumerate() {
        let mut page = TeletextPage::new(221, "JÄÄKIEKKO".to_string(), subheader.clone());

        for game in chunk {
            page.add_game_result(
                game.home_team.clone(),
                game.away_team.clone(),
                game.time.clone(),
                game.result.clone(),
                game.score_type.clone(),
                game.is_overtime,
                game.is_shootout,
            );
            page.add_spacer();
        }

        page.set_pagination((i + 1) as u16, total_pages);
        pages.push(page);
    }

    pages
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up terminal
    let mut stdout = stdout();
    enable_raw_mode()?;
    execute!(stdout, Hide)?;

    // Get game data, falling back to mock data if fetch fails
    let games = fetch_liiga_data().unwrap_or_else(|e| {
        // In a real app, you might want to show an error message
        eprintln!("Error fetching data: {}", e);
        vec![] // We'll handle empty data below
    });

    // Create pages (4 games per page)
    let mut pages = if games.is_empty() {
        // Create a single error page if no data
        let mut error_page =
            TeletextPage::new(221, "JÄÄKIEKKO".to_string(), "SM-LIIGA".to_string());
        error_page.add_error_message("Ei tuloksia saatavilla.");
        error_page.add_spacer();
        error_page.add_error_message("Yritä myöhemmin uudelleen.");
        error_page.set_pagination(1, 1);
        vec![error_page]
    } else {
        create_pages(&games, 4)
    };

    let mut current_page_idx = 0;

    // Render the initial page
    if !pages.is_empty() {
        pages[current_page_idx].render(&mut stdout)?;
    }

    // Page switching timer (optional, for auto-cycling pages like real teletext)
    let auto_switch = false; // Set to true to enable auto page switching
    let mut last_switch = std::time::Instant::now();
    let switch_interval = Duration::from_secs(10); // Switch every 10 seconds

    // Handle user input
    loop {
        // Auto page switching logic
        if auto_switch && last_switch.elapsed() >= switch_interval && pages.len() > 1 {
            current_page_idx = (current_page_idx + 1) % pages.len();
            pages[current_page_idx].render(&mut stdout)?;
            last_switch = std::time::Instant::now();
        }

        // Check for user input with a small timeout to allow auto switching
        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key_event) = read()? {
                match key_event.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Left => {
                        if !pages.is_empty() && pages.len() > 1 {
                            current_page_idx = if current_page_idx == 0 {
                                pages.len() - 1
                            } else {
                                current_page_idx - 1
                            };
                            pages[current_page_idx].render(&mut stdout)?;
                            last_switch = std::time::Instant::now(); // Reset timer after manual switch
                        }
                    }
                    KeyCode::Right => {
                        if !pages.is_empty() && pages.len() > 1 {
                            current_page_idx = (current_page_idx + 1) % pages.len();
                            pages[current_page_idx].render(&mut stdout)?;
                            last_switch = std::time::Instant::now(); // Reset timer after manual switch
                        }
                    }
                    KeyCode::Char('r') => {
                        // Refresh data
                        let fresh_games = fetch_liiga_data().unwrap_or_else(|_| vec![]);

                        // Update the pages with fresh data
                        pages = if fresh_games.is_empty() {
                            let mut error_page = TeletextPage::new(
                                221,
                                "JÄÄKIEKKO".to_string(),
                                "SM-LIIGA".to_string(),
                            );
                            error_page.add_error_message("Ei tuloksia saatavilla.");
                            error_page.add_spacer();
                            error_page.add_error_message("Yritä myöhemmin uudelleen.");
                            error_page.set_pagination(1, 1);
                            vec![error_page]
                        } else {
                            create_pages(&fresh_games, 4)
                        };

                        // Reset to first page and render
                        current_page_idx = 0;
                        if !pages.is_empty() {
                            pages[current_page_idx].render(&mut stdout)?;
                        }
                    }
                    _ => {}
                }
            }
        } else {
            // Small sleep to prevent CPU hogging in the loop
            thread::sleep(Duration::from_millis(10));
        }
    }

    // Clean up terminal
    disable_raw_mode()?;
    execute!(stdout, Show)?;

    Ok(())
}
