//! # Model Loader
//!
//! Generic model source resolution, download, and caching — runtime-agnostic.
//!
//! This crate resolves model sources (HuggingFace repos, local paths) to local
//! file paths. It handles downloading, caching, retry logic, and multi-part
//! files, but does NOT load models into any specific runtime. Consumers
//! (llama-agent, ane-embedding, etc.) use the resolved path to load models
//! into their own backend.

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
    load_huggingface_model_with_path, load_huggingface_model_with_path_and_folder,
};
pub use loader::ModelResolver;
pub use multipart::{download_folder_model, download_multi_part_model};
pub use types::{
    ModelConfig, ModelMetadata, ModelSource, ResolvedModel, RetryConfig, MODEL_EXTENSIONS,
};
