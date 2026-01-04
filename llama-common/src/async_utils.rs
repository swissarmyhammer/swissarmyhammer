//! Shared async utilities and patterns

use async_trait::async_trait;
use std::time::Duration;
use swissarmyhammer_common::Pretty;
use tokio::time::{sleep, timeout};

/// Trait for async operations that can be retried
#[async_trait]
pub trait Retryable {
    type Output: Send;
    type Error: std::error::Error + Send + Sync;

    /// Execute the operation once
    async fn execute(&mut self) -> Result<Self::Output, Self::Error>;

    /// Check if the error is retriable
    fn is_retriable_error(&self, error: &Self::Error) -> bool;

    /// Get the delay before the next retry
    fn retry_delay(&self, attempt: u32) -> Duration {
        // Exponential backoff with jitter
        let base_delay = Duration::from_millis(100);
        let exponential_delay = base_delay * 2_u32.pow(attempt.min(10));

        // Add some jitter (up to 25% of the delay)
        let jitter_ms = (exponential_delay.as_millis() as f64 * 0.25 * rand()) as u64;
        exponential_delay + Duration::from_millis(jitter_ms)
    }

    /// Execute with automatic retry logic
    async fn execute_with_retry(&mut self, max_attempts: u32) -> Result<Self::Output, Self::Error> {
        let mut last_error = None;

        for attempt in 0..max_attempts {
            match self.execute().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    if !self.is_retriable_error(&error) {
                        return Err(error);
                    }

                    last_error = Some(error);

                    if attempt < max_attempts - 1 {
                        let delay = self.retry_delay(attempt);
                        sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap())
    }
}

/// Execute an async operation with a timeout
pub async fn with_timeout<T, E, F>(
    operation: F,
    timeout_duration: Duration,
) -> Result<T, TimeoutError<E>>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    match timeout(timeout_duration, operation).await {
        Ok(result) => result.map_err(TimeoutError::Operation),
        Err(_) => Err(TimeoutError::Timeout {
            duration: timeout_duration,
        }),
    }
}

/// Action to take when a timeout occurs
#[derive(Debug)]
pub enum TimeoutAction<E> {
    /// Return an error when timeout occurs
    ReturnError(E),
    /// Log a warning and continue (returns Ok(None))
    LogWarning,
}

/// Execute an async operation with a timeout and configurable timeout handling
pub async fn with_timeout_action<T, E, F>(
    operation: F,
    timeout_duration: Duration,
    action: TimeoutAction<E>,
    context: &str,
) -> Result<Option<T>, E>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    match timeout(timeout_duration, operation).await {
        Ok(result) => result.map(Some),
        Err(_) => match action {
            TimeoutAction::ReturnError(error) => Err(error),
            TimeoutAction::LogWarning => {
                tracing::warn!("{} timed out after {}", context, Pretty(&timeout_duration));
                Ok(None)
            }
        },
    }
}

/// Error type for timeout operations
#[derive(Debug, thiserror::Error)]
pub enum TimeoutError<E> {
    #[error("Operation timed out after {duration:?}")]
    Timeout { duration: Duration },
    #[error("Operation failed: {0}")]
    Operation(E),
}

/// Simple pseudo-random number generator for jitter
/// Using a basic LCG to avoid external dependencies
fn rand() -> f64 {
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

    #[derive(Error, Debug, PartialEq)]
    enum TestError {
        #[error("Retriable error")]
        Retriable,
        #[error("Non-retriable error")]
        NonRetriable,
    }

    struct TestOperation {
        attempts: AtomicU32,
        fail_attempts: u32,
        retriable: bool,
    }

    impl TestOperation {
        fn new(fail_attempts: u32, retriable: bool) -> Self {
            Self {
                attempts: AtomicU32::new(0),
                fail_attempts,
                retriable,
            }
        }
    }

    #[async_trait]
    impl Retryable for TestOperation {
        type Output = u32;
        type Error = TestError;

        async fn execute(&mut self) -> Result<Self::Output, Self::Error> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);

            if attempt < self.fail_attempts {
                if self.retriable {
                    Err(TestError::Retriable)
                } else {
                    Err(TestError::NonRetriable)
                }
            } else {
                Ok(attempt + 1)
            }
        }

        fn is_retriable_error(&self, error: &Self::Error) -> bool {
            matches!(error, TestError::Retriable)
        }
    }

    #[tokio::test]
    async fn test_retry_success_eventually() {
        let mut operation = TestOperation::new(2, true);
        let result = operation.execute_with_retry(5).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3); // Succeeded on 3rd attempt
    }

    #[tokio::test]
    async fn test_retry_non_retriable_error() {
        let mut operation = TestOperation::new(1, false);
        let result = operation.execute_with_retry(5).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), TestError::NonRetriable);
        // Should only attempt once since error is not retriable
        assert_eq!(operation.attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_max_attempts_exceeded() {
        let mut operation = TestOperation::new(10, true); // Fail more times than max attempts
        let result = operation.execute_with_retry(3).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), TestError::Retriable);
        assert_eq!(operation.attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_with_timeout_success() {
        let operation = async {
            sleep(Duration::from_millis(10)).await;
            Ok::<_, TestError>(42)
        };

        let result = with_timeout(operation, Duration::from_millis(100)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_with_timeout_timeout() {
        let operation = async {
            sleep(Duration::from_millis(100)).await;
            Ok::<_, TestError>(42)
        };

        let result = with_timeout(operation, Duration::from_millis(10)).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TimeoutError::Timeout { .. }));
    }

    #[tokio::test]
    async fn test_with_timeout_operation_error() {
        let operation = async { Err::<u32, _>(TestError::NonRetriable) };

        let result = with_timeout(operation, Duration::from_millis(100)).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TimeoutError::Operation(TestError::NonRetriable)
        ));
    }

    #[test]
    fn test_retry_delay_increases() {
        let operation = TestOperation::new(0, true);

        let delay1 = operation.retry_delay(0);
        let delay2 = operation.retry_delay(1);
        let delay3 = operation.retry_delay(2);

        // Should increase exponentially (with jitter, but still generally increasing)
        assert!(delay2.as_millis() >= delay1.as_millis());
        assert!(delay3.as_millis() >= delay2.as_millis());
    }

    #[tokio::test]
    async fn test_with_timeout_action_return_error() {
        let operation = async {
            sleep(Duration::from_millis(100)).await;
            Ok::<_, TestError>(42)
        };

        let result = with_timeout_action(
            operation,
            Duration::from_millis(10),
            TimeoutAction::ReturnError(TestError::NonRetriable),
            "test operation",
        )
        .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), TestError::NonRetriable);
    }

    #[tokio::test]
    async fn test_with_timeout_action_log_warning() {
        let operation = async {
            sleep(Duration::from_millis(100)).await;
            Ok::<_, TestError>(42)
        };

        let result = with_timeout_action(
            operation,
            Duration::from_millis(10),
            TimeoutAction::LogWarning,
            "test operation",
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None); // Should return None when timeout with log warning
    }

    #[tokio::test]
    async fn test_with_timeout_action_success() {
        let operation = async {
            sleep(Duration::from_millis(10)).await;
            Ok::<_, TestError>(42)
        };

        let result = with_timeout_action(
            operation,
            Duration::from_millis(100),
            TimeoutAction::LogWarning,
            "test operation",
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(42)); // Should return Some(value) when succeeds
    }
}
