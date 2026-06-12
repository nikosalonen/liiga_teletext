// src/data_fetcher/api/bracket_api.rs
use crate::config::Config;
use crate::data_fetcher::api::date_logic::parse_date_and_season;
use crate::data_fetcher::api::http_client::create_http_client_with_timeout;
use crate::data_fetcher::api::tournament_logic::{TournamentType, fetch_tournament_games};
use crate::data_fetcher::models::ScheduleApiGame;
use crate::data_fetcher::models::bracket::{PlayoffBracket, build_playoff_bracket};
use crate::error::AppError;
use chrono::{DateTime, Utc};
use tracing::info;

/// Returns the bracket visibility grace period in days. Defaults to
/// `bracket::VISIBILITY_GRACE_DAYS` but can be overridden with the
/// `LIIGA_BRACKET_GRACE_DAYS` environment variable for testing the
/// bracket view outside the playoff season.
fn bracket_grace_days() -> i64 {
    std::env::var(crate::constants::env_vars::BRACKET_GRACE_DAYS)
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(crate::constants::bracket::VISIBILITY_GRACE_DAYS)
}

/// Returns true when every playoff game in the schedule concluded longer
/// ago than the grace period — i.e. the bracket belongs to a finished
/// season. During playoffs the latest game start is always recent or in
/// the future, so an active bracket is never considered stale.
fn is_bracket_stale(games: &[ScheduleApiGame], now: DateTime<Utc>, grace_days: i64) -> bool {
    let latest_start = games
        .iter()
        .filter(|g| g.play_off_phase.is_some())
        .filter_map(|g| DateTime::parse_from_rfc3339(&g.start).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .max();

    match latest_start {
        Some(latest) => now.signed_duration_since(latest) > chrono::Duration::days(grace_days),
        // No parsable playoff games: nothing to declare stale
        None => false,
    }
}

/// Fetches and constructs the playoff bracket for the current season.
/// The bracket is only reported as available (`has_data`) while the
/// playoffs are upcoming, ongoing, or recently concluded — an old season's
/// bracket is not offered during the off-season.
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
    let mut bracket = build_playoff_bracket(&games, &season_str);

    let grace_days = bracket_grace_days();
    if bracket.has_data && is_bracket_stale(&games, Utc::now(), grace_days) {
        info!(
            "Playoff bracket for season {season} concluded more than {grace_days} days ago, hiding playoffs view"
        );
        bracket.has_data = false;
    }

    info!(
        "Bracket built: has_data={}, phases={}",
        bracket.has_data,
        bracket.phases.len()
    );

    Ok(bracket)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playoff_game(start: &str) -> ScheduleApiGame {
        ScheduleApiGame {
            id: 1,
            season: 2026,
            start: start.to_string(),
            home_team_name: "Tappara".to_string(),
            away_team_name: "Kärpät".to_string(),
            serie: 2,
            finished_type: None,
            started: false,
            ended: false,
            game_time: None,
            play_off_phase: Some(1),
            play_off_pair: Some(1),
            play_off_req_wins: Some(4),
            home_team_goals: 0,
            away_team_goals: 0,
        }
    }

    fn at(date: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(date)
            .unwrap()
            .with_timezone(&Utc)
    }

    const GRACE: i64 = crate::constants::bracket::VISIBILITY_GRACE_DAYS;

    #[test]
    fn test_bracket_not_stale_during_playoffs() {
        // Latest game is in the future (next round already scheduled)
        let games = vec![
            playoff_game("2026-04-10T18:30:00Z"),
            playoff_game("2026-04-14T18:30:00Z"),
        ];
        assert!(!is_bracket_stale(&games, at("2026-04-11T12:00:00Z"), GRACE));
    }

    #[test]
    fn test_bracket_not_stale_shortly_after_finals() {
        // Finals ended a week ago - still within the grace period
        let games = vec![playoff_game("2026-05-10T18:30:00Z")];
        assert!(!is_bracket_stale(&games, at("2026-05-17T12:00:00Z"), GRACE));
    }

    #[test]
    fn test_bracket_stale_in_off_season() {
        // Finals ended in May, it's mid-June: hide the bracket
        let games = vec![playoff_game("2026-05-10T18:30:00Z")];
        assert!(is_bracket_stale(&games, at("2026-06-12T12:00:00Z"), GRACE));
    }

    #[test]
    fn test_bracket_not_stale_without_parsable_games() {
        let game = playoff_game("not-a-date");
        assert!(!is_bracket_stale(
            &[game],
            at("2026-06-12T12:00:00Z"),
            GRACE
        ));
        assert!(!is_bracket_stale(&[], at("2026-06-12T12:00:00Z"), GRACE));
    }

    #[test]
    fn test_extended_grace_period_keeps_old_bracket_visible() {
        // A large grace period (as set via LIIGA_BRACKET_GRACE_DAYS)
        // keeps the previous season's bracket available for testing
        let games = vec![playoff_game("2026-05-10T18:30:00Z")];
        assert!(!is_bracket_stale(&games, at("2026-06-12T12:00:00Z"), 400));
    }
}
