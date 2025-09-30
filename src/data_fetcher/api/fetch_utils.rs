//! Generic HTTP fetching utilities with caching, retry logic, and error handling

use reqwest::Client;
use serde::de::DeserializeOwned;
use std::time::Duration;
use tracing::{debug, error, info, instrument, warn};

use crate::data_fetcher::cache::{cache_http_response, get_cached_http_response, has_live_games};
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
    let max_retries = 3u32;
    let mut backoff = Duration::from_millis(250);
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

    // Determine TTL for successful HTTP responses
    let ttl_seconds = if url.contains("/games/") {
        300 // 5 minutes for game data
    } else if url.contains("/schedule") {
        1800 // 30 minutes for schedule data
    } else {
        600 // 10 minutes for other data
    };

    // For both tournament and schedule URLs, check if the response contains live games
    let final_ttl =
        if (url.contains("tournament=") && url.contains("date=")) || url.contains("/schedule") {
            // Try to parse as ScheduleResponse to check for live games
            match serde_json::from_str::<ScheduleResponse>(&response_text) {
                Ok(schedule_response) => {
                    if has_live_games(&schedule_response) {
                        info!(
                            "Live games detected in response from {}, using short cache TTL",
                            url
                        );
                        crate::constants::cache_ttl::LIVE_GAMES_SECONDS // Use live games TTL (15 seconds)
                    } else {
                        debug!(
                            "No live games detected in response from {}, using default TTL",
                            url
                        );
                        ttl_seconds // Use default TTL for completed games
                    }
                }
                Err(_) => ttl_seconds, // Fallback to default if parsing fails
            }
        } else {
            ttl_seconds // Use default TTL for other URLs
        };

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
