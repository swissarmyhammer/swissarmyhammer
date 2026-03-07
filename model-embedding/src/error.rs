use thiserror::Error;

/// Errors that can occur during embedding operations.
///
/// This is the shared error type used by all embedding backends.
/// Backend-specific errors are wrapped in the `Backend` variant.
#[derive(Error, Debug)]
pub enum EmbeddingError {
    /// Error when model is not loaded
    #[error("Model not loaded - call load() first")]
    ModelNotLoaded,

    /// Error loading or initializing the model
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

    /// Embedding dimension mismatch
    #[error("Embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Backend-specific error (wraps errors from llama.cpp, ONNX Runtime, etc.)
    #[error("Backend error: {0}")]
    Backend(#[from] Box<dyn std::error::Error + Send + Sync>),
}

impl EmbeddingError {
    pub fn model<S: Into<String>>(message: S) -> Self {
        Self::Model(message.into())
    }

    pub fn text_processing<S: Into<String>>(message: S) -> Self {
        Self::TextProcessing(message.into())
    }

    pub fn batch_processing<S: Into<String>>(message: S) -> Self {
        Self::BatchProcessing(message.into())
    }

    pub fn text_encoding<S: Into<String>>(message: S) -> Self {
        Self::TextEncoding(message.into())
    }

    pub fn configuration<S: Into<String>>(message: S) -> Self {
        Self::Configuration(message.into())
    }
}

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

        let error = EmbeddingError::ModelNotLoaded;
        assert_eq!(error.to_string(), "Model not loaded - call load() first");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let embedding_error: EmbeddingError = io_error.into();
        assert!(matches!(embedding_error, EmbeddingError::Io(_)));
    }

    #[test]
    fn test_dimension_mismatch() {
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
    fn test_backend_error() {
        let backend_err: Box<dyn std::error::Error + Send + Sync> = "onnx runtime failed".into();
        let error: EmbeddingError = backend_err.into();
        assert!(matches!(error, EmbeddingError::Backend(_)));
        assert!(error.to_string().contains("onnx runtime failed"));
    }
}
