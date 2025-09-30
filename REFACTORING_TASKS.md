# Atomic Refactoring Tasks - AI Execution Guide

## 🤖 Instructions for AI Models

### General Guidelines

**BEFORE STARTING ANY TASK:**
1. ✅ Read the ENTIRE task card from start to finish
2. ✅ Verify all prerequisite tasks are completed
3. ✅ Create a backup branch: `git checkout -b refactor/task-{TASK_NUMBER}`
4. ✅ Run tests BEFORE making changes: `cargo test --all-features`
5. ✅ Note the number of passing tests

**WHILE WORKING:**
1. ✅ Follow the steps EXACTLY in order
2. ✅ Do NOT add new features or change behavior
3. ✅ Do NOT optimize or refactor beyond the task scope
4. ✅ Copy code EXACTLY - preserve all comments, whitespace, and formatting
5. ✅ After EACH step, verify the code compiles: `cargo check`

**AFTER COMPLETING TASK:**
1. ✅ Run full test suite: `cargo test --all-features`
2. ✅ Verify the SAME number of tests pass as before
3. ✅ Run clippy: `cargo clippy --all-features --all-targets`
4. ✅ Run formatter: `cargo fmt`
5. ✅ Commit with message: `refactor: [Task {TASK_NUMBER}] {task description}`

**IF ANYTHING FAILS:**
1. ⚠️ STOP immediately
2. ⚠️ Do NOT proceed to next step
3. ⚠️ Report the error with full output
4. ⚠️ Reset: `git checkout main && git branch -D refactor/task-{TASK_NUMBER}`

---

## Phase 1: UI Refactoring - teletext_ui.rs

### TASK 1.1: Extract Color Constants

**Prerequisites:** None  
**Estimated Time:** 10 minutes  
**Risk Level:** LOW

**Objective:** Move all color-related functions and constants from `teletext_ui.rs` to a new `colors.rs` file.

**Step-by-Step Instructions:**

1. **Create new file:**
   ```bash
   mkdir -p src/ui/teletext
   touch src/ui/teletext/colors.rs
   ```

2. **Copy content to `src/ui/teletext/colors.rs`:**
   - Open `src/teletext_ui.rs`
   - Find lines 17-47 (all functions named `*_bg()` and `*_fg()`)
   - Copy these EXACT lines to `src/ui/teletext/colors.rs`:
     ```rust
     use crossterm::style::Color;

     // Constants for teletext appearance
     pub fn header_bg() -> Color {
         Color::AnsiValue(21)
     } // Bright blue
     pub fn header_fg() -> Color {
         Color::AnsiValue(21)
     } // Bright blue
     pub fn subheader_fg() -> Color {
         Color::AnsiValue(46)
     } // Bright green
     pub fn result_fg() -> Color {
         Color::AnsiValue(46)
     } // Bright green
     pub fn text_fg() -> Color {
         Color::AnsiValue(231)
     } // Pure white
     pub fn home_scorer_fg() -> Color {
         Color::AnsiValue(51)
     } // Bright cyan
     pub fn away_scorer_fg() -> Color {
         Color::AnsiValue(51)
     } // Bright cyan
     pub fn winning_goal_fg() -> Color {
         Color::AnsiValue(201)
     } // Bright magenta
     pub fn goal_type_fg() -> Color {
         Color::AnsiValue(226)
     } // Bright yellow
     pub fn title_bg() -> Color {
         Color::AnsiValue(46)
     } // Bright green
     ```

3. **Delete from original file:**
   - In `src/teletext_ui.rs`, delete lines 17-47 (the color functions you just copied)

4. **Add import at top of `src/teletext_ui.rs`:**
   ```rust
   use crate::ui::teletext::colors::*;
   ```
   - Add this line after the existing imports (around line 15)

5. **Create module file `src/ui/teletext/mod.rs`:**
   ```bash
   touch src/ui/teletext/mod.rs
   ```
   
6. **Add to `src/ui/teletext/mod.rs`:**
   ```rust
   pub mod colors;
   ```

7. **Update `src/ui/mod.rs`:**
   - If file doesn't exist, create it: `touch src/ui/mod.rs`
   - Add:
     ```rust
     pub mod interactive;
     pub mod teletext;
     ```

8. **Verify compilation:**
   ```bash
   cargo check
   ```

9. **Run tests:**
   ```bash
   cargo test --all-features
   ```

10. **Commit:**
    ```bash
    git add -A
    git commit -m "refactor: [Task 1.1] Extract color constants to separate module"
    ```

**Success Criteria:**
- ✅ `cargo check` passes
- ✅ `cargo test --all-features` passes with same number of tests
- ✅ No clippy warnings: `cargo clippy --all-features --all-targets`

---

### TASK 1.2: Extract Team Abbreviations

**Prerequisites:** Task 1.1 completed  
**Estimated Time:** 15 minutes  
**Risk Level:** LOW

**Objective:** Move the `get_team_abbreviation` function to a new file.

**Step-by-Step Instructions:**

1. **Create new file:**
   ```bash
   touch src/ui/components/abbreviations.rs
   mkdir -p src/ui/components
   ```

2. **Copy function to `src/ui/components/abbreviations.rs`:**
   - Open `src/teletext_ui.rs`
   - Find the function `get_team_abbreviation` (starts around line 75)
   - Copy lines 53-130 (entire function with documentation) to the new file
   - Add this at the top of the new file:
     ```rust
     /// Returns the abbreviated form of a team name for compact display.
     ```

3. **Make function public in new file:**
   - Ensure the function signature starts with `pub fn`

4. **Delete from original file:**
   - In `src/teletext_ui.rs`, delete the `get_team_abbreviation` function (lines 53-130)

5. **Update `src/lib.rs`:**
   - Find the line: `pub use teletext_ui::{...get_team_abbreviation...};`
   - Change to: `pub use ui::components::abbreviations::get_team_abbreviation;`

6. **Create `src/ui/components/mod.rs`:**
   ```bash
   touch src/ui/components/mod.rs
   ```
   - Add content:
     ```rust
     pub mod abbreviations;
     ```

7. **Update `src/ui/mod.rs`:**
   ```rust
   pub mod components;
   pub mod interactive;
   pub mod teletext;
   ```

8. **Add import to `src/teletext_ui.rs`:**
   ```rust
   use crate::ui::components::abbreviations::get_team_abbreviation;
   ```

9. **Verify compilation:**
   ```bash
   cargo check
   ```

10. **Run tests:**
    ```bash
    cargo test --all-features
    ```

11. **Commit:**
    ```bash
    git add -A
    git commit -m "refactor: [Task 1.2] Extract team abbreviations to components module"
    ```

**Success Criteria:**
- ✅ `cargo check` passes
- ✅ All tests pass
- ✅ Public API in `lib.rs` still exports `get_team_abbreviation`

---

### TASK 1.3: Extract CompactDisplayConfig

**Prerequisites:** Task 1.2 completed  
**Estimated Time:** 20 minutes  
**Risk Level:** MEDIUM

**Objective:** Move `CompactDisplayConfig` struct and implementation to dedicated file.

**Step-by-Step Instructions:**

1. **Create new file:**
   ```bash
   touch src/ui/teletext/compact_display.rs
   ```

2. **Copy struct and impl to new file:**
   - Open `src/teletext_ui.rs`
   - Find `CompactDisplayConfig` struct (around line 132-146)
   - Find all `impl` blocks for `CompactDisplayConfig`
   - Copy these sections to `src/ui/teletext/compact_display.rs`
   - Include all documentation comments

3. **Add necessary imports to `src/ui/teletext/compact_display.rs`:**
   ```rust
   /// Configuration for compact display mode layout parameters.
   #[derive(Debug, Clone)]
   pub struct CompactDisplayConfig {
       // ... (copy the struct definition)
   }

   impl Default for CompactDisplayConfig {
       // ... (copy the default impl)
   }

   impl CompactDisplayConfig {
       // ... (copy all methods)
   }
   ```

4. **Delete from `src/teletext_ui.rs`:**
   - Remove the `CompactDisplayConfig` struct and all its implementations

5. **Add to `src/ui/teletext/mod.rs`:**
   ```rust
   pub mod colors;
   pub mod compact_display;

   // Re-export for backward compatibility
   pub use compact_display::CompactDisplayConfig;
   ```

6. **Add import to `src/teletext_ui.rs`:**
   ```rust
   use crate::ui::teletext::compact_display::CompactDisplayConfig;
   ```

7. **Update `src/lib.rs`:**
   - Find: `pub use teletext_ui::{CompactDisplayConfig, ...};`
   - Change to: `pub use ui::teletext::CompactDisplayConfig;`

8. **Verify compilation:**
   ```bash
   cargo check
   ```

9. **Run tests:**
   ```bash
   cargo test --all-features
   ```

10. **Commit:**
    ```bash
    git add -A
    git commit -m "refactor: [Task 1.3] Extract CompactDisplayConfig to dedicated module"
    ```

**Success Criteria:**
- ✅ `cargo check` passes
- ✅ All tests pass
- ✅ Public API unchanged

---

## Phase 2: Data Fetcher - API Module

### TASK 2.1: Extract URL Builders

**Prerequisites:** Phase 1 completed  
**Estimated Time:** 15 minutes  
**Risk Level:** LOW

**Objective:** Move URL building functions to dedicated module.

**Step-by-Step Instructions:**

1. **Create directory and file:**
   ```bash
   mkdir -p src/data_fetcher/api
   touch src/data_fetcher/api/urls.rs
   ```

2. **Copy URL functions to `src/data_fetcher/api/urls.rs`:**
   - Open `src/data_fetcher/api.rs`
   - Find these functions (lines 82-170):
     - `build_tournament_url`
     - `build_game_url`
     - `build_schedule_url`
     - `build_tournament_schedule_url`
     - `create_tournament_key`
   - Copy ALL of them with their documentation to the new file

3. **Add to new file header:**
   ```rust
   //! URL building utilities for API endpoints
   ```

4. **Make all functions public:**
   - Ensure each function has `pub fn` in its signature

5. **Delete from `src/data_fetcher/api.rs`:**
   - Remove the 5 functions you just copied

6. **Create `src/data_fetcher/api/mod.rs`:**
   ```bash
   touch src/data_fetcher/api/mod.rs
   ```

7. **Add to `src/data_fetcher/api/mod.rs`:**
   ```rust
   pub mod urls;

   // Re-export public API
   pub use urls::*;
   ```

8. **Update `src/data_fetcher.rs` (or `src/data_fetcher/mod.rs`):**
   - If it's currently a file named `data_fetcher.rs`, rename:
     ```bash
     mv src/data_fetcher.rs src/data_fetcher/mod.rs
     ```
   - Add:
     ```rust
     pub mod api;
     ```

9. **Update imports in `src/data_fetcher/api.rs`:**
   - At the top, add:
     ```rust
     use super::api::urls::*;
     ```

10. **Verify all imports in codebase:**
    ```bash
    grep -r "build_tournament_url\|build_game_url\|build_schedule_url" src/ tests/
    ```
    - Update any direct imports to use the new path

11. **Verify compilation:**
    ```bash
    cargo check
    ```

12. **Run tests:**
    ```bash
    cargo test --all-features
    ```

13. **Commit:**
    ```bash
    git add -A
    git commit -m "refactor: [Task 2.1] Extract URL builders to api/urls module"
    ```

**Success Criteria:**
- ✅ `cargo check` passes
- ✅ All tests pass
- ✅ All URL functions accessible via `data_fetcher::api::*`

---

### TASK 2.2: Extract HTTP Client

**Prerequisites:** Task 2.1 completed  
**Estimated Time:** 15 minutes  
**Risk Level:** LOW

**Objective:** Move HTTP client creation to dedicated module.

**Step-by-Step Instructions:**

1. **Create file:**
   ```bash
   touch src/data_fetcher/api/client.rs
   ```

2. **Copy to `src/data_fetcher/api/client.rs`:**
   - Open `src/data_fetcher/api.rs`
   - Find function `create_http_client_with_timeout` (around line 50-55)
   - Find test function `create_test_http_client` (if exists, around line 58-62)
   - Copy both functions with all documentation
   - Add at top:
     ```rust
     use reqwest::Client;
     use std::time::Duration;
     ```

3. **Add to new file:**
   ```rust
   //! HTTP client configuration and creation

   use reqwest::Client;
   use std::time::Duration;

   /// Creates a properly configured HTTP client with connection pooling and timeout handling.
   pub fn create_http_client_with_timeout(timeout_seconds: u64) -> Result<Client, reqwest::Error> {
       Client::builder()
           .timeout(Duration::from_secs(timeout_seconds))
           .pool_max_idle_per_host(crate::constants::HTTP_POOL_MAX_IDLE_PER_HOST)
           .build()
   }

   /// Creates an HTTP client for testing with default timeout
   #[cfg(test)]
   pub fn create_test_http_client() -> Client {
       create_http_client_with_timeout(crate::constants::DEFAULT_HTTP_TIMEOUT_SECONDS)
           .expect("Failed to create test HTTP client")
   }
   ```

4. **Delete from `src/data_fetcher/api.rs`:**
   - Remove both functions

5. **Update `src/data_fetcher/api/mod.rs`:**
   ```rust
   pub mod client;
   pub mod urls;

   // Re-export public API
   pub use client::*;
   pub use urls::*;
   ```

6. **Add import to `src/data_fetcher/api.rs`:**
   ```rust
   use crate::data_fetcher::api::client::*;
   ```

7. **Verify compilation:**
   ```bash
   cargo check
   ```

8. **Run tests:**
   ```bash
   cargo test --all-features
   ```

9. **Commit:**
   ```bash
   git add -A
   git commit -m "refactor: [Task 2.2] Extract HTTP client to api/client module"
   ```

**Success Criteria:**
- ✅ `cargo check` passes
- ✅ All tests pass

---

## Phase 3: Cache Module Split

### TASK 3.1: Extract Cache Type Definitions

**Prerequisites:** Phase 2 completed  
**Estimated Time:** 20 minutes  
**Risk Level:** MEDIUM

**Objective:** Move cache struct definitions to separate types module.

**Step-by-Step Instructions:**

1. **Create directory and file:**
   ```bash
   mkdir -p src/data_fetcher/cache
   touch src/data_fetcher/cache/types.rs
   ```

2. **Copy structs to `src/data_fetcher/cache/types.rs`:**
   - Open `src/data_fetcher/cache.rs`
   - Find these structs (lines 38-74):
     - `CachedTournamentData`
     - `CachedDetailedGameData`
     - `CachedGoalEventsData`
     - `CachedHttpResponse`
   - Copy ALL struct definitions (just the struct, not impl blocks yet)
   - Add necessary imports:
     ```rust
     use crate::data_fetcher::models::{DetailedGameResponse, GoalEventData, ScheduleResponse};
     use std::time::Instant;
     ```

3. **Make all structs public:**
   - Ensure each has `#[derive(Debug, Clone)]`
   - Ensure each has `pub struct`

4. **Copy impl blocks:**
   - Find all `impl` blocks for these structs (lines 75-234)
   - Copy them to the new file below the structs

5. **Add necessary imports to new file:**
   ```rust
   use std::time::{Duration, Instant};
   use tracing::debug;
   use crate::constants::cache_ttl;
   ```

6. **Rename original file:**
   ```bash
   mv src/data_fetcher/cache.rs src/data_fetcher/cache/mod.rs
   ```

7. **Delete from `src/data_fetcher/cache/mod.rs`:**
   - Remove struct definitions and their impl blocks

8. **Add to top of `src/data_fetcher/cache/mod.rs`:**
   ```rust
   pub mod types;
   pub use types::*;
   ```

9. **Verify compilation:**
   ```bash
   cargo check
   ```

10. **Run tests:**
    ```bash
    cargo test --all-features
    ```

11. **Commit:**
    ```bash
    git add -A
    git commit -m "refactor: [Task 3.1] Extract cache type definitions to types module"
    ```

**Success Criteria:**
- ✅ `cargo check` passes
- ✅ All tests pass
- ✅ Cache types still accessible

---

## Quick Reference: Common Issues & Solutions

### Issue: "Cannot find module X"
**Solution:** Check that you added `pub mod X;` to the parent module's `mod.rs`

### Issue: "Function X is private"
**Solution:** Ensure function has `pub fn` and is re-exported in `mod.rs` with `pub use`

### Issue: "Circular dependency"
**Solution:** Use `super::` or `crate::` paths instead of relative imports

### Issue: Tests failing after refactor
**Solution:** 
1. Check that all `pub use` re-exports are in place
2. Verify imports in test files
3. Run `cargo clean && cargo test`

### Issue: Clippy warnings
**Solution:** Run `cargo clippy --fix --all-features --all-targets`

---

## Task Checklist Template

For each task, use this checklist:

- [ ] Prerequisites completed
- [ ] Tests passing before changes
- [ ] Created new file(s)
- [ ] Copied code exactly
- [ ] Updated module declarations
- [ ] Added re-exports
- [ ] Updated imports
- [ ] `cargo check` passes
- [ ] `cargo test` passes
- [ ] `cargo clippy` clean
- [ ] `cargo fmt` applied
- [ ] Committed changes

---

## Priority Queue

Execute tasks in this order:

**Week 1 (High Priority):**
1. Task 1.1 → 1.2 → 1.3 (UI Colors & Components)
2. Task 2.1 → 2.2 (API URLs & Client)
3. Task 3.1 (Cache Types)

**Week 2 (Medium Priority):**
4. Remaining teletext_ui splits
5. Remaining data_fetcher/api splits
6. Cache module splits

**Week 3 (Low Priority):**
7. Config module split
8. Main.rs restructure
9. Test reorganization

---

**Document Version:** 1.0  
**Last Updated:** 2025-09-30  
**Total Tasks Defined:** 7 (more to be added based on progress)