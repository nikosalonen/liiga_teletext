//! Change detection utilities for game data updates.
//!
//! This module provides efficient change detection to avoid unnecessary UI updates
//! by computing hashes of game data and comparing them across refreshes.

use crate::data_fetcher::{GameData, cache::has_live_games_from_game_data};
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

        // Hash only essential goal event fields for efficient change detection
        // These fields capture the most important changes: new goals, score updates, and timing
        for goal in &game.goal_events {
            goal.scorer_player_id.hash(&mut hasher);
            goal.minute.hash(&mut hasher);
            goal.home_team_score.hash(&mut hasher);
            goal.away_team_score.hash(&mut hasher);
            // Omitted fields for performance:
            // - scorer_name: derived from scorer_player_id via players cache
            // - is_winning_goal: calculated field, can be derived
            // - is_home_team: derived from team comparison
            // - goal_types: less critical for change detection, rarely updated
        }
    }

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
        let all_scheduled = !games.is_empty() && games.iter().all(|g| is_future_game(g));

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
    // TODO: Fix testing_utils import issue
    // use super::super::super::testing_utils::create_basic_game;

    #[test]
    fn test_calculate_games_hash_empty() {
        let games: Vec<GameData> = vec![];
        let hash = calculate_games_hash(&games);
        // Should return a deterministic hash for empty data
        assert!(hash > 0);
    }

    // TODO: Re-enable when testing_utils import issue is resolved
    // #[test]
    // fn test_calculate_games_hash() {
    //     let games1 = vec![create_basic_game(1, "TPS", "HIFK", "3-2", "runkosarja")];
    //     let games2 = vec![create_basic_game(1, "TPS", "HIFK", "3-2", "runkosarja")];
    //
    //     let hash1 = calculate_games_hash(&games1);
    //     let hash2 = calculate_games_hash(&games2);
    //
    //     // Same data should produce same hash
    //     assert_eq!(hash1, hash2);
    //
    //     // Different data should produce different hash
    //     let games3 = vec![create_basic_game(1, "TPS", "HIFK", "4-2", "runkosarja")];
    //     let hash3 = calculate_games_hash(&games3);
    //     assert_ne!(hash1, hash3);
    // }
}
