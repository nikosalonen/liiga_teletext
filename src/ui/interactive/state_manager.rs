//! State management for interactive UI
//!
//! This module provides structured state management for the interactive UI,
//! organizing different types of state into logical groupings and providing
//! clean interfaces for state operations.

use crate::data_fetcher::GameData;
use crate::teletext_ui::TeletextPage;
use std::time::{Duration, Instant};

/// Timer state for various interactive UI operations
#[derive(Debug)]
pub struct TimerState {
    pub last_manual_refresh: Instant,
    pub last_auto_refresh: Instant,
    pub last_page_change: Instant,
    pub last_date_navigation: Instant,
    pub last_resize: Instant,
    pub last_activity: Instant,
    pub cache_monitor_timer: Instant,
    #[allow(dead_code)]
    pub last_rate_limit_hit: Instant,
}

impl TimerState {
    /// Initialize all timers with appropriate default values
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            last_manual_refresh: now.checked_sub(Duration::from_secs(15)).unwrap_or(now),
            last_auto_refresh: now.checked_sub(Duration::from_secs(10)).unwrap_or(now),
            last_page_change: now.checked_sub(Duration::from_millis(200)).unwrap_or(now),
            last_date_navigation: now.checked_sub(Duration::from_millis(250)).unwrap_or(now),
            last_resize: now.checked_sub(Duration::from_millis(500)).unwrap_or(now),
            last_activity: now,
            cache_monitor_timer: now,
            last_rate_limit_hit: now.checked_sub(Duration::from_secs(60)).unwrap_or(now),
        }
    }

    /// Update activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Get time since last activity
    pub fn time_since_activity(&self) -> Duration {
        self.last_activity.elapsed()
    }

    /// Update manual refresh timestamp
    #[allow(dead_code)]
    pub fn update_manual_refresh(&mut self) {
        self.last_manual_refresh = Instant::now();
    }

    /// Update auto refresh timestamp
    pub fn update_auto_refresh(&mut self) {
        self.last_auto_refresh = Instant::now();
    }

    /// Update page change timestamp
    #[allow(dead_code)]
    pub fn update_page_change(&mut self) {
        self.last_page_change = Instant::now();
    }

    /// Update date navigation timestamp
    #[allow(dead_code)]
    pub fn update_date_navigation(&mut self) {
        self.last_date_navigation = Instant::now();
    }

    /// Update resize timestamp
    pub fn update_resize(&mut self) {
        self.last_resize = Instant::now();
    }

    /// Update cache monitor timestamp
    pub fn update_cache_monitor(&mut self) {
        self.cache_monitor_timer = Instant::now();
    }
}

impl Default for TimerState {
    fn default() -> Self {
        Self::new()
    }
}

/// UI rendering and interaction state
#[derive(Debug)]
pub struct UIState {
    pub needs_refresh: bool,
    pub needs_render: bool,
    pub current_page: Option<TeletextPage>,
    pub pending_resize: bool,
    pub resize_timer: Instant,
}

impl UIState {
    /// Create new UI state
    pub fn new() -> Self {
        Self {
            needs_refresh: true,
            needs_render: false,
            current_page: None,
            pending_resize: false,
            resize_timer: Instant::now(),
        }
    }

    /// Mark that a refresh is needed
    pub fn request_refresh(&mut self) {
        self.needs_refresh = true;
    }

    /// Mark that a render is needed
    pub fn request_render(&mut self) {
        self.needs_render = true;
    }

    /// Clear refresh flag
    pub fn clear_refresh_flag(&mut self) {
        self.needs_refresh = false;
    }

    /// Clear render flag
    pub fn clear_render_flag(&mut self) {
        self.needs_render = false;
    }

    /// Check if refresh is needed
    pub fn needs_refresh(&self) -> bool {
        self.needs_refresh
    }

    /// Check if render is needed
    pub fn needs_render(&self) -> bool {
        self.needs_render
    }

    /// Set current page
    pub fn set_current_page(&mut self, page: TeletextPage) {
        self.current_page = Some(page);
        self.request_render();
    }

    /// Get current page reference
    pub fn current_page(&self) -> Option<&TeletextPage> {
        self.current_page.as_ref()
    }

    /// Get mutable current page reference
    pub fn current_page_mut(&mut self) -> Option<&mut TeletextPage> {
        self.current_page.as_mut()
    }

    /// Handle resize event
    pub fn handle_resize(&mut self) {
        self.pending_resize = true;
        self.resize_timer = Instant::now();

        // Immediately update the current page's layout and trigger re-render
        if let Some(page) = &mut self.current_page {
            page.handle_resize();
        }

        // Request immediate re-render to show the updated layout
        self.needs_render = true;
    }

    /// Check if resize is ready to process
    #[allow(dead_code)]
    pub fn is_resize_ready(&self) -> bool {
        self.pending_resize && self.resize_timer.elapsed() >= Duration::from_millis(500)
    }

    /// Clear resize flag
    #[allow(dead_code)]
    pub fn clear_resize_flag(&mut self) {
        self.pending_resize = false;
    }
}

impl Default for UIState {
    fn default() -> Self {
        Self::new()
    }
}

/// Navigation state for date and page management
#[derive(Debug, Clone)]
pub struct NavigationState {
    pub current_date: Option<String>,
    pub preserved_page_for_restoration: Option<usize>,
}

impl NavigationState {
    /// Create new navigation state
    pub fn new(initial_date: Option<String>) -> Self {
        Self {
            current_date: initial_date,
            preserved_page_for_restoration: None,
        }
    }

    /// Set current date
    pub fn set_current_date(&mut self, date: Option<String>) {
        self.current_date = date;
    }

    /// Get current date reference
    pub fn current_date(&self) -> &Option<String> {
        &self.current_date
    }

    /// Preserve current page number for restoration
    pub fn preserve_page(&mut self, page_number: usize) {
        self.preserved_page_for_restoration = Some(page_number);
    }

    /// Get preserved page number and clear it
    #[allow(dead_code)]
    pub fn take_preserved_page(&mut self) -> Option<usize> {
        self.preserved_page_for_restoration.take()
    }

    /// Get preserved page number without clearing
    pub fn preserved_page(&self) -> Option<usize> {
        self.preserved_page_for_restoration
    }
}

impl Default for NavigationState {
    fn default() -> Self {
        Self::new(None)
    }
}

/// Change detection state for data comparison
#[derive(Debug)]
pub struct ChangeDetectionState {
    pub last_games_hash: u64,
    pub last_games: Vec<GameData>,
}

impl ChangeDetectionState {
    /// Create new change detection state
    pub fn new() -> Self {
        Self {
            last_games_hash: 0,
            last_games: Vec::new(),
        }
    }

    /// Update state with new game data and return if data changed
    pub fn update_and_check_changes(&mut self, games: &[GameData], new_hash: u64) -> bool {
        let data_changed = new_hash != self.last_games_hash;
        if data_changed {
            self.last_games_hash = new_hash;
            self.last_games = games.to_vec();
        }
        data_changed
    }

    /// Update state without checking for changes (used after successful fetch)
    pub fn update_state(&mut self, games: Vec<GameData>, new_hash: u64) {
        self.last_games_hash = new_hash;
        self.last_games = games;
    }

    /// Get last games reference
    pub fn last_games(&self) -> &[GameData] {
        &self.last_games
    }

    /// Get last games hash
    #[allow(dead_code)]
    pub fn last_games_hash(&self) -> u64 {
        self.last_games_hash
    }
}

impl Default for ChangeDetectionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Adaptive polling and retry state
#[derive(Debug)]
pub struct AdaptivePollingState {
    pub retry_backoff: Duration,
    pub last_backoff_hit: Instant,
}

impl AdaptivePollingState {
    /// Create new adaptive polling state
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            retry_backoff: Duration::from_secs(0),
            last_backoff_hit: now.checked_sub(Duration::from_secs(60)).unwrap_or(now),
        }
    }

    /// Apply retry backoff after an error
    pub fn apply_backoff(&mut self) {
        let base_next = if self.retry_backoff.is_zero() {
            Duration::from_secs(2)
        } else {
            self.retry_backoff.saturating_mul(2)
        };

        // Cap the backoff at 10 seconds (base before jitter)
        let capped_next = std::cmp::min(base_next, Duration::from_secs(10));

        // Apply ±20% jitter to avoid synchronized retries across clients
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let rand_fraction = (nanos % 1000) as f64 / 1000.0; // 0.0..1.0
        let jitter_factor = 0.8_f64 + 0.4_f64 * rand_fraction; // 0.8..1.2
        let jittered_secs = (capped_next.as_secs_f64() * jitter_factor).min(10.0);

        self.retry_backoff = Duration::from_secs_f64(jittered_secs);
        self.last_backoff_hit = Instant::now();
    }

    /// Reset backoff after successful operation
    pub fn reset_backoff(&mut self) {
        self.retry_backoff = Duration::from_secs(0);
    }

    /// Check if currently in backoff period
    #[allow(dead_code)]
    pub fn is_in_backoff(&self) -> bool {
        if self.retry_backoff.is_zero() {
            false
        } else {
            let backoff_remaining = self
                .retry_backoff
                .saturating_sub(self.last_backoff_hit.elapsed());
            backoff_remaining > Duration::from_secs(0)
        }
    }

    /// Get remaining backoff duration
    pub fn backoff_remaining(&self) -> Duration {
        if self.retry_backoff.is_zero() {
            Duration::from_secs(0)
        } else {
            self.retry_backoff
                .saturating_sub(self.last_backoff_hit.elapsed())
        }
    }

    /// Get retry backoff duration
    pub fn retry_backoff(&self) -> Duration {
        self.retry_backoff
    }

    /// Get last backoff hit timestamp
    pub fn last_backoff_hit(&self) -> Instant {
        self.last_backoff_hit
    }
}

impl Default for AdaptivePollingState {
    fn default() -> Self {
        Self::new()
    }
}

/// Main interactive state coordinator
#[derive(Debug)]
pub struct InteractiveState {
    pub timers: TimerState,
    pub ui: UIState,
    pub navigation: NavigationState,
    pub change_detection: ChangeDetectionState,
    pub adaptive_polling: AdaptivePollingState,
}

impl InteractiveState {
    /// Create new interactive state
    pub fn new(initial_date: Option<String>) -> Self {
        Self {
            timers: TimerState::new(),
            ui: UIState::new(),
            navigation: NavigationState::new(initial_date),
            change_detection: ChangeDetectionState::new(),
            adaptive_polling: AdaptivePollingState::new(),
        }
    }

    /// Update activity (delegates to timer state)
    pub fn update_activity(&mut self) {
        self.timers.update_activity();
    }

    /// Get time since last activity (delegates to timer state)
    pub fn time_since_activity(&self) -> Duration {
        self.timers.time_since_activity()
    }

    /// Request refresh (delegates to UI state)
    pub fn request_refresh(&mut self) {
        self.ui.request_refresh();
    }

    /// Check if refresh is needed (delegates to UI state)
    pub fn needs_refresh(&self) -> bool {
        self.ui.needs_refresh()
    }

    /// Clear refresh flag (delegates to UI state)
    pub fn clear_refresh_flag(&mut self) {
        self.ui.clear_refresh_flag();
    }

    /// Check if render is needed (delegates to UI state)
    pub fn needs_render(&self) -> bool {
        self.ui.needs_render()
    }

    /// Request render (delegates to UI state)
    pub fn request_render(&mut self) {
        self.ui.request_render();
    }

    /// Clear render flag (delegates to UI state)
    pub fn clear_render_flag(&mut self) {
        self.ui.clear_render_flag();
    }

    /// Set current page (delegates to UI state)
    pub fn set_current_page(&mut self, page: TeletextPage) {
        self.ui.set_current_page(page);
    }

    /// Get current page reference (delegates to UI state)
    pub fn current_page(&self) -> Option<&TeletextPage> {
        self.ui.current_page()
    }

    /// Get mutable current page reference (delegates to UI state)
    pub fn current_page_mut(&mut self) -> Option<&mut TeletextPage> {
        self.ui.current_page_mut()
    }

    /// Handle resize event (delegates to UI state)
    pub fn handle_resize(&mut self) {
        self.ui.handle_resize();
    }

    /// Check if resize is ready (delegates to UI state)
    #[allow(dead_code)]
    pub fn is_resize_ready(&self) -> bool {
        self.ui.is_resize_ready()
    }

    /// Clear resize flag (delegates to UI state)
    #[allow(dead_code)]
    pub fn clear_resize_flag(&mut self) {
        self.ui.clear_resize_flag();
    }

    /// Set current date (delegates to navigation state)
    pub fn set_current_date(&mut self, date: Option<String>) {
        self.navigation.set_current_date(date);
    }

    /// Get current date reference (delegates to navigation state)
    pub fn current_date(&self) -> &Option<String> {
        self.navigation.current_date()
    }

    /// Preserve current page number (delegates to navigation state)
    pub fn preserve_page(&mut self, page_number: usize) {
        self.navigation.preserve_page(page_number);
    }

    /// Get preserved page number and clear it (delegates to navigation state)
    #[allow(dead_code)]
    pub fn take_preserved_page(&mut self) -> Option<usize> {
        self.navigation.take_preserved_page()
    }

    /// Get preserved page number without clearing (delegates to navigation state)
    pub fn preserved_page(&self) -> Option<usize> {
        self.navigation.preserved_page()
    }
}

impl Default for InteractiveState {
    fn default() -> Self {
        Self::new(None)
    }
}
