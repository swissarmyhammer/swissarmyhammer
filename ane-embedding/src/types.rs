use model_loader::ModelSource;
use serde::{Deserialize, Serialize};

/// Pooling strategy for converting per-token embeddings to a single vector.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Pooling {
    /// Average all token embeddings (most common for sentence embeddings)
    #[default]
    Mean,
    /// Use the first token ([CLS]) embedding
    Cls,
    /// Use the last token embedding
    LastToken,
}

/// Configuration for the ANE embedding model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AneEmbeddingConfig {
    /// Model source (HuggingFace repo or local path)
    pub model_source: ModelSource,
    /// Whether to L2-normalize embeddings to unit vectors
    pub normalize_embeddings: bool,
    /// Maximum sequence length (tokens). None = use model's configured max.
    pub max_sequence_length: Option<usize>,
    /// Pooling strategy for per-token to sentence embedding
    pub pooling: Pooling,
    /// Enable debug logging
    pub debug: bool,
}

impl Default for AneEmbeddingConfig {
    fn default() -> Self {
        Self {
            model_source: ModelSource::HuggingFace {
                repo: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
                filename: Some("model.onnx".to_string()),
                folder: Some("onnx".to_string()),
            },
            normalize_embeddings: true,
            max_sequence_length: None,
            pooling: Pooling::Mean,
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
        assert_eq!(config.pooling, Pooling::Mean);
        assert!(config.max_sequence_length.is_none());
        assert!(!config.debug);
    }

    #[test]
    fn test_pooling_default() {
        assert_eq!(Pooling::default(), Pooling::Mean);
    }

    #[test]
    fn test_config_serialization() {
        let config = AneEmbeddingConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AneEmbeddingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pooling, config.pooling);
        assert_eq!(parsed.normalize_embeddings, config.normalize_embeddings);
    }
}
