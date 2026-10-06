use crate::constants::colors::{TELETEXT_CYAN, TELETEXT_WHITE, TELETEXT_YELLOW};
use crate::error::AppError;
use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor, Stylize},
};
use semver::Version;
use std::io::{Write, stdout};

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

/// One line of a version box.
#[derive(Debug, Clone, PartialEq)]
pub enum BoxLine {
    /// Text in teletext white.
    Text(String),
    /// A whole line in one color.
    Colored(String, Color),
    /// `label:` in white and a value in `color`. Values in one box line up.
    Field {
        label: &'static str,
        value: String,
        color: Color,
    },
}

impl BoxLine {
    pub fn blank() -> Self {
        BoxLine::Text(String::new())
    }
}

/// Draws `lines` in a double-line box sized to the longest line. The first
/// line is a header with a rule under it when more lines follow. Without
/// `use_color` the box has no escape codes, which keeps tests readable.
pub fn render_box(lines: &[BoxLine], use_color: bool) -> String {
    // "Label:" plus one space after the longest label
    let label_width = lines
        .iter()
        .filter_map(|line| match line {
            BoxLine::Field { label, .. } => Some(label.chars().count() + 2),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let label_cell = |label: &str| format!("{:<label_width$}", format!("{label}:"));

    let plain: Vec<String> = lines
        .iter()
        .map(|line| match line {
            BoxLine::Text(text) | BoxLine::Colored(text, _) => text.clone(),
            BoxLine::Field { label, value, .. } => format!("{}{value}", label_cell(label)),
        })
        .collect();
    let width = plain.iter().map(|l| l.chars().count()).max().unwrap_or(0);

    let paint = |text: &str, color: Color| {
        if use_color {
            text.with(color).to_string()
        } else {
            text.to_string()
        }
    };
    let rule = "═".repeat(width + 2);
    let border = |left: char, right: char| paint(&format!("{left}{rule}{right}\n"), TELETEXT_WHITE);

    let mut out = border('╔', '╗');
    for (i, (line, text)) in lines.iter().zip(&plain).enumerate() {
        let content = match line {
            BoxLine::Text(text) => paint(text, TELETEXT_WHITE),
            BoxLine::Colored(text, color) => paint(text, *color),
            BoxLine::Field {
                label,
                value,
                color,
            } => format!(
                "{}{}",
                paint(&label_cell(label), TELETEXT_WHITE),
                paint(value, *color)
            ),
        };
        let padding = " ".repeat(width - text.chars().count());
        out.push_str(&paint("║ ", TELETEXT_WHITE));
        out.push_str(&content);
        out.push_str(&paint(&format!("{padding} ║\n"), TELETEXT_WHITE));
        if i == 0 && lines.len() > 1 {
            out.push_str(&border('╠', '╣'));
        }
    }
    out.push_str(&border('╚', '╝'));
    out
}

/// OS and CPU of the running binary, e.g. `macos aarch64`. This is the real
/// platform, not the release asset name: a cargo build on glibc Linux would
/// otherwise show the musl asset that `--update` downloads.
pub fn platform() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

/// One-line version for when stdout is not a terminal, like `cargo --version`.
pub fn plain_version() -> String {
    format!("{CRATE_NAME} {CURRENT_VERSION}")
}

/// Lines of the version box. `latest` is the crates.io version, or `None`
/// when the check failed.
pub fn version_box(latest: Option<&Version>) -> Vec<BoxLine> {
    let mut lines = vec![
        BoxLine::Text("Liiga Teletext Status".to_string()),
        BoxLine::blank(),
        BoxLine::Field {
            label: "Version",
            value: CURRENT_VERSION.to_string(),
            color: TELETEXT_WHITE,
        },
    ];
    let update_available = latest.filter(|latest| **latest > current_version());
    if let Some(latest) = update_available {
        lines.push(BoxLine::Field {
            label: "Latest",
            value: latest.to_string(),
            color: TELETEXT_CYAN,
        });
    }
    lines.push(BoxLine::Field {
        label: "Platform",
        value: platform(),
        color: TELETEXT_WHITE,
    });
    lines.push(BoxLine::blank());

    match (latest, update_available) {
        (_, Some(_)) => {
            lines.push(BoxLine::Text("Update available! Run:".to_string()));
            lines.push(BoxLine::Colored(
                "liiga_teletext --update".to_string(),
                TELETEXT_CYAN,
            ));
        }
        (Some(_), None) => {
            lines.push(BoxLine::Text(
                "You're running the latest version!".to_string(),
            ));
        }
        (None, _) => {
            lines.push(BoxLine::Colored(
                "Couldn't check for updates.".to_string(),
                TELETEXT_YELLOW,
            ));
        }
    }
    lines
}

/// Prints the version box with a blank line before it.
pub fn print_version_box(latest: Option<&Version>) {
    print!("\n{}", render_box(&version_box(latest), true));
    stdout().flush().ok();
}

/// Prints the update box when `latest` is newer than this binary. Prints
/// nothing otherwise.
pub fn print_version_info(latest: &Version) {
    if *latest > current_version() {
        print_version_box(Some(latest));
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

    fn render_plain(lines: &[BoxLine]) -> String {
        render_box(lines, false)
    }

    fn field(label: &'static str, value: &str) -> BoxLine {
        BoxLine::Field {
            label,
            value: value.to_string(),
            color: TELETEXT_CYAN,
        }
    }

    #[test]
    fn box_lines_up_field_values_with_one_space_after_the_longest_label() {
        let lines = [
            BoxLine::Text("Status".to_string()),
            BoxLine::blank(),
            field("Version", "1.2.3"),
            field("Platform", "macos aarch64"),
        ];
        assert_eq!(
            render_plain(&lines),
            "╔═════════════════════════╗\n\
             ║ Status                  ║\n\
             ╠═════════════════════════╣\n\
             ║                         ║\n\
             ║ Version:  1.2.3         ║\n\
             ║ Platform: macos aarch64 ║\n\
             ╚═════════════════════════╝\n"
        );
    }

    #[test]
    fn box_without_fields_has_no_label_padding() {
        let lines = [BoxLine::Colored(
            "liiga_teletext --update".to_string(),
            TELETEXT_CYAN,
        )];
        assert_eq!(
            render_plain(&lines),
            "╔═════════════════════════╗\n\
             ║ liiga_teletext --update ║\n\
             ╚═════════════════════════╝\n"
        );
    }

    #[test]
    fn box_colors_only_when_asked() {
        let lines = [field("Version", "1.2.3")];
        assert!(!render_box(&lines, false).contains('\x1b'));
        let colored = render_box(&lines, true);
        assert!(
            colored.contains("\x1b[38;5;51m1.2.3"),
            "value in cyan: {colored:?}"
        );
    }

    fn rendered_version_box(latest: Option<&Version>) -> String {
        render_plain(&version_box(latest))
    }

    #[test]
    fn up_to_date_box_shows_version_and_platform() {
        let output = rendered_version_box(Some(&current_version()));
        assert!(output.contains(&format!("Version:  {CURRENT_VERSION} ")));
        assert!(output.contains(&format!("Platform: {} ", platform())));
        assert!(output.contains("You're running the latest version!"));
        assert!(!output.contains("--update"));
    }

    #[test]
    fn update_box_shows_both_versions_and_the_update_command() {
        let newer = Version::new(current_version().major + 1, 0, 0);
        let output = rendered_version_box(Some(&newer));
        assert!(output.contains(&format!("Version:  {CURRENT_VERSION} ")));
        assert!(output.contains(&format!("Latest:   {newer} ")));
        assert!(output.contains("liiga_teletext --update"));
    }

    #[test]
    fn failed_check_still_shows_the_version() {
        let output = rendered_version_box(None);
        assert!(output.contains(&format!("Version:  {CURRENT_VERSION} ")));
        assert!(output.contains("Couldn't check for updates."));
        assert!(!output.contains("latest version"));
    }

    #[test]
    fn plain_version_is_name_and_version() {
        assert_eq!(plain_version(), format!("liiga_teletext {CURRENT_VERSION}"));
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
