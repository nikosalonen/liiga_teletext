use crate::config::Config;
use crate::data_fetcher::models::standings::{StandingsEntry, StandingsResponse};
use crate::error::AppError;
use chrono::{Datelike, Local, Utc};
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
#[instrument]
pub async fn fetch_standings(live_mode: bool) -> Result<(Vec<StandingsEntry>, Vec<u16>), AppError> {
    let config = Config::load().await?;
    let client = create_http_client_with_timeout(config.http_timeout_seconds)?;

    let season = determine_current_season();
    let url = build_standings_url(&config.api_domain, season);
    info!("Fetching standings from: {url}");

    let response: StandingsResponse = fetch(&client, &url).await?;
    info!(
        "Fetched standings: {} teams, playoff lines: {:?}",
        response.season.len(),
        response.playoffs_lines
    );

    let playoffs_lines = response.playoffs_lines.clone();

    let mut entries: Vec<StandingsEntry> =
        response.season.iter().map(StandingsEntry::from).collect();

    if live_mode {
        // Sort by live ranking when in live mode
        entries.sort_by_key(|e| {
            response
                .season
                .iter()
                .find(|t| t.team_id == e.team_id)
                .map(|t| t.live_ranking)
                .unwrap_or(999)
        });
    } else {
        // Sort by official ranking, suppress live indicators
        entries.sort_by_key(|e| {
            response
                .season
                .iter()
                .find(|t| t.team_id == e.team_id)
                .map(|t| t.ranking)
                .unwrap_or(999)
        });
        for entry in &mut entries {
            entry.live_points_delta = None;
            entry.live_position_change = None;
        }
    }

    Ok((entries, playoffs_lines))
}

/// Determine the current Liiga season year for the API.
/// Liiga seasons span two calendar years (e.g., 2025-2026).
/// The API uses the ending year as the season identifier (e.g., 2026 for 2025-2026).
/// Regular season runs roughly September to March.
fn determine_current_season() -> i32 {
    let now = Utc::now().with_timezone(&Local);
    let year = now.year();
    let month = now.month();

    // If we're in January-August, we're in the ending year already
    // If we're in September-December, the season ends next year
    if month <= 8 { year } else { year + 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_current_season() {
        let season = determine_current_season();
        let now = Utc::now().with_timezone(&Local);
        let year = now.year();
        let month = now.month();

        // API uses ending year: Jan-Aug = current year, Sep-Dec = next year
        if month <= 8 {
            assert_eq!(season, year);
        } else {
            assert_eq!(season, year + 1);
        }
    }
}
