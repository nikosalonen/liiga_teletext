// src/data_fetcher/api/bracket_api.rs
use crate::config::Config;
use crate::data_fetcher::api::date_logic::parse_date_and_season;
use crate::data_fetcher::api::http_client::create_http_client_with_timeout;
use crate::data_fetcher::api::tournament_logic::{TournamentType, fetch_tournament_games};
use crate::data_fetcher::models::bracket::{PlayoffBracket, build_playoff_bracket};
use crate::error::AppError;
use chrono::Utc;
use tracing::info;

/// Fetches and constructs the playoff bracket for the current season.
pub async fn fetch_playoff_bracket(config: &Config) -> Result<PlayoffBracket, AppError> {
    let client = create_http_client_with_timeout(config.http_timeout_seconds)?;

    // Determine current season from today's date
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let (_year, _month, season) = parse_date_and_season(&today);

    info!("Fetching playoff bracket for season {season}");

    let games = fetch_tournament_games(&client, config, &[TournamentType::Playoffs], season).await;

    let playoff_count = games.iter().filter(|g| g.play_off_phase.is_some()).count();
    info!(
        "Schedule returned {} total games, {} with play_off_phase set",
        games.len(),
        playoff_count
    );

    let season_str = format!("{}-{}", season - 1, season);
    let bracket = build_playoff_bracket(&games, &season_str);

    info!(
        "Bracket built: has_data={}, phases={}",
        bracket.has_data,
        bracket.phases.len()
    );

    Ok(bracket)
}
