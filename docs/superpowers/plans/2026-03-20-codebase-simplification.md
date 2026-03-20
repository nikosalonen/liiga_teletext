# Codebase Simplification & Optimization Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce codebase complexity and line count (~33k lines) by eliminating dead code, deduplicating the cache layer, decomposing oversized files, and consolidating scattered modules — preserving all existing behavior (one deliberate exception: player cache gains a 24h TTL, noted in Task 2.5).

**Dead code policy:** A function is NOT dead if it has callers in `#[cfg(test)]` modules — the Rust compiler treats test items as callers. Only remove functions/fields with zero callers across both production and test code. For functions marked `#[allow(dead_code)]` that ARE called, just remove the annotation.

**Architecture:** Each phase is independently shippable and testable. Phase 1 (dead code) de-noises the codebase so later phases have clear signal. Phase 2 (generic cache) yields the largest code reduction. Phases 3-6 are structural improvements that make the code easier to navigate and modify.

**Tech Stack:** Rust 2024, tokio, lru crate, crossterm

**Estimated net reduction:** ~2,500-3,000 lines (8-9% of codebase)

---

## File Structure Overview

### New files to create:
- `src/data_fetcher/cache/ttl_cache.rs` — Generic TTL-aware LRU cache wrapper (replaces 5 boilerplate files)
- `src/teletext_ui/layout/mod.rs` — Re-exports for split layout module
- `src/teletext_ui/layout/config.rs` — LayoutConfig struct and builders
- `src/teletext_ui/layout/columns.rs` — Column width calculation logic
- `src/teletext_ui/layout/ansi_cache.rs` — AnsiCodeCache (only if still needed after dead code audit)

### Files to delete:
- `src/data_fetcher/cache/player_cache.rs` — Replaced by generic cache
- `src/data_fetcher/cache/tournament_cache.rs` — Replaced by generic cache (domain logic moves to orchestrator)
- `src/data_fetcher/cache/detailed_game_cache.rs` — Replaced by generic cache
- `src/data_fetcher/cache/goal_events_cache.rs` — Replaced by generic cache
- `src/data_fetcher/cache/http_response_cache.rs` — Replaced by generic cache
- `src/data_fetcher/cache/types.rs` — TTL logic absorbed into generic cache
- `src/teletext_ui/layout.rs` — Replaced by `layout/` directory module
- `src/teletext_ui/utils.rs` — 10-line file, inline into caller
- `src/teletext_ui/mode_utils.rs` — Dead-code-heavy accessors, merge into core.rs
- `src/teletext_ui/score_formatting.rs` — Merge into `formatting.rs`

### Files to significantly modify:
- `src/data_fetcher/cache/mod.rs` — Updated re-exports for generic cache
- `src/data_fetcher/cache/core.rs` — Simplified with generic cache API
- `src/teletext_ui/mod.rs` — Updated module declarations
- `src/teletext_ui/core.rs` — Absorb mode_utils.rs methods
- `src/teletext_ui/formatting.rs` — Absorb score_formatting.rs
- `src/constants.rs` — Remove unused constants
- `src/ui/interactive/navigation_manager.rs` — Convert struct to free functions

---

## Phase 1: Dead Code Cleanup

### Task 1.1: Audit and remove unused constants

**Files:**
- Modify: `src/constants.rs`

- [ ] **Step 1: Identify which constants with `#[allow(dead_code)]` are actually used**

Run: `cargo clippy --all-features --all-targets -- -D warnings 2>&1`
Then for each constant marked `#[allow(dead_code)]` in constants.rs, grep for its usage:
```bash
# For each constant, check if it's used outside its own module
grep -rn "CONSTANT_NAME" src/ --include="*.rs" | grep -v "constants.rs" | grep -v "#\[allow"
```

- [ ] **Step 2: Remove unused constants and their `#[allow(dead_code)]` annotations**

Remove constants that have zero callers outside their definition file (and also zero callers in tests within the file). Based on exploration, these include many in `polling`, `tournament`, and `cache_ttl` modules. Keep any constant that IS referenced. **Also remove any test assertions in the same file that reference the removed constants.**

- [ ] **Step 3: For constants that ARE used, remove the `#[allow(dead_code)]` annotation**

If a constant is used but has `#[allow(dead_code)]`, just remove the annotation.

- [ ] **Step 4: Run tests**

Run: `cargo test --all-features`
Expected: All tests pass

- [ ] **Step 5: Run clippy to verify**

Run: `cargo clippy --all-features --all-targets -- -D warnings`
Expected: No warnings. If clippy now flags previously-suppressed dead code, remove that code.

- [ ] **Step 6: Commit**

```bash
git add src/constants.rs
git commit -m "refactor: remove unused constants and dead_code annotations"
```

---

### Task 1.2: Remove dead code from cache types

**Files:**
- Modify: `src/data_fetcher/cache/types.rs`
- Modify: `src/data_fetcher/cache/goal_events_cache.rs`

- [ ] **Step 1: Remove dead fields from CachedGoalEventsData**

In `types.rs`, the `CachedGoalEventsData` struct has two dead fields:
```rust
#[allow(dead_code)]
pub last_known_score: Option<String>,
#[allow(dead_code)]
pub was_cleared: bool,
```

Check if `new_cleared()` constructor (which sets these) is actually called anywhere:
```bash
grep -rn "new_cleared" src/ --include="*.rs"
```

If `new_cleared` is unused or only called from dead code, remove the fields, the constructor, and `clear_goal_events_cache_for_game()` in goal_events_cache.rs that uses it.

If it IS used, keep it but note it for Phase 2 consolidation.

- [ ] **Step 2: Remove dead `time_until_expiry()` from CachedTournamentData**

In `types.rs`, `CachedTournamentData::time_until_expiry()` is marked `#[allow(dead_code)]`. Verify no callers and remove.

- [ ] **Step 3: Audit `get_ttl()` on CachedGoalEventsData**

This method is marked `#[allow(dead_code)]` BUT is called from `get_cached_goal_events_data()` and `get_cached_goal_events_entry()` in `goal_events_cache.rs`. **Do NOT remove it** — just remove the `#[allow(dead_code)]` annotation since it IS used.

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test --all-features && cargo clippy --all-features --all-targets -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add src/data_fetcher/cache/types.rs src/data_fetcher/cache/goal_events_cache.rs
git commit -m "refactor: remove dead fields and methods from cache types"
```

---

### Task 1.3: Remove dead code from individual cache modules

**Files:**
- Modify: `src/data_fetcher/cache/player_cache.rs`
- Modify: `src/data_fetcher/cache/tournament_cache.rs`
- Modify: `src/data_fetcher/cache/detailed_game_cache.rs`
- Modify: `src/data_fetcher/cache/goal_events_cache.rs`
- Modify: `src/data_fetcher/cache/http_response_cache.rs`

- [ ] **Step 1: For each cache module, identify functions marked `#[allow(dead_code)]`**

Each cache has monitoring functions (`get_*_cache_size`, `get_*_cache_capacity`, `clear_*_cache`) that are likely unused outside tests. Grep for each:
```bash
grep -rn "get_cache_size\|get_cache_capacity\|clear_cache\|get_cached_disambiguated\|has_cached_disambiguated\|get_cached_player_name\|cache_players_with_formatting\|get_cached_tournament_data_for_auto_refresh\|get_cached_tournament_data_with_live_check\|invalidate_tournament_cache_for_date\|invalidate_cache_for_games_near_start_time\|get_cached_goal_events_entry" src/ --include="*.rs"
```

- [ ] **Step 2: Remove functions with zero external callers**

For each function that is only defined but never called (excluding its own `#[allow(dead_code)]` line), remove it. This should remove ~150-200 lines across the 5 cache files.

- [ ] **Step 3: Remove excessive doc comments on functions being kept**

The player_cache.rs functions have ~20-line doc comments with full `# Example` sections for simple cache get/put operations. Trim to 1-2 line comments. Remove the `# Example` blocks — these are internal functions, not public API.

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test --all-features && cargo clippy --all-features --all-targets -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add src/data_fetcher/cache/
git commit -m "refactor: remove dead cache functions and trim verbose docs"
```

---

### Task 1.4: Remove dead code from teletext_ui modules

**Files:**
- Modify: `src/teletext_ui/score_formatting.rs`
- Modify: `src/teletext_ui/mode_utils.rs`
- Modify: `src/teletext_ui/layout.rs` (remove `#![allow(dead_code)]` file-level suppression)
- Modify: `src/teletext_ui/wide_mode.rs`
- Modify: `src/teletext_ui/indicators.rs`

- [ ] **Step 1: Remove file-level `#![allow(dead_code)]` from layout.rs**

Line 3 of layout.rs has `#![allow(dead_code)]` which suppresses all warnings for the entire 4,109-line file. Remove it and see what clippy reports:
```bash
cargo clippy --all-features --all-targets -- -D warnings 2>&1 | grep "layout.rs"
```

- [ ] **Step 2: Remove dead functions exposed by removing the blanket allow**

Delete functions/types that clippy identifies as dead. Track what you remove — if something turns out to be needed, the compiler will tell you.

- [ ] **Step 3: Audit score_formatting.rs, mode_utils.rs, wide_mode.rs**

For each `#[allow(dead_code)]` function in these files, verify usage and remove if dead.

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test --all-features && cargo clippy --all-features --all-targets -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add src/teletext_ui/
git commit -m "refactor: remove dead code from teletext_ui modules"
```

---

### Task 1.5: Remove dead code from remaining modules

**Files:**
- Modify: `src/ui/interactive/state_manager.rs` (12 dead_code annotations)
- Modify: `src/ui/interactive/event_handler.rs` (6 dead_code annotations)
- Modify: `src/ui/teletext/page_config.rs` (5 dead_code annotations)
- Modify: `src/error.rs` (4 dead_code annotations)

- [ ] **Step 1: Audit each file's `#[allow(dead_code)]` items**

For each, grep for callers. Remove functions/fields that have no callers.

- [ ] **Step 2: Run full test suite and clippy**

Run: `cargo test --all-features && cargo clippy --all-features --all-targets -- -D warnings`

- [ ] **Step 3: Commit**

```bash
git add src/ui/ src/error.rs
git commit -m "refactor: remove dead code from UI and error modules"
```

---

## Phase 2: Generic Cache Layer

### Task 2.1: Create generic TtlCache wrapper

**Files:**
- Create: `src/data_fetcher/cache/ttl_cache.rs`

- [ ] **Step 1: Write tests for generic cache behavior**

Create tests in the new file that validate:
- Insert + get returns value
- Expired entries return None
- LRU eviction works when at capacity
- `clear()` empties cache
- `len()` returns correct count

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_insert_and_get() {
        let cache = TtlCache::<String, String>::new(10);
        cache.insert("key".into(), "value".into(), Duration::from_secs(60)).await;
        let result = cache.get(&"key".into()).await;
        assert_eq!(result, Some("value".into()));
    }

    #[tokio::test]
    async fn test_expired_entry_returns_none() {
        let cache = TtlCache::<String, String>::new(10);
        cache.insert("key".into(), "value".into(), Duration::from_millis(1)).await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        let result = cache.get(&"key".into()).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = TtlCache::<String, String>::new(10);
        cache.insert("a".into(), "1".into(), Duration::from_secs(60)).await;
        cache.insert("b".into(), "2".into(), Duration::from_secs(60)).await;
        assert_eq!(cache.len().await, 2);
        cache.clear().await;
        assert_eq!(cache.len().await, 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --all-features -- cache::ttl_cache`
Expected: Compilation error (TtlCache doesn't exist yet)

- [ ] **Step 3: Implement TtlCache**

```rust
use lru::LruCache;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

struct CacheEntry<V> {
    data: V,
    cached_at: Instant,
    ttl: Duration,
}

impl<V> CacheEntry<V> {
    fn is_expired(&self) -> bool {
        self.cached_at.elapsed() > self.ttl
    }
}

pub struct TtlCache<K: Eq + std::hash::Hash, V: Clone> {
    inner: RwLock<LruCache<K, CacheEntry<V>>>,
}

impl<K: Eq + std::hash::Hash, V: Clone> TtlCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: RwLock::new(LruCache::new(
                NonZeroUsize::new(capacity).expect("cache capacity must be > 0"),
            )),
        }
    }

    pub async fn get(&self, key: &K) -> Option<V> {
        let mut cache = self.inner.write().await;
        if let Some(entry) = cache.get(key) {
            if !entry.is_expired() {
                return Some(entry.data.clone());
            }
            // Expired — remove it
            cache.pop(key);
        }
        None
    }

    pub async fn insert(&self, key: K, value: V, ttl: Duration) {
        let entry = CacheEntry {
            data: value,
            cached_at: Instant::now(),
            ttl,
        };
        self.inner.write().await.put(key, entry);
    }

    pub async fn clear(&self) {
        self.inner.write().await.clear();
    }

    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    pub async fn capacity(&self) -> usize {
        self.inner.read().await.cap().get()
    }

    /// Gets entry only if it passes a custom freshness predicate.
    /// Used by tournament cache for aggressive TTL on starting games.
    /// The predicate receives the entry's `cached_at` timestamp.
    pub async fn get_if(&self, key: &K, predicate: impl FnOnce(Instant) -> bool) -> Option<V> {
        let mut cache = self.inner.write().await;
        if let Some(entry) = cache.get(key) {
            if !entry.is_expired() && predicate(entry.cached_at) {
                return Some(entry.data.clone());
            }
            cache.pop(key);
        }
        None
    }

    /// Removes an entry by key. Returns true if the key was present.
    pub async fn remove(&self, key: &K) -> bool {
        self.inner.write().await.pop(key).is_some()
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --all-features -- cache::ttl_cache`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add src/data_fetcher/cache/ttl_cache.rs
git commit -m "feat: add generic TtlCache wrapper for LRU + TTL caching"
```

---

### Task 2.2: Migrate HTTP response cache to TtlCache

**Files:**
- Modify: `src/data_fetcher/cache/mod.rs`
- Modify: `src/data_fetcher/api/fetch_utils.rs` (primary caller of HTTP cache functions)
- Modify: `src/data_fetcher/cache/core.rs` (update tests that reference HTTP cache)
- Delete: `src/data_fetcher/cache/http_response_cache.rs`

Start with the simplest cache (http_response_cache) to validate the approach.

- [ ] **Step 1: Find all callers of http_response_cache functions**

```bash
grep -rn "cache_http_response\|get_cached_http_response\|HTTP_RESPONSE_CACHE" src/ --include="*.rs" | grep -v "http_response_cache.rs"
```

- [ ] **Step 2: Replace http_response_cache.rs with a static TtlCache instance**

In `mod.rs`, replace the module with:
```rust
pub static HTTP_CACHE: LazyLock<TtlCache<String, String>> =
    LazyLock::new(|| TtlCache::new(100));
```

- [ ] **Step 3: Update all callers to use the new API**

Replace `cache_http_response(url, data, ttl)` → `HTTP_CACHE.insert(url, data, Duration::from_secs(ttl)).await`
Replace `get_cached_http_response(url)` → `HTTP_CACHE.get(&url).await`

- [ ] **Step 4: Delete http_response_cache.rs and CachedHttpResponse from types.rs**

- [ ] **Step 5: Run tests and clippy**

Run: `cargo test --all-features && cargo clippy --all-features --all-targets -- -D warnings`

- [ ] **Step 6: Commit**

```bash
git add src/data_fetcher/cache/
git commit -m "refactor: migrate HTTP response cache to generic TtlCache"
```

---

### Task 2.3: Migrate detailed game cache to TtlCache

**Files:**
- Modify: `src/data_fetcher/cache/mod.rs`
- Delete: `src/data_fetcher/cache/detailed_game_cache.rs`
- Modify: `src/data_fetcher/cache/types.rs`

- [ ] **Step 1: Find all callers**

```bash
grep -rn "cache_detailed_game_data\|get_cached_detailed_game_data\|DETAILED_GAME_CACHE\|create_detailed_game_key" src/ --include="*.rs" | grep -v "detailed_game_cache.rs"
```

- [ ] **Step 2: Create static instance and TTL helper**

```rust
pub static DETAILED_GAME_CACHE: LazyLock<TtlCache<String, DetailedGameResponse>> =
    LazyLock::new(|| TtlCache::new(200));

fn detailed_game_ttl(is_live: bool) -> Duration {
    if is_live {
        Duration::from_secs(cache_ttl::LIVE_GAMES_SECONDS)
    } else {
        Duration::from_secs(cache_ttl::COMPLETED_GAMES_SECONDS)
    }
}
```

- [ ] **Step 3: Update callers, delete old file and CachedDetailedGameData type**

- [ ] **Step 4: Update tests in core.rs**

`cache/core.rs` has extensive tests (~1,600 lines) that import from the individual cache modules. For each migrated cache, update the test imports and function calls to use the new API. Tests that called `cache_detailed_game_data(...)` should now call the new wrapper or use `DETAILED_GAME_CACHE.insert(...)` directly.

- [ ] **Step 5: Run tests and clippy**

- [ ] **Step 6: Commit**

```bash
git commit -m "refactor: migrate detailed game cache to generic TtlCache"
```

---

### Task 2.4: Migrate goal events cache to TtlCache

**Files:**
- Modify: `src/data_fetcher/cache/mod.rs`
- Delete: `src/data_fetcher/cache/goal_events_cache.rs`
- Modify: `src/data_fetcher/cache/types.rs`

- [ ] **Step 1: Find all callers**

- [ ] **Step 2: Create static instance with same key format**

Key format: `"goal_events_{season}_{game_id}"` — keep the same `create_goal_events_key` function.

- [ ] **Step 3: Provide thin wrapper functions if the API needs game_id/season params**

If callers use `cache_goal_events_data(season, game_id, data, is_live)`, provide a wrapper:
```rust
pub async fn cache_goal_events(season: i32, game_id: i32, data: Vec<GoalEventData>, is_live: bool) {
    let key = format!("goal_events_{season}_{game_id}");
    let ttl = game_state_ttl(is_live);
    GOAL_EVENTS_CACHE.insert(key, data, ttl).await;
}
```

- [ ] **Step 4: Delete old file and CachedGoalEventsData type**

- [ ] **Step 5: Update tests in core.rs that reference goal events cache functions**

- [ ] **Step 6: Run tests and clippy**

- [ ] **Step 7: Commit**

```bash
git commit -m "refactor: migrate goal events cache to generic TtlCache"
```

---

### Task 2.5: Migrate player cache to TtlCache

**Files:**
- Modify: `src/data_fetcher/cache/mod.rs`
- Delete: `src/data_fetcher/cache/player_cache.rs`

- [ ] **Step 1: Find all callers**

Player cache is different: `LruCache<i32, HashMap<i64, String>>` — no TTL, just LRU.

- [ ] **Step 2: Use TtlCache with effectively-infinite TTL**

**Deliberate behavioral change:** The existing player cache has no TTL (entries persist until LRU eviction). We use `Duration::MAX` to preserve this behavior as closely as possible while using the generic TtlCache.

```rust
const PLAYER_CACHE_TTL: Duration = Duration::from_secs(86400 * 365); // effectively infinite

pub static PLAYER_CACHE: LazyLock<TtlCache<i32, HashMap<i64, String>>> =
    LazyLock::new(|| TtlCache::new(100));
```

Keep `cache_players` and `get_cached_players` as thin wrappers since they're widely used. The wrappers pass `PLAYER_CACHE_TTL` as the TTL.

- [ ] **Step 3: Migrate `cache_players_with_disambiguation` — the only complex function**

This function does real work (disambiguation). Move the logic to a standalone function that calls `PLAYER_CACHE.insert()`.

- [ ] **Step 4: Delete old file**

- [ ] **Step 5: Update tests in core.rs that reference player cache functions**

- [ ] **Step 6: Run tests and clippy**

- [ ] **Step 7: Commit**

```bash
git commit -m "refactor: migrate player cache to generic TtlCache"
```

---

### Task 2.6: Migrate tournament cache and simplify core.rs

**Files:**
- Modify: `src/data_fetcher/cache/mod.rs`
- Delete: `src/data_fetcher/cache/tournament_cache.rs`
- Modify: `src/data_fetcher/cache/types.rs` (should be deletable now)
- Modify: `src/data_fetcher/cache/core.rs`
- Modify: `src/data_fetcher/api/tournament_api.rs` (primary caller)
- Modify: `src/lib.rs` (update public API re-exports)

**Important:** Tournament cache is the most complex migration because it has variable-TTL logic and custom expiration. The existing code uses `CachedTournamentData` with a `has_live_games` flag that determines the TTL, and `get_cached_tournament_data_with_start_check` applies a separate aggressive TTL for starting games.

- [ ] **Step 1: Migrate tournament cache with domain-aware wrappers**

The generic `TtlCache` sets TTL at insertion time, which works here because `has_live_games` is known at insertion. Use `get_if()` for the start-check variant:

```rust
pub static TOURNAMENT_CACHE: LazyLock<TtlCache<String, ScheduleResponse>> =
    LazyLock::new(|| TtlCache::new(50));

/// Caches tournament data with TTL based on live game state
pub async fn cache_tournament(key: String, data: ScheduleResponse, has_live_games: bool) {
    let ttl = game_state_ttl(has_live_games);
    TOURNAMENT_CACHE.insert(key, data, ttl).await;
}

/// Gets cached tournament data with aggressive TTL for games near start time.
/// Uses `get_if()` to apply a shorter freshness window when starting games exist.
pub async fn get_tournament_with_start_check(
    key: &str,
    has_starting_games: bool,
) -> Option<ScheduleResponse> {
    if has_starting_games {
        let aggressive_ttl = Duration::from_secs(cache_ttl::STARTING_GAMES_SECONDS);
        TOURNAMENT_CACHE.get_if(&key.to_string(), |cached_at| {
            cached_at.elapsed() <= aggressive_ttl
        }).await
    } else {
        TOURNAMENT_CACHE.get(&key.to_string()).await
    }
}
```

- [ ] **Step 2: Move `should_bypass_cache_for_starting_games` and `has_live_games` out of cache**

These are domain functions, not cache functions. Move to `data_fetcher/game_utils.rs` or the tournament logic module. Update imports in `tournament_api.rs`.

- [ ] **Step 3: Simplify core.rs**

With generic caches, `get_all_cache_stats` becomes:
```rust
pub async fn get_all_cache_stats() -> CacheStats {
    CacheStats {
        player_cache: CacheInfo { size: PLAYER_CACHE.len().await, capacity: PLAYER_CACHE.capacity().await },
        tournament_cache: CacheInfo { size: TOURNAMENT_CACHE.len().await, capacity: TOURNAMENT_CACHE.capacity().await },
        // ... etc
    }
}
```

Remove `get_detailed_cache_debug_info()` and `reset_all_caches_with_confirmation()` if they're dead code.

- [ ] **Step 4: Update `src/lib.rs` re-exports**

`lib.rs` lines 60-64 re-export `CacheInfo`, `CacheStats`, `clear_all_caches`, `get_all_cache_stats`, `get_detailed_cache_debug_info`, and `reset_all_caches_with_confirmation` as public API. If any of these are removed, update the `pub use` block in `lib.rs` to match. This is a public API change — ensure nothing external depends on removed functions.

- [ ] **Step 5: Delete types.rs if all wrapper types are gone**

- [ ] **Step 6: Update tests in core.rs that reference tournament cache functions**

- [ ] **Step 7: Run tests and clippy**

- [ ] **Step 8: Commit**

```bash
git commit -m "refactor: migrate tournament cache and simplify cache core"
```

---

## Phase 3: layout.rs Decomposition

### Task 3.1: Split layout.rs into focused modules

**Files:**
- Delete: `src/teletext_ui/layout.rs` (4,109 lines)
- Create: `src/teletext_ui/layout/mod.rs`
- Create: `src/teletext_ui/layout/config.rs`
- Create: `src/teletext_ui/layout/columns.rs`
- Create: `src/teletext_ui/layout/ansi_cache.rs` (only if still alive after Phase 1)
- Modify: `src/teletext_ui/mod.rs`

- [ ] **Step 1: Map the current layout.rs structure**

After Phase 1 dead code removal, re-read layout.rs and categorize remaining code into:
1. **LayoutConfig struct + builders** → `config.rs`
2. **Column width calculation functions** → `columns.rs`
3. **AnsiCodeCache** → `ansi_cache.rs` (if still needed)
4. **ColumnLayoutManager** → `mod.rs` (thin coordinator)

- [ ] **Step 2: Delete layout.rs BEFORE creating layout/ directory**

**Critical ordering:** Rust cannot have both `layout.rs` (file module) and `layout/` (directory module) simultaneously — the compiler will reject it. Delete `layout.rs` first, then create the directory.

```bash
# Save contents, delete file, create directory
cp src/teletext_ui/layout.rs src/teletext_ui/layout.rs.bak
rm src/teletext_ui/layout.rs
mkdir src/teletext_ui/layout
```

- [ ] **Step 3: Create layout/mod.rs with re-exports**

```rust
// src/teletext_ui/layout/mod.rs
mod config;
mod columns;

pub use config::*;
pub use columns::*;

// Re-export ColumnLayoutManager
```

- [ ] **Step 4: Move LayoutConfig and related types to config.rs**

Move the LayoutConfig struct, its Default impl, and builder methods.

- [ ] **Step 5: Move column calculation functions to columns.rs**

Move `calculate_*` functions, width computation, and dynamic column logic.

- [ ] **Step 6: Move AnsiCodeCache to ansi_cache.rs (if alive)**

If AnsiCodeCache survived Phase 1 dead code removal, give it its own file. If it was removed, skip this.

- [ ] **Step 7: Update mod.rs coordinator and re-exports**

Ensure `ColumnLayoutManager` still works. The `teletext_ui/mod.rs` declaration `pub mod layout;` works for both file and directory modules — no change needed there.

- [ ] **Step 8: Run tests and clippy**

Run: `cargo test --all-features && cargo clippy --all-features --all-targets -- -D warnings`

- [ ] **Step 9: Clean up and commit**

Remove the `.bak` file and commit:
```bash
rm -f src/teletext_ui/layout.rs.bak
git add src/teletext_ui/layout/ src/teletext_ui/mod.rs
git commit -m "refactor: decompose layout.rs into focused modules"
```

---

## Phase 4: teletext_ui Module Consolidation

### Task 4.1: Eliminate utils.rs (10 lines, 1 function)

**Files:**
- Delete: `src/teletext_ui/utils.rs`
- Modify: `src/teletext_ui/core.rs`
- Modify: `src/teletext_ui/mod.rs`
- Modify: All callers (likely 6+: `core.rs`, `formatting.rs`, `footer.rs`, `game_display.rs`, `rendering.rs`, `standings_display.rs`)

- [ ] **Step 1: Find all callers of `get_ansi_code`**

```bash
grep -rn "get_ansi_code\|super::utils" src/teletext_ui/ --include="*.rs"
```

Note: `get_ansi_code` has 6+ callers across teletext_ui — it cannot be inlined into a single file.

- [ ] **Step 2: Move the function to core.rs as `pub(super)`**

Move `get_ansi_code` to `core.rs` with `pub(super)` visibility. Then update all import paths from `use super::utils::get_ansi_code` to `use super::core::get_ansi_code` (or just `use super::get_ansi_code` since core.rs is re-exported via `pub use core::*` in mod.rs).

- [ ] **Step 3: Remove utils.rs and its module declaration**

- [ ] **Step 4: Run tests and clippy**

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor: inline teletext_ui/utils.rs into caller"
```

---

### Task 4.2: Merge score_formatting.rs into formatting.rs

**Files:**
- Delete: `src/teletext_ui/score_formatting.rs`
- Modify: `src/teletext_ui/formatting.rs`
- Modify: `src/teletext_ui/mod.rs`

- [ ] **Step 1: Check if score_formatting.rs still has content after Phase 1**

Every function in `score_formatting.rs` is marked `#[allow(dead_code)]`. Phase 1 Task 1.4 may have already removed all of them. If the file is empty or has only dead code remaining:
- Just delete the file and its module declaration. Skip to Step 4.

If functions survived Phase 1:

- [ ] **Step 2: Read both files and confirm they're related**

Both deal with formatting display strings. They're the same concern — merge them.

- [ ] **Step 3: Move surviving functions from score_formatting.rs into formatting.rs**

- [ ] **Step 3: Update imports across the codebase**

```bash
grep -rn "score_formatting" src/ --include="*.rs"
```

Update any `use super::score_formatting::` to point to `formatting::`.

- [ ] **Step 4: Delete score_formatting.rs and its module declaration**

- [ ] **Step 5: Run tests and clippy**

- [ ] **Step 6: Commit**

```bash
git commit -m "refactor: merge score_formatting into formatting module"
```

---

### Task 4.3: Merge mode_utils.rs into core.rs

**Files:**
- Delete: `src/teletext_ui/mode_utils.rs`
- Modify: `src/teletext_ui/core.rs`
- Modify: `src/teletext_ui/mod.rs`

- [ ] **Step 1: Check what survived Phase 1 dead code cleanup in mode_utils.rs**

If most methods were removed as dead code, the remainder should be small enough to inline into core.rs.

- [ ] **Step 2: Move surviving methods into the TeletextPage impl block in core.rs**

- [ ] **Step 3: Delete mode_utils.rs and its module declaration**

- [ ] **Step 4: Run tests and clippy**

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor: merge mode_utils into teletext_ui/core"
```

---

## Phase 5: NavigationManager Simplification

### Task 5.1: Convert NavigationManager from struct to module functions

**Files:**
- Modify: `src/ui/interactive/navigation_manager.rs`
- Modify: `src/ui/interactive/refresh_coordinator.rs`

- [ ] **Step 1: Verify NavigationManager is stateless**

```bash
grep -n "struct NavigationManager" src/ui/interactive/navigation_manager.rs
```

Confirm it has no fields (just `pub struct NavigationManager;`).

- [ ] **Step 2: Convert all `impl NavigationManager` methods to free functions**

Change:
```rust
impl NavigationManager {
    pub fn create_page(&self, config: PageCreationConfig<'_>) -> TeletextPage { ... }
}
```
To:
```rust
pub fn create_page(config: PageCreationConfig<'_>) -> TeletextPage { ... }
```

- [ ] **Step 3: Update all call sites**

In refresh_coordinator.rs and any other callers, change:
```rust
self.navigation_manager.create_page(config)
```
To:
```rust
navigation_manager::create_page(config)
```

Remove `NavigationManager::new()` calls and stored fields.

- [ ] **Step 4: Run tests and clippy**

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor: convert NavigationManager from stateless struct to module functions"
```

---

## Phase 6: Verbose Logging Reduction

### Task 6.1: Reduce excessive debug logging in cache and data modules

**Files:**
- Modify: Various files in `src/data_fetcher/cache/` (post-Phase 2 versions)
- Modify: `src/data_fetcher/api/game_api.rs`

- [ ] **Step 1: Identify log-heavy patterns**

Many functions log on entry AND exit with nearly identical messages:
```rust
debug!("Attempting to retrieve X for key: {}", key);
// ... 3 lines of actual logic ...
debug!("Cache hit for X: key={}, age={:?}", key, age);
```

- [ ] **Step 2: Apply logging guidelines**

- Keep `info!` for cache state changes (entry created, expired, bypassed)
- Keep `debug!` for cache misses (useful for debugging)
- Remove redundant entry/exit logging pairs — keep only the meaningful one
- Remove `#[instrument]` from trivial functions (single hashmap lookup)

- [ ] **Step 3: Run tests**

- [ ] **Step 4: Commit**

```bash
git commit -m "refactor: reduce verbose logging in cache and data modules"
```

---

## Validation Checklist (run after each phase)

After completing each phase:

- [ ] `cargo fmt`
- [ ] `cargo clippy --all-features --all-targets -- -D warnings` — zero warnings
- [ ] `cargo test --all-features` — all tests pass
- [ ] `cargo build --release` — builds successfully
- [ ] Verify no new `#[allow(dead_code)]` annotations were introduced
