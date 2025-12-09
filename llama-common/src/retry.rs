//! Unified retry utilities for consistent error handling across all crates

use crate::error::LlamaError;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Whether to add jitter to delays
    pub use_jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(1000),
            backoff_multiplier: 2.0,
            max_delay: Duration::from_secs(30),
            use_jitter: true,
        }
    }
}

/// Trait for errors that can be classified for retry behavior
pub trait RetryableError: std::error::Error + Send + Sync {
    /// Check if this specific error instance should be retried
    fn is_retriable(&self) -> bool;

    /// Get custom retry delay for this error, if any
    fn custom_retry_delay(&self, _attempt: u32) -> Option<Duration> {
        None
    }

    /// Check if retrying should stop regardless of attempt count
    fn should_stop_retrying(&self, _attempt: u32) -> bool {
        false
    }
}

/// Automatic implementation of RetryableError for all LlamaError types
impl<T: LlamaError> RetryableError for T {
    fn is_retriable(&self) -> bool {
        // Use the existing LlamaError trait method
        LlamaError::is_retriable(self)
    }
}

/// Manager for retry operations with exponential backoff and jitter
pub struct RetryManager {
    config: RetryConfig,
}

impl RetryManager {
    /// Create a new retry manager with default configuration
    pub fn new() -> Self {
        Self {
            config: RetryConfig::default(),
        }
    }

    /// Create a retry manager with custom configuration
    pub fn with_config(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Execute an operation with retry logic
    pub async fn retry<F, T, E, Fut>(&self, operation_name: &str, mut operation: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: RetryableError + std::fmt::Display,
    {
        let mut attempt = 0;
        let mut delay = self.config.initial_delay;

        loop {
            match operation().await {
                Ok(result) => {
                    if attempt > 0 {
                        info!(
                            "Operation '{}' succeeded after {} retries",
                            operation_name, attempt
                        );
                    }
                    return Ok(result);
                }
                Err(error) => {
                    attempt += 1;

                    // Check if we should retry this error
                    if !error.is_retriable() {
                        warn!(
                            "Operation '{}' failed with non-retriable error: {}",
                            operation_name, error
                        );
                        return Err(error);
                    }

                    // Check if we've exceeded max retries
                    if attempt > self.config.max_retries {
                        warn!(
                            "Operation '{}' failed after {} attempts: {}",
                            operation_name,
                            attempt - 1,
                            error
                        );
                        return Err(error);
                    }

                    // Check if error says to stop retrying
                    if error.should_stop_retrying(attempt) {
                        warn!(
                            "Operation '{}' stopped retrying at attempt {}: {}",
                            operation_name, attempt, error
                        );
                        return Err(error);
                    }

                    // Use custom delay if provided, otherwise use exponential backoff
                    let actual_delay = error
                        .custom_retry_delay(attempt)
                        .unwrap_or_else(|| self.calculate_delay(delay, attempt));

                    warn!(
                        "Operation '{}' attempt {} failed: {}. Retrying in {:?}...",
                        operation_name, attempt, error, actual_delay
                    );

                    sleep(actual_delay).await;

                    // Update delay for next attempt
                    delay = self.calculate_next_delay(delay);
                }
            }
        }
    }

    /// Calculate delay with exponential backoff and optional jitter
    fn calculate_delay(&self, base_delay: Duration, _attempt: u32) -> Duration {
        let mut delay = base_delay;

        // Add jitter if enabled (up to 25% of the delay)
        if self.config.use_jitter {
            let jitter_ms = (delay.as_millis() as f64 * 0.25 * pseudo_random()) as u64;
            delay += Duration::from_millis(jitter_ms);
        }

        delay.min(self.config.max_delay)
    }

    /// Calculate the next delay using exponential backoff
    fn calculate_next_delay(&self, current_delay: Duration) -> Duration {
        let next_delay_ms =
            (current_delay.as_millis() as f64 * self.config.backoff_multiplier) as u64;
        Duration::from_millis(next_delay_ms).min(self.config.max_delay)
    }
}

impl Default for RetryManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Enhanced error classification for common error patterns
pub fn classify_error_for_retry(error: &dyn std::error::Error) -> bool {
    let error_msg = error.to_string().to_lowercase();

    // Server errors (5xx) are typically retriable
    if error_msg.contains("500") || error_msg.contains("internal server error") {
        return true;
    }
    if error_msg.contains("502") || error_msg.contains("bad gateway") {
        return true;
    }
    if error_msg.contains("503") || error_msg.contains("service unavailable") {
        return true;
    }
    if error_msg.contains("504") || error_msg.contains("gateway timeout") {
        return true;
    }

    // Rate limiting should not be retried immediately
    if error_msg.contains("429") || error_msg.contains("too many requests") {
        return false;
    }

    // Network-level errors are retriable
    if error_msg.contains("connection")
        || error_msg.contains("timeout")
        || error_msg.contains("network")
        || error_msg.contains("dns")
    {
        return true;
    }

    // Client errors (4xx) are generally not retriable
    if error_msg.contains("400") || error_msg.contains("bad request") {
        return false;
    }
    if error_msg.contains("401") || error_msg.contains("unauthorized") {
        return false;
    }
    if error_msg.contains("403") || error_msg.contains("forbidden") {
        return false;
    }
    if error_msg.contains("404") || error_msg.contains("not found") {
        return false;
    }

    // Default to retriable for unknown errors (conservative approach)
    true
}

/// Simple pseudo-random number generator for jitter
/// Using a basic LCG to avoid external dependencies
fn pseudo_random() -> f64 {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEED: AtomicU64 = AtomicU64::new(1);

    let prev = SEED.load(Ordering::Relaxed);
    let next = prev.wrapping_mul(1103515245).wrapping_add(12345);
    SEED.store(next, Ordering::Relaxed);

    // Convert to 0.0..1.0
    (next as f64) / (u64::MAX as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use thiserror::Error;

    #[derive(Error, Debug, PartialEq, Clone)]
    enum TestError {
        #[error("Retriable error")]
        Retriable,
        #[error("Non-retriable error")]
        NonRetriable,
    }

    impl RetryableError for TestError {
        fn is_retriable(&self) -> bool {
            match self {
                TestError::Retriable => true,
                TestError::NonRetriable => false,
            }
        }
    }

    struct TestOperation {
        attempts: AtomicU32,
        fail_attempts: u32,
        error_type: TestError,
    }

    impl TestOperation {
        fn new(fail_attempts: u32, error_type: TestError) -> Self {
            Self {
                attempts: AtomicU32::new(0),
                fail_attempts,
                error_type,
            }
        }

        async fn execute(&self) -> Result<u32, TestError> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);

            if attempt < self.fail_attempts {
                Err(self.error_type.clone())
            } else {
                Ok(attempt + 1)
            }
        }
    }

    #[tokio::test]
    async fn test_retry_success_eventually() {
        let operation = TestOperation::new(2, TestError::Retriable);
        let retry_manager = RetryManager::with_config(RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(1),
            backoff_multiplier: 1.0,
            max_delay: Duration::from_millis(10),
            use_jitter: false,
        });

        let result = retry_manager.retry("test", || operation.execute()).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3); // Succeeded on 3rd attempt
        assert_eq!(operation.attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_non_retriable_error() {
        let operation = TestOperation::new(1, TestError::NonRetriable);
        let retry_manager = RetryManager::with_config(RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(1),
            ..Default::default()
        });

        let result = retry_manager.retry("test", || operation.execute()).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), TestError::NonRetriable);
        // Should only attempt once since error is not retriable
        assert_eq!(operation.attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_max_attempts_exceeded() {
        let operation = TestOperation::new(10, TestError::Retriable);
        let retry_manager = RetryManager::with_config(RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(1),
            backoff_multiplier: 1.0,
            max_delay: Duration::from_millis(10),
            use_jitter: false,
        });

        let result = retry_manager.retry("test", || operation.execute()).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), TestError::Retriable);
        assert_eq!(operation.attempts.load(Ordering::SeqCst), 4); // 1 initial + 3 retries
    }

    #[test]
    fn test_classify_error_for_retry() {
        #[derive(Error, Debug)]
        #[error("{0}")]
        struct TestError(String);

        // Server errors should be retriable
        assert!(classify_error_for_retry(&TestError(
            "500 Internal Server Error".to_string()
        )));
        assert!(classify_error_for_retry(&TestError(
            "502 Bad Gateway".to_string()
        )));
        assert!(classify_error_for_retry(&TestError(
            "503 Service Unavailable".to_string()
        )));
        assert!(classify_error_for_retry(&TestError(
            "504 Gateway Timeout".to_string()
        )));

        // Rate limiting should not be retriable
        assert!(!classify_error_for_retry(&TestError(
            "429 Too Many Requests".to_string()
        )));

        // Client errors should not be retriable
        assert!(!classify_error_for_retry(&TestError(
            "404 Not Found".to_string()
        )));
        assert!(!classify_error_for_retry(&TestError(
            "403 Forbidden".to_string()
        )));
        assert!(!classify_error_for_retry(&TestError(
            "401 Unauthorized".to_string()
        )));

        // Network errors should be retriable
        assert!(classify_error_for_retry(&TestError(
            "Connection timeout".to_string()
        )));
        assert!(classify_error_for_retry(&TestError(
            "Network unreachable".to_string()
        )));
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay, Duration::from_millis(1000));
        assert_eq!(config.backoff_multiplier, 2.0);
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert!(config.use_jitter);
    }

    #[test]
    fn test_delay_calculation() {
        let manager = RetryManager::with_config(RetryConfig {
            use_jitter: false,
            max_delay: Duration::from_secs(60),
            ..Default::default()
        });

        let base_delay = Duration::from_millis(1000);
        let delay = manager.calculate_delay(base_delay, 1);

        // Without jitter, should be the base delay
        assert_eq!(delay, base_delay);
    }

    #[test]
    fn test_exponential_backoff() {
        let manager = RetryManager::with_config(RetryConfig {
            backoff_multiplier: 2.0,
            max_delay: Duration::from_secs(60),
            ..Default::default()
        });

        let mut delay = Duration::from_millis(1000);

        delay = manager.calculate_next_delay(delay);
        assert_eq!(delay, Duration::from_millis(2000));

        delay = manager.calculate_next_delay(delay);
        assert_eq!(delay, Duration::from_millis(4000));

        delay = manager.calculate_next_delay(delay);
        assert_eq!(delay, Duration::from_millis(8000));
    }
}
