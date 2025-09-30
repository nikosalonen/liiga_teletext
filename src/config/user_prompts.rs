//! User interaction and prompts for configuration setup
//!
//! This module handles user prompts and input collection for configuration
//! initialization when config files don't exist or need user input.

use crate::error::AppError;
use tokio::io::{self, AsyncBufReadExt};

/// Prompts the user for API domain input and returns the trimmed input.
///
/// This function displays a prompt asking for the API domain and waits for
/// user input from stdin. It handles the asynchronous input reading and
/// returns the trimmed input string.
///
/// # Returns
/// * `Ok(String)` - The trimmed user input
/// * `Err(AppError)` - Error reading from stdin
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
    println!("Please enter your API domain: ");
    let mut input = String::new();
    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin);
    reader.read_line(&mut input).await?;
    Ok(input.trim().to_string())
}
