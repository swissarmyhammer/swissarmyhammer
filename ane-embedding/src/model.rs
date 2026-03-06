use std::time::Instant;

use async_trait::async_trait;
use coreml_rs::{ComputePlatform, CoreMLModelOptions, CoreMLModelWithState};
use model_embedding::{EmbeddingResult, TextEmbedder};
use ndarray::{Array2, ArrayD};
use tokenizers::Tokenizer;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::error::EmbeddingError;
use crate::types::AneEmbeddingConfig;

struct Inner {
    model: Option<CoreMLModelWithState>,
    tokenizer: Option<Tokenizer>,
    embedding_dim: Option<usize>,
}

/// Text embedding model using CoreML for Apple Neural Engine.
///
/// Loads a single static-shape FP16 `.mlpackage` for a fixed sequence length.
/// Inputs are padded or truncated to fit. Mean pooling is baked into the model.
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
            }),
            config,
        }
    }

    /// The fixed sequence length for this model.
    pub fn seq_length(&self) -> usize {
        self.config.seq_length
    }

    /// Load the CoreML model and tokenizer.
    async fn load_model(&self) -> crate::error::Result<()> {
        let mut inner = self.inner.lock().await;
        if inner.model.is_some() {
            return Ok(());
        }

        // Load tokenizer
        let tok_path = self.config.tokenizer_path();
        info!(path = %tok_path.display(), "Loading tokenizer");
        let tokenizer = Tokenizer::from_file(&tok_path).map_err(|e| {
            EmbeddingError::tokenization(format!(
                "Failed to load tokenizer at {}: {e}",
                tok_path.display()
            ))
        })?;

        // Load CoreML model
        let model_path = self.config.model_path();
        info!(
            seq_length = self.config.seq_length,
            path = %model_path.display(),
            "Loading CoreML model"
        );

        if !model_path.exists() {
            return Err(EmbeddingError::configuration(format!(
                "Model not found: {}",
                model_path.display()
            )));
        }

        let opts = CoreMLModelOptions {
            compute_platform: ComputePlatform::CpuAndANE,
            ..Default::default()
        };

        let model_path_str = model_path.to_string_lossy().to_string();
        let coreml_model = CoreMLModelWithState::new(model_path_str, opts);
        let mut coreml_model = coreml_model
            .load()
            .map_err(|e| EmbeddingError::coreml(format!("Failed to load .mlpackage: {e}")))?;

        // Detect embedding dimension with a dummy inference
        let sl = self.config.seq_length;
        let dummy_ids = Array2::<i32>::zeros((1, sl)).into_dyn();
        let dummy_mask = Array2::<i32>::zeros((1, sl)).into_dyn();

        coreml_model
            .add_input("input_ids", dummy_ids)
            .map_err(|e| EmbeddingError::coreml(format!("Failed to add dummy input: {e}")))?;
        coreml_model
            .add_input("attention_mask", dummy_mask)
            .map_err(|e| EmbeddingError::coreml(format!("Failed to add dummy mask: {e}")))?;

        if let Ok(output) = coreml_model.predict() {
            if let Some((_, arr)) = output.outputs.into_iter().find(|(k, _)| k == "embedding") {
                let dim = *arr.shape().last().unwrap_or(&0) as usize;
                if dim > 0 {
                    inner.embedding_dim = Some(dim);
                    info!(embedding_dim = dim, "Detected embedding dimension");
                }
            }
        }

        inner.model = Some(coreml_model);
        inner.tokenizer = Some(tokenizer);

        info!(
            seq_length = self.config.seq_length,
            "CoreML model ready"
        );

        Ok(())
    }

    /// Embed a single text string. Pads or truncates to the fixed seq_length.
    async fn embed_impl(&self, text: &str) -> crate::error::Result<EmbeddingResult> {
        let start = Instant::now();
        let mut inner = self.inner.lock().await;
        let t_lock = start.elapsed();

        if inner.model.is_none() {
            return Err(EmbeddingError::ModelNotLoaded);
        }

        // Tokenize
        let t0 = Instant::now();
        let tokenizer = inner
            .tokenizer
            .as_ref()
            .ok_or(EmbeddingError::ModelNotLoaded)?;
        let encoding = tokenizer
            .encode(text, true)
            .map_err(|e| EmbeddingError::tokenization(e.to_string()))?;

        let input_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();
        let token_count = input_ids.len();
        let seq_len = token_count.min(self.config.seq_length);
        let t_tokenize = t0.elapsed();

        // Prepare tensors — pad or truncate to fixed seq_length
        let t0 = Instant::now();
        let padded_len = self.config.seq_length;
        let mut ids_padded = vec![0i32; padded_len];
        let mut mask_padded = vec![0i32; padded_len];

        for i in 0..seq_len {
            ids_padded[i] = input_ids[i] as i32;
            mask_padded[i] = attention_mask[i] as i32;
        }

        let ids_array: ArrayD<i32> = Array2::from_shape_vec((1, padded_len), ids_padded)
            .map_err(|e| EmbeddingError::text_processing(format!("Shape error: {e}")))?
            .into_dyn();
        let mask_array: ArrayD<i32> = Array2::from_shape_vec((1, padded_len), mask_padded)
            .map_err(|e| EmbeddingError::text_processing(format!("Shape error: {e}")))?
            .into_dyn();
        let t_tensors = t0.elapsed();

        let model = inner.model.as_mut().unwrap();

        let t0 = Instant::now();
        model
            .add_input("input_ids", ids_array)
            .map_err(|e| EmbeddingError::coreml(format!("Failed to add input_ids: {e}")))?;
        model
            .add_input("attention_mask", mask_array)
            .map_err(|e| EmbeddingError::coreml(format!("Failed to add attention_mask: {e}")))?;
        let t_add_input = t0.elapsed();

        // Run inference
        let t0 = Instant::now();
        let output = model
            .predict()
            .map_err(|e| EmbeddingError::coreml(format!("Prediction failed: {e}")))?;
        let t_predict = t0.elapsed();

        let t0 = Instant::now();
        let embedding_array = output
            .outputs
            .into_iter()
            .find(|(k, _)| k == "embedding")
            .ok_or_else(|| EmbeddingError::text_processing("No 'embedding' output found"))?
            .1;

        let embedding = mlarray_to_f32(embedding_array);
        let t_extract = t0.elapsed();

        if embedding.is_empty() {
            return Err(EmbeddingError::text_processing("Empty embedding output"));
        }

        let dim = embedding.len();
        if inner.embedding_dim.is_none() {
            inner.embedding_dim = Some(dim);
        }

        let total = start.elapsed();
        debug!(
            tokens = token_count,
            seq_len = seq_len,
            lock_us = t_lock.as_micros(),
            tokenize_us = t_tokenize.as_micros(),
            tensors_us = t_tensors.as_micros(),
            add_input_us = t_add_input.as_micros(),
            predict_us = t_predict.as_micros(),
            extract_us = t_extract.as_micros(),
            total_us = total.as_micros(),
            "embed_impl timing breakdown"
        );

        let mut result = EmbeddingResult::new(
            text.to_string(),
            embedding,
            seq_len,
            total.as_millis() as u64,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_model() {
        let config = AneEmbeddingConfig::default();
        let model = AneEmbeddingModel::new(config);
        assert!(!model.is_loaded());
        assert_eq!(model.embedding_dimension(), None);
        assert_eq!(model.seq_length(), 128);
    }
}
