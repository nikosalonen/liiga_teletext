# Design Document

## Overview

This design implements player name disambiguation for the Liiga Teletext application. The feature ensures that when multiple players on the same team share the same last name, their names are displayed with the first letter of their first name (e.g., "Koivu M.", "Kurri J.") to distinguish them clearly in the scorer list.

The solution integrates with the existing player name processing pipeline, extending the current `format_for_display` function and modifying the goal event processing to include team-scoped disambiguation logic.

## Architecture

### Current Data Flow

1. **Player Data Retrieval**: Player information is fetched from the API with `first_name` and `last_name` fields
2. **Name Formatting**: The `format_for_display` function converts full names to display format (currently last name only)
3. **Caching**: Formatted names are cached using `cache_players_with_formatting`
4. **Goal Event Processing**: `process_goal_events` uses cached player names to populate `GoalEventData.scorer_name`
5. **Display**: The teletext UI renders scorer names in various display modes

### New Architecture Components

The disambiguation system will be implemented as a new processing layer between name formatting and caching:

```
Player Data (API) → Name Formatting → Disambiguation → Caching → Goal Events → Display
```

### Key Design Decisions

1. **Team-Scoped Disambiguation**: Only players within the same team are considered for disambiguation
2. **Lazy Evaluation**: Disambiguation occurs during goal event processing when team context is available
3. **Backward Compatibility**: Existing display logic remains unchanged; only the name resolution changes
4. **Performance**: Minimal impact on existing caching and processing performance

## Components and Interfaces

### 1. Enhanced Player Name Processing

#### New Function: `format_with_disambiguation`
```rust
pub fn format_with_disambiguation(
    players: &[(i64, String, String)], // (id, first_name, last_name)
) -> HashMap<i64, String>
```

This function will:
- Group players by last name
- Apply disambiguation rules for duplicate last names
- Return a mapping of player ID to disambiguated display name

#### Enhanced Function: `format_for_display`
The existing function will be extended with an optional disambiguation parameter:
```rust
pub fn format_for_display(full_name: &str) -> String // Existing signature
pub fn format_for_display_with_first_initial(first_name: &str, last_name: &str) -> String // New variant
```

### 2. Goal Event Processing Enhancement

#### Modified Function: `process_goal_events`
The function will be enhanced to perform team-scoped disambiguation:

```rust
pub fn process_goal_events_with_disambiguation<T>(
    game: &T, 
    home_players: &[(i64, String, String)], // (id, first_name, last_name)
    away_players: &[(i64, String, String)],
) -> Vec<GoalEventData>
where T: HasTeams
```

This will:
- Apply disambiguation separately for home and away teams
- Generate disambiguated names for each team's players
- Process goal events using the disambiguated names

### 3. Cache Integration

#### Enhanced Function: `cache_players_with_disambiguation`
```rust
pub async fn cache_players_with_disambiguation(
    game_id: i32, 
    home_players: HashMap<i64, (String, String)>, // (first_name, last_name)
    away_players: HashMap<i64, (String, String)>,
)
```

This will:
- Apply team-scoped disambiguation before caching
- Maintain separate disambiguation contexts for home and away teams
- Cache the final disambiguated names

## Data Models

### Enhanced Player Data Structure

The existing `Player` struct already contains the necessary fields:
```rust
pub struct Player {
    pub id: i64,
    pub last_name: String,
    pub first_name: String,
}
```

### New Disambiguation Context

```rust
#[derive(Debug, Clone)]
pub struct DisambiguationContext {
    pub players: Vec<(i64, String, String)>, // (id, first_name, last_name)
    pub disambiguated_names: HashMap<i64, String>,
}

impl DisambiguationContext {
    pub fn new(players: Vec<(i64, String, String)>) -> Self;
    pub fn get_disambiguated_name(&self, player_id: i64) -> Option<&String>;
    pub fn needs_disambiguation(&self, last_name: &str) -> bool;
}
```

## Error Handling

### Graceful Degradation

1. **Missing First Name**: If a player's first name is empty or missing, fall back to last name only
2. **Invalid Characters**: Handle non-alphabetic first characters gracefully
3. **Empty Player Data**: Continue processing other players if some player data is incomplete
4. **API Failures**: Maintain existing fallback behavior when player data is unavailable

### Error Scenarios

1. **Partial Player Data**: Some players have complete data, others don't
   - **Solution**: Apply disambiguation only to players with complete data
   
2. **Unicode Handling**: Finnish characters in names (ä, ö, å)
   - **Solution**: Use Rust's built-in Unicode support for character operations
   
3. **Memory Constraints**: Large number of players per team
   - **Solution**: Use efficient HashMap operations and avoid unnecessary cloning

## Testing Strategy

### Unit Tests

1. **Disambiguation Logic**
   - Test basic disambiguation (two players with same last name)
   - Test no disambiguation needed (unique last names)
   - Test multiple players with same last name (3+ players)
   - Test edge cases (empty names, special characters)

2. **Integration with Existing Functions**
   - Test compatibility with existing `format_for_display`
   - Test caching behavior with disambiguated names
   - Test goal event processing with disambiguated names

3. **Team Scoping**
   - Test that home and away teams are processed separately
   - Test that players with same last name on different teams don't affect each other

### Integration Tests

1. **End-to-End Display**
   - Test that disambiguated names appear correctly in teletext output
   - Test all display modes (normal, compact, wide)
   - Test with real game data scenarios

2. **Performance Tests**
   - Ensure disambiguation doesn't significantly impact processing time
   - Test with large numbers of players per team

### Test Data Scenarios

1. **Basic Disambiguation**: Team with "Koivu, Mikko" and "Koivu, Saku"
2. **No Disambiguation**: Team with unique last names
3. **Multiple Same Names**: Team with three "Koivu" players
4. **Cross-Team**: "Koivu" on both home and away teams (should not disambiguate)
5. **Edge Cases**: Empty first names, special characters, long names

## Implementation Phases

### Phase 1: Core Disambiguation Logic
- Implement `format_with_disambiguation` function
- Add unit tests for disambiguation logic
- Ensure proper handling of edge cases

### Phase 2: Integration with Goal Processing
- Modify `process_goal_events` to use disambiguation
- Update caching logic to handle team-scoped disambiguation
- Add integration tests

### Phase 3: Display Integration
- Ensure disambiguated names display correctly in all UI modes
- Test with various terminal widths and display configurations
- Verify backward compatibility

### Phase 4: Performance Optimization
- Profile the disambiguation performance impact
- Optimize if necessary
- Add performance regression tests

## Backward Compatibility

The design maintains full backward compatibility:

1. **Existing API**: All existing function signatures remain unchanged
2. **Display Logic**: No changes to teletext rendering code
3. **Caching**: Cache structure remains the same, only content changes
4. **Configuration**: No new configuration options required

## Performance Considerations

1. **Minimal Overhead**: Disambiguation only occurs when multiple players share a last name
2. **Efficient Grouping**: Use HashMap for O(1) lookups during disambiguation
3. **Memory Usage**: Avoid unnecessary string cloning during processing
4. **Caching**: Leverage existing player name caching to avoid repeated disambiguation

## Security Considerations

1. **Input Validation**: Validate player name data to prevent injection attacks
2. **Memory Safety**: Use Rust's memory safety features to prevent buffer overflows
3. **Unicode Safety**: Properly handle Unicode characters in player names
