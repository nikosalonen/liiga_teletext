
# Project Improvement Suggestions

This document outlines potential improvements for the `liiga_teletext` project, focusing on code quality, performance, and user experience.

## 1. Code Structure and Maintainability

### 1.1. Consolidate Error Handling

**Observation:** Error handling is spread throughout the `data_fetcher` module, with repetitive `format!` macros creating error messages. This makes it difficult to maintain consistent error reporting.

**Suggestion:** Create a custom error enum for the application to centralize error types and messages. This will improve code readability and make it easier to add new error types in the future.

**Example (`src/error.rs`):**
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Failed to fetch data from API: {0}")]
    ApiFetch(#[from] reqwest::Error),

    #[error("Failed to parse API response: {0}")]
    ApiParse(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
```

### 1.2. Reduce Code Duplication in `data_fetcher/api.rs`

**Observation:** The `fetch_tournament_data` and `fetch_game_data` functions contain similar logic for handling HTTP requests and responses.

**Suggestion:** Create a generic `fetch` function that takes a URL and returns a `Result<T, AppError>`, where `T` is a deserializable type. This will reduce code duplication and simplify the API fetching logic.

## 2. Performance

### 2.1. Asynchronous File I/O

**Observation:** The `Config::load` and `Config::save` functions use synchronous file I/O, which can block the main thread.

**Suggestion:** Use `tokio::fs` for asynchronous file operations to avoid blocking. This is especially important in an application that already uses `tokio` for its runtime.

### 2.2. Optimize Player Name Formatting

**Observation:** The `process_team_goals` function formats player names by splitting the string and capitalizing the last name. This is done for every goal, every time data is fetched.

**Suggestion:** Cache the formatted player names along with the player data to avoid redundant processing. The player name formatting can be done once when the player data is first fetched and cached.

## 3. User Experience

### 3.1. More Informative Error Messages

**Observation:** When the API fails, the user is shown a generic error message.

**Suggestion:** Provide more specific error messages to the user. For example, if the API returns a 404, inform the user that no games were found for the specified date. If the API domain is incorrect, guide the user on how to update it using the `--config` flag.

### 3.2. Configuration Management

**Observation:** The application currently prompts for the API domain on first run. This is good, but it could be more robust.

**Suggestion:**
-   Add a `--reset-config` flag to allow users to easily reset their configuration.
-   When the API domain is invalid, automatically prompt the user to enter a new one, rather than just exiting with an error.

## 4. Testing

### 4.1. Add More Unit Tests

**Observation:** The project has some tests in `teletext_ui.rs`, but other critical parts of the application, such as `data_fetcher` and `config`, lack sufficient test coverage.

**Suggestion:** Add unit tests for the following:
-   `Config` loading and saving logic.
-   `data_fetcher` functions, using mock HTTP responses to test different API scenarios (e.g., successful response, error response, no games found).
-   `process_goal_events` to ensure it correctly handles various goal types and player name formats.

### 4.2. Integration Tests

**Observation:** There are no integration tests to verify the end-to-end workflow of the application.

**Suggestion:** Create integration tests that run the application with a mock API server to simulate real-world usage. This will help catch bugs that might be missed by unit tests.
