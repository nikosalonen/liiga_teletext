# Contextual Keybindings & Cache Reset

## Problem

Keyboard shortcut hints in the footer are static — they show options that aren't meaningful in the current context (e.g., 'p' for brackets outside playoff season, 's' for standings while in bracket view, 't' for today when today has no games). Additionally, there's no way to reset the persistent player name cache without manually deleting files.

## Changes

### 1. Playoff bracket key ('p') — only when data exists

**Footer**: Show `p=Pudotuspeli` in the Games view footer only when `has_bracket_data` is true.

**Input handler**: Ignore 'p' keypress when no bracket data exists.

**Detection**: Use the bracket API's `has_data` flag. Fetch bracket data once on first refresh cycle (not on every refresh — cache the result). Store `has_bracket_data: bool` in `InteractiveState` and propagate through `TeletextPage` to the footer.

**Threading the value to footer**: `TeletextPage` already has `is_standings_page` and `standings_live_mode` fields. Add `is_bracket_page: bool` and `has_bracket_data: bool` fields. Set `is_bracket_page` in `create_bracket_page()`. The view_mode derivation in `core.rs:587-593` gains a third branch for bracket pages. The `has_bracket_data` flag is passed through to the footer function.

**Footer parameter**: To avoid growing the already-11-parameter `render_footer_with_view` further, bundle context into a `FooterContext` struct.

**Files**:
- `src/teletext_ui/footer.rs` — introduce `FooterContext` struct, add `has_bracket_data`, conditionally render `p=Pudotuspeli`
- `src/teletext_ui/core.rs` — add `is_bracket_page` and `has_bracket_data` fields to `TeletextPage`, update view_mode derivation to handle bracket, pass `has_bracket_data` to footer
- `src/ui/interactive/input_handler.rs` — guard 'p' handler on bracket data availability
- `src/ui/interactive/state_manager.rs` — store `has_bracket_data` in `NavigationState`
- `src/ui/interactive/refresh_coordinator.rs` — on first refresh, fetch bracket data and set `has_bracket_data` in state; the bracket is already fetched during `perform_bracket_refresh`, reuse the `has_data` flag
- `src/ui/interactive/navigation_manager.rs` — set `is_bracket_page = true` in `create_bracket_page()`

### 2. No standings key ('s') in bracket view

**Footer**: Split the current `_ =>` catch-all match arm into explicit `Games`, `Bracket`, and `None` arms. The Bracket arm shows `q=Lopeta ←→=Sivut p=Pudotuspeli` without `s=Taulukko`. `None` (non-interactive/`--once` mode) keeps current behavior.

**Input handler**: Already a no-op — `ViewMode::Bracket => {}` exists at line 487. No code change needed.

**Files**:
- `src/teletext_ui/footer.rs` — separate Bracket match arm

### 3. Today key ('t') — hidden on auto-forwarded initial view

**Problem**: On launch, if today has no games, the app auto-forwards to the next date with games. Pressing 't' would go to today (no games) and re-forward — a no-op. But after the user manually navigates away via Shift+arrows, 't' should reappear.

**Solution**: Track `initial_fetched_date: Option<String>` — the fetched_date from the first successful data fetch. This captures the auto-forwarded date (e.g., "2026-03-25" when today "2026-03-22" has no games).

**Updated `show_today_shortcut` logic** (replaces existing logic in `core.rs:594-606`):
- `show_today_shortcut = true` when `fetched_date` differs from BOTH `initial_fetched_date` AND the default date (today/yesterday)
- On initial auto-forward: `fetched_date == initial_fetched_date` → false (correct: 't' hidden)
- After manual navigation: `fetched_date` differs from `initial_fetched_date` → true (correct: 't' shown)
- When today has games and user hasn't navigated: `fetched_date == default_date` → false (correct: 't' hidden)

**State tracking**: Add `initial_fetched_date: Option<String>` to `TeletextPage`. Set it once on the first page creation from `InteractiveState`. The refresh coordinator sets it after the first successful fetch.

**Files**:
- `src/ui/interactive/state_manager.rs` — add `initial_fetched_date: Option<String>` to `NavigationState`
- `src/ui/interactive/refresh_coordinator.rs` — set `initial_fetched_date` after first successful fetch
- `src/teletext_ui/core.rs` — add `initial_fetched_date` field to `TeletextPage`, update `show_today_shortcut` calculation

### 4. `--reset-cache` CLI flag

**Behavior**: Delete all `players_*.json` files from the cache directory, print confirmation, then continue running the app normally (fresh state).

**Implementation**:
- Add `--reset-cache` boolean flag to `Args` in `cli.rs` under Configuration help heading
- In `main.rs` (before entering interactive/once mode): if flag set, call cache reset
- Cache reset: standalone async function in `persistence.rs` (NOT a method on `PlayerNameStore` — avoids triggering LazyLock initialization). Uses `tokio::fs::read_dir` + filename prefix filter (no glob crate needed). The `PLAYER_NAME_STORE` hasn't been accessed yet at this point, so it starts fresh naturally.
- Do NOT add to `is_noninteractive_mode` — the app continues running after reset

**Files**:
- `src/cli.rs` — add `--reset-cache` flag
- `src/main.rs` — call reset before mode dispatch
- `src/data_fetcher/cache/persistence.rs` — add standalone `clear_all_cache_files()` async function

## Testing

- Footer tests: verify correct controls string for each ViewMode + `has_bracket_data` combination
- Footer tests: verify `None` view mode (non-interactive) still works
- Input handler: verify 'p' is no-op without bracket data (already a no-op for 's' in Bracket)
- Cache reset: verify files are deleted when present, graceful when cache dir missing or empty
- Today shortcut: verify hidden on auto-forward, visible after manual navigation, hidden when on today with games
