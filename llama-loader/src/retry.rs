use crate::error::ModelError;
use llama_common::retry::RetryManager;
use std::path::PathBuf;

/// Downloads a model file with retry logic and exponential backoff
///
/// This function now uses the unified retry manager for consistent behavior
pub async fn download_with_retry(
    repo_api: &hf_hub::api::tokio::ApiRepo,
    filename: &str,
    repo: &str,
    retry_config: &crate::types::RetryConfig,
) -> Result<PathBuf, ModelError> {
    let retry_manager = RetryManager::with_config(retry_config.clone().into());
    let operation_name = format!("download {}", filename);

    retry_manager
        .retry(&operation_name, || {
            let _repo = repo;
            async move {
                match repo_api.get(filename).await {
                    Ok(path) => Ok(path),
                    Err(e) => {
                        // Convert HuggingFace error to ModelError for retry logic
                        if e.to_string().to_lowercase().contains("not found")
                            || e.to_string().contains("404")
                        {
                            Err(ModelError::NotFound(format!(
                                "HuggingFace resource not found: {}",
                                e
                            )))
                        } else {
                            Err(ModelError::Network(format!("HuggingFace API error: {}", e)))
                        }
                    }
                }
            }
        })
        .await
        .map_err(|e| {
            // Add additional context to the error
            match e {
                ModelError::NotFound(msg) => ModelError::LoadingFailed(format_download_error(
                    filename,
                    repo,
                    &std::io::Error::new(std::io::ErrorKind::NotFound, msg),
                    retry_config.max_retries,
                )),
                ModelError::Network(msg) => ModelError::LoadingFailed(format_download_error(
                    filename,
                    repo,
                    &std::io::Error::other(msg),
                    retry_config.max_retries,
                )),
                other => other,
            }
        })
}

/// Determines if an error is retriable based on the error message
pub fn is_retriable_error(error: &dyn std::error::Error) -> bool {
    let error_msg = error.to_string().to_lowercase();

    // Check for specific HTTP status codes or error patterns
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
    // 429 errors should not be retried - HuggingFace explicitly asks us to wait
    if error_msg.contains("429") || error_msg.contains("too many requests") {
        return false;
    }

    // Network-level errors are retriable
    if error_msg.contains("connection")
        || error_msg.contains("timeout")
        || error_msg.contains("network")
    {
        return true;
    }

    // Client errors (4xx) are generally not retriable
    if error_msg.contains("404") || error_msg.contains("not found") {
        return false;
    }
    if error_msg.contains("403") || error_msg.contains("forbidden") {
        return false;
    }
    if error_msg.contains("401") || error_msg.contains("unauthorized") {
        return false;
    }

    // Default to retriable for unknown errors
    true
}

/// Formats a comprehensive error message for download failures
pub fn format_download_error(
    filename: &str,
    repo: &str,
    error: &dyn std::error::Error,
    retries_attempted: u32,
) -> String {
    let base_message = format!(
        "Failed to download model file '{}' from repository '{}' after {} retries: {}",
        filename, repo, retries_attempted, error
    );

    let error_msg = error.to_string().to_lowercase();

    // Add specific guidance based on error type
    let guidance = if error_msg.contains("404") || error_msg.contains("not found") {
        "📁 File not found. Verify the filename exists in the repository. You can browse the repo at https://huggingface.co/"
    } else if error_msg.contains("403") || error_msg.contains("forbidden") {
        "🔒 Access forbidden. Check if the repository is private and if you need authentication."
    } else if error_msg.contains("429") || error_msg.contains("too many requests") {
        "⏱️ Rate limited by HuggingFace. Wait 5-10 minutes before trying again to respect their API limits."
    } else if error_msg.contains("500")
        || error_msg.contains("502")
        || error_msg.contains("503")
        || error_msg.contains("504")
    {
        "🏥 Server error on HuggingFace. This is temporary - try again in a few minutes."
    } else {
        "🌐 Network error. Check your internet connection and try again."
    };

    let additional_help = "💡 Check model file exists, is valid GGUF format, and sufficient memory is available\n🔧 You can increase retry attempts by configuring retry_config.max_retries";

    format!("{}\n{}\n{}", base_message, guidance, additional_help)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RetryConfig;

    #[test]
    fn test_is_retriable_error_server_errors() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        // Server errors should be retriable
        assert!(is_retriable_error(&TestError(
            "500 Internal Server Error".to_string()
        )));
        assert!(is_retriable_error(&TestError(
            "502 Bad Gateway".to_string()
        )));
        assert!(is_retriable_error(&TestError(
            "503 Service Unavailable".to_string()
        )));
        assert!(is_retriable_error(&TestError(
            "504 Gateway Timeout".to_string()
        )));
        assert!(!is_retriable_error(&TestError(
            "429 Too Many Requests".to_string()
        )));
    }

    #[test]
    fn test_is_retriable_error_client_errors() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        // Client errors should not be retriable
        assert!(!is_retriable_error(&TestError("404 Not Found".to_string())));
        assert!(!is_retriable_error(&TestError("403 Forbidden".to_string())));
        assert!(!is_retriable_error(&TestError(
            "401 Unauthorized".to_string()
        )));
    }

    #[test]
    fn test_is_retriable_error_network_errors() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        // Network errors should be retriable
        assert!(is_retriable_error(&TestError(
            "Connection timeout".to_string()
        )));
        assert!(is_retriable_error(&TestError(
            "Network unreachable".to_string()
        )));
        assert!(is_retriable_error(&TestError(
            "Connection refused".to_string()
        )));
    }

    #[test]
    fn test_format_download_error() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        let error = TestError("404 Not Found".to_string());
        let result = format_download_error("model.gguf", "test/repo", &error, 3);

        assert!(result.contains("model.gguf"));
        assert!(result.contains("test/repo"));
        assert!(result.contains("3 retries"));
        assert!(result.contains("📁")); // Should contain file not found guidance
    }

    #[test]
    fn test_exponential_backoff_calculation() {
        let retry_config = RetryConfig::default();
        let mut delay = retry_config.initial_delay_ms;

        // Test exponential backoff progression
        assert_eq!(delay, 1000); // Initial: 1s

        delay = ((delay as f64) * retry_config.backoff_multiplier) as u64;
        delay = delay.min(retry_config.max_delay_ms);
        assert_eq!(delay, 2000); // 2s

        delay = ((delay as f64) * retry_config.backoff_multiplier) as u64;
        delay = delay.min(retry_config.max_delay_ms);
        assert_eq!(delay, 4000); // 4s

        // Continue until we hit the max
        for _ in 0..10 {
            delay = ((delay as f64) * retry_config.backoff_multiplier) as u64;
            delay = delay.min(retry_config.max_delay_ms);
        }
        assert_eq!(delay, retry_config.max_delay_ms); // Should cap at 30s
    }
}
