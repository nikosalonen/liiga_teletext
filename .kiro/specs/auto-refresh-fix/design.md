# Design Document

## Overview

The auto-refresh functionality in the Liiga Teletext application is not working correctly for ongoing matches. The system should automatically refresh data every minute when live games are detected, but there are issues in the detection logic, cache management, and refresh timing. This design addresses these issues by improving the live game detection, fixing cache TTL handling, and enhancing the auto-refresh logic.

## Architecture

The auto-refresh system consists of several interconnected components:

1. **Main Event Loop** (`src/main.rs`) - Controls the overall refresh timing and triggers
2. **Live Game Detection** (`src/data_fetcher/cache.rs`) - Determines if games are currently ongoing
3. **Cache Management** (`src/data_fetcher/cache.rs`) - Manages data freshness with appropriate TTLs
4. **Game State Processing** (`src/data_fetcher/processors.rs`) - Converts API data to internal game states
5. **UI Indicators** (`src/teletext_ui.rs`) - Shows loading indicators during auto-refresh

## Components and Interfaces

### 1. Live Game Detection Enhancement

**Current Issue:** The `has_live_games_from_game_data` function may not be correctly identifying ongoing games.

**Solution:** Enhance the detection logic to ensure accurate identification of live games:

```rust
// Enhanced live game detection
pub fn has_live_games_from_game_data(games: &[GameData]) -> bool {
    let has_live = games.iter().any(|game| game.score_type == ScoreType::Ongoing);
    
    // Add debug logging for troubleshooting
    if has_live {
        let ongoing_count = games.iter().filter(|g| g.score_type == ScoreType::Ongoing).count();
        tracing::debug!("Live games detected: {} ongoing out of {} total games", ongoing_count, games.len());
    } else {
        tracing::debug!("No live games detected in {} games", games.len());
    }
    
    has_live
}
```

### 2. Cache TTL Management

**Current Issue:** Cache entries for live games may not be expiring quickly enough, causing stale data to be served.

**Solution:** Implement dynamic TTL based on game state:

```rust
impl CachedTournamentData {
    pub fn is_expired(&self) -> bool {
        let ttl = if self.has_live_games {
            Duration::from_secs(30) // 30 seconds for live games
        } else {
            Duration::from_secs(3600) // 1 hour for completed games
        };
        
        let is_expired = self.cached_at.elapsed() > ttl;
        
        if is_expired {
            tracing::debug!("Cache expired: has_live_games={}, age={:?}, ttl={:?}", 
                self.has_live_games, self.cached_at.elapsed(), ttl);
        }
        
        is_expired
    }
}
```

### 3. Auto-Refresh Logic Improvements

**Current Issue:** The auto-refresh timing and conditions may not be working correctly.

**Solution:** Enhance the main event loop logic with better state tracking and logging:

```rust
// Enhanced auto-refresh logic in main.rs
if !needs_refresh 
    && !last_games.is_empty() 
    && last_auto_refresh.elapsed() >= Duration::from_secs(60) 
{
    let has_ongoing_games = has_live_games_from_game_data(&last_games);
    let all_scheduled = !last_games.is_empty() && last_games.iter().all(is_future_game);
    
    tracing::debug!("Auto-refresh check: has_ongoing={}, all_scheduled={}, time_elapsed={:?}", 
        has_ongoing_games, all_scheduled, last_auto_refresh.elapsed());
    
    if let Some(ref date) = current_date {
        if is_historical_date(date) {
            tracing::debug!("Auto-refresh skipped for historical date: {}", date);
        } else if has_ongoing_games {
            needs_refresh = true;
            tracing::info!("Auto-refresh triggered for ongoing games");
        } else if all_scheduled {
            tracing::debug!("Auto-refresh skipped - all games are scheduled");
        } else {
            tracing::debug!("Auto-refresh skipped - no ongoing games, mixed game states");
        }
    } else if has_ongoing_games {
        needs_refresh = true;
        tracing::info!("Auto-refresh triggered for ongoing games");
    } else {
        tracing::debug!("Auto-refresh skipped - no ongoing games");
    }
}
```

### 4. Game State Processing

**Current Issue:** The `determine_game_status` function may not be correctly setting ScoreType::Ongoing.

**Solution:** Ensure robust game state determination:

```rust
pub fn determine_game_status(game: &ScheduleGame) -> (ScoreType, bool, bool) {
    let is_overtime = matches!(
        game.finished_type.as_deref(),
        Some("ENDED_DURING_EXTENDED_GAME_TIME")
    );

    let is_shootout = matches!(
        game.finished_type.as_deref(),
        Some("ENDED_DURING_WINNING_SHOT_COMPETITION")
    );

    let score_type = if !game.started {
        ScoreType::Scheduled
    } else if !game.ended {
        // This is the critical path for ongoing games
        ScoreType::Ongoing
    } else {
        ScoreType::Final
    };

    tracing::debug!("Game {} status: started={}, ended={}, score_type={:?}", 
        game.id, game.started, game.ended, score_type);

    (score_type, is_overtime, is_shootout)
}
```

## Data Models

The existing data models are sufficient, but we need to ensure proper state tracking:

- `GameData.score_type` - Must accurately reflect ScoreType::Ongoing for live games
- `CachedTournamentData.has_live_games` - Must be set correctly when caching
- Auto-refresh timing variables in main.rs - Must be properly managed

## Error Handling

Enhanced error handling for auto-refresh scenarios:

1. **Network Failures:** Log and continue with existing data, retry on next cycle
2. **Cache Corruption:** Clear affected cache entries and fetch fresh data
3. **API Response Errors:** Log detailed error information for debugging
4. **Timeout Handling:** Use shorter timeouts for auto-refresh to avoid blocking UI

```rust
// Enhanced error handling in fetch operations
match fetch_liiga_data(current_date.clone()).await {
    Ok((games, fetched_date)) => {
        // Success path
        tracing::debug!("Auto-refresh successful: {} games fetched", games.len());
    }
    Err(e) => {
        tracing::warn!("Auto-refresh failed: {}, continuing with cached data", e);
        // Don't update needs_refresh, will retry on next cycle
        continue;
    }
}
```

## Testing Strategy

### Unit Tests

1. **Live Game Detection Tests:**
   - Test `has_live_games_from_game_data` with various game state combinations
   - Verify correct identification of ongoing vs completed vs scheduled games

2. **Cache TTL Tests:**
   - Test cache expiration for live games (30 second TTL)
   - Test cache expiration for completed games (1 hour TTL)
   - Verify cache invalidation when game state changes

3. **Game State Processing Tests:**
   - Test `determine_game_status` with all possible game state combinations
   - Verify ScoreType::Ongoing is set correctly for started but not ended games

### Integration Tests

1. **Auto-Refresh Flow Tests:**
   - Mock ongoing games and verify auto-refresh triggers every 60 seconds
   - Mock completed games and verify auto-refresh doesn't trigger
   - Test transition from ongoing to completed games

2. **Cache Integration Tests:**
   - Verify cache entries are marked with correct `has_live_games` flag
   - Test cache invalidation during auto-refresh cycles
   - Verify fresh data is fetched when cache expires

### Manual Testing Scenarios

1. **Live Game Simulation:**
   - Start application during live games
   - Verify auto-refresh occurs every minute
   - Verify loading indicator appears during refresh

2. **Game State Transitions:**
   - Test behavior when games transition from ongoing to completed
   - Verify auto-refresh stops when all games complete

3. **Error Recovery:**
   - Test behavior when API is temporarily unavailable
   - Verify graceful degradation and recovery

## Performance Considerations

1. **Reduced Cache TTL Impact:** 30-second TTL for live games will increase API calls but ensure data freshness
2. **Logging Overhead:** Debug logging will be minimal performance impact but crucial for troubleshooting
3. **UI Responsiveness:** Auto-refresh should not block user interactions
4. **Memory Usage:** Enhanced logging may slightly increase memory usage but within acceptable limits

## Security Considerations

No additional security concerns are introduced by these changes. The modifications maintain the existing security model while improving functionality and observability.