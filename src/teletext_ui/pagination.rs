// src/teletext_ui/pagination.rs - Pagination logic extracted from core.rs

use super::core::{TeletextRow, TeletextPage};

impl TeletextPage {
    /// Calculates the height requirement for a single game row.
    /// Considers goal events, error messages, and future games headers.
    ///
    /// # Arguments
    /// * `game` - The teletext row to calculate height for
    ///
    /// # Returns
    /// * `u16` - Height in terminal lines required for this row
    pub(super) fn calculate_game_height(game: &TeletextRow) -> u16 {
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
            TeletextRow::FutureGamesHeader(_) => 1u16, // Single line for future games header
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
        let base_height = Self::calculate_game_height(game);
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
            // When ignoring height limit, return all content in one page
            return (self.content_rows.iter().collect(), false);
        }

        let available_height = self.screen_height.saturating_sub(5); // Reserve space for header, subheader, and footer
        let mut current_height = 0u16;
        let mut page_content = Vec::new();
        let mut has_more = false;
        let mut items_per_page = Vec::new();
        let mut current_page_items = Vec::new();

        // First, calculate how many items fit on each page
        for game in self.content_rows.iter() {
            let game_height = self.calculate_effective_game_height(game);

            if current_height + game_height <= available_height {
                current_page_items.push(game);
                current_height += game_height;
            } else if !current_page_items.is_empty() {
                items_per_page.push(current_page_items.len());
                current_page_items = vec![game];
                current_height = game_height;
            }
        }
        if !current_page_items.is_empty() {
            items_per_page.push(current_page_items.len());
        }

        // Calculate the starting index for the current page
        let mut start_idx = 0;
        for (page_idx, &items) in items_per_page.iter().enumerate() {
            if page_idx == self.current_page {
                break;
            }
            start_idx += items;
        }

        // Get the items for the current page
        if let Some(&items_in_current_page) = items_per_page.get(self.current_page) {
            let end_idx = (start_idx + items_in_current_page).min(self.content_rows.len());
            page_content = self.content_rows[start_idx..end_idx].iter().collect();
            has_more = end_idx < self.content_rows.len();
        }

        (page_content, has_more)
    }

    /// Calculates the total number of pages required to display all content.
    /// Takes into account terminal height limitations and game content size.
    ///
    /// # Returns
    /// * `usize` - Total number of pages needed
    pub fn total_pages(&self) -> usize {
        let mut total_pages = 1;
        let mut current_height = 0u16;
        let available_height = self.screen_height.saturating_sub(5);
        let mut current_page_items = 0;

        for game in &self.content_rows {
            let game_height = self.calculate_effective_game_height(game);
            if current_height + game_height > available_height {
                if current_page_items > 0 {
                    total_pages += 1;
                    current_height = game_height;
                    current_page_items = 1;
                }
            } else {
                current_height += game_height;
                current_page_items += 1;
            }
        }

        total_pages
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