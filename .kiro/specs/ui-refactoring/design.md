# UI Module Refactoring Design

## Overview

This design outlines the refactoring of UI-related code from main.rs into a proper UI module structure. The goal is to consolidate duplicate functionality, improve code organization, and establish clear separation of concerns between application setup and UI implementation.

## Architecture

### Current State
- main.rs contains a `run_interactive_ui` function with complex UI logic
- src/ui/interactive.rs contains a different `run_interactive_ui` function that is unused
- Duplicate helper functions exist in both files
- main.rs handles both application setup and UI implementation

### Target State
- main.rs focuses on application setup, configuration, and coordination
- src/ui/interactive.rs contains all interactive UI functionality
- Single source of truth for UI-related functions
- Clear module boundaries with proper imports

## Components and Interfaces

### UI Module Structure
```
src/ui/
├── mod.rs          # Module exports
├── interactive.rs  # Interactive UI implementation
└── helpers.rs      # UI helper functions (if needed)
```

### Main Interface
The UI module will expose a single main function:
```rust
pub async fn run_interactive_ui(args: &Args) -> Result<(), AppError>
```

### Helper Functions to Move
From main.rs to src/ui/interactive.rs:
- `run_interactive_ui` (the main implementation)
- `calculate_games_hash`
- `create_page`
- `create_base_page`
- `create_future_games_page`
- Date navigation functions (`find_previous_date_with_games`, `find_next_date_with_games`)
- Game validation functions (`is_future_game`, `is_game_near_start_time`)
- Key handling functions (`is_date_navigation_key`)
- Utility functions (`get_target_date_for_navigation`, `would_be_previous_season`)

### Data Flow
1. main.rs parses arguments and sets up logging
2. main.rs calls `ui::run_interactive_ui(args)`
3. UI module handles all user interaction, data fetching, and display
4. UI module returns control to main.rs for cleanup

## Data Models

### Arguments Structure
The UI module will receive the complete `Args` structure, allowing it to access:
- `date`: Optional date parameter
- `disable_links`: Video link settings
- `debug`: Debug mode flag
- `min_refresh_interval`: Refresh timing configuration

### Internal State Management
The UI module will manage:
- Current page state
- Last refresh timestamps
- Game data cache and hashing
- User activity tracking
- Terminal state management

## Error Handling

### Error Propagation
- UI module functions return `Result<(), AppError>`
- Terminal cleanup handled within UI module with proper error recovery
- Network errors handled gracefully with user feedback

### Terminal State Management
- Raw mode enable/disable handled within UI module
- Alternate screen management contained in UI module
- Cleanup guaranteed even on error conditions

## Testing Strategy

### Unit Tests
- Test helper functions in isolation
- Mock data fetching for UI logic tests
- Test page creation and navigation logic

### Integration Tests
- Test complete UI flow with mock data
- Verify terminal state management
- Test error handling scenarios

### Refactoring Validation
- Ensure identical behavior before and after refactoring
- Verify no functionality is lost during the move
- Test all user interaction scenarios

## Implementation Approach

### Phase 1: Prepare UI Module
1. Update src/ui/interactive.rs with the complete implementation from main.rs
2. Adapt function signatures to work with the Args structure
3. Add necessary imports and dependencies

### Phase 2: Update Main Module
1. Remove UI implementation from main.rs
2. Update main.rs to call the UI module function
3. Remove duplicate helper functions from main.rs

### Phase 3: Clean Up and Test
1. Update imports throughout the codebase
2. Remove unused code and imports
3. Run comprehensive tests to ensure functionality is preserved

## Migration Strategy

### Backward Compatibility
- Maintain identical external behavior
- Preserve all command-line argument handling
- Keep the same error messages and user experience

### Risk Mitigation
- Move code in small, testable chunks
- Maintain git history for easy rollback
- Test each phase independently before proceeding

## Performance Considerations

### No Performance Impact
- Code movement should not affect runtime performance
- Memory usage patterns remain the same
- Network request patterns unchanged

### Potential Improvements
- Better code organization may improve compile times
- Clearer module boundaries may help with future optimizations