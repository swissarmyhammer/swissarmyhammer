//! Health checking functionality for SearXNG instances
//!
//! This module provides functionality to perform health checks on SearXNG instances
//! to determine their availability and response times for load balancing decisions.

use super::instance_discovery::{HealthStatus, SearxInstance};
use futures_util::future;
use reqwest::Client;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, warn};
use url::Url;

/// Health checker for SearXNG instances
#[derive(Debug)]
pub struct HealthChecker {
    http_client: Client,
    check_timeout: Duration,
}

impl HealthChecker {
    /// Creates a new health checker with default timeout
    pub fn new() -> Self {
        Self {
            http_client: Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent("SwissArmyHammer/1.0 (Health Check)")
                .build()
                .unwrap_or_else(|_| Client::new()),
            check_timeout: Duration::from_secs(5),
        }
    }

    /// Creates a new health checker with custom timeout
    pub fn with_timeout(check_timeout: Duration) -> Self {
        Self {
            http_client: Client::builder()
                .timeout(check_timeout)
                .user_agent("SwissArmyHammer/1.0 (Health Check)")
                .build()
                .unwrap_or_else(|_| Client::new()),
            check_timeout,
        }
    }

    /// Performs a health check on a single instance
    ///
    /// This performs a lightweight check by making a simple GET request to the instance
    /// and measuring response time. It doesn't perform a full search to avoid overloading instances.
    pub async fn check_instance_health(&self, instance: &SearxInstance) -> HealthStatus {
        debug!("Checking health of instance: {}", instance.url);

        let start_time = Instant::now();

        // Construct health check URL - just check if the instance responds
        let health_url = match Url::parse(&instance.url) {
            Ok(mut url) => {
                // Use a simple endpoint that doesn't perform actual search
                url.set_path("/");
                url
            }
            Err(e) => {
                return HealthStatus {
                    is_healthy: false,
                    response_time: Duration::from_secs(0),
                    error: Some(format!("Invalid URL: {e}")),
                };
            }
        };

        // Perform the health check with timeout
        let result = timeout(self.check_timeout, self.http_client.get(health_url).send()).await;

        let response_time = start_time.elapsed();

        match result {
            Ok(Ok(response)) => {
                if response.status().is_success() || response.status().as_u16() == 405 {
                    // Consider 405 Method Not Allowed as healthy since the server is responding
                    // Some SearXNG instances might not allow GET on root path
                    debug!(
                        "Health check passed for {}: {} in {:?}",
                        instance.url,
                        response.status(),
                        response_time
                    );
                    HealthStatus {
                        is_healthy: true,
                        response_time,
                        error: None,
                    }
                } else {
                    warn!(
                        "Health check failed for {}: HTTP {}",
                        instance.url,
                        response.status().as_u16()
                    );
                    HealthStatus {
                        is_healthy: false,
                        response_time,
                        error: Some(format!("HTTP error: {}", response.status())),
                    }
                }
            }
            Ok(Err(e)) => {
                if e.is_timeout() {
                    warn!("Health check timeout for {}", instance.url);
                    HealthStatus {
                        is_healthy: false,
                        response_time: self.check_timeout,
                        error: Some("Request timeout".to_string()),
                    }
                } else if e.is_connect() {
                    warn!("Connection failed for {}: {}", instance.url, e);
                    HealthStatus {
                        is_healthy: false,
                        response_time,
                        error: Some("Connection failed".to_string()),
                    }
                } else {
                    warn!("Network error for {}: {}", instance.url, e);
                    HealthStatus {
                        is_healthy: false,
                        response_time,
                        error: Some(format!("Network error: {e}")),
                    }
                }
            }
            Err(_) => {
                // Timeout occurred
                warn!("Health check timeout for {}", instance.url);
                HealthStatus {
                    is_healthy: false,
                    response_time: self.check_timeout,
                    error: Some("Health check timeout".to_string()),
                }
            }
        }
    }

    /// Performs health checks on multiple instances concurrently
    ///
    /// This method updates the health status of all provided instances in-place.
    /// It uses concurrent execution but limits the number of concurrent checks
    /// to avoid overwhelming the network or instances.
    pub async fn bulk_health_check(&self, instances: &mut [SearxInstance]) {
        debug!(
            "Starting bulk health check for {} instances",
            instances.len()
        );

        if instances.is_empty() {
            return;
        }

        let start_time = Instant::now();

        // Use semaphore to limit concurrent checks
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(5)); // Max 5 concurrent

        // Create futures for all health checks
        let mut check_futures = Vec::new();

        for (index, instance) in instances.iter().enumerate() {
            let instance_clone = instance.clone();
            let checker = &self;
            let permit = semaphore.clone();

            let future = async move {
                let _permit = permit.acquire().await.unwrap();
                let health = checker.check_instance_health(&instance_clone).await;
                (index, health)
            };

            check_futures.push(future);
        }

        // Execute all checks concurrently
        let results = future::join_all(check_futures).await;

        // Update instances with health check results
        let mut healthy_count = 0;
        for (index, health_status) in results {
            let instance = &mut instances[index];

            if health_status.is_healthy {
                instance.mark_healthy(health_status.response_time);
                healthy_count += 1;
            } else {
                instance.mark_failed();
                if let Some(error) = &health_status.error {
                    debug!("Instance {} failed health check: {}", instance.url, error);
                }
            }
        }

        let total_time = start_time.elapsed();
        debug!(
            "Bulk health check completed: {}/{} healthy in {:?}",
            healthy_count,
            instances.len(),
            total_time
        );
    }

    /// Performs a search-based health check on an instance
    ///
    /// This is a more thorough check that actually performs a minimal search
    /// to ensure the instance's search functionality is working. Use sparingly
    /// to avoid overloading instances.
    pub async fn check_search_functionality(&self, instance: &SearxInstance) -> HealthStatus {
        debug!("Checking search functionality for: {}", instance.url);

        let start_time = Instant::now();

        // Construct search URL with minimal query
        let search_url = match Url::parse(&instance.url) {
            Ok(mut url) => {
                url.set_path("/search");
                url.query_pairs_mut()
                    .append_pair("q", "test")
                    .append_pair("format", "json")
                    .append_pair("pageno", "1");
                url
            }
            Err(e) => {
                return HealthStatus {
                    is_healthy: false,
                    response_time: Duration::from_secs(0),
                    error: Some(format!("Invalid URL: {e}")),
                };
            }
        };

        let result = timeout(self.check_timeout, self.http_client.get(search_url).send()).await;

        let response_time = start_time.elapsed();

        match result {
            Ok(Ok(response)) => {
                if response.status().is_success() {
                    // Try to parse as JSON to ensure search API is working
                    match response.json::<serde_json::Value>().await {
                        Ok(json) => {
                            // Check if response has expected structure
                            if json.get("results").is_some() || json.get("error").is_some() {
                                debug!("Search functionality check passed for {}", instance.url);
                                HealthStatus {
                                    is_healthy: true,
                                    response_time,
                                    error: None,
                                }
                            } else {
                                warn!(
                                    "Search response missing expected structure for {}",
                                    instance.url
                                );
                                HealthStatus {
                                    is_healthy: false,
                                    response_time,
                                    error: Some("Invalid search response structure".to_string()),
                                }
                            }
                        }
                        Err(e) => {
                            warn!(
                                "Failed to parse search response for {}: {}",
                                instance.url, e
                            );
                            HealthStatus {
                                is_healthy: false,
                                response_time,
                                error: Some("Invalid JSON response".to_string()),
                            }
                        }
                    }
                } else {
                    warn!(
                        "Search functionality check failed for {}: HTTP {}",
                        instance.url,
                        response.status().as_u16()
                    );
                    HealthStatus {
                        is_healthy: false,
                        response_time,
                        error: Some(format!("HTTP error: {}", response.status())),
                    }
                }
            }
            Ok(Err(e)) => {
                warn!(
                    "Search functionality check error for {}: {}",
                    instance.url, e
                );
                HealthStatus {
                    is_healthy: false,
                    response_time,
                    error: Some(format!("Request error: {e}")),
                }
            }
            Err(_) => {
                warn!("Search functionality check timeout for {}", instance.url);
                HealthStatus {
                    is_healthy: false,
                    response_time: self.check_timeout,
                    error: Some("Search check timeout".to_string()),
                }
            }
        }
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::web_search::instance_discovery::SearxInstance;
    use std::time::Duration;

    #[test]
    fn test_health_checker_new() {
        let checker = HealthChecker::new();
        assert_eq!(checker.check_timeout, Duration::from_secs(5));
    }

    #[test]
    fn test_health_checker_with_timeout() {
        let timeout = Duration::from_secs(10);
        let checker = HealthChecker::with_timeout(timeout);
        assert_eq!(checker.check_timeout, timeout);
    }

    #[tokio::test]
    async fn test_check_instance_health_invalid_url() {
        let checker = HealthChecker::new();
        let instance = SearxInstance::new("not-a-url".to_string(), "A+".to_string(), 95.0, 1000);

        let health = checker.check_instance_health(&instance).await;

        assert!(!health.is_healthy);
        assert!(health.error.is_some());
        assert!(health.error.unwrap().contains("Invalid URL"));
    }

    #[tokio::test]
    async fn test_bulk_health_check_empty() {
        let checker = HealthChecker::new();
        let mut instances: Vec<SearxInstance> = vec![];

        // Should not panic with empty slice
        checker.bulk_health_check(&mut instances).await;

        assert!(instances.is_empty());
    }

    #[tokio::test]
    async fn test_bulk_health_check_with_instances() {
        let checker = HealthChecker::new();
        let mut instances = vec![
            SearxInstance::new(
                "https://httpbin.org".to_string(), // This should respond
                "A+".to_string(),
                95.0,
                1000,
            ),
            SearxInstance::new(
                "https://this-domain-should-not-exist-12345.com".to_string(), // This should fail
                "A+".to_string(),
                95.0,
                1000,
            ),
        ];

        checker.bulk_health_check(&mut instances).await;

        // At least verify the function completes without panic
        // Actual network results may vary in test environment
        assert_eq!(instances.len(), 2);
    }

    #[tokio::test]
    async fn test_check_search_functionality_invalid_url() {
        let checker = HealthChecker::new();
        let instance = SearxInstance::new("invalid-url".to_string(), "A+".to_string(), 95.0, 1000);

        let health = checker.check_search_functionality(&instance).await;

        assert!(!health.is_healthy);
        assert!(health.error.is_some());
        assert!(health.error.unwrap().contains("Invalid URL"));
    }

    #[test]
    fn test_health_checker_default() {
        let checker = HealthChecker::default();
        assert_eq!(checker.check_timeout, Duration::from_secs(5));
    }

    // Integration test that requires network access - marked with ignore
    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_check_instance_health_real_instance() {
        let checker = HealthChecker::new();
        let instance = SearxInstance::new(
            "https://httpbin.org".to_string(), // Using httpbin as a test endpoint
            "A+".to_string(),
            95.0,
            1000,
        );

        let health = checker.check_instance_health(&instance).await;

        // httpbin should respond successfully
        assert!(health.is_healthy);
        assert!(health.response_time > Duration::from_millis(0));
        assert!(health.error.is_none());
    }
}
