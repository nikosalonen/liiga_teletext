# File I/O Standardization Guidelines

This document outlines the standardized approach to file I/O operations across the liiga_teletext project.

## Core Principles

### 1. Async-First Approach
- **Default**: Use `tokio::fs` for all file operations in the main application
- **Rationale**: The project uses `tokio` as its async runtime, so file operations should be non-blocking
- **Performance**: Async I/O prevents blocking the main thread during file operations

### 2. Context-Appropriate Exceptions
- **Build scripts**: Use `std::fs` (synchronous) operations
- **Rationale**: Build scripts run outside the async runtime during compilation
- **Example**: `build.rs` intentionally uses `std::fs::create_dir_all()`

## Implementation Standards

### ✅ Correct Patterns

#### Application Code (Async Context)
```rust
// Configuration loading/saving
use tokio::fs;

pub async fn load_config() -> Result<Config, AppError> {
    let content = fs::read_to_string(&config_path).await?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

pub async fn save_config(&self, path: &str) -> Result<(), AppError> {
    fs::create_dir_all(parent_dir).await?;
    let mut file = fs::File::create(path).await?;
    file.write_all(content.as_bytes()).await?;
    Ok(())
}

// Log directory creation
if !Path::new(&log_dir).exists() {
    tokio::fs::create_dir_all(&log_dir).await?;
}
```

#### Tests (Async Context)
```rust
#[tokio::test]
async fn test_file_operations() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("test.txt");

    // Write test data
    tokio::fs::write(&file_path, "test content").await.unwrap();

    // Read and verify
    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(content, "test content");
}
```

#### Build Scripts (Sync Context)
```rust
// build.rs
use std::fs;

fn main() {
    // Build scripts must use std::fs (sync) operations
    if !dir.exists() {
        fs::create_dir_all(&dir).unwrap();
    }
}
```

### ❌ Incorrect Patterns

#### Mixing Sync and Async
```rust
// DON'T: Mix sync and async operations
async fn bad_example() {
    // This blocks the async runtime
    std::fs::create_dir_all(&dir).unwrap();

    // This is async but inconsistent with above
    let content = tokio::fs::read_to_string(&file).await?;
}
```

#### Using Sync in Async Context
```rust
// DON'T: Use sync operations in async functions
async fn bad_example() {
    // This blocks the entire async runtime
    std::fs::write(&path, content).unwrap();
}
```

## File-Specific Standards

### `src/main.rs`
- **Status**: ✅ Standardized
- **Approach**: All file operations use `tokio::fs`
- **Example**: Log directory creation now uses `tokio::fs::create_dir_all()`

### `src/config.rs`
- **Status**: ✅ Standardized
- **Approach**: All operations async with `tokio::fs`
- **Methods**: `load()`, `save()`, `save_to_path()`, `load_from_path()`

### `build.rs`
- **Status**: ✅ Standardized (Correctly Sync)
- **Approach**: Uses `std::fs` as required for build scripts
- **Rationale**: Build scripts run outside async runtime

### `tests/integration_tests.rs`
- **Status**: ✅ Standardized
- **Approach**: All test file operations use `tokio::fs`
- **Benefit**: Consistent with application code patterns

## Error Handling

### Consistent Error Propagation
```rust
// Use ? operator for error propagation
let content = tokio::fs::read_to_string(&path).await?;

// Map errors to custom error types when needed
tokio::fs::create_dir_all(&dir).await.map_err(|e| {
    AppError::log_setup_error(format!("Failed to create directory: {}", e))
})?;
```

### Context-Aware Error Messages
```rust
// Provide meaningful context in error messages
tokio::fs::write(&config_path, content).await.map_err(|e| {
    AppError::config_error(format!("Failed to write config to {}: {}", config_path, e))
})?;
```

## Performance Considerations

### Async Benefits
- **Non-blocking**: File operations don't block the async runtime
- **Concurrent**: Multiple file operations can run concurrently
- **Scalable**: Better resource utilization under high load

### When Sync is Appropriate
- **Build scripts**: No async runtime available
- **Simple utilities**: When async overhead isn't justified
- **Legacy compatibility**: When interfacing with sync-only APIs

## Testing Strategy

### Test Consistency
- Use `#[tokio::test]` for async tests
- Use `tokio::fs` operations in test setup/teardown
- Maintain consistency with application code patterns

### Example Test Pattern
```rust
#[tokio::test]
async fn test_config_roundtrip() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // Setup
    let original_config = Config { /* ... */ };
    let content = toml::to_string_pretty(&original_config).unwrap();
    tokio::fs::write(&config_path, content).await.unwrap();

    // Test
    let loaded_config = Config::load_from_path(&config_path_str).await.unwrap();

    // Verify
    assert_eq!(original_config.field, loaded_config.field);
}
```

## Migration Checklist

When adding new file operations:

- [ ] **Context Check**: Am I in an async context?
- [ ] **Runtime Check**: Is `tokio` runtime available?
- [ ] **Consistency Check**: Are similar operations in this file using `tokio::fs`?
- [ ] **Error Handling**: Am I using proper error propagation?
- [ ] **Testing**: Are my tests using consistent patterns?

## Common Pitfalls

### Blocking in Async Context
```rust
// ❌ This blocks the entire async runtime
async fn bad_sync_in_async() {
    std::fs::write(&path, content).unwrap(); // Blocks!
}

// ✅ This is non-blocking
async fn good_async_pattern() {
    tokio::fs::write(&path, content).await?; // Non-blocking
}
```

### Missing Error Context
```rust
// ❌ Generic error without context
tokio::fs::write(&path, content).await?;

// ✅ Contextual error with meaningful message
tokio::fs::write(&path, content).await.map_err(|e| {
    AppError::config_error(format!("Failed to write config to {}: {}", path, e))
})?;
```

## Conclusion

This standardization ensures:
- **Consistency**: All file operations follow the same patterns
- **Performance**: Non-blocking I/O operations in async contexts
- **Maintainability**: Clear guidelines for future development
- **Correctness**: Appropriate sync/async usage based on context

By following these guidelines, the codebase maintains high performance while remaining consistent and maintainable.
