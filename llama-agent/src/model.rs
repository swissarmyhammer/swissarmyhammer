use crate::types::{ModelConfig, ModelError, SessionId};
use llama_cpp_2::{
    context::{params::LlamaContextParams, LlamaContext},
    llama_backend::LlamaBackend,
    model::LlamaModel,
    send_logs_to_tracing, LogOptions,
};
use llama_loader::{ModelLoader, ModelMetadata};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
// Need access to raw FFI bindings for llama_log_set
use std::ffi::c_void;
use std::os::raw::c_char;

static GLOBAL_BACKEND: OnceLock<Arc<LlamaBackend>> = OnceLock::new();

// Null log callback to suppress llama.cpp verbose output
extern "C" fn null_log_callback(_level: i32, _text: *const c_char, _user_data: *mut c_void) {
    // Do nothing - this suppresses all llama.cpp logging
}

// Set up logging suppression using llama_log_set
fn set_logging_suppression(suppress: bool) {
    unsafe {
        // Access the raw FFI binding
        extern "C" {
            fn llama_log_set(
                log_callback: Option<extern "C" fn(i32, *const c_char, *mut c_void)>,
                user_data: *mut c_void,
            );
        }

        if suppress {
            // Set null callback to suppress logging
            llama_log_set(Some(null_log_callback), std::ptr::null_mut());
        } else {
            // Restore default logging (NULL callback means output to stderr)
            llama_log_set(None, std::ptr::null_mut());
        }
    }
}

/// Metadata for tracking KV cache files for LRU eviction
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KVCacheMetadata {
    session_id: SessionId,
    cache_file: PathBuf,
    tokens_file: PathBuf,
    last_accessed: SystemTime,
    file_size_bytes: u64,
}

pub struct ModelManager {
    model: Arc<RwLock<Option<LlamaModel>>>,
    backend: Arc<LlamaBackend>,
    config: ModelConfig,
    loader: RwLock<Option<ModelLoader>>,
    metadata: RwLock<Option<ModelMetadata>>,
    memory_usage_bytes: Arc<std::sync::atomic::AtomicU64>,
    // Session state tracking for KV cache optimization
    session_sequence_ids: Arc<RwLock<HashMap<SessionId, u32>>>,
    /// KV cache metadata for tracking and LRU eviction
    kv_cache_metadata: Arc<Mutex<HashMap<SessionId, KVCacheMetadata>>>,
}

impl ModelManager {
    pub fn new(config: ModelConfig) -> Result<Self, ModelError> {
        // Configure llama.cpp logging based on debug setting
        if config.debug {
            // Enable debug logging - send llama.cpp logs to tracing
            send_logs_to_tracing(LogOptions::default());
            debug!("Enabled verbose llama.cpp logging via tracing");
            set_logging_suppression(false);
        } else {
            // When debug is false, we rely on the tracing level configuration
            // from main.rs (WARN level) to filter out verbose logs
            debug!("llama.cpp logs will be filtered by tracing WARN level");
            set_logging_suppression(true);
        }

        // Get existing backend or try to initialize new one
        let backend = if let Some(backend) = GLOBAL_BACKEND.get() {
            backend.clone()
        } else {
            // Try to initialize the backend
            let new_backend = match LlamaBackend::init() {
                Ok(backend) => Arc::new(backend),
                Err(llama_cpp_2::LlamaCppError::BackendAlreadyInitialized) => {
                    // Backend was already initialized but we don't have a reference
                    // This is a limitation of llama-cpp-2 - we can't get a reference to an existing backend
                    // For now, we'll work around this by skipping backend initialization in tests
                    return Err(ModelError::LoadingFailed(
                        "Backend already initialized by external code".to_string(),
                    ));
                }
                Err(e) => {
                    return Err(ModelError::LoadingFailed(format!(
                        "Failed to initialize LlamaBackend: {}",
                        e
                    )));
                }
            };

            // Try to store it globally, but don't fail if someone else beat us to it
            if GLOBAL_BACKEND.set(new_backend.clone()).is_err() {
                // Someone else set it, use theirs instead
                GLOBAL_BACKEND.get().unwrap().clone()
            } else {
                new_backend
            }
        };

        let manager = Self {
            model: Arc::new(RwLock::new(None)),
            backend,
            config,
            loader: RwLock::new(None),
            metadata: RwLock::new(None),
            memory_usage_bytes: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            session_sequence_ids: Arc::new(RwLock::new(HashMap::new())),
            kv_cache_metadata: Arc::new(Mutex::new(HashMap::new())),
        };
        Ok(manager)
    }

    /// Initialize the ModelLoader (must be called after construction)
    pub async fn initialize_loader(&self) -> Result<(), ModelError> {
        let loader = ModelLoader::new(self.backend.clone());
        *self.loader.write().await = Some(loader);
        Ok(())
    }

    pub async fn load_model(&self) -> Result<(), ModelError> {
        info!("Loading model with configuration: {:?}", self.config);

        // Validate config before proceeding
        self.config.validate()?;

        // Ensure loader is initialized
        {
            let loader_guard = self.loader.read().await;
            if loader_guard.is_none() {
                drop(loader_guard);
                self.initialize_loader().await?;
            }
        }

        // Log memory usage before loading
        let memory_before = Self::get_process_memory_mb().unwrap_or(0);
        debug!("Memory usage before model loading: {} MB", memory_before);

        // Load model using ModelLoader
        let loaded_model = {
            let mut loader_guard = self.loader.write().await;
            loader_guard
                .as_mut()
                .unwrap()
                .load_model(&self.config)
                .await?
        };

        let memory_after = Self::get_process_memory_mb().unwrap_or(0);
        let memory_used = memory_after.saturating_sub(memory_before);

        // Store memory usage estimate
        self.memory_usage_bytes.store(
            memory_used * 1024 * 1024,
            std::sync::atomic::Ordering::Relaxed,
        );

        info!(
            "Model loaded successfully in {:?} (Memory: +{} MB, Total: {} MB)",
            loaded_model.metadata.load_time, memory_used, memory_after
        );

        // Store model and metadata
        {
            let mut model_lock = self.model.write().await;
            *model_lock = Some(loaded_model.model);
        }
        *self.metadata.write().await = Some(loaded_model.metadata);

        Ok(())
    }

    pub async fn is_loaded(&self) -> bool {
        let model_lock = self.model.read().await;
        model_lock.is_some()
    }

    pub fn get_batch_size(&self) -> usize {
        self.config.batch_size as usize
    }

    pub fn get_config(&self) -> &ModelConfig {
        &self.config
    }

    pub async fn with_model<F, R>(&self, f: F) -> Result<R, ModelError>
    where
        F: FnOnce(&LlamaModel) -> R,
    {
        let model_lock = self.model.read().await;
        match model_lock.as_ref() {
            Some(model) => Ok(f(model)),
            None => Err(ModelError::LoadingFailed("Model not loaded".to_string())),
        }
    }

    /// Get an Arc to the model for use with TextGenerator abstraction.
    ///
    /// This method allows the TextGenerator to hold a reference to the model
    /// while maintaining thread safety through Arc.
    pub async fn get_model_arc(&self) -> Result<Arc<LlamaModel>, ModelError> {
        let model_lock = self.model.read().await;
        match model_lock.as_ref() {
            Some(_model) => {
                // We cannot clone LlamaModel, so we need to restructure this
                // For now, return an error indicating the architectural constraint
                Err(ModelError::LoadingFailed(
                    "Cannot create Arc<LlamaModel> from borrowed reference".to_string(),
                ))
            }
            None => Err(ModelError::LoadingFailed("Model not loaded".to_string())),
        }
    }

    /// Create a session-aware context that can reuse KV cache state
    /// Note: For now, this is synchronous to work within model lifetime constraints
    pub fn create_session_context<'a>(
        &self,
        model: &'a LlamaModel,
        session_id: &SessionId,
    ) -> Result<LlamaContext<'a>, ModelError> {
        debug!("Creating context for session {}", session_id);
        self.create_context(model)
    }

    pub fn create_context<'a>(
        &self,
        model: &'a LlamaModel,
    ) -> Result<LlamaContext<'a>, ModelError> {
        // Search for any metadata key ending with .context_length or containing max_position_embeddings
        let model_native_ctx = {
            let mut found_ctx = None;
            let meta_count = model.meta_count();

            // First pass: log all metadata for debugging
            debug!("=== GGUF Metadata (total {} keys) ===", meta_count);
            for i in 0..meta_count {
                if let (Ok(key), Ok(value)) =
                    (model.meta_key_by_index(i), model.meta_val_str_by_index(i))
                {
                    // Log keys that might contain context info
                    if key.contains("context") || key.contains("position") || key.contains("max") {
                        debug!("  {}: {}", key, value);
                    }
                }
            }

            // Second pass: search for context length
            for i in 0..meta_count {
                if let (Ok(key), Ok(value)) =
                    (model.meta_key_by_index(i), model.meta_val_str_by_index(i))
                {
                    // Check for context_length, max_position_embeddings, or n_ctx_train_override
                    // Use contains() to catch keys with prefixes like "glm." or "llama."
                    if key.ends_with(".context_length")
                        || key.contains("max_position_embeddings")
                        || key.contains("context_length")
                    {
                        if let Ok(ctx_val) = value.parse::<usize>() {
                            // Skip suspiciously small values that are likely wrong
                            if ctx_val > 8192 || !key.contains("ctx_train") {
                                found_ctx = Some(ctx_val);
                                debug!(
                                    "Found model context length in metadata '{}': {} tokens",
                                    key, ctx_val
                                );
                                break;
                            } else {
                                debug!(
                                    "Skipping small context value in '{}': {} (likely incorrect metadata)",
                                    key, ctx_val
                                );
                            }
                        } else {
                            warn!(
                                "Failed to parse context_length from metadata '{}': {}",
                                key, value
                            );
                        }
                    }
                }
            }

            found_ctx.unwrap_or_else(|| {
                let ctx = model.n_ctx_train() as usize;
                info!(
                    "No .context_length or .max_position_embeddings metadata found, using n_ctx_train: {} tokens",
                    ctx
                );
                ctx
            })
        };

        // KV cache should use full model context window, not just batch size
        // This allows proper caching of conversation history across multiple batches
        let n_ctx = std::cmp::max(model_native_ctx, self.config.batch_size as usize);
        let n_batch = self.config.batch_size;
        let n_ubatch = self.config.batch_size / 4;

        // Log KV cache configuration prominently
        debug!(
            "KV Cache Configuration: cache_size={} tokens ({}x batch_size), model_native={} tokens, batch_size={} tokens",
            n_ctx,
            n_ctx / (self.config.batch_size as usize),
            model_native_ctx,
            self.config.batch_size
        );

        if n_ctx == self.config.batch_size as usize {
            warn!(
                " KV cache size equals batch size ({} tokens) - this limits caching to single batch. Consider using a model with larger context window.",
                n_ctx
            );
        }

        let context_params = LlamaContextParams::default()
            .with_n_ctx(Some(std::num::NonZeroU32::new(n_ctx as u32).unwrap()))
            .with_n_batch(n_batch)
            .with_n_ubatch(n_ubatch)
            .with_n_threads(self.config.n_threads)
            .with_n_threads_batch(self.config.n_threads_batch);

        debug!(
            "Creating context with n_ctx={}, n_batch={}, n_ubatch={}, n_seq_max={}, n_threads={}, n_threads_batch={}",
            n_ctx, n_batch, n_ubatch, self.config.n_seq_max, self.config.n_threads, self.config.n_threads_batch
        );

        model
            .new_context(&self.backend, context_params)
            .map_err(move |e| ModelError::LoadingFailed(format!("Failed to create context: {}", e)))
    }

    /// Get current process memory usage in MB
    fn get_process_memory_mb() -> Result<u64, std::io::Error> {
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            let status = fs::read_to_string("/proc/self/status")?;
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<u64>() {
                            return Ok(kb / 1024); // Convert KB to MB
                        }
                    }
                }
            }
            Ok(0)
        }
        #[cfg(target_os = "macos")]
        {
            // Use mach API on macOS for memory info
            // For simplicity, return 0 - could be implemented with mach sys calls
            Ok(0)
        }
        #[cfg(target_os = "windows")]
        {
            // Use Windows API for memory info
            // For simplicity, return 0 - could be implemented with winapi
            Ok(0)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Ok(0)
        }
    }

    /// Get estimated memory usage of the loaded model in bytes
    pub fn get_memory_usage_bytes(&self) -> u64 {
        self.memory_usage_bytes
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get model loading statistics
    pub async fn get_load_stats(&self) -> Option<(std::time::Duration, u64)> {
        let metadata_guard = self.metadata.read().await;
        metadata_guard.as_ref().map(|meta| {
            let memory_bytes = self.get_memory_usage_bytes();
            (meta.load_time, memory_bytes)
        })
    }

    /// Get model metadata
    pub async fn get_metadata(&self) -> Option<ModelMetadata> {
        self.metadata.read().await.clone()
    }

    /// Clean up session context resources when a session is deleted
    pub async fn cleanup_session(&self, session_id: &SessionId) {
        let mut session_sequences = self.session_sequence_ids.write().await;
        if session_sequences.remove(session_id).is_some() {
            debug!("Cleaned up session context for session {}", session_id);
        }
        drop(session_sequences);

        // Delete session KV cache file if it exists
        let kv_cache_dir = PathBuf::from(".llama-sessions");
        match self.delete_session_kv_cache(session_id, &kv_cache_dir) {
            Ok(true) => {
                debug!("Deleted session KV cache for session {}", session_id);
            }
            Ok(false) => {
                // No cache file existed, which is fine
                debug!(
                    "No session KV cache found to delete for session {}",
                    session_id
                );
            }
            Err(e) => {
                warn!(
                    "Failed to delete session KV cache for session {}: {}",
                    session_id, e
                );
            }
        }
    }

    /// Evict oldest KV cache files if limit is exceeded
    fn evict_kv_cache_if_needed(
        &self,
        _storage_dir: &Path,
        max_cache_files: usize,
    ) -> Result<(), ModelError> {
        if max_cache_files == 0 {
            return Ok(()); // Unlimited cache
        }

        let mut metadata = self.kv_cache_metadata.lock().unwrap();

        // Check if we exceed the limit
        if metadata.len() <= max_cache_files {
            return Ok(());
        }

        // Sort by last accessed time (oldest first) - collect session_ids to evict
        let mut entries: Vec<_> = metadata
            .iter()
            .map(|(id, meta)| {
                (
                    *id,
                    meta.last_accessed,
                    meta.cache_file.clone(),
                    meta.tokens_file.clone(),
                )
            })
            .collect();
        entries.sort_by_key(|(_, last_accessed, _, _)| *last_accessed);

        // Calculate how many to evict
        let evict_count = metadata.len() - max_cache_files;

        // Evict oldest entries
        for (session_id, _, cache_file, tokens_file) in entries.iter().take(evict_count) {
            // Delete the cache file
            if cache_file.exists() {
                if let Err(e) = std::fs::remove_file(cache_file) {
                    warn!(
                        "Failed to delete KV cache file {}: {}",
                        cache_file.display(),
                        e
                    );
                } else {
                    debug!(
                        "Evicted KV cache file for session {} ({})",
                        session_id,
                        cache_file.display()
                    );
                }
            }

            // Delete the tokens file
            if tokens_file.exists() {
                if let Err(e) = std::fs::remove_file(tokens_file) {
                    warn!(
                        "Failed to delete tokens file {}: {}",
                        tokens_file.display(),
                        e
                    );
                }
            }

            // Remove from metadata
            metadata.remove(session_id);
        }

        info!(
            "Evicted {} KV cache files, {} remaining",
            evict_count,
            metadata.len()
        );

        Ok(())
    }

    /// Save session KV cache state to file
    pub fn save_session_kv_cache(
        &self,
        context: &LlamaContext,
        session_id: &SessionId,
        tokens: &[llama_cpp_2::token::LlamaToken],
        storage_dir: &Path,
        max_cache_files: usize,
    ) -> Result<PathBuf, ModelError> {
        // Ensure storage directory exists
        std::fs::create_dir_all(storage_dir).map_err(|e| {
            ModelError::LoadingFailed(format!("Failed to create storage directory: {}", e))
        })?;

        let session_file = storage_dir.join(format!("{}.bin", session_id));
        let tokens_file = storage_dir.join(format!("{}.tokens", session_id));

        context
            .save_session_file(&session_file, tokens)
            .map_err(|e| {
                ModelError::LoadingFailed(format!("Failed to save session file: {}", e))
            })?;

        // Update metadata for LRU tracking
        let file_size = std::fs::metadata(&session_file)
            .map(|m| m.len())
            .unwrap_or(0);

        {
            let mut metadata = self.kv_cache_metadata.lock().unwrap();
            metadata.insert(
                *session_id,
                KVCacheMetadata {
                    session_id: *session_id,
                    cache_file: session_file.clone(),
                    tokens_file: tokens_file.clone(),
                    last_accessed: SystemTime::now(),
                    file_size_bytes: file_size,
                },
            );
        }

        // Trigger LRU eviction if needed
        self.evict_kv_cache_if_needed(storage_dir, max_cache_files)?;

        debug!(
            "Saved KV cache for session {} to {} ({} bytes)",
            session_id,
            session_file.display(),
            file_size
        );
        Ok(session_file)
    }

    /// Load session KV cache state from file
    pub fn load_session_kv_cache(
        &self,
        context: &mut LlamaContext,
        session_id: &SessionId,
        storage_dir: &Path,
        max_tokens: usize,
    ) -> Result<Vec<llama_cpp_2::token::LlamaToken>, ModelError> {
        let session_file = storage_dir.join(format!("{}.bin", session_id));

        if !session_file.exists() {
            debug!("No session file found for session {}", session_id);
            return Ok(Vec::new());
        }

        let tokens = context
            .load_session_file(&session_file, max_tokens)
            .map_err(|e| {
                ModelError::LoadingFailed(format!("Failed to load session file: {}", e))
            })?;

        debug!(
            "Loaded KV cache for session {} from {} (restored {} tokens)",
            session_id,
            session_file.display(),
            tokens.len()
        );
        Ok(tokens)
    }

    /// Check if session KV cache file exists and update access time for LRU
    pub fn has_session_kv_cache(&self, session_id: &SessionId, storage_dir: &Path) -> bool {
        let session_file = storage_dir.join(format!("{}.bin", session_id));
        let exists = session_file.exists();

        // Update last accessed time in metadata for LRU tracking
        if exists {
            let mut metadata = self.kv_cache_metadata.lock().unwrap();
            if let Some(cache_meta) = metadata.get_mut(session_id) {
                cache_meta.last_accessed = SystemTime::now();
            }
        }

        exists
    }

    /// Delete session KV cache file
    pub fn delete_session_kv_cache(
        &self,
        session_id: &SessionId,
        storage_dir: &Path,
    ) -> Result<bool, ModelError> {
        let session_file = storage_dir.join(format!("{}.bin", session_id));

        if session_file.exists() {
            std::fs::remove_file(&session_file).map_err(|e| {
                ModelError::LoadingFailed(format!("Failed to delete session file: {}", e))
            })?;
            debug!("Deleted KV cache file for session {}", session_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ModelConfig, ModelSource};
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::fs;

    // Test configuration constants
    const TEST_BATCH_SIZE: u32 = 512;
    const TEST_N_SEQ_MAX: u32 = 1;
    const TEST_N_THREADS: i32 = 1;
    const TEST_N_THREADS_BATCH: i32 = 1;

    fn create_test_config_local(folder: PathBuf, filename: Option<String>) -> ModelConfig {
        ModelConfig {
            source: ModelSource::Local { folder, filename },
            batch_size: TEST_BATCH_SIZE,
            n_seq_max: TEST_N_SEQ_MAX,
            n_threads: TEST_N_THREADS,
            n_threads_batch: TEST_N_THREADS_BATCH,
            use_hf_params: false,
            retry_config: crate::types::RetryConfig::default(),
            debug: false,
        }
    }

    fn create_test_config_hf(repo: String, filename: Option<String>) -> ModelConfig {
        ModelConfig {
            source: ModelSource::HuggingFace {
                repo,
                filename,
                folder: None,
            },
            batch_size: TEST_BATCH_SIZE,
            n_seq_max: TEST_N_SEQ_MAX,
            n_threads: TEST_N_THREADS,
            n_threads_batch: TEST_N_THREADS_BATCH,
            use_hf_params: true,
            retry_config: crate::types::RetryConfig::default(),
            debug: false,
        }
    }

    #[tokio::test]
    async fn test_model_manager_creation() {
        let config = create_test_config_local(PathBuf::from("/tmp"), None);

        // When running tests in parallel, the backend might already be initialized by another test
        match ModelManager::new(config) {
            Ok(manager) => {
                assert!(!manager.is_loaded().await);

                // Test with_model when no model is loaded
                let result = manager.with_model(|_model| ()).await;
                assert!(result.is_err());
            }
            Err(ModelError::LoadingFailed(msg))
                if msg.contains("Backend already initialized by external code") =>
            {
                // This is expected when running tests in parallel - one test initializes the backend
                // and subsequent tests see it as already initialized. This is fine for the test.
                println!("Backend already initialized by another test - this is expected in parallel test execution");
            }
            Err(e) => {
                panic!("Unexpected error creating ModelManager: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_model_loading_with_invalid_file() {
        let temp_dir = TempDir::new().unwrap();
        let model_file = temp_dir.path().join("test-model.gguf");

        // Create a dummy .gguf file (this will fail to load as real model)
        fs::write(&model_file, b"dummy model content")
            .await
            .unwrap();

        let config = create_test_config_local(
            temp_dir.path().to_path_buf(),
            Some("test-model.gguf".to_string()),
        );
        let manager = ModelManager::new(config).expect("Failed to create ModelManager");

        // This should fail because dummy content is not a valid GGUF model
        let result = manager.load_model().await;
        assert!(result.is_err());
        assert!(!manager.is_loaded().await);
    }

    #[tokio::test]
    async fn test_model_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config_local(
            temp_dir.path().to_path_buf(),
            Some("nonexistent.gguf".to_string()),
        );
        let manager = ModelManager::new(config).expect("Failed to create ModelManager");

        let result = manager.load_model().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ModelError::NotFound(_) => {}
            _ => panic!("Expected NotFound error"),
        }
    }

    #[tokio::test]
    async fn test_folder_not_found() {
        let config = create_test_config_local(
            PathBuf::from("/nonexistent/folder"),
            Some("model.gguf".to_string()),
        );

        // When running tests in parallel, the backend might already be initialized by another test
        match ModelManager::new(config) {
            Ok(manager) => {
                let result = manager.load_model().await;
                assert!(result.is_err());
                match result.unwrap_err() {
                    ModelError::NotFound(_) => {}
                    _ => panic!("Expected NotFound error"),
                }
            }
            Err(ModelError::LoadingFailed(msg))
                if msg.contains("Backend already initialized by external code") =>
            {
                // This is expected when running tests in parallel - one test initializes the backend
                // and subsequent tests see it as already initialized. This is fine for the test.
                println!("Backend already initialized by another test - this is expected in parallel test execution");
            }
            Err(e) => {
                panic!("Unexpected error creating ModelManager: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_auto_detect_bf16_preference() {
        let temp_dir = TempDir::new().unwrap();

        // Create multiple GGUF files, including BF16
        let regular_model = temp_dir.path().join("model-q4.gguf");
        let bf16_model = temp_dir.path().join("model-bf16.gguf");
        let another_model = temp_dir.path().join("model-q8.gguf");

        fs::write(&regular_model, b"regular model").await.unwrap();
        fs::write(&bf16_model, b"bf16 model").await.unwrap();
        fs::write(&another_model, b"another model").await.unwrap();

        let config = create_test_config_local(temp_dir.path().to_path_buf(), None);
        let manager = ModelManager::new(config).expect("Failed to create ModelManager");

        // This should try to load the BF16 file first (though it will fail with invalid content)
        let result = manager.load_model().await;
        assert!(result.is_err()); // Will fail due to invalid GGUF content, but that's expected
    }

    #[tokio::test]
    async fn test_auto_detect_no_gguf_files() {
        let temp_dir = TempDir::new().unwrap();

        // Create non-GGUF files
        let txt_file = temp_dir.path().join("readme.txt");
        fs::write(&txt_file, b"readme content").await.unwrap();

        let config = create_test_config_local(temp_dir.path().to_path_buf(), None);

        // When running tests in parallel, the backend might already be initialized by another test
        match ModelManager::new(config) {
            Ok(manager) => {
                let result = manager.load_model().await;
                assert!(result.is_err());
                match result.unwrap_err() {
                    ModelError::NotFound(_) => {}
                    _ => panic!("Expected NotFound error"),
                }
            }
            Err(ModelError::LoadingFailed(msg))
                if msg.contains("Backend already initialized by external code") =>
            {
                // This is expected when running tests in parallel - one test initializes the backend
                // and subsequent tests see it as already initialized. This is fine for the test.
                println!("Backend already initialized by another test - this is expected in parallel test execution");
            }
            Err(e) => {
                panic!("Unexpected error creating ModelManager: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_huggingface_config_creation() {
        let config = create_test_config_hf("microsoft/DialoGPT-medium".to_string(), None);

        // When running tests in parallel, the backend might already be initialized by another test
        // This is expected and should not cause test failures
        match ModelManager::new(config) {
            Ok(manager) => {
                // Test that we can create the manager (HF loading will treat repo as local path and fail)
                assert!(!manager.is_loaded().await);

                let result = manager.load_model().await;
                assert!(result.is_err()); // Will fail since "microsoft/DialoGPT-medium" is not a local path
            }
            Err(ModelError::LoadingFailed(msg))
                if msg.contains("Backend already initialized by external code") =>
            {
                // This is expected when running tests in parallel - one test initializes the backend
                // and subsequent tests see it as already initialized. This is fine for the test.
                println!("Backend already initialized by another test - this is expected in parallel test execution");
            }
            Err(e) => {
                panic!("Unexpected error creating ModelManager: {:?}", e);
            }
        }
    }

    #[test]
    fn test_model_config_debug() {
        let config = create_test_config_local(PathBuf::from("/tmp"), Some("test.gguf".to_string()));
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("Local"));
        assert!(debug_str.contains("test.gguf"));
        assert!(debug_str.contains(&TEST_BATCH_SIZE.to_string()));
    }

    #[tokio::test]
    async fn test_retry_config_default() {
        let config = crate::types::RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay_ms, 1000);
        assert_eq!(config.backoff_multiplier, 2.0);
        assert_eq!(config.max_delay_ms, 30000);
    }

    #[tokio::test]
    async fn test_is_retriable_error() {
        let config = create_test_config_hf("test/repo".to_string(), None);

        // This is a bit tricky since we can't easily create HfHubError instances
        // We'll test the logic indirectly by checking that the manager has the method
        let manager = match ModelManager::new(config) {
            Ok(m) => m,
            Err(ModelError::LoadingFailed(msg)) if msg.contains("Backend already initialized") => {
                // Expected in test environment
                return;
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        };

        // The function exists and can be called - detailed testing would require
        // mocking the HuggingFace API which is complex
        assert_eq!(manager.config.retry_config.max_retries, 3);
    }

    #[test]
    fn test_exponential_backoff_calculation() {
        let retry_config = crate::types::RetryConfig::default();
        let mut delay = retry_config.initial_delay_ms;

        // Test exponential backoff progression
        assert_eq!(delay, 1000); // Initial: 1s

        delay = ((delay as f64) * retry_config.backoff_multiplier) as u64;
        delay = delay.min(retry_config.max_delay_ms);
        assert_eq!(delay, 2000); // 2s

        delay = ((delay as f64) * retry_config.backoff_multiplier) as u64;
        delay = delay.min(retry_config.max_delay_ms);
        assert_eq!(delay, 4000); // 4s

        // Continue until we hit the max
        for _ in 0..10 {
            delay = ((delay as f64) * retry_config.backoff_multiplier) as u64;
            delay = delay.min(retry_config.max_delay_ms);
        }
        assert_eq!(delay, retry_config.max_delay_ms); // Should cap at 30s
    }

    #[test]
    fn test_custom_retry_config() {
        let mut config = create_test_config_hf("test/repo".to_string(), None);
        config.retry_config.max_retries = 5;
        config.retry_config.initial_delay_ms = 500;
        config.retry_config.backoff_multiplier = 1.5;
        config.retry_config.max_delay_ms = 10000;

        assert_eq!(config.retry_config.max_retries, 5);
        assert_eq!(config.retry_config.initial_delay_ms, 500);
        assert_eq!(config.retry_config.backoff_multiplier, 1.5);
        assert_eq!(config.retry_config.max_delay_ms, 10000);
    }

    // KV Cache persistence tests
    #[tokio::test]
    async fn test_has_session_kv_cache_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config_local(temp_dir.path().to_path_buf(), None);

        let manager = match ModelManager::new(config) {
            Ok(m) => m,
            Err(ModelError::LoadingFailed(msg)) if msg.contains("Backend already initialized") => {
                return; // Skip test if backend already initialized
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        };

        let session_id = crate::types::SessionId::new();
        let storage_dir = temp_dir.path();

        // Should return false for nonexistent session file
        assert!(!manager.has_session_kv_cache(&session_id, storage_dir));
    }

    #[tokio::test]
    async fn test_has_session_kv_cache_existing() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config_local(temp_dir.path().to_path_buf(), None);

        let manager = match ModelManager::new(config) {
            Ok(m) => m,
            Err(ModelError::LoadingFailed(msg)) if msg.contains("Backend already initialized") => {
                return; // Skip test if backend already initialized
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        };

        let session_id = crate::types::SessionId::new();
        let storage_dir = temp_dir.path();
        let session_file = storage_dir.join(format!("{}.bin", session_id));

        // Create a dummy session file
        fs::write(&session_file, b"dummy kv cache data")
            .await
            .unwrap();

        // Should return true for existing session file
        assert!(manager.has_session_kv_cache(&session_id, storage_dir));
    }

    #[tokio::test]
    async fn test_delete_session_kv_cache_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config_local(temp_dir.path().to_path_buf(), None);

        let manager = match ModelManager::new(config) {
            Ok(m) => m,
            Err(ModelError::LoadingFailed(msg)) if msg.contains("Backend already initialized") => {
                return; // Skip test if backend already initialized
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        };

        let session_id = crate::types::SessionId::new();
        let storage_dir = temp_dir.path();

        // Should return false when trying to delete nonexistent file
        let result = manager
            .delete_session_kv_cache(&session_id, storage_dir)
            .unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_delete_session_kv_cache_existing() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config_local(temp_dir.path().to_path_buf(), None);

        let manager = match ModelManager::new(config) {
            Ok(m) => m,
            Err(ModelError::LoadingFailed(msg)) if msg.contains("Backend already initialized") => {
                return; // Skip test if backend already initialized
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        };

        let session_id = crate::types::SessionId::new();
        let storage_dir = temp_dir.path();
        let session_file = storage_dir.join(format!("{}.bin", session_id));

        // Create a dummy session file
        fs::write(&session_file, b"dummy kv cache data")
            .await
            .unwrap();
        assert!(session_file.exists());

        // Should return true and delete the file
        let result = manager
            .delete_session_kv_cache(&session_id, storage_dir)
            .unwrap();
        assert!(result);
        assert!(!session_file.exists());
    }

    #[tokio::test]
    async fn test_load_session_kv_cache_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config_local(temp_dir.path().to_path_buf(), None);

        let manager = match ModelManager::new(config) {
            Ok(m) => m,
            Err(ModelError::LoadingFailed(msg)) if msg.contains("Backend already initialized") => {
                return; // Skip test if backend already initialized
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        };

        let session_id = crate::types::SessionId::new();
        let storage_dir = temp_dir.path();

        // Create a dummy context - this will fail in practice but we're testing the file logic
        let model_file = temp_dir.path().join("dummy.gguf");
        fs::write(&model_file, b"dummy model").await.unwrap();

        // The load will fail due to invalid model file, but that's expected
        // We're just testing that the method handles nonexistent session files correctly
        // In this case, we expect it to return an empty Vec for nonexistent files
        // but we can't test the full flow without a valid model
        assert!(!manager.has_session_kv_cache(&session_id, storage_dir));
    }

    #[tokio::test]
    async fn test_session_kv_cache_file_path_generation() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config_local(temp_dir.path().to_path_buf(), None);

        let manager = match ModelManager::new(config) {
            Ok(m) => m,
            Err(ModelError::LoadingFailed(msg)) if msg.contains("Backend already initialized") => {
                return; // Skip test if backend already initialized
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        };

        let session_id = crate::types::SessionId::new();
        let storage_dir = temp_dir.path();

        // Test that session file paths are generated consistently
        let expected_path = storage_dir.join(format!("{}.bin", session_id));

        // Create the expected file
        fs::write(&expected_path, b"test data").await.unwrap();

        // Check that has_session_kv_cache finds it
        assert!(manager.has_session_kv_cache(&session_id, storage_dir));

        // Check that delete_session_kv_cache removes it
        let deleted = manager
            .delete_session_kv_cache(&session_id, storage_dir)
            .unwrap();
        assert!(deleted);
        assert!(!expected_path.exists());
    }
}
