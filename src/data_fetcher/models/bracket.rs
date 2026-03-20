use std::hash::Hash;

/// A single playoff series between two teams
#[derive(Debug, Clone, Hash)]
pub struct BracketMatchup {
    pub phase: i32,
    pub pair: i32,
    pub serie: i32,
    pub team1: String,
    pub team2: String,
    pub team1_wins: u8,
    pub team2_wins: u8,
    pub req_wins: u8,
    pub is_decided: bool,
    pub has_live_game: bool,
    pub winner: Option<String>,
}

/// All matchups in a single playoff round
#[derive(Debug, Clone, Hash)]
pub struct BracketPhase {
    pub phase_number: i32,
    pub name: String,
    pub matchups: Vec<BracketMatchup>,
}

/// The complete playoff bracket for a season
#[derive(Debug, Clone, Hash)]
pub struct PlayoffBracket {
    pub season: String,
    pub phases: Vec<BracketPhase>,
    pub has_data: bool,
}

use crate::data_fetcher::models::schedule::ScheduleApiGame;
use crate::ui::interactive::series_utils::playoff_phase_name;
use std::collections::HashMap;

/// Builds a playoff bracket from schedule API games.
///
/// Groups games by (serie, phase, pair), counts wins per team,
/// determines series state, and orders into phases.
pub fn build_playoff_bracket(games: &[ScheduleApiGame], season: &str) -> PlayoffBracket {
    let playoff_games: Vec<&ScheduleApiGame> = games
        .iter()
        .filter(|g| {
            g.play_off_phase.is_some()
                && !g.home_team_name.is_empty()
                && !g.away_team_name.is_empty()
                && !g.start.is_empty()
        })
        .collect();

    if playoff_games.is_empty() {
        return PlayoffBracket {
            season: season.to_string(),
            phases: vec![],
            has_data: false,
        };
    }

    let mut series_map: HashMap<(i32, i32, i32), Vec<&ScheduleApiGame>> = HashMap::new();
    for game in &playoff_games {
        let key = (
            game.serie,
            game.play_off_phase.unwrap_or(0),
            game.play_off_pair.unwrap_or(0),
        );
        series_map.entry(key).or_default().push(game);
    }

    let mut phase_map: HashMap<i32, Vec<BracketMatchup>> = HashMap::new();

    for ((serie, phase, pair), series_games) in &series_map {
        let mut sorted_games = series_games.clone();
        sorted_games.sort_by(|a, b| a.start.cmp(&b.start));

        let first_game = sorted_games[0];
        let team1 = first_game.home_team_name.clone();

        let team2 = sorted_games
            .iter()
            .find_map(|g| {
                if g.home_team_name != team1 {
                    Some(g.home_team_name.clone())
                } else if g.away_team_name != team1 {
                    Some(g.away_team_name.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| first_game.away_team_name.clone());

        let mut team1_wins: u8 = 0;
        let mut team2_wins: u8 = 0;
        let mut has_live_game = false;

        for game in &sorted_games {
            if game.started && !game.ended {
                has_live_game = true;
            }
            if game.ended {
                // In playoffs, games cannot end in a tie (OT/SO always decides).
                // If scores are equal, the API data may be incomplete — skip.
                if game.home_team_goals == game.away_team_goals {
                    continue;
                }
                let home_won = game.home_team_goals > game.away_team_goals;
                if (home_won && game.home_team_name == team1)
                    || (!home_won && game.away_team_name == team1)
                {
                    team1_wins += 1;
                } else {
                    team2_wins += 1;
                }
            }
        }

        let req_wins = first_game.play_off_req_wins.map(|w| w as u8).unwrap_or(4);

        let is_decided = team1_wins >= req_wins || team2_wins >= req_wins;
        let winner = if is_decided {
            if team1_wins >= req_wins {
                Some(team1.clone())
            } else {
                Some(team2.clone())
            }
        } else {
            None
        };

        let matchup = BracketMatchup {
            phase: *phase,
            pair: *pair,
            serie: *serie,
            team1,
            team2,
            team1_wins,
            team2_wins,
            req_wins,
            is_decided,
            has_live_game,
            winner,
        };

        phase_map.entry(*phase).or_default().push(matchup);
    }

    let mut phases: Vec<BracketPhase> = phase_map
        .into_iter()
        .map(|(phase_number, mut matchups)| {
            matchups.sort_by_key(|m| m.pair);
            BracketPhase {
                phase_number,
                name: playoff_phase_name(phase_number, "playoffs").to_string(),
                matchups,
            }
        })
        .collect();

    phases.sort_by_key(|p| p.phase_number);

    PlayoffBracket {
        season: season.to_string(),
        phases,
        has_data: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::models::schedule::ScheduleApiGame;

    #[allow(clippy::too_many_arguments)]
    fn make_playoff_game(
        id: i32,
        home: &str,
        away: &str,
        home_goals: i32,
        away_goals: i32,
        phase: i32,
        pair: i32,
        serie: i32,
        started: bool,
        ended: bool,
        start: &str,
    ) -> ScheduleApiGame {
        ScheduleApiGame {
            id,
            season: 2026,
            start: start.to_string(),
            home_team_name: home.to_string(),
            away_team_name: away.to_string(),
            serie,
            finished_type: if ended {
                Some("ENDED".to_string())
            } else {
                None
            },
            started,
            ended,
            game_time: None,
            play_off_phase: Some(phase),
            play_off_pair: Some(pair),
            play_off_req_wins: Some(4),
            home_team_goals: home_goals,
            away_team_goals: away_goals,
        }
    }

    #[test]
    fn test_empty_schedule_returns_no_data() {
        let bracket = build_playoff_bracket(&[], "2025-2026");
        assert!(!bracket.has_data);
        assert!(bracket.phases.is_empty());
    }

    #[test]
    fn test_non_playoff_games_ignored() {
        let games = vec![ScheduleApiGame {
            id: 1,
            season: 2026,
            start: "2026-03-15T18:00:00Z".to_string(),
            home_team_name: "HIFK".to_string(),
            away_team_name: "TPS".to_string(),
            serie: 1,
            finished_type: Some("ENDED".to_string()),
            started: true,
            ended: true,
            game_time: None,
            play_off_phase: None,
            play_off_pair: None,
            play_off_req_wins: None,
            home_team_goals: 3,
            away_team_goals: 1,
        }];
        let bracket = build_playoff_bracket(&games, "2025-2026");
        assert!(!bracket.has_data);
    }

    #[test]
    fn test_single_phase_quarterfinals() {
        let games = vec![
            make_playoff_game(
                1,
                "HIFK",
                "TPS",
                3,
                1,
                2,
                1,
                2,
                true,
                true,
                "2026-03-15T18:00:00Z",
            ),
            make_playoff_game(
                2,
                "TPS",
                "HIFK",
                2,
                4,
                2,
                1,
                2,
                true,
                true,
                "2026-03-17T18:00:00Z",
            ),
            make_playoff_game(
                3,
                "HIFK",
                "TPS",
                5,
                2,
                2,
                1,
                2,
                true,
                true,
                "2026-03-19T18:00:00Z",
            ),
            make_playoff_game(
                4,
                "TPS",
                "HIFK",
                1,
                3,
                2,
                1,
                2,
                true,
                true,
                "2026-03-21T18:00:00Z",
            ),
        ];
        let bracket = build_playoff_bracket(&games, "2025-2026");
        assert!(bracket.has_data);
        assert_eq!(bracket.phases.len(), 1);
        assert_eq!(bracket.phases[0].phase_number, 2);
        assert_eq!(bracket.phases[0].matchups.len(), 1);
        let m = &bracket.phases[0].matchups[0];
        assert_eq!(m.team1, "HIFK");
        assert_eq!(m.team2, "TPS");
        assert_eq!(m.team1_wins, 4);
        assert_eq!(m.team2_wins, 0);
        assert!(m.is_decided);
        assert_eq!(m.winner.as_deref(), Some("HIFK"));
    }

    #[test]
    fn test_zero_zero_series() {
        let games = vec![make_playoff_game(
            1,
            "HIFK",
            "TPS",
            0,
            0,
            2,
            1,
            2,
            false,
            false,
            "2026-03-25T18:00:00Z",
        )];
        let bracket = build_playoff_bracket(&games, "2025-2026");
        assert!(bracket.has_data);
        let m = &bracket.phases[0].matchups[0];
        assert_eq!(m.team1_wins, 0);
        assert_eq!(m.team2_wins, 0);
        assert!(!m.is_decided);
        assert!(m.winner.is_none());
    }

    #[test]
    fn test_has_live_game() {
        let games = vec![make_playoff_game(
            1,
            "HIFK",
            "TPS",
            2,
            1,
            2,
            1,
            2,
            true,
            false,
            "2026-03-15T18:00:00Z",
        )];
        let bracket = build_playoff_bracket(&games, "2025-2026");
        assert!(bracket.phases[0].matchups[0].has_live_game);
    }

    #[test]
    fn test_different_serie_ids_not_mixed() {
        let games = vec![
            make_playoff_game(
                1,
                "HIFK",
                "TPS",
                3,
                1,
                2,
                1,
                2,
                true,
                true,
                "2026-03-15T18:00:00Z",
            ),
            make_playoff_game(
                2,
                "Lukko",
                "KooKoo",
                2,
                3,
                2,
                1,
                3,
                true,
                true,
                "2026-03-15T18:00:00Z",
            ),
        ];
        let bracket = build_playoff_bracket(&games, "2025-2026");
        let total_matchups: usize = bracket.phases.iter().map(|p| p.matchups.len()).sum();
        assert_eq!(total_matchups, 2);
    }

    #[test]
    fn test_req_wins_defaults_to_4() {
        let mut game = make_playoff_game(
            1,
            "HIFK",
            "TPS",
            0,
            0,
            2,
            1,
            2,
            false,
            false,
            "2026-03-15T18:00:00Z",
        );
        game.play_off_req_wins = None;
        let bracket = build_playoff_bracket(&[game], "2025-2026");
        assert_eq!(bracket.phases[0].matchups[0].req_wins, 4);
    }

    #[test]
    fn test_bronze_match_req_wins_1() {
        let mut game = make_playoff_game(
            1,
            "HIFK",
            "TPS",
            3,
            1,
            4,
            1,
            2,
            true,
            true,
            "2026-03-15T18:00:00Z",
        );
        game.play_off_req_wins = Some(1);
        let bracket = build_playoff_bracket(&[game], "2025-2026");
        let m = &bracket.phases[0].matchups[0];
        assert_eq!(m.req_wins, 1);
        assert!(m.is_decided);
        assert_eq!(m.winner.as_deref(), Some("HIFK"));
    }

    #[test]
    fn test_phases_ordered_by_number() {
        let games = vec![
            make_playoff_game(
                1,
                "HIFK",
                "TPS",
                3,
                1,
                3,
                1,
                2,
                true,
                true,
                "2026-03-15T18:00:00Z",
            ),
            make_playoff_game(
                2,
                "Lukko",
                "KooKoo",
                2,
                3,
                2,
                1,
                2,
                true,
                true,
                "2026-03-10T18:00:00Z",
            ),
        ];
        let bracket = build_playoff_bracket(&games, "2025-2026");
        assert_eq!(bracket.phases[0].phase_number, 2);
        assert_eq!(bracket.phases[1].phase_number, 3);
    }

    #[test]
    fn test_full_bracket_qf_sf_final() {
        let games = vec![
            make_playoff_game(
                1,
                "HIFK",
                "TPS",
                3,
                1,
                2,
                1,
                2,
                true,
                true,
                "2026-03-10T18:00:00Z",
            ),
            make_playoff_game(
                2,
                "TPS",
                "HIFK",
                2,
                4,
                2,
                1,
                2,
                true,
                true,
                "2026-03-12T18:00:00Z",
            ),
            make_playoff_game(
                3,
                "HIFK",
                "TPS",
                5,
                2,
                2,
                1,
                2,
                true,
                true,
                "2026-03-14T18:00:00Z",
            ),
            make_playoff_game(
                4,
                "TPS",
                "HIFK",
                3,
                1,
                2,
                1,
                2,
                true,
                true,
                "2026-03-16T18:00:00Z",
            ),
            make_playoff_game(
                5,
                "HIFK",
                "TPS",
                4,
                2,
                2,
                1,
                2,
                true,
                true,
                "2026-03-18T18:00:00Z",
            ),
            make_playoff_game(
                6,
                "TPS",
                "HIFK",
                1,
                3,
                2,
                1,
                2,
                true,
                true,
                "2026-03-20T18:00:00Z",
            ),
            make_playoff_game(
                7,
                "Lukko",
                "KooKoo",
                3,
                1,
                2,
                2,
                2,
                true,
                true,
                "2026-03-10T18:00:00Z",
            ),
            make_playoff_game(
                8,
                "KooKoo",
                "Lukko",
                2,
                4,
                2,
                2,
                2,
                true,
                true,
                "2026-03-12T18:00:00Z",
            ),
            make_playoff_game(
                9,
                "Lukko",
                "KooKoo",
                5,
                0,
                2,
                2,
                2,
                true,
                true,
                "2026-03-14T18:00:00Z",
            ),
            make_playoff_game(
                10,
                "KooKoo",
                "Lukko",
                3,
                2,
                2,
                2,
                2,
                true,
                true,
                "2026-03-16T18:00:00Z",
            ),
            make_playoff_game(
                11,
                "Lukko",
                "KooKoo",
                4,
                1,
                2,
                2,
                2,
                true,
                true,
                "2026-03-18T18:00:00Z",
            ),
            make_playoff_game(
                12,
                "HIFK",
                "Lukko",
                3,
                1,
                3,
                1,
                2,
                true,
                true,
                "2026-03-25T18:00:00Z",
            ),
            make_playoff_game(
                13,
                "Lukko",
                "HIFK",
                4,
                2,
                3,
                1,
                2,
                true,
                true,
                "2026-03-27T18:00:00Z",
            ),
            make_playoff_game(
                14,
                "HIFK",
                "Lukko",
                2,
                1,
                3,
                1,
                2,
                true,
                true,
                "2026-03-29T18:00:00Z",
            ),
            make_playoff_game(
                15,
                "HIFK",
                "Tappara",
                0,
                0,
                5,
                1,
                2,
                false,
                false,
                "2026-04-15T18:00:00Z",
            ),
        ];
        let bracket = build_playoff_bracket(&games, "2025-2026");
        assert!(bracket.has_data);
        assert_eq!(bracket.phases.len(), 3);
        assert_eq!(bracket.phases[0].phase_number, 2);
        assert_eq!(bracket.phases[0].matchups.len(), 2);
        assert_eq!(bracket.phases[1].phase_number, 3);
        assert_eq!(bracket.phases[1].matchups.len(), 1);
        assert_eq!(bracket.phases[2].phase_number, 5);
        assert!(!bracket.phases[1].matchups[0].is_decided);
        assert_eq!(bracket.phases[1].matchups[0].team1_wins, 2);
        assert_eq!(bracket.phases[1].matchups[0].team2_wins, 1);
    }

    #[test]
    fn test_first_round_included() {
        let games = vec![
            make_playoff_game(
                1,
                "Team7",
                "Team10",
                4,
                1,
                1,
                1,
                2,
                true,
                true,
                "2026-03-05T18:00:00Z",
            ),
            make_playoff_game(
                2,
                "Team10",
                "Team7",
                2,
                3,
                1,
                1,
                2,
                true,
                true,
                "2026-03-07T18:00:00Z",
            ),
            make_playoff_game(
                3,
                "HIFK",
                "Team7",
                3,
                1,
                2,
                1,
                2,
                true,
                true,
                "2026-03-15T18:00:00Z",
            ),
        ];
        let bracket = build_playoff_bracket(&games, "2025-2026");
        assert!(bracket.has_data);
        assert_eq!(bracket.phases.len(), 2);
        assert_eq!(bracket.phases[0].phase_number, 1);
        assert_eq!(bracket.phases[0].name, "1. KIERROS");
        assert_eq!(bracket.phases[1].phase_number, 2);
    }
}
