//! `--update`: replaces the running binary with the latest release.
//!
//! Prefers a prebuilt binary from the GitHub release. Falls back to
//! `cargo install` when no binary is published for this platform yet.

use crate::error::AppError;
use crate::version;
use semver::Version;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

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

/// True when `exe` (after resolving symlinks) is the cargo-installed binary:
/// it sits directly in `cargo_bin` and has the name `cargo install` uses. A
/// renamed copy there would be left unchanged by `cargo install`.
pub fn is_inside_cargo_bin(exe: &Path, cargo_bin: &Path) -> bool {
    let exe = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
    let cargo_bin = std::fs::canonicalize(cargo_bin).unwrap_or_else(|_| cargo_bin.to_path_buf());
    let installed_name = format!("{}{}", env!("CARGO_PKG_NAME"), std::env::consts::EXE_SUFFIX);
    exe.parent() == Some(cargo_bin.as_path())
        && exe
            .file_name()
            .is_some_and(|name| name == installed_name.as_str())
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

/// Release files are fetched by direct URL, not the GitHub REST API, which
/// limits clients that aren't logged in to 60 requests per hour.
pub const RELEASE_DOWNLOAD_BASE: &str =
    "https://github.com/nikosalonen/liiga_teletext/releases/download";

/// HTTP client for release downloads. The binary is several MB, so there is no
/// limit on the whole request: a slow but steady download keeps going. A
/// connect timeout and a per-read timeout still fail a stalled one.
pub fn download_client() -> Result<reqwest::Client, AppError> {
    Ok(reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
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

/// Swaps `new_binary` in for the running `exe` (already canonicalized) and
/// removes `new_binary` afterwards, whether or not the swap worked.
///
/// Unix renames over the resolved path: `self_replace` follows only one
/// symlink level. Windows cannot overwrite a running `.exe`, so it uses
/// `self_replace`.
fn replace_binary(new_binary: &Path, exe: &Path) -> Result<(), AppError> {
    let exe_dir = exe.parent().unwrap_or(exe);
    #[cfg(unix)]
    let replaced = std::fs::rename(new_binary, exe);
    #[cfg(not(unix))]
    let replaced = self_replace::self_replace(new_binary);
    let _ = std::fs::remove_file(new_binary);
    replaced.map_err(|e| io_error_in(e, exe_dir))
}

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
        .args([
            "install",
            env!("CARGO_PKG_NAME"),
            "--locked",
            "--version",
            &version,
        ])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::SelfUpdate(format!(
            "cargo install exited with {status}"
        )))
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
    let exe = std::fs::canonicalize(&exe).map_err(|e| {
        AppError::SelfUpdate(format!(
            "cannot resolve the path of the running binary {}: {e}",
            exe.display()
        ))
    })?;
    let exe_dir = exe.parent().ok_or_else(|| {
        AppError::SelfUpdate("cannot find the folder of the running binary".to_string())
    })?;

    let reason = match asset_name() {
        Some(asset) => {
            let client = download_client()?;
            match try_prebuilt(&client, RELEASE_DOWNLOAD_BASE, &latest, asset, exe_dir).await? {
                Prebuilt::Ready(new_binary) => {
                    println!("Checksum OK");
                    replace_binary(&new_binary, &exe)?;
                    return Ok(UpdateOutcome::Replaced {
                        from: current,
                        to: latest,
                    });
                }
                Prebuilt::Unavailable(reason) => reason,
            }
        }
        None => "No prebuilt binary for this platform".to_string(),
    };

    if cargo_fallback_allowed(&exe) {
        println!("{reason}");
        println!("Installing with cargo instead ...");
        run_cargo_install(&latest)?;
        return Ok(UpdateOutcome::InstalledWithCargo { to: latest });
    }

    let advice = if asset_name().is_some() {
        "Try again in a few minutes, or run:"
    } else {
        "Run:"
    };
    Err(AppError::SelfUpdate(format!(
        "{reason}. {advice} cargo install {} --locked",
        env!("CARGO_PKG_NAME")
    )))
}

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

    #[test]
    fn renamed_binary_in_cargo_bin_is_not_inside() {
        // `cargo install` would write `liiga_teletext` next to a renamed copy
        // and leave the running file unchanged.
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let exe = bin.join("liiga_teletext-aarch64-apple-darwin");
        std::fs::write(&exe, b"").unwrap();

        assert!(!is_inside_cargo_bin(&exe, &bin));
    }

    #[cfg(unix)]
    #[test]
    fn replace_binary_updates_symlink_target() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        let link = dir.path().join("221");
        let new = dir.path().join("new");
        std::fs::write(&real, "old").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        std::fs::write(&new, "new").unwrap();
        let canonical = std::fs::canonicalize(&link).unwrap();

        replace_binary(&new, &canonical).unwrap();

        assert_eq!(std::fs::read_to_string(&real).unwrap(), "new");
        assert!(std::fs::symlink_metadata(&link).unwrap().is_symlink());
        assert_eq!(std::fs::read_to_string(&link).unwrap(), "new");
        assert!(!new.exists());
    }

    #[cfg(unix)]
    #[test]
    fn replace_binary_failure_removes_new_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("locked");
        let staging = dir.path().join("staging");
        std::fs::create_dir(&locked).unwrap();
        std::fs::create_dir(&staging).unwrap();
        let target = locked.join("liiga_teletext");
        let new = staging.join("new");
        std::fs::write(&target, "old").unwrap();
        std::fs::write(&new, "new").unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
        if std::fs::File::create(locked.join("probe")).is_ok() {
            return; // Running as root: permissions are not enforced.
        }

        let result = replace_binary(&new, &target);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(result.is_err());
        assert!(!new.exists());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "old");
    }

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

        let outcome = try_prebuilt(
            &client,
            &server.uri(),
            &Version::new(1, 2, 3),
            ASSET,
            dir.path(),
        )
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

        let outcome = try_prebuilt(
            &client,
            &server.uri(),
            &Version::new(1, 2, 3),
            ASSET,
            dir.path(),
        )
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
        let outcome = try_prebuilt(
            &client,
            "http://127.0.0.1:1",
            &Version::new(1, 2, 3),
            ASSET,
            dir.path(),
        )
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

        let result = try_prebuilt(
            &client,
            &server.uri(),
            &Version::new(1, 2, 3),
            ASSET,
            dir.path(),
        )
        .await;
        let err = result
            .err()
            .expect("mismatch must be an error, not Unavailable");
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

        let result = try_prebuilt(
            &client,
            &server.uri(),
            &Version::new(1, 2, 3),
            ASSET,
            &locked,
        )
        .await;
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();

        let err = result.err().expect("unwritable folder must be an error");
        assert!(
            err.to_string().contains(&locked.display().to_string()),
            "{err}"
        );
    }
}
