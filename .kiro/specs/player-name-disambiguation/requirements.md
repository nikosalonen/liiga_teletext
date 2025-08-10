# Requirements Document

## Introduction

This feature implements player name disambiguation in the scorer list display. When multiple players on the same team have the same last name, their names should include the first letter of their first name to distinguish them (e.g., "Koivu M.", "Kurri J.", "Selänne"). This follows common hockey scoring conventions and improves clarity for users viewing goal scorer information.

## Requirements

### Requirement 1

**User Story:** As a hockey fan viewing game results, I want players with the same last name to be clearly distinguished in the scorer list, so that I can identify which specific player scored each goal.

#### Acceptance Criteria

1. WHEN multiple players on the same team have the same last name THEN the system SHALL display their names with the first letter of their first name appended (e.g., "Koivu M.", "Koivu S.")
2. WHEN a player has a unique last name on their team THEN the system SHALL display only their last name without first initial (e.g., "Selänne")
3. WHEN displaying disambiguated names THEN the system SHALL format them as "{LastName} {FirstInitial}." with proper capitalization
4. WHEN processing player names THEN the system SHALL handle Finnish characters (ä, ö, å) correctly in both last names and first initials

### Requirement 2

**User Story:** As a developer maintaining the teletext display, I want the name disambiguation logic to be team-scoped, so that players with the same last name on different teams don't affect each other's display format.

#### Acceptance Criteria

1. WHEN determining name disambiguation THEN the system SHALL only consider players within the same team
2. WHEN a player named "Koivu" is on the home team and another "Koivu" is on the away team THEN both SHALL display as "Koivu" without disambiguation
3. WHEN multiple "Koivu" players are on the same team THEN they SHALL be disambiguated with first initials
4. WHEN processing goal events THEN the system SHALL correctly identify which team each scorer belongs to for disambiguation purposes

### Requirement 3

**User Story:** As a user viewing goal scorer information, I want the disambiguation to work consistently across all display modes (normal, compact, wide), so that player identification remains clear regardless of the display format.

#### Acceptance Criteria

1. WHEN displaying goal scorers in normal mode THEN disambiguated names SHALL be formatted consistently
2. WHEN displaying goal scorers in compact mode THEN disambiguated names SHALL fit within the allocated space constraints
3. WHEN displaying goal scorers in wide mode THEN disambiguated names SHALL maintain the same disambiguation logic
4. WHEN the display format changes THEN the disambiguation logic SHALL remain consistent across all modes

### Requirement 4

**User Story:** As a system processing real-time game data, I want the disambiguation to handle edge cases gracefully, so that the display remains functional even with incomplete or unusual player data.

#### Acceptance Criteria

1. WHEN a player's first name is missing or empty THEN the system SHALL fall back to displaying only the last name
2. WHEN a player's first name contains multiple words THEN the system SHALL use the first letter of the first word for disambiguation
3. WHEN a player's first name starts with a non-alphabetic character THEN the system SHALL handle it gracefully without crashing
4. WHEN player data is incomplete THEN the system SHALL not break the disambiguation logic for other players
