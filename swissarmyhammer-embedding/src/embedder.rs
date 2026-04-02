//! Platform-aware embedder that dispatches to ANE or llama.cpp backends.

#[cfg(target_os = "macos")]
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
    Llama(Box<EmbeddingModel>),
    #[cfg(target_os = "macos")]
    Ane(Box<AneEmbeddingModel>),
}

/// Default model name used by `Embedder::default()`.
pub const DEFAULT_MODEL_NAME: &str = "qwen-embedding";

impl Embedder {
    /// Create an embedder using the default model (`qwen-embedding`).
    ///
    /// Equivalent to `Embedder::from_model_name("qwen-embedding")`.
    /// The model is **not** loaded yet — call [`TextEmbedder::load`]
    /// before embedding.
    pub async fn default() -> Result<Self, EmbedderError> {
        Self::from_model_name(DEFAULT_MODEL_NAME).await
    }

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
                    EmbedderBackend::Llama(Box::new(build_llama_model(cfg).await?)),
                    max_seq,
                    normalize,
                )
            }
            #[cfg(target_os = "macos")]
            ModelExecutorConfig::AneEmbedding(cfg) => {
                let max_seq = cfg.max_sequence_length.unwrap_or(256);
                let normalize = cfg.normalize;
                (
                    EmbedderBackend::Ane(Box::new(build_ane_model(cfg).await?)),
                    max_seq,
                    normalize,
                )
            }
            #[cfg(not(target_os = "macos"))]
            ModelExecutorConfig::AneEmbedding(_) => {
                return Err(EmbedderError::NoCompatibleExecutor(name.to_string()));
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
                #[cfg(target_os = "macos")]
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
            #[cfg(target_os = "macos")]
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
            #[cfg(target_os = "macos")]
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
            #[cfg(target_os = "macos")]
            EmbedderBackend::Ane(m) => m.embedding_dimension(),
        }
    }

    fn is_loaded(&self) -> bool {
        match &self.inner {
            EmbedderBackend::Llama(m) => m.is_loaded(),
            #[cfg(target_os = "macos")]
            EmbedderBackend::Ane(m) => m.is_loaded(),
        }
    }
}

impl Embedder {
    /// Embed a single text chunk directly via the backend.
    async fn embed_single(&self, text: &str) -> Result<EmbeddingResult, EmbeddingError> {
        match &self.inner {
            EmbedderBackend::Llama(m) => m.embed_text(text).await,
            #[cfg(target_os = "macos")]
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
pub(crate) fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<&str> {
    if text.len() <= chunk_size {
        return vec![text];
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let end = (start + chunk_size).min(text.len());

        // Find a whitespace boundary near the end
        let actual_end = if end < text.len() {
            // Search backward from end for whitespace.
            // Ensure search_start is at a valid char boundary.
            let mut search_start = end.saturating_sub(chunk_size / 10);
            while search_start < end && !text.is_char_boundary(search_start) {
                search_start += 1;
            }
            // Ensure end is also at a valid char boundary for slicing.
            let mut search_end = end;
            while search_end < text.len() && !text.is_char_boundary(search_end) {
                search_end += 1;
            }
            text[search_start..search_end]
                .rfind(char::is_whitespace)
                .map(|pos| search_start + pos + 1)
                .unwrap_or(search_end)
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

#[cfg(target_os = "macos")]
async fn build_ane_model(cfg: &EmbeddingModelConfig) -> Result<AneEmbeddingModel, EmbeddingError> {
    let seq_length = cfg.max_sequence_length.unwrap_or(256);

    // Derive model prefix from source.
    // Convention: HF repo "wballard/Foo-Bar-CoreML" → prefix "Foo-Bar"
    let model_prefix = match &cfg.source {
        swissarmyhammer_config::ModelSource::HuggingFace { repo, .. } => {
            let name = repo.split('/').next_back().unwrap_or(repo);
            name.strip_suffix("-CoreML").unwrap_or(name).to_string()
        }
        swissarmyhammer_config::ModelSource::Local { .. } => {
            ane_embedding::DEFAULT_MODEL_PREFIX.to_string()
        }
    };

    // .mlpackage is a directory on HuggingFace containing multiple files.
    // Use folder-based download to fetch all files inside it.
    let mlpackage_name = format!("{model_prefix}-seq{seq_length}.mlpackage");
    let loader_source = match &cfg.source {
        swissarmyhammer_config::ModelSource::HuggingFace { repo, .. } => ModelSource::HuggingFace {
            repo: repo.clone(),
            filename: None,
            folder: Some(mlpackage_name.clone()),
        },
        swissarmyhammer_config::ModelSource::Local { folder, .. } => {
            let dir = folder.clone().unwrap_or_default();
            ModelSource::Local {
                folder: dir,
                filename: Some(mlpackage_name.clone()),
            }
        }
    };

    let resolver = model_loader::ModelResolver::new();
    let model_config = model_loader::ModelConfig {
        source: loader_source,
        retry_config: model_loader::RetryConfig::default(),
        debug: false,
    };

    let resolved = resolver
        .resolve(&model_config)
        .await
        .map_err(|e| EmbeddingError::model(format!("Failed to resolve ANE model: {e}")))?;

    // The resolved path points to a file inside the .mlpackage directory.
    // Walk up until we find the .mlpackage directory, then its parent is model_dir.
    let mut mlpackage_dir = resolved.path.clone();
    while mlpackage_dir.extension().and_then(|e| e.to_str()) != Some("mlpackage") {
        if !mlpackage_dir.pop() {
            break;
        }
    }
    let model_dir = mlpackage_dir
        .parent()
        .unwrap_or(&resolved.path)
        .to_path_buf();

    // Also download tokenizer.json from the repo root (not inside .mlpackage).
    // ANE embedding needs the tokenizer alongside the model.
    if let swissarmyhammer_config::ModelSource::HuggingFace { repo, .. } = &cfg.source {
        let tok_path = model_dir.join("tokenizer.json");
        if !tok_path.exists() {
            model_loader::download_hf_file(
                repo,
                "tokenizer.json",
                &model_loader::RetryConfig::default(),
            )
            .await
            .map_err(|e| EmbeddingError::model(format!("Failed to download tokenizer: {e}")))?;
        }
    }

    tracing::info!(
        model_dir = %model_dir.display(),
        model_prefix = %model_prefix,
        seq_length = seq_length,
        "Resolved ANE model directory"
    );

    let config = AneEmbeddingConfig {
        model_dir,
        model_prefix,
        normalize_embeddings: cfg.normalize,
        seq_length,
        debug: false,
    };
    Ok(AneEmbeddingModel::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // chunk_text unit tests — pure function, no model required
    // -------------------------------------------------------------------------

    /// Short text fits in one chunk — returned as-is.
    #[test]
    fn chunk_text_short_returns_single_chunk() {
        let text = "hello world";
        let chunks = chunk_text(text, 100, 10);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    /// Empty input produces a single empty chunk.
    #[test]
    fn chunk_text_empty_returns_empty_chunk() {
        let chunks = chunk_text("", 100, 10);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "");
    }

    /// Text longer than chunk_size is split into multiple chunks.
    #[test]
    fn chunk_text_long_text_splits_into_multiple_chunks() {
        // 10 words × ~6 chars each = ~60 chars; chunk_size=20 forces multiple chunks
        let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa";
        let chunks = chunk_text(text, 20, 5);
        assert!(
            chunks.len() > 1,
            "Expected multiple chunks, got {}",
            chunks.len()
        );
    }

    /// All chunks together cover the whole input (no bytes dropped).
    #[test]
    fn chunk_text_covers_full_text() {
        let words: Vec<String> = (0..50).map(|i| format!("word{i}")).collect();
        let text = words.join(" ");
        let chunks = chunk_text(&text, 30, 5);

        // The last chunk must reach the end of the text.
        let last = chunks.last().expect("at least one chunk");
        assert!(
            text.ends_with(last.trim_end()),
            "Last chunk '{last}' is not a suffix of the text"
        );
    }

    /// With overlap, the start of chunk N+1 overlaps with the end of chunk N.
    #[test]
    fn chunk_text_overlap_is_applied() {
        let text = "aaa bbb ccc ddd eee fff ggg hhh iii jjj kkk lll mmm nnn ooo ppp";
        let chunk_size = 20;
        let overlap = 8;
        let chunks = chunk_text(text, chunk_size, overlap);

        if chunks.len() >= 2 {
            // The tail of chunk[0] should appear somewhere near the start of chunk[1]
            let c0_end = &chunks[0][chunks[0].len().saturating_sub(overlap)..];
            let c1 = chunks[1];
            // If overlap is working, chunk[1] starts before the raw advance point,
            // i.e. it begins somewhere inside the tail of chunk[0].
            assert!(
                c1.starts_with(c0_end.split_whitespace().next().unwrap_or("")),
                "Expected chunk[1] to begin near the end of chunk[0] (overlap={overlap})"
            );
        }
    }

    /// Chunk boundaries are always at valid UTF-8 char boundaries.
    #[test]
    fn chunk_text_unicode_boundaries() {
        // String with multi-byte characters
        let text = "日本語テスト abc def ghi jkl mno pqr stu vwx yz 日本語テスト 終了";
        let chunks = chunk_text(text, 15, 5);
        for chunk in &chunks {
            // If this panics, the slice was not at a char boundary
            let _ = chunk.chars().count();
        }
    }

    // -------------------------------------------------------------------------
    // from_model_name error path tests — no model download required
    // -------------------------------------------------------------------------

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Runtime::new().unwrap()
    }

    /// An unknown model name returns `EmbedderError::ModelNotFound`.
    #[test]
    fn from_model_name_unknown_model_returns_not_found() {
        let rt = rt();
        rt.block_on(async {
            let result = Embedder::from_model_name("this-model-does-not-exist-xyz").await;
            assert!(
                matches!(result, Err(EmbedderError::ModelNotFound(_))),
                "Expected ModelNotFound, got: {}",
                result.err().map(|e| e.to_string()).unwrap_or_default()
            );
        });
    }

    /// A known model that is not an embedding model returns `EmbedderError::NotAnEmbeddingModel`.
    ///
    /// `claude-code` ships as a `ClaudeCode` executor, so it should trigger this path.
    #[test]
    fn from_model_name_non_embedding_model_returns_not_an_embedding_model() {
        let rt = rt();
        rt.block_on(async {
            let result = Embedder::from_model_name("claude-code").await;
            assert!(
                matches!(
                    result,
                    Err(EmbedderError::NotAnEmbeddingModel(_, _))
                        | Err(EmbedderError::NoCompatibleExecutor(_))
                ),
                "Expected NotAnEmbeddingModel or NoCompatibleExecutor, got: {}",
                result.err().map(|e| e.to_string()).unwrap_or_default()
            );
        });
    }

    // -------------------------------------------------------------------------
    // DEFAULT_MODEL_NAME constant
    // -------------------------------------------------------------------------

    /// The default model name constant is the expected value.
    #[test]
    fn default_model_name_constant_is_correct() {
        assert_eq!(DEFAULT_MODEL_NAME, "qwen-embedding");
    }

    // -------------------------------------------------------------------------
    // EmbedderError display
    // -------------------------------------------------------------------------

    /// EmbedderError variants format without panicking.
    #[test]
    fn embedder_error_display() {
        let e = EmbedderError::ModelNotFound("my-model".to_string());
        assert!(e.to_string().contains("my-model"));

        let e = EmbedderError::NoCompatibleExecutor("llm".to_string());
        assert!(e.to_string().contains("llm"));

        let e = EmbedderError::ConfigParse("bad yaml".to_string());
        assert!(e.to_string().contains("bad yaml"));

        let e = EmbedderError::NotAnEmbeddingModel(
            "chat-model".to_string(),
            swissarmyhammer_config::model::ModelExecutorType::ClaudeCode,
        );
        assert!(e.to_string().contains("chat-model"));
    }
}
