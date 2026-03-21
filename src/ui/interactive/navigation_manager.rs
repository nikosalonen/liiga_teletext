//! Navigation management for interactive UI
//!
//! This module handles all aspects of page navigation, creation, and management
//! for the interactive UI, including:
//! - Page creation for different game types (regular, future, loading, error)
//! - Page restoration and state management
//! - Game analysis and validation for navigation decisions
//! - Loading indicator coordination

use super::series_utils::{get_subheader, playoff_phase_name};
use crate::data_fetcher::models::bracket::PlayoffBracket;
use crate::data_fetcher::{GameData, is_historical_date};
use crate::teletext_ui::bracket_display::render_bracket;
use crate::teletext_ui::{GameResultData, TeletextPage, TeletextRow};
use chrono::NaiveDate;

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
    sorted_games.sort_by_key(|g| {
        let serie_order = match g.serie.as_str() {
            "playoffs" => 0,
            "playout" => 1,
            "qualifications" => 2,
            _ => 3,
        };
        (
            serie_order,
            g.play_off_phase.unwrap_or(i32::MAX),
            g.start.clone(),
            g.play_off_pair.unwrap_or(i32::MAX),
            g.home_team.clone(),
        )
    });

    let mut last_header: Option<(&str, i32)> = None;
    for game in &sorted_games {
        if let Some(phase) = game.play_off_phase {
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
        223,
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
) -> TeletextPage {
    let subheader = format!("PUDOTUSPELIT {}", bracket.season);

    // Force normal mode (no compact/wide), same as standings
    let mut page = TeletextPage::new(
        224,
        "JÄÄKIEKKO".to_string(),
        subheader,
        disable_links,
        true,
        false,
        false,
        false,
    );

    let rows = render_bracket(bracket, terminal_width);
    for row in rows {
        if let TeletextRow::BracketLine(line) = row {
            page.add_bracket_line(line);
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
