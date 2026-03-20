# Playoff Bracket Visualization — Design Spec

## Context

The Liiga teletext app currently shows game results and standings but has no dedicated view for playoff bracket progression. During playoffs, users want to see the full tournament tree — which teams are in each round, series scores, and who advances. This feature adds a new interactive page (page 224) accessible via the `p` key that renders the playoff bracket as an ASCII tree using box-drawing characters.

## Requirements

- **Key binding**: `p` toggles between the bracket view and the previous view
- **Layout**: Tree bracket with box-drawing characters showing matchup progression from quarterfinals through the final
- **Data**: Series score per matchup (team names + wins), no individual game scores
- **Phases**: Include all phases present in the data: 1. KIERROS, PUOLIVÄLIERÄT, VÄLIERÄT, FINAALI, PRONSSIOTTELU
- **Off-season**: When no playoff data exists, show "PUDOTUSPELIT EIVÄT OLE KÄYNNISSÄ" message
- **Refresh**: Auto-refresh at 60s during live playoff games, 1 hour otherwise
- **Page number**: 224
- **Date navigation**: Disabled in bracket view (bracket is season-wide, not date-specific)

## Data Model

### New: `src/data_fetcher/models/bracket.rs`

```rust
/// A single playoff series between two teams
#[derive(Debug, Clone, Hash)]
pub struct BracketMatchup {
    pub phase: i32,               // 1-5 (maps to playoff_phase_name())
    pub pair: i32,                // matchup number within phase
    pub serie: i32,               // tournament ID (prevents mixing playoffs/playout)
    pub team1: String,            // first team (typically higher seed)
    pub team2: String,            // second team
    pub team1_wins: u8,
    pub team2_wins: u8,
    pub req_wins: u8,             // 4 for BO7, 1 for bronze
    pub is_decided: bool,         // one team reached req_wins
    pub has_live_game: bool,      // any game in series is started && !ended
    pub winner: Option<String>,   // team name of winner, if decided
}

/// All matchups in a single playoff round
#[derive(Debug, Clone, Hash)]
pub struct BracketPhase {
    pub phase_number: i32,
    pub name: String,             // Finnish name from playoff_phase_name()
    pub matchups: Vec<BracketMatchup>,
}

/// The complete playoff bracket for a season
#[derive(Debug, Clone, Hash)]
pub struct PlayoffBracket {
    pub season: String,           // e.g., "2025-2026"
    pub phases: Vec<BracketPhase>,// ordered by phase_number
    pub has_data: bool,           // false during regular season
}
```

### Bracket Construction

New function `build_playoff_bracket(schedule_games: &[ScheduleApiGame], season: &str) -> PlayoffBracket`:

1. Filter to games where `play_off_phase.is_some()`
2. If no playoff games found, return `PlayoffBracket { has_data: false, .. }`
3. Group by `(serie, phase, pair)` — each group is one series. Using the `serie` integer ID prevents mixing playoff and playout games that share the same phase/pair numbers (see `playoff_series.rs` test `test_different_tournaments_same_phase_pair_not_mixed`)
4. For each series:
   - Identify the two teams from `home_team_name`/`away_team_name` across all games in the group
   - Count wins per team from games where `ended == true`
   - `req_wins` comes from `play_off_req_wins` on `ScheduleApiGame`, defaulting to 4 when absent (matching `playoff_series.rs` line 82)
   - Team order: the team appearing as `home_team_name` in the game with the earliest `start` is `team1` (higher seed gets home ice in game 1)
   - Determine `is_decided`: either team's win count >= `req_wins`
   - Set `winner` if decided
   - `has_live_game`: any game in the group has `started && !ended`
5. Group matchups into `BracketPhase` structs, ordered by phase number
6. Phase names from existing `playoff_phase_name()` in `series_utils.rs`

### Edge Cases

- **0-0 series** (no games played yet): Both wins show as 0, team names still displayed
- **Season determination**: Use existing season calculation logic from `season_utils.rs` (Jan-Aug = current year, Sep-Dec = next year)
- **Only playoffs** tournament ID filtered: exclude playout, qualifications, etc. by checking `serie` against known playoff tournament IDs (the same way `calculate_series_scores` already separates them)

## API Integration

### New: `src/data_fetcher/api/bracket_api.rs`

```rust
pub async fn fetch_playoff_bracket(
    client: &Client,
    config: &Config,
    season: i32,
) -> Result<PlayoffBracket, AppError>
```

- Calls `fetch_tournament_games(client, config, &[TournamentType::Playoffs], season)` — reuses existing function from `tournament_logic.rs`
- Passes result to `build_playoff_bracket()`
- Season string formatted as `"{prev_year}-{season_year}"`

### Caching

- Bracket data cached with key `"bracket-{season}"`
- TTL: 60s during live playoff games (any game has `started && !ended`), 1 hour otherwise
- Uses same cache infrastructure as standings

## Tree Bracket Rendering

### New: `src/teletext_ui/bracket_display.rs`

Renders the bracket as an ASCII tree. The layout shows two halves of the bracket vertically separated, meeting at the final:

```
  TEKSTI-TV        JÄÄKIEKKO         s.224
  ────────────────────────────────────────
  PUDOTUSPELIT 2025-2026

  PUOLIVÄLIERÄT    VÄLIERÄT      FINAALI

  HIFK   4 ─┐
             ├── HIFK  2 ─┐
  TPS    3 ─┘              │
                            ├── ???
  Pelicans 3─┐              │
             ├── Peli. 1 ─┘
  KalPa  1 ─┘

  Lukko  4 ─┐
             ├── Lukko 1 ─┐
  KooKoo 2 ─┘              │
                            ├── ???
  Tappara 4─┐              │
             ├── Tapp. 0 ─┘
  Kärpät  2─┘

  PRONSSIOTTELU
  ??? vs ???
```

**With 1. KIERROS present** (some seasons), it extends to the left:

```
  1. KIERROS   PUOLIV.     VÄLIERÄT    FINAALI

  Team1  3 ─┐
             ├── Win1 ─┐
  Team2  1 ─┘          ├── Win? 2 ─┐
  Team3  4 ─┐          │            │
             ├── T3   ─┘            ├── ...
  Team4  2 ─┘                      │
  ...
```

### Terminal Width and Team Name Truncation

- **Minimum width**: 60 columns for 3-phase bracket, 75 columns for 4-phase bracket
- **Below minimum**: Fall back to stacked phase list (no tree, just phases listed top to bottom)
- **Team name truncation**: Max 7 characters for 4-phase brackets (e.g., "Pelican" → "Peli."), max 9 for 3-phase brackets. Truncation adds trailing "." when shortened.
- **Bracket forces normal mode**: compact_mode and wide_mode are ignored (same as standings, see `create_standings_page` line 501)

### Rendering Algorithm

1. Determine max phases present (typically 3: QF, SF, F)
2. Calculate column widths based on terminal width and number of phases
3. For each half of the bracket:
   a. Determine QF matchups — derive bracket halves dynamically by matching SF team names to QF winners (robust to any pair numbering scheme)
   b. Render QF matchups (3 rows each: team1, connector ├──, team2)
   c. Render SF matchup at vertical midpoint of its two feeding QFs
   d. Render connecting lines (─, ┐, ┘, ├, ┤)
4. Render final at midpoint between the two SF results
5. Render bronze match separately at the bottom

### Color Scheme

- Phase headers (PUOLIVÄLIERÄT, VÄLIERÄT, etc.): cyan
- Team names: white (cyan if series has a live game in progress)
- Win counts: yellow
- Series-clinching win count (the winning number): green
- Box-drawing characters: white
- "MESTARI" / champion label: green, bold
- "???" placeholders: dark gray/dim

### TeletextRow Variant

Add `TeletextRow::BracketLine(String)` — a single pre-formatted line containing the bracket content. This integrates with the existing `TeletextPage` row system and pagination (the page slices `content_rows` by index). The bracket renderer in `bracket_display.rs` produces `Vec<TeletextRow::BracketLine>` rows.

### Pagination

The bracket should fit on a single page for standard terminal heights (24+ rows). If the terminal is very short, the existing `TeletextPage` pagination system handles overflow — left/right arrows paginate within the bracket view.

## Interactive UI Integration

### ViewMode (state_manager.rs)

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    Games,
    Standings { live_mode: bool },
    Bracket,  // NEW
}
```

`Bracket` is a unit variant, so `Copy` derive remains valid.

### Exhaustive Match Updates

Every existing `match` on `ViewMode` must gain a `Bracket` arm. Specific locations:

| File | Location | Required behavior for `Bracket` |
|------|----------|------|
| `input_handler.rs` ~line 315 | `let is_standings = matches!(...)` | Change to: `let is_non_game_view = matches!(params.current_view, ViewMode::Standings { .. } \| ViewMode::Bracket)` — disables date navigation for bracket too |
| `input_handler.rs` ~line 451 | `'s'` key match (Games/Standings toggle) | Add `ViewMode::Bracket => {}` arm — pressing `s` from bracket does nothing (user should press `p` to leave) |
| `input_handler.rs` ~line 470 | `'l'` key match (live mode toggle) | Already uses `if let ViewMode::Standings` — no change needed, silently ignores Bracket |
| `event_handler.rs` ~line 157 | Standings hash reset on view change | Add: also reset bracket hash when switching away from bracket |
| `refresh_coordinator.rs` ~line 458 | `if let ViewMode::Standings` branch | Add `ViewMode::Bracket` branch BEFORE the standings check, routing to `perform_bracket_refresh()` |
| `state_manager.rs` tests | `toggle_view()` helper | Add Bracket arm that returns Games |

### NavigationState Additions

```rust
// In NavigationState:
pub preserved_bracket_return_view: Option<ViewMode>,  // plain Copy type, no Box needed
```

### ChangeDetectionState Additions

```rust
// In ChangeDetectionState:
last_bracket_hash: Option<u64>,

// Methods:
pub fn update_bracket_hash(&mut self, new_hash: u64) -> bool  // returns true if changed
pub fn reset_bracket_hash(&mut self)  // called when switching away from bracket view
```

### KeyEventParams Addition

Add `preserved_bracket_return_view: &'a mut Option<ViewMode>` to `KeyEventParams` struct. Sync-back in `event_handler.rs` after input handling (same pattern as `preserved_games_page` and `preserved_live_mode`).

### Key Binding (input_handler.rs)

```rust
KeyCode::Char('p') => {
    match *params.current_view {
        ViewMode::Bracket => {
            // Return to previous view (default: Games)
            *params.current_view = params.preserved_bracket_return_view
                .take()
                .unwrap_or(ViewMode::Games);
        }
        other => {
            // Save current view and switch to bracket
            *params.preserved_bracket_return_view = Some(other);
            *params.current_view = ViewMode::Bracket;
        }
    }
    *params.needs_refresh = true;
}
```

### Refresh Coordinator (refresh_coordinator.rs)

New `perform_bracket_refresh()`:
1. Check if `ViewMode::Bracket`
2. Fetch playoff bracket via `fetch_playoff_bracket()`
3. If `!bracket.has_data`: render "not underway" message page
4. If bracket has data: render tree bracket page
5. Hash bracket for change detection via `update_bracket_hash()`
6. Auto-refresh interval: 60s if any matchup has `has_live_game`, 1 hour otherwise

### Page Creation (navigation_manager.rs)

`create_bracket_page(bracket: &PlayoffBracket) -> TeletextPage`:
- Page number: 224
- Title: "JÄÄKIEKKO"
- Subheader: "PUDOTUSPELIT {season}"
- Forces normal mode (no compact/wide)
- Content: `Vec<TeletextRow::BracketLine>` from `bracket_display.rs`

## Files Changed

### New files
| File | Purpose |
|------|---------|
| `src/data_fetcher/models/bracket.rs` | `PlayoffBracket`, `BracketPhase`, `BracketMatchup` structs + `build_playoff_bracket()` |
| `src/data_fetcher/api/bracket_api.rs` | `fetch_playoff_bracket()` — fetches and builds bracket |
| `src/teletext_ui/bracket_display.rs` | Tree rendering with box-drawing chars and color |

### Modified files
| File | Change |
|------|--------|
| `src/data_fetcher/models/mod.rs` | Add `pub mod bracket;` export |
| `src/data_fetcher/api/mod.rs` | Add `pub mod bracket_api;` export |
| `src/ui/interactive/state_manager.rs` | `ViewMode::Bracket`, `preserved_bracket_return_view` in `NavigationState`, `last_bracket_hash` in `ChangeDetectionState` |
| `src/ui/interactive/input_handler.rs` | `KeyCode::Char('p')` handler, `Bracket` arms in existing matches, disable date nav for bracket |
| `src/ui/interactive/event_handler.rs` | Bracket hash reset on view change, sync-back for `preserved_bracket_return_view` |
| `src/ui/interactive/refresh_coordinator.rs` | `perform_bracket_refresh()`, bracket change detection, ViewMode::Bracket branch |
| `src/ui/interactive/navigation_manager.rs` | `create_bracket_page()` |
| `src/teletext_ui/core.rs` | `TeletextRow::BracketLine(String)` variant |
| `src/teletext_ui/mod.rs` | Export `bracket_display` module |
| `src/teletext_ui/game_display.rs` | Render `BracketLine` rows in content renderer |

### Reused existing code
| Code | Location | Purpose |
|------|----------|---------|
| `fetch_tournament_games()` | `src/data_fetcher/api/tournament_logic.rs:107` | Fetch all playoff schedule games |
| `build_tournament_schedule_url()` | `src/data_fetcher/api/urls.rs:86` | Build `/schedule?tournament=playoffs` URL |
| `playoff_phase_name()` | `src/ui/interactive/series_utils.rs:66` | Finnish names for playoff phases |
| `TournamentType::Playoffs` | `src/data_fetcher/api/tournament_logic.rs` | Tournament type constant |
| Change detection hash pattern | `src/ui/interactive/change_detection.rs` | Hash-based change detection |
| `render_buffered()` | `src/teletext_ui/core.rs:449` | Buffered terminal output |
| Season calculation | `src/data_fetcher/api/season_utils.rs` | Determine current season year |

## Verification

1. `cargo test --all-features` — all existing tests pass
2. `cargo clippy --all-features --all-targets -- -D warnings` — zero warnings
3. Unit tests for `build_playoff_bracket()`:
   - Empty schedule → `has_data: false`
   - Single phase (QF only) → correct matchups and scores
   - Full bracket (QF + SF + F) → correct bracket structure
   - Bronze game handled separately
   - 1. KIERROS included when present
   - Decided series show correct winner
   - 0-0 series (no completed games yet) → both wins = 0, teams displayed
   - Playoff and playout with same phase/pair numbers → not mixed (grouped by `serie`)
   - `req_wins` defaults to 4 when `play_off_req_wins` is None
4. Unit tests for bracket rendering:
   - Correct box-drawing character placement
   - Team names truncated appropriately for width
   - Color codes applied correctly
   - Narrow terminal falls back to stacked layout
5. Unit tests for ViewMode::Bracket integration:
   - `'p'` key toggles bracket on/off
   - `'s'` key ignored while in bracket view
   - Date navigation disabled in bracket view
   - Bracket hash reset when switching away
6. Manual testing: run with `cargo run --release`, press `p` during playoff season to see live bracket
