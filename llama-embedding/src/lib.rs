//! # llama-embedding
//!
//! Text embedding functionality using llama-cpp-2 with batch processing support.
//! This crate provides a reusable library for generating text embeddings using
//! GGUF models via the llama-cpp-2 Rust bindings.
//!
//! ## Features
//!
//! - **Model Loading**: Integration with `llama-loader` for unified model management
//! - **Single Text Embedding**: Generate embeddings for individual texts
//! - **Batch Processing**: Efficient processing of multiple texts
//! - **File Processing**: Stream processing of large text files
//! - **Configurable**: Support for normalization, sequence limits, and debug output
//! - **MD5 Hashing**: Automatic text hashing for deduplication
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use llama_embedding::{EmbeddingModel, EmbeddingConfig};
//! use llama_loader::ModelSource;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Configure the embedding model
//!     let config = EmbeddingConfig {
//!         model_source: ModelSource::HuggingFace {
//!             repo: "Qwen/Qwen3-Embedding-0.6B-GGUF".to_string(),
//!             filename: None,
//!             folder: None,
//!         },
//!         normalize_embeddings: true,
//!         max_sequence_length: Some(512),
//!         debug: false,
//!     };
//!
//!     // Create and load the model
//!     let mut model = EmbeddingModel::new(config).await?;
//!     model.load_model().await?;
//!
//!     // Generate embedding for a single text
//!     let result = model.embed_text("Hello, world!").await?;
//!     println!("Embedding dimension: {}", result.dimension());
//!     println!("Processing time: {}ms", result.processing_time_ms);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Batch Processing
//!
//! ```rust,no_run
//! use llama_embedding::{EmbeddingModel, BatchProcessor, EmbeddingConfig};
//! use std::sync::Arc;
//! use std::path::Path;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = EmbeddingConfig::default();
//!     let mut model = EmbeddingModel::new(config).await?;
//!     model.load_model().await?;
//!
//!     // Create batch processor
//!     let mut processor = BatchProcessor::new(Arc::new(model), 32);
//!
//!     // Process a file containing texts (one per line)
//!     let results = processor.process_file(Path::new("texts.txt")).await?;
//!     println!("Generated {} embeddings", results.len());
//!
//!     Ok(())
//! }
//! ```

pub mod batch;
pub mod error;
pub mod model;
pub mod types;

// Re-export main types for convenience
pub use batch::{BatchConfig, BatchProcessor, BatchStats, ProgressCallback, ProgressInfo};
pub use error::{EmbeddingError, EmbeddingResult as Result};
pub use model::EmbeddingModel;
pub use types::{EmbeddingConfig, EmbeddingResult};

// Re-export commonly used types from dependencies
pub use llama_loader::ModelSource;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_api_availability() {
        // Verify that all main types are accessible
        let _config: Option<EmbeddingConfig> = None;
        let _error: Option<EmbeddingError> = None;
        let _result: Option<EmbeddingResult> = None;

        // Test passes if this compiles
    }

    #[test]
    fn test_model_source_reexport() {
        // Verify ModelSource is properly re-exported
        let _source = ModelSource::HuggingFace {
            repo: "test/repo".to_string(),
            filename: None,
            folder: None,
        };

        // Test passes if this compiles
    }
}
