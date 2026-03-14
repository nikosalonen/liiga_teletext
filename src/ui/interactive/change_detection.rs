//! Change detection utilities for game and standings data updates.
//!
//! This module provides efficient change detection to avoid unnecessary UI updates
//! by computing hashes of game and standings data and comparing them across refreshes.

use crate::data_fetcher::models::standings::StandingsEntry;
use crate::data_fetcher::{GameData, has_live_games_from_game_data};
use crate::teletext_ui::ScoreType;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Calculates a hash of the games data for change detection
/// Optimized to focus on essential fields that indicate meaningful changes
pub(super) fn calculate_games_hash(games: &[GameData]) -> u64 {
    let mut hasher = DefaultHasher::new();

    for game in games {
        game.home_team.hash(&mut hasher);
        game.away_team.hash(&mut hasher);
        game.result.hash(&mut hasher);
        game.time.hash(&mut hasher);
        game.score_type.hash(&mut hasher);
        game.is_overtime.hash(&mut hasher);
        game.is_shootout.hash(&mut hasher);
        game.serie.hash(&mut hasher);
        game.played_time.hash(&mut hasher);
        game.start.hash(&mut hasher);
        game.play_off_phase.hash(&mut hasher);
        game.play_off_pair.hash(&mut hasher);
        game.play_off_req_wins.hash(&mut hasher);
        if let Some(ref score) = game.series_score {
            score.home_team_wins.hash(&mut hasher);
            score.away_team_wins.hash(&mut hasher);
            score.req_wins.hash(&mut hasher);
        }

        // Hash only essential goal event fields for efficient change detection
        // These fields capture the most important changes: new goals, score updates, and timing
        for goal in &game.goal_events {
            goal.scorer_player_id.hash(&mut hasher);
            goal.minute.hash(&mut hasher);
            goal.home_team_score.hash(&mut hasher);
            goal.away_team_score.hash(&mut hasher);
            goal.video_clip_url.hash(&mut hasher);
        }
    }

    hasher.finish()
}

/// Calculates a hash of standings data for change detection.
/// Includes `live_mode` so toggling it always triggers a page rebuild
/// (subheader and footer change even when the underlying data is identical).
pub(super) fn calculate_standings_hash(
    standings: &[StandingsEntry],
    playoffs_lines: &[u16],
    live_mode: bool,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    live_mode.hash(&mut hasher);
    for entry in standings {
        entry.hash(&mut hasher);
    }
    playoffs_lines.hash(&mut hasher);
    hasher.finish()
}

/// Helper function to check if a game is in the future
fn is_future_game(game: &GameData) -> bool {
    game.score_type == ScoreType::Scheduled
}

/// Performs change detection and logs detailed information about changes
pub(super) fn detect_and_log_changes(games: &[GameData], last_games: &[GameData]) -> bool {
    let games_hash = calculate_games_hash(games);
    let last_games_hash = calculate_games_hash(last_games);
    let data_changed = games_hash != last_games_hash;

    if data_changed {
        tracing::debug!("Data changed, updating UI");

        // Log specific changes for live games to help debug game clock updates
        if !last_games.is_empty() && games.len() == last_games.len() {
            for (i, (new_game, old_game)) in games.iter().zip(last_games.iter()).enumerate() {
                if new_game.played_time != old_game.played_time
                    && new_game.score_type == ScoreType::Ongoing
                {
                    tracing::info!(
                        "Game clock update detected: Game {} - {} vs {} - time changed from {}s to {}s",
                        i + 1,
                        new_game.home_team,
                        new_game.away_team,
                        old_game.played_time,
                        new_game.played_time
                    );
                }
            }
        }
    } else {
        // Track ongoing games with static time to confirm API limitations
        let ongoing_games: Vec<_> = games
            .iter()
            .enumerate()
            .filter(|(_, game)| game.score_type == ScoreType::Ongoing)
            .collect();

        if !ongoing_games.is_empty() {
            tracing::debug!(
                "No data changes detected despite {} ongoing game(s): {}",
                ongoing_games.len(),
                ongoing_games
                    .iter()
                    .map(|(i, game)| format!(
                        "{}. {} vs {} ({}s)",
                        i + 1,
                        game.home_team,
                        game.away_team,
                        game.played_time
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        // Check if all games are scheduled (future games) - only relevant if no ongoing games
        let has_ongoing_games = has_live_games_from_game_data(games);
        let all_scheduled = !games.is_empty() && games.iter().all(is_future_game);

        if all_scheduled && !has_ongoing_games {
            tracing::info!("All games are scheduled - auto-refresh disabled");
        } else if has_ongoing_games {
            tracing::info!("Ongoing games detected - auto-refresh enabled");
        }

        tracing::debug!("No data changes detected, skipping UI update");
    }

    data_changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_game(home: &str, away: &str, result: &str, serie: &str) -> GameData {
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
            start: "2024-01-15T18:30:00Z".to_string(),
            play_off_phase: None,
            play_off_pair: None,
            play_off_req_wins: None,
            series_score: None,
        }
    }

    fn make_standings_entry(team: &str, points: u16) -> StandingsEntry {
        StandingsEntry {
            team_name: team.to_string(),
            team_id: team.to_string(),
            games_played: 40,
            wins: 20,
            ot_wins: 5,
            ot_losses: 5,
            losses: 10,
            goals_for: 100,
            goals_against: 80,
            points,
            live_goals_for: 100,
            live_goals_against: 80,
            live_points_delta: None,
            live_position_change: None,
            live_game_active: false,
        }
    }

    #[test]
    fn test_calculate_standings_hash_empty() {
        let hash = calculate_standings_hash(&[], &[], false);
        let hash2 = calculate_standings_hash(&[], &[], false);
        assert_eq!(
            hash, hash2,
            "Empty standings should produce deterministic hash"
        );
    }

    #[test]
    fn test_calculate_standings_hash_deterministic() {
        let standings = vec![make_standings_entry("TPS", 60)];
        let playoffs = vec![6u16, 10];

        let hash1 = calculate_standings_hash(&standings, &playoffs, false);
        let hash2 = calculate_standings_hash(&standings, &playoffs, false);
        assert_eq!(hash1, hash2, "Same data should produce same hash");
    }

    #[test]
    fn test_calculate_standings_hash_sensitive_to_data_changes() {
        let standings1 = vec![make_standings_entry("TPS", 60)];
        let standings2 = vec![make_standings_entry("TPS", 63)];

        let hash1 = calculate_standings_hash(&standings1, &[], false);
        let hash2 = calculate_standings_hash(&standings2, &[], false);
        assert_ne!(
            hash1, hash2,
            "Different points should produce different hash"
        );
    }

    #[test]
    fn test_calculate_standings_hash_live_mode_toggle() {
        let standings = vec![make_standings_entry("TPS", 60)];

        let hash_off = calculate_standings_hash(&standings, &[], false);
        let hash_on = calculate_standings_hash(&standings, &[], true);
        assert_ne!(
            hash_off, hash_on,
            "Toggling live_mode must produce different hash"
        );
    }

    #[test]
    fn test_calculate_standings_hash_sensitive_to_playoffs_lines() {
        let standings = vec![make_standings_entry("TPS", 60)];

        let hash1 = calculate_standings_hash(&standings, &[6, 10], false);
        let hash2 = calculate_standings_hash(&standings, &[6, 12], false);
        assert_ne!(
            hash1, hash2,
            "Different playoffs_lines should produce different hash"
        );
    }

    #[test]
    fn test_calculate_standings_hash_sensitive_to_live_game_active() {
        let entry1 = make_standings_entry("TPS", 60);
        let mut entry2 = make_standings_entry("TPS", 60);
        entry2.live_game_active = true;
        entry2.live_points_delta = Some(0);

        let hash1 = calculate_standings_hash(&[entry1], &[], true);
        let hash2 = calculate_standings_hash(&[entry2], &[], true);
        assert_ne!(
            hash1, hash2,
            "live_game_active change should produce different hash"
        );
    }

    #[test]
    fn test_calculate_games_hash_empty() {
        let games: Vec<GameData> = vec![];
        let hash = calculate_games_hash(&games);
        // Should return a deterministic hash for empty data
        assert!(hash > 0);
    }

    #[test]
    fn test_calculate_games_hash() {
        let games1 = vec![make_game("TPS", "HIFK", "3-2", "runkosarja")];
        let games2 = vec![make_game("TPS", "HIFK", "3-2", "runkosarja")];

        let hash1 = calculate_games_hash(&games1);
        let hash2 = calculate_games_hash(&games2);

        // Same data should produce same hash
        assert_eq!(hash1, hash2);

        // Different data should produce different hash
        let games3 = vec![make_game("TPS", "HIFK", "4-2", "runkosarja")];
        let hash3 = calculate_games_hash(&games3);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_series_score_change_detected() {
        use crate::data_fetcher::models::PlayoffSeriesScore;

        let mut game1 = make_game("TPS", "HIFK", "3-2", "playoffs");
        let mut game2 = game1.clone();

        game1.series_score = None;
        game2.series_score = Some(PlayoffSeriesScore {
            home_team_wins: 2,
            away_team_wins: 1,
            req_wins: 4,
        });

        let hash1 = calculate_games_hash(&[game1]);
        let hash2 = calculate_games_hash(&[game2]);
        assert_ne!(
            hash1, hash2,
            "series_score change should produce different hash"
        );
    }

    #[test]
    fn test_video_clip_url_change_detected() {
        use crate::data_fetcher::GoalEventData;

        let mut game1 = make_game("TPS", "HIFK", "3-2", "runkosarja");
        let mut game2 = game1.clone();

        let goal = GoalEventData {
            scorer_player_id: 1,
            scorer_name: "Test".to_string(),
            minute: 10,
            home_team_score: 1,
            away_team_score: 0,
            is_winning_goal: false,
            goal_types: vec![],
            is_home_team: true,
            video_clip_url: None,
        };

        game1.goal_events = vec![goal.clone()];
        let mut goal_with_video = goal;
        goal_with_video.video_clip_url = Some("https://example.com/video.mp4".to_string());
        game2.goal_events = vec![goal_with_video];

        let hash1 = calculate_games_hash(&[game1]);
        let hash2 = calculate_games_hash(&[game2]);
        assert_ne!(
            hash1, hash2,
            "video_clip_url change should produce different hash"
        );
    }
}
