# Implementation Plan

- [x] 1. Extract and move UI helper functions from main.rs to interactive.rs
  - Move all UI-related helper functions from main.rs to src/ui/interactive.rs
  - Update function signatures to work with the UI module context
  - Add necessary imports for moved functions
  - _Requirements: 1.1, 2.1, 2.2_

- [-] 2. Replace the current run_interactive_ui implementation in interactive.rs
  - Replace the existing simple implementation with the full implementation from main.rs
  - Adapt the function signature to accept Args instead of individual parameters
  - Ensure all terminal management and user input handling is preserved
  - _Requirements: 1.1, 2.2, 4.1_

- [ ] 3. Update main.rs to use the UI module
  - Remove the run_interactive_ui implementation from main.rs
  - Update main.rs to import and call the UI module's run_interactive_ui function
  - Remove all UI helper functions that were moved to the UI module
  - _Requirements: 1.2, 3.1, 4.2_

- [ ] 4. Clean up imports and remove duplicate code
  - Remove unused imports from main.rs after moving UI code
  - Update any remaining references to moved functions
  - Remove duplicate constants and helper functions
  - _Requirements: 2.1, 3.2_

- [ ] 5. Test the refactored implementation
  - Verify that the application compiles without warnings
  - Test interactive mode functionality to ensure identical behavior
  - Test all user input scenarios (navigation, refresh, quit)
  - _Requirements: 3.3, 4.3_