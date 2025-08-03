//! Resize handling module for detecting terminal size changes
//!
//! This module provides functionality for detecting terminal resize events,
//! debouncing rapid changes, and determining when layout updates are needed.
//! It's designed to handle the challenges of terminal resize detection across
//! different terminal emulators and operating systems.
//!
//! ## Features
//!
//! - **Resize Detection**: Detects when terminal dimensions change
//! - **Debouncing**: Prevents excessive updates during rapid resize operations
//! - **Error Recovery**: Handles terminal size detection failures gracefully
//! - **Size Validation**: Ensures terminal sizes are reasonable before reporting changes
//! - **Fallback Handling**: Uses last known good size when detection fails
//!
//! ## Debouncing Strategy
//!
//! The resize handler uses a 100ms debounce period to prevent excessive layout
//! recalculations during window resize operations. This provides a good balance
//! between responsiveness and performance.
//!
//! ## Usage
//!
//! ```rust
//! use liiga_teletext::ui::resize::ResizeHandler;
//! use liiga_teletext::ui::layout::LayoutCalculator;
//!
//! let mut handler = ResizeHandler::new();
//! let mut layout_calculator = LayoutCalculator::new();
//!
//! // In your main loop:
//! let current_size = (100, 30);
//! if let Some(new_size) = handler.check_for_resize(current_size) {
//!     // Terminal was resized, update layout
//!     layout_calculator.calculate_layout(new_size);
//! }
//! ```

use crate::constants::dynamic_ui;
use crate::error::AppError;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Handles terminal resize detection and debouncing
#[derive(Debug)]
pub struct ResizeHandler {
    last_size: (u16, u16),
    resize_debounce: Duration,
    last_resize_time: Option<Instant>,
    last_known_good_size: Option<(u16, u16)>,
    consecutive_failures: u32,
}

impl ResizeHandler {
    /// Creates a new ResizeHandler with default debounce settings
    pub fn new() -> Self {
        Self {
            last_size: (0, 0),
            resize_debounce: Duration::from_millis(dynamic_ui::RESIZE_DEBOUNCE_MS),
            last_resize_time: None,
            last_known_good_size: None,
            consecutive_failures: 0,
        }
    }

    /// Creates a new ResizeHandler with custom debounce duration
    pub fn with_debounce(debounce_ms: u64) -> Self {
        Self {
            last_size: (0, 0),
            resize_debounce: Duration::from_millis(debounce_ms),
            last_resize_time: None,
            last_known_good_size: None,
            consecutive_failures: 0,
        }
    }

    /// Checks if the current terminal size has changed and if enough time has passed
    /// since the last resize to warrant a layout update
    pub fn check_for_resize(&mut self, current_size: (u16, u16)) -> Option<(u16, u16)> {
        let now = Instant::now();

        // Check if size has actually changed
        if current_size != self.last_size {
            self.last_resize_time = Some(now);
            self.last_size = current_size;
        }

        // Check if we should update layout based on debounce timing
        if let Some(last_resize) = self.last_resize_time {
            if now.duration_since(last_resize) >= self.resize_debounce {
                self.last_resize_time = None; // Reset debounce timer
                return Some(current_size);
            }
        }

        None
    }

    /// Determines if layout should be updated based on size change significance
    pub fn should_update_layout(&self, current_size: (u16, u16)) -> bool {
        let (current_width, current_height) = current_size;
        let (last_width, last_height) = self.last_size;

        // Always update if this is the first size check
        if self.last_size == (0, 0) {
            return true;
        }

        // Update if width change affects detail level thresholds
        let width_change_significant = (current_width
            >= dynamic_ui::EXTENDED_DETAIL_WIDTH_THRESHOLD)
            != (last_width >= dynamic_ui::EXTENDED_DETAIL_WIDTH_THRESHOLD)
            || (current_width >= dynamic_ui::STANDARD_DETAIL_WIDTH_THRESHOLD)
                != (last_width >= dynamic_ui::STANDARD_DETAIL_WIDTH_THRESHOLD);

        // Update if height change affects games per page significantly
        let height_change_significant = (current_height as i16 - last_height as i16).abs() >= 3;

        width_change_significant || height_change_significant
    }

    /// Gets the last recorded terminal size
    pub fn last_size(&self) -> (u16, u16) {
        self.last_size
    }

    /// Resets the resize handler state
    pub fn reset(&mut self) {
        self.last_size = (0, 0);
        self.last_resize_time = None;
        self.last_known_good_size = None;
        self.consecutive_failures = 0;
    }

    /// Checks if a resize is currently being debounced
    pub fn is_debouncing(&self) -> bool {
        if let Some(last_resize) = self.last_resize_time {
            Instant::now().duration_since(last_resize) < self.resize_debounce
        } else {
            false
        }
    }

    /// Safely detects terminal size with error handling and recovery
    pub fn detect_terminal_size_safe(&mut self) -> Result<(u16, u16), AppError> {
        match crossterm::terminal::size() {
            Ok(size) => {
                // Validate the detected size
                if size.0 == 0 || size.1 == 0 {
                    self.consecutive_failures += 1;
                    warn!(
                        "Terminal size detection returned zero dimensions: {:?}",
                        size
                    );

                    // Try to recover using last known good size
                    if let Some(good_size) = self.last_known_good_size {
                        info!("Using last known good terminal size: {:?}", good_size);
                        return Ok(good_size);
                    }

                    return Err(AppError::resize_operation_failed(
                        "Terminal size detection returned zero dimensions",
                    ));
                }

                // Reset failure counter on successful detection
                self.consecutive_failures = 0;
                self.last_known_good_size = Some(size);

                Ok(size)
            }
            Err(e) => {
                self.consecutive_failures += 1;
                error!(
                    "Failed to detect terminal size (attempt {}): {}",
                    self.consecutive_failures, e
                );

                // Try to recover using last known good size
                if let Some(good_size) = self.last_known_good_size {
                    info!(
                        "Using last known good terminal size after detection failure: {:?}",
                        good_size
                    );
                    return Ok(good_size);
                }

                // If we have too many consecutive failures, use emergency fallback
                if self.consecutive_failures >= 3 {
                    warn!(
                        "Too many consecutive terminal size detection failures, using emergency fallback"
                    );
                    let emergency_size = (
                        dynamic_ui::MIN_TERMINAL_WIDTH,
                        dynamic_ui::MIN_TERMINAL_HEIGHT,
                    );
                    self.last_known_good_size = Some(emergency_size);
                    return Ok(emergency_size);
                }

                Err(AppError::resize_operation_failed(format!(
                    "Terminal size detection failed: {e}"
                )))
            }
        }
    }

    /// Safely checks for resize with comprehensive error handling
    pub fn check_for_resize_safe(&mut self) -> Result<Option<(u16, u16)>, AppError> {
        let current_size = self.detect_terminal_size_safe()?;

        let now = Instant::now();

        // Check if size has actually changed
        if current_size != self.last_size {
            self.last_resize_time = Some(now);
            self.last_size = current_size;

            debug!("Terminal size changed to: {:?}", current_size);
        }

        // Check if we should update layout based on debounce timing
        if let Some(last_resize) = self.last_resize_time {
            if now.duration_since(last_resize) >= self.resize_debounce {
                self.last_resize_time = None; // Reset debounce timer
                debug!(
                    "Resize debounce completed, returning size: {:?}",
                    current_size
                );
                return Ok(Some(current_size));
            }
        }

        Ok(None)
    }

    /// Gets the last known good terminal size for recovery purposes
    pub fn last_known_good_size(&self) -> Option<(u16, u16)> {
        self.last_known_good_size
    }

    /// Gets the number of consecutive detection failures
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// Manually sets a known good size (useful for testing or recovery)
    pub fn set_known_good_size(&mut self, size: (u16, u16)) {
        self.last_known_good_size = Some(size);
        self.consecutive_failures = 0;
        debug!("Manually set known good terminal size: {:?}", size);
    }
}

impl Default for ResizeHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_resize_handler_creation() {
        let handler = ResizeHandler::new();
        assert_eq!(handler.last_size, (0, 0));
        assert_eq!(
            handler.resize_debounce,
            Duration::from_millis(dynamic_ui::RESIZE_DEBOUNCE_MS)
        );
    }

    #[test]
    fn test_resize_handler_with_custom_debounce() {
        let handler = ResizeHandler::with_debounce(50);
        assert_eq!(handler.resize_debounce, Duration::from_millis(50));
    }

    #[test]
    fn test_first_resize_detection() {
        let mut handler = ResizeHandler::with_debounce(10); // Short debounce for testing

        // First call should not trigger immediate resize
        let result = handler.check_for_resize((80, 24));
        assert!(result.is_none());

        // After debounce period, should return the size
        thread::sleep(Duration::from_millis(15));
        let result = handler.check_for_resize((80, 24));
        assert_eq!(result, Some((80, 24)));
    }

    #[test]
    fn test_resize_debouncing() {
        let mut handler = ResizeHandler::with_debounce(50);

        // Rapid size changes should be debounced
        handler.check_for_resize((80, 24));
        handler.check_for_resize((90, 30));
        handler.check_for_resize((100, 35));

        // Should still be debouncing
        assert!(handler.is_debouncing());

        // Should not return a size yet
        let result = handler.check_for_resize((100, 35));
        assert!(result.is_none());
    }

    #[test]
    fn test_should_update_layout() {
        let mut handler = ResizeHandler::new();

        // First call should always update
        assert!(handler.should_update_layout((80, 24)));

        // Set initial size
        handler.check_for_resize((80, 24));

        // Small changes should not trigger update
        assert!(!handler.should_update_layout((82, 25)));

        // Significant width change (crossing threshold) should trigger update
        assert!(handler.should_update_layout((100, 24))); // Crosses standard threshold

        // Set new size
        handler.check_for_resize((100, 24));

        // Significant height change should trigger update
        assert!(handler.should_update_layout((100, 30))); // Height change >= 3

        // Crossing extended threshold should trigger update
        assert!(handler.should_update_layout((120, 24))); // Crosses extended threshold
    }

    #[test]
    fn test_reset_functionality() {
        let mut handler = ResizeHandler::new();

        // Set some state
        handler.check_for_resize((80, 24));
        assert_ne!(handler.last_size(), (0, 0));

        // Reset should clear state
        handler.reset();
        assert_eq!(handler.last_size(), (0, 0));
        assert!(!handler.is_debouncing());
    }

    #[test]
    fn test_last_size_tracking() {
        let mut handler = ResizeHandler::new();

        // Initial size should be (0, 0)
        assert_eq!(handler.last_size(), (0, 0));

        // After resize check, should track the size
        handler.check_for_resize((80, 24));
        assert_eq!(handler.last_size(), (80, 24));

        // Should update to new size
        handler.check_for_resize((100, 30));
        assert_eq!(handler.last_size(), (100, 30));
    }

    #[test]
    fn test_resize_detection_timing_accuracy() {
        let mut handler = ResizeHandler::with_debounce(100); // Use default debounce time

        // Record start time
        let start = Instant::now();

        // Trigger a resize
        handler.check_for_resize((80, 24));

        // Should detect change immediately (within requirement 4.1: 100ms)
        let detection_time = start.elapsed();
        assert!(
            detection_time < Duration::from_millis(10),
            "Resize detection took too long: {detection_time:?}"
        );

        // Should be debouncing
        assert!(handler.is_debouncing());

        // Wait for debounce to complete
        thread::sleep(Duration::from_millis(110));

        // Should now return the size
        let result = handler.check_for_resize((80, 24));
        assert_eq!(result, Some((80, 24)));
    }

    #[test]
    fn test_rapid_resize_changes_debouncing() {
        let mut handler = ResizeHandler::with_debounce(50);

        // Simulate rapid terminal resizing
        let sizes = vec![
            (80, 24),
            (85, 25),
            (90, 26),
            (95, 27),
            (100, 28),
            (105, 29),
            (110, 30),
            (115, 31),
            (120, 32),
        ];

        // Apply all size changes rapidly
        for size in &sizes {
            handler.check_for_resize(*size);
            // Small delay to simulate rapid but not instantaneous changes
            thread::sleep(Duration::from_millis(5));
        }

        // Should still be debouncing after rapid changes
        assert!(handler.is_debouncing());

        // Last size should be tracked correctly
        assert_eq!(handler.last_size(), (120, 32));

        // Should not return a size yet due to debouncing
        let result = handler.check_for_resize((120, 32));
        assert!(result.is_none());

        // Wait for debounce to complete
        thread::sleep(Duration::from_millis(60));

        // Now should return the final size
        let result = handler.check_for_resize((120, 32));
        assert_eq!(result, Some((120, 32)));
    }

    #[test]
    fn test_size_validation_edge_cases() {
        let mut handler = ResizeHandler::new();

        // Test with zero dimensions
        assert!(handler.should_update_layout((0, 0)));
        handler.check_for_resize((0, 0));

        // Test with very small dimensions
        assert!(handler.should_update_layout((1, 1)));
        handler.check_for_resize((1, 1));

        // Test with very large dimensions
        assert!(handler.should_update_layout((u16::MAX, u16::MAX)));
        handler.check_for_resize((u16::MAX, u16::MAX));

        // Test crossing all thresholds
        handler.reset();
        handler.check_for_resize((
            dynamic_ui::MIN_TERMINAL_WIDTH,
            dynamic_ui::MIN_TERMINAL_HEIGHT,
        ));

        // Cross standard threshold
        assert!(handler.should_update_layout((dynamic_ui::STANDARD_DETAIL_WIDTH_THRESHOLD, 24)));
        handler.check_for_resize((dynamic_ui::STANDARD_DETAIL_WIDTH_THRESHOLD, 24));

        // Cross extended threshold
        assert!(handler.should_update_layout((dynamic_ui::EXTENDED_DETAIL_WIDTH_THRESHOLD, 24)));
        handler.check_for_resize((dynamic_ui::EXTENDED_DETAIL_WIDTH_THRESHOLD, 24));

        // Test height change boundary (exactly 3 lines difference)
        assert!(handler.should_update_layout((dynamic_ui::EXTENDED_DETAIL_WIDTH_THRESHOLD, 27)));

        // Test height change just under boundary (2 lines difference)
        handler.check_for_resize((dynamic_ui::EXTENDED_DETAIL_WIDTH_THRESHOLD, 27));
        assert!(!handler.should_update_layout((dynamic_ui::EXTENDED_DETAIL_WIDTH_THRESHOLD, 29)));
    }

    #[test]
    fn test_debounce_timing_precision() {
        let debounce_ms = 50; // Use a longer debounce time for more reliable testing
        let mut handler = ResizeHandler::with_debounce(debounce_ms);

        // Trigger initial resize
        handler.check_for_resize((80, 24));

        // Check that debouncing is active immediately after resize
        assert!(handler.is_debouncing());

        // Wait slightly less than debounce time
        thread::sleep(Duration::from_millis(debounce_ms - 10));

        // Should still be debouncing
        assert!(handler.is_debouncing());
        let result = handler.check_for_resize((80, 24));
        assert!(result.is_none());

        // Wait for remaining debounce time plus a buffer
        thread::sleep(Duration::from_millis(20));

        // Should no longer be debouncing and should return size
        let result = handler.check_for_resize((80, 24));
        assert_eq!(result, Some((80, 24)));
        assert!(!handler.is_debouncing());
    }

    #[test]
    fn test_no_size_change_behavior() {
        let mut handler = ResizeHandler::with_debounce(20);

        // Set initial size
        handler.check_for_resize((100, 30));

        // Wait for debounce
        thread::sleep(Duration::from_millis(25));
        let result = handler.check_for_resize((100, 30));
        assert_eq!(result, Some((100, 30)));

        // Subsequent calls with same size should not trigger debouncing
        let result = handler.check_for_resize((100, 30));
        assert!(result.is_none());
        assert!(!handler.is_debouncing());

        // Should not require layout update for same size
        assert!(!handler.should_update_layout((100, 30)));
    }

    #[test]
    fn test_safe_resize_detection_with_recovery() {
        let mut handler = ResizeHandler::new();

        // Set a known good size first
        handler.set_known_good_size((100, 30));
        assert_eq!(handler.last_known_good_size(), Some((100, 30)));
        assert_eq!(handler.consecutive_failures(), 0);

        // Test that we can get the known good size
        let good_size = handler.last_known_good_size().unwrap();
        assert_eq!(good_size, (100, 30));
    }

    #[test]
    fn test_consecutive_failure_tracking() {
        let mut handler = ResizeHandler::new();

        // Initially no failures
        assert_eq!(handler.consecutive_failures(), 0);

        // Set a known good size to test recovery
        handler.set_known_good_size((80, 24));

        // Reset should clear failures
        handler.reset();
        assert_eq!(handler.consecutive_failures(), 0);
        assert_eq!(handler.last_known_good_size(), None);
    }

    #[test]
    fn test_manual_known_good_size_setting() {
        let mut handler = ResizeHandler::new();

        // Test setting known good size
        handler.set_known_good_size((120, 40));
        assert_eq!(handler.last_known_good_size(), Some((120, 40)));
        assert_eq!(handler.consecutive_failures(), 0);

        // Test that it resets failure count
        handler.consecutive_failures = 5; // Simulate failures
        handler.set_known_good_size((100, 30));
        assert_eq!(handler.consecutive_failures(), 0);
    }
}
