# Design Document

## Overview

The compact mode feature adds a new display option to the Liiga Teletext application that shows game results in a condensed format while preserving the authentic teletext visual styling. This mode will be activated via command-line flags (`-c` or `--compact`) and will display only essential information: team short identifiers and scores.

The design integrates seamlessly with the existing architecture by extending the current CLI argument parsing, page creation logic, and rendering system without disrupting existing functionality.

## Architecture

### Command-Line Interface Extension

The compact mode will be implemented by extending the existing `Args` struct in `src/main.rs` with new flags:

- `-c` (short flag)
- `--compact` (long flag)

These flags will be mutually exclusive with detailed display and will work in both interactive and non-interactive modes.

### Data Flow Integration

The compact mode will integrate into the existing data flow:

1. **CLI Parsing** → Compact flag detection
2. **Data Fetching** → Same data fetching logic (no changes needed)
3. **Page Creation** → Modified page creation with compact formatting
4. **Rendering** → Enhanced rendering logic for compact display

### Display Logic Modification

The compact display will be implemented by:

1. **Extending TeletextPage** with compact mode awareness
2. **Modifying game result rendering** to show abbreviated format
3. **Preserving teletext styling** while reducing information density

## Components and Interfaces

### CLI Arguments Structure

```rust
// Addition to existing Args struct in src/main.rs
#[derive(Parser, Debug)]
struct Args {
    // ... existing fields ...
    
    /// Display games in compact format showing only team identifiers and scores
    #[arg(short = 'c', long = "compact", help_heading = "Display Options")]
    compact: bool,
}
```

### TeletextPage Enhancement

The `TeletextPage` struct will be extended to support compact mode:

```rust
// Addition to TeletextPage in src/teletext_ui.rs
pub struct TeletextPage {
    // ... existing fields ...
    compact_mode: bool,
}

impl TeletextPage {
    // New constructor parameter
    pub fn new(
        page_number: u16,
        title: String,
        subheader: String,
        disable_video_links: bool,
        show_footer: bool,
        ignore_height_limit: bool,
        compact_mode: bool, // New parameter
    ) -> Self
    
    // New method to enable compact rendering
    pub fn set_compact_mode(&mut self, compact: bool)
    pub fn is_compact_mode(&self) -> bool
}
```

### Game Result Rendering

The compact mode will modify how `TeletextRow::GameResult` is rendered:

**Current format:**
```
Tappara - HIFK                    3-2
  Mäenalanen 15:23 (yv)
  Komarov 28:45
```

**Compact format:**
```
TAP-HIK 3-2  
TPS-JYP 1-4  
ILV-KAL 2-1
```

### Team Name Abbreviation

A new mapping system will convert full team names to 3-4 character abbreviations:

```rust
// New module in src/teletext_ui.rs or separate file
fn get_team_abbreviation(team_name: &str) -> &str {
    match team_name {
        "Tappara" => "TAP",
        "HIFK" => "HIFK",
        "TPS" => "TPS",
        "JYP" => "JYP",
        // ... complete mapping
        _ => &team_name[..3.min(team_name.len())], // Fallback
    }
}
```

## Data Models

### Compact Display Configuration

```rust
#[derive(Debug, Clone)]
pub struct CompactDisplayConfig {
    pub max_games_per_line: usize,
    pub team_name_width: usize,
    pub score_width: usize,
    pub game_separator: &'static str,
}

impl Default for CompactDisplayConfig {
    fn default() -> Self {
        Self {
            max_games_per_line: 3,
            team_name_width: 7, // "TAP-PEL"
            score_width: 5,     // " 3-2 "
            game_separator: "  ",
        }
    }
}
```

### Game Display Data

The existing `GameResultData` struct will be used without modification, but the rendering logic will extract only essential fields:

- `home_team` → abbreviated
- `away_team` → abbreviated  
- `result` → score only
- `score_type` → for status indicators

## Error Handling

### Invalid Flag Combinations

The application will validate flag combinations and show appropriate error messages:

```rust
fn validate_args(args: &Args) -> Result<(), AppError> {
    // Compact mode validation
    if args.compact {
        // No conflicting validations currently needed
        // Future: could add restrictions if needed
    }
    Ok(())
}
```

### Terminal Width Constraints

The compact mode will list one game per line no matter the width of the terminal

### Missing Team Abbreviations

For teams without predefined abbreviations:
- Use first 3-4 characters of team name
- Log warning for missing abbreviations
- Maintain consistent formatting

## Testing Strategy

### Unit Tests

1. **CLI Argument Parsing**
   - Test `-c` flag recognition
   - Test `--compact` flag recognition
   - Test flag combination validation

2. **Team Abbreviation Logic**
   - Test known team name mappings
   - Test fallback abbreviation generation
   - Test edge cases (empty names, special characters)

3. **Compact Rendering Logic**
   - Test single game formatting
   - Test multiple games per line
   - Test terminal width adaptation

### Integration Tests

1. **End-to-End Compact Mode**
   - Test compact mode with real game data
   - Test compact mode in both interactive and non-interactive modes

2. **Compatibility Testing**
   - Ensure compact mode doesn't break existing functionality
   - Test compact mode with all existing flags
   - Test compact mode with different date selections

### Visual Testing

1. **Teletext Styling Preservation**
   - Verify colors remain authentic
   - Verify layout maintains teletext appearance
   - Test with various game states (live, final, scheduled)

2. **Responsive Layout**
   - Test with different terminal widths
   - Verify graceful degradation on narrow terminals
   - Test pagination in compact mode

## Implementation Phases

### Phase 1: Core Infrastructure
- Add CLI flags to Args struct
- Extend TeletextPage with compact mode support
- Implement team abbreviation mapping
- Add basic compact rendering logic

### Phase 2: Display Logic
- Implement multi-game-per-line layout
- Add terminal width adaptation
- Integrate with existing page creation functions
- Preserve teletext styling in compact format

### Phase 3: Integration & Polish
- Update main.rs to pass compact flag through the system
- Ensure compatibility with interactive and non-interactive modes
- Add comprehensive error handling
- Optimize performance for compact rendering

### Phase 4: Testing & Documentation
- Add unit tests for all new functionality
- Add integration tests for end-to-end scenarios
- Update help text and documentation
- Performance testing and optimization
