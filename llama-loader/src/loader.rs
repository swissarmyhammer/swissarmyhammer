use crate::error::ModelError;
use crate::huggingface::load_huggingface_model_with_path_and_folder;
use crate::types::{LoadedModel, ModelConfig, ModelMetadata, ModelSource, RetryConfig};
use llama_cpp_2::{
    llama_backend::LlamaBackend,
    model::{params::LlamaModelParams, LlamaModel},
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

/// Creates default model parameters optimized for Metal GPU offloading
///
/// This function configures LlamaModelParams with settings that enable
/// automatic GPU layer offloading and memory locking for optimal performance.
///
/// Configuration:
/// - `n_gpu_layers = i32::MAX`: Request all available layers be offloaded to GPU
/// - `use_mlock = true`: Lock model in RAM to prevent swapping for better performance
pub fn default_model_params() -> LlamaModelParams {
    LlamaModelParams::default()
        .with_n_gpu_layers(i32::MAX as u32)
        .with_use_mlock(true)
}

/// Manages loading of LLAMA models from various sources
pub struct ModelLoader {
    backend: Arc<LlamaBackend>,
    retry_config: RetryConfig,
}

impl ModelLoader {
    /// Create a new ModelLoader with the given backend
    pub fn new(backend: Arc<LlamaBackend>) -> Self {
        Self {
            backend,
            retry_config: RetryConfig::default(),
        }
    }

    /// Create a new ModelLoader with custom retry config
    pub fn new_with_config(backend: Arc<LlamaBackend>, retry_config: RetryConfig) -> Self {
        Self {
            backend,
            retry_config,
        }
    }

    /// Load a model from the specified configuration
    pub async fn load_model(&self, config: &ModelConfig) -> Result<LoadedModel, ModelError> {
        config.validate()?;

        let _start_time = Instant::now();
        info!("Loading model from config: {:?}", config.source);

        match &config.source {
            ModelSource::HuggingFace {
                repo,
                filename,
                folder,
            } => {
                self.load_huggingface_model_direct(
                    repo,
                    filename.as_deref(),
                    folder.as_deref(),
                    &config.retry_config,
                )
                .await
            }
            ModelSource::Local { folder, filename } => {
                self.load_local_model(folder, filename.as_deref()).await
            }
        }
    }

    /// Load HuggingFace model directly using hf-hub caching
    async fn load_huggingface_model_direct(
        &self,
        repo: &str,
        filename: Option<&str>,
        folder: Option<&str>,
        retry_config: &RetryConfig,
    ) -> Result<LoadedModel, ModelError> {
        let start_time = Instant::now();
        info!("Loading HuggingFace model: {}", repo);

        // Check if file is already cached by attempting to get it without download
        let cache_hit = self.check_hf_cache_exists(repo, filename, folder).await;

        if cache_hit {
            info!("Model found in HuggingFace cache, loading from cache");
        } else {
            info!("Model not in cache, downloading from HuggingFace");
        }

        // Load from HuggingFace (hf-hub handles all caching internally)
        let (model_path, actual_filename) =
            load_huggingface_model_with_path_and_folder(repo, filename, folder, retry_config)
                .await?;

        // Get file metadata
        let file_metadata = tokio::fs::metadata(&model_path).await?;
        let size_bytes = file_metadata.len();

        // Load the model using llama-cpp-2 directly from hf-hub's cached path
        let model_params = default_model_params();
        let model =
            LlamaModel::load_from_file(&self.backend, &model_path, &model_params).map_err(|e| {
                ModelError::LoadingFailed(format!(
                    "Failed to load model from {}: {}",
                    model_path.display(),
                    e
                ))
            })?;

        let load_time = start_time.elapsed();
        let context_size = model.n_ctx_train() as usize;
        let metadata = ModelMetadata {
            source: ModelSource::HuggingFace {
                repo: repo.to_string(),
                filename: Some(actual_filename.clone()),
                folder: folder.map(|s| s.to_string()),
            },
            filename: actual_filename,
            size_bytes,
            load_time,
            cache_hit,
            context_size,
        };

        Ok(LoadedModel {
            model,
            path: model_path,
            metadata,
        })
    }

    /// Load a model from HuggingFace (deprecated - use load_model with ModelConfig instead)
    pub async fn load_huggingface_model(
        &self,
        repo: &str,
        filename: Option<&str>,
        retry_config: &RetryConfig,
    ) -> Result<LoadedModel, ModelError> {
        self.load_huggingface_model_direct(repo, filename, None, retry_config)
            .await
    }

    /// Load a model from HuggingFace using the loader's default retry config
    pub async fn load_huggingface_model_with_defaults(
        &self,
        repo: &str,
        filename: Option<&str>,
    ) -> Result<LoadedModel, ModelError> {
        // Clone the retry config to avoid borrow conflicts
        let retry_config = self.retry_config.clone();
        self.load_huggingface_model_direct(repo, filename, None, &retry_config)
            .await
    }

    /// Load a model from local filesystem
    pub async fn load_local_model(
        &self,
        folder: &Path,
        filename: Option<&str>,
    ) -> Result<LoadedModel, ModelError> {
        let start_time = Instant::now();
        info!("Loading local model from folder: {:?}", folder);

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
            // Auto-detect with BF16 preference
            self.auto_detect_model_file(folder).await?
        };

        info!("Loading model from path: {:?}", model_path);

        // Get file metadata for proper size tracking
        let file_metadata = tokio::fs::metadata(&model_path).await?;
        let size_bytes = file_metadata.len();

        let model_params = default_model_params();
        let model =
            LlamaModel::load_from_file(&self.backend, &model_path, &model_params).map_err(|e| {
                ModelError::LoadingFailed(format!(
                    "Failed to load model from {}: {}",
                    model_path.display(),
                    e
                ))
            })?;

        let load_time = start_time.elapsed();
        let filename_str = model_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let context_size = model.n_ctx_train() as usize;
        let metadata = ModelMetadata {
            source: ModelSource::Local {
                folder: folder.to_path_buf(),
                filename: Some(filename_str.clone()),
            },
            filename: filename_str,
            size_bytes,
            load_time,
            cache_hit: false, // Local files don't use cache
            context_size,
        };

        Ok(LoadedModel {
            model,
            path: model_path,
            metadata,
        })
    }

    /// Auto-detect model file in local directory with BF16 preference
    async fn auto_detect_model_file(&self, folder: &Path) -> Result<PathBuf, ModelError> {
        let mut gguf_files = Vec::new();
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
                if extension == "gguf" {
                    let filename = path.file_name().unwrap().to_string_lossy().to_lowercase();
                    if filename.contains("bf16") {
                        bf16_files.push(path);
                    } else {
                        gguf_files.push(path);
                    }
                }
            }
        }

        // Prioritize BF16 files
        if !bf16_files.is_empty() {
            info!("Found BF16 model file: {:?}", bf16_files[0]);
            return Ok(bf16_files[0].clone());
        }

        // Fallback to first GGUF file
        if !gguf_files.is_empty() {
            info!("Found GGUF model file: {:?}", gguf_files[0]);
            return Ok(gguf_files[0].clone());
        }

        Err(ModelError::NotFound(format!(
            "No .gguf model files found in {}",
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

        // Determine the target filename (same logic as in huggingface.rs)
        let target_filename = if let Some(filename) = filename {
            filename.to_string()
        } else {
            // For cache check with auto-detection, check for any GGUF files
            // We'll scan the cache directory for any .gguf files
            // This is a simplified approach - in real usage, auto-detection would pick the best file
            String::new() // Will be handled below
        };

        // Try to get the file - this will only succeed if it's already in cache
        // We do this by trying to access the cached path directly
        match std::env::var("HF_HOME")
            .or_else(|_| std::env::var("XDG_CACHE_HOME").map(|p| format!("{}/huggingface", p)))
        {
            Ok(cache_dir) => {
                // Construct the expected cache path
                let cache_path = std::path::PathBuf::from(cache_dir)
                    .join("hub")
                    .join(format!("models--{}", repo.replace('/', "--")))
                    .join("snapshots");

                // Check if any snapshot directory contains the target file or any GGUF files
                if let Ok(mut entries) = tokio::fs::read_dir(&cache_path).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                            if target_filename.is_empty() {
                                // Auto-detection case - check for any .gguf files
                                let search_dir = if let Some(folder_name) = folder {
                                    entry.path().join(folder_name)
                                } else {
                                    entry.path()
                                };

                                if let Ok(mut files) = tokio::fs::read_dir(&search_dir).await {
                                    while let Ok(Some(file_entry)) = files.next_entry().await {
                                        if let Some(ext) = file_entry.path().extension() {
                                            if ext == "gguf" {
                                                return true;
                                            }
                                        }
                                    }
                                }
                            } else {
                                // Specific filename case
                                let file_path = if let Some(folder_name) = folder {
                                    entry.path().join(folder_name).join(&target_filename)
                                } else {
                                    entry.path().join(&target_filename)
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
            Err(_) => {
                // Use default cache location
                if let Some(home) = dirs::home_dir() {
                    let default_cache = home
                        .join(".cache")
                        .join("huggingface")
                        .join("hub")
                        .join(format!("models--{}", repo.replace('/', "--")))
                        .join("snapshots");

                    if let Ok(mut entries) = tokio::fs::read_dir(&default_cache).await {
                        while let Ok(Some(entry)) = entries.next_entry().await {
                            if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                                if target_filename.is_empty() {
                                    // Auto-detection case - check for any .gguf files
                                    let search_dir = if let Some(folder_name) = folder {
                                        entry.path().join(folder_name)
                                    } else {
                                        entry.path()
                                    };

                                    if let Ok(mut files) = tokio::fs::read_dir(&search_dir).await {
                                        while let Ok(Some(file_entry)) = files.next_entry().await {
                                            if let Some(ext) = file_entry.path().extension() {
                                                if ext == "gguf" {
                                                    return true;
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    // Specific filename case
                                    let file_path = if let Some(folder_name) = folder {
                                        entry.path().join(folder_name).join(&target_filename)
                                    } else {
                                        entry.path().join(&target_filename)
                                    };

                                    if file_path.exists() {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_model_params_configures_gpu_layers() {
        let params = default_model_params();
        // i32::MAX tells llama.cpp to offload all available layers to GPU
        assert_eq!(
            params.n_gpu_layers(),
            i32::MAX,
            "n_gpu_layers should be i32::MAX to offload all layers"
        );
    }

    #[test]
    fn test_default_model_params_enables_mlock() {
        let params = default_model_params();
        assert!(
            params.use_mlock(),
            "use_mlock should be true for better performance"
        );
    }

    #[test]
    fn test_model_loader_creation() {
        // We can't create a real LlamaBackend in unit tests
        // This test just verifies the structure compiles correctly
        // If this test runs, the struct definition is valid
    }
}
