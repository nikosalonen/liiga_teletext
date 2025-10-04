// src/teletext_ui.rs - Updated with better display formatting

use crate::config::Config;
use crate::data_fetcher::GoalEventData;
use crate::error::AppError;
use crossterm::{execute, style::Print};
use std::io::{Stdout, Write};
use tracing::debug;

use crate::ui::teletext::colors::*;

// Re-export types for backward compatibility
pub use crate::ui::teletext::compact_display::CompactDisplayConfig;
pub use crate::ui::teletext::game_result::{GameResultData, ScoreType};
pub use crate::ui::teletext::loading_indicator::LoadingIndicator;
pub use crate::ui::teletext::page_config::TeletextPageConfig;

// Import layout management components
use super::layout::ColumnLayoutManager;

pub(super) const AWAY_TEAM_OFFSET: usize = 30; // Position for away team content (updated for wider home column: 26 + 3 + 1)
pub const CONTENT_MARGIN: usize = 2; // Small margin for game content from terminal border

// Import utilities from modules
use super::season_utils::calculate_days_until_regular_season;
use super::utils::get_ansi_code;

#[derive(Debug)]
pub struct TeletextPage {
    page_number: u16,
    title: String,
    subheader: String,
    pub(super) content_rows: Vec<TeletextRow>,
    pub(super) current_page: usize,
    pub(super) screen_height: u16,
    pub(super) disable_video_links: bool,
    pub(super) show_footer: bool,
    pub(super) ignore_height_limit: bool,
    pub(super) auto_refresh_disabled: bool,
    pub(super) season_countdown: Option<String>,
    pub(super) fetched_date: Option<String>, // Date for which data was fetched
    pub(super) loading_indicator: Option<LoadingIndicator>,
    pub(super) auto_refresh_indicator: Option<LoadingIndicator>, // Subtle indicator for auto-refresh
    pub(super) error_warning_active: bool,                       // Show footer warning when true
    pub(super) compact_mode: bool,                               // Enable compact display mode
    pub(super) wide_mode: bool,                                  // Enable wide display mode
    pub(super) layout_manager: ColumnLayoutManager, // Layout management for dynamic column calculations
}

#[derive(Debug)]
pub enum TeletextRow {
    GameResult {
        home_team: String,
        away_team: String,
        time: String,
        result: String,
        score_type: ScoreType,
        is_overtime: bool,
        is_shootout: bool,
        goal_events: Vec<GoalEventData>,
        played_time: i32,
    },
    ErrorMessage(String),
    FutureGamesHeader(String), // For "Seuraavat ottelut {date}" line
}

impl TeletextPage {
    /// Creates a new TeletextPage instance with the specified parameters.
    ///
    /// # Arguments
    /// * `page_number` - The teletext page number (e.g., 221 for sports)
    /// * `title` - The title displayed at the top of the page
    /// * `subheader` - The subtitle displayed below the title
    /// * `disable_video_links` - Whether to disable clickable video links
    /// * `show_footer` - Whether to show the control footer
    /// * `ignore_height_limit` - Whether to ignore terminal height limits
    ///
    /// # Returns
    /// * `TeletextPage` - A new instance configured with the provided parameters
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::TeletextPage;
    ///
    /// let page = TeletextPage::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    ///     false,
    ///     true,
    ///     false,
    ///     false,
    ///     false, // wide_mode
    /// );
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        page_number: u16,
        title: String,
        subheader: String,
        disable_video_links: bool,
        show_footer: bool,
        ignore_height_limit: bool,
        compact_mode: bool,
        wide_mode: bool,
    ) -> Self {
        // Get terminal size, fallback to reasonable default if can't get size
        let (terminal_width, screen_height) = if ignore_height_limit {
            // Use reasonable defaults for non-interactive mode
            let width = if wide_mode {
                136u16 // Wide enough to accommodate wide mode (128+ required)
            } else {
                80u16 // Standard width for normal mode
            };
            (width, 24u16)
        } else {
            crossterm::terminal::size().unwrap_or((80, 24))
        };

        // Initialize ColumnLayoutManager with terminal width and content margin
        let layout_manager = ColumnLayoutManager::new(terminal_width as usize, CONTENT_MARGIN);

        TeletextPage {
            page_number,
            title,
            subheader,
            content_rows: Vec::new(),
            current_page: 0,
            screen_height,
            disable_video_links,
            show_footer,
            ignore_height_limit,
            auto_refresh_disabled: false,
            season_countdown: None,
            fetched_date: None,
            loading_indicator: None,
            auto_refresh_indicator: None,
            error_warning_active: false,
            compact_mode,
            wide_mode,
            layout_manager,
        }
    }

    /// Creates a new TeletextPage from a configuration struct.
    /// This provides a more ergonomic API compared to the many-parameter constructor.
    /// Validates that compact_mode and wide_mode are not both enabled.
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::{TeletextPage, TeletextPageConfig};
    ///
    /// let config = TeletextPageConfig::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    /// );
    /// let page = TeletextPage::from_config(config)?;
    /// # Ok::<(), liiga_teletext::AppError>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if both compact_mode and wide_mode are enabled in the configuration.
    #[allow(dead_code)] // Used in tests
    pub fn from_config(config: TeletextPageConfig) -> Result<Self, AppError> {
        // Validate mode exclusivity before creating the page
        if let Err(msg) = config.validate_mode_exclusivity() {
            return Err(AppError::config_error(format!(
                "Invalid TeletextPageConfig: {msg}"
            )));
        }

        Ok(Self::new(
            config.page_number,
            config.title,
            config.subheader,
            config.disable_video_links,
            config.show_footer,
            config.ignore_height_limit,
            config.compact_mode,
            config.wide_mode,
        ))
    }

    /// Gets a reference to the layout manager for rendering operations.
    /// This allows rendering methods to access layout calculations and positioning.
    ///
    /// # Returns
    /// * `&ColumnLayoutManager` - Reference to the layout manager
    #[allow(dead_code)]
    pub fn layout_manager(&self) -> &ColumnLayoutManager {
        &self.layout_manager
    }

    /// Gets a mutable reference to the layout manager for cache management and optimization.
    /// This allows clearing caches and updating layout calculations.
    ///
    /// # Returns
    /// * `&mut ColumnLayoutManager` - Mutable reference to the layout manager
    #[allow(dead_code)]
    pub fn layout_manager_mut(&mut self) -> &mut ColumnLayoutManager {
        &mut self.layout_manager
    }

    /// Updates the page layout when terminal size changes.
    /// Recalculates content positioning and pagination based on new dimensions.
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::TeletextPage;
    /// use std::io::stdout;
    ///
    /// let mut page = TeletextPage::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    ///     false,
    ///     true,
    ///     false,
    ///     false,
    ///     false, // wide_mode
    /// );
    ///
    /// // Set a fixed screen height for testing to avoid terminal size issues
    /// page.set_screen_height(20);
    ///
    /// // When terminal is resized
    /// page.handle_resize();
    ///
    /// // Only render if we have a proper terminal (skip in CI)
    /// if std::env::var("CI").is_err() {
    ///     let mut stdout = stdout();
    ///     page.render_buffered(&mut stdout)?;
    /// }
    /// # Ok::<(), liiga_teletext::AppError>(())
    /// ```
    #[allow(dead_code)]
    pub fn handle_resize(&mut self) {
        // Update screen height and terminal width
        if let Ok((width, height)) = crossterm::terminal::size() {
            self.screen_height = height;

            // Update layout manager with new terminal width
            self.layout_manager = ColumnLayoutManager::new(width as usize, CONTENT_MARGIN);

            // Recalculate current page to ensure content fits
            let available_height = self.screen_height.saturating_sub(5); // Reserve space for header, subheader, and footer
            let mut current_height = 0u16;
            let mut current_page = 0;

            for game in &self.content_rows {
                let game_height = Self::calculate_game_height(game);

                if current_height + game_height > available_height {
                    current_page += 1;
                    current_height = game_height;
                } else {
                    current_height += game_height;
                }
            }

            // Ensure current_page is within bounds
            self.current_page = self.current_page.min(current_page);
        }
    }

    /// Distributes games between left and right columns for wide mode display.
    /// Uses left-column-first filling logic similar to pagination.
    ///
    /// # Returns
    /// * `(Vec<&TeletextRow>, Vec<&TeletextRow>)` - Left and right column games
    pub fn distribute_games_for_wide_display(&self) -> (Vec<&TeletextRow>, Vec<&TeletextRow>) {
        if !self.wide_mode || !self.can_fit_two_pages() {
            // If not in wide mode or can't fit two columns, return all games in left column
            let all_games: Vec<&TeletextRow> = self.content_rows.iter().collect();
            return (all_games, Vec::new());
        }

        let (visible_rows, _) = self.get_page_content();
        if visible_rows.is_empty() {
            return (Vec::new(), Vec::new());
        }

        // Split games roughly evenly between columns using balanced distribution
        // Left column gets the extra game if there's an odd number
        let total_games = visible_rows.len();
        let games_per_column = total_games.div_ceil(2);

        let mut left_games: Vec<&TeletextRow> = Vec::new();
        let mut right_games: Vec<&TeletextRow> = Vec::new();

        for (i, game) in visible_rows.iter().enumerate() {
            if i < games_per_column {
                left_games.push(game);
            } else {
                right_games.push(game);
            }
        }

        (left_games, right_games)
    }

    /// Renders only the loading indicator area without redrawing the entire screen
    #[allow(dead_code)] // Method for future use
    pub fn render_loading_indicator_only(&self, stdout: &mut Stdout) -> Result<(), AppError> {
        if !self.show_footer {
            return Ok(());
        }

        super::footer::render_loading_indicator_only(
            stdout,
            self.screen_height,
            self.ignore_height_limit,
            &self.loading_indicator,
        )
    }

    /// Sets whether to show the season countdown in the footer.
    /// The countdown will be displayed in the footer area if enabled and regular season hasn't started.
    pub async fn set_show_season_countdown(&mut self, games: &[crate::data_fetcher::GameData]) {
        // Only show countdown if we have games and they're not from the regular season yet
        if games.is_empty() {
            return;
        }

        // Check if any game is from the regular season (runkosarja)
        let has_regular_season_games = games.iter().any(|game| game.serie == "RUNKOSARJA");

        if has_regular_season_games {
            // Regular season has started, don't show countdown
            return;
        }

        // Find the earliest regular season game by fetching future regular season games
        // Only attempt countdown calculation if we can load config without user interaction
        match tokio::task::spawn_blocking(|| std::env::var("LIIGA_API_DOMAIN")).await {
            Ok(Ok(api_domain))
                if !api_domain.is_empty()
                    && api_domain != "placeholder"
                    && api_domain != "test"
                    && api_domain != "unset" =>
            {
                // Config is available via environment variable, safe to calculate countdown
                let client = reqwest::Client::new();
                let config = match Config::load().await {
                    Ok(config) => config,
                    Err(_) => return, // Skip countdown if config loading fails
                };

                if let Some(days_until_season) =
                    calculate_days_until_regular_season(&client, &config, None).await
                {
                    let countdown_text = format!("Runkosarjan alkuun {days_until_season} päivää");
                    self.season_countdown = Some(countdown_text);
                }
            }
            _ => {
                // No valid API domain available, skip countdown to prevent interactive config reads
                debug!("Skipping season countdown due to missing or invalid LIIGA_API_DOMAIN");
            }
        }
    }

    /// Renders the page content to the provided stdout.
    /// Handles all formatting, colors, and layout according to current settings.
    /// Optimized to reduce flickering by minimizing screen clears and using cursor positioning.
    ///
    /// # Arguments
    /// * `stdout` - Mutable reference to stdout for writing
    ///
    /// # Returns
    /// * `Result<(), AppError>` - Ok if rendering succeeded, Err otherwise
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::TeletextPage;
    /// use std::io::stdout;
    ///
    /// let mut page = TeletextPage::new(
    ///     221,
    ///     "JÄÄKIEKKO".to_string(),
    ///     "SM-LIIGA".to_string(),
    ///     false,
    ///     true,
    ///     false,
    ///     false,
    ///     false, // wide_mode
    /// );
    ///
    /// // Set a fixed screen height for testing to avoid terminal size issues
    /// page.set_screen_height(20);
    ///
    /// // When terminal is resized
    /// page.handle_resize();
    ///
    /// // Only render if we have a proper terminal (skip in CI)
    /// if std::env::var("CI").is_err() {
    ///     let mut stdout = stdout();
    ///     page.render_buffered(&mut stdout)?;
    /// }
    /// # Ok::<(), liiga_teletext::AppError>(())
    /// ```
    /// Renders the page content using double buffering for reduced flickering.
    /// This method builds all terminal escape sequences and content in a buffer first,
    /// then writes everything in a single operation.
    pub fn render_buffered(&self, stdout: &mut Stdout) -> Result<(), AppError> {
        // Get terminal dimensions - use appropriate width in non-interactive mode
        let width = if self.ignore_height_limit {
            // Use wider default width for non-interactive mode when wide mode is enabled
            if self.wide_mode {
                136u16 // Wide enough to accommodate wide mode (128+ required)
            } else {
                80u16 // Standard width for normal mode
            }
        } else {
            // Hide cursor to prevent visual artifacts during rendering
            execute!(stdout, crossterm::cursor::Hide)?;

            // Get terminal dimensions
            let (width, _) = crossterm::terminal::size()?;
            width
        };

        // Get content for current page to calculate buffer size
        let (visible_rows, _) = self.get_page_content();

        // Calculate expected buffer size to avoid reallocations
        let expected_size = self.calculate_buffer_size(width, &visible_rows);

        // Build the entire screen content in a string buffer (double buffering)
        let mut buffer = String::with_capacity(expected_size);

        // Only clear the screen in interactive mode using more efficient method
        if !self.ignore_height_limit {
            buffer.push_str("\x1b[H"); // Move to home position
            buffer.push_str("\x1b[0J"); // Clear from cursor down
        }

        // Format the header text with date if available
        let header_text = if let Some(ref date) = self.fetched_date {
            let formatted_date = match chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
                Ok(date) => date.format("%d.%m.%Y").to_string(),
                Err(_) => date.clone(),
            };
            format!("SM-LIIGA {} {}", self.page_number, formatted_date)
        } else {
            format!("SM-LIIGA {}", self.page_number)
        };

        // Use optimized ANSI code generation for headers (requirement 4.3)
        let title_bg_code = get_ansi_code(title_bg(), 46);
        let header_fg_code = get_ansi_code(header_fg(), 21);
        let header_bg_code = get_ansi_code(header_bg(), 21);
        let subheader_fg_code = get_ansi_code(subheader_fg(), 46);

        // Pre-calculate header width for better performance
        let header_width = (width as usize).saturating_sub(20);

        // Batch header ANSI code generation (requirement 4.3)
        let mut header_buffer = String::with_capacity(200); // Pre-allocate for performance

        // Build header line
        header_buffer.push_str(&format!(
            "\x1b[1;1H\x1b[48;5;{}m\x1b[38;5;{}m{:<20}\x1b[48;5;{}m\x1b[38;5;231m{:>width$}\x1b[0m",
            title_bg_code,
            header_fg_code,
            self.title,
            header_bg_code,
            header_text,
            width = header_width
        ));

        // Build subheader with pagination info
        let total_pages = self.total_pages();
        let page_info = if total_pages > 1 && !self.ignore_height_limit {
            format!("{}/{}", self.current_page + 1, total_pages)
        } else {
            String::new()
        };

        // Build subheader line
        header_buffer.push_str(&format!(
            "\x1b[2;1H\x1b[38;5;{}m{:<20}{:>width$}\x1b[0m",
            subheader_fg_code,
            self.subheader,
            page_info,
            width = header_width
        ));

        // Add batched header to main buffer
        buffer.push_str(&header_buffer);

        // Build content starting at line 4 (1-based ANSI positioning)
        let mut current_line: usize = 4;
        let text_fg_code = get_ansi_code(text_fg(), 231);
        let result_fg_code = get_ansi_code(result_fg(), 46);

        // Handle rendering modes

        if self.wide_mode && self.can_fit_two_pages() {
            // Wide mode rendering - two columns
            self.render_wide_mode_content(
                &mut buffer,
                &visible_rows,
                width,
                &mut current_line,
                text_fg_code,
                subheader_fg_code,
            );
        } else if self.compact_mode {
            // Compact mode rendering
            self.render_compact_mode_content(
                &mut buffer,
                &visible_rows,
                width as usize,
                &mut current_line,
                text_fg_code,
            );
        } else {
            // Normal rendering mode
            self.render_normal_mode_content(
                &mut buffer,
                &visible_rows,
                &mut current_line,
                text_fg_code,
                result_fg_code,
                subheader_fg_code,
            );
        }

        // Add footer if enabled
        if self.show_footer {
            let footer_y = super::footer::calculate_footer_position(
                self.ignore_height_limit,
                current_line,
                self.screen_height,
            );

            super::footer::render_footer(
                stdout,
                &mut buffer,
                footer_y,
                width as usize,
                total_pages,
                &self.loading_indicator,
                &self.auto_refresh_indicator,
                self.auto_refresh_disabled,
                self.error_warning_active,
                &self.season_countdown,
            )?;
        }

        // Write entire buffer in one operation (minimizes flicker)
        execute!(stdout, Print(buffer))?;

        // Show cursor again
        execute!(stdout, crossterm::cursor::Show)?;

        stdout.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::GoalEventData;
    use crate::data_fetcher::models::GameData;
    use crate::teletext_ui::formatting::get_team_abbreviation;
    use crate::ui::teletext::{CompactModeValidation, TerminalWidthValidation};
    use crossterm::style::Color;

    #[test]
    fn test_team_abbreviation() {
        // Test current Liiga teams
        assert_eq!(get_team_abbreviation("Tappara"), "TAP");
        assert_eq!(get_team_abbreviation("HIFK"), "IFK");
        assert_eq!(get_team_abbreviation("TPS"), "TPS");
        assert_eq!(get_team_abbreviation("JYP"), "JYP");
        assert_eq!(get_team_abbreviation("Ilves"), "ILV");
        assert_eq!(get_team_abbreviation("KalPa"), "KAL");
        assert_eq!(get_team_abbreviation("Kärpät"), "KÄR");
        assert_eq!(get_team_abbreviation("Lukko"), "LUK");
        assert_eq!(get_team_abbreviation("Pelicans"), "PEL");
        assert_eq!(get_team_abbreviation("SaiPa"), "SAI");
        assert_eq!(get_team_abbreviation("Sport"), "SPO");
        assert_eq!(get_team_abbreviation("HPK"), "HPK");
        assert_eq!(get_team_abbreviation("Jukurit"), "JUK");
        assert_eq!(get_team_abbreviation("Ässät"), "ÄSS");
        assert_eq!(get_team_abbreviation("KooKoo"), "KOO");

        // Test alternative team name formats
        assert_eq!(get_team_abbreviation("HIFK Helsinki"), "IFK");
        assert_eq!(get_team_abbreviation("TPS Turku"), "TPS");
        assert_eq!(get_team_abbreviation("Tampereen Tappara"), "TAP");
        assert_eq!(get_team_abbreviation("Tampereen Ilves"), "ILV");
        assert_eq!(get_team_abbreviation("Jyväskylän JYP"), "JYP");
        assert_eq!(get_team_abbreviation("Kuopion KalPa"), "KUO");
        assert_eq!(get_team_abbreviation("Oulun Kärpät"), "KÄR");
        assert_eq!(get_team_abbreviation("Rauman Lukko"), "LUK");
        assert_eq!(get_team_abbreviation("Lahden Pelicans"), "PEL");
        assert_eq!(get_team_abbreviation("Lappeenrannan SaiPa"), "SAI");
        assert_eq!(get_team_abbreviation("Vaasan Sport"), "SPO");
        assert_eq!(get_team_abbreviation("Hämeenlinnan HPK"), "HPK");
        assert_eq!(get_team_abbreviation("Mikkelin Jukurit"), "JUK");
        assert_eq!(get_team_abbreviation("Porin Ässät"), "ÄSS");
        assert_eq!(get_team_abbreviation("Kouvolan KooKoo"), "KOO");

        // Test fallback for unknown team names (letters only, uppercase)
        assert_eq!(get_team_abbreviation("Unknown Team"), "UNK"); // "UnknownTeam" -> "UNK"
        assert_eq!(get_team_abbreviation("New Team"), "NEW"); // "NewTeam" -> "NEW"
        assert_eq!(get_team_abbreviation("AB"), "AB"); // Short name
        assert_eq!(get_team_abbreviation("A"), "A"); // Very short name
    }

    #[test]
    fn test_compact_display_config() {
        // Test default configuration
        let config = CompactDisplayConfig::default();
        assert_eq!(config.max_games_per_line, 3);
        assert_eq!(config.team_name_width, 8);
        assert_eq!(config.score_width, 6);
        assert_eq!(config.game_separator, "  ");

        // Test custom configuration
        let custom_config = CompactDisplayConfig::new(3, 10, 8, " | ");
        assert_eq!(custom_config.max_games_per_line, 3);
        assert_eq!(custom_config.team_name_width, 10);
        assert_eq!(custom_config.score_width, 8);
        assert_eq!(custom_config.game_separator, " | ");

        // Test terminal width adaptation
        assert_eq!(config.calculate_games_per_line(80), 3);
        assert_eq!(config.calculate_games_per_line(100), 3);
        assert_eq!(config.calculate_games_per_line(0), 1);

        // Test terminal width sufficiency
        assert!(config.is_terminal_width_sufficient(20));
        assert!(config.is_terminal_width_sufficient(18));
        assert!(!config.is_terminal_width_sufficient(17));
    }

    #[test]
    fn test_loading_indicator() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            false,
            false,
        );

        // Test showing loading indicator
        page.show_loading("Etsitään otteluita...".to_string());
        assert!(page.loading_indicator.is_some());

        if let Some(ref indicator) = page.loading_indicator {
            assert_eq!(indicator.message(), "Etsitään otteluita...");
            assert_eq!(indicator.current_frame(), "|"); // First frame
        }

        // Test updating animation
        page.update_loading_animation();
        if let Some(ref indicator) = page.loading_indicator {
            assert_eq!(indicator.current_frame(), "/"); // Second frame
        }

        // Test hiding loading indicator
        page.hide_loading();
        assert!(page.loading_indicator.is_none());
    }

    #[test]
    fn test_page_navigation() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            false,
            false,
        );
        page.set_screen_height(20); // Set fixed screen height for testing

        // Add enough games with goal events to create multiple pages
        for i in 0..10 {
            let goal_events = vec![
                GoalEventData {
                    scorer_player_id: i64::from(i),
                    scorer_name: format!("Scorer {i}"),
                    minute: 10,
                    home_team_score: 1,
                    away_team_score: 0,
                    is_winning_goal: false,
                    goal_types: vec![],
                    is_home_team: true,
                    video_clip_url: None,
                },
                GoalEventData {
                    scorer_player_id: i64::from(i + 100),
                    scorer_name: format!("Scorer {val}", val = i + 100),
                    minute: 20,
                    home_team_score: 1,
                    away_team_score: 1,
                    is_winning_goal: false,
                    goal_types: vec![],
                    is_home_team: false,
                    video_clip_url: None,
                },
            ];

            page.add_game_result(GameResultData::new(&GameData {
                home_team: format!("Home {i}"),
                away_team: format!("Away {i}"),
                time: "18.00".to_string(),
                result: "1-1".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                goal_events,
                played_time: 1200,
                serie: "RUNKOSARJA".to_string(),
                start: "2025-01-01T00:00:00Z".to_string(),
            }));
        }

        let initial_page = page.current_page;
        page.next_page();
        assert!(page.current_page > initial_page, "Should move to next page");

        page.previous_page();
        assert_eq!(
            page.current_page, initial_page,
            "Should return to initial page"
        );
    }

    #[test]
    fn test_page_wrapping() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            false,
            false,
        );
        page.set_screen_height(20); // Set fixed screen height for testing

        // Add enough games with goal events to create multiple pages
        for i in 0..10 {
            let goal_events = vec![
                GoalEventData {
                    scorer_player_id: i64::from(i),
                    scorer_name: format!("Scorer {i}"),
                    minute: 10,
                    home_team_score: 1,
                    away_team_score: 0,
                    is_winning_goal: false,
                    goal_types: vec![],
                    is_home_team: true,
                    video_clip_url: None,
                },
                GoalEventData {
                    scorer_player_id: i64::from(i + 100),
                    scorer_name: format!("Scorer {val}", val = i + 100),
                    minute: 20,
                    home_team_score: 1,
                    away_team_score: 1,
                    is_winning_goal: false,
                    goal_types: vec![],
                    is_home_team: false,
                    video_clip_url: None,
                },
            ];

            page.add_game_result(GameResultData::new(&GameData {
                home_team: format!("Home {i}"),
                away_team: format!("Away {i}"),
                time: "18.00".to_string(),
                result: "1-1".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                goal_events,
                played_time: 1200,
                serie: "RUNKOSARJA".to_string(),
                start: "2025-01-01T00:00:00Z".to_string(),
            }));
        }

        let total_pages = page.total_pages();
        assert!(total_pages > 1, "Should have multiple pages");

        // Test wrapping from last to first page
        page.current_page = total_pages - 1;
        page.next_page();
        assert_eq!(page.current_page, 0, "Should wrap to first page");

        // Test wrapping from first to last page
        page.current_page = 0;
        page.previous_page();
        assert_eq!(
            page.current_page,
            total_pages - 1,
            "Should wrap to last page"
        );
    }

    #[test]
    fn test_game_height_calculation() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            true, // ignore_height_limit = true to show all games
            false,
            false,
        );

        // Test game without goals
        page.add_game_result(GameResultData::new(&GameData {
            home_team: "Home".to_string(),
            away_team: "Away".to_string(),
            time: "18.00".to_string(),
            result: "0-0".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            goal_events: vec![],
            played_time: 0,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        // Test game with goals
        let goals = vec![GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Scorer".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec![],
            is_home_team: true,
            video_clip_url: None,
        }];

        page.add_game_result(GameResultData::new(&GameData {
            home_team: "Home".to_string(),
            away_team: "Away".to_string(),
            time: "18.00".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            goal_events: goals,
            played_time: 600,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        let (content, _) = page.get_page_content();
        assert_eq!(content.len(), 2, "Should show both games");
    }

    #[test]
    fn test_error_message_display() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            true, // ignore_height_limit = true to show all content
            false,
            false,
        );
        let error_msg = "Test Error";
        page.add_error_message(error_msg);

        let (content, _) = page.get_page_content();
        assert_eq!(content.len(), 1, "Should have one row");
        match &content[0] {
            TeletextRow::ErrorMessage(msg) => assert_eq!(msg, error_msg),
            _ => panic!("Should be an error message"),
        }
    }

    #[test]
    fn test_game_result_display() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            true, // ignore_height_limit = true to show all games
            false,
            false,
        );

        // Test scheduled game display
        page.add_game_result(GameResultData::new(&GameData {
            home_team: "Home".to_string(),
            away_team: "Away".to_string(),
            time: "18.00".to_string(),
            result: "0-0".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            goal_events: vec![],
            played_time: 0,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        // Test ongoing game with goals
        let goal_events = vec![
            GoalEventData {
                scorer_player_id: 123,
                scorer_name: "Scorer".to_string(),
                minute: 10,
                home_team_score: 1,
                away_team_score: 0,
                is_winning_goal: false,
                goal_types: vec!["YV".to_string()],
                is_home_team: true,
                video_clip_url: Some("http://example.com".to_string()),
            },
            GoalEventData {
                scorer_player_id: 456,
                scorer_name: "Away Scorer".to_string(),
                minute: 25,
                home_team_score: 1,
                away_team_score: 1,
                is_winning_goal: false,
                goal_types: vec![],
                is_home_team: false,
                video_clip_url: None,
            },
        ];

        page.add_game_result(GameResultData::new(&GameData {
            home_team: "Home".to_string(),
            away_team: "Away".to_string(),
            time: "".to_string(),
            result: "1-1".to_string(),
            score_type: ScoreType::Ongoing,
            is_overtime: false,
            is_shootout: false,
            goal_events: goal_events.clone(),
            played_time: 1500,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        // Test finished game with overtime
        page.add_game_result(GameResultData::new(&GameData {
            home_team: "Home OT".to_string(),
            away_team: "Away OT".to_string(),
            time: "".to_string(),
            result: "3-2".to_string(),
            score_type: ScoreType::Final,
            is_overtime: true,
            is_shootout: false,
            goal_events: vec![],
            played_time: 3900,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        // Test finished game with shootout
        page.add_game_result(GameResultData::new(&GameData {
            home_team: "Home SO".to_string(),
            away_team: "Away SO".to_string(),
            time: "".to_string(),
            result: "4-3".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: true,
            goal_events: vec![],
            played_time: 3600,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        let (content, _) = page.get_page_content();
        assert_eq!(content.len(), 4, "Should show all games");

        // Verify each game type is present
        let mut found_scheduled = false;
        let mut found_ongoing = false;
        let mut found_overtime = false;
        let mut found_shootout = false;

        for row in content {
            if let TeletextRow::GameResult {
                score_type,
                is_overtime,
                is_shootout,
                ..
            } = row
            {
                match score_type {
                    ScoreType::Scheduled => found_scheduled = true,
                    ScoreType::Ongoing => found_ongoing = true,
                    ScoreType::Final => {
                        if *is_overtime {
                            found_overtime = true;
                        } else if *is_shootout {
                            found_shootout = true;
                        }
                    }
                }
            }
        }

        assert!(found_scheduled, "Should contain scheduled game");
        assert!(found_ongoing, "Should contain ongoing game");
        assert!(found_overtime, "Should contain overtime game");
        assert!(found_shootout, "Should contain shootout game");
    }

    #[test]
    fn test_video_link_display() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false, // video links enabled
            true,
            false,
            false,
            false,
        );

        let goal_events = vec![GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Scorer".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec![],
            is_home_team: true,
            video_clip_url: Some("http://example.com".to_string()),
        }];

        page.add_game_result(GameResultData::new(&GameData {
            home_team: "Home".to_string(),
            away_team: "Away".to_string(),
            time: "".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            goal_events: goal_events.clone(),
            played_time: 3600,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        // Create another page with video links disabled
        let mut page_no_video = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            true, // video links disabled
            true,
            false,
            false,
            false,
        );

        page_no_video.add_game_result(GameResultData::new(&GameData {
            home_team: "Home".to_string(),
            away_team: "Away".to_string(),
            time: "".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            goal_events,
            played_time: 3600,
            serie: "RUNKOSARJA".to_string(),
            start: "2025-01-01T00:00:00Z".to_string(),
        }));

        let (content, _) = page.get_page_content();
        let (content_no_video, _) = page_no_video.get_page_content();

        assert_eq!(
            content.len(),
            content_no_video.len(),
            "Should have same number of games"
        );
    }

    #[test]
    fn test_buffer_size_calculation() {
        let page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            false,
            false,
        );

        // Test with empty content
        let empty_rows: Vec<&TeletextRow> = vec![];
        let empty_size = page.calculate_buffer_size(80, &empty_rows);

        // Should have base overhead + terminal width overhead (500 + 80*4 = 820, +25% = 1025)
        assert!(empty_size > 500); // Base overhead
        assert!(empty_size < 1500); // Should be reasonable for empty content

        // Test with game content
        let goal_events = vec![
            GoalEventData {
                scorer_name: "Player 1".to_string(),
                scorer_player_id: 1001,
                minute: 10,
                home_team_score: 1,
                away_team_score: 0,
                is_home_team: true,
                is_winning_goal: false,
                goal_types: vec![],
                video_clip_url: None,
            },
            GoalEventData {
                scorer_name: "Player 2".to_string(),
                scorer_player_id: 1002,
                minute: 25,
                home_team_score: 1,
                away_team_score: 1,
                is_home_team: false,
                is_winning_goal: false,
                goal_types: vec![],
                video_clip_url: None,
            },
        ];

        let game_row = TeletextRow::GameResult {
            home_team: "Team A".to_string(),
            away_team: "Team B".to_string(),
            time: "20:00".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            goal_events,
            played_time: 3600,
        };

        let game_rows = vec![&game_row];
        let game_size = page.calculate_buffer_size(80, &game_rows);

        // Should be larger than empty content
        assert!(game_size > empty_size);

        // Should account for game content + goal events
        // Game: 130 bytes + 2 goals * 70 bytes = 270 bytes content overhead
        assert!(game_size > empty_size + 200);

        // Test with error message
        let error_row = TeletextRow::ErrorMessage("Test error message".to_string());
        let error_rows = vec![&error_row];
        let error_size = page.calculate_buffer_size(80, &error_rows);

        // Should be appropriately sized for error message
        assert!(error_size > empty_size);
        assert!(error_size < game_size); // Smaller than game with goals

        // Test scaling with terminal width
        let wide_size = page.calculate_buffer_size(160, &empty_rows);
        assert!(wide_size > empty_size); // Larger terminal should need more buffer
    }

    #[test]
    fn test_get_ansi_code_helper() {
        // Test with AnsiValue color
        let ansi_color = Color::AnsiValue(42);
        assert_eq!(get_ansi_code(ansi_color, 100), 42);

        // Test with non-AnsiValue color (should use fallback)
        let rgb_color = Color::Rgb { r: 255, g: 0, b: 0 };
        assert_eq!(get_ansi_code(rgb_color, 100), 100);

        // Test with different fallback values
        let reset_color = Color::Reset;
        assert_eq!(get_ansi_code(reset_color, 231), 231);
        assert_eq!(get_ansi_code(reset_color, 46), 46);

        // Test actual teletext colors
        assert_eq!(get_ansi_code(text_fg(), 231), 231); // Should be 231 (white)
        assert_eq!(get_ansi_code(header_bg(), 21), 21); // Should be 21 (blue)
        assert_eq!(get_ansi_code(result_fg(), 46), 46); // Should be 46 (green)
    }

    #[test]
    fn test_page_preservation() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "SM-LIIGA".to_string(),
            false,
            true,
            false,
            false,
            false,
        );

        // Set a small screen height to ensure multiple pages
        page.set_screen_height(10);

        // Add enough content to create multiple pages
        for i in 0..20 {
            page.add_error_message(&format!("Test message {i}"));
        }

        // Navigate to page 2
        page.next_page();
        assert_eq!(page.get_current_page(), 1);

        // Create a new page with the same content and preserved page number
        let mut new_page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "SM-LIIGA".to_string(),
            false,
            true,
            false,
            false,
            false,
        );

        // Set the same small screen height
        new_page.set_screen_height(10);

        // Add the same content
        for i in 0..20 {
            new_page.add_error_message(&format!("Test message {i}"));
        }

        // Set the current page to match the original
        new_page.set_current_page(1);
        assert_eq!(new_page.get_current_page(), 1);

        // Verify both pages show the same content
        let (original_content, _) = page.get_page_content();
        let (new_content, _) = new_page.get_page_content();
        assert_eq!(original_content.len(), new_content.len());
    }

    #[test]
    fn test_compact_mode_getter_setter() {
        let mut page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            false,
            false,
        );

        // Test initial state
        assert!(!page.is_compact_mode());

        // Test setting compact mode to true
        assert!(page.set_compact_mode(true).is_ok());
        assert!(page.is_compact_mode());

        // Test setting compact mode to false
        assert!(page.set_compact_mode(false).is_ok());
        assert!(!page.is_compact_mode());
    }

    #[test]
    fn test_format_compact_game() {
        let page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            true,
            false,
        );

        let config = CompactDisplayConfig::default();

        // Test scheduled game
        let scheduled_game = TeletextRow::GameResult {
            home_team: "Tappara".to_string(),
            away_team: "HIFK".to_string(),
            time: "18:30".to_string(),
            result: "".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            goal_events: vec![],
            played_time: 0,
        };

        let formatted = page.format_compact_game(&scheduled_game, &config);
        assert!(formatted.contains("TAP-IFK"));
        assert!(formatted.contains("18:30"));

        // Test final game
        let final_game = TeletextRow::GameResult {
            home_team: "Tappara".to_string(),
            away_team: "HIFK".to_string(),
            time: "18:30".to_string(),
            result: "3-2".to_string(),
            score_type: ScoreType::Final,
            is_overtime: true,
            is_shootout: false,
            goal_events: vec![],
            played_time: 3900,
        };

        let formatted = page.format_compact_game(&final_game, &config);
        assert!(formatted.contains("TAP-IFK"));
        assert!(formatted.contains("3-2 ja"));
    }

    #[test]
    fn test_group_games_for_compact_display() {
        let page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            true,
            false,
        );

        let config = CompactDisplayConfig::new(2, 10, 8, " | ");

        let games = vec![
            TeletextRow::GameResult {
                home_team: "Tappara".to_string(),
                away_team: "HIFK".to_string(),
                time: "18:30".to_string(),
                result: "3-2".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                goal_events: vec![],
                played_time: 3900,
            },
            TeletextRow::GameResult {
                home_team: "HIFK".to_string(),
                away_team: "Tappara".to_string(),
                time: "19:00".to_string(),
                result: "1-4".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                goal_events: vec![],
                played_time: 3900,
            },
        ];

        let game_refs: Vec<&TeletextRow> = games.iter().collect();
        let lines = page.group_games_for_compact_display(&game_refs, &config, 80);

        assert_eq!(lines.len(), 1); // Should fit on one line with 2 games per line
        assert!(lines[0].contains("TAP-IFK"));
        assert!(lines[0].contains("IFK-TAP"));
        assert!(lines[0].contains(" | ")); // Should contain separator
    }

    #[test]
    fn test_terminal_width_adaptation() {
        let page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            true,
            false,
        );

        // Test wide terminal
        assert!(page.is_terminal_suitable_for_compact(120));
        assert_eq!(page.calculate_compact_games_per_line(120), 3); // Default config allows up to 3

        // Test narrow terminal
        assert!(page.is_terminal_suitable_for_compact(30)); // 30 >= 18 (8+6+4)
        assert_eq!(page.calculate_compact_games_per_line(30), 1);

        // Test very narrow terminal
        assert!(!page.is_terminal_suitable_for_compact(10)); // 10 < 18
        assert_eq!(page.calculate_compact_games_per_line(10), 1);

        // Test with custom config that allows multiple games per line
        let custom_config = CompactDisplayConfig::new(3, 8, 6, "  ");
        assert_eq!(custom_config.calculate_games_per_line(120), 3);
        assert_eq!(custom_config.calculate_games_per_line(30), 1);
    }

    #[test]
    fn test_terminal_width_validation() {
        let config = CompactDisplayConfig::default();

        // Test sufficient width
        let validation = config.validate_terminal_width(80);
        match validation {
            TerminalWidthValidation::Sufficient {
                current_width,
                required_width,
                excess,
            } => {
                assert_eq!(current_width, 80);
                assert_eq!(required_width, 18); // team_name_width + score_width + margins
                assert_eq!(excess, 62);
            }
            _ => panic!("Expected sufficient validation"),
        }

        // Test insufficient width
        let validation = config.validate_terminal_width(10);
        match validation {
            TerminalWidthValidation::Insufficient {
                current_width,
                required_width,
                shortfall,
            } => {
                assert_eq!(current_width, 10);
                assert_eq!(required_width, 18);
                assert_eq!(shortfall, 8);
            }
            _ => panic!("Expected insufficient validation"),
        }
    }

    #[test]
    fn test_compact_mode_compatibility_validation() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            true,
            false,
        );

        // Test compatible page (no warnings)
        let validation = page.validate_compact_mode_compatibility();
        match validation {
            CompactModeValidation::Compatible => {
                // Expected for empty page
            }
            _ => panic!("Expected compatible validation for empty page"),
        }

        // Test page with error messages - now properly handled in compact mode
        page.add_error_message("Test error");
        let validation = page.validate_compact_mode_compatibility();
        match validation {
            CompactModeValidation::Compatible => {
                // Expected - error messages are now properly handled in compact mode
            }
            CompactModeValidation::CompatibleWithWarnings { warnings } => {
                // If there are warnings, they should be about other things, not error messages
                assert!(!warnings.iter().any(|w| w.contains("error messages")));
            }
            CompactModeValidation::Incompatible { .. } => {
                panic!("Unexpected incompatible validation for page with error messages");
            }
        }

        // Reset page for next test
        let mut many_games_page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            true,
            false,
        );

        // Test page with many games (manually create games to avoid testing_utils dependency)
        for i in 0..25 {
            let away_team_index = i + 1;
            let game = GameData {
                home_team: format!("Team{i}"),
                away_team: format!("Team{away_team_index}"),
                time: "18:30".to_string(),
                result: "1-0".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 3600,
                start: "2024-01-15T18:30:00Z".to_string(),
            };
            let game_data = GameResultData::new(&game);
            many_games_page.add_game_result(game_data);
        }

        let validation = many_games_page.validate_compact_mode_compatibility();
        match validation {
            CompactModeValidation::CompatibleWithWarnings { warnings } => {
                assert!(warnings.iter().any(|w| w.contains("Many games detected")));
            }
            _ => panic!("Expected warnings for page with many games"),
        }
    }

    #[test]
    fn test_team_abbreviation_comprehensive() {
        // Test all current Liiga teams
        assert_eq!(get_team_abbreviation("Tappara"), "TAP");
        assert_eq!(get_team_abbreviation("HIFK"), "IFK");
        assert_eq!(get_team_abbreviation("TPS"), "TPS");
        assert_eq!(get_team_abbreviation("JYP"), "JYP");
        assert_eq!(get_team_abbreviation("Ilves"), "ILV");
        assert_eq!(get_team_abbreviation("KalPa"), "KAL");
        assert_eq!(get_team_abbreviation("Kärpät"), "KÄR");
        assert_eq!(get_team_abbreviation("Lukko"), "LUK");
        assert_eq!(get_team_abbreviation("Pelicans"), "PEL");
        assert_eq!(get_team_abbreviation("SaiPa"), "SAI");
        assert_eq!(get_team_abbreviation("Sport"), "SPO");
        assert_eq!(get_team_abbreviation("HPK"), "HPK");
        assert_eq!(get_team_abbreviation("Jukurit"), "JUK");
        assert_eq!(get_team_abbreviation("Ässät"), "ÄSS");
        assert_eq!(get_team_abbreviation("KooKoo"), "KOO");

        // Test full team names
        assert_eq!(get_team_abbreviation("HIFK Helsinki"), "IFK");
        assert_eq!(get_team_abbreviation("TPS Turku"), "TPS");
        assert_eq!(get_team_abbreviation("Tampereen Tappara"), "TAP");
        assert_eq!(get_team_abbreviation("Tampereen Ilves"), "ILV");
        assert_eq!(get_team_abbreviation("Jyväskylän JYP"), "JYP");

        // Test case sensitivity (fallback extracts letters and uppercases)
        assert_eq!(get_team_abbreviation("tappara"), "TAP"); // "tappara" -> "TAP"
        assert_eq!(get_team_abbreviation("TAPPARA"), "TAP"); // "TAPPARA" -> "TAP"
        assert_eq!(get_team_abbreviation("TapPaRa"), "TAP"); // "TapPaRa" -> "TAP"

        // Test fallback for unknown teams (letters only, uppercase)
        assert_eq!(get_team_abbreviation("Unknown Team"), "UNK"); // "UnknownTeam" -> "UNK"
        assert_eq!(get_team_abbreviation("New Team"), "NEW"); // "NewTeam" -> "NEW"
        assert_eq!(get_team_abbreviation("Future Club"), "FUT"); // "FutureClub" -> "FUT"

        // Test special character handling (non-letters removed, uppercase)
        assert_eq!(get_team_abbreviation("HC Blues"), "HCB"); // "HCBlues" -> "HCB"
        assert_eq!(get_team_abbreviation("K-Espoo"), "KES"); // Known team (exact match)
        assert_eq!(get_team_abbreviation("HC-Jokers"), "HCJ"); // "HCJokers" -> "HCJ"
        assert_eq!(get_team_abbreviation("Team #1"), "TEA"); // "Team" -> "TEA"
        assert_eq!(get_team_abbreviation("123 ABC 456"), "ABC"); // "ABC" -> "ABC"
        assert_eq!(get_team_abbreviation("!@#$%"), "!@#$%"); // No letters -> return original

        // Test edge cases
        assert_eq!(get_team_abbreviation(""), ""); // No letters
        assert_eq!(get_team_abbreviation("A"), "A"); // Single letter
        assert_eq!(get_team_abbreviation("AB"), "AB"); // Two letters
        assert_eq!(get_team_abbreviation("ABC"), "ABC"); // Three letters
        assert_eq!(get_team_abbreviation("ABCD"), "ABC"); // Four letters -> truncate to 3
        assert_eq!(get_team_abbreviation("ABCDE"), "ABC"); // Five letters -> truncate to 3
    }

    #[test]
    fn test_compact_display_config_comprehensive() {
        // Test default configuration
        let config = CompactDisplayConfig::default();
        assert_eq!(config.team_name_width, 8);
        assert_eq!(config.score_width, 6);
        assert_eq!(config.max_games_per_line, 3);
        assert_eq!(config.game_separator, "  ");

        // Test custom configuration
        let custom_config = CompactDisplayConfig::new(3, 10, 8, " | ");
        assert_eq!(custom_config.max_games_per_line, 3);
        assert_eq!(custom_config.team_name_width, 10);
        assert_eq!(custom_config.score_width, 8);
        assert_eq!(custom_config.game_separator, " | ");

        // Test terminal width calculations (includes CONTENT_MARGIN * 2 = 4)
        assert_eq!(config.get_minimum_terminal_width(), 18); // 8 + 6 + 4
        assert_eq!(custom_config.get_minimum_terminal_width(), 22); // 10 + 8 + 4

        // Test games per line calculation with different terminal widths
        assert_eq!(config.calculate_games_per_line(80), 3); // Default max is 3, fits easily
        assert_eq!(custom_config.calculate_games_per_line(80), 3); // Can fit 3 games
        assert_eq!(custom_config.calculate_games_per_line(40), 1); // Can fit 1 game (corrected expectation)
        assert_eq!(custom_config.calculate_games_per_line(20), 1); // Can fit 1 game
        assert_eq!(custom_config.calculate_games_per_line(10), 1); // Too narrow but return 1

        // Test terminal width sufficiency
        assert!(config.is_terminal_width_sufficient(80));
        assert!(config.is_terminal_width_sufficient(18)); // Exactly minimum
        assert!(!config.is_terminal_width_sufficient(17)); // Below minimum
        assert!(!config.is_terminal_width_sufficient(0));
    }

    #[test]
    fn test_multi_column_compact_layout() {
        let config = CompactDisplayConfig::default();

        // Test 1 column layout (narrow terminal)
        assert_eq!(config.calculate_games_per_line(20), 1); // 20 chars total, 18 needed minimum

        // Test 2 column layout (medium terminal)
        // Each game: 8 (team) + 6 (score) = 14 chars
        // Two games: 14 + 2 (separator) + 14 = 30 chars content + 4 (margins) = 34 chars total
        assert_eq!(config.calculate_games_per_line(34), 2);
        assert_eq!(config.calculate_games_per_line(40), 2);

        // Test 3 column layout (wide terminal)
        // Three games: 14 + 2 + 14 + 2 + 14 = 46 chars content + 4 (margins) = 50 chars total
        assert_eq!(config.calculate_games_per_line(50), 3);
        assert_eq!(config.calculate_games_per_line(80), 3);
        assert_eq!(config.calculate_games_per_line(120), 3); // Max is 3, even on very wide terminals

        // Test edge cases
        assert_eq!(config.calculate_games_per_line(0), 1); // Always return at least 1
        assert_eq!(config.calculate_games_per_line(18), 1); // Exactly minimum width
        assert_eq!(config.calculate_games_per_line(33), 1); // Just under 2-column threshold
    }

    #[test]
    fn test_compact_mode_spacing() {
        let page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            false,
            false,
            true,
            false,
        );

        let config = CompactDisplayConfig::new(2, 8, 6, "  "); // 2 games per line

        // Create test rows
        let game1 = TeletextRow::GameResult {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "18:30".to_string(),
            result: "3-2".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            goal_events: Vec::new(),
            played_time: 0,
        };

        let game2 = TeletextRow::GameResult {
            home_team: "Blues".to_string(),
            away_team: "Jokerit".to_string(),
            time: "19:00".to_string(),
            result: "1-4".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            goal_events: Vec::new(),
            played_time: 0,
        };

        let game3 = TeletextRow::GameResult {
            home_team: "Lukko".to_string(),
            away_team: "KalPa".to_string(),
            time: "19:30".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: true,
            is_shootout: false,
            goal_events: Vec::new(),
            played_time: 0,
        };

        let rows = vec![&game1, &game2, &game3];
        let result = page.group_games_for_compact_display(&rows, &config, 80);

        // Should have:
        // Line 1: game1 + game2 (2 games per line)
        // Line 2: empty line for spacing
        // Line 3: game3 (remaining game)
        assert_eq!(result.len(), 3);
        assert!(!result[0].is_empty()); // First line with games
        assert!(result[1].is_empty()); // Empty spacing line
        assert!(!result[2].is_empty()); // Second line with remaining game

        // Verify the content contains expected teams (using correct abbreviations)
        assert!(result[0].contains("IFK"));
        assert!(result[0].contains("TAP")); // "Tappara" -> "TAP"
        assert!(result[0].contains("BLU")); // "Blues" -> "BLU" (fallback rule, uppercase)
        assert!(result[2].contains("LUK")); // "Lukko" -> "LUK"
    }

    #[test]
    fn test_compact_formatting_various_game_states() {
        let page = TeletextPage::new(
            221,
            "TEST".to_string(),
            "TEST".to_string(),
            false,
            true,
            false,
            true,
            false,
        );

        let config = CompactDisplayConfig::default();

        // Test scheduled game
        let scheduled_game = TeletextRow::GameResult {
            home_team: "Tappara".to_string(),
            away_team: "HIFK".to_string(),
            time: "18:30".to_string(),
            result: "".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            goal_events: vec![],
            played_time: 0,
        };

        let formatted = page.format_compact_game(&scheduled_game, &config);
        assert!(formatted.contains("TAP-IFK"));
        assert!(formatted.contains("18:30"));

        // Test ongoing game
        let ongoing_game = TeletextRow::GameResult {
            home_team: "Tappara".to_string(),
            away_team: "HIFK".to_string(),
            time: "18:30".to_string(),
            result: "1-0".to_string(),
            score_type: ScoreType::Ongoing,
            is_overtime: false,
            is_shootout: false,
            goal_events: vec![],
            played_time: 2400, // 40 minutes
        };

        let formatted = page.format_compact_game(&ongoing_game, &config);
        assert!(formatted.contains("TAP-IFK"));
        assert!(formatted.contains("1-0"));

        // Test final game with overtime
        let overtime_game = TeletextRow::GameResult {
            home_team: "Tappara".to_string(),
            away_team: "HIFK".to_string(),
            time: "18:30".to_string(),
            result: "3-2".to_string(),
            score_type: ScoreType::Final,
            is_overtime: true,
            is_shootout: false,
            goal_events: vec![],
            played_time: 3900,
        };

        let formatted = page.format_compact_game(&overtime_game, &config);
        assert!(formatted.contains("TAP-IFK"));
        assert!(formatted.contains("3-2 ja"));

        // Test final game with shootout
        let shootout_game = TeletextRow::GameResult {
            home_team: "Tappara".to_string(),
            away_team: "HIFK".to_string(),
            time: "18:30".to_string(),
            result: "4-3".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: true,
            goal_events: vec![],
            played_time: 3900,
        };

        let formatted = page.format_compact_game(&shootout_game, &config);
        assert!(formatted.contains("TAP-IFK"));
        assert!(formatted.contains("4-3 rl"));

        // Test non-game row (should return empty string)
        let error_row = TeletextRow::ErrorMessage("Error".to_string());

        let formatted = page.format_compact_game(&error_row, &config);
        assert_eq!(formatted, "");
    }

    #[test]
    fn test_header_truncation_logic() {
        let page = TeletextPage::new(
            1,
            "Test Title".to_string(),
            "Test Subheader".to_string(),
            false,
            false,
            false,
            true,
            false,
        );
        let config = CompactDisplayConfig::default();

        // Test short header - should not be truncated
        let short_header = TeletextRow::FutureGamesHeader("Short Header".to_string());
        let formatted = page.format_compact_game(&short_header, &config);
        assert!(formatted.contains("Short Header"));
        assert!(!formatted.contains("..."));

        // Test "Seuraavat ottelut" header - should be abbreviated to preserve date
        let future_games_header =
            TeletextRow::FutureGamesHeader("Seuraavat ottelut 07.08.".to_string());
        let formatted = page.format_compact_game(&future_games_header, &config);
        assert!(formatted.contains("Seur. ottelut 07.08."));
        assert!(!formatted.contains("Seuraavat ottelut"));
        assert!(!formatted.contains("..."));

        // Test header under 30 characters - should not be truncated
        let medium_header = TeletextRow::FutureGamesHeader("Medium length header text".to_string());
        let formatted = page.format_compact_game(&medium_header, &config);
        assert!(formatted.contains("Medium length header text"));
        assert!(!formatted.contains("..."));

        // Test header exactly 30 characters - should not be truncated
        let exact_header =
            TeletextRow::FutureGamesHeader("123456789012345678901234567890".to_string());
        let formatted = page.format_compact_game(&exact_header, &config);
        assert!(formatted.contains("123456789012345678901234567890"));
        assert!(!formatted.contains("..."));

        // Test header over 30 characters - should be truncated
        let very_long_header = TeletextRow::FutureGamesHeader(
            "This is a very long header that should be truncated at thirty chars".to_string(),
        );
        let formatted = page.format_compact_game(&very_long_header, &config);
        assert!(formatted.contains("This is a very long header tha..."));
        assert!(
            !formatted
                .contains("This is a very long header that should be truncated at thirty chars")
        );

        // Verify the truncated part length is 33 characters (30 + 3 for "...")
        let truncated_part = "This is a very long header tha...";
        assert_eq!(truncated_part.len(), 33);
    }

    // PHASE 4: WIDE MODE UNIT TESTS

    #[test]
    fn test_wide_mode_getter_setter() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // show_videos
            true,  // ignore_height_limit
            false, // compact_mode
            false, // wide_mode
            false, // enable_colors
        );

        // Test initial state
        assert!(!page.is_wide_mode());

        // Test setter
        assert!(page.set_wide_mode(true).is_ok());
        assert!(page.is_wide_mode());

        // Test setter again
        assert!(page.set_wide_mode(false).is_ok());
        assert!(!page.is_wide_mode());
    }

    #[test]
    fn test_can_fit_two_pages_false_when_wide_mode_disabled() {
        let page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // show_videos
            true,  // ignore_height_limit (non-interactive mode)
            false, // compact_mode
            false, // wide_mode - DISABLED
            false, // enable_colors
        );

        // Should return false when wide_mode is disabled, regardless of terminal width
        assert!(!page.can_fit_two_pages());
    }

    #[test]
    fn test_can_fit_two_pages_with_sufficient_width() {
        let page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // disable_video_links
            true,  // show_footer
            true,  // ignore_height_limit (non-interactive mode - uses 136 width)
            false, // compact_mode
            true,  // wide_mode - ENABLED
        );

        // Should return true when wide_mode is enabled and in non-interactive mode (136 >= 128)
        assert!(page.can_fit_two_pages());
    }

    #[test]
    fn test_can_fit_two_pages_with_insufficient_width() {
        // This test simulates a narrow terminal by using interactive mode
        // In interactive mode, crossterm::terminal::size() would be called,
        // but it will fallback to 80 chars if it can't get the size
        let page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // show_videos
            false, // ignore_height_limit (interactive mode - uses crossterm or fallback to 80)
            false, // compact_mode
            true,  // wide_mode - ENABLED
            false, // enable_colors
        );

        // Should return false when width is insufficient (80 < 128)
        // Note: This test relies on the fallback width of 80 when crossterm can't get terminal size
        assert!(!page.can_fit_two_pages());
    }

    #[test]
    fn test_distribute_games_for_wide_display_disabled() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // show_videos
            true,  // ignore_height_limit
            false, // compact_mode
            false, // wide_mode - DISABLED
            false, // enable_colors
        );

        // Add some test games using GameData -> GameResultData
        let test_game = GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "18:30".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T18:30:00Z".to_string(),
        };
        let test_game_data = GameResultData::new(&test_game);
        page.add_game_result(test_game_data);

        let (left_games, right_games) = page.distribute_games_for_wide_display();

        // When wide mode is disabled, all games should go to left column
        assert_eq!(left_games.len(), 1);
        assert_eq!(right_games.len(), 0);
    }

    #[test]
    fn test_distribute_games_for_wide_display_insufficient_width() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // show_videos
            false, // ignore_height_limit (interactive mode - narrow terminal)
            false, // compact_mode
            true,  // wide_mode - ENABLED but insufficient width
            false, // enable_colors
        );

        // Add some test games
        let test_game = GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "18:30".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T18:30:00Z".to_string(),
        };
        let test_game_data = GameResultData::new(&test_game);
        page.add_game_result(test_game_data);

        let (left_games, right_games) = page.distribute_games_for_wide_display();

        // When terminal width is insufficient, all games should go to left column
        assert_eq!(left_games.len(), 1);
        assert_eq!(right_games.len(), 0);
    }

    #[test]
    fn test_distribute_games_for_wide_display_enabled() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // disable_video_links
            true,  // show_footer
            true,  // ignore_height_limit (non-interactive mode - wide terminal)
            false, // compact_mode
            true,  // wide_mode - ENABLED
        );

        // Add multiple test games to test distribution
        for i in 0..4 {
            let test_game = GameData {
                home_team: format!("Team{i}A"),
                away_team: format!("Team{i}B"),
                time: "18:30".to_string(),
                result: "2-1".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 3600,
                start: "2024-01-15T18:30:00Z".to_string(),
            };
            let test_game_data = GameResultData::new(&test_game);
            page.add_game_result(test_game_data);
        }

        let (left_games, right_games) = page.distribute_games_for_wide_display();

        // With 4 games, balanced distribution should put 2 in left, 2 in right
        assert_eq!(left_games.len(), 2, "Left column should have 2 games");
        assert_eq!(right_games.len(), 2, "Right column should have 2 games");
        assert_eq!(
            left_games.len() + right_games.len(),
            4,
            "Total games should equal 4"
        );
    }

    #[test]
    fn test_distribute_games_for_wide_display_odd_number() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // disable_video_links
            true,  // show_footer
            true,  // ignore_height_limit (non-interactive mode - wide terminal)
            false, // compact_mode
            true,  // wide_mode - ENABLED
        );

        // Add 3 test games (odd number)
        for i in 0..3 {
            let test_game = GameData {
                home_team: format!("Team{i}A"),
                away_team: format!("Team{i}B"),
                time: "18:30".to_string(),
                result: "2-1".to_string(),
                score_type: ScoreType::Final,
                is_overtime: false,
                is_shootout: false,
                serie: "runkosarja".to_string(),
                goal_events: vec![],
                played_time: 3600,
                start: "2024-01-15T18:30:00Z".to_string(),
            };
            let test_game_data = GameResultData::new(&test_game);
            page.add_game_result(test_game_data);
        }

        let (left_games, right_games) = page.distribute_games_for_wide_display();

        // With 3 games, balanced distribution should put 2 in left, 1 in right
        // (left column gets the extra game if odd number)
        assert_eq!(left_games.len(), 2, "Left column should have 2 games");
        assert_eq!(right_games.len(), 1, "Right column should have 1 game");
        assert_eq!(
            left_games.len() + right_games.len(),
            3,
            "Total games should equal 3"
        );
    }

    #[test]
    fn test_wide_mode_with_test_games() {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false, // show_videos
            true,  // ignore_height_limit (wide terminal)
            false, // compact_mode
            true,  // wide_mode
            false, // enable_colors
        );

        // Add test games
        let test_game1 = GameData {
            home_team: "HIFK".to_string(),
            away_team: "Tappara".to_string(),
            time: "18:30".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T18:30:00Z".to_string(),
        };

        let test_game2 = GameData {
            home_team: "TPS".to_string(),
            away_team: "KalPa".to_string(),
            time: "19:00".to_string(),
            result: "1-3".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T19:00:00Z".to_string(),
        };

        page.add_game_result(GameResultData::new(&test_game1));
        page.add_game_result(GameResultData::new(&test_game2));

        // Test that games are properly distributed
        let (left_games, right_games) = page.distribute_games_for_wide_display();

        // Should have games distributed (exact distribution depends on content size)
        assert_eq!(
            left_games.len() + right_games.len(),
            2,
            "Should have both games distributed"
        );
        assert!(
            !left_games.is_empty(),
            "Should have at least one game in left column"
        );
    }

    #[test]
    fn test_teletext_page_config_mode_exclusivity() {
        // Test that new config has both modes disabled by default
        let config = TeletextPageConfig::new(221, "Test".to_string(), "Test".to_string());
        assert!(!config.compact_mode);
        assert!(!config.wide_mode);
        assert!(config.validate_mode_exclusivity().is_ok());

        // Test setter methods enforce mutual exclusivity
        let mut config = TeletextPageConfig::new(221, "Test".to_string(), "Test".to_string());

        // Enable compact mode
        config.set_compact_mode(true);
        assert!(config.compact_mode);
        assert!(!config.wide_mode);
        assert!(config.validate_mode_exclusivity().is_ok());

        // Enable wide mode - should disable compact mode
        config.set_wide_mode(true);
        assert!(!config.compact_mode);
        assert!(config.wide_mode);
        assert!(config.validate_mode_exclusivity().is_ok());

        // Enable compact mode again - should disable wide mode
        config.set_compact_mode(true);
        assert!(config.compact_mode);
        assert!(!config.wide_mode);
        assert!(config.validate_mode_exclusivity().is_ok());
    }

    #[test]
    fn test_teletext_page_config_validation() {
        // Test valid configurations
        let mut config = TeletextPageConfig::new(221, "Test".to_string(), "Test".to_string());
        assert!(config.validate_mode_exclusivity().is_ok());

        config.set_compact_mode(true);
        assert!(config.validate_mode_exclusivity().is_ok());

        config.set_wide_mode(true);
        assert!(config.validate_mode_exclusivity().is_ok());

        // Test invalid configuration (both modes enabled)
        let mut config = TeletextPageConfig::new(221, "Test".to_string(), "Test".to_string());
        config.compact_mode = true;
        config.wide_mode = true;
        assert!(config.validate_mode_exclusivity().is_err());
        assert_eq!(
            config.validate_mode_exclusivity().unwrap_err(),
            "compact_mode and wide_mode cannot be enabled simultaneously"
        );
    }

    #[test]
    fn test_teletext_page_mode_exclusivity() {
        let mut page = TeletextPage::new(
            221,
            "Test".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            false, // compact_mode
            false, // wide_mode
        );

        // Test initial state
        assert!(!page.is_compact_mode());
        assert!(!page.is_wide_mode());
        assert!(page.validate_mode_exclusivity().is_ok());

        // Test setter methods enforce mutual exclusivity
        assert!(page.set_compact_mode(true).is_ok());
        assert!(page.is_compact_mode());
        assert!(!page.is_wide_mode());
        assert!(page.validate_mode_exclusivity().is_ok());

        // Enable wide mode - should disable compact mode
        assert!(page.set_wide_mode(true).is_ok());
        assert!(!page.is_compact_mode());
        assert!(page.is_wide_mode());
        assert!(page.validate_mode_exclusivity().is_ok());

        // Enable compact mode again - should disable wide mode
        assert!(page.set_compact_mode(true).is_ok());
        assert!(page.is_compact_mode());
        assert!(!page.is_wide_mode());
        assert!(page.validate_mode_exclusivity().is_ok());
    }

    #[test]
    fn test_teletext_page_validation() {
        let mut page = TeletextPage::new(
            221,
            "Test".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            false,
            false,
        );

        // Test valid configurations
        assert!(page.validate_mode_exclusivity().is_ok());

        assert!(page.set_compact_mode(true).is_ok());
        assert!(page.validate_mode_exclusivity().is_ok());

        assert!(page.set_wide_mode(true).is_ok());
        assert!(page.validate_mode_exclusivity().is_ok());

        // Test invalid configuration (both modes enabled)
        let page = TeletextPage::new(
            221,
            "Test".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            true, // compact_mode
            true, // wide_mode
        );
        assert!(page.validate_mode_exclusivity().is_err());
        assert_eq!(
            page.validate_mode_exclusivity().unwrap_err(),
            "compact_mode and wide_mode cannot be enabled simultaneously"
        );
    }

    #[test]
    fn test_from_config_returns_error_with_invalid_config() {
        let mut config = TeletextPageConfig::new(221, "Test".to_string(), "Test".to_string());
        config.compact_mode = true;
        config.wide_mode = true;

        // This should return an error
        let result = TeletextPage::from_config(config);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(matches!(error, AppError::Config(_)));
        assert!(
            error
                .to_string()
                .contains("compact_mode and wide_mode cannot be enabled simultaneously")
        );
    }

    #[test]
    fn test_from_config_succeeds_with_valid_config() {
        let config = TeletextPageConfig::new(221, "Test".to_string(), "Test".to_string());

        // This should succeed
        let result = TeletextPage::from_config(config);
        assert!(result.is_ok());

        let page = result.unwrap();
        assert_eq!(page.page_number, 221);
        assert_eq!(page.title, "Test");
        assert_eq!(page.subheader, "Test");
        assert!(!page.is_compact_mode());
        assert!(!page.is_wide_mode());
    }

    #[test]
    fn test_setter_validation_conflicts() {
        let mut page = TeletextPage::new(
            221,
            "Test".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            false, // compact_mode
            false, // wide_mode
        );

        // Test that enabling compact mode when wide mode is active automatically disables wide mode
        page.set_wide_mode(true).unwrap();
        let result = page.set_compact_mode(true);
        assert!(result.is_ok());
        // Verify that wide mode was automatically disabled
        assert!(!page.is_wide_mode());
        assert!(page.is_compact_mode());

        // Test that enabling wide mode when compact mode is active automatically disables compact mode
        let mut page = TeletextPage::new(
            221,
            "Test".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            false, // compact_mode
            false, // wide_mode
        );
        page.set_compact_mode(true).unwrap();
        let result = page.set_wide_mode(true);
        assert!(result.is_ok());
        // Verify that compact mode was automatically disabled
        assert!(!page.is_compact_mode());
        assert!(page.is_wide_mode());

        // Test that disabling modes doesn't cause conflicts
        let mut page = TeletextPage::new(
            221,
            "Test".to_string(),
            "Test".to_string(),
            false,
            true,
            false,
            true,  // compact_mode
            false, // wide_mode
        );
        assert!(page.set_compact_mode(false).is_ok());
        assert!(page.set_wide_mode(false).is_ok());
    }
}
#[test]
fn test_video_link_functionality_with_dynamic_layout() {
    use crate::data_fetcher::models::GameData;

    // Test video link positioning and behavior with the new dynamic layout system
    let mut page = TeletextPage::new(
        221,
        "TEST".to_string(),
        "TEST".to_string(),
        false, // video links enabled
        true,
        false,
        false,
        false,
    );

    // Create goal events with mixed video link scenarios
    let goal_events = vec![
        GoalEventData {
            scorer_player_id: 123,
            scorer_name: "Komarov".to_string(),
            minute: 5,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec!["YV".to_string()],
            is_home_team: true,
            video_clip_url: Some("http://example.com/goal1".to_string()),
        },
        GoalEventData {
            scorer_player_id: 456,
            scorer_name: "VeryLongPlayerNameHere".to_string(),
            minute: 12,
            home_team_score: 1,
            away_team_score: 1,
            is_winning_goal: false,
            goal_types: vec!["IM".to_string(), "TM".to_string()],
            is_home_team: false,
            video_clip_url: None, // No video link
        },
        GoalEventData {
            scorer_player_id: 789,
            scorer_name: "Pesonen".to_string(),
            minute: 18,
            home_team_score: 2,
            away_team_score: 1,
            is_winning_goal: true,
            goal_types: vec!["VL".to_string()],
            is_home_team: true,
            video_clip_url: Some("http://example.com/goal2".to_string()),
        },
    ];

    page.add_game_result(GameResultData::new(&GameData {
        home_team: "HIFK".to_string(),
        away_team: "TPS".to_string(),
        time: "".to_string(),
        result: "2-1".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        goal_events: goal_events.clone(),
        played_time: 3600,
        serie: "RUNKOSARJA".to_string(),
        start: "2025-01-01T00:00:00Z".to_string(),
    }));

    // Test with video links disabled
    let mut page_no_video = TeletextPage::new(
        221,
        "TEST".to_string(),
        "TEST".to_string(),
        true, // video links disabled
        true,
        false,
        false,
        false,
    );

    page_no_video.add_game_result(GameResultData::new(&GameData {
        home_team: "HIFK".to_string(),
        away_team: "TPS".to_string(),
        time: "".to_string(),
        result: "2-1".to_string(),
        score_type: ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        goal_events,
        played_time: 3600,
        serie: "RUNKOSARJA".to_string(),
        start: "2025-01-01T00:00:00Z".to_string(),
    }));

    let (content, _) = page.get_page_content();
    let (content_no_video, _) = page_no_video.get_page_content();

    // Basic verification that both pages have the same structure
    assert_eq!(
        content.len(),
        content_no_video.len(),
        "Should have same number of content rows"
    );

    // Verify that video link functionality is preserved
    // The actual video link rendering is tested in the layout system
    // This test ensures the integration works correctly
    assert!(
        !content.is_empty(),
        "Should have content with video links enabled"
    );
    assert!(
        !content_no_video.is_empty(),
        "Should have content with video links disabled"
    );
}
