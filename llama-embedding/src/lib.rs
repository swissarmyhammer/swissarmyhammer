//! # llama-embedding
//!
//! Text embedding functionality using llama-cpp-2 with batch processing support.
//! This crate provides a reusable library for generating text embeddings using
//! GGUF models via the llama-cpp-2 Rust bindings.
//!
//! ## Features
//!
//! - **Model Loading**: Integration with `model-loader` for unified model management
//! - **Single Text Embedding**: Generate embeddings for individual texts
//! - **Batch Processing**: Efficient processing of multiple texts
//! - **File Processing**: Stream processing of large text files
//! - **Configurable**: Support for normalization, sequence limits, and debug output
//! - **MD5 Hashing**: Automatic text hashing for deduplication
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use llama_embedding::{EmbeddingModel, EmbeddingConfig, TextEmbedder};
//! use model_loader::ModelSource;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
//!     let model = EmbeddingModel::new(config).await?;
//!     model.load().await?;
//!
//!     let result = model.embed_text("Hello, world!").await?;
//!     println!("Embedding dimension: {}", result.dimension());
//!     println!("Processing time: {}ms", result.processing_time_ms());
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Batch Processing
//!
//! ```rust,no_run
//! use llama_embedding::{EmbeddingModel, BatchProcessor, EmbeddingConfig, TextEmbedder};
//! use std::path::Path;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = EmbeddingConfig::default();
//!     let model = EmbeddingModel::new(config).await?;
//!     model.load().await?;
//!
//!     let mut processor = BatchProcessor::new(&model, 32);
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
pub use error::{EmbeddingError, EmbedResult as Result};
pub use model::EmbeddingModel;
pub use types::{EmbeddingConfig, EmbeddingResult};

// Re-export commonly used types from dependencies
pub use model_loader::ModelSource;
pub use model_embedding::TextEmbedder;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_api_availability() {
        let _config: Option<EmbeddingConfig> = None;
        let _error: Option<EmbeddingError> = None;
        let _result: Option<EmbeddingResult> = None;
        let _trait: Option<&dyn TextEmbedder> = None;
    }

    #[test]
    fn test_model_source_reexport() {
        let _source = ModelSource::HuggingFace {
            repo: "test/repo".to_string(),
            filename: None,
            folder: None,
        };
    }
}
