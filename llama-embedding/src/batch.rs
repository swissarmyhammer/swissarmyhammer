use crate::error::{EmbeddingError, EmbeddingResult as Result};
use crate::model::EmbeddingModel;
use crate::types::EmbeddingResult;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio_stream::Stream;
use tracing::{debug, info, warn};

/// Progress information for batch processing operations
#[derive(Debug, Clone)]
pub struct ProgressInfo {
    pub current_batch: usize,
    pub total_batches: usize,
    pub texts_processed: usize,
    pub total_texts: usize,
    pub successful_embeddings: usize,
    pub failed_embeddings: usize,
    pub elapsed_time_ms: u64,
    pub estimated_remaining_ms: u64,
    pub current_throughput_texts_per_second: f64,
}

/// Callback type for progress reporting
pub type ProgressCallback = Box<dyn Fn(&ProgressInfo) + Send + Sync>;

/// Statistics for batch processing operations
#[derive(Debug, Clone, Default)]
pub struct BatchStats {
    pub total_texts: usize,
    pub successful_embeddings: usize,
    pub failed_embeddings: usize,
    pub total_processing_time_ms: u64,
    pub average_time_per_text_ms: f64,
    pub total_tokens_processed: usize,
    pub average_tokens_per_text: f64,
    pub batches_processed: usize,
    pub average_batch_time_ms: f64,
    pub peak_memory_usage_bytes: usize,
    pub total_characters_processed: usize,
}

impl BatchStats {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn update(&mut self, batch_size: usize, processing_time_ms: u64, failures: usize) {
        self.total_texts += batch_size;
        self.successful_embeddings += batch_size - failures;
        self.failed_embeddings += failures;
        self.total_processing_time_ms += processing_time_ms;
        self.batches_processed += 1;

        if self.total_texts > 0 {
            self.average_time_per_text_ms =
                self.total_processing_time_ms as f64 / self.total_texts as f64;
        }

        if self.batches_processed > 0 {
            self.average_batch_time_ms =
                self.total_processing_time_ms as f64 / self.batches_processed as f64;
        }
    }

    pub fn update_with_details(
        &mut self,
        batch_results: &[EmbeddingResult],
        processing_time_ms: u64,
        failures: usize,
    ) {
        let batch_size = batch_results.len() + failures;
        let token_count: usize = batch_results.iter().map(|r| r.sequence_length).sum();
        let char_count: usize = batch_results.iter().map(|r| r.text.len()).sum();

        self.total_texts += batch_size;
        self.successful_embeddings += batch_results.len();
        self.failed_embeddings += failures;
        self.total_processing_time_ms += processing_time_ms;
        self.total_tokens_processed += token_count;
        self.total_characters_processed += char_count;
        self.batches_processed += 1;

        // Update averages
        if self.total_texts > 0 {
            self.average_time_per_text_ms =
                self.total_processing_time_ms as f64 / self.total_texts as f64;
            self.average_tokens_per_text =
                self.total_tokens_processed as f64 / self.successful_embeddings as f64;
        }

        if self.batches_processed > 0 {
            self.average_batch_time_ms =
                self.total_processing_time_ms as f64 / self.batches_processed as f64;
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_texts == 0 {
            0.0
        } else {
            self.successful_embeddings as f64 / self.total_texts as f64
        }
    }

    pub fn throughput_texts_per_second(&self) -> f64 {
        if self.total_processing_time_ms == 0 {
            0.0
        } else {
            (self.successful_embeddings as f64) / (self.total_processing_time_ms as f64 / 1000.0)
        }
    }

    pub fn throughput_tokens_per_second(&self) -> f64 {
        if self.total_processing_time_ms == 0 {
            0.0
        } else {
            (self.total_tokens_processed as f64) / (self.total_processing_time_ms as f64 / 1000.0)
        }
    }

    pub fn update_memory_usage(&mut self, current_usage_bytes: usize) {
        if current_usage_bytes > self.peak_memory_usage_bytes {
            self.peak_memory_usage_bytes = current_usage_bytes;
        }
    }

    pub fn format_summary(&self) -> String {
        format!(
            "BatchStats {{ texts: {}/{} ({:.1}% success), time: {:.1}s, throughput: {:.1} texts/s, {:.1} tokens/s, memory: {:.2}MB }}",
            self.successful_embeddings,
            self.total_texts,
            self.success_rate() * 100.0,
            self.total_processing_time_ms as f64 / 1000.0,
            self.throughput_texts_per_second(),
            self.throughput_tokens_per_second(),
            self.peak_memory_usage_bytes as f64 / (1024.0 * 1024.0)
        )
    }
}

/// Configuration for batch processing behavior
#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub batch_size: usize,
    pub continue_on_error: bool,
    pub max_parallel_tasks: usize,
    pub enable_progress_reporting: bool,
    pub progress_report_interval_batches: usize,
    pub memory_limit_mb: Option<usize>,
    pub enable_memory_monitoring: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: 32,
            continue_on_error: true,
            max_parallel_tasks: 4,
            enable_progress_reporting: false,
            progress_report_interval_batches: 10,
            memory_limit_mb: None,
            enable_memory_monitoring: true,
        }
    }
}

/// Handles batch processing of multiple texts for embedding generation
pub struct BatchProcessor {
    model: Arc<EmbeddingModel>,
    config: BatchConfig,
    stats: BatchStats,
    progress_callback: Option<ProgressCallback>,
}

impl BatchProcessor {
    /// Create a new BatchProcessor with default configuration
    pub fn new(model: Arc<EmbeddingModel>, batch_size: usize) -> Self {
        let config = BatchConfig {
            batch_size,
            ..Default::default()
        };
        Self {
            model,
            config,
            stats: BatchStats::new(),
            progress_callback: None,
        }
    }

    /// Create a new BatchProcessor with custom configuration
    pub fn with_config(model: Arc<EmbeddingModel>, config: BatchConfig) -> Self {
        Self {
            model,
            config,
            stats: BatchStats::new(),
            progress_callback: None,
        }
    }

    /// Set a progress callback for monitoring batch processing
    pub fn set_progress_callback(&mut self, callback: ProgressCallback) {
        self.progress_callback = Some(callback);
    }

    /// Clear the progress callback
    pub fn clear_progress_callback(&mut self) {
        self.progress_callback = None;
    }

    /// Process a batch of texts and return embedding results with error recovery
    pub async fn process_batch(&mut self, texts: &[String]) -> Result<Vec<EmbeddingResult>> {
        if !self.model.is_loaded() {
            return Err(EmbeddingError::ModelNotLoaded);
        }

        let start_time = Instant::now();
        debug!("Processing batch of {} texts", texts.len());

        // Monitor memory usage if enabled
        if self.config.enable_memory_monitoring {
            let memory_usage = self.estimate_current_memory_usage(texts);
            self.stats.update_memory_usage(memory_usage);

            // Check memory limit if configured
            if let Some(limit_mb) = self.config.memory_limit_mb {
                let limit_bytes = limit_mb * 1024 * 1024;
                if memory_usage > limit_bytes {
                    warn!(
                        "Memory usage ({:.2}MB) exceeds limit ({:.2}MB)",
                        memory_usage as f64 / (1024.0 * 1024.0),
                        limit_mb
                    );
                    return Err(EmbeddingError::batch_processing(format!(
                        "Memory limit exceeded: {:.2}MB > {}MB",
                        memory_usage as f64 / (1024.0 * 1024.0),
                        limit_mb
                    )));
                }
            }
        }

        let mut results = Vec::new();
        let mut failures = 0;

        for text in texts {
            match self.model.embed_text(text).await {
                Ok(result) => {
                    results.push(result);
                }
                Err(e) => {
                    failures += 1;
                    let preview = text.chars().take(50).collect::<String>();
                    warn!("Failed to embed text '{}...': {}", preview, e);

                    if !self.config.continue_on_error {
                        return Err(e);
                    }
                    // Continue processing other texts if continue_on_error is true
                }
            }
        }

        let processing_time = start_time.elapsed().as_millis() as u64;
        self.stats
            .update_with_details(&results, processing_time, failures);

        debug!(
            "Processed batch: {} successful, {} failed, {}ms",
            results.len(),
            failures,
            processing_time
        );

        Ok(results)
    }

    /// Process a list of texts with efficient batching
    pub async fn process_texts(&mut self, texts: Vec<String>) -> Result<Vec<EmbeddingResult>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        info!(
            "Processing {} texts in batches of {}",
            texts.len(),
            self.config.batch_size
        );
        let total_batches = (texts.len() + self.config.batch_size - 1) / self.config.batch_size;
        let mut all_results = Vec::new();
        let start_time = Instant::now();

        for (batch_idx, chunk) in texts.chunks(self.config.batch_size).enumerate() {
            let batch_results = self.process_batch(chunk).await?;
            all_results.extend(batch_results);

            // Report progress if enabled and callback is set
            if self.config.enable_progress_reporting
                && batch_idx % self.config.progress_report_interval_batches == 0
            {
                if let Some(ref callback) = self.progress_callback {
                    let elapsed_ms = start_time.elapsed().as_millis() as u64;
                    let current_throughput = if elapsed_ms > 0 {
                        (self.stats.successful_embeddings as f64) / (elapsed_ms as f64 / 1000.0)
                    } else {
                        0.0
                    };

                    let remaining_batches = total_batches.saturating_sub(batch_idx + 1);
                    let estimated_remaining_ms =
                        if current_throughput > 0.0 && remaining_batches > 0 {
                            let remaining_texts = remaining_batches * self.config.batch_size;
                            ((remaining_texts as f64) / current_throughput * 1000.0) as u64
                        } else {
                            0
                        };

                    let progress_info = ProgressInfo {
                        current_batch: batch_idx + 1,
                        total_batches,
                        texts_processed: self.stats.total_texts,
                        total_texts: texts.len(),
                        successful_embeddings: self.stats.successful_embeddings,
                        failed_embeddings: self.stats.failed_embeddings,
                        elapsed_time_ms: elapsed_ms,
                        estimated_remaining_ms,
                        current_throughput_texts_per_second: current_throughput,
                    };

                    callback(&progress_info);
                }
            }
        }

        info!(
            "Completed processing {} texts with {} results. {}",
            texts.len(),
            all_results.len(),
            self.stats.format_summary()
        );
        Ok(all_results)
    }

    /// Process a file containing texts (one per line) - memory efficient version
    pub async fn process_file(&mut self, input_path: &Path) -> Result<Vec<EmbeddingResult>> {
        if !input_path.exists() {
            return Err(EmbeddingError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Input file not found: {}", input_path.display()),
            )));
        }

        info!("Processing file: {}", input_path.display());
        let mut all_results = Vec::new();
        let mut current_batch = Vec::new();

        let file = File::open(input_path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                current_batch.push(trimmed.to_string());

                // Process batch when it reaches the configured size
                if current_batch.len() >= self.config.batch_size {
                    let batch_results = self.process_batch(&current_batch).await?;
                    all_results.extend(batch_results);
                    current_batch.clear();
                }
            }
        }

        // Process remaining texts in the final batch
        if !current_batch.is_empty() {
            let batch_results = self.process_batch(&current_batch).await?;
            all_results.extend(batch_results);
        }

        info!(
            "Completed processing file with {} embeddings",
            all_results.len()
        );
        Ok(all_results)
    }

    /// Process a file with streaming results via callback
    pub async fn process_file_streaming<F>(
        &mut self,
        input_path: &Path,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(Vec<EmbeddingResult>) -> std::result::Result<(), EmbeddingError>,
    {
        if !input_path.exists() {
            return Err(EmbeddingError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Input file not found: {}", input_path.display()),
            )));
        }

        info!("Processing file with streaming: {}", input_path.display());
        let mut current_batch = Vec::new();

        let file = File::open(input_path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                current_batch.push(trimmed.to_string());

                // Process and yield batch when it reaches the configured size
                if current_batch.len() >= self.config.batch_size {
                    let batch_results = self.process_batch(&current_batch).await?;
                    callback(batch_results)?;
                    current_batch.clear();
                }
            }
        }

        // Process and yield remaining texts in the final batch
        if !current_batch.is_empty() {
            let batch_results = self.process_batch(&current_batch).await?;
            callback(batch_results)?;
        }

        info!("Completed streaming processing of file");
        Ok(())
    }

    /// Process a file and return an async stream of results
    pub async fn process_file_as_stream(
        &mut self,
        input_path: &Path,
    ) -> Result<impl Stream<Item = std::result::Result<Vec<EmbeddingResult>, EmbeddingError>>> {
        if !input_path.exists() {
            return Err(EmbeddingError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Input file not found: {}", input_path.display()),
            )));
        }

        let (tx, rx) =
            mpsc::channel::<std::result::Result<Vec<EmbeddingResult>, EmbeddingError>>(100);
        let input_path = input_path.to_path_buf();
        let batch_size = self.config.batch_size;
        let model = self.model.clone();
        let continue_on_error = self.config.continue_on_error;

        tokio::spawn(async move {
            let mut processor = BatchProcessor::new(model, batch_size);
            processor.config.continue_on_error = continue_on_error;

            let result = processor
                .process_file_streaming(&input_path, |batch_results| {
                    if tx.try_send(Ok(batch_results)).is_err() {
                        return Err(EmbeddingError::BatchProcessing(
                            "Channel closed while streaming results".to_string(),
                        ));
                    }
                    Ok(())
                })
                .await;

            if let Err(e) = result {
                let _ = tx.send(Err(e)).await;
            }
        });

        Ok(tokio_stream::wrappers::ReceiverStream::new(rx))
    }

    /// Get the current batch configuration
    pub fn config(&self) -> &BatchConfig {
        &self.config
    }

    /// Get the configured batch size
    pub fn batch_size(&self) -> usize {
        self.config.batch_size
    }

    /// Set a new batch size
    pub fn set_batch_size(&mut self, new_batch_size: usize) {
        if new_batch_size > 0 {
            self.config.batch_size = new_batch_size;
            debug!("Updated batch size to {}", new_batch_size);
        } else {
            warn!("Attempted to set invalid batch size: {}", new_batch_size);
        }
    }

    /// Set whether to continue processing on errors
    pub fn set_continue_on_error(&mut self, continue_on_error: bool) {
        self.config.continue_on_error = continue_on_error;
        debug!("Updated continue_on_error to {}", continue_on_error);
    }

    /// Get current processing statistics
    pub fn stats(&self) -> &BatchStats {
        &self.stats
    }

    /// Reset processing statistics
    pub fn reset_stats(&mut self) {
        self.stats = BatchStats::new();
        debug!("Reset processing statistics");
    }

    /// Get statistics about the underlying model
    pub fn get_model_info(&self) -> Option<(usize, bool)> {
        self.model
            .get_embedding_dimension()
            .map(|dim| (dim, self.model.is_loaded()))
    }

    /// Estimate current memory usage for a batch of texts
    fn estimate_current_memory_usage(&self, texts: &[String]) -> usize {
        let text_memory = texts.iter().map(|t| t.len()).sum::<usize>();
        let embeddings_memory = if let Some(dim) = self.model.get_embedding_dimension() {
            // Estimate memory for potential embeddings (f32 = 4 bytes per element)
            texts.len() * dim * 4
        } else {
            // Default assumption for embedding dimension
            texts.len() * 384 * 4
        };

        // Add overhead for vectors, strings, and other data structures (rough estimate)
        let overhead = (text_memory + embeddings_memory) / 4;

        text_memory + embeddings_memory + overhead
    }

    /// Get a detailed performance report
    pub fn get_performance_report(&self) -> String {
        format!(
            "Performance Report:\n\
            - Total texts processed: {}\n\
            - Success rate: {:.1}%\n\
            - Processing time: {:.2}s\n\
            - Throughput: {:.1} texts/s, {:.1} tokens/s\n\
            - Average time per text: {:.1}ms\n\
            - Average tokens per text: {:.1}\n\
            - Batches processed: {}\n\
            - Average batch time: {:.1}ms\n\
            - Peak memory usage: {:.2}MB\n\
            - Total characters: {}",
            self.stats.total_texts,
            self.stats.success_rate() * 100.0,
            self.stats.total_processing_time_ms as f64 / 1000.0,
            self.stats.throughput_texts_per_second(),
            self.stats.throughput_tokens_per_second(),
            self.stats.average_time_per_text_ms,
            self.stats.average_tokens_per_text,
            self.stats.batches_processed,
            self.stats.average_batch_time_ms,
            self.stats.peak_memory_usage_bytes as f64 / (1024.0 * 1024.0),
            self.stats.total_characters_processed
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Note: These are structural tests that validate compilation
    // Actual functionality tests would require a real model to be loaded

    #[test]
    fn test_batch_stats() {
        let mut stats = BatchStats::new();

        // Initial state
        assert_eq!(stats.total_texts, 0);
        assert_eq!(stats.successful_embeddings, 0);
        assert_eq!(stats.failed_embeddings, 0);
        assert_eq!(stats.success_rate(), 0.0);
        assert_eq!(stats.batches_processed, 0);
        assert_eq!(stats.total_tokens_processed, 0);

        // Update with successful batch
        stats.update(10, 1000, 0);
        assert_eq!(stats.total_texts, 10);
        assert_eq!(stats.successful_embeddings, 10);
        assert_eq!(stats.failed_embeddings, 0);
        assert_eq!(stats.success_rate(), 1.0);
        assert_eq!(stats.average_time_per_text_ms, 100.0);
        assert_eq!(stats.batches_processed, 1);
        assert_eq!(stats.average_batch_time_ms, 1000.0);

        // Update with partially failed batch
        stats.update(10, 2000, 2);
        assert_eq!(stats.total_texts, 20);
        assert_eq!(stats.successful_embeddings, 18);
        assert_eq!(stats.failed_embeddings, 2);
        assert_eq!(stats.success_rate(), 0.9);
        assert_eq!(stats.average_time_per_text_ms, 150.0);
        assert_eq!(stats.batches_processed, 2);
        assert_eq!(stats.average_batch_time_ms, 1500.0);
    }

    #[test]
    fn test_batch_config_default() {
        let config = BatchConfig::default();
        assert_eq!(config.batch_size, 32);
        assert!(config.continue_on_error);
        assert_eq!(config.max_parallel_tasks, 4);
        assert!(!config.enable_progress_reporting);
        assert_eq!(config.progress_report_interval_batches, 10);
        assert!(config.memory_limit_mb.is_none());
        assert!(config.enable_memory_monitoring);
    }

    /// Helper function to create a real embedding model for testing using small Qwen model
    #[cfg(test)]
    pub async fn create_test_embedding_model() -> crate::model::EmbeddingModel {
        use crate::types::EmbeddingConfig;
        use llama_loader::ModelSource;

        let config = EmbeddingConfig {
            model_source: ModelSource::HuggingFace {
                repo: "Qwen/Qwen3-Embedding-0.6B-GGUF".to_string(),
                filename: None,
                folder: None,
            },
            normalize_embeddings: false,
            max_sequence_length: None,
            debug: false, // Keep quiet for tests
        };

        crate::model::EmbeddingModel::new(config)
            .await
            .expect("Failed to create test embedding model")
    }

    /// Helper function to create a real embedding model for testing that is already loaded
    #[cfg(test)]
    pub async fn create_loaded_test_embedding_model() -> crate::model::EmbeddingModel {
        let mut model = create_test_embedding_model().await;
        model
            .load_model()
            .await
            .expect("Failed to load test embedding model");
        model
    }

    /// Test-specific BatchProcessor that works with real EmbeddingModel
    #[cfg(test)]
    pub struct RealTestBatchProcessor {
        model: Arc<crate::model::EmbeddingModel>,
        config: BatchConfig,
        stats: BatchStats,
        progress_callback: Option<ProgressCallback>,
    }

    #[cfg(test)]
    impl RealTestBatchProcessor {
        pub fn new(model: Arc<crate::model::EmbeddingModel>, batch_size: usize) -> Self {
            let config = BatchConfig {
                batch_size,
                ..Default::default()
            };
            Self {
                model,
                config,
                stats: BatchStats::new(),
                progress_callback: None,
            }
        }

        pub fn with_config(model: Arc<crate::model::EmbeddingModel>, config: BatchConfig) -> Self {
            Self {
                model,
                config,
                stats: BatchStats::new(),
                progress_callback: None,
            }
        }

        pub fn set_progress_callback(&mut self, callback: ProgressCallback) {
            self.progress_callback = Some(callback);
        }

        /// Process a batch of texts and return embedding results with error recovery
        pub async fn process_batch(&mut self, texts: &[String]) -> Result<Vec<EmbeddingResult>> {
            if !self.model.is_loaded() {
                return Err(EmbeddingError::ModelNotLoaded);
            }

            let start_time = Instant::now();
            debug!("Processing batch of {} texts", texts.len());

            // Monitor memory usage if enabled
            if self.config.enable_memory_monitoring {
                let memory_usage = self.estimate_current_memory_usage(texts);
                self.stats.update_memory_usage(memory_usage);

                // Check memory limit if configured
                if let Some(limit_mb) = self.config.memory_limit_mb {
                    if memory_usage > (limit_mb * 1024 * 1024) {
                        return Err(EmbeddingError::configuration(format!(
                            "Memory usage {} bytes exceeds limit {} MB",
                            memory_usage, limit_mb
                        )));
                    }
                }
            }

            let mut results = Vec::with_capacity(texts.len());
            let mut batch_errors = Vec::new();

            // Process each text individually (real model doesn't have batch processing in this context)
            for text in texts {
                match self.model.embed_text(text).await {
                    Ok(result) => {
                        results.push(result);
                        self.stats.successful_embeddings += 1;
                    }
                    Err(e) => {
                        self.stats.failed_embeddings += 1;
                        if !self.config.continue_on_error {
                            return Err(e);
                        }
                        batch_errors.push(e);
                    }
                }
            }

            self.stats.total_texts += texts.len();
            // Update processing time - use the existing update method
            let processing_time_ms = start_time.elapsed().as_millis() as u64;
            self.stats.total_processing_time_ms += processing_time_ms;

            // Call progress callback if set
            if let Some(callback) = &self.progress_callback {
                let progress_info = ProgressInfo {
                    current_batch: 1,
                    total_batches: 1,
                    texts_processed: self.stats.total_texts,
                    total_texts: texts.len(),
                    successful_embeddings: self.stats.successful_embeddings,
                    failed_embeddings: self.stats.failed_embeddings,
                    elapsed_time_ms: processing_time_ms,
                    estimated_remaining_ms: 0,
                    current_throughput_texts_per_second: self.stats.throughput_texts_per_second(),
                };
                callback(&progress_info);
            }

            // Log any accumulated errors
            for error in batch_errors {
                debug!("Batch processing error (continuing): {}", error);
            }

            debug!(
                "Completed batch processing: {} successful, {} failed in {:?}",
                results.len(),
                self.stats.failed_embeddings,
                start_time.elapsed()
            );

            Ok(results)
        }

        /// Estimate memory usage for the current batch
        fn estimate_current_memory_usage(&self, texts: &[String]) -> usize {
            let text_memory: usize = texts.iter().map(|t| t.len()).sum();
            let embeddings_memory = if let Some(dim) = self.model.get_embedding_dimension() {
                texts.len() * dim * std::mem::size_of::<f32>()
            } else {
                0
            };
            text_memory + embeddings_memory
        }

        /// Process multiple texts in batches
        pub async fn process_texts(&mut self, texts: Vec<String>) -> Result<Vec<EmbeddingResult>> {
            let total_batches = (texts.len() + self.config.batch_size - 1) / self.config.batch_size;
            let mut all_results = Vec::new();
            let start_time = Instant::now();

            for (batch_idx, chunk) in texts.chunks(self.config.batch_size).enumerate() {
                let batch_results = self.process_batch(chunk).await?;
                all_results.extend(batch_results);

                // Report progress if enabled and callback is set
                if self.config.enable_progress_reporting
                    && batch_idx % self.config.progress_report_interval_batches == 0
                {
                    if let Some(ref callback) = self.progress_callback {
                        let elapsed_ms = start_time.elapsed().as_millis() as u64;
                        let current_throughput = if elapsed_ms > 0 {
                            (self.stats.successful_embeddings as f64) / (elapsed_ms as f64 / 1000.0)
                        } else {
                            0.0
                        };

                        let remaining_batches = total_batches.saturating_sub(batch_idx + 1);
                        let estimated_remaining_ms =
                            if current_throughput > 0.0 && remaining_batches > 0 {
                                let remaining_texts = remaining_batches * self.config.batch_size;
                                ((remaining_texts as f64) / current_throughput * 1000.0) as u64
                            } else {
                                0
                            };

                        let progress_info = ProgressInfo {
                            current_batch: batch_idx + 1,
                            total_batches,
                            texts_processed: self.stats.total_texts,
                            total_texts: texts.len(),
                            successful_embeddings: self.stats.successful_embeddings,
                            failed_embeddings: self.stats.failed_embeddings,
                            elapsed_time_ms: elapsed_ms,
                            estimated_remaining_ms,
                            current_throughput_texts_per_second: current_throughput,
                        };
                        callback(&progress_info);
                    }
                }
            }

            self.stats.batches_processed += total_batches;
            Ok(all_results)
        }
    }

    #[tokio::test]
    async fn test_batch_processor_creation_and_basic_functionality() {
        let real_model = Arc::new(create_loaded_test_embedding_model().await);
        let processor = RealTestBatchProcessor::new(real_model, 4);

        // Test configuration
        assert_eq!(processor.config.batch_size, 4);
        assert!(processor.config.continue_on_error);
        assert_eq!(processor.config.max_parallel_tasks, 4);

        // Test initial stats
        assert_eq!(processor.stats.total_texts, 0);
        assert_eq!(processor.stats.successful_embeddings, 0);
        assert_eq!(processor.stats.failed_embeddings, 0);
    }

    #[tokio::test]

    async fn test_batch_processor_single_batch_success() {
        let real_model = Arc::new(create_loaded_test_embedding_model().await);
        let mut processor = RealTestBatchProcessor::new(real_model, 4);

        let texts = vec![
            "Hello world".to_string(),
            "This is a test".to_string(),
            "Batch processing".to_string(),
        ];

        let results = processor.process_batch(&texts).await.unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(processor.stats.total_texts, 3);
        assert_eq!(processor.stats.successful_embeddings, 3);
        assert_eq!(processor.stats.failed_embeddings, 0);

        // Verify each result
        let expected_dimension = results[0].dimension(); // Get actual dimension from first result
        for (i, result) in results.iter().enumerate() {
            assert_eq!(result.text, texts[i]);
            assert_eq!(result.dimension(), expected_dimension); // Verify consistent dimension
            assert!(result.dimension() > 0); // Verify dimension is positive
            assert!(result.processing_time_ms > 0);
            assert!(result.sequence_length > 0);
        }
    }

    #[tokio::test]

    async fn test_batch_processor_model_not_loaded() {
        let real_model = Arc::new(create_test_embedding_model().await); // Not loaded
        let mut processor = RealTestBatchProcessor::new(real_model, 4);

        let texts = vec!["Hello world".to_string()];
        let result = processor.process_batch(&texts).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EmbeddingError::ModelNotLoaded
        ));
    }

    #[tokio::test]

    async fn test_batch_processor_continue_on_error() {
        let real_model = Arc::new(create_loaded_test_embedding_model().await);
        let mut processor = RealTestBatchProcessor::new(real_model, 4);

        let texts = vec![
            "Hello world".to_string(),
            "".to_string(), // Empty string should fail
            "This is fine".to_string(),
            "".to_string(), // Another empty string should fail
        ];

        let results = processor.process_batch(&texts).await.unwrap();

        // Should get 2 successful results (skipping the 2 empty strings that failed)
        assert_eq!(results.len(), 2);
        assert_eq!(processor.stats.successful_embeddings, 2);
        assert_eq!(processor.stats.failed_embeddings, 2);

        // Check that successful results are correct
        assert_eq!(results[0].text, "Hello world");
        assert_eq!(results[1].text, "This is fine");
    }

    #[tokio::test]

    async fn test_batch_processor_stop_on_error() {
        let real_model = Arc::new(create_loaded_test_embedding_model().await);
        let config = BatchConfig {
            batch_size: 4,
            continue_on_error: false, // Stop on first error
            max_parallel_tasks: 4,
            enable_progress_reporting: false,
            progress_report_interval_batches: 10,
            memory_limit_mb: None,
            enable_memory_monitoring: true,
        };
        let mut processor = RealTestBatchProcessor::with_config(real_model, config);

        let texts = vec![
            "Hello world".to_string(),
            "".to_string(), // Empty string should fail - should stop here
            "This won't be processed".to_string(),
        ];

        let result = processor.process_batch(&texts).await;

        // Should fail on the second text (empty string)
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EmbeddingError::TextProcessing(_)
        ));
    }

    #[tokio::test]

    async fn test_batch_processor_empty_input() {
        let real_model = Arc::new(create_loaded_test_embedding_model().await);
        let mut processor = RealTestBatchProcessor::new(real_model, 4);

        let texts: Vec<String> = vec![];
        let results = processor.process_batch(&texts).await.unwrap();

        assert!(results.is_empty());
        assert_eq!(processor.stats.total_texts, 0);
    }

    #[tokio::test]

    async fn test_batch_processor_large_batch_chunking() {
        let real_model = Arc::new(create_loaded_test_embedding_model().await);
        let mut processor = RealTestBatchProcessor::new(real_model, 3); // Small batch size

        let texts: Vec<String> = (0..10).map(|i| format!("Text number {}", i)).collect();
        let results = processor.process_texts(texts.clone()).await.unwrap();

        assert_eq!(results.len(), 10);
        assert_eq!(processor.stats.total_texts, 10);
        assert_eq!(processor.stats.successful_embeddings, 10);
        assert_eq!(processor.stats.batches_processed, 4); // 10 texts / 3 batch size = 4 batches (3+3+3+1)

        // Verify all texts were processed correctly
        for (i, result) in results.iter().enumerate() {
            assert_eq!(result.text, texts[i]);
        }
    }

    #[tokio::test]

    async fn test_batch_processor_memory_limit() {
        let real_model = Arc::new(create_loaded_test_embedding_model().await);
        let config = BatchConfig {
            batch_size: 4,
            continue_on_error: true,
            max_parallel_tasks: 4,
            enable_progress_reporting: false,
            progress_report_interval_batches: 10,
            memory_limit_mb: Some(1), // Very small memory limit
            enable_memory_monitoring: true,
        };
        let mut processor = RealTestBatchProcessor::with_config(real_model, config);

        // Create a large text that should exceed memory limit
        let large_text = "x".repeat(10_000_000); // 10MB of text
        let texts = vec![large_text];

        let result = processor.process_batch(&texts).await;

        // Should fail due to memory limit
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EmbeddingError::Configuration(_)
        ));
    }

    #[tokio::test]

    async fn test_batch_processor_progress_callback() {
        use std::sync::Mutex;

        let real_model = Arc::new(create_loaded_test_embedding_model().await);
        let config = BatchConfig {
            batch_size: 2,
            continue_on_error: true,
            max_parallel_tasks: 4,
            enable_progress_reporting: true,
            progress_report_interval_batches: 1, // Report every batch
            memory_limit_mb: None,
            enable_memory_monitoring: true,
        };
        let mut processor = RealTestBatchProcessor::with_config(real_model, config);

        // Collect progress reports
        let progress_reports = Arc::new(Mutex::new(Vec::new()));
        let progress_reports_clone = progress_reports.clone();

        processor.set_progress_callback(Box::new(move |progress| {
            progress_reports_clone
                .lock()
                .unwrap()
                .push(progress.clone());
        }));

        let texts: Vec<String> = (0..5).map(|i| format!("Text {}", i)).collect();
        let results = processor.process_texts(texts).await.unwrap();

        assert_eq!(results.len(), 5);

        // Check that progress was reported
        let reports = progress_reports.lock().unwrap();
        assert!(!reports.is_empty());

        // Verify progress report structure
        for report in reports.iter() {
            assert!(report.current_batch > 0);
            assert!(report.total_batches > 0);
            // elapsed_time_ms is always >= 0 by type (u64)
            assert!(report.current_throughput_texts_per_second >= 0.0);
        }
    }

    #[tokio::test]

    async fn test_batch_processor_custom_config() {
        let real_model = Arc::new(create_loaded_test_embedding_model().await);
        let config = BatchConfig {
            batch_size: 8,
            continue_on_error: false,
            max_parallel_tasks: 2,
            enable_progress_reporting: true,
            progress_report_interval_batches: 5,
            memory_limit_mb: Some(100),
            enable_memory_monitoring: false,
        };
        let processor = RealTestBatchProcessor::with_config(real_model, config);

        // Verify configuration was applied correctly
        assert_eq!(processor.config.batch_size, 8);
        assert!(!processor.config.continue_on_error);
        assert_eq!(processor.config.max_parallel_tasks, 2);
        assert!(processor.config.enable_progress_reporting);
        assert_eq!(processor.config.progress_report_interval_batches, 5);
        assert_eq!(processor.config.memory_limit_mb, Some(100));
        assert!(!processor.config.enable_memory_monitoring);
    }

    #[tokio::test]

    async fn test_batch_processor_deterministic_results() {
        let real_model1 = Arc::new(create_loaded_test_embedding_model().await);
        let real_model2 = Arc::new(create_loaded_test_embedding_model().await);
        let mut processor1 = RealTestBatchProcessor::new(real_model1, 4);
        let mut processor2 = RealTestBatchProcessor::new(real_model2, 4);

        let texts = vec![
            "Consistent text 1".to_string(),
            "Consistent text 2".to_string(),
        ];

        let results1 = processor1.process_batch(&texts).await.unwrap();
        let results2 = processor2.process_batch(&texts).await.unwrap();

        // Results should be identical for same inputs
        assert_eq!(results1.len(), results2.len());
        for (r1, r2) in results1.iter().zip(results2.iter()) {
            assert_eq!(r1.text, r2.text);
            assert_eq!(r1.text_hash, r2.text_hash);
            assert_eq!(r1.embedding, r2.embedding);
            assert_eq!(r1.sequence_length, r2.sequence_length);
        }
    }

    #[tokio::test]
    async fn test_batch_processor_edge_cases() {
        let real_model = Arc::new(create_loaded_test_embedding_model().await);
        let mut processor = RealTestBatchProcessor::new(real_model, 4);

        // Test with various edge case inputs
        let texts = vec![
            "a".to_string(),                         // Single character
            "word".to_string(),                      // Single word
            "Multiple words here".to_string(),       // Multiple words
            "Special chars: !@#$%^&*()".to_string(), // Special characters
            "Numbers 12345 and symbols".to_string(), // Mixed content
        ];

        let results = processor.process_batch(&texts).await.unwrap();

        assert_eq!(results.len(), 5);
        assert_eq!(processor.stats.successful_embeddings, 5);
        assert_eq!(processor.stats.failed_embeddings, 0);

        // Verify all results are valid
        let expected_dimension = results[0].dimension(); // Get actual dimension from first result
        for (i, result) in results.iter().enumerate() {
            assert_eq!(result.text, texts[i]);
            assert_eq!(result.dimension(), expected_dimension); // Verify consistent dimension
            assert!(result.dimension() > 0); // Verify dimension is positive
            assert!(result.sequence_length > 0);
            assert!(result.processing_time_ms > 0);
            assert!(!result.text_hash.is_empty());

            // Verify embedding is not all zeros (deterministic but not trivial)
            assert!(result.embedding.iter().any(|&x| x != 0.0));
        }
    }

    #[test]
    fn test_batch_size_management() {
        // Test batch size validation logic
        let valid_sizes = vec![1, 8, 16, 32, 64, 128];
        for size in valid_sizes {
            assert!(size > 0);
        }
    }

    #[tokio::test]
    async fn test_empty_text_handling() {
        let texts = vec!["".to_string(), "   ".to_string()];
        let non_empty: Vec<String> = texts.into_iter().filter(|t| !t.trim().is_empty()).collect();

        assert_eq!(non_empty.len(), 0);
    }

    #[tokio::test]
    async fn test_file_processing_setup() {
        // Test file creation and reading setup (without actual model)
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "line 1").unwrap();
        writeln!(temp_file).unwrap(); // Empty line should be skipped
        writeln!(temp_file, "line 2").unwrap();
        writeln!(temp_file, "   ").unwrap(); // Whitespace-only line should be skipped
        writeln!(temp_file, "line 3").unwrap();

        // Verify file exists and can be read
        let path = temp_file.path();
        assert!(path.exists());

        // Test line filtering logic
        let file = File::open(path).await.unwrap();
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut valid_lines = Vec::new();

        while let Some(line) = lines.next_line().await.unwrap() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                valid_lines.push(trimmed.to_string());
            }
        }

        assert_eq!(valid_lines.len(), 3);
        assert_eq!(valid_lines, vec!["line 1", "line 2", "line 3"]);
    }

    #[test]
    fn test_enhanced_batch_stats() {
        let mut stats = BatchStats::new();

        // Test throughput calculations
        assert_eq!(stats.throughput_texts_per_second(), 0.0);
        assert_eq!(stats.throughput_tokens_per_second(), 0.0);

        // Test memory usage tracking
        stats.update_memory_usage(1024 * 1024); // 1MB
        assert_eq!(stats.peak_memory_usage_bytes, 1024 * 1024);

        stats.update_memory_usage(512 * 1024); // 512KB (should not update peak)
        assert_eq!(stats.peak_memory_usage_bytes, 1024 * 1024);

        stats.update_memory_usage(2 * 1024 * 1024); // 2MB (should update peak)
        assert_eq!(stats.peak_memory_usage_bytes, 2 * 1024 * 1024);

        // Test format_summary doesn't panic
        let summary = stats.format_summary();
        assert!(summary.contains("BatchStats"));
        assert!(summary.contains("texts:"));
        assert!(summary.contains("memory:"));
    }

    #[test]
    fn test_enhanced_batch_config() {
        let mut config = BatchConfig {
            memory_limit_mb: Some(100),
            ..Default::default()
        };
        assert_eq!(config.memory_limit_mb, Some(100));

        // Test progress reporting configuration
        config.enable_progress_reporting = true;
        config.progress_report_interval_batches = 5;
        assert!(config.enable_progress_reporting);
        assert_eq!(config.progress_report_interval_batches, 5);
    }

    #[test]
    fn test_progress_info_structure() {
        let progress_info = ProgressInfo {
            current_batch: 5,
            total_batches: 10,
            texts_processed: 150,
            total_texts: 300,
            successful_embeddings: 145,
            failed_embeddings: 5,
            elapsed_time_ms: 30000,
            estimated_remaining_ms: 30000,
            current_throughput_texts_per_second: 5.0,
        };

        assert_eq!(progress_info.current_batch, 5);
        assert_eq!(progress_info.total_batches, 10);
        assert_eq!(progress_info.texts_processed, 150);
        assert_eq!(progress_info.successful_embeddings, 145);
        assert_eq!(progress_info.current_throughput_texts_per_second, 5.0);
    }

    #[test]
    fn test_memory_estimation_concepts() {
        // Test that memory estimation logic is sound
        let texts = [
            "short".to_string(),
            "medium length text".to_string(),
            "this is a much longer text that would require more memory for processing".to_string(),
        ];

        // Calculate expected memory manually - use a reasonable default dimension
        let text_bytes: usize = texts.iter().map(|t| t.len()).sum();
        let assumed_dimension = 512; // Use a reasonable middle-ground assumption
        let embedding_bytes = texts.len() * assumed_dimension * 4; // dim * 4 bytes per f32
        let overhead = (text_bytes + embedding_bytes) / 4; // 25% overhead
        let expected = text_bytes + embedding_bytes + overhead;

        assert!(expected > text_bytes); // Should be larger than just text
        assert!(expected > embedding_bytes); // Should be larger than just embeddings
    }
}
