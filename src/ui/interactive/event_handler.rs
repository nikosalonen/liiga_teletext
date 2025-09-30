//! Event handling coordination for interactive UI
//!
//! This module provides unified event handling for the interactive UI,
//! coordinating between different event types (keyboard, resize, etc.)
//! and managing their interaction with the state manager.

use super::input_handler::{KeyEventParams, handle_key_event};
use super::refresh_manager::calculate_poll_interval;
use super::state_manager::InteractiveState;
use crate::error::AppError;
use crossterm::event::{self, Event};
use std::time::Duration;

/// Result of processing an event
#[derive(Debug, PartialEq)]
pub enum EventResult {
    /// Continue processing events
    Continue,
    /// Exit the application
    Exit,
    /// Event was handled, continue processing
    Handled,
}

/// Configuration for event handler
#[derive(Debug, Clone)]
pub struct EventHandlerConfig {
    /// Whether debug mode is enabled (affects terminal handling)
    pub debug_mode: bool,
    /// Custom poll interval override (None for adaptive)
    pub poll_interval_override: Option<Duration>,
    /// Whether to enable resize event debouncing
    pub resize_debouncing: bool,
}

impl Default for EventHandlerConfig {
    fn default() -> Self {
        Self {
            debug_mode: false,
            poll_interval_override: None,
            resize_debouncing: true,
        }
    }
}

/// Main event handler for interactive UI
pub struct EventHandler {
    config: EventHandlerConfig,
}

impl EventHandler {
    /// Create a new event handler with default configuration
    pub fn new() -> Self {
        Self {
            config: EventHandlerConfig::default(),
        }
    }

    /// Create a new event handler with custom configuration
    pub fn with_config(config: EventHandlerConfig) -> Self {
        Self { config }
    }

    /// Create event handler for debug mode
    pub fn for_debug() -> Self {
        Self::with_config(EventHandlerConfig {
            debug_mode: true,
            ..Default::default()
        })
    }

    /// Create event handler with custom poll interval
    pub fn with_poll_interval(interval: Duration) -> Self {
        Self::with_config(EventHandlerConfig {
            poll_interval_override: Some(interval),
            ..Default::default()
        })
    }

    /// Process events for one iteration of the main loop
    ///
    /// This method handles:
    /// - Event polling with adaptive intervals
    /// - Keyboard event coordination
    /// - Resize event handling with debouncing
    /// - Activity tracking in the state manager
    ///
    /// Returns EventResult indicating what action should be taken.
    pub async fn process_events(
        &self,
        state: &mut InteractiveState,
    ) -> Result<EventResult, AppError> {
        // Calculate poll interval (adaptive or override)
        let poll_interval = self
            .config
            .poll_interval_override
            .unwrap_or_else(|| calculate_poll_interval(state.time_since_activity()));

        // Check for events
        if event::poll(poll_interval)? {
            // Update activity tracking in state manager
            state.update_activity();

            // Read and process the event
            match event::read()? {
                Event::Key(key_event) => self.handle_keyboard_event(state, &key_event).await,
                Event::Resize(_, _) => {
                    self.handle_resize_event(state);
                    Ok(EventResult::Handled)
                }
                _ => Ok(EventResult::Continue),
            }
        } else {
            Ok(EventResult::Continue)
        }
    }

    /// Handle keyboard events by coordinating with the input handler
    async fn handle_keyboard_event(
        &self,
        state: &mut InteractiveState,
        key_event: &event::KeyEvent,
    ) -> Result<EventResult, AppError> {
        // Extract state variables for compatibility with existing input handler
        let mut needs_render = state.needs_render();
        let mut needs_refresh = state.needs_refresh();
        let mut current_date = state.current_date().clone();

        // Use existing input handler with extracted state
        let should_exit = handle_key_event(KeyEventParams {
            key_event,
            current_page: &mut state.ui.current_page,
            needs_render: &mut needs_render,
            needs_refresh: &mut needs_refresh,
            current_date: &mut current_date,
            last_manual_refresh: &mut state.timers.last_manual_refresh,
            last_page_change: &mut state.timers.last_page_change,
            last_date_navigation: &mut state.timers.last_date_navigation,
        })
        .await?;

        // Update state manager with any changes from input handler
        self.sync_state_after_input(state, needs_render, needs_refresh, current_date);

        if should_exit {
            Ok(EventResult::Exit)
        } else {
            Ok(EventResult::Handled)
        }
    }

    /// Handle resize events with optional debouncing
    fn handle_resize_event(&self, state: &mut InteractiveState) {
        tracing::debug!("Resize event received");

        if self.config.resize_debouncing {
            // Use debounced resize handling
            if state.timers.last_resize.elapsed() >= Duration::from_millis(500) {
                tracing::debug!("Processing debounced resize event");
                state.handle_resize();
                state.timers.update_resize();
            } else {
                tracing::debug!("Resize event ignored due to debouncing");
            }
        } else {
            // Process resize immediately
            tracing::debug!("Processing immediate resize event");
            state.handle_resize();
            state.timers.update_resize();
        }
    }

    /// Synchronize state manager after input handler operations
    fn sync_state_after_input(
        &self,
        state: &mut InteractiveState,
        needs_render: bool,
        needs_refresh: bool,
        current_date: Option<String>,
    ) {
        if needs_render {
            state.request_render();
        }
        if needs_refresh {
            state.request_refresh();
        }
        state.set_current_date(current_date);
    }

    /// Check if the application should exit based on current state
    pub fn should_exit(&self, state: &InteractiveState) -> bool {
        // This could be extended with additional exit conditions
        // For now, exit is only determined by keyboard events
        false
    }

    /// Get the current event handler configuration
    pub fn config(&self) -> &EventHandlerConfig {
        &self.config
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Event handler builder for more complex configurations
pub struct EventHandlerBuilder {
    config: EventHandlerConfig,
}

impl EventHandlerBuilder {
    /// Create a new event handler builder
    pub fn new() -> Self {
        Self {
            config: EventHandlerConfig::default(),
        }
    }

    /// Enable debug mode
    pub fn debug_mode(mut self, enabled: bool) -> Self {
        self.config.debug_mode = enabled;
        self
    }

    /// Set custom poll interval
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.config.poll_interval_override = Some(interval);
        self
    }

    /// Enable or disable resize debouncing
    pub fn resize_debouncing(mut self, enabled: bool) -> Self {
        self.config.resize_debouncing = enabled;
        self
    }

    /// Build the event handler
    pub fn build(self) -> EventHandler {
        EventHandler::with_config(self.config)
    }
}

impl Default for EventHandlerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_result_equality() {
        assert_eq!(EventResult::Continue, EventResult::Continue);
        assert_eq!(EventResult::Exit, EventResult::Exit);
        assert_eq!(EventResult::Handled, EventResult::Handled);
        assert_ne!(EventResult::Continue, EventResult::Exit);
    }

    #[test]
    fn test_event_handler_config_default() {
        let config = EventHandlerConfig::default();
        assert!(!config.debug_mode);
        assert!(config.poll_interval_override.is_none());
        assert!(config.resize_debouncing);
    }

    #[test]
    fn test_event_handler_creation() {
        let handler = EventHandler::new();
        assert!(!handler.config.debug_mode);

        let debug_handler = EventHandler::for_debug();
        assert!(debug_handler.config.debug_mode);

        let custom_handler = EventHandler::with_poll_interval(Duration::from_millis(100));
        assert_eq!(
            custom_handler.config.poll_interval_override,
            Some(Duration::from_millis(100))
        );
    }

    #[test]
    fn test_event_handler_builder() {
        let handler = EventHandlerBuilder::new()
            .debug_mode(true)
            .poll_interval(Duration::from_millis(50))
            .resize_debouncing(false)
            .build();

        assert!(handler.config.debug_mode);
        assert_eq!(
            handler.config.poll_interval_override,
            Some(Duration::from_millis(50))
        );
        assert!(!handler.config.resize_debouncing);
    }

    #[test]
    fn test_event_handler_should_exit_default() {
        let handler = EventHandler::new();
        let state = InteractiveState::new(None);

        // Default implementation should not exit
        assert!(!handler.should_exit(&state));
    }
}
