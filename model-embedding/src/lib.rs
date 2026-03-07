//! Shared embedding trait and types for text embedding backends.
//!
//! This crate defines the [`TextEmbedder`] trait that all embedding backends
//! implement (e.g., `llama-embedding` for llama.cpp, future `ane-embedding`
//! for ONNX Runtime + CoreML).
//!
//! # Implementing `TextEmbedder`
//!
//! The trait is sealed — only crates within this workspace can implement it.
//! To add a new backend, implement `private::Sealed` and `TextEmbedder`:
//!
//! ```rust,ignore
//! use model_embedding::{TextEmbedder, EmbeddingError, EmbeddingResult};
//!
//! struct MyBackend { /* ... */ }
//!
//! impl model_embedding::private::Sealed for MyBackend {}
//!
//! #[async_trait::async_trait]
//! impl TextEmbedder for MyBackend {
//!     async fn load(&self) -> Result<(), EmbeddingError> { todo!() }
//!     async fn embed_text(&self, text: &str) -> Result<EmbeddingResult, EmbeddingError> { todo!() }
//!     fn embedding_dimension(&self) -> Option<usize> { None }
//!     fn is_loaded(&self) -> bool { false }
//! }
//! ```
//!
//! # Batch Processing
//!
//! Use [`BatchProcessor`] for efficient processing of multiple texts:
//!
//! ```rust,ignore
//! use model_embedding::{BatchProcessor, BatchConfig};
//!
//! let mut processor = BatchProcessor::new(&embedder, 32);
//! let results = processor.process_texts(texts).await?;
//! ```

pub mod batch;
pub mod error;
/// Sealed trait module — backends must implement `Sealed` to implement `TextEmbedder`.
pub mod private {
    pub trait Sealed {}
}
pub mod types;

pub use batch::{
    BatchConfig, BatchFailure, BatchProcessor, BatchStats, ProgressCallback, ProgressInfo,
};
pub use error::EmbeddingError;
pub use types::EmbeddingResult;

use async_trait::async_trait;

/// Core trait for text embedding backends.
///
/// Implementors provide a way to load a model and embed text into dense vectors.
/// Both llama-embedding (CPU/GPU via llama.cpp) and ane-embedding (Apple Neural
/// Engine via ONNX Runtime + CoreML) implement this trait.
///
/// This trait is sealed — it can only be implemented by types that also
/// implement [`private::Sealed`].
#[async_trait]
pub trait TextEmbedder: private::Sealed + Send + Sync {
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
        let _: fn() -> EmbeddingError = || EmbeddingError::ModelNotLoaded;
        let _: fn() -> BatchConfig = BatchConfig::default;
        let _: fn() -> BatchStats = BatchStats::new;
    }
}
