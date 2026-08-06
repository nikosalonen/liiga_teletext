//! Keyboard input handling and date navigation for the interactive UI.
//!
//! This module handles:
//! - Keyboard event processing (quit, refresh, page navigation)
//! - Date navigation with Shift + Arrow keys
//! - Finding previous/next dates with games
//! - Season boundary checking

use crate::data_fetcher::{fetch_liiga_data, is_historical_date};
use crate::error::AppError;
use crate::teletext_ui::TeletextPage;
use chrono::{Datelike, Local, NaiveDate, Utc};
use crossterm::event::{self, KeyCode, KeyEventKind, KeyModifiers};
use std::io::stdout;
use std::time::{Duration, Instant};

use super::state_manager::ViewMode;

/// Parameters for keyboard event handling
pub(super) struct KeyEventParams<'a> {
    pub key_event: &'a event::KeyEvent,
    pub current_page: &'a mut Option<TeletextPage>,
    pub needs_render: &'a mut bool,
    pub needs_refresh: &'a mut bool,
    pub current_date: &'a mut Option<String>,
    pub last_manual_refresh: &'a mut Instant,
    pub last_page_change: &'a mut Instant,
    pub last_date_navigation: &'a mut Instant,
    pub current_view: &'a mut ViewMode,
    pub preserved_games_page: &'a mut Option<usize>,
    pub preserved_live_mode: &'a mut bool,
    pub has_bracket_data: bool,
    pub page_input: &'a mut String,
    pub last_page_input: &'a mut Instant,
}

/// Checks if the given key event matches the date navigation shortcut.
/// Uses Shift + Left/Right for all platforms (works reliably in all terminals)
fn is_date_navigation_key(key_event: &event::KeyEvent, is_left: bool) -> bool {
    let expected_code = if is_left {
        KeyCode::Left
    } else {
        KeyCode::Right
    };

    if key_event.code != expected_code {
        return false;
    }

    // Use Shift key for date navigation (works reliably in all terminals)
    let has_shift_modifier = key_event.modifiers.contains(KeyModifiers::SHIFT);

    if has_shift_modifier {
        tracing::debug!(
            "Date navigation key detected: Shift + {}",
            if is_left { "Left" } else { "Right" }
        );
        return true;
    }

    false
}

/// Gets the target date for navigation, using current_date if available,
/// otherwise determining the appropriate date based on current time.
fn get_target_date_for_navigation(current_date: &Option<String>) -> String {
    current_date.as_ref().cloned().unwrap_or_else(|| {
        // If no current date, use today/yesterday based on time
        if crate::data_fetcher::processors::should_show_todays_games() {
            Utc::now()
                .with_timezone(&Local)
                .format("%Y-%m-%d")
                .to_string()
        } else {
            let yesterday = Utc::now()
                .with_timezone(&Local)
                .date_naive()
                .pred_opt()
                .expect("Date underflow cannot happen");
            yesterday.format("%Y-%m-%d").to_string()
        }
    })
}

/// Checks if a date would require historical/schedule endpoint (from previous season).
/// This prevents navigation to very old games via arrow keys, but allows reasonable historical access.
fn would_be_previous_season(date: &str) -> bool {
    let now = Utc::now().with_timezone(&Local);

    let date_parts: Vec<&str> = date.split('-').collect();
    if date_parts.len() < 2 {
        return false;
    }

    let date_year = date_parts[0].parse::<i32>().unwrap_or(now.year());
    let date_month = date_parts[1].parse::<u32>().unwrap_or(now.month());

    let current_year = now.year();
    let current_month = now.month();

    // Allow navigation within the past 2 years for reasonable historical access
    // This covers the current season and the previous season
    if date_year < current_year - 1 {
        return true;
    }

    // For dates within the past 2 years, use more nuanced season logic
    if date_year == current_year {
        // Same year - check if we're trying to go to off-season of previous season
        // Hockey season: September-February (regular), March-May (playoffs/playout)
        // Off-season: June-August

        // If we're in new regular season (September-December) and date is from off-season
        // (June-August), it's from the previous season
        if (9..=12).contains(&current_month) && (6..=8).contains(&date_month) {
            return true;
        }
    } else if date_year == current_year - 1 {
        // Previous year - allow access to recent hockey season games
        // Only block if we're trying to access very old off-season games

        // If we're currently in the new season (September+) and trying to access
        // off-season games from the previous year (June-August), block it
        if current_month >= 9 && (6..=8).contains(&date_month) {
            return true;
        }
    }

    false
}

/// Upper bound on how long a Shift+arrow date search may run. The search
/// currently blocks the event loop, so without a cap a long empty stretch
/// (off-season) could freeze the UI for minutes of sequential fetches.
const MAX_DATE_SEARCH_DURATION: Duration = Duration::from_secs(20);

/// Probes candidate dates in order until `probe` reports a hit, giving up
/// once the time budget is exhausted.
async fn search_dates_with_budget<F, Fut>(
    candidates: impl Iterator<Item = String>,
    budget: Duration,
    mut probe: F,
) -> Option<String>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
{
    let started = tokio::time::Instant::now();
    for date in candidates {
        if started.elapsed() >= budget {
            tracing::warn!("Date search time budget exhausted, giving up");
            return None;
        }
        if let Some(hit) = probe(date).await {
            return Some(hit);
        }
    }
    None
}

/// Finds the previous date with games by checking dates going backwards.
/// Returns None if no games are found within the current season or a reasonable time range.
/// Prevents navigation to previous season games for better UX.
async fn find_previous_date_with_games(current_date: &str) -> Option<String> {
    let current_parsed = match NaiveDate::parse_from_str(current_date, "%Y-%m-%d") {
        Ok(date) => date,
        Err(_) => return None,
    };

    tracing::info!(
        "Starting search for previous date with games from: {}",
        current_date
    );

    // Search up to 30 days in the past, stopping at the previous season
    // boundary and giving up once the time budget is spent.
    let candidates = (1..=30)
        .filter_map(|days_back| {
            current_parsed
                .checked_sub_days(chrono::Days::new(days_back))
                .map(|d| d.format("%Y-%m-%d").to_string())
        })
        .take_while(|date_string| {
            let previous_season = would_be_previous_season(date_string);
            if previous_season {
                tracing::info!(
                    "Reached previous season boundary at {}, stopping navigation (use -d flag for historical games)",
                    date_string
                );
            }
            !previous_season
        });

    let result = search_dates_with_budget(candidates, MAX_DATE_SEARCH_DURATION, |date_string| {
        // Only a result for the probed date itself counts as a hit here:
        // the fetcher's fallback can jump ahead, but a later date is in the
        // wrong direction for a backward search.
        probe_date(date_string, |requested, fetched| {
            (fetched == requested).then(|| requested.to_string())
        })
    })
    .await;

    if result.is_none() {
        tracing::info!(
            "No previous date with games found within current season from {}",
            current_date
        );
    }
    result
}

/// Fetches games for one candidate date and applies `accept` to decide
/// whether the (possibly fallback-shifted) result concludes the search.
async fn probe_date<A>(date_string: String, accept: A) -> Option<String>
where
    A: Fn(&str, &str) -> Option<String>,
{
    let fetch_future = fetch_liiga_data(Some(date_string.clone()));
    let timeout_duration = Duration::from_secs(crate::constants::DEFAULT_HTTP_TIMEOUT_SECONDS + 5);

    match tokio::time::timeout(timeout_duration, fetch_future).await {
        Ok(Ok((games, fetched_date))) if !games.is_empty() => {
            if let Some(hit) = accept(&date_string, &fetched_date) {
                tracing::info!("Found date with games: {hit} (probed {date_string})");
                return Some(hit);
            }
            tracing::debug!(
                "Skipping date {date_string} because fetcher returned unusable date: {fetched_date}"
            );
        }
        Ok(Ok(_)) => {
            // No games found, continue searching
        }
        Ok(Err(e)) => {
            tracing::warn!("Error fetching data for {date_string}: {e} (continuing search)");
        }
        Err(_) => {
            tracing::warn!("Timeout fetching data for {date_string} (continuing search)");
        }
    }

    // Small delay to prevent API spam
    tokio::time::sleep(Duration::from_millis(50)).await;
    None
}

/// Decides whether a forward date-search probe found the answer.
///
/// `fetched` is the date the fetcher actually returned games for. When the
/// requested date was empty, the fetcher's own fallback may have jumped ahead
/// and returned games for a later date — that later date IS the next date
/// with games, so the search can stop there instead of re-probing day by day.
/// A `fetched` before `requested` is rejected so a backward jump can't
/// masquerade as forward progress. ISO dates compare correctly as strings.
fn forward_search_hit(requested: &str, fetched: &str) -> Option<String> {
    (fetched >= requested).then(|| fetched.to_string())
}

/// Finds the next date with games by checking dates going forwards.
/// Returns None if no games are found within a reasonable time range.
async fn find_next_date_with_games(current_date: &str) -> Option<String> {
    let current_parsed = match NaiveDate::parse_from_str(current_date, "%Y-%m-%d") {
        Ok(date) => date,
        Err(_) => return None,
    };

    tracing::info!(
        "Starting search for next date with games from: {}",
        current_date
    );

    // Search up to 60 days in the future (handles off-season periods),
    // giving up once the time budget is spent.
    let candidates = (1..=60).filter_map(|days_ahead| {
        current_parsed
            .checked_add_days(chrono::Days::new(days_ahead))
            .map(|d| d.format("%Y-%m-%d").to_string())
    });

    let result = search_dates_with_budget(candidates, MAX_DATE_SEARCH_DURATION, |date_string| {
        probe_date(date_string, forward_search_hit)
    })
    .await;

    if result.is_none() {
        tracing::info!(
            "No next date with games found within search range from {}",
            current_date
        );
    }
    result
}

/// Page numbers for the available views (teletext-style page entry)
const PAGE_GAMES: &str = "221";
const PAGE_STANDINGS: &str = "222";
const PAGE_BRACKET: &str = "223";

/// Handles a digit key press for teletext-style page number entry.
/// Digits accumulate in the header (e.g. "22-"); after three digits the
/// matching view is shown, or a block-graphics "SIVUA EI LÖYDY" page
/// for numbers that aren't in use.
fn handle_page_number_input(params: &mut KeyEventParams<'_>, digit: char) {
    params.page_input.push(digit);
    *params.last_page_input = Instant::now();

    if params.page_input.len() < 3 {
        // Show the accumulating digits in the header page number slot
        if let Some(page) = params.current_page.as_mut() {
            page.set_page_input(Some(params.page_input.clone()));
        }
        *params.needs_render = true;
        return;
    }

    let entered = std::mem::take(params.page_input);
    if let Some(page) = params.current_page.as_mut() {
        page.set_page_input(None);
    }
    *params.needs_render = true;

    // Preserve the games page position when leaving the games view,
    // mirroring the behavior of the 's' and 'p' shortcuts.
    let preserve_games_page = |params: &mut KeyEventParams<'_>| {
        if matches!(*params.current_view, ViewMode::Games)
            && let Some(page) = params.current_page.as_ref()
        {
            *params.preserved_games_page = Some(page.get_current_page());
        }
    };

    match entered.as_str() {
        PAGE_GAMES => {
            if !matches!(*params.current_view, ViewMode::Games) {
                tracing::info!("Page entry: switching to games view");
                if let ViewMode::Standings { live_mode } = *params.current_view {
                    *params.preserved_live_mode = live_mode;
                }
                *params.current_view = ViewMode::Games;
                *params.needs_refresh = true;
            }
        }
        PAGE_STANDINGS => {
            if !matches!(*params.current_view, ViewMode::Standings { .. }) {
                tracing::info!("Page entry: switching to standings view");
                preserve_games_page(params);
                *params.current_view = ViewMode::Standings {
                    live_mode: *params.preserved_live_mode,
                };
                *params.needs_refresh = true;
            }
        }
        PAGE_BRACKET if params.has_bracket_data => {
            if !matches!(*params.current_view, ViewMode::Bracket) {
                tracing::info!("Page entry: switching to bracket view");
                preserve_games_page(params);
                *params.current_view = ViewMode::Bracket;
                *params.needs_refresh = true;
            }
        }
        other => {
            let number = other.parse::<u16>().unwrap_or(0);
            tracing::info!("Page entry: page {number} not found");
            *params.current_page = Some(super::navigation_manager::create_page_not_found_page(
                number,
            ));
        }
    }
}

/// Handle keyboard events
pub(super) async fn handle_key_event(mut params: KeyEventParams<'_>) -> Result<bool, AppError> {
    // Only handle key press events, ignore Release/Repeat to prevent double-toggling on Windows
    if params.key_event.kind != KeyEventKind::Press {
        return Ok(false);
    }

    tracing::debug!(
        "Key event: {:?}, modifiers: {:?}",
        params.key_event.code,
        params.key_event.modifiers
    );

    // A non-digit key abandons any partial teletext page entry so stale
    // digits don't prepend to a later entry.
    let is_digit_key = matches!(params.key_event.code, KeyCode::Char(c) if c.is_ascii_digit());
    if !is_digit_key && !params.page_input.is_empty() {
        params.page_input.clear();
        if let Some(page) = params.current_page.as_mut() {
            page.set_page_input(None);
        }
        *params.needs_render = true;
    }

    // Disable date navigation in standings and bracket views
    let is_non_game_view = matches!(
        params.current_view,
        ViewMode::Standings { .. } | ViewMode::Bracket
    );

    // Check for date navigation first (Shift + Arrow keys)
    if !is_non_game_view && is_date_navigation_key(params.key_event, true) {
        // Shift + Left: Previous date with games
        if params.last_date_navigation.elapsed() >= Duration::from_millis(250) {
            tracing::info!("Previous date navigation requested");
            tracing::debug!("Current date state: {:?}", params.current_date);
            let target_date = get_target_date_for_navigation(params.current_date);

            // Show loading indicator
            if let Some(page) = params.current_page.as_mut() {
                page.show_loading("Etsitään edellisiä otteluita...".to_string());
                // Force immediate render to show loading indicator
                let mut stdout = stdout();
                let _ = page.render_buffered(&mut stdout);
                *params.needs_render = true;
            }

            tracing::info!(
                "Searching for previous date with games from: {}",
                target_date
            );

            let result = find_previous_date_with_games(&target_date).await;

            if let Some(prev_date) = result {
                *params.current_date = Some(prev_date.clone());

                // Small delay to ensure all cache writes (especially roster data from concurrent fetches)
                // are fully committed to the cache before triggering the refresh that renders the page.
                // This prevents "Pelaaja <number>" from appearing for games whose rosters haven't
                // been fully cached yet from the concurrent fetch operations (3 games at a time).
                tokio::time::sleep(Duration::from_millis(100)).await;

                *params.needs_refresh = true;
                tracing::info!("Navigated to previous date: {prev_date}");
            } else {
                tracing::warn!("No previous date with games found");
            }

            // Hide loading indicator
            if let Some(page) = params.current_page.as_mut() {
                page.hide_loading();
            }
            *params.last_date_navigation = Instant::now();
        }
    } else if !is_non_game_view && is_date_navigation_key(params.key_event, false) {
        // Shift + Right: Next date with games
        if params.last_date_navigation.elapsed() >= Duration::from_millis(250) {
            tracing::info!("Next date navigation requested");
            tracing::debug!("Current date state: {:?}", params.current_date);
            let target_date = get_target_date_for_navigation(params.current_date);

            // Show loading indicator
            if let Some(page) = params.current_page.as_mut() {
                page.show_loading("Etsitään seuraavia otteluita...".to_string());
                // Force immediate render to show loading indicator
                let mut stdout = stdout();
                let _ = page.render_buffered(&mut stdout);
                *params.needs_render = true;
            }

            tracing::info!("Searching for next date with games from: {target_date}");

            let result = find_next_date_with_games(&target_date).await;

            if let Some(next_date) = result {
                *params.current_date = Some(next_date.clone());

                // Small delay to ensure all cache writes (especially roster data from concurrent fetches)
                // are fully committed to the cache before triggering the refresh that renders the page.
                // This prevents "Pelaaja <number>" from appearing for games whose rosters haven't
                // been fully cached yet from the concurrent fetch operations (3 games at a time).
                tokio::time::sleep(Duration::from_millis(100)).await;

                *params.needs_refresh = true;
                tracing::info!("Navigated to next date: {next_date}");
            } else {
                tracing::warn!("No next date with games found");
            }

            // Hide loading indicator
            if let Some(page) = params.current_page.as_mut() {
                page.hide_loading();
            }
            *params.last_date_navigation = Instant::now();
        }
    } else {
        // Handle regular key events (without modifiers)
        match params.key_event.code {
            KeyCode::Char('q') => {
                tracing::info!("Quit requested");
                return Ok(true); // Signal to quit
            }
            // Teletext-style page number entry: type three digits to jump
            // to a page (221 = games, 222 = standings, 223 = playoffs)
            KeyCode::Char(c) if c.is_ascii_digit() => {
                handle_page_number_input(&mut params, c);
            }
            KeyCode::Char('r') => {
                // Check if auto-refresh is disabled - ignore manual refresh too
                if let Some(page) = params.current_page.as_ref()
                    && page.is_auto_refresh_disabled()
                {
                    tracing::info!("Manual refresh ignored - auto-refresh is disabled");
                    return Ok(false); // Skip refresh when auto-refresh is disabled
                }

                // Check if current date is historical - don't refresh historical data
                if let Some(date) = params.current_date
                    && is_historical_date(date)
                {
                    tracing::info!("Manual refresh skipped for historical date: {date}");
                    return Ok(false); // Skip refresh for historical dates
                }

                if params.last_manual_refresh.elapsed() >= Duration::from_secs(15) {
                    tracing::info!("Manual refresh requested");
                    *params.needs_refresh = true;
                    *params.last_manual_refresh = Instant::now();
                }
            }
            KeyCode::Left if params.last_page_change.elapsed() >= Duration::from_millis(200) => {
                if let Some(page) = params.current_page.as_mut() {
                    page.previous_page();
                    *params.needs_render = true;
                }
                *params.last_page_change = Instant::now();
            }
            KeyCode::Right if params.last_page_change.elapsed() >= Duration::from_millis(200) => {
                if let Some(page) = params.current_page.as_mut() {
                    page.next_page();
                    *params.needs_render = true;
                }
                *params.last_page_change = Instant::now();
            }
            KeyCode::Char('p')
                if params.has_bracket_data || matches!(*params.current_view, ViewMode::Bracket) =>
            {
                tracing::info!("Bracket view toggle requested");
                match *params.current_view {
                    ViewMode::Bracket => {
                        *params.current_view = ViewMode::Games;
                    }
                    _ => {
                        // Preserve current games page so the fast-restore
                        // path rebuilds the page from cached data on return
                        // (avoids change-detection skip when data is unchanged).
                        if let Some(page) = params.current_page.as_ref() {
                            *params.preserved_games_page = Some(page.get_current_page());
                        }
                        *params.current_view = ViewMode::Bracket;
                    }
                }
                *params.needs_refresh = true;
            }
            KeyCode::Char('s') => {
                tracing::info!("View toggle requested");
                match *params.current_view {
                    ViewMode::Standings { live_mode } => {
                        *params.preserved_live_mode = live_mode;
                        *params.current_view = ViewMode::Games;
                    }
                    _ => {
                        // Preserve current games page when leaving games
                        if let Some(page) = params.current_page.as_ref() {
                            *params.preserved_games_page = Some(page.get_current_page());
                        }
                        *params.current_view = ViewMode::Standings {
                            live_mode: *params.preserved_live_mode,
                        };
                    }
                }
                *params.needs_refresh = true;
            }
            KeyCode::Char('l') => {
                if let ViewMode::Standings { live_mode } = *params.current_view {
                    tracing::info!("Live mode toggle requested");
                    *params.current_view = ViewMode::Standings {
                        live_mode: !live_mode,
                    };
                    *params.needs_refresh = true;
                }
            }
            KeyCode::Char('t') => {
                tracing::info!("Today's view requested");
                *params.current_date = None;
                *params.needs_refresh = true;
            }
            _ => {}
        }
    }

    Ok(false) // Continue running
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_search_hit_accepts_requested_date() {
        assert_eq!(
            forward_search_hit("2026-08-08", "2026-08-08"),
            Some("2026-08-08".to_string())
        );
    }

    #[test]
    fn test_forward_search_hit_accepts_fallback_jumped_date() {
        // Probing an empty date makes the fetcher's fallback jump ahead and
        // return games for a later date — that later date is the answer.
        assert_eq!(
            forward_search_hit("2026-08-08", "2026-08-14"),
            Some("2026-08-14".to_string())
        );
    }

    #[test]
    fn test_forward_search_hit_rejects_earlier_date() {
        assert_eq!(forward_search_hit("2026-08-08", "2026-08-05"), None);
    }

    #[tokio::test]
    async fn test_search_returns_first_hit() {
        let result = search_dates_with_budget(
            (1..=10).map(|i| format!("2026-08-{i:02}")),
            Duration::from_secs(20),
            |date| async move { (date == "2026-08-03").then_some(date) },
        )
        .await;
        assert_eq!(result, Some("2026-08-03".to_string()));
    }

    #[tokio::test(start_paused = true)]
    async fn test_search_stops_probing_when_budget_exhausted() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let probes = AtomicUsize::new(0);
        let result = search_dates_with_budget(
            (1..=100).map(|i| format!("date-{i}")),
            Duration::from_secs(20),
            |_date| {
                probes.fetch_add(1, Ordering::SeqCst);
                async {
                    // Each probe simulates a slow fetch (paused tokio time).
                    tokio::time::sleep(Duration::from_secs(15)).await;
                    None
                }
            },
        )
        .await;

        assert_eq!(result, None);
        let count = probes.load(Ordering::SeqCst);
        assert!(
            count <= 2,
            "search must stop once the budget is spent, probed {count} times"
        );
    }

    struct KeyEventState {
        current_page: Option<TeletextPage>,
        needs_render: bool,
        needs_refresh: bool,
        current_date: Option<String>,
        last_manual_refresh: Instant,
        last_page_change: Instant,
        last_date_navigation: Instant,
        current_view: ViewMode,
        preserved_games_page: Option<usize>,
        preserved_live_mode: bool,
        page_input: String,
        last_page_input: Instant,
    }

    impl KeyEventState {
        fn new() -> Self {
            Self {
                current_page: None,
                needs_render: false,
                needs_refresh: false,
                current_date: None,
                last_manual_refresh: Instant::now(),
                last_page_change: Instant::now(),
                last_date_navigation: Instant::now(),
                current_view: ViewMode::Games,
                preserved_games_page: None,
                preserved_live_mode: false,
                page_input: String::new(),
                last_page_input: Instant::now(),
            }
        }

        fn params<'a>(&'a mut self, key_event: &'a event::KeyEvent) -> KeyEventParams<'a> {
            KeyEventParams {
                key_event,
                current_page: &mut self.current_page,
                needs_render: &mut self.needs_render,
                needs_refresh: &mut self.needs_refresh,
                current_date: &mut self.current_date,
                last_manual_refresh: &mut self.last_manual_refresh,
                last_page_change: &mut self.last_page_change,
                last_date_navigation: &mut self.last_date_navigation,
                current_view: &mut self.current_view,
                preserved_games_page: &mut self.preserved_games_page,
                preserved_live_mode: &mut self.preserved_live_mode,
                has_bracket_data: false,
                page_input: &mut self.page_input,
                last_page_input: &mut self.last_page_input,
            }
        }
    }

    #[tokio::test]
    async fn test_non_digit_key_clears_partial_page_entry() {
        let mut state = KeyEventState::new();
        state.page_input = String::from("2");

        let key_event = event::KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        let quit = handle_key_event(state.params(&key_event)).await.unwrap();

        assert!(!quit);
        assert!(
            state.page_input.is_empty(),
            "non-digit key must abandon the partial page entry, got {:?}",
            state.page_input
        );
    }

    #[tokio::test]
    async fn test_digit_key_extends_partial_page_entry() {
        let mut state = KeyEventState::new();
        state.page_input = String::from("2");

        let key_event = event::KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE);
        handle_key_event(state.params(&key_event)).await.unwrap();

        assert_eq!(state.page_input, "22");
    }
}
