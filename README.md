# Liiga 221 Teletext Terminal App

A Rust terminal application that displays Finnish Liiga hockey results in authentic YLE Teksti-TV style.

<img width="889" height="562" alt="image" src="https://github.com/user-attachments/assets/968d1926-3485-4bb9-bbd6-bbffa15c23a1" />

## Features

- **Authentic teletext interface** - YLE Teksti-TV channel 221 appearance
- **Real-time updates** - Automatic refresh (every minute for live games, hourly for completed)
- **Tournament support** - Regular season, playoffs, playout, qualifications, practice games
- **Interactive navigation** - Arrow keys for page navigation, automatic date navigation
- **Detailed game info** - Scores, goal scorers with timestamps, video links
- **Multiple display modes** - Compact (multi-column), wide (side-by-side), standard
- **Configuration system** - Platform-specific storage, customizable settings
- **Performance optimized** - Caching, request deduplication, intelligent refresh intervals

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

### Display Modes

#### Compact Mode (`--compact`)
Space-efficient multi-column layout with team abbreviations. Adapts from 1-3 columns based on terminal width.

```bash
liiga_teletext --compact --once  # Quick compact view
```

#### Wide Mode (`--wide`)
Side-by-side two-column layout for wide terminals (128+ characters). Shows full game details in each column.

```bash
liiga_teletext --wide --date 2025-03-21  # Wide view for specific date
```

## Player Name Disambiguation

When multiple players share the same last name on a team, the app automatically adds first initials for clarity (e.g., "Koivu M.", "Koivu S."). This works per-team, so players with the same last name on different teams remain unchanged.

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

- **Smart caching** - HTTP response caching with time-based expiration
- **Request deduplication** - Prevents simultaneous identical API calls
- **Adaptive refresh** - Intelligent intervals based on game state and count
- **Connection pooling** - Efficient HTTP connection reuse
- **Async architecture** - Non-blocking operations for responsive UI

## Development

```bash
# Build and test
cargo build --all-features
cargo test --all-features

# Code quality checks
cargo fmt
cargo clippy --all-features --all-targets -- -D warnings
```

Uses Rust 2024 edition with modular architecture: CLI/main, data fetcher, teletext UI, config, and performance modules.

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
