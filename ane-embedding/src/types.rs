use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Default max sequence length for ANE embedding.
pub const DEFAULT_SEQ_LENGTH: usize = 256;

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
        assert_eq!(config.seq_length, 256);
        assert_eq!(config.model_prefix, "Qwen3-Embedding-0.6B");
        assert!(!config.debug);
    }

    #[test]
    fn test_default_config_model_dir() {
        let config = AneEmbeddingConfig::default();
        assert_eq!(
            config.model_dir,
            PathBuf::from("var/data/models/qwen3-embedding-0.6b")
        );
    }

    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_SEQ_LENGTH, 256);
        assert_eq!(DEFAULT_MODEL_PREFIX, "Qwen3-Embedding-0.6B");
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
    fn test_config_serialization_custom_values() {
        let config = AneEmbeddingConfig {
            model_dir: PathBuf::from("/custom/path"),
            model_prefix: "MyModel".to_string(),
            normalize_embeddings: false,
            seq_length: 128,
            debug: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AneEmbeddingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.model_dir, PathBuf::from("/custom/path"));
        assert_eq!(parsed.model_prefix, "MyModel");
        assert!(!parsed.normalize_embeddings);
        assert_eq!(parsed.seq_length, 128);
        assert!(parsed.debug);
    }

    #[test]
    fn test_config_deserialization_from_json() {
        let json = r#"{
            "model_dir": "/tmp/models",
            "model_prefix": "test-model",
            "normalize_embeddings": false,
            "seq_length": 64,
            "debug": true
        }"#;
        let config: AneEmbeddingConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.model_dir, PathBuf::from("/tmp/models"));
        assert_eq!(config.model_prefix, "test-model");
        assert!(!config.normalize_embeddings);
        assert_eq!(config.seq_length, 64);
        assert!(config.debug);
    }

    #[test]
    fn test_model_path() {
        let config = AneEmbeddingConfig::default();
        assert_eq!(
            config.model_path(),
            PathBuf::from(
                "var/data/models/qwen3-embedding-0.6b/Qwen3-Embedding-0.6B-seq256.mlpackage"
            )
        );
    }

    #[test]
    fn test_model_path_custom() {
        let config = AneEmbeddingConfig {
            model_dir: PathBuf::from("/models"),
            model_prefix: "BERT".to_string(),
            seq_length: 512,
            ..AneEmbeddingConfig::default()
        };
        assert_eq!(
            config.model_path(),
            PathBuf::from("/models/BERT-seq512.mlpackage")
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

    #[test]
    fn test_tokenizer_path_custom_dir() {
        let config = AneEmbeddingConfig {
            model_dir: PathBuf::from("/custom/dir"),
            ..AneEmbeddingConfig::default()
        };
        assert_eq!(
            config.tokenizer_path(),
            PathBuf::from("/custom/dir/tokenizer.json")
        );
    }

    #[test]
    fn test_config_debug_impl() {
        let config = AneEmbeddingConfig::default();
        let debug = format!("{config:?}");
        assert!(debug.contains("AneEmbeddingConfig"));
        assert!(debug.contains("seq_length"));
    }

    #[test]
    fn test_config_clone() {
        let config = AneEmbeddingConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.model_dir, config.model_dir);
        assert_eq!(cloned.model_prefix, config.model_prefix);
        assert_eq!(cloned.seq_length, config.seq_length);
        assert_eq!(cloned.normalize_embeddings, config.normalize_embeddings);
        assert_eq!(cloned.debug, config.debug);
    }
}
