//! Retrying an operation that failed for a transient reason.
//!
//! [`retry_with_backoff`] is the one retry loop the server has. Both the prompt
//! reload path and the file-watch startup path drive it, each supplying its own
//! predicate for which errors are worth another attempt.

use swissarmyhammer_common::{Result, SwissArmyHammerError};

/// Maximum retry attempts for operations with transient errors
const MAX_RETRIES: u32 = 3;

/// Initial backoff delay in milliseconds for retry operations
const INITIAL_BACKOFF_MS: u64 = 100;

/// Factor the backoff delay grows by after each failed attempt. Two makes the
/// growth exponential, so the delays are 100 ms then 200 ms.
const BACKOFF_MULTIPLIER: u64 = 2;

/// Determine if a retry should be attempted based on the error and attempt count.
///
/// # Arguments
///
/// * `attempt` - Current attempt number (1-indexed)
/// * `error` - The error that occurred
/// * `is_retryable` - Function to determine if an error is retryable
///
/// # Returns
///
/// * `bool` - True if should retry, false otherwise
fn should_retry(
    attempt: u32,
    error: &SwissArmyHammerError,
    is_retryable: fn(&SwissArmyHammerError) -> bool,
) -> bool {
    attempt < MAX_RETRIES && is_retryable(error)
}

/// Log a retry attempt with backoff information.
///
/// # Arguments
///
/// * `operation_name` - Name of the operation being retried
/// * `attempt` - Current attempt number (1-indexed)
/// * `backoff_ms` - Backoff delay in milliseconds
/// * `error` - The error that occurred
fn log_retry_attempt(
    operation_name: &str,
    attempt: u32,
    backoff_ms: u64,
    error: &SwissArmyHammerError,
) {
    tracing::warn!(
        "{} attempt {} failed, retrying in {}ms: {}",
        operation_name,
        attempt,
        backoff_ms,
        error
    );
}

/// Retry an async operation with exponential backoff.
///
/// # Arguments
///
/// * `operation` - The async operation to retry
/// * `is_retryable` - Function to determine if an error is retryable
/// * `operation_name` - Name of the operation for logging
///
/// # Returns
///
/// * `Result<T>` - The result of the operation
pub(super) async fn retry_with_backoff<F, T, Fut>(
    mut operation: F,
    is_retryable: fn(&SwissArmyHammerError) -> bool,
    operation_name: &str,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_error = None;
    let mut backoff_ms = INITIAL_BACKOFF_MS;

    for attempt in 1..=MAX_RETRIES {
        match operation().await {
            Ok(result) => {
                if attempt > 1 {
                    tracing::info!("{} succeeded on attempt {}", operation_name, attempt);
                }
                return Ok(result);
            }
            Err(e) => {
                if should_retry(attempt, &e, is_retryable) {
                    log_retry_attempt(operation_name, attempt, backoff_ms, &e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                    backoff_ms *= BACKOFF_MULTIPLIER;
                }
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| SwissArmyHammerError::Other {
        message: format!("{} failed", operation_name),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::server::McpServer;

    // ---------------------------------------------------------------
    // Retry helper tests
    // ---------------------------------------------------------------

    #[test]
    fn test_should_retry_within_limit() {
        let err = SwissArmyHammerError::Io(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "timed out",
        ));
        assert!(should_retry(1, &err, McpServer::is_retryable_fs_error));
        assert!(should_retry(2, &err, McpServer::is_retryable_fs_error));
        assert!(!should_retry(3, &err, McpServer::is_retryable_fs_error));
    }

    #[test]
    fn test_should_retry_non_retryable() {
        let err = SwissArmyHammerError::Other {
            message: "permanent failure".to_string(),
        };
        assert!(!should_retry(1, &err, McpServer::is_retryable_fs_error));
    }

    #[tokio::test]
    async fn test_retry_with_backoff_succeeds_immediately() {
        let mut call_count = 0u32;
        let result: swissarmyhammer_common::Result<&str> = retry_with_backoff(
            || {
                call_count += 1;
                async { Ok("success") }
            },
            |_| true,
            "test_op",
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_retry_with_backoff_non_retryable_fails_immediately() {
        let result: swissarmyhammer_common::Result<&str> = retry_with_backoff(
            || async {
                Err(SwissArmyHammerError::Other {
                    message: "permanent".to_string(),
                })
            },
            |_| false, // never retry
            "test_op",
        )
        .await;
        assert!(result.is_err());
    }
}
