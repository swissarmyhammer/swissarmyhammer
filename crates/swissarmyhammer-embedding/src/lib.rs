//! Unified embedding interface for SwissArmyHammer.
//!
//! Resolves a named model config (e.g. `"qwen-embedding"`), selects the best
//! executor for the current platform (ANE on Apple Silicon, llama.cpp elsewhere),
//! downloads the model on first use, and provides embeddings through the
//! [`TextEmbedder`] trait.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use swissarmyhammer_embedding::Embedder;
//! use model_embedding::TextEmbedder;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let embedder = Embedder::from_model_name("qwen-embedding").await?;
//!     embedder.load().await?;
//!
//!     let result = embedder.embed_text("hello world").await?;
//!     println!("dim={}, {}ms", result.dimension(), result.processing_time_ms());
//!     Ok(())
//! }
//! ```

mod embedder;

pub use embedder::{Embedder, DEFAULT_MODEL_NAME};
pub use model_embedding::{
    BatchConfig, BatchProcessor, BatchStats, EmbeddingError, EmbeddingResult, TextEmbedder,
};
pub use swissarmyhammer_config::ModelExecutorType;
