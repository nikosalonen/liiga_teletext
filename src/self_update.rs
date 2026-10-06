//! `--update`: replaces the running binary with the latest release.
//!
//! Prefers a prebuilt binary from the GitHub release. Falls back to
//! `cargo install` when no binary is published for this platform yet.

use crate::error::AppError;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
