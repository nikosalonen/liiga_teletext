# Liiga 221 Teletext Terminal App

A Rust terminal application that displays Finnish Liiga hockey results in a YLE Teksti-TV style interface.

<img width="889" height="562" alt="image" src="https://github.com/user-attachments/assets/968d1926-3485-4bb9-bbd6-bbffa15c23a1" />


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
- **Compact mode** with multi-column layout for space-efficient display
- **Wide mode** with two-column side-by-side layout for wide terminals (128+ characters)
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
- `-c, --compact` - Enable compact mode with space-efficient multi-column layout
- `-w, --wide` - Enable wide mode with two-column side-by-side layout (requires 128+ character wide terminal)
- `--min-refresh-interval <SECONDS>` - Set minimum refresh interval in seconds (default: auto-detect based on game count). Higher values reduce API calls but may miss updates. Use with caution.

#### Configuration
- `--config [DOMAIN]` - Update API domain in config (prompts if not provided)
  - **Breaking Change**: The `-c` short flag has been removed to avoid conflict with `--compact`. Use the full `--config` flag instead.
- `--set-log-file <PATH>` - Set a persistent custom log file location
- `--clear-log-file` - Clear custom log file path and revert to default location
- `-l, --list-config` - List current configuration settings

#### Debug Options
- `--debug` - Enable debug mode (doesn't clear terminal, logs to file)
- `--log-file <PATH>` - Specify a custom log file path for this session

#### Info
- `-V, --version` - Show version information

### Compact Mode

The application features a compact display mode that provides a space-efficient layout for viewing multiple games:

#### Features
- **Multi-column layout**: Displays up to 3 columns of games when terminal width allows
- **Adaptive design**: Automatically falls back to 2 or 1 columns for narrower terminals
- **Team abbreviations**: Uses 3-character team abbreviations to save space
- **Clean formatting**: Maintains authentic teletext colors and styling
- **Visual spacing**: Adds empty rows between game groups for improved readability

#### Usage
```bash
# Enable compact mode
liiga_teletext --compact

# Combine with other options
liiga_teletext --compact --once        # Single compact view
liiga_teletext --compact --date 2024-01-15  # Compact view for specific date
```

#### Layout Examples
```text
# Wide terminal (3 columns):
KalPa 2-1 HIFK    Tappara 3-2 JYP    Blues 1-0 Lukko

# Medium terminal (2 columns):
KalPa 2-1 HIFK    Tappara 3-2 JYP
Blues 1-0 Lukko

# Narrow terminal (1 column):
KalPa 2-1 HIFK
Tappara 3-2 JYP
Blues 1-0 Lukko
```

#### Requirements
- **Minimum terminal width**: 18 characters for basic compact mode
- **Optimal width**: 60+ characters for multi-column layout
- **Terminal compatibility**: Works with all terminal types that support ANSI colors

### Wide Mode

The application features a wide display mode that provides a side-by-side two-column layout for viewing games on wide terminals:

#### Features
- **Two-column layout**: Displays games in two columns side by side, maximizing wide terminal usage
- **Full game details**: Each column shows complete game information including goal scorers, timestamps, and video links
- **Authentic teletext layout**: Each column maintains the full 60-character teletext format for authentic appearance
- **Intelligent distribution**: Games are distributed evenly between columns (left column gets extra if odd number)
- **Goal scorer positioning**: Home team scorers appear under home team, away team scorers under away team
- **Automatic fallback**: Falls back gracefully to normal single-column mode on narrow terminals
- **Header/footer spanning**: Page headers and footers span the full terminal width

#### Usage
```bash
# Enable wide mode
liiga_teletext --wide

# Combine with other options
liiga_teletext --wide --once        # Single wide view
liiga_teletext --wide --date 2024-01-15  # Wide view for specific date
liiga_teletext --wide --plain       # Wide mode without video links
```

#### Layout Example
```text
# Wide terminal (128+ characters):
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ Liiga 221 - Finnish Hockey League Results                                                    Page 1/2         │
├─────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                                                 │
│ HIFK      3-2  KalPa                    Tappara   2-1  JYP                                                     │
│ ■ 18:30 Loppu (jatkoaika)               ■ 18:30 Loppu                                                          │
│ Maalit:                                  Maalit:                                                                │
│   1-0 05:23 Virtanen (YV)                 1-0 12:15 Hakala                                                     │
│   1-1 12:45    Korhonen                   1-1 25:30    Nieminen                                                │
│   2-1 33:12 Laine                         2-1 45:20 Saarinen (YV)                                             │
│   2-2 58:01    Koskinen                                                                                         │
│   3-2 62:30 Virtanen (RL)                                                                                      │
│                                                                                                                 │
│ Blues     1-0  Lukko                     Ässät    4-3  Pelicans                                                │
│ ■ 18:30 Loppu                           ■ 18:30 Loppu (jatkoaika)                                             │
│ Maalit:                                  Maalit:                                                                │
│   1-0 25:15 Mattila                       1-0 08:30 Lehtonen                                                   │
│                                           1-1 15:20    Koivisto                                                │
│                                           2-1 28:45 Rantala                                                    │
│                                           2-2 35:15    Heikkinen                                               │
│                                           3-2 42:20 Salminen                                                   │
│                                           3-3 56:40    Latvala                                                 │
│                                           4-3 63:15 Lehtonen (RL)                                              │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Requirements
- **Minimum terminal width**: 128 characters for wide mode activation
- **Column width**: Each column uses 60 characters for full teletext layout
- **Column separation**: 8 characters of spacing between columns
- **Terminal compatibility**: Works with all terminal types that support ANSI colors and cursor positioning
- **Mutual exclusivity**: Cannot be used together with compact mode (`-c`)

## Player Name Disambiguation

The app supports authentic hockey-style player name disambiguation within each team. When multiple players share the same last name on the same team, names are shown as "Last F." (e.g., "Koivu M.", "Koivu S."). Players on different teams do not affect each other.

### How it works
- Team-scoped grouping by last name (case-insensitive)
- First initial derived from the first alphabetic character of the first name (Unicode-aware, e.g., Ä/Ö/Å)
- Graceful fallback to last name only when the initial cannot be determined

### Quick examples (library usage)

```rust
use liiga_teletext::data_fetcher::player_names::{
    format_with_disambiguation,
    format_for_display_with_first_initial,
    DisambiguationContext,
};

// Format a single player with first initial
let shown = format_for_display_with_first_initial("Mikko", "Koivu");
assert_eq!(shown, "Koivu M.");

// Team-scoped disambiguation for a single team
let team_players = vec![
    (1_i64, "Mikko".to_string(), "Koivu".to_string()),
    (2, "Saku".to_string(), "Koivu".to_string()),
    (3, "Teemu".to_string(), "Selänne".to_string()),
];
let disambiguated = format_with_disambiguation(&team_players);
assert_eq!(disambiguated.get(&1), Some(&"Koivu M.".to_string()));
assert_eq!(disambiguated.get(&2), Some(&"Koivu S.".to_string()));
assert_eq!(disambiguated.get(&3), Some(&"Selänne".to_string()));

// Context helper when processing a team's goal events
let context = DisambiguationContext::new(team_players);
assert_eq!(context.needs_disambiguation("Koivu"), true);
```

### End-to-end goal processing

Use team-scoped disambiguation in goal event processing so each side is handled independently:

```rust
use liiga_teletext::data_fetcher::processors::process_goal_events_with_disambiguation;
use liiga_teletext::data_fetcher::models::ScheduleGame;

let game = ScheduleGame::default();
let home_players = vec![(1, "Mikko".to_string(), "Koivu".to_string())];
let away_players = vec![(2, "Saku".to_string(), "Koivu".to_string())];

let events = process_goal_events_with_disambiguation(&game, &home_players, &away_players);
// Home Koivu -> "Koivu" (no conflict on home team)
// Away Koivu -> "Koivu" (no conflict on away team)
```

### Caching disambiguated names

Cache disambiguated names per game for fast lookups during rendering:

```rust
use std::collections::HashMap;
use liiga_teletext::data_fetcher::cache::cache_players_with_disambiguation;

let mut home = HashMap::new();
home.insert(101, ("Mikko".to_string(), "Koivu".to_string()));
home.insert(102, ("Saku".to_string(), "Koivu".to_string()));

let mut away = HashMap::new();
away.insert(201, ("Teemu".to_string(), "Selänne".to_string()));

tokio::spawn(async move {
    cache_players_with_disambiguation(12345, home, away).await;
});
```

### Best practices and notes
- Always apply disambiguation per team; never mix home/away when grouping by last name
- Do not hold locks across async awaits; prefer `tokio::sync::Mutex` if synchronization is needed
- Use cached names when available to avoid recomputing during rendering
- Unicode is supported end-to-end; initials are derived from the first alphabetic char
- If a first name starts with a non-alphabetic character or is missing, show last name only

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
- **Rate Limiting Protection**: Adaptive refresh intervals based on game count with exponential backoff
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
- [x] Rate limiting protection with exponential backoff
- [x] Adaptive refresh intervals based on game count
- [x] Configurable minimum refresh intervals
- [x] Robust error handling with user-friendly messages
- [x] Comprehensive test coverage with testing utilities
- [x] Advanced UI components with interactive features
- [x] Compact mode with multi-column layout and team abbreviations
- [x] Wide mode with two-column side-by-side layout for wide terminals (128+ characters)

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
