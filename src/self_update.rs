//! `--update`: replaces the running binary with the latest release.
//!
//! Prefers a prebuilt binary from the GitHub release. If none can be used
//! (no binary for this platform, not published yet, GitHub unreachable), falls
//! back to `cargo install`, but only when the running binary is cargo's own
//! copy and `cargo` is on PATH, and never on Windows. A checksum mismatch, an
//! HTTP error from GitHub or a disk error always stops the update.

use crate::error::AppError;
use crate::version;
use semver::Version;
use sha2::{Digest, Sha256};
use std::fmt;
use std::io::ErrorKind;
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

/// File name `cargo install` gives the binary.
fn installed_name() -> String {
    format!("{}{}", env!("CARGO_PKG_NAME"), std::env::consts::EXE_SUFFIX)
}

/// True when `exe` (after resolving symlinks) is the cargo-installed binary:
/// it sits directly in `cargo_bin` and has the name `cargo install` uses. A
/// renamed copy there would be left unchanged by `cargo install`.
pub fn is_inside_cargo_bin(exe: &Path, cargo_bin: &Path) -> bool {
    let exe = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
    let cargo_bin = std::fs::canonicalize(cargo_bin).unwrap_or_else(|_| cargo_bin.to_path_buf());
    exe.parent() == Some(cargo_bin.as_path())
        && exe
            .file_name()
            .is_some_and(|name| name == installed_name().as_str())
}

fn cargo_bin_dir() -> Option<PathBuf> {
    std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".cargo")))
        .map(|cargo_home| cargo_home.join("bin"))
}

/// The `cargo` command, as `run_update` uses it. Tests swap in a fake.
trait Cargo {
    fn on_path(&self) -> bool;
    fn install(&self, version: &Version) -> Result<(), AppError>;
}

struct SystemCargo;

impl Cargo for SystemCargo {
    fn on_path(&self) -> bool {
        Command::new("cargo")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    fn install(&self, version: &Version) -> Result<(), AppError> {
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
}

/// Release files are fetched by direct URL, not the GitHub REST API, which
/// limits clients that aren't logged in to 60 requests per hour.
pub const RELEASE_DOWNLOAD_BASE: &str =
    "https://github.com/nikosalonen/liiga_teletext/releases/download";

/// Release page, shown to users who have to download a binary by hand.
const RELEASES_PAGE: &str = "https://github.com/nikosalonen/liiga_teletext/releases";

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

/// Fetches `<asset>.sha256` for `version`. `Ok(None)` on 404: the release, or
/// this platform's file in it, isn't published yet.
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
    parse_checksum(&body).map(Some).map_err(|_| {
        AppError::SelfUpdate(format!(
            "{url} does not contain a SHA-256 digest. A proxy may be changing the download."
        ))
    })
}

/// Turns a disk error into a message that names the folder, so the user knows
/// where the update tried to write.
fn io_error_in(err: std::io::Error, dir: &Path) -> AppError {
    let dir = dir.display();
    AppError::SelfUpdate(match err.kind() {
        ErrorKind::PermissionDenied => {
            format!("no permission to write to {dir}. Re-run with write access to that folder.")
        }
        ErrorKind::ReadOnlyFilesystem => format!("{dir} is on a read-only file system"),
        _ => format!("could not write to {dir}: {err}"),
    })
}

/// Removes a temp file the update no longer needs. A failure is only logged:
/// the update itself already succeeded or failed.
fn remove_temp_file(path: &Path) {
    if let Err(e) = std::fs::remove_file(path)
        && e.kind() != ErrorKind::NotFound
    {
        tracing::warn!("Could not remove {}: {e}", path.display());
    }
}

/// Streams `url` into a temp file in `dest_dir` and checks its SHA-256.
/// On any error the temp file is removed. Callers pass the binary's own
/// folder, so the final swap is a rename within one file system.
pub async fn download_and_verify(
    client: &reqwest::Client,
    url: &str,
    expected: &[u8; 32],
    dest_dir: &Path,
) -> Result<PathBuf, AppError> {
    let temp_path = dest_dir.join(format!(".liiga_teletext-update-{}", std::process::id()));
    let result = write_verified(client, url, expected, &temp_path, dest_dir).await;
    if result.is_err() {
        remove_temp_file(&temp_path);
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
        file.write_all(&chunk)
            .await
            .map_err(|e| io_error_in(e, dest_dir))?;
    }
    file.sync_all()
        .await
        .map_err(|e| io_error_in(e, dest_dir))?;
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
        tokio::fs::set_permissions(temp_path, std::fs::Permissions::from_mode(0o755))
            .await
            .map_err(|e| io_error_in(e, dest_dir))?;
    }
    Ok(())
}

/// Why no prebuilt binary could be used. Only these cases may fall back to
/// `cargo install`.
#[derive(Debug)]
pub enum Unavailable {
    /// No binary is built for this OS and CPU.
    NoAssetForPlatform,
    /// The release, or this platform's file in it, isn't published yet.
    NotPublished(Version),
    /// GitHub could not be reached, or the download broke off.
    Network(reqwest::Error),
}

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAssetForPlatform => write!(f, "No prebuilt binary for this platform"),
            Self::NotPublished(version) => {
                write!(f, "Binaries for {version} are not published yet")
            }
            Self::Network(e) => write!(f, "Could not download from GitHub releases: {e}"),
        }
    }
}

/// Result of looking for a prebuilt binary.
pub enum Prebuilt {
    /// Downloaded and verified; ready to swap in.
    Ready(PathBuf),
    /// Not usable for a reason that allows the cargo fallback.
    Unavailable(Unavailable),
}

/// Sorts a download error into "may fall back to cargo" or "stop".
///
/// Only a failure to reach GitHub may fall back. An HTTP error status (403,
/// 429, 5xx) means GitHub answered but refused; that is usually brief, and
/// starting a several-minute source build would be a surprising answer to it.
/// A checksum mismatch, a bad checksum file or a disk error means something is
/// wrong, and falling back would hide it.
fn unavailable_or_error(err: AppError) -> Result<Unavailable, AppError> {
    match err {
        AppError::ApiFetch(e) if e.status().is_none() => Ok(Unavailable::Network(e)),
        AppError::ApiFetch(e) => Err(AppError::SelfUpdate(format!(
            "GitHub releases answered with an error: {e}. Try again later."
        ))),
        other => Err(other),
    }
}

/// Downloads and verifies the prebuilt binary for `version`.
///
/// A 404 or a network failure gives `Unavailable`. Anything else is an `Err`;
/// see `unavailable_or_error`.
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
            return Ok(Prebuilt::Unavailable(Unavailable::NotPublished(
                version.clone(),
            )));
        }
        Err(e) => return unavailable_or_error(e).map(Prebuilt::Unavailable),
    };

    println!("Downloading {asset} ...");
    let url = format!("{release_base}/v{version}/{asset}");
    match download_and_verify(client, &url, &expected, dest_dir).await {
        Ok(path) => Ok(Prebuilt::Ready(path)),
        Err(e) => unavailable_or_error(e).map(Prebuilt::Unavailable),
    }
}

/// Swaps `new_binary` in for the running `exe` (already canonicalized) and
/// removes `new_binary` afterwards, whether or not the swap worked.
///
/// Unix renames over the resolved path: `self_replace` follows only one
/// symlink level. The new file keeps the mode set by `download_and_verify`
/// (0o755) and is owned by the user running the update. Windows cannot
/// overwrite a running `.exe`, so it uses `self_replace`, which finds the
/// running binary itself; there `exe` only names the folder in errors.
fn replace_binary(new_binary: &Path, exe: &Path) -> Result<(), AppError> {
    let exe_dir = exe.parent().unwrap_or(exe);
    #[cfg(unix)]
    let replaced = std::fs::rename(new_binary, exe);
    #[cfg(not(unix))]
    let replaced = self_replace::self_replace(new_binary);
    remove_temp_file(new_binary);
    replaced.map_err(|e| io_error_in(e, exe_dir))
}

/// What `run_update` did.
pub enum UpdateOutcome {
    AlreadyLatest(Version),
    Replaced { from: Version, to: Version },
    InstalledWithCargo { to: Version },
}

/// Error text for when the update can't finish by itself.
///
/// `cargo install` is suggested only to users who installed with cargo (or on
/// a platform with no prebuilt binary, where it is the only way). For anyone
/// else it would add a second copy in cargo's bin folder and leave the binary
/// they actually run unchanged.
fn manual_steps(reason: &Unavailable, latest: &Version, installed_with_cargo: bool) -> String {
    let cargo_install = concat!("cargo install ", env!("CARGO_PKG_NAME"), " --locked");
    match reason {
        Unavailable::NoAssetForPlatform => format!("{reason}. Run: {cargo_install}"),
        _ if installed_with_cargo => {
            format!("{reason}. Try again in a few minutes, or run: {cargo_install}")
        }
        _ => format!(
            "{reason}. Try again in a few minutes, or download it from {RELEASES_PAGE}/tag/v{latest}"
        ),
    }
}

/// Everything `run_update` looks up or runs. Tests point it at mock servers, a
/// binary in a temp folder and a fake cargo.
struct Updater<'a> {
    crates_io_base: &'a str,
    release_base: &'a str,
    asset: Option<&'a str>,
    /// The running binary, as `current_exe` reports it (may be a symlink).
    exe: PathBuf,
    cargo_bin: Option<PathBuf>,
    cargo: &'a (dyn Cargo + Sync),
}

impl Updater<'_> {
    async fn run(&self) -> Result<UpdateOutcome, AppError> {
        let current = version::current_version();
        let latest = version::fetch_latest_version(self.crates_io_base)
            .await
            .map_err(|e| AppError::SelfUpdate(format!("could not check for updates: {e}")))?;
        println!("Current version: {current}");
        println!("Latest version:  {latest}");
        if latest <= current {
            return Ok(UpdateOutcome::AlreadyLatest(current));
        }

        // Resolve symlinks (e.g. /usr/local/bin/221) so we replace the real file.
        let exe = std::fs::canonicalize(&self.exe).map_err(|e| {
            AppError::SelfUpdate(format!(
                "cannot resolve the path of the running binary {}: {e}",
                self.exe.display()
            ))
        })?;
        let exe_dir = exe.parent().ok_or_else(|| {
            AppError::SelfUpdate("cannot find the folder of the running binary".to_string())
        })?;

        let reason = match self.asset {
            Some(asset) => {
                let client = download_client()?;
                match try_prebuilt(&client, self.release_base, &latest, asset, exe_dir).await? {
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
            None => Unavailable::NoAssetForPlatform,
        };

        // `cargo install` replaces this binary only when it is cargo's own
        // copy. On Windows cargo cannot overwrite the running `.exe`.
        let installed_with_cargo = self
            .cargo_bin
            .as_deref()
            .is_some_and(|bin| is_inside_cargo_bin(&exe, bin));
        if installed_with_cargo && !cfg!(windows) && self.cargo.on_path() {
            println!("{reason}");
            println!("Installing with cargo instead ...");
            self.cargo.install(&latest)?;
            return Ok(UpdateOutcome::InstalledWithCargo { to: latest });
        }

        Err(AppError::SelfUpdate(manual_steps(
            &reason,
            &latest,
            installed_with_cargo,
        )))
    }
}

/// Updates the running binary to the latest version on crates.io: swaps in
/// the prebuilt release binary, or falls back to `cargo install` where that
/// is safe. Returns an `Err` with manual steps when neither works.
pub async fn run_update() -> Result<UpdateOutcome, AppError> {
    let exe = std::env::current_exe()
        .map_err(|e| AppError::SelfUpdate(format!("cannot find the running binary: {e}")))?;
    Updater {
        crates_io_base: version::CRATES_IO_BASE,
        release_base: RELEASE_DOWNLOAD_BASE,
        asset: asset_name(),
        exe,
        cargo_bin: cargo_bin_dir(),
        cargo: &SystemCargo,
    }
    .run()
    .await
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
        let exe = bin.join(installed_name());
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

    async fn mount_release(
        server: &MockServer,
        version: &str,
        checksum: ResponseTemplate,
        binary: ResponseTemplate,
    ) {
        Mock::given(method("GET"))
            .and(path(format!("/v{version}/{ASSET}.sha256")))
            .respond_with(checksum)
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/v{version}/{ASSET}")))
            .respond_with(binary)
            .mount(server)
            .await;
    }

    async fn release_server(checksum: ResponseTemplate, binary: ResponseTemplate) -> MockServer {
        let server = MockServer::start().await;
        mount_release(&server, "1.2.3", checksum, binary).await;
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

    async fn try_prebuilt_from(server: &MockServer, dir: &Path) -> Result<Prebuilt, AppError> {
        let client = download_client().unwrap();
        try_prebuilt(&client, &server.uri(), &Version::new(1, 2, 3), ASSET, dir).await
    }

    #[tokio::test]
    async fn checksum_server_error_stops_the_update() {
        // GitHub answered, so this must not count as "unavailable" and start
        // a cargo build.
        for status in [403, 429, 500, 503] {
            let server =
                release_server(ResponseTemplate::new(status), ResponseTemplate::new(404)).await;
            let dir = tempfile::tempdir().unwrap();

            let err = try_prebuilt_from(&server, dir.path())
                .await
                .err()
                .unwrap_or_else(|| panic!("{status} must be an error, not Unavailable"));
            assert!(err.to_string().contains(&status.to_string()), "{err}");
        }
    }

    #[tokio::test]
    async fn binary_server_error_stops_the_update_and_removes_temp_file() {
        let server = release_server(
            ResponseTemplate::new(200).set_body_string(sha256sum_line(NEW_BINARY)),
            ResponseTemplate::new(503),
        )
        .await;
        let dir = tempfile::tempdir().unwrap();

        let result = try_prebuilt_from(&server, dir.path()).await;
        assert!(result.is_err());
        assert_eq!(files_in(dir.path()), 0);
    }

    #[tokio::test]
    async fn html_checksum_file_stops_the_update() {
        // A captive portal or proxy can answer 200 with a web page.
        let server = release_server(
            ResponseTemplate::new(200).set_body_string("<!DOCTYPE html><html>Log in</html>"),
            ResponseTemplate::new(200).set_body_bytes(NEW_BINARY.to_vec()),
        )
        .await;
        let dir = tempfile::tempdir().unwrap();

        let err = try_prebuilt_from(&server, dir.path())
            .await
            .err()
            .expect("a bad checksum file must be an error, not Unavailable");
        assert!(err.to_string().contains(".sha256"), "{err}");
        assert_eq!(files_in(dir.path()), 0);
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

    // `run_update` decisions, through `Updater`.

    use std::sync::atomic::{AtomicBool, Ordering};

    struct FakeCargo {
        on_path: bool,
        ran_install: AtomicBool,
    }

    impl FakeCargo {
        fn new(on_path: bool) -> Self {
            Self {
                on_path,
                ran_install: AtomicBool::new(false),
            }
        }

        fn ran_install(&self) -> bool {
            self.ran_install.load(Ordering::SeqCst)
        }
    }

    impl Cargo for FakeCargo {
        fn on_path(&self) -> bool {
            self.on_path
        }

        fn install(&self, _version: &Version) -> Result<(), AppError> {
            self.ran_install.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    /// Newer than any real release, so the update goes ahead.
    const LATEST: &str = "999.0.0";

    async fn mount_crates_io(server: &MockServer, response: ResponseTemplate) {
        Mock::given(method("GET"))
            .and(path("/api/v1/crates/liiga_teletext"))
            .respond_with(response)
            .mount(server)
            .await;
    }

    fn crates_io_says(version: &str) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "crate": { "max_stable_version": version }
        }))
    }

    /// A home folder with cargo's bin folder and an "old" binary.
    struct Install {
        home: tempfile::TempDir,
        cargo_bin: PathBuf,
        exe: PathBuf,
    }

    /// With `in_cargo_bin` false the binary sits in another folder, like a
    /// prebuilt binary in /usr/local/bin.
    fn install(in_cargo_bin: bool) -> Install {
        let home = tempfile::tempdir().unwrap();
        let cargo_bin = home.path().join("bin");
        std::fs::create_dir(&cargo_bin).unwrap();
        let dir = if in_cargo_bin {
            cargo_bin.clone()
        } else {
            let other = home.path().join("usr-local-bin");
            std::fs::create_dir(&other).unwrap();
            other
        };
        let exe = dir.join(installed_name());
        std::fs::write(&exe, "old").unwrap();
        Install {
            home,
            cargo_bin,
            exe,
        }
    }

    fn updater<'a>(server_uri: &'a str, install: &Install, cargo: &'a FakeCargo) -> Updater<'a> {
        Updater {
            crates_io_base: server_uri,
            release_base: server_uri,
            asset: Some(ASSET),
            exe: install.exe.clone(),
            cargo_bin: Some(install.cargo_bin.clone()),
            cargo,
        }
    }

    /// Server where crates.io offers `LATEST` and the release answers with
    /// `checksum` and `binary`.
    async fn update_server(checksum: ResponseTemplate, binary: ResponseTemplate) -> MockServer {
        let server = MockServer::start().await;
        mount_crates_io(&server, crates_io_says(LATEST)).await;
        mount_release(&server, LATEST, checksum, binary).await;
        server
    }

    #[tokio::test]
    async fn up_to_date_or_newer_does_not_download() {
        // "0.0.1" stands for a yanked release: crates.io offers an older version.
        for offered in [version::current_version().to_string(), "0.0.1".to_string()] {
            let server = MockServer::start().await;
            mount_crates_io(&server, crates_io_says(&offered)).await;
            let install = install(true);
            let cargo = FakeCargo::new(true);
            let uri = server.uri();

            let outcome = updater(&uri, &install, &cargo).run().await.unwrap();

            assert!(matches!(outcome, UpdateOutcome::AlreadyLatest(_)));
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
            assert!(!cargo.ran_install());
        }
    }

    #[tokio::test]
    async fn crates_io_failure_stops_before_download() {
        let server = MockServer::start().await;
        mount_crates_io(&server, ResponseTemplate::new(500)).await;
        let install = install(true);
        let cargo = FakeCargo::new(true);
        let uri = server.uri();

        let err = updater(&uri, &install, &cargo).run().await.err().unwrap();

        assert!(
            err.to_string().contains("could not check for updates"),
            "{err}"
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
        assert!(!cargo.ran_install());
    }

    #[tokio::test]
    async fn checksum_mismatch_never_falls_back_to_cargo() {
        let server = update_server(
            ResponseTemplate::new(200).set_body_string(sha256sum_line(b"something else")),
            ResponseTemplate::new(200).set_body_bytes(NEW_BINARY.to_vec()),
        )
        .await;
        let install = install(true);
        let cargo = FakeCargo::new(true);
        let uri = server.uri();

        let err = updater(&uri, &install, &cargo).run().await.err().unwrap();

        assert!(err.to_string().contains("checksum mismatch"), "{err}");
        assert!(!cargo.ran_install());
        assert_eq!(std::fs::read_to_string(&install.exe).unwrap(), "old");
    }

    #[tokio::test]
    async fn github_error_status_never_falls_back_to_cargo() {
        let server = update_server(ResponseTemplate::new(503), ResponseTemplate::new(404)).await;
        let install = install(true);
        let cargo = FakeCargo::new(true);
        let uri = server.uri();

        let result = updater(&uri, &install, &cargo).run().await;

        assert!(result.is_err());
        assert!(!cargo.ran_install());
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn unpublished_release_falls_back_to_cargo_for_cargo_installs() {
        let server = update_server(ResponseTemplate::new(404), ResponseTemplate::new(404)).await;
        let install = install(true);
        let cargo = FakeCargo::new(true);
        let uri = server.uri();

        let outcome = updater(&uri, &install, &cargo).run().await.unwrap();

        assert!(matches!(outcome, UpdateOutcome::InstalledWithCargo { .. }));
        assert!(cargo.ran_install());
    }

    #[tokio::test]
    async fn unpublished_release_outside_cargo_bin_points_to_release_page() {
        // `cargo install` would add a second copy and leave this one old.
        let server = update_server(ResponseTemplate::new(404), ResponseTemplate::new(404)).await;
        let install = install(false);
        let cargo = FakeCargo::new(true);
        let uri = server.uri();

        let err = updater(&uri, &install, &cargo).run().await.err().unwrap();

        let message = err.to_string();
        assert!(
            message.contains(&format!("{RELEASES_PAGE}/tag/v{LATEST}")),
            "{message}"
        );
        assert!(!message.contains("cargo install"), "{message}");
        assert!(!cargo.ran_install());
    }

    #[tokio::test]
    async fn unpublished_release_without_cargo_on_path_suggests_cargo_install() {
        let server = update_server(ResponseTemplate::new(404), ResponseTemplate::new(404)).await;
        let install = install(true);
        let cargo = FakeCargo::new(false);
        let uri = server.uri();

        let err = updater(&uri, &install, &cargo).run().await.err().unwrap();

        assert!(
            err.to_string()
                .contains("cargo install liiga_teletext --locked"),
            "{err}"
        );
        assert!(!cargo.ran_install());
    }

    #[tokio::test]
    async fn unsupported_platform_suggests_cargo_install() {
        let server = MockServer::start().await;
        mount_crates_io(&server, crates_io_says(LATEST)).await;
        let install = install(false);
        let cargo = FakeCargo::new(true);
        let uri = server.uri();
        let mut updater = updater(&uri, &install, &cargo);
        updater.asset = None;

        let err = updater.run().await.err().unwrap();

        assert!(
            err.to_string()
                .contains("No prebuilt binary for this platform. Run: cargo install"),
            "{err}"
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlinked_binary_replaces_the_real_file() {
        // README suggests `ln -s ~/.cargo/bin/liiga_teletext /usr/local/bin/221`.
        let server = update_server(
            ResponseTemplate::new(200).set_body_string(sha256sum_line(NEW_BINARY)),
            ResponseTemplate::new(200).set_body_bytes(NEW_BINARY.to_vec()),
        )
        .await;
        let mut install = install(true);
        let link = install.home.path().join("221");
        std::os::unix::fs::symlink(&install.exe, &link).unwrap();
        let real = std::mem::replace(&mut install.exe, link.clone());
        let cargo = FakeCargo::new(true);
        let uri = server.uri();

        let outcome = updater(&uri, &install, &cargo).run().await.unwrap();

        assert!(matches!(outcome, UpdateOutcome::Replaced { .. }));
        assert_eq!(std::fs::read(&real).unwrap(), NEW_BINARY);
        assert!(std::fs::symlink_metadata(&link).unwrap().is_symlink());
        assert!(!cargo.ran_install());
    }
}
