//! HTTP client creation and configuration utilities

use reqwest::Client;
use std::time::Duration;

/// Creates a properly configured HTTP client with connection pooling and timeout handling.
/// This follows the coding guidelines for HTTP client usage with proper timeout handling,
/// connection pooling, and HTTP/2 multiplexing when available.
///
/// # Returns
/// * `Result<Client, reqwest::Error>` - A configured reqwest HTTP client or error
///
/// # Features
/// * Configurable timeout for requests (default: 30 seconds, configurable via config/env)
/// * Connection pooling with centralized pool size configuration
/// * HTTP/2 multiplexing when available
/// * Automatic retry logic for transient failures (implemented in fetch function)
pub fn create_http_client_with_timeout(timeout_seconds: u64) -> Result<Client, reqwest::Error> {
    Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .pool_max_idle_per_host(crate::constants::HTTP_POOL_MAX_IDLE_PER_HOST)
        .build()
}

/// Creates an HTTP client for testing with default timeout
#[cfg(test)]
pub fn create_test_http_client() -> Client {
    create_http_client_with_timeout(crate::constants::DEFAULT_HTTP_TIMEOUT_SECONDS)
        .expect("Failed to create test HTTP client")
}