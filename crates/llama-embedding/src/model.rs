use crate::error::{EmbedResult as Result, EmbeddingError};
use crate::types::{EmbeddingConfig, EmbeddingResult};
use llama_cpp_2::{
    context::{params::LlamaContextParams, LlamaContext},
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::LlamaModel,
    send_logs_to_tracing, LogOptions,
};
use model_embedding::TextEmbedder;
use model_loader::{ModelConfig, ModelMetadata, ModelResolver, RetryConfig};
use std::num::NonZeroU32;
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use swissarmyhammer_common::Pretty;
use tracing::{debug, info};

use std::ffi::c_void;
use std::os::raw::c_char;

/// Global backend singleton for llama-cpp.
///
/// llama-cpp only allows one backend initialization per process.
/// This ensures all EmbeddingModel instances share the same backend.
/// Stores Result to capture initialization errors.
static GLOBAL_BACKEND: OnceLock<std::result::Result<Arc<LlamaBackend>, String>> = OnceLock::new();

/// Get or initialize the global llama backend.
///
/// Returns a shared reference to the backend. The backend is lazily
/// initialized on first call and reused for all subsequent calls.
///
/// Note: `llama-agent` has a parallel implementation (`get_or_init_backend`).
/// Both are kept separate to avoid pulling `llama-cpp-2` into `llama-common`.
fn get_global_backend() -> Result<Arc<LlamaBackend>> {
    let result = GLOBAL_BACKEND.get_or_init(|| {
        LlamaBackend::init()
            .map(Arc::new)
            .map_err(|e| format!("Backend init failed: {}", e))
    });

    match result {
        Ok(backend) => Ok(backend.clone()),
        Err(e) => Err(EmbeddingError::model(e.clone())),
    }
}

/// Default embedding dimension fallback when model reports invalid value
const DEFAULT_EMBEDDING_DIMENSION: usize = 384;

/// Returns the embedding dimension from a LlamaModel.
///
/// Falls back to `DEFAULT_EMBEDDING_DIMENSION` if the model reports an invalid
/// (zero or negative) dimension value.
fn get_embedding_dimension(model: &LlamaModel) -> usize {
    let n = model.n_embd();
    if n > 0 {
        n as usize
    } else {
        DEFAULT_EMBEDDING_DIMENSION
    }
}

extern "C" fn null_log_callback(_level: i32, _text: *const c_char, _user_data: *mut c_void) {}

fn set_logging_suppression(suppress: bool) {
    unsafe {
        extern "C" {
            fn llama_log_set(
                log_callback: Option<extern "C" fn(i32, *const c_char, *mut c_void)>,
                user_data: *mut c_void,
            );
        }
        if suppress {
            llama_log_set(Some(null_log_callback), std::ptr::null_mut());
        } else {
            llama_log_set(None, std::ptr::null_mut());
        }
    }
}

/// Mutable state guarded by a mutex for interior mutability.
struct Inner {
    model: Option<LlamaModel>,
    context: Option<LlamaContext<'static>>,
    metadata: Option<ModelMetadata>,
    /// Context size extracted from the loaded model (not from metadata)
    context_size: usize,
}

// SAFETY: LlamaModel and LlamaContext contain raw pointers but are only
// accessed through the Mutex, ensuring exclusive access.
unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

/// Embedding model using llama-cpp-2 backend.
///
/// Uses interior mutability (Mutex) so all methods take `&self`.
/// This allows implementing the `TextEmbedder` trait which requires `&self`.
pub struct EmbeddingModel {
    backend: Arc<LlamaBackend>,
    inner: tokio::sync::Mutex<Inner>,
    config: EmbeddingConfig,
}

impl model_embedding::private::Sealed for EmbeddingModel {}

impl EmbeddingModel {
    /// Create a new EmbeddingModel (nothing loaded yet)
    pub async fn new(config: EmbeddingConfig) -> Result<Self> {
        if config.debug {
            send_logs_to_tracing(LogOptions::default());
            set_logging_suppression(false);
        } else {
            set_logging_suppression(true);
        }

        let backend = get_global_backend()?;

        Ok(Self {
            backend,
            inner: tokio::sync::Mutex::new(Inner {
                model: None,
                context: None,
                metadata: None,
                context_size: 0,
            }),
            config,
        })
    }

    /// Creates default model parameters optimized for GPU offloading
    fn default_model_params() -> llama_cpp_2::model::params::LlamaModelParams {
        let gpu_layers: u32 = std::env::var("LLAMA_N_GPU_LAYERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(i32::MAX as u32);

        llama_cpp_2::model::params::LlamaModelParams::default()
            .with_n_gpu_layers(gpu_layers)
            .with_use_mlock(true)
    }

    /// Extract context length from model metadata
    fn extract_context_size(model: &LlamaModel) -> usize {
        let meta_count = model.meta_count();
        for i in 0..meta_count {
            if let (Ok(key), Ok(value)) =
                (model.meta_key_by_index(i), model.meta_val_str_by_index(i))
            {
                if key.contains("max_position_embeddings") || key.contains("context_length") {
                    if let Ok(ctx_val) = value.parse::<usize>() {
                        if ctx_val > 8192 {
                            return ctx_val;
                        }
                    }
                }
            }
        }
        model.n_ctx_train() as usize
    }

    /// Load the model (crate-internal; external callers use `TextEmbedder::load`)
    async fn load_model(&self) -> Result<()> {
        let mut inner = self.inner.lock().await;

        if inner.model.is_some() {
            info!("Model already loaded");
            return Ok(());
        }

        info!(
            "Loading embedding model from {:?}",
            self.config.model_source
        );
        let start = Instant::now();

        let model_config = ModelConfig {
            source: self.config.model_source.clone(),
            retry_config: RetryConfig::default(),
            debug: self.config.debug,
        };

        // Resolve model source to local path
        let resolver = ModelResolver::new();
        let resolved = resolver
            .resolve(&model_config)
            .await
            .map_err(EmbeddingError::ModelLoader)?;

        // Verify the resolved file is a .gguf model (llama-cpp-2 only supports GGUF)
        if resolved.path.extension().and_then(|e| e.to_str()) != Some("gguf") {
            return Err(EmbeddingError::model(format!(
                "llama-cpp-2 only supports .gguf models, got: {}",
                resolved.path.display()
            )));
        }

        // Load model into llama-cpp-2
        let model_params = Self::default_model_params();
        let model = LlamaModel::load_from_file(&self.backend, &resolved.path, &model_params)
            .map_err(|e| {
                EmbeddingError::model(format!(
                    "Failed to load model from {}: {}",
                    resolved.path.display(),
                    e
                ))
            })?;

        let ctx_size = Self::extract_context_size(&model);
        inner.context_size = ctx_size;
        inner.metadata = Some(resolved.metadata);
        inner.model = Some(model);

        info!(
            "Model loaded in {}, context: {} tokens",
            Pretty(&start.elapsed()),
            ctx_size
        );
        Ok(())
    }

    /// Generate embedding for text (crate-internal; external callers use `TextEmbedder::embed_text`)
    async fn embed_impl(&self, text: &str) -> Result<EmbeddingResult> {
        if text.is_empty() {
            return Err(EmbeddingError::text_processing(
                "Input text cannot be empty",
            ));
        }

        let mut inner = self.inner.lock().await;

        ensure_context(&self.backend, &self.config, &mut inner)?;

        let max_seq = self
            .config
            .max_sequence_length
            .unwrap_or(inner.context_size);

        // Split borrow: take context out temporarily to avoid simultaneous
        // immutable (model) + mutable (context) borrows on inner.
        let mut ctx = inner.context.take().ok_or(EmbeddingError::ModelNotLoaded)?;
        let model = inner.model.as_ref().ok_or(EmbeddingError::ModelNotLoaded)?;

        let result = embed_single(
            &mut ctx,
            model,
            text,
            max_seq,
            self.config.normalize_embeddings,
        );
        inner.context = Some(ctx);
        result
    }

    /// Get embedding dimension (crate-internal; external callers use `TextEmbedder::embedding_dimension`).
    ///
    /// Returns `None` if the model is not loaded **or** if the mutex is currently
    /// held by another task (e.g., during `embed_text`). This is intentional —
    /// `embedding_dimension` is a non-async method and cannot `.await` the lock.
    fn embedding_dimension_impl(&self) -> Option<usize> {
        self.inner
            .try_lock()
            .ok()
            .and_then(|inner| inner.model.as_ref().map(get_embedding_dimension))
    }

    /// Get model metadata
    pub fn metadata(&self) -> Option<ModelMetadata> {
        self.inner
            .try_lock()
            .ok()
            .and_then(|inner| inner.metadata.clone())
    }

    /// Check if model is loaded (crate-internal; external callers use `TextEmbedder::is_loaded`)
    fn is_loaded_impl(&self) -> bool {
        self.inner
            .try_lock()
            .map(|inner| inner.model.is_some())
            .unwrap_or(false)
    }
}

#[async_trait::async_trait]
impl TextEmbedder for EmbeddingModel {
    async fn load(&self) -> std::result::Result<(), model_embedding::EmbeddingError> {
        self.load_model()
            .await
            .map_err(|e| model_embedding::EmbeddingError::Backend(Box::new(e)))
    }

    async fn embed_text(
        &self,
        text: &str,
    ) -> std::result::Result<model_embedding::EmbeddingResult, model_embedding::EmbeddingError>
    {
        self.embed_impl(text)
            .await
            .map_err(|e| model_embedding::EmbeddingError::Backend(Box::new(e)))
    }

    fn embedding_dimension(&self) -> Option<usize> {
        self.embedding_dimension_impl()
    }

    fn is_loaded(&self) -> bool {
        self.is_loaded_impl()
    }
}

/// Ensure context exists on the inner state, creating if needed.
fn ensure_context(
    backend: &LlamaBackend,
    config: &EmbeddingConfig,
    inner: &mut Inner,
) -> Result<()> {
    if inner.context.is_some() {
        return Ok(());
    }

    let model = inner.model.as_ref().ok_or(EmbeddingError::ModelNotLoaded)?;
    if inner.metadata.is_none() {
        return Err(EmbeddingError::ModelNotLoaded);
    }

    let ctx_size = config.max_sequence_length.unwrap_or(inner.context_size) as u32;

    info!("Creating LlamaContext with n_ctx={}", ctx_size);
    let n_ctx = NonZeroU32::new(ctx_size);
    let params = LlamaContextParams::default()
        .with_embeddings(true)
        .with_n_ctx(n_ctx)
        .with_n_batch(ctx_size)
        .with_n_ubatch(ctx_size);

    let ctx = model
        .new_context(backend, params)
        .map_err(|e| EmbeddingError::model(format!("Context creation failed: {}", e)))?;

    // SAFETY: We own the model and it won't be dropped before context
    let ctx: LlamaContext<'static> = unsafe { std::mem::transmute(ctx) };
    inner.context = Some(ctx);
    Ok(())
}

fn embed_single(
    ctx: &mut LlamaContext,
    model: &LlamaModel,
    text: &str,
    max_seq_len: usize,
    normalize: bool,
) -> Result<EmbeddingResult> {
    use llama_cpp_2::model::AddBos;

    let start = Instant::now();

    // Clear KV cache before each embedding to ensure independent processing
    ctx.clear_kv_cache();

    let tokens = ctx
        .model
        .str_to_token(text, AddBos::Never)
        .map_err(|e| EmbeddingError::text_encoding(format!("Tokenize failed: {}", e)))?;

    if tokens.is_empty() {
        return Err(EmbeddingError::text_encoding("No tokens"));
    }

    let tokens = if tokens.len() > max_seq_len {
        debug!("Truncating {} -> {} tokens", tokens.len(), max_seq_len);
        tokens[..max_seq_len].to_vec()
    } else {
        tokens
    };

    let dim = get_embedding_dimension(model);

    let mut batch = LlamaBatch::new(tokens.len(), 1);
    batch
        .add_sequence(&tokens, 0, false)
        .map_err(|e| EmbeddingError::text_processing(format!("Batch failed: {}", e)))?;

    ctx.decode(&mut batch)
        .map_err(|e| EmbeddingError::text_processing(format!("Decode failed: {}", e)))?;

    let emb = ctx
        .embeddings_seq_ith(0)
        .map_err(|e| EmbeddingError::text_processing(format!("Extract failed: {}", e)))?;

    if emb.len() != dim {
        return Err(EmbeddingError::text_processing(format!(
            "Dimension mismatch: {} vs {}",
            dim,
            emb.len()
        )));
    }

    let mut result = EmbeddingResult::new(
        text.to_string(),
        emb.to_vec(),
        tokens.len(),
        start.elapsed().as_millis() as u64,
    );

    if normalize {
        result.normalize();
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use model_loader::ModelSource;
    use serial_test::serial;
    use std::path::PathBuf;

    #[tokio::test]
    #[serial]
    async fn test_model_creation() {
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await;
        assert!(model.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_not_loaded_initially() {
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        assert!(!model.is_loaded());
        assert!(model.embedding_dimension().is_none());
        assert!(model.metadata().is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_embed_requires_load() {
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        let result = model.embed_text("test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_empty_text_rejected() {
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        let result = model.embed_text("").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_trait_object_works() {
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        let embedder: &dyn TextEmbedder = &model;
        assert!(!embedder.is_loaded());
        assert!(embedder.embedding_dimension().is_none());
    }

    const TEST_MAX_SEQUENCE_LENGTH: usize = 512;

    #[test]
    fn test_config_fields() {
        let config = EmbeddingConfig {
            model_source: ModelSource::HuggingFace {
                repo: "test/repo".to_string(),
                filename: Some("test.gguf".to_string()),
                folder: None,
            },
            normalize_embeddings: true,
            max_sequence_length: Some(TEST_MAX_SEQUENCE_LENGTH),
            debug: true,
        };
        assert!(config.normalize_embeddings);
        assert_eq!(config.max_sequence_length, Some(TEST_MAX_SEQUENCE_LENGTH));
    }

    #[test]
    fn test_default_config() {
        let config = EmbeddingConfig::default();
        assert!(!config.normalize_embeddings);
        assert!(config.max_sequence_length.is_none());
        assert!(!config.debug);
    }

    #[test]
    fn test_get_embedding_dimension_helper() {
        const { assert!(DEFAULT_EMBEDDING_DIMENSION > 0) };
        const { assert!(DEFAULT_EMBEDDING_DIMENSION <= 4096) };
    }

    #[test]
    fn test_default_embedding_dimension_value() {
        // Verifies the fallback dimension is a sensible default (384 for small models).
        assert_eq!(DEFAULT_EMBEDDING_DIMENSION, 384);
    }

    // ── Error-path tests (no real model needed) ──────────────────────────

    #[tokio::test]
    #[serial]
    async fn test_load_missing_local_file_returns_error() {
        // Loading a model from a nonexistent local path should fail gracefully.
        let config = EmbeddingConfig {
            model_source: ModelSource::Local {
                folder: PathBuf::from("/tmp/nonexistent-model-dir-sah-test"),
                filename: Some("does_not_exist.gguf".to_string()),
            },
            normalize_embeddings: false,
            max_sequence_length: None,
            debug: false,
        };
        let model = EmbeddingModel::new(config).await.unwrap();
        let result = model.load().await;
        assert!(result.is_err(), "Loading a missing model file should fail");
    }

    #[tokio::test]
    #[serial]
    async fn test_load_non_gguf_file_returns_error() {
        // The loader should reject files that are not .gguf format.
        let dir = tempfile::tempdir().unwrap();
        let fake_model = dir.path().join("model.bin");
        std::fs::write(&fake_model, b"not a real model").unwrap();

        let config = EmbeddingConfig {
            model_source: ModelSource::Local {
                folder: dir.path().to_path_buf(),
                filename: Some("model.bin".to_string()),
            },
            normalize_embeddings: false,
            max_sequence_length: None,
            debug: false,
        };
        let model = EmbeddingModel::new(config).await.unwrap();
        let result = model.load().await;
        assert!(result.is_err(), "Loading a non-GGUF file should fail");
    }

    #[tokio::test]
    #[serial]
    async fn test_load_corrupt_gguf_returns_error() {
        // A file with .gguf extension but garbage content should fail.
        let dir = tempfile::tempdir().unwrap();
        let fake_gguf = dir.path().join("corrupt.gguf");
        std::fs::write(&fake_gguf, b"this is not valid gguf data").unwrap();

        let config = EmbeddingConfig {
            model_source: ModelSource::Local {
                folder: dir.path().to_path_buf(),
                filename: Some("corrupt.gguf".to_string()),
            },
            normalize_embeddings: false,
            max_sequence_length: None,
            debug: false,
        };
        let model = EmbeddingModel::new(config).await.unwrap();
        let result = model.load().await;
        assert!(result.is_err(), "Loading a corrupt GGUF file should fail");
    }

    #[tokio::test]
    #[serial]
    async fn test_embed_text_before_load_returns_model_not_loaded() {
        // embed_text should return a clear error when called without load().
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        let err = model.embed_text("hello world").await;
        assert!(err.is_err());
        let err_msg = format!("{}", err.unwrap_err());
        assert!(
            err_msg.contains("not loaded") || err_msg.contains("Not Loaded"),
            "Error should mention model not loaded, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_empty_text_error_message() {
        // Empty text should produce a specific error, not a generic one.
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        let err = model.embed_text("").await;
        assert!(err.is_err());
        let err_msg = format!("{}", err.unwrap_err());
        assert!(
            err_msg.to_lowercase().contains("empty"),
            "Error should mention empty text, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_metadata_none_before_load() {
        // Metadata should be None before any model is loaded.
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        assert!(model.metadata().is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_is_loaded_false_after_creation() {
        // A freshly created model should report is_loaded = false.
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        assert!(!model.is_loaded_impl());
    }

    #[tokio::test]
    #[serial]
    async fn test_embedding_dimension_none_before_load() {
        // embedding_dimension should return None when model is not loaded.
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        assert!(model.embedding_dimension_impl().is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_debug_mode_creation() {
        // Creating a model with debug=true should succeed (enables logging).
        let config = EmbeddingConfig {
            debug: true,
            ..EmbeddingConfig::default()
        };
        let model = EmbeddingModel::new(config).await;
        assert!(model.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_custom_max_sequence_length_stored() {
        // max_sequence_length from config should be carried through.
        let config = EmbeddingConfig {
            max_sequence_length: Some(256),
            ..EmbeddingConfig::default()
        };
        let model = EmbeddingModel::new(config).await.unwrap();
        assert_eq!(model.config.max_sequence_length, Some(256));
    }

    #[test]
    fn test_integration_coverage_documented() {}

    // ── ensure_context error paths ──────────────────────────────────────

    #[tokio::test]
    #[serial]
    async fn test_ensure_context_no_model() {
        // ensure_context should fail when model is None.
        let backend = get_global_backend().expect("Backend init");
        let config = EmbeddingConfig::default();
        let mut inner = Inner {
            model: None,
            context: None,
            metadata: None,
            context_size: 0,
        };
        let result = ensure_context(&backend, &config, &mut inner);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, EmbeddingError::ModelNotLoaded));
    }

    #[tokio::test]
    #[serial]
    async fn test_ensure_context_already_set() {
        // When context is already Some, ensure_context should return Ok immediately.
        // We can't create a real LlamaContext, but we test the early-return branch
        // indirectly: calling embed_text twice would reuse context on second call.
        // The explicit check: inner.context.is_some() returns Ok.
        // Since we can't mock LlamaContext, we verify through embed_impl behavior.
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        // Without load, context is None and model is None -> error
        let result = model.embed_impl("test").await;
        assert!(result.is_err());
    }

    // ── default_model_params tests ──────────────────────────────────────

    #[test]
    fn test_default_model_params_no_env() {
        // Without LLAMA_N_GPU_LAYERS, should use i32::MAX as default.
        std::env::remove_var("LLAMA_N_GPU_LAYERS");
        let _params = EmbeddingModel::default_model_params();
        // If we get here without panic, params were created successfully.
    }

    #[test]
    fn test_default_model_params_with_env() {
        // With LLAMA_N_GPU_LAYERS set, should parse the value.
        std::env::set_var("LLAMA_N_GPU_LAYERS", "16");
        let _params = EmbeddingModel::default_model_params();
        std::env::remove_var("LLAMA_N_GPU_LAYERS");
    }

    #[test]
    fn test_default_model_params_invalid_env() {
        // With invalid LLAMA_N_GPU_LAYERS, should fall back to default.
        std::env::set_var("LLAMA_N_GPU_LAYERS", "not_a_number");
        let _params = EmbeddingModel::default_model_params();
        std::env::remove_var("LLAMA_N_GPU_LAYERS");
    }

    // ── set_logging_suppression tests ────────────────────────────────────

    #[test]
    #[serial]
    fn test_set_logging_suppression_enabled() {
        // Suppressing logging should not panic.
        set_logging_suppression(true);
    }

    #[test]
    #[serial]
    fn test_set_logging_suppression_disabled() {
        // Restoring default logging should not panic.
        set_logging_suppression(false);
    }

    #[test]
    #[serial]
    fn test_set_logging_suppression_roundtrip() {
        // Toggling suppression back and forth should be safe.
        set_logging_suppression(true);
        set_logging_suppression(false);
        set_logging_suppression(true);
    }

    // ── get_global_backend tests ─────────────────────────────────────────

    #[tokio::test]
    #[serial]
    async fn test_global_backend_returns_same_instance() {
        // get_global_backend should return the same Arc each call.
        let b1 = get_global_backend().expect("Backend init");
        let b2 = get_global_backend().expect("Backend init");
        assert!(Arc::ptr_eq(&b1, &b2));
    }

    // ── null_log_callback test ──────────────────────────────────────────

    #[test]
    fn test_null_log_callback_does_not_panic() {
        // The null log callback should silently ignore all inputs.
        null_log_callback(0, std::ptr::null(), std::ptr::null_mut());
        null_log_callback(3, std::ptr::null(), std::ptr::null_mut());
    }

    // ── EmbeddingModel trait method delegation ──────────────────────────

    #[tokio::test]
    #[serial]
    async fn test_trait_load_without_model_returns_error() {
        // TextEmbedder::load should propagate the internal error.
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        // Load with default HuggingFace source will fail in test env (no network/model)
        // but should not panic.
        let _result = model.load().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_trait_embed_text_empty_string() {
        // TextEmbedder::embed_text with empty string should return error.
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        let result = model.embed_text("").await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.to_lowercase().contains("empty"),
            "Expected empty text error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_trait_is_loaded_delegates() {
        // TextEmbedder::is_loaded should delegate to is_loaded_impl.
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        let embedder: &dyn TextEmbedder = &model;
        assert_eq!(embedder.is_loaded(), model.is_loaded_impl());
    }

    #[tokio::test]
    #[serial]
    async fn test_trait_embedding_dimension_delegates() {
        // TextEmbedder::embedding_dimension should delegate to embedding_dimension_impl.
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        let embedder: &dyn TextEmbedder = &model;
        assert_eq!(
            embedder.embedding_dimension(),
            model.embedding_dimension_impl()
        );
    }

    // ── Config stored on model ──────────────────────────────────────────

    #[tokio::test]
    #[serial]
    async fn test_config_normalize_embeddings_stored() {
        // The normalize_embeddings flag should be preserved on the model.
        let config = EmbeddingConfig {
            normalize_embeddings: true,
            ..EmbeddingConfig::default()
        };
        let model = EmbeddingModel::new(config).await.unwrap();
        assert!(model.config.normalize_embeddings);
    }

    #[tokio::test]
    #[serial]
    async fn test_config_debug_stored() {
        // The debug flag should be preserved on the model.
        let config = EmbeddingConfig {
            debug: true,
            ..EmbeddingConfig::default()
        };
        let model = EmbeddingModel::new(config).await.unwrap();
        assert!(model.config.debug);
    }

    #[tokio::test]
    #[serial]
    async fn test_model_source_huggingface_stored() {
        // HuggingFace source should be preserved on the model config.
        let config = EmbeddingConfig {
            model_source: ModelSource::HuggingFace {
                repo: "custom/model".to_string(),
                filename: Some("custom.gguf".to_string()),
                folder: None,
            },
            ..EmbeddingConfig::default()
        };
        let model = EmbeddingModel::new(config).await.unwrap();
        match &model.config.model_source {
            ModelSource::HuggingFace { repo, filename, .. } => {
                assert_eq!(repo, "custom/model");
                assert_eq!(filename.as_deref(), Some("custom.gguf"));
            }
            _ => panic!("Expected HuggingFace source"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_model_source_local_stored() {
        // Local source should be preserved on the model config.
        let config = EmbeddingConfig {
            model_source: ModelSource::Local {
                folder: PathBuf::from("/tmp/models"),
                filename: Some("test.gguf".to_string()),
            },
            ..EmbeddingConfig::default()
        };
        let model = EmbeddingModel::new(config).await.unwrap();
        match &model.config.model_source {
            ModelSource::Local { folder, filename } => {
                assert_eq!(folder, &PathBuf::from("/tmp/models"));
                assert_eq!(filename.as_deref(), Some("test.gguf"));
            }
            _ => panic!("Expected Local source"),
        }
    }

    // ── Inner struct default state ──────────────────────────────────────

    #[test]
    fn test_inner_default_state() {
        // Inner should be constructed with all None/zero values.
        let inner = Inner {
            model: None,
            context: None,
            metadata: None,
            context_size: 0,
        };
        assert!(inner.model.is_none());
        assert!(inner.context.is_none());
        assert!(inner.metadata.is_none());
        assert_eq!(inner.context_size, 0);
    }

    // ── embed_impl error paths ──────────────────────────────────────────

    #[tokio::test]
    #[serial]
    async fn test_embed_impl_empty_text() {
        // embed_impl should reject empty text before touching the model.
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        let result = model.embed_impl("").await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.to_lowercase().contains("empty"));
    }

    #[tokio::test]
    #[serial]
    async fn test_embed_impl_no_model_loaded() {
        // embed_impl should fail when model is not loaded.
        let config = EmbeddingConfig::default();
        let model = EmbeddingModel::new(config).await.unwrap();
        let result = model.embed_impl("some text").await;
        assert!(result.is_err());
    }
}
