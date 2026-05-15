use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::Duration;

/// Known model file extensions for auto-detection.
///
/// Used by both local directory scanning and HuggingFace repository detection.
pub const MODEL_EXTENSIONS: &[&str] =
    &["gguf", "onnx", "mlmodel", "mlpackage", "bin", "safetensors"];

/// A resolved model with its file path and metadata.
///
/// This is the result of model resolution — the model file has been located
/// (downloaded from HuggingFace or found locally) but not loaded into any
/// runtime. Consumers (llama-agent, ane-embedding, etc.) load the file
/// into their own backend.
#[derive(Debug)]
pub struct ResolvedModel {
    /// Path to the model file on disk
    pub path: PathBuf,
    /// Metadata about the resolution process
    pub metadata: ModelMetadata,
}

/// Metadata about a resolved model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// The source from which the model was resolved
    pub source: ModelSource,
    /// The filename of the model
    pub filename: String,
    /// Size of the model file in bytes
    pub size_bytes: u64,
    /// Time taken to resolve (download/locate) the model
    #[serde(with = "duration_secs")]
    pub resolve_time: Duration,
    /// Whether this model was found in cache
    pub cache_hit: bool,
}

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_f64(d.as_secs_f64())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let secs = f64::deserialize(d)?;
        Ok(Duration::from_secs_f64(secs))
    }
}

/// Configuration for model retry logic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay between retries in milliseconds
    pub initial_delay_ms: u64,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
    /// Maximum delay between retries in milliseconds
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000, // 1 second
            backoff_multiplier: 2.0,
            max_delay_ms: 30000, // 30 seconds
        }
    }
}

impl From<RetryConfig> for llama_common::retry::RetryConfig {
    fn from(old_config: RetryConfig) -> Self {
        llama_common::retry::RetryConfig {
            max_retries: old_config.max_retries,
            initial_delay: Duration::from_millis(old_config.initial_delay_ms),
            backoff_multiplier: old_config.backoff_multiplier,
            max_delay: Duration::from_millis(old_config.max_delay_ms),
            use_jitter: true, // Enable jitter by default for better behavior
        }
    }
}

impl From<llama_common::retry::RetryConfig> for RetryConfig {
    fn from(new_config: llama_common::retry::RetryConfig) -> Self {
        Self {
            max_retries: new_config.max_retries,
            initial_delay_ms: new_config.initial_delay.as_millis().min(u64::MAX as u128) as u64,
            backoff_multiplier: new_config.backoff_multiplier,
            max_delay_ms: new_config.max_delay.as_millis().min(u64::MAX as u128) as u64,
        }
    }
}

/// Represents different sources from which models can be loaded
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ModelSource {
    /// Load from HuggingFace repository
    HuggingFace {
        /// Repository name (e.g., "microsoft/DialoGPT-medium")
        repo: String,
        /// Optional specific filename to load
        filename: Option<String>,
        /// Optional folder within the repository (for chunked models)
        folder: Option<String>,
    },
    /// Load from local filesystem
    Local {
        /// Path to the folder containing the model
        folder: PathBuf,
        /// Optional specific filename to load
        filename: Option<String>,
    },
}

/// Configuration for model resolution
///
/// Contains only the fields needed by the resolver: source location,
/// retry behavior, and debug logging. Consumer-specific fields (batch size,
/// thread counts, etc.) belong in the consumer's own config type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// The source from which to load the model
    pub source: ModelSource,
    /// Configuration for retry logic
    pub retry_config: RetryConfig,
    /// Enable debug output
    pub debug: bool,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            source: ModelSource::HuggingFace {
                repo: "microsoft/DialoGPT-medium".to_string(),
                filename: None,
                folder: None,
            },
            retry_config: RetryConfig::default(),
            debug: false,
        }
    }
}

impl ModelConfig {
    /// Validate the model configuration
    pub fn validate(&self) -> Result<(), crate::error::ModelError> {
        self.source.validate()?;
        Ok(())
    }

    /// Compute a hash of the model source for creating unique cache directories
    pub fn compute_model_hash(&self) -> String {
        let mut hasher = DefaultHasher::new();

        match &self.source {
            ModelSource::HuggingFace {
                repo,
                filename,
                folder,
            } => {
                repo.hash(&mut hasher);
                filename.hash(&mut hasher);
                folder.hash(&mut hasher);
            }
            ModelSource::Local { folder, filename } => {
                folder.hash(&mut hasher);
                filename.hash(&mut hasher);
            }
        }

        format!("{:x}", hasher.finish())
    }
}

impl ModelSource {
    /// Validate that the model source configuration is valid
    pub fn validate(&self) -> Result<(), crate::error::ModelError> {
        match self {
            ModelSource::HuggingFace {
                repo,
                filename,
                folder,
            } => {
                if repo.is_empty() {
                    return Err(crate::error::ModelError::InvalidConfig(
                        "HuggingFace repo name cannot be empty".to_string(),
                    ));
                }

                // Validate repo format (should contain at least one '/')
                if !repo.contains('/') {
                    return Err(crate::error::ModelError::InvalidConfig(
                        "HuggingFace repo must be in format 'org/repo'".to_string(),
                    ));
                }

                // Check for invalid characters
                if repo
                    .chars()
                    .any(|c| !c.is_alphanumeric() && !"-_./".contains(c))
                {
                    return Err(crate::error::ModelError::InvalidConfig(
                        "Invalid characters in HuggingFace repo name".to_string(),
                    ));
                }

                if let Some(f) = filename {
                    if f.is_empty() {
                        return Err(crate::error::ModelError::InvalidConfig(
                            "Filename cannot be empty".to_string(),
                        ));
                    }

                    // Validate file extension against supported model formats
                    let lower = f.to_lowercase();
                    let has_valid_ext = MODEL_EXTENSIONS
                        .iter()
                        .any(|ext| lower.ends_with(&format!(".{}", ext)));
                    if !has_valid_ext {
                        return Err(crate::error::ModelError::InvalidConfig(format!(
                            "Unsupported model file extension in '{}'. Supported: {}",
                            f,
                            MODEL_EXTENSIONS.join(", ")
                        )));
                    }
                }

                if let Some(f) = folder {
                    if f.is_empty() {
                        return Err(crate::error::ModelError::InvalidConfig(
                            "Folder name cannot be empty".to_string(),
                        ));
                    }
                    // Validate folder format (no leading/trailing slashes, no invalid characters)
                    if f.starts_with('/') || f.ends_with('/') {
                        return Err(crate::error::ModelError::InvalidConfig(
                            "Folder name should not have leading or trailing slashes".to_string(),
                        ));
                    }
                }

                Ok(())
            }
            ModelSource::Local { folder, filename } => {
                if !folder.exists() {
                    return Err(crate::error::ModelError::NotFound(format!(
                        "Local folder does not exist: {}",
                        folder.display()
                    )));
                }

                if !folder.is_dir() {
                    return Err(crate::error::ModelError::InvalidConfig(format!(
                        "Path is not a directory: {}",
                        folder.display()
                    )));
                }

                if let Some(f) = filename {
                    if f.is_empty() {
                        return Err(crate::error::ModelError::InvalidConfig(
                            "Filename cannot be empty".to_string(),
                        ));
                    }

                    let full_path = folder.join(f);
                    if !full_path.exists() {
                        return Err(crate::error::ModelError::NotFound(format!(
                            "Model file does not exist: {}",
                            full_path.display()
                        )));
                    }

                    // Allow both files and directories (.mlpackage is a directory bundle)
                    if !full_path.is_file() && !full_path.is_dir() {
                        return Err(crate::error::ModelError::InvalidConfig(format!(
                            "Path is not a file or package: {}",
                            full_path.display()
                        )));
                    }
                }

                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_model_source_validation_huggingface() {
        // Valid HuggingFace repo
        let source = ModelSource::HuggingFace {
            repo: "microsoft/DialoGPT-medium".to_string(),
            filename: Some("model.gguf".to_string()),
            folder: None,
        };
        assert!(source.validate().is_ok());

        // Any file extension is valid now (not just .gguf)
        let source = ModelSource::HuggingFace {
            repo: "microsoft/DialoGPT-medium".to_string(),
            filename: Some("model.onnx".to_string()),
            folder: None,
        };
        assert!(source.validate().is_ok());

        // Empty repo
        let source = ModelSource::HuggingFace {
            repo: "".to_string(),
            filename: None,
            folder: None,
        };
        assert!(source.validate().is_err());

        // Invalid repo format (no slash)
        let source = ModelSource::HuggingFace {
            repo: "invalid-repo".to_string(),
            filename: None,
            folder: None,
        };
        assert!(source.validate().is_err());

        // Empty filename
        let source = ModelSource::HuggingFace {
            repo: "microsoft/DialoGPT-medium".to_string(),
            filename: Some("".to_string()),
            folder: None,
        };
        assert!(source.validate().is_err());
    }

    #[test]
    fn test_model_source_validation_local() {
        // Test with actual temp directory
        let temp_dir = std::env::temp_dir();

        // Valid local source with existing directory
        let source = ModelSource::Local {
            folder: temp_dir.clone(),
            filename: None,
        };
        assert!(source.validate().is_ok());

        // Non-existent directory
        let source = ModelSource::Local {
            folder: PathBuf::from("/non/existent/path"),
            filename: None,
        };
        assert!(source.validate().is_err());

        // Empty filename
        let source = ModelSource::Local {
            folder: temp_dir,
            filename: Some("".to_string()),
        };
        assert!(source.validate().is_err());
    }

    #[test]
    fn test_model_metadata_creation() {
        let metadata = ModelMetadata {
            source: ModelSource::HuggingFace {
                repo: "test/repo".to_string(),
                filename: Some("test.gguf".to_string()),
                folder: None,
            },
            filename: "test.gguf".to_string(),
            size_bytes: 1024,
            resolve_time: Duration::from_secs(1),
            cache_hit: false,
        };

        assert_eq!(metadata.filename, "test.gguf");
        assert_eq!(metadata.size_bytes, 1024);
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay_ms, 1000);
        assert_eq!(config.backoff_multiplier, 2.0);
        assert_eq!(config.max_delay_ms, 30000);
    }

    #[test]
    fn test_retry_config_into_llama_common() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay_ms: 2000,
            backoff_multiplier: 3.0,
            max_delay_ms: 60000,
        };
        let common: llama_common::retry::RetryConfig = config.into();
        assert_eq!(common.max_retries, 5);
        assert_eq!(common.initial_delay, Duration::from_millis(2000));
        assert_eq!(common.backoff_multiplier, 3.0);
        assert_eq!(common.max_delay, Duration::from_millis(60000));
        assert!(common.use_jitter); // Always enabled
    }

    #[test]
    fn test_retry_config_from_llama_common() {
        let common = llama_common::retry::RetryConfig {
            max_retries: 7,
            initial_delay: Duration::from_millis(500),
            backoff_multiplier: 1.5,
            max_delay: Duration::from_millis(10000),
            use_jitter: false,
        };
        let config: RetryConfig = common.into();
        assert_eq!(config.max_retries, 7);
        assert_eq!(config.initial_delay_ms, 500);
        assert_eq!(config.backoff_multiplier, 1.5);
        assert_eq!(config.max_delay_ms, 10000);
    }

    #[test]
    fn test_retry_config_roundtrip() {
        let original = RetryConfig {
            max_retries: 4,
            initial_delay_ms: 1500,
            backoff_multiplier: 2.5,
            max_delay_ms: 45000,
        };
        let common: llama_common::retry::RetryConfig = original.clone().into();
        let roundtrip: RetryConfig = common.into();
        assert_eq!(roundtrip.max_retries, original.max_retries);
        assert_eq!(roundtrip.initial_delay_ms, original.initial_delay_ms);
        assert_eq!(roundtrip.backoff_multiplier, original.backoff_multiplier);
        assert_eq!(roundtrip.max_delay_ms, original.max_delay_ms);
    }

    #[test]
    fn test_model_config_default() {
        let config = ModelConfig::default();
        assert!(!config.debug);
        assert_eq!(config.retry_config.max_retries, 3);
        match &config.source {
            ModelSource::HuggingFace {
                repo,
                filename,
                folder,
            } => {
                assert_eq!(repo, "microsoft/DialoGPT-medium");
                assert!(filename.is_none());
                assert!(folder.is_none());
            }
            _ => panic!("Expected HuggingFace source"),
        }
    }

    #[test]
    fn test_model_config_validate_delegates_to_source() {
        // Valid config
        let config = ModelConfig::default();
        assert!(config.validate().is_ok());

        // Invalid config (empty repo)
        let config = ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "".to_string(),
                filename: None,
                folder: None,
            },
            retry_config: RetryConfig::default(),
            debug: false,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_compute_model_hash_huggingface() {
        let config1 = ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "org/repo".to_string(),
                filename: Some("model.gguf".to_string()),
                folder: None,
            },
            retry_config: RetryConfig::default(),
            debug: false,
        };
        let config2 = ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "org/other-repo".to_string(),
                filename: Some("model.gguf".to_string()),
                folder: None,
            },
            retry_config: RetryConfig::default(),
            debug: false,
        };

        let hash1 = config1.compute_model_hash();
        let hash2 = config2.compute_model_hash();
        assert_ne!(
            hash1, hash2,
            "Different repos should yield different hashes"
        );

        // Same config should yield same hash
        let hash1_again = config1.compute_model_hash();
        assert_eq!(hash1, hash1_again);
    }

    #[test]
    fn test_compute_model_hash_local() {
        let config = ModelConfig {
            source: ModelSource::Local {
                folder: PathBuf::from("/some/path"),
                filename: Some("model.gguf".to_string()),
            },
            retry_config: RetryConfig::default(),
            debug: false,
        };
        let hash = config.compute_model_hash();
        assert!(!hash.is_empty());

        // Different folder produces different hash
        let config2 = ModelConfig {
            source: ModelSource::Local {
                folder: PathBuf::from("/other/path"),
                filename: Some("model.gguf".to_string()),
            },
            retry_config: RetryConfig::default(),
            debug: false,
        };
        assert_ne!(config.compute_model_hash(), config2.compute_model_hash());
    }

    #[test]
    fn test_compute_model_hash_folder_changes_hash() {
        let config_no_folder = ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "org/repo".to_string(),
                filename: Some("model.gguf".to_string()),
                folder: None,
            },
            retry_config: RetryConfig::default(),
            debug: false,
        };
        let config_with_folder = ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "org/repo".to_string(),
                filename: Some("model.gguf".to_string()),
                folder: Some("subfolder".to_string()),
            },
            retry_config: RetryConfig::default(),
            debug: false,
        };
        assert_ne!(
            config_no_folder.compute_model_hash(),
            config_with_folder.compute_model_hash()
        );
    }

    #[test]
    fn test_model_source_hf_invalid_characters() {
        let source = ModelSource::HuggingFace {
            repo: "org/repo with spaces".to_string(),
            filename: None,
            folder: None,
        };
        let err = source.validate().unwrap_err();
        assert!(format!("{}", err).contains("Invalid characters"));
    }

    #[test]
    fn test_model_source_hf_unsupported_extension() {
        let source = ModelSource::HuggingFace {
            repo: "org/repo".to_string(),
            filename: Some("model.txt".to_string()),
            folder: None,
        };
        let err = source.validate().unwrap_err();
        assert!(format!("{}", err).contains("Unsupported model file extension"));
    }

    #[test]
    fn test_model_source_hf_all_valid_extensions() {
        for ext in MODEL_EXTENSIONS {
            let source = ModelSource::HuggingFace {
                repo: "org/repo".to_string(),
                filename: Some(format!("model.{}", ext)),
                folder: None,
            };
            assert!(
                source.validate().is_ok(),
                "Extension .{} should be valid",
                ext
            );
        }
    }

    #[test]
    fn test_model_source_hf_empty_folder() {
        let source = ModelSource::HuggingFace {
            repo: "org/repo".to_string(),
            filename: None,
            folder: Some("".to_string()),
        };
        let err = source.validate().unwrap_err();
        assert!(format!("{}", err).contains("Folder name cannot be empty"));
    }

    #[test]
    fn test_model_source_hf_folder_leading_slash() {
        let source = ModelSource::HuggingFace {
            repo: "org/repo".to_string(),
            filename: None,
            folder: Some("/leading".to_string()),
        };
        let err = source.validate().unwrap_err();
        assert!(format!("{}", err).contains("leading or trailing slashes"));
    }

    #[test]
    fn test_model_source_hf_folder_trailing_slash() {
        let source = ModelSource::HuggingFace {
            repo: "org/repo".to_string(),
            filename: None,
            folder: Some("trailing/".to_string()),
        };
        let err = source.validate().unwrap_err();
        assert!(format!("{}", err).contains("leading or trailing slashes"));
    }

    #[test]
    fn test_model_source_hf_valid_folder() {
        let source = ModelSource::HuggingFace {
            repo: "org/repo".to_string(),
            filename: None,
            folder: Some("subfolder".to_string()),
        };
        assert!(source.validate().is_ok());
    }

    #[test]
    fn test_model_source_local_not_a_directory() {
        // Create a temp file (not a directory)
        let temp = tempfile::NamedTempFile::new().unwrap();
        let source = ModelSource::Local {
            folder: temp.path().to_path_buf(),
            filename: None,
        };
        let err = source.validate().unwrap_err();
        assert!(format!("{}", err).contains("not a directory"));
    }

    #[test]
    fn test_model_source_local_file_exists() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let model_file = temp_dir.path().join("model.gguf");
        std::fs::write(&model_file, "fake model").unwrap();

        let source = ModelSource::Local {
            folder: temp_dir.path().to_path_buf(),
            filename: Some("model.gguf".to_string()),
        };
        assert!(source.validate().is_ok());
    }

    #[test]
    fn test_model_source_local_file_not_found() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let source = ModelSource::Local {
            folder: temp_dir.path().to_path_buf(),
            filename: Some("nonexistent.gguf".to_string()),
        };
        let err = source.validate().unwrap_err();
        assert!(format!("{}", err).contains("does not exist"));
    }

    #[test]
    fn test_model_source_serialization() {
        let source = ModelSource::HuggingFace {
            repo: "org/repo".to_string(),
            filename: Some("model.gguf".to_string()),
            folder: None,
        };
        let json = serde_json::to_string(&source).unwrap();
        let deserialized: ModelSource = serde_json::from_str(&json).unwrap();
        assert_eq!(source, deserialized);
    }

    #[test]
    fn test_model_source_local_serialization() {
        let source = ModelSource::Local {
            folder: PathBuf::from("/tmp/models"),
            filename: Some("model.gguf".to_string()),
        };
        let json = serde_json::to_string(&source).unwrap();
        let deserialized: ModelSource = serde_json::from_str(&json).unwrap();
        assert_eq!(source, deserialized);
    }

    #[test]
    fn test_model_metadata_serialization_roundtrip() {
        let metadata = ModelMetadata {
            source: ModelSource::HuggingFace {
                repo: "test/repo".to_string(),
                filename: Some("model.gguf".to_string()),
                folder: Some("subfolder".to_string()),
            },
            filename: "model.gguf".to_string(),
            size_bytes: 4096,
            resolve_time: Duration::from_secs_f64(1.5),
            cache_hit: true,
        };
        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: ModelMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.filename, "model.gguf");
        assert_eq!(deserialized.size_bytes, 4096);
        assert!(deserialized.cache_hit);
        // Duration roundtrip should be close (floating point)
        let diff = (deserialized.resolve_time.as_secs_f64() - 1.5).abs();
        assert!(diff < 0.001, "Duration roundtrip mismatch: {}", diff);
    }

    #[test]
    fn test_retry_config_serialization() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay_ms: 2000,
            backoff_multiplier: 3.0,
            max_delay_ms: 60000,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RetryConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.max_retries, 5);
        assert_eq!(deserialized.initial_delay_ms, 2000);
        assert_eq!(deserialized.backoff_multiplier, 3.0);
        assert_eq!(deserialized.max_delay_ms, 60000);
    }

    #[test]
    fn test_model_config_serialization() {
        let config = ModelConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ModelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.debug, config.debug);
        assert_eq!(
            deserialized.retry_config.max_retries,
            config.retry_config.max_retries
        );
    }

    #[test]
    fn test_resolved_model_creation() {
        let model = ResolvedModel {
            path: PathBuf::from("/tmp/model.gguf"),
            metadata: ModelMetadata {
                source: ModelSource::Local {
                    folder: PathBuf::from("/tmp"),
                    filename: Some("model.gguf".to_string()),
                },
                filename: "model.gguf".to_string(),
                size_bytes: 2048,
                resolve_time: Duration::from_millis(100),
                cache_hit: false,
            },
        };
        assert_eq!(model.path, PathBuf::from("/tmp/model.gguf"));
        assert_eq!(model.metadata.size_bytes, 2048);
        assert!(!model.metadata.cache_hit);
    }

    #[test]
    fn test_model_extensions_constant() {
        assert!(MODEL_EXTENSIONS.contains(&"gguf"));
        assert!(MODEL_EXTENSIONS.contains(&"onnx"));
        assert!(MODEL_EXTENSIONS.contains(&"mlmodel"));
        assert!(MODEL_EXTENSIONS.contains(&"mlpackage"));
        assert!(MODEL_EXTENSIONS.contains(&"bin"));
        assert!(MODEL_EXTENSIONS.contains(&"safetensors"));
        assert!(!MODEL_EXTENSIONS.contains(&"txt"));
        assert!(!MODEL_EXTENSIONS.contains(&"json"));
    }

    #[test]
    fn test_model_source_equality() {
        let a = ModelSource::HuggingFace {
            repo: "org/repo".to_string(),
            filename: None,
            folder: None,
        };
        let b = ModelSource::HuggingFace {
            repo: "org/repo".to_string(),
            filename: None,
            folder: None,
        };
        assert_eq!(a, b);

        let c = ModelSource::HuggingFace {
            repo: "org/other".to_string(),
            filename: None,
            folder: None,
        };
        assert_ne!(a, c);
    }

    #[test]
    fn test_model_source_clone() {
        let source = ModelSource::HuggingFace {
            repo: "org/repo".to_string(),
            filename: Some("model.gguf".to_string()),
            folder: Some("sub".to_string()),
        };
        let cloned = source.clone();
        assert_eq!(source, cloned);
    }

    #[test]
    fn test_retry_config_clone() {
        let config = RetryConfig {
            max_retries: 10,
            initial_delay_ms: 500,
            backoff_multiplier: 1.5,
            max_delay_ms: 5000,
        };
        let cloned = config.clone();
        assert_eq!(cloned.max_retries, 10);
        assert_eq!(cloned.initial_delay_ms, 500);
    }

    #[test]
    fn test_model_config_clone() {
        let config = ModelConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.debug, config.debug);
    }

    #[test]
    fn test_model_metadata_clone() {
        let metadata = ModelMetadata {
            source: ModelSource::Local {
                folder: PathBuf::from("/tmp"),
                filename: None,
            },
            filename: "test.gguf".to_string(),
            size_bytes: 100,
            resolve_time: Duration::from_secs(0),
            cache_hit: true,
        };
        let cloned = metadata.clone();
        assert_eq!(cloned.filename, "test.gguf");
        assert!(cloned.cache_hit);
    }

    #[test]
    fn test_model_source_hf_case_insensitive_extension() {
        // Upper case extension should also be valid
        let source = ModelSource::HuggingFace {
            repo: "org/repo".to_string(),
            filename: Some("model.GGUF".to_string()),
            folder: None,
        };
        assert!(source.validate().is_ok());

        let source = ModelSource::HuggingFace {
            repo: "org/repo".to_string(),
            filename: Some("model.Safetensors".to_string()),
            folder: None,
        };
        assert!(source.validate().is_ok());
    }
}
