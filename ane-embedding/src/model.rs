use std::time::Instant;

use async_trait::async_trait;
use coreml_rs::{ComputePlatform, CoreMLModelOptions, CoreMLModelWithState};
use model_embedding::{EmbeddingResult, TextEmbedder};
use model_loader::{ModelConfig, ModelResolver, ModelSource, RetryConfig};
use ndarray::Array2;
use tokenizers::Tokenizer;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::error::EmbeddingError;
use crate::types::AneEmbeddingConfig;

struct Inner {
    model: Option<CoreMLModelWithState>,
    tokenizer: Option<Tokenizer>,
    embedding_dim: Option<usize>,
    max_length: usize,
}

/// Text embedding model using CoreML for Apple Neural Engine.
///
/// Loads a `.mlpackage` directly via `coreml-rs` for hardware-accelerated
/// inference on Apple Silicon. Mean pooling is baked into the model.
pub struct AneEmbeddingModel {
    inner: Mutex<Inner>,
    config: AneEmbeddingConfig,
}

// Safety: CoreMLModelWithState is thread-safe for inference.
// Access is serialized by Mutex.
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
            .map(|g| g.model.is_some())
            .unwrap_or(false)
    }
}

impl AneEmbeddingModel {
    /// Create a new ANE embedding model with the given configuration.
    /// Call `load()` before using `embed_text()`.
    pub fn new(config: AneEmbeddingConfig) -> Self {
        Self {
            inner: Mutex::new(Inner {
                model: None,
                tokenizer: None,
                embedding_dim: None,
                max_length: config.max_sequence_length,
            }),
            config,
        }
    }

    /// Load the CoreML model and tokenizer.
    async fn load_model(&self) -> crate::error::Result<()> {
        let mut inner = self.inner.lock().await;
        if inner.model.is_some() {
            return Ok(());
        }

        // Resolve model file via model-loader
        let resolver = ModelResolver::new();
        let model_config = ModelConfig {
            source: self.config.model_source.clone(),
            retry_config: RetryConfig::default(),
            debug: self.config.debug,
        };

        info!("Resolving CoreML model...");
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

        // Load tokenizer — download from HuggingFace if not found locally
        let tokenizer = match load_tokenizer(&model_path) {
            Ok(t) => t,
            Err(_) => {
                // Tokenizer not found near model file; download it from the repo
                if let ModelSource::HuggingFace { ref repo, .. } = self.config.model_source {
                    info!("Downloading tokenizer.json from {}", repo);
                    let retry = RetryConfig::default();
                    let (tok_path, _) =
                        model_loader::load_huggingface_model_with_path(
                            repo,
                            Some("tokenizer.json"),
                            &retry,
                        )
                        .await
                        .map_err(EmbeddingError::ModelLoader)?;
                    Tokenizer::from_file(&tok_path).map_err(|e| {
                        EmbeddingError::tokenization(format!("Failed to load tokenizer: {e}"))
                    })?
                } else {
                    return Err(EmbeddingError::configuration(
                        "tokenizer.json not found near model file",
                    ));
                }
            }
        };

        // Load CoreML model with ANE compute preference
        let opts = CoreMLModelOptions {
            compute_platform: ComputePlatform::CpuAndANE,
            ..Default::default()
        };

        let model_path_str = model_path.to_string_lossy().to_string();
        info!("Loading CoreML model...");
        let coreml_model = CoreMLModelWithState::new(model_path_str, opts);
        let mut coreml_model = coreml_model
            .load()
            .map_err(|e| EmbeddingError::coreml(format!("Failed to load .mlpackage: {e}")))?;

        let desc = coreml_model
            .description()
            .map_err(|e| EmbeddingError::coreml(format!("Failed to get model description: {e}")))?;
        debug!(description = ?desc, "CoreML model loaded");

        // Detect embedding dimension by running a dummy inference
        let seq_len = inner.max_length;
        let dummy_ids = Array2::<f32>::zeros((1, seq_len)).into_dyn();
        let dummy_mask = Array2::<f32>::zeros((1, seq_len)).into_dyn();

        coreml_model
            .add_input("input_ids", dummy_ids)
            .map_err(|e| EmbeddingError::coreml(format!("Failed to add dummy input: {e}")))?;
        coreml_model
            .add_input("attention_mask", dummy_mask)
            .map_err(|e| EmbeddingError::coreml(format!("Failed to add dummy mask: {e}")))?;

        if let Ok(output) = coreml_model.predict() {
            if let Some((_, arr)) = output.outputs.into_iter().find(|(k, _)| k == "embedding") {
                let shape = arr.shape().to_vec();
                let dim = *shape.last().unwrap_or(&0) as usize;
                if dim > 0 {
                    inner.embedding_dim = Some(dim);
                    info!(embedding_dim = dim, "Detected embedding dimension");
                }
            }
        }

        inner.model = Some(coreml_model);
        inner.tokenizer = Some(tokenizer);

        Ok(())
    }

    /// Embed a single text string.
    async fn embed_impl(&self, text: &str) -> crate::error::Result<EmbeddingResult> {
        let start = Instant::now();
        let mut inner = self.inner.lock().await;

        if inner.model.is_none() {
            return Err(EmbeddingError::ModelNotLoaded);
        }

        // Tokenize first (immutable borrow of tokenizer)
        let tokenizer = inner.tokenizer.as_ref().ok_or(EmbeddingError::ModelNotLoaded)?;
        let encoding = tokenizer
            .encode(text, true)
            .map_err(|e| EmbeddingError::tokenization(e.to_string()))?;

        let input_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();
        let max_length = inner.max_length;
        let seq_len = input_ids.len().min(max_length);

        // Prepare tensors — pad or truncate to max_length for static shapes
        // Use f32 inputs: coreml-rs 0.5.4 has a bug in bindInputI32 where it
        // tags i32 data as float32 dataType, corrupting the values.
        let mut ids_padded = vec![0.0f32; max_length];
        let mut mask_padded = vec![0.0f32; max_length];

        for i in 0..seq_len {
            ids_padded[i] = input_ids[i] as f32;
            mask_padded[i] = attention_mask[i] as f32;
        }

        let ids_array = Array2::from_shape_vec((1, max_length), ids_padded)
            .map_err(|e| EmbeddingError::text_processing(format!("Shape error: {e}")))?
            .into_dyn();
        let mask_array = Array2::from_shape_vec((1, max_length), mask_padded)
            .map_err(|e| EmbeddingError::text_processing(format!("Shape error: {e}")))?
            .into_dyn();

        // Now take mutable borrow for model inference
        let model = inner.model.as_mut().unwrap();

        model
            .add_input("input_ids", ids_array)
            .map_err(|e| EmbeddingError::coreml(format!("Failed to add input_ids: {e}")))?;
        model
            .add_input("attention_mask", mask_array)
            .map_err(|e| EmbeddingError::coreml(format!("Failed to add attention_mask: {e}")))?;

        // Run inference — output is already mean-pooled [1, embedding_dim]
        let output = model
            .predict()
            .map_err(|e| EmbeddingError::coreml(format!("Prediction failed: {e}")))?;

        let embedding_array = output
            .outputs
            .into_iter()
            .find(|(k, _)| k == "embedding")
            .ok_or_else(|| EmbeddingError::text_processing("No 'embedding' output found"))?
            .1;

        let embedding = mlarray_to_f32(embedding_array);

        if embedding.is_empty() {
            return Err(EmbeddingError::text_processing("Empty embedding output"));
        }

        // Cache dimension on first real inference
        let dim = embedding.len();
        if inner.embedding_dim.is_none() {
            inner.embedding_dim = Some(dim);
        }

        let mut result = EmbeddingResult::new(
            text.to_string(),
            embedding,
            seq_len,
            start.elapsed().as_millis() as u64,
        );

        if self.config.normalize_embeddings {
            result.normalize();
        }

        Ok(result)
    }
}

/// Extract f32 values from an MLArray, handling both f32 and f16 outputs.
fn mlarray_to_f32(array: coreml_rs::mlarray::MLArray) -> Vec<f32> {
    use coreml_rs::mlarray::MLArray;
    match array {
        MLArray::Float32Array(a) => a.into_raw_vec(),
        MLArray::Float16Array(a) => a.into_raw_vec().iter().map(|v| v.to_f32()).collect(),
        other => {
            tracing::warn!("Unexpected output type, shape: {:?}", other.shape());
            vec![]
        }
    }
}

/// Load a HuggingFace tokenizer from the model directory.
///
/// Looks for `tokenizer.json` in the same directory as the model file,
/// or in the parent directory (for models stored in subdirectories).
fn load_tokenizer(model_path: &std::path::Path) -> crate::error::Result<Tokenizer> {
    let model_dir = model_path
        .parent()
        .ok_or_else(|| EmbeddingError::configuration("Model path has no parent directory"))?;

    // Try model dir first, then parent (for repos with subfolders)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_model() {
        let config = AneEmbeddingConfig::default();
        let model = AneEmbeddingModel::new(config);
        assert!(!model.is_loaded());
        assert_eq!(model.embedding_dimension(), None);
    }
}
