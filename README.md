# Liiga 221 Teletext Terminal App

A Rust terminal application that displays Finnish Liiga hockey results in a YLE Teksti-TV style interface.

<img width="886" height="658" alt="image" src="https://github.com/user-attachments/assets/4a7a3f7d-1766-4515-a61f-da96a29dcaeb" />

## Features

- **Authentic teletext interface** with YLE Teksti-TV channel 221 appearance
- **Real-time game updates** with automatic refresh (every minute for live games, hourly for completed games)
- **Comprehensive tournament support** including:
  - Regular season (`runkosarja`)
  - Playoffs (`playoffs`)
  - Playout (`playout`)
  - Qualifications (`qualifications`)
  - Practice games (`valmistavat_ottelut`) - May-September
- **Season countdown** - Shows days until regular season starts during off-season
- **Interactive navigation** with arrow key page navigation
- **Detailed game information** including:
  - Game status (scheduled, ongoing, finished)
  - Score with overtime/shootout indicators
  - Goal scorers with timestamps
  - Video links for goals (toggleable)
  - Pagination for multiple games
  - Future games display with date headers
- **Robust configuration system** with platform-specific storage
- **Comprehensive logging** with file rotation and configurable locations
- **Version checking** and update notifications
- **Debug mode** for development and troubleshooting
- **Non-interactive mode** for scripting and automation
- **Performance monitoring** with request deduplication and metrics collection
- **Advanced caching** with intelligent cache management and emergency cache clearing

## Installation

### Install from crates.io

```bash
cargo install liiga_teletext
```

You can create a symlink to the binary to make it available from anywhere:

```bash
sudo ln -s ~/.cargo/bin/liiga_teletext /usr/local/bin/221 # 221 is the channel number of YLE Teksti-TV
```

### Install from source

1. Make sure Rust and Cargo are installed on your system. If not, install them from [rustup.rs](https://rustup.rs/).

2. Clone this repository:

```bash
git clone https://github.com/nikosalonen/liiga_teletext.git
cd liiga_teletext
```

3. Build and run the application:

```bash
cargo build --release
cargo run --release
```

## Project Structure

```
liiga_teletext/
├── src/                    # Source code directory
│   ├── main.rs            # Main application logic and event handling
│   ├── teletext_ui.rs     # UI components and rendering
│   ├── config.rs          # Configuration handling
│   ├── error.rs           # Error handling and custom error types
│   ├── constants.rs       # Application constants and color definitions
│   ├── performance.rs     # Performance monitoring and request deduplication
│   ├── testing_utils.rs   # Test utilities and mock data builders
│   ├── data_fetcher.rs    # Data fetching module entry point
│   ├── data_fetcher/      # Data fetching related modules
│   │   ├── api.rs         # API integration and HTTP requests
│   │   ├── models.rs      # Data models and structures
│   │   ├── processors.rs  # Data processing and transformation
│   │   ├── cache.rs       # Caching functionality
│   │   └── player_names.rs # Player name resolution
│   ├── ui/                # UI-related modules
│   │   ├── mod.rs         # UI module entry point
│   │   └── interactive.rs # Interactive UI components
│   └── schemas/           # JSON schema definitions
│       ├── game_schema.json          # Game data structure schema
│       └── game_schedule_schema.json # Game schedule data structure schema
├── tests/                 # Integration tests
├── build.rs               # Build script for version embedding
└── docs/                  # Documentation files
```

## Usage

### Interactive Mode (Default)
- Press `q` to quit the application
- Use left/right arrow keys to navigate between pages
- Use **Shift+Left/Right** to navigate between dates with games
  - **Note**: Date navigation is limited to the current season for performance and UX reasons
  - To view games from previous seasons, use the `-d` flag with a specific date
- Press `r` to manually refresh data
- Data refreshes automatically:
  - Every minute for live games
  - Every hour for non-live games

### Command Line Options

#### Display Options
- `-d, --date <DATE>` - Show games for a specific date in YYYY-MM-DD format
- `-o, --once` - Show scores once and exit immediately (useful for scripts)
- `-p, --plain` - Disable clickable video links in the output

#### Configuration
- `-c, --config [DOMAIN]` - Update API domain in config (prompts if not provided)
- `--set-log-file <PATH>` - Set a persistent custom log file location
- `--clear-log-file` - Clear custom log file path and revert to default location
- `-l, --list-config` - List current configuration settings

#### Debug Options
- `--debug` - Enable debug mode (doesn't clear terminal, logs to file)
- `--log-file <PATH>` - Specify a custom log file path for this session

#### Info
- `-V, --version` - Show version information

## Configuration

On first run, you will be prompted to enter your API domain. This will be saved to a config file at:

- Linux: `~/.config/liiga_teletext/config.toml`
- macOS: `~/Library/Application Support/liiga_teletext/config.toml`
- Windows: `%APPDATA%\liiga_teletext\config.toml`

The configuration can be manually edited at any time by modifying this file. You can:

- Update the API domain
- Set a custom log file path

### Logging

The application includes comprehensive logging that can be configured:

- **Default location**: `~/.config/liiga_teletext/logs/liiga_teletext.log`
- **Custom location**: Can be set via `--set-log-file` or `--log-file`
- **Debug mode**: Logs are written to file instead of terminal display
- **Log rotation**: Logs are automatically rotated by date

## Tournament Support

The application intelligently handles different tournament types based on the season:

- **Regular Season** (`runkosarja`) - Main league games (September-April)
- **Playoffs** (`playoffs`) - Championship playoffs (March-June)
- **Playout** (`playout`) - Relegation playoffs (March-June)
- **Qualifications** (`qualifications`) - Qualification games (March-June)
- **Practice Games** (`valmistavat_ottelut`) - Preseason practice games (May-September)

During off-season periods, the app shows a countdown to the next regular season start.

## Performance Features

The application includes several performance optimizations:

- **HTTP Response Caching**: Intelligent caching with time-based expiration reduces API calls
- **Request Deduplication**: Prevents multiple identical API calls from running simultaneously
- **Performance Metrics**: Tracks API call counts, cache hit rates, and response times
- **Connection Pooling**: Reuses HTTP connections for better performance
- **Incremental Updates**: Only refreshes data when necessary based on game state
- **Memory Management**: Efficient data structures and resource cleanup
- **Async Architecture**: Non-blocking I/O operations for responsive UI
- **Emergency Cache Management**: Automatic cache clearing when memory usage is high

## Features Status

- [x] Real API integration with comprehensive error handling
- [x] Multiple pages of content with intelligent pagination
- [x] Live game updates with automatic refresh
- [x] Goal scorer information with player name resolution
- [x] Support for all tournament types including practice games
- [x] Automatic refresh based on game state
- [x] Configurable video link display
- [x] Command-line argument support with clap
- [x] Comprehensive logging system with rotation
- [x] Version checking and update notifications
- [x] Debug mode for development
- [x] Future games display with date headers
- [x] Configuration management with platform-specific paths
- [x] Season countdown during off-season
- [x] Historical game data support
- [x] HTTP response caching for performance
- [x] Request deduplication to prevent duplicate API calls
- [x] Performance metrics collection and monitoring
- [x] Emergency cache management for memory optimization
- [x] Robust error handling with user-friendly messages
- [x] Comprehensive test coverage with testing utilities
- [x] Advanced UI components with interactive features

## Development

### Building and Testing

```bash
# Build with all features
cargo build --all-features

# Run tests with all features
cargo test --all-features

# Run integration tests
cargo test --test integration_tests

# Check code quality
cargo fmt
cargo clippy --all-features --all-targets -- -D warnings
```

**Note**: This project uses Rust 2024 edition and requires all clippy warnings to be resolved before committing.

### Architecture

The application follows a modular architecture with clear separation of concerns:

- **CLI/Main** (`main.rs`) - Application entry point and interactive UI event loop
- **Data Fetcher** (`data_fetcher/`) - API integration, caching, and data processing
- **Teletext UI** (`teletext_ui.rs`) - Authentic YLE Teksti-TV style rendering
- **Configuration** (`config.rs`) - Platform-specific config management
- **Error Handling** (`error.rs`) - Centralized error types with context
- **Performance** (`performance.rs`) - Request deduplication and metrics collection
- **Testing Utils** (`testing_utils.rs`) - Mock data builders and test utilities

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines including commit message format and development workflow.

**Important**: Before submitting any changes, please ensure:
- All tests pass: `cargo test --all-features`
- Code is formatted: `cargo fmt`
- No clippy warnings: `cargo clippy --all-features --all-targets -- -D warnings`
- New functionality includes appropriate tests
- Follow conventional commits for commit messages

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgements

- [YLE Teksti-TV](https://yle.fi/aihe/tekstitv?P=221) for the design inspiration
- [NHL-235](https://github.com/Hamatti/nhl-235) by Juha-Matti Santala for pioneering the concept of bringing teletext hockey scores to the terminal. My original inspiration for this project.
