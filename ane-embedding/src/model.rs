use std::sync::OnceLock;
use std::time::Instant;

use async_trait::async_trait;
use model_embedding::{EmbeddingResult, TextEmbedder};
use model_loader::{ModelConfig, ModelResolver, RetryConfig};
use onnxruntime_coreml_sys::{self as ort, LoggingLevel, Session, SessionOptions, Tensor};
use tokenizers::Tokenizer;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::error::EmbeddingError;
use crate::types::{AneEmbeddingConfig, Pooling};

/// Global ORT initialization (safe to call multiple times, only initializes once)
static ORT_INIT: OnceLock<std::result::Result<(), String>> = OnceLock::new();

fn ensure_ort_initialized() -> std::result::Result<(), EmbeddingError> {
    let result = ORT_INIT.get_or_init(|| {
        ort::init().map_err(|e| format!("Failed to initialize ORT: {}", e.message))
    });
    match result {
        Ok(()) => Ok(()),
        Err(msg) => Err(EmbeddingError::onnx_runtime(msg.clone())),
    }
}

struct Inner {
    session: Option<Session>,
    tokenizer: Option<Tokenizer>,
    env: Option<ort::Env>,
    embedding_dim: Option<usize>,
    max_length: usize,
}

/// Text embedding model using ONNX Runtime with CoreML execution provider.
///
/// Uses the Apple Neural Engine on supported hardware, with CPU fallback elsewhere.
pub struct AneEmbeddingModel {
    inner: Mutex<Inner>,
    config: AneEmbeddingConfig,
}

// Safety: Inner contains ORT handles that are thread-safe (ORT guarantees
// thread-safe inference on a single session). Access is serialized by Mutex.
unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

impl model_embedding::private::Sealed for AneEmbeddingModel {}

#[async_trait]
impl TextEmbedder for AneEmbeddingModel {
    async fn load(&self) -> std::result::Result<(), model_embedding::EmbeddingError> {
        self.load_model().await.map_err(Into::into)
    }

    async fn embed_text(
        &self,
        text: &str,
    ) -> std::result::Result<EmbeddingResult, model_embedding::EmbeddingError> {
        self.embed_impl(text).await.map_err(Into::into)
    }

    fn embedding_dimension(&self) -> Option<usize> {
        self.inner.try_lock().ok().and_then(|g| g.embedding_dim)
    }

    fn is_loaded(&self) -> bool {
        self.inner
            .try_lock()
            .map(|g| g.session.is_some())
            .unwrap_or(false)
    }
}

impl AneEmbeddingModel {
    /// Create a new ANE embedding model with the given configuration.
    /// Call `load()` before using `embed_text()`.
    pub fn new(config: AneEmbeddingConfig) -> Self {
        Self {
            inner: Mutex::new(Inner {
                session: None,
                tokenizer: None,
                env: None,
                embedding_dim: None,
                max_length: config.max_sequence_length.unwrap_or(512),
            }),
            config,
        }
    }

    /// Load the ONNX model and tokenizer.
    async fn load_model(&self) -> crate::error::Result<()> {
        ensure_ort_initialized()?;

        let mut inner = self.inner.lock().await;
        if inner.session.is_some() {
            return Ok(());
        }

        // Resolve model file via model-loader
        let resolver = ModelResolver::new();
        let model_config = ModelConfig {
            source: self.config.model_source.clone(),
            retry_config: RetryConfig::default(),
            debug: self.config.debug,
        };

        info!("Resolving ONNX model...");
        let resolved = resolver
            .resolve(&model_config)
            .await
            .map_err(EmbeddingError::ModelLoader)?;
        let model_path = resolved.path.clone();

        info!(
            path = %model_path.display(),
            size_bytes = resolved.metadata.size_bytes,
            "Model resolved"
        );

        // Load tokenizer from the same directory
        let tokenizer = load_tokenizer(&model_path)?;
        if let Some(max_len) = self.config.max_sequence_length {
            inner.max_length = max_len;
        }

        // Create ORT env + session with CoreML
        let env = ort::Env::new(LoggingLevel::Warning, "ane-embedding")
            .map_err(EmbeddingError::from)?;

        let opts = create_session_options()?;

        let model_path_str = model_path.to_string_lossy();
        info!("Loading ONNX session...");
        let session =
            Session::new(&env, &model_path_str, &opts).map_err(EmbeddingError::from)?;

        debug!(
            inputs = ?session.input_names(),
            outputs = ?session.output_names(),
            "Session loaded"
        );

        inner.session = Some(session);
        inner.tokenizer = Some(tokenizer);
        inner.env = Some(env);

        Ok(())
    }

    /// Embed a single text string.
    async fn embed_impl(&self, text: &str) -> crate::error::Result<EmbeddingResult> {
        let start = Instant::now();
        let inner = self.inner.lock().await;

        let session = inner
            .session
            .as_ref()
            .ok_or(EmbeddingError::ModelNotLoaded)?;
        let tokenizer = inner
            .tokenizer
            .as_ref()
            .ok_or(EmbeddingError::ModelNotLoaded)?;

        // Tokenize with padding/truncation to fixed length
        let encoding = tokenizer
            .encode(text, true)
            .map_err(|e| EmbeddingError::tokenization(e.to_string()))?;

        let input_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();
        let token_type_ids = encoding.get_type_ids();
        let seq_len = input_ids.len().min(inner.max_length);

        // Prepare tensors — pad or truncate to max_length for static shapes
        let padded_len = inner.max_length;
        let mut ids_padded = vec![0i64; padded_len];
        let mut mask_padded = vec![0i64; padded_len];
        let mut types_padded = vec![0i64; padded_len];

        for i in 0..seq_len {
            ids_padded[i] = input_ids[i] as i64;
            mask_padded[i] = attention_mask[i] as i64;
            types_padded[i] = token_type_ids[i] as i64;
        }

        let shape = [1i64, padded_len as i64];
        let input_ids_tensor =
            Tensor::from_i64(&ids_padded, &shape).map_err(EmbeddingError::from)?;
        let attention_mask_tensor =
            Tensor::from_i64(&mask_padded, &shape).map_err(EmbeddingError::from)?;
        let token_type_ids_tensor =
            Tensor::from_i64(&types_padded, &shape).map_err(EmbeddingError::from)?;

        // Run inference — input order: input_ids, attention_mask, token_type_ids
        let outputs = session
            .run(&[
                &input_ids_tensor,
                &attention_mask_tensor,
                &token_type_ids_tensor,
            ])
            .map_err(EmbeddingError::from)?;

        if outputs.is_empty() {
            return Err(EmbeddingError::text_processing("No output from model"));
        }

        // Extract embeddings — output shape is typically [1, seq_len, hidden_dim]
        let output = &outputs[0];
        let output_shape = output.shape().map_err(EmbeddingError::from)?;
        let output_data = output.as_f32_slice().map_err(EmbeddingError::from)?;

        let embedding = match output_shape.len() {
            3 => {
                // [batch, seq_len, hidden_dim] — needs pooling
                let hidden_dim = output_shape[2] as usize;
                pool_embeddings(output_data, &mask_padded, hidden_dim, self.config.pooling)
            }
            2 => {
                // [batch, hidden_dim] — already pooled (e.g., sentence-transformers)
                output_data.to_vec()
            }
            _ => {
                return Err(EmbeddingError::text_processing(format!(
                    "Unexpected output shape: {:?}",
                    output_shape
                )));
            }
        };

        // Detect embedding dimension on first run
        if inner.embedding_dim.is_none() {
            // We need to drop the lock to mutate; do it after
        }
        let dim = embedding.len();

        let mut result = EmbeddingResult::new(text.to_string(), embedding, seq_len, start.elapsed().as_millis() as u64);

        if self.config.normalize_embeddings {
            result.normalize();
        }

        // Update cached dimension outside the main lock scope
        drop(inner);
        if let Ok(mut inner) = self.inner.try_lock() {
            if inner.embedding_dim.is_none() {
                inner.embedding_dim = Some(dim);
            }
        }

        Ok(result)
    }
}

/// Create ORT session options with CoreML on supported platforms.
fn create_session_options() -> crate::error::Result<SessionOptions> {
    let opts = SessionOptions::new().map_err(EmbeddingError::from)?;

    #[cfg(target_os = "macos")]
    let opts = {
        use onnxruntime_coreml_sys::{COREML_FLAG_CREATE_MLPROGRAM, COREML_FLAG_STATIC_INPUT_SHAPES};
        opts.with_coreml(COREML_FLAG_CREATE_MLPROGRAM | COREML_FLAG_STATIC_INPUT_SHAPES)
            .map_err(EmbeddingError::from)?
    };

    Ok(opts)
}

/// Load a HuggingFace tokenizer from the model directory.
///
/// Looks for `tokenizer.json` in the same directory as the model file,
/// or in the parent directory (for models stored in subdirectories).
fn load_tokenizer(model_path: &std::path::Path) -> crate::error::Result<Tokenizer> {
    let model_dir = model_path
        .parent()
        .ok_or_else(|| EmbeddingError::configuration("Model path has no parent directory"))?;

    // Try model dir first, then parent (for repos with onnx/ subfolder)
    let candidates = [
        model_dir.join("tokenizer.json"),
        model_dir
            .parent()
            .map(|p| p.join("tokenizer.json"))
            .unwrap_or_default(),
    ];

    for path in &candidates {
        if path.exists() {
            debug!(path = %path.display(), "Loading tokenizer");
            return Tokenizer::from_file(path)
                .map_err(|e| EmbeddingError::tokenization(format!("Failed to load tokenizer: {e}")));
        }
    }

    Err(EmbeddingError::configuration(format!(
        "tokenizer.json not found near model at {}",
        model_path.display()
    )))
}

/// Pool per-token embeddings into a single sentence embedding.
fn pool_embeddings(data: &[f32], attention_mask: &[i64], hidden_dim: usize, pooling: Pooling) -> Vec<f32> {
    match pooling {
        Pooling::Mean => {
            let mut sum = vec![0.0f32; hidden_dim];
            let mut count = 0.0f32;
            for (i, &mask) in attention_mask.iter().enumerate() {
                if mask != 0 {
                    let offset = i * hidden_dim;
                    if offset + hidden_dim <= data.len() {
                        for (j, s) in sum.iter_mut().enumerate() {
                            *s += data[offset + j];
                        }
                        count += 1.0;
                    }
                }
            }
            if count > 0.0 {
                for s in &mut sum {
                    *s /= count;
                }
            }
            sum
        }
        Pooling::Cls => {
            // First token embedding
            data[..hidden_dim].to_vec()
        }
        Pooling::LastToken => {
            // Find last non-masked token
            let last_idx = attention_mask
                .iter()
                .rposition(|&m| m != 0)
                .unwrap_or(0);
            let offset = last_idx * hidden_dim;
            if offset + hidden_dim <= data.len() {
                data[offset..offset + hidden_dim].to_vec()
            } else {
                data[..hidden_dim].to_vec()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_mean() {
        // 2 tokens, hidden_dim=3, both unmasked
        let data = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mask = [1, 1];
        let result = pool_embeddings(&data, &mask, 3, Pooling::Mean);
        assert_eq!(result, vec![2.5, 3.5, 4.5]);
    }

    #[test]
    fn test_pool_mean_with_mask() {
        // 3 tokens, hidden_dim=2, only first 2 unmasked
        let data = [1.0, 2.0, 3.0, 4.0, 0.0, 0.0];
        let mask = [1, 1, 0];
        let result = pool_embeddings(&data, &mask, 2, Pooling::Mean);
        assert_eq!(result, vec![2.0, 3.0]);
    }

    #[test]
    fn test_pool_cls() {
        let data = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mask = [1, 1];
        let result = pool_embeddings(&data, &mask, 3, Pooling::Cls);
        assert_eq!(result, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_pool_last_token() {
        let data = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 0.0, 0.0, 0.0];
        let mask = [1, 1, 0];
        let result = pool_embeddings(&data, &mask, 3, Pooling::LastToken);
        assert_eq!(result, vec![4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_new_model() {
        let config = AneEmbeddingConfig::default();
        let model = AneEmbeddingModel::new(config);
        assert!(!model.is_loaded());
        assert_eq!(model.embedding_dimension(), None);
    }
}
