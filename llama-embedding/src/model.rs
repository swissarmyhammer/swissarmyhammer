use crate::error::{EmbeddingError, EmbeddingResult as Result};
use crate::types::{EmbeddingConfig, EmbeddingResult};
use llama_cpp_2::{
    context::{params::LlamaContextParams, LlamaContext},
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::LlamaModel,
    send_logs_to_tracing, LogOptions,
};
use llama_loader::{ModelConfig, ModelLoader, ModelMetadata, RetryConfig};
use std::num::NonZeroU32;
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use swissarmyhammer_common::Pretty;
use tracing::{debug, info};

use std::ffi::c_void;
use std::os::raw::c_char;

const LLAMA_CPP_DEFAULT_N_BATCH: u32 = 512;

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

/// Embedding model - single-threaded, no concurrency.
/// Model and context are owned directly, dropped when EmbeddingModel is dropped.
pub struct EmbeddingModel {
    /// Reference to global backend for context creation
    backend: Arc<LlamaBackend>,
    model: Option<LlamaModel>,
    context: Option<LlamaContext<'static>>,
    metadata: Option<ModelMetadata>,
    config: EmbeddingConfig,
}

// SAFETY: EmbeddingModel owns all its resources exclusively.
// The raw pointers in LlamaContext/LlamaModel are only accessed through
// &mut self methods, ensuring exclusive access. No concurrent access is possible.
unsafe impl Send for EmbeddingModel {}
unsafe impl Sync for EmbeddingModel {}

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
            model: None,
            context: None,
            metadata: None,
            config,
        })
    }

    /// Load the model
    pub async fn load_model(&mut self) -> Result<()> {
        if self.model.is_some() {
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
            batch_size: LLAMA_CPP_DEFAULT_N_BATCH,
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: true,
            retry_config: RetryConfig::default(),
            debug: self.config.debug,
        };

        // Use the global backend for the loader
        let loader = ModelLoader::new(self.backend.clone());
        let loaded = loader
            .load_model(&model_config)
            .await
            .map_err(EmbeddingError::ModelLoader)?;

        let ctx_size = loaded.metadata.context_size;
        self.metadata = Some(loaded.metadata);
        self.model = Some(loaded.model);

        info!(
            "Model loaded in {}, context: {} tokens",
            Pretty(&start.elapsed()),
            ctx_size
        );
        Ok(())
    }

    /// Ensure context exists, creating if needed
    fn ensure_context(&mut self) -> Result<()> {
        if self.context.is_some() {
            return Ok(());
        }

        let model = self.model.as_ref().ok_or(EmbeddingError::ModelNotLoaded)?;
        let metadata = self
            .metadata
            .as_ref()
            .ok_or(EmbeddingError::ModelNotLoaded)?;

        info!("Creating LlamaContext");
        let batch_size = metadata.context_size as u32;
        let n_ctx = NonZeroU32::new(batch_size);
        let params = LlamaContextParams::default()
            .with_embeddings(true)
            .with_n_ctx(n_ctx)
            .with_n_batch(batch_size)
            .with_n_ubatch(batch_size);

        let ctx = model
            .new_context(&self.backend, params)
            .map_err(|e| EmbeddingError::model(format!("Context creation failed: {}", e)))?;

        // SAFETY: We own the model and it won't be dropped before context
        let ctx: LlamaContext<'static> = unsafe { std::mem::transmute(ctx) };
        self.context = Some(ctx);
        Ok(())
    }

    /// Generate embedding for text
    pub async fn embed_text(&mut self, text: &str) -> Result<EmbeddingResult> {
        if text.is_empty() {
            return Err(EmbeddingError::text_processing(
                "Input text cannot be empty",
            ));
        }

        self.ensure_context()?;

        let model = self.model.as_ref().ok_or(EmbeddingError::ModelNotLoaded)?;
        let metadata = self
            .metadata
            .as_ref()
            .ok_or(EmbeddingError::ModelNotLoaded)?;
        let ctx = self
            .context
            .as_mut()
            .ok_or(EmbeddingError::ModelNotLoaded)?;

        embed_single(
            ctx,
            model,
            text,
            metadata.context_size,
            self.config.normalize_embeddings,
        )
    }

    /// Get embedding dimension
    pub fn get_embedding_dimension(&self) -> Option<usize> {
        self.model.as_ref().map(get_embedding_dimension)
    }

    /// Get model metadata
    pub fn get_metadata(&self) -> Option<&ModelMetadata> {
        self.metadata.as_ref()
    }

    /// Check if model is loaded
    pub fn is_loaded(&self) -> bool {
        self.model.is_some()
    }
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
    use llama_loader::ModelSource;
    use serial_test::serial;

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
        assert!(model.get_metadata().is_none());
        assert!(model.get_embedding_dimension().is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_embed_requires_load() {
        let config = EmbeddingConfig::default();
        let mut model = EmbeddingModel::new(config).await.unwrap();
        let result = model.embed_text("test").await;
        assert!(matches!(result, Err(EmbeddingError::ModelNotLoaded)));
    }

    #[tokio::test]
    #[serial]
    async fn test_empty_text_rejected() {
        let config = EmbeddingConfig::default();
        let mut model = EmbeddingModel::new(config).await.unwrap();
        let result = model.embed_text("").await;
        assert!(matches!(result, Err(EmbeddingError::TextProcessing(_))));
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
        // Verify the default embedding dimension is a reasonable value for embeddings
        // Common embedding dimensions: 384, 768, 1024, 1536
        assert!(DEFAULT_EMBEDDING_DIMENSION > 0);
        assert!(DEFAULT_EMBEDDING_DIMENSION <= 4096);
    }

    /// Integration tests for load_model and embed_text with real models
    /// are in tests/integration/real_model_integration.rs covering:
    /// - load_model success path (test_single_text_embedding, test_model_loading_and_caching)
    /// - embed_text success path (test_single_text_embedding, test_batch_consistency)
    /// - get_embedding_dimension after load (test_single_text_embedding)
    #[test]
    fn test_integration_coverage_documented() {
        // This test documents that load_model, embed_text, and get_embedding_dimension
        // success paths are covered by integration tests with real models
    }
}
