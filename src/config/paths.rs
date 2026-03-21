use std::path::{Path, PathBuf};

/// Returns the platform-specific path for the config file.
///
/// # Returns
/// String containing the absolute path to the config file
///
/// # Notes
/// - Uses platform-specific config directory (e.g., ~/.config on Linux)
/// - Falls back to current directory if config directory is unavailable
pub fn get_config_path() -> String {
    dirs::config_dir()
        .unwrap_or_else(|| Path::new(".").to_path_buf())
        .join("liiga_teletext")
        .join("config.toml")
        .to_string_lossy()
        .to_string()
}

/// Returns the platform-specific path for the log directory.
///
/// # Returns
/// String containing the absolute path to the log directory
///
/// # Notes
/// - Uses platform-specific config directory (e.g., ~/.config on Linux)
/// - Falls back to current directory if config directory is unavailable
pub fn get_log_dir_path() -> String {
    dirs::config_dir()
        .unwrap_or_else(|| Path::new(".").to_path_buf())
        .join("liiga_teletext")
        .join("logs")
        .to_string_lossy()
        .to_string()
}

/// Returns the platform-specific path for the cache directory.
///
/// Uses the platform cache directory (e.g., `~/.cache` on Linux, `%LOCALAPPDATA%` on Windows)
/// rather than the config directory, since cache files can be safely deleted without data loss.
pub fn get_cache_dir_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| Path::new(".").to_path_buf())
        .join("liiga_teletext")
}
