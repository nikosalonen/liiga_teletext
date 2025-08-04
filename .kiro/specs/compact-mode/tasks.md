# Implementation Plan

- [x] 1. Add CLI flag support for compact mode
  - Add `compact: bool` field to Args struct in src/main.rs
  - Configure clap derive attributes for `-c` and `--compact` flags
  - Update help text to describe compact mode functionality
  - _Requirements: 1.1, 1.2_

- [x] 2. Implement team name abbreviation system
  - [x] 2.1 Create team abbreviation mapping function
    - Write `get_team_abbreviation()` function with comprehensive team name mappings
    - Include fallback logic for unknown team names
    - Add unit tests for abbreviation mapping
    - _Requirements: 3.3_

  - [x] 2.2 Create compact display configuration
    - Define `CompactDisplayConfig` struct with layout parameters
    - Implement default configuration for optimal display
    - Add methods for terminal width adaptation
    - _Requirements: 3.4, 3.5_

- [ ] 3. Extend TeletextPage for compact mode support
  - [x] 3.1 Add compact mode field to TeletextPage struct
    - Add `compact_mode: bool` field to TeletextPage
    - Update constructor to accept compact mode parameter
    - Add getter and setter methods for compact mode
    - _Requirements: 1.1, 1.2_

  - [x] 3.2 Implement compact game result rendering
    - Modify `render_game_result()` method to support compact format
    - Implement multi-game-per-line layout logic
    - Preserve teletext colors and styling in compact format
    - _Requirements: 1.4, 3.1, 3.2, 3.4_

  - [x] 3.3 Add terminal width adaptation logic
    - Calculate optimal games per line based on terminal width
    - Implement graceful fallback for narrow terminals
    - Add responsive layout adjustments
    - _Requirements: 3.5_

- [x] 4. Update page creation functions
  - [x] 4.1 Modify create_base_page function
    - Add compact mode parameter to create_base_page function
    - Pass compact mode flag to TeletextPage constructor
    - Ensure compact mode works with existing page features
    - _Requirements: 1.1, 1.2_

  - [x] 4.2 Update create_page and create_future_games_page functions
    - Add compact mode parameter to both functions
    - Propagate compact mode flag through function calls
    - Maintain backward compatibility with existing callers
    - _Requirements: 1.1, 1.2_

- [x] 5. Integrate compact mode into main application flow
  - [x] 5.1 Update main.rs to handle compact flag
    - Extract compact flag from parsed arguments
    - Pass compact mode to page creation functions
    - Ensure compact mode works in both interactive and non-interactive modes
    - _Requirements: 1.1, 1.2, 2.1, 2.2_

  - [x] 5.2 Update interactive UI to support compact mode
    - Modify run_interactive_ui function to accept compact parameter
    - Pass compact mode through to page creation calls
    - Ensure compact mode preserves all interactive features
    - _Requirements: 2.1, 2.3_

- [x] 6. Implement compact rendering logic
  - [x] 6.1 Create compact game formatting functions
    - Write function to format single game in compact format
    - Implement logic to group multiple games per line
    - Add proper spacing and alignment for compact display
    - _Requirements: 3.1, 3.2, 3.4_

  - [x] 6.2 Handle different game states in compact mode
    - Format live games with appropriate indicators
    - Display final scores clearly in compact format
    - Show upcoming games with start times
    - _Requirements: 4.3, 5.2, 5.3, 5.4_

  - [x] 6.3 Preserve teletext visual styling
    - Maintain authentic teletext colors in compact mode
    - Ensure proper contrast and readability
    - Keep consistent visual hierarchy
    - _Requirements: 1.4, 3.4_

- [x] 7. Add comprehensive error handling
  - [x] 7.1 Handle terminal width constraints
    - Add validation for minimum terminal width
    - Implement fallback behavior for very narrow terminals
    - Display appropriate warnings when needed
    - _Requirements: 4.4_

  - [x] 7.2 Validate compact mode compatibility
    - Ensure compact mode works with date selection
    - Verify compatibility with all existing flags
    - Add error messages for invalid combinations if needed
    - _Requirements: 2.3, 4.4_

- [ ] 8. Create comprehensive test suite
  - [ ] 8.1 Write unit tests for compact functionality
    - Test team abbreviation mapping with various inputs
    - Test compact formatting logic with different game data
    - Test terminal width adaptation algorithms
    - _Requirements: 5.1_

  - [ ] 8.2 Add integration tests for compact mode
    - Test end-to-end compact mode in non-interactive mode
    - Test compact mode with interactive navigation
    - Test compact mode with different date selections
    - _Requirements: 2.1, 2.2, 2.3, 5.1_

  - [ ] 8.3 Add visual regression tests
    - Verify compact mode preserves teletext styling
    - Test compact layout with various terminal sizes
    - Ensure consistent formatting across different game states
    - _Requirements: 1.4, 3.4, 3.5_
