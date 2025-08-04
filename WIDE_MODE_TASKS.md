# Wide Mode Implementation Tasks

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
- [ ] Add `can_fit_two_pages()` method to `TeletextPage` in `src/teletext_ui.rs`
- [ ] Add `get_wide_column_width()` method to calculate column width
- [ ] Implement minimum width check (80+ characters for wide mode)

### Task 2.2: Implement Game Distribution Logic
- [ ] Add `distribute_games_for_wide_display()` method to `TeletextPage`
- [ ] Implement left-column-first filling logic (like pagination)
- [ ] Handle height calculations for proper game distribution
- [ ] Ensure overflow games go to right column

### Task 2.3: Implement Wide Column Formatting
- [ ] Add `format_game_for_wide_column()` method to `TeletextPage`
- [ ] Preserve all game details (goal scorers, timestamps, video links)
- [ ] Constrain output to column width
- [ ] Handle text truncation if necessary

## Phase 3: Rendering Integration

### Task 3.1: Update Main Rendering Logic
- [ ] Add wide mode rendering path in `render_buffered()` method in `src/teletext_ui.rs`
- [ ] Implement header/footer spanning full width
- [ ] Implement two-column layout rendering
- [ ] Add fallback to normal rendering when width insufficient

### Task 3.2: Implement Column Rendering
- [ ] Render left column starting at line 4, position 1
- [ ] Render right column starting at line 4, position (column_width + 10)
- [ ] Handle proper line positioning for each column
- [ ] Ensure proper spacing between columns (10 character separator)

### Task 3.3: Add Error Handling
- [ ] Add width validation with informative error messages
- [ ] Implement graceful fallback to normal mode
- [ ] Add warnings when terminal width is insufficient
- [ ] Handle edge cases (very narrow terminals, no games, etc.)

## Phase 4: Testing

### Task 4.1: Unit Tests
- [ ] Add test for `can_fit_two_pages()` method
- [ ] Add test for `distribute_games_for_wide_display()` method
- [ ] Add test for `format_game_for_wide_column()` method
- [ ] Add test for wide mode getter/setter methods
- [ ] Add test for width calculation methods

### Task 4.2: Integration Tests
- [ ] Add test for wide mode CLI flag parsing
- [ ] Add test for wide mode with various terminal widths
- [ ] Add test for wide mode fallback behavior
- [ ] Add test for wide mode with different game states
- [ ] Add test for mutual exclusivity with compact mode

### Task 4.3: Visual Testing
- [ ] Test with narrow terminals (< 80 chars)
- [ ] Test with optimal terminals (80-120 chars)
- [ ] Test with wide terminals (> 120 chars)
- [ ] Test with various game counts (1, 2, 5, 10+ games)
- [ ] Test header/footer spanning behavior

## Phase 5: Polish and Documentation

### Task 5.1: Update Documentation
- [ ] Update help text for wide mode flag
- [ ] Add examples of wide mode output
- [ ] Document minimum terminal width requirements
- [ ] Update README with wide mode information

### Task 5.2: Performance and Edge Cases
- [ ] Optimize rendering performance for wide mode
- [ ] Handle very long team names gracefully
- [ ] Handle games with many goal scorers
- [ ] Test with maximum game counts

### Task 5.3: Final Integration
- [ ] Ensure all existing functionality still works
- [ ] Test with all existing flags and combinations
- [ ] Verify no regressions in normal or compact modes
- [ ] Run full test suite to ensure compatibility

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

- **Default wide width**: 2x normal width (minimum 80 characters)
- **Column separator**: 10 characters of whitespace
- **Header/footer**: Span 100% width
- **Game distribution**: Fill left column first, then right column
- **Fallback**: Use normal rendering when insufficient width
- **Mutual exclusivity**: Cannot use `-c` and `-w` together

## Success Criteria

- [ ] Wide mode works with terminals 80+ characters wide
- [ ] Falls back gracefully to normal mode on narrow terminals
- [ ] Preserves all game details (goal scorers, timestamps, video links)
- [ ] Header and footer span full width
- [ ] Games are distributed left-column-first
- [ ] No regressions in existing functionality
- [ ] All tests pass
- [ ] Documentation is updated
