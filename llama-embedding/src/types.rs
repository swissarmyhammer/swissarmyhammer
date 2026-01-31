use llama_loader::ModelSource;
use serde::{Deserialize, Serialize};

/// Configuration for embedding operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Model source (HuggingFace or local)
    pub model_source: ModelSource,
    /// Normalize embeddings to unit vectors
    pub normalize_embeddings: bool,
    /// Maximum sequence length for tokenization.
    /// If None, uses the model's context_size from metadata after loading.
    pub max_sequence_length: Option<usize>,
    /// Enable debug logging
    pub debug: bool,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model_source: ModelSource::HuggingFace {
                repo: "Qwen/Qwen3-Embedding-0.6B-GGUF".to_string(),
                filename: Some("Qwen3-Embedding-0.6B-Q8_0.gguf".to_string()),
                folder: None,
            },
            normalize_embeddings: false,
            max_sequence_length: None,
            debug: false,
        }
    }
}

/// Result of a single text embedding operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResult {
    /// Original text that was embedded
    pub text: String,
    /// MD5 hash of the text for deduplication
    pub text_hash: String,
    /// Embedding vector
    pub embedding: Vec<f32>,
    /// Length of the tokenized sequence
    pub sequence_length: usize,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

impl EmbeddingResult {
    /// Create a new embedding result
    pub fn new(
        text: String,
        embedding: Vec<f32>,
        sequence_length: usize,
        processing_time_ms: u64,
    ) -> Self {
        let text_hash = format!("{:x}", md5::compute(&text));

        Self {
            text,
            text_hash,
            embedding,
            sequence_length,
            processing_time_ms,
        }
    }

    /// Normalize the embedding vector to unit length (L2 norm)
    pub fn normalize(&mut self) {
        let magnitude: f32 = self.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for value in &mut self.embedding {
                *value /= magnitude;
            }
        }
    }

    /// Get the embedding dimension
    pub fn dimension(&self) -> usize {
        self.embedding.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_result_creation() {
        let embedding_vec = vec![1.0, 2.0, 3.0];
        let result = EmbeddingResult::new("test text".to_string(), embedding_vec.clone(), 5, 100);

        assert_eq!(result.text, "test text");
        assert_eq!(result.embedding, embedding_vec);
        assert_eq!(result.sequence_length, 5);
        assert_eq!(result.processing_time_ms, 100);
        assert_eq!(result.dimension(), 3);
        // MD5 of "test text" should be consistent
        assert_eq!(result.text_hash, "1e2db57dd6527ad4f8f281ab028d2c70");
    }

    #[test]
    fn test_embedding_normalization() {
        let mut result = EmbeddingResult::new(
            "test".to_string(),
            vec![3.0, 4.0], // magnitude = 5.0
            2,
            50,
        );

        result.normalize();

        // Check that the vector is normalized
        let magnitude: f32 = result.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (magnitude - 1.0).abs() < 1e-6,
            "Expected magnitude ~1.0, got {}",
            magnitude
        );
        assert!((result.embedding[0] - 0.6).abs() < 1e-6);
        assert!((result.embedding[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_embedding_config_default() {
        let config = EmbeddingConfig::default();
        assert!(!config.normalize_embeddings);
        assert!(config.max_sequence_length.is_none());
        assert!(!config.debug);

        match config.model_source {
            ModelSource::HuggingFace { repo, filename, .. } => {
                assert_eq!(repo, "Qwen/Qwen3-Embedding-0.6B-GGUF");
                assert_eq!(filename.as_deref(), Some("Qwen3-Embedding-0.6B-Q8_0.gguf"));
            }
            _ => panic!("Expected HuggingFace model source"),
        }
    }
}
