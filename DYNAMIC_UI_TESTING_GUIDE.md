# Dynamic UI Testing Guide

This guide provides comprehensive manual testing procedures for the dynamic UI space feature to ensure it works correctly across different terminal sizes and configurations.

## Prerequisites

- Build the application: `cargo build --release`
- Ensure you have access to different terminal emulators for testing
- Have the ability to resize terminal windows during testing

## Test Scenarios

### 1. Minimum Terminal Size Testing

**Objective**: Verify the application works correctly at minimum supported dimensions.

**Steps**:
1. Resize terminal to exactly 80x24 characters
2. Run the application: `./target/release/liiga_teletext`
3. Verify:
   - Application starts without errors
   - Content is displayed properly
   - All UI elements are visible
   - No text overflow or truncation issues
   - Pagination works correctly

**Expected Results**:
- Detail level should be "Minimal"
- Content should fit within terminal bounds
- No emergency mode warnings
- Games should be displayed with basic information

### 2. Standard Detail Level Testing

**Objective**: Test the standard detail level functionality.

**Steps**:
1. Resize terminal to 100x30 characters
2. Run the application
3. Verify:
   - More detailed information is displayed
   - Team names are longer than minimal mode
   - Time information is more precise
   - Goal events show additional details

**Expected Results**:
- Detail level should be "Standard"
- Enhanced information without overwhelming the display
- Smooth transition from minimal mode

### 3. Extended Detail Level Testing

**Objective**: Test the extended detail level functionality.

**Steps**:
1. Resize terminal to 120x40 characters or larger
2. Run the application
3. Verify:
   - Full team names are displayed
   - Complete goal information is shown
   - Additional game statistics are visible
   - Layout utilizes available space effectively

**Expected Results**:
- Detail level should be "Extended"
- Maximum information density
- Professional appearance with full details

### 4. Dynamic Resize Testing

**Objective**: Verify the application responds correctly to terminal resize events.

**Steps**:
1. Start application at 80x24
2. Gradually increase terminal size to 120x40
3. Gradually decrease back to 80x24
4. Perform rapid resize operations
5. Verify:
   - Layout updates immediately on resize
   - No flickering or display artifacts
   - Content adapts smoothly between detail levels
   - Pagination adjusts correctly

**Expected Results**:
- Smooth transitions between detail levels
- No visual glitches during resize
- Content remains readable throughout
- Performance remains responsive

### 5. Emergency Mode Testing

**Objective**: Test graceful degradation for very small terminals.

**Steps**:
1. Resize terminal to 60x15 characters
2. Run the application
3. Verify:
   - Application still functions
   - Emergency mode warning is displayed
   - Essential information is still visible
   - No crashes or errors occur

**Expected Results**:
- Emergency mode activated
- Degradation warning displayed
- Basic functionality preserved
- Stable operation despite constraints

### 6. Performance Testing

**Objective**: Verify performance remains acceptable during intensive use.

**Steps**:
1. Start application with live games data
2. Perform continuous resize operations for 30 seconds
3. Navigate between pages rapidly
4. Monitor:
   - CPU usage
   - Memory consumption
   - Response time to user input
   - Layout calculation speed

**Expected Results**:
- CPU usage remains reasonable (<50% on modern hardware)
- Memory usage stable (no memory leaks)
- Responsive user interface
- Smooth animations and transitions

### 7. Cross-Terminal Testing

**Objective**: Ensure compatibility across different terminal emulators.

**Terminals to Test**:
- macOS Terminal.app
- iTerm2
- VS Code integrated terminal
- tmux/screen sessions
- SSH sessions

**Steps**:
1. Test each terminal emulator with the application
2. Verify:
   - Resize detection works correctly
   - Colors display properly
   - Text rendering is correct
   - Performance is consistent

**Expected Results**:
- Consistent behavior across all terminals
- Proper color rendering
- Accurate resize detection
- No terminal-specific issues

### 8. Edge Case Testing

**Objective**: Test unusual scenarios and edge cases.

**Test Cases**:

#### Ultra-wide Terminal
1. Resize to 200x30 characters
2. Verify layout doesn't become too sparse
3. Check that padding is reasonable

#### Ultra-tall Terminal
1. Resize to 100x60 characters
2. Verify many games fit on one page
3. Check pagination behavior

#### Rapid Size Changes
1. Rapidly resize between different sizes
2. Verify no crashes or display corruption
3. Check cache performance

#### Long-running Session
1. Run application for extended period (30+ minutes)
2. Perform various operations
3. Monitor for memory leaks or performance degradation

### 9. Backward Compatibility Testing

**Objective**: Ensure existing functionality is preserved.

**Steps**:
1. Test with minimum terminal size (80x24)
2. Compare behavior with previous version
3. Verify:
   - Same information is displayed
   - Navigation works identically
   - Performance is similar or better
   - No regressions in functionality

**Expected Results**:
- Identical behavior at minimum size
- All existing features work
- No performance regressions
- Smooth upgrade experience

### 10. Integration Testing

**Objective**: Test integration with other application features.

**Steps**:
1. Test with auto-refresh enabled
2. Test with manual refresh
3. Test with different data sources
4. Test with network interruptions
5. Verify:
   - Layout updates work during data refresh
   - Resize handling works with live data
   - Error states display correctly
   - Recovery from errors is smooth

**Expected Results**:
- Seamless integration with existing features
- No conflicts between dynamic UI and other systems
- Robust error handling
- Consistent user experience

## Performance Benchmarks

### Layout Calculation Performance
- Target: <1ms for cache hits
- Target: <5ms for cache misses
- Target: <10ms for full recalculation

### Memory Usage
- Target: <50MB total memory usage
- Target: No memory leaks over time
- Target: Efficient cache management

### Responsiveness
- Target: <100ms response to resize events
- Target: <50ms response to user input
- Target: Smooth 60fps animations

## Troubleshooting Common Issues

### Issue: Layout doesn't update on resize
**Solution**: Check terminal emulator resize event support

### Issue: Text appears truncated
**Solution**: Verify terminal size detection accuracy

### Issue: Performance degradation over time
**Solution**: Check cache cleanup and memory management

### Issue: Colors don't display correctly
**Solution**: Verify terminal color support and settings

## Reporting Issues

When reporting issues, please include:
1. Terminal emulator and version
2. Terminal size when issue occurred
3. Steps to reproduce
4. Expected vs actual behavior
5. Screenshots if applicable
6. System information (OS, architecture)

## Success Criteria

The dynamic UI feature is considered successful if:
- All test scenarios pass
- Performance meets benchmarks
- No regressions in existing functionality
- Smooth user experience across all terminal sizes
- Robust error handling and recovery
- Consistent behavior across different terminals