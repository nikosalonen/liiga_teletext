// src/teletext_ui/pagination.rs - Pagination logic extracted from core.rs

use super::core::{TeletextPage, TeletextRow};

impl TeletextPage {
    /// Calculates the height requirement for a single game row.
    /// Considers goal events, error messages, and future games headers.
    ///
    /// # Arguments
    /// * `game` - The teletext row to calculate height for
    ///
    /// # Returns
    /// * `u16` - Height in terminal lines required for this row
    pub(super) fn calculate_game_height(&self, game: &TeletextRow) -> u16 {
        match game {
            TeletextRow::GameResult { goal_events, .. } => {
                let base_height = 1; // Game result line
                let home_scorers = goal_events.iter().filter(|e| e.is_home_team).count();
                let away_scorers = goal_events.iter().filter(|e| !e.is_home_team).count();
                let scorer_lines = home_scorers.max(away_scorers);
                let spacer = 1; // Space between games
                base_height + scorer_lines as u16 + spacer
            }
            TeletextRow::ErrorMessage(_) => 2u16, // Error message + spacer
            // Every header renders as a single line. Keeping one attached to
            // the content below it is handled by placement_height, not by
            // inflating the height here.
            TeletextRow::FutureGamesHeader(_)
            | TeletextRow::PlayoffPhaseHeader(_)
            | TeletextRow::SeriesHeader(_) => 1u16,
            TeletextRow::StandingsHeader => {
                if self.standings_use_spacing() {
                    2u16
                } else {
                    1u16
                }
            }
            TeletextRow::StandingsRow { position, .. } => {
                let base = if self.standings_use_spacing() {
                    2u16
                } else {
                    1u16
                };
                // Add extra line for playoff separator (drawn before positions after playoff lines)
                if self
                    .playoffs_lines
                    .iter()
                    .any(|&line| *position == line + 1)
                {
                    base + 1
                } else {
                    base
                }
            }
            TeletextRow::BracketLine(_) => 1u16,
            TeletextRow::BracketPageBreak => 0u16,
        }
    }

    /// Calculates the effective game height considering wide mode.
    /// In wide mode, we can fit two games side by side, effectively halving the height usage.
    ///
    /// # Arguments
    /// * `game` - The teletext row to calculate effective height for
    ///
    /// # Returns
    /// * `u16` - Effective height in terminal lines considering layout mode
    pub(super) fn calculate_effective_game_height(&self, game: &TeletextRow) -> u16 {
        let base_height = self.calculate_game_height(game);
        if self.wide_mode && self.can_fit_two_pages() {
            // In wide mode, we can fit two games in the same vertical space
            // Add spacing between games (1 extra line per game except the last)
            let height_with_spacing = base_height + 1; // Add space between games
            // So each game effectively uses half the height
            height_with_spacing.div_ceil(2) // Round up to ensure we don't underestimate
        } else {
            base_height
        }
    }

    /// Returns true if standings rows should have blank-line spacing between them.
    /// Spacing is used when the terminal is tall enough to fit all standings content
    /// with extra blank lines, making the table easier to read.
    pub(super) fn standings_use_spacing(&self) -> bool {
        if !self.is_standings_page {
            return false;
        }
        let available_height = self.screen_height.saturating_sub(5);
        let standings_rows = self
            .content_rows
            .iter()
            .filter(|r| matches!(r, TeletextRow::StandingsRow { .. }))
            .count() as u16;
        let header_lines = 1u16; // StandingsHeader
        let separator_lines = self.playoffs_lines.len() as u16;
        // With spacing: header + blank + (each row + blank) + separators
        // Last row doesn't strictly need a blank after it, but it's fine
        let total_with_spacing = (header_lines + 1) + (standings_rows * 2) + separator_lines;
        total_with_spacing <= available_height
    }

    /// Returns true for rows that only make sense directly above the content
    /// they introduce, and so must never end a page on their own.
    fn is_section_header(row: &TeletextRow) -> bool {
        matches!(
            row,
            TeletextRow::FutureGamesHeader(_)
                | TeletextRow::PlayoffPhaseHeader(_)
                | TeletextRow::SeriesHeader(_)
        )
    }

    /// Height that must be free for the row at `index` to be placed.
    ///
    /// For ordinary rows this is just their own height. A section header also
    /// claims the rows it introduces, up to and including the first one that
    /// isn't itself a header — a lone "HARJOITUSOTTELUT" at the foot of a page
    /// tells the reader nothing, so it belongs on the next page with its games.
    fn placement_height(&self, index: usize) -> u16 {
        let rows = &self.content_rows;
        let mut total = self.calculate_effective_game_height(&rows[index]);
        if !Self::is_section_header(&rows[index]) {
            return total;
        }

        for row in &rows[index + 1..] {
            // A forced break means nothing can follow the header here anyway.
            if matches!(row, TeletextRow::BracketPageBreak) {
                break;
            }
            total += self.calculate_effective_game_height(row);
            if !Self::is_section_header(row) {
                break;
            }
        }
        total
    }

    /// Splits the content rows into pages that fit the current screen height.
    ///
    /// Single source of truth for both `get_page_content` and `total_pages`;
    /// if the two chunked separately they could disagree and navigation would
    /// run off the end of the real content.
    fn paginate(&self) -> Vec<Vec<&TeletextRow>> {
        let available_height = self.screen_height.saturating_sub(5);

        let mut pages: Vec<Vec<&TeletextRow>> = Vec::new();
        let mut current_page_items: Vec<&TeletextRow> = Vec::new();
        let mut current_height = 0u16;

        for (index, row) in self.content_rows.iter().enumerate() {
            if matches!(row, TeletextRow::BracketPageBreak) {
                if !current_page_items.is_empty() {
                    pages.push(std::mem::take(&mut current_page_items));
                    current_height = 0;
                }
                continue;
            }

            let row_height = self.calculate_effective_game_height(row);
            let needed = self.placement_height(index);

            if current_height + needed <= available_height {
                current_page_items.push(row);
                current_height += row_height;
            } else if !current_page_items.is_empty() {
                pages.push(std::mem::take(&mut current_page_items));
                current_page_items.push(row);
                current_height = row_height;
            } else if row_height <= available_height {
                // Already at the top of a page and the group still doesn't fit.
                // Breaking again would gain nothing, so place the row here.
                current_page_items.push(row);
                current_height = row_height;
            }
            // Otherwise the row alone overflows the page and is skipped.
        }

        if !current_page_items.is_empty() {
            pages.push(current_page_items);
        }

        pages
    }

    /// Calculates and returns the content that should be displayed on the current page.
    /// Handles pagination based on available screen height and content size.
    ///
    /// # Returns
    /// A tuple containing:
    /// * Vec<&TeletextRow> - Content rows that should be displayed on the current page
    /// * bool - Whether there are more pages after the current one
    ///
    /// # Notes
    /// - When ignore_height_limit is true, returns all content in a single page
    /// - Otherwise, calculates how many items fit on each page based on screen height
    /// - Reserves 5 lines for header, subheader, and footer
    /// - Maintains consistent item grouping across pages
    pub(super) fn get_page_content(&self) -> (Vec<&TeletextRow>, bool) {
        if self.ignore_height_limit {
            return (
                self.content_rows
                    .iter()
                    .filter(|r| !matches!(r, TeletextRow::BracketPageBreak))
                    .collect(),
                false,
            );
        }

        let pages = self.paginate();

        if let Some(items) = pages.get(self.current_page) {
            let has_more = self.current_page + 1 < pages.len();
            (items.clone(), has_more)
        } else {
            (Vec::new(), false)
        }
    }

    /// Calculates the total number of pages required to display all content.
    /// Takes into account terminal height limitations and game content size.
    ///
    /// # Returns
    /// * `usize` - Total number of pages needed
    pub fn total_pages(&self) -> usize {
        // An empty page still counts as one page to display.
        self.paginate().len().max(1)
    }

    /// Gets the current page number (0-based index)
    ///
    /// # Returns
    /// * `usize` - Current page index
    pub fn get_current_page(&self) -> usize {
        self.current_page
    }

    /// Sets the current page number (0-based index)
    /// Ensures the page number is within valid bounds
    ///
    /// # Arguments
    /// * `page` - The page number to set (0-based)
    pub fn set_current_page(&mut self, page: usize) {
        let total_pages = self.total_pages();
        if total_pages > 0 {
            self.current_page = page.min(total_pages - 1);
        } else {
            self.current_page = 0;
        }
    }

    /// Moves to the next page of content if available.
    /// Wraps around to the first page when at the end.
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::TeletextPage;
    /// use crossterm::event::KeyCode;
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
    /// let event = KeyCode::Right;
    /// if event == KeyCode::Right {
    ///     page.next_page();
    /// }
    /// ```
    pub fn next_page(&mut self) {
        let total = self.total_pages();
        if total <= 1 {
            return;
        }
        self.current_page = (self.current_page + 1) % total;
    }

    /// Moves to the previous page of content if available.
    /// Wraps around to the last page when at the beginning.
    ///
    /// # Example
    /// ```
    /// use liiga_teletext::TeletextPage;
    /// use crossterm::event::KeyCode;
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
    /// let event = KeyCode::Left;
    /// if event == KeyCode::Left {
    ///     page.previous_page();
    /// }
    /// ```
    pub fn previous_page(&mut self) {
        let total = self.total_pages();
        if total <= 1 {
            return;
        }
        self.current_page = if self.current_page == 0 {
            total - 1
        } else {
            self.current_page - 1
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::GameData;
    use crate::teletext_ui::{GameResultData, ScoreType};

    fn scheduled_game(index: usize) -> GameData {
        GameData {
            home_team: format!("Home {index}"),
            away_team: format!("Away {index}"),
            time: "18.00".to_string(),
            result: "0-0".to_string(),
            score_type: ScoreType::Scheduled,
            is_overtime: false,
            is_shootout: false,
            goal_events: vec![],
            played_time: 0,
            serie: "PITSITURNAUS".to_string(),
            start: "2026-08-07T15:00:00Z".to_string(),
            play_off_phase: None,
            play_off_pair: None,
            play_off_req_wins: None,
            series_score: None,
            is_placeholder: false,
        }
    }

    /// Builds a page with `screen_height` and no height-limit override.
    fn page_with_height(screen_height: u16) -> TeletextPage {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "HARJOITUSOTTELUT".to_string(),
            true,
            false,
            false, // ignore_height_limit - pagination must actually run
            false,
            false,
        );
        page.set_screen_height(screen_height);
        page
    }

    fn header_texts(rows: &[&TeletextRow]) -> Vec<String> {
        rows.iter()
            .filter_map(|row| match row {
                TeletextRow::SeriesHeader(text) | TeletextRow::PlayoffPhaseHeader(text) => {
                    Some(text.clone())
                }
                _ => None,
            })
            .collect()
    }

    fn game_count(rows: &[&TeletextRow]) -> usize {
        rows.iter()
            .filter(|row| matches!(row, TeletextRow::GameResult { .. }))
            .count()
    }

    #[test]
    fn test_series_header_is_not_stranded_at_page_bottom() {
        // available = 10 lines. Four scheduled games fill 8, leaving exactly
        // enough room for the header itself but not for any game beneath it.
        let mut page = page_with_height(15);
        for index in 0..4 {
            page.add_game_result(GameResultData::new(&scheduled_game(index)));
        }
        page.add_series_header("HARJOITUSOTTELUT".to_string());
        for index in 4..6 {
            page.add_game_result(GameResultData::new(&scheduled_game(index)));
        }

        let (first_page, has_more) = page.get_page_content();
        assert!(has_more);
        assert_eq!(
            header_texts(&first_page),
            Vec::<String>::new(),
            "a header with no room for its games must move to the next page"
        );

        page.set_current_page(1);
        let (second_page, _) = page.get_page_content();
        assert_eq!(header_texts(&second_page), vec!["HARJOITUSOTTELUT"]);
        assert!(
            game_count(&second_page) >= 1,
            "the header must be followed by at least one of its games"
        );
    }

    #[test]
    fn test_no_content_is_lost_when_header_moves_pages() {
        let mut page = page_with_height(15);
        for index in 0..4 {
            page.add_game_result(GameResultData::new(&scheduled_game(index)));
        }
        page.add_series_header("HARJOITUSOTTELUT".to_string());
        for index in 4..6 {
            page.add_game_result(GameResultData::new(&scheduled_game(index)));
        }

        let total = page.total_pages();
        let mut seen_games = 0;
        let mut seen_headers = 0;
        for page_index in 0..total {
            page.set_current_page(page_index);
            let (rows, _) = page.get_page_content();
            seen_games += game_count(&rows);
            seen_headers += header_texts(&rows).len();
        }

        assert_eq!(seen_games, 6, "every game must appear on exactly one page");
        assert_eq!(seen_headers, 1);
    }

    #[test]
    fn test_reported_88x29_preseason_page_keeps_header_with_its_games() {
        // The reported case: an 88x29 terminal (24 usable lines) showing the
        // PITSITURNAUS header, its 10 games, then the HARJOITUSOTTELUT header,
        // which landed on line 24 with its first game pushed to page 2.
        let mut page = page_with_height(29);
        page.add_series_header("PITSITURNAUS".to_string());
        for index in 0..10 {
            page.add_game_result(GameResultData::new(&scheduled_game(index)));
        }
        page.add_series_header("HARJOITUSOTTELUT".to_string());
        for index in 10..13 {
            page.add_game_result(GameResultData::new(&scheduled_game(index)));
        }

        let (first_page, has_more) = page.get_page_content();
        assert!(has_more, "content should still span two pages");

        let headers = header_texts(&first_page);
        assert_eq!(headers, vec!["PITSITURNAUS", "HARJOITUSOTTELUT"]);
        assert!(
            !TeletextPage::is_section_header(first_page.last().unwrap()),
            "page 1 must not end on the HARJOITUSOTTELUT header"
        );
        assert_eq!(
            game_count(&first_page),
            11,
            "the practice header should bring its first game onto page 1"
        );
    }

    #[test]
    fn test_no_page_ends_with_a_header_at_any_terminal_height() {
        // Mirrors a real preseason day: two series, ten games between them.
        // Whatever the terminal height, a header must never be the last row on
        // a page, and no game may be dropped or duplicated.
        //
        // Heights below 8 leave less than one header plus one game (1 + 2) of
        // usable room, where no split can avoid stranding the header.
        for screen_height in 9..=40u16 {
            let mut page = page_with_height(screen_height);
            page.add_series_header("PITSITURNAUS".to_string());
            for index in 0..6 {
                page.add_game_result(GameResultData::new(&scheduled_game(index)));
            }
            page.add_series_header("HARJOITUSOTTELUT".to_string());
            for index in 6..10 {
                page.add_game_result(GameResultData::new(&scheduled_game(index)));
            }

            let total = page.total_pages();
            let mut seen_games = 0;
            let mut seen_headers = 0;
            for page_index in 0..total {
                page.set_current_page(page_index);
                let (rows, _) = page.get_page_content();
                seen_games += game_count(&rows);
                seen_headers += header_texts(&rows).len();

                if let Some(last) = rows.last() {
                    assert!(
                        !TeletextPage::is_section_header(last),
                        "height {screen_height}: page {page_index} of {total} ends with a header"
                    );
                }
            }

            assert_eq!(
                seen_games, 10,
                "height {screen_height}: games lost or duplicated across pages"
            );
            assert_eq!(
                seen_headers, 2,
                "height {screen_height}: headers lost or duplicated across pages"
            );
        }
    }

    #[test]
    fn test_total_pages_agrees_with_page_content() {
        // total_pages() and get_page_content() must chunk identically, or
        // navigation runs off the end of the real content.
        let mut page = page_with_height(15);
        for index in 0..4 {
            page.add_game_result(GameResultData::new(&scheduled_game(index)));
        }
        page.add_series_header("HARJOITUSOTTELUT".to_string());
        for index in 4..9 {
            page.add_game_result(GameResultData::new(&scheduled_game(index)));
        }

        let total = page.total_pages();
        for page_index in 0..total {
            page.set_current_page(page_index);
            let (rows, has_more) = page.get_page_content();
            assert!(!rows.is_empty(), "page {page_index} of {total} is empty");
            assert_eq!(
                has_more,
                page_index + 1 < total,
                "has_more disagrees with total_pages on page {page_index}"
            );
        }
    }
}
