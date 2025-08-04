# Technology Stack

## Language & Edition
- **Rust 2024 edition** - Latest Rust features and improvements
- Requires all clippy warnings to be resolved before committing

## Build System
- **Cargo** - Standard Rust package manager and build tool
- Custom build script (`build.rs`) for version embedding

## Key Dependencies
- **tokio** - Async runtime with full feature set
- **reqwest** - HTTP client with JSON support and connection pooling
- **crossterm** - Cross-platform terminal manipulation
- **chrono** - Date and time handling
- **serde/serde_json** - Serialization/deserialization
- **clap** - Command-line argument parsing with derive features
- **tracing/tracing-subscriber** - Structured logging with file rotation
- **lru** - LRU cache implementation for performance optimization

## Development Dependencies
- **tempfile** - Temporary file handling for tests
- **wiremock** - HTTP mocking for API tests
- **tokio-test** - Async testing utilities
- **serial_test** - Sequential test execution when needed

## Common Commands

### Building
```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Build with all features
cargo build --all-features
```

### Testing
```bash
# Run all tests
cargo test --all-features

# Run integration tests specifically
cargo test --test integration_tests

# Run tests with output
cargo test --all-features -- --nocapture
```

### Code Quality
```bash
# Format code
cargo fmt

# Run clippy (must pass with no warnings)
cargo clippy --all-features --all-targets -- -D warnings

# Check without building
cargo check --all-features
```

### Running
```bash
# Development run
cargo run

# Release run
cargo run --release

# With specific arguments
cargo run -- --help
cargo run -- --date 2024-01-15
```

## Architecture Patterns
- **Modular design** with clear separation of concerns
- **Async/await** throughout for non-blocking operations
- **Error handling** with custom error types using `thiserror`
- **Configuration management** with platform-specific paths
- **Caching layer** with LRU cache and intelligent invalidation
- **Performance monitoring** with request deduplication and metrics