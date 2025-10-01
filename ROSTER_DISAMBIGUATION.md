# Roster-Based Player Name Disambiguation

✅ **STATUS: FULLY IMPLEMENTED** - Roster-based disambiguation is now active by default!

## Overview

This document explains the enhanced player name disambiguation system that uses **complete team rosters** instead of only players who have scored.

**Update**: As of this commit, roster-based disambiguation has been fully integrated into the main game processing flow (`process_single_game()`) and is now used by default for all games with goals.

## Problem Statement

The initial disambiguation implementation had a limitation: it only considered players who had actually scored goals when determining if disambiguation was needed. This could lead to situations where:

1. **Mikko Koivu scores 1 goal** → Shows as "Koivu" (no disambiguation)
2. **Saku Koivu scores later** → Now both need disambiguation, but Mikko's earlier goal display doesn't match

## Solution: Full Roster Disambiguation with Active Player Filtering

By fetching the complete team rosters from the detailed game API (`https://liiga.fi/api/v2/games/<season>/<game_id>`), we can determine **all active players** with potential name conflicts upfront, ensuring consistent disambiguation throughout the game.

**Important**: Only **active** players are considered for disambiguation. A player is considered active if they:
- Have a line assignment (`line` is not null)
- Are not injured (`injured: false`)
- Are not suspended (`suspended: false`)
- Have not been removed from the roster (`removed: false`)

This ensures that injured or scratched players like Aarno Erholtz don't trigger unnecessary disambiguation for active players like Emil Erholtz.

## API Endpoints

### Schedule API

`https://liiga.fi/api/v1/games/schedule/<date>`

- Returns basic game information
- Includes goal events with embedded `scorerPlayer` data
- **Limitation**: Only includes players who have scored

### Detailed Game API

`https://liiga.fi/api/v2/games/<season>/<game_id>`

- Returns complete game details
- **Includes full team rosters**: `homeTeamPlayers` and `awayTeamPlayers` arrays
- Each player object contains:

  ```json
  {
    "id": 40132448,
    "firstName": "JESPER",
    "lastName": "KOTAJÄRVI",
    "jersey": 4,
    "role": "DEFENSEMAN",
    // ... other fields
  }
  ```

## Implementation

### Function: `create_goal_events_with_rosters`

Located in: `src/data_fetcher/processors/goal_events.rs`

```rust
pub fn create_goal_events_with_rosters(
    game: &ScheduleGame,
    home_roster: &[Player],
    away_roster: &[Player],
) -> Vec<GoalEventData>
```

**Parameters:**

- `game`: The schedule game containing goal events
- `home_roster`: Complete home team roster from detailed game API
- `away_roster`: Complete away team roster from detailed game API

**Returns:**

- Vector of `GoalEventData` with properly disambiguated player names

### How It Works

1. **Converts rosters** to the disambiguation format:

   ```rust
   let home_players: Vec<(i64, String, String)> = home_roster
       .iter()
       .map(|p| (p.id, p.first_name.clone(), p.last_name.clone()))
       .collect();
   ```

2. **Applies team-scoped disambiguation** using the full roster:
   - If Mikko Koivu and Saku Koivu are both on the roster → Both show as "Koivu M." and "Koivu S."
   - Even if only one has scored so far

3. **Processes goal events** with the disambiguation context from step 2

## Integration Points

### Current Usage

✅ **IMPLEMENTED**: The system now uses roster-based disambiguation by default!

**Primary Integration** (as of this commit):

- **`src/data_fetcher/api/game_api.rs:process_single_game()`** (lines 101-158)
  - Automatically fetches full rosters for games with goals
  - Uses cached roster data when available
  - Falls back to scorer-only disambiguation if roster fetch fails
  - This is the main code path for displaying games

**Additional Usage**:

- `src/data_fetcher/api/game_api.rs:process_game_response_with_cache()` (lines 283-300)
- `src/data_fetcher/api/game_api.rs:process_goal_events_for_historical_game_with_players()` (lines 768-820)

### Recommended Integration

For schedule-based game processing, you can:

1. **Fetch the detailed game data** when processing each game:

   ```rust
   let detailed_url = build_game_url(&config.api_domain, game.season, game.id);
   let detailed_response: DetailedGameResponse = fetch(client, &detailed_url).await?;
   ```

2. **Use roster-based disambiguation**:

   ```rust
   let goal_events = create_goal_events_with_rosters(
       &game,
       &detailed_response.home_team_players,
       &detailed_response.away_team_players,
   );
   ```

3. **Cache the roster data** to avoid repeated API calls:

   ```rust
   cache_players_with_disambiguation(
       game.id,
       &detailed_response.home_team_players,
       &detailed_response.away_team_players,
   ).await;
   ```

## Caching Strategy

To minimize API calls while maintaining accuracy:

1. **Cache full rosters** per game with appropriate TTL:
   - Live games: Short TTL (1-2 minutes) - rosters rarely change mid-game
   - Finished games: Long TTL (hours/days) - rosters are static

2. **Cache disambiguated names** per game:
   - Use `cache_players_with_disambiguation()` from `src/data_fetcher/cache/player_cache.rs`
   - This caches the final formatted names for quick lookup

3. **Fallback to scorer-only disambiguation** if roster fetch fails:
   - The existing `create_basic_goal_events()` function already does this
   - Ensures the app works even if the detailed API is unavailable

## Rate Limiting

To prevent API throttling (429 errors), the system implements **semaphore-based rate limiting**:

### Implementation (`src/data_fetcher/api/game_api.rs:process_response_games`)

- **Max 3 concurrent requests** to the detailed game API at any time
- **1 second delay** between each request to spread out load
- Uses `tokio::sync::Semaphore` to control concurrency
- When processing 10 games on a date, only 3 will fetch detailed data simultaneously
- Other requests wait for a semaphore slot to become available
- First game has no delay, subsequent games wait 1 second before fetching

### Why This Matters

**Before rate limiting**:
```
Game 1 ──────→ API (detailed roster)
Game 2 ──────→ API (detailed roster)
Game 3 ──────→ API (detailed roster)
Game 4 ──────→ API (detailed roster)  ← All fire at once!
Game 5 ──────→ API (detailed roster)  ← Triggers 429 rate limit!
...
Result: "Pelaaja 12345" (fallback names shown)
```

**After rate limiting**:
```
Game 1 ──────→ API ✓
Game 2 ──────→ API ✓
Game 3 ──────→ API ✓
Game 4 ─(wait)─→ API ✓  ← Waits for slot
Game 5 ─(wait)─→ API ✓  ← Waits for slot
...
Result: Proper disambiguated names shown
```

### Configuration

The rate limiting parameters in the code:
```rust
let semaphore = Arc::new(Semaphore::new(3)); // Max 3 concurrent requests

// Add 1 second delay between requests
if game_idx > 0 {
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
}
```

These values can be adjusted based on API rate limits, but the current settings are safe defaults:
- **3 concurrent requests**: Provides reasonable parallelism without overwhelming
- **1 second delay**: Spreads out requests over time
- **Combined effect**: Maximum ~3 requests per second to the API
- Prevents 429 errors while maintaining acceptable performance

## Benefits

### ✅ Consistency

All goal displays use the same naming convention throughout the game, regardless of when goals are scored.

### ✅ Accuracy

Disambiguation considers all potential conflicts, not just players who have scored.

### ✅ Performance

With proper caching, roster data is fetched once per game and reused for all goal event processing.

### ✅ Backward Compatibility

The existing `create_basic_goal_events()` function remains unchanged and serves as a fallback.

## Example Scenarios

### Scenario 1: Both Koivus on Roster, Only One Scores

**Roster:**

- Mikko Koivu (player ID: 1001)
- Saku Koivu (player ID: 1002)
- Teemu Selänne (player ID: 1003)

**Goals:**

- 5:00 - Mikko Koivu → Display: **"Koivu M."** (disambiguated)
- 10:00 - Selänne → Display: **"Selänne"** (no conflict)

Without roster-based disambiguation, Mikko's goal would show as "Koivu" initially.

### Scenario 2: Player Scores Multiple Goals

**Roster:**

- Mikko Koivu (player ID: 1001)
- Saku Koivu (player ID: 1002)

**Goals:**

- 5:00 - Mikko Koivu
- 15:00 - Mikko Koivu (2nd goal)
- 30:00 - Saku Koivu

**All displays:** "Koivu M." for Mikko, "Koivu S." for Saku - consistent throughout.

## Testing

Test coverage includes:

- `tests/disambiguation_integration_tests.rs` - End-to-end disambiguation flows
- `tests/disambiguation_display_tests.rs` - UI mode compatibility
- `tests/simple_disambiguation_test.rs` - Core disambiguation logic

To test roster-based disambiguation:

```bash
cargo test --all-features
```

## Future Enhancements

### Potential Improvements

1. **Lazy roster fetching**: Only fetch rosters when goal events contain ambiguous names
2. **Incremental updates**: Update roster data if line changes occur during the game
3. **Cross-game caching**: Cache player data across multiple games for the same team
4. **Assistant disambiguation**: Apply the same logic to assist players in goal events

## Migration Path

To migrate existing code to use roster-based disambiguation:

1. **Identify places** calling `create_basic_goal_events()`
2. **Add roster fetching** before the call
3. **Switch to** `create_goal_events_with_rosters()`
4. **Add caching** to minimize API calls
5. **Keep fallback** to `create_basic_goal_events()` if roster fetch fails

## References

- Core disambiguation logic: `src/data_fetcher/player_names/disambiguation.rs`
- Player caching: `src/data_fetcher/cache/player_cache.rs`
- Goal event processing: `src/data_fetcher/processors/goal_events.rs`
- API integration: `src/data_fetcher/api/game_api.rs`
