use crate::error::AppError;
use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use semver::Version;
use std::io::stdout;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Base URL of crates.io. Tests pass a wiremock URL instead.
pub const CRATES_IO_BASE: &str = "https://crates.io";

/// Version of the running binary.
pub fn current_version() -> Version {
    Version::parse(CURRENT_VERSION).expect("CARGO_PKG_VERSION is valid semver")
}

/// Fetches the newest stable version of this crate from crates.io.
pub async fn fetch_latest_version(crates_io_base: &str) -> Result<Version, AppError> {
    let url = format!("{crates_io_base}/api/v1/crates/{CRATE_NAME}");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(format!("{CRATE_NAME}/{CURRENT_VERSION}"))
        .build()?;

    let json: serde_json::Value = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let latest = json
        .get("crate")
        .and_then(|c| c.get("max_stable_version"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            AppError::api_no_data("crates.io response has no max_stable_version", &url)
        })?;

    Ok(Version::parse(latest)?)
}

/// Handle to the background crates.io version check.
pub type VersionCheck = tokio::task::JoinHandle<Result<Version, AppError>>;

/// Starts the crates.io version check in the background, so it runs while the
/// app shows games. Pass the handle to `report_version_check` afterwards.
pub fn spawn_version_check() -> VersionCheck {
    tokio::spawn(fetch_latest_version(CRATES_IO_BASE))
}

/// Prints the update notice, or why the check failed. Call this only once the
/// terminal is back to normal: anything printed while the interactive UI owns
/// the screen is lost.
pub async fn report_version_check(version_check: VersionCheck) {
    match version_check.await {
        Ok(Ok(latest)) => print_version_info(&latest),
        Ok(Err(e)) => {
            tracing::warn!("Failed to check for updates: {e}");
            eprintln!("Failed to check for updates: {e}");
        }
        Err(e) => tracing::warn!("Version check task failed: {e}"),
    }
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

/// Prints a box with the update command when `latest` is newer than this
/// binary. Prints nothing otherwise.
pub fn print_version_info(latest: &Version) {
    if *latest > current_version() {
        println!();
        print_version_status_box(vec![
            ("Liiga Teletext Status".to_string(), None),
            ("".to_string(), None),
            (
                format!("Current Version: {CURRENT_VERSION}"),
                Some(Color::AnsiValue(231)), // Authentic teletext white
            ),
            (
                format!("Latest Version:  {latest}"),
                Some(Color::AnsiValue(51)), // Authentic teletext cyan
            ),
            ("".to_string(), None),
            ("Update available! Run:".to_string(), None),
            (
                "liiga_teletext --update".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const CRATE_PATH: &str = "/api/v1/crates/liiga_teletext";

    #[tokio::test]
    async fn reads_max_stable_version() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(CRATE_PATH))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": { "max_stable_version": "9.8.7", "newest_version": "10.0.0-rc.1" }
            })))
            .mount(&server)
            .await;

        let latest = fetch_latest_version(&server.uri()).await.unwrap();
        assert_eq!(latest, Version::new(9, 8, 7));
    }

    #[tokio::test]
    async fn server_error_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(CRATE_PATH))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        assert!(fetch_latest_version(&server.uri()).await.is_err());
    }

    #[tokio::test]
    async fn missing_field_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(CRATE_PATH))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "crate": {} })),
            )
            .mount(&server)
            .await;

        assert!(fetch_latest_version(&server.uri()).await.is_err());
    }
}
