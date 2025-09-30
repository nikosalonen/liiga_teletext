//! Navigation management for interactive UI
//!
//! This module handles all aspects of page navigation, creation, and management
//! for the interactive UI, including:
//! - Page creation for different game types (regular, future, loading, error)
//! - Page restoration and state management
//! - Game analysis and validation for navigation decisions
//! - Loading indicator coordination

use super::series_utils::get_subheader;
use crate::data_fetcher::{GameData, is_historical_date};
use crate::teletext_ui::{GameResultData, ScoreType, TeletextPage};
use chrono::{NaiveDate, Utc};

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
    pub should_show_indicator: bool,
    pub current_date: &'a Option<String>,
    pub disable_links: bool,
    pub compact_mode: bool,
    pub wide_mode: bool,
}

/// Result of a navigation operation
#[derive(Debug)]
pub enum NavigationResult {
    /// Page was created successfully
    PageCreated(TeletextPage),
    /// Page was restored successfully
    PageRestored(TeletextPage),
    /// No page was created (e.g., for future games that don't qualify)
    NoPage,
    /// Loading page was created
    LoadingPage(TeletextPage),
    /// Error page was created
    ErrorPage(TeletextPage),
}

/// Main navigation manager for the interactive UI
pub struct NavigationManager;

impl NavigationManager {
    /// Create a new navigation manager
    pub fn new() -> Self {
        Self
    }

    /// Creates or restores a teletext page based on the current state and data
    pub async fn create_or_restore_page(
        &self,
        config: PageCreationConfig<'_>,
    ) -> Option<TeletextPage> {
        // Restore the preserved page number
        if let Some(preserved_page_for_restoration) = config.preserved_page_for_restoration {
            let mut page = self
                .create_page(
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
                self.create_error_page(
                    config.fetched_date,
                    config.disable_links,
                    config.compact_mode,
                    config.wide_mode,
                )
            } else {
                // Try to create a future games page, fall back to regular page if not future games
                let show_future_header = config.current_date.is_none();
                match self
                    .create_future_games_page(
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
                        let mut page = self
                            .create_page(
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
    pub async fn handle_page_restoration(&self, params: PageRestorationParams<'_>) -> bool {
        let mut needs_render = false;

        // If we showed a loading screen but data didn't change, we still need to restore pagination
        if !params.data_changed
            && !params.had_error
            && params.preserved_page_for_restoration.is_some()
            && let Some(current) = params.current_page
        {
            // Check if current page is a loading page by checking if it has error messages
            if current.has_error_messages()
                && let Some(preserved_page_for_restoration) = params.preserved_page_for_restoration
            {
                let games_to_use = if params.games.is_empty() {
                    params.last_games
                } else {
                    params.games
                };
                let mut page = self
                    .create_page(
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
        &self,
        current_page: &mut Option<TeletextPage>,
        config: LoadingIndicatorConfig<'_>,
    ) -> bool {
        let mut needs_render = false;

        if config.should_show_indicator
            && let Some(page) = current_page
        {
            page.show_auto_refresh_indicator();
            needs_render = true;
        }

        if config.should_show_loading {
            *current_page = Some(self.create_loading_page(
                config.current_date,
                config.disable_links,
                config.compact_mode,
                config.wide_mode,
            ));
            needs_render = true;
        } else {
            tracing::debug!("Skipping loading screen due to ongoing games");
        }

        needs_render
    }

    /// Creates a base TeletextPage with common initialization logic
    #[allow(clippy::too_many_arguments)]
    async fn create_base_page(
        &self,
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

        for game in games {
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
        &self,
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
        self.create_base_page(
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
        &self,
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
        if !games.is_empty() && self.is_future_game(&games[0]) {
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
            let mut page = self
                .create_base_page(
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
        &self,
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
        &self,
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
    pub fn is_future_game(&self, game: &GameData) -> bool {
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
                tracing::warn!("Failed to parse game start time '{}': {}", game.start, e);
                false
            }
        }
    }

    /// Checks if a game is scheduled to start within the next few minutes or has recently started
    #[allow(dead_code)] // Reserved for future use
    pub fn is_game_near_start_time(&self, game: &GameData) -> bool {
        if game.score_type != ScoreType::Scheduled || game.start.is_empty() {
            return false;
        }

        match chrono::DateTime::parse_from_rfc3339(&game.start) {
            Ok(game_start) => {
                let time_diff = Utc::now().signed_duration_since(game_start.with_timezone(&Utc));

                // Extended window: Check if game should start within the next 5 minutes or started within the last 10 minutes
                // This is more aggressive to catch games that should have started but haven't updated their status yet
                let is_near_start = time_diff >= chrono::Duration::minutes(-5)
                    && time_diff <= chrono::Duration::minutes(10);

                if is_near_start {
                    tracing::debug!(
                        "Game near start time: {} vs {} - start: {}, time_diff: {:?}",
                        game.home_team,
                        game.away_team,
                        game_start,
                        time_diff
                    );
                }

                is_near_start
            }
            Err(e) => {
                tracing::warn!("Failed to parse game start time '{}': {}", game.start, e);
                false
            }
        }
    }
}

impl Default for NavigationManager {
    fn default() -> Self {
        Self::new()
    }
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

    #[test]
    fn test_format_date_for_display() {
        assert_eq!(format_date_for_display("2024-01-15"), "15.01.");
        assert_eq!(format_date_for_display("2024-12-31"), "31.12.");

        // Test invalid date - should return original string
        assert_eq!(format_date_for_display("invalid-date"), "invalid-date");
    }

    #[test]
    fn test_navigation_manager_creation() {
        let manager = NavigationManager::new();
        // NavigationManager should be created successfully
        // This is a basic test to ensure the struct can be instantiated
        assert_eq!(std::mem::size_of_val(&manager), 0); // Zero-sized struct
    }

    #[test]
    fn test_navigation_manager_default() {
        let manager = NavigationManager::default();
        // Should be equivalent to NavigationManager::new()
        assert_eq!(std::mem::size_of_val(&manager), 0);
    }

    #[tokio::test]
    async fn test_is_future_game() {
        let manager = NavigationManager::new();

        // Create a future game (different date)
        let future_game = crate::data_fetcher::GameData {
            home_team: "Team A".to_string(),
            away_team: "Team B".to_string(),
            time: "18:30".to_string(),
            result: "".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 0,
            start: "2030-01-15T18:30:00Z".to_string(), // Future date
        };

        assert!(manager.is_future_game(&future_game));

        // Create a past game
        let past_game = crate::data_fetcher::GameData {
            home_team: "Team A".to_string(),
            away_team: "Team B".to_string(),
            time: "18:30".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2020-01-15T18:30:00Z".to_string(), // Past date
        };

        assert!(!manager.is_future_game(&past_game));
    }

    #[test]
    fn test_loading_indicator_config() {
        let config = LoadingIndicatorConfig {
            should_show_loading: true,
            should_show_indicator: false,
            current_date: &Some("2024-01-15".to_string()),
            disable_links: false,
            compact_mode: false,
            wide_mode: false,
        };

        assert!(config.should_show_loading);
        assert!(!config.should_show_indicator);
        assert_eq!(config.current_date, &Some("2024-01-15".to_string()));
    }
}
