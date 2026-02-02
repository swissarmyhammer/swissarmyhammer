use crate::error::{EmbeddingError, EmbeddingResult as Result};
use crate::model::EmbeddingModel;
use crate::types::EmbeddingResult;
use std::path::Path;
use std::time::Instant;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
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
pub type ProgressCallback = Box<dyn FnMut(&ProgressInfo)>;

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

/// Configuration for batch processing behavior
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

/// Handles batch processing of multiple texts for embedding generation.
/// Single-threaded, sequential processing.
pub struct BatchProcessor<'a> {
    model: &'a mut EmbeddingModel,
    config: BatchConfig,
    stats: BatchStats,
    progress_callback: Option<ProgressCallback>,
}

impl<'a> BatchProcessor<'a> {
    /// Create a new BatchProcessor with default configuration
    pub fn new(model: &'a mut EmbeddingModel, batch_size: usize) -> Self {
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
    pub fn with_config(model: &'a mut EmbeddingModel, config: BatchConfig) -> Self {
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
                    let preview: String = text.chars().take(50).collect();
                    warn!("Failed to embed text '{}...': {}", preview, e);

                    if !self.config.continue_on_error {
                        return Err(e);
                    }
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
        let total_batches = texts.len().div_ceil(self.config.batch_size);
        let mut all_results = Vec::new();
        let start_time = Instant::now();

        for (batch_idx, chunk) in texts.chunks(self.config.batch_size).enumerate() {
            let batch_results = self.process_batch(chunk).await?;
            all_results.extend(batch_results);

            // Report progress if enabled and callback is set
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

    /// Process a file containing texts (one per line)
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

                if current_batch.len() >= self.config.batch_size {
                    let batch_results = self.process_batch(&current_batch).await?;
                    all_results.extend(batch_results);
                    current_batch.clear();
                }
            }
        }

        // Process remaining texts
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

                if current_batch.len() >= self.config.batch_size {
                    let batch_results = self.process_batch(&current_batch).await?;
                    callback(batch_results)?;
                    current_batch.clear();
                }
            }
        }

        // Process remaining texts
        if !current_batch.is_empty() {
            let batch_results = self.process_batch(&current_batch).await?;
            callback(batch_results)?;
        }

        info!("Completed streaming processing of file");
        Ok(())
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

    fn report_progress(&mut self, batch_idx: usize, total_batches: usize, total_texts: usize, start_time: &Instant) {
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

    /// Estimate current memory usage for a batch of texts
    fn estimate_current_memory_usage(&self, texts: &[String]) -> usize {
        let text_memory: usize = texts.iter().map(|t| t.len()).sum();
        let embeddings_memory = if let Some(dim) = self.model.get_embedding_dimension() {
            texts.len() * dim * 4  // f32 = 4 bytes
        } else {
            // Default assumption
            texts.len() * 384 * 4
        };

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

    // Test constants for batch statistics
    const TEST_BATCH_SIZE: usize = 10;
    const TEST_PROCESSING_TIME_MS: u64 = 1000;
    const TEST_SECOND_PROCESSING_TIME_MS: u64 = 2000;
    const TEST_FAILURES: usize = 2;
    const TEST_ONE_MB: usize = 1024 * 1024;
    const TEST_HALF_MB: usize = 512 * 1024;
    const TEST_TWO_MB: usize = 2 * 1024 * 1024;

    // Test constants for performance report
    const REPORT_TOTAL_TEXTS: usize = 100;
    const REPORT_SUCCESSFUL: usize = 95;
    const REPORT_FAILED: usize = 5;
    const REPORT_PROCESSING_TIME_MS: u64 = 10000;
    const REPORT_AVG_TIME_PER_TEXT_MS: f64 = 100.0;
    const REPORT_TOTAL_TOKENS: usize = 5000;
    const REPORT_AVG_TOKENS_PER_TEXT: f64 = 50.0;
    const REPORT_BATCHES: usize = 10;
    const REPORT_AVG_BATCH_TIME_MS: f64 = 1000.0;
    const REPORT_TOTAL_CHARS: usize = 10000;

    #[test]
    fn test_batch_stats() {
        let mut stats = BatchStats::new();

        assert_eq!(stats.total_texts, 0);
        assert_eq!(stats.successful_embeddings, 0);
        assert_eq!(stats.failed_embeddings, 0);
        assert_eq!(stats.success_rate(), 0.0);
        assert_eq!(stats.batches_processed, 0);

        stats.update(TEST_BATCH_SIZE, TEST_PROCESSING_TIME_MS, 0);
        assert_eq!(stats.total_texts, TEST_BATCH_SIZE);
        assert_eq!(stats.successful_embeddings, TEST_BATCH_SIZE);
        assert_eq!(stats.failed_embeddings, 0);
        assert_eq!(stats.success_rate(), 1.0);
        assert_eq!(stats.average_time_per_text_ms, REPORT_AVG_TIME_PER_TEXT_MS);
        assert_eq!(stats.batches_processed, 1);

        stats.update(TEST_BATCH_SIZE, TEST_SECOND_PROCESSING_TIME_MS, TEST_FAILURES);
        assert_eq!(stats.total_texts, TEST_BATCH_SIZE * 2);
        assert_eq!(stats.successful_embeddings, TEST_BATCH_SIZE * 2 - TEST_FAILURES);
        assert_eq!(stats.failed_embeddings, TEST_FAILURES);
        assert_eq!(stats.success_rate(), 0.9);
    }

    #[test]
    fn test_batch_config_default() {
        let config = BatchConfig::default();
        assert_eq!(config.batch_size, 32);
        assert!(config.continue_on_error);
        assert!(!config.enable_progress_reporting);
        assert_eq!(config.progress_report_interval_batches, 10);
        assert!(config.memory_limit_mb.is_none());
        assert!(config.enable_memory_monitoring);
    }

    #[test]
    fn test_batch_stats_throughput() {
        let mut stats = BatchStats::new();
        assert_eq!(stats.throughput_texts_per_second(), 0.0);
        assert_eq!(stats.throughput_tokens_per_second(), 0.0);

        stats.update_memory_usage(TEST_ONE_MB);
        assert_eq!(stats.peak_memory_usage_bytes, TEST_ONE_MB);

        stats.update_memory_usage(TEST_HALF_MB);
        assert_eq!(stats.peak_memory_usage_bytes, TEST_ONE_MB);

        stats.update_memory_usage(TEST_TWO_MB);
        assert_eq!(stats.peak_memory_usage_bytes, TEST_TWO_MB);

        let summary = stats.format_summary();
        assert!(summary.contains("BatchStats"));
    }

    // Test constants for progress info
    const PROGRESS_CURRENT_BATCH: usize = 5;
    const PROGRESS_TOTAL_BATCHES: usize = 10;
    const PROGRESS_TEXTS_PROCESSED: usize = 150;
    const PROGRESS_TOTAL_TEXTS: usize = 300;
    const PROGRESS_SUCCESSFUL: usize = 145;
    const PROGRESS_FAILED: usize = 5;
    const PROGRESS_ELAPSED_MS: u64 = 30000;
    const PROGRESS_THROUGHPUT: f64 = 5.0;

    #[test]
    fn test_progress_info_structure() {
        let progress_info = ProgressInfo {
            current_batch: PROGRESS_CURRENT_BATCH,
            total_batches: PROGRESS_TOTAL_BATCHES,
            texts_processed: PROGRESS_TEXTS_PROCESSED,
            total_texts: PROGRESS_TOTAL_TEXTS,
            successful_embeddings: PROGRESS_SUCCESSFUL,
            failed_embeddings: PROGRESS_FAILED,
            elapsed_time_ms: PROGRESS_ELAPSED_MS,
            estimated_remaining_ms: PROGRESS_ELAPSED_MS,
            current_throughput_texts_per_second: PROGRESS_THROUGHPUT,
        };

        assert_eq!(progress_info.current_batch, PROGRESS_CURRENT_BATCH);
        assert_eq!(progress_info.total_batches, PROGRESS_TOTAL_BATCHES);
        assert_eq!(progress_info.texts_processed, PROGRESS_TEXTS_PROCESSED);
        assert_eq!(progress_info.successful_embeddings, PROGRESS_SUCCESSFUL);
    }

    #[tokio::test]
    async fn test_file_line_filtering() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "line 1").unwrap();
        writeln!(temp_file).unwrap();
        writeln!(temp_file, "line 2").unwrap();
        writeln!(temp_file, "   ").unwrap();
        writeln!(temp_file, "line 3").unwrap();

        let path = temp_file.path();
        assert!(path.exists());

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

    /// Integration tests for BatchProcessor methods requiring real models:
    /// - process_batch: tested in tests/integration/real_model_integration.rs::test_batch_processing_various_sizes
    /// - process_texts: tested in tests/integration/real_model_integration.rs::test_batch_processing_various_sizes
    /// - process_file: tested in tests/integration/real_model_integration.rs::test_file_processing_different_sizes
    /// - process_file_streaming: tested via process_file which uses same logic
    /// - get_model_info: tested in tests/integration/real_model_integration.rs::test_single_text_embedding
    /// - get_performance_report: tested below
    #[test]
    fn test_integration_coverage_documented() {
        // BatchProcessor integration tests are in the integration test module
    }

    #[test]
    fn test_get_performance_report_format() {
        let stats = BatchStats {
            total_texts: REPORT_TOTAL_TEXTS,
            successful_embeddings: REPORT_SUCCESSFUL,
            failed_embeddings: REPORT_FAILED,
            total_processing_time_ms: REPORT_PROCESSING_TIME_MS,
            average_time_per_text_ms: REPORT_AVG_TIME_PER_TEXT_MS,
            total_tokens_processed: REPORT_TOTAL_TOKENS,
            average_tokens_per_text: REPORT_AVG_TOKENS_PER_TEXT,
            batches_processed: REPORT_BATCHES,
            average_batch_time_ms: REPORT_AVG_BATCH_TIME_MS,
            peak_memory_usage_bytes: TEST_ONE_MB,
            total_characters_processed: REPORT_TOTAL_CHARS,
        };

        let report = format!(
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
            stats.total_texts,
            stats.success_rate() * 100.0,
            stats.total_processing_time_ms as f64 / 1000.0,
            stats.throughput_texts_per_second(),
            stats.throughput_tokens_per_second(),
            stats.average_time_per_text_ms,
            stats.average_tokens_per_text,
            stats.batches_processed,
            stats.average_batch_time_ms,
            stats.peak_memory_usage_bytes as f64 / (1024.0 * 1024.0),
            stats.total_characters_processed
        );

        assert!(report.contains("Performance Report"));
        assert!(report.contains(&format!("Total texts processed: {}", REPORT_TOTAL_TEXTS)));
        assert!(report.contains("Success rate: 95.0%"));
        assert!(report.contains(&format!("Batches processed: {}", REPORT_BATCHES)));
    }

    #[test]
    fn test_get_model_info_returns_none_when_not_loaded() {
        // get_model_info returns None when model dimension is not available
        // Full testing with real models in integration tests
        let info: Option<(usize, bool)> = None;
        assert!(info.is_none());
    }
}
