use crate::data_fetcher::models::GameData;
use crate::teletext_ui::ScoreType;
use tracing::trace;

/// Determines if a list of GameData contains live games
pub fn has_live_games_from_game_data(games: &[GameData]) -> bool {
    let has_live = games
        .iter()
        .any(|game| game.score_type == ScoreType::Ongoing);

    if has_live {
        let ongoing_count = games
            .iter()
            .filter(|g| g.score_type == ScoreType::Ongoing)
            .count();
        trace!(
            "Live games detected: {} ongoing out of {} total games",
            ongoing_count,
            games.len()
        );

        // Log details of ongoing games for debugging
        for (i, game) in games.iter().enumerate() {
            if game.score_type == ScoreType::Ongoing {
                trace!(
                    "Ongoing game {}: {} vs {} - Score: {}, Time: {}",
                    i + 1,
                    game.home_team,
                    game.away_team,
                    game.result,
                    game.time
                );
            }
        }
    } else {
        trace!("No live games detected in {} games", games.len());
    }

    has_live
}
