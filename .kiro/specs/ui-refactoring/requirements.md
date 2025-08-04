# UI Module Refactoring Requirements

## Introduction

This feature involves refactoring the UI-related code from main.rs into a proper UI module structure. Currently, there is duplicate functionality between src/ui/interactive.rs and main.rs, with the main.rs version being the one actually used. We need to consolidate this code into the UI module and update all imports accordingly.

## Requirements

### Requirement 1

**User Story:** As a developer, I want the UI code to be properly organized in a dedicated module, so that the codebase is more maintainable and follows good separation of concerns.

#### Acceptance Criteria

1. WHEN the application starts THEN the UI functionality SHALL be handled by the src/ui module
2. WHEN examining main.rs THEN it SHALL NOT contain UI implementation details
3. WHEN looking at the UI module THEN it SHALL contain all interactive UI functionality

### Requirement 2

**User Story:** As a developer, I want to eliminate duplicate code between main.rs and the UI module, so that there is a single source of truth for UI functionality.

#### Acceptance Criteria

1. WHEN examining the codebase THEN there SHALL be no duplicate UI functions between main.rs and src/ui/
2. WHEN the run_interactive_ui function is called THEN it SHALL use the implementation from src/ui/interactive.rs
3. WHEN helper functions like calculate_games_hash are needed THEN they SHALL exist only in the UI module

### Requirement 3

**User Story:** As a developer, I want all UI-related imports to be properly updated, so that the application continues to work correctly after the refactoring.

#### Acceptance Criteria

1. WHEN main.rs needs UI functionality THEN it SHALL import from the ui module
2. WHEN the application is compiled THEN there SHALL be no unused import warnings
3. WHEN the application runs THEN it SHALL behave identically to before the refactoring

### Requirement 4

**User Story:** As a developer, I want the UI module to handle all the complex interactive functionality, so that main.rs focuses only on application setup and coordination.

#### Acceptance Criteria

1. WHEN examining the UI module THEN it SHALL handle date navigation, page management, and user input
2. WHEN main.rs calls the UI THEN it SHALL pass the necessary configuration and receive results
3. WHEN the UI needs to create pages THEN it SHALL use its own helper functions for page creation