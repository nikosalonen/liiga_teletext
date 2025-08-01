//! Resize handling module for detecting terminal size changes
//!
//! This module provides functionality for detecting terminal resize events,
//! debouncing rapid changes, and determining when layout updates are needed.

use std::time::{Duration, Instant};
use crate::constants::dynamic_ui;

/// Handles terminal resize detection and debouncing
#[derive(Debug)]
pub struct ResizeHandler {
    last_size: (u16, u16),
    resize_debounce: Duration,
    last_resize_time: Option<Instant>,
}

impl ResizeHandler {
    /// Creates a new ResizeHandler with default debounce settings
    pub fn new() -> Self {
        Self {
            last_size: (0, 0),
            resize_debounce: Duration::from_millis(dynamic_ui::RESIZE_DEBOUNCE_MS),
            last_resize_time: None,
        }
    }

    /// Creates a new ResizeHandler with custom debounce duration
    pub fn with_debounce(debounce_ms: u64) -> Self {
        Self {
            last_size: (0, 0),
            resize_debounce: Duration::from_millis(debounce_ms),
            last_resize_time: None,
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
        let width_change_significant = 
            (current_width >= dynamic_ui::EXTENDED_DETAIL_WIDTH_THRESHOLD) != 
            (last_width >= dynamic_ui::EXTENDED_DETAIL_WIDTH_THRESHOLD) ||
            (current_width >= dynamic_ui::STANDARD_DETAIL_WIDTH_THRESHOLD) != 
            (last_width >= dynamic_ui::STANDARD_DETAIL_WIDTH_THRESHOLD);

        // Update if height change affects games per page significantly
        let height_change_significant = 
            (current_height as i16 - last_height as i16).abs() >= 3;

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
    }

    /// Checks if a resize is currently being debounced
    pub fn is_debouncing(&self) -> bool {
        if let Some(last_resize) = self.last_resize_time {
            Instant::now().duration_since(last_resize) < self.resize_debounce
        } else {
            false
        }
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
        assert_eq!(handler.resize_debounce, Duration::from_millis(dynamic_ui::RESIZE_DEBOUNCE_MS));
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
}