//! Refresh coordination for interactive UI
//!
//! This module coordinates all auto-refresh operations including:
//! - Data fetching with timeout and error handling
//! - Change detection and logging
//! - Game analysis and live game tracking
//! - Cache monitoring and maintenance
//! - Backoff and retry logic coordination

use crate::data_fetcher::api::standings_api::fetch_standings;
use crate::data_fetcher::{GameData, fetch_liiga_data, has_live_games_from_game_data};
use crate::error::AppError;
use crate::teletext_ui::{ScoreType, TeletextPage};
use std::time::Duration;
use tracing;

use super::change_detection::{
    calculate_games_hash, calculate_standings_hash, detect_and_log_changes,
};
use super::indicators::determine_indicator_states;
use super::navigation_manager::{
    self, LoadingIndicatorConfig, PageCreationConfig, PageRestorationParams,
};
use super::refresh_manager::{
    AutoRefreshParams, calculate_auto_refresh_interval, calculate_min_refresh_interval,
    should_trigger_auto_refresh,
};
use super::state_manager::{InteractiveState, ViewMode};

/// Maximum consecutive transient empty API responses before accepting the empty
/// state as legitimate. Prevents showing stale data indefinitely while still
/// protecting against brief API glitches.
const MAX_TRANSIENT_EMPTY: u32 = 3;

/// Result of a refresh operation
#[derive(Debug)]
pub struct RefreshResult {
    pub games: Vec<GameData>,
    pub had_error: bool,
    pub fetched_date: String,
    pub should_retry: bool,
    pub new_page: Option<TeletextPage>,
    #[allow(dead_code)]
    pub needs_render: bool,
    /// True when `process_refresh_results` should skip change detection.
    /// Set for date-mismatch discards (to avoid clearing game state),
    /// standings results (which carry no game data), and transient empty
    /// API responses (to preserve the existing display).
    pub skip_change_detection: bool,
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
    /// When true, the user navigated to a different date — transient-empty
    /// preservation should be skipped because `last_games` belongs to the
    /// previous date and is not relevant.
    pub is_date_change: bool,
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

/// Check if fetched data should be discarded due to a date mismatch.
/// Returns true when a date is already set and the fetched date differs.
fn should_discard_for_date_mismatch(current_date: &Option<String>, fetched_date: &str) -> bool {
    current_date
        .as_deref()
        .is_some_and(|d| !d.trim().is_empty() && d.trim() != fetched_date.trim())
}

/// Perform the network fetch for game data with timeout.
/// This is a free function so it can be spawned as a background task,
/// allowing the UI to animate the loading indicator while waiting.
async fn fetch_games_with_timeout(
    current_date: Option<String>,
    timeout_seconds: u64,
) -> (Vec<GameData>, bool, String, bool) {
    let timeout_duration = Duration::from_secs(timeout_seconds);
    let fetch_future = fetch_liiga_data(current_date.clone());

    match tokio::time::timeout(timeout_duration, fetch_future).await {
        Ok(Ok((games, fetched_date))) => {
            tracing::debug!("Auto-refresh successful: fetched {} games", games.len());
            (games, false, fetched_date, false)
        }
        Ok(Err(e)) => {
            log_fetch_error(&e, &current_date);
            (Vec::new(), true, String::new(), true)
        }
        Err(_) => {
            tracing::warn!(
                "Auto-refresh timeout after {timeout_seconds}s, continuing with existing data"
            );
            (Vec::new(), true, String::new(), true)
        }
    }
}

/// Log fetch error details for debugging
fn log_fetch_error(e: &AppError, current_date: &Option<String>) {
    tracing::error!("Auto-refresh failed: {e}");
    tracing::error!(
        "Error details - Type: {}, Current date: {current_date:?}",
        std::any::type_name_of_val(e),
    );

    match e {
        AppError::NetworkTimeout { url } => {
            tracing::warn!("Auto-refresh timeout for URL: {url}, will retry on next cycle");
        }
        AppError::NetworkConnection { url, message } => {
            tracing::warn!(
                "Auto-refresh connection error for URL: {url}, details: {message}, will retry on next cycle"
            );
        }
        AppError::ApiServerError {
            status,
            message,
            url,
        } => {
            tracing::warn!(
                "Auto-refresh server error: HTTP {status} - {message} (URL: {url}), will retry on next cycle"
            );
        }
        AppError::ApiServiceUnavailable {
            status,
            message,
            url,
        } => {
            tracing::warn!(
                "Auto-refresh service unavailable: HTTP {status} - {message} (URL: {url}), will retry on next cycle"
            );
        }
        AppError::ApiRateLimit { message, url } => {
            tracing::warn!(
                "Auto-refresh rate limited: {message} (URL: {url}), will retry on next cycle"
            );
        }
        _ => {
            tracing::warn!("Auto-refresh error: {e}, will retry on next cycle");
        }
    }

    tracing::info!("Continuing with existing data due to auto-refresh failure");
}

/// Animate the loading spinner on the current page while waiting for a background fetch task.
/// Uses `tokio::select!` to alternate between checking the task and advancing the animation.
type FetchResult = (Vec<GameData>, bool, String, bool);

async fn animate_during_fetch(
    state: &mut InteractiveState,
    mut handle: tokio::task::JoinHandle<FetchResult>,
) -> FetchResult {
    let mut stdout = std::io::stdout();

    loop {
        tokio::select! {
            result = &mut handle => {
                match result {
                    Ok(value) => return value,
                    Err(join_error) => {
                        tracing::error!("Background fetch task failed: {join_error}");
                        return (Vec::new(), true, String::new(), true);
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if let Some(page) = state.current_page_mut()
                    && page.is_auto_refresh_indicator_active()
                {
                    page.update_auto_refresh_animation();
                    let _ = page.render_buffered(&mut stdout);
                }
            }
        }
    }
}

/// Coordinates all refresh operations for the interactive UI
pub struct RefreshCoordinator {
    cache_config: CacheMonitoringConfig,
    /// Tracks how many consecutive refresh cycles returned empty games while
    /// previous games existed.  After a threshold we stop treating the empty
    /// response as transient so stale data is not shown indefinitely.
    consecutive_transient_empty: u32,
}

impl RefreshCoordinator {
    /// Create a new refresh coordinator
    pub fn new() -> Self {
        Self {
            cache_config: CacheMonitoringConfig::default(),
            consecutive_transient_empty: 0,
        }
    }

    /// Create a refresh coordinator with custom cache monitoring configuration
    #[allow(dead_code)]
    pub fn with_cache_config(cache_config: CacheMonitoringConfig) -> Self {
        Self {
            cache_config,
            consecutive_transient_empty: 0,
        }
    }

    /// Reset the transient empty counter.
    /// Should be called when the user navigates to a different date so that
    /// the counter doesn't carry over stale state from the previous date.
    pub fn reset_transient_empty_counter(&mut self) {
        if self.consecutive_transient_empty > 0 {
            tracing::debug!(
                "Resetting transient empty counter (was {})",
                self.consecutive_transient_empty
            );
            self.consecutive_transient_empty = 0;
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
            let is_standings_live = matches!(
                state.current_view(),
                ViewMode::Standings { live_mode: true }
            );
            let is_bracket = matches!(state.current_view(), ViewMode::Bracket);
            let (auto_refresh_interval, game_count_for_min_interval) = if is_standings_live {
                (
                    Duration::from_secs(crate::constants::refresh::LIVE_GAMES_INTERVAL_SECONDS),
                    0,
                )
            } else if is_bracket {
                (Duration::from_secs(60), 0)
            } else {
                (
                    calculate_auto_refresh_interval(state.change_detection.last_games()),
                    state.change_detection.last_games().len(),
                )
            };
            let min_interval_between_refreshes = calculate_min_refresh_interval(
                game_count_for_min_interval,
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

    /// Process fetched game data: change detection, page creation, and indicator cleanup.
    /// The actual network fetch is performed separately so it can run as a background task.
    async fn process_fetched_data(
        &mut self,
        params: DataFetchParams<'_>,
        games: Vec<GameData>,
        had_error: bool,
        fetched_date: String,
        should_retry: bool,
    ) -> Result<RefreshResult, AppError> {
        // Determine indicator states
        let (should_show_loading, _) =
            determine_indicator_states(params.current_date, params.last_games);

        // Initialize page state
        let mut current_page: Option<TeletextPage> = None;
        let mut needs_render = navigation_manager::manage_loading_indicators(
            &mut current_page,
            LoadingIndicatorConfig {
                should_show_loading,
                current_date: params.current_date,
                disable_links: params.disable_links,
                compact_mode: params.compact_mode,
                wide_mode: params.wide_mode,
            },
        );

        // Prepare the effective date for page creation (may differ from requested date on first fetch)
        let mut updated_current_date = params.current_date.clone();
        if !had_error && !fetched_date.is_empty() {
            updated_current_date = Some(fetched_date.clone());
            tracing::debug!("Updated current_date to: {:?}", updated_current_date);
        }

        // Reset the transient-empty streak on fetch errors so intermittent failures
        // don't count toward the consecutive empty counter.
        if had_error {
            self.consecutive_transient_empty = 0;
        }

        // Short-circuit transient empty responses before change detection to avoid
        // unnecessary work.  If the API returns empty games but we previously had
        // games, preserve the existing display — unless this has happened too many
        // times in a row, which indicates the empty state is permanent.
        if !had_error && games.is_empty() && !params.last_games.is_empty() && !params.is_date_change
        {
            self.consecutive_transient_empty += 1;
            if self.consecutive_transient_empty <= MAX_TRANSIENT_EMPTY {
                tracing::warn!(
                    "API returned empty games but we previously had {} games, preserving existing display ({}/{})",
                    params.last_games.len(),
                    self.consecutive_transient_empty,
                    MAX_TRANSIENT_EMPTY,
                );
                return Ok(RefreshResult {
                    games: params.last_games.to_vec(),
                    had_error,
                    fetched_date,
                    should_retry: false,
                    new_page: None,
                    needs_render: false,
                    skip_change_detection: true,
                });
            }
            tracing::warn!(
                "Empty games response persisted for {} consecutive refreshes, accepting as legitimate",
                self.consecutive_transient_empty,
            );
            // Reset counter after accepting empty as legitimate to avoid unbounded growth
            // and repeated warn! logs on every subsequent refresh.
            self.consecutive_transient_empty = 0;
            // Fall through to normal processing — the empty state will be shown to the user.
        }
        // Reset counter on any non-empty response.
        if !games.is_empty() {
            self.consecutive_transient_empty = 0;
        }

        // Perform change detection and logging
        let data_changed = detect_and_log_changes(&games, params.last_games);

        // Handle page creation/restoration based on data changes and errors
        // Always create a page if we have no games (to show the error message with navigation hints)
        // or if data changed and there was no error.
        if (data_changed || games.is_empty()) && !had_error {
            if let Some(page) = navigation_manager::create_or_restore_page(PageCreationConfig {
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
        let restoration_render =
            navigation_manager::handle_page_restoration(PageRestorationParams {
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

        Ok(RefreshResult {
            games,
            had_error,
            fetched_date,
            should_retry,
            new_page: current_page,
            needs_render,
            skip_change_detection: false,
        })
    }

    /// Perform comprehensive data fetching and refresh cycle
    pub async fn perform_refresh_cycle(
        &mut self,
        state: &mut InteractiveState,
        config: &RefreshCycleConfig,
        is_date_change: bool,
    ) -> Result<RefreshResult, AppError> {
        tracing::debug!("Fetching new data");

        // Always preserve the current page number before refresh, regardless of loading screen
        if let Some(page) = state.current_page() {
            state.preserve_page(page.get_current_page());
        }

        // Branch on view mode. Standings and Bracket are league-wide (not date-scoped),
        // so the date-mismatch discard in perform_refresh_cycle does not apply here.
        if matches!(state.current_view(), ViewMode::Bracket) {
            let preserved_page = state.preserved_page();
            return self
                .perform_bracket_refresh(state, config, preserved_page)
                .await;
        }

        if let ViewMode::Standings { live_mode } = state.current_view() {
            let preserved_page = state.preserved_page();
            let last_games = state.change_detection.last_games().to_vec();
            return self
                .perform_standings_refresh(state, config, live_mode, preserved_page, &last_games)
                .await;
        }

        // Restore preserved games page when switching back from standings.
        // If we have cached game data, rebuild the page immediately instead
        // of doing a full network fetch. The next auto-refresh cycle will
        // update the data if needed.
        if let Some(preserved) = state.navigation.preserved_games_page.take() {
            state.navigation.preserved_page_for_restoration = Some(preserved);

            let last_games = state.change_detection.last_games();
            if !last_games.is_empty() {
                let fetched_date = state.current_date().clone().unwrap_or_default();
                let games: Vec<_> = last_games.to_vec();
                let new_page = navigation_manager::create_or_restore_page(PageCreationConfig {
                    games: &games,
                    disable_links: config.disable_links,
                    compact_mode: config.compact_mode,
                    wide_mode: config.wide_mode,
                    fetched_date: &fetched_date,
                    preserved_page_for_restoration: state.preserved_page(),
                    current_date: state.current_date(),
                    updated_current_date: state.current_date(),
                })
                .await;

                tracing::info!(
                    "Restored games page from cached data ({} games)",
                    games.len()
                );
                return Ok(RefreshResult {
                    games,
                    had_error: false,
                    fetched_date,
                    should_retry: false,
                    new_page,
                    needs_render: true,
                    skip_change_detection: false,
                });
            }
        }

        // Show auto-refresh spinner on the current page before fetching
        let should_show_indicator = {
            let (_, show) = determine_indicator_states(
                state.current_date(),
                state.change_detection.last_games(),
            );
            show
        };
        if should_show_indicator && let Some(page) = state.current_page_mut() {
            page.show_auto_refresh_indicator();
            state.request_render();
        }

        // Render immediately so spinner is visible during fetch
        if state.needs_render() {
            if let Some(page) = state.current_page() {
                let mut stdout = std::io::stdout();
                let _ = page.render_buffered(&mut stdout);
            }
            state.clear_render_flag();
        }

        // Spawn the network fetch as a background task so the spinner can animate
        let current_date_for_fetch = state.current_date().clone();
        let timeout_seconds = crate::constants::DEFAULT_HTTP_TIMEOUT_SECONDS + 5;
        let fetch_handle = tokio::spawn(fetch_games_with_timeout(
            current_date_for_fetch,
            timeout_seconds,
        ));

        // Animate the spinner while waiting for the fetch to complete
        let (games, had_error, raw_fetched_date, should_retry) =
            animate_during_fetch(state, fetch_handle).await;
        // Normalize the fetched date at the boundary to prevent whitespace
        // from poisoning current_date or causing spurious date-mismatch discards.
        let fetched_date = raw_fetched_date.trim().to_string();

        // Process the fetched data (change detection, page creation, etc.)
        let result = self
            .process_fetched_data(
                DataFetchParams {
                    current_date: state.current_date(),
                    last_games: state.change_detection.last_games(),
                    disable_links: config.disable_links,
                    compact_mode: config.compact_mode,
                    wide_mode: config.wide_mode,
                    preserved_page_for_restoration: state.preserved_page(),
                    is_date_change,
                },
                games,
                had_error,
                fetched_date,
                should_retry,
            )
            .await?;

        // Guard against silent date jumps during auto-refresh. The orchestrator's
        // fallback logic may return games from a completely different date (e.g.
        // jumping from today to a future date when no games exist for the requested
        // date). When a date is already set and the fetched date differs, discard
        // the results so the UI stays on the user's current date. When the fetched
        // date matches (or no date was previously set), update current_date to track
        // the actual date being displayed.
        if !result.had_error && !result.fetched_date.is_empty() {
            if should_discard_for_date_mismatch(state.current_date(), &result.fetched_date) {
                tracing::warn!(
                    "Auto-refresh returned date {} but current date is {:?}, discarding to prevent date jump",
                    result.fetched_date,
                    state.current_date()
                );
                // Hide the auto-refresh spinner that was shown before the fetch
                if let Some(page) = state.current_page_mut() {
                    page.hide_auto_refresh_indicator();
                    state.request_render();
                }
                return Ok(RefreshResult {
                    games: vec![],
                    had_error: false,
                    fetched_date: state.current_date().clone().unwrap_or_default(),
                    should_retry: false,
                    new_page: None,
                    needs_render: false,
                    skip_change_detection: true,
                });
            }
            state.set_current_date(Some(result.fetched_date.clone()));
            tracing::debug!("Updated current_date to: {:?}", state.current_date());
        }

        Ok(result)
    }

    /// Perform standings-specific refresh cycle.
    ///
    /// Shows a subtle spinner for auto-refreshes (when the current page is already
    /// a standings page) vs a full loading screen for initial loads. Detects unchanged
    /// data via hashing and skips UI rebuild when standings have not changed.
    async fn perform_standings_refresh(
        &self,
        state: &mut InteractiveState,
        config: &RefreshCycleConfig,
        live_mode: bool,
        preserved_page: Option<usize>,
        last_games: &[GameData],
    ) -> Result<RefreshResult, AppError> {
        tracing::info!("Fetching standings data (live_mode: {live_mode})");

        let last_standings_hash = state.change_detection.last_standings_hash();
        let is_auto_refresh = state.current_page().is_some_and(|p| p.is_standings_page())
            && last_standings_hash.is_some();

        if is_auto_refresh {
            // Show subtle spinner on existing page instead of full loading screen
            if let Some(page) = state.current_page_mut() {
                page.show_auto_refresh_indicator();
                state.request_render();
            }
            // Render spinner immediately
            if let Some(page) = state.current_page() {
                let mut stdout = std::io::stdout();
                if let Err(e) = page.render_buffered(&mut stdout) {
                    tracing::warn!("Failed to render auto-refresh spinner for standings: {e}");
                }
            }
            state.clear_render_flag();
        } else {
            // Show loading indicator immediately so the UI feels responsive
            let mut loading_page = TeletextPage::new(
                223,
                "JÄÄKIEKKO".to_string(),
                "SARJATAULUKKO".to_string(),
                config.disable_links,
                true,
                false,
                false,
                false,
            );
            loading_page.add_error_message("Haetaan sarjataulukkoa...");
            let mut stdout = std::io::stdout();
            if let Err(e) = loading_page.render_buffered(&mut stdout) {
                tracing::warn!("Failed to render standings loading page: {e}");
            }
        }

        let app_config = match crate::config::Config::load().await {
            Ok(config) => config,
            Err(e) => {
                if is_auto_refresh && let Some(page) = state.current_page_mut() {
                    page.hide_auto_refresh_indicator();
                    state.request_render();
                }
                return Err(e);
            }
        };
        let http_timeout = app_config.http_timeout_seconds;
        // Safety margin above the HTTP client timeout so reqwest reports the actual error
        let timeout_duration = Duration::from_secs(http_timeout + 5);
        let fetch_future = fetch_standings(&app_config, live_mode);

        let (mut standings, playoffs_lines, had_error) = match tokio::time::timeout(
            timeout_duration,
            fetch_future,
        )
        .await
        {
            Ok(Ok((standings, playoffs_lines))) => {
                tracing::info!("Standings fetched: {} teams", standings.len());
                (standings, playoffs_lines, false)
            }
            Ok(Err(e)) => {
                tracing::error!("Failed to fetch standings: {e}");
                (vec![], vec![], true)
            }
            Err(_) => {
                tracing::error!(
                    "Standings fetch timed out after {}s (safety timeout, HTTP client should have timed out at {http_timeout}s)",
                    http_timeout + 5
                );
                (vec![], vec![], true)
            }
        };

        // Cross-reference with game data to detect live teams that the standings
        // API alone can't identify (e.g., teams in a 0-0 game where goals haven't changed)
        if live_mode && !had_error {
            let live_team_names: std::collections::HashSet<&str> = last_games
                .iter()
                .filter(|g| g.score_type == ScoreType::Ongoing)
                .flat_map(|g| [g.home_team.as_str(), g.away_team.as_str()])
                .collect();

            if !live_team_names.is_empty() {
                for entry in &mut standings {
                    if !entry.live_game_active && live_team_names.contains(entry.team_name.as_str())
                    {
                        tracing::info!(
                            "Marking team '{}' as live from game data (0-0 game detection)",
                            entry.team_name
                        );
                        entry.live_game_active = true;
                        if entry.live_points_delta.is_none() {
                            entry.live_points_delta = Some(0);
                        }
                    }
                }
            }
        }

        let data_changed = if !had_error {
            let new_hash = calculate_standings_hash(&standings, &playoffs_lines, live_mode);
            state.change_detection.update_standings_hash(new_hash)
        } else {
            true // errors always count as "changed" to show the error page
        };

        // Hide auto-refresh spinner for auto-refresh case
        if is_auto_refresh && let Some(page) = state.current_page_mut() {
            page.hide_auto_refresh_indicator();
            if !data_changed {
                page.skip_next_screen_clear();
            }
            state.request_render();
        }

        if !data_changed {
            tracing::debug!("Standings data unchanged, skipping UI update");
            return Ok(RefreshResult {
                games: vec![],
                had_error: false,
                fetched_date: String::new(),
                should_retry: false,
                new_page: None,
                needs_render: true,
                skip_change_detection: true,
            });
        }

        let new_page = if !had_error {
            let mut page = navigation_manager::create_standings_page(
                &standings,
                &playoffs_lines,
                live_mode,
                config.disable_links,
                config.compact_mode,
                config.wide_mode,
            );
            if let Some(saved_page) = preserved_page {
                page.set_current_page(saved_page);
            }
            Some(page)
        } else {
            let mut error_page = TeletextPage::new(
                223,
                "JÄÄKIEKKO".to_string(),
                "SARJATAULUKKO".to_string(),
                config.disable_links,
                true,
                false,
                config.compact_mode,
                config.wide_mode,
            );
            error_page.add_error_message("Sarjataulukon lataus epäonnistui.");
            error_page.add_error_message("Paina 's' palataksesi otteluihin.");
            Some(error_page)
        };

        Ok(RefreshResult {
            games: vec![],
            had_error,
            fetched_date: String::new(),
            should_retry: had_error,
            new_page,
            needs_render: true,
            // Standings refreshes carry no game data, so mark as discarded to
            // prevent process_refresh_results from overwriting last_games with
            // an empty vector.
            skip_change_detection: true,
        })
    }

    /// Perform bracket-specific refresh cycle.
    ///
    /// Shows a subtle spinner for auto-refreshes (when the current page is already
    /// a bracket page) vs a full loading screen for initial loads. Detects unchanged
    /// data via hashing and skips UI rebuild when bracket data has not changed.
    async fn perform_bracket_refresh(
        &self,
        state: &mut InteractiveState,
        config: &RefreshCycleConfig,
        preserved_page: Option<usize>,
    ) -> Result<RefreshResult, AppError> {
        tracing::info!("Fetching bracket data");

        let last_bracket_hash = state.change_detection.last_bracket_hash();
        let is_auto_refresh = state.current_page().is_some() && last_bracket_hash.is_some();

        if is_auto_refresh {
            if let Some(page) = state.current_page_mut() {
                page.show_auto_refresh_indicator();
                state.request_render();
            }
            if let Some(page) = state.current_page() {
                let mut stdout = std::io::stdout();
                if let Err(e) = page.render_buffered(&mut stdout) {
                    tracing::warn!("Failed to render auto-refresh spinner for bracket: {e}");
                }
            }
            state.clear_render_flag();
        } else {
            let mut loading_page = TeletextPage::new(
                224,
                "JÄÄKIEKKO".to_string(),
                "PUDOTUSPELIT".to_string(),
                config.disable_links,
                true,
                false,
                false,
                false,
            );
            loading_page.add_error_message("Haetaan pudotuspelejä...");
            let mut stdout = std::io::stdout();
            if let Err(e) = loading_page.render_buffered(&mut stdout) {
                tracing::warn!("Failed to render bracket loading page: {e}");
            }
        }

        let app_config = match crate::config::Config::load().await {
            Ok(config) => config,
            Err(e) => {
                if is_auto_refresh && let Some(page) = state.current_page_mut() {
                    page.hide_auto_refresh_indicator();
                    state.request_render();
                }
                return Err(e);
            }
        };

        let http_timeout = app_config.http_timeout_seconds;
        let timeout_duration = std::time::Duration::from_secs(http_timeout + 5);

        let bracket_result = tokio::time::timeout(
            timeout_duration,
            crate::data_fetcher::api::bracket_api::fetch_playoff_bracket(&app_config),
        )
        .await;

        let (bracket, had_error) = match bracket_result {
            Ok(Ok(bracket)) => {
                tracing::info!("Bracket fetched: has_data={}", bracket.has_data);
                (Some(bracket), false)
            }
            Ok(Err(e)) => {
                tracing::error!("Failed to fetch bracket: {e}");
                (None, true)
            }
            Err(_) => {
                tracing::error!("Bracket fetch timed out");
                (None, true)
            }
        };

        let data_changed = if let Some(ref b) = bracket {
            let new_hash = super::change_detection::calculate_bracket_hash(b);
            state.change_detection.update_bracket_hash(new_hash)
        } else {
            true
        };

        if is_auto_refresh && let Some(page) = state.current_page_mut() {
            page.hide_auto_refresh_indicator();
            if !data_changed {
                page.skip_next_screen_clear();
            }
            state.request_render();
        }

        if !data_changed {
            tracing::debug!("Bracket data unchanged, skipping UI update");
            return Ok(RefreshResult {
                games: vec![],
                had_error: false,
                fetched_date: String::new(),
                should_retry: false,
                new_page: None,
                needs_render: true,
                skip_change_detection: true,
            });
        }

        let terminal_width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);

        let new_page = if let Some(bracket) = bracket {
            let mut page = navigation_manager::create_bracket_page(
                &bracket,
                config.disable_links,
                terminal_width,
            );
            if let Some(saved_page) = preserved_page {
                page.set_current_page(saved_page);
            }
            Some(page)
        } else {
            let mut error_page = TeletextPage::new(
                224,
                "JÄÄKIEKKO".to_string(),
                "PUDOTUSPELIT".to_string(),
                config.disable_links,
                true,
                false,
                false,
                false,
            );
            error_page.add_error_message("Pudotuspelien lataus epäonnistui.");
            error_page.add_error_message("Paina 'p' palataksesi.");
            Some(error_page)
        };

        Ok(RefreshResult {
            games: vec![],
            had_error,
            fetched_date: String::new(),
            should_retry: had_error,
            new_page,
            needs_render: true,
            skip_change_detection: true,
        })
    }

    /// Process refresh results and update state
    pub fn process_refresh_results(
        &self,
        state: &mut InteractiveState,
        result: &RefreshResult,
    ) -> bool {
        let mut needs_state_render = false;

        // Skip change detection for results that carry no meaningful game data
        // (date-mismatch discards, standings refreshes, or transient-empty preserves)
        // to avoid clearing last_games state
        if result.skip_change_detection {
            tracing::debug!(
                "Skipping change detection (standings, date-mismatch, or transient-empty preserve)"
            );
            // Still hide the auto-refresh spinner so it doesn't stay stuck
            if let Some(page) = state.current_page_mut()
                && page.is_auto_refresh_indicator_active()
            {
                page.hide_auto_refresh_indicator();
                page.skip_next_screen_clear();
                state.request_render();
                needs_state_render = true;
            }
            return needs_state_render;
        }

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
            if let Some(_new_page) = result.new_page.as_ref() {
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
            if let Some(page) = state.current_page_mut()
                && page.is_error_warning_active()
            {
                page.hide_error_warning();
                state.request_render();
                needs_state_render = true;
            }
            state
                .change_detection
                .update_state(result.games.clone(), games_hash);
        } else {
            tracing::debug!(
                "Preserving last_games due to fetch error; will retry without clearing state"
            );
        }

        // Hide auto-refresh spinner after fetch completes
        if let Some(page) = state.current_page_mut()
            && page.is_auto_refresh_indicator_active()
        {
            page.hide_auto_refresh_indicator();
            if !data_changed {
                page.skip_next_screen_clear();
            }
            state.request_render();
            needs_state_render = true;
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
            tracing::trace!("Detailed cache debug info: {debug_info}");
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
        let all_scheduled =
            !games.is_empty() && games.iter().all(navigation_manager::is_future_game);

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
        assert_eq!(
            coordinator.cache_config.cache_monitor_interval,
            Duration::from_secs(300)
        );
        assert_eq!(coordinator.consecutive_transient_empty, 0);
    }

    #[test]
    fn test_refresh_coordinator_default() {
        let coordinator = RefreshCoordinator::default();
        // Should be equivalent to RefreshCoordinator::new()
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
            is_date_change: false,
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

    #[test]
    fn test_should_discard_for_date_mismatch_different_dates() {
        let current = Some("2025-03-13".to_string());
        assert!(should_discard_for_date_mismatch(&current, "2025-03-20"));
    }

    #[test]
    fn test_should_discard_for_date_mismatch_same_date() {
        let current = Some("2025-03-13".to_string());
        assert!(!should_discard_for_date_mismatch(&current, "2025-03-13"));
    }

    #[test]
    fn test_should_discard_for_date_mismatch_no_current_date() {
        assert!(!should_discard_for_date_mismatch(&None, "2025-03-20"));
    }

    #[test]
    fn test_should_discard_for_date_mismatch_empty_fetched() {
        let current = Some("2025-03-13".to_string());
        // Empty fetched date is guarded by the caller with `!result.fetched_date.is_empty()`,
        // but the function itself treats it as a mismatch (defense-in-depth)
        assert!(should_discard_for_date_mismatch(&current, ""));
    }

    #[test]
    fn test_should_discard_for_date_mismatch_empty_current_date() {
        // Some("") should be treated as unset, not as a date to compare against
        let current = Some("".to_string());
        assert!(!should_discard_for_date_mismatch(&current, "2025-03-20"));
    }

    #[test]
    fn test_should_discard_for_date_mismatch_whitespace_current_date() {
        // Some("  ") should be treated as unset, like Some("")
        let current = Some("  ".to_string());
        assert!(!should_discard_for_date_mismatch(&current, "2025-03-20"));
    }

    #[test]
    fn test_standings_live_mode_uses_live_refresh_interval() {
        let mut state = InteractiveState::new(Some("2025-03-13".to_string()));
        // Clear the initial refresh flag so the interval logic is exercised
        state.clear_refresh_flag();
        state.toggle_view(); // Games -> Standings { live_mode: false }
        state.toggle_live_mode(); // Standings { live_mode: true }

        let config = RefreshCycleConfig {
            min_refresh_interval: None,
            disable_links: false,
            compact_mode: false,
            wide_mode: false,
        };

        let coordinator = RefreshCoordinator::new();

        // With last_auto_refresh just now, should NOT trigger (interval not elapsed)
        state.timers.last_auto_refresh = std::time::Instant::now();
        assert!(!coordinator.should_trigger_refresh(&state, &config));

        // With last_auto_refresh 16s ago, SHOULD trigger (15s interval elapsed)
        // Use a non-historical date so is_historical_date doesn't block auto-refresh
        state.set_current_date(Some("2099-03-13".to_string()));
        state.timers.last_auto_refresh = std::time::Instant::now()
            .checked_sub(Duration::from_secs(16))
            .unwrap();
        assert!(coordinator.should_trigger_refresh(&state, &config));
    }

    #[test]
    fn test_standings_live_mode_not_clamped_by_game_count() {
        // When last_games has 6+ entries the game-count-based min interval would be 30s,
        // which must NOT clamp the 15s live-standings interval.
        let mut state = InteractiveState::new(Some("2099-03-13".to_string()));
        state.clear_refresh_flag();

        // Seed 6 games so calculate_min_refresh_interval would return 30s for games view
        let teams = [
            ("TPS", "HIFK"),
            ("Ilves", "Lukko"),
            ("Tappara", "KalPa"),
            ("Pelicans", "JYP"),
            ("KooKoo", "SaiPa"),
            ("Ässät", "Sport"),
        ];
        let games: Vec<_> = teams
            .iter()
            .map(|(h, a)| crate::testing_utils::TestDataBuilder::create_basic_game(h, a))
            .collect();
        let hash = calculate_games_hash(&games);
        state.change_detection.update_state(games, hash);
        assert_eq!(state.change_detection.last_games().len(), 6);

        state.toggle_view(); // Games -> Standings { live_mode: false }
        state.toggle_live_mode(); // Standings { live_mode: true }
        state.clear_refresh_flag();

        let config = RefreshCycleConfig {
            min_refresh_interval: None,
            disable_links: false,
            compact_mode: false,
            wide_mode: false,
        };
        let coordinator = RefreshCoordinator::new();

        // 16s ago should trigger (15s live interval, min_interval must not clamp to 30s)
        state.timers.last_auto_refresh = std::time::Instant::now()
            .checked_sub(Duration::from_secs(16))
            .unwrap();
        assert!(coordinator.should_trigger_refresh(&state, &config));
    }

    #[test]
    fn test_standings_non_live_mode_uses_default_refresh_interval() {
        // Use a non-historical date so is_historical_date doesn't block auto-refresh
        let mut state = InteractiveState::new(Some("2099-03-13".to_string()));
        state.clear_refresh_flag();
        state.toggle_view(); // Games -> Standings { live_mode: false }

        let config = RefreshCycleConfig {
            min_refresh_interval: None,
            disable_links: false,
            compact_mode: false,
            wide_mode: false,
        };

        let coordinator = RefreshCoordinator::new();

        // With last_auto_refresh 16s ago, should NOT trigger for non-live standings
        // (falls through to calculate_auto_refresh_interval which returns 60s with no games)
        state.timers.last_auto_refresh = std::time::Instant::now()
            .checked_sub(Duration::from_secs(16))
            .unwrap();
        assert!(!coordinator.should_trigger_refresh(&state, &config));
    }

    #[test]
    fn test_process_refresh_results_skips_discarded() {
        let coordinator = RefreshCoordinator::new();
        let mut state = InteractiveState::new(Some("2025-03-13".to_string()));

        // Seed state with some games
        let mut game = crate::testing_utils::TestDataBuilder::create_basic_game("TPS", "HIFK");
        game.result = "3-2".to_string();
        let games_hash = calculate_games_hash(std::slice::from_ref(&game));
        state
            .change_detection
            .update_state(vec![game.clone()], games_hash);

        // Process a date-mismatch-discarded result
        let discarded_result = RefreshResult {
            games: vec![],
            had_error: false,
            fetched_date: "2025-03-13".to_string(),
            should_retry: false,
            new_page: None,
            needs_render: false,
            skip_change_detection: true,
        };
        coordinator.process_refresh_results(&mut state, &discarded_result);

        // last_games should be preserved, not cleared
        assert_eq!(state.change_detection.last_games().len(), 1);
        assert_eq!(state.change_detection.last_games()[0].home_team, "TPS");
    }

    #[test]
    fn test_process_refresh_results_updates_state_on_success() {
        let coordinator = RefreshCoordinator::new();
        let mut state = InteractiveState::new(Some("2025-03-13".to_string()));

        let mut game = crate::testing_utils::TestDataBuilder::create_basic_game("TPS", "HIFK");
        game.result = "3-2".to_string();

        let result = RefreshResult {
            games: vec![game.clone()],
            had_error: false,
            fetched_date: "2025-03-13".to_string(),
            should_retry: false,
            new_page: None,
            needs_render: false,
            skip_change_detection: false,
        };
        coordinator.process_refresh_results(&mut state, &result);

        assert_eq!(state.change_detection.last_games().len(), 1);
        assert_eq!(state.change_detection.last_games()[0].home_team, "TPS");
    }

    #[test]
    fn test_should_discard_for_date_mismatch_whitespace_in_fetched_date() {
        // Trailing/leading whitespace in the fetched date should be treated
        // as equal to the trimmed variant, not cause a spurious discard.
        let current = Some("2025-03-13".to_string());
        assert!(!should_discard_for_date_mismatch(&current, "2025-03-13 "));
        assert!(!should_discard_for_date_mismatch(&current, " 2025-03-13"));
        assert!(!should_discard_for_date_mismatch(&current, " 2025-03-13 "));
    }

    #[test]
    fn test_should_discard_for_date_mismatch_whitespace_in_current_date() {
        // Whitespace in current_date should also be normalized for comparison
        let current = Some("2025-03-13 ".to_string());
        assert!(!should_discard_for_date_mismatch(&current, "2025-03-13"));
    }

    #[test]
    fn test_standings_refresh_result_preserves_last_games() {
        let coordinator = RefreshCoordinator::new();
        let mut state = InteractiveState::new(Some("2025-03-13".to_string()));

        // Seed state with some games
        let game = crate::testing_utils::TestDataBuilder::create_basic_game("TPS", "HIFK");
        let games_hash = calculate_games_hash(std::slice::from_ref(&game));
        state
            .change_detection
            .update_state(vec![game.clone()], games_hash);

        // Simulate a standings refresh result with skip_change_detection set to
        // prevent process_refresh_results from overwriting last_games with an empty vec
        let standings_result = RefreshResult {
            games: vec![],
            had_error: false,
            fetched_date: String::new(),
            should_retry: false,
            new_page: None,
            needs_render: true,
            skip_change_detection: true,
        };
        coordinator.process_refresh_results(&mut state, &standings_result);

        // last_games must be preserved, not overwritten with empty vec
        assert_eq!(state.change_detection.last_games().len(), 1);
        assert_eq!(state.change_detection.last_games()[0].home_team, "TPS");
    }

    #[test]
    fn test_reset_transient_empty_counter() {
        let mut coordinator = RefreshCoordinator::new();
        assert_eq!(coordinator.consecutive_transient_empty, 0);

        coordinator.consecutive_transient_empty = 2;
        coordinator.reset_transient_empty_counter();
        assert_eq!(coordinator.consecutive_transient_empty, 0);
    }

    #[test]
    fn test_reset_transient_empty_counter_noop_when_zero() {
        let mut coordinator = RefreshCoordinator::new();
        coordinator.reset_transient_empty_counter();
        assert_eq!(coordinator.consecutive_transient_empty, 0);
    }

    #[tokio::test]
    async fn test_transient_empty_preserves_existing_games() {
        let mut coordinator = RefreshCoordinator::new();
        let current_date = Some("2025-03-13".to_string());
        let last_games = vec![crate::testing_utils::TestDataBuilder::create_basic_game(
            "TPS", "HIFK",
        )];
        let params = DataFetchParams {
            current_date: &current_date,
            last_games: &last_games,
            disable_links: true,
            compact_mode: false,
            wide_mode: false,
            preserved_page_for_restoration: None,
            is_date_change: false,
        };

        // First empty response — should preserve existing games
        let result = coordinator
            .process_fetched_data(params, vec![], false, "2025-03-13".to_string(), false)
            .await
            .unwrap();

        assert_eq!(result.games.len(), 1);
        assert_eq!(result.games[0].home_team, "TPS");
        assert!(result.skip_change_detection);
        assert!(
            !result.should_retry,
            "transient-empty preservation is a deliberate decision, not a failure"
        );
        assert!(!result.had_error);
        assert_eq!(coordinator.consecutive_transient_empty, 1);
    }

    #[tokio::test]
    async fn test_transient_empty_propagates_had_error() {
        let mut coordinator = RefreshCoordinator::new();
        let current_date = Some("2025-03-13".to_string());
        let last_games = vec![crate::testing_utils::TestDataBuilder::create_basic_game(
            "TPS", "HIFK",
        )];
        let params = DataFetchParams {
            current_date: &current_date,
            last_games: &last_games,
            disable_links: true,
            compact_mode: false,
            wide_mode: false,
            preserved_page_for_restoration: None,
            is_date_change: false,
        };

        // Empty response with error — should NOT enter transient-empty branch;
        // errors follow the normal processing path so stale games are not silently preserved.
        let result = coordinator
            .process_fetched_data(params, vec![], true, "2025-03-13".to_string(), false)
            .await
            .unwrap();

        assert_eq!(result.games.len(), 0);
        assert!(result.had_error);
        assert!(!result.skip_change_detection);
        // Transient empty counter should NOT have been incremented for error responses
        assert_eq!(coordinator.consecutive_transient_empty, 0);
    }

    #[tokio::test]
    async fn test_transient_empty_accepts_after_threshold() {
        let mut coordinator = RefreshCoordinator::new();
        let current_date = Some("2025-03-13".to_string());
        let last_games = vec![crate::testing_utils::TestDataBuilder::create_basic_game(
            "TPS", "HIFK",
        )];

        // Exhaust transient empty allowance (MAX_TRANSIENT_EMPTY = 3)
        for i in 0..MAX_TRANSIENT_EMPTY {
            let params = DataFetchParams {
                current_date: &current_date,
                last_games: &last_games,
                disable_links: true,
                compact_mode: false,
                wide_mode: false,
                preserved_page_for_restoration: None,
                is_date_change: false,
            };
            let result = coordinator
                .process_fetched_data(params, vec![], false, "2025-03-13".to_string(), false)
                .await
                .unwrap();
            assert_eq!(result.games.len(), 1, "cycle {i} should preserve games");
            assert!(result.skip_change_detection);
        }

        // 4th empty response — should accept as legitimate (empty games)
        let params = DataFetchParams {
            current_date: &current_date,
            last_games: &last_games,
            disable_links: true,
            compact_mode: false,
            wide_mode: false,
            preserved_page_for_restoration: None,
            is_date_change: false,
        };
        let result = coordinator
            .process_fetched_data(params, vec![], false, "2025-03-13".to_string(), false)
            .await
            .unwrap();
        assert!(
            result.games.is_empty(),
            "should accept empty after threshold"
        );
        assert!(!result.skip_change_detection);
        // Counter should be reset after accepting
        assert_eq!(coordinator.consecutive_transient_empty, 0);
    }

    #[tokio::test]
    async fn test_transient_empty_resets_on_nonempty_response() {
        let mut coordinator = RefreshCoordinator::new();
        let current_date = Some("2025-03-13".to_string());
        let existing_games = vec![crate::testing_utils::TestDataBuilder::create_basic_game(
            "TPS", "HIFK",
        )];

        // Simulate 2 transient empty responses
        for _ in 0..2 {
            let params = DataFetchParams {
                current_date: &current_date,
                last_games: &existing_games,
                disable_links: true,
                compact_mode: false,
                wide_mode: false,
                preserved_page_for_restoration: None,
                is_date_change: false,
            };
            coordinator
                .process_fetched_data(params, vec![], false, "2025-03-13".to_string(), false)
                .await
                .unwrap();
        }
        assert_eq!(coordinator.consecutive_transient_empty, 2);

        // Non-empty response should reset counter
        let new_games = vec![crate::testing_utils::TestDataBuilder::create_basic_game(
            "Kärpät", "Lukko",
        )];
        let params = DataFetchParams {
            current_date: &current_date,
            last_games: &existing_games,
            disable_links: true,
            compact_mode: false,
            wide_mode: false,
            preserved_page_for_restoration: None,
            is_date_change: false,
        };
        coordinator
            .process_fetched_data(params, new_games, false, "2025-03-13".to_string(), false)
            .await
            .unwrap();
        assert_eq!(coordinator.consecutive_transient_empty, 0);
    }

    #[tokio::test]
    async fn test_transient_empty_skipped_on_date_change() {
        let mut coordinator = RefreshCoordinator::new();
        let current_date = Some("2025-03-14".to_string());
        let last_games = vec![crate::testing_utils::TestDataBuilder::create_basic_game(
            "TPS", "HIFK",
        )];

        // Empty response on a date change — should NOT preserve old games
        let params = DataFetchParams {
            current_date: &current_date,
            last_games: &last_games,
            disable_links: true,
            compact_mode: false,
            wide_mode: false,
            preserved_page_for_restoration: None,
            is_date_change: true,
        };
        let result = coordinator
            .process_fetched_data(params, vec![], false, "2025-03-14".to_string(), false)
            .await
            .unwrap();

        assert!(
            result.games.is_empty(),
            "date change should not preserve old games"
        );
        assert!(!result.skip_change_detection);
        assert_eq!(coordinator.consecutive_transient_empty, 0);
    }
}
