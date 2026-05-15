use std::time::Instant;

use async_trait::async_trait;
use model_embedding::{EmbeddingResult, TextEmbedder};
use tokenizers::Tokenizer;
use tokio::sync::Mutex;
use tracing::{info, trace};

use crate::coreml::CoreMLModel;
use crate::error::EmbeddingError;
use crate::types::AneEmbeddingConfig;

struct Inner {
    model: Option<CoreMLModel>,
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

// Safety: CoreMLModel wraps an MLModel which is thread-safe for prediction per Apple docs.
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

        let coreml_model = CoreMLModel::load(&model_path)?;

        // Detect embedding dimension from model description
        if let Ok(Some(dim)) = coreml_model.embedding_dim() {
            inner.embedding_dim = Some(dim);
            info!(embedding_dim = dim, "Detected embedding dimension");
        }

        inner.model = Some(coreml_model);
        inner.tokenizer = Some(tokenizer);

        info!(seq_length = self.config.seq_length, "CoreML model ready");

        Ok(())
    }

    /// Embed a single text string. Pads or truncates to the fixed seq_length.
    async fn embed_impl(&self, text: &str) -> crate::error::Result<EmbeddingResult> {
        let start = Instant::now();
        let inner = self.inner.lock().await;
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
        let t_tensors = t0.elapsed();

        let model = inner.model.as_ref().unwrap();

        // Run inference
        let t0 = Instant::now();
        let output = model.predict_embedding(&ids_padded, &mask_padded, padded_len)?;
        let t_predict = t0.elapsed();

        let embedding = output.embedding;

        if embedding.is_empty() {
            return Err(EmbeddingError::text_processing("Empty embedding output"));
        }

        let total = start.elapsed();
        trace!(
            tokens = token_count,
            seq_len = seq_len,
            lock_us = t_lock.as_micros(),
            tokenize_us = t_tokenize.as_micros(),
            tensors_us = t_tensors.as_micros(),
            predict_us = t_predict.as_micros(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_model() {
        let config = AneEmbeddingConfig::default();
        let model = AneEmbeddingModel::new(config);
        assert!(!model.is_loaded());
        assert_eq!(model.embedding_dimension(), None);
        assert_eq!(model.seq_length(), 256);
    }

    #[test]
    fn test_seq_length_custom() {
        let config = AneEmbeddingConfig {
            seq_length: 128,
            ..AneEmbeddingConfig::default()
        };
        let model = AneEmbeddingModel::new(config);
        assert_eq!(model.seq_length(), 128);
    }

    #[test]
    fn test_is_loaded_returns_false_before_load() {
        let model = AneEmbeddingModel::new(AneEmbeddingConfig::default());
        assert!(!model.is_loaded());
    }

    #[test]
    fn test_embedding_dimension_none_before_load() {
        let model = AneEmbeddingModel::new(AneEmbeddingConfig::default());
        assert_eq!(model.embedding_dimension(), None);
    }

    #[tokio::test]
    async fn test_embed_text_before_load_returns_model_not_loaded() {
        let model = AneEmbeddingModel::new(AneEmbeddingConfig::default());
        let result = model.embed_impl("hello").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, EmbeddingError::ModelNotLoaded),
            "Expected ModelNotLoaded, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_load_fails_with_missing_tokenizer() {
        // Use a temp dir with no files — tokenizer load will fail
        let tmp = tempfile::tempdir().unwrap();
        let config = AneEmbeddingConfig {
            model_dir: tmp.path().to_path_buf(),
            ..AneEmbeddingConfig::default()
        };
        let model = AneEmbeddingModel::new(config);
        let result = model.load_model().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, EmbeddingError::Tokenization(_)),
            "Expected Tokenization error for missing tokenizer, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_load_fails_with_missing_model_file() {
        // Create a temp dir with a valid tokenizer.json but no .mlpackage
        let tmp = tempfile::tempdir().unwrap();
        let tok_path = tmp.path().join("tokenizer.json");
        // Write a minimal valid tokenizer JSON (from HuggingFace tokenizers format)
        std::fs::write(
            &tok_path,
            r#"{
                "version": "1.0",
                "model": {
                    "type": "BPE",
                    "vocab": {"a": 0, "b": 1},
                    "merges": []
                }
            }"#,
        )
        .unwrap();

        let config = AneEmbeddingConfig {
            model_dir: tmp.path().to_path_buf(),
            ..AneEmbeddingConfig::default()
        };
        let model = AneEmbeddingModel::new(config);
        let result = model.load_model().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, EmbeddingError::Configuration(_)),
            "Expected Configuration error for missing model, got: {err}"
        );
        assert!(
            err.to_string().contains("not found"),
            "Error should mention model not found: {err}"
        );
    }

    #[tokio::test]
    async fn test_trait_embed_text_before_load() {
        let model = AneEmbeddingModel::new(AneEmbeddingConfig::default());
        let result = TextEmbedder::embed_text(&model, "hello").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            model_embedding::EmbeddingError::ModelNotLoaded
        ));
    }

    #[tokio::test]
    async fn test_trait_load_with_bad_path() {
        let config = AneEmbeddingConfig {
            model_dir: std::path::PathBuf::from("/nonexistent/path/to/model"),
            ..AneEmbeddingConfig::default()
        };
        let model = AneEmbeddingModel::new(config);
        let result = TextEmbedder::load(&model).await;
        assert!(result.is_err());
    }
}
