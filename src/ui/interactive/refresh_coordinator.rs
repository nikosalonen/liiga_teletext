//! Refresh coordination for interactive UI
//!
//! This module coordinates all auto-refresh operations including:
//! - Data fetching with timeout and error handling
//! - Change detection and logging
//! - Game analysis and live game tracking
//! - Cache monitoring and maintenance
//! - Backoff and retry logic coordination

use crate::data_fetcher::{GameData, fetch_liiga_data, has_live_games_from_game_data};
use crate::error::AppError;
use crate::teletext_ui::{ScoreType, TeletextPage};
use std::time::{Duration, Instant};
use tracing;

use super::change_detection::{calculate_games_hash, detect_and_log_changes};
use super::indicators::determine_indicator_states;
use super::navigation_manager::{
    LoadingIndicatorConfig, NavigationManager, PageCreationConfig, PageRestorationParams,
};
use super::refresh_manager::{
    AutoRefreshParams, calculate_auto_refresh_interval, calculate_min_refresh_interval,
    should_trigger_auto_refresh,
};
use super::state_manager::InteractiveState;

/// Result of a refresh operation
#[derive(Debug)]
pub struct RefreshResult {
    pub games: Vec<GameData>,
    pub had_error: bool,
    pub fetched_date: String,
    pub should_retry: bool,
    pub new_page: Option<TeletextPage>,
    pub needs_render: bool,
}

/// Parameters for data fetching operations
#[derive(Debug)]
pub struct DataFetchParams<'a> {
    pub current_date: &'a Option<String>,
    pub last_games: &'a [GameData],
    pub disable_links: bool,
    pub compact_mode: bool,
    pub wide_mode: bool,
    pub preserved_page_for_restoration: Option<usize>,
}

/// Configuration for refresh cycle operations
#[derive(Debug)]
pub struct RefreshCycleConfig {
    pub min_refresh_interval: Option<u64>,
    pub disable_links: bool,
    pub compact_mode: bool,
    pub wide_mode: bool,
}

/// Cache monitoring configuration
#[derive(Debug)]
pub struct CacheMonitoringConfig {
    pub cache_monitor_interval: Duration,
}

impl Default for CacheMonitoringConfig {
    fn default() -> Self {
        Self {
            cache_monitor_interval: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Coordinates all refresh operations for the interactive UI
pub struct RefreshCoordinator {
    nav_manager: NavigationManager,
    cache_config: CacheMonitoringConfig,
}

impl RefreshCoordinator {
    /// Create a new refresh coordinator
    pub fn new() -> Self {
        Self {
            nav_manager: NavigationManager::new(),
            cache_config: CacheMonitoringConfig::default(),
        }
    }

    /// Create a refresh coordinator with custom cache monitoring configuration
    pub fn with_cache_config(cache_config: CacheMonitoringConfig) -> Self {
        Self {
            nav_manager: NavigationManager::new(),
            cache_config,
        }
    }

    /// Check if auto-refresh should be triggered
    pub fn should_trigger_refresh(
        &self,
        state: &InteractiveState,
        config: &RefreshCycleConfig,
    ) -> bool {
        if !state.needs_refresh() {
            // Calculate refresh intervals
            let auto_refresh_interval =
                calculate_auto_refresh_interval(state.change_detection.last_games());
            let min_interval_between_refreshes = calculate_min_refresh_interval(
                state.change_detection.last_games().len(),
                config.min_refresh_interval,
            );

            // Debug logging for backoff enforcement
            if state.adaptive_polling.retry_backoff() > Duration::from_secs(0) {
                let backoff_remaining = state.adaptive_polling.backoff_remaining();
                if backoff_remaining > Duration::from_secs(0) {
                    tracing::trace!(
                        "Retry backoff active: {}s remaining (total backoff: {}s, elapsed since error: {}s)",
                        backoff_remaining.as_secs(),
                        state.adaptive_polling.retry_backoff().as_secs(),
                        state
                            .adaptive_polling
                            .last_backoff_hit()
                            .elapsed()
                            .as_secs()
                    );
                }
            }

            should_trigger_auto_refresh(AutoRefreshParams {
                needs_refresh: state.needs_refresh(),
                games: state.change_detection.last_games(),
                last_auto_refresh: state.timers.last_auto_refresh,
                auto_refresh_interval,
                min_interval_between_refreshes,
                last_rate_limit_hit: state.adaptive_polling.last_backoff_hit(),
                rate_limit_backoff: state.adaptive_polling.retry_backoff(),
                current_date: state.current_date(),
            })
        } else {
            true // Already flagged for refresh
        }
    }

    /// Handle data fetching with error handling and timeout
    async fn fetch_data_with_timeout(
        &self,
        current_date: Option<String>,
        timeout_duration: Duration,
    ) -> (Vec<GameData>, bool, String, bool) {
        let fetch_future = fetch_liiga_data(current_date.clone());

        match tokio::time::timeout(timeout_duration, fetch_future).await {
            Ok(fetch_result) => match fetch_result {
                Ok((games, fetched_date)) => {
                    tracing::debug!("Auto-refresh successful: fetched {} games", games.len());
                    (games, false, fetched_date, false)
                }
                Err(e) => {
                    tracing::error!("Auto-refresh failed: {}", e);
                    tracing::error!(
                        "Error details - Type: {}, Current date: {:?}",
                        std::any::type_name_of_val(&e),
                        current_date
                    );

                    // Log specific error context for debugging
                    match &e {
                        AppError::NetworkTimeout { url } => {
                            tracing::warn!(
                                "Auto-refresh timeout for URL: {}, will retry on next cycle",
                                url
                            );
                        }
                        AppError::NetworkConnection { url, message } => {
                            tracing::warn!(
                                "Auto-refresh connection error for URL: {}, details: {}, will retry on next cycle",
                                url,
                                message
                            );
                        }
                        AppError::ApiServerError {
                            status,
                            message,
                            url,
                        } => {
                            tracing::warn!(
                                "Auto-refresh server error: HTTP {} - {} (URL: {}), will retry on next cycle",
                                status,
                                message,
                                url
                            );
                        }
                        AppError::ApiServiceUnavailable {
                            status,
                            message,
                            url,
                        } => {
                            tracing::warn!(
                                "Auto-refresh service unavailable: HTTP {} - {} (URL: {}), will retry on next cycle",
                                status,
                                message,
                                url
                            );
                        }
                        AppError::ApiRateLimit { message, url } => {
                            tracing::warn!(
                                "Auto-refresh rate limited: {} (URL: {}), will retry on next cycle",
                                message,
                                url
                            );
                        }
                        _ => {
                            tracing::warn!("Auto-refresh error: {}, will retry on next cycle", e);
                        }
                    }

                    // Graceful degradation: continue with existing data instead of showing error page
                    tracing::info!("Continuing with existing data due to auto-refresh failure");

                    (Vec::new(), true, String::new(), true)
                }
            },
            Err(_) => {
                // Timeout occurred during auto-refresh
                tracing::warn!(
                    "Auto-refresh timeout after {:?}, continuing with existing data",
                    timeout_duration
                );
                (Vec::new(), true, String::new(), true)
            }
        }
    }

    /// Handle data fetching and page creation coordination
    async fn handle_data_fetching(
        &self,
        params: DataFetchParams<'_>,
    ) -> Result<RefreshResult, AppError> {
        // Determine indicator states
        let (should_show_loading, should_show_indicator) =
            determine_indicator_states(params.current_date, params.last_games);

        // Initialize page state
        let mut current_page: Option<TeletextPage> = None;
        let mut needs_render = self.nav_manager.manage_loading_indicators(
            &mut current_page,
            LoadingIndicatorConfig {
                should_show_loading,
                should_show_indicator,
                current_date: params.current_date,
                disable_links: params.disable_links,
                compact_mode: params.compact_mode,
                wide_mode: params.wide_mode,
            },
        );

        // Fetch data with timeout
        let timeout_duration = Duration::from_secs(15);
        let (games, had_error, fetched_date, should_retry) = self
            .fetch_data_with_timeout(params.current_date.clone(), timeout_duration)
            .await;

        // Update current_date to track the actual date being displayed
        let mut updated_current_date = params.current_date.clone();
        if !had_error && !fetched_date.is_empty() {
            updated_current_date = Some(fetched_date.clone());
            tracing::debug!("Updated current_date to: {:?}", updated_current_date);
        }

        // Perform change detection and logging
        let data_changed = detect_and_log_changes(&games, params.last_games);

        // Handle page creation/restoration based on data changes and errors
        // Always create a page if we have no games (to show the error message with navigation hints)
        // or if data changed and there was no error
        if (data_changed || games.is_empty()) && !had_error {
            if let Some(page) = self
                .nav_manager
                .create_or_restore_page(PageCreationConfig {
                    games: &games,
                    disable_links: params.disable_links,
                    compact_mode: params.compact_mode,
                    wide_mode: params.wide_mode,
                    fetched_date: &fetched_date,
                    preserved_page_for_restoration: params.preserved_page_for_restoration,
                    current_date: params.current_date,
                    updated_current_date: &updated_current_date,
                })
                .await
            {
                current_page = Some(page);
                needs_render = true;
            }
        } else if had_error {
            tracing::debug!(
                "Auto-refresh failed but no data changes detected, continuing with existing UI"
            );
        }

        // Handle page restoration when loading screen was shown but data didn't change
        let restoration_render = self
            .nav_manager
            .handle_page_restoration(PageRestorationParams {
                current_page: &mut current_page,
                data_changed,
                had_error,
                preserved_page_for_restoration: params.preserved_page_for_restoration,
                games: &games,
                last_games: params.last_games,
                disable_links: params.disable_links,
                fetched_date: &fetched_date,
                updated_current_date: &updated_current_date,
                compact_mode: params.compact_mode,
                wide_mode: params.wide_mode,
            })
            .await;
        needs_render = needs_render || restoration_render;

        // Hide auto-refresh indicator after data is fetched
        if let Some(page) = &mut current_page {
            page.hide_auto_refresh_indicator();
            needs_render = true;
        }

        Ok(RefreshResult {
            games,
            had_error,
            fetched_date,
            should_retry,
            new_page: current_page,
            needs_render,
        })
    }

    /// Perform comprehensive data fetching and refresh cycle
    pub async fn perform_refresh_cycle(
        &self,
        state: &mut InteractiveState,
        config: &RefreshCycleConfig,
    ) -> Result<RefreshResult, AppError> {
        tracing::debug!("Fetching new data");

        // Always preserve the current page number before refresh, regardless of loading screen
        if let Some(page) = state.current_page() {
            state.preserve_page(page.get_current_page());
        }

        // Handle data fetching using the helper function
        let result = self
            .handle_data_fetching(DataFetchParams {
                current_date: state.current_date(),
                last_games: state.change_detection.last_games(),
                disable_links: config.disable_links,
                compact_mode: config.compact_mode,
                wide_mode: config.wide_mode,
                preserved_page_for_restoration: state.preserved_page(),
            })
            .await?;

        // Update current_date to track the actual date being displayed
        if !result.had_error && !result.fetched_date.is_empty() {
            state.set_current_date(Some(result.fetched_date.clone()));
            tracing::debug!("Updated current_date to: {:?}", state.current_date());
        }

        Ok(result)
    }

    /// Process refresh results and update state
    pub fn process_refresh_results(
        &self,
        state: &mut InteractiveState,
        result: &RefreshResult,
    ) -> bool {
        let mut needs_state_render = false;

        // Change detection using a simple hash of game data
        let games_hash = calculate_games_hash(&result.games);
        let data_changed = state
            .change_detection
            .update_and_check_changes(&result.games, games_hash);

        if data_changed {
            tracing::debug!("Data changed, updating UI");

            // Log specific changes for live games to help debug game clock updates
            self.log_game_changes(state, &result.games);

            // Update the current page if we have a new one
            if let Some(new_page) = result.new_page.as_ref() {
                // We need to take ownership of the page, not clone it
                // Since we can't clone TeletextPage, we'll need to restructure this
                // For now, we'll handle this in the calling code
            }
        } else if result.had_error {
            tracing::debug!(
                "Auto-refresh failed but no data changes detected, continuing with existing UI"
            );
            if let Some(page) = state.current_page_mut() {
                page.show_error_warning();
                state.request_render();
                needs_state_render = true;
            }
        } else {
            // Track ongoing games with static time to confirm API limitations
            self.analyze_ongoing_games(&result.games);

            // Check if all games are scheduled (future games) - only relevant if no ongoing games
            self.analyze_game_schedule(&result.games);

            tracing::debug!("No data changes detected, skipping UI update");
        }

        // Update change detection variables only on successful fetch
        if !result.had_error {
            if let Some(page) = state.current_page_mut() {
                if page.is_error_warning_active() {
                    page.hide_error_warning();
                    state.request_render();
                    needs_state_render = true;
                }
            }
            state
                .change_detection
                .update_state(result.games.clone(), games_hash);
        } else {
            tracing::debug!(
                "Preserving last_games due to fetch error; will retry without clearing state"
            );
        }

        needs_state_render
    }

    /// Update refresh timing and backoff state
    pub fn update_refresh_timing(&self, state: &mut InteractiveState, should_retry: bool) {
        state.clear_refresh_flag();

        // Only update last_auto_refresh if we shouldn't retry
        // This ensures that failed auto-refresh attempts will be retried on the next cycle
        if !should_retry {
            state.timers.update_auto_refresh();
            // Reset backoff window after a successful cycle
            if state.adaptive_polling.retry_backoff() > Duration::from_secs(0) {
                tracing::debug!("Resetting retry backoff after successful refresh");
            }
            state.adaptive_polling.reset_backoff();
            tracing::trace!("Auto-refresh cycle completed successfully");
        } else {
            state.adaptive_polling.apply_backoff();
            let jittered_secs = state.adaptive_polling.retry_backoff().as_secs_f64();
            tracing::debug!("Auto-refresh failed; applying retry backoff of {jittered_secs:.2}s");
        }
    }

    /// Monitor cache usage and log statistics for long-running sessions
    pub async fn monitor_cache_usage(&self) {
        // The LRU cache automatically manages memory by evicting least recently used entries
        // when it reaches capacity. We just need to log the current state for monitoring.
        use crate::data_fetcher::cache::{get_all_cache_stats, get_detailed_cache_debug_info};

        let stats = get_all_cache_stats().await;

        tracing::debug!(
            "Cache status - Player: {}/{} ({}%), Tournament: {}/{} ({}%), Detailed Game: {}/{} ({}%), Goal Events: {}/{} ({}%), HTTP Response: {}/{} ({}%)",
            stats.player_cache.size,
            stats.player_cache.capacity,
            if stats.player_cache.capacity > 0 {
                (stats.player_cache.size * 100) / stats.player_cache.capacity
            } else {
                0
            },
            stats.tournament_cache.size,
            stats.tournament_cache.capacity,
            if stats.tournament_cache.capacity > 0 {
                (stats.tournament_cache.size * 100) / stats.tournament_cache.capacity
            } else {
                0
            },
            stats.detailed_game_cache.size,
            stats.detailed_game_cache.capacity,
            if stats.detailed_game_cache.capacity > 0 {
                (stats.detailed_game_cache.size * 100) / stats.detailed_game_cache.capacity
            } else {
                0
            },
            stats.goal_events_cache.size,
            stats.goal_events_cache.capacity,
            if stats.goal_events_cache.capacity > 0 {
                (stats.goal_events_cache.size * 100) / stats.goal_events_cache.capacity
            } else {
                0
            },
            stats.http_response_cache.size,
            stats.http_response_cache.capacity,
            if stats.http_response_cache.capacity > 0 {
                (stats.http_response_cache.size * 100) / stats.http_response_cache.capacity
            } else {
                0
            }
        );

        // Log detailed cache information for debugging if needed
        if tracing::enabled!(tracing::Level::TRACE) {
            let debug_info = get_detailed_cache_debug_info().await;
            tracing::trace!("Detailed cache debug info: {}", debug_info);
        }

        // The LRU cache implementation automatically handles memory management,
        // so we don't need manual cleanup logic anymore.
        // This ensures that the oldest/least recently used entries are always removed first.
    }

    /// Check if cache monitoring should be performed
    pub fn should_monitor_cache(&self, state: &InteractiveState) -> bool {
        state.timers.cache_monitor_timer.elapsed() >= self.cache_config.cache_monitor_interval
    }

    /// Update cache monitoring timer
    pub fn update_cache_monitor_timer(&self, state: &mut InteractiveState) {
        state.timers.update_cache_monitor();
    }

    /// Log detailed changes for live games to help debug game clock updates
    fn log_game_changes(&self, state: &InteractiveState, new_games: &[GameData]) {
        if !state.change_detection.last_games().is_empty()
            && new_games.len() == state.change_detection.last_games().len()
        {
            for (i, (new_game, old_game)) in new_games
                .iter()
                .zip(state.change_detection.last_games().iter())
                .enumerate()
            {
                if new_game.played_time != old_game.played_time
                    && new_game.score_type == ScoreType::Ongoing
                {
                    tracing::info!(
                        "Game clock update detected: Game {} - {} vs {} - time changed from {}s to {}s",
                        i + 1,
                        new_game.home_team,
                        new_game.away_team,
                        old_game.played_time,
                        new_game.played_time
                    );
                }
            }
        }
    }

    /// Analyze ongoing games with static time to confirm API limitations
    fn analyze_ongoing_games(&self, games: &[GameData]) {
        let ongoing_games: Vec<_> = games
            .iter()
            .enumerate()
            .filter(|(_, game)| game.score_type == ScoreType::Ongoing)
            .collect();

        if !ongoing_games.is_empty() {
            tracing::debug!(
                "No data changes detected despite {} ongoing game(s): {}",
                ongoing_games.len(),
                ongoing_games
                    .iter()
                    .map(|(i, game)| format!(
                        "{}. {} vs {} ({}s)",
                        i + 1,
                        game.home_team,
                        game.away_team,
                        game.played_time
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    /// Analyze game schedule for future games and auto-refresh decisions
    fn analyze_game_schedule(&self, games: &[GameData]) {
        // Check if all games are scheduled (future games) - only relevant if no ongoing games
        let has_ongoing_games = has_live_games_from_game_data(games);
        let all_scheduled = !games.is_empty()
            && games
                .iter()
                .all(|game| self.nav_manager.is_future_game(game));

        if all_scheduled && !has_ongoing_games {
            tracing::info!("All games are scheduled - auto-refresh disabled");
        } else if has_ongoing_games {
            tracing::info!("Ongoing games detected - auto-refresh enabled");
        }
    }
}

impl Default for RefreshCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refresh_coordinator_creation() {
        let coordinator = RefreshCoordinator::new();
        // RefreshCoordinator should be created successfully
        // This is a basic test to ensure the struct can be instantiated
        assert_eq!(std::mem::size_of_val(&coordinator.nav_manager), 0); // NavigationManager is zero-sized
    }

    #[test]
    fn test_refresh_coordinator_default() {
        let coordinator = RefreshCoordinator::default();
        // Should be equivalent to RefreshCoordinator::new()
        assert_eq!(std::mem::size_of_val(&coordinator.nav_manager), 0);
        assert_eq!(
            coordinator.cache_config.cache_monitor_interval,
            Duration::from_secs(300)
        );
    }

    #[test]
    fn test_refresh_coordinator_with_cache_config() {
        let custom_config = CacheMonitoringConfig {
            cache_monitor_interval: Duration::from_secs(600),
        };
        let coordinator = RefreshCoordinator::with_cache_config(custom_config);
        assert_eq!(
            coordinator.cache_config.cache_monitor_interval,
            Duration::from_secs(600)
        );
    }

    #[test]
    fn test_cache_monitoring_config_default() {
        let config = CacheMonitoringConfig::default();
        assert_eq!(config.cache_monitor_interval, Duration::from_secs(300));
    }

    #[test]
    fn test_data_fetch_params() {
        let current_date = Some("2024-01-15".to_string());
        let games = vec![];
        let params = DataFetchParams {
            current_date: &current_date,
            last_games: &games,
            disable_links: false,
            compact_mode: false,
            wide_mode: false,
            preserved_page_for_restoration: None,
        };

        assert_eq!(params.current_date, &Some("2024-01-15".to_string()));
        assert!(!params.disable_links);
        assert!(!params.compact_mode);
        assert!(!params.wide_mode);
        assert_eq!(params.preserved_page_for_restoration, None);
    }

    #[test]
    fn test_refresh_cycle_config() {
        let config = RefreshCycleConfig {
            min_refresh_interval: Some(10),
            disable_links: true,
            compact_mode: false,
            wide_mode: true,
        };

        assert_eq!(config.min_refresh_interval, Some(10));
        assert!(config.disable_links);
        assert!(!config.compact_mode);
        assert!(config.wide_mode);
    }
}
