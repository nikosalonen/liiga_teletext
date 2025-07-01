# Finnish Liiga Teletext Terminal App

A Rust terminal application that displays Finnish Liiga hockey results in a YLE Teksti-TV style interface.

![image](https://github.com/user-attachments/assets/f0e4003a-98e6-4ab8-8bb9-4adac18f5a46)

## Features

- Teletext-style interface with colored headers and content
- Live game updates with automatic refresh
- Support for multiple tournaments (regular season, playoffs, playout, qualifications, practice games)
- Command-line argument support using clap
- Comprehensive logging system with configurable log file locations
- Version checking and update notifications
- Detailed game information including:
  - Game status (scheduled, ongoing, finished)
  - Score with overtime/shootout indicators
  - Goal scorers with timestamps
  - Video links for goals (can be disabled)
  - Pagination for multiple games
  - Future games display with date headers
- Keyboard navigation
- Authentic YLE Teksti-TV appearance
- Debug mode for development and troubleshooting

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

└── src/               # Source code directory
    ├── main.rs        # Main application logic and event handling
    ├── teletext_ui.rs # UI components and rendering
    ├── data_fetcher/  # Data fetching related modules
    ├── data_fetcher.rs# API integration and data processing
    ├── config.rs      # Configuration handling
    └── schemas/       # JSON schema definitions
        ├── game_schema.json         # Game data structure schema
        └── game_schedule_schema.json# Game schedule data structure schema
```

## Usage

### Interactive Mode (Default)
- Press `q` to quit the application
- Use left/right arrow keys to navigate between pages
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

The application supports all major Liiga tournaments:

- **Regular Season** (`runkosarja`) - Main league games
- **Playoffs** (`playoffs`) - Championship playoffs
- **Playout** (`playout`) - Relegation playoffs
- **Qualifications** (`qualifications`) - Qualification games
- **Practice Games** (`valmistavat_ottelut`) - Preseason practice games (May-September)

## Features Status

- [x] Real API integration
- [x] Multiple pages of content with pagination
- [x] Live game updates
- [x] Goal scorer information
- [x] Support for multiple tournaments including practice games
- [x] Automatic refresh based on game state
- [x] Configurable video link display
- [x] Command-line argument support
- [x] Comprehensive logging system
- [x] Version checking and update notifications
- [x] Debug mode for development
- [x] Future games display with date headers
- [x] Configuration management

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. See [CONTRIBUTING.md](CONTRIBUTING.md) for more details.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgements

- [YLE Teksti-TV](https://www.yle.fi/tv/teksti-tv) for the design inspiration
- [NHL-235](https://github.com/Hamatti/nhl-235) by Juha-Matti Santala for pioneering the concept of bringing teletext hockey scores to the terminal. My original inspiration for this project.
