//! Keyboard input handling and date navigation for the interactive UI.
//!
//! This module handles:
//! - Keyboard event processing (quit, refresh, page navigation)
//! - Date navigation with Shift + Arrow keys
//! - Finding previous/next dates with games
//! - Season boundary checking

use crate::data_fetcher::models::calendar::SeasonCalendar;
use crate::data_fetcher::{fetch_liiga_data, is_historical_date};
use crate::error::AppError;
use crate::teletext_ui::TeletextPage;
use chrono::{Datelike, Days, Local, Months, NaiveDate, Utc};
use crossterm::event::{self, KeyCode, KeyEventKind, KeyModifiers};
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
    /// The day selected in the calendar view
    pub calendar_cursor: &'a mut Option<NaiveDate>,
    /// The season the calendar has fetched, if any
    pub calendar: Option<&'a SeasonCalendar>,
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
    would_be_previous_season_at(date, Utc::now().with_timezone(&Local))
}

/// [`would_be_previous_season`] with an injected "now" for testability.
fn would_be_previous_season_at(date: &str, now: chrono::DateTime<Local>) -> bool {
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
        // Same year - check if we're trying to go to the true off-season.
        // Hockey season: September-February (regular), March-May (playoffs/playout),
        // preseason practice games (valmistavat_ottelut) in August-early September.
        // Off-season with no games at all: June-July.

        // If we're in the new regular season (September-December) and the date is
        // from the gameless off-season (June-July), block it. August stays
        // navigable: its practice games belong to the season that starts in
        // September, i.e. the current one.
        if (9..=12).contains(&current_month) && (6..=7).contains(&date_month) {
            return true;
        }
    } else if date_year == current_year - 1 {
        // Previous year - allow access to recent hockey season games
        // Only block if we're trying to access very old off-season games

        // If we're currently in the new season (September+) and trying to access
        // off-season/preseason games from the previous year (June-August) - which
        // belong to the season before the current one - block it
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
/// once the time budget is exhausted. Each probe receives a timeout capped
/// at the remaining budget, so a single slow probe cannot block past the
/// budget (the search blocks the event loop).
async fn search_dates_with_budget<F, Fut>(
    candidates: impl Iterator<Item = String>,
    budget: Duration,
    probe_timeout: Duration,
    mut probe: F,
) -> Option<String>
where
    F: FnMut(String, Duration) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
{
    let started = tokio::time::Instant::now();
    for date in candidates {
        let remaining = budget.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            tracing::warn!("Date search time budget exhausted, giving up");
            return None;
        }
        if let Some(hit) = probe(date, probe_timeout.min(remaining)).await {
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

    let search_started = tokio::time::Instant::now();
    let probe_timeout = probe_timeout_duration().await;
    let remaining_budget = || MAX_DATE_SEARCH_DURATION.saturating_sub(search_started.elapsed());

    // Fast path: the current date's (usually cached) schedule responses carry
    // the API's previousGameDate hint — jump straight there instead of probing
    // every intermediate empty day with a full fetch each. The hint fetch and
    // probe share the scan's time budget so the whole search stays bounded.
    if let Ok(Some(hint)) = tokio::time::timeout(
        remaining_budget(),
        crate::data_fetcher::api::fetch_previous_game_date_hint(current_date),
    )
    .await
        && !would_be_previous_season(&hint)
    {
        tracing::info!("Trying previousGameDate hint: {hint}");
        let hit = probe_date(
            hint,
            probe_timeout.min(remaining_budget()),
            |requested, fetched| (fetched == requested).then(|| requested.to_string()),
        )
        .await;
        if hit.is_some() {
            return hit;
        }
        tracing::info!("previousGameDate hint had no usable games, falling back to scan");
    }

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

    let result = search_dates_with_budget(
        candidates,
        remaining_budget(),
        probe_timeout,
        |date_string, timeout| {
            // Only a result for the probed date itself counts as a hit here:
            // the fetcher's fallback can jump ahead, but a later date is in the
            // wrong direction for a backward search.
            probe_date(date_string, timeout, |requested, fetched| {
                (fetched == requested).then(|| requested.to_string())
            })
        },
    )
    .await;

    if result.is_none() {
        tracing::info!(
            "No previous date with games found within current season from {}",
            current_date
        );
    }
    result
}

/// Per-probe timeout: the configured HTTP timeout plus a safety margin so the
/// HTTP layer reports the actual error before the outer timeout fires. Falls
/// back to the default when the config cannot be loaded.
async fn probe_timeout_duration() -> Duration {
    let timeout_seconds = crate::config::Config::load()
        .await
        .map(|config| config.http_timeout_seconds)
        .unwrap_or(crate::constants::DEFAULT_HTTP_TIMEOUT_SECONDS);
    crate::constants::http_timeout_with_margin(timeout_seconds)
}

/// Fetches games for one candidate date and applies `accept` to decide
/// whether the (possibly fallback-shifted) result concludes the search.
async fn probe_date<A>(date_string: String, timeout_duration: Duration, accept: A) -> Option<String>
where
    A: Fn(&str, &str) -> Option<String>,
{
    let fetch_future = fetch_liiga_data(Some(date_string.clone()));

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

    let search_started = tokio::time::Instant::now();
    let probe_timeout = probe_timeout_duration().await;
    let remaining_budget = || MAX_DATE_SEARCH_DURATION.saturating_sub(search_started.elapsed());

    // Fast path: the current date's (usually cached) schedule responses carry
    // the API's nextGameDate hint — jump straight there instead of probing
    // every intermediate empty day with a full fetch each. (The scan's first
    // candidate would usually also resolve this via the fetcher's forward
    // fallback, but the hint skips that extra probe when the hint fetch is
    // served from cache.) The hint fetch and probe share the scan's time
    // budget so the whole search stays bounded.
    if let Ok(Some(hint)) = tokio::time::timeout(
        remaining_budget(),
        crate::data_fetcher::api::fetch_next_game_date_hint(current_date),
    )
    .await
    {
        tracing::info!("Trying nextGameDate hint: {hint}");
        let hit = probe_date(
            hint,
            probe_timeout.min(remaining_budget()),
            forward_search_hit,
        )
        .await;
        if hit.is_some() {
            return hit;
        }
        tracing::info!("nextGameDate hint had no usable games, falling back to scan");
    }

    // Search up to 60 days in the future (handles off-season periods),
    // giving up once the time budget is spent.
    let candidates = (1..=60).filter_map(|days_ahead| {
        current_parsed
            .checked_add_days(chrono::Days::new(days_ahead))
            .map(|d| d.format("%Y-%m-%d").to_string())
    });

    let result = search_dates_with_budget(
        candidates,
        remaining_budget(),
        probe_timeout,
        |date_string, timeout| probe_date(date_string, timeout, forward_search_hit),
    )
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
const PAGE_CALENDAR: &str = "224";

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
        PAGE_CALENDAR => open_calendar(params),
        other => {
            let number = other.parse::<u16>().unwrap_or(0);
            tracing::info!("Page entry: page {number} not found");
            *params.current_page = Some(super::navigation_manager::create_page_not_found_page(
                number,
            ));
        }
    }
}

/// Opens the calendar at the date on screen, or at today when no date is set.
fn open_calendar(params: &mut KeyEventParams<'_>) {
    if matches!(*params.current_view, ViewMode::Calendar) {
        return;
    }
    tracing::info!("Switching to calendar view");
    let cursor = params
        .current_date
        .as_deref()
        .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        .unwrap_or_else(|| Local::now().date_naive());

    match *params.current_view {
        // Keep the games page position, so leaving the calendar rebuilds the
        // games page from the games already fetched
        ViewMode::Games => {
            if let Some(page) = params.current_page.as_ref() {
                *params.preserved_games_page = Some(page.get_current_page());
            }
        }
        ViewMode::Standings { live_mode } => *params.preserved_live_mode = live_mode,
        ViewMode::Bracket | ViewMode::Calendar => {}
    }
    *params.calendar_cursor = Some(cursor);
    *params.current_view = ViewMode::Calendar;
    // Shows the stored season right away if it has the day; the refresh
    // brings it up to date
    *params.needs_refresh = true;
}

/// Leaves the calendar for the games page of `date`, or of the date already
/// on screen when `date` is `None`.
fn close_calendar(params: &mut KeyEventParams<'_>, date: Option<NaiveDate>) {
    let new_date = date
        .map(|day| day.format("%Y-%m-%d").to_string())
        .filter(|day| params.current_date.as_deref() != Some(day.as_str()));
    match new_date {
        Some(day) => {
            tracing::info!("Calendar: opening games of {day}");
            *params.current_date = Some(day);
            *params.preserved_games_page = None;
        }
        None => {
            // Same date as before: rebuild the games page from the games
            // already fetched. A fetch would find them unchanged and bring no
            // new page, leaving the calendar on screen.
            params.preserved_games_page.get_or_insert(0);
        }
    }
    *params.current_view = ViewMode::Games;
    *params.needs_refresh = true;
}

/// Handles a key in the calendar view. Returns false for keys the calendar
/// doesn't use; those work as in the other views (q, s, p, r, digits).
///
/// Only the cursor changes here: the event handler rebuilds the page.
fn handle_calendar_key(params: &mut KeyEventParams<'_>) -> bool {
    let Some(cursor) = *params.calendar_cursor else {
        return false;
    };
    let shift = params.key_event.modifiers.contains(KeyModifiers::SHIFT);
    let new_cursor = match params.key_event.code {
        KeyCode::Left if shift => cursor.checked_sub_months(Months::new(1)),
        KeyCode::Right if shift => cursor.checked_add_months(Months::new(1)),
        KeyCode::Left => cursor.pred_opt(),
        KeyCode::Right => cursor.succ_opt(),
        KeyCode::Up => cursor.checked_sub_days(Days::new(7)),
        KeyCode::Down => cursor.checked_add_days(Days::new(7)),
        KeyCode::Char('t') => Some(Local::now().date_naive()),
        KeyCode::Enter => {
            // A day without games has nothing to open
            if params
                .calendar
                .is_some_and(|calendar| calendar.has_games_on(cursor))
            {
                close_calendar(params, Some(cursor));
            }
            return true;
        }
        KeyCode::Esc | KeyCode::Char('k') => {
            close_calendar(params, None);
            return true;
        }
        _ => return false,
    };
    if let Some(day) = new_cursor {
        *params.calendar_cursor = Some(day);
    }
    true
}

/// Direction of a Shift+arrow date search.
#[derive(Debug, Clone, Copy)]
enum DateDirection {
    Previous,
    Next,
}

/// Moves to the nearest date with games in `direction` (Shift + Left/Right).
///
/// The search runs in a background task; a loading spinner appears, and
/// animates, only if it outlasts the grace period. Presses closer together
/// than 250 ms are ignored.
async fn navigate_to_date_with_games(params: &mut KeyEventParams<'_>, direction: DateDirection) {
    if params.last_date_navigation.elapsed() < Duration::from_millis(250) {
        return;
    }

    let (label, search_message) = match direction {
        DateDirection::Previous => ("previous", "Etsitään edellisiä otteluita..."),
        DateDirection::Next => ("next", "Etsitään seuraavia otteluita..."),
    };
    tracing::debug!("Current date state: {:?}", params.current_date);
    let target_date = get_target_date_for_navigation(params.current_date);
    tracing::info!("Searching for {label} date with games from: {target_date}");

    let search_handle = tokio::task::spawn(async move {
        match direction {
            DateDirection::Previous => find_previous_date_with_games(&target_date).await,
            DateDirection::Next => find_next_date_with_games(&target_date).await,
        }
    });
    let result = super::indicators::animate_page_during_task_with_grace(
        params.current_page,
        search_handle,
        Some(search_message),
        super::indicators::SEARCH_LOADING_GRACE,
    )
    .await
    .unwrap_or_else(|join_error| {
        tracing::error!("Date search task failed: {join_error}");
        None
    });

    // The search awaited every fetch it made, so their cache writes are done
    // and the refresh can start right away.
    if let Some(date) = result {
        tracing::info!("Navigated to {label} date: {date}");
        *params.current_date = Some(date);
        *params.needs_refresh = true;
    } else {
        tracing::warn!("No {label} date with games found");
    }

    // Hide the loading indicator and repaint to erase it, if it was shown
    if let Some(page) = params.current_page.as_mut()
        && page.is_loading_indicator_active()
    {
        page.hide_loading();
        *params.needs_render = true;
    }
    *params.last_date_navigation = Instant::now();
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

    if matches!(*params.current_view, ViewMode::Calendar) && handle_calendar_key(&mut params) {
        return Ok(false);
    }

    // Disable date navigation outside the games view
    let is_non_game_view = matches!(
        params.current_view,
        ViewMode::Standings { .. } | ViewMode::Bracket | ViewMode::Calendar
    );

    // Check for date navigation first (Shift + Arrow keys)
    if !is_non_game_view && is_date_navigation_key(params.key_event, true) {
        navigate_to_date_with_games(&mut params, DateDirection::Previous).await;
    } else if !is_non_game_view && is_date_navigation_key(params.key_event, false) {
        navigate_to_date_with_games(&mut params, DateDirection::Next).await;
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
                    crate::data_fetcher::cache::clear_response_caches().await;
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
                        if matches!(*params.current_view, ViewMode::Games)
                            && let Some(page) = params.current_page.as_ref()
                        {
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
                        if matches!(*params.current_view, ViewMode::Games)
                            && let Some(page) = params.current_page.as_ref()
                        {
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
            KeyCode::Char('k') => open_calendar(&mut params),
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

    fn local_datetime(date: &str) -> chrono::DateTime<Local> {
        use chrono::TimeZone;
        let naive = NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        Local.from_local_datetime(&naive).unwrap()
    }

    #[test]
    fn test_august_practice_games_belong_to_current_season() {
        // Practice games (valmistavat_ottelut) are played in August and belong
        // to the season that starts in September: navigating back to them from
        // the regular season must stay allowed.
        for now in ["2026-09-15", "2026-10-15", "2026-12-15"] {
            assert!(
                !would_be_previous_season_at("2026-08-20", local_datetime(now)),
                "August practice games should be reachable from {now}"
            );
        }
    }

    #[test]
    fn test_gameless_offseason_is_previous_season() {
        // June-July of the same year have no games; from the new regular
        // season they count as the previous season's territory
        assert!(would_be_previous_season_at(
            "2026-07-10",
            local_datetime("2026-09-15")
        ));
        assert!(would_be_previous_season_at(
            "2026-06-10",
            local_datetime("2026-11-15")
        ));
    }

    #[test]
    fn test_previous_year_preseason_is_previous_season() {
        // August of the PREVIOUS year belongs to the season before the current
        // one once the new season has started
        assert!(would_be_previous_season_at(
            "2025-08-20",
            local_datetime("2026-10-15")
        ));
        // ...but while the current season is still running (before September),
        // the previous year's August preseason opened this same season
        assert!(!would_be_previous_season_at(
            "2026-08-20",
            local_datetime("2027-02-15")
        ));
    }

    #[test]
    fn test_dates_older_than_two_years_are_previous_season() {
        assert!(would_be_previous_season_at(
            "2024-05-01",
            local_datetime("2026-08-14")
        ));
    }

    #[test]
    fn test_malformed_date_is_not_previous_season() {
        assert!(!would_be_previous_season_at(
            "not-a-date",
            local_datetime("2026-08-14")
        ));
        assert!(!would_be_previous_season_at(
            "2026",
            local_datetime("2026-08-14")
        ));
    }

    /// A single slow probe must not block past the search budget: its timeout
    /// is capped at the remaining budget, and once the budget is spent no
    /// further probes run.
    #[tokio::test(start_paused = true)]
    async fn test_search_probe_timeout_capped_by_remaining_budget() {
        let mut granted_timeouts = Vec::new();
        let result = search_dates_with_budget(
            ["2026-08-28".to_string(), "2026-08-29".to_string()].into_iter(),
            Duration::from_secs(10),
            Duration::from_secs(35),
            |_, timeout| {
                granted_timeouts.push(timeout);
                async move {
                    // Consume the entire allowance without finding anything
                    tokio::time::sleep(timeout).await;
                    None
                }
            },
        )
        .await;

        assert_eq!(result, None);
        // First probe was capped to the 10s budget; the second never ran
        assert_eq!(granted_timeouts, vec![Duration::from_secs(10)]);
    }

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
            Duration::from_secs(15),
            |date, _timeout| async move { (date == "2026-08-03").then_some(date) },
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
            Duration::from_secs(15),
            |_date, _timeout| {
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
        calendar_cursor: Option<NaiveDate>,
        calendar: Option<SeasonCalendar>,
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
                calendar_cursor: None,
                calendar: None,
            }
        }

        /// A state in the calendar view at `cursor`, with a season calendar
        /// that has games on `game_days`.
        fn in_calendar(cursor: &str, game_days: &[&str]) -> Self {
            use crate::data_fetcher::models::ScheduleApiGame;
            let games: Vec<ScheduleApiGame> = game_days
                .iter()
                .map(|day| ScheduleApiGame {
                    id: 1,
                    season: 2027,
                    start: format!("{day}T12:00:00Z"),
                    home_team_name: "HIFK".to_string(),
                    away_team_name: "TPS".to_string(),
                    serie: 1,
                    finished_type: None,
                    started: false,
                    ended: false,
                    game_time: None,
                    play_off_phase: None,
                    play_off_pair: None,
                    play_off_req_wins: None,
                    home_team_goals: 0,
                    away_team_goals: 0,
                })
                .collect();
            let mut state = Self::new();
            state.current_view = ViewMode::Calendar;
            state.calendar_cursor = Some(day(cursor));
            state.calendar = Some(SeasonCalendar::from_schedule_in(
                2027,
                &games,
                &chrono::FixedOffset::east_opt(7200).unwrap(),
            ));
            state
        }

        async fn press(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
            let key_event = event::KeyEvent::new(code, modifiers);
            handle_key_event(self.params(&key_event)).await.unwrap()
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
                calendar_cursor: &mut self.calendar_cursor,
                calendar: self.calendar.as_ref(),
            }
        }
    }

    fn day(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    #[tokio::test]
    async fn test_k_opens_calendar_at_date_on_screen() {
        let mut state = KeyEventState::new();
        state.current_date = Some("2026-11-20".to_string());
        state.current_page = Some(TeletextPage::new(221, "JÄÄKIEKKO", "SM-LIIGA"));

        state.press(KeyCode::Char('k'), KeyModifiers::NONE).await;

        assert_eq!(state.current_view, ViewMode::Calendar);
        assert_eq!(state.calendar_cursor, Some(day("2026-11-20")));
        assert_eq!(
            state.preserved_games_page,
            Some(0),
            "the games page position is kept for coming back"
        );
        assert!(state.needs_refresh);
    }

    #[tokio::test]
    async fn test_page_224_opens_calendar() {
        let mut state = KeyEventState::new();
        state.current_date = Some("2026-11-20".to_string());

        for digit in ['2', '2', '4'] {
            state.press(KeyCode::Char(digit), KeyModifiers::NONE).await;
        }

        assert_eq!(state.current_view, ViewMode::Calendar);
        assert_eq!(state.calendar_cursor, Some(day("2026-11-20")));
    }

    #[tokio::test]
    async fn test_calendar_arrows_move_cursor() {
        let mut state = KeyEventState::in_calendar("2026-11-27", &[]);

        state.press(KeyCode::Right, KeyModifiers::NONE).await;
        assert_eq!(state.calendar_cursor, Some(day("2026-11-28")));
        state.press(KeyCode::Left, KeyModifiers::NONE).await;
        state.press(KeyCode::Left, KeyModifiers::NONE).await;
        assert_eq!(state.calendar_cursor, Some(day("2026-11-26")));
        state.press(KeyCode::Down, KeyModifiers::NONE).await;
        assert_eq!(state.calendar_cursor, Some(day("2026-12-03")));
        state.press(KeyCode::Up, KeyModifiers::NONE).await;
        assert_eq!(state.calendar_cursor, Some(day("2026-11-26")));

        assert_eq!(state.current_view, ViewMode::Calendar);
        assert!(
            !state.needs_refresh,
            "moving the cursor must not fetch; the event handler redraws the page"
        );
    }

    #[tokio::test]
    async fn test_calendar_shift_arrows_move_by_month() {
        let mut state = KeyEventState::in_calendar("2027-01-31", &[]);

        state.press(KeyCode::Right, KeyModifiers::SHIFT).await;
        assert_eq!(
            state.calendar_cursor,
            Some(day("2027-02-28")),
            "a day past the month's end clamps to its last day"
        );
        state.press(KeyCode::Left, KeyModifiers::SHIFT).await;
        state.press(KeyCode::Left, KeyModifiers::SHIFT).await;
        assert_eq!(state.calendar_cursor, Some(day("2026-12-28")));
    }

    #[tokio::test]
    async fn test_calendar_enter_opens_games_of_selected_day() {
        let mut state = KeyEventState::in_calendar("2026-11-27", &["2026-11-27"]);
        state.current_date = Some("2026-11-20".to_string());
        state.preserved_games_page = Some(2);

        state.press(KeyCode::Enter, KeyModifiers::NONE).await;

        assert_eq!(state.current_view, ViewMode::Games);
        assert_eq!(state.current_date.as_deref(), Some("2026-11-27"));
        assert_eq!(
            state.preserved_games_page, None,
            "another day's games must be fetched, not rebuilt from the old ones"
        );
        assert!(state.needs_refresh);
    }

    #[tokio::test]
    async fn test_calendar_enter_on_day_without_games_does_nothing() {
        let mut state = KeyEventState::in_calendar("2026-11-26", &["2026-11-27"]);
        state.current_date = Some("2026-11-20".to_string());

        state.press(KeyCode::Enter, KeyModifiers::NONE).await;

        assert_eq!(state.current_view, ViewMode::Calendar);
        assert_eq!(state.current_date.as_deref(), Some("2026-11-20"));
        assert!(!state.needs_refresh);
    }

    #[tokio::test]
    async fn test_calendar_enter_on_date_already_shown_rebuilds_games_page() {
        let mut state = KeyEventState::in_calendar("2026-11-27", &["2026-11-27"]);
        state.current_date = Some("2026-11-27".to_string());

        state.press(KeyCode::Enter, KeyModifiers::NONE).await;

        assert_eq!(state.current_view, ViewMode::Games);
        assert_eq!(
            state.preserved_games_page,
            Some(0),
            "unchanged games would bring no new page, so they are rebuilt"
        );
    }

    #[tokio::test]
    async fn test_calendar_back_keys_return_to_games() {
        for code in [KeyCode::Esc, KeyCode::Char('k')] {
            let mut state = KeyEventState::in_calendar("2026-11-27", &["2026-11-27"]);
            state.current_date = Some("2026-11-20".to_string());
            state.preserved_games_page = Some(1);

            state.press(code, KeyModifiers::NONE).await;

            assert_eq!(state.current_view, ViewMode::Games, "{code:?}");
            assert_eq!(state.current_date.as_deref(), Some("2026-11-20"));
            assert_eq!(state.preserved_games_page, Some(1));
        }
    }

    #[tokio::test]
    async fn test_calendar_passes_other_keys_on() {
        let mut state = KeyEventState::in_calendar("2026-11-27", &[]);

        state.press(KeyCode::Char('s'), KeyModifiers::NONE).await;

        assert_eq!(state.current_view, ViewMode::Standings { live_mode: false });
        assert!(
            state.press(KeyCode::Char('q'), KeyModifiers::NONE).await,
            "q quits from every view"
        );
    }

    #[tokio::test]
    async fn test_leaving_calendar_for_standings_keeps_games_page_position() {
        let mut state = KeyEventState::in_calendar("2026-11-27", &[]);
        state.preserved_games_page = Some(3);
        state.current_page = Some(TeletextPage::new(224, "JÄÄKIEKKO", "OTTELUKALENTERI"));

        state.press(KeyCode::Char('s'), KeyModifiers::NONE).await;

        assert_eq!(
            state.preserved_games_page,
            Some(3),
            "the calendar page's position is not a games page position"
        );
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

    #[tokio::test]
    #[serial_test::serial]
    async fn test_manual_refresh_skips_cached_responses() {
        use crate::data_fetcher::cache::{
            cache_http_response, cache_tournament_data, get_cached_http_response,
            get_cached_tournament_data,
        };
        use crate::data_fetcher::models::ScheduleResponse;

        let url = "https://example.test/api/v2/games?tournament=runkosarja&date=2026-09-25";
        cache_http_response(url.to_string(), "{}".to_string(), 1800).await;
        // The tournament cache is read before any HTTP request, so on a day
        // without live games it is what would serve stale data on refresh
        let tournament_key = "runkosarja-2026-09-25";
        cache_tournament_data(
            tournament_key.to_string(),
            ScheduleResponse {
                games: Vec::new(),
                previous_game_date: None,
                next_game_date: None,
            },
        )
        .await;

        let mut state = KeyEventState::new();
        state.last_manual_refresh = Instant::now() - Duration::from_secs(20);

        let key_event = event::KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
        handle_key_event(state.params(&key_event)).await.unwrap();

        assert!(state.needs_refresh);
        assert!(
            get_cached_http_response(url).await.is_none(),
            "manual refresh must fetch fresh data, not reuse the cached response"
        );
        assert!(
            get_cached_tournament_data(tournament_key).await.is_none(),
            "manual refresh must not reuse cached tournament data"
        );
    }
}
