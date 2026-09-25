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

    /// Lines a row takes in a wide-mode column, including the blank line that
    /// `render_wide_mode_content` draws after it. Headers and error messages
    /// are one line there, plus that blank line.
    pub(super) fn wide_column_row_height(&self, row: &TeletextRow) -> u16 {
        match row {
            TeletextRow::FutureGamesHeader(_)
            | TeletextRow::PlayoffPhaseHeader(_)
            | TeletextRow::SeriesHeader(_)
            | TeletextRow::ErrorMessage(_) => 2,
            _ => self.calculate_game_height(row),
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
        let mut total = self.calculate_game_height(&rows[index]);
        if !Self::is_section_header(&rows[index]) {
            return total;
        }

        for row in &rows[index + 1..] {
            // A forced break means nothing can follow the header here anyway.
            if matches!(row, TeletextRow::BracketPageBreak) {
                break;
            }
            total += self.calculate_game_height(row);
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
        // Same conditions render_buffered uses to pick wide or compact rendering
        if self.wide_mode && self.can_fit_two_pages() {
            return self.paginate_wide();
        }
        if self.compact_mode {
            return self.paginate_compact();
        }

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

            let row_height = self.calculate_game_height(row);
            let needed = self.placement_height(index);

            if current_height + needed <= available_height {
                current_page_items.push(row);
                current_height += row_height;
            } else if !current_page_items.is_empty() {
                pages.push(std::mem::take(&mut current_page_items));
                current_page_items.push(row);
                current_height = row_height;
            } else {
                // Already at the top of a page and the row (or its header group)
                // still doesn't fit. Breaking again would gain nothing, so place
                // it here. A row taller than the whole page is clipped by the
                // renderer rather than dropped: showing the result and the
                // scorers that fit beats an empty page.
                current_page_items.push(row);
                current_height = row_height;
            }
        }

        if !current_page_items.is_empty() {
            pages.push(current_page_items);
        }

        pages
    }

    /// Splits content rows into pages of two wide-mode columns.
    /// See [`pack_two_columns`] for how rows fill the columns.
    fn paginate_wide(&self) -> Vec<Vec<&TeletextRow>> {
        let available_height = self.screen_height.saturating_sub(5);
        let units: Vec<WideUnit> = self
            .content_rows
            .iter()
            .map(|row| WideUnit {
                height: self.wide_column_row_height(row),
                keep_with_next: Self::is_section_header(row),
                page_break: matches!(row, TeletextRow::BracketPageBreak),
            })
            .collect();

        pack_two_columns(&units, available_height)
            .into_iter()
            .map(|indices| indices.into_iter().map(|i| &self.content_rows[i]).collect())
            .collect()
    }

    /// Splits content rows into pages by the lines compact mode actually draws.
    ///
    /// Compact mode puts several games on one line followed by a blank line,
    /// draws headers on a line of their own, and draws no scorer lines, so the
    /// normal per-game heights would overestimate each page several times over.
    /// Mirrors `group_games_for_compact_display`, which lays out each page's rows.
    fn paginate_compact(&self) -> Vec<Vec<&TeletextRow>> {
        use crate::ui::teletext::compact_display::CompactDisplayConfig;

        let available_height = self.screen_height.saturating_sub(5);
        let games_per_line = CompactDisplayConfig::default()
            .calculate_games_per_line(self.compact_render_width())
            .max(1);

        let mut pages: Vec<Vec<&TeletextRow>> = Vec::new();
        let mut current_page_items: Vec<&TeletextRow> = Vec::new();
        let mut current_height = 0u16;
        let mut games_in_line = 0usize;

        let rows = &self.content_rows;
        for (index, row) in rows.iter().enumerate() {
            if matches!(row, TeletextRow::BracketPageBreak) {
                if !current_page_items.is_empty() {
                    pages.push(std::mem::take(&mut current_page_items));
                    current_height = 0;
                    games_in_line = 0;
                }
                continue;
            }

            let is_game = matches!(row, TeletextRow::GameResult { .. });
            let is_header = Self::is_section_header(row);
            // A new game line costs the line itself plus the blank line after it.
            let height_for = |games_in_line: usize| -> u16 {
                if is_game {
                    if games_in_line == 0 { 2 } else { 0 }
                } else if is_header {
                    1
                } else {
                    // Rows compact mode can't draw (error messages, standings)
                    // keep their normal height so nothing is under-budgeted.
                    self.calculate_game_height(row)
                }
            };
            // Keep a header on the same page as the first game line below it.
            let followed_by_game = rows
                .get(index + 1)
                .is_some_and(|next| matches!(next, TeletextRow::GameResult { .. }));
            let header_group = if is_header && followed_by_game { 2 } else { 0 };

            let mut row_height = height_for(games_in_line);
            if current_height + row_height + header_group > available_height
                && !current_page_items.is_empty()
            {
                pages.push(std::mem::take(&mut current_page_items));
                current_height = 0;
                games_in_line = 0;
                row_height = height_for(0);
            }

            current_page_items.push(row);
            current_height += row_height;
            games_in_line = if is_game {
                (games_in_line + 1) % games_per_line
            } else {
                0
            };
        }

        if !current_page_items.is_empty() {
            pages.push(current_page_items);
        }

        pages
    }

    /// Terminal width compact mode will render at. Matches `render_buffered`:
    /// fixed widths in non-interactive mode, the live terminal width otherwise.
    fn compact_render_width(&self) -> usize {
        if self.ignore_height_limit {
            if self.wide_mode { 136 } else { 80 }
        } else {
            crossterm::terminal::size()
                .map(|(width, _)| width as usize)
                .unwrap_or(80)
        }
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

/// One content row as wide-mode pagination sees it.
#[derive(Debug, Clone, Copy)]
pub(super) struct WideUnit {
    /// Lines the row takes in a column, including the blank line after it.
    pub(super) height: u16,
    /// A section header: keep it in the same column as the row below it.
    pub(super) keep_with_next: bool,
    /// A forced page break (bracket pages); takes no space.
    pub(super) page_break: bool,
}

/// Packs rows into pages of two columns, each `available` lines tall: the left
/// column fills first, then the right, then a new page starts. Returns the row
/// indices on each page, in order.
///
/// A row taller than a whole column still gets a column of its own rather than
/// being dropped.
pub(super) fn pack_two_columns(units: &[WideUnit], available: u16) -> Vec<Vec<usize>> {
    let mut pages = Vec::new();
    let mut page = Vec::new();
    let mut in_right_column = false;
    let mut used = 0u16;

    for (index, unit) in units.iter().enumerate() {
        if unit.page_break {
            if !page.is_empty() {
                pages.push(std::mem::take(&mut page));
            }
            in_right_column = false;
            used = 0;
            continue;
        }

        let next_height = units
            .get(index + 1)
            .filter(|next| unit.keep_with_next && !next.page_break)
            .map_or(0, |next| next.height);
        let needed = unit.height + next_height;
        // The last row in a column doesn't draw its trailing blank line.
        let fits = used + needed <= available + 1;

        if !fits && used > 0 {
            if in_right_column {
                pages.push(std::mem::take(&mut page));
                in_right_column = false;
            } else {
                in_right_column = true;
            }
            used = 0;
        }

        page.push(index);
        used += unit.height;
    }

    if !page.is_empty() {
        pages.push(page);
    }
    pages
}

/// Chooses how many rows of a wide-mode page go into the left column, so the
/// taller column is as short as possible. On a tie the left column takes more.
/// Never splits directly after a header, which would strand it at the bottom
/// of the left column, unless there is no other choice.
pub(super) fn balanced_split_index(heights: &[u16], is_header: &[bool]) -> usize {
    let total: u32 = heights.iter().map(|&h| u32::from(h)).sum();
    let mut best: Option<(u32, usize)> = None;
    let mut left: u32 = 0;

    for split in 0..=heights.len() {
        if split > 0 {
            left += u32::from(heights[split - 1]);
        }
        let strands_header = split > 0 && split < heights.len() && is_header[split - 1];
        if strands_header {
            continue;
        }
        let taller = left.max(total - left);
        if best.is_none_or(|(best_taller, _)| taller <= best_taller) {
            best = Some((taller, split));
        }
    }

    best.map_or(heights.len(), |(_, split)| split)
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

    /// A finished 7-0 game: one result line plus seven scorer lines plus a spacer.
    fn seven_goal_game() -> GameData {
        let mut game = scheduled_game(0);
        game.score_type = ScoreType::Final;
        game.result = "7-0".to_string();
        game.goal_events = (1..=7)
            .map(|goal| {
                crate::testing_utils::TestDataBuilder::create_goal_event(
                    "Scorer",
                    goal * 5,
                    goal,
                    0,
                    true,
                )
            })
            .collect();
        game
    }

    /// Largest 1-based ANSI row that `buffer` positions the cursor on.
    fn max_rendered_row(buffer: &str) -> usize {
        // Cursor moves look like "\x1b[{row};{col}H"; colors ("\x1b[38;5;..m") don't end in H.
        buffer
            .split("\x1b[")
            .skip(1)
            .filter_map(|code| code.split_once('H').map(|(position, _)| position))
            .filter_map(|position| position.split_once(';'))
            .filter_map(|(row, column)| {
                column.parse::<usize>().ok()?;
                row.parse::<usize>().ok()
            })
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn test_game_taller_than_page_is_still_shown() {
        // available = 7 lines, but the game needs 9. It used to be skipped
        // silently, leaving an empty page with no hint that a game existed.
        let mut page = page_with_height(12);
        page.add_game_result(GameResultData::new(&seven_goal_game()));

        let (rows, _) = page.get_page_content();
        assert_eq!(game_count(&rows), 1);
        assert_eq!(page.total_pages(), 1);
    }

    #[test]
    fn test_game_taller_than_page_is_clipped_above_footer() {
        let mut page = page_with_height(12);
        page.add_game_result(GameResultData::new(&seven_goal_game()));

        let (rows, _) = page.get_page_content();
        let mut buffer = String::new();
        let mut current_line = 4;
        page.render_normal_mode_content(&mut buffer, &rows, &mut current_line, 231, 46, 46);

        // Rows 11 and 12 belong to the loading line and the footer.
        assert!(
            max_rendered_row(&buffer) <= 10,
            "content drew on row {}",
            max_rendered_row(&buffer)
        );
    }

    #[test]
    fn test_compact_mode_paginates_by_compact_lines() {
        // Compact mode draws no scorer lines, so seven games with five scorers
        // each take at most 7 x 2 lines (one game per line plus a blank line),
        // well within the 19 available. Normal-mode heights spread them over 4 pages.
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            true,
            false,
            false,
            true, // compact_mode
            false,
        );
        page.set_screen_height(24);
        for index in 0..7 {
            let mut game = scheduled_game(index);
            game.score_type = ScoreType::Final;
            game.result = "5-0".to_string();
            game.goal_events = (1..=5)
                .map(|goal| {
                    crate::testing_utils::TestDataBuilder::create_goal_event(
                        "Scorer",
                        goal * 5,
                        goal,
                        0,
                        true,
                    )
                })
                .collect();
            page.add_game_result(GameResultData::new(&game));
        }

        assert_eq!(page.total_pages(), 1);
        let (rows, has_more) = page.get_page_content();
        assert_eq!(game_count(&rows), 7);
        assert!(!has_more);
    }

    fn unit(height: u16) -> WideUnit {
        WideUnit {
            height,
            keep_with_next: false,
            page_break: false,
        }
    }

    #[test]
    fn test_wide_page_splits_columns_by_height() {
        // Max scorer counts 7, 7, 5, 1, 1. Splitting by count put the first
        // three (24 lines) in the left column.
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            true,
            false,
            true, // ignore_height_limit: wide mode assumes 136 columns
            false,
            true, // wide_mode
        );
        for (index, scorers) in [7, 7, 5, 1, 1].into_iter().enumerate() {
            let mut game = scheduled_game(index);
            game.score_type = ScoreType::Final;
            game.goal_events = (1..=scorers)
                .map(|goal| {
                    crate::testing_utils::TestDataBuilder::create_goal_event(
                        "Scorer",
                        goal * 5,
                        goal,
                        0,
                        true,
                    )
                })
                .collect();
            page.add_game_result(GameResultData::new(&game));
        }

        let (left, right) = page.distribute_games_for_wide_display();
        assert_eq!((left.len(), right.len()), (2, 3));
    }

    #[test]
    fn test_wide_pages_fill_left_then_right_column() {
        // 24-row terminal: 19 lines per column. Heights include the blank
        // line after each game, which the last game in a column doesn't draw.
        let units: Vec<WideUnit> = [9, 9, 7, 3, 3, 9, 9].into_iter().map(unit).collect();

        let pages = pack_two_columns(&units, 19);

        // Left: 9 + 9 (17 lines). Right: 7 + 3 + 3. The last two need a new page.
        assert_eq!(pages, vec![vec![0, 1, 2, 3, 4], vec![5, 6]]);
    }

    #[test]
    fn test_wide_pages_place_oversized_row_alone() {
        let units: Vec<WideUnit> = [30, 3].into_iter().map(unit).collect();
        assert_eq!(pack_two_columns(&units, 19), vec![vec![0, 1]]);
    }

    #[test]
    fn test_wide_column_split_balances_by_height() {
        // Splitting by count (3 left, 2 right) gives a 24-line left column
        // that runs off a 24-row terminal. Balancing by height gives 18 / 13.
        assert_eq!(balanced_split_index(&[9, 9, 7, 3, 3], &[false; 5]), 2);
    }

    #[test]
    fn test_wide_column_split_gives_left_the_extra_equal_row() {
        assert_eq!(balanced_split_index(&[3, 3, 3], &[false; 3]), 2);
    }

    #[test]
    fn test_wide_column_split_keeps_header_with_its_game() {
        // Best split by height is after index 1, which is a header: that
        // would leave it alone at the bottom of the left column.
        let heights = [3, 2, 3, 3];
        let headers = [false, true, false, false];
        assert_ne!(balanced_split_index(&heights, &headers), 2);
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
