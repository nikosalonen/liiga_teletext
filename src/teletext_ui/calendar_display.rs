//! Calendar page (224): a month grid where game days stand out, and the
//! games of the selected day beside or under it.

use crate::data_fetcher::models::calendar::{CalendarGame, CalendarGameState, SeasonCalendar};
use crate::teletext_ui::bracket_display::truncate_team_name;
use crate::teletext_ui::core::TeletextRow;
use crate::teletext_ui::formatting::finnish_weekday;
use chrono::{Datelike, Days, NaiveDate};

// ANSI 256 colors, matching the bracket page
const WHITE: u8 = 231;
const DIM: u8 = 240;
const YELLOW: u8 = 226;
const GREEN: u8 = 46;
const CYAN: u8 = 51;
const BLACK: u8 = 16;
const RESET: &str = "\x1b[0m";

/// Width of a team name in the day's game list
const TEAM_WIDTH: usize = 10;

/// Columns of one day cell: the number with a space on each side, so the
/// selected day's highlight is a block around it.
const CELL_WIDTH: usize = 4;

/// Visible width of the month grid: week number and a space, then seven days.
const GRID_WIDTH: usize = 3 + 7 * CELL_WIDTH;

/// Space between the month and the day's games when they sit side by side.
const COLUMN_GAP: usize = 6;

/// Visible width the day's games need: two team names and a result, or the
/// "+ 4 muuta, Enter näyttää kaikki" line.
const LIST_WIDTH: usize = 32;

/// Grid row that the day's games start on when they sit beside the month:
/// level with the weekday header.
const LIST_TOP: usize = 2;

fn fg(code: u8) -> String {
    format!("\x1b[38;5;{code}m")
}

fn finnish_month(month: u32) -> &'static str {
    match month {
        1 => "TAMMIKUU",
        2 => "HELMIKUU",
        3 => "MAALISKUU",
        4 => "HUHTIKUU",
        5 => "TOUKOKUU",
        6 => "KESÄKUU",
        7 => "HEINÄKUU",
        8 => "ELOKUU",
        9 => "SYYSKUU",
        10 => "LOKAKUU",
        11 => "MARRASKUU",
        _ => "JOULUKUU",
    }
}

/// Removes ANSI color codes, leaving the text as it shows on screen.
fn strip_ansi(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c in chars.by_ref() {
                if c == 'm' {
                    break;
                }
            }
        } else {
            plain.push(c);
        }
    }
    plain
}

fn visible_width(text: &str) -> usize {
    strip_ansi(text).chars().count()
}

/// Renders the month of `cursor` with `cursor` selected, and the selected
/// day's games: beside the month when `max_width` has room, under it
/// otherwise. The rows fit in `max_lines`. Blank rows go between the weeks
/// when they fit; when the day has more games than fit, the last line says
/// how many were left out.
pub fn render_calendar(
    calendar: &SeasonCalendar,
    cursor: NaiveDate,
    today: NaiveDate,
    max_width: usize,
    max_lines: usize,
) -> Vec<TeletextRow> {
    let lines = if max_width >= GRID_WIDTH + COLUMN_GAP + LIST_WIDTH {
        side_by_side(calendar, cursor, today, max_lines)
    } else {
        stacked(calendar, cursor, today, max_lines)
    };
    lines.into_iter().map(TeletextRow::BracketLine).collect()
}

/// The month on the left and the day's games on the right. With the games
/// out of the way, the weeks nearly always have room for blank rows.
fn side_by_side(
    calendar: &SeasonCalendar,
    cursor: NaiveDate,
    today: NaiveDate,
    max_lines: usize,
) -> Vec<String> {
    let spaced_grid = month_grid(calendar, cursor, today, true);
    let grid_is_spaced = spaced_grid.len() <= max_lines;
    let grid = if grid_is_spaced {
        spaced_grid
    } else {
        month_grid(calendar, cursor, today, false)
    };
    let games = calendar.games_on(cursor);
    let room = max_lines.saturating_sub(LIST_TOP);
    // Spaced like the weeks, so each game lines up with a week row
    let spaced_list = day_summary(games, cursor, room, true);
    let list = if grid_is_spaced && spaced_list.len() <= room {
        spaced_list
    } else {
        day_summary(games, cursor, room, false)
    };

    let height = grid.len().max(LIST_TOP + list.len());
    (0..height)
        .map(|row| {
            let left = grid.get(row).map_or("", String::as_str);
            match row
                .checked_sub(LIST_TOP)
                .and_then(|index| list.get(index))
                .filter(|right| !right.is_empty())
            {
                Some(right) => {
                    let padding = (GRID_WIDTH + COLUMN_GAP).saturating_sub(visible_width(left));
                    format!("{left}{}{right}", " ".repeat(padding))
                }
                None => left.to_string(),
            }
        })
        .collect()
}

/// The month with the day's games under it, for narrow terminals. The weeks
/// get blank rows only when every game still fits under them.
fn stacked(
    calendar: &SeasonCalendar,
    cursor: NaiveDate,
    today: NaiveDate,
    max_lines: usize,
) -> Vec<String> {
    let games = calendar.games_on(cursor);
    let summary_height = if games.is_empty() { 1 } else { 2 + games.len() };
    let spaced = month_grid(calendar, cursor, today, true);
    let mut lines = if spaced.len() + 1 + summary_height <= max_lines {
        spaced
    } else {
        month_grid(calendar, cursor, today, false)
    };
    lines.push(String::new());
    let room = max_lines.saturating_sub(lines.len());
    lines.extend(day_summary(games, cursor, room, false));
    lines
}

/// Title, weekday header and one row per week, Monday first, with ISO week
/// numbers in the first column. `spaced` puts a blank row under the header
/// and between the weeks.
fn month_grid(
    calendar: &SeasonCalendar,
    cursor: NaiveDate,
    today: NaiveDate,
    spaced: bool,
) -> Vec<String> {
    let mut lines = vec![
        format!(
            "{}{} {}{RESET}",
            fg(YELLOW),
            finnish_month(cursor.month()),
            cursor.year()
        ),
        String::new(),
        format!("{}vk  ma  ti  ke  to  pe  la  su{RESET}", fg(CYAN)),
    ];

    let first_of_month = cursor.with_day(1).expect("day 1 exists in every month");
    let mut week_start =
        first_of_month - Days::new(u64::from(first_of_month.weekday().num_days_from_monday()));
    while week_start.month() == cursor.month() || week_start < first_of_month {
        if spaced {
            lines.push(String::new());
        }
        let mut line = format!("{}{:>2}{RESET} ", fg(DIM), week_start.iso_week().week());
        for offset in 0..7 {
            let day = week_start + Days::new(offset);
            if day.month() == cursor.month() {
                line.push_str(&day_cell(calendar, day, cursor, today));
            } else {
                line.push_str(&" ".repeat(CELL_WIDTH));
            }
        }
        lines.push(line.trim_end().to_string());
        week_start = week_start + Days::new(7);
    }
    lines
}

/// One day cell, colored by what the day holds.
fn day_cell(
    calendar: &SeasonCalendar,
    day: NaiveDate,
    cursor: NaiveDate,
    today: NaiveDate,
) -> String {
    let cell = format!(" {:>2} ", day.day());
    if day == cursor {
        return format!("\x1b[48;5;{CYAN}m{}{cell}{RESET}", fg(BLACK));
    }
    let games = calendar.games_on(day);
    let color = if games
        .iter()
        .any(|game| game.state == CalendarGameState::Live)
    {
        GREEN
    } else if day == today {
        YELLOW
    } else if games.is_empty() {
        DIM
    } else {
        WHITE
    };
    format!("{}{cell}{RESET}", fg(color))
}

/// The selected day's title line, a blank line and its games, in at most
/// `max_lines`. `spaced` puts a blank line between the games; the caller
/// checks that they fit.
fn day_summary(
    games: &[CalendarGame],
    day: NaiveDate,
    max_lines: usize,
    spaced: bool,
) -> Vec<String> {
    if max_lines == 0 {
        return Vec::new();
    }
    let count = match games.len() {
        0 => format!("{}ei otteluita", fg(DIM)),
        1 => format!("{}1 ottelu", fg(WHITE)),
        n => format!("{}{n} ottelua", fg(WHITE)),
    };
    let mut lines = vec![format!(
        "{}{} {}  {count}{RESET}",
        fg(CYAN),
        finnish_weekday(day),
        day.format("%d.%m.%Y")
    )];
    if games.is_empty() || max_lines < 3 {
        return lines;
    }

    lines.push(String::new());
    let room = max_lines - 2;
    let shown = if games.len() > room {
        room - 1
    } else {
        games.len()
    };
    for (index, game) in games[..shown].iter().enumerate() {
        if spaced && index > 0 {
            lines.push(String::new());
        }
        lines.push(game_line(game));
    }
    if shown < games.len() {
        lines.push(format!(
            "{}+ {} muuta, Enter näyttää kaikki{RESET}",
            fg(DIM),
            games.len() - shown
        ));
    }
    lines
}

fn game_line(game: &CalendarGame) -> String {
    let (result, result_color) = match game.state {
        CalendarGameState::Scheduled => (game.start_time.clone(), WHITE),
        CalendarGameState::Live => (format!("{}-{}", game.home_goals, game.away_goals), GREEN),
        CalendarGameState::Final { overtime, shootout } => {
            let score = format!("{}-{}", game.home_goals, game.away_goals);
            let suffix = if shootout {
                " rl"
            } else if overtime {
                " ja"
            } else {
                ""
            };
            (format!("{score}{suffix}"), WHITE)
        }
    };
    format!(
        "{}{:<width$}  {:<width$}  {}{result}{RESET}",
        fg(WHITE),
        truncate_team_name(&game.home_team, TEAM_WIDTH),
        truncate_team_name(&game.away_team, TEAM_WIDTH),
        fg(result_color),
        width = TEAM_WIDTH
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::models::ScheduleApiGame;
    use chrono::FixedOffset;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    fn game(start: &str, home: &str, away: &str, started: bool, ended: bool) -> ScheduleApiGame {
        ScheduleApiGame {
            id: 1,
            season: 2027,
            start: start.to_string(),
            home_team_name: home.to_string(),
            away_team_name: away.to_string(),
            serie: 1,
            finished_type: ended.then(|| "ENDED_DURING_REGULAR_TIME".to_string()),
            started,
            ended,
            game_time: None,
            play_off_phase: None,
            play_off_pair: None,
            play_off_req_wins: None,
            home_team_goals: 3,
            away_team_goals: 2,
        }
    }

    fn calendar(games: &[ScheduleApiGame]) -> SeasonCalendar {
        SeasonCalendar::from_schedule_in(2027, games, &FixedOffset::east_opt(7200).unwrap())
    }

    /// The text of each row, without color codes.
    fn plain_lines(rows: &[TeletextRow]) -> Vec<String> {
        rows.iter()
            .map(|row| match row {
                TeletextRow::BracketLine(line) => strip_ansi(line).trim_end().to_string(),
                other => panic!("unexpected row {other:?}"),
            })
            .collect()
    }

    /// Narrow enough to put the games under the month.
    const NARROW: usize = 40;
    /// Wide enough to put the games beside the month.
    const WIDE: usize = 78;

    #[test]
    fn test_month_grid_starts_weeks_on_monday() {
        // November 2026 starts on a Sunday and spans six weeks (44-49)
        let rows = render_calendar(
            &calendar(&[]),
            date("2026-11-27"),
            date("2026-11-27"),
            NARROW,
            12,
        );
        let lines = plain_lines(&rows);

        assert_eq!(lines[0], "MARRASKUU 2026");
        assert_eq!(lines[2], "vk  ma  ti  ke  to  pe  la  su");
        assert_eq!(lines[3], "44                           1");
        assert_eq!(lines[4], "45   2   3   4   5   6   7   8");
        assert_eq!(lines[7], "48  23  24  25  26  27  28  29");
        assert_eq!(lines[8], "49  30");
    }

    #[test]
    fn test_month_starting_on_monday_has_no_leading_blank_week() {
        // June 2026 starts on a Monday and spans five weeks
        let rows = render_calendar(
            &calendar(&[]),
            date("2026-06-10"),
            date("2026-11-27"),
            NARROW,
            11,
        );
        let lines = plain_lines(&rows);

        assert_eq!(lines[3], "23   1   2   3   4   5   6   7");
        assert_eq!(lines[7], "27  29  30");
        assert_eq!(lines[8], "", "the blank line before the day summary");
    }

    #[test]
    fn test_weeks_get_blank_rows_when_they_fit() {
        let rows = render_calendar(
            &calendar(&[]),
            date("2026-11-27"),
            date("2026-11-27"),
            NARROW,
            19,
        );
        let lines = plain_lines(&rows);

        assert_eq!(lines[3], "", "blank row under the weekday header");
        assert_eq!(lines[4], "44                           1");
        assert_eq!(lines[5], "");
        assert_eq!(lines[6], "45   2   3   4   5   6   7   8");
        assert_eq!(lines[14], "49  30");
        assert_eq!(lines.last().unwrap(), "pe 27.11.2026  ei otteluita");
    }

    #[test]
    fn test_selected_day_is_highlighted_as_a_block() {
        let rows = render_calendar(
            &calendar(&[]),
            date("2026-11-27"),
            date("2026-11-01"),
            NARROW,
            12,
        );
        let TeletextRow::BracketLine(week) = &rows[7] else {
            panic!("expected a line");
        };

        assert!(
            week.contains(&format!("\x1b[48;5;{CYAN}m{} 27 ", fg(BLACK))),
            "27 should have the cursor background: {week:?}"
        );
    }

    #[test]
    fn test_game_days_and_live_days_are_colored() {
        let games = [
            game("2026-11-24T15:30:00Z", "HIFK", "TPS", true, true),
            game("2026-11-25T15:30:00Z", "Ilves", "Lukko", true, false),
        ];
        let rows = render_calendar(
            &calendar(&games),
            date("2026-11-01"),
            date("2026-11-01"),
            NARROW,
            12,
        );
        let TeletextRow::BracketLine(week) = &rows[7] else {
            panic!("expected a line");
        };

        assert!(week.contains(&format!("{} 24 ", fg(WHITE))), "{week:?}");
        assert!(week.contains(&format!("{} 25 ", fg(GREEN))), "{week:?}");
        assert!(week.contains(&format!("{} 26 ", fg(DIM))), "{week:?}");
    }

    #[test]
    fn test_day_summary_lists_games() {
        let mut overtime = game("2026-11-27T16:30:00Z", "Kärpät", "Ässät", true, true);
        overtime.finished_type = Some("ENDED_DURING_EXTENDED_GAME_TIME".to_string());
        let games = [
            game("2026-11-27T15:30:00Z", "HIFK", "Tappara", false, false),
            overtime,
        ];

        let rows = render_calendar(
            &calendar(&games),
            date("2026-11-27"),
            date("2026-11-27"),
            NARROW,
            14,
        );
        let lines = plain_lines(&rows);

        assert_eq!(lines[10], "pe 27.11.2026  2 ottelua");
        assert_eq!(lines[11], "");
        assert_eq!(lines[12], "HIFK        Tappara     17.30");
        assert_eq!(lines[13], "Kärpät      Ässät       3-2 ja");
        assert_eq!(lines.len(), 14);
    }

    #[test]
    fn test_games_sit_beside_the_month_on_wide_terminals() {
        let games = [game(
            "2026-11-27T15:30:00Z",
            "HIFK",
            "Tappara",
            false,
            false,
        )];

        let rows = render_calendar(
            &calendar(&games),
            date("2026-11-27"),
            date("2026-11-27"),
            WIDE,
            19,
        );
        let lines = plain_lines(&rows);

        let list_column = GRID_WIDTH + COLUMN_GAP;
        assert_eq!(
            lines[2],
            format!(
                "{:<list_column$}pe 27.11.2026  1 ottelu",
                "vk  ma  ti  ke  to  pe  la  su"
            )
        );
        assert_eq!(lines[3], "", "blank row under the header on both sides");
        assert_eq!(
            lines[4],
            format!(
                "{:<list_column$}HIFK        Tappara     17.30",
                "44                           1"
            )
        );
        assert_eq!(lines.len(), 15, "spaced grid, nothing under it");
    }

    #[test]
    fn test_games_beside_spaced_weeks_line_up_with_week_rows() {
        let games = [
            game("2026-11-27T15:30:00Z", "HIFK", "Tappara", false, false),
            game("2026-11-27T16:30:00Z", "Ilves", "Lukko", false, false),
        ];

        let rows = render_calendar(
            &calendar(&games),
            date("2026-11-27"),
            date("2026-11-27"),
            WIDE,
            19,
        );
        let lines = plain_lines(&rows);

        assert!(lines[4].ends_with("HIFK        Tappara     17.30"));
        assert_eq!(lines[5], "");
        assert!(lines[6].ends_with("Ilves       Lukko       18.30"));
    }

    #[test]
    fn test_day_without_games() {
        let rows = render_calendar(
            &calendar(&[]),
            date("2026-11-26"),
            date("2026-11-27"),
            NARROW,
            12,
        );
        let lines = plain_lines(&rows);

        assert_eq!(lines.last().unwrap(), "to 26.11.2026  ei otteluita");
    }

    #[test]
    fn test_games_that_dont_fit_are_counted() {
        let games: Vec<_> = (0..7)
            .map(|i| {
                game(
                    &format!("2026-11-27T15:{i:02}:00Z"),
                    "HIFK",
                    "TPS",
                    false,
                    false,
                )
            })
            .collect();

        // Compact grid 9 lines + blank + title + blank leaves room for 3 lines
        let rows = render_calendar(
            &calendar(&games),
            date("2026-11-27"),
            date("2026-11-27"),
            NARROW,
            15,
        );
        let lines = plain_lines(&rows);

        assert_eq!(lines.len(), 15);
        assert_eq!(lines[14], "+ 5 muuta, Enter näyttää kaikki");
    }
}
