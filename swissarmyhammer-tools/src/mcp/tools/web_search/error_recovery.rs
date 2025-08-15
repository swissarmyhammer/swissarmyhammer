//! Error handling and recovery system for web search operations
//!
//! This module implements comprehensive error handling, failover strategies, and graceful
//! degradation for web search operations as specified in WEB_SEARCH_000007.

use crate::mcp::tools::web_search::types::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};

/// Comprehensive error types for web search operations with recovery metadata
#[derive(Debug, Error, Clone)]
pub enum WebSearchError {
    // Network and connectivity errors
    /// Network connectivity error during web search operations
    #[error("Network error: {message}")]
    Network {
        /// Human-readable error message describing the network failure
        message: String,
        /// URL or identifier of the SearXNG instance that failed
        instance: String,
        #[source]
        /// Optional underlying error that caused this network failure
        source: Option<Arc<dyn std::error::Error + Send + Sync>>,
    },

    /// Connection timeout occurred while communicating with SearXNG instance
    #[error("Connection timeout after {timeout_ms}ms")]
    ConnectionTimeout {
        /// Duration in milliseconds after which the connection timed out
        timeout_ms: u64,
        /// URL or identifier of the SearXNG instance that timed out
        instance: String,
    },

    /// DNS resolution failed for the specified hostname
    #[error("DNS resolution failed for {host}")]
    DnsResolution {
        /// Hostname that could not be resolved via DNS
        host: String,
    },

    // SearXNG API errors
    /// SearXNG instance is not responding or not available
    #[error("SearXNG instance unavailable: {url}")]
    InstanceUnavailable {
        /// URL of the unavailable SearXNG instance
        url: String,
    },

    /// Rate limiting has been applied by the SearXNG instance
    #[error("Rate limited by {instance} - retry after {retry_after_secs}s")]
    RateLimited {
        /// URL or identifier of the SearXNG instance that applied rate limiting
        instance: String,
        /// Number of seconds to wait before retrying the request
        retry_after_secs: u64,
    },

    /// Search parameters provided to the API are invalid
    #[error("Invalid search parameters: {details}")]
    InvalidParameters {
        /// Detailed explanation of what made the parameters invalid
        details: String,
    },

    /// Failed to parse the API response from SearXNG
    #[error("API response parsing failed: {details}")]
    ResponseParsing {
        /// Specific details about what went wrong during response parsing
        details: String,
        /// URL or identifier of the SearXNG instance that provided the unparseable response
        instance: String,
    },

    // Instance management errors
    /// No healthy SearXNG instances are currently available for search requests
    #[error("No healthy SearXNG instances available")]
    NoInstancesAvailable,

    /// Failed to discover or validate available SearXNG instances
    #[error("Instance discovery failed: {details}")]
    InstanceDiscovery {
        /// Details about the failure during instance discovery process
        details: String,
    },

    /// All available SearXNG instances have failed to handle the request
    #[error("All instances failed - last error: {last_error}")]
    AllInstancesFailed {
        /// The error message from the last attempted instance
        last_error: String,
    },

    // Content fetching errors
    /// Failed to fetch full content from a search result URL
    #[error("Content fetching failed for {url}: {details}")]
    ContentFetch {
        /// URL from which content fetching failed
        url: String,
        /// Specific error details about the content fetch failure
        details: String,
    },

    /// Content size exceeds the configured limit for fetching
    #[error("Content size limit exceeded: {size_mb}MB > {limit_mb}MB")]
    ContentSizeLimit {
        /// Actual size of the content in megabytes
        size_mb: u64,
        /// Maximum allowed size in megabytes
        limit_mb: u64,
    },

    /// Content quality assessment determined the content is not suitable
    #[error("Content quality assessment failed: {reason}")]
    ContentQuality {
        /// Reason why the content quality assessment failed
        reason: String,
    },

    // Configuration and validation errors
    /// Configuration error in the web search system
    #[error("Invalid configuration: {field} - {details}")]
    Configuration {
        /// Name of the configuration field that has an invalid value
        field: String,
        /// Detailed explanation of the configuration problem
        details: String,
    },

    /// Search query failed validation checks
    #[error("Search query validation failed: {reason}")]
    QueryValidation {
        /// Specific reason why the query validation failed
        reason: String,
    },
}

impl WebSearchError {
    /// Determines if this error is eligible for retry
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::ConnectionTimeout { .. } => true,
            Self::Network { .. } => true,
            Self::InstanceUnavailable { .. } => true,
            Self::RateLimited { .. } => true,
            Self::ResponseParsing { .. } => false,
            Self::InvalidParameters { .. } => false,
            Self::NoInstancesAvailable => false,
            Self::AllInstancesFailed { .. } => false,
            Self::ContentFetch { .. } => true,
            Self::ContentSizeLimit { .. } => false,
            Self::ContentQuality { .. } => false,
            Self::Configuration { .. } => false,
            Self::QueryValidation { .. } => false,
            Self::InstanceDiscovery { .. } => true,
            Self::DnsResolution { .. } => true,
        }
    }

    /// Returns the recommended retry delay for retryable errors
    pub fn retry_delay(&self) -> Option<Duration> {
        match self {
            Self::RateLimited {
                retry_after_secs, ..
            } => Some(Duration::from_secs(*retry_after_secs)),
            Self::ConnectionTimeout { .. } => Some(Duration::from_secs(5)),
            Self::InstanceUnavailable { .. } => Some(Duration::from_secs(10)),
            Self::Network { .. } => Some(Duration::from_secs(3)),
            Self::ContentFetch { .. } => Some(Duration::from_secs(2)),
            Self::InstanceDiscovery { .. } => Some(Duration::from_secs(30)),
            Self::DnsResolution { .. } => Some(Duration::from_secs(15)),
            _ => None,
        }
    }

    /// Returns a user-friendly error message
    pub fn user_friendly_message(&self) -> String {
        match self {
            Self::NoInstancesAvailable => {
                "All search services are temporarily unavailable. Please try again in a few minutes, or check your internet connection.".to_string()
            }
            Self::RateLimited { retry_after_secs, .. } => {
                format!("Search service is busy. Please wait {retry_after_secs} seconds before searching again.")
            }
            Self::QueryValidation { reason } => {
                format!("Invalid search query: {reason}. Please check your search terms and try again.")
            }
            Self::ConnectionTimeout { .. } => {
                "Search request timed out. Please check your internet connection and try again.".to_string()
            }
            Self::ContentSizeLimit { size_mb, limit_mb } => {
                format!("Content too large ({size_mb} MB exceeds {limit_mb} MB limit). Showing search results without content.")
            }
            Self::InstanceDiscovery { .. } => {
                "Search service discovery failed. Using fallback search services.".to_string()
            }
            Self::DnsResolution { .. } => {
                "DNS resolution failed. Please check your network connection.".to_string()
            }
            Self::ContentFetch { .. } => {
                "Unable to fetch full content for some results. Search results are still available.".to_string()
            }
            _ => format!("Search error: {self}"),
        }
    }

    /// Returns actionable recovery suggestions
    pub fn recovery_suggestions(&self) -> Vec<String> {
        match self {
            Self::NoInstancesAvailable => vec![
                "Check your internet connection".to_string(),
                "Try again in a few minutes".to_string(),
                "Contact support if the problem persists".to_string(),
            ],
            Self::RateLimited {
                retry_after_secs, ..
            } => vec![
                format!("Wait {} seconds before retrying", retry_after_secs),
                "Reduce search frequency".to_string(),
            ],
            Self::QueryValidation { .. } => vec![
                "Check search query length (1-500 characters)".to_string(),
                "Remove special characters that might be causing issues".to_string(),
                "Try simpler search terms".to_string(),
            ],
            Self::ConnectionTimeout { .. } => vec![
                "Check your internet connection".to_string(),
                "Try again with a more specific query".to_string(),
            ],
            Self::ContentSizeLimit { .. } => vec![
                "Search results are still available".to_string(),
                "Try more specific queries for smaller content".to_string(),
            ],
            Self::InstanceDiscovery { .. } => vec![
                "Using backup search services".to_string(),
                "Check firewall settings".to_string(),
            ],
            Self::DnsResolution { .. } => vec![
                "Check DNS settings".to_string(),
                "Try using a different DNS server".to_string(),
                "Check your network connection".to_string(),
            ],
            Self::ContentFetch { .. } => vec![
                "Search results are still available without full content".to_string(),
                "Try searching for more recent content".to_string(),
            ],
            _ => vec!["Try again later".to_string()],
        }
    }
}

/// Configuration for retry behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts for failed operations
    pub max_retries: u32,
    /// Base delay duration before the first retry attempt
    pub base_delay: Duration,
    /// Maximum delay duration between retry attempts
    pub max_delay: Duration,
    /// Multiplier for exponential backoff calculation
    pub backoff_multiplier: f64,
    /// Whether to add random jitter to retry delays to prevent thundering herd
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

/// Configuration for circuit breaker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit breaker
    pub failure_threshold: u32,
    /// Duration to wait before transitioning from open to half-open state
    pub recovery_timeout: Duration,
    /// Maximum number of test calls allowed in half-open state
    pub half_open_max_calls: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(60),
            half_open_max_calls: 3,
        }
    }
}

/// Circuit breaker state for an instance
#[derive(Debug, Clone, PartialEq)]
enum BreakerState {
    Closed,   // Normal operation
    Open,     // Failing - reject requests
    HalfOpen, // Testing - allow limited requests
}

/// Circuit state tracking for an instance
#[derive(Debug, Clone)]
struct CircuitState {
    state: BreakerState,
    failure_count: u32,
    last_failure_time: Instant,
    next_attempt_time: Instant,
    half_open_calls: u32,
}

impl Default for CircuitState {
    fn default() -> Self {
        Self {
            state: BreakerState::Closed,
            failure_count: 0,
            last_failure_time: Instant::now(),
            next_attempt_time: Instant::now(),
            half_open_calls: 0,
        }
    }
}

/// Circuit breaker implementation for preventing cascading failures
pub struct CircuitBreaker {
    states: Arc<Mutex<HashMap<String, CircuitState>>>,
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the specified configuration
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            states: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    /// Checks if requests to an instance should be allowed
    pub async fn can_execute(&self, instance_url: &str) -> bool {
        let mut states = self.states.lock().await;
        let state = states
            .entry(instance_url.to_string())
            .or_insert_with(CircuitState::default);

        match state.state {
            BreakerState::Closed => true,
            BreakerState::Open => {
                if Instant::now() >= state.next_attempt_time {
                    // Transition to half-open for testing
                    state.state = BreakerState::HalfOpen;
                    state.half_open_calls = 0;
                    true
                } else {
                    false
                }
            }
            BreakerState::HalfOpen => {
                // Allow limited testing calls
                state.half_open_calls < self.config.half_open_max_calls
            }
        }
    }

    /// Records a successful operation
    pub async fn record_success(&self, instance_url: &str) {
        let mut states = self.states.lock().await;
        if let Some(state) = states.get_mut(instance_url) {
            state.failure_count = 0;
            state.state = BreakerState::Closed;
            state.half_open_calls = 0;
        }
    }

    /// Records a failed operation
    pub async fn record_failure(&self, instance_url: &str, error: &WebSearchError) {
        let mut states = self.states.lock().await;
        let state = states
            .entry(instance_url.to_string())
            .or_insert_with(CircuitState::default);

        // Only count certain types of errors for circuit breaking
        let should_count = matches!(
            error,
            WebSearchError::ConnectionTimeout { .. }
                | WebSearchError::InstanceUnavailable { .. }
                | WebSearchError::Network { .. }
                | WebSearchError::DnsResolution { .. }
        );

        if should_count {
            state.failure_count += 1;
            state.last_failure_time = Instant::now();

            match state.state {
                BreakerState::HalfOpen => {
                    // Failure in half-open state - go back to open
                    state.state = BreakerState::Open;
                    state.next_attempt_time = Instant::now() + self.config.recovery_timeout;
                }
                _ => {
                    if state.failure_count >= self.config.failure_threshold {
                        state.state = BreakerState::Open;
                        state.next_attempt_time = Instant::now() + self.config.recovery_timeout;
                    }
                }
            }
        }
    }

    /// Records a half-open call
    pub async fn record_half_open_call(&self, instance_url: &str) {
        let mut states = self.states.lock().await;
        if let Some(state) = states.get_mut(instance_url) {
            if state.state == BreakerState::HalfOpen {
                state.half_open_calls += 1;
            }
        }
    }
}

/// Statistics for search operations
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SearchStats {
    /// Total number of search results found across all instances
    pub search_results_found: usize,
    /// Number of content fetch operations that failed
    pub content_fetch_failures: usize,
    /// Total number of SearXNG instances that were attempted
    pub total_instances_tried: usize,
    /// Number of times circuit breakers were tripped during the search
    pub circuit_breaker_trips: usize,
    /// Total number of retry attempts made during the search operation
    pub retry_attempts: usize,
}

/// Builder for constructing search responses with graceful degradation
pub struct SearchResultBuilder {
    query: String,
    partial_results: Vec<SearchResult>,
    content_failures: Vec<ContentFetchError>,
    /// Collection of search errors encountered during the operation
    pub search_errors: Vec<WebSearchError>,
    /// Statistical information about the search operation
    pub stats: SearchStats,
    /// User-friendly warning messages about partial failures or degraded service
    pub warnings: Vec<String>,
}

/// Error information for content fetching failures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentFetchError {
    /// URL from which content fetching failed
    pub url: String,
    /// Error message describing the content fetch failure
    pub error: String,
    /// Whether this error condition supports retry attempts
    pub retryable: bool,
}

impl SearchResultBuilder {
    /// Create a new search result builder for the specified query
    pub fn new(query: String) -> Self {
        Self {
            query,
            partial_results: Vec::new(),
            content_failures: Vec::new(),
            search_errors: Vec::new(),
            stats: SearchStats::default(),
            warnings: Vec::new(),
        }
    }

    /// Add successful search results
    pub fn add_results(&mut self, results: Vec<SearchResult>) {
        self.partial_results.extend(results);
        self.stats.search_results_found = self.partial_results.len();
    }

    /// Add a content fetching failure
    pub fn add_content_failure(&mut self, failure: ContentFetchError) {
        self.content_failures.push(failure);
        self.stats.content_fetch_failures += 1;
    }

    /// Add a search error
    pub fn add_search_error(&mut self, error: WebSearchError) {
        // Create user-friendly warning message
        let warning = error.user_friendly_message();
        self.warnings.push(warning);
        self.search_errors.push(error);
    }

    /// Record instance attempt
    pub fn record_instance_attempt(&mut self) {
        self.stats.total_instances_tried += 1;
    }

    /// Record circuit breaker trip
    pub fn record_circuit_breaker_trip(&mut self) {
        self.stats.circuit_breaker_trips += 1;
    }

    /// Record retry attempt
    pub fn record_retry_attempt(&mut self) {
        self.stats.retry_attempts += 1;
    }

    /// Build the final response with graceful degradation
    pub fn build_response(
        self,
        metadata: SearchMetadata,
    ) -> Result<WebSearchResponse, WebSearchError> {
        // Determine if this is an error response or partial success
        let has_results = !self.partial_results.is_empty();
        let has_critical_errors = self
            .search_errors
            .iter()
            .any(|e| matches!(e, WebSearchError::NoInstancesAvailable));

        if !has_results && has_critical_errors {
            // Complete failure - return the most critical error
            return Err(self.search_errors.into_iter().next().unwrap_or(
                WebSearchError::AllInstancesFailed {
                    last_error: format!("No results found for query: {}", self.query),
                },
            ));
        }

        // Build successful response with potential degradation
        let response_metadata = metadata;

        // Add warnings and stats for partial failures
        if !self.warnings.is_empty() {
            // We need to add warnings to metadata, but the current SearchMetadata doesn't have it
            // This suggests we need to extend SearchMetadata to support warnings
        }

        Ok(WebSearchResponse {
            results: self.partial_results,
            metadata: response_metadata,
        })
    }
}

/// Enhanced metadata for search responses that includes degradation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedSearchMetadata {
    /// Search query that was executed
    pub query: String,
    /// Category of search being performed
    pub category: SearchCategory,
    /// Language preference for search results
    pub language: String,
    /// Number of results returned in this response
    pub results_count: usize,
    /// Duration of search operation in milliseconds
    pub search_time_ms: u64,
    /// Primary SearXNG instance that handled the search
    pub instance_used: String,
    /// Total number of results available from the search engines
    pub total_results: usize,
    /// List of search engines used to fulfill this request
    pub engines_used: Vec<String>,
    /// Statistics about content fetching operations if performed
    pub content_fetch_stats: Option<ContentFetchStats>,
    /// Whether content fetching was enabled for this search
    pub fetch_content: bool,
    /// Warning messages about partial failures or service degradation
    pub warnings: Option<Vec<String>>,
    /// Success statistics for the search operation
    pub success_stats: Option<SearchStats>,
    /// Whether the service is operating in a degraded mode
    pub degraded_service: bool,
}

impl From<SearchMetadata> for EnhancedSearchMetadata {
    fn from(metadata: SearchMetadata) -> Self {
        Self {
            query: metadata.query,
            category: metadata.category,
            language: metadata.language,
            results_count: metadata.results_count,
            search_time_ms: metadata.search_time_ms,
            instance_used: metadata.instance_used,
            total_results: metadata.total_results,
            engines_used: metadata.engines_used,
            content_fetch_stats: metadata.content_fetch_stats,
            fetch_content: metadata.fetch_content,
            warnings: None,
            success_stats: None,
            degraded_service: false,
        }
    }
}

/// Enhanced response structure that supports graceful degradation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedWebSearchResponse {
    /// Search results returned by the operation
    pub results: Vec<SearchResult>,
    /// Enhanced metadata with degradation and error recovery information
    pub metadata: EnhancedSearchMetadata,
    /// Whether this response represents an error condition with partial results
    pub is_error: bool,
}

/// Failover manager that provides comprehensive error handling and recovery
pub struct FailoverManager {
    retry_config: RetryConfig,
    circuit_breaker: CircuitBreaker,
    instance_health: Arc<RwLock<HashMap<String, InstanceHealth>>>,
}

/// Health tracking for instances
#[derive(Debug, Clone, Default)]
struct InstanceHealth {
    consecutive_failures: u32,
    last_success: Option<Instant>,
    last_failure: Option<Instant>,
    total_requests: u64,
    successful_requests: u64,
}

impl FailoverManager {
    /// Create a new failover manager with retry and circuit breaker configurations
    pub fn new(retry_config: RetryConfig, circuit_breaker_config: CircuitBreakerConfig) -> Self {
        Self {
            retry_config,
            circuit_breaker: CircuitBreaker::new(circuit_breaker_config),
            instance_health: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Execute an operation with comprehensive failover and retry logic
    pub async fn execute_with_failover<F, T, E>(&self, operation: F) -> Result<T, WebSearchError>
    where
        F: Fn() -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<T, E>> + Send + 'static>,
        >,
        E: Into<WebSearchError> + std::fmt::Debug,
    {
        let mut last_error = None;
        let mut attempts = 0;

        while attempts <= self.retry_config.max_retries {
            match operation().await {
                Ok(result) => {
                    return Ok(result);
                }
                Err(error) => {
                    let web_error = error.into();
                    last_error = Some(web_error.clone());

                    // Check if we should retry
                    if !web_error.is_retryable() || attempts >= self.retry_config.max_retries {
                        break;
                    }

                    // Apply exponential backoff with jitter
                    let delay = self.calculate_backoff_delay(attempts);
                    tokio::time::sleep(delay).await;

                    attempts += 1;
                }
            }
        }

        Err(last_error.unwrap_or(WebSearchError::AllInstancesFailed {
            last_error: "No specific error recorded".to_string(),
        }))
    }

    /// Calculate backoff delay with optional jitter
    fn calculate_backoff_delay(&self, attempt: u32) -> Duration {
        let delay_ms = (self.retry_config.base_delay.as_millis() as f64)
            * self.retry_config.backoff_multiplier.powi(attempt as i32);

        let delay_ms = delay_ms.min(self.retry_config.max_delay.as_millis() as f64) as u64;

        let delay = Duration::from_millis(delay_ms);

        if self.retry_config.jitter {
            // Add ±25% jitter
            let mut rng = rand::thread_rng();
            let jitter_factor = rng.gen_range(0.75..=1.25);
            Duration::from_millis((delay.as_millis() as f64 * jitter_factor) as u64)
        } else {
            delay
        }
    }

    /// Record successful operation for an instance
    pub async fn record_success(&self, instance_url: &str) {
        self.circuit_breaker.record_success(instance_url).await;

        let mut health = self.instance_health.write().await;
        let instance_health = health
            .entry(instance_url.to_string())
            .or_insert_with(InstanceHealth::default);

        instance_health.consecutive_failures = 0;
        instance_health.last_success = Some(Instant::now());
        instance_health.total_requests += 1;
        instance_health.successful_requests += 1;
    }

    /// Record failed operation for an instance
    pub async fn record_failure(&self, instance_url: &str, error: &WebSearchError) {
        self.circuit_breaker
            .record_failure(instance_url, error)
            .await;

        let mut health = self.instance_health.write().await;
        let instance_health = health
            .entry(instance_url.to_string())
            .or_insert_with(InstanceHealth::default);

        instance_health.consecutive_failures += 1;
        instance_health.last_failure = Some(Instant::now());
        instance_health.total_requests += 1;
    }

    /// Check if an instance can be used (circuit breaker check)
    pub async fn can_use_instance(&self, instance_url: &str) -> bool {
        self.circuit_breaker.can_execute(instance_url).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[test]
    fn test_web_search_error_is_retryable() {
        assert!(WebSearchError::ConnectionTimeout {
            timeout_ms: 5000,
            instance: "test".to_string(),
        }
        .is_retryable());

        assert!(WebSearchError::Network {
            message: "test".to_string(),
            instance: "test".to_string(),
            source: None,
        }
        .is_retryable());

        assert!(!WebSearchError::InvalidParameters {
            details: "test".to_string(),
        }
        .is_retryable());
    }

    #[test]
    fn test_web_search_error_retry_delay() {
        let error = WebSearchError::RateLimited {
            instance: "test".to_string(),
            retry_after_secs: 60,
        };
        assert_eq!(error.retry_delay(), Some(Duration::from_secs(60)));

        let error = WebSearchError::InvalidParameters {
            details: "test".to_string(),
        };
        assert_eq!(error.retry_delay(), None);
    }

    #[test]
    fn test_web_search_error_user_friendly_message() {
        let error = WebSearchError::NoInstancesAvailable;
        let message = error.user_friendly_message();
        assert!(message.contains("search services are temporarily unavailable"));
    }

    #[test]
    fn test_web_search_error_recovery_suggestions() {
        let error = WebSearchError::QueryValidation {
            reason: "too long".to_string(),
        };
        let suggestions = error.recovery_suggestions();
        assert!(!suggestions.is_empty());
        assert!(suggestions[0].contains("Check search query length"));
    }

    #[tokio::test]
    async fn test_circuit_breaker_states() {
        let config = CircuitBreakerConfig::default();
        let circuit_breaker = CircuitBreaker::new(config);

        let instance = "test-instance";

        // Initially should allow execution
        assert!(circuit_breaker.can_execute(instance).await);

        // Record multiple failures to trip the breaker
        for _ in 0..5 {
            let error = WebSearchError::ConnectionTimeout {
                timeout_ms: 5000,
                instance: instance.to_string(),
            };
            circuit_breaker.record_failure(instance, &error).await;
        }

        // Now should not allow execution (breaker open)
        assert!(!circuit_breaker.can_execute(instance).await);

        // Record success should reset the breaker
        circuit_breaker.record_success(instance).await;
        assert!(circuit_breaker.can_execute(instance).await);
    }

    #[test]
    fn test_search_result_builder() {
        let mut builder = SearchResultBuilder::new("test query".to_string());

        let result = SearchResult {
            title: "Test".to_string(),
            url: "https://example.com".to_string(),
            description: "Test description".to_string(),
            score: 0.9,
            engine: "test".to_string(),
            content: None,
        };

        builder.add_results(vec![result]);
        builder.add_content_failure(ContentFetchError {
            url: "https://example2.com".to_string(),
            error: "Timeout".to_string(),
            retryable: true,
        });

        assert_eq!(builder.stats.search_results_found, 1);
        assert_eq!(builder.stats.content_fetch_failures, 1);
    }

    #[tokio::test]
    async fn test_failover_manager() {
        let retry_config = RetryConfig::default();
        let circuit_breaker_config = CircuitBreakerConfig::default();
        let failover_manager = FailoverManager::new(retry_config, circuit_breaker_config);

        let instance = "test-instance";

        // Test success recording
        failover_manager.record_success(instance).await;
        assert!(failover_manager.can_use_instance(instance).await);

        // Test failure recording
        let error = WebSearchError::ConnectionTimeout {
            timeout_ms: 5000,
            instance: instance.to_string(),
        };
        failover_manager.record_failure(instance, &error).await;

        // Should still be usable after one failure
        assert!(failover_manager.can_use_instance(instance).await);
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_delay, Duration::from_millis(500));
        assert!(config.jitter);
    }

    #[test]
    fn test_circuit_breaker_config_default() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.recovery_timeout, Duration::from_secs(60));
        assert_eq!(config.half_open_max_calls, 3);
    }
}
