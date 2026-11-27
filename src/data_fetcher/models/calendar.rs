//! Season calendar: every game of one season, grouped by local date.
//!
//! The calendar view (page 224) shows a month grid where game days stand
//! out, plus the games of the selected day. One season schedule request per
//! tournament gives all of it, so moving around the calendar needs no
//! further fetches.

use super::ScheduleApiGame;
use chrono::{DateTime, Datelike, NaiveDate, TimeZone};
use std::collections::BTreeMap;

/// The season a date belongs to, named by the year it ends in (the API's
/// season identifier: 2026 means 2025-2026).
///
/// July starts the next season, because the August practice games belong to
/// the season that starts in September. Standings use the same split.
pub fn season_for_date(date: NaiveDate) -> i32 {
    if date.month() <= 6 {
        date.year()
    } else {
        date.year() + 1
    }
}

/// Where a calendar game stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CalendarGameState {
    Scheduled,
    Live,
    Final { overtime: bool, shootout: bool },
}

/// One game as the calendar shows it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CalendarGame {
    /// Start time in local time, as the games page writes it ("18.30")
    pub start_time: String,
    pub home_team: String,
    pub away_team: String,
    pub home_goals: i32,
    pub away_goals: i32,
    pub state: CalendarGameState,
}

/// All games of one season, by local date.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SeasonCalendar {
    pub season: i32,
    games_by_date: BTreeMap<NaiveDate, Vec<CalendarGame>>,
}

impl SeasonCalendar {
    /// Builds the calendar from season schedule games, placing each game on
    /// its date in the local time zone.
    pub fn from_schedule(season: i32, games: &[ScheduleApiGame]) -> Self {
        Self::from_schedule_in(season, games, &chrono::Local)
    }

    /// [`Self::from_schedule`] in a given time zone, so tests don't depend
    /// on the machine's zone.
    pub fn from_schedule_in<Tz: TimeZone>(season: i32, games: &[ScheduleApiGame], tz: &Tz) -> Self
    where
        Tz::Offset: std::fmt::Display,
    {
        let mut dated: Vec<(DateTime<Tz>, CalendarGame)> = games
            .iter()
            .filter_map(|game| match DateTime::parse_from_rfc3339(&game.start) {
                Ok(start) => {
                    let start = start.with_timezone(tz);
                    Some((start.clone(), calendar_game(game, &start)))
                }
                Err(e) => {
                    // Without a start time the game has no day to sit on
                    tracing::warn!(
                        "Calendar: skipping game {} with unreadable start time {:?}: {e}",
                        game.id,
                        game.start
                    );
                    None
                }
            })
            .collect();
        dated.sort_by(|(a, _), (b, _)| a.cmp(b));

        let mut games_by_date: BTreeMap<NaiveDate, Vec<CalendarGame>> = BTreeMap::new();
        for (start, game) in dated {
            games_by_date
                .entry(start.date_naive())
                .or_default()
                .push(game);
        }
        Self {
            season,
            games_by_date,
        }
    }

    /// The games on `date`, in start order. Empty when there are none.
    pub fn games_on(&self, date: NaiveDate) -> &[CalendarGame] {
        self.games_by_date.get(&date).map_or(&[], Vec::as_slice)
    }

    pub fn has_games_on(&self, date: NaiveDate) -> bool {
        self.games_by_date.contains_key(&date)
    }

    /// Number of games in the season.
    pub fn game_count(&self) -> usize {
        self.games_by_date.values().map(Vec::len).sum()
    }

    /// Whether `date` falls in this calendar's season, i.e. whether this
    /// calendar can answer for it without fetching another season.
    pub fn covers(&self, date: NaiveDate) -> bool {
        season_for_date(date) == self.season
    }
}

fn calendar_game<Tz: TimeZone>(game: &ScheduleApiGame, start: &DateTime<Tz>) -> CalendarGame
where
    Tz::Offset: std::fmt::Display,
{
    let state = if game.ended {
        let finished_type = game.finished_type.as_deref();
        CalendarGameState::Final {
            overtime: finished_type == Some("ENDED_DURING_EXTENDED_GAME_TIME"),
            shootout: finished_type == Some("ENDED_DURING_WINNING_SHOT_COMPETITION"),
        }
    } else if game.started {
        CalendarGameState::Live
    } else {
        CalendarGameState::Scheduled
    };

    CalendarGame {
        start_time: start.format("%H.%M").to_string(),
        home_team: game.home_team_name.clone(),
        away_team: game.away_team_name.clone(),
        home_goals: game.home_team_goals,
        away_goals: game.away_team_goals,
        state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn finland_winter() -> FixedOffset {
        FixedOffset::east_opt(2 * 3600).unwrap()
    }

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    fn schedule_game(id: i32, start: &str, started: bool, ended: bool) -> ScheduleApiGame {
        ScheduleApiGame {
            id,
            season: 2027,
            start: start.to_string(),
            home_team_name: format!("Home{id}"),
            away_team_name: format!("Away{id}"),
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

    #[test]
    fn test_season_for_date_splits_at_july() {
        assert_eq!(season_for_date(date("2026-06-30")), 2026);
        assert_eq!(season_for_date(date("2026-07-01")), 2027);
        // August practice games open the season that starts in September
        assert_eq!(season_for_date(date("2026-08-15")), 2027);
        assert_eq!(season_for_date(date("2026-11-27")), 2027);
        assert_eq!(season_for_date(date("2027-03-10")), 2027);
    }

    #[test]
    fn test_games_grouped_by_local_date_in_start_order() {
        let games = vec![
            schedule_game(2, "2026-11-27T16:30:00Z", false, false),
            schedule_game(1, "2026-11-27T15:00:00Z", false, false),
            schedule_game(3, "2026-11-28T15:00:00Z", false, false),
        ];

        let calendar = SeasonCalendar::from_schedule_in(2027, &games, &finland_winter());

        let friday = calendar.games_on(date("2026-11-27"));
        assert_eq!(friday.len(), 2);
        assert_eq!(friday[0].home_team, "Home1");
        assert_eq!(friday[0].start_time, "17.00");
        assert_eq!(friday[1].start_time, "18.30");
        assert_eq!(calendar.games_on(date("2026-11-28")).len(), 1);
        assert!(!calendar.has_games_on(date("2026-11-29")));
        assert_eq!(calendar.game_count(), 3);
    }

    #[test]
    fn test_late_utc_start_lands_on_next_local_day() {
        // 23:30 UTC is 01:30 the next day in Finland
        let games = vec![schedule_game(1, "2026-11-27T23:30:00Z", false, false)];

        let calendar = SeasonCalendar::from_schedule_in(2027, &games, &finland_winter());

        assert!(calendar.has_games_on(date("2026-11-28")));
        assert!(!calendar.has_games_on(date("2026-11-27")));
    }

    #[test]
    fn test_game_states() {
        let mut overtime = schedule_game(3, "2026-11-27T15:00:00Z", true, true);
        overtime.finished_type = Some("ENDED_DURING_EXTENDED_GAME_TIME".to_string());
        let mut shootout = schedule_game(4, "2026-11-27T15:00:01Z", true, true);
        shootout.finished_type = Some("ENDED_DURING_WINNING_SHOT_COMPETITION".to_string());
        let games = vec![
            schedule_game(1, "2026-11-27T14:00:00Z", false, false),
            schedule_game(2, "2026-11-27T14:30:00Z", true, false),
            overtime,
            shootout,
            schedule_game(5, "2026-11-27T15:00:02Z", true, true),
        ];

        let calendar = SeasonCalendar::from_schedule_in(2027, &games, &finland_winter());
        let states: Vec<_> = calendar
            .games_on(date("2026-11-27"))
            .iter()
            .map(|g| g.state)
            .collect();

        assert_eq!(
            states,
            vec![
                CalendarGameState::Scheduled,
                CalendarGameState::Live,
                CalendarGameState::Final {
                    overtime: true,
                    shootout: false
                },
                CalendarGameState::Final {
                    overtime: false,
                    shootout: true
                },
                CalendarGameState::Final {
                    overtime: false,
                    shootout: false
                },
            ]
        );
    }

    #[test]
    fn test_unparsable_start_is_skipped() {
        let games = vec![schedule_game(1, "", false, false)];

        let calendar = SeasonCalendar::from_schedule_in(2027, &games, &finland_winter());

        assert_eq!(
            calendar,
            SeasonCalendar::from_schedule_in(2027, &[], &finland_winter())
        );
    }

    #[test]
    fn test_covers_only_its_own_season() {
        let calendar = SeasonCalendar::from_schedule_in(2027, &[], &finland_winter());

        assert!(calendar.covers(date("2026-08-01")));
        assert!(calendar.covers(date("2027-05-31")));
        assert!(!calendar.covers(date("2026-06-30")));
        assert!(!calendar.covers(date("2027-07-01")));
    }
}
