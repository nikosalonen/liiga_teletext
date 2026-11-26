// src/teletext_ui/layout/ansi.rs - ANSI escape helpers that place colored text on screen
//
// Lines and columns are 1-based, as in the ANSI cursor-position code. Every
// helper that writes text ends it with a color reset.

use super::config::LayoutConfig;
use crate::teletext_ui::CONTENT_MARGIN;

/// Moves the cursor to `line`, `column`.
pub fn position_code(line: usize, column: usize) -> String {
    format!("\x1b[{line};{column}H")
}

/// Moves the cursor to `line`, `column` and sets the 256-color foreground.
pub fn color_position_code(line: usize, column: usize, color: u8) -> String {
    format!("\x1b[{line};{column}H\x1b[38;5;{color}m")
}

/// `text` at `line`, `column` in `color`.
pub fn text_at(line: usize, column: usize, color: u8, text: &str) -> String {
    format!("{}{text}\x1b[0m", color_position_code(line, column, color))
}

/// `text` at `line`, `column` in `color`, cut or padded to exactly `width`
/// characters.
pub fn padded_text_at(line: usize, column: usize, color: u8, text: &str, width: usize) -> String {
    let shown: String = text.chars().take(width).collect();
    format!(
        "{}{shown:<width$}\x1b[0m",
        color_position_code(line, column, color)
    )
}

/// The "  -  " that separates the home and away team names.
pub fn separator_at(line: usize, column: usize, color: u8) -> String {
    text_at(line, column, color, "  -  ")
}

/// A whole game line: home team, separator, away team, then `time_score` at
/// the layout's time column in `result_color`.
pub fn game_line_at(
    line: usize,
    layout: &LayoutConfig,
    home_team: &str,
    away_team: &str,
    time_score: &str,
    text_color: u8,
    result_color: u8,
) -> String {
    let home_pos = CONTENT_MARGIN + 1;
    let separator_pos = home_pos + layout.home_team_width;
    let away_pos = separator_pos + layout.separator_width;
    let home_width = layout.home_team_width;
    let away_width = layout.away_team_width;
    format!(
        "{}{:<home_width$}{}{}{:<away_width$}{}{time_score}\x1b[0m",
        color_position_code(line, home_pos, text_color),
        home_team.chars().take(home_width).collect::<String>(),
        separator_at(line, separator_pos, text_color),
        color_position_code(line, away_pos, text_color),
        away_team.chars().take(away_width).collect::<String>(),
        color_position_code(line, layout.time_column, result_color),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_codes() {
        assert_eq!(position_code(5, 10), "\x1b[5;10H");
        assert_eq!(color_position_code(3, 7, 196), "\x1b[3;7H\x1b[38;5;196m");
    }

    #[test]
    fn test_text_at_resets_color() {
        assert_eq!(text_at(1, 2, 46, "3-2"), "\x1b[1;2H\x1b[38;5;46m3-2\x1b[0m");
    }

    #[test]
    fn test_padded_text_at_cuts_and_pads_by_characters() {
        assert_eq!(
            padded_text_at(1, 1, 231, "Kärpät", 8),
            "\x1b[1;1H\x1b[38;5;231mKärpät  \x1b[0m"
        );
        assert_eq!(
            padded_text_at(1, 1, 231, "Jokerit", 4),
            "\x1b[1;1H\x1b[38;5;231mJoke\x1b[0m"
        );
    }

    #[test]
    fn test_game_line_at_places_every_part() {
        let layout = LayoutConfig {
            home_team_width: 6,
            separator_width: 5,
            away_team_width: 6,
            time_column: 20,
            ..LayoutConfig::default()
        };
        let line = game_line_at(4, &layout, "TPS", "HIFK", "3-2", 231, 46);
        let home_pos = CONTENT_MARGIN + 1;
        assert_eq!(
            line,
            format!(
                "\x1b[4;{home_pos}H\x1b[38;5;231mTPS   \x1b[4;{}H\x1b[38;5;231m  -  \x1b[0m\x1b[4;{}H\x1b[38;5;231mHIFK  \x1b[4;20H\x1b[38;5;46m3-2\x1b[0m",
                home_pos + 6,
                home_pos + 11,
            )
        );
    }
}
