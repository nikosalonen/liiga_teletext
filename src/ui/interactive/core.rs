//! Interactive UI module for the liiga_teletext application
//!
//! This module contains the main interactive UI loop and all UI-related helper functions.
//! It handles user input, screen updates, page creation, and the main application flow.

use crate::error::AppError;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::io::stdout;
use std::time::Duration;

// Import utilities from sibling modules
use super::state_manager::InteractiveState;
use super::event_handler::{EventHandler, EventHandlerBuilder, EventResult};
use super::refresh_coordinator::{RefreshCoordinator, RefreshCycleConfig};

// Teletext page constants (removed unused constants)

// UI timing constants (removed unused constants)









/// Parameters for keyboard event handling
/// Handle keyboard events

/// Setup terminal for interactive mode
fn setup_terminal(debug_mode: bool) -> Result<std::io::Stdout, AppError> {
    let mut stdout = stdout();

    if !debug_mode {
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen)?;
    }

    Ok(stdout)
}

/// Cleanup terminal after interactive mode
fn cleanup_terminal(debug_mode: bool, mut stdout: std::io::Stdout) -> Result<(), AppError> {
    if !debug_mode {
        disable_raw_mode()?;
        execute!(stdout, LeaveAlternateScreen)?;
    }
    Ok(())
}

/// Runs the interactive UI with adaptive polling and change detection
pub async fn run_interactive_ui(
    date: Option<String>,
    disable_links: bool,
    debug_mode: bool,
    min_refresh_interval: Option<u64>,
    compact_mode: bool,
    wide_mode: bool,
) -> Result<(), AppError> {
    // Setup terminal for interactive mode
    let mut stdout = setup_terminal(debug_mode)?;
    
    // Initialize all state through the state manager
    let mut state = InteractiveState::new(date);
    
    // Create event handler with appropriate configuration
    let event_handler = if debug_mode {
        EventHandler::for_debug()
    } else {
        EventHandler::new()
    };
    
    // Create refresh coordinator
    let refresh_coordinator = RefreshCoordinator::new();
    
    // Create refresh cycle configuration
    let refresh_config = RefreshCycleConfig {
        min_refresh_interval,
        disable_links,
        compact_mode,
        wide_mode,
    };

    loop {
        // Check if auto-refresh should be triggered
        if refresh_coordinator.should_trigger_refresh(&state, &refresh_config) {
            state.request_refresh();
        }

        // Data fetching with change detection using RefreshCoordinator
        if state.needs_refresh() {
            // Perform comprehensive refresh cycle
            let mut refresh_result = refresh_coordinator.perform_refresh_cycle(&mut state, &refresh_config).await?;
            
            // Update the current page if we have a new one (must be done before processing results)
            if let Some(new_page) = refresh_result.new_page.take() {
                state.set_current_page(new_page);
            }
            
            // Process refresh results and update state
            let needs_state_render = refresh_coordinator.process_refresh_results(&mut state, &refresh_result);
            if needs_state_render {
                // State render was already requested by process_refresh_results
            }
            
            // Update refresh timing and backoff state
            refresh_coordinator.update_refresh_timing(&mut state, refresh_result.should_retry);
        }

        // Update auto-refresh indicator animation (only when active)
        if let Some(page) = state.current_page_mut()
            && page.is_auto_refresh_indicator_active()
        {
            page.update_auto_refresh_animation();
            state.request_render();
        }

        // Batched UI rendering - only render when necessary
        // Use buffered rendering to minimize flickering
        if state.needs_render() {
            if let Some(page) = state.current_page() {
                page.render_buffered(&mut stdout)?;
                tracing::debug!("UI rendered with buffering");
            }
            state.clear_render_flag();
        }

        // Process events using the event handler
        match event_handler.process_events(&mut state).await? {
            EventResult::Exit => {
                tracing::info!("Exit requested through event handler");
                break;
            }
            EventResult::Handled | EventResult::Continue => {
                // Continue with the loop
            }
        }

        // Periodic cache monitoring for long-running sessions
        if refresh_coordinator.should_monitor_cache(&state) {
            tracing::debug!("Monitoring cache usage");
            refresh_coordinator.monitor_cache_usage().await;
            refresh_coordinator.update_cache_monitor_timer(&mut state);
        }

        // Small sleep to prevent tight loops when not processing events
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Cleanup terminal
    cleanup_terminal(debug_mode, stdout)?;
    Ok(())
}

