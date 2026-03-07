use llama_common::error::{ErrorCategory, LlamaError};
use thiserror::Error;

/// Errors that can occur during ANE embedding operations
#[derive(Error, Debug)]
pub enum EmbeddingError {
    /// Error from the model loader
    #[error("Model loading error: {0}")]
    ModelLoader(#[from] model_loader::ModelError),

    /// Error from CoreML runtime
    #[error("CoreML error: {0}")]
    CoreML(String),

    /// Error during tokenization
    #[error("Tokenization error: {0}")]
    Tokenization(String),

    /// Error during text processing
    #[error("Text processing error: {0}")]
    TextProcessing(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Error when model is not loaded
    #[error("Model not loaded - call load() first")]
    ModelNotLoaded,

    /// Error when embedding dimensions don't match expectations
    #[error("Embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
}

impl EmbeddingError {
    pub fn coreml<S: Into<String>>(message: S) -> Self {
        Self::CoreML(message.into())
    }

    pub fn tokenization<S: Into<String>>(message: S) -> Self {
        Self::Tokenization(message.into())
    }

    pub fn text_processing<S: Into<String>>(message: S) -> Self {
        Self::TextProcessing(message.into())
    }

    pub fn configuration<S: Into<String>>(message: S) -> Self {
        Self::Configuration(message.into())
    }
}

impl LlamaError for EmbeddingError {
    fn category(&self) -> ErrorCategory {
        match self {
            EmbeddingError::ModelLoader(e) => e.category(),
            EmbeddingError::CoreML(_) => ErrorCategory::System,
            EmbeddingError::Tokenization(_) => ErrorCategory::User,
            EmbeddingError::TextProcessing(_) => ErrorCategory::System,
            EmbeddingError::Configuration(_) => ErrorCategory::User,
            EmbeddingError::Io(_) => ErrorCategory::System,
            EmbeddingError::ModelNotLoaded => ErrorCategory::User,
            EmbeddingError::DimensionMismatch { .. } => ErrorCategory::User,
        }
    }

    fn error_code(&self) -> &'static str {
        match self {
            EmbeddingError::ModelLoader(_) => "ANE_EMBEDDING_MODEL_LOADER",
            EmbeddingError::CoreML(_) => "ANE_EMBEDDING_COREML",
            EmbeddingError::Tokenization(_) => "ANE_EMBEDDING_TOKENIZATION",
            EmbeddingError::TextProcessing(_) => "ANE_EMBEDDING_TEXT_PROCESSING",
            EmbeddingError::Configuration(_) => "ANE_EMBEDDING_CONFIGURATION",
            EmbeddingError::Io(_) => "ANE_EMBEDDING_IO",
            EmbeddingError::ModelNotLoaded => "ANE_EMBEDDING_MODEL_NOT_LOADED",
            EmbeddingError::DimensionMismatch { .. } => "ANE_EMBEDDING_DIMENSION_MISMATCH",
        }
    }

    fn user_friendly_message(&self) -> String {
        match self {
            EmbeddingError::ModelLoader(e) => {
                format!("Model loader: {}", e.user_friendly_message())
            }
            EmbeddingError::CoreML(msg) => format!("CoreML error: {msg}"),
            EmbeddingError::Tokenization(msg) => format!("Tokenization error: {msg}"),
            EmbeddingError::TextProcessing(msg) => format!("Text processing error: {msg}"),
            EmbeddingError::Configuration(msg) => format!("Configuration error: {msg}"),
            EmbeddingError::Io(e) => format!("IO error: {e}"),
            EmbeddingError::ModelNotLoaded => "Model not loaded - call load() first".to_string(),
            EmbeddingError::DimensionMismatch { expected, actual } => {
                format!("Dimension mismatch: expected {expected}, got {actual}")
            }
        }
    }
}

/// Convenience conversion from model-embedding's EmbeddingError
impl From<EmbeddingError> for model_embedding::EmbeddingError {
    fn from(e: EmbeddingError) -> Self {
        match e {
            EmbeddingError::ModelNotLoaded => model_embedding::EmbeddingError::ModelNotLoaded,
            EmbeddingError::Configuration(msg) => {
                model_embedding::EmbeddingError::Configuration(msg)
            }
            EmbeddingError::Io(e) => model_embedding::EmbeddingError::Io(e),
            EmbeddingError::DimensionMismatch { expected, actual } => {
                model_embedding::EmbeddingError::DimensionMismatch { expected, actual }
            }
            other => model_embedding::EmbeddingError::Backend(Box::new(other)),
        }
    }
}

pub type Result<T> = std::result::Result<T, EmbeddingError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let e = EmbeddingError::coreml("session failed");
        assert!(matches!(e, EmbeddingError::CoreML(_)));
        assert_eq!(e.to_string(), "CoreML error: session failed");

        let e = EmbeddingError::tokenization("bad input");
        assert!(matches!(e, EmbeddingError::Tokenization(_)));

        let e = EmbeddingError::ModelNotLoaded;
        assert_eq!(e.to_string(), "Model not loaded - call load() first");
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(
            EmbeddingError::ModelNotLoaded.category(),
            ErrorCategory::User
        );
        assert_eq!(
            EmbeddingError::coreml("fail").category(),
            ErrorCategory::System
        );
        assert_eq!(
            EmbeddingError::configuration("bad").category(),
            ErrorCategory::User
        );
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(
            EmbeddingError::coreml("x").error_code(),
            "ANE_EMBEDDING_COREML"
        );
        assert_eq!(
            EmbeddingError::ModelNotLoaded.error_code(),
            "ANE_EMBEDDING_MODEL_NOT_LOADED"
        );
    }

    #[test]
    fn test_conversion_to_model_embedding_error() {
        let e: model_embedding::EmbeddingError = EmbeddingError::ModelNotLoaded.into();
        assert!(matches!(e, model_embedding::EmbeddingError::ModelNotLoaded));

        let e: model_embedding::EmbeddingError = EmbeddingError::coreml("runtime fail").into();
        assert!(matches!(e, model_embedding::EmbeddingError::Backend(_)));
    }
}
