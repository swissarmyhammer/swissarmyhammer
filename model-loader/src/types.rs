use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::Duration;

/// Known model file extensions for auto-detection.
///
/// Used by both local directory scanning and HuggingFace repository detection.
pub const MODEL_EXTENSIONS: &[&str] = &["gguf", "onnx", "mlmodel", "bin", "safetensors"];

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
#[derive(Debug, Clone)]
pub struct ModelMetadata {
    /// The source from which the model was resolved
    pub source: ModelSource,
    /// The filename of the model
    pub filename: String,
    /// Size of the model file in bytes
    pub size_bytes: u64,
    /// Time taken to resolve (download/locate) the model
    pub resolve_time: Duration,
    /// Whether this model was found in cache
    pub cache_hit: bool,
}

impl serde::Serialize for ModelMetadata {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ModelMetadata", 5)?;
        state.serialize_field("source", &self.source)?;
        state.serialize_field("filename", &self.filename)?;
        state.serialize_field("size_bytes", &self.size_bytes)?;
        state.serialize_field("resolve_time_secs", &self.resolve_time.as_secs_f64())?;
        state.serialize_field("cache_hit", &self.cache_hit)?;
        state.end()
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
            initial_delay_ms: new_config.initial_delay.as_millis() as u64,
            backoff_multiplier: new_config.backoff_multiplier,
            max_delay_ms: new_config.max_delay.as_millis() as u64,
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

                    if !full_path.is_file() {
                        return Err(crate::error::ModelError::InvalidConfig(format!(
                            "Path is not a file: {}",
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
}
