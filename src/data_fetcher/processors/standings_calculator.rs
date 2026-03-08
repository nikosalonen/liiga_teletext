use crate::data_fetcher::GameData;
use crate::data_fetcher::models::schedule::ScheduleGame;
use crate::data_fetcher::models::standings::StandingsEntry;
use crate::teletext_ui::ScoreType;
use std::collections::HashMap;

/// Intermediate accumulator for per-team stats
struct TeamStats {
    team_name: String,
    games_played: u16,
    wins: u16,
    ot_wins: u16,
    ot_losses: u16,
    losses: u16,
    goals_for: u16,
    goals_against: u16,
}

impl TeamStats {
    fn new(team_name: String) -> Self {
        Self {
            team_name,
            games_played: 0,
            wins: 0,
            ot_wins: 0,
            ot_losses: 0,
            losses: 0,
            goals_for: 0,
            goals_against: 0,
        }
    }

    fn points(&self) -> u16 {
        self.wins * 3 + self.ot_wins * 2 + self.ot_losses
    }
}

/// Determines whether a game ended in overtime/shootout based on finished_type
fn is_overtime_result(finished_type: &Option<String>) -> bool {
    matches!(
        finished_type.as_deref(),
        Some("ENDED_DURING_EXTENDED_GAME_TIME" | "ENDED_DURING_WINNING_SHOT_COMPETITION")
    )
}

/// Compute standings from completed schedule games
pub fn compute_standings(games: &[ScheduleGame]) -> Vec<StandingsEntry> {
    let mut stats: HashMap<String, TeamStats> = HashMap::new();

    for game in games {
        if !game.ended {
            continue;
        }

        // Only count runkosarja games for standings
        if game.serie != "runkosarja" {
            continue;
        }

        let home_id = game
            .home_team
            .team_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let away_id = game
            .away_team
            .team_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        let home_name = game
            .home_team
            .team_name
            .clone()
            .unwrap_or_else(|| home_id.clone());
        let away_name = game
            .away_team
            .team_name
            .clone()
            .unwrap_or_else(|| away_id.clone());

        let home_goals = game.home_team.goals;
        let away_goals = game.away_team.goals;
        let is_ot = is_overtime_result(&game.finished_type);

        let home_stats = stats
            .entry(home_id.clone())
            .or_insert_with(|| TeamStats::new(home_name));
        home_stats.games_played += 1;
        home_stats.goals_for += home_goals as u16;
        home_stats.goals_against += away_goals as u16;

        if home_goals > away_goals {
            if is_ot {
                home_stats.ot_wins += 1;
            } else {
                home_stats.wins += 1;
            }
        } else if is_ot {
            home_stats.ot_losses += 1;
        } else {
            home_stats.losses += 1;
        }

        let away_stats = stats
            .entry(away_id.clone())
            .or_insert_with(|| TeamStats::new(away_name));
        away_stats.games_played += 1;
        away_stats.goals_for += away_goals as u16;
        away_stats.goals_against += home_goals as u16;

        if away_goals > home_goals {
            if is_ot {
                away_stats.ot_wins += 1;
            } else {
                away_stats.wins += 1;
            }
        } else if is_ot {
            away_stats.ot_losses += 1;
        } else {
            away_stats.losses += 1;
        }
    }

    let mut entries: Vec<StandingsEntry> = stats
        .into_iter()
        .map(|(team_id, s)| {
            let points = s.points();
            StandingsEntry {
                team_name: s.team_name,
                team_id,
                games_played: s.games_played,
                wins: s.wins,
                ot_wins: s.ot_wins,
                ot_losses: s.ot_losses,
                losses: s.losses,
                goals_for: s.goals_for,
                goals_against: s.goals_against,
                points,
                live_points_delta: None,
                live_position_change: None,
            }
        })
        .collect();

    // Sort: points desc, goal diff desc, goals for desc
    entries.sort_by(|a, b| {
        b.points
            .cmp(&a.points)
            .then_with(|| b.goal_difference().cmp(&a.goal_difference()))
            .then_with(|| b.goals_for.cmp(&a.goals_for))
    });

    entries
}

/// Apply live game results to standings projections
pub fn apply_live_results(standings: &mut [StandingsEntry], live_games: &[GameData]) {
    // Record original positions
    let original_positions: HashMap<String, usize> = standings
        .iter()
        .enumerate()
        .map(|(i, e)| (e.team_id.clone(), i))
        .collect();

    for game in live_games {
        if game.score_type != ScoreType::Ongoing {
            continue;
        }

        // Parse score from result like "3-2"
        let scores: Vec<&str> = game.result.split('-').collect();
        if scores.len() != 2 {
            continue;
        }
        let home_goals: i32 = scores[0].trim().parse().unwrap_or(0);
        let away_goals: i32 = scores[1].trim().parse().unwrap_or(0);

        // Project points: assume regulation win for the leading team
        // If tied, assume OT result (winner gets 2, loser gets 1)
        let (home_delta, away_delta) = if home_goals > away_goals {
            (3i16, 0i16) // Home regulation win
        } else if away_goals > home_goals {
            (0i16, 3i16) // Away regulation win
        } else {
            (1i16, 1i16) // Tied - both could get at least 1 pt from OT
        };

        // Apply deltas to matching teams
        for entry in standings.iter_mut() {
            if entry.team_id == game.home_team || entry.team_name == game.home_team {
                entry.live_points_delta = Some(home_delta);
            } else if entry.team_id == game.away_team || entry.team_name == game.away_team {
                entry.live_points_delta = Some(away_delta);
            }
        }
    }

    // Re-sort with projected points to determine position changes
    let mut projected: Vec<(usize, u16, i16, u16, String)> = standings
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let projected_pts = e.points as i16 + e.live_points_delta.unwrap_or(0);
            (
                i,
                e.points,
                e.goal_difference(),
                projected_pts as u16,
                e.team_id.clone(),
            )
        })
        .collect();

    projected.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| b.2.cmp(&a.2)));

    // Determine position changes
    let new_positions: HashMap<String, usize> = projected
        .iter()
        .enumerate()
        .map(|(new_pos, (_, _, _, _, team_id))| (team_id.clone(), new_pos))
        .collect();

    for entry in standings.iter_mut() {
        if entry.live_points_delta.is_some()
            && let (Some(&orig), Some(&new_pos)) = (
                original_positions.get(&entry.team_id),
                new_positions.get(&entry.team_id),
            )
        {
            let change = orig as i16 - new_pos as i16; // positive = moved up
            if change != 0 {
                entry.live_position_change = Some(change);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::models::schedule::{ScheduleGame, ScheduleTeam};

    fn make_team(id: &str, name: &str, goals: i32) -> ScheduleTeam {
        ScheduleTeam {
            team_id: Some(id.to_string()),
            team_placeholder: None,
            team_name: Some(name.to_string()),
            goals,
            time_out: None,
            powerplay_instances: 0,
            powerplay_goals: 0,
            short_handed_instances: 0,
            short_handed_goals: 0,
            ranking: None,
            game_start_date_time: None,
            goal_events: vec![],
        }
    }

    fn make_game(
        home_id: &str,
        home_name: &str,
        home_goals: i32,
        away_id: &str,
        away_name: &str,
        away_goals: i32,
        finished_type: Option<&str>,
    ) -> ScheduleGame {
        ScheduleGame {
            id: 1,
            season: 2025,
            start: "2025-01-15T18:30:00Z".to_string(),
            end: Some("2025-01-15T21:00:00Z".to_string()),
            home_team: make_team(home_id, home_name, home_goals),
            away_team: make_team(away_id, away_name, away_goals),
            finished_type: finished_type.map(|s| s.to_string()),
            started: true,
            ended: true,
            game_time: 3600,
            serie: "runkosarja".to_string(),
        }
    }

    #[test]
    fn test_compute_standings_regulation_win() {
        let games = vec![make_game(
            "TAP",
            "Tappara",
            3,
            "HIFK",
            "HIFK Helsinki",
            1,
            Some("ENDED_DURING_REGULAR_TIME"),
        )];

        let standings = compute_standings(&games);
        assert_eq!(standings.len(), 2);

        // Tappara should be first with 3 points
        assert_eq!(standings[0].team_id, "TAP");
        assert_eq!(standings[0].points, 3);
        assert_eq!(standings[0].wins, 1);
        assert_eq!(standings[0].losses, 0);

        // HIFK should be second with 0 points
        assert_eq!(standings[1].team_id, "HIFK");
        assert_eq!(standings[1].points, 0);
        assert_eq!(standings[1].losses, 1);
    }

    #[test]
    fn test_compute_standings_overtime_win() {
        let games = vec![make_game(
            "TAP",
            "Tappara",
            3,
            "HIFK",
            "HIFK Helsinki",
            2,
            Some("ENDED_DURING_EXTENDED_GAME_TIME"),
        )];

        let standings = compute_standings(&games);

        // Tappara: OT win = 2 pts
        assert_eq!(standings[0].team_id, "TAP");
        assert_eq!(standings[0].points, 2);
        assert_eq!(standings[0].ot_wins, 1);

        // HIFK: OT loss = 1 pt
        assert_eq!(standings[1].team_id, "HIFK");
        assert_eq!(standings[1].points, 1);
        assert_eq!(standings[1].ot_losses, 1);
    }

    #[test]
    fn test_compute_standings_shootout_win() {
        let games = vec![make_game(
            "TAP",
            "Tappara",
            2,
            "HIFK",
            "HIFK Helsinki",
            3,
            Some("ENDED_DURING_WINNING_SHOT_COMPETITION"),
        )];

        let standings = compute_standings(&games);

        // HIFK: shootout win = 2 pts
        assert_eq!(standings[0].team_id, "HIFK");
        assert_eq!(standings[0].points, 2);
        assert_eq!(standings[0].ot_wins, 1);

        // Tappara: shootout loss = 1 pt
        assert_eq!(standings[1].team_id, "TAP");
        assert_eq!(standings[1].points, 1);
        assert_eq!(standings[1].ot_losses, 1);
    }

    #[test]
    fn test_compute_standings_skips_non_ended_games() {
        let mut game = make_game("TAP", "Tappara", 2, "HIFK", "HIFK Helsinki", 1, None);
        game.ended = false;

        let standings = compute_standings(&[game]);
        assert!(standings.is_empty());
    }

    #[test]
    fn test_compute_standings_skips_non_runkosarja() {
        let mut game = make_game(
            "TAP",
            "Tappara",
            3,
            "HIFK",
            "HIFK Helsinki",
            1,
            Some("ENDED_DURING_REGULAR_TIME"),
        );
        game.serie = "playoffs".to_string();

        let standings = compute_standings(&[game]);
        assert!(standings.is_empty());
    }

    #[test]
    fn test_standings_sorting() {
        let games = vec![
            make_game(
                "A",
                "Team A",
                5,
                "B",
                "Team B",
                1,
                Some("ENDED_DURING_REGULAR_TIME"),
            ),
            make_game(
                "C",
                "Team C",
                4,
                "A",
                "Team A",
                3,
                Some("ENDED_DURING_REGULAR_TIME"),
            ),
        ];

        let standings = compute_standings(&games);

        // A: 1 win (3pts) + 1 loss (0pts) = 3 pts, GF=8, GA=5, GD=+3
        // C: 1 win = 3 pts, GF=4, GA=3, GD=+1
        // B: 0 pts
        // Both A and C have 3 pts, but A has better GD
        assert_eq!(standings[0].team_id, "A");
        assert_eq!(standings[1].team_id, "C");
        assert_eq!(standings[2].team_id, "B");
    }

    #[test]
    fn test_apply_live_results() {
        let games = vec![
            make_game(
                "A",
                "Team A",
                3,
                "B",
                "Team B",
                1,
                Some("ENDED_DURING_REGULAR_TIME"),
            ),
            make_game(
                "C",
                "Team C",
                2,
                "D",
                "Team D",
                1,
                Some("ENDED_DURING_REGULAR_TIME"),
            ),
        ];

        let mut standings = compute_standings(&games);
        assert_eq!(standings.len(), 4);

        // Simulate a live game where B is beating C
        let live_games = vec![GameData {
            home_team: "B".to_string(),
            away_team: "C".to_string(),
            time: "".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Ongoing,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 2400,
            start: "2025-01-16T18:30:00Z".to_string(),
        }];

        apply_live_results(&mut standings, &live_games);

        // B should get +3 projected
        let b = standings.iter().find(|e| e.team_id == "B").unwrap();
        assert_eq!(b.live_points_delta, Some(3));

        // C should get +0 projected
        let c = standings.iter().find(|e| e.team_id == "C").unwrap();
        assert_eq!(c.live_points_delta, Some(0));
    }
}
