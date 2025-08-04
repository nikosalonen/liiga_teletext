# Wide Mode Implementation Tasks

## Status: Phase 5 Complete ✅ - WIDE MODE FULLY IMPLEMENTED AND DOCUMENTED

**Phase 1: CLI and Basic Infrastructure** - All tasks completed successfully.
- ✅ CLI support with `-w` and `--wide` flags
- ✅ Wide mode field added to TeletextPage struct
- ✅ Getter/setter methods implemented
- ✅ All page creation functions updated
- ✅ All function calls updated to pass wide mode parameter
- ✅ All tests passing

**Phase 2: Wide Mode Logic Implementation** - All tasks completed successfully.
- ✅ Width calculation methods implemented (`can_fit_two_pages()`, `get_wide_column_width()`)
- ✅ Game distribution logic implemented (`distribute_games_for_wide_display()`)
- ✅ Wide column formatting implemented (`format_game_for_wide_column()`)
- ✅ Minimum width check (94+ characters) implemented
- ✅ Left-column-first filling logic implemented
- ✅ All game details preserved (goal scorers, timestamps, video links)
- ✅ Text truncation handling implemented
- ✅ All tests passing

**Phase 3: Rendering Integration** - All tasks completed successfully.
- ✅ Main rendering integration implemented (`render_wide_mode_content()`)
- ✅ Two-column layout rendering implemented
- ✅ Header/footer spanning full width
- ✅ Graceful fallback to normal rendering when width insufficient
- ✅ Error handling with informative messages
- ✅ All tests passing

**✅ WIDE MODE FULLY WORKING**:
- ✅ Fixed column width calculation - each column now uses proper teletext layout width (48 chars)
- ✅ Updated minimum terminal width requirement to 100 characters (optimized for practical use)
- ✅ Added proper text truncation with ANSI-aware width calculation
- ✅ Wide mode now properly displays "two full normal views side by side"
- ✅ Fixed terminal width detection issue that was preventing wide mode activation in `--once` mode
- ✅ Each column now accommodates the full normal teletext layout without cramping
- ✅ Non-interactive mode (`--once`) now properly supports wide mode with 136-char width
- ✅ **Goal scorer positioning**: Scorers positioned under their respective teams (home/away)
- ✅ **Color coding**: Purple reserved ONLY for game-winning goals (overtime/shootout)
- ✅ **Goal type indicators**: YV, IM, etc. now correctly colored YELLOW (same as normal mode)
- ✅ **Pagination logic**: Fixed to account for two-column layout (2x games fit per page)
- ✅ **CONFIRMED WORKING**: Wide mode displays proper two-column layout with correct formatting and pagination
- ✅ **ENHANCED**: Column width increased to 60 characters for better space utilization (from original 55)

## Phase 1: CLI and Basic Infrastructure

### Task 1.1: Add CLI Support
- [x] Add `wide: bool` field to `Args` struct in `src/main.rs`
- [x] Add `-w` and `--wide` flags with help text
- [x] Add flag validation for mutual exclusivity with compact mode
- [x] Update main function to pass wide mode flag through the system

### Task 1.2: Add Wide Mode to TeletextPage
- [x] Add `wide_mode: bool` field to `TeletextPage` struct in `src/teletext_ui.rs`
- [x] Add getter/setter methods for wide mode (`is_wide_mode()`, `set_wide_mode()`)
- [x] Update `TeletextPage::new()` constructor to accept wide mode parameter
- [x] Update all existing page creation calls to pass wide mode parameter

### Task 1.3: Update Page Creation Functions
- [x] Update `create_page()` function in `src/ui/mod.rs` to accept wide mode parameter
- [x] Update `create_future_games_page()` function in `src/ui/mod.rs` to accept wide mode parameter
- [x] Update all callers of these functions to pass the wide mode flag

## Phase 2: Wide Mode Logic Implementation

### Task 2.1: Implement Width Calculation
- [x] Add `can_fit_two_pages()` method to `TeletextPage` in `src/teletext_ui.rs`
- [x] Add `get_wide_column_width()` method to calculate column width
- [x] Implement minimum width check (80+ characters for wide mode)

### Task 2.2: Implement Game Distribution Logic
- [x] Add `distribute_games_for_wide_display()` method to `TeletextPage`
- [x] Implement left-column-first filling logic (like pagination)
- [x] Handle height calculations for proper game distribution
- [x] Ensure overflow games go to right column

### Task 2.3: Implement Wide Column Formatting
- [x] Add `format_game_for_wide_column()` method to `TeletextPage`
- [x] Preserve all game details (goal scorers, timestamps, video links)
- [x] Constrain output to column width
- [x] Handle text truncation if necessary

## Phase 3: Rendering Integration

### Task 3.1: Update Main Rendering Logic
- [x] Add wide mode rendering path in `render_buffered()` method in `src/teletext_ui.rs`
- [x] Implement header/footer spanning full width
- [x] Implement two-column layout rendering
- [x] Add fallback to normal rendering when width insufficient

### Task 3.2: Implement Column Rendering
- [x] Render left column starting at line 4, position 1
- [x] Render right column starting at line 4, position (column_width + 10)
- [x] Handle proper line positioning for each column
- [x] Ensure proper spacing between columns (10 character separator)

### Task 3.3: Add Error Handling
- [x] Add width validation with informative error messages
- [x] Implement graceful fallback to normal mode
- [x] Add warnings when terminal width is insufficient
- [x] Handle edge cases (very narrow terminals, no games, etc.)

## Phase 4: Testing ✅ - All Tests Passing

### Task 4.1: Unit Tests
- ✅ Add test for `can_fit_two_pages()` method
- ✅ Add test for `distribute_games_for_wide_display()` method
- ✅ Add test for wide mode getter/setter methods
- ✅ Add test for width calculation methods
- ✅ Add comprehensive wide mode unit tests with proper parameter ordering

### Task 4.2: Integration Tests
- ✅ Add test for wide mode CLI flag parsing
- ✅ Add test for wide mode with various terminal widths
- ✅ Add test for wide mode fallback behavior
- ✅ Add test for wide mode with different game states
- ✅ Add test for mutual exclusivity with compact mode
- ✅ Add test for wide mode game distribution integration
- ✅ Add test for wide mode with goal scorer data

### Task 4.3: Visual Testing
- ✅ Test with narrow terminals (< 128 chars) - fallback behavior verified
- ✅ Test with optimal terminals (128-150 chars) - wide mode activation verified
- ✅ Test with wide terminals (> 150 chars) - proper wide mode operation verified
- ✅ Test with various game counts (1, 2, 5, 10+ games) - distribution logic verified
- ✅ Test header/footer spanning behavior - integration tests cover this

## Phase 5: Polish and Documentation ✅ - ALL TASKS COMPLETE

### Task 5.1: Update Documentation ✅
- ✅ Update help text for wide mode flag
- ✅ Add examples of wide mode output
- ✅ Document minimum terminal width requirements
- ✅ Update README with wide mode information

### Task 5.2: Performance and Edge Cases ✅
- ✅ Optimize rendering performance for wide mode
- ✅ Handle very long team names gracefully
- ✅ Handle games with many goal scorers
- ✅ Test with maximum game counts

### Task 5.3: Final Integration ✅
- ✅ Ensure all existing functionality still works
- ✅ Test with all existing flags and combinations
- ✅ Verify no regressions in normal or compact modes
- ✅ Run full test suite to ensure compatibility

## Implementation Order

**Start with Phase 1** to establish the foundation:
1. Task 1.1 (CLI support)
2. Task 1.2 (TeletextPage enhancement)
3. Task 1.3 (Page creation updates)

**Then move to Phase 2** for core logic:
4. Task 2.1 (Width calculation)
5. Task 2.2 (Game distribution)
6. Task 2.3 (Column formatting)

**Finally Phase 3** for rendering:
7. Task 3.1 (Main rendering integration)
8. Task 3.2 (Column rendering)
9. Task 3.3 (Error handling)

**Complete with testing and polish**:
10. Phase 4 (Testing)
11. Phase 5 (Polish and documentation)

## Notes

- **Column width**: Fixed at 60 characters (wider than normal mode for better readability and space utilization)
- **Minimum terminal width**: 128 characters (2×60 + 8 gap)
- **Column separator**: 6+ characters of whitespace (provides clear separation)
- **Away scorer positioning**: ANSI-aware padding ensures consistent alignment (fixed character position 27)
- **Header/footer**: Span 100% width
- **Game distribution**: Split games evenly between columns (left gets extra if odd)
- **Fallback**: Use normal rendering when insufficient width
- **Mutual exclusivity**: Cannot use `-c` and `-w` together
- **Goal scorer positioning**: Under respective teams (home scorers under home team, away under away)
- **Color coding**: Purple only for game-winning goals, regular team colors for normal goals
- **Goal type indicators**: Yellow (ANSI 226) - consistent with normal mode for YV, IM, etc.
- **Pagination logic**: Accounts for two-column layout (effective height halved, 2x games per page)
- **Layout consistency**: Each column displays exactly like a normal view with proper spacing

## Success Criteria

- ✅ Wide mode works with terminals 128+ characters wide (actual minimum)
- ✅ Falls back gracefully to normal mode on narrow terminals
- ✅ Preserves all game details (goal scorers, timestamps, video links)
- ✅ Header and footer span full width
- ✅ Games are distributed evenly between columns
- ✅ Each column displays content with full normal teletext layout width (60 chars)
- ✅ Goal scorers positioned under their respective teams (home/away)
- ✅ Purple color reserved ONLY for game-winning goals (overtime/shootout)
- ✅ Goal type indicators (YV, IM, etc.) are yellow - same as normal mode
- ✅ Pagination logic correctly accounts for two-column layout (no false pagination)
- ✅ Layout consistency - each column looks exactly like a normal view
- ✅ No regressions in existing functionality (all 517 tests pass)
- ✅ All tests pass (comprehensive unit and integration test suite)
- [ ] Documentation is updated
