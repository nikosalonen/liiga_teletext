# Liiga Teletext Refactoring Plan

## Executive Summary

The codebase has grown to **~22,665 lines** with significant bloat in key files. This plan outlines a systematic approach to modularize the code, improve maintainability, and reduce complexity.

## Current State Analysis

### File Size Breakdown
```
Total Lines: 22,665

Top Bloated Files:
1. teletext_ui.rs           - 4,675 lines (UI rendering logic)
2. data_fetcher/api.rs      - 4,537 lines (API calls, data fetching)
3. data_fetcher/cache.rs    - 3,282 lines (Caching logic)
4. data_fetcher/player_names.rs - 2,388 lines (Player name handling)
5. ui/interactive.rs        - 2,181 lines (Interactive UI logic)
6. data_fetcher/processors.rs - 1,350 lines (Data processing)
7. data_fetcher/models.rs   - 1,033 lines (Data models)
8. config.rs                - 931 lines (Configuration)
9. error.rs                 - 740 lines (Error handling)
10. main.rs                 - 614 lines (CLI entry point)
```

### Key Issues Identified

1. **Monolithic Files**: Several files exceed 1,000 lines
2. **Mixed Responsibilities**: Single files handling multiple concerns
3. **Tight Coupling**: Heavy interdependencies between modules
4. **Test Complexity**: Integration tests mixing multiple concerns (3,951 lines)
5. **Duplication**: Similar logic scattered across files

## Refactoring Strategy

### Phase 1: Split Monolithic UI Files (Priority: HIGH)

#### 1.1 Split `teletext_ui.rs` (4,675 lines)
**Target Structure:**
```
src/ui/
├── mod.rs                    (re-exports)
├── interactive.rs            (existing)
├── teletext/
│   ├── mod.rs               (main TeletextPage)
│   ├── page.rs              (page rendering logic)
│   ├── header.rs            (header rendering)
│   ├── footer.rs            (footer rendering)
│   ├── game_display.rs      (game result display)
│   ├── compact_display.rs   (compact mode)
│   ├── wide_display.rs      (wide mode)
│   ├── colors.rs            (color constants)
│   └── formatting.rs        (text formatting utilities)
└── components/
    ├── scoreboard.rs        (score display)
    ├── goal_events.rs       (goal event rendering)
    └── abbreviations.rs     (team abbreviations)
```

**Benefits:**
- Each file <500 lines
- Clear separation of concerns
- Easier testing of individual components
- Better code navigation

#### 1.2 Split `ui/interactive.rs` (2,181 lines)
**Target Structure:**
```
src/ui/interactive/
├── mod.rs                   (main UI loop)
├── event_handler.rs         (keyboard event handling)
├── state_manager.rs         (UI state management)
├── refresh_logic.rs         (auto-refresh logic)
├── navigation.rs            (page/date navigation)
└── terminal_setup.rs        (terminal initialization)
```

### Phase 2: Modularize Data Fetching Layer (Priority: HIGH)

#### 2.1 Split `data_fetcher/api.rs` (4,537 lines)
**Target Structure:**
```
src/data_fetcher/
├── mod.rs
├── api/
│   ├── mod.rs               (main fetch functions)
│   ├── client.rs            (HTTP client setup)
│   ├── urls.rs              (URL builders)
│   ├── fetch.rs             (generic fetch function)
│   ├── tournament.rs        (tournament data fetching)
│   ├── game_details.rs      (detailed game fetching)
│   ├── schedule.rs          (schedule fetching)
│   └── date_logic.rs        (date determination logic)
├── cache.rs                 (existing, needs splitting too)
├── models.rs                (existing)
├── processors.rs            (existing)
└── player_names.rs          (existing, needs splitting)
```

**Benefits:**
- Single Responsibility Principle per file
- Testable HTTP client configuration
- Isolated URL building logic
- Better error handling per endpoint

#### 2.2 Split `data_fetcher/cache.rs` (3,282 lines)
**Target Structure:**
```
src/data_fetcher/cache/
├── mod.rs                   (public API)
├── types.rs                 (cache data structures)
├── tournament_cache.rs      (tournament caching)
├── game_cache.rs            (game detail caching)
├── goal_events_cache.rs     (goal events caching)
├── player_cache.rs          (player data caching)
├── http_cache.rs            (HTTP response caching)
├── ttl.rs                   (TTL logic)
└── stats.rs                 (cache statistics)
```

**Benefits:**
- Cache types isolated
- Easier to test individual caches
- Better performance monitoring per cache
- Simpler debugging

#### 2.3 Split `data_fetcher/player_names.rs` (2,388 lines)
**Target Structure:**
```
src/data_fetcher/players/
├── mod.rs                   (main API)
├── disambiguation.rs        (name disambiguation logic)
├── formatting.rs            (display name formatting)
├── roster_data.rs           (roster data structures)
└── initials.rs              (initial generation)
```

#### 2.4 Refactor `data_fetcher/processors.rs` (1,350 lines)
**Target Structure:**
```
src/data_fetcher/processors/
├── mod.rs                   (public API)
├── game_status.rs           (game status determination)
├── goal_events.rs           (goal event processing)
├── time_formatting.rs       (time formatting)
└── tournament_logic.rs      (tournament type logic)
```

### Phase 3: Simplify Configuration & Error Handling (Priority: MEDIUM)

#### 3.1 Refactor `config.rs` (931 lines)
**Target Structure:**
```
src/config/
├── mod.rs                   (Config struct)
├── loader.rs                (loading config)
├── saver.rs                 (saving config)
├── paths.rs                 (platform-specific paths)
├── validation.rs            (config validation)
└── prompts.rs               (user prompts)
```

#### 3.2 Refactor `error.rs` (740 lines)
**Current size is reasonable, but could be improved:**
```
src/error/
├── mod.rs                   (AppError enum)
├── types.rs                 (error type definitions)
└── formatting.rs            (error display implementations)
```

### Phase 4: Simplify Main Entry Point (Priority: MEDIUM)

#### 4.1 Refactor `main.rs` (614 lines)
**Target Structure:**
```
src/
├── main.rs                  (minimal - just call run())
├── cli/
│   ├── mod.rs              (Args struct, CLI parsing)
│   ├── commands.rs         (command handlers)
│   ├── version.rs          (version checking)
│   └── logging.rs          (logging setup)
└── app.rs                  (main app logic)
```

### Phase 5: Modularize Testing (Priority: LOW)

#### 5.1 Split Test Files
**Target Structure:**
```
tests/
├── common/                  (shared test utilities)
│   ├── mod.rs
│   ├── fixtures.rs         (test data)
│   └── helpers.rs          (test helpers)
├── integration/
│   ├── api_tests.rs
│   ├── cache_tests.rs
│   └── ui_tests.rs
├── disambiguation/
│   ├── display_tests.rs    (existing, split further)
│   └── integration_tests.rs (existing, split further)
└── e2e/
    └── full_flow_tests.rs
```

## Implementation Order

### Sprint 1: Core Refactoring (2-3 days)
1. ✅ Create directory structure
2. ✅ Split `teletext_ui.rs` into modules
3. ✅ Split `data_fetcher/api.rs` into modules
4. ✅ Update imports throughout codebase
5. ✅ Run tests to ensure no breakage

### Sprint 2: Cache & Data Processing (2-3 days)
1. ✅ Split `cache.rs` into modules
2. ✅ Split `player_names.rs` into modules
3. ✅ Split `processors.rs` into modules
4. ✅ Update tests
5. ✅ Run full test suite

### Sprint 3: Configuration & Entry Point (1-2 days)
1. ✅ Split `config.rs` into modules
2. ✅ Refactor `main.rs` to be minimal
3. ✅ Split error handling if needed
4. ✅ Update documentation

### Sprint 4: Interactive UI (1-2 days)
1. ✅ Split `ui/interactive.rs` into modules
2. ✅ Test interactive mode thoroughly
3. ✅ Update user-facing documentation

### Sprint 5: Testing & Polish (1-2 days)
1. ✅ Reorganize test files
2. ✅ Add unit tests for new modules
3. ✅ Update CI/CD if needed
4. ✅ Update CONTRIBUTING.md

## Expected Outcomes

### File Size Reduction
```
Before:  ~22,665 lines total
         - Largest file: 4,675 lines
         - Files >1000 lines: 7 files

After:   ~22,665 lines total (same logic)
         - Largest file: <800 lines (target)
         - Files >1000 lines: 0 files
         - Average file size: 200-400 lines
```

### Code Quality Improvements
- ✅ Better adherence to Single Responsibility Principle
- ✅ Improved testability (unit tests per module)
- ✅ Easier code navigation
- ✅ Reduced cognitive load
- ✅ Better documentation per module
- ✅ Clearer module boundaries

### Maintainability Improvements
- ✅ Easier onboarding for new contributors
- ✅ Faster bug location and fixing
- ✅ Simpler to add new features
- ✅ Better separation of concerns
- ✅ Reduced merge conflicts

## Risk Mitigation

### Risks
1. **Breaking existing tests**: Run tests after each split
2. **Import complexity**: Use `pub use` for backward compatibility
3. **Performance impact**: None expected (no logic changes)
4. **Merge conflicts**: Do this work in a dedicated branch

### Mitigation Strategy
1. Create comprehensive test suite before starting
2. Split one module at a time
3. Keep backward-compatible re-exports
4. Use feature flags for gradual rollout if needed
5. Document all changes in CHANGELOG.md

## Success Metrics

- [ ] All files under 800 lines
- [ ] No files over 1,000 lines
- [ ] 100% test pass rate maintained
- [ ] No performance degradation
- [ ] CI/CD passes
- [ ] Documentation updated
- [ ] Code review approved

## Dependencies & Tools

### Development Tools
- `cargo fmt` - Code formatting
- `cargo clippy` - Linting
- `cargo test` - Testing
- `cargo modules` - Module visualization (optional)
- `tokei` - Line counting

### Pre-requisites
- All tests passing
- Clean `git status`
- Feature branch created
- Team alignment on approach

## Timeline

**Total Estimated Time: 7-12 days**

- Sprint 1: 2-3 days (Core UI + API)
- Sprint 2: 2-3 days (Cache + Data)
- Sprint 3: 1-2 days (Config + Main)
- Sprint 4: 1-2 days (Interactive UI)
- Sprint 5: 1-2 days (Testing + Polish)

## Next Steps

1. **Review this plan** with the team
2. **Create feature branch**: `refactor/modularize-codebase`
3. **Start with Sprint 1**: Begin with `teletext_ui.rs`
4. **Incremental commits**: Commit after each file split
5. **Continuous testing**: Run tests after each change

## Notes

- This refactoring is **non-functional** - no behavior changes
- Focus on **structure**, not new features
- Keep **backward compatibility** where possible
- Use **pub use** liberally for smooth transition
- Document **architectural decisions** in code comments
- Consider creating **architecture diagrams** after refactoring

---

**Version:** 1.0  
**Date:** 2025-09-30  
**Author:** Code Refactoring Analysis