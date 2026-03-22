# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rust terminal application displaying Finnish Hockey League (Liiga) results in authentic YLE Teksti-TV (teletext) style. Features real-time data fetching from the Liiga API, multi-level caching, interactive page navigation, and multiple display modes.

- **Rust Edition**: 2024 (requires Rust 1.89+)
- **Async Runtime**: Tokio (full features)
- **Terminal**: Crossterm for cross-platform raw mode / alternate screen

## Development Commands

```bash
# Build
cargo build --release

# Run tests (MUST pass before any commit)
cargo test --all-features

# Run a single test by name
cargo test test_name_here

# Run tests in a specific module
cargo test --all-features -- module_name::

# Clippy (MUST pass with zero warnings before any commit)
cargo clippy --all-features --all-targets -- -D warnings

# Format (MUST run before any commit)
cargo fmt

# Run the app
cargo run --release                          # Interactive mode (default)
cargo run --release -- --once                # One-shot display, exit immediately
cargo run --release -- --date 2025-01-15     # Specific date
cargo run --release -- --compact             # Compact multi-column layout
cargo run --release -- --wide                # Wide two-column (128+ chars terminal)
```

## Code Architecture

### Data Flow

```
main.rs (CLI parsing via clap)
  → commands.rs (dispatch: --once, --version, config ops)
  → app.rs (interactive mode: RAII terminal setup, enters ui::run_interactive_ui)

Data Fetching:
  data_fetcher/api/orchestrator.rs::fetch_liiga_data()
    → determine date → build tournament list → fetch from Liiga API
    → data_fetcher/processors/ (game status, goal events, player names)
    → Returns Vec<GameData> + date string

Rendering:
  teletext_ui/ (TeletextPage: paginated content, buffered rendering)
  ui/interactive/ (event loop: state management, refresh coordination, input handling)
```

### Module Responsibilities

- **`cli.rs`** — Clap `Args` struct with all CLI flags
- **`commands.rs`** — Command handlers for non-interactive modes
- **`app.rs`** — Terminal setup (raw mode, alternate screen) with RAII cleanup
- **`data_fetcher/api/`** — HTTP client, API orchestration, URL building, date/season logic
- **`data_fetcher/models/`** — API response types (`GameData`, `ScheduleResponse`, `DetailedGameResponse`, `StandingsResponse`)
- **`data_fetcher/game_utils.rs`** — Game state utilities and helper functions
- **`data_fetcher/processors/`** — Game status determination, goal processing, player name fetching
- **`data_fetcher/cache/`** — Multi-level TTL caching (HTTP responses, tournaments, games, goals, players) and persistent disk-backed player name store
- **`data_fetcher/player_names/`** — Name formatting and disambiguation (e.g., "Saarela #7")
- **`teletext_ui/`** — Teletext page structure, rendering pipeline, pagination, display modes, standings display, season utils
- **`ui/interactive/`** — Interactive event loop: state management, navigation, refresh coordination, input handling, change detection, standings/series display
- **`schemas/`** — JSON schemas for API responses (game schedule, game details)
- **`ui/components/`** — Team abbreviations
- **`ui/teletext/`** — Colors, game result display, loading indicators, page config
- **`config/`** — TOML config with platform-specific paths, validation, user prompts
- **`error.rs`** — `AppError` enum via thiserror (API, network, config, validation errors)
- **`constants.rs`** — Cache TTLs, polling intervals, timeouts
- **`logging.rs`** — Tracing setup with daily rolling file appender
- **`version.rs`** — Crates.io version check

### Interactive Mode Event Loop (`ui/interactive/core.rs`)

```
loop {
  1. Check if auto-refresh needed (RefreshCoordinator)
  2. Fetch data if refresh requested → update state
  3. Render page if state changed (buffered output)
  4. Process keyboard events (←/→ pages, Shift+←/→ dates, 's' standings, 'r' refresh, 'q' quit)
  5. Sleep 50ms
}
```

Auto-refresh intervals: 1 minute during live games, 1 hour for completed games only. Polling rate adapts to idle time (50ms → 200ms → 500ms).

### Caching Strategy

**In-memory TTL cache** — varies by game state:
- Live games: 15s
- Completed games: 1 hour
- Starting soon: 30s
- Player data: 24 hours

**Persistent player name cache** (`data_fetcher/cache/persistence.rs`) — disk-backed JSON store keyed by team per season. Disambiguated player names (e.g., "A. Saarela") are persisted to the platform cache directory so completed games skip the detailed game API endpoint on restart. Uses atomic write (tmp + rename), a sequence counter for dirty tracking, and season-scoped files (`players_{season}.json`).

### Configuration

TOML config at platform-specific paths (Linux: `~/.config/liiga_teletext/`, macOS: `~/Library/Application Support/liiga_teletext/`, Windows: `%APPDATA%\liiga_teletext/`).

Environment variable overrides: `LIIGA_API_DOMAIN`, `LIIGA_LOG_FILE`, `LIIGA_HTTP_TIMEOUT`.

## Critical Requirements

### Before Any Commit

1. `cargo test --all-features` — must pass
2. `cargo clippy --all-features --all-targets -- -D warnings` — zero warnings
3. `cargo fmt` — must be formatted

### Common Clippy Issues to Avoid

- `await_holding_lock`: Use `tokio::sync::Mutex` instead of `std::sync::Mutex` in async code
- `uninlined_format_args`: Use `format!("{var}")` not `format!("{}", var)`
- `needless_return`: Omit explicit `return` on last expression
- `unused_mut`: Remove unnecessary `mut` keywords

### Testing Conventions

- `#[tokio::test]` for async tests
- `wiremock` for HTTP mocking, `tempfile` for file ops
- `TestDataBuilder` in `testing_utils.rs` for mock game data
- Integration tests in `tests/` directory
