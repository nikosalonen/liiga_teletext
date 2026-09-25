//! Generic HTTP fetching utilities with caching, retry logic, and error handling

use reqwest::Client;
use serde::de::DeserializeOwned;
use std::time::Duration;
use tracing::{debug, error, info, instrument, warn};

use crate::data_fetcher::cache::{
    cache_http_response, get_cached_http_response, schedule_cache_ttl,
};
use crate::data_fetcher::models::ScheduleResponse;
use crate::error::AppError;

/// Generic fetch function with HTTP caching, retry logic, and comprehensive error handling.
///
/// This function:
/// - Checks HTTP response cache first
/// - Implements retry logic with exponential backoff for transient failures
/// - Respects Retry-After headers for rate limiting
/// - Caches successful responses with adaptive TTL based on content
/// - Provides detailed error handling for various HTTP status codes
///
/// # Arguments
/// * `client` - HTTP client for making requests
/// * `url` - URL to fetch data from
///
/// # Returns
/// * `Result<T, AppError>` - Parsed response data or error
#[instrument(skip(client))]
pub(super) async fn fetch<T: DeserializeOwned>(client: &Client, url: &str) -> Result<T, AppError> {
    fetch_with_retries(client, url, crate::constants::retry::MAX_ATTEMPTS).await
}

/// Same as [`fetch`] but with a custom retry budget. Use a low budget for
/// endpoints whose failures are expected and long-lived (e.g. availability
/// checks for unpublished tournaments) to avoid wasted requests and rate
/// limiting.
#[instrument(skip(client))]
pub(super) async fn fetch_with_retries<T: DeserializeOwned>(
    client: &Client,
    url: &str,
    max_retries: u32,
) -> Result<T, AppError> {
    info!("Fetching data from URL: {url}");

    // Check HTTP response cache first
    if let Some(cached_response) = get_cached_http_response(url).await {
        debug!("Using cached HTTP response for URL: {url}");
        match serde_json::from_str::<T>(&cached_response) {
            Ok(parsed) => return Ok(parsed),
            Err(e) => {
                warn!("Failed to parse cached response for URL {}: {}", url, e);
                // Continue with fresh request if cached response is invalid
            }
        }
    }

    // Handle reqwest errors with retries/backoff for transient failures
    let mut attempt = 0u32;
    let mut backoff = Duration::from_millis(crate::constants::retry::INITIAL_BACKOFF_MS);
    let response = loop {
        match client.get(url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if (status.as_u16() == 429 || status.is_server_error()) && attempt < max_retries {
                    // Respect Retry-After if provided
                    let retry_after = resp
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .map(Duration::from_secs);
                    let wait = retry_after.unwrap_or(backoff);
                    warn!(
                        "Transient {} from {}. Retrying in {:?} (attempt {}/{})",
                        status,
                        url,
                        wait,
                        attempt + 1,
                        max_retries
                    );
                    tokio::time::sleep(wait).await;
                    attempt += 1;
                    backoff = backoff.saturating_mul(2);
                    continue;
                }
                break resp;
            }
            Err(e) => {
                if (e.is_timeout() || e.is_connect()) && attempt < max_retries {
                    warn!(
                        "Request error {} for {}. Retrying in {:?} (attempt {}/{})",
                        e,
                        url,
                        backoff,
                        attempt + 1,
                        max_retries
                    );
                    tokio::time::sleep(backoff).await;
                    attempt += 1;
                    backoff = backoff.saturating_mul(2);
                    continue;
                }
                error!("Request failed for URL {}: {}", url, e);
                return if e.is_timeout() {
                    Err(AppError::network_timeout(url))
                } else if e.is_connect() {
                    Err(AppError::network_connection(url, e.to_string()))
                } else {
                    Err(AppError::ApiFetch(e))
                };
            }
        }
    };

    let status = response.status();
    let headers = response.headers().clone();

    debug!("Response status: {status}");
    debug!("Response headers: {:?}", headers);

    if !status.is_success() {
        let status_code = status.as_u16();
        let reason = status.canonical_reason().unwrap_or("Unknown error");

        error!("HTTP {} - {} (URL: {})", status_code, reason, url);

        // Return specific error types based on HTTP status code
        return Err(match status_code {
            404 => AppError::api_not_found(url),
            429 => AppError::api_rate_limit(reason, url),
            400..=499 => AppError::api_client_error(status_code, reason, url),
            500..=599 => {
                if status_code == 502 || status_code == 503 {
                    AppError::api_service_unavailable(status_code, reason, url)
                } else {
                    AppError::api_server_error(status_code, reason, url)
                }
            }
            _ => AppError::api_server_error(status_code, reason, url),
        });
    }

    let response_text = match response.text().await {
        Ok(text) => text,
        Err(e) => {
            error!("Failed to read response text from URL {}: {}", url, e);
            return Err(AppError::ApiFetch(e));
        }
    };

    debug!("Response length: {} bytes", response_text.len());
    let preview: String = response_text.chars().take(1024).collect();
    debug!("Response text (first 1024 chars): {preview}");

    let final_ttl = http_cache_ttl(url, &response_text, chrono::Utc::now());

    // Enhanced JSON parsing with more specific error handling
    match serde_json::from_str::<T>(&response_text) {
        Ok(parsed) => {
            // Cache only valid/parsable payloads; move the body (no clone)
            cache_http_response(url.to_string(), response_text, final_ttl).await;
            Ok(parsed)
        }
        Err(e) => {
            error!("Failed to parse API response: {} (URL: {})", e, url);
            error!(
                "Response text (first 200 chars): {}",
                &response_text.chars().take(200).collect::<String>()
            );

            // Check if it's malformed JSON vs unexpected structure
            if response_text.trim().is_empty() {
                Err(AppError::api_no_data("Response body is empty", url))
            } else if !response_text.trim_start().starts_with('{')
                && !response_text.trim_start().starts_with('[')
            {
                Err(AppError::api_malformed_json(
                    "Response is not valid JSON",
                    url,
                ))
            } else {
                // Valid JSON but unexpected structure
                Err(AppError::api_unexpected_structure(e.to_string(), url))
            }
        }
    }
}

/// Picks the HTTP cache TTL (in seconds) for a successful response.
///
/// The TTL depends on the endpoint. Tournament day responses can drop below it
/// while a game is live, about to start, or late to start (see
/// [`schedule_cache_ttl`]). `/schedule` responses are season lists that do not
/// parse as `ScheduleResponse`, so they keep the endpoint TTL.
fn http_cache_ttl(url: &str, response_text: &str, now: chrono::DateTime<chrono::Utc>) -> u64 {
    let endpoint_ttl = if url.contains("/games/") {
        300 // 5 minutes for game data
    } else if url.contains("/schedule") {
        1800 // 30 minutes for schedule data
    } else if url.contains("/standings/") {
        crate::constants::cache_ttl::LIVE_GAMES_SECONDS // Short TTL so live standings refresh promptly
    } else {
        600 // 10 minutes for other data
    };

    let is_schedule_like =
        (url.contains("tournament=") && url.contains("date=")) || url.contains("/schedule");
    if !is_schedule_like {
        return endpoint_ttl;
    }

    match serde_json::from_str::<ScheduleResponse>(response_text) {
        Ok(schedule_response) => {
            let ttl = schedule_cache_ttl(&schedule_response, now, endpoint_ttl);
            if ttl < endpoint_ttl {
                info!("Live or upcoming game in response from {url}, using {ttl}s cache TTL");
            } else {
                debug!("No live or upcoming games in response from {url}, using default TTL");
            }
            ttl
        }
        Err(_) => endpoint_ttl,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    const TOURNAMENT_URL: &str =
        "https://liiga.fi/api/v2/games?tournament=runkosarja&date=2026-10-01";

    fn schedule_body(started: bool, ended: bool) -> String {
        format!(
            r#"{{"games":[{{"id":1,"season":2026,"start":"2026-10-01T15:30:00Z",
                "homeTeam":{{"teamId":"a","teamName":"A","goals":0,"goalEvents":[]}},
                "awayTeam":{{"teamId":"b","teamName":"B","goals":0,"goalEvents":[]}},
                "finishedType":null,"started":{started},"ended":{ended},
                "serie":"RUNKOSARJA"}}],
                "previousGameDate":null,"nextGameDate":null}}"#
        )
    }

    #[test]
    fn tournament_response_just_before_puck_drop_gets_starting_ttl() {
        let two_minutes_before = Utc.with_ymd_and_hms(2026, 10, 1, 15, 28, 0).unwrap();
        let ttl = http_cache_ttl(
            TOURNAMENT_URL,
            &schedule_body(false, false),
            two_minutes_before,
        );
        assert_eq!(ttl, crate::constants::cache_ttl::STARTING_GAMES_SECONDS);
    }

    #[test]
    fn tournament_response_fetched_before_the_window_expires_when_it_opens() {
        // Fetched at T-6 min: the 10-minute default would keep the "not
        // started" response cached until T+4 min and hide the puck drop.
        let six_minutes_before = Utc.with_ymd_and_hms(2026, 10, 1, 15, 24, 0).unwrap();
        let ttl = http_cache_ttl(
            TOURNAMENT_URL,
            &schedule_body(false, false),
            six_minutes_before,
        );
        assert_eq!(ttl, 60);
    }

    #[test]
    fn tournament_response_with_live_game_gets_live_ttl() {
        let mid_game = Utc.with_ymd_and_hms(2026, 10, 1, 16, 0, 0).unwrap();
        let ttl = http_cache_ttl(TOURNAMENT_URL, &schedule_body(true, false), mid_game);
        assert_eq!(ttl, crate::constants::cache_ttl::LIVE_GAMES_SECONDS);
    }

    #[test]
    fn finished_tournament_response_keeps_default_ttl() {
        let evening = Utc.with_ymd_and_hms(2026, 10, 1, 20, 0, 0).unwrap();
        let ttl = http_cache_ttl(TOURNAMENT_URL, &schedule_body(true, true), evening);
        assert_eq!(ttl, 600);
    }
}
