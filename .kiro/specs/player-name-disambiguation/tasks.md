# Implementation Plan

- [x] 1. Implement core disambiguation logic in player_names module
  - Create `format_with_disambiguation` function that groups players by last name and applies disambiguation rules
  - Add `format_for_display_with_first_initial` function for creating disambiguated names
  - Create `DisambiguationContext` struct to manage team-scoped disambiguation
  - Write comprehensive unit tests for all disambiguation scenarios
  - _Requirements: 1.1, 1.3, 1.4, 4.1, 4.2, 4.3, 4.4_

- [x] 2. Create disambiguation utilities and helper functions
  - Implement helper function to extract first initial from first name with Unicode support
  - Add function to determine if disambiguation is needed for a given last name
  - Create utility to group players by last name within a team
  - Write unit tests for helper functions including edge cases
  - _Requirements: 1.4, 4.1, 4.2, 4.3_

- [x] 3. Enhance goal event processing with team-scoped disambiguation
  - Modify `process_goal_events` function to accept separate home and away player data
  - Implement team-scoped disambiguation logic that processes home and away teams separately
  - Update `process_team_goals` to use disambiguated player names
  - Ensure backward compatibility with existing function signatures
  - _Requirements: 2.1, 2.2, 2.3, 2.4_

- [x] 4. Update caching layer to support disambiguated names
  - Enhance `cache_players_with_disambiguation` function to handle team-scoped disambiguation
  - Modify cache key structure to support team-specific player name resolution
  - Update existing cache retrieval functions to work with disambiguated names
  - Write tests to verify caching behavior with disambiguation
  - _Requirements: 2.1, 2.2, 2.3_

- [x] 5. Integrate disambiguation with API data processing
  - Update API processing functions to collect and pass player data with first and last names
  - Modify data flow to apply disambiguation before caching player names
  - Ensure proper handling of missing or incomplete player data
  - Update error handling to gracefully degrade when player data is unavailable
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [x] 6. Add comprehensive unit tests for disambiguation logic
  - Test basic two-player disambiguation scenario (e.g., "Koivu M." and "Koivu S.")
  - Test no disambiguation needed when all last names are unique
  - Test multiple players with same last name (3+ players)
  - Test cross-team scenarios where same last names on different teams don't disambiguate
  - _Requirements: 1.1, 1.2, 2.1, 2.2_

- [x] 7. Add edge case handling and error resilience tests
  - Test handling of empty or missing first names
  - Test Unicode character support for Finnish names (ä, ö, å)
  - Test handling of first names with multiple words or hyphens
  - Test graceful degradation when player data is incomplete
  - _Requirements: 1.4, 4.1, 4.2, 4.3, 4.4_

- [x] 8. Verify display compatibility across all UI modes
  - Test that disambiguated names display correctly in normal mode
  - Verify compact mode handles disambiguated names within space constraints
  - Ensure wide mode maintains consistent disambiguation logic
  - Test that name truncation works properly with disambiguated names
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [-] 9. Add integration tests for end-to-end functionality
  - Create test scenarios with real-world player name combinations
  - Test complete data flow from API response to teletext display
  - Verify that goal events show correct disambiguated scorer names
  - Test performance impact of disambiguation on large datasets
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 3.1, 3.2, 3.3_

- [ ] 10. Update documentation and add usage examples
  - Add documentation for new disambiguation functions
  - Create code examples showing how disambiguation works
  - Update existing function documentation to reflect disambiguation behavior
  - Add performance notes and best practices for using disambiguation
  - _Requirements: All requirements for maintainability and developer experience_
