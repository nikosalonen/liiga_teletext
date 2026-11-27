# Liiga 221 Teletext Terminal App

Finnish Liiga hockey results in your terminal, in the style of YLE Teksti-TV page 221.

<img width="2024" height="1594" alt="image" src="https://github.com/user-attachments/assets/828bcaf8-1ec7-4f90-990a-7f35b2c63756" />

## Features

- Scores, goal scorers with times, and video links, laid out like teletext page 221
- Goal videos open in a small [web player](https://nikosalonen.github.io/liiga_teletext/play.html), so they play in Firefox too
- Automatic refresh: every 15 seconds during live games, every 30 seconds around a game's start, every minute otherwise
- Regular season, playoffs, playout, qualifications and preseason games
- League standings, with a live mode that updates during games
- Playoff series scores and a full playoff bracket
- A season calendar that shows which days have games, to jump to any of them
- Teletext page numbers (221, 222, 223, 224), a colored Fastext-style footer and a header clock
- Standard, compact (multi-column) and wide (two-column) layouts
- Players who share a last name on the same team get a first initial, e.g. "Koivu M." and "Koivu S."

## Installation

You need a terminal with Unicode support.

### Prebuilt binaries

Download the binary for your platform from the
[latest release](https://github.com/nikosalonen/liiga_teletext/releases/latest).
Builds exist for macOS (Apple Silicon), Linux (x86_64 and arm64) and
Windows (x86_64). They need no Rust toolchain. On an Intel Mac, install
[with cargo](#with-cargo).

On macOS and Linux:

```bash
chmod +x liiga_teletext-*

# macOS only: browsers quarantine downloads, and Gatekeeper blocks the unsigned binary
xattr -d com.apple.quarantine liiga_teletext-*-apple-darwin

# Rename it and move it to a folder on your PATH. Use the file name you downloaded.
sudo mv liiga_teletext-aarch64-apple-darwin /usr/local/bin/liiga_teletext
```

On Windows, rename the file to `liiga_teletext.exe` and put it in a folder on
your `PATH`.

### With cargo

Needs Rust 1.93.1 or newer. Run `rustup update` to upgrade.

```bash
cargo install liiga_teletext

# Optional: also run it as 221, the YLE Teksti-TV page number
sudo ln -s ~/.cargo/bin/liiga_teletext /usr/local/bin/221
```

To build from source instead:

```bash
git clone https://github.com/nikosalonen/liiga_teletext.git
cd liiga_teletext
cargo run --release
```

### Updating

```bash
liiga_teletext --update
```

This downloads the prebuilt binary for your platform and checks its SHA-256
checksum before replacing the current one. Use `sudo` if the binary is in a
folder your user can't write to, such as `/usr/local/bin`.

If the binary is not published yet or GitHub can't be reached, a cargo install
runs `cargo install` for you instead. Other installs, and all Windows installs,
get a link to download the binary by hand. After a prebuilt update,
`cargo install --list` still shows the old version.

## Usage

```bash
liiga_teletext                     # interactive mode
liiga_teletext --once              # print the scores and exit
liiga_teletext --date 2025-03-21   # games on a specific date
liiga_teletext --compact           # multi-column layout
liiga_teletext --wide              # two columns, needs a terminal 128+ characters wide
```

### Keys

| Key | Action |
|---|---|
| `←` / `→` | Previous or next page |
| `Shift+←` / `Shift+→` | Previous or next date with games. Stays in the current season; use the calendar (`k`) or `--date` for older seasons |
| `s` | Standings. Press again for live mode, once more to go back |
| `l` | Toggle live mode in standings |
| `p` | Playoff bracket |
| `k` | Calendar. Press again, or `Esc`, to go back |
| `t` | Back to today |
| `r` | Refresh now, at most once every 15 seconds |
| `q` | Quit |
| `221`, `222`, `223`, `224` | Type a page number to jump to games, standings, the bracket or the calendar. Any other number shows "SIVUA EI LÖYDY" (page not found) |

In the calendar, days with games are white, days without are grey, and the
selected day's games are listed under the month:

| Key | Action |
|---|---|
| `←` / `→` | Previous or next day |
| `↑` / `↓` | Same day a week earlier or later |
| `Shift+←` / `Shift+→` | Previous or next month. Moving past the season loads the next one |
| `Enter` | Open the selected day's games |
| `t` | Back to today |

The bracket is available while the playoffs are upcoming, ongoing or finished
less than 14 days ago. On terminals of 80x24 or larger it shows every round side
by side with connectors, with upcoming rounds as placeholders.

### Options

| Option | Description |
|---|---|
| `-d, --date <DATE>` | Show games for a date (YYYY-MM-DD) |
| `-o, --once` | Show the scores once and exit. Useful in scripts |
| `-c, --compact` | Compact layout with team abbreviations, 1 to 3 columns depending on terminal width |
| `-w, --wide` | Two columns side by side, each with full game details. Needs a terminal 128+ characters wide |
| `-p, --plain` | No clickable video links |
| `--min-refresh-interval <SECONDS>` | Minimum time between refreshes. Higher values mean fewer API calls but slower updates |
| `--config [DOMAIN]` | Set the API domain. Prompts for it if left out |
| `--set-log-file <PATH>` | Save a custom log file path in the config |
| `--clear-log-file` | Remove the custom log file path and use the default |
| `--log-file <PATH>` | Use a log file for this run only |
| `--reset-cache` | Clear the cached player names |
| `-l, --list-config` | Show the current settings |
| `--debug` | Don't clear the terminal, and write logs to the log file |
| `-V, --version` | Show the version, platform and whether an update is available. Prints only `liiga_teletext <version>` when piped |
| `--update` | Update to the latest version (see [Updating](#updating)) |

## Configuration

On first run, the app asks for the API domain and saves it to `config.toml`:

- Linux: `~/.config/liiga_teletext/config.toml`
- macOS: `~/Library/Application Support/liiga_teletext/config.toml`
- Windows: `%APPDATA%\liiga_teletext\config.toml`

You can edit the file by hand. Logs go to `logs/liiga_teletext.log.YYYY-MM-DD`
next to it, one file per UTC day. A custom log path gets the same date suffix.

### Environment variables

These override the config file for one run. `--list-config` marks them with
"(from env)".

- `LIIGA_API_DOMAIN` - API domain. Skips the first-run prompt
- `LIIGA_LOG_FILE` - Log file path
- `LIIGA_HTTP_TIMEOUT` - HTTP timeout in seconds (default: 10)
- `LIIGA_BRACKET_GRACE_DAYS` - Days the playoff bracket stays visible after the last playoff game (default: 14). Set it to `400` to see last season's bracket in the off-season

### Timezone

Game times use your local timezone. On macOS and Linux, the app checks at
startup that it found your real timezone. If `TZ` forces UTC or the timezone
data can't be read, every start time would be off by your UTC offset, so the
app prints a warning. In interactive mode the warning appears after you quit.

## Tournaments

The app picks which tournaments to fetch from the time of year:

- Regular season (`runkosarja`): September to April
- Playoffs (`playoffs`), playout (`playout`) and qualifications (`qualifications`): March to June
- Preseason games (`valmistavat_ottelut`): May to September

In the off-season it shows a countdown to the next regular season.

## Development

```bash
cargo build --all-features
cargo test --all-features
cargo fmt
cargo clippy --all-features --all-targets -- -D warnings
```

Before you open a pull request, tests must pass, the code must be formatted and
clippy must show no warnings. New features need tests. Commit messages follow
Conventional Commits. See [CONTRIBUTING.md](CONTRIBUTING.md) for the full
workflow.

## License

MIT. See [LICENSE](LICENSE).

## Acknowledgements

- [YLE Teksti-TV](https://yle.fi/aihe/tekstitv?P=221) for the design inspiration
- [NHL-235](https://github.com/Hamatti/nhl-235) by Juha-Matti Santala for pioneering teletext hockey scores in the terminal. My original inspiration for this project.
