# Contextual Keybindings & Cache Reset Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make keyboard shortcut hints in the footer context-aware (hide irrelevant keys) and add a `--reset-cache` CLI flag.

**Architecture:** Four independent changes: (1) show 'p' only when bracket API has data, (2) hide 's' in bracket view, (3) hide 't' on auto-forwarded initial date, (4) add `--reset-cache` flag. Changes 1-3 touch the footer renderer and state; change 4 is CLI-only.

**Tech Stack:** Rust, crossterm, clap, tokio

**Spec:** `docs/superpowers/specs/2026-03-22-contextual-keybindings-and-cache-reset-design.md`

---

### Task 1: Add `FooterContext` struct and `is_bracket_page` / `has_bracket_data` fields

Replace the 11-parameter `render_footer_with_view` with a `FooterContext` struct. Add `is_bracket_page` and `has_bracket_data` fields to `TeletextPage`. Thread `ViewMode::Bracket` to the footer.

**Files:**
- Modify: `src/teletext_ui/footer.rs:23-35` (function signature → struct)
- Modify: `src/teletext_ui/core.rs:31-55` (add fields to `TeletextPage`)
- Modify: `src/teletext_ui/core.rs:580-620` (view_mode derivation + footer call)

- [ ] **Step 1: Define `FooterContext` in `footer.rs`**

Replace the function signature of `render_footer_with_view` with a struct parameter:

```rust
/// Context for rendering the footer
pub struct FooterContext<'a> {
    pub footer_y: usize,
    pub width: usize,
    pub total_pages: usize,
    pub auto_refresh_indicator: &'a Option<LoadingIndicator>,
    pub auto_refresh_disabled: bool,
    pub error_warning_active: bool,
    pub season_countdown: &'a Option<String>,
    pub view_mode: Option<&'a crate::ui::interactive::state_manager::ViewMode>,
    pub show_today_shortcut: bool,
    pub has_bracket_data: bool,
}
```

Update the function signature:
```rust
pub fn render_footer_with_view(
    _stdout: &mut Stdout,
    buffer: &mut String,
    ctx: &FooterContext<'_>,
) -> Result<(), AppError> {
```

Update all field accesses inside the function from bare names to `ctx.field_name`.

- [ ] **Step 2: Add fields to `TeletextPage` in `core.rs`**

Add after `is_loading_page` (line 54):
```rust
    pub(super) is_bracket_page: bool,
    pub(super) has_bracket_data: bool,
    pub(super) initial_fetched_date: Option<String>,
```

Initialize them as `false`, `false`, `None` in `TeletextPage::new()` (around line 170).

- [ ] **Step 3: Update view_mode derivation in `render_buffered` (`core.rs:587-593`)**

Replace:
```rust
let view_mode = if self.is_standings_page {
    Some(crate::ui::interactive::state_manager::ViewMode::Standings {
        live_mode: self.standings_live_mode,
    })
} else {
    Some(crate::ui::interactive::state_manager::ViewMode::Games)
};
```

With:
```rust
let view_mode = if self.is_standings_page {
    Some(crate::ui::interactive::state_manager::ViewMode::Standings {
        live_mode: self.standings_live_mode,
    })
} else if self.is_bracket_page {
    Some(crate::ui::interactive::state_manager::ViewMode::Bracket)
} else {
    Some(crate::ui::interactive::state_manager::ViewMode::Games)
};
```

- [ ] **Step 4: Update the footer call site in `render_buffered` (`core.rs:608-620`)**

Replace the direct `render_footer_with_view` call with `FooterContext`:
```rust
super::footer::render_footer_with_view(
    stdout,
    &mut buffer,
    &super::footer::FooterContext {
        footer_y,
        width: width as usize,
        total_pages,
        auto_refresh_indicator: &self.auto_refresh_indicator,
        auto_refresh_disabled: self.auto_refresh_disabled,
        error_warning_active: self.error_warning_active,
        season_countdown: &self.season_countdown,
        view_mode: view_mode.as_ref(),
        show_today_shortcut,
        has_bracket_data: self.has_bracket_data,
    },
)?;
```

- [ ] **Step 5: Set `is_bracket_page = true` in `create_bracket_page` (`navigation_manager.rs:529-556`)**

After `page` is created (after line 546), add:
```rust
page.is_bracket_page = true;
```

Note: `is_bracket_page` is `pub(super)` within `teletext_ui`, and `navigation_manager` is in `ui::interactive`, so we need a setter method. Add to `indicators.rs` (next to `set_fetched_date`):
```rust
pub fn set_bracket_page(&mut self, is_bracket: bool) {
    self.is_bracket_page = is_bracket;
}

pub fn set_has_bracket_data(&mut self, has_data: bool) {
    self.has_bracket_data = has_data;
}

pub fn set_initial_fetched_date(&mut self, date: Option<String>) {
    self.initial_fetched_date = date;
}
```

Then in `create_bracket_page` after line 546:
```rust
page.set_bracket_page(true);
```

- [ ] **Step 6: Run `cargo test --all-features` and `cargo clippy`**

Expect: compilation passes, all existing tests pass. The footer controls strings haven't changed yet — that's next.

- [ ] **Step 7: Commit**

```bash
git add src/teletext_ui/footer.rs src/teletext_ui/core.rs src/teletext_ui/indicators.rs src/ui/interactive/navigation_manager.rs
git commit -m "refactor: introduce FooterContext struct and bracket/today page fields"
```

---

### Task 2: Footer shows 'p' conditionally and hides 's' in bracket view

Update the footer match arms to show `p=Pudotuspeli` in Games view when `has_bracket_data` is true, and show a separate bracket footer without `s=Taulukko`.

**Files:**
- Modify: `src/teletext_ui/footer.rs:37-73` (match arms)

- [ ] **Step 1: Write tests for the new footer behavior**

Add to the existing `mod tests` in `footer.rs`:

```rust
#[test]
fn test_footer_games_view_with_bracket_data() {
    let mut buffer = String::new();
    let mut stdout = std::io::stdout();
    let ctx = FooterContext {
        footer_y: 23,
        width: 80,
        total_pages: 2,
        auto_refresh_indicator: &None,
        auto_refresh_disabled: false,
        error_warning_active: false,
        season_countdown: &None,
        view_mode: Some(&crate::ui::interactive::state_manager::ViewMode::Games),
        show_today_shortcut: false,
        has_bracket_data: true,
    };
    render_footer_with_view(&mut stdout, &mut buffer, &ctx).unwrap();
    assert!(buffer.contains("p=Pudotuspeli"));
    assert!(buffer.contains("s=Taulukko"));
}

#[test]
fn test_footer_games_view_without_bracket_data() {
    let mut buffer = String::new();
    let mut stdout = std::io::stdout();
    let ctx = FooterContext {
        footer_y: 23,
        width: 80,
        total_pages: 2,
        auto_refresh_indicator: &None,
        auto_refresh_disabled: false,
        error_warning_active: false,
        season_countdown: &None,
        view_mode: Some(&crate::ui::interactive::state_manager::ViewMode::Games),
        show_today_shortcut: false,
        has_bracket_data: false,
    };
    render_footer_with_view(&mut stdout, &mut buffer, &ctx).unwrap();
    assert!(!buffer.contains("p=Pudotuspeli"));
    assert!(buffer.contains("s=Taulukko"));
}

#[test]
fn test_footer_bracket_view_no_standings_key() {
    let mut buffer = String::new();
    let mut stdout = std::io::stdout();
    let ctx = FooterContext {
        footer_y: 23,
        width: 80,
        total_pages: 2,
        auto_refresh_indicator: &None,
        auto_refresh_disabled: false,
        error_warning_active: false,
        season_countdown: &None,
        view_mode: Some(&crate::ui::interactive::state_manager::ViewMode::Bracket),
        show_today_shortcut: false,
        has_bracket_data: true,
    };
    render_footer_with_view(&mut stdout, &mut buffer, &ctx).unwrap();
    assert!(!buffer.contains("s=Taulukko"));
    assert!(buffer.contains("p=Pudotuspeli"));
}

#[test]
fn test_footer_none_view_mode() {
    let mut buffer = String::new();
    let mut stdout = std::io::stdout();
    let ctx = FooterContext {
        footer_y: 23,
        width: 80,
        total_pages: 1,
        auto_refresh_indicator: &None,
        auto_refresh_disabled: false,
        error_warning_active: false,
        season_countdown: &None,
        view_mode: None,
        show_today_shortcut: false,
        has_bracket_data: false,
    };
    render_footer_with_view(&mut stdout, &mut buffer, &ctx).unwrap();
    assert!(buffer.contains("s=Taulukko"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --all-features -- teletext_ui::footer::tests`
Expected: FAIL (controls strings don't include `p=Pudotuspeli` yet)

- [ ] **Step 3: Update the footer match arms**

Replace the `_ =>` catch-all (line 63) with explicit Games, Bracket, and None arms. The Games arm conditionally includes `p=Pudotuspeli`. The Bracket arm shows `p=Pudotuspeli` instead of `s=Taulukko`.

```rust
Some(crate::ui::interactive::state_manager::ViewMode::Bracket) => {
    match (ctx.auto_refresh_disabled, ctx.total_pages > 1) {
        (true, true) => "q=Lopeta ←→=Sivut p=Pudotuspeli (Ei päivity)",
        (true, false) => "q=Lopeta p=Pudotuspeli (Ei päivity)",
        (false, true) => "q=Lopeta ←→=Sivut p=Pudotuspeli",
        (false, false) => "q=Lopeta p=Pudotuspeli",
    }
}
Some(crate::ui::interactive::state_manager::ViewMode::Games) | None => {
    match (ctx.auto_refresh_disabled, ctx.show_today_shortcut, ctx.total_pages > 1, ctx.has_bracket_data) {
        (true, true, true, true) => "q=Lopeta ←→=Sivut s=Taulukko p=Pudotuspeli t=Tänään (Ei päivity)",
        (true, true, true, false) => "q=Lopeta ←→=Sivut s=Taulukko t=Tänään (Ei päivity)",
        (true, true, false, true) => "q=Lopeta s=Taulukko p=Pudotuspeli t=Tänään (Ei päivity)",
        (true, true, false, false) => "q=Lopeta s=Taulukko t=Tänään (Ei päivity)",
        (true, false, true, true) => "q=Lopeta ←→=Sivut s=Taulukko p=Pudotuspeli (Ei päivity)",
        (true, false, true, false) => "q=Lopeta ←→=Sivut s=Taulukko (Ei päivity)",
        (true, false, false, true) => "q=Lopeta s=Taulukko p=Pudotuspeli (Ei päivity)",
        (true, false, false, false) => "q=Lopeta s=Taulukko (Ei päivity)",
        (false, true, true, true) => "q=Lopeta ←→=Sivut s=Taulukko p=Pudotuspeli t=Tänään",
        (false, true, true, false) => "q=Lopeta ←→=Sivut s=Taulukko t=Tänään",
        (false, true, false, true) => "q=Lopeta s=Taulukko p=Pudotuspeli t=Tänään",
        (false, true, false, false) => "q=Lopeta s=Taulukko t=Tänään",
        (false, false, true, true) => "q=Lopeta ←→=Sivut s=Taulukko p=Pudotuspeli",
        (false, false, true, false) => "q=Lopeta ←→=Sivut s=Taulukko",
        (false, false, false, true) => "q=Lopeta s=Taulukko p=Pudotuspeli",
        (false, false, false, false) => "q=Lopeta s=Taulukko",
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --all-features -- teletext_ui::footer::tests`
Expected: PASS

- [ ] **Step 5: Run full test suite and clippy**

Run: `cargo test --all-features && cargo clippy --all-features --all-targets -- -D warnings`

- [ ] **Step 6: Commit**

```bash
git add src/teletext_ui/footer.rs
git commit -m "feat: contextual footer keys — show p=Pudotuspeli conditionally, hide s in bracket view"
```

---

### Task 3: Propagate `has_bracket_data` from bracket API to state and pages

Fetch bracket data on first Games-view refresh to determine whether playoff bracket is available. Store the flag in state and propagate to pages.

**Files:**
- Modify: `src/ui/interactive/state_manager.rs:167-192` (add `has_bracket_data` to `NavigationState`)
- Modify: `src/ui/interactive/refresh_coordinator.rs:310-440` (fetch bracket on first refresh, set flag)
- Modify: `src/ui/interactive/navigation_manager.rs:57-120` (pass `has_bracket_data` to pages)
- Modify: `src/ui/interactive/event_handler.rs:122-151` (pass `has_bracket_data` to input handler)
- Modify: `src/ui/interactive/input_handler.rs:20-33,455-469` (guard 'p' key)

- [ ] **Step 1: Add `has_bracket_data` to `NavigationState`**

In `state_manager.rs`, add field to `NavigationState`:
```rust
pub has_bracket_data: bool,
```

Initialize as `false` in `NavigationState::new()`.

Add a convenience accessor to `InteractiveState`:
```rust
pub fn has_bracket_data(&self) -> bool {
    self.navigation.has_bracket_data
}
```

- [ ] **Step 2: Fetch bracket `has_data` on first Games-view refresh**

In `refresh_coordinator.rs`, add a `bracket_checked: bool` field to `RefreshCoordinator`:
```rust
pub struct RefreshCoordinator {
    cache_config: CacheMonitoringConfig,
    consecutive_transient_empty: u32,
    bracket_checked: bool,
}
```

Initialize as `false` in `new()`.

In `perform_refresh_cycle`, add the one-time bracket check at the **very beginning** of the method (after line 452, before any view-mode branches or early returns). This ensures it executes even if the first refresh hits the cached-game-restoration early return path:
```rust
// One-time check for bracket data availability on first Games refresh
if !self.bracket_checked {
    self.bracket_checked = true;
    if let Ok(config) = crate::config::Config::load().await {
        let timeout = std::time::Duration::from_secs(config.http_timeout_seconds + 5);
        if let Ok(Ok(bracket)) = tokio::time::timeout(
            timeout,
            crate::data_fetcher::api::bracket_api::fetch_playoff_bracket(&config),
        ).await {
            state.navigation.has_bracket_data = bracket.has_data;
            tracing::info!("Bracket data check: has_data={}", bracket.has_data);
        }
    }
}
```

Also update `has_bracket_data` when bracket view refreshes (in `perform_bracket_refresh`, after line 863):
```rust
if let Some(ref b) = bracket {
    state.navigation.has_bracket_data = b.has_data;
}
```

- [ ] **Step 3: Pass `has_bracket_data` to pages during creation**

In `refresh_coordinator.rs`, after pages are created via `create_or_restore_page` or `create_page` (around lines 395-410), set the flag:
```rust
if let Some(ref mut page) = current_page {
    page.set_has_bracket_data(state.has_bracket_data());
}
```

Similarly, set it on bracket pages in `perform_bracket_refresh` after `create_bracket_page` (around line 907):
```rust
page.set_has_bracket_data(state.has_bracket_data());
```

And on standings pages in `perform_standings_refresh` after page creation.

- [ ] **Step 4: Guard 'p' key in input handler**

Add `has_bracket_data: bool` (by value — it's `Copy`) to `KeyEventParams`:
```rust
pub(super) struct KeyEventParams<'a> {
    // ... existing fields ...
    pub has_bracket_data: bool,
}
```

In `event_handler.rs`, pass the field:
```rust
has_bracket_data: state.navigation.has_bracket_data,
```

In `input_handler.rs`, wrap the 'p' handler (line 455) with a guard. Always allow exiting bracket view even if `has_bracket_data` is false:
```rust
KeyCode::Char('p') => {
    if params.has_bracket_data || matches!(*params.current_view, ViewMode::Bracket) {
        tracing::info!("Bracket view toggle requested");
        match *params.current_view {
            ViewMode::Bracket => {
                *params.current_view = params
                    .preserved_bracket_return_view
                    .take()
                    .unwrap_or(ViewMode::Games);
            }
            other => {
                *params.preserved_bracket_return_view = Some(other);
                *params.current_view = ViewMode::Bracket;
            }
        }
        *params.needs_refresh = true;
    }
}
```

- [ ] **Step 5: Run full test suite and clippy**

Run: `cargo test --all-features && cargo clippy --all-features --all-targets -- -D warnings`

- [ ] **Step 6: Commit**

```bash
git add src/ui/interactive/state_manager.rs src/ui/interactive/refresh_coordinator.rs src/ui/interactive/navigation_manager.rs src/ui/interactive/event_handler.rs src/ui/interactive/input_handler.rs
git commit -m "feat: show p=Pudotuspeli only when bracket API reports playoff data"
```

---

### Task 4: Hide 't=Tänään' on auto-forwarded initial date

Track the initial fetched date so 't' is hidden when still viewing the auto-forwarded date, and shown after manual navigation.

**Files:**
- Modify: `src/ui/interactive/state_manager.rs` (add `initial_fetched_date` to `NavigationState`)
- Modify: `src/ui/interactive/refresh_coordinator.rs` (set `initial_fetched_date` on first fetch)
- Modify: `src/teletext_ui/core.rs:594-606` (update `show_today_shortcut` logic)
- Modify: `src/ui/interactive/navigation_manager.rs` (pass `initial_fetched_date` to pages)

- [ ] **Step 1: Add `initial_fetched_date` to `NavigationState`**

In `state_manager.rs`:
```rust
pub initial_fetched_date: Option<String>,
```

Initialize as `None` in `NavigationState::new()`.

Add accessor to `InteractiveState`:
```rust
pub fn initial_fetched_date(&self) -> &Option<String> {
    &self.navigation.initial_fetched_date
}
```

- [ ] **Step 2: Set `initial_fetched_date` on first successful fetch**

In `src/ui/interactive/core.rs`, after `perform_refresh_cycle` returns the `refresh_result` (line 85) and before `process_refresh_results` is called (line 94), add:

```rust
// Set the initial fetched date once on the first successful fetch
if state.navigation.initial_fetched_date.is_none()
    && !refresh_result.fetched_date.is_empty()
{
    state.navigation.initial_fetched_date = Some(refresh_result.fetched_date.clone());
    tracing::info!("Initial fetched date set to: {}", refresh_result.fetched_date);
}
```

This is placed in `core.rs` (the main event loop) because `perform_refresh_cycle` delegates to different methods (games, standings, bracket) and we need a single capture point. The `RefreshResult.fetched_date` is available after any successful refresh.

- [ ] **Step 3: Pass `initial_fetched_date` to pages**

After page creation in `refresh_coordinator.rs` (same places as Task 3 step 3), set:
```rust
if let Some(ref mut page) = current_page {
    page.set_initial_fetched_date(state.initial_fetched_date().clone());
    page.set_has_bracket_data(state.has_bracket_data());
}
```

- [ ] **Step 4: Update `show_today_shortcut` logic in `core.rs:594-606`**

Replace:
```rust
let show_today_shortcut = self.fetched_date.as_ref().is_some_and(|date| {
    let default_date = if should_show_todays_games() {
        Local::now().format("%Y-%m-%d").to_string()
    } else {
        Local::now()
            .date_naive()
            .pred_opt()
            .expect("Date underflow cannot happen")
            .format("%Y-%m-%d")
            .to_string()
    };
    date != &default_date
});
```

With:
```rust
let show_today_shortcut = self.fetched_date.as_ref().is_some_and(|date| {
    let default_date = if should_show_todays_games() {
        Local::now().format("%Y-%m-%d").to_string()
    } else {
        Local::now()
            .date_naive()
            .pred_opt()
            .expect("Date underflow cannot happen")
            .format("%Y-%m-%d")
            .to_string()
    };
    // Hide 't' when on the default date (today/yesterday)
    if date == &default_date {
        return false;
    }
    // Hide 't' when still viewing the initial auto-forwarded date
    if let Some(ref initial) = self.initial_fetched_date {
        if date == initial {
            return false;
        }
    }
    true
});
```

- [ ] **Step 5: Run full test suite and clippy**

Run: `cargo test --all-features && cargo clippy --all-features --all-targets -- -D warnings`

- [ ] **Step 6: Commit**

```bash
git add src/ui/interactive/state_manager.rs src/ui/interactive/core.rs src/ui/interactive/refresh_coordinator.rs src/teletext_ui/core.rs src/ui/interactive/navigation_manager.rs
git commit -m "feat: hide t=Tänään on auto-forwarded initial date, show after navigation"
```

---

### Task 5: Add `--reset-cache` CLI flag

Add a CLI flag that deletes all persistent player cache files and continues running normally.

**Files:**
- Modify: `src/cli.rs` (add flag)
- Modify: `src/main.rs` (call reset before mode dispatch)
- Modify: `src/data_fetcher/cache/persistence.rs` (add `clear_all_cache_files()`)

- [ ] **Step 1: Write test for `clear_all_cache_files`**

Add to `persistence.rs` test module:
```rust
#[tokio::test]
async fn test_clear_all_cache_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    // Create fake cache files
    tokio::fs::write(cache_dir.join("players_2025.json"), "{}").await.unwrap();
    tokio::fs::write(cache_dir.join("players_2024.json"), "{}").await.unwrap();
    tokio::fs::write(cache_dir.join("other_file.txt"), "keep").await.unwrap();

    let count = clear_all_cache_files_in(&cache_dir).await;
    assert_eq!(count, 2);

    // Verify player files deleted
    assert!(!cache_dir.join("players_2025.json").exists());
    assert!(!cache_dir.join("players_2024.json").exists());
    // Verify other files untouched
    assert!(cache_dir.join("other_file.txt").exists());
}

#[tokio::test]
async fn test_clear_all_cache_files_empty_dir() {
    let temp_dir = tempfile::tempdir().unwrap();
    let count = clear_all_cache_files_in(&temp_dir.path().to_path_buf()).await;
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_clear_all_cache_files_missing_dir() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("nonexistent_subdir");
    let count = clear_all_cache_files_in(&path).await;
    assert_eq!(count, 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --all-features -- cache::persistence::tests::test_clear_all_cache_files`
Expected: FAIL (function doesn't exist)

- [ ] **Step 3: Implement `clear_all_cache_files` in `persistence.rs`**

Add as standalone functions (NOT methods on `PlayerNameStore`) — this avoids triggering LazyLock initialization:

```rust
/// Deletes all player cache files from the given directory.
/// Returns the count of deleted files.
async fn clear_all_cache_files_in(cache_dir: &std::path::Path) -> usize {
    let mut deleted = 0;
    let mut entries = match tokio::fs::read_dir(cache_dir).await {
        Ok(entries) => entries,
        Err(e) => {
            tracing::debug!("Cache directory not accessible: {e}");
            return 0;
        }
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("players_") && name_str.ends_with(".json") {
            if let Err(e) = tokio::fs::remove_file(entry.path()).await {
                tracing::warn!("Failed to delete {}: {e}", entry.path().display());
            } else {
                deleted += 1;
            }
        }
    }
    deleted
}

/// Deletes all persistent player cache files from the default cache directory.
/// Returns the count of deleted files.
pub async fn clear_all_cache_files() -> usize {
    let cache_dir = get_cache_dir_path();
    let count = clear_all_cache_files_in(&cache_dir).await;
    if count > 0 {
        info!("Deleted {count} player cache file(s) from {}", cache_dir.display());
    } else {
        info!("No player cache files found in {}", cache_dir.display());
    }
    count
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --all-features -- cache::persistence::tests::test_clear_all_cache_files`
Expected: PASS

- [ ] **Step 5: Add `--reset-cache` flag to `cli.rs`**

Add to `Args` struct:
```rust
/// Clear persistent player name cache and start fresh.
/// Removes cached player names from disk; the app continues running normally after reset.
#[arg(long = "reset-cache", help_heading = "Configuration")]
pub reset_cache: bool,
```

- [ ] **Step 6: Call cache reset in `main.rs`**

Add after `let _config = Config::load().await?;` (line 54) and before `if args.once` (line 56):

```rust
if args.reset_cache {
    let count = crate::data_fetcher::cache::persistence::clear_all_cache_files().await;
    println!("Cleared {count} player cache file(s).");
}
```

Make `clear_all_cache_files` accessible: ensure `persistence.rs` is `pub` in its module chain. Check `data_fetcher/cache/mod.rs` and `data_fetcher/mod.rs` — add `pub use` if needed:

In `data_fetcher/cache/mod.rs`, add:
```rust
pub use persistence::clear_all_cache_files;
```

In `data_fetcher/mod.rs`, ensure `cache` is `pub`:
```rust
pub mod cache;
```

- [ ] **Step 7: Run full test suite and clippy**

Run: `cargo test --all-features && cargo clippy --all-features --all-targets -- -D warnings`

- [ ] **Step 8: Commit**

```bash
git add src/cli.rs src/main.rs src/data_fetcher/cache/persistence.rs src/data_fetcher/cache/mod.rs src/data_fetcher/mod.rs
git commit -m "feat: add --reset-cache flag to clear persistent player name cache"
```

---

### Task 6: Update documentation

Update CLAUDE.md, cli.rs help text, and README.md to reflect the new behaviors.

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md`

- [ ] **Step 1: Update CLAUDE.md**

In the event loop section, update the keyboard events line to note conditional keys:
```
4. Process keyboard events (←/→ pages, Shift+←/→ dates, 's' standings, 'p' bracket (playoffs only), 'l' live mode, 't' today, 'r' refresh, 'q' quit)
```

Add `--reset-cache` to the run commands section.

- [ ] **Step 2: Update README.md**

In the Interactive Mode section, note that 'p' only appears during playoffs:
```markdown
- Press `p` to toggle playoff bracket view (visible during playoffs)
```

In the Configuration section, add:
```markdown
- `--reset-cache` - Clear cached player names and start fresh
```

- [ ] **Step 3: Run cargo fmt**

Run: `cargo fmt`

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md README.md
git commit -m "docs: document contextual keybindings and --reset-cache flag"
```

---

### Verification

After all tasks, run:
```bash
cargo test --all-features
cargo clippy --all-features --all-targets -- -D warnings
cargo fmt --check
```

Manual smoke test:
1. Run `cargo run --release` — verify footer shows appropriate keys
2. If during regular season: 'p' should NOT appear in footer
3. Navigate with Shift+arrows — verify 't=Tänään' appears after navigation
4. Run `cargo run --release -- --reset-cache` — verify cache files deleted and app continues
