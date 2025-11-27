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

```
src/
├── main.rs                   # Entry point
├── lib.rs                    # Library exports
├── app.rs                    # Application entry and terminal management
├── cli.rs                    # CLI argument definitions (clap)
├── commands.rs               # Command handlers
├── constants.rs              # Application constants
├── error.rs                  # Error types (thiserror)
├── logging.rs                # Logging setup (tracing)
├── performance.rs            # Performance utilities
├── testing_utils.rs          # Test utilities
├── version.rs                # Version checking
├── config/                   # Configuration management
│   ├── mod.rs
│   ├── paths.rs              # Platform-specific config paths
│   ├── user_prompts.rs       # User interaction prompts
│   └── validation.rs         # Config validation
├── data_fetcher/             # API and data processing
│   ├── mod.rs
│   ├── game_utils.rs         # Game data utilities
│   ├── api/                  # HTTP client and API calls
│   │   ├── mod.rs
│   │   ├── core.rs           # Core API functionality
│   │   ├── date_logic.rs     # Date handling logic
│   │   ├── fetch_utils.rs    # Fetch utilities
│   │   ├── game_api.rs       # Game data API
│   │   ├── http_client.rs    # HTTP client wrapper
│   │   ├── orchestrator.rs   # API call orchestration
│   │   ├── season_schedule.rs # Season schedule handling
│   │   ├── season_utils.rs   # Season utilities
│   │   ├── tournament_api.rs # Tournament API
│   │   ├── tournament_logic.rs # Tournament logic
│   │   └── urls.rs           # API URL construction
│   ├── cache/                # Response caching
│   │   ├── mod.rs
│   │   ├── core.rs           # Core cache functionality
│   │   ├── detailed_game_cache.rs
│   │   ├── goal_events_cache.rs
│   │   ├── http_response_cache.rs
│   │   ├── player_cache.rs
│   │   ├── tournament_cache.rs
│   │   └── types.rs          # Cache type definitions
│   ├── models/               # Data structures
│   │   ├── mod.rs
│   │   ├── common.rs         # Common model types
│   │   ├── detailed.rs       # Detailed game models
│   │   ├── goals.rs          # Goal event models
│   │   ├── players.rs        # Player models
│   │   └── schedule.rs       # Schedule models
│   ├── player_names/         # Player name handling
│   │   ├── mod.rs
│   │   ├── disambiguation.rs # Name disambiguation logic
│   │   └── formatting.rs     # Name formatting
│   └── processors/           # Data transformation
│       ├── mod.rs
│       ├── core.rs           # Core processing
│       ├── game_status.rs    # Game status processing
│       ├── goal_events.rs    # Goal event processing
│       ├── player_fetching.rs # Player data fetching
│       └── time_formatting.rs # Time formatting
├── teletext_ui/              # Teletext rendering (core logic)
│   ├── mod.rs
│   ├── compact_mode_rendering.rs
│   ├── content.rs            # Content generation
│   ├── core.rs               # Core rendering
│   ├── footer.rs             # Footer rendering
│   ├── formatting.rs         # Text formatting
│   ├── game_display.rs       # Game display logic
│   ├── indicators.rs         # Status indicators
│   ├── layout.rs             # Layout management
│   ├── mode_utils.rs         # Display mode utilities
│   ├── pagination.rs         # Page pagination
│   ├── rendering.rs          # Main rendering logic
│   ├── score_formatting.rs   # Score formatting
│   ├── season_utils.rs       # Season utilities
│   ├── utils.rs              # General utilities
│   ├── validation.rs         # Input validation
│   └── wide_mode.rs          # Wide display mode
├── ui/                       # UI components and interactive mode
│   ├── mod.rs
│   ├── components/           # Reusable UI components
│   │   ├── mod.rs
│   │   └── abbreviations.rs  # Team abbreviations
│   ├── interactive/          # Interactive UI loop
│   │   ├── mod.rs
│   │   ├── change_detection.rs
│   │   ├── core.rs
│   │   ├── event_handler.rs
│   │   ├── indicators.rs
│   │   ├── input_handler.rs
│   │   ├── navigation_manager.rs
│   │   ├── refresh_coordinator.rs
│   │   ├── refresh_manager.rs
│   │   ├── series_utils.rs
│   │   ├── state_manager.rs
│   │   └── terminal_manager.rs
│   └── teletext/             # Teletext UI types (re-exports)
│       ├── mod.rs
│       ├── colors.rs         # Color definitions
│       ├── compact_display.rs
│       ├── game_result.rs    # Game result types
│       ├── loading_indicator.rs
│       └── page_config.rs    # Page configuration
└── schemas/                  # JSON schemas
    ├── game_schedule_schema.json
    └── game_schema.json
```

### Module Relationships: teletext_ui/ vs ui/teletext/

The project has two teletext-related modules that serve different purposes:

**`src/teletext_ui/`** - Core Teletext Rendering Logic:
- Contains the main `TeletextPage` struct and rendering implementation
- Handles layout calculation, pagination, and text formatting
- Generates the actual teletext-style output with proper spacing and alignment
- Manages display modes (standard, compact, wide)
- This is where the visual teletext output is produced

**`src/ui/teletext/`** - UI Type Definitions and Re-exports:
- Contains type definitions like `GameResultData`, `ScoreType`, `TeletextPageConfig`
- Provides re-exports for backward compatibility with older code paths
- Acts as a bridge between the interactive UI (`ui/interactive/`) and teletext rendering
- Defines color schemes and display configuration types

**Why two modules?** The separation allows:
1. Core rendering logic to remain independent of interactive UI concerns
2. Type definitions to be shared across different UI components
3. Backward compatibility when refactoring the rendering pipeline

### Core Modules Overview

| Module | Purpose |
|--------|---------|
| `main.rs` | Application entry point, initializes logging and runs the app |
| `lib.rs` | Library exports for integration tests |
| `app.rs` | Main application logic, terminal setup, and run loop |
| `cli.rs` | CLI argument definitions using clap derive macros |
| `commands.rs` | Command handlers for config, version check, etc. |
| `logging.rs` | Tracing/logging setup with file and console output |
| `version.rs` | Version checking against GitHub releases |
| `error.rs` | Centralized error types using thiserror |
| `constants.rs` | Application-wide constants (URLs, timeouts, etc.) |
| `config/` | TOML configuration loading, validation, and platform paths |
| `data_fetcher/` | API client, caching, models, and data processing |
| `teletext_ui/` | Core teletext-style rendering and layout |
| `ui/` | Interactive mode, components, and type definitions |

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
