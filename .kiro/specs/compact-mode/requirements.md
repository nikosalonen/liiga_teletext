# Requirements Document

## Introduction

This feature adds a compact display mode to the Liiga Teletext application that shows only essential game information (team short identifiers and scores) while maintaining the authentic teletext visual style. The compact mode will be activated via command-line flags (-c or --compact) and will provide a more condensed view of game results for users who want quick score overviews without additional details.

## Requirements

### Requirement 1

**User Story:** As a hockey fan, I want to view game scores in a compact format, so that I can quickly scan multiple game results without scrolling through detailed information.

#### Acceptance Criteria

1. WHEN the user runs the application with `-c` flag THEN the system SHALL display games in compact format
2. WHEN the user runs the application with `--compact` flag THEN the system SHALL display games in compact format
3. WHEN compact mode is active THEN the system SHALL show only team short identifiers and scores for each game
4. WHEN compact mode is active THEN the system SHALL maintain the authentic teletext visual styling and colors
5. WHEN compact mode is active THEN the system SHALL display multiple games per screen line where space permits

### Requirement 2

**User Story:** As a terminal user, I want the compact mode to work with both interactive and non-interactive modes, so that I can use it in scripts or for quick terminal checks.

#### Acceptance Criteria

1. WHEN compact mode is enabled in interactive mode THEN the system SHALL allow navigation between different views while maintaining compact display
2. WHEN compact mode is enabled in non-interactive mode THEN the system SHALL output compact results and exit
3. WHEN compact mode is combined with date selection THEN the system SHALL show compact results for the specified date
4. WHEN compact mode is active THEN the system SHALL preserve all existing filtering and date functionality

### Requirement 3

**User Story:** As a user who values screen real estate, I want the compact mode to maximize information density, so that I can see more games at once without losing readability.

#### Acceptance Criteria

1. WHEN displaying games in compact mode THEN the system SHALL remove goal scorer details and timestamps
2. WHEN displaying games in compact mode THEN the system SHALL remove video links
3. WHEN displaying games in compact mode THEN the system SHALL use abbreviated team names (3 character identifiers)
4. WHEN displaying games in compact mode THEN the system SHALL maintain clear visual separation between games
5. WHEN displaying games in compact mode THEN the system SHALL preserve game status indicators (live, final, upcoming)

### Requirement 4

**User Story:** As a user familiar with the existing interface, I want the compact mode to integrate seamlessly with current functionality, so that I don't lose access to other features.

#### Acceptance Criteria

1. WHEN compact mode is not specified THEN the system SHALL display games in the current detailed format
2. WHEN compact mode is active THEN the system SHALL still support automatic refresh functionality
3. WHEN compact mode is active THEN the system SHALL still display tournament type information
4. WHEN compact mode is active THEN the system SHALL still show season countdown during off-season
5. WHEN invalid flags are combined with compact mode THEN the system SHALL display appropriate error messages

### Requirement 5

**User Story:** As a developer integrating hockey data, I want the compact mode to provide consistent output format, so that I can reliably parse the results in automated scripts.

#### Acceptance Criteria

1. WHEN compact mode is used in non-interactive mode THEN the system SHALL produce consistent output formatting
2. WHEN compact mode displays live games THEN the system SHALL include live game indicators
3. WHEN compact mode displays completed games THEN the system SHALL show final scores clearly
4. WHEN compact mode displays upcoming games THEN the system SHALL show scheduled start times
5. WHEN no games are available THEN the system SHALL display appropriate compact format message
