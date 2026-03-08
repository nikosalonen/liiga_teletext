use crate::data_fetcher::models::{GameData, PlayoffSeriesScore, ScheduleApiGame};
use std::collections::HashMap;

/// Series key: (serie/tournament ID, play_off_phase, play_off_pair)
type SeriesKey = (i32, i32, i32);

/// Calculates series scores for playoff games based on the full tournament schedule.
///
/// Groups games by (serie, play_off_phase, play_off_pair), counts wins per team from
/// completed games up to and including the target date.
pub fn calculate_series_scores(
    all_schedule_games: &[ScheduleApiGame],
    target_games: &mut [GameData],
    target_date: &str,
) {
    let mut series_wins: HashMap<SeriesKey, HashMap<String, u8>> = HashMap::new();
    let mut series_req_wins: HashMap<SeriesKey, u8> = HashMap::new();

    // Build a lookup from (home_team, start) -> serie for matching target games
    let serie_lookup: HashMap<(&str, &str), i32> = all_schedule_games
        .iter()
        .filter(|g| g.play_off_phase.is_some())
        .map(|g| ((g.home_team_name.as_str(), g.start.as_str()), g.serie))
        .collect();

    // Count wins from completed schedule games on or before the target date
    for sched_game in all_schedule_games {
        let (Some(phase), Some(pair)) = (sched_game.play_off_phase, sched_game.play_off_pair)
        else {
            continue;
        };
        if !sched_game.ended {
            continue;
        }

        // Only count games that started on or before the target date
        let game_date = sched_game.start.get(..10).unwrap_or("");
        if game_date > target_date {
            continue;
        }

        let key = (sched_game.serie, phase, pair);

        if let Some(req_wins) = sched_game.play_off_req_wins {
            series_req_wins.insert(key, req_wins as u8);
        }

        // Determine winner from schedule game scores directly
        // Skip games with tied or zero-zero scores (data may be incomplete)
        if sched_game.home_team_goals == sched_game.away_team_goals {
            continue;
        }
        let winner = if sched_game.home_team_goals > sched_game.away_team_goals {
            &sched_game.home_team_name
        } else {
            &sched_game.away_team_name
        };
        *series_wins
            .entry(key)
            .or_default()
            .entry(winner.to_string())
            .or_insert(0) += 1;
    }

    // Set series_score on each target game that has playoff fields
    for game in target_games.iter_mut() {
        let (Some(phase), Some(pair)) = (game.play_off_phase, game.play_off_pair) else {
            continue;
        };

        // Find the actual serie ID by matching this target game to its schedule game
        let Some(&serie) = serie_lookup.get(&(game.home_team.as_str(), game.start.as_str())) else {
            continue;
        };
        let key = (serie, phase, pair);

        let req_wins = game
            .play_off_req_wins
            .map(|r| r as u8)
            .or_else(|| series_req_wins.get(&key).copied())
            .unwrap_or(4);

        if let Some(wins) = series_wins.get(&key) {
            let home_wins = wins.get(&game.home_team).copied().unwrap_or(0);
            let away_wins = wins.get(&game.away_team).copied().unwrap_or(0);

            game.series_score = Some(PlayoffSeriesScore {
                home_team_wins: home_wins,
                away_team_wins: away_wins,
                req_wins,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::teletext_ui::ScoreType;

    struct ScheduleApiGameBuilder {
        serie: i32,
        phase: i32,
        pair: i32,
        req_wins: i32,
    }

    impl ScheduleApiGameBuilder {
        fn new(phase: i32, pair: i32, req_wins: i32) -> Self {
            Self {
                serie: 2, // playoffs by default
                phase,
                pair,
                req_wins,
            }
        }

        fn with_serie(mut self, serie: i32) -> Self {
            self.serie = serie;
            self
        }

        #[allow(clippy::too_many_arguments)]
        fn game(
            &self,
            id: i32,
            home: &str,
            away: &str,
            start: &str,
            ended: bool,
            home_goals: i32,
            away_goals: i32,
        ) -> ScheduleApiGame {
            ScheduleApiGame {
                id,
                season: 2024,
                start: start.to_string(),
                home_team_name: home.to_string(),
                away_team_name: away.to_string(),
                serie: self.serie,
                finished_type: if ended {
                    Some("FINISHED".to_string())
                } else {
                    None
                },
                started: ended,
                ended,
                game_time: if ended { Some(3600) } else { None },
                play_off_phase: Some(self.phase),
                play_off_pair: Some(self.pair),
                play_off_req_wins: Some(self.req_wins),
                home_team_goals: home_goals,
                away_team_goals: away_goals,
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn make_game_data(
        home: &str,
        away: &str,
        result: &str,
        start: &str,
        serie: &str,
        phase: i32,
        pair: i32,
        req_wins: i32,
    ) -> GameData {
        GameData {
            home_team: home.to_string(),
            away_team: away.to_string(),
            time: "18:30".to_string(),
            result: result.to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: serie.to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: start.to_string(),
            play_off_phase: Some(phase),
            play_off_pair: Some(pair),
            play_off_req_wins: Some(req_wins),
            series_score: None,
        }
    }

    #[test]
    fn test_series_with_3_games_one_team_leads_2_1() {
        let b = ScheduleApiGameBuilder::new(1, 1, 4);
        let schedule = vec![
            b.game(1, "TPS", "HIFK", "2024-03-20T18:00:00Z", true, 3, 1),
            b.game(2, "HIFK", "TPS", "2024-03-22T18:00:00Z", true, 4, 2),
            b.game(3, "TPS", "HIFK", "2024-03-24T18:00:00Z", true, 2, 1),
        ];

        let mut games = vec![
            make_game_data(
                "TPS",
                "HIFK",
                "3-1",
                "2024-03-20T18:00:00Z",
                "playoffs",
                1,
                1,
                4,
            ),
            make_game_data(
                "HIFK",
                "TPS",
                "4-2",
                "2024-03-22T18:00:00Z",
                "playoffs",
                1,
                1,
                4,
            ),
            make_game_data(
                "TPS",
                "HIFK",
                "2-1",
                "2024-03-24T18:00:00Z",
                "playoffs",
                1,
                1,
                4,
            ),
        ];

        calculate_series_scores(&schedule, &mut games, "2024-12-31");

        // TPS won games 1 and 3, HIFK won game 2
        let score0 = games[0].series_score.as_ref().unwrap();
        assert_eq!(score0.home_team_wins, 2); // TPS wins when TPS is home
        assert_eq!(score0.away_team_wins, 1); // HIFK wins when HIFK is away

        let score1 = games[1].series_score.as_ref().unwrap();
        assert_eq!(score1.home_team_wins, 1); // HIFK wins when HIFK is home
        assert_eq!(score1.away_team_wins, 2); // TPS wins when TPS is away
    }

    #[test]
    fn test_decided_series_4_wins() {
        let b = ScheduleApiGameBuilder::new(1, 1, 4);
        let schedule = vec![
            b.game(1, "TPS", "HIFK", "2024-03-20T18:00:00Z", true, 3, 1),
            b.game(2, "HIFK", "TPS", "2024-03-22T18:00:00Z", true, 1, 4),
            b.game(3, "TPS", "HIFK", "2024-03-24T18:00:00Z", true, 2, 1),
            b.game(4, "HIFK", "TPS", "2024-03-26T18:00:00Z", true, 0, 3),
        ];

        let mut games = vec![
            make_game_data(
                "TPS",
                "HIFK",
                "3-1",
                "2024-03-20T18:00:00Z",
                "playoffs",
                1,
                1,
                4,
            ),
            make_game_data(
                "HIFK",
                "TPS",
                "1-4",
                "2024-03-22T18:00:00Z",
                "playoffs",
                1,
                1,
                4,
            ),
            make_game_data(
                "TPS",
                "HIFK",
                "2-1",
                "2024-03-24T18:00:00Z",
                "playoffs",
                1,
                1,
                4,
            ),
            make_game_data(
                "HIFK",
                "TPS",
                "0-3",
                "2024-03-26T18:00:00Z",
                "playoffs",
                1,
                1,
                4,
            ),
        ];

        calculate_series_scores(&schedule, &mut games, "2024-12-31");

        let score = games[0].series_score.as_ref().unwrap();
        assert_eq!(score.home_team_wins, 4); // TPS
        assert_eq!(score.away_team_wins, 0); // HIFK
        assert_eq!(score.req_wins, 4);
    }

    #[test]
    fn test_bronze_game_req_wins_1() {
        let b = ScheduleApiGameBuilder::new(4, 1, 1);
        let schedule = vec![b.game(1, "TPS", "HIFK", "2024-04-01T18:00:00Z", true, 3, 2)];
        let mut games = vec![make_game_data(
            "TPS",
            "HIFK",
            "3-2",
            "2024-04-01T18:00:00Z",
            "playoffs",
            4,
            1,
            1,
        )];

        calculate_series_scores(&schedule, &mut games, "2024-12-31");

        let score = games[0].series_score.as_ref().unwrap();
        assert_eq!(score.req_wins, 1);
        assert_eq!(score.home_team_wins, 1);
        assert_eq!(score.away_team_wins, 0);
    }

    #[test]
    fn test_mixed_phases_on_same_day() {
        let b1 = ScheduleApiGameBuilder::new(1, 1, 4);
        let b2 = ScheduleApiGameBuilder::new(2, 1, 4);
        let schedule = vec![
            b1.game(1, "TPS", "HIFK", "2024-03-20T18:00:00Z", true, 3, 1),
            b2.game(2, "Lukko", "Ilves", "2024-03-20T18:00:00Z", true, 2, 1),
        ];

        let mut games = vec![
            make_game_data(
                "TPS",
                "HIFK",
                "3-1",
                "2024-03-20T18:00:00Z",
                "playoffs",
                1,
                1,
                4,
            ),
            make_game_data(
                "Lukko",
                "Ilves",
                "2-1",
                "2024-03-20T18:00:00Z",
                "playoffs",
                2,
                1,
                4,
            ),
        ];

        calculate_series_scores(&schedule, &mut games, "2024-12-31");

        let score0 = games[0].series_score.as_ref().unwrap();
        assert_eq!(score0.home_team_wins, 1);
        assert_eq!(score0.away_team_wins, 0);

        let score1 = games[1].series_score.as_ref().unwrap();
        assert_eq!(score1.home_team_wins, 1);
        assert_eq!(score1.away_team_wins, 0);
    }

    #[test]
    fn test_no_playoff_fields_skipped() {
        let schedule = vec![];
        let mut games = vec![GameData {
            home_team: "TPS".to_string(),
            away_team: "HIFK".to_string(),
            time: "18:30".to_string(),
            result: "3-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T18:30:00Z".to_string(),
            play_off_phase: None,
            play_off_pair: None,
            play_off_req_wins: None,
            series_score: None,
        }];

        calculate_series_scores(&schedule, &mut games, "2024-12-31");
        assert!(games[0].series_score.is_none());
    }

    #[test]
    fn test_series_counts_all_games_up_to_target_date() {
        // Schedule has 5 games across different dates (API returns all as ended).
        // Target date is 2024-03-24, so only games 1-3 should be counted.
        let b = ScheduleApiGameBuilder::new(1, 1, 4);
        let schedule = vec![
            b.game(1, "TPS", "HIFK", "2024-03-20T18:00:00Z", true, 3, 1), // TPS wins
            b.game(2, "HIFK", "TPS", "2024-03-22T18:00:00Z", true, 4, 2), // HIFK wins
            b.game(3, "TPS", "HIFK", "2024-03-24T18:00:00Z", true, 2, 1), // TPS wins
            b.game(4, "HIFK", "TPS", "2024-03-26T18:00:00Z", true, 1, 3), // future: TPS wins
            b.game(5, "TPS", "HIFK", "2024-03-28T18:00:00Z", true, 0, 2), // future: HIFK wins
        ];

        // Viewing date 2024-03-24: only game 3 is on this date
        let mut games = vec![make_game_data(
            "TPS",
            "HIFK",
            "2-1",
            "2024-03-24T18:00:00Z",
            "playoffs",
            1,
            1,
            4,
        )];

        calculate_series_scores(&schedule, &mut games, "2024-03-24");

        let score = games[0].series_score.as_ref().unwrap();
        // Only games 1-3 counted: TPS won games 1, 3 = 2 wins
        assert_eq!(score.home_team_wins, 2);
        // HIFK won game 2 = 1 win
        assert_eq!(score.away_team_wins, 1);
        assert_eq!(score.req_wins, 4);
    }

    #[test]
    fn test_different_tournaments_same_phase_pair_not_mixed() {
        // Playoffs (serie=1189) and playout (serie=2050) both have phase=1 pair=1
        // They should NOT share series scores
        let playoffs_builder = ScheduleApiGameBuilder::new(1, 1, 4).with_serie(1189);
        let playout_builder = ScheduleApiGameBuilder::new(1, 1, 4).with_serie(2050);

        let schedule = vec![
            // 3 completed playoff games
            playoffs_builder.game(1, "TPS", "HIFK", "2024-03-20T18:00:00Z", true, 3, 1),
            playoffs_builder.game(2, "HIFK", "TPS", "2024-03-22T18:00:00Z", true, 4, 2),
            playoffs_builder.game(3, "TPS", "HIFK", "2024-03-24T18:00:00Z", true, 2, 1),
            // 1 completed playout game (same phase=1, pair=1!)
            playout_builder.game(4, "Jukurit", "Pelicans", "2024-03-20T18:00:00Z", true, 3, 2),
            playout_builder.game(5, "Pelicans", "Jukurit", "2024-03-22T18:00:00Z", true, 1, 4),
        ];

        // Target: only the playout game on 2024-03-22
        let mut games = vec![make_game_data(
            "Pelicans",
            "Jukurit",
            "1-4",
            "2024-03-22T18:00:00Z",
            "playout",
            1,
            1,
            4,
        )];

        calculate_series_scores(&schedule, &mut games, "2024-12-31");

        let score = games[0].series_score.as_ref().unwrap();
        // Playout series only: Jukurit won both games
        assert_eq!(score.home_team_wins, 0); // Pelicans (home in this game) won 0
        assert_eq!(score.away_team_wins, 2); // Jukurit (away in this game) won 2
    }
}
