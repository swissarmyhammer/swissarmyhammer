//! Standard test models for llama-agent testing
//!
//! This module defines the canonical test models used across all llama-agent tests.
//! Use these constants to ensure consistency and avoid model file mismatches.
//!
//! # Test Model Selection
//!
//! These models are chosen for testing because they:
//! - Are small (~600MB) and download quickly from HuggingFace
//! - Support both chat/generation and embedding tasks
//! - Are quantized (Q4) for efficient resource usage
//! - Are actively maintained and widely used
//!
//! # Usage
//!
//! ```rust
//! use llama_agent::test_models::{TEST_MODEL_REPO, TEST_MODEL_FILE};
//! use llama_agent::types::{ModelConfig, ModelSource};
//!
//! let model_config = ModelConfig {
//!     source: ModelSource::HuggingFace {
//!         repo: TEST_MODEL_REPO.to_string(),
//!         filename: Some(TEST_MODEL_FILE.to_string()),
//!         folder: None,
//!     },
//!     // ... other config fields
//! };
//! ```

/// Standard test model repository for chat/generation tasks
///
/// **Model**: Qwen3-0.6B (0.6 billion parameters)
/// **Size**: ~600MB
/// **Quantization**: IQ4_NL (4-bit)
///
/// This is the canonical model for all llama-agent generation tests.
/// Use this instead of hardcoding model names in tests.
pub const TEST_MODEL_REPO: &str = "unsloth/Qwen3-0.6B-GGUF";

/// Standard test model filename for chat/generation tasks
///
/// **File**: Qwen3-0.6B-IQ4_NL.gguf
/// **Quantization**: IQ4_NL (improved 4-bit quantization with normal distribution)
///
/// This quantization provides good quality while keeping file size manageable.
pub const TEST_MODEL_FILE: &str = "Qwen3-0.6B-IQ4_NL.gguf";

/// Standard test model repository for embedding tasks
///
/// **Model**: Qwen3-Embedding-0.6B (0.6 billion parameters)
/// **Size**: ~1.2GB
/// **Output**: 1024-dimensional embeddings
///
/// This is the canonical model for all llama-agent embedding tests.
pub const TEST_EMBEDDING_MODEL_REPO: &str = "Qwen/Qwen3-Embedding-0.6B-GGUF";

/// Default embedding model filename
///
/// When None, uses the default file from the repository.
/// The model manager will select the appropriate file automatically.
pub const TEST_EMBEDDING_MODEL_FILE: Option<&str> = None;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_constants_are_valid() {
        // Verify constants are non-empty
        assert!(!TEST_MODEL_REPO.is_empty());
        assert!(!TEST_MODEL_FILE.is_empty());
        assert!(!TEST_EMBEDDING_MODEL_REPO.is_empty());
    }

    #[test]
    fn test_model_repo_format() {
        // Verify repo format is "namespace/repo-name"
        assert!(TEST_MODEL_REPO.contains('/'));
        assert_eq!(TEST_MODEL_REPO.matches('/').count(), 1);

        assert!(TEST_EMBEDDING_MODEL_REPO.contains('/'));
        assert_eq!(TEST_EMBEDDING_MODEL_REPO.matches('/').count(), 1);
    }

    #[test]
    fn test_model_file_extension() {
        // Verify file has .gguf extension
        assert!(TEST_MODEL_FILE.ends_with(".gguf"));
    }

    #[test]
    fn test_model_file_naming_convention() {
        // Verify quantization is in filename
        assert!(TEST_MODEL_FILE.contains("IQ4_NL"));
    }
}
