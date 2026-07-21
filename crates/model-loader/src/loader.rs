use crate::error::ModelError;
use crate::huggingface::load_huggingface_model_with_path_and_folder;
use crate::observer::DownloadObserver;
use crate::types::{
    ModelConfig, ModelMetadata, ModelSource, ResolvedModel, RetryConfig, MODEL_EXTENSIONS,
};
use std::path::{Path, PathBuf};
use std::time::Instant;
use swissarmyhammer_common::Pretty;
use tracing::info;

/// Resolves model sources to local file paths.
///
/// This is a runtime-agnostic resolver: it locates model files (downloading
/// from HuggingFace if needed, or finding them locally) and returns a
/// [`ResolvedModel`] with the file path and metadata. Consumers (llama-agent,
/// ane-embedding, etc.) then load the file into their own backend.
#[derive(Clone, Default)]
pub struct ModelResolver {
    /// Optional progress callback forwarded to every download this resolver
    /// performs. `None` (the default) is byte-identical to the pre-observer
    /// behavior.
    download_observer: Option<DownloadObserver>,
}

impl std::fmt::Debug for ModelResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelResolver")
            .field("download_observer", &self.download_observer.is_some())
            .finish()
    }
}

impl ModelResolver {
    /// Create a new ModelResolver
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach a download progress observer (builder style).
    ///
    /// The observer receives a [`crate::observer::DownloadEvent`] for every
    /// file this resolver downloads.
    pub fn with_download_observer(mut self, observer: DownloadObserver) -> Self {
        self.download_observer = Some(observer);
        self
    }

    /// Resolve a model from the specified configuration.
    ///
    /// Downloads from HuggingFace or locates on disk, returning the path
    /// and metadata. Does NOT load the model into any runtime.
    ///
    /// # Errors
    ///
    /// * [`ModelError::InvalidConfig`] — `config` fails validation
    /// * [`ModelError::NotFound`] — a local model file does not exist, no
    ///   model file could be auto-detected, or the HuggingFace resource is
    ///   missing
    /// * [`ModelError::Network`] / [`ModelError::LoadingFailed`] — the
    ///   HuggingFace download failed (client construction, exhausted retries)
    /// * [`ModelError::Io`] — reading resolved file metadata failed
    pub async fn resolve(&self, config: &ModelConfig) -> Result<ResolvedModel, ModelError> {
        config.validate()?;

        info!("Resolving model from config: {}", Pretty(&config.source));

        match &config.source {
            ModelSource::HuggingFace {
                repo,
                filename,
                folder,
            } => {
                self.resolve_huggingface(
                    repo,
                    filename.as_deref(),
                    folder.as_deref(),
                    &config.retry_config,
                )
                .await
            }
            ModelSource::Local { folder, filename } => {
                self.resolve_local(folder, filename.as_deref()).await
            }
        }
    }

    /// Resolve a HuggingFace model to a local path
    async fn resolve_huggingface(
        &self,
        repo: &str,
        filename: Option<&str>,
        folder: Option<&str>,
        retry_config: &RetryConfig,
    ) -> Result<ResolvedModel, ModelError> {
        let start_time = Instant::now();
        info!("Resolving HuggingFace model: {}", repo);

        // Check if file is already cached
        let cache_hit = self.check_hf_cache_exists(repo, filename, folder).await;

        if cache_hit {
            info!("Model found in HuggingFace cache");
        } else {
            info!("Model not in cache, downloading from HuggingFace");
        }

        // Download/locate from HuggingFace (hf-hub handles caching internally)
        let (model_path, actual_filename) = load_huggingface_model_with_path_and_folder(
            repo,
            filename,
            folder,
            retry_config,
            self.download_observer.as_ref(),
        )
        .await?;

        // Get file metadata
        let file_metadata = tokio::fs::metadata(&model_path).await?;
        let size_bytes = file_metadata.len();

        let resolve_time = start_time.elapsed();

        let metadata = ModelMetadata {
            source: ModelSource::HuggingFace {
                repo: repo.to_string(),
                filename: Some(actual_filename.clone()),
                folder: folder.map(|s| s.to_string()),
            },
            filename: actual_filename,
            size_bytes,
            resolve_time,
            cache_hit,
        };

        Ok(ResolvedModel {
            path: model_path,
            metadata,
        })
    }

    /// Resolve a model from local filesystem
    async fn resolve_local(
        &self,
        folder: &Path,
        filename: Option<&str>,
    ) -> Result<ResolvedModel, ModelError> {
        let start_time = Instant::now();
        info!("Resolving local model from folder: {}", Pretty(&folder));

        let model_path = if let Some(filename) = filename {
            let path = folder.join(filename);
            if !path.exists() {
                return Err(ModelError::NotFound(format!(
                    "model file does not exist: {}",
                    path.display()
                )));
            }
            path
        } else {
            // Auto-detect model file
            self.auto_detect_model_file(folder).await?
        };

        info!("Resolved model path: {}", Pretty(&model_path));

        // Get file metadata for proper size tracking
        let file_metadata = tokio::fs::metadata(&model_path).await?;
        let size_bytes = file_metadata.len();

        let resolve_time = start_time.elapsed();
        let filename_str = model_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let metadata = ModelMetadata {
            source: ModelSource::Local {
                folder: folder.to_path_buf(),
                filename: Some(filename_str.clone()),
            },
            filename: filename_str,
            size_bytes,
            resolve_time,
            cache_hit: false, // Local files don't use cache
        };

        Ok(ResolvedModel {
            path: model_path,
            metadata,
        })
    }

    /// Auto-detect model file in local directory with BF16 preference
    async fn auto_detect_model_file(&self, folder: &Path) -> Result<PathBuf, ModelError> {
        let mut model_files = Vec::new();
        let mut bf16_files = Vec::new();

        // Read directory
        let mut entries = match tokio::fs::read_dir(folder).await {
            Ok(entries) => entries,
            Err(e) => {
                return Err(ModelError::LoadingFailed(format!(
                    "cannot read directory {}: {}",
                    folder.display(),
                    e
                )))
            }
        };

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| ModelError::LoadingFailed(e.to_string()))?
        {
            let path = entry.path();
            let Some(extension) = path.extension() else {
                continue;
            };
            let ext = extension.to_string_lossy().to_lowercase();
            if !MODEL_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }
            let filename = path.file_name().unwrap().to_string_lossy().to_lowercase();
            if filename.contains("bf16") {
                bf16_files.push(path);
            } else {
                model_files.push(path);
            }
        }

        // Prioritize BF16 files
        if !bf16_files.is_empty() {
            info!("Found BF16 model file: {}", Pretty(&bf16_files[0]));
            return Ok(bf16_files[0].clone());
        }

        // Fallback to first model file
        if !model_files.is_empty() {
            info!("Found model file: {}", Pretty(&model_files[0]));
            return Ok(model_files[0].clone());
        }

        Err(ModelError::NotFound(format!(
            "no model files found in {}",
            folder.display()
        )))
    }

    /// Check if a HuggingFace model file exists in the cache
    async fn check_hf_cache_exists(
        &self,
        repo: &str,
        filename: Option<&str>,
        folder: Option<&str>,
    ) -> bool {
        // Resolve the HF cache root the same way hf-hub does: HF_HOME, then
        // XDG_CACHE_HOME/huggingface, then ~/.cache/huggingface.
        let cache_root = match std::env::var("HF_HOME")
            .or_else(|_| std::env::var("XDG_CACHE_HOME").map(|p| format!("{}/huggingface", p)))
        {
            Ok(dir) => PathBuf::from(dir),
            Err(_) => match dirs::home_dir() {
                Some(home) => home.join(".cache").join("huggingface"),
                None => return false,
            },
        };

        // Empty target means "auto-detect any model file" in the snapshot scan.
        let target_filename = filename.unwrap_or_default();

        self.check_cache_snapshots(&hf_snapshots_dir(cache_root, repo), target_filename, folder)
            .await
    }

    /// Check snapshot directories for cached model files
    async fn check_cache_snapshots(
        &self,
        cache_path: &Path,
        target_filename: &str,
        folder: Option<&str>,
    ) -> bool {
        let Ok(mut entries) = tokio::fs::read_dir(cache_path).await else {
            return false;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            if !entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let found = if target_filename.is_empty() {
                snapshot_has_any_model_file(&entry.path(), folder).await
            } else {
                snapshot_has_named_file(&entry.path(), target_filename, folder)
            };
            if found {
                return true;
            }
        }
        false
    }
}

/// HuggingFace cache layout: `<root>/hub/models--<org>--<repo>/snapshots`.
/// Single definition of the directory-name components, used everywhere the
/// cache path is constructed.
const HF_CACHE_HUB_DIR: &str = "hub";
const HF_CACHE_MODELS_PREFIX: &str = "models--";
const HF_CACHE_SNAPSHOTS_DIR: &str = "snapshots";

/// Build the snapshots directory for `repo` under an HF cache root.
fn hf_snapshots_dir(cache_root: PathBuf, repo: &str) -> PathBuf {
    cache_root
        .join(HF_CACHE_HUB_DIR)
        .join(format!(
            "{HF_CACHE_MODELS_PREFIX}{}",
            repo.replace('/', "--")
        ))
        .join(HF_CACHE_SNAPSHOTS_DIR)
}

/// Auto-detection case: does the snapshot dir (or its `folder` subdir)
/// contain any file with a model extension?
async fn snapshot_has_any_model_file(snapshot: &Path, folder: Option<&str>) -> bool {
    let search_dir = match folder {
        Some(folder_name) => snapshot.join(folder_name),
        None => snapshot.to_path_buf(),
    };
    let Ok(mut files) = tokio::fs::read_dir(&search_dir).await else {
        return false;
    };
    while let Ok(Some(file_entry)) = files.next_entry().await {
        let path = file_entry.path();
        let Some(ext) = path.extension() else {
            continue;
        };
        let ext_str = ext.to_string_lossy().to_lowercase();
        if MODEL_EXTENSIONS.contains(&ext_str.as_str()) {
            return true;
        }
    }
    false
}

/// Specific-filename case: does the snapshot dir (or its `folder` subdir)
/// contain `target_filename`?
fn snapshot_has_named_file(snapshot: &Path, target_filename: &str, folder: Option<&str>) -> bool {
    let file_path = match folder {
        Some(folder_name) => snapshot.join(folder_name).join(target_filename),
        None => snapshot.join(target_filename),
    };
    file_path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_resolver_creation() {
        let _resolver = ModelResolver::new();
        let _resolver2 = ModelResolver::default();
    }

    #[test]
    fn test_model_extensions() {
        use crate::types::MODEL_EXTENSIONS;
        assert!(MODEL_EXTENSIONS.contains(&"gguf"));
        assert!(MODEL_EXTENSIONS.contains(&"onnx"));
        assert!(MODEL_EXTENSIONS.contains(&"safetensors"));
        assert!(MODEL_EXTENSIONS.contains(&"mlmodel"));
        assert!(MODEL_EXTENSIONS.contains(&"bin"));
    }

    #[test]
    fn test_model_resolver_default() {
        // Verify default() and new() both work without panicking
        let _resolver = ModelResolver::default();
        let _resolver2 = ModelResolver::new();
    }

    #[tokio::test]
    async fn test_resolve_local_with_filename() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let model_file = temp_dir.path().join("model.gguf");
        std::fs::write(&model_file, "fake model data").unwrap();

        let resolver = ModelResolver::new();
        let config = ModelConfig {
            source: ModelSource::Local {
                folder: temp_dir.path().to_path_buf(),
                filename: Some("model.gguf".to_string()),
            },
            retry_config: crate::types::RetryConfig::default(),
            debug: false,
        };

        let result = resolver.resolve(&config).await;
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.path, model_file);
        assert_eq!(resolved.metadata.filename, "model.gguf");
        assert!(!resolved.metadata.cache_hit);
        assert!(resolved.metadata.size_bytes > 0);
    }

    #[tokio::test]
    async fn test_resolve_local_file_not_found() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let resolver = ModelResolver::new();
        let config = ModelConfig {
            source: ModelSource::Local {
                folder: temp_dir.path().to_path_buf(),
                filename: Some("nonexistent.gguf".to_string()),
            },
            retry_config: crate::types::RetryConfig::default(),
            debug: false,
        };

        let result = resolver.resolve(&config).await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("does not exist"));
    }

    #[tokio::test]
    async fn test_resolve_local_auto_detect() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let model_file = temp_dir.path().join("mymodel.gguf");
        std::fs::write(&model_file, "fake model").unwrap();
        // Also create a non-model file
        std::fs::write(temp_dir.path().join("readme.txt"), "not a model").unwrap();

        let resolver = ModelResolver::new();
        let config = ModelConfig {
            source: ModelSource::Local {
                folder: temp_dir.path().to_path_buf(),
                filename: None,
            },
            retry_config: crate::types::RetryConfig::default(),
            debug: false,
        };

        let result = resolver.resolve(&config).await;
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert!(resolved.path.to_string_lossy().contains("mymodel.gguf"));
    }

    #[tokio::test]
    async fn test_resolve_local_auto_detect_bf16_preferred() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("model.gguf"), "regular").unwrap();
        std::fs::write(temp_dir.path().join("model-bf16.gguf"), "bf16").unwrap();

        let resolver = ModelResolver::new();
        let config = ModelConfig {
            source: ModelSource::Local {
                folder: temp_dir.path().to_path_buf(),
                filename: None,
            },
            retry_config: crate::types::RetryConfig::default(),
            debug: false,
        };

        let result = resolver.resolve(&config).await;
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert!(
            resolved.path.to_string_lossy().contains("bf16"),
            "BF16 model should be preferred"
        );
    }

    #[tokio::test]
    async fn test_resolve_local_auto_detect_no_model_files() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("readme.txt"), "not a model").unwrap();
        std::fs::write(temp_dir.path().join("config.json"), "{}").unwrap();

        let resolver = ModelResolver::new();
        let config = ModelConfig {
            source: ModelSource::Local {
                folder: temp_dir.path().to_path_buf(),
                filename: None,
            },
            retry_config: crate::types::RetryConfig::default(),
            debug: false,
        };

        let result = resolver.resolve(&config).await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("no model files found"));
    }

    #[tokio::test]
    async fn test_resolve_local_auto_detect_various_extensions() {
        for ext in MODEL_EXTENSIONS {
            let temp_dir = tempfile::TempDir::new().unwrap();
            let filename = format!("model.{}", ext);
            std::fs::write(temp_dir.path().join(&filename), "data").unwrap();

            let resolver = ModelResolver::new();
            let config = ModelConfig {
                source: ModelSource::Local {
                    folder: temp_dir.path().to_path_buf(),
                    filename: None,
                },
                retry_config: crate::types::RetryConfig::default(),
                debug: false,
            };

            let result = resolver.resolve(&config).await;
            assert!(result.is_ok(), "Should auto-detect .{} files", ext);
        }
    }

    #[tokio::test]
    async fn test_resolve_invalid_config() {
        let resolver = ModelResolver::new();
        let config = ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "".to_string(),
                filename: None,
                folder: None,
            },
            retry_config: crate::types::RetryConfig::default(),
            debug: false,
        };

        let result = resolver.resolve(&config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolve_local_metadata_source_is_local() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let model_file = temp_dir.path().join("model.onnx");
        std::fs::write(&model_file, "onnx data").unwrap();

        let resolver = ModelResolver::new();
        let config = ModelConfig {
            source: ModelSource::Local {
                folder: temp_dir.path().to_path_buf(),
                filename: Some("model.onnx".to_string()),
            },
            retry_config: crate::types::RetryConfig::default(),
            debug: false,
        };

        let resolved = resolver.resolve(&config).await.unwrap();
        match &resolved.metadata.source {
            ModelSource::Local { folder, filename } => {
                assert_eq!(folder, temp_dir.path());
                assert_eq!(filename.as_deref(), Some("model.onnx"));
            }
            _ => panic!("Expected Local source in metadata"),
        }
    }
}
