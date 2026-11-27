//! Event handling coordination for interactive UI
//!
//! This module provides unified event handling for the interactive UI,
//! coordinating between different event types (keyboard, resize, etc.)
//! and managing their interaction with the state manager.

use super::input_handler::{KeyEventParams, handle_key_event};
use super::refresh_manager::calculate_poll_interval;
use super::state_manager::{InteractiveState, ViewMode};
use crate::error::AppError;
use crossterm::event::{self, Event};

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

/// Main event handler for interactive UI
#[derive(Default)]
pub struct EventHandler;

impl EventHandler {
    /// Create a new event handler
    pub fn new() -> Self {
        Self
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
        let poll_interval = calculate_poll_interval(state.time_since_activity());

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
        let previous_view = state.current_view();
        let mut current_view = previous_view;
        let mut preserved_games_page = state.navigation.preserved_games_page;
        let mut preserved_live_mode = state.navigation.preserved_live_mode;
        let previous_cursor = state.navigation.calendar_cursor;
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
            current_view: &mut current_view,
            preserved_games_page: &mut preserved_games_page,
            preserved_live_mode: &mut preserved_live_mode,
            has_bracket_data: state.navigation.has_bracket_data,
            page_input: &mut state.navigation.page_input,
            last_page_input: &mut state.timers.last_page_input,
            calendar_cursor: &mut state.navigation.calendar_cursor,
            calendar: state.change_detection.last_calendar(),
        })
        .await?;

        // Update state manager with any changes from input handler
        self.sync_state_after_input(state, needs_render, needs_refresh, current_date);

        // Reset standings hash when switching away from standings view so that
        // re-entering standings always shows a full loading screen, not a spinner
        // on the games page.
        if matches!(previous_view, ViewMode::Standings { .. })
            && !matches!(current_view, ViewMode::Standings { .. })
        {
            state.change_detection.reset_standings_hash();
        }

        // Reset bracket hash when switching away from bracket view
        if matches!(previous_view, ViewMode::Bracket) && current_view != ViewMode::Bracket {
            state.change_detection.reset_bracket_hash();
        }

        state.navigation.current_view = current_view;
        state.navigation.preserved_games_page = preserved_games_page;
        state.navigation.preserved_live_mode = preserved_live_mode;

        // Draw the calendar when entering it or when the cursor moved; the
        // calendar keys themselves only move the cursor
        if current_view == ViewMode::Calendar
            && (previous_view != ViewMode::Calendar
                || state.navigation.calendar_cursor != previous_cursor)
        {
            let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
            state.show_calendar_at_cursor(width, height);
        }

        if should_exit {
            Ok(EventResult::Exit)
        } else {
            Ok(EventResult::Handled)
        }
    }

    /// Handle resize events. The resize itself is debounced: the main loop
    /// processes it once resize events stop coming.
    fn handle_resize_event(&self, state: &mut InteractiveState) {
        tracing::debug!("Resize event received");

        // Track the most recent resize event for the debounce timer
        state.timers.update_resize();
        state.ui.pending_resize = true;

        tracing::debug!("Resize event marked as pending (will be processed after debounce period)");
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
}
