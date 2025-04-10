# Finnish Liiga Teletext Terminal App

A Rust terminal application that displays Finnish Liiga hockey results in a YLE Teksti-TV style interface.

## Features

- Teletext-style interface with colored headers and content
- Displays Finnish Liiga hockey results
- Keyboard navigation
- Authentic YLE Teksti-TV appearance

## Installation

1. Make sure Rust and Cargo are installed on your system. If not, install them from [rustup.rs](https://rustup.rs/).

2. Clone this repository or create a new project using the provided files:

```bash
cargo new liiga_teletext
cd liiga_teletext
```

3. Replace the default files with the files from this project.

4. Build and run the application:

```bash
cargo build --release
cargo run --release
```

## Project Structure

```
liiga_teletext/
├── Cargo.toml
├── src/
│   ├── main.rs         # Main application logic
│   ├── teletext_ui.rs  # UI components and rendering
│   └── data_fetcher.rs # Data fetching from API (optional)
```

## Usage

- Press `q` to quit the application
- Use arrow keys to navigate between pages (when implemented)

## Customization

### API Integration

To fetch real data instead of using mock data:

1. Find a suitable API for Finnish Liiga hockey results
2. Implement the `fetch_liiga_data()` function in `data_fetcher.rs`
3. Replace the mock data call in `main.rs` with your implemented function

### Display Customization

- Modify the colors in `teletext_ui.rs` to match your preferred teletext style
- Add additional content types to the `TeletextRow` enum
- Implement additional pages for different types of hockey statistics

## Future Improvements

- [ ] Add real API integration
- [ ] Implement multiple pages of content
- [ ] Add support for displaying standings
- [ ] Add support for past game results
- [ ] Implement caching for data
- [ ] Add configuration options