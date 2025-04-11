# Finnish Liiga Teletext Terminal App

A Rust terminal application that displays Finnish Liiga hockey results in a YLE Teksti-TV style interface.

<img width="503" alt="image" src="https://github.com/user-attachments/assets/e4c44281-039e-4152-b505-13cef5ec2889" />


## Features

- Teletext-style interface with colored headers and content
- Live game updates with automatic refresh
- Support for multiple tournaments (regular season, playoffs, playout, qualifications)
- Detailed game information including:
  - Game status (scheduled, ongoing, finished)
  - Score with overtime/shootout indicators
  - Goal scorers with timestamps
  - Video links for goals (can be disabled)
  - Pagination for multiple games
- Keyboard navigation
- Authentic YLE Teksti-TV appearance

## Installation

1. Make sure Rust and Cargo are installed on your system. If not, install them from [rustup.rs](https://rustup.rs/).

2. Clone this repository:

```bash
git clone https://github.com/nikosalonen/liiga_teletext.git
cd liiga_teletext
```

3. Create a config.toml file in the project root with your API configuration (example.config.toml is in the root):

```toml
api_domain = "YOUR_API_DOMAIN"
disable_video_links = false  # Optional: Set to true to disable video links
```

4. Build and run the application:

```bash
cargo build --release
cargo run --release

# To run without video links:
cargo run --release -- novideo
```

## Project Structure

```
liiga_teletext/
├── Cargo.toml
├── config.toml         # API configuration
├── src/
│   ├── main.rs         # Main application logic and event handling
│   ├── teletext_ui.rs  # UI components and rendering
│   ├── data_fetcher.rs # API integration and data processing
│   └── config.rs       # Configuration handling
```

## Usage

- Press `q` to quit the application
- Use left/right arrow keys to navigate between pages
- Data refreshes automatically:
  - Every minute for live games
  - Every hour for non-live games
- Launch with `novideo` parameter to disable video links (e.g., `cargo run --release -- novideo`)

## Configuration

The application requires a `config.toml` file with the following structure:

```toml
api_domain = "YOUR_API_DOMAIN"
disable_video_links = false  # Optional: Set to true to disable video links
```

You can also disable video links by:

1. Setting `disable_video_links = true` in config.toml
2. Running the application with the `novideo` parameter

## Features Status

- [x] Real API integration
- [x] Multiple pages of content with pagination
- [x] Live game updates
- [x] Goal scorer information
- [x] Support for multiple tournaments
- [x] Automatic refresh based on game state
- [x] Configurable video link display
- [ ] Display standings
- [ ] Display season statistics
- [ ] Configuration options for refresh intervals
- [ ] Custom color schemes

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
