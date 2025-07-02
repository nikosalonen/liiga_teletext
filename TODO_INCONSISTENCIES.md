# TODO: Code Inconsistencies to Fix

## High Priority Inconsistencies

### 1. **~~Inconsistent Error Handling Patterns~~ ✅ COMPLETED**
**Location:** Throughout codebase
**Issue:** ~~Mixed error handling approaches~~ **RESOLVED**
- ~~Some functions return `AppError` while others use generic `Result<T, Box<dyn std::error::Error>>`~~ **Fixed:** All functions now use `AppError` consistently
- ~~Inconsistent error message formatting and specificity~~ **Fixed:** Enhanced `AppError` with specific error types and helper methods
- ~~Some API calls have detailed error handling while others have minimal~~ **Fixed:** Standardized error handling patterns

**Files fixed:**
- ✅ `src/main.rs` - Standardized error handling in main function, removed `eprintln!` calls, improved version parsing
- ✅ `src/data_fetcher/api.rs` - Enhanced error specificity and consistency
- ✅ `src/config.rs` - Ensured consistent error propagation with specific `AppError::Config` type
- ✅ `src/lib.rs` - Fixed documentation example to use `AppError`
- ✅ `src/data_fetcher/processors.rs` - Updated to use `AppError::DateTimeParse`
- ✅ `src/error.rs` - Enhanced with specific error types: `Config`, `VersionParse`, `DateTimeParse`, `LogSetup`

**Action Completed:** ✅ Created standardized error handling patterns and applied consistently across codebase

### 2. **~~Inconsistent Date/Time Handling~~ ✅ COMPLETED**
**Location:** Multiple files
**Issue:** ~~Mixed use of `chrono::Local` and `chrono::Utc`~~ **RESOLVED**
- ~~Some functions use local time, others use UTC~~ **Fixed:** All internal calculations now use UTC consistently
- ~~Date parsing done differently in different parts of code~~ **Fixed:** Standardized timezone handling approach
- ~~Inconsistent timezone handling~~ **Fixed:** UTC for internal calculations, Local only for display formatting

**Files fixed:**
- ✅ `src/data_fetcher/api.rs` - Updated `determine_fetch_date()` and `build_tournament_list()` to use UTC internally, convert to Local for display
- ✅ `src/teletext_ui.rs` - Updated `calculate_days_until_regular_season()` to use UTC consistently for calculations
- ✅ `src/data_fetcher/processors.rs` - Updated `should_show_todays_games()` to use UTC internally, convert to Local for comparison
- ✅ `src/main.rs` - Updated date formatting for error messages to use UTC internally, convert to Local for display

**Action Completed:** ✅ Standardized on consistent timezone handling approach: UTC for internal calculations, Local timezone only for display formatting

### 3. **Inconsistent Player Name Formatting**
**Location:** `src/data_fetcher/cache.rs` and `src/data_fetcher/api.rs`
**Issue:** Two different player name formatting functions with different purposes
- `format_player_name()` in `cache.rs` - formats to last name only (e.g., "Koivu")
- `format_player_full_name()` in `api.rs` - formats to full name (e.g., "Mikko Koivu")

**Files to fix:**
- `src/data_fetcher/cache.rs`
- `src/data_fetcher/api.rs`
- `src/data_fetcher/processors.rs`

**Action:** Consolidate into clear, well-documented functions with consistent naming

## Medium Priority Inconsistencies

### 4. **~~Inconsistent Async/Sync Operations~~ ✅ COMPLETED**
**Location:** `src/config.rs` and `src/build.rs`
**Issue:** ~~Mixed async/sync file operations~~ **RESOLVED**
- ~~`Config::load()` and `Config::save()` use `tokio::fs` (async)~~ **Fixed:** All application code now uses async `tokio::fs` consistently
- ~~`build.rs` uses synchronous file operations~~ **Fixed:** Documented as correct approach for build scripts
- ~~Some call sites may not handle async properly~~ **Fixed:** All call sites now use async properly

**Files fixed:**
- ✅ `src/main.rs` - Changed sync `std::fs::create_dir_all()` to async `tokio::fs::create_dir_all()`
- ✅ `src/config.rs` - Standardized all test file operations to use `tokio::fs`
- ✅ `tests/integration_tests.rs` - Updated to use `tokio::fs` consistently
- ✅ `build.rs` - Documented why sync operations are correct for build scripts
- ✅ `FILE_IO_GUIDELINES.md` - Created comprehensive documentation of file I/O standards

**Action Completed:** ✅ Standardized file I/O approach: async `tokio::fs` for application code, sync `std::fs` for build scripts

### 5. **Inconsistent API Response Handling**
**Location:** `src/data_fetcher/api.rs`
**Issue:** Generic error handling that could be more specific
- The `fetch` function has basic error handling
- Some API calls lack specific error types for different failure modes

**Files to fix:**
- `src/data_fetcher/api.rs`
- `src/error.rs` - May need additional error variants

**Action:** Add specific error types for different API failure scenarios

### 6. **Inconsistent Test Coverage**
**Location:** Throughout codebase
**Issue:** Uneven test coverage across modules
- Some modules have comprehensive tests
- Others have minimal or no unit tests
- Integration tests exist but unit tests missing for critical functions

**Files to add tests to:**
- `src/config.rs` - Add more unit tests for config operations
- `src/data_fetcher/processors.rs` - Add tests for edge cases
- `src/error.rs` - Add tests for error handling

**Action:** Add comprehensive unit tests for all modules

## Low Priority Inconsistencies

### 7. **Inconsistent Documentation Style**
**Location:** Throughout codebase
**Issue:** Varying documentation quality and style
- Some functions have extensive documentation with examples
- Others have minimal or no documentation
- Documentation style varies between modules

**Files to improve:**
- `src/main.rs` - Add more comprehensive documentation
- `src/teletext_ui.rs` - Standardize documentation style
- `src/data_fetcher/models.rs` - Add more examples

**Action:** Create documentation style guide and apply consistently

### 8. **Inconsistent Configuration Management**
**Location:** `src/main.rs` and `src/lib.rs`
**Issue:** Config loading logic could be more consistent
- Error handling for config operations varies
- Some config operations duplicated

**Files to fix:**
- `src/main.rs`
- `src/lib.rs`

**Action:** Consolidate configuration management logic

### 9. **Inconsistent UI Constants**
**Location:** `src/teletext_ui.rs`
**Issue:** Hardcoded values that could be configurable
- Color constants defined inline
- Some magic numbers that could be constants

**Files to fix:**
- `src/teletext_ui.rs`

**Action:** Extract hardcoded values to configurable constants

### 10. **Inconsistent Code Organization**
**Location:** `src/data_fetcher/` module
**Issue:** Some functions could be better organized
- Related functionality spread across multiple files
- Some helper functions could be grouped better

**Files to reorganize:**
- `src/data_fetcher/api.rs`
- `src/data_fetcher/processors.rs`
- `src/data_fetcher/cache.rs`

**Action:** Reorganize related functionality into logical groups

## Implementation Priority

### Phase 1 (High Priority)
1. ✅ **COMPLETED** - Fix inconsistent error handling patterns
2. ✅ **COMPLETED** - Standardize date/time handling
3. Consolidate player name formatting

### Phase 2 (Medium Priority)
4. Review async/sync operations
5. Improve API response handling
6. Add comprehensive test coverage

### Phase 3 (Low Priority)
7. Standardize documentation style
8. Consolidate configuration management
9. Extract UI constants
10. Reorganize code structure

## Success Criteria
- All error handling follows consistent patterns
- Player name formatting is clear and well-documented
- Date/time handling is consistent throughout
- Test coverage is comprehensive
- Documentation style is uniform
- Code organization is logical and maintainable
