# Requirements Document

## Introduction

The Liiga Teletext application currently has an issue where matches are not automatically refreshing when there are ongoing matches in the API response. The application is supposed to automatically refresh every minute when live games are detected, but this functionality is not working correctly. Users expect real-time updates during live games to see score changes, goal events, and game status updates without manual intervention.

## Requirements

### Requirement 1

**User Story:** As a user watching live hockey games, I want the application to automatically refresh the display every minute when there are ongoing matches, so that I can see real-time score updates and game events without manually pressing 'r'.

#### Acceptance Criteria

1. WHEN there are ongoing games (ScoreType::Ongoing) in the current data THEN the system SHALL automatically refresh the data every 60 seconds
2. WHEN the auto-refresh occurs for ongoing games THEN the system SHALL show a subtle loading indicator to inform the user that data is being updated
3. WHEN ongoing games are detected THEN the system SHALL log "Auto-refresh triggered for ongoing games" to help with debugging
4. WHEN the auto-refresh completes THEN the system SHALL update the display with new game data and hide the loading indicator

### Requirement 2

**User Story:** As a user, I want the live game detection to work accurately, so that auto-refresh is triggered when games are actually in progress and not triggered when all games are completed or scheduled.

#### Acceptance Criteria

1. WHEN a game has started=true AND ended=false THEN the system SHALL classify it as ScoreType::Ongoing
2. WHEN the has_live_games_from_game_data function is called THEN it SHALL return true if any game has ScoreType::Ongoing
3. WHEN all games are completed (ended=true) THEN the system SHALL not trigger auto-refresh for live games
4. WHEN all games are scheduled (started=false) THEN the system SHALL not trigger auto-refresh for live games

### Requirement 3

**User Story:** As a user, I want the cache to be properly invalidated for live games, so that auto-refresh actually fetches fresh data from the API instead of serving stale cached data.

#### Acceptance Criteria

1. WHEN there are ongoing games THEN the tournament cache SHALL use a TTL of 30 seconds maximum
2. WHEN auto-refresh is triggered for ongoing games THEN the system SHALL bypass expired cache entries
3. WHEN fetching data for ongoing games THEN the system SHALL mark the cache entry with has_live_games=true
4. WHEN the cache TTL expires for live games THEN the system SHALL fetch fresh data from the API on the next request

### Requirement 4

**User Story:** As a developer debugging auto-refresh issues, I want comprehensive logging of the auto-refresh logic, so that I can identify when and why auto-refresh is or isn't being triggered.

#### Acceptance Criteria

1. WHEN checking for auto-refresh conditions THEN the system SHALL log the current state including: has_ongoing_games, time_since_last_refresh, and all_games_scheduled
2. WHEN auto-refresh is skipped THEN the system SHALL log the specific reason (historical date, all scheduled, no ongoing games, etc.)
3. WHEN auto-refresh is triggered THEN the system SHALL log "Auto-refresh triggered for ongoing games" with timestamp
4. WHEN cache decisions are made for live games THEN the system SHALL log cache hit/miss status and TTL information