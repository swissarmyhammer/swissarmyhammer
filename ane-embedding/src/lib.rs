//! Text embedding using ONNX Runtime with CoreML for Apple Neural Engine.
//!
//! This crate implements the [`TextEmbedder`] trait from `model-embedding`,
//! using ONNX Runtime with CoreML execution provider for hardware-accelerated
//! inference on Apple Silicon.
//!
//! # Differences from llama-embedding
//!
//! - **Runtime**: ONNX Runtime (via `onnxruntime-coreml-sys`) instead of llama.cpp
//! - **Model format**: ONNX instead of GGUF
//! - **Tokenization**: HuggingFace `tokenizers` crate instead of llama.cpp built-in
//! - **Compute**: CoreML EP on Apple Silicon, CPU fallback elsewhere
//! - **Pooling**: Explicit mean pooling (ONNX models output per-token embeddings)

pub mod error;
pub mod model;
pub mod types;

pub use error::EmbeddingError;
pub use model::AneEmbeddingModel;
pub use model_embedding::TextEmbedder;
pub use model_loader::ModelSource;
pub use types::AneEmbeddingConfig;
