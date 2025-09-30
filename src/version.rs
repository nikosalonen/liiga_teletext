use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use semver::Version;
use std::io::stdout;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Checks for the latest version of this crate on crates.io.
///
/// Returns `Some(version_string)` if a newer version is available,
/// or `None` if there was an error checking or if the current version is up to date.
pub async fn check_latest_version() -> Option<String> {
    let crates_io_url = format!("https://crates.io/api/v1/crates/{CRATE_NAME}");

    // Create a properly configured HTTP client with timeout handling
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10)) // Shorter timeout for update checks
        .build()
        .unwrap_or_else(|_| reqwest::Client::new()); // Fallback to default client if builder fails

    let user_agent = format!("{CRATE_NAME}/{CURRENT_VERSION}");
    let response = match client
        .get(&crates_io_url)
        .header("User-Agent", user_agent)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Failed to check for updates: {e}");
            return None;
        }
    };

    let json: serde_json::Value = match response.json::<serde_json::Value>().await {
        Ok(json) => json,
        Err(e) => {
            eprintln!("Failed to parse update response: {e}");
            return None;
        }
    };

    // Try max_stable_version instead of newest_version
    json.get("crate")
        .and_then(|c| c.get("max_stable_version"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Helper to print a dynamic-width version status box with optional color highlights
pub fn print_version_status_box(lines: Vec<(String, Option<Color>)>) {
    // Compute max content width
    let max_content_width = lines
        .iter()
        .map(|(l, _)| l.chars().count())
        .max()
        .unwrap_or(0);
    let box_width = max_content_width + 4; // 2 for borders, 2 for padding
    let border = format!("╔{:═<width$}╗", "", width = box_width - 2);
    let sep = format!("╠{:═<width$}╣", "", width = box_width - 2);
    let bottom = format!("╚{:═<width$}╝", "", width = box_width - 2);
    // Print top border
    execute!(
        stdout(),
        SetForegroundColor(Color::AnsiValue(231)), // Authentic teletext white
        Print(format!("{border}\n"))
    )
    .ok();
    // Print lines
    for (i, (line, color)) in lines.iter().enumerate() {
        let padded = format!("║ {line:<max_content_width$} ║");
        match color {
            Some(c) => {
                // Print up to the colored part, then color, then reset
                if let Some((pre, col)) = line.split_once(':') {
                    let pre = format!("║ {pre}:");
                    let col = col.trim_start();
                    let pad = max_content_width - (pre.chars().count() - 2 + col.chars().count());
                    execute!(
                        stdout(),
                        SetForegroundColor(Color::AnsiValue(231)), // Authentic teletext white
                        Print(pre),
                        SetForegroundColor(*c),
                        Print(col),
                        SetForegroundColor(Color::AnsiValue(231)), // Authentic teletext white
                        Print(format!("{:pad$} ║\n", "", pad = pad)),
                    )
                    .ok();
                } else {
                    execute!(
                        stdout(),
                        SetForegroundColor(*c),
                        Print(padded),
                        SetForegroundColor(Color::AnsiValue(231)), // Authentic teletext white
                        Print("\n")
                    )
                    .ok();
                }
            }
            None => {
                execute!(
                    stdout(),
                    SetForegroundColor(Color::AnsiValue(231)), // Authentic teletext white
                    Print(padded),
                    Print("\n")
                )
                .ok();
            }
        }
        // Separator after first or second line if needed
        if i == 0 && lines.len() > 2 {
            execute!(stdout(), Print(format!("{sep}\n"))).ok();
        }
    }
    // Print bottom border
    execute!(stdout(), Print(format!("{bottom}\n")), ResetColor).ok();
}

pub fn print_version_info(latest_version: &str) {
    let current = match Version::parse(CURRENT_VERSION) {
        Ok(v) => v,
        Err(_) => {
            // If we can't parse the current version, just show a generic message
            println!("Update available! Latest version: {latest_version}");
            return;
        }
    };
    let latest = match Version::parse(latest_version) {
        Ok(v) => v,
        Err(_) => {
            // If we can't parse the latest version, don't show anything
            return;
        }
    };

    if latest > current {
        println!();
        print_version_status_box(vec![
            ("Liiga Teletext Status".to_string(), None),
            ("".to_string(), None),
            (
                format!("Current Version: {CURRENT_VERSION}"),
                Some(Color::AnsiValue(231)), // Authentic teletext white
            ),
            (
                format!("Latest Version:  {latest_version}"),
                Some(Color::AnsiValue(51)), // Authentic teletext cyan
            ),
            ("".to_string(), None),
            ("Update available! Run:".to_string(), None),
            (
                "cargo install liiga_teletext".to_string(),
                Some(Color::AnsiValue(51)), // Authentic teletext cyan
            ),
        ]);
    }
}

pub fn print_logo() {
    execute!(
        stdout(),
        SetForegroundColor(Color::AnsiValue(51)), // Authentic teletext cyan
        Print(format!(
            "\n{}",
            r#"

██╗░░░░░██╗██╗░██████╗░░█████╗░  ██████╗░██████╗░░░███╗░░
██║░░░░░██║██║██╔════╝░██╔══██╗  ╚════██╗╚════██╗░████║░░
██║░░░░░██║██║██║░░██╗░███████║  ░░███╔═╝░░███╔═╝██╔██║░░
██║░░░░░██║██║██║░░╚██╗██╔══██║  ██╔══╝░░██╔══╝░░╚═╝██║░░
███████╗██║██║╚██████╔╝██║░░██║  ███████╗███████╗███████╗
╚══════╝╚═╝╚═╝░╚═════╝░╚═╝░░╚═╝  ╚══════╝╚══════╝╚══════╝
"#
        )),
        ResetColor
    )
    .ok();
}
