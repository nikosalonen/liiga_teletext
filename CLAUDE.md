# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rust terminal application displaying Finnish Hockey League (Liiga) results in authentic YLE Teksti-TV (teletext) style. Features real-time data fetching from the Liiga API, multi-level caching, interactive page navigation, and multiple display modes.

- **Rust Edition**: 2024, MSRV 1.88 (let chains). Keep `rust-version` in Cargo.toml, the README, and the CI `msrv` job in sync; only bump when code actually needs a newer feature
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
- **`data_fetcher/api/`** — HTTP client, API orchestration, URL building, date/season logic, bracket API
- **`data_fetcher/models/`** — API response types (`GameData`, `ScheduleResponse`, `DetailedGameResponse`, `StandingsResponse`, `BracketResponse`)
- **`data_fetcher/game_utils.rs`** — Game state utilities and helper functions
- **`data_fetcher/processors/`** — Game status determination, goal processing, player name fetching
- **`data_fetcher/cache/`** — Multi-level TTL caching (HTTP responses, tournaments, games, goals, players) and persistent disk-backed player name store
- **`data_fetcher/player_names/`** — Name formatting and disambiguation (e.g., "Saarela #7")
- **`teletext_ui/`** — Teletext page structure, rendering pipeline, pagination, display modes, standings display, bracket display, season utils
- **`teletext_ui/layout/`** — Column layout management, ANSI cache, layout config
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
  4. Process keyboard events (←/→ pages, Shift+←/→ dates, 's' standings, 'p' bracket (playoffs only), 'l' live mode, 't' today, 'r' refresh, 'q' quit, digits for teletext page entry: 221 games / 222 standings / 223 bracket, others show a "SIVUA EI LÖYDY" page)
  5. Sleep 50ms
}
```

Auto-refresh intervals: 15 seconds during live games, 30 seconds near start time, 60 seconds otherwise (completed games served from 1-hour cache). Polling rate adapts to idle time (50ms → 200ms → 500ms).

### Caching Strategy

**In-memory TTL cache** — varies by game state:
- Live games: 15s
- Completed games: 1 hour
- Starting soon: 30s
- Player data: never expires (LRU eviction only)

**Persistent player name cache** (`data_fetcher/cache/persistence.rs`) — disk-backed JSON store keyed by team per season. Disambiguated player names (e.g., "A. Saarela") are persisted to the platform cache directory so completed games skip the detailed game API endpoint on restart. Uses atomic write (tmp + rename), a sequence counter for dirty tracking, and season-scoped files (`players_{season}.json`).

**Tournament negative cache** (`data_fetcher/api/tournament_logic.rs`) — secondary tournament endpoints (e.g. unannounced `valmistavat_ottelut`) that fail with 502/503/404 are skipped for 15 minutes, with a reduced retry budget (1 instead of 3). `runkosarja` is exempt and always re-checked.

**Playoff bracket visibility** (`data_fetcher/api/bracket_api.rs`) — the bracket (`p` / page 223) is hidden once every playoff game concluded more than 14 days ago (`LIIGA_BRACKET_GRACE_DAYS` overrides). The bracket renders as a full side-by-side path with connectors on terminals ≥ ~80x24 (`teletext_ui/bracket_display.rs::render_full_path`), falling back to sequential tree and stacked layouts on smaller terminals.

### Configuration

TOML config at platform-specific paths (Linux: `~/.config/liiga_teletext/`, macOS: `~/Library/Application Support/liiga_teletext/`, Windows: `%APPDATA%\liiga_teletext/`).

Environment variable overrides: `LIIGA_API_DOMAIN`, `LIIGA_LOG_FILE`, `LIIGA_HTTP_TIMEOUT`, `LIIGA_API_FETCH_TIMEOUT`, `LIIGA_BRACKET_GRACE_DAYS` (extends playoff bracket visibility, e.g. `400` to view last season's bracket in the off-season).

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

<!-- code-review-graph MCP tools -->
## MCP Tools: code-review-graph

**IMPORTANT: This project has a knowledge graph. ALWAYS use the
code-review-graph MCP tools BEFORE using Grep/Glob/Read to explore
the codebase.** The graph is faster, cheaper (fewer tokens), and gives
you structural context (callers, dependents, test coverage) that file
scanning cannot.

### When to use graph tools FIRST

- **Exploring code**: `semantic_search_nodes` or `query_graph` instead of Grep
- **Understanding impact**: `get_impact_radius` instead of manually tracing imports
- **Code review**: `detect_changes` + `get_review_context` instead of reading entire files
- **Finding relationships**: `query_graph` with callers_of/callees_of/imports_of/tests_for
- **Architecture questions**: `get_architecture_overview` + `list_communities`

Fall back to Grep/Glob/Read **only** when the graph doesn't cover what you need.

### Key Tools

| Tool | Use when |
| ------ | ---------- |
| `detect_changes` | Reviewing code changes — gives risk-scored analysis |
| `get_review_context` | Need source snippets for review — token-efficient |
| `get_impact_radius` | Understanding blast radius of a change |
| `get_affected_flows` | Finding which execution paths are impacted |
| `query_graph` | Tracing callers, callees, imports, tests, dependencies |
| `semantic_search_nodes` | Finding functions/classes by name or keyword |
| `get_architecture_overview` | Understanding high-level codebase structure |
| `refactor_tool` | Planning renames, finding dead code |

### Workflow

1. The graph auto-updates on file changes (via hooks).
2. Use `detect_changes` for code review.
3. Use `get_affected_flows` to understand impact.
4. Use `query_graph` pattern="tests_for" to check coverage.
