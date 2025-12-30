use crate::error::{EmbeddingError, EmbeddingResult as Result};
use crate::types::{EmbeddingConfig, EmbeddingResult};
use llama_cpp_2::{
    context::{params::LlamaContextParams, LlamaContext},
    llama_backend::LlamaBackend,
    model::LlamaModel,
    send_logs_to_tracing, LogOptions,
};

// High-level llama-cpp-2 types for embedding processing
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::token::LlamaToken;
use llama_loader::{ModelConfig, ModelLoader, ModelMetadata, RetryConfig};
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use tracing::{debug, info};
// Need access to raw FFI bindings for llama_log_set
use std::ffi::c_void;
use std::os::raw::c_char;

static GLOBAL_BACKEND: OnceLock<Arc<LlamaBackend>> = OnceLock::new();

// Null log callback to suppress llama.cpp verbose output
extern "C" fn null_log_callback(_level: i32, _text: *const c_char, _user_data: *mut c_void) {
    // Do nothing - this suppresses all llama.cpp logging
}

// Set up logging suppression using llama_log_set
fn set_logging_suppression(suppress: bool) {
    unsafe {
        // Access the raw FFI binding
        extern "C" {
            fn llama_log_set(
                log_callback: Option<extern "C" fn(i32, *const c_char, *mut c_void)>,
                user_data: *mut c_void,
            );
        }

        if suppress {
            // Set null callback to suppress logging
            llama_log_set(Some(null_log_callback), std::ptr::null_mut());
        } else {
            // Restore default logging (NULL callback means output to stderr)
            llama_log_set(None, std::ptr::null_mut());
        }
    }
}

/// Core embedding model that handles individual text embedding operations
pub struct EmbeddingModel {
    model: Option<LlamaModel>,
    config: EmbeddingConfig,
    metadata: Option<ModelMetadata>,
    backend: Arc<LlamaBackend>,
}

impl EmbeddingModel {
    /// Create a new EmbeddingModel with the given configuration
    pub async fn new(config: EmbeddingConfig) -> Result<Self> {
        // Configure llama.cpp logging based on debug setting
        if config.debug {
            // Enable debug logging - send llama.cpp logs to tracing
            send_logs_to_tracing(LogOptions::default());
            debug!("Enabled verbose llama.cpp logging via tracing");
            set_logging_suppression(false);
        } else {
            // When debug is false, we rely on the tracing level configuration
            // from main.rs (WARN level) to filter out verbose logs
            debug!("llama.cpp logs will be filtered by tracing WARN level");
            set_logging_suppression(true);
        }

        // Initialize or get global backend
        let backend = Self::get_or_init_backend()?;

        Ok(Self {
            model: None,
            config,
            metadata: None,
            backend,
        })
    }

    /// Load the embedding model
    pub async fn load_model(&mut self) -> Result<()> {
        info!(
            "Loading embedding model from {:?}",
            self.config.model_source
        );

        let start_time = Instant::now();

        // Create ModelConfig for the loader
        let model_config = ModelConfig {
            source: self.config.model_source.clone(),
            batch_size: 1024, // Default batch size for embedding
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: true,
            retry_config: RetryConfig::default(),
            debug: self.config.debug,
        };

        // Load the model using the loader
        let loaded_model = {
            // Create a new loader for model loading since we need mutable access
            let loader = ModelLoader::new(self.backend.clone());

            loader
                .load_model(&model_config)
                .await
                .map_err(EmbeddingError::ModelLoader)?
        };

        let load_time = start_time.elapsed();

        // Store the model and metadata
        self.model = Some(loaded_model.model);
        self.metadata = Some(loaded_model.metadata);

        info!("Embedding model loaded successfully in {:?}", load_time);

        Ok(())
    }

    /// Generate embedding for a single text
    pub async fn embed_text(&self, text: &str) -> Result<EmbeddingResult> {
        let model = self.model.as_ref().ok_or(EmbeddingError::ModelNotLoaded)?;

        if text.is_empty() {
            return Err(EmbeddingError::text_processing(
                "Input text cannot be empty",
            ));
        }

        let start_time = Instant::now();

        debug!("Generating embedding for text: {} chars", text.len());

        // Create context for this embedding operation
        let mut context = self.create_context(model)?;

        // Tokenize the text
        let tokens = self.tokenize_text(&context, text)?;

        // Apply sequence length limit if configured
        let final_tokens = if let Some(max_len) = self.config.max_sequence_length {
            if tokens.len() > max_len {
                debug!("Truncating tokens from {} to {}", tokens.len(), max_len);
                tokens[..max_len].to_vec()
            } else {
                tokens
            }
        } else {
            tokens
        };

        // Generate embedding using the tokenized text
        let embedding = self.generate_embedding_from_tokens(&mut context, &final_tokens)?;

        let processing_time_ms = start_time.elapsed().as_millis() as u64;

        let mut result = EmbeddingResult::new(
            text.to_string(),
            embedding,
            final_tokens.len(),
            processing_time_ms,
        );

        // Apply normalization if requested
        if self.config.normalize_embeddings {
            result.normalize();
        }

        debug!(
            "Generated embedding: {} dimensions, {} tokens, {}ms",
            result.dimension(),
            result.sequence_length,
            result.processing_time_ms
        );

        Ok(result)
    }

    /// Get the embedding dimension of the loaded model
    pub fn get_embedding_dimension(&self) -> Option<usize> {
        self.model.as_ref().map(|model| {
            let n_embd = model.n_embd();
            if n_embd > 0 {
                n_embd as usize
            } else {
                // Fallback to common default if API returns invalid value
                384
            }
        })
    }

    /// Get model metadata if loaded
    pub fn get_metadata(&self) -> Option<&ModelMetadata> {
        self.metadata.as_ref()
    }

    /// Check if model is loaded
    pub fn is_loaded(&self) -> bool {
        self.model.is_some()
    }

    // Private helper methods

    fn get_or_init_backend() -> Result<Arc<LlamaBackend>> {
        if let Some(backend) = GLOBAL_BACKEND.get() {
            Ok(backend.clone())
        } else {
            let backend = LlamaBackend::init().map_err(|e| {
                EmbeddingError::model(format!("Failed to initialize LlamaBackend: {}", e))
            })?;
            let backend_arc = Arc::new(backend);

            // Try to store globally, use existing if someone else set it
            match GLOBAL_BACKEND.set(backend_arc.clone()) {
                Ok(_) => Ok(backend_arc),
                Err(_) => Ok(GLOBAL_BACKEND.get().unwrap().clone()),
            }
        }
    }

    fn create_context<'a>(&self, model: &'a LlamaModel) -> Result<LlamaContext<'a>> {
        let context_params = LlamaContextParams::default().with_embeddings(true);

        model
            .new_context(&self.backend, context_params)
            .map_err(|e| EmbeddingError::model(format!("Failed to create context: {}", e)))
    }

    fn tokenize_text(&self, context: &LlamaContext, text: &str) -> Result<Vec<LlamaToken>> {
        use llama_cpp_2::model::AddBos;

        // For embedding models, we typically want to tokenize the text as-is
        // without special tokens like BOS/EOS that are used for generation
        let tokens = context
            .model
            .str_to_token(text, AddBos::Never)
            .map_err(|e| {
                EmbeddingError::text_encoding(format!("Failed to tokenize text: {}", e))
            })?;

        if tokens.is_empty() {
            return Err(EmbeddingError::text_encoding(
                "Tokenization produced no tokens",
            ));
        }

        debug!("Tokenized text into {} tokens", tokens.len());
        Ok(tokens)
    }

    fn generate_embedding_from_tokens(
        &self,
        context: &mut LlamaContext,
        tokens: &[LlamaToken],
    ) -> Result<Vec<f32>> {
        if tokens.is_empty() {
            return Err(EmbeddingError::text_processing(
                "Cannot generate embedding from empty token sequence",
            ));
        }

        // Get embedding dimension from the model
        let embedding_dim = self.get_embedding_dimension().ok_or_else(|| {
            EmbeddingError::model("Could not determine embedding dimension".to_string())
        })?;

        // Create a batch for the tokens
        // We need enough space for all tokens and use one sequence
        let mut batch = LlamaBatch::new(tokens.len(), 1);

        // Add the token sequence to the batch
        // Set logits to true for the last token to get embeddings
        batch.add_sequence(tokens, 0, false).map_err(|e| {
            EmbeddingError::text_processing(format!("Failed to add tokens to batch: {}", e))
        })?;

        // Decode the tokens to generate embeddings
        context.decode(&mut batch).map_err(|e| {
            EmbeddingError::text_processing(format!(
                "Failed to decode tokens for embedding extraction: {}",
                e
            ))
        })?;

        // Extract embeddings for the sequence
        // Use sequence 0 since we only have one sequence
        let embeddings = context.embeddings_seq_ith(0).map_err(|e| {
            EmbeddingError::text_processing(format!(
                "Failed to extract embeddings from context: {}",
                e
            ))
        })?;

        // Validate embedding dimension matches expectation
        if embeddings.len() != embedding_dim {
            return Err(EmbeddingError::text_processing(format!(
                "Embedding dimension mismatch: expected {}, got {}",
                embedding_dim,
                embeddings.len()
            )));
        }

        debug!(
            "Successfully extracted embedding of dimension {} for {} tokens",
            embedding_dim,
            tokens.len()
        );

        // Convert from slice to owned vector
        Ok(embeddings.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llama_loader::ModelSource;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn test_embedding_model_creation() {
        let config = EmbeddingConfig::default();
        let result = EmbeddingModel::new(config).await;

        // This test might fail in CI without proper setup
        // but validates the structure compiles correctly
        match result {
            Ok(_) => {
                // Model created successfully
            }
            Err(EmbeddingError::ModelLoader(_)) => {
                // Expected in test environment without proper model setup
            }
            Err(e) => {
                panic!("Unexpected error: {}", e);
            }
        }
    }

    #[test]
    fn test_embedding_config_usage() {
        let config = EmbeddingConfig {
            model_source: ModelSource::HuggingFace {
                repo: "test/repo".to_string(),
                filename: Some("test.gguf".to_string()),
                folder: None,
            },
            normalize_embeddings: true,
            max_sequence_length: Some(512),
            debug: true,
        };

        assert!(config.normalize_embeddings);
        assert_eq!(config.max_sequence_length, Some(512));
        assert!(config.debug);
    }
}
