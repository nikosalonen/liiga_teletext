use crate::data_fetcher::models::{GameData, PlayoffSeriesScore, ScheduleApiGame};
use std::collections::HashMap;

/// Calculates series scores for playoff games based on the full tournament schedule.
///
/// Groups games by (play_off_phase, play_off_pair), counts wins per team from
/// completed games, and sets the series_score on matching target_games.
pub fn calculate_series_scores(
    all_schedule_games: &[ScheduleApiGame],
    target_games: &mut [GameData],
) {
    let mut series_wins: HashMap<(i32, i32), HashMap<String, u8>> = HashMap::new();
    let mut series_req_wins: HashMap<(i32, i32), u8> = HashMap::new();

    // Build a lookup of game results from target_games (which have parsed scores)
    let target_results: HashMap<String, (i32, i32)> = target_games
        .iter()
        .filter_map(|g| {
            let (home_goals, away_goals) = parse_result(&g.result)?;
            let key = format!("{}_{}", g.home_team, g.start);
            Some((key, (home_goals, away_goals)))
        })
        .collect();

    // For each completed schedule game, determine the winner and count wins
    for sched_game in all_schedule_games {
        let (Some(phase), Some(pair)) = (sched_game.play_off_phase, sched_game.play_off_pair)
        else {
            continue;
        };
        if !sched_game.ended {
            continue;
        }

        if let Some(req_wins) = sched_game.play_off_req_wins {
            series_req_wins.insert((phase, pair), req_wins as u8);
        }

        // Match this schedule game to a target game by home_team + start
        let key = format!("{}_{}", sched_game.home_team_name, sched_game.start);
        if let Some(&(home_goals, away_goals)) = target_results.get(&key) {
            let winner = if home_goals > away_goals {
                &sched_game.home_team_name
            } else {
                &sched_game.away_team_name
            };
            *series_wins
                .entry((phase, pair))
                .or_default()
                .entry(winner.to_string())
                .or_insert(0) += 1;
        }
    }

    // Set series_score on each target game that has playoff fields
    for game in target_games.iter_mut() {
        let (Some(phase), Some(pair)) = (game.play_off_phase, game.play_off_pair) else {
            continue;
        };

        let req_wins = game
            .play_off_req_wins
            .map(|r| r as u8)
            .or_else(|| series_req_wins.get(&(phase, pair)).copied())
            .unwrap_or(4);

        if let Some(wins) = series_wins.get(&(phase, pair)) {
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

fn parse_result(result: &str) -> Option<(i32, i32)> {
    let (home, away) = result.split_once('-')?;
    let home = home.trim().parse::<i32>().ok()?;
    let away = away.trim().parse::<i32>().ok()?;
    Some((home, away))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::teletext_ui::ScoreType;

    struct ScheduleApiGameBuilder {
        phase: i32,
        pair: i32,
        req_wins: i32,
    }

    impl ScheduleApiGameBuilder {
        fn new(phase: i32, pair: i32, req_wins: i32) -> Self {
            Self {
                phase,
                pair,
                req_wins,
            }
        }

        fn game(
            &self,
            id: i32,
            home: &str,
            away: &str,
            start: &str,
            ended: bool,
        ) -> ScheduleApiGame {
            ScheduleApiGame {
                id,
                season: 2024,
                start: start.to_string(),
                home_team_name: home.to_string(),
                away_team_name: away.to_string(),
                serie: 5,
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
            }
        }
    }

    fn make_game_data(
        home: &str,
        away: &str,
        result: &str,
        start: &str,
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
            serie: "playoffs".to_string(),
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
            b.game(1, "TPS", "HIFK", "2024-03-20T18:00:00Z", true),
            b.game(2, "HIFK", "TPS", "2024-03-22T18:00:00Z", true),
            b.game(3, "TPS", "HIFK", "2024-03-24T18:00:00Z", true),
        ];

        let mut games = vec![
            make_game_data("TPS", "HIFK", "3-1", "2024-03-20T18:00:00Z", 1, 1, 4),
            make_game_data("HIFK", "TPS", "4-2", "2024-03-22T18:00:00Z", 1, 1, 4),
            make_game_data("TPS", "HIFK", "2-1", "2024-03-24T18:00:00Z", 1, 1, 4),
        ];

        calculate_series_scores(&schedule, &mut games);

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
            b.game(1, "TPS", "HIFK", "2024-03-20T18:00:00Z", true),
            b.game(2, "HIFK", "TPS", "2024-03-22T18:00:00Z", true),
            b.game(3, "TPS", "HIFK", "2024-03-24T18:00:00Z", true),
            b.game(4, "HIFK", "TPS", "2024-03-26T18:00:00Z", true),
        ];

        let mut games = vec![
            make_game_data("TPS", "HIFK", "3-1", "2024-03-20T18:00:00Z", 1, 1, 4),
            make_game_data("HIFK", "TPS", "1-4", "2024-03-22T18:00:00Z", 1, 1, 4),
            make_game_data("TPS", "HIFK", "2-1", "2024-03-24T18:00:00Z", 1, 1, 4),
            make_game_data("HIFK", "TPS", "0-3", "2024-03-26T18:00:00Z", 1, 1, 4),
        ];

        calculate_series_scores(&schedule, &mut games);

        let score = games[0].series_score.as_ref().unwrap();
        assert_eq!(score.home_team_wins, 4); // TPS
        assert_eq!(score.away_team_wins, 0); // HIFK
        assert_eq!(score.req_wins, 4);
    }

    #[test]
    fn test_bronze_game_req_wins_1() {
        let b = ScheduleApiGameBuilder::new(4, 1, 1);
        let schedule = vec![b.game(1, "TPS", "HIFK", "2024-04-01T18:00:00Z", true)];
        let mut games = vec![make_game_data(
            "TPS",
            "HIFK",
            "3-2",
            "2024-04-01T18:00:00Z",
            4,
            1,
            1,
        )];

        calculate_series_scores(&schedule, &mut games);

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
            b1.game(1, "TPS", "HIFK", "2024-03-20T18:00:00Z", true),
            b2.game(2, "Lukko", "Ilves", "2024-03-20T18:00:00Z", true),
        ];

        let mut games = vec![
            make_game_data("TPS", "HIFK", "3-1", "2024-03-20T18:00:00Z", 1, 1, 4),
            make_game_data("Lukko", "Ilves", "2-1", "2024-03-20T18:00:00Z", 2, 1, 4),
        ];

        calculate_series_scores(&schedule, &mut games);

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

        calculate_series_scores(&schedule, &mut games);
        assert!(games[0].series_score.is_none());
    }
}
