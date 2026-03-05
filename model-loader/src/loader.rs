use crate::error::ModelError;
use crate::huggingface::load_huggingface_model_with_path_and_folder;
use crate::types::{ModelConfig, ModelMetadata, ModelSource, ResolvedModel, RetryConfig, MODEL_EXTENSIONS};
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
pub struct ModelResolver;

impl ModelResolver {
    /// Create a new ModelResolver
    pub fn new() -> Self {
        Self
    }

    /// Resolve a model from the specified configuration.
    ///
    /// Downloads from HuggingFace or locates on disk, returning the path
    /// and metadata. Does NOT load the model into any runtime.
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
        let (model_path, actual_filename) =
            load_huggingface_model_with_path_and_folder(repo, filename, folder, retry_config)
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
                    "Model file does not exist: {}",
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
                    "Cannot read directory {}: {}",
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
            if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if MODEL_EXTENSIONS.contains(&ext.as_str()) {
                    let filename = path.file_name().unwrap().to_string_lossy().to_lowercase();
                    if filename.contains("bf16") {
                        bf16_files.push(path);
                    } else {
                        model_files.push(path);
                    }
                }
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
            "No model files found in {}",
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
        use hf_hub::api::tokio::ApiBuilder;

        // Create HuggingFace API client
        let api = match ApiBuilder::new().build() {
            Ok(api) => api,
            Err(_) => return false,
        };

        let _repo_api = api.model(repo.to_string());

        // Determine the target filename
        let target_filename = if let Some(filename) = filename {
            filename.to_string()
        } else {
            String::new() // Will be handled below
        };

        // Try to get the file - this will only succeed if it's already in cache
        match std::env::var("HF_HOME")
            .or_else(|_| std::env::var("XDG_CACHE_HOME").map(|p| format!("{}/huggingface", p)))
        {
            Ok(cache_dir) => {
                let cache_path = std::path::PathBuf::from(cache_dir)
                    .join("hub")
                    .join(format!("models--{}", repo.replace('/', "--")))
                    .join("snapshots");

                self.check_cache_snapshots(&cache_path, &target_filename, folder)
                    .await
            }
            Err(_) => {
                if let Some(home) = dirs::home_dir() {
                    let default_cache = home
                        .join(".cache")
                        .join("huggingface")
                        .join("hub")
                        .join(format!("models--{}", repo.replace('/', "--")))
                        .join("snapshots");

                    self.check_cache_snapshots(&default_cache, &target_filename, folder)
                        .await
                } else {
                    false
                }
            }
        }
    }

    /// Check snapshot directories for cached model files
    async fn check_cache_snapshots(
        &self,
        cache_path: &Path,
        target_filename: &str,
        folder: Option<&str>,
    ) -> bool {
        if let Ok(mut entries) = tokio::fs::read_dir(cache_path).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                    if target_filename.is_empty() {
                        // Auto-detection case - check for any model files
                        let search_dir = if let Some(folder_name) = folder {
                            entry.path().join(folder_name)
                        } else {
                            entry.path()
                        };

                        if let Ok(mut files) = tokio::fs::read_dir(&search_dir).await {
                            while let Ok(Some(file_entry)) = files.next_entry().await {
                                if let Some(ext) = file_entry.path().extension() {
                                    let ext_str = ext.to_string_lossy().to_lowercase();
                                    if MODEL_EXTENSIONS.contains(&ext_str.as_str()) {
                                        return true;
                                    }
                                }
                            }
                        }
                    } else {
                        // Specific filename case
                        let file_path = if let Some(folder_name) = folder {
                            entry.path().join(folder_name).join(target_filename)
                        } else {
                            entry.path().join(target_filename)
                        };

                        if file_path.exists() {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

impl Default for ModelResolver {
    fn default() -> Self {
        Self::new()
    }
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
}
