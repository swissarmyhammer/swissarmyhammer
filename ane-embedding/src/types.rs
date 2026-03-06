use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Default max sequence length for ANE embedding.
pub const DEFAULT_SEQ_LENGTH: usize = 128;

/// Default model name prefix for .mlpackage files.
pub const DEFAULT_MODEL_PREFIX: &str = "Qwen3-Embedding-0.6B";

/// Configuration for the ANE embedding model.
///
/// Uses a single static-shape FP16 `.mlpackage` for a fixed sequence length.
/// The model file is named `{model_prefix}-seq{seq_length}.mlpackage`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AneEmbeddingConfig {
    /// Directory containing the .mlpackage and tokenizer.json
    pub model_dir: PathBuf,
    /// Prefix for the model filename (e.g. "Qwen3-Embedding-0.6B")
    pub model_prefix: String,
    /// Whether to L2-normalize embeddings to unit vectors
    pub normalize_embeddings: bool,
    /// Fixed sequence length for the static-shape model.
    /// Inputs are padded or truncated to this length.
    pub seq_length: usize,
    /// Enable debug logging
    pub debug: bool,
}

impl Default for AneEmbeddingConfig {
    fn default() -> Self {
        Self {
            model_dir: PathBuf::from("var/data/models/qwen3-embedding-0.6b"),
            model_prefix: DEFAULT_MODEL_PREFIX.to_string(),
            normalize_embeddings: true,
            seq_length: DEFAULT_SEQ_LENGTH,
            debug: false,
        }
    }
}

impl AneEmbeddingConfig {
    /// Return the .mlpackage path.
    pub fn model_path(&self) -> PathBuf {
        self.model_dir.join(format!(
            "{}-seq{}.mlpackage",
            self.model_prefix, self.seq_length
        ))
    }

    /// Return the tokenizer.json path.
    pub fn tokenizer_path(&self) -> PathBuf {
        self.model_dir.join("tokenizer.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AneEmbeddingConfig::default();
        assert!(config.normalize_embeddings);
        assert_eq!(config.seq_length, 128);
        assert_eq!(config.model_prefix, "Qwen3-Embedding-0.6B");
        assert!(!config.debug);
    }

    #[test]
    fn test_config_serialization() {
        let config = AneEmbeddingConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AneEmbeddingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.normalize_embeddings, config.normalize_embeddings);
        assert_eq!(parsed.seq_length, config.seq_length);
        assert_eq!(parsed.model_prefix, config.model_prefix);
    }

    #[test]
    fn test_model_path() {
        let config = AneEmbeddingConfig::default();
        assert_eq!(
            config.model_path(),
            PathBuf::from(
                "var/data/models/qwen3-embedding-0.6b/Qwen3-Embedding-0.6B-seq128.mlpackage"
            )
        );
    }

    #[test]
    fn test_tokenizer_path() {
        let config = AneEmbeddingConfig::default();
        assert_eq!(
            config.tokenizer_path(),
            PathBuf::from("var/data/models/qwen3-embedding-0.6b/tokenizer.json")
        );
    }
}
