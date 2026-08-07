//! Navigation management for interactive UI
//!
//! This module handles all aspects of page navigation, creation, and management
//! for the interactive UI, including:
//! - Page creation for different game types (regular, future, loading, error)
//! - Page restoration and state management
//! - Game analysis and validation for navigation decisions
//! - Loading indicator coordination

use super::series_utils::{get_subheader, is_playoff_type, playoff_phase_name, series_group_label};
use crate::data_fetcher::models::bracket::PlayoffBracket;
use crate::data_fetcher::{GameData, is_historical_date};
use crate::teletext_ui::bracket_display::render_bracket;
use crate::teletext_ui::{GameResultData, TeletextPage, TeletextRow};
use chrono::NaiveDate;
use std::collections::{HashMap, HashSet};

/// Configuration for creating or restoring a teletext page
#[derive(Debug)]
pub struct PageCreationConfig<'a> {
    pub games: &'a [GameData],
    pub disable_links: bool,
    pub compact_mode: bool,
    pub wide_mode: bool,
    pub fetched_date: &'a str,
    pub preserved_page_for_restoration: Option<usize>,
    pub current_date: &'a Option<String>,
    pub updated_current_date: &'a Option<String>,
}

/// Parameters for page restoration operations
#[derive(Debug)]
pub struct PageRestorationParams<'a> {
    pub current_page: &'a mut Option<TeletextPage>,
    pub data_changed: bool,
    pub had_error: bool,
    pub preserved_page_for_restoration: Option<usize>,
    pub games: &'a [GameData],
    pub last_games: &'a [GameData],
    pub disable_links: bool,
    pub fetched_date: &'a str,
    pub updated_current_date: &'a Option<String>,
    pub compact_mode: bool,
    pub wide_mode: bool,
}

/// Configuration for loading indicators
#[derive(Debug)]
pub struct LoadingIndicatorConfig<'a> {
    pub should_show_loading: bool,
    pub current_date: &'a Option<String>,
    pub disable_links: bool,
    pub compact_mode: bool,
    pub wide_mode: bool,
}

/// Creates or restores a teletext page based on the current state and data
pub async fn create_or_restore_page(config: PageCreationConfig<'_>) -> Option<TeletextPage> {
    // Restore the preserved page number
    if let Some(preserved_page_for_restoration) = config.preserved_page_for_restoration {
        let mut page = create_page(
            config.games,
            config.disable_links,
            true,
            false,
            config.compact_mode,
            config.wide_mode,
            false, // suppress_countdown - false for interactive mode
            Some(config.fetched_date.to_string()),
            Some(preserved_page_for_restoration),
        )
        .await;

        // Disable auto-refresh for historical dates
        if let Some(date) = config.updated_current_date
            && is_historical_date(date)
        {
            page.set_auto_refresh_disabled(true);
        }

        Some(page)
    } else {
        let page = if config.games.is_empty() {
            create_error_page(
                config.fetched_date,
                config.disable_links,
                config.compact_mode,
                config.wide_mode,
            )
        } else {
            // Try to create a future games page, fall back to regular page if not future games
            let show_future_header = config.current_date.is_none();
            match create_future_games_page(
                config.games,
                config.disable_links,
                true,
                false,
                config.compact_mode,
                config.wide_mode,
                false, // suppress_countdown - false for interactive mode
                show_future_header,
                Some(config.fetched_date.to_string()),
                None,
            )
            .await
            {
                Some(page) => page,
                None => {
                    let mut page = create_page(
                        config.games,
                        config.disable_links,
                        true,
                        false,
                        config.compact_mode,
                        config.wide_mode,
                        false, // suppress_countdown - false for interactive mode
                        Some(config.fetched_date.to_string()),
                        None,
                    )
                    .await;

                    // Disable auto-refresh for historical dates
                    if let Some(date) = config.updated_current_date
                        && is_historical_date(date)
                    {
                        page.set_auto_refresh_disabled(true);
                    }

                    page
                }
            }
        };

        Some(page)
    }
}

/// Handles page restoration when loading screen was shown but data didn't change
pub async fn handle_page_restoration(params: PageRestorationParams<'_>) -> bool {
    let mut needs_render = false;

    // If we showed a loading screen but data didn't change, we still need to restore pagination
    if !params.data_changed
        && !params.had_error
        && params.preserved_page_for_restoration.is_some()
        && let Some(current) = params.current_page
    {
        // Check if current page is a loading page using the dedicated marker
        if current.is_loading_page()
            && let Some(preserved_page_for_restoration) = params.preserved_page_for_restoration
        {
            let games_to_use = if params.games.is_empty() {
                params.last_games
            } else {
                params.games
            };
            let mut page = create_page(
                games_to_use,
                params.disable_links,
                true,
                false,
                params.compact_mode,
                params.wide_mode,
                false, // suppress_countdown - false for interactive mode
                Some(params.fetched_date.to_string()),
                Some(preserved_page_for_restoration),
            )
            .await;

            // Disable auto-refresh for historical dates
            if let Some(date) = params.updated_current_date
                && is_historical_date(date)
            {
                page.set_auto_refresh_disabled(true);
            }

            *params.current_page = Some(page);
            needs_render = true;
        }
    }

    needs_render
}

/// Manages loading and auto-refresh indicators for the current page
pub fn manage_loading_indicators(
    current_page: &mut Option<TeletextPage>,
    config: LoadingIndicatorConfig<'_>,
) -> bool {
    if config.should_show_loading {
        *current_page = Some(create_loading_page(
            config.current_date,
            config.disable_links,
            config.compact_mode,
            config.wide_mode,
        ));
        true
    } else {
        tracing::debug!("Skipping loading screen due to ongoing games");
        false
    }
}

/// Creates a base TeletextPage with common initialization logic
#[allow(clippy::too_many_arguments)]
async fn create_base_page(
    games: &[GameData],
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
    compact_mode: bool,
    wide_mode: bool,
    suppress_countdown: bool,
    future_games_header: Option<String>,
    fetched_date: Option<String>,
    current_page: Option<usize>,
) -> TeletextPage {
    let subheader = get_subheader(games);
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        subheader,
        disable_video_links,
        show_footer,
        ignore_height_limit,
        compact_mode,
        wide_mode,
    );

    // Set the fetched date if provided
    if let Some(date) = fetched_date {
        page.set_fetched_date(date);
    }

    // Add future games header first if provided
    if let Some(header) = future_games_header {
        page.add_future_games_header(header);
    }

    // Sort games by serie then play_off_phase for grouping, then add phase headers.
    // Playoffs come before playout/qualifications so they display first.
    // Placeholder games (teams not yet determined) are kept in the data to
    // prevent transient-empty detection from triggering, but are filtered
    // out of the display since their cryptic API names (e.g. "RS5", "QF2")
    // would confuse users.
    let mut sorted_games: Vec<&GameData> = games.iter().filter(|g| !g.is_placeholder).collect();

    // Games that share a serie belong together. Preseason days in particular mix
    // an API-named tournament (e.g. PITSITURNAUS) with standalone PRACTICE games,
    // and sorting purely by start time interleaves the two. Non-playoff series are
    // ordered by their earliest game so the day still reads chronologically.
    let mut group_start: HashMap<String, &str> = HashMap::new();
    for game in &sorted_games {
        let entry = group_start
            .entry(game.serie.to_ascii_lowercase())
            .or_insert(game.start.as_str());
        if game.start.as_str() < *entry {
            *entry = game.start.as_str();
        }
    }

    sorted_games.sort_by_key(|g| {
        let serie_key = g.serie.to_ascii_lowercase();
        let serie_order = match serie_key.as_str() {
            "playoffs" => 0,
            "playout" => 1,
            "qualifications" => 2,
            _ => 3,
        };
        // playOffPhase is 0 on every preseason game, so it only orders playoff series.
        let phase = if is_playoff_type(&g.serie) {
            g.play_off_phase.unwrap_or(i32::MAX)
        } else {
            i32::MAX
        };
        (
            serie_order,
            group_start
                .get(&serie_key)
                .copied()
                .unwrap_or("")
                .to_string(),
            serie_key,
            phase,
            g.start.clone(),
            g.play_off_pair.unwrap_or(i32::MAX),
            g.home_team.clone(),
        )
    });

    // Only label the groups when the day actually holds more than one serie —
    // an ordinary runkosarja day would just repeat the subheader.
    let distinct_series = sorted_games
        .iter()
        .map(|g| g.serie.to_ascii_lowercase())
        .collect::<HashSet<_>>()
        .len();
    let show_series_headers = distinct_series > 1;

    // Phase headers only make sense for playoff-type series. The preseason API
    // sets playOffPhase to 0 on every game, so gating on the field alone would
    // emit a generic "OTTELUT" header at each serie/phase boundary.
    let mut last_header: Option<(&str, i32)> = None;
    let mut last_serie: Option<String> = None;
    for game in &sorted_games {
        let serie_key = game.serie.to_ascii_lowercase();
        // Playoff-type series are already labelled by their phase header.
        if show_series_headers
            && !is_playoff_type(&game.serie)
            && last_serie.as_deref() != Some(serie_key.as_str())
        {
            page.add_series_header(series_group_label(&game.serie));
        }
        last_serie = Some(serie_key);

        if let Some(phase) = game.play_off_phase
            && is_playoff_type(&game.serie)
        {
            let key = (game.serie.as_str(), phase);
            if last_header != Some(key) {
                let header = playoff_phase_name(phase, &game.serie);
                page.add_playoff_phase_header(header.to_string());
                last_header = Some(key);
            }
        }
        page.add_game_result(GameResultData::new(game));
    }

    // Set season countdown if regular season hasn't started yet (unless suppressed)
    if !suppress_countdown {
        page.set_show_season_countdown(games).await;
    }

    // Set the current page AFTER content is added (so total_pages() is correct)
    if let Some(page_num) = current_page {
        page.set_current_page(page_num);
    }

    page
}

/// Creates a TeletextPage for regular games
#[allow(clippy::too_many_arguments)]
pub async fn create_page(
    games: &[GameData],
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
    compact_mode: bool,
    wide_mode: bool,
    suppress_countdown: bool,
    fetched_date: Option<String>,
    current_page: Option<usize>,
) -> TeletextPage {
    create_base_page(
        games,
        disable_video_links,
        show_footer,
        ignore_height_limit,
        compact_mode,
        wide_mode,
        suppress_countdown,
        None,
        fetched_date,
        current_page,
    )
    .await
}

/// Creates a TeletextPage for future games if the games are scheduled
#[allow(clippy::too_many_arguments)]
pub async fn create_future_games_page(
    games: &[GameData],
    disable_video_links: bool,
    show_footer: bool,
    ignore_height_limit: bool,
    compact_mode: bool,
    wide_mode: bool,
    suppress_countdown: bool,
    show_future_header: bool,
    fetched_date: Option<String>,
    current_page: Option<usize>,
) -> Option<TeletextPage> {
    // Check if these are future games by validating both time and start fields
    if !games.is_empty() && is_future_game(&games[0]) {
        // Extract date from the first game's start field (assuming format YYYY-MM-DDThh:mm:ssZ)
        let start_str = &games[0].start;
        let date_str = start_str.split('T').next().unwrap_or("");
        let formatted_date = format_date_for_display(date_str);

        tracing::debug!(
            "First game serie: '{}', subheader: '{}'",
            games[0].serie,
            get_subheader(games)
        );

        let future_games_header = if show_future_header {
            Some(format!("Seuraavat ottelut {formatted_date}"))
        } else {
            None
        };
        let mut page = create_base_page(
            games,
            disable_video_links,
            show_footer,
            ignore_height_limit,
            compact_mode,
            wide_mode,
            suppress_countdown,
            future_games_header,
            fetched_date, // Pass the fetched date to show it in the header
            current_page,
        )
        .await;

        // Set auto-refresh disabled for scheduled games
        page.set_auto_refresh_disabled(true);

        Some(page)
    } else {
        None
    }
}

/// Create loading page for data fetching
pub fn create_loading_page(
    current_date: &Option<String>,
    disable_links: bool,
    compact_mode: bool,
    wide_mode: bool,
) -> TeletextPage {
    let mut loading_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        disable_links,
        true,
        false,
        compact_mode,
        wide_mode,
    );

    loading_page.set_is_loading_page(true);

    if let Some(date) = current_date {
        if is_historical_date(date) {
            loading_page.add_error_message(&format!(
                "Haetaan historiallista dataa päivälle {}...",
                format_date_for_display(date)
            ));
            loading_page.add_error_message("Tämä voi kestää hetken, odotathan...");
        } else {
            loading_page.add_error_message(&format!(
                "Haetaan otteluita päivälle {}...",
                format_date_for_display(date)
            ));
        }
    } else {
        loading_page.add_error_message("Haetaan päivän otteluita...");
    }

    loading_page
}

/// Create error page for empty games
pub fn create_error_page(
    fetched_date: &str,
    disable_links: bool,
    compact_mode: bool,
    wide_mode: bool,
) -> TeletextPage {
    let mut error_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        disable_links,
        true,
        false,
        compact_mode,
        wide_mode,
    );

    let formatted_date = format_date_for_display(fetched_date);

    if is_historical_date(fetched_date) {
        error_page.add_error_message(&format!("Ei otteluita päivälle {}", formatted_date));
        error_page.add_error_message("");
        error_page.add_error_message("Käytä Shift + nuolia siirtyäksesi toiselle päivälle");
        error_page.add_error_message("tai käynnistä sovellus uudelleen (-d parametrilla)");
        error_page.add_error_message("nähdäksesi päivän ottelut.");
    } else {
        error_page.add_error_message(&format!("Ei otteluita päivälle {}", formatted_date));
        error_page.add_error_message("");
        error_page.add_error_message("Käytä Shift + nuolia siirtyäksesi toiselle päivälle");
        error_page.add_error_message("tai paina 'r' päivittääksesi tiedot.");
    }

    error_page
}

/// 3x5 block-graphics glyphs for the "SIVUA EI LÖYDY" page art.
/// Returns the five rows of a glyph, or None for unsupported characters.
fn block_glyph(c: char) -> Option<[&'static str; 5]> {
    match c {
        'S' => Some(["███", "█  ", "███", "  █", "███"]),
        'I' => Some(["███", " █ ", " █ ", " █ ", "███"]),
        'V' => Some(["█ █", "█ █", "█ █", "█ █", " █ "]),
        'U' => Some(["█ █", "█ █", "█ █", "█ █", "███"]),
        'A' => Some([" █ ", "█ █", "███", "█ █", "█ █"]),
        'E' => Some(["███", "█  ", "███", "█  ", "███"]),
        'L' => Some(["█  ", "█  ", "█  ", "█  ", "███"]),
        // Ö: umlaut dots on the first row, squashed O below
        'Ö' => Some(["█ █", "███", "█ █", "█ █", "███"]),
        'Y' => Some(["█ █", "█ █", " █ ", " █ ", " █ "]),
        'D' => Some(["██ ", "█ █", "█ █", "█ █", "██ "]),
        ' ' => Some(["   ", "   ", "   ", "   ", "   "]),
        _ => None,
    }
}

/// Renders text as 5-row block-graphics art (teletext mosaic style).
/// Unsupported characters are skipped.
fn render_block_text(text: &str) -> Vec<String> {
    let glyphs: Vec<[&str; 5]> = text.chars().filter_map(block_glyph).collect();
    (0..5)
        .map(|row| glyphs.iter().map(|g| g[row]).collect::<Vec<_>>().join(" "))
        .collect()
}

/// Creates the teletext-style "page not found" page shown when the user
/// enters a page number that isn't in use (anything except 221/222/223).
pub fn create_page_not_found_page(page_number: u16) -> TeletextPage {
    let mut page = TeletextPage::new(
        page_number,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false,
        true,
        false,
        false,
        false,
    );

    // Block art centered within the classic 40-column teletext area
    page.add_banner_line(" ".to_string());
    for line in render_block_text("SIVUA") {
        page.add_banner_line(format!("{:^40}", line));
    }
    page.add_banner_line(" ".to_string());
    for line in render_block_text("EI LÖYDY") {
        page.add_banner_line(format!("{:^40}", line));
    }
    page.add_banner_line(" ".to_string());
    page.add_error_message(&format!("Sivu {page_number} ei ole käytössä"));
    page.add_error_message("221 Ottelut  222 Taulukko  223 Pudotuspelit");

    page
}

/// Validates if a game is in the future by checking both time and start fields
pub fn is_future_game(game: &GameData) -> bool {
    // Check if time field is non-empty (indicates scheduled game)
    if game.time.is_empty() {
        return false;
    }

    // Check if start field contains a valid future date
    if game.start.is_empty() {
        return false;
    }

    // Parse the start date to validate it's on a future date (not just future time today)
    // Expected format: YYYY-MM-DDThh:mm:ssZ
    match chrono::DateTime::parse_from_rfc3339(&game.start) {
        Ok(game_start) => {
            // Convert to local timezone for date comparison
            let game_local = game_start.with_timezone(&chrono::Local);
            let now_local = chrono::Local::now();

            // Extract just the date parts for comparison
            let game_date = game_local.date_naive();
            let today = now_local.date_naive();

            let is_future = game_date > today;

            if !is_future {
                tracing::debug!(
                    "Game date {} is not in the future (today: {})",
                    game_date,
                    today
                );
            }

            is_future
        }
        Err(e) => {
            tracing::warn!("Failed to parse game start time '{}': {e}", game.start);
            false
        }
    }
}

/// Creates a TeletextPage for standings display
pub fn create_standings_page(
    standings: &[crate::data_fetcher::models::standings::StandingsEntry],
    playoffs_lines: &[u16],
    live_mode: bool,
    disable_links: bool,
    _compact_mode: bool,
    _wide_mode: bool,
) -> TeletextPage {
    let subheader = if live_mode {
        "SARJATAULUKKO (LIVE)".to_string()
    } else {
        "SARJATAULUKKO".to_string()
    };

    // Force normal mode for standings - compact/wide renderers don't support standings rows
    let mut page = TeletextPage::new(
        222,
        "JÄÄKIEKKO".to_string(),
        subheader,
        disable_links,
        true,
        false,
        false,
        false,
    );

    page.set_standings_mode(true, live_mode);
    page.set_playoffs_lines(playoffs_lines);
    page.add_standings_header();

    for (i, entry) in standings.iter().enumerate() {
        page.add_standings_row((i + 1) as u16, entry);
    }

    page
}

/// Creates a teletext page for the playoff bracket.
pub fn create_bracket_page(
    bracket: &PlayoffBracket,
    disable_links: bool,
    terminal_width: u16,
    terminal_height: u16,
) -> TeletextPage {
    let subheader = format!("PUDOTUSPELIT {}", bracket.season);

    // Force normal mode (no compact/wide), same as standings
    let mut page = TeletextPage::new(
        223,
        "JÄÄKIEKKO".to_string(),
        subheader,
        disable_links,
        true,
        false,
        false,
        false,
    );

    page.set_bracket_page(true);

    let rows = render_bracket(bracket, terminal_width, terminal_height);
    for row in rows {
        match row {
            TeletextRow::BracketLine(line) => page.add_bracket_line(line),
            TeletextRow::BracketPageBreak => page.add_bracket_page_break(),
            _ => {}
        }
    }

    page
}

/// Formats a date string for display in Finnish format (DD.MM.)
pub fn format_date_for_display(date_str: &str) -> String {
    // Parse the date using chrono for better error handling
    match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        Ok(date) => date.format("%d.%m.").to_string(),
        Err(_) => date_str.to_string(), // Fallback if parsing fails
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::teletext_ui::ScoreType;

    #[test]
    fn test_format_date_for_display() {
        assert_eq!(format_date_for_display("2024-01-15"), "15.01.");
        assert_eq!(format_date_for_display("2024-12-31"), "31.12.");

        // Test invalid date - should return original string
        assert_eq!(format_date_for_display("invalid-date"), "invalid-date");
    }

    #[test]
    fn test_render_block_text_dimensions() {
        let sivua = render_block_text("SIVUA");
        assert_eq!(sivua.len(), 5);
        // 5 glyphs of width 3, joined with single spaces
        assert!(sivua.iter().all(|row| row.chars().count() == 19));

        let ei_loydy = render_block_text("EI LÖYDY");
        assert_eq!(ei_loydy.len(), 5);
        assert!(ei_loydy.iter().all(|row| row.chars().count() == 31));
    }

    #[test]
    fn test_render_block_text_skips_unsupported_chars() {
        let with_unsupported = render_block_text("S?I");
        let without = render_block_text("SI");
        assert_eq!(with_unsupported, without);
    }

    #[test]
    fn test_create_page_not_found_page() {
        let page = create_page_not_found_page(234);
        let debug = format!("{page:?}");
        // Header shows the entered page number
        assert!(debug.contains("page_number: 234"));
        // Explanation row and block art are present
        assert!(debug.contains("Sivu 234 ei ole käytössä"));
        assert!(debug.contains("█"));
    }

    #[tokio::test]
    async fn test_is_future_game() {
        // Create a future game (different date)
        let future_game = {
            let mut game =
                crate::testing_utils::TestDataBuilder::create_basic_game("Team A", "Team B");
            game.result = "".to_string();
            game.score_type = ScoreType::Scheduled;
            game.played_time = 0;
            game.start = (chrono::Utc::now() + chrono::Duration::days(30))
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string();
            game
        };

        assert!(is_future_game(&future_game));

        // Create a past game
        let past_game = {
            let mut game =
                crate::testing_utils::TestDataBuilder::create_basic_game("Team A", "Team B");
            game.result = "2-1".to_string();
            game.start = "2020-01-15T18:30:00Z".to_string(); // Past date
            game
        };

        assert!(!is_future_game(&past_game));
    }

    #[test]
    fn test_loading_indicator_config() {
        let config = LoadingIndicatorConfig {
            should_show_loading: true,
            current_date: &Some("2024-01-15".to_string()),
            disable_links: false,
            compact_mode: false,
            wide_mode: false,
        };

        assert!(config.should_show_loading);
        assert_eq!(config.current_date, &Some("2024-01-15".to_string()));
    }

    #[tokio::test]
    async fn test_placeholder_games_filtered_from_display() {
        let real_game = crate::testing_utils::TestDataBuilder::create_basic_game("TPS", "HIFK");
        let placeholder =
            crate::testing_utils::TestDataBuilder::create_placeholder_game("QF1", "QF2");
        let games = vec![real_game, placeholder];

        let page = create_base_page(
            &games, true,  // disable_video_links
            false, // show_footer
            true,  // ignore_height_limit
            false, // compact_mode
            false, // wide_mode
            true,  // suppress_countdown
            None,  // future_games_header
            None,  // fetched_date
            None,  // current_page
        )
        .await;

        // Page should contain only the real game, not the placeholder
        assert_eq!(page.game_count(), 1);
    }

    #[tokio::test]
    async fn test_no_phase_headers_for_preseason_games() {
        // The preseason API sets playOffPhase to Some(0) on every game and mixes
        // series (tournament games like PITSITURNAUS alongside PRACTICE), which
        // used to emit a generic "OTTELUT" header at every serie/phase boundary.
        let make = |id: i32, home: &str, away: &str, serie: &str, start: &str| {
            let mut game = crate::testing_utils::TestDataBuilder::create_custom_game(
                id, home, away, "", serie,
            );
            game.play_off_phase = Some(0);
            game.play_off_pair = Some(0);
            game.start = start.to_string();
            game
        };
        let games = vec![
            make(1, "Sport", "TPS", "PITSITURNAUS", "2026-08-07T05:45:00Z"),
            make(2, "Lukko", "Ässät", "PITSITURNAUS", "2026-08-07T07:00:00Z"),
            make(3, "HIFK", "JYP", "PRACTICE", "2026-08-07T12:00:00Z"),
            make(4, "HPK", "Lukko", "PITSITURNAUS", "2026-08-07T12:00:00Z"),
        ];

        let page = create_base_page(
            &games, true, false, true, false, false, true, None, None, None,
        )
        .await;

        assert_eq!(
            page.playoff_phase_headers(),
            Vec::<&str>::new(),
            "non-playoff games must not get phase headers"
        );
        assert_eq!(page.game_count(), 4);
    }

    /// Builds a preseason-shaped game: the API sets playOffPhase on every game
    /// regardless of serie, and names the tournament in `serie`.
    fn preseason_game(id: i32, home: &str, away: &str, serie: &str, start: &str) -> GameData {
        let mut game =
            crate::testing_utils::TestDataBuilder::create_custom_game(id, home, away, "", serie);
        game.play_off_phase = Some(0);
        game.play_off_pair = Some(0);
        game.start = start.to_string();
        game
    }

    #[tokio::test]
    async fn test_preseason_games_grouped_by_serie() {
        // A real preseason day: the PITSITURNAUS tournament runs alongside
        // standalone PRACTICE games, and pure start-time sorting interleaves them.
        let games = vec![
            preseason_game(1, "Sport", "TPS", "PITSITURNAUS", "2026-08-07T05:45:00Z"),
            preseason_game(2, "HIFK", "JYP", "PRACTICE", "2026-08-07T12:00:00Z"),
            preseason_game(3, "HPK", "Lukko", "PITSITURNAUS", "2026-08-07T12:00:00Z"),
            preseason_game(4, "KalPa", "Jukurit", "PRACTICE", "2026-08-07T15:00:00Z"),
            preseason_game(5, "Lukko", "Ässät", "PITSITURNAUS", "2026-08-07T07:00:00Z"),
        ];

        let page = create_base_page(
            &games, true, false, true, false, false, true, None, None, None,
        )
        .await;

        // Groups are ordered by their earliest game; PITSITURNAUS starts first.
        assert_eq!(
            page.series_headers(),
            vec!["PITSITURNAUS", "HARJOITUSOTTELUT"]
        );
        assert_eq!(
            page.game_home_teams(),
            vec!["Sport", "Lukko", "HPK", "HIFK", "KalPa"]
        );
        // Series grouping must not introduce playoff phase headers.
        assert_eq!(page.playoff_phase_headers(), Vec::<&str>::new());
    }

    #[tokio::test]
    async fn test_grouped_page_never_strands_a_header_when_paginated() {
        // End-to-end over the interactive path: build today's real shape
        // (PITSITURNAUS then standalone practice games) with pagination live,
        // and confirm no page ends on a bare series header.
        let mut games: Vec<GameData> = (0..6)
            .map(|index| {
                preseason_game(
                    index,
                    "Sport",
                    "TPS",
                    "PITSITURNAUS",
                    &format!("2026-08-07T{:02}:00:00Z", 5 + index),
                )
            })
            .collect();
        games.extend((0..3).map(|index| {
            preseason_game(
                100 + index,
                "HIFK",
                "JYP",
                "PRACTICE",
                &format!("2026-08-07T{:02}:00:00Z", 12 + index),
            )
        }));

        for screen_height in 9..=30u16 {
            let mut page = create_base_page(
                &games, true, false, false, // ignore_height_limit - pagination must run
                false, false, true, None, None, None,
            )
            .await;
            page.set_screen_height(screen_height);

            let total = page.total_pages();
            for page_index in 0..total {
                page.set_current_page(page_index);
                assert!(
                    !page.last_visible_row_is_header(),
                    "height {screen_height}: page {page_index} of {total} ends with a header"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_no_series_headers_when_all_games_share_a_serie() {
        let games = vec![
            preseason_game(1, "Sport", "TPS", "PITSITURNAUS", "2026-08-07T05:45:00Z"),
            preseason_game(2, "Lukko", "Ässät", "PITSITURNAUS", "2026-08-07T07:00:00Z"),
        ];

        let page = create_base_page(
            &games, true, false, true, false, false, true, None, None, None,
        )
        .await;

        // The subheader already names the serie, so per-group labels would just repeat it.
        assert_eq!(page.series_headers(), Vec::<&str>::new());
        assert_eq!(page.game_count(), 2);
    }

    #[tokio::test]
    async fn test_regular_season_day_gets_no_series_headers() {
        let games = vec![
            crate::testing_utils::TestDataBuilder::create_basic_game("TPS", "HIFK"),
            crate::testing_utils::TestDataBuilder::create_basic_game("Ilves", "Tappara"),
        ];

        let page = create_base_page(
            &games, true, false, true, false, false, true, None, None, None,
        )
        .await;

        assert_eq!(page.series_headers(), Vec::<&str>::new());
    }

    #[tokio::test]
    async fn test_playoff_games_keep_phase_headers_instead_of_series_headers() {
        // Mixing playoffs with another serie must still label playoffs by phase,
        // not duplicate a "PLAYOFFS" series header above it.
        let mut playoff = crate::testing_utils::TestDataBuilder::create_custom_game(
            1, "TPS", "HIFK", "", "playoffs",
        );
        playoff.play_off_phase = Some(2);
        playoff.start = "2026-04-01T16:30:00Z".to_string();

        let mut regular = crate::testing_utils::TestDataBuilder::create_custom_game(
            2,
            "Ilves",
            "Lukko",
            "",
            "runkosarja",
        );
        regular.start = "2026-04-01T16:30:00Z".to_string();

        let page = create_base_page(
            &[playoff, regular],
            true,
            false,
            true,
            false,
            false,
            true,
            None,
            None,
            None,
        )
        .await;

        assert_eq!(page.playoff_phase_headers(), vec!["PUOLIVÄLIERÄT"]);
        assert_eq!(page.series_headers(), vec!["RUNKOSARJA"]);
    }

    #[tokio::test]
    async fn test_phase_headers_still_shown_for_playoffs() {
        let make = |id: i32, home: &str, away: &str, phase: i32| {
            let mut game = crate::testing_utils::TestDataBuilder::create_custom_game(
                id, home, away, "", "playoffs",
            );
            game.play_off_phase = Some(phase);
            game.start = "2026-04-01T16:30:00Z".to_string();
            game
        };
        let games = vec![
            make(1, "TPS", "HIFK", 2),
            make(2, "Kärpät", "Tappara", 2),
            make(3, "Ilves", "Lukko", 3),
        ];

        let page = create_base_page(
            &games, true, false, true, false, false, true, None, None, None,
        )
        .await;

        assert_eq!(
            page.playoff_phase_headers(),
            vec!["PUOLIVÄLIERÄT", "VÄLIERÄT"]
        );
    }

    #[tokio::test]
    async fn test_loading_page_is_marked_and_restored() {
        // create_loading_page should mark the page as a loading page
        let loading = create_loading_page(&Some("2024-01-15".to_string()), false, false, false);
        assert!(loading.is_loading_page());

        // A regular page should NOT be marked as loading
        let games = vec![crate::testing_utils::TestDataBuilder::create_basic_game(
            "HIFK", "Tappara",
        )];
        let regular = create_base_page(
            &games, true, false, true, false, false, true, None, None, None,
        )
        .await;
        assert!(!regular.is_loading_page());
    }

    #[tokio::test]
    async fn test_only_placeholder_games_produces_empty_display() {
        let placeholder1 =
            crate::testing_utils::TestDataBuilder::create_placeholder_game("QF1", "QF2");
        let placeholder2 =
            crate::testing_utils::TestDataBuilder::create_placeholder_game("SF1", "SF2");
        let games = vec![placeholder1, placeholder2];

        // Games vec is non-empty (prevents transient-empty detection)...
        assert!(!games.is_empty());

        let page = create_base_page(
            &games, true,  // disable_video_links
            false, // show_footer
            true,  // ignore_height_limit
            false, // compact_mode
            false, // wide_mode
            true,  // suppress_countdown
            None,  // future_games_header
            None,  // fetched_date
            None,  // current_page
        )
        .await;

        // ...but page renders zero games since all are placeholders
        assert_eq!(page.game_count(), 0);
    }
}
