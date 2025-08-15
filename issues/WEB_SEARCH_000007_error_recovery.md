# WEB_SEARCH_000007: Error Handling and Recovery

Refer to /Users/wballard/github/sah-search/ideas/web_search.md

## Overview
Implement comprehensive error handling, recovery strategies, and graceful degradation for web search operations.

## Goals
- Handle all categories of errors defined in the specification
- Implement automatic failover to backup instances
- Provide graceful degradation when partial failures occur
- Create clear, actionable error messages for users
- Add retry logic with exponential backoff
- Support partial result delivery when some operations fail

## Tasks
1. **Error Type System**: Define comprehensive error types for web search
2. **Failover Logic**: Automatic failover to backup SearXNG instances  
3. **Graceful Degradation**: Return partial results when possible
4. **Retry Mechanisms**: Exponential backoff with configurable limits
5. **Error Recovery**: Automatic recovery from transient failures

## Implementation Details

### Comprehensive Error Type System
```rust
#[derive(Debug, thiserror::Error)]
pub enum WebSearchError {
    // Network and connectivity errors
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("Connection timeout after {timeout_ms}ms")]
    ConnectionTimeout { timeout_ms: u64 },
    
    #[error("DNS resolution failed for {host}")]
    DnsResolution { host: String },
    
    // SearXNG API errors
    #[error("SearXNG instance unavailable: {url}")]
    InstanceUnavailable { url: String },
    
    #[error("Rate limited by {instance} - retry after {retry_after_secs}s")]
    RateLimited { instance: String, retry_after_secs: u64 },
    
    #[error("Invalid search parameters: {details}")]
    InvalidParameters { details: String },
    
    #[error("API response parsing failed: {details}")]
    ResponseParsing { details: String },
    
    // Instance management errors
    #[error("No healthy SearXNG instances available")]
    NoInstancesAvailable,
    
    #[error("Instance discovery failed: {details}")]
    InstanceDiscovery { details: String },
    
    #[error("All instances failed - last error: {last_error}")]
    AllInstancesFailed { last_error: String },
    
    // Content fetching errors
    #[error("Content fetching failed for {url}: {details}")]
    ContentFetch { url: String, details: String },
    
    #[error("Content size limit exceeded: {size_mb}MB > {limit_mb}MB")]
    ContentSizeLimit { size_mb: u64, limit_mb: u64 },
    
    #[error("Content quality assessment failed: {reason}")]
    ContentQuality { reason: String },
    
    // Configuration and validation errors
    #[error("Invalid configuration: {field} - {details}")]
    Configuration { field: String, details: String },
    
    #[error("Search query validation failed: {reason}")]
    QueryValidation { reason: String },
}

impl WebSearchError {
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::ConnectionTimeout { .. } => true,
            Self::Network(e) => e.is_timeout() || e.is_connect(),
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
    
    pub fn retry_delay(&self) -> Option<Duration> {
        match self {
            Self::RateLimited { retry_after_secs, .. } => {
                Some(Duration::from_secs(*retry_after_secs))
            }
            Self::ConnectionTimeout { .. } => Some(Duration::from_secs(5)),
            Self::InstanceUnavailable { .. } => Some(Duration::from_secs(10)),
            _ => None,
        }
    }
}
```

### Failover and Recovery Manager
```rust
pub struct FailoverManager {
    instance_manager: Arc<InstanceManager>,
    retry_config: RetryConfig,
    circuit_breaker: CircuitBreaker,
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
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

impl FailoverManager {
    pub async fn execute_with_failover<F, T>(
        &self,
        operation: F,
    ) -> Result<T, WebSearchError>
    where
        F: Fn(&SearxInstance) -> Pin<Box<dyn Future<Output = Result<T, WebSearchError>> + Send>> + Send + Sync,
        T: Send,
    {
        let mut last_error = None;
        let mut attempts = 0;
        
        while attempts <= self.retry_config.max_retries {
            // Get next healthy instance
            if let Some(instance) = self.instance_manager.get_next_healthy_instance().await {
                match operation(instance).await {
                    Ok(result) => {
                        // Success - reset circuit breaker for this instance
                        self.circuit_breaker.record_success(&instance.url);
                        return Ok(result);
                    }
                    Err(error) => {
                        // Record failure
                        self.circuit_breaker.record_failure(&instance.url, &error);
                        self.instance_manager.record_instance_failure(&instance.url, &error).await;
                        
                        last_error = Some(error.clone());
                        
                        // Check if we should retry
                        if !error.is_retryable() || attempts >= self.retry_config.max_retries {
                            break;
                        }
                        
                        // Apply exponential backoff with jitter
                        let delay = self.calculate_backoff_delay(attempts);
                        tokio::time::sleep(delay).await;
                    }
                }
            } else {
                // No instances available
                return Err(WebSearchError::NoInstancesAvailable);
            }
            
            attempts += 1;
        }
        
        Err(last_error.unwrap_or(WebSearchError::AllInstancesFailed {
            last_error: "No specific error recorded".to_string(),
        }))
    }
    
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
}
```

### Circuit Breaker Implementation
```rust
pub struct CircuitBreaker {
    states: Arc<DashMap<String, CircuitState>>,
    config: CircuitBreakerConfig,
}

#[derive(Debug, Clone)]
struct CircuitState {
    state: BreakerState,
    failure_count: u32,
    last_failure_time: Instant,
    next_attempt_time: Instant,
}

#[derive(Debug, Clone, PartialEq)]
enum BreakerState {
    Closed,      // Normal operation
    Open,        // Failing - reject requests
    HalfOpen,    // Testing - allow limited requests
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub recovery_timeout: Duration,
    pub half_open_max_calls: u32,
}

impl CircuitBreaker {
    pub fn can_execute(&self, instance_url: &str) -> bool {
        let state = self.states
            .entry(instance_url.to_string())
            .or_insert_with(|| CircuitState {
                state: BreakerState::Closed,
                failure_count: 0,
                last_failure_time: Instant::now(),
                next_attempt_time: Instant::now(),
            });
        
        match state.state {
            BreakerState::Closed => true,
            BreakerState::Open => {
                if Instant::now() >= state.next_attempt_time {
                    // Transition to half-open for testing
                    state.state = BreakerState::HalfOpen;
                    true
                } else {
                    false
                }
            }
            BreakerState::HalfOpen => true, // Allow limited testing
        }
    }
    
    pub fn record_success(&self, instance_url: &str) {
        if let Some(mut state) = self.states.get_mut(instance_url) {
            state.failure_count = 0;
            state.state = BreakerState::Closed;
        }
    }
    
    pub fn record_failure(&self, instance_url: &str, error: &WebSearchError) {
        let mut state = self.states
            .entry(instance_url.to_string())
            .or_insert_with(|| CircuitState {
                state: BreakerState::Closed,
                failure_count: 0,
                last_failure_time: Instant::now(),
                next_attempt_time: Instant::now(),
            });
        
        state.failure_count += 1;
        state.last_failure_time = Instant::now();
        
        // Only count certain types of errors for circuit breaking
        let should_count = match error {
            WebSearchError::ConnectionTimeout { .. } => true,
            WebSearchError::InstanceUnavailable { .. } => true,
            WebSearchError::Network(_) => true,
            _ => false,
        };
        
        if should_count && state.failure_count >= self.config.failure_threshold {
            state.state = BreakerState::Open;
            state.next_attempt_time = Instant::now() + self.config.recovery_timeout;
        }
    }
}
```

### Graceful Degradation
```rust
pub struct SearchResultBuilder {
    query: String,
    partial_results: Vec<SearchResult>,
    content_failures: Vec<ContentFetchError>,
    search_errors: Vec<WebSearchError>,
}

impl SearchResultBuilder {
    pub fn build_response(self) -> WebSearchResponse {
        let mut warnings = Vec::new();
        let mut success_stats = SearchStats::default();
        
        // Handle partial search results
        if !self.partial_results.is_empty() {
            success_stats.search_results_found = self.partial_results.len();
        }
        
        // Handle content fetching failures
        if !self.content_failures.is_empty() {
            warnings.push(format!(
                "Content fetching failed for {} results", 
                self.content_failures.len()
            ));
            success_stats.content_fetch_failures = self.content_failures.len();
        }
        
        // Handle search errors
        for error in &self.search_errors {
            warnings.push(format!("Search warning: {}", error));
        }
        
        WebSearchResponse {
            query: self.query,
            results: self.partial_results,
            is_error: false, // Partial success is not an error
            metadata: SearchMetadata {
                warnings: if warnings.is_empty() { None } else { Some(warnings) },
                success_stats: Some(success_stats),
                degraded_service: !self.search_errors.is_empty() || !self.content_failures.is_empty(),
                ..Default::default()
            },
        }
    }
}
```

### Enhanced Error Messages
```rust
impl WebSearchError {
    pub fn user_friendly_message(&self) -> String {
        match self {
            Self::NoInstancesAvailable => {
                "All search services are temporarily unavailable. Please try again in a few minutes, or check your internet connection.".to_string()
            }
            Self::RateLimited { retry_after_secs, .. } => {
                format!("Search service is busy. Please wait {} seconds before searching again.", retry_after_secs)
            }
            Self::QueryValidation { reason } => {
                format!("Invalid search query: {}. Please check your search terms and try again.", reason)
            }
            Self::ConnectionTimeout { .. } => {
                "Search request timed out. Please check your internet connection and try again.".to_string()
            }
            Self::ContentSizeLimit { size_mb, limit_mb } => {
                format!("Content too large ({} MB exceeds {} MB limit). Showing search results without content.", size_mb, limit_mb)
            }
            _ => format!("Search error: {}", self),
        }
    }
    
    pub fn recovery_suggestions(&self) -> Vec<String> {
        match self {
            Self::NoInstancesAvailable => vec![
                "Check your internet connection".to_string(),
                "Try again in a few minutes".to_string(),
                "Contact support if the problem persists".to_string(),
            ],
            Self::RateLimited { retry_after_secs, .. } => vec![
                format!("Wait {} seconds before retrying", retry_after_secs),
                "Reduce search frequency".to_string(),
            ],
            Self::QueryValidation { .. } => vec![
                "Check search query length (1-500 characters)".to_string(),
                "Remove special characters that might be causing issues".to_string(),
                "Try simpler search terms".to_string(),
            ],
            _ => vec!["Try again later".to_string()],
        }
    }
}
```

## Success Criteria
- [x] All error types properly handled with appropriate recovery strategies
- [x] Automatic failover successfully switches between instances
- [x] Partial results delivered when some operations fail
- [x] Circuit breaker prevents cascading failures
- [x] User-friendly error messages with actionable suggestions
- [x] Exponential backoff prevents overwhelming failed services

## Testing Strategy
- Error injection tests for all error types
- Failover tests with multiple failing instances
- Circuit breaker tests with failure thresholds
- Graceful degradation tests with partial failures
- Recovery timing tests for exponential backoff
- User experience tests for error message clarity

## Integration Points
- Integrates with all previous web search components
- Uses existing error handling patterns from the codebase
- Enhances MCP tool responses with error information
- Provides CLI with detailed error reporting
- Maintains backwards compatibility with existing APIs

## Configuration Options
```toml
[web_search.error_handling]
# Retry configuration
max_retries = 3
base_retry_delay = 500        # milliseconds
max_retry_delay = 30000       # milliseconds (30 seconds)
backoff_multiplier = 2.0
enable_jitter = true

# Circuit breaker
circuit_breaker_failure_threshold = 5
circuit_breaker_recovery_timeout = 60000  # milliseconds (1 minute)
circuit_breaker_half_open_max_calls = 3

# Graceful degradation
allow_partial_results = true
content_fetch_timeout = 45    # seconds
max_content_failures = 50     # percent of content fetches that can fail

# Error reporting
include_error_details = false # for privacy
user_friendly_messages = true
include_recovery_suggestions = true
```

## Error Response Examples

### Partial Success Response
```json
{
  "content": [{
    "type": "text", 
    "text": "Found 8 search results (content fetching failed for 2 results)"
  }],
  "is_error": false,
  "metadata": {
    "query": "rust programming",
    "degraded_service": true,
    "warnings": ["Content fetching failed for 2 results"],
    "success_stats": {
      "search_results_found": 8,
      "content_fetch_failures": 2,
      "total_instances_tried": 2
    }
  }
}
```

### Complete Failure Response
```json
{
  "content": [{
    "type": "text",
    "text": "Search failed: All search services are temporarily unavailable"
  }],
  "is_error": true,
  "metadata": {
    "query": "rust programming", 
    "error_type": "no_instances_available",
    "error_details": "All SearXNG instances failed health checks",
    "recovery_suggestions": [
      "Check your internet connection",
      "Try again in a few minutes",
      "Contact support if the problem persists"
    ],
    "retry_after": 300
  }
}
```