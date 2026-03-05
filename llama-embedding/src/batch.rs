//! Batch processing for llama-embedding.
//!
//! Re-exports the generic [`model_embedding::BatchProcessor`] and related types.
//! All batch processing goes through the shared `model-embedding` implementation.

pub use model_embedding::batch::{
    BatchConfig, BatchProcessor, BatchStats, ProgressCallback, ProgressInfo,
};
