# Finnish Liiga Teletext Terminal App

A Rust terminal application that displays Finnish Liiga hockey results in a YLE Teksti-TV style interface.

![image](https://github.com/user-attachments/assets/f0e4003a-98e6-4ab8-8bb9-4adac18f5a46)

## Features

- Teletext-style interface with colored headers and content
- Live game updates with automatic refresh
- Support for multiple tournaments (regular season, playoffs, playout, qualifications)
- Command-line argument support using clap
- Detailed game information including:
  - Game status (scheduled, ongoing, finished)
  - Score with overtime/shootout indicators
  - Goal scorers with timestamps
  - Video links for goals (can be disabled)
  - Pagination for multiple games
- Keyboard navigation
- Authentic YLE Teksti-TV appearance

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

- Press `q` to quit the application
- Use left/right arrow keys to navigate between pages
- Data refreshes automatically:
  - Every minute for live games
  - Every hour for non-live games
- Use `-d` or `--date` to specify a date to show games for.
- Use `-o` or `--once` to show scores once and exit immediately.
- Use `-p` or `--plain` to disable clickable video links.
- Use `-c` or `--config` to update the API domain.
- Use `-l` or `--list-config` to list the current configuration.

## Configuration

On first run, you will be prompted to enter your API domain. This will be saved to a config file at:

- Linux: `~/.config/liiga_teletext/config.toml`
- macOS: `~/Library/Application Support/liiga_teletext/config.toml`
- Windows: `%APPDATA%\liiga_teletext\config.toml`

The configuration can be manually edited at any time by modifying this file. You can:

- Update the API domain

## Features Status

- [x] Real API integration
- [x] Multiple pages of content with pagination
- [x] Live game updates
- [x] Goal scorer information
- [x] Support for multiple tournaments
- [x] Automatic refresh based on game state
- [x] Configurable video link display
- [x] Command-line argument support

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. See [CONTRIBUTING.md](CONTRIBUTING.md) for more details.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgements

- [YLE Teksti-TV](https://www.yle.fi/tv/teksti-tv) for the design inspiration
- [NHL-235](https://github.com/Hamatti/nhl-235) by Juha-Matti Santala for pioneering the concept of bringing teletext hockey scores to the terminal. My original inspiration for this project.
