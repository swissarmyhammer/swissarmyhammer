//! # Llama Loader
//!
//! Shared model loading functionality for the llama-agent ecosystem.
//! This crate provides common types and interfaces for loading GGUF models
//! from HuggingFace and local sources.

pub mod detection;
pub mod download_lock;
pub mod error;
pub mod huggingface;
pub mod loader;
pub mod multipart;
pub mod retry;
pub mod types;

// Re-export main types for convenience
pub use detection::{
    auto_detect_hf_model_file, auto_detect_hf_model_file_with_folder, get_folder_files,
};
pub use download_lock::DownloadCoordinator;
pub use error::ModelError;
pub use huggingface::{
    load_huggingface_model, load_huggingface_model_with_folder, load_huggingface_model_with_path,
    load_huggingface_model_with_path_and_folder,
};
pub use loader::ModelLoader;
pub use multipart::{download_folder_model, download_multi_part_model};
pub use types::{LoadedModel, ModelConfig, ModelMetadata, ModelSource, RetryConfig};
