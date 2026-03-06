//! Platform-aware embedder that dispatches to ANE or llama.cpp backends.

use ane_embedding::{AneEmbeddingConfig, AneEmbeddingModel};
use llama_embedding::{EmbeddingConfig, EmbeddingModel};
use model_embedding::{EmbeddingError, EmbeddingResult, TextEmbedder};
use model_loader::ModelSource;
use swissarmyhammer_config::model::{
    EmbeddingModelConfig, ModelExecutorConfig, ModelExecutorType, ModelManager,
};
use swissarmyhammer_config::parse_model_config;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EmbedderError {
    #[error("model '{0}' not found in builtin or user configs")]
    ModelNotFound(String),
    #[error("model '{0}' has no embedding executor for this platform")]
    NoCompatibleExecutor(String),
    #[error("model '{0}' is not an embedding model (executor type: {1:?})")]
    NotAnEmbeddingModel(String, ModelExecutorType),
    #[error("config parse error: {0}")]
    ConfigParse(String),
    #[error("embedding error: {0}")]
    Embedding(#[from] EmbeddingError),
}

/// A unified embedder that resolves a named model, picks the right backend
/// for the current platform, and handles download + loading.
///
/// When text exceeds `max_sequence_length`, the embedder automatically splits
/// it into overlapping chunks, embeds each, and mean-pools the results.
pub struct Embedder {
    inner: EmbedderBackend,
    model_name: String,
    /// Max tokens the backend model supports. Used for chunk-pooling.
    max_sequence_length: usize,
    /// Whether to L2-normalize the final embedding.
    normalize: bool,
}

enum EmbedderBackend {
    Llama(EmbeddingModel),
    Ane(AneEmbeddingModel),
}

impl Embedder {
    /// Create an embedder from a builtin or user model name.
    ///
    /// Resolves the name via `ModelManager`, selects the first executor
    /// compatible with the current platform, and constructs the appropriate
    /// backend. The model is **not** loaded yet — call [`TextEmbedder::load`]
    /// before embedding.
    pub async fn from_model_name(name: &str) -> Result<Self, EmbedderError> {
        let model_info = ModelManager::find_agent_by_name(name)
            .map_err(|_| EmbedderError::ModelNotFound(name.to_string()))?;

        let config = parse_model_config(&model_info.content)
            .map_err(|e| EmbedderError::ConfigParse(e.to_string()))?;

        let executor = config
            .select_executor()
            .ok_or_else(|| EmbedderError::NoCompatibleExecutor(name.to_string()))?;

        let (backend, max_seq, normalize) = match executor {
            ModelExecutorConfig::LlamaEmbedding(cfg) => {
                let max_seq = cfg.max_sequence_length.unwrap_or(512);
                let normalize = cfg.normalize;
                (
                    EmbedderBackend::Llama(build_llama_model(cfg).await?),
                    max_seq,
                    normalize,
                )
            }
            ModelExecutorConfig::AneEmbedding(cfg) => {
                let max_seq = cfg.max_sequence_length.unwrap_or(128);
                let normalize = cfg.normalize;
                (
                    EmbedderBackend::Ane(build_ane_model(cfg)?),
                    max_seq,
                    normalize,
                )
            }
            other => {
                let exec_type = match other {
                    ModelExecutorConfig::ClaudeCode(_) => ModelExecutorType::ClaudeCode,
                    ModelExecutorConfig::LlamaAgent(_) => ModelExecutorType::LlamaAgent,
                    _ => unreachable!(),
                };
                return Err(EmbedderError::NotAnEmbeddingModel(
                    name.to_string(),
                    exec_type,
                ));
            }
        };

        tracing::info!(
            "Created {} embedder for model '{}' (max_seq={}, normalize={})",
            match &backend {
                EmbedderBackend::Llama(_) => "llama",
                EmbedderBackend::Ane(_) => "ane",
            },
            name,
            max_seq,
            normalize,
        );

        Ok(Self {
            inner: backend,
            model_name: name.to_string(),
            max_sequence_length: max_seq,
            normalize,
        })
    }

    /// Which backend was selected.
    pub fn backend_name(&self) -> &'static str {
        match &self.inner {
            EmbedderBackend::Llama(_) => "llama",
            EmbedderBackend::Ane(_) => "ane",
        }
    }

    /// The resolved model config name.
    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    /// Max sequence length the model supports.
    pub fn max_sequence_length(&self) -> usize {
        self.max_sequence_length
    }
}

// Delegate TextEmbedder to whichever backend was selected.
impl model_embedding::private::Sealed for Embedder {}

#[async_trait::async_trait]
impl TextEmbedder for Embedder {
    async fn load(&self) -> Result<(), EmbeddingError> {
        match &self.inner {
            EmbedderBackend::Llama(m) => m.load().await,
            EmbedderBackend::Ane(m) => m.load().await,
        }
    }

    async fn embed_text(&self, text: &str) -> Result<EmbeddingResult, EmbeddingError> {
        // Estimate token count (~4 chars per token is conservative for English).
        // If the text likely fits in one model call, skip chunking.
        let approx_tokens = text.len() / 3;
        if approx_tokens <= self.max_sequence_length {
            return self.embed_single(text).await;
        }

        // Text is likely too long — chunk and pool.
        self.embed_chunked(text).await
    }

    fn embedding_dimension(&self) -> Option<usize> {
        match &self.inner {
            EmbedderBackend::Llama(m) => m.embedding_dimension(),
            EmbedderBackend::Ane(m) => m.embedding_dimension(),
        }
    }

    fn is_loaded(&self) -> bool {
        match &self.inner {
            EmbedderBackend::Llama(m) => m.is_loaded(),
            EmbedderBackend::Ane(m) => m.is_loaded(),
        }
    }
}

impl Embedder {
    /// Embed a single text chunk directly via the backend.
    async fn embed_single(&self, text: &str) -> Result<EmbeddingResult, EmbeddingError> {
        match &self.inner {
            EmbedderBackend::Llama(m) => m.embed_text(text).await,
            EmbedderBackend::Ane(m) => m.embed_text(text).await,
        }
    }

    /// Split long text into overlapping chunks, embed each, and mean-pool.
    async fn embed_chunked(&self, text: &str) -> Result<EmbeddingResult, EmbeddingError> {
        let start = std::time::Instant::now();

        // Approximate chars per chunk. Use ~3 chars/token (conservative)
        // to avoid exceeding the token limit after tokenization.
        let chars_per_chunk = self.max_sequence_length * 3;
        let overlap_chars = chars_per_chunk / 4; // 25% overlap

        let chunks = chunk_text(text, chars_per_chunk, overlap_chars);
        tracing::debug!(
            chunks = chunks.len(),
            max_seq = self.max_sequence_length,
            text_len = text.len(),
            "Chunk-pooling long text"
        );

        if chunks.is_empty() {
            return self.embed_single(text).await;
        }

        // Embed each chunk
        let mut embeddings: Vec<Vec<f32>> = Vec::with_capacity(chunks.len());
        for chunk in &chunks {
            let result = self.embed_single(chunk).await?;
            embeddings.push(result.embedding().to_vec());
        }

        // Mean-pool: average all chunk embeddings element-wise
        let dim = embeddings[0].len();
        let mut pooled = vec![0.0f32; dim];
        let n = embeddings.len() as f32;
        for emb in &embeddings {
            for (i, &v) in emb.iter().enumerate() {
                pooled[i] += v;
            }
        }
        for v in &mut pooled {
            *v /= n;
        }

        // Normalize the pooled result if configured
        if self.normalize {
            let magnitude: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
            if magnitude > 0.0 {
                for v in &mut pooled {
                    *v /= magnitude;
                }
            }
        }

        let total_ms = start.elapsed().as_millis() as u64;
        Ok(EmbeddingResult::new(
            text.to_string(),
            pooled,
            text.len(), // approximate "sequence length" for the full text
            total_ms,
        ))
    }
}

/// Split text into chunks of approximately `chunk_size` characters with overlap.
/// Splits on whitespace boundaries to avoid breaking words.
fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<&str> {
    if text.len() <= chunk_size {
        return vec![text];
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let end = (start + chunk_size).min(text.len());

        // Find a whitespace boundary near the end
        let actual_end = if end < text.len() {
            // Search backward from end for whitespace
            let search_start = end.saturating_sub(chunk_size / 10);
            text[search_start..end]
                .rfind(char::is_whitespace)
                .map(|pos| search_start + pos + 1)
                .unwrap_or(end)
        } else {
            end
        };

        // Ensure we're at a valid char boundary
        let actual_end = if actual_end < text.len() {
            let mut e = actual_end;
            while e < text.len() && !text.is_char_boundary(e) {
                e += 1;
            }
            e
        } else {
            text.len()
        };

        chunks.push(&text[start..actual_end]);

        if actual_end >= text.len() {
            break;
        }

        // Advance with overlap
        let advance = actual_end.saturating_sub(start).saturating_sub(overlap);
        start += advance.max(1);

        // Ensure start is at a char boundary
        while start < text.len() && !text.is_char_boundary(start) {
            start += 1;
        }
    }

    chunks
}

// ---------------------------------------------------------------------------
// Backend construction helpers
// ---------------------------------------------------------------------------

/// Convert the config-crate `ModelSource` to the model-loader `ModelSource`.
fn convert_source(src: &swissarmyhammer_config::ModelSource) -> ModelSource {
    match src {
        swissarmyhammer_config::ModelSource::HuggingFace {
            repo,
            filename,
            folder,
        } => ModelSource::HuggingFace {
            repo: repo.clone(),
            filename: filename.clone(),
            folder: folder.clone(),
        },
        swissarmyhammer_config::ModelSource::Local { filename, folder } => ModelSource::Local {
            folder: folder.clone().unwrap_or_default(),
            filename: Some(filename.to_string_lossy().to_string()),
        },
    }
}

async fn build_llama_model(cfg: &EmbeddingModelConfig) -> Result<EmbeddingModel, EmbeddingError> {
    let config = EmbeddingConfig {
        model_source: convert_source(&cfg.source),
        normalize_embeddings: cfg.normalize,
        max_sequence_length: cfg.max_sequence_length,
        debug: false,
    };
    EmbeddingModel::new(config)
        .await
        .map_err(|e| EmbeddingError::model(e.to_string()))
}

fn build_ane_model(cfg: &EmbeddingModelConfig) -> Result<AneEmbeddingModel, EmbeddingError> {
    // Resolve model directory from the source
    let model_dir = match &cfg.source {
        swissarmyhammer_config::ModelSource::HuggingFace { repo, .. } => {
            // Use HuggingFace cache directory
            let cache_dir = dirs::cache_dir()
                .unwrap_or_else(|| std::path::PathBuf::from(".cache"))
                .join("huggingface")
                .join("hub")
                .join(format!("models--{}", repo.replace('/', "--")));
            // Snapshot directory (latest)
            let snapshots = cache_dir.join("snapshots");
            if snapshots.exists() {
                // Use the first (latest) snapshot
                std::fs::read_dir(&snapshots)
                    .ok()
                    .and_then(|mut d| d.next())
                    .and_then(|e| e.ok())
                    .map(|e| e.path())
                    .unwrap_or(cache_dir)
            } else {
                cache_dir
            }
        }
        swissarmyhammer_config::ModelSource::Local { filename, folder } => {
            folder.clone().unwrap_or_else(|| filename.parent().unwrap_or(std::path::Path::new(".")).to_path_buf())
        }
    };

    let seq_length = cfg.max_sequence_length.unwrap_or(128);

    let config = AneEmbeddingConfig {
        model_dir,
        model_prefix: ane_embedding::DEFAULT_MODEL_PREFIX.to_string(),
        normalize_embeddings: cfg.normalize,
        seq_length,
        debug: false,
    };
    Ok(AneEmbeddingModel::new(config))
}
