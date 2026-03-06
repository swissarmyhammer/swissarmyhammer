//! Text embedding using CoreML for Apple Neural Engine.
//!
//! This crate implements the [`TextEmbedder`] trait from `model-embedding`,
//! loading a `.mlpackage` model directly via `coreml-rs` for hardware-accelerated
//! inference on Apple Silicon's Neural Engine.
//!
//! # Differences from llama-embedding
//!
//! - **Runtime**: CoreML (via `coreml-rs`) instead of llama.cpp
//! - **Model format**: CoreML `.mlpackage` instead of GGUF
//! - **Tokenization**: HuggingFace `tokenizers` crate instead of llama.cpp built-in
//! - **Compute**: Apple Neural Engine on Apple Silicon, CPU fallback elsewhere
//! - **Pooling**: Mean pooling baked into the `.mlpackage` at conversion time

pub mod error;
pub mod model;
pub mod types;

pub use error::EmbeddingError;
pub use model::AneEmbeddingModel;
pub use model_embedding::TextEmbedder;
pub use types::{AneEmbeddingConfig, DEFAULT_MODEL_PREFIX, DEFAULT_SEQ_LENGTH};
