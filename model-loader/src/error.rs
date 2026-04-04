use llama_common::error::{ErrorCategory, LlamaError};
use std::time::Duration;
use thiserror::Error;

/// Errors that can occur during model resolution operations
#[derive(Debug, Error)]
pub enum ModelError {
    /// Model resolution/loading failed
    #[error("model loading failed: {0}")]
    LoadingFailed(String),

    /// Model not found at the specified location
    #[error("model not found: {0}")]
    NotFound(String),

    /// Invalid model configuration
    #[error("invalid model config: {0}")]
    InvalidConfig(String),

    /// Model inference operation failed
    #[error("model inference failed: {0}")]
    InferenceFailed(String),

    /// Network error during model download
    #[error("network error: {0}")]
    Network(String),

    /// I/O error during file operations
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    /// Cache operation error
    #[error("cache error: {0}")]
    Cache(String),
}

impl ModelError {
    /// Create a new ModelError from a string message
    pub fn new(message: impl Into<String>) -> Self {
        Self::LoadingFailed(message.into())
    }

    /// Check if this error is retriable
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            ModelError::Network(_) | ModelError::Io(_) | ModelError::LoadingFailed(_)
        )
    }
}

impl LlamaError for ModelError {
    fn category(&self) -> ErrorCategory {
        match self {
            ModelError::LoadingFailed(_) => ErrorCategory::System,
            ModelError::NotFound(_) => ErrorCategory::User,
            ModelError::InvalidConfig(_) => ErrorCategory::User,
            ModelError::InferenceFailed(_) => ErrorCategory::System,
            ModelError::Network(_) => ErrorCategory::External,
            ModelError::Io(_) => ErrorCategory::System,
            ModelError::Cache(_) => ErrorCategory::System,
        }
    }

    fn error_code(&self) -> &'static str {
        match self {
            ModelError::LoadingFailed(_) => "MODEL_LOADING_FAILED",
            ModelError::NotFound(_) => "MODEL_NOT_FOUND",
            ModelError::InvalidConfig(_) => "MODEL_INVALID_CONFIG",
            ModelError::InferenceFailed(_) => "MODEL_INFERENCE_FAILED",
            ModelError::Network(_) => "MODEL_NETWORK_ERROR",
            ModelError::Io(_) => "MODEL_IO_ERROR",
            ModelError::Cache(_) => "MODEL_CACHE_ERROR",
        }
    }

    fn user_friendly_message(&self) -> String {
        match self {
            ModelError::LoadingFailed(msg) => {
                format!("Model loading failed: {msg}\n🔧 Check available memory and verify model file integrity")
            }
            ModelError::NotFound(msg) => {
                format!("Model not found: {msg}\n📁 Verify file path is correct, file exists and is readable")
            }
            ModelError::InvalidConfig(msg) => {
                format!("Invalid model config: {msg}\n⚙️ Ensure valid model source path and appropriate settings")
            }
            ModelError::InferenceFailed(msg) => {
                format!("Model inference failed: {msg}\n🦾 Check input format, model compatibility, and available system resources")
            }
            ModelError::Network(msg) => {
                format!("Network error: {msg}\n🌐 Check internet connection and HuggingFace availability")
            }
            ModelError::Io(e) => {
                format!("I/O error: {e}\n💾 Check disk space, file permissions, and storage availability")
            }
            ModelError::Cache(msg) => {
                format!("Cache error: {msg}\n💽 Check cache directory permissions and disk space")
            }
        }
    }

    fn custom_retry_delay(&self, attempt: u32) -> Option<Duration> {
        match self {
            // For rate limiting errors, use increasing delays
            ModelError::Network(msg)
                if msg.to_lowercase().contains("429")
                    || msg.to_lowercase().contains("rate limit") =>
            {
                Some(Duration::from_secs(60 * (attempt + 1) as u64)) // 1min, 2min, 3min, etc.
            }
            _ => None, // Use default exponential backoff
        }
    }

    fn should_stop_retrying(&self, _attempt: u32) -> bool {
        match self {
            // Don't retry authentication or authorization errors
            ModelError::Network(msg) => {
                let msg_lower = msg.to_lowercase();
                msg_lower.contains("401")
                    || msg_lower.contains("403")
                    || msg_lower.contains("unauthorized")
                    || msg_lower.contains("forbidden")
            }
            ModelError::NotFound(_) => true, // Don't retry if model doesn't exist
            ModelError::InvalidConfig(_) => true, // Don't retry config errors
            _ => false,
        }
    }
}

// Convert from HuggingFace hub errors
impl From<hf_hub::api::tokio::ApiError> for ModelError {
    fn from(err: hf_hub::api::tokio::ApiError) -> Self {
        let err_str = format!("{}", err);
        if err_str.contains("not found") || err_str.contains("404") {
            ModelError::NotFound(format!("HuggingFace resource not found: {}", err))
        } else {
            ModelError::Network(format!("HuggingFace API error: {}", err))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_model_error_creation() {
        let err = ModelError::new("test error");
        assert!(matches!(err, ModelError::LoadingFailed(_)));
    }

    #[test]
    fn test_error_retriability() {
        assert!(ModelError::Network("test".to_string()).is_retriable());
        assert!(ModelError::LoadingFailed("test".to_string()).is_retriable());
        assert!(!ModelError::InvalidConfig("test".to_string()).is_retriable());
        assert!(!ModelError::InferenceFailed("test".to_string()).is_retriable());
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let model_err = ModelError::from(io_err);
        assert!(matches!(model_err, ModelError::Io(_)));
    }

    #[test]
    fn test_error_display() {
        let err = ModelError::LoadingFailed("test error".to_string());
        let display_str = format!("{}", err);
        assert!(display_str.contains("test error"));
        assert!(
            !display_str.contains("🔧"),
            "Display should not contain emojis"
        );

        // user_friendly_message has the emojis
        let friendly = err.user_friendly_message();
        assert!(friendly.contains("🔧"));
        assert!(friendly.contains("test error"));
    }

    #[test]
    fn test_error_display_all_variants() {
        // Verify Display output for each variant
        let cases: Vec<(ModelError, &str)> = vec![
            (
                ModelError::LoadingFailed("load err".into()),
                "model loading failed: load err",
            ),
            (
                ModelError::NotFound("not found err".into()),
                "model not found: not found err",
            ),
            (
                ModelError::InvalidConfig("config err".into()),
                "invalid model config: config err",
            ),
            (
                ModelError::InferenceFailed("infer err".into()),
                "model inference failed: infer err",
            ),
            (
                ModelError::Network("net err".into()),
                "network error: net err",
            ),
            (
                ModelError::Cache("cache err".into()),
                "cache error: cache err",
            ),
        ];

        for (err, expected) in cases {
            let display = format!("{}", err);
            assert_eq!(display, expected, "Display mismatch for {:?}", err);
        }

        // Io variant wraps std::io::Error
        let io_err = ModelError::Io(io::Error::other("disk full"));
        let display = format!("{}", io_err);
        assert!(display.contains("disk full"));
        assert!(display.starts_with("i/o error:"));
    }

    #[test]
    fn test_error_retriability_all_variants() {
        // Retriable: Network, Io, LoadingFailed
        assert!(ModelError::Network("n".into()).is_retriable());
        assert!(ModelError::LoadingFailed("l".into()).is_retriable());
        assert!(ModelError::Io(io::Error::other("x")).is_retriable());

        // Not retriable: NotFound, InvalidConfig, InferenceFailed, Cache
        assert!(!ModelError::NotFound("n".into()).is_retriable());
        assert!(!ModelError::InvalidConfig("c".into()).is_retriable());
        assert!(!ModelError::InferenceFailed("i".into()).is_retriable());
        assert!(!ModelError::Cache("c".into()).is_retriable());
    }

    #[test]
    fn test_category_all_variants() {
        assert_eq!(
            ModelError::LoadingFailed("x".into()).category(),
            ErrorCategory::System
        );
        assert_eq!(
            ModelError::NotFound("x".into()).category(),
            ErrorCategory::User
        );
        assert_eq!(
            ModelError::InvalidConfig("x".into()).category(),
            ErrorCategory::User
        );
        assert_eq!(
            ModelError::InferenceFailed("x".into()).category(),
            ErrorCategory::System
        );
        assert_eq!(
            ModelError::Network("x".into()).category(),
            ErrorCategory::External
        );
        assert_eq!(
            ModelError::Io(io::Error::other("x")).category(),
            ErrorCategory::System
        );
        assert_eq!(
            ModelError::Cache("x".into()).category(),
            ErrorCategory::System
        );
    }

    #[test]
    fn test_error_code_all_variants() {
        assert_eq!(
            ModelError::LoadingFailed("x".into()).error_code(),
            "MODEL_LOADING_FAILED"
        );
        assert_eq!(
            ModelError::NotFound("x".into()).error_code(),
            "MODEL_NOT_FOUND"
        );
        assert_eq!(
            ModelError::InvalidConfig("x".into()).error_code(),
            "MODEL_INVALID_CONFIG"
        );
        assert_eq!(
            ModelError::InferenceFailed("x".into()).error_code(),
            "MODEL_INFERENCE_FAILED"
        );
        assert_eq!(
            ModelError::Network("x".into()).error_code(),
            "MODEL_NETWORK_ERROR"
        );
        assert_eq!(
            ModelError::Io(io::Error::other("x")).error_code(),
            "MODEL_IO_ERROR"
        );
        assert_eq!(
            ModelError::Cache("x".into()).error_code(),
            "MODEL_CACHE_ERROR"
        );
    }

    #[test]
    fn test_user_friendly_message_all_variants() {
        let loading = ModelError::LoadingFailed("bad load".into()).user_friendly_message();
        assert!(loading.contains("bad load"));
        assert!(loading.contains("🔧"));

        let not_found = ModelError::NotFound("missing".into()).user_friendly_message();
        assert!(not_found.contains("missing"));
        assert!(not_found.contains("📁"));

        let config = ModelError::InvalidConfig("bad cfg".into()).user_friendly_message();
        assert!(config.contains("bad cfg"));
        assert!(config.contains("⚙️"));

        let inference = ModelError::InferenceFailed("inf err".into()).user_friendly_message();
        assert!(inference.contains("inf err"));
        assert!(inference.contains("🦾"));

        let network = ModelError::Network("net err".into()).user_friendly_message();
        assert!(network.contains("net err"));
        assert!(network.contains("🌐"));

        let io_msg = ModelError::Io(io::Error::other("io err")).user_friendly_message();
        assert!(io_msg.contains("io err"));
        assert!(io_msg.contains("💾"));

        let cache = ModelError::Cache("cache err".into()).user_friendly_message();
        assert!(cache.contains("cache err"));
        assert!(cache.contains("💽"));
    }

    #[test]
    fn test_custom_retry_delay_rate_limit() {
        // Rate-limited network error should get custom delay
        let rate_err = ModelError::Network("429 rate limit exceeded".into());
        let delay = rate_err.custom_retry_delay(0);
        assert_eq!(delay, Some(Duration::from_secs(60))); // 1 minute for attempt 0

        let delay = rate_err.custom_retry_delay(1);
        assert_eq!(delay, Some(Duration::from_secs(120))); // 2 minutes for attempt 1

        let delay = rate_err.custom_retry_delay(2);
        assert_eq!(delay, Some(Duration::from_secs(180))); // 3 minutes for attempt 2
    }

    #[test]
    fn test_custom_retry_delay_non_rate_limit() {
        // Non-rate-limit errors should return None (use default backoff)
        assert_eq!(
            ModelError::Network("500 server error".into()).custom_retry_delay(0),
            None
        );
        assert_eq!(
            ModelError::LoadingFailed("x".into()).custom_retry_delay(0),
            None
        );
        assert_eq!(ModelError::NotFound("x".into()).custom_retry_delay(0), None);
        assert_eq!(
            ModelError::InvalidConfig("x".into()).custom_retry_delay(0),
            None
        );
        assert_eq!(
            ModelError::InferenceFailed("x".into()).custom_retry_delay(0),
            None
        );
        assert_eq!(
            ModelError::Io(io::Error::other("x")).custom_retry_delay(0),
            None
        );
        assert_eq!(ModelError::Cache("x".into()).custom_retry_delay(0), None);
    }

    #[test]
    fn test_should_stop_retrying_network_auth_errors() {
        // 401 Unauthorized
        assert!(ModelError::Network("401 unauthorized".into()).should_stop_retrying(0));
        // 403 Forbidden
        assert!(ModelError::Network("403 forbidden".into()).should_stop_retrying(0));
        // Normal network error should not stop
        assert!(!ModelError::Network("500 server error".into()).should_stop_retrying(0));
        assert!(!ModelError::Network("connection timeout".into()).should_stop_retrying(0));
    }

    #[test]
    fn test_should_stop_retrying_all_variants() {
        // NotFound always stops
        assert!(ModelError::NotFound("x".into()).should_stop_retrying(0));
        // InvalidConfig always stops
        assert!(ModelError::InvalidConfig("x".into()).should_stop_retrying(0));
        // Others don't stop
        assert!(!ModelError::LoadingFailed("x".into()).should_stop_retrying(0));
        assert!(!ModelError::InferenceFailed("x".into()).should_stop_retrying(0));
        assert!(!ModelError::Io(io::Error::other("x")).should_stop_retrying(0));
        assert!(!ModelError::Cache("x".into()).should_stop_retrying(0));
    }

    #[test]
    fn test_model_error_new() {
        // Test with &str
        let err = ModelError::new("hello");
        assert!(matches!(err, ModelError::LoadingFailed(ref s) if s == "hello"));

        // Test with String
        let err = ModelError::new(String::from("world"));
        assert!(matches!(err, ModelError::LoadingFailed(ref s) if s == "world"));
    }

    #[test]
    fn test_model_error_debug() {
        // Ensure Debug is implemented for all variants
        let errors: Vec<ModelError> = vec![
            ModelError::LoadingFailed("a".into()),
            ModelError::NotFound("b".into()),
            ModelError::InvalidConfig("c".into()),
            ModelError::InferenceFailed("d".into()),
            ModelError::Network("e".into()),
            ModelError::Io(io::Error::other("f")),
            ModelError::Cache("g".into()),
        ];
        for err in errors {
            let debug_str = format!("{:?}", err);
            assert!(!debug_str.is_empty());
        }
    }
}
