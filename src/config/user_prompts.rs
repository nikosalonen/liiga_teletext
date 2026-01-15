//! User interaction and prompts for configuration setup
//!
//! This module handles user prompts and input collection for configuration
//! initialization when config files don't exist or need user input.

use crate::error::AppError;
use crossterm::{
    cursor,
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use std::io::{stdout, Write};
use std::time::Duration;
use tokio::io::{self, AsyncBufReadExt};

// Teletext colors
const TELETEXT_WHITE: Color = Color::AnsiValue(231);
const TELETEXT_CYAN: Color = Color::AnsiValue(51);
const TELETEXT_GREEN: Color = Color::AnsiValue(46);
const TELETEXT_YELLOW: Color = Color::AnsiValue(226);
const TELETEXT_RED: Color = Color::AnsiValue(196);

/// Prints a teletext-style header box
fn print_header_box(title: &str) {
    let width = 50;
    let border_top = format!("╔{:═<width$}╗", "", width = width - 2);
    let border_bottom = format!("╚{:═<width$}╝", "", width = width - 2);

    let title_padding = (width - 2 - title.len()) / 2;
    let title_line = format!(
        "║{:>pad$}{}{:<pad2$}║",
        "",
        title,
        "",
        pad = title_padding,
        pad2 = width - 2 - title_padding - title.len()
    );

    let _ = execute!(
        stdout(),
        Print("\n"),
        SetForegroundColor(TELETEXT_CYAN),
        Print(&border_top),
        Print("\n"),
        SetForegroundColor(TELETEXT_WHITE),
        Print(&title_line),
        Print("\n"),
        SetForegroundColor(TELETEXT_CYAN),
        Print(&border_bottom),
        Print("\n"),
        ResetColor
    );
}

/// Prints colored text
fn print_colored(text: &str, color: Color) {
    let _ = execute!(
        stdout(),
        SetForegroundColor(color),
        Print(text),
        ResetColor
    );
}

/// Prints colored text with newline
fn println_colored(text: &str, color: Color) {
    let _ = execute!(
        stdout(),
        SetForegroundColor(color),
        Print(text),
        Print("\n"),
        ResetColor
    );
}

/// Animated spinner during API test
async fn test_api_with_animation(api_url: &str) -> Result<(), String> {
    let url = if api_url.starts_with("http://") || api_url.starts_with("https://") {
        api_url.to_string()
    } else {
        format!("https://{api_url}")
    };

    let test_url = format!("{}/tournament", url.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    // Start spinner animation in background
    let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let (tx, mut rx) = tokio::sync::oneshot::channel::<Result<(), String>>();

    // Spawn the actual API test
    let test_future = async move {
        let result = client.get(&test_url).send().await;
        match result {
            Ok(response) if response.status().is_success() => Ok(()),
            Ok(response) => Err(format!("API returned error status: {}", response.status())),
            Err(e) => Err(format!("Connection failed: {e}")),
        }
    };

    // Run spinner while waiting for API response
    tokio::spawn(async move {
        let result = test_future.await;
        let _ = tx.send(result);
    });

    let mut frame = 0;
    loop {
        // Check if API test completed
        match rx.try_recv() {
            Ok(result) => {
                // Clear the spinner line
                let _ = execute!(
                    stdout(),
                    cursor::MoveToColumn(0),
                    Clear(ClearType::CurrentLine)
                );
                return result;
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                // Still waiting, update spinner
                let _ = execute!(
                    stdout(),
                    cursor::MoveToColumn(0),
                    Clear(ClearType::CurrentLine),
                    SetForegroundColor(TELETEXT_YELLOW),
                    Print(spinner_frames[frame]),
                    Print(" "),
                    SetForegroundColor(TELETEXT_WHITE),
                    Print("Testing API connection..."),
                    ResetColor
                );
                let _ = stdout().flush();
                frame = (frame + 1) % spinner_frames.len();
                tokio::time::sleep(Duration::from_millis(80)).await;
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                // Channel closed unexpectedly
                let _ = execute!(
                    stdout(),
                    cursor::MoveToColumn(0),
                    Clear(ClearType::CurrentLine)
                );
                return Err("Connection test interrupted".to_string());
            }
        }
    }
}

/// Tests if the API URL is reachable by calling the /tournament endpoint.
///
/// # Arguments
/// * `api_url` - The API URL to test (with or without https://)
///
/// # Returns
/// * `Ok(())` - API is reachable
/// * `Err(String)` - Error message describing the failure
pub async fn test_api_url(api_url: &str) -> Result<(), String> {
    let url = if api_url.starts_with("http://") || api_url.starts_with("https://") {
        api_url.to_string()
    } else {
        format!("https://{api_url}")
    };

    let test_url = format!("{}/tournament", url.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(&test_url)
        .send()
        .await
        .map_err(|e| format!("Connection failed: {e}"))?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!(
            "API returned error status: {}",
            response.status()
        ))
    }
}

/// Reads a single line from stdin.
async fn read_line() -> Result<String, AppError> {
    let mut input = String::new();
    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin);
    reader.read_line(&mut input).await?;
    Ok(input.trim().to_string())
}

/// Prompts the user for API domain input with validation and testing.
///
/// This function displays instructions about the URL format, prompts for input,
/// and tests the API URL before accepting it. If the test fails, the user can
/// try again or cancel by pressing Enter without input.
///
/// # Returns
/// * `Ok(String)` - The validated API domain
/// * `Err(AppError)` - Error reading from stdin or user cancelled
///
/// # Example
/// ```no_run
/// use liiga_teletext::config::user_prompts::prompt_for_api_domain;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let api_domain = prompt_for_api_domain().await?;
/// println!("Got API domain: {}", api_domain);
/// # Ok(())
/// # }
/// ```
pub async fn prompt_for_api_domain() -> Result<String, AppError> {
    print_header_box("API DOMAIN CONFIGURATION");

    println!();
    println_colored("  Enter the API domain URL for fetching game data.", TELETEXT_WHITE);
    println!();

    print_colored("  Format: ", TELETEXT_WHITE);
    println_colored("https://example.com/api/v2", TELETEXT_GREEN);

    println!();
    print_colored("  • ", TELETEXT_CYAN);
    println_colored("Include https:// prefix (or it will be added)", TELETEXT_WHITE);

    print_colored("  • ", TELETEXT_CYAN);
    println_colored("Do NOT include trailing slash", TELETEXT_WHITE);

    println!();
    print_colored("  Press ", TELETEXT_WHITE);
    print_colored("Enter", TELETEXT_YELLOW);
    println_colored(" without input to cancel.", TELETEXT_WHITE);

    println!();
    let _ = execute!(
        stdout(),
        SetForegroundColor(TELETEXT_CYAN),
        Print("  ────────────────────────────────────────────────\n"),
        ResetColor
    );

    loop {
        println!();
        print_colored("  API domain: ", TELETEXT_CYAN);
        let _ = stdout().flush();

        let input = read_line().await?;

        if input.is_empty() {
            println!();
            println_colored("  Configuration cancelled.", TELETEXT_YELLOW);
            return Err(AppError::config_error("Configuration cancelled by user"));
        }

        println!();

        match test_api_with_animation(&input).await {
            Ok(()) => {
                print_colored("  ✓ ", TELETEXT_GREEN);
                println_colored("API connection successful!", TELETEXT_GREEN);
                println!();
                return Ok(input);
            }
            Err(e) => {
                print_colored("  ✗ ", TELETEXT_RED);
                print_colored("API test failed: ", TELETEXT_RED);
                println_colored(&e, TELETEXT_WHITE);
                println!();
                print_colored("  Please check the URL and try again, or press ", TELETEXT_WHITE);
                print_colored("Enter", TELETEXT_YELLOW);
                println_colored(" to cancel.", TELETEXT_WHITE);
            }
        }
    }
}
