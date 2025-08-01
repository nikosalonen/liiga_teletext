# Implementation Plan

- [x] 1. Enhance live game detection with improved logging
  - Add debug logging to `has_live_games_from_game_data` function to track ongoing game detection
  - Include count of ongoing games and total games in log messages
  - Add logging to show when no live games are detected
  - _Requirements: 2.2, 4.1_

- [x] 2. Improve game state determination logging
  - Add debug logging to `determine_game_status` function in processors.rs
  - Log game ID, started/ended status, and resulting ScoreType for each game
  - Ensure ScoreType::Ongoing is correctly set for started but not ended games
  - _Requirements: 2.1, 4.1_

- [x] 3. Fix cache TTL handling for live games
  - Enhance `CachedTournamentData::is_expired()` method with debug logging
  - Log cache expiration decisions including has_live_games flag, age, and TTL
  - Ensure 30-second TTL is properly applied for live games
  - _Requirements: 3.1, 3.4, 4.4_

- [ ] 4. Enhance auto-refresh logic with comprehensive logging
  - Add detailed logging to auto-refresh condition checking in main.rs
  - Log has_ongoing_games, all_scheduled status, and time_elapsed values
  - Add specific log messages for each auto-refresh decision path
  - Ensure "Auto-refresh triggered for ongoing games" message is logged when appropriate
  - _Requirements: 1.3, 4.1, 4.2_

- [ ] 5. Improve cache invalidation for ongoing games
  - Ensure tournament cache entries are properly marked with has_live_games=true when ongoing games are present
  - Add logging to cache operations to track when live game cache entries are created
  - Verify cache bypass logic works correctly for expired entries during auto-refresh
  - _Requirements: 3.2, 3.3, 4.4_

- [ ] 6. Add error handling and recovery for auto-refresh failures
  - Enhance error handling in the auto-refresh data fetching logic
  - Log detailed error information when auto-refresh fails
  - Ensure graceful degradation when API calls fail during auto-refresh
  - Continue with existing data and retry on next cycle when errors occur
  - _Requirements: 1.4, 4.2_

- [ ] 7. Create comprehensive unit tests for live game detection
  - Write tests for `has_live_games_from_game_data` with various game state combinations
  - Test scenarios with all ongoing, all completed, all scheduled, and mixed game states
  - Verify correct boolean return values and logging output
  - _Requirements: 2.2, 2.3, 2.4_

- [ ] 8. Create unit tests for game state processing
  - Write tests for `determine_game_status` covering all possible game state combinations
  - Test started=true, ended=false scenarios to ensure ScoreType::Ongoing is set
  - Test started=false scenarios for ScoreType::Scheduled
  - Test started=true, ended=true scenarios for ScoreType::Final
  - _Requirements: 2.1_

- [ ] 9. Create unit tests for cache TTL behavior
  - Write tests for `CachedTournamentData::is_expired()` with live games (30s TTL)
  - Write tests for cache expiration with completed games (1h TTL)
  - Test cache age calculations and expiration logic
  - _Requirements: 3.1, 3.4_

- [ ] 10. Create integration tests for auto-refresh flow
  - Write tests that simulate ongoing games and verify auto-refresh triggers every 60 seconds
  - Write tests that simulate completed games and verify auto-refresh doesn't trigger inappropriately
  - Test the complete flow from game state detection through cache management to refresh triggering
  - _Requirements: 1.1, 1.2, 1.4_