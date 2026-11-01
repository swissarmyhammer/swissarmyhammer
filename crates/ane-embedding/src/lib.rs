//! Text embedding using CoreML for Apple Neural Engine.
//!
//! This crate implements the [`TextEmbedder`] trait from `model-embedding`,
//! loading a `.mlpackage` model directly via `objc2-core-ml` for hardware-accelerated
//! inference on Apple Silicon's Neural Engine.
//!
//! # Design
//!
//! - **Runtime**: CoreML (via `objc2-core-ml`)
//! - **Model format**: CoreML `.mlpackage`
//! - **Tokenization**: HuggingFace `tokenizers` crate
//! - **Compute**: Apple Neural Engine on Apple Silicon, CPU fallback elsewhere
//! - **Pooling**: Mean pooling baked into the `.mlpackage` at conversion time

#[cfg(target_os = "macos")]
mod coreml;
pub mod error;
#[cfg(target_os = "macos")]
pub mod model;
pub mod types;

pub use error::EmbeddingError;
#[cfg(target_os = "macos")]
pub use model::AneEmbeddingModel;
pub use model_embedding::TextEmbedder;
pub use types::{AneEmbeddingConfig, DEFAULT_MODEL_PREFIX, DEFAULT_SEQ_LENGTH};
