pub mod batch;
pub mod error;
pub mod types;

pub use batch::{BatchConfig, BatchProcessor, BatchStats, ProgressCallback, ProgressInfo};
pub use error::EmbeddingError;
pub use types::EmbeddingResult;

use async_trait::async_trait;

/// Core trait for text embedding backends.
///
/// Implementors provide a way to load a model and embed text into dense vectors.
/// Both llama-embedding (CPU/GPU via llama.cpp) and ane-embedding (Apple Neural
/// Engine via ONNX Runtime + CoreML) implement this trait.
#[async_trait]
pub trait TextEmbedder: Send + Sync {
    /// Load the model (download if needed, initialize runtime).
    /// Backends manage their own synchronization.
    async fn load(&self) -> Result<(), EmbeddingError>;

    /// Embed a single text, returning the embedding result.
    /// Backends manage their own synchronization for concurrent access.
    async fn embed_text(&self, text: &str) -> Result<EmbeddingResult, EmbeddingError>;

    /// Get the embedding dimension (None if not yet loaded).
    fn embedding_dimension(&self) -> Option<usize>;

    /// Check if the model is ready for inference.
    fn is_loaded(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_api_available() {
        // Verify all public types are accessible
        let _: fn() -> EmbeddingError = || EmbeddingError::ModelNotLoaded;
        let _: fn() -> BatchConfig = BatchConfig::default;
        let _: fn() -> BatchStats = BatchStats::new;
    }
}
