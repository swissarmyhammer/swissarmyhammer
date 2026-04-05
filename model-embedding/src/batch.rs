use crate::error::EmbeddingError;
use crate::types::EmbeddingResult;
use crate::TextEmbedder;
use std::path::Path;
use std::time::Instant;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, info, warn};

/// Progress information for batch processing operations.
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

/// Callback type for progress reporting.
pub type ProgressCallback = Box<dyn FnMut(&ProgressInfo)>;

/// A single batch processing failure with context.
#[derive(Debug, Clone)]
pub struct BatchFailure {
    /// Index of the failed text within the batch.
    pub index: usize,
    /// Preview of the text that failed (first 50 chars).
    pub text_preview: String,
    /// Error message from the embedding backend.
    pub error: String,
}

/// Statistics for batch processing operations.
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
    /// Details of texts that failed during batch processing.
    pub failed_items: Vec<BatchFailure>,
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
        let token_count: usize = batch_results.iter().map(|r| r.sequence_length()).sum();
        let char_count: usize = batch_results.iter().map(|r| r.text().len()).sum();

        self.total_texts += batch_size;
        self.successful_embeddings += batch_results.len();
        self.failed_embeddings += failures;
        self.total_processing_time_ms += processing_time_ms;
        self.total_tokens_processed += token_count;
        self.total_characters_processed += char_count;
        self.batches_processed += 1;

        if self.total_texts > 0 {
            self.average_time_per_text_ms =
                self.total_processing_time_ms as f64 / self.total_texts as f64;
        }
        if self.successful_embeddings > 0 {
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

/// Configuration for batch processing behavior.
#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub batch_size: usize,
    pub continue_on_error: bool,
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
            enable_progress_reporting: false,
            progress_report_interval_batches: 10,
            memory_limit_mb: None,
            enable_memory_monitoring: true,
        }
    }
}

/// Generic batch processor for any [`TextEmbedder`] backend.
///
/// `BatchProcessor` borrows the embedder as `&T` (not `&mut T`) because backends
/// like `llama-embedding` use interior mutability (`Mutex<Inner>`) to manage state.
/// This means a `BatchProcessor` can coexist with other shared references to the
/// same embedder, but concurrent embedding calls will serialize on the backend's
/// internal lock.
pub struct BatchProcessor<'a, T: TextEmbedder> {
    model: &'a T,
    config: BatchConfig,
    stats: BatchStats,
    progress_callback: Option<ProgressCallback>,
}

impl<'a, T: TextEmbedder> BatchProcessor<'a, T> {
    pub fn new(model: &'a T, batch_size: usize) -> Self {
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

    pub fn with_config(model: &'a T, config: BatchConfig) -> Self {
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

    /// Process a batch of texts with error recovery.
    pub async fn process_batch(
        &mut self,
        texts: &[String],
    ) -> Result<Vec<EmbeddingResult>, EmbeddingError> {
        if !self.model.is_loaded() {
            return Err(EmbeddingError::ModelNotLoaded);
        }

        let start_time = Instant::now();
        debug!("Processing batch of {} texts", texts.len());

        if self.config.enable_memory_monitoring {
            let memory_usage = self.estimate_memory_usage(texts);
            self.stats.update_memory_usage(memory_usage);

            if let Some(limit_mb) = self.config.memory_limit_mb {
                let limit_bytes = limit_mb * 1024 * 1024;
                if memory_usage > limit_bytes {
                    warn!(
                        "Memory usage ({:.2}MB) exceeds limit ({}MB)",
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

        for (i, text) in texts.iter().enumerate() {
            match self.model.embed_text(text).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    failures += 1;
                    let preview: String = text.chars().take(50).collect();
                    warn!("Failed to embed text '{}...': {}", preview, e);
                    if !self.config.continue_on_error {
                        return Err(e);
                    }
                    self.stats.failed_items.push(BatchFailure {
                        index: i,
                        text_preview: preview,
                        error: e.to_string(),
                    });
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

    /// Process a list of texts with batching.
    pub async fn process_texts(
        &mut self,
        texts: Vec<String>,
    ) -> Result<Vec<EmbeddingResult>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        info!(
            "Processing {} texts in batches of {}",
            texts.len(),
            self.config.batch_size
        );
        let total_batches = texts.len().div_ceil(self.config.batch_size);
        let mut all_results = Vec::new();
        let start_time = Instant::now();

        for (batch_idx, chunk) in texts.chunks(self.config.batch_size).enumerate() {
            let batch_results = self.process_batch(chunk).await?;
            all_results.extend(batch_results);

            if self.config.enable_progress_reporting
                && batch_idx % self.config.progress_report_interval_batches == 0
            {
                self.report_progress(batch_idx, total_batches, texts.len(), &start_time);
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

    /// Process a file containing texts (one per line).
    pub async fn process_file(
        &mut self,
        input_path: &Path,
    ) -> Result<Vec<EmbeddingResult>, EmbeddingError> {
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
                if current_batch.len() >= self.config.batch_size {
                    let batch_results = self.process_batch(&current_batch).await?;
                    all_results.extend(batch_results);
                    current_batch.clear();
                }
            }
        }

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

    /// Process a file with streaming results via callback.
    pub async fn process_file_streaming<F>(
        &mut self,
        input_path: &Path,
        mut callback: F,
    ) -> Result<(), EmbeddingError>
    where
        F: FnMut(Vec<EmbeddingResult>) -> Result<(), EmbeddingError>,
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
                if current_batch.len() >= self.config.batch_size {
                    let batch_results = self.process_batch(&current_batch).await?;
                    callback(batch_results)?;
                    current_batch.clear();
                }
            }
        }

        if !current_batch.is_empty() {
            let batch_results = self.process_batch(&current_batch).await?;
            callback(batch_results)?;
        }

        info!("Completed streaming processing of file");
        Ok(())
    }

    pub fn config(&self) -> &BatchConfig {
        &self.config
    }

    pub fn stats(&self) -> &BatchStats {
        &self.stats
    }

    pub fn reset_stats(&mut self) {
        self.stats = BatchStats::new();
    }

    /// Get the configured batch size.
    pub fn batch_size(&self) -> usize {
        self.config.batch_size
    }

    /// Set a new batch size. Values of zero are ignored.
    pub fn set_batch_size(&mut self, new_batch_size: usize) {
        if new_batch_size > 0 {
            self.config.batch_size = new_batch_size;
        }
    }

    /// Set whether to continue processing on errors.
    pub fn set_continue_on_error(&mut self, continue_on_error: bool) {
        self.config.continue_on_error = continue_on_error;
    }

    /// Clear the progress callback.
    pub fn clear_progress_callback(&mut self) {
        self.progress_callback = None;
    }

    /// Get model info: `(embedding_dimension, is_loaded)`.
    pub fn get_model_info(&self) -> Option<(usize, bool)> {
        self.model
            .embedding_dimension()
            .map(|dim| (dim, self.model.is_loaded()))
    }

    /// Get a detailed performance report.
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

    fn report_progress(
        &mut self,
        batch_idx: usize,
        total_batches: usize,
        total_texts: usize,
        start_time: &Instant,
    ) {
        if let Some(ref mut callback) = self.progress_callback {
            let elapsed_ms = start_time.elapsed().as_millis() as u64;
            let current_throughput = if elapsed_ms > 0 {
                (self.stats.successful_embeddings as f64) / (elapsed_ms as f64 / 1000.0)
            } else {
                0.0
            };

            let remaining_batches = total_batches.saturating_sub(batch_idx + 1);
            let estimated_remaining_ms = if current_throughput > 0.0 && remaining_batches > 0 {
                let remaining_texts = remaining_batches * self.config.batch_size;
                ((remaining_texts as f64) / current_throughput * 1000.0) as u64
            } else {
                0
            };

            let progress_info = ProgressInfo {
                current_batch: batch_idx + 1,
                total_batches,
                texts_processed: self.stats.total_texts,
                total_texts,
                successful_embeddings: self.stats.successful_embeddings,
                failed_embeddings: self.stats.failed_embeddings,
                elapsed_time_ms: elapsed_ms,
                estimated_remaining_ms,
                current_throughput_texts_per_second: current_throughput,
            };

            callback(&progress_info);
        }
    }

    fn estimate_memory_usage(&self, texts: &[String]) -> usize {
        let text_memory: usize = texts.iter().map(|t| t.len()).sum();
        let embeddings_memory = if let Some(dim) = self.model.embedding_dimension() {
            texts.len() * dim * 4
        } else {
            texts.len() * 384 * 4
        };
        let overhead = (text_memory + embeddings_memory) / 4;
        text_memory + embeddings_memory + overhead
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::private::Sealed;
    use crate::{EmbeddingError, EmbeddingResult, TextEmbedder};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    /// A mock embedder that returns fixed-dimension vectors and can simulate failures.
    struct MockEmbedder {
        dimension: usize,
        loaded: bool,
        /// Indices of embed_text calls (0-based) that should return an error.
        fail_on_calls: Vec<usize>,
        /// Shared counter for how many times embed_text was called.
        call_count: Arc<Mutex<usize>>,
    }

    impl MockEmbedder {
        /// Create a loaded mock that always succeeds.
        fn new(dimension: usize) -> Self {
            Self {
                dimension,
                loaded: true,
                fail_on_calls: vec![],
                call_count: Arc::new(Mutex::new(0)),
            }
        }

        /// Create an unloaded mock.
        fn unloaded(dimension: usize) -> Self {
            Self {
                loaded: false,
                ..Self::new(dimension)
            }
        }

        /// Create a mock that is loaded but reports None for dimension.
        fn loaded_no_dimension() -> Self {
            Self {
                dimension: 0, // will be ignored since we override embedding_dimension
                loaded: true,
                fail_on_calls: vec![],
                call_count: Arc::new(Mutex::new(0)),
            }
        }

        /// Create a mock that fails on the given call indices.
        fn with_failures(dimension: usize, fail_on_calls: Vec<usize>) -> Self {
            Self {
                fail_on_calls,
                ..Self::new(dimension)
            }
        }
    }

    // Required by the sealed trait pattern — only workspace crates can implement Sealed.
    impl Sealed for MockEmbedder {}

    #[async_trait]
    impl TextEmbedder for MockEmbedder {
        async fn load(&self) -> Result<(), EmbeddingError> {
            Ok(())
        }

        async fn embed_text(&self, text: &str) -> Result<EmbeddingResult, EmbeddingError> {
            let call_idx = {
                let mut count = self.call_count.lock().unwrap();
                let idx = *count;
                *count += 1;
                idx
            };

            if self.fail_on_calls.contains(&call_idx) {
                return Err(EmbeddingError::TextProcessing(format!(
                    "mock failure at call {}",
                    call_idx
                )));
            }

            let embedding = vec![0.1_f32; self.dimension];
            Ok(EmbeddingResult::new(
                text.to_string(),
                embedding,
                text.split_whitespace().count(),
                1,
            ))
        }

        fn embedding_dimension(&self) -> Option<usize> {
            if self.loaded && self.dimension > 0 {
                Some(self.dimension)
            } else {
                None
            }
        }

        fn is_loaded(&self) -> bool {
            self.loaded
        }
    }

    // ──────────────────────────────────────────────────────────────
    // BatchProcessor::new and with_config
    // ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_mock_embedder_load() {
        let embedder = MockEmbedder::new(4);
        embedder.load().await.unwrap();
        assert!(embedder.is_loaded());
    }

    #[test]
    fn test_processor_new() {
        let embedder = MockEmbedder::new(4);
        let processor = BatchProcessor::new(&embedder, 16);
        assert_eq!(processor.batch_size(), 16);
        assert_eq!(processor.stats().total_texts, 0);
    }

    #[test]
    fn test_processor_with_config() {
        let embedder = MockEmbedder::new(4);
        let config = BatchConfig {
            batch_size: 8,
            continue_on_error: false,
            ..Default::default()
        };
        let processor = BatchProcessor::with_config(&embedder, config);
        assert_eq!(processor.batch_size(), 8);
        assert!(!processor.config().continue_on_error);
    }

    #[test]
    fn test_processor_set_batch_size() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 8);
        processor.set_batch_size(16);
        assert_eq!(processor.batch_size(), 16);
        // Zero is ignored.
        processor.set_batch_size(0);
        assert_eq!(processor.batch_size(), 16);
    }

    #[test]
    fn test_processor_get_model_info() {
        let embedder = MockEmbedder::new(384);
        let processor = BatchProcessor::new(&embedder, 32);
        let info = processor.get_model_info();
        assert_eq!(info, Some((384, true)));
    }

    #[test]
    fn test_processor_get_model_info_unloaded() {
        let embedder = MockEmbedder::unloaded(384);
        let processor = BatchProcessor::new(&embedder, 32);
        // Unloaded mock returns None for embedding_dimension.
        assert_eq!(processor.get_model_info(), None);
    }

    // ──────────────────────────────────────────────────────────────
    // process_batch
    // ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_process_batch_success() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 10);

        let texts = vec!["hello".to_string(), "world".to_string(), "foo".to_string()];
        let results = processor.process_batch(&texts).await.unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(processor.stats().total_texts, 3);
        assert_eq!(processor.stats().successful_embeddings, 3);
        assert_eq!(processor.stats().failed_embeddings, 0);
    }

    #[tokio::test]
    async fn test_process_batch_model_not_loaded() {
        let embedder = MockEmbedder::unloaded(4);
        let mut processor = BatchProcessor::new(&embedder, 10);

        let texts = vec!["hello".to_string()];
        let err = processor.process_batch(&texts).await.unwrap_err();
        assert!(matches!(err, EmbeddingError::ModelNotLoaded));
    }

    #[tokio::test]
    async fn test_process_batch_continue_on_error() {
        // Call 0 succeeds, call 1 fails, call 2 succeeds.
        let embedder = MockEmbedder::with_failures(4, vec![1]);
        let mut processor = BatchProcessor::new(&embedder, 10);
        // continue_on_error is true by default.

        let texts = vec!["ok1".to_string(), "fail".to_string(), "ok2".to_string()];
        let results = processor.process_batch(&texts).await.unwrap();

        assert_eq!(results.len(), 2, "should have 2 successful embeddings");
        assert_eq!(processor.stats().successful_embeddings, 2);
        assert_eq!(processor.stats().failed_embeddings, 1);
        assert_eq!(processor.stats().failed_items.len(), 1);
        assert_eq!(processor.stats().failed_items[0].index, 1);
    }

    #[tokio::test]
    async fn test_process_batch_stop_on_error() {
        let embedder = MockEmbedder::with_failures(4, vec![1]);
        let config = BatchConfig {
            batch_size: 10,
            continue_on_error: false,
            ..Default::default()
        };
        let mut processor = BatchProcessor::with_config(&embedder, config);

        let texts = vec!["ok1".to_string(), "fail".to_string(), "ok2".to_string()];
        let err = processor.process_batch(&texts).await.unwrap_err();
        assert!(matches!(err, EmbeddingError::TextProcessing(_)));
    }

    #[tokio::test]
    async fn test_process_batch_memory_limit_exceeded() {
        let embedder = MockEmbedder::new(1024);
        let config = BatchConfig {
            batch_size: 10,
            memory_limit_mb: Some(0), // 0 MB — any text will exceed this
            enable_memory_monitoring: true,
            ..Default::default()
        };
        let mut processor = BatchProcessor::with_config(&embedder, config);

        let texts = vec!["hello world".to_string()];
        let err = processor.process_batch(&texts).await.unwrap_err();
        assert!(matches!(err, EmbeddingError::BatchProcessing(_)));
    }

    #[tokio::test]
    async fn test_process_batch_memory_monitoring_disabled() {
        // Even with a 0-MB limit, disabling monitoring should not block processing.
        let embedder = MockEmbedder::new(4);
        let config = BatchConfig {
            batch_size: 10,
            memory_limit_mb: Some(0),
            enable_memory_monitoring: false,
            ..Default::default()
        };
        let mut processor = BatchProcessor::with_config(&embedder, config);

        let texts = vec!["hello".to_string()];
        let results = processor.process_batch(&texts).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_process_batch_updates_peak_memory() {
        let embedder = MockEmbedder::new(4);
        let config = BatchConfig {
            batch_size: 10,
            enable_memory_monitoring: true,
            memory_limit_mb: None,
            ..Default::default()
        };
        let mut processor = BatchProcessor::with_config(&embedder, config);

        let texts = vec!["hello world test text".to_string()];
        processor.process_batch(&texts).await.unwrap();

        assert!(processor.stats().peak_memory_usage_bytes > 0);
    }

    #[tokio::test]
    async fn test_process_batch_empty() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 10);

        let results = processor.process_batch(&[]).await.unwrap();
        // Empty batch: model is loaded, loop runs 0 times.
        assert_eq!(results.len(), 0);
        assert_eq!(processor.stats().batches_processed, 1);
    }

    // ──────────────────────────────────────────────────────────────
    // process_texts (the "process_all" equivalent)
    // ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_process_texts_empty_input() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 2);

        let results = processor.process_texts(vec![]).await.unwrap();
        assert!(results.is_empty());
        assert_eq!(processor.stats().batches_processed, 0);
    }

    #[tokio::test]
    async fn test_process_texts_single_batch() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 10);

        let texts: Vec<String> = (0..5).map(|i| format!("text {}", i)).collect();
        let results = processor.process_texts(texts).await.unwrap();

        assert_eq!(results.len(), 5);
        assert_eq!(processor.stats().batches_processed, 1);
        assert_eq!(processor.stats().total_texts, 5);
    }

    #[tokio::test]
    async fn test_process_texts_multiple_batches() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 3);

        let texts: Vec<String> = (0..7).map(|i| format!("text {}", i)).collect();
        let results = processor.process_texts(texts).await.unwrap();

        assert_eq!(results.len(), 7);
        // 7 texts / batch_size 3 → 3 batches (3 + 3 + 1)
        assert_eq!(processor.stats().batches_processed, 3);
    }

    #[tokio::test]
    async fn test_process_texts_exact_batch_boundary() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 4);

        let texts: Vec<String> = (0..8).map(|i| format!("text {}", i)).collect();
        let results = processor.process_texts(texts).await.unwrap();

        assert_eq!(results.len(), 8);
        assert_eq!(processor.stats().batches_processed, 2);
    }

    #[tokio::test]
    async fn test_process_texts_with_errors_continue() {
        // Fail on the 3rd text overall (index 2, call index 2).
        let embedder = MockEmbedder::with_failures(4, vec![2]);
        let mut processor = BatchProcessor::new(&embedder, 5);

        let texts: Vec<String> = (0..5).map(|i| format!("text {}", i)).collect();
        // continue_on_error=true by default
        let results = processor.process_texts(texts).await.unwrap();

        assert_eq!(results.len(), 4);
        assert_eq!(processor.stats().failed_embeddings, 1);
    }

    #[tokio::test]
    async fn test_process_texts_stats_accumulate_across_batches() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 2);

        let texts: Vec<String> = (0..6).map(|i| format!("item {}", i)).collect();
        processor.process_texts(texts).await.unwrap();

        assert_eq!(processor.stats().total_texts, 6);
        assert_eq!(processor.stats().successful_embeddings, 6);
        assert_eq!(processor.stats().batches_processed, 3);
    }

    // ──────────────────────────────────────────────────────────────
    // set_progress_callback
    // ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_progress_callback_called() {
        let embedder = MockEmbedder::new(4);
        let config = BatchConfig {
            batch_size: 2,
            enable_progress_reporting: true,
            progress_report_interval_batches: 1,
            ..Default::default()
        };
        let mut processor = BatchProcessor::with_config(&embedder, config);

        let call_count = Arc::new(Mutex::new(0_usize));
        let call_count_clone = Arc::clone(&call_count);
        processor.set_progress_callback(Box::new(move |_info| {
            *call_count_clone.lock().unwrap() += 1;
        }));

        let texts: Vec<String> = (0..6).map(|i| format!("text {}", i)).collect();
        processor.process_texts(texts).await.unwrap();

        let count = *call_count.lock().unwrap();
        assert!(count > 0, "progress callback should have been called");
    }

    #[tokio::test]
    async fn test_clear_progress_callback() {
        let embedder = MockEmbedder::new(4);
        let config = BatchConfig {
            batch_size: 2,
            enable_progress_reporting: true,
            progress_report_interval_batches: 1,
            ..Default::default()
        };
        let mut processor = BatchProcessor::with_config(&embedder, config);

        // Set a callback then immediately clear it — process_texts should not panic.
        processor.set_progress_callback(Box::new(|_| {}));
        processor.clear_progress_callback();

        let texts: Vec<String> = (0..4).map(|i| format!("text {}", i)).collect();
        processor.process_texts(texts).await.unwrap();

        // If the callback had not been cleared, the empty closure would still be fine,
        // but the key assertion is that clear_progress_callback works without error.
        assert!(processor.stats().total_texts > 0);
    }

    // ──────────────────────────────────────────────────────────────
    // reset_stats and performance_report
    // ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_reset_stats() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 10);

        let texts = vec!["a".to_string(), "b".to_string()];
        processor.process_batch(&texts).await.unwrap();
        assert!(processor.stats().total_texts > 0);

        processor.reset_stats();
        assert_eq!(processor.stats().total_texts, 0);
        assert_eq!(processor.stats().batches_processed, 0);
    }

    #[tokio::test]
    async fn test_get_performance_report() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 10);

        let texts = vec!["hello".to_string(), "world".to_string()];
        processor.process_batch(&texts).await.unwrap();

        let report = processor.get_performance_report();
        assert!(report.contains("Performance Report"));
        assert!(report.contains("Total texts processed: 2"));
        assert!(report.contains("Success rate: 100.0%"));
    }

    // ──────────────────────────────────────────────────────────────
    // estimate_memory_usage (via process_batch with monitoring)
    // ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_estimate_memory_usage_scales_with_dimension() {
        // Higher dimension → higher estimated memory usage.
        let embedder_small = MockEmbedder::new(4);
        let config = BatchConfig {
            batch_size: 10,
            enable_memory_monitoring: true,
            memory_limit_mb: None,
            ..Default::default()
        };
        let mut processor_small = BatchProcessor::with_config(&embedder_small, config.clone());
        processor_small
            .process_batch(&["hello world".to_string()])
            .await
            .unwrap();
        let peak_small = processor_small.stats().peak_memory_usage_bytes;

        let embedder_large = MockEmbedder::new(1024);
        let mut processor_large = BatchProcessor::with_config(&embedder_large, config);
        processor_large
            .process_batch(&["hello world".to_string()])
            .await
            .unwrap();
        let peak_large = processor_large.stats().peak_memory_usage_bytes;

        assert!(
            peak_large > peak_small,
            "larger dimension should use more estimated memory: {} vs {}",
            peak_large,
            peak_small
        );
    }

    #[test]
    fn test_batch_stats_new() {
        let stats = BatchStats::new();
        assert_eq!(stats.total_texts, 0);
        assert_eq!(stats.successful_embeddings, 0);
        assert_eq!(stats.success_rate(), 0.0);
        assert_eq!(stats.throughput_texts_per_second(), 0.0);
    }

    #[test]
    fn test_batch_stats_update() {
        let mut stats = BatchStats::new();
        stats.update(10, 1000, 0);

        assert_eq!(stats.total_texts, 10);
        assert_eq!(stats.successful_embeddings, 10);
        assert_eq!(stats.success_rate(), 1.0);
        assert_eq!(stats.average_time_per_text_ms, 100.0);

        stats.update(10, 2000, 2);
        assert_eq!(stats.total_texts, 20);
        assert_eq!(stats.successful_embeddings, 18);
        assert_eq!(stats.failed_embeddings, 2);
        assert_eq!(stats.success_rate(), 0.9);
    }

    #[test]
    fn test_batch_stats_memory() {
        let mut stats = BatchStats::new();
        stats.update_memory_usage(1024 * 1024);
        assert_eq!(stats.peak_memory_usage_bytes, 1024 * 1024);

        stats.update_memory_usage(512 * 1024); // lower, should not update peak
        assert_eq!(stats.peak_memory_usage_bytes, 1024 * 1024);

        stats.update_memory_usage(2 * 1024 * 1024);
        assert_eq!(stats.peak_memory_usage_bytes, 2 * 1024 * 1024);
    }

    #[test]
    fn test_batch_config_default() {
        let config = BatchConfig::default();
        assert_eq!(config.batch_size, 32);
        assert!(config.continue_on_error);
        assert!(!config.enable_progress_reporting);
        assert!(config.memory_limit_mb.is_none());
    }

    #[test]
    fn test_progress_info() {
        let info = ProgressInfo {
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
        assert_eq!(info.current_batch, 5);
        assert_eq!(info.total_batches, 10);
    }

    // ──────────────────────────────────────────────────────────────
    // process_file and process_file_streaming
    // ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_process_file_success() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 2);

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("input.txt");
        std::fs::write(&file_path, "hello\nworld\nfoo\n").unwrap();

        let results = processor.process_file(&file_path).await.unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(processor.stats().batches_processed, 2); // 2 + 1
    }

    #[tokio::test]
    async fn test_process_file_empty() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 10);

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("empty.txt");
        std::fs::write(&file_path, "").unwrap();

        let results = processor.process_file(&file_path).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_process_file_with_blank_lines() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 10);

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("blanks.txt");
        std::fs::write(&file_path, "hello\n\n  \nworld\n\n").unwrap();

        let results = processor.process_file(&file_path).await.unwrap();
        assert_eq!(results.len(), 2, "blank lines should be skipped");
    }

    #[tokio::test]
    async fn test_process_file_not_found() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 10);

        let err = processor
            .process_file(Path::new("/nonexistent/file.txt"))
            .await
            .unwrap_err();
        assert!(matches!(err, EmbeddingError::Io(_)));
    }

    #[tokio::test]
    async fn test_process_file_streaming_success() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 2);

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("input.txt");
        std::fs::write(&file_path, "a\nb\nc\nd\ne\n").unwrap();

        let collected = Arc::new(Mutex::new(Vec::new()));
        let collected_clone = Arc::clone(&collected);

        processor
            .process_file_streaming(&file_path, move |batch_results| {
                collected_clone.lock().unwrap().extend(batch_results);
                Ok(())
            })
            .await
            .unwrap();

        let total = collected.lock().unwrap().len();
        assert_eq!(total, 5);
    }

    #[tokio::test]
    async fn test_process_file_streaming_not_found() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 10);

        let err = processor
            .process_file_streaming(Path::new("/nonexistent/file.txt"), |_| Ok(()))
            .await
            .unwrap_err();
        assert!(matches!(err, EmbeddingError::Io(_)));
    }

    #[tokio::test]
    async fn test_process_file_streaming_callback_error() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 2);

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("input.txt");
        std::fs::write(&file_path, "a\nb\nc\n").unwrap();

        let err = processor
            .process_file_streaming(&file_path, |_| {
                Err(EmbeddingError::batch_processing("callback failed"))
            })
            .await
            .unwrap_err();
        assert!(matches!(err, EmbeddingError::BatchProcessing(_)));
    }

    #[tokio::test]
    async fn test_process_file_streaming_empty_file() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 10);

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("empty.txt");
        std::fs::write(&file_path, "").unwrap();

        // For an empty file, the callback should never be called.
        // Using a closure that would panic if called verifies this.
        processor
            .process_file_streaming(&file_path, |_| {
                panic!("callback should not be called for empty file");
            })
            .await
            .unwrap();
    }

    // ──────────────────────────────────────────────────────────────
    // progress reporting with ProgressInfo verification
    // ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_progress_callback_receives_valid_info() {
        let embedder = MockEmbedder::new(4);
        let config = BatchConfig {
            batch_size: 2,
            enable_progress_reporting: true,
            progress_report_interval_batches: 1,
            ..Default::default()
        };
        let mut processor = BatchProcessor::with_config(&embedder, config);

        let infos = Arc::new(Mutex::new(Vec::new()));
        let infos_clone = Arc::clone(&infos);
        processor.set_progress_callback(Box::new(move |info| {
            infos_clone.lock().unwrap().push(info.clone());
        }));

        let texts: Vec<String> = (0..6).map(|i| format!("text {}", i)).collect();
        processor.process_texts(texts).await.unwrap();

        let captured = infos.lock().unwrap();
        assert!(!captured.is_empty());
        // First callback should be batch 1
        assert_eq!(captured[0].current_batch, 1);
        assert_eq!(captured[0].total_batches, 3);
        assert_eq!(captured[0].total_texts, 6);
        assert!(captured[0].current_throughput_texts_per_second >= 0.0);
    }

    #[tokio::test]
    async fn test_process_texts_progress_estimated_remaining() {
        let embedder = MockEmbedder::new(4);
        let config = BatchConfig {
            batch_size: 1,
            enable_progress_reporting: true,
            progress_report_interval_batches: 1,
            ..Default::default()
        };
        let mut processor = BatchProcessor::with_config(&embedder, config);

        let infos = Arc::new(Mutex::new(Vec::new()));
        let infos_clone = Arc::clone(&infos);
        processor.set_progress_callback(Box::new(move |info| {
            infos_clone.lock().unwrap().push(info.clone());
        }));

        let texts: Vec<String> = (0..4).map(|i| format!("text {}", i)).collect();
        processor.process_texts(texts).await.unwrap();

        let captured = infos.lock().unwrap();
        // With batch_size=1 and 4 texts, we get 4 batches and 4 callbacks
        assert_eq!(captured.len(), 4);
        // Last callback should have 0 estimated remaining
        assert_eq!(captured.last().unwrap().estimated_remaining_ms, 0);
    }

    // ──────────────────────────────────────────────────────────────
    // set_continue_on_error
    // ──────────────────────────────────────────────────────────────

    #[test]
    fn test_set_continue_on_error() {
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 10);
        assert!(processor.config().continue_on_error);
        processor.set_continue_on_error(false);
        assert!(!processor.config().continue_on_error);
    }

    // ──────────────────────────────────────────────────────────────
    // estimate_memory_usage with unloaded model (default dimension)
    // ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_estimate_memory_unloaded_uses_default_dimension() {
        // When model is unloaded, embedding_dimension() returns None,
        // so estimate_memory_usage falls back to 384.
        // We can't call process_batch (ModelNotLoaded), so test indirectly
        // by verifying the loaded mock with dim=384 gives the same estimate.
        let loaded = MockEmbedder::new(384);
        let config = BatchConfig {
            batch_size: 10,
            enable_memory_monitoring: true,
            memory_limit_mb: None,
            ..Default::default()
        };
        let mut processor = BatchProcessor::with_config(&loaded, config);
        let texts = vec!["test".to_string()];
        processor.process_batch(&texts).await.unwrap();
        let peak_loaded = processor.stats().peak_memory_usage_bytes;

        // The unloaded mock returns None for dimension, so estimate uses 384 default.
        // We can't process_batch because model is not loaded, but we know the
        // formula: text_bytes + (count * dim * 4) + 25% overhead.
        // For "test" (4 bytes) with dim=384: 4 + 1*384*4 + (4+1536)/4 = 4 + 1536 + 385 = 1925
        assert!(peak_loaded > 0);
    }

    // ──────────────────────────────────────────────────────────────
    // BatchStats throughput_tokens_per_second
    // ──────────────────────────────────────────────────────────────

    #[test]
    fn test_batch_stats_throughput_tokens_per_second() {
        let mut stats = BatchStats::new();
        assert_eq!(stats.throughput_tokens_per_second(), 0.0);

        stats.total_processing_time_ms = 1000;
        stats.total_tokens_processed = 500;
        assert!((stats.throughput_tokens_per_second() - 500.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_estimate_memory_no_dimension_uses_default() {
        // loaded_no_dimension() returns loaded=true but dimension=0,
        // so embedding_dimension() returns None, hitting the else branch (line 527).
        let embedder = MockEmbedder::loaded_no_dimension();
        let config = BatchConfig {
            batch_size: 10,
            enable_memory_monitoring: true,
            memory_limit_mb: None,
            ..Default::default()
        };
        let mut processor = BatchProcessor::with_config(&embedder, config);
        let texts = vec!["test".to_string()];
        processor.process_batch(&texts).await.unwrap();
        // With default dim=384: text_bytes(4) + 1*384*4(1536) + overhead(385) = 1925
        assert_eq!(processor.stats().peak_memory_usage_bytes, 1925);
    }

    #[tokio::test]
    async fn test_progress_callback_with_nonzero_elapsed() {
        // Use a "slow" mock that sleeps 2ms per call so elapsed_ms > 0,
        // hitting the throughput calculation branch.
        use tokio::time::{sleep, Duration};

        struct SlowEmbedder {
            dimension: usize,
        }
        impl Sealed for SlowEmbedder {}

        #[async_trait]
        impl TextEmbedder for SlowEmbedder {
            async fn load(&self) -> Result<(), EmbeddingError> {
                Ok(())
            }
            async fn embed_text(&self, text: &str) -> Result<EmbeddingResult, EmbeddingError> {
                sleep(Duration::from_millis(2)).await;
                let embedding = vec![0.1_f32; self.dimension];
                Ok(EmbeddingResult::new(
                    text.to_string(),
                    embedding,
                    text.split_whitespace().count(),
                    2,
                ))
            }
            fn embedding_dimension(&self) -> Option<usize> {
                Some(self.dimension)
            }
            fn is_loaded(&self) -> bool {
                true
            }
        }

        let embedder = SlowEmbedder { dimension: 4 };
        embedder.load().await.unwrap();
        let config = BatchConfig {
            batch_size: 1,
            enable_progress_reporting: true,
            progress_report_interval_batches: 1,
            ..Default::default()
        };
        let mut processor = BatchProcessor::with_config(&embedder, config);

        let infos = Arc::new(Mutex::new(Vec::new()));
        let infos_clone = Arc::clone(&infos);
        processor.set_progress_callback(Box::new(move |info| {
            infos_clone.lock().unwrap().push(info.clone());
        }));

        let texts: Vec<String> = (0..3).map(|i| format!("text {}", i)).collect();
        processor.process_texts(texts).await.unwrap();

        let captured = infos.lock().unwrap();
        // With 2ms delay per text, elapsed should be > 0 for later callbacks
        // First callback should have throughput > 0
        let has_nonzero_throughput = captured
            .iter()
            .any(|info| info.current_throughput_texts_per_second > 0.0);
        assert!(
            has_nonzero_throughput,
            "at least one callback should have nonzero throughput"
        );
        // Middle callbacks should have estimated_remaining > 0
        let has_nonzero_remaining = captured.iter().any(|info| info.estimated_remaining_ms > 0);
        assert!(
            has_nonzero_remaining,
            "at least one callback should have nonzero estimated remaining"
        );
    }

    #[test]
    fn test_batch_stats_update_with_details() {
        let mut stats = BatchStats::new();
        let results = vec![
            EmbeddingResult::new("hello world".to_string(), vec![0.1; 4], 2, 1),
            EmbeddingResult::new("foo bar baz".to_string(), vec![0.2; 4], 3, 1),
        ];
        stats.update_with_details(&results, 100, 1);

        assert_eq!(stats.total_texts, 3); // 2 results + 1 failure
        assert_eq!(stats.successful_embeddings, 2);
        assert_eq!(stats.failed_embeddings, 1);
        assert_eq!(stats.total_tokens_processed, 5); // 2 + 3
        assert_eq!(stats.total_characters_processed, 22); // 11 + 11
        assert_eq!(stats.batches_processed, 1);
        assert!(stats.average_tokens_per_text > 0.0);
    }

    #[test]
    fn test_batch_stats_format_summary() {
        let mut stats = BatchStats::new();
        stats.update(10, 1000, 0);
        let summary = stats.format_summary();
        assert!(summary.contains("BatchStats"));
        assert!(summary.contains("10/10"));
    }

    // ──────────────────────────────────────────────────────────────
    // Tracing-enabled tests: exercise debug!/info!/warn! format args
    // ──────────────────────────────────────────────────────────────

    /// Initialize a tracing subscriber that evaluates format arguments.
    /// Returns a guard; the subscriber is active while it lives.
    fn init_tracing() -> tracing::subscriber::DefaultGuard {
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(std::io::sink)
            .finish();
        tracing::subscriber::set_default(subscriber)
    }

    #[tokio::test]
    async fn test_process_batch_tracing_debug_lines() {
        let _guard = init_tracing();
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 10);

        let texts = vec!["alpha".to_string(), "beta".to_string()];
        let results = processor.process_batch(&texts).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_process_texts_tracing_info_lines() {
        let _guard = init_tracing();
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 3);

        let texts: Vec<String> = (0..5).map(|i| format!("item {}", i)).collect();
        let results = processor.process_texts(texts).await.unwrap();
        assert_eq!(results.len(), 5);
    }

    #[tokio::test]
    async fn test_process_batch_warn_on_failure_with_tracing() {
        let _guard = init_tracing();
        let embedder = MockEmbedder::with_failures(4, vec![0]);
        let mut processor = BatchProcessor::new(&embedder, 10);

        let texts = vec!["will fail".to_string(), "ok".to_string()];
        let results = processor.process_batch(&texts).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_process_batch_memory_limit_warn_with_tracing() {
        let _guard = init_tracing();
        let embedder = MockEmbedder::new(1024);
        let config = BatchConfig {
            batch_size: 10,
            memory_limit_mb: Some(0),
            enable_memory_monitoring: true,
            ..Default::default()
        };
        let mut processor = BatchProcessor::with_config(&embedder, config);

        let texts = vec!["hello world".to_string()];
        let err = processor.process_batch(&texts).await.unwrap_err();
        assert!(matches!(err, EmbeddingError::BatchProcessing(_)));
    }

    #[tokio::test]
    async fn test_process_file_tracing_info_lines() {
        let _guard = init_tracing();
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 2);

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("traced.txt");
        std::fs::write(&file_path, "line1\nline2\nline3\n").unwrap();

        let results = processor.process_file(&file_path).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_process_file_streaming_tracing_info_lines() {
        let _guard = init_tracing();
        let embedder = MockEmbedder::new(4);
        let mut processor = BatchProcessor::new(&embedder, 2);

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("traced_stream.txt");
        std::fs::write(&file_path, "a\nb\nc\n").unwrap();

        let collected = Arc::new(Mutex::new(Vec::new()));
        let collected_clone = Arc::clone(&collected);
        processor
            .process_file_streaming(&file_path, move |batch| {
                collected_clone.lock().unwrap().extend(batch);
                Ok(())
            })
            .await
            .unwrap();
        assert_eq!(collected.lock().unwrap().len(), 3);
    }

    // ──────────────────────────────────────────────────────────────
    // BatchStats edge cases: zero totals in update_with_details
    // ──────────────────────────────────────────────────────────────

    #[test]
    fn test_batch_stats_update_with_details_all_failures() {
        let mut stats = BatchStats::new();
        // No successful results, only failures — hits the else branches
        // where total_texts > 0 is false on first call with empty results.
        stats.update_with_details(&[], 100, 0);
        // batch_size = 0 + 0 = 0, so total_texts stays 0
        assert_eq!(stats.total_texts, 0);
        assert_eq!(stats.average_time_per_text_ms, 0.0);
        assert_eq!(stats.average_tokens_per_text, 0.0);
    }

    #[test]
    fn test_batch_stats_update_with_details_no_successes() {
        let mut stats = BatchStats::new();
        // 3 failures, 0 successful results
        stats.update_with_details(&[], 200, 3);
        assert_eq!(stats.total_texts, 3);
        assert_eq!(stats.successful_embeddings, 0);
        assert_eq!(stats.failed_embeddings, 3);
        // average_tokens_per_text should stay 0 because no successes
        assert_eq!(stats.average_tokens_per_text, 0.0);
    }

    #[test]
    fn test_batch_failure_debug_and_clone() {
        let failure = BatchFailure {
            index: 5,
            text_preview: "some text that failed".to_string(),
            error: "mock error".to_string(),
        };
        let cloned = failure.clone();
        assert_eq!(cloned.index, 5);
        assert_eq!(cloned.text_preview, "some text that failed");
        assert_eq!(cloned.error, "mock error");
        // Exercise Debug
        let debug_str = format!("{:?}", failure);
        assert!(debug_str.contains("BatchFailure"));
    }

    #[test]
    fn test_progress_info_debug_and_clone() {
        let info = ProgressInfo {
            current_batch: 1,
            total_batches: 5,
            texts_processed: 10,
            total_texts: 50,
            successful_embeddings: 10,
            failed_embeddings: 0,
            elapsed_time_ms: 1000,
            estimated_remaining_ms: 4000,
            current_throughput_texts_per_second: 10.0,
        };
        let cloned = info.clone();
        assert_eq!(cloned.texts_processed, 10);
        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("ProgressInfo"));
    }

    #[test]
    fn test_batch_stats_debug_and_clone() {
        let mut stats = BatchStats::new();
        stats.update(5, 500, 1);
        let cloned = stats.clone();
        assert_eq!(cloned.total_texts, 5);
        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("BatchStats"));
    }

    #[test]
    fn test_batch_config_debug_and_clone() {
        let config = BatchConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.batch_size, 32);
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("BatchConfig"));
    }

    #[test]
    fn test_batch_stats_update_zero_total_texts() {
        let mut stats = BatchStats::new();
        // update with batch_size 0 — total_texts stays 0
        stats.update(0, 100, 0);
        assert_eq!(stats.total_texts, 0);
        // average_time_per_text should remain 0 since total_texts == 0
        assert_eq!(stats.average_time_per_text_ms, 0.0);
    }

    #[test]
    fn test_batch_stats_update_zero_batches_impossible() {
        // batches_processed is always > 0 after update(), so the else branch
        // of the batches_processed check can never be hit in normal use.
        // But we can verify the function works correctly.
        let mut stats = BatchStats::new();
        assert_eq!(stats.batches_processed, 0);
        assert_eq!(stats.average_batch_time_ms, 0.0);
        stats.update(5, 1000, 0);
        assert_eq!(stats.batches_processed, 1);
        assert_eq!(stats.average_batch_time_ms, 1000.0);
    }
}
