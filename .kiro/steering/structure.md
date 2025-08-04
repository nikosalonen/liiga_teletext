# Project Structure

## Root Directory
- `Cargo.toml` - Package manifest with dependencies and metadata
- `Cargo.lock` - Dependency lock file (committed to repo)
- `build.rs` - Build script for version embedding
- `config.toml` - Application configuration template
- `README.md` - Main project documentation
- `CONTRIBUTING.md` - Development and contribution guidelines

## Source Code Organization (`src/`)

### Core Application Files
- `main.rs` - Application entry point, CLI handling, and interactive UI event loop
- `lib.rs` - Library interface with public API exports
- `config.rs` - Configuration management with platform-specific paths
- `error.rs` - Centralized error types using `thiserror`
- `constants.rs` - Application constants and color definitions

### Data Layer (`src/data_fetcher/`)
- `mod.rs` - Module entry point and re-exports
- `api.rs` - HTTP API integration and external data fetching
- `models.rs` - Data structures and response models
- `processors.rs` - Data transformation and business logic
- `cache.rs` - Caching functionality with LRU implementation
- `player_names.rs` - Player name resolution and mapping

### UI Layer (`src/ui/`)
- `mod.rs` - UI module entry point
- `interactive.rs` - Interactive terminal UI components
- `teletext_ui.rs` - Teletext-style rendering and display logic (root level)

### Supporting Modules
- `performance.rs` - Request deduplication and performance monitoring
- `testing_utils.rs` - Test utilities and mock data builders

### Data Schemas (`src/schemas/`)
- `game_schema.json` - JSON schema for game data structure
- `game_schedule_schema.json` - JSON schema for schedule data structure

## Testing (`tests/`)
- `integration_tests.rs` - End-to-end integration tests

## Configuration & Build
- `.kiro/` - Kiro IDE configuration and steering rules
- `.github/` - GitHub Actions and repository configuration
- `target/` - Cargo build artifacts (gitignored)
- `scripts/` - Build and deployment scripts

## Architecture Principles

### Module Boundaries
- **Clear separation** between data fetching, UI rendering, and application logic
- **Single responsibility** - each module has a focused purpose
- **Dependency direction** - UI depends on data layer, not vice versa

### File Naming Conventions
- Use `snake_case` for all Rust files
- Module files use `mod.rs` for directory modules
- Test files end with `_tests.rs` or use `tests/` directory

### Code Organization
- **Public APIs** are re-exported through `lib.rs`
- **Internal modules** use `pub(crate)` for cross-module access
- **Error handling** is centralized in `error.rs`
- **Constants** are grouped in `constants.rs`

### Data Flow
1. **CLI/Main** → handles user input and coordinates application flow
2. **Data Fetcher** → fetches, processes, and caches external data
3. **Teletext UI** → renders data in authentic teletext format
4. **Config** → manages persistent application settings