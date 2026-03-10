use super::core::TeletextPage;
use super::utils::get_ansi_code;
use crate::teletext_ui::CONTENT_MARGIN;
use crate::ui::teletext::colors::*;

impl TeletextPage {
    /// Renders a standings header row with column labels.
    pub(crate) fn render_standings_header(
        &self,
        buffer: &mut String,
        current_line: &mut usize,
        text_fg_code: u8,
    ) {
        let header = if self.compact_mode {
            format!(" {:>2}  {:<14} {:>4}", "#", "Joukkue", "P")
        } else {
            format!(
                " {:>2}  {:<14} {:>2} {:>2} {:>2} {:>2} {:>2} {:>3} {:>3} {:>4}",
                "#", "Joukkue", "O", "V", "JV", "JH", "H", "TM", "PM", "P"
            )
        };

        let line_code = format!(
            "\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
            *current_line + 1,
            CONTENT_MARGIN + 1,
            text_fg_code,
            header
        );
        buffer.push_str(&line_code);
        *current_line += 1;

        // Add blank line after header when terminal is tall enough
        if self.standings_use_spacing() {
            *current_line += 1;
        }
    }

    /// Renders a playoff separator line (─── across table width).
    fn render_standings_separator(&self, buffer: &mut String, current_line: &mut usize) {
        let dim_code = get_ansi_code(text_fg(), 240);

        let width = if self.compact_mode { 24 } else { 48 };
        let separator: String = "\u{2500}".repeat(width);

        let line_code = format!(
            "\x1b[{};{}H\x1b[38;5;{dim_code}m{separator}\x1b[0m",
            *current_line + 1,
            CONTENT_MARGIN + 1,
        );
        buffer.push_str(&line_code);
        *current_line += 1;
    }

    /// Renders a single standings row.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_standings_row(
        &self,
        buffer: &mut String,
        position: u16,
        team_name: &str,
        games_played: u16,
        wins: u16,
        ot_wins: u16,
        ot_losses: u16,
        losses: u16,
        goals_for: u16,
        goals_against: u16,
        points: u16,
        live_points_delta: &Option<i16>,
        live_position_change: &Option<i16>,
        current_line: &mut usize,
    ) {
        // Draw playoff separator lines (API provides playoffsLines as positions after which to draw)
        // e.g., [4, 12] means draw lines after position 4 and 12, i.e., before positions 5 and 13
        if self.playoffs_lines.iter().any(|&line| position == line + 1) {
            self.render_standings_separator(buffer, current_line);
        }

        let yellow_code = get_ansi_code(position_fg(), 226);
        let white_code = get_ansi_code(text_fg(), 231);
        let green_code = get_ansi_code(result_fg(), 46);
        let magenta_code = get_ansi_code(winning_goal_fg(), 201);
        let cyan_code = get_ansi_code(home_scorer_fg(), 51);

        // Position change indicator (1 char)
        let pos_indicator = match live_position_change {
            Some(change) if *change > 0 => format!("\x1b[38;5;{magenta_code}m\u{2191}\x1b[0m"),
            Some(change) if *change < 0 => format!("\x1b[38;5;{magenta_code}m\u{2193}\x1b[0m"),
            _ => " ".to_string(),
        };

        // Team name color: cyan if live game active
        let team_color = if live_points_delta.is_some() {
            cyan_code
        } else {
            white_code
        };

        // Truncate team name to 14 chars
        let display_name: String = team_name.chars().take(14).collect();

        // Show potential points (base + delta) when live game is active
        let display_points = match live_points_delta {
            Some(d) if *d != 0 => (points as i16 + d) as u16,
            _ => points,
        };

        let row = if self.compact_mode {
            let live_suffix = match live_points_delta {
                Some(d) if *d > 0 => format!(" \x1b[38;5;{magenta_code}m+{d}\x1b[0m"),
                _ => String::new(),
            };

            format!(
                "{pos_indicator}\x1b[38;5;{yellow_code}m{:>2}\x1b[0m  \x1b[38;5;{team_color}m{:<14}\x1b[0m \x1b[38;5;{green_code}m{:>4}\x1b[0m{live_suffix}",
                position, display_name, display_points,
            )
        } else {
            let live_suffix = match live_points_delta {
                Some(d) if *d > 0 => format!(" \x1b[38;5;{magenta_code}m+{d}\x1b[0m"),
                Some(d) if *d != 0 => format!(" \x1b[38;5;{magenta_code}m{d}\x1b[0m"),
                _ => String::new(),
            };

            format!(
                "{pos_indicator}\x1b[38;5;{yellow_code}m{:>2}\x1b[0m  \x1b[38;5;{team_color}m{:<14}\x1b[0m \x1b[38;5;{white_code}m{:>2} {:>2} {:>2} {:>2} {:>2} {:>3} {:>3}\x1b[0m \x1b[38;5;{green_code}m{:>4}\x1b[0m{live_suffix}",
                position,
                display_name,
                games_played,
                wins,
                ot_wins,
                ot_losses,
                losses,
                goals_for,
                goals_against,
                display_points,
            )
        };

        let line_code = format!("\x1b[{};{}H{}", *current_line + 1, CONTENT_MARGIN + 1, row);
        buffer.push_str(&line_code);
        *current_line += 1;

        // Add blank line between rows when terminal is tall enough
        if self.standings_use_spacing() {
            *current_line += 1;
        }
    }
}

/// Color for position numbers (yellow) — delegates to the shared teletext palette
fn position_fg() -> crossterm::style::Color {
    goal_type_fg()
}
