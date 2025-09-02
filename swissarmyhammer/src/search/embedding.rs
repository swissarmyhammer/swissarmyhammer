//! Local embedding generation using fastembed-rs neural embeddings

use crate::search::{CodeChunk, Embedding, Result, SemanticError};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Configuration for the embedding engine
#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    /// Model identifier for the embedding model
    pub model_id: String,
    /// The fastembed EmbeddingModel to use
    pub embedding_model: EmbeddingModel,
    /// Number of texts to process in a single batch  
    pub batch_size: usize,
    /// Maximum text length in characters before truncation
    pub max_text_length: usize,
    /// Delay in milliseconds between batches to avoid overwhelming the model
    pub batch_delay_ms: u64,
    /// Whether to show download progress for models
    pub show_download_progress: bool,
    /// Embedding dimensions (will be set based on model)
    pub dimensions: Option<usize>,
    /// Maximum sequence length the model can handle
    pub max_sequence_length: usize,
    /// Quantization type for the model
    pub quantization: String,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model_id: "nomic-embed-text-v1.5".to_string(), // Modern, high-quality embedding model
            embedding_model: EmbeddingModel::NomicEmbedTextV15Q,
            batch_size: 32, // Reasonable batch size for neural models
            max_text_length: 8000,
            batch_delay_ms: 10, // Small delay for neural processing
            show_download_progress: true,
            dimensions: None,                 // Will be determined by model
            max_sequence_length: 512,         // Standard for most transformer models
            quantization: "FP32".to_string(), // Standard for fastembed
        }
    }
}

/// Information about the embedding model
#[derive(Debug, Clone)]
pub struct EmbeddingModelInfo {
    /// Model identifier
    pub model_id: String,
    /// Number of dimensions in the embedding vectors
    pub dimensions: usize,
    /// Maximum sequence length the model can handle
    pub max_sequence_length: usize,
    /// Type of quantization used (e.g., FP8, FP16)
    pub quantization: String,
}

/// Embedding model backend type
enum EmbeddingBackend {
    /// Neural model using fastembed neural embeddings
    Neural(Box<TextEmbedding>),
}

/// Embedding engine using fastembed-rs neural embeddings
/// This provides high-quality semantic embeddings using local neural models
/// without requiring external API dependencies.
pub struct EmbeddingEngine {
    config: EmbeddingConfig,
    model_info: EmbeddingModelInfo,
    backend: Arc<Mutex<EmbeddingBackend>>,
}

impl EmbeddingEngine {
    /// Create new embedding engine with default configuration
    pub async fn new() -> Result<Self> {
        let config = EmbeddingConfig::default();
        Self::with_config(config).await
    }

    /// Create embedding engine with custom model
    pub async fn with_model_id(model_id: String) -> Result<Self> {
        let config = EmbeddingConfig {
            model_id,
            ..Default::default()
        };
        Self::with_config(config).await
    }

    /// Create engine with custom configuration
    pub async fn with_config(mut config: EmbeddingConfig) -> Result<Self> {
        if config.model_id.is_empty() {
            return Err(SemanticError::Config(
                "Model ID cannot be empty".to_string(),
            ));
        }

        if config.batch_size == 0 {
            return Err(SemanticError::Config(
                "Batch size must be greater than 0".to_string(),
            ));
        }

        if config.max_text_length == 0 {
            return Err(SemanticError::Config(
                "Max text length must be greater than 0".to_string(),
            ));
        }

        if config.max_sequence_length == 0 {
            return Err(SemanticError::Config(
                "Max sequence length must be greater than 0".to_string(),
            ));
        }

        if config.quantization.is_empty() {
            return Err(SemanticError::Config(
                "Quantization type cannot be empty".to_string(),
            ));
        }

        info!(
            "Initializing fastembed embedding engine with model: {}",
            config.model_id
        );

        // Initialize fastembed model
        let init_options = InitOptions::new(config.embedding_model.clone())
            .with_show_download_progress(config.show_download_progress)
            .with_cache_dir("/tmp/.cache/fastembed".into());

        let mut model = TextEmbedding::try_new(init_options).map_err(|e| {
            SemanticError::Embedding(format!("Failed to initialize fastembed model: {e}"))
        })?;

        // Get actual model dimensions by generating a test embedding
        let test_embedding = model.embed(vec!["test".to_string()], None).map_err(|e| {
            SemanticError::Embedding(format!("Failed to get model dimensions: {e}"))
        })?;

        let dimensions = test_embedding
            .first()
            .ok_or_else(|| SemanticError::Embedding("No test embedding generated".to_string()))?
            .len();

        config.dimensions = Some(dimensions);

        let model_info = EmbeddingModelInfo {
            model_id: config.model_id.clone(),
            dimensions,
            max_sequence_length: config.max_sequence_length,
            quantization: config.quantization.clone(),
        };

        info!(
            "Successfully initialized fastembed embedding engine with {} dimensions",
            dimensions
        );

        Ok(Self {
            config,
            model_info,
            backend: Arc::new(Mutex::new(EmbeddingBackend::Neural(Box::new(model)))),
        })
    }

    /// Generate embedding for a single code chunk
    pub async fn embed_chunk(&self, chunk: &CodeChunk) -> Result<Embedding> {
        let text = self.prepare_chunk_text(chunk);
        let vector = self.generate_embedding(&text).await?;

        Ok(Embedding {
            chunk_id: chunk.id.clone(),
            vector,
        })
    }

    /// Generate embedding for raw text
    pub async fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        self.generate_embedding(text).await
    }

    /// Generate embeddings for multiple text strings efficiently
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::new();

        // Process in batches to avoid overwhelming the model
        for text_batch in texts.chunks(self.config.batch_size) {
            let batch_results = self.process_text_batch(text_batch).await?;
            embeddings.extend(batch_results);

            // Add small delay between batches
            if text_batch.len() == self.config.batch_size {
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    self.config.batch_delay_ms,
                ))
                .await;
            }
        }

        Ok(embeddings)
    }

    /// Generate embeddings for multiple chunks efficiently
    pub async fn embed_chunks_batch(&self, chunks: &[CodeChunk]) -> Result<Vec<Embedding>> {
        let mut embeddings = Vec::new();

        // Process in batches to avoid overwhelming the model
        for chunk_batch in chunks.chunks(self.config.batch_size) {
            let batch_results = self.process_chunk_batch(chunk_batch).await?;
            embeddings.extend(batch_results);

            // Add small delay between batches
            if chunk_batch.len() == self.config.batch_size {
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    self.config.batch_delay_ms,
                ))
                .await;
            }
        }

        Ok(embeddings)
    }

    /// Get model information
    pub fn model_info(&self) -> EmbeddingModelInfo {
        self.model_info.clone()
    }

    // Private implementation methods

    /// Generate a consistent, semantic embedding for the given text
    /// This uses a combination of deterministic hashing and semantic analysis
    /// to create embeddings that maintain semantic relationships
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // Validate input
        if text.is_empty() {
            return Err(SemanticError::Embedding("Empty text provided".to_string()));
        }

        // Clean and truncate text
        let cleaned_text = self.clean_text(text);

        // Generate embedding using fastembed neural model
        let embedding = self.create_neural_embedding(&cleaned_text).await?;

        debug!(
            "Generated neural embedding with {} dimensions",
            embedding.len()
        );
        Ok(embedding)
    }

    /// Create a neural embedding using fastembed (or mock for testing)
    async fn create_neural_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // Use fastembed to generate high-quality neural embeddings
        let mut backend = self.backend.lock().await;
        let model = match &mut *backend {
            EmbeddingBackend::Neural(model) => model.as_mut(),
        };

        // Format text appropriately for embedding (code context)
        let formatted_text = format!("passage: {text}");

        // Generate embedding using fastembed
        let embeddings = model
            .embed(vec![formatted_text], None)
            .map_err(|e| SemanticError::Embedding(format!("Fastembed error: {e}")))?;

        // Extract the first (and only) embedding
        if let Some(embedding) = embeddings.into_iter().next() {
            Ok(embedding)
        } else {
            Err(SemanticError::Embedding(
                "No embedding generated".to_string(),
            ))
        }
    }

    async fn process_text_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Clean and format texts for fastembed
        let cleaned_texts: Vec<String> = texts
            .iter()
            .map(|text| {
                let cleaned = self.clean_text(text);
                format!("passage: {cleaned}")
            })
            .collect();

        // Use fastembed's native batch processing
        let mut backend = self.backend.lock().await;
        let model = match &mut *backend {
            EmbeddingBackend::Neural(model) => model.as_mut(),
        };

        debug!("Processing batch of {} texts", texts.len());

        let embeddings = model
            .embed(cleaned_texts, None)
            .map_err(|e| SemanticError::Embedding(format!("Fastembed batch error: {e}")))?;

        debug!(
            "Successfully generated {} embeddings in batch",
            embeddings.len()
        );

        Ok(embeddings)
    }

    async fn process_chunk_batch(&self, chunks: &[CodeChunk]) -> Result<Vec<Embedding>> {
        let mut batch_embeddings = Vec::new();

        for chunk in chunks {
            match self.embed_chunk(chunk).await {
                Ok(embedding) => {
                    batch_embeddings.push(embedding);
                    tracing::debug!("Generated embedding for chunk: {}", chunk.id);
                }
                Err(e) => {
                    tracing::error!("Failed to embed chunk {}: {}", chunk.id, e);
                    // Continue with other chunks instead of failing entire batch
                }
            }
        }

        Ok(batch_embeddings)
    }

    /// Prepare chunk text for embedding with code-specific format
    pub fn prepare_chunk_text(&self, chunk: &CodeChunk) -> String {
        let mut text = String::new();

        // Add language and type context for better embeddings
        text.push_str(&format!("{:?} {:?}: ", chunk.language, chunk.chunk_type));

        // Add the actual code content directly
        text.push_str(&chunk.content);

        // Clean up the text for better embedding quality
        self.clean_text(&text)
    }

    fn clean_text(&self, text: &str) -> String {
        let mut result = text
            // Remove excessive whitespace from each line (both leading and trailing)
            .lines()
            .map(|line| line.trim())
            .collect::<Vec<_>>()
            .join("\n");

        // Remove excessive blank lines (3 or more consecutive newlines become 2)
        while result.contains("\n\n\n") {
            result = result.replace("\n\n\n", "\n\n");
        }

        // Truncate if too long (embedding models have token limits)
        result.chars().take(self.config.max_text_length).collect()
    }

    /// Create embedding engine for testing using small real model
    pub async fn new_for_testing() -> Result<Self> {
        Self::new_for_testing_with_config(EmbeddingConfig {
            model_id: swissarmyhammer_config::DEFAULT_TEST_EMBEDDING_MODEL.to_string(),
            embedding_model: EmbeddingModel::BGESmallENV15, // Small, fast embedding model for testing
            batch_size: 1,
            max_text_length: 1000,
            batch_delay_ms: 0,
            show_download_progress: false,
            dimensions: Some(384), // BGE-small-en-v1.5 has 384 dimensions
            max_sequence_length: 256,
            quantization: "FP32".to_string(),
        })
        .await
    }

    /// Create embedding engine for testing with custom config using real small model
    pub async fn new_for_testing_with_config(config: EmbeddingConfig) -> Result<Self> {
        info!("Creating embedding engine for testing with real small model");

        info!("Creating embedding engine with model: {}", config.model_id);

        // Initialize real fastembed model for testing
        let init_options = InitOptions::new(config.embedding_model.clone())
            .with_show_download_progress(config.show_download_progress)
            .with_cache_dir("/tmp/.cache/fastembed".into());

        let mut model = TextEmbedding::try_new(init_options).map_err(|e| {
            SemanticError::Embedding(format!("Failed to initialize test embedding model: {e}"))
        })?;

        // Get actual model dimensions by generating a test embedding
        let test_embedding = model.embed(vec!["test".to_string()], None).map_err(|e| {
            SemanticError::Embedding(format!("Failed to get test model dimensions: {e}"))
        })?;

        let actual_dimensions = test_embedding.first().map(|e| e.len()).unwrap_or(384);

        let model_info = EmbeddingModelInfo {
            model_id: config.model_id.clone(),
            dimensions: actual_dimensions,
            max_sequence_length: config.max_sequence_length,
            quantization: config.quantization.clone(),
        };

        info!(
            "Test embedding engine created successfully with {} dimensions",
            actual_dimensions
        );
        Ok(Self {
            config,
            model_info,
            backend: Arc::new(Mutex::new(EmbeddingBackend::Neural(Box::new(model)))),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{ChunkType, ContentHash, Language};
    use crate::test_utils::IsolatedTestHome;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_embedding_engine_creation() {
        let _guard = IsolatedTestHome::new();
        let engine = EmbeddingEngine::new_for_testing().await;
        assert!(engine.is_ok());
    }

    #[tokio::test]
    async fn test_embedding_engine_with_model_id() {
        let _guard = IsolatedTestHome::new();
        let config = EmbeddingConfig {
            model_id: "custom-model".to_string(),
            embedding_model: EmbeddingModel::NomicEmbedTextV15, // Not used for mock
            batch_size: 1,
            max_text_length: 1000,
            batch_delay_ms: 0,
            show_download_progress: false,
            dimensions: Some(384),
            max_sequence_length: 256,
            quantization: "FP32".to_string(),
        };

        let engine = EmbeddingEngine::new_for_testing_with_config(config).await;
        assert!(engine.is_ok());

        let engine = engine.unwrap();
        assert_eq!(engine.model_info().model_id, "custom-model");
    }

    #[tokio::test]
    async fn test_embedding_engine_invalid_config() {
        let _guard = IsolatedTestHome::new();
        let config = EmbeddingConfig {
            model_id: "".to_string(),
            ..Default::default()
        };

        let engine = EmbeddingEngine::with_config(config).await;
        assert!(engine.is_err());
    }

    #[tokio::test]
    async fn test_embed_text() {
        let _guard = IsolatedTestHome::new();
        let engine = EmbeddingEngine::new_for_testing().await.unwrap();
        let embedding = engine.embed_text("fn main() {}").await;

        assert!(embedding.is_ok());
        let embedding = embedding.unwrap();
        assert_eq!(embedding.len(), 384); // BGE-small-en-v1.5 has 384 dimensions

        // Check that embedding values are normalized (typical for embeddings)
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.001); // Should be approximately 1.0
    }

    #[tokio::test]
    async fn test_embed_text_empty() {
        let _guard = IsolatedTestHome::new();
        let engine = EmbeddingEngine::new_for_testing().await.unwrap();
        let embedding = engine.embed_text("").await;

        assert!(embedding.is_err());
    }

    #[tokio::test]
    async fn test_embed_chunk() {
        let _guard = IsolatedTestHome::new();
        // Use mock engine for testing to avoid network dependencies
        let engine = EmbeddingEngine::new_for_testing().await.unwrap();

        let chunk = CodeChunk {
            id: "test_chunk".to_string(),
            file_path: PathBuf::from("test.rs"),
            language: Language::Rust,
            content: "fn main() {\n    println!(\"Hello, world!\");\n}".to_string(),
            start_line: 1,
            end_line: 3,
            chunk_type: ChunkType::Function,
            content_hash: ContentHash("hash123".to_string()),
        };

        let embedding = engine.embed_chunk(&chunk).await;
        assert!(embedding.is_ok());

        let embedding = embedding.unwrap();
        assert_eq!(embedding.chunk_id, "test_chunk");
        assert_eq!(embedding.vector.len(), 384);
    }

    #[tokio::test]
    async fn test_embed_batch() {
        let _guard = IsolatedTestHome::new();
        let engine = EmbeddingEngine::new_for_testing().await.unwrap();
        let texts = vec!["fn main() {}", "println!(\"hello\");"];
        let embeddings = engine.embed_batch(&texts).await;

        assert!(embeddings.is_ok());
        let embeddings = embeddings.unwrap();
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 384);
        assert_eq!(embeddings[1].len(), 384);
    }

    #[tokio::test]
    async fn test_semantic_consistency() {
        let _guard = IsolatedTestHome::new();
        let engine = EmbeddingEngine::new_for_testing().await.unwrap();

        // Test that similar texts produce similar embeddings
        let text1 = "fn add(a: i32, b: i32) -> i32 { a + b }";
        let text2 = "fn subtract(x: i32, y: i32) -> i32 { x - y }";
        let text3 = "let message = \"Hello, world!\";";

        let emb1 = engine.embed_text(text1).await.unwrap();
        let emb2 = engine.embed_text(text2).await.unwrap();
        let emb3 = engine.embed_text(text3).await.unwrap();

        // Calculate cosine similarity
        let similarity_fn =
            |a: &[f32], b: &[f32]| -> f32 { a.iter().zip(b.iter()).map(|(x, y)| x * y).sum() };

        let sim_12 = similarity_fn(&emb1, &emb2);
        let sim_13 = similarity_fn(&emb1, &emb3);

        // Functions should be more similar to each other than to strings
        assert!(sim_12 > sim_13);
    }

    #[tokio::test]
    async fn test_model_info() {
        let _guard = IsolatedTestHome::new();
        // Use real small model for testing
        let engine = EmbeddingEngine::new_for_testing().await.unwrap();

        let info = engine.model_info();
        assert_eq!(
            info.model_id,
            swissarmyhammer_config::DEFAULT_TEST_EMBEDDING_MODEL
        );
        assert_eq!(info.dimensions, 384); // BGE-small-en-v1.5 has 384 dimensions
        assert_eq!(info.max_sequence_length, 256);
        assert_eq!(info.quantization, "FP32");
    }

    #[tokio::test]
    async fn test_prepare_chunk_text() {
        let _guard = IsolatedTestHome::new();
        // Use mock engine for testing to avoid network dependencies
        let engine = EmbeddingEngine::new_for_testing().await.unwrap();

        let chunk = CodeChunk {
            id: "test_chunk".to_string(),
            file_path: PathBuf::from("test.rs"),
            language: Language::Rust,
            content: "fn main() {}".to_string(),
            start_line: 1,
            end_line: 1,
            chunk_type: ChunkType::Function,
            content_hash: ContentHash("hash123".to_string()),
        };

        let prepared_text = engine.prepare_chunk_text(&chunk);
        assert!(prepared_text.contains("Rust"));
        assert!(prepared_text.contains("Function"));
        assert!(prepared_text.contains("fn main() {}"));
    }

    #[tokio::test]
    async fn test_clean_text() {
        let _guard = IsolatedTestHome::new();
        // Use mock engine for testing to avoid network dependencies
        let engine = EmbeddingEngine::new_for_testing().await.unwrap();

        let text = "line1  \n\n\n\nline2\n   line3   \n\n\n\nline4";
        let cleaned = engine.clean_text(text);

        // Should remove excessive whitespace and blank lines
        assert!(!cleaned.contains("   "));
        assert!(!cleaned.contains("\n\n\n"));
        assert!(cleaned.contains("line1"));
        assert!(cleaned.contains("line2"));
        assert!(cleaned.contains("line3"));
        assert!(cleaned.contains("line4"));
    }

    #[tokio::test]
    async fn test_clean_text_truncation() {
        let _guard = IsolatedTestHome::new();
        // Create a config with limited text length for testing text truncation
        let config = EmbeddingConfig {
            model_id: swissarmyhammer_config::DEFAULT_TEST_EMBEDDING_MODEL.to_string(),
            embedding_model: EmbeddingModel::BGESmallENV15,
            batch_size: 1,
            max_text_length: 10, // Test truncation at 10 characters
            batch_delay_ms: 0,
            show_download_progress: false,
            dimensions: Some(384), // BGE-small-en-v1.5 has 384 dimensions
            max_sequence_length: 256,
            quantization: "FP32".to_string(),
        };

        // Use mock engine for testing to avoid network dependencies
        let engine = EmbeddingEngine::new_for_testing_with_config(config)
            .await
            .unwrap();

        let long_text = "This is a very long text that should be truncated";
        let cleaned = engine.clean_text(long_text);

        assert_eq!(cleaned.len(), 10);
        assert_eq!(cleaned, "This is a ");
    }

    #[test]
    fn test_embedding_config_default() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.model_id, "nomic-embed-text-v1.5");
        assert_eq!(config.batch_size, 32);
        assert_eq!(config.max_text_length, 8000);
        assert_eq!(config.batch_delay_ms, 10);
    }
}
