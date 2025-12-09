//! Error types for text generation operations.

use thiserror::Error;

/// Errors that can occur during text generation operations.
///
/// These errors consolidate the error handling patterns used across the
/// generation implementations while maintaining compatibility with existing
/// error types and messages.
#[derive(Debug, Error)]
pub enum GenerationError {
    /// Configuration validation failed.
    #[error("Invalid generation configuration: {0}")]
    InvalidConfig(String),

    /// Failed to tokenize the input prompt.
    #[error("Failed to tokenize prompt: {0}")]
    TokenizationFailed(String),

    /// Failed to create or manage the generation batch.
    #[error("Batch processing failed: {0}")]
    BatchFailed(String),

    /// Failed to decode the generated token.
    #[error("Token decoding failed: {0}")]
    DecodingFailed(String),

    /// Failed to convert token to string representation.
    #[error("Token to string conversion failed: {0}")]
    TokenConversionFailed(String),

    /// Context processing failed during generation.
    #[error("Context processing failed: {0}")]
    ContextFailed(String),

    /// Failed to acquire context lock for thread-safe access.
    #[error("Failed to acquire context lock")]
    ContextLock,

    /// Generation was cancelled by user request.
    #[error("Generation cancelled")]
    Cancelled,

    /// Stream sender channel was closed unexpectedly.
    #[error("Stream channel closed")]
    StreamClosed,

    /// A stopper indicated generation should terminate.
    #[error("Generation stopped: {0}")]
    Stopped(String),

    /// An unexpected error occurred during generation.
    #[error("Generation error: {0}")]
    GenerationFailed(String),
}

impl GenerationError {
    /// Create a new tokenization error from a source error.
    pub fn tokenization<E: std::error::Error>(err: E) -> Self {
        Self::TokenizationFailed(err.to_string())
    }

    /// Create a new batch error from a source error.
    pub fn batch<E: std::error::Error>(err: E) -> Self {
        Self::BatchFailed(err.to_string())
    }

    /// Create a new decoding error from a source error.
    pub fn decoding<E: std::error::Error>(err: E) -> Self {
        Self::DecodingFailed(err.to_string())
    }

    /// Create a new token conversion error from a source error.
    pub fn token_conversion<E: std::error::Error>(err: E) -> Self {
        Self::TokenConversionFailed(err.to_string())
    }

    /// Create a new context error from a source error.
    pub fn context<E: std::error::Error>(err: E) -> Self {
        Self::ContextFailed(err.to_string())
    }

    /// Create a new generation error from a source error.
    pub fn generation<E: std::error::Error>(err: E) -> Self {
        Self::GenerationFailed(err.to_string())
    }
}

// Provide conversion from configuration validation errors
impl From<String> for GenerationError {
    fn from(msg: String) -> Self {
        Self::InvalidConfig(msg)
    }
}
