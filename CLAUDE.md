# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust terminal application that displays Finnish Liiga hockey results in an authentic YLE Teksti-TV (teletext) style interface. The application fetches real-time game data from the Liiga API and presents it with nostalgic teletext aesthetics, including proper color schemes, pagination, and interactive navigation.

## Development Commands

### Build and Test
```bash
# Standard build
cargo build --release

# Build with all features enabled
cargo build --all-features

# Run tests (ALWAYS run with all features)
cargo test --all-features

# Run the application
cargo run --release
```

### Code Quality (MANDATORY before commits)
```bash
# Format code
cargo fmt

# Run clippy with zero warnings required
cargo clippy --all-features --all-targets -- -D warnings

# Check for unused dependencies (if available)
cargo machete
```

### Running the Application
```bash
# Interactive mode (default)
./target/release/liiga_teletext

# Show today's scores once and exit
./target/release/liiga_teletext --once

# Compact multi-column layout
./target/release/liiga_teletext --compact

# Wide two-column layout (requires 128+ char terminal)
./target/release/liiga_teletext --wide

# Show specific date
./target/release/liiga_teletext --date 2024-01-15

# Configure API domain
./target/release/liiga_teletext --config https://api.example.com

# Debug mode with file logging
./target/release/liiga_teletext --debug
```

## Architecture Overview

The application follows a modular async/await architecture:

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   CLI/Main      │───▶│   Data Fetcher   │───▶│  Teletext UI    │
│  (main.rs)      │    │ (data_fetcher/)  │    │(teletext_ui.rs) │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                        │                       │
         ▼                        ▼                       ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Config        │    │     Models       │    │   Performance   │
│  (config.rs)    │    │   (models.rs)    │    │ (performance/)  │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │
         ▼
┌─────────────────┐
│  Error Handling │
│   (error.rs)    │
└─────────────────┘
```

### Key Modules

#### `src/main.rs`
- Application entry point with CLI argument parsing using clap
- Interactive UI event loop with crossterm for terminal manipulation
- Version checking and automatic update notifications
- Automatic refresh logic based on game states (every minute for live games, hourly for completed)

#### `src/data_fetcher/`
- **`api.rs`**: HTTP client with connection pooling, timeout handling, 429 rate limit handling with jittered backoff
- **`models.rs`**: Data structures for API responses (ScheduleGame, DetailedGame, GoalEvent, GameData)
- **`processors.rs`**: Business logic for game state determination, score formatting, goal event processing
- **`cache.rs`**: In-memory HTTP response caching with LRU eviction and configurable TTL

#### `src/teletext_ui.rs`
- Teletext-style rendering with authentic YLE Teksti-TV colors and layout
- Automatic pagination based on terminal height
- Support for compact, wide, and standard display modes
- Goal scorer information with timestamps and video links

#### `src/config.rs`
- TOML-based configuration with platform-specific storage locations
- API domain management with automatic https:// prefix handling
- Custom log file path configuration

#### `src/error.rs`
- Centralized error handling using thiserror with context-specific error types
- Error types: ApiFetch, ApiParse, Config, DateTimeParse, LogSetup

## Critical Development Guidelines

### MANDATORY: Clippy Warning Prevention

**NEVER** introduce these common warnings:

1. **`await_holding_lock`** - Use `tokio::sync::Mutex` instead of `std::sync::Mutex` in async code
2. **`uninlined_format_args`** - Always use `format!("{var}")` instead of `format!("{}", var)`
3. **`assertions_on_constants`** - Remove `assert!(true)` or make meaningful assertions
4. **`needless_return`** - Remove explicit `return` from last expression
5. **`unused_mut`** - Remove `mut` from variables that aren't mutated
6. **`redundant_clone`** - Remove unnecessary `.clone()` calls

### Testing Requirements

- **MANDATORY**: Run `cargo test --all-features` before any commit
- All tests must pass locally before pushing changes
- Add tests for any new functionality or bug fixes
- Use `#[tokio::test]` for async test functions
- Mock external dependencies with wiremock for HTTP calls
- Use tempfile for test file operations
- Test both success and failure scenarios

### Code Patterns

#### Error Handling
```rust
// Always use ? operator for propagating errors
let config = Config::load().await?;

// Use context-specific error types
return Err(AppError::config_error("Invalid API domain"));
```

#### Async/Await Usage
```rust
// Use tokio::main for async main functions
#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Implementation
}

// Handle async errors properly
match fetch_data().await {
    Ok(data) => process_data(data),
    Err(e) => log_error_and_continue(e),
}
```

#### Configuration Management
```rust
// Always load config at application start
let config = Config::load().await?;

// Use config validation
if !config.api_domain.starts_with("https://") {
    return Err(AppError::config_error("API domain must use HTTPS"));
}
```

### HTTP Client Usage
- Always use reqwest with JSON features enabled
- Implement proper timeout handling (30 seconds default)
- Use connection pooling (100 connections per host)
- Handle 429 rate limiting with jittered exponential backoff
- Cache responses appropriately based on content type and game state

### UI/Terminal Handling
- Use crossterm for cross-platform terminal manipulation
- Handle various terminal sizes gracefully (compact/wide modes)
- Maintain authentic teletext color scheme (defined in constants)
- Support both interactive and non-interactive modes
- Provide immediate feedback for user actions

## Key Constants and Configuration

### Cache TTL Values (`src/constants.rs`)
- Live games: 8 seconds (shorter than auto-refresh)
- Completed games: 1 hour
- Starting games: 30 seconds
- Player data: 24 hours

### Refresh Intervals
- Live games: Every minute
- Completed games: Every hour
- Manual refresh: 10 second cooldown

### Display Options
- Standard: Single column with full details
- Compact: Multi-column (1-3 based on terminal width) with abbreviations
- Wide: Two-column side-by-side (requires 128+ char terminal)

## Configuration Files

### Config Location
- Linux: `~/.config/liiga_teletext/config.toml`
- macOS: `~/Library/Application Support/liiga_teletext/config.toml`
- Windows: `%APPDATA%\liiga_teletext\config.toml`

### Log Location
- Default: `~/.config/liiga_teletext/logs/liiga_teletext.log`
- Configurable via `--set-log-file` or config file
- Daily rotation with automatic cleanup

### Sample config.toml
```toml
api_domain = "https://api.example.com"
log_file_path = "/custom/log/path.log"  # Optional
```

## Tournament Support

The application handles different tournament types:
- **Regular Season** (`runkosarja`) - Main league games (September-April)
- **Playoffs** (`playoffs`) - Championship playoffs (March-June)
- **Playout** (`playout`) - Relegation playoffs (March-June)
- **Qualifications** (`qualifications`) - Qualification games (March-June)
- **Practice Games** (`valmistavat_ottelut`) - Preseason (May-September)

## Performance Features

- **Smart caching**: HTTP response caching with time-based expiration
- **Request deduplication**: Prevents simultaneous identical API calls
- **Rate limiting**: Global cooldown with 429 retry handling using jittered backoff
- **Connection pooling**: Efficient HTTP connection reuse
- **Async architecture**: Non-blocking operations for responsive UI

## Current Version and Dependencies

- **Version**: 0.15.10
- **Rust Edition**: 2024
- **Minimum Rust**: 1.90
- **Key Dependencies**: tokio (async runtime), reqwest (HTTP), crossterm (terminal), clap (CLI), chrono (dates), serde (JSON)

## Commit Guidelines

Follow Conventional Commits:
- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation changes
- `refactor:` - Code refactoring
- `test:` - Adding tests
- `chore:` - Maintenance tasks

## Branch Strategy

- `main` - Production branch
- `feat/*` - Feature branches
- Current: `feat/429-jitter-cooldown` (adds 429 rate limit handling with jittered backoff)
