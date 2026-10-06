# Self-updater Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `liiga_teletext --update` installs the latest release: a prebuilt binary from GitHub when one exists for this platform, otherwise `cargo install`.

**Architecture:**
- A new bin-only module, `src/self_update.rs`, holds the update logic.
- It finds the latest version on crates.io, using a reworked `version::fetch_latest_version`.
- It downloads `liiga_teletext-<target>` plus its `.sha256` file from the GitHub release.
- It verifies the checksum while streaming and swaps the binary in with the `self-replace` crate.
- It falls back to `cargo install` only when the running binary lives in cargo's bin folder.
- A new workflow, `release-binaries.yml`, builds 5 targets on each tag and uploads them to the release in one final job.

**Tech Stack:** Rust 2024, tokio, reqwest 0.13 (rustls), `sha2` 0.11, `self-replace` 1.5, wiremock + tempfile for tests, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-06-self-updater-design.md`

## Global Constraints

- MSRV stays at 1.93.1. Don't bump `rust-version`.
- Only two new runtime dependencies: `sha2 = "0.11"` and `self-replace = "1.5"`. No `hex`, no `self_update`, no archive crates.
- Update starts only from an explicit `--update` flag. No prompts and no automatic updates.
- Latest version comes from crates.io `max_stable_version`.
- Asset names: `liiga_teletext-aarch64-apple-darwin`, `liiga_teletext-x86_64-apple-darwin`, `liiga_teletext-x86_64-unknown-linux-musl`, `liiga_teletext-aarch64-unknown-linux-musl`, `liiga_teletext-x86_64-pc-windows-msvc.exe`. Each has a `<asset>.sha256` file next to it.
- Download URL: `https://github.com/nikosalonen/liiga_teletext/releases/download/v<version>/<asset>`. Don't use the GitHub REST API.
- Download client timeout: 120 s. crates.io lookup timeout: 10 s.
- A checksum mismatch aborts and never falls back to cargo.
- The cargo fallback runs only when `cargo` is on PATH **and** the running binary (after resolving symlinks) is in `$CARGO_HOME/bin` or `~/.cargo/bin`. Never on Windows.
- Never call `sudo`.
- Before every commit: `cargo fmt`, `cargo clippy --all-features --all-targets -- -D warnings` (zero warnings), and `cargo test --all-features`.
- Clippy style: inline format args (`format!("{x}")`), no needless `return`.
- Every commit message ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Launched through a symlink.** The README suggests `sudo ln -s ~/.cargo/bin/liiga_teletext /usr/local/bin/221`. Running `221 --update` must update the real binary in `~/.cargo/bin` and allow the cargo fallback. Pinned by `symlink_into_cargo_bin_counts_as_inside` (Task 2).
2. **Release published, binaries not uploaded yet.** For the few minutes after crates.io shows the new version, `.sha256` returns 404. That must fall back, not fail. Pinned by `missing_checksum_means_unavailable` (Task 3).
3. **Corrupt or tampered download.** A mismatch must error, leave no temp file behind and leave the current binary untouched. Pinned by `checksum_mismatch_is_an_error_and_removes_temp_file` (Task 3).
4. **Install folder not writable** (for example a root-owned `/usr/local/bin`). The user must get a clear message that names the folder, not a bare "Permission denied (os error 13)". Pinned by `unwritable_folder_names_the_folder` (Task 3).
5. **Garbage `.sha256` body** (an HTML error page, or non-ASCII text that happens to be 64 bytes long). Parsing must return an error, not panic on a char-boundary slice. Pinned by `checksum_rejects_non_ascii_without_panicking` (Task 2).

---

## File Structure

| File | Change | Responsibility |
| --- | --- | --- |
| `src/version.rs` | Modify | Add `fetch_latest_version` (returns `Result`) and `current_version`; `check_latest_version` wraps it; change hint text to `liiga_teletext --update` |
| `src/self_update.rs` | Create | Asset naming, checksum parsing, cargo-bin check, download + verify, update flow |
| `src/error.rs` | Modify | Add `AppError::SelfUpdate(String)` |
| `src/cli.rs` | Modify | `--update` flag; count it as non-interactive |
| `src/commands.rs` | Modify | `handle_update_command`; reject `--update` combined with other commands |
| `src/main.rs` | Modify | `mod self_update;` and dispatch `--update` |
| `Cargo.toml` | Modify | Add `sha2`, `self-replace` |
| `.github/workflows/release-binaries.yml` | Create | Build 5 targets; upload binaries and checksums to the release on tags |
| `README.md`, `CLAUDE.md` | Modify | Document prebuilt binaries, `--update`, the new module and workflow |
| `docs/superpowers/specs/2026-10-06-self-updater-design.md` | Modify | Record the separate workflow file and the plain output |

`version.rs`, `commands.rs` and `self_update.rs` are bin-only. They are declared in `main.rs`, not `lib.rs`, so unit tests live inside each file.

---

### Task 1: `fetch_latest_version` returns errors instead of printing them

**Files:**
- Modify: `src/version.rs:1-51` (the imports, constants and `check_latest_version`) and `src/version.rs:~152` (hint text)
- Test: the new `#[cfg(test)] mod tests` at the end of `src/version.rs`

**Interfaces:**
- Consumes: `AppError::api_no_data(message, url)` and `AppError::ApiFetch` / `AppError::VersionParse` (`From` impls already exist in `src/error.rs`).
- Produces:
  - `pub const CRATES_IO_BASE: &str = "https://crates.io";`
  - `pub async fn fetch_latest_version(crates_io_base: &str) -> Result<semver::Version, AppError>`
  - `pub async fn check_latest_version() -> Option<String>` (signature unchanged)

- [ ] **Step 1: Write the failing tests**

Append to `src/version.rs`:

```rust
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
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "crate": {} })))
            .mount(&server)
            .await;

        assert!(fetch_latest_version(&server.uri()).await.is_err());
    }
}
```

- [ ] **Step 2: Run the tests and check they fail**

Run: `cargo test --all-features -- version::tests`
Expected: compile error, `cannot find function fetch_latest_version`.

- [ ] **Step 3: Implement**

Replace the top of `src/version.rs` (the imports through the end of `check_latest_version`) with:

```rust
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
        .ok_or_else(|| AppError::api_no_data("crates.io response has no max_stable_version", &url))?;

    Ok(Version::parse(latest)?)
}

/// Checks for the latest version of this crate on crates.io.
///
/// Returns `Some(version_string)` on success, or `None` if the check failed
/// (the error is printed to stderr).
pub async fn check_latest_version() -> Option<String> {
    match fetch_latest_version(CRATES_IO_BASE).await {
        Ok(latest) => Some(latest.to_string()),
        Err(e) => {
            eprintln!("Failed to check for updates: {e}");
            None
        }
    }
}
```

In `print_version_info`, change the hint line:

```rust
            (
                "liiga_teletext --update".to_string(),
                Some(Color::AnsiValue(51)), // Authentic teletext cyan
            ),
```

(It replaces `"cargo install liiga_teletext".to_string()`.)

- [ ] **Step 4: Run the tests and check they pass**

Run: `cargo test --all-features -- version::tests`
Expected: 3 passed.

- [ ] **Step 5: Lint, format, full test run, commit**

```bash
cargo fmt && cargo clippy --all-features --all-targets -- -D warnings && cargo test --all-features
git add src/version.rs
git commit -m "refactor: return errors from crates.io version lookup

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Pure helpers in `self_update.rs` (asset names, checksum parsing, cargo-bin check)

**Files:**
- Create: `src/self_update.rs`
- Modify: `src/error.rs` (add a variant after `LogSetup`), `src/main.rs:1-13` (module list), `Cargo.toml` (`[dependencies]`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `AppError::SelfUpdate(String)` with message `"Update failed: {0}"`
  - `pub fn asset_name_for(os: &str, arch: &str) -> Option<&'static str>`
  - `pub fn asset_name() -> Option<&'static str>`
  - `pub fn parse_checksum(body: &str) -> Result<[u8; 32], AppError>`
  - `pub fn is_inside_cargo_bin(exe: &Path, cargo_bin: &Path) -> bool`
  - `pub fn cargo_fallback_allowed(exe: &Path) -> bool`
  - `fn to_hex(bytes: &[u8]) -> String` (private, used in Task 3)

- [ ] **Step 1: Add dependencies, the error variant and the module stub**

`Cargo.toml`, in `[dependencies]` after `semver`:

```toml
sha2 = "0.11"
self-replace = "1.5"
```

`src/error.rs`, after the `LogSetup(String),` variant:

```rust
    #[error("Update failed: {0}")]
    SelfUpdate(String),
```

`src/main.rs`, in the module list after `mod logging;`:

```rust
#[allow(dead_code)] // Wired up to --update in a later commit; remove then.
mod self_update;
```

Create `src/self_update.rs` with only this header for now:

```rust
//! `--update`: replaces the running binary with the latest release.
//!
//! Prefers a prebuilt binary from the GitHub release. Falls back to
//! `cargo install` when no binary is published for this platform yet.

use crate::error::AppError;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
```

- [ ] **Step 2: Write the failing tests**

Append to `src/self_update.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_each_supported_platform() {
        assert_eq!(
            asset_name_for("macos", "aarch64"),
            Some("liiga_teletext-aarch64-apple-darwin")
        );
        assert_eq!(
            asset_name_for("macos", "x86_64"),
            Some("liiga_teletext-x86_64-apple-darwin")
        );
        assert_eq!(
            asset_name_for("linux", "x86_64"),
            Some("liiga_teletext-x86_64-unknown-linux-musl")
        );
        assert_eq!(
            asset_name_for("linux", "aarch64"),
            Some("liiga_teletext-aarch64-unknown-linux-musl")
        );
        assert_eq!(
            asset_name_for("windows", "x86_64"),
            Some("liiga_teletext-x86_64-pc-windows-msvc.exe")
        );
    }

    #[test]
    fn unsupported_platform_has_no_asset() {
        assert_eq!(asset_name_for("freebsd", "x86_64"), None);
        assert_eq!(asset_name_for("windows", "aarch64"), None);
    }

    const DIGEST_HEX: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

    #[test]
    fn parses_plain_digest_and_sha256sum_format() {
        let plain = parse_checksum(DIGEST_HEX).unwrap();
        let sha256sum = parse_checksum(&format!("{DIGEST_HEX}  liiga_teletext-x\n")).unwrap();
        let upper = parse_checksum(&DIGEST_HEX.to_uppercase()).unwrap();
        assert_eq!(plain, sha256sum);
        assert_eq!(plain, upper);
        assert_eq!(plain[0], 0x9f);
        assert_eq!(plain[31], 0x08);
        assert_eq!(to_hex(&plain), DIGEST_HEX);
    }

    #[test]
    fn checksum_rejects_wrong_length_and_non_hex() {
        assert!(parse_checksum("").is_err());
        assert!(parse_checksum(&DIGEST_HEX[..63]).is_err());
        assert!(parse_checksum(&"g".repeat(64)).is_err());
        assert!(parse_checksum("<!DOCTYPE html><html>Not Found</html>").is_err());
    }

    #[test]
    fn checksum_rejects_non_ascii_without_panicking() {
        // 32 × "é" is 64 bytes, so a length check alone would let it through
        // and byte slicing would panic on a char boundary.
        assert!(parse_checksum(&"é".repeat(32)).is_err());
    }

    #[test]
    fn binary_in_cargo_bin_is_inside() {
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let exe = bin.join("liiga_teletext");
        std::fs::write(&exe, b"").unwrap();

        assert!(is_inside_cargo_bin(&exe, &bin));
    }

    #[test]
    fn binary_elsewhere_is_not_inside() {
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        let other = home.path().join("usr-local-bin");
        std::fs::create_dir(&bin).unwrap();
        std::fs::create_dir(&other).unwrap();
        let exe = other.join("liiga_teletext");
        std::fs::write(&exe, b"").unwrap();

        assert!(!is_inside_cargo_bin(&exe, &bin));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_into_cargo_bin_counts_as_inside() {
        // README suggests `ln -s ~/.cargo/bin/liiga_teletext /usr/local/bin/221`.
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        let other = home.path().join("usr-local-bin");
        std::fs::create_dir(&bin).unwrap();
        std::fs::create_dir(&other).unwrap();
        let real = bin.join("liiga_teletext");
        std::fs::write(&real, b"").unwrap();
        let link = other.join("221");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        assert!(is_inside_cargo_bin(&link, &bin));
    }
}
```

- [ ] **Step 3: Run the tests and check they fail**

Run: `cargo test --all-features -- self_update::tests`
Expected: compile errors, `cannot find function asset_name_for` (and the others).

- [ ] **Step 4: Implement**

Insert between the header and the tests module in `src/self_update.rs`:

```rust
/// Name of the release asset built for `os`/`arch` (values of
/// `std::env::consts::{OS, ARCH}`), or `None` when no binary is built for it.
/// Linux always gets the static musl build so it runs on any distro.
pub fn asset_name_for(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("macos", "aarch64") => Some("liiga_teletext-aarch64-apple-darwin"),
        ("macos", "x86_64") => Some("liiga_teletext-x86_64-apple-darwin"),
        ("linux", "x86_64") => Some("liiga_teletext-x86_64-unknown-linux-musl"),
        ("linux", "aarch64") => Some("liiga_teletext-aarch64-unknown-linux-musl"),
        ("windows", "x86_64") => Some("liiga_teletext-x86_64-pc-windows-msvc.exe"),
        _ => None,
    }
}

/// Release asset for the platform this binary runs on.
pub fn asset_name() -> Option<&'static str> {
    asset_name_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// Parses a `.sha256` file: a 64-character hex digest, optionally followed by
/// whitespace and a file name (the `sha256sum` output format).
pub fn parse_checksum(body: &str) -> Result<[u8; 32], AppError> {
    let hex = body.split_whitespace().next().unwrap_or("");
    // Checking ASCII hex first also makes the byte slicing below safe.
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppError::SelfUpdate(
            "checksum file does not contain a SHA-256 digest".to_string(),
        ));
    }

    let mut digest = [0u8; 32];
    for (i, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|e| AppError::SelfUpdate(format!("invalid checksum: {e}")))?;
    }
    Ok(digest)
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// True when `exe` (after resolving symlinks) sits directly in `cargo_bin`.
pub fn is_inside_cargo_bin(exe: &Path, cargo_bin: &Path) -> bool {
    let exe = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
    let cargo_bin = std::fs::canonicalize(cargo_bin).unwrap_or_else(|_| cargo_bin.to_path_buf());
    exe.parent() == Some(cargo_bin.as_path())
}

fn cargo_bin_dir() -> Option<PathBuf> {
    std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".cargo")))
        .map(|cargo_home| cargo_home.join("bin"))
}

fn cargo_on_path() -> bool {
    Command::new("cargo")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Whether `cargo install` would replace the binary that is running.
///
/// It would not if the binary lives outside cargo's bin folder: cargo would put
/// a second copy in `~/.cargo/bin` and leave this one unchanged. On Windows
/// cargo cannot overwrite the running `.exe` at all.
pub fn cargo_fallback_allowed(exe: &Path) -> bool {
    if cfg!(windows) {
        return false;
    }
    cargo_bin_dir().is_some_and(|bin| is_inside_cargo_bin(exe, &bin)) && cargo_on_path()
}
```

- [ ] **Step 5: Run the tests and check they pass**

Run: `cargo test --all-features -- self_update::tests`
Expected: 8 passed on macOS and Linux (7 on Windows, where the symlink test is skipped).

- [ ] **Step 6: Lint, format, full test run, commit**

```bash
cargo fmt && cargo clippy --all-features --all-targets -- -D warnings && cargo test --all-features
git add Cargo.toml Cargo.lock src/error.rs src/main.rs src/self_update.rs
git commit -m "feat: add self-update helpers for asset names and checksums

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Download and verify a prebuilt binary

**Files:**
- Modify: `src/self_update.rs` (add functions above the tests module, plus tests)

**Interfaces:**
- Consumes (Task 2): `parse_checksum`, `to_hex`, `AppError::SelfUpdate`.
- Produces:
  - `pub const RELEASE_DOWNLOAD_BASE: &str`
  - `pub const RELEASES_PAGE: &str`
  - `pub fn download_client() -> Result<reqwest::Client, AppError>`
  - `pub async fn fetch_checksum(client: &reqwest::Client, release_base: &str, version: &Version, asset: &str) -> Result<Option<[u8; 32]>, AppError>`
  - `pub async fn download_and_verify(client: &reqwest::Client, url: &str, expected: &[u8; 32], dest_dir: &Path) -> Result<PathBuf, AppError>`
  - `pub enum Prebuilt { Ready(PathBuf), Unavailable(String) }`
  - `pub async fn try_prebuilt(client: &reqwest::Client, release_base: &str, version: &Version, asset: &str, dest_dir: &Path) -> Result<Prebuilt, AppError>`
  - `fn io_error_in(err: std::io::Error, dir: &Path) -> AppError` (private, used in Task 4)

- [ ] **Step 1: Write the failing tests**

Add inside the existing `mod tests` in `src/self_update.rs`:

```rust
    use semver::Version;
    use sha2::{Digest, Sha256};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const ASSET: &str = "liiga_teletext-aarch64-apple-darwin";
    const NEW_BINARY: &[u8] = b"pretend this is a new binary";

    fn digest_of(bytes: &[u8]) -> [u8; 32] {
        <[u8; 32]>::try_from(Sha256::digest(bytes).as_slice()).unwrap()
    }

    async fn release_server(checksum: ResponseTemplate, binary: ResponseTemplate) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!("/v1.2.3/{ASSET}.sha256")))
            .respond_with(checksum)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/v1.2.3/{ASSET}")))
            .respond_with(binary)
            .mount(&server)
            .await;
        server
    }

    fn sha256sum_line(bytes: &[u8]) -> String {
        format!("{}  {ASSET}\n", to_hex(&digest_of(bytes)))
    }

    fn files_in(dir: &Path) -> usize {
        std::fs::read_dir(dir).unwrap().count()
    }

    #[tokio::test]
    async fn fetch_checksum_reads_digest() {
        let server = release_server(
            ResponseTemplate::new(200).set_body_string(sha256sum_line(NEW_BINARY)),
            ResponseTemplate::new(404),
        )
        .await;
        let client = download_client().unwrap();

        let digest = fetch_checksum(&client, &server.uri(), &Version::new(1, 2, 3), ASSET)
            .await
            .unwrap();
        assert_eq!(digest, Some(digest_of(NEW_BINARY)));
    }

    #[tokio::test]
    async fn fetch_checksum_server_error_is_an_error() {
        let server = release_server(ResponseTemplate::new(500), ResponseTemplate::new(404)).await;
        let client = download_client().unwrap();

        let result = fetch_checksum(&client, &server.uri(), &Version::new(1, 2, 3), ASSET).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn prebuilt_ready_writes_verified_binary() {
        let server = release_server(
            ResponseTemplate::new(200).set_body_string(sha256sum_line(NEW_BINARY)),
            ResponseTemplate::new(200).set_body_bytes(NEW_BINARY.to_vec()),
        )
        .await;
        let dir = tempfile::tempdir().unwrap();
        let client = download_client().unwrap();

        let outcome = try_prebuilt(&client, &server.uri(), &Version::new(1, 2, 3), ASSET, dir.path())
            .await
            .unwrap();
        let Prebuilt::Ready(new_binary) = outcome else {
            panic!("expected Ready");
        };
        assert_eq!(std::fs::read(&new_binary).unwrap(), NEW_BINARY);
        assert_eq!(new_binary.parent(), Some(dir.path()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&new_binary).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o755);
        }
    }

    #[tokio::test]
    async fn missing_checksum_means_unavailable() {
        // crates.io shows the new version before the binaries finish uploading.
        let server = release_server(ResponseTemplate::new(404), ResponseTemplate::new(404)).await;
        let dir = tempfile::tempdir().unwrap();
        let client = download_client().unwrap();

        let outcome = try_prebuilt(&client, &server.uri(), &Version::new(1, 2, 3), ASSET, dir.path())
            .await
            .unwrap();
        assert!(matches!(outcome, Prebuilt::Unavailable(_)));
        assert_eq!(files_in(dir.path()), 0);
    }

    #[tokio::test]
    async fn unreachable_github_means_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let client = download_client().unwrap();

        // Port 1 on localhost refuses connections.
        let outcome = try_prebuilt(&client, "http://127.0.0.1:1", &Version::new(1, 2, 3), ASSET, dir.path())
            .await
            .unwrap();
        assert!(matches!(outcome, Prebuilt::Unavailable(_)));
    }

    #[tokio::test]
    async fn checksum_mismatch_is_an_error_and_removes_temp_file() {
        let server = release_server(
            ResponseTemplate::new(200).set_body_string(sha256sum_line(b"something else")),
            ResponseTemplate::new(200).set_body_bytes(NEW_BINARY.to_vec()),
        )
        .await;
        let dir = tempfile::tempdir().unwrap();
        let client = download_client().unwrap();

        let result = try_prebuilt(&client, &server.uri(), &Version::new(1, 2, 3), ASSET, dir.path()).await;
        let err = result.err().expect("mismatch must be an error, not Unavailable");
        assert!(err.to_string().contains("checksum mismatch"), "{err}");
        assert_eq!(files_in(dir.path()), 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unwritable_folder_names_the_folder() {
        use std::os::unix::fs::PermissionsExt;
        let server = release_server(
            ResponseTemplate::new(200).set_body_string(sha256sum_line(NEW_BINARY)),
            ResponseTemplate::new(200).set_body_bytes(NEW_BINARY.to_vec()),
        )
        .await;
        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
        if std::fs::File::create(locked.join("probe")).is_ok() {
            return; // Running as root: permissions are not enforced.
        }
        let client = download_client().unwrap();

        let result = try_prebuilt(&client, &server.uri(), &Version::new(1, 2, 3), ASSET, &locked).await;
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();

        let err = result.err().expect("unwritable folder must be an error");
        assert!(err.to_string().contains(&locked.display().to_string()), "{err}");
    }
```

- [ ] **Step 2: Run the tests and check they fail**

Run: `cargo test --all-features -- self_update::tests`
Expected: compile errors, `cannot find function download_client` (and `try_prebuilt`, `Prebuilt`, `fetch_checksum`).

- [ ] **Step 3: Implement**

Extend the imports at the top of `src/self_update.rs`:

```rust
use crate::error::AppError;
use semver::Version;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
```

Add above the tests module:

```rust
/// Release files are fetched by direct URL, not the GitHub REST API, which
/// limits clients that aren't logged in to 60 requests per hour.
pub const RELEASE_DOWNLOAD_BASE: &str =
    "https://github.com/nikosalonen/liiga_teletext/releases/download";
pub const RELEASES_PAGE: &str = "https://github.com/nikosalonen/liiga_teletext/releases/latest";

/// HTTP client for release downloads. The binary is several MB, so the
/// timeout is longer than the 10 s used for JSON requests.
pub fn download_client() -> Result<reqwest::Client, AppError> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
        .build()?)
}

/// Fetches `<asset>.sha256` for `version`. `Ok(None)` means the release has
/// no binary for this asset (404).
pub async fn fetch_checksum(
    client: &reqwest::Client,
    release_base: &str,
    version: &Version,
    asset: &str,
) -> Result<Option<[u8; 32]>, AppError> {
    let url = format!("{release_base}/v{version}/{asset}.sha256");
    let response = client.get(&url).send().await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let body = response.error_for_status()?.text().await?;
    parse_checksum(&body).map(Some)
}

/// Turns a permission error into a message that names the folder, so the user
/// knows which folder needs write access.
fn io_error_in(err: std::io::Error, dir: &Path) -> AppError {
    if err.kind() == std::io::ErrorKind::PermissionDenied {
        AppError::SelfUpdate(format!(
            "no permission to write to {}. Re-run with write access to that folder.",
            dir.display()
        ))
    } else {
        AppError::Io(err)
    }
}

/// Streams `url` into a temp file in `dest_dir` and checks its SHA-256.
/// On any error the temp file is removed. The temp file is in the same folder
/// as the binary it will replace, so the final swap is a same-disk rename.
pub async fn download_and_verify(
    client: &reqwest::Client,
    url: &str,
    expected: &[u8; 32],
    dest_dir: &Path,
) -> Result<PathBuf, AppError> {
    let temp_path = dest_dir.join(format!(".liiga_teletext-update-{}", std::process::id()));
    let result = write_verified(client, url, expected, &temp_path, dest_dir).await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temp_path).await;
    }
    result.map(|()| temp_path)
}

async fn write_verified(
    client: &reqwest::Client,
    url: &str,
    expected: &[u8; 32],
    temp_path: &Path,
    dest_dir: &Path,
) -> Result<(), AppError> {
    let mut response = client.get(url).send().await?.error_for_status()?;
    let mut file = tokio::fs::File::create(temp_path)
        .await
        .map_err(|e| io_error_in(e, dest_dir))?;
    let mut hasher = Sha256::new();
    while let Some(chunk) = response.chunk().await? {
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    file.sync_all().await?;
    drop(file);

    let actual = hasher.finalize();
    if actual.as_slice() != expected.as_slice() {
        return Err(AppError::SelfUpdate(format!(
            "checksum mismatch for {url}: expected {}, got {}",
            to_hex(expected),
            to_hex(actual.as_slice())
        )));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(temp_path, std::fs::Permissions::from_mode(0o755)).await?;
    }
    Ok(())
}

/// Result of looking for a prebuilt binary.
pub enum Prebuilt {
    /// Downloaded and verified; ready to swap in.
    Ready(PathBuf),
    /// Not usable for an expected reason (not published yet, network down).
    /// The caller may fall back to cargo. The string says why.
    Unavailable(String),
}

/// Downloads and verifies the prebuilt binary for `version`.
///
/// Network failures and a missing release give `Unavailable`. A checksum
/// mismatch, a bad checksum file or a disk error is an `Err`: those mean
/// something is wrong, and falling back would hide it.
pub async fn try_prebuilt(
    client: &reqwest::Client,
    release_base: &str,
    version: &Version,
    asset: &str,
    dest_dir: &Path,
) -> Result<Prebuilt, AppError> {
    let expected = match fetch_checksum(client, release_base, version, asset).await {
        Ok(Some(digest)) => digest,
        Ok(None) => {
            return Ok(Prebuilt::Unavailable(format!(
                "Binaries for {version} are not published yet"
            )));
        }
        Err(AppError::ApiFetch(e)) => {
            return Ok(Prebuilt::Unavailable(format!(
                "Could not reach GitHub releases: {e}"
            )));
        }
        Err(e) => return Err(e),
    };

    println!("Downloading {asset} ...");
    let url = format!("{release_base}/v{version}/{asset}");
    match download_and_verify(client, &url, &expected, dest_dir).await {
        Ok(path) => Ok(Prebuilt::Ready(path)),
        Err(AppError::ApiFetch(e)) => Ok(Prebuilt::Unavailable(format!("Download failed: {e}"))),
        Err(e) => Err(e),
    }
}
```

If `actual.as_slice()` doesn't compile with the resolved `sha2` version, use `AsRef::<[u8]>::as_ref(&actual)` instead.

- [ ] **Step 4: Run the tests and check they pass**

Run: `cargo test --all-features -- self_update::tests`
Expected: all pass (15 on Unix).

- [ ] **Step 5: Lint, format, full test run, commit**

```bash
cargo fmt && cargo clippy --all-features --all-targets -- -D warnings && cargo test --all-features
git add src/self_update.rs
git commit -m "feat: download and verify prebuilt release binaries

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: `--update` flag and the update flow

**Files:**
- Modify: `src/self_update.rs` (add `UpdateOutcome`, `run_cargo_install`, `run_update`)
- Modify: `src/version.rs` (add `current_version`)
- Modify: `src/cli.rs:24-31` (`is_noninteractive_mode`), `src/cli.rs:~111` (new flag), `src/cli.rs:147-154` (test)
- Modify: `src/commands.rs:17-24` (`validate_args`), plus a new `handle_update_command` and a new tests module
- Modify: `src/main.rs` (remove the `#[allow(dead_code)]` added in Task 2; dispatch `--update`)

**Interfaces:**
- Consumes:
  - Task 1: `version::fetch_latest_version`, `version::CRATES_IO_BASE`
  - Task 2: `asset_name`, `cargo_fallback_allowed`
  - Task 3: `download_client`, `try_prebuilt`, `Prebuilt`, `io_error_in`, `RELEASE_DOWNLOAD_BASE`, `RELEASES_PAGE`
- Produces:
  - `pub fn version::current_version() -> semver::Version`
  - `pub enum UpdateOutcome { AlreadyLatest(Version), Replaced { from: Version, to: Version }, InstalledWithCargo { to: Version } }`
  - `pub async fn run_update() -> Result<UpdateOutcome, AppError>`
  - `pub async fn commands::handle_update_command() -> Result<(), AppError>`
  - `Args::update: bool`

- [ ] **Step 1: Write the failing tests**

In `src/cli.rs`, add to `once_version_and_config_commands_are_noninteractive`:

```rust
        assert!(is_noninteractive_mode(&parse(&["--update"])));
```

Append to `src/commands.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse(flags: &[&str]) -> Args {
        Args::parse_from(std::iter::once("liiga_teletext").chain(flags.iter().copied()))
    }

    #[test]
    fn update_alone_is_valid() {
        assert!(validate_args(&parse(&["--update"])).is_ok());
    }

    #[test]
    fn update_cannot_be_combined_with_other_commands() {
        for other in [["--update", "--once"], ["--update", "--version"], ["--update", "--list-config"]] {
            assert!(validate_args(&parse(&other)).is_err(), "{other:?}");
        }
    }
}
```

- [ ] **Step 2: Run the tests and check they fail**

Run: `cargo test --all-features -- cli::tests commands::tests`
Expected: the parse panics with `unexpected argument '--update' found`.

- [ ] **Step 3: Add the flag and argument validation**

`src/cli.rs`, right after the `version` field:

```rust
    /// Update liiga_teletext to the latest version
    #[arg(long = "update", help_heading = "Info")]
    pub update: bool,
```

`src/cli.rs`, in `is_noninteractive_mode`, add `|| args.update` after `|| args.version`. Also add `/// - --update flag is set` to its doc list.

`src/commands.rs`, in `validate_args` before `Ok(())`:

```rust
    let other_command = args.once
        || args.version
        || args.list_config
        || args.reset_cache
        || args.new_api_domain.is_some()
        || args.new_log_file_path.is_some()
        || args.clear_log_file_path;
    if args.update && other_command {
        return Err(AppError::config_error(
            "--update cannot be combined with other commands",
        ));
    }
```

Run: `cargo test --all-features -- cli::tests commands::tests`
Expected: pass.

- [ ] **Step 4: Check how `self-replace` treats symlinks**

The running binary may be a symlink (`/usr/local/bin/221`). The swap must replace the real file, not the link.

Run: `grep -n "canonicalize\|read_link" ~/.cargo/registry/src/*/self-replace-1.*/src/unix.rs`
- **It canonicalizes `current_exe()`:** use `self_replace::self_replace(&new_binary)` as written in Step 5.
- **It does not:** on Unix, replace the `self_replace::self_replace(&new_binary)` call in Step 5 with `std::fs::rename(&new_binary, &exe)`. `exe` is already canonicalized, and a rename over a running binary is safe on Unix. Keep `self_replace` for Windows only:

```rust
#[cfg(unix)]
let replaced = std::fs::rename(&new_binary, &exe).map_err(|e| io_error_in(e, exe_dir));
#[cfg(windows)]
let replaced = self_replace::self_replace(&new_binary).map_err(|e| io_error_in(e, exe_dir));
```

- [ ] **Step 5: Implement the update flow**

`src/version.rs`, after `CRATES_IO_BASE`:

```rust
/// Version of the running binary.
pub fn current_version() -> Version {
    Version::parse(CURRENT_VERSION).expect("CARGO_PKG_VERSION is valid semver")
}
```

`src/self_update.rs`, add `use crate::version;` to the imports, then above the tests module:

```rust
/// What `run_update` did.
pub enum UpdateOutcome {
    AlreadyLatest(Version),
    Replaced { from: Version, to: Version },
    InstalledWithCargo { to: Version },
}

fn run_cargo_install(version: &Version) -> Result<(), AppError> {
    let version = version.to_string();
    // Output goes straight to the terminal so the user sees cargo's progress.
    let status = Command::new("cargo")
        .args(["install", env!("CARGO_PKG_NAME"), "--locked", "--version", &version])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::SelfUpdate(format!("cargo install exited with {status}")))
    }
}

/// Updates the running binary to the latest version on crates.io.
pub async fn run_update() -> Result<UpdateOutcome, AppError> {
    let current = version::current_version();
    let latest = version::fetch_latest_version(version::CRATES_IO_BASE)
        .await
        .map_err(|e| AppError::SelfUpdate(format!("could not check for updates: {e}")))?;
    println!("Current version: {current}");
    println!("Latest version:  {latest}");
    if latest <= current {
        return Ok(UpdateOutcome::AlreadyLatest(current));
    }

    let exe = std::env::current_exe()?;
    // Resolve symlinks (e.g. /usr/local/bin/221) so we replace the real file.
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    let exe_dir = exe
        .parent()
        .ok_or_else(|| AppError::SelfUpdate("cannot find the folder of the running binary".to_string()))?;

    let reason = match asset_name() {
        Some(asset) => {
            let client = download_client()?;
            match try_prebuilt(&client, RELEASE_DOWNLOAD_BASE, &latest, asset, exe_dir).await? {
                Prebuilt::Ready(new_binary) => {
                    println!("Checksum OK");
                    let replaced =
                        self_replace::self_replace(&new_binary).map_err(|e| io_error_in(e, exe_dir));
                    let _ = std::fs::remove_file(&new_binary);
                    replaced?;
                    return Ok(UpdateOutcome::Replaced { from: current, to: latest });
                }
                Prebuilt::Unavailable(reason) => reason,
            }
        }
        None => "No prebuilt binary for this platform".to_string(),
    };

    println!("{reason}");
    if cargo_fallback_allowed(&exe) {
        println!("Installing with cargo instead ...");
        run_cargo_install(&latest)?;
        return Ok(UpdateOutcome::InstalledWithCargo { to: latest });
    }

    Err(AppError::SelfUpdate(format!(
        "{reason}. Download it from {RELEASES_PAGE} or run: cargo install {} --locked",
        env!("CARGO_PKG_NAME")
    )))
}
```

`src/commands.rs`, add `use crate::self_update::{self, UpdateOutcome};` to the imports, and after `handle_version_command`:

```rust
/// Handles the --update command.
pub async fn handle_update_command() -> Result<(), AppError> {
    match self_update::run_update().await? {
        UpdateOutcome::AlreadyLatest(current) => println!("Already up to date ({current})"),
        UpdateOutcome::Replaced { from, to } => println!("Updated {from} → {to}"),
        UpdateOutcome::InstalledWithCargo { to } => println!("Installed {to} with cargo"),
    }
    Ok(())
}
```

`src/main.rs`:
- Remove the `#[allow(dead_code)] // Wired up ...` line above `mod self_update;`.
- Right after the `if args.version { ... }` block, add:

```rust
    if args.update {
        return commands::handle_update_command().await;
    }
```

- [ ] **Step 6: Lint, format, full test run**

Run: `cargo fmt && cargo clippy --all-features --all-targets -- -D warnings && cargo test --all-features`
Expected: zero warnings, all tests pass. If clippy flags any item in `self_update.rs` as unused, it was meant to be wired up in this task. Find where it should be called rather than adding `allow(dead_code)`.

- [ ] **Step 7: Manual smoke test**

Run: `cargo run -- --update`
Expected: on the current version (0.27.0, the latest on crates.io), it prints the current and latest versions, then `Already up to date (0.27.0)`, and exits with code 0.

Run: `cargo run -- --update --once`
Expected: `Configuration error: --update cannot be combined with other commands`, non-zero exit.

Run: `cargo run -- --help`
Expected: `--update` is listed under "Info".

- [ ] **Step 8: Commit**

```bash
git add src/cli.rs src/commands.rs src/main.rs src/self_update.rs src/version.rs
git commit -m "feat: add --update to install the latest release

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Release workflow that builds and uploads binaries

**Files:**
- Create: `.github/workflows/release-binaries.yml`

**Interfaces:**
- Consumes: the asset names from Global Constraints. They must match `asset_name_for` in Task 2 exactly.
- Produces: on each `v*.*.*` tag, the GitHub release gets 5 binaries and 5 `.sha256` files.

This is a separate file rather than a job in `publish.yml`, so it can run as a dry run on pull requests. `publish.yml` checks that the tag is a version tag, which a PR run would fail. A single `release` job uploads every file at once. That avoids five matrix jobs racing to create the same release.

- [ ] **Step 1: Write the workflow**

```yaml
---
name: Release binaries

on:
  push:
    tags:
      - 'v*.*.*'
  # Dry run: build every target without uploading anything.
  pull_request:
    paths:
      - '.github/workflows/release-binaries.yml'
      - 'Cargo.lock'
  workflow_dispatch:

permissions:
  contents: read

jobs:
  build:
    name: Build ${{ matrix.target }}
    runs-on: ${{ matrix.runner }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: aarch64-apple-darwin
            runner: macos-latest
          - target: x86_64-apple-darwin
            runner: macos-latest
          - target: x86_64-unknown-linux-musl
            runner: ubuntu-latest
          - target: aarch64-unknown-linux-musl
            runner: ubuntu-24.04-arm
          - target: x86_64-pc-windows-msvc
            runner: windows-latest
    steps:
      - uses: actions/checkout@v7

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.target }}

      # aws-lc-sys (rustls crypto) compiles C code and needs musl-gcc.
      - name: Install musl tools
        if: contains(matrix.target, 'musl')
        run: sudo apt-get update && sudo apt-get install -y musl-tools

      - name: Install NASM
        if: runner.os == 'Windows'
        uses: ilammy/setup-nasm@v1

      - name: Build
        run: cargo build --release --locked --target ${{ matrix.target }}

      - name: Stage binary
        shell: bash
        run: |
          mkdir dist
          if [ "$RUNNER_OS" = "Windows" ]; then
            cp "target/${{ matrix.target }}/release/liiga_teletext.exe" \
               "dist/liiga_teletext-${{ matrix.target }}.exe"
          else
            cp "target/${{ matrix.target }}/release/liiga_teletext" \
               "dist/liiga_teletext-${{ matrix.target }}"
          fi

      - uses: actions/upload-artifact@v4
        with:
          name: liiga_teletext-${{ matrix.target }}
          path: dist/*
          if-no-files-found: error

  release:
    name: Upload to GitHub release
    needs: build
    if: startsWith(github.ref, 'refs/tags/v')
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/download-artifact@v4
        with:
          path: dist
          merge-multiple: true

      - name: Write checksums
        working-directory: dist
        run: |
          for f in liiga_teletext-*; do
            sha256sum "$f" > "$f.sha256"
          done
          ls -l

      # Creates the release if the tag was pushed without one; otherwise
      # adds the files to the release made in the GitHub UI.
      - uses: softprops/action-gh-release@v2
        with:
          files: dist/*
```

- [ ] **Step 2: Lint the workflow**

Run: `actionlint .github/workflows/release-binaries.yml` if `actionlint` is installed (`brew install actionlint`). Otherwise run `python3 -c "import yaml,sys; yaml.safe_load(open(sys.argv[1]))" .github/workflows/release-binaries.yml` to at least check the YAML parses.
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release-binaries.yml
git commit -m "ci: build prebuilt binaries for GitHub releases

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 4: Dry run on the PR**

After the branch is pushed and a PR is open, the `pull_request` trigger runs the build matrix (the workflow file changed in this PR).
Check: `gh pr checks --watch`
Expected: all 5 `Build …` jobs pass, and `Upload to GitHub release` is skipped.
If a musl job fails while compiling `aws-lc-sys`, switch those two targets to `cargo-zigbuild`:
- Install it with `pip install ziglang cargo-zigbuild`.
- Build with `cargo zigbuild --release --locked --target <target>`.
- Drop the musl-tools step.

Re-run until green.

---

### Task 6: Documentation

**Files:**
- Modify: `README.md:40-53` (Installation)
- Modify: `CLAUDE.md` (Module Responsibilities; Configuration or a new short "Releases" note)
- Modify: `docs/superpowers/specs/2026-10-06-self-updater-design.md` (record the decisions made while planning)

**Interfaces:**
- Consumes: the final flag name `--update`, asset names and workflow file name from Tasks 4–5.
- Produces: docs only.

- [ ] **Step 1: README**

Replace the start of the "Installation" section (up to and including the symlink block) with:

````markdown
## Installation

### Prebuilt binaries

Download the binary for your platform from the
[latest release](https://github.com/nikosalonen/liiga_teletext/releases/latest).
Builds are available for macOS (Apple Silicon and Intel), Linux (x86_64 and arm64)
and Windows (x86_64). On macOS and Linux, make it executable:

```bash
chmod +x liiga_teletext-*
```

### Install from crates.io

```bash
cargo install liiga_teletext
```

You can create a symlink to the binary to make it available from anywhere:

```bash
sudo ln -s ~/.cargo/bin/liiga_teletext /usr/local/bin/221 # 221 is the channel number of YLE Teksti-TV
```

### Updating

```bash
liiga_teletext --update
```

This downloads the prebuilt binary for your platform and checks its SHA-256
checksum before replacing the current one. If no prebuilt binary is available
and you installed with cargo, it runs `cargo install` for you instead.
````

- [ ] **Step 2: CLAUDE.md**

In "Module Responsibilities", after the `version.rs` line, add:

```markdown
- **`self_update.rs`** — `--update`. Downloads `liiga_teletext-<target>` and its `.sha256` from the GitHub release for the crates.io latest version, verifies it, and swaps it in with `self-replace`. A missing release (404) or network error falls back to `cargo install`, but only when the running binary (symlinks resolved) is in cargo's bin folder and never on Windows. A checksum mismatch is an error and never falls back
```

After the "Configuration" section, add:

```markdown
### Releases

Creating a `vX.Y.Z` release triggers two workflows. `publish.yml` publishes to crates.io. `release-binaries.yml` builds the 5 prebuilt targets and attaches them with `.sha256` files to the GitHub release. Asset names must match `self_update.rs::asset_name_for`. `release-binaries.yml` also runs as a build-only dry run on PRs that touch it or `Cargo.lock`.
```

- [ ] **Step 3: Spec**

In `docs/superpowers/specs/2026-10-06-self-updater-design.md`:
- Under "Release pipeline", replace the first paragraph with: "A separate workflow, `.github/workflows/release-binaries.yml`, runs on `v*.*.*` tags and as a build-only dry run on PRs that touch it or `Cargo.lock`. A matrix `build` job uploads each binary as an artifact. A single `release` job then writes the `.sha256` files with `sha256sum` and uploads everything with `softprops/action-gh-release`, so matrix jobs don't race to create the release."
- Replace steps 3 and 4 of the numbered list with "3. Uploads the binary as a workflow artifact." and delete the line "The job needs `permissions: contents: write`." The `release` job description above now covers checksums, upload and permissions.
- Under "Output", replace "using the same teletext colours as `--version` (white text, cyan for versions)" with "as plain text".
- Under "Flow", change the fallback line to `cargo install liiga_teletext --locked --version <latest>`.

- [ ] **Step 4: Commit**

```bash
git add README.md CLAUDE.md docs/superpowers/specs/2026-10-06-self-updater-design.md
git commit -m "docs: document prebuilt binaries and --update

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## After all tasks

- Push the branch and open a PR. The Task 5 dry run must be green before merging.
- Test the real prebuilt swap by hand. Released 0.27.0 has no `--update`, so use a local build:
  1. Before bumping the version for the release, build this branch: `cargo build --release`. This gives a binary that reports 0.27.0 and has `--update`.
  2. Copy it into a scratch folder, plus a symlink to it named `221`.
  3. After 0.28.0 is released and `release-binaries.yml` has uploaded its assets, run `./221 --update` in that folder.
  4. Expect `Checksum OK`, then `Updated 0.27.0 → 0.28.0`.
  5. Check that `./221 --version` shows 0.28.0 and that `221` is still a symlink.
