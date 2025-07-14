use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

/// Type alias for complex request handle type
type RequestHandle = Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>;

/// Request deduplication to prevent multiple identical API calls
pub struct RequestDeduplicator {
    pending_requests: Arc<RwLock<HashMap<String, RequestHandle>>>,
}

impl RequestDeduplicator {
    pub fn new() -> Self {
        Self {
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if a request is already in progress
    pub async fn is_request_pending(&self, key: &str) -> bool {
        let pending = self.pending_requests.read().await;
        pending.contains_key(key)
    }

    /// Register a new request
    pub async fn register_request(&self, key: String, handle: tokio::task::JoinHandle<()>) {
        let mut pending = self.pending_requests.write().await;
        pending.insert(key, Arc::new(Mutex::new(Some(handle))));
    }

    /// Complete a request
    pub async fn complete_request(&self, key: &str) {
        let mut pending = self.pending_requests.write().await;
        pending.remove(key);
    }
}

impl Default for RequestDeduplicator {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance metrics collector
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub api_call_count: u64,
    pub cache_hit_count: u64,
    pub cache_miss_count: u64,
    pub average_response_time_ms: f64,
    pub last_updated: Instant,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            api_call_count: 0,
            cache_hit_count: 0,
            cache_miss_count: 0,
            average_response_time_ms: 0.0,
            last_updated: Instant::now(),
        }
    }

    pub fn record_api_call(&mut self, response_time: Duration) {
        self.api_call_count += 1;
        let response_time_ms = response_time.as_millis() as f64;

        // Calculate rolling average
        if self.api_call_count == 1 {
            self.average_response_time_ms = response_time_ms;
        } else {
            self.average_response_time_ms =
                (self.average_response_time_ms * (self.api_call_count - 1) as f64 + response_time_ms)
                / self.api_call_count as f64;
        }

        self.last_updated = Instant::now();
    }

    pub fn record_cache_hit(&mut self) {
        self.cache_hit_count += 1;
        self.last_updated = Instant::now();
    }

    pub fn record_cache_miss(&mut self) {
        self.cache_miss_count += 1;
        self.last_updated = Instant::now();
    }

    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hit_count + self.cache_miss_count;
        if total == 0 {
            0.0
        } else {
            self.cache_hit_count as f64 / total as f64
        }
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_request_deduplicator() {
        let deduplicator = RequestDeduplicator::new();
        let key = "test_request".to_string();

        // Initially no request should be pending
        assert!(!deduplicator.is_request_pending(&key).await);

        // Register a request
        let handle = tokio::spawn(async {
            sleep(Duration::from_millis(10)).await;
        });
        deduplicator.register_request(key.clone(), handle).await;

        // Now request should be pending
        assert!(deduplicator.is_request_pending(&key).await);

        // Complete the request
        deduplicator.complete_request(&key).await;

        // Request should no longer be pending
        assert!(!deduplicator.is_request_pending(&key).await);
    }

    #[test]
    fn test_performance_metrics() {
        let mut metrics = PerformanceMetrics::new();

        // Test initial state
        assert_eq!(metrics.api_call_count, 0);
        assert_eq!(metrics.cache_hit_count, 0);
        assert_eq!(metrics.cache_miss_count, 0);
        assert_eq!(metrics.cache_hit_rate(), 0.0);

        // Test API call recording
        metrics.record_api_call(Duration::from_millis(100));
        assert_eq!(metrics.api_call_count, 1);
        assert_eq!(metrics.average_response_time_ms, 100.0);

        metrics.record_api_call(Duration::from_millis(200));
        assert_eq!(metrics.api_call_count, 2);
        assert_eq!(metrics.average_response_time_ms, 150.0);

        // Test cache metrics
        metrics.record_cache_hit();
        metrics.record_cache_miss();
        assert_eq!(metrics.cache_hit_count, 1);
        assert_eq!(metrics.cache_miss_count, 1);
        assert_eq!(metrics.cache_hit_rate(), 0.5);

        metrics.record_cache_hit();
        assert_eq!(metrics.cache_hit_rate(), 2.0 / 3.0);
    }
}
