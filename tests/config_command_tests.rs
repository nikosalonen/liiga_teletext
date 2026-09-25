//! Runs the real binary against a config file in a temporary home directory,
//! to check what the config-update commands write back to that file.
#![cfg(unix)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

const FILE_DOMAIN: &str = "https://file.example.com";

/// Where `dirs::config_dir()` puts the config file for this home directory.
fn config_file(home: &Path) -> PathBuf {
    let config_dir = if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
    } else {
        home.join(".config")
    };
    config_dir.join("liiga_teletext/config.toml")
}

fn write_config(home: &Path, content: &str) -> PathBuf {
    let path = config_file(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    path
}

/// Starts a server that answers every request with 200, so the API domain
/// prompt accepts its URL. Returns that URL.
fn start_ok_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        }
    });
    url
}

/// Runs the binary with only the given `LIIGA_*` variables set and `input`
/// on stdin. The process is killed after 30 seconds, so a prompt that
/// waits for more input fails the test instead of hanging it.
fn run(home: &TempDir, args: &[&str], env: &[(&str, &str)], input: &str) -> ExitStatus {
    let mut command = Command::new(env!("CARGO_BIN_EXE_liiga_teletext"));
    command
        .args(args)
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join(".config"))
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .env_remove("LIIGA_API_DOMAIN")
        .env_remove("LIIGA_LOG_FILE")
        .env_remove("LIIGA_HTTP_TIMEOUT")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (key, value) in env {
        command.env(key, value);
    }

    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() > deadline {
            child.kill().unwrap();
            panic!("{args:?} did not finish in 30 seconds (waiting for input?)");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn set_log_file_does_not_save_environment_overrides() {
    let home = tempfile::tempdir().unwrap();
    let config_path = write_config(home.path(), &format!("api_domain = \"{FILE_DOMAIN}\"\n"));
    let log_path = home.path().join("custom.log");

    let status = run(
        &home,
        &["--set-log-file", log_path.to_str().unwrap()],
        &[
            ("LIIGA_API_DOMAIN", "https://env.example.com"),
            ("LIIGA_HTTP_TIMEOUT", "99"),
        ],
        "",
    );

    assert!(status.success(), "command failed: {status}");
    let saved = std::fs::read_to_string(&config_path).unwrap();
    assert!(saved.contains(FILE_DOMAIN), "saved domain changed: {saved}");
    assert!(
        !saved.contains("env.example.com"),
        "env domain saved: {saved}"
    );
    assert!(!saved.contains("99"), "env timeout saved: {saved}");
    assert!(saved.contains("custom.log"), "log path not saved: {saved}");
}

#[test]
fn set_log_file_keeps_an_invalid_config_file() {
    let home = tempfile::tempdir().unwrap();
    let broken = format!("api_domain = \"{FILE_DOMAIN}\"\nhttp_timeout_seconds = \n");
    let config_path = write_config(home.path(), &broken);

    // Answer the domain prompt with a working URL, so that code which
    // falls back to defaults would get as far as saving over the file
    let input = format!("{}\n", start_ok_server());
    let status = run(&home, &["--set-log-file", "/tmp/unused.log"], &[], &input);

    assert!(!status.success(), "command should fail on an invalid file");
    assert_eq!(std::fs::read_to_string(&config_path).unwrap(), broken);
}
