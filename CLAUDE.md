# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust terminal application that displays Finnish Hockey League (Liiga) results in authentic YLE Teksti-TV style. The app features real-time data fetching, teletext-style UI rendering, and comprehensive CLI options.

## Development Commands

### Building and Testing
```bash
# Build the project
cargo build --release

# Run the application
cargo run --release

# Run tests (MUST pass before any commit)
cargo test --all-features

# Format code
cargo fmt

# Run clippy (MUST pass with zero warnings)
cargo clippy --all-features --all-targets -- -D warnings
```

### Running the Application
```bash
# Interactive mode (default)
cargo run --release

# Show games for specific date
cargo run --release -- --date 2025-01-15

# One-time display (no refresh loop)
cargo run --release -- --once

# Compact multi-column layout
cargo run --release -- --compact

# Wide two-column layout (requires 128+ character terminal)
cargo run --release -- --wide
```

## Code Architecture

### Module Structure
The codebase follows a modular architecture with clear separation of concerns:

- **`src/main.rs`** - Entry point and CLI argument handling
- **`src/app.rs`** - Main application logic and interactive UI loop
- **`src/data_fetcher/`** - API integration and data processing
  - `api/` - HTTP client and API calls
  - `models/` - Data structures and API response models
  - `processors/` - Data transformation and business logic
  - `cache/` - Response caching with TTL
- **`src/teletext_ui/`** - Teletext-style UI rendering and page management
- **`src/ui/`** - Modern UI components and utilities
- **`src/config.rs`** - TOML-based configuration management
- **`src/error.rs`** - Centralized error handling with thiserror

### Key Design Patterns

**Async/Await Architecture**: All I/O operations use Tokio's async runtime
```rust
#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Implementation
}
```

**Error Handling**: Uses thiserror for comprehensive error types
```rust
// Always propagate errors with ?
let config = Config::load().await?;
let data = fetch_liiga_data(&config).await?;
```

**Configuration Management**: Platform-specific config directories
```rust
// Config automatically loaded from:
// Linux: ~/.config/liiga_teletext/config.toml
// macOS: ~/Library/Application Support/liiga_teletext/config.toml
// Windows: %APPDATA%\liiga_teletext\config.toml
let config = Config::load().await?;
```

## Critical Requirements

### Before Any Commit
1. **MUST run and pass**: `cargo test --all-features`
2. **MUST pass with zero warnings**: `cargo clippy --all-features --all-targets -- -D warnings`
3. **MUST format**: `cargo fmt`

### Common Clippy Issues to Avoid
- **await_holding_lock**: Use `tokio::sync::Mutex` instead of `std::sync::Mutex` in async code
- **uninlined_format_args**: Use `format!("{var}")` instead of `format!("{}", var)`
- **assertions_on_constants**: Remove `assert!(true)` or similar constant assertions
- **needless_return**: Remove explicit `return` on last expression
- **unused_mut**: Remove unnecessary `mut` keywords

### Testing Guidelines
- Use `#[tokio::test]` for async test functions
- Mock external dependencies with wiremock for HTTP calls
- Use tempfile for file operations in tests
- Test both success and failure scenarios
- Add tests for any new functionality

## Configuration

The application uses TOML configuration stored in platform-specific directories:

```toml
# config.toml
api_domain = "https://liiga.fi/api/v2"
log_file_path = "/custom/path/to/logfile.log"  # Optional
```

Configuration is managed through:
```bash
# Update API domain
cargo run -- --config

# List current config
cargo run -- --list-config

# Set custom log file
cargo run -- --set-log-file /path/to/logfile.log
```

## Key Dependencies

- **tokio**: Async runtime (with "full" features)
- **reqwest**: HTTP client (with "json" and "blocking" features)
- **crossterm**: Cross-platform terminal manipulation
- **clap**: CLI parsing (with "derive" features)
- **serde**: Serialization (with "derive" features)
- **chrono**: Date/time handling
- **thiserror**: Error handling

## Development Notes

- **Rust Edition**: 2024 (requires Rust 1.89+)
- **Target Platforms**: Linux, macOS, Windows
- **Terminal Requirements**: Unicode support recommended
- **API Integration**: Real-time data from Liiga API with intelligent caching
- **UI Style**: Authentic YLE Teksti-TV channel 221 aesthetics

When adding new features, follow existing patterns for error handling, async/await usage, and configuration management. Always test both interactive and non-interactive modes.