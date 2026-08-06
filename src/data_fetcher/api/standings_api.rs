use crate::config::Config;
use crate::data_fetcher::models::standings::{StandingsEntry, StandingsResponse};
use crate::error::AppError;
use chrono::{Datelike, Utc};
use std::collections::HashMap;
use tracing::{info, instrument};

use super::fetch_utils::fetch;
use super::http_client::create_http_client_with_timeout;
use super::urls::build_standings_url;

/// Fetches standings from the Liiga standings API.
///
/// The API provides pre-computed standings including live ranking/points
/// when games are in progress.
/// When `live_mode` is true, sorts by `live_ranking` and shows live indicators.
/// When `live_mode` is false, sorts by `ranking` and suppresses live indicators.
/// Returns (standings entries sorted by ranking, playoff line positions).
#[instrument(skip(config))]
pub async fn fetch_standings(
    config: &Config,
    live_mode: bool,
) -> Result<(Vec<StandingsEntry>, Vec<u16>), AppError> {
    let now = Utc::now();
    let season = season_for_date(now.year(), now.month());
    fetch_standings_for_season(config, live_mode, season, is_lookahead_month(now.month())).await
}

/// True during the off-season look-ahead window, when the upcoming season's
/// standings may not exist in the API yet and falling back to the previous
/// season's final table is preferable to an empty page.
fn is_lookahead_month(month: u32) -> bool {
    (7..=8).contains(&month)
}

/// Fetches the previous season's standings as the look-ahead fallback.
async fn fetch_previous_season(
    client: &reqwest::Client,
    config: &Config,
    season: i32,
) -> Result<StandingsResponse, AppError> {
    let previous = season - 1;
    let url = build_standings_url(&config.api_domain, previous);
    info!("No standings for season {season} yet, falling back to {previous}: {url}");
    fetch(client, &url).await
}

async fn fetch_standings_for_season(
    config: &Config,
    live_mode: bool,
    season: i32,
    allow_previous_fallback: bool,
) -> Result<(Vec<StandingsEntry>, Vec<u16>), AppError> {
    let client = create_http_client_with_timeout(config.http_timeout_seconds)?;

    let url = build_standings_url(&config.api_domain, season);
    info!("Fetching standings from: {url}");

    // During the off-season look-ahead the upcoming season's standings may
    // not exist in the API yet (empty team list or 404); fall back to the
    // previous season's final table instead of showing an empty page.
    let response: StandingsResponse = match fetch(&client, &url).await {
        Ok(StandingsResponse { season: teams, .. })
            if teams.is_empty() && allow_previous_fallback =>
        {
            fetch_previous_season(&client, config, season).await?
        }
        Ok(response) => response,
        Err(e) if e.is_not_found() && allow_previous_fallback => {
            fetch_previous_season(&client, config, season).await?
        }
        Err(e) => return Err(e),
    };
    info!(
        "Fetched standings: {} teams, playoff lines: {:?}",
        response.season.len(),
        response.playoffs_lines
    );

    let playoffs_lines = response.playoffs_lines.clone();

    let mut entries: Vec<StandingsEntry> =
        response.season.iter().map(StandingsEntry::from).collect();

    if live_mode {
        let ranking_map: HashMap<&str, u16> = response
            .season
            .iter()
            .map(|t| (t.team_id.as_str(), t.live_ranking))
            .collect();
        entries.sort_by_key(|e| *ranking_map.get(e.team_id.as_str()).unwrap_or(&999));
    } else {
        let ranking_map: HashMap<&str, u16> = response
            .season
            .iter()
            .map(|t| (t.team_id.as_str(), t.ranking))
            .collect();
        entries.sort_by_key(|e| *ranking_map.get(e.team_id.as_str()).unwrap_or(&999));
        for entry in &mut entries {
            entry.live_points_delta = None;
            entry.live_position_change = None;
            entry.live_game_active = false;
            entry.live_goals_for = entry.goals_for;
            entry.live_goals_against = entry.goals_against;
        }
    }

    Ok((entries, playoffs_lines))
}

/// Maps a calendar date to the season whose standings are most relevant.
/// Liiga seasons span two calendar years and the API uses the ending year
/// as the season identifier (e.g., 2026 for 2025-2026).
/// January–June: the season being played (or whose playoffs just ended).
/// July onward: the upcoming season — the API already serves the new
/// season's team lineup (zeroed stats) so the off-season/preseason
/// standings page looks ahead instead of back.
fn season_for_date(year: i32, month: u32) -> i32 {
    if month <= 6 { year } else { year + 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_season_during_regular_season_and_playoffs() {
        // Season being played: identified by its ending year.
        assert_eq!(season_for_date(2025, 9), 2026); // regular season start
        assert_eq!(season_for_date(2025, 12), 2026);
        assert_eq!(season_for_date(2026, 1), 2026);
        assert_eq!(season_for_date(2026, 4), 2026); // playoffs
        assert_eq!(season_for_date(2026, 5), 2026); // late finals / just ended
        assert_eq!(season_for_date(2026, 6), 2026); // post-season wrap-up
    }

    #[test]
    fn test_season_flips_to_upcoming_in_off_season() {
        // From July the standings page looks ahead to the starting season.
        assert_eq!(season_for_date(2026, 7), 2027);
        assert_eq!(season_for_date(2026, 8), 2027); // preseason
    }

    #[test]
    fn test_lookahead_window_is_july_august() {
        for month in 1..=12 {
            assert_eq!(is_lookahead_month(month), (7..=8).contains(&month));
        }
    }

    fn make_test_config(api_domain: String) -> Config {
        Config {
            api_domain,
            log_file_path: None,
            http_timeout_seconds: 1,
        }
    }

    fn standings_json(team_name: &str) -> serde_json::Value {
        serde_json::json!({
            "season": [{
                "teamId": "t1",
                "teamName": team_name,
                "ranking": 1,
                "liveRanking": 1,
                "games": 0,
                "wins": 0,
                "overtimeWins": 0,
                "losses": 0,
                "overtimeLosses": 0,
                "points": 0,
                "livePoints": 0,
                "goals": 0,
                "goalsAgainst": 0,
                "liveGoals": 0,
                "liveGoalsAgainst": 0
            }],
            "playoffsLines": [4, 12]
        })
    }

    async fn mount_standings(
        server: &wiremock::MockServer,
        season: i32,
        response: wiremock::ResponseTemplate,
    ) {
        use wiremock::matchers::{method, path, query_param};
        wiremock::Mock::given(method("GET"))
            .and(path("/standings/"))
            .and(query_param("season", season.to_string()))
            .respond_with(response)
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn test_lookahead_falls_back_to_previous_season_when_next_empty() {
        let server = wiremock::MockServer::start().await;
        let empty = serde_json::json!({"season": [], "playoffsLines": []});
        mount_standings(
            &server,
            2027,
            wiremock::ResponseTemplate::new(200).set_body_json(empty),
        )
        .await;
        mount_standings(
            &server,
            2026,
            wiremock::ResponseTemplate::new(200).set_body_json(standings_json("Tappara")),
        )
        .await;

        let config = make_test_config(server.uri());
        let (entries, _) = fetch_standings_for_season(&config, false, 2027, true)
            .await
            .unwrap();
        assert_eq!(entries.len(), 1, "should fall back to previous season");
        assert_eq!(entries[0].team_name, "Tappara");
    }

    #[tokio::test]
    async fn test_lookahead_falls_back_to_previous_season_when_next_missing() {
        let server = wiremock::MockServer::start().await;
        mount_standings(&server, 2027, wiremock::ResponseTemplate::new(404)).await;
        mount_standings(
            &server,
            2026,
            wiremock::ResponseTemplate::new(200).set_body_json(standings_json("KooKoo")),
        )
        .await;

        let config = make_test_config(server.uri());
        let (entries, _) = fetch_standings_for_season(&config, false, 2027, true)
            .await
            .unwrap();
        assert_eq!(entries.len(), 1, "should fall back to previous season");
        assert_eq!(entries[0].team_name, "KooKoo");
    }

    #[tokio::test]
    async fn test_no_fallback_outside_lookahead_window() {
        let server = wiremock::MockServer::start().await;
        let empty = serde_json::json!({"season": [], "playoffsLines": []});
        mount_standings(
            &server,
            2027,
            wiremock::ResponseTemplate::new(200).set_body_json(empty),
        )
        .await;

        let config = make_test_config(server.uri());
        let (entries, _) = fetch_standings_for_season(&config, false, 2027, false)
            .await
            .unwrap();
        assert!(
            entries.is_empty(),
            "in-season an empty response must not be papered over with old data"
        );
    }
}
