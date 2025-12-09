use llama_common::error::{ErrorCategory, LlamaError};
use thiserror::Error;

/// Errors that can occur during embedding operations
#[derive(Error, Debug)]
pub enum EmbeddingError {
    /// Error from the model loader
    #[error("Model loading error: {0}")]
    ModelLoader(#[from] llama_loader::ModelError),

    /// Error initializing or using the llama-cpp-2 model
    #[error("Model error: {0}")]
    Model(String),

    /// Error during text processing or tokenization
    #[error("Text processing error: {0}")]
    TextProcessing(String),

    /// Error during batch processing
    #[error("Batch processing error: {0}")]
    BatchProcessing(String),

    /// Error with text encoding
    #[error("Text encoding error: {0}")]
    TextEncoding(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Error when model is not loaded
    #[error("Model not loaded - call load_model() first")]
    ModelNotLoaded,

    /// Error when embedding dimensions don't match expectations
    #[error("Embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
}

impl EmbeddingError {
    /// Create a new model error
    pub fn model<S: Into<String>>(message: S) -> Self {
        Self::Model(message.into())
    }

    /// Create a new text processing error
    pub fn text_processing<S: Into<String>>(message: S) -> Self {
        Self::TextProcessing(message.into())
    }

    /// Create a new batch processing error
    pub fn batch_processing<S: Into<String>>(message: S) -> Self {
        Self::BatchProcessing(message.into())
    }

    /// Create a new text encoding error
    pub fn text_encoding<S: Into<String>>(message: S) -> Self {
        Self::TextEncoding(message.into())
    }

    /// Create a new configuration error
    pub fn configuration<S: Into<String>>(message: S) -> Self {
        Self::Configuration(message.into())
    }
}

impl LlamaError for EmbeddingError {
    fn category(&self) -> ErrorCategory {
        match self {
            EmbeddingError::ModelLoader(model_error) => model_error.category(),
            EmbeddingError::Model(_) => ErrorCategory::System,
            EmbeddingError::TextProcessing(_) => ErrorCategory::System,
            EmbeddingError::BatchProcessing(_) => ErrorCategory::System,
            EmbeddingError::TextEncoding(_) => ErrorCategory::User,
            EmbeddingError::Configuration(_) => ErrorCategory::User,
            EmbeddingError::Io(_) => ErrorCategory::System,
            EmbeddingError::ModelNotLoaded => ErrorCategory::User,
            EmbeddingError::DimensionMismatch { .. } => ErrorCategory::User,
        }
    }

    fn error_code(&self) -> &'static str {
        match self {
            EmbeddingError::ModelLoader(_) => "EMBEDDING_MODEL_LOADER",
            EmbeddingError::Model(_) => "EMBEDDING_MODEL",
            EmbeddingError::TextProcessing(_) => "EMBEDDING_TEXT_PROCESSING",
            EmbeddingError::BatchProcessing(_) => "EMBEDDING_BATCH_PROCESSING",
            EmbeddingError::TextEncoding(_) => "EMBEDDING_TEXT_ENCODING",
            EmbeddingError::Configuration(_) => "EMBEDDING_CONFIGURATION",
            EmbeddingError::Io(_) => "EMBEDDING_IO",
            EmbeddingError::ModelNotLoaded => "EMBEDDING_MODEL_NOT_LOADED",
            EmbeddingError::DimensionMismatch { .. } => "EMBEDDING_DIMENSION_MISMATCH",
        }
    }

    fn user_friendly_message(&self) -> String {
        match self {
            EmbeddingError::ModelLoader(model_error) => {
                format!("🔗 {}", model_error.user_friendly_message())
            }
            EmbeddingError::Model(msg) => {
                format!("🦾 Model Error: {}\n💡 Check model compatibility, available memory, and ensure the model is properly loaded.", msg)
            }
            EmbeddingError::TextProcessing(msg) => {
                format!("📝 Text Processing Error: {}\n💡 Verify input text format, encoding, and length limits. Check for unsupported characters or malformed content.", msg)
            }
            EmbeddingError::BatchProcessing(msg) => {
                format!("📦 Batch Processing Error: {}\n💡 Check batch size limits, memory availability, and ensure all texts in the batch are valid.", msg)
            }
            EmbeddingError::TextEncoding(msg) => {
                format!("🔤 Text Encoding Error: {}\n💡 Ensure text is properly encoded (UTF-8) and contains valid characters for the model.", msg)
            }
            EmbeddingError::Configuration(msg) => {
                format!("⚙️ Configuration Error: {}\n💡 Check embedding configuration settings, model parameters, and ensure all required values are provided.", msg)
            }
            EmbeddingError::Io(io_error) => {
                format!("💾 I/O Error: {}\n💡 Check file permissions, disk space, and ensure all required files are accessible.", io_error)
            }
            EmbeddingError::ModelNotLoaded => {
                "🚫 Model Not Loaded\n💡 Call load_model() first before performing embedding operations.".to_string()
            }
            EmbeddingError::DimensionMismatch { expected, actual } => {
                format!("📏 Embedding Dimension Mismatch: expected {}, got {}\n💡 Ensure all embeddings have consistent dimensions or adjust your model configuration.", expected, actual)
            }
        }
    }
}

/// Result type alias for embedding operations
pub type EmbeddingResult<T> = Result<T, EmbeddingError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_error_creation() {
        let error = EmbeddingError::model("test model error");
        assert!(matches!(error, EmbeddingError::Model(_)));
        assert_eq!(error.to_string(), "Model error: test model error");

        let error = EmbeddingError::text_processing("test text error");
        assert!(matches!(error, EmbeddingError::TextProcessing(_)));
        assert_eq!(error.to_string(), "Text processing error: test text error");

        let error = EmbeddingError::ModelNotLoaded;
        assert!(matches!(error, EmbeddingError::ModelNotLoaded));
        assert_eq!(
            error.to_string(),
            "Model not loaded - call load_model() first"
        );
    }

    #[test]
    fn test_error_conversion() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let embedding_error: EmbeddingError = io_error.into();
        assert!(matches!(embedding_error, EmbeddingError::Io(_)));
    }

    #[test]
    fn test_dimension_mismatch_error() {
        let error = EmbeddingError::DimensionMismatch {
            expected: 384,
            actual: 768,
        };
        assert_eq!(
            error.to_string(),
            "Embedding dimension mismatch: expected 384, got 768"
        );
    }

    #[test]
    fn test_llama_error_categories() {
        use llama_common::error::{ErrorCategory, LlamaError};

        // User errors
        let config_error = EmbeddingError::configuration("invalid setting");
        assert_eq!(config_error.category(), ErrorCategory::User);
        assert!(config_error.is_user_error());
        assert!(!config_error.is_retriable());

        let encoding_error = EmbeddingError::text_encoding("invalid encoding");
        assert_eq!(encoding_error.category(), ErrorCategory::User);
        assert!(encoding_error.is_user_error());

        let not_loaded_error = EmbeddingError::ModelNotLoaded;
        assert_eq!(not_loaded_error.category(), ErrorCategory::User);
        assert!(not_loaded_error.is_user_error());

        let dimension_error = EmbeddingError::DimensionMismatch {
            expected: 384,
            actual: 768,
        };
        assert_eq!(dimension_error.category(), ErrorCategory::User);
        assert!(dimension_error.is_user_error());

        // System errors
        let model_error = EmbeddingError::model("processing failed");
        assert_eq!(model_error.category(), ErrorCategory::System);
        assert!(!model_error.is_user_error());
        assert!(model_error.is_retriable());

        let text_processing_error = EmbeddingError::text_processing("tokenization failed");
        assert_eq!(text_processing_error.category(), ErrorCategory::System);
        assert!(text_processing_error.is_retriable());

        let batch_error = EmbeddingError::batch_processing("batch failed");
        assert_eq!(batch_error.category(), ErrorCategory::System);

        let io_error = EmbeddingError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "access denied",
        ));
        assert_eq!(io_error.category(), ErrorCategory::System);
        assert!(io_error.is_retriable());
    }

    #[test]
    fn test_llama_error_codes() {
        use llama_common::error::LlamaError;

        assert_eq!(
            EmbeddingError::model("test").error_code(),
            "EMBEDDING_MODEL"
        );
        assert_eq!(
            EmbeddingError::text_processing("test").error_code(),
            "EMBEDDING_TEXT_PROCESSING"
        );
        assert_eq!(
            EmbeddingError::batch_processing("test").error_code(),
            "EMBEDDING_BATCH_PROCESSING"
        );
        assert_eq!(
            EmbeddingError::text_encoding("test").error_code(),
            "EMBEDDING_TEXT_ENCODING"
        );
        assert_eq!(
            EmbeddingError::configuration("test").error_code(),
            "EMBEDDING_CONFIGURATION"
        );
        assert_eq!(
            EmbeddingError::ModelNotLoaded.error_code(),
            "EMBEDDING_MODEL_NOT_LOADED"
        );
        assert_eq!(
            EmbeddingError::DimensionMismatch {
                expected: 384,
                actual: 768
            }
            .error_code(),
            "EMBEDDING_DIMENSION_MISMATCH"
        );

        let io_error = EmbeddingError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert_eq!(io_error.error_code(), "EMBEDDING_IO");
    }

    #[test]
    fn test_user_friendly_messages() {
        use llama_common::error::LlamaError;

        let config_error = EmbeddingError::configuration("missing api key");
        let message = config_error.user_friendly_message();
        assert!(message.contains("Configuration Error"));
        assert!(message.contains("⚙️"));
        assert!(message.contains("missing api key"));
        assert!(message.contains("💡"));

        let model_error = EmbeddingError::model("out of memory");
        let message = model_error.user_friendly_message();
        assert!(message.contains("Model Error"));
        assert!(message.contains("🦾"));
        assert!(message.contains("out of memory"));

        let not_loaded_error = EmbeddingError::ModelNotLoaded;
        let message = not_loaded_error.user_friendly_message();
        assert!(message.contains("Model Not Loaded"));
        assert!(message.contains("🚫"));
        assert!(message.contains("load_model()"));

        let dimension_error = EmbeddingError::DimensionMismatch {
            expected: 384,
            actual: 768,
        };
        let message = dimension_error.user_friendly_message();
        assert!(message.contains("Dimension Mismatch"));
        assert!(message.contains("📏"));
        assert!(message.contains("384"));
        assert!(message.contains("768"));
    }

    #[test]
    fn test_model_loader_error_delegation() {
        use llama_common::error::LlamaError;
        use llama_loader::error::ModelError;

        let model_error = ModelError::NotFound("test model".to_string());
        let embedding_error = EmbeddingError::ModelLoader(model_error);

        // Should delegate to the underlying ModelError
        assert_eq!(embedding_error.category(), ErrorCategory::User);
        assert_eq!(embedding_error.error_code(), "EMBEDDING_MODEL_LOADER");

        let message = embedding_error.user_friendly_message();
        assert!(message.contains("🔗")); // Linking emoji
        assert!(message.contains("Model not found")); // From ModelError
    }
}
