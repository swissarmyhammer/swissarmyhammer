use model_loader::ModelSource;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the ANE embedding model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AneEmbeddingConfig {
    /// Model source (HuggingFace repo or local path to .mlpackage)
    pub model_source: ModelSource,
    /// Whether to L2-normalize embeddings to unit vectors
    pub normalize_embeddings: bool,
    /// Maximum sequence length (tokens). Must match the .mlpackage static shape.
    pub max_sequence_length: usize,
    /// Enable debug logging
    pub debug: bool,
}

impl Default for AneEmbeddingConfig {
    fn default() -> Self {
        Self {
            model_source: ModelSource::Local {
                folder: PathBuf::from("var/data/models/qwen3-embedding-0.6b"),
                filename: Some("Qwen3-Embedding-0.6B.mlpackage".to_string()),
            },
            normalize_embeddings: true,
            max_sequence_length: 512,
            debug: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AneEmbeddingConfig::default();
        assert!(config.normalize_embeddings);
        assert_eq!(config.max_sequence_length, 512);
        assert!(!config.debug);
    }

    #[test]
    fn test_config_serialization() {
        let config = AneEmbeddingConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AneEmbeddingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.normalize_embeddings, config.normalize_embeddings);
        assert_eq!(parsed.max_sequence_length, config.max_sequence_length);
    }
}
