//! Core IndexContext for in-memory file indexing
//!
//! This module provides the main `IndexContext` structure that maintains parsed
//! tree-sitter ASTs for files, with support for gitignore patterns and
//! async parsing with progress notifications.
//!
//! # Example
//!
//! ```ignore
//! use swissarmyhammer_treesitter::IndexContext;
//!
//! // Create context for a path
//! let mut context = IndexContext::new("/path/to/project")
//!     .with_progress(|status| {
//!         println!("{}: {}/{}", status.message, status.files_parsed, status.files_total);
//!     });
//!
//! // Scan and parse files
//! let result = context.scan().await?;
//!
//! // Query the index
//! if let Some(parsed) = context.get("src/main.rs")? {
//!     println!("Language: {}", parsed.language);
//! }
//! ```

use crate::chunk::{chunk_file, ChunkGraph};
use crate::error::{Result, TreeSitterError};
use crate::language::LanguageRegistry;
use crate::parsed_file::ParsedFile;
use ignore::WalkBuilder;
use llama_embedding::{EmbeddingConfig, EmbeddingModel};
use llama_loader::ModelSource;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// Default maximum file size to parse (10 MB)
pub const DEFAULT_MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Default parse timeout in milliseconds
pub const DEFAULT_PARSE_TIMEOUT_MS: u64 = 5000;

/// Configuration for embedding model
#[derive(Debug, Clone)]
pub struct EmbeddingModelConfig {
    /// HuggingFace repo (default: "nomic-ai/nomic-embed-code-GGUF")
    pub repo: String,

    /// Model filename (default: "nomic-embed-code.Q4_0.gguf")
    pub filename: String,
}

impl Default for EmbeddingModelConfig {
    fn default() -> Self {
        Self {
            repo: "nomic-ai/nomic-embed-code-GGUF".to_string(),
            filename: "nomic-embed-code.Q4_0.gguf".to_string(),
        }
    }
}

/// Configuration for the index
#[derive(Debug, Clone)]
pub struct IndexConfig {
    /// Maximum file size to parse (bytes)
    pub max_file_size: u64,

    /// Parse timeout in milliseconds
    pub parse_timeout_ms: u64,

    /// Whether to respect .gitignore patterns
    pub respect_gitignore: bool,

    /// Embedding model configuration
    pub embedding: EmbeddingModelConfig,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            parse_timeout_ms: DEFAULT_PARSE_TIMEOUT_MS,
            respect_gitignore: true,
            embedding: EmbeddingModelConfig::default(),
        }
    }
}

/// Current status of the index during scan operations
///
/// A simple snapshot of counters that grow during scanning:
/// - `files_total` grows during discovery
/// - `files_parsed`, `files_skipped`, `files_errored` grow during parsing
/// - `files_embedded` grows during embedding (if enabled)
///
/// Progress can be derived: `(parsed + skipped + errored) / total`
/// Progress status for indexing operations
///
/// Done can be derived: `parsed + skipped + errored == total` (and `embedded == parsed` if embedding)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexStatus {
    /// Total files discovered that need parsing (grows during discovery)
    pub files_total: usize,

    /// Files successfully parsed so far (grows during parsing)
    pub files_parsed: usize,

    /// Files skipped - unsupported language, too large, etc. (grows during parsing)
    pub files_skipped: usize,

    /// Files that encountered parse errors (grows during parsing)
    pub files_errored: usize,

    /// Files that have been embedded (grows during embedding phase)
    /// When embedding is disabled, this stays at 0.
    pub files_embedded: usize,

    /// Total chunks discovered for embedding (set at start of embedding phase)
    pub chunks_total: usize,

    /// Chunks successfully embedded so far (grows during embedding phase)
    pub chunks_embedded: usize,

    /// Current file being processed (None when not actively processing)
    pub current_file: Option<PathBuf>,

    /// Root path being indexed
    pub root_path: PathBuf,
}

impl IndexStatus {
    /// Create a new status for a root path
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            root_path,
            ..Default::default()
        }
    }

    /// Number of files processed so far
    pub fn files_processed(&self) -> usize {
        self.files_parsed + self.files_skipped + self.files_errored
    }

    /// Progress as a fraction (0.0 to 1.0), None if total is 0
    ///
    /// When embedding has started (files_embedded > 0), this considers both phases.
    pub fn progress(&self) -> Option<f64> {
        if self.files_total == 0 {
            return None;
        }

        let parse_progress = self.files_processed() as f64 / self.files_total as f64;

        // If embedding has started, factor it into progress
        if self.files_embedded > 0 {
            // Two phases: parsing (50%) + embedding (50%)
            let embed_progress = if self.files_parsed > 0 {
                self.files_embedded as f64 / self.files_parsed as f64
            } else {
                0.0
            };
            Some((parse_progress + embed_progress) / 2.0)
        } else {
            Some(parse_progress)
        }
    }

    /// Check if a phase is complete: total > 0 and processed == total
    fn is_phase_complete(processed: usize, total: usize) -> bool {
        total > 0 && processed == total
    }

    /// Whether parsing phase is complete
    pub fn is_parsing_complete(&self) -> bool {
        Self::is_phase_complete(self.files_processed(), self.files_total)
    }

    /// Whether embedding phase is complete
    ///
    /// Returns true when all parsed files have been embedded.
    /// If no embedding is happening (files_embedded == 0), returns false.
    pub fn is_embedding_complete(&self) -> bool {
        Self::is_phase_complete(self.files_embedded, self.files_parsed)
    }

    /// Whether all processing is complete
    ///
    /// Complete when parsing is done AND either:
    /// - No embedding was started (files_embedded == 0)
    /// - Or all embedding is done (files_embedded == files_parsed)
    pub fn is_complete(&self) -> bool {
        self.is_parsing_complete()
            && (self.files_embedded == 0 || self.is_embedding_complete())
    }
}

/// Result of a scan operation
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// Root path that was scanned
    pub root_path: PathBuf,

    /// Number of files successfully parsed
    pub files_parsed: usize,

    /// Number of files skipped (unsupported language, too large, etc.)
    pub files_skipped: usize,

    /// Errors encountered during parsing (non-fatal)
    pub errors: Vec<(PathBuf, String)>,

    /// Total time taken in milliseconds
    pub total_time_ms: u64,
}

/// Index statistics
#[derive(Debug, Clone)]
pub struct IndexStats {
    /// Total number of indexed files
    pub total_files: usize,

    /// Number of chunks in the graph
    pub total_chunks: usize,
}

/// Progress callback type
pub type ProgressCallback = Arc<dyn Fn(IndexStatus) + Send + Sync>;

/// Main index context for tree-sitter parsing
///
/// `IndexContext` is the primary interface for scanning and querying parsed files.
/// It is constructed with a root path, optionally configured, and then scanned
/// to populate the index. Once scanned, it can be queried for parsed files.
///
/// # Example
///
/// ```ignore
/// let mut context = IndexContext::new("/path/to/project");
/// context.scan().await?;
///
/// if let Some(parsed) = context.get("src/main.rs")? {
///     println!("Language: {}", parsed.language);
/// }
/// ```
pub struct IndexContext {
    /// Root path to scan
    root_path: PathBuf,

    /// Configuration
    config: IndexConfig,

    /// Optional progress callback
    progress_callback: Option<ProgressCallback>,

    /// Parsed files: path -> ParsedFile
    files: HashMap<PathBuf, Arc<ParsedFile>>,

    /// Graph of semantic chunks with embeddings
    chunk_graph: ChunkGraph,

    /// Embedding model (lazy-loaded on first scan with embedding enabled)
    embedding_model: Option<Arc<EmbeddingModel>>,

    /// Last scan status (updated during and after scan)
    last_status: IndexStatus,
}

impl IndexContext {
    /// Create a new index context for a path
    ///
    /// The path can be a file or directory. If a directory, all supported
    /// files within will be discovered during scan.
    pub fn new(root_path: impl AsRef<Path>) -> Self {
        let root = root_path.as_ref().to_path_buf();
        Self {
            last_status: IndexStatus::new(root.clone()),
            root_path: root,
            config: IndexConfig::default(),
            progress_callback: None,
            files: HashMap::new(),
            chunk_graph: ChunkGraph::new(),
            embedding_model: None,
        }
    }

    /// Set a progress callback
    ///
    /// The callback will be invoked with status updates during scan operations.
    pub fn with_progress<F>(mut self, callback: F) -> Self
    where
        F: Fn(IndexStatus) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Arc::new(callback));
        self
    }

    /// Set a pre-wrapped progress callback
    ///
    /// This takes a `ProgressCallback` (Arc-wrapped) directly, useful when
    /// the callback has already been wrapped.
    pub fn with_progress_callback(mut self, callback: ProgressCallback) -> Self {
        self.progress_callback = Some(callback);
        self
    }

    /// Set custom configuration
    pub fn with_config(mut self, config: IndexConfig) -> Self {
        self.config = config;
        self
    }

    /// Get the root path
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Get the current configuration
    pub fn config(&self) -> &IndexConfig {
        &self.config
    }

    /// Get the chunk graph (read-only)
    pub fn chunk_graph(&self) -> &ChunkGraph {
        &self.chunk_graph
    }

    /// Get the chunk graph (mutable)
    pub fn chunk_graph_mut(&mut self) -> &mut ChunkGraph {
        &mut self.chunk_graph
    }

    /// Send a progress update if a callback is configured
    fn send_progress(&self, status: IndexStatus) {
        if let Some(ref callback) = self.progress_callback {
            callback(status);
        }
    }

    /// Scan the root path and parse all supported files
    ///
    /// This is an async operation that discovers files and parses them.
    /// Progress updates are sent via the callback if configured.
    pub async fn scan(&mut self) -> Result<ScanResult> {
        let start = Instant::now();

        // Validate root path
        if !self.root_path.exists() {
            return Err(TreeSitterError::FileNotFound(self.root_path.clone()));
        }

        // Reset status for this scan
        self.last_status = IndexStatus::new(self.root_path.clone());

        // Send starting status
        self.send_progress(self.last_status.clone());

        let registry = LanguageRegistry::global();
        let mut errors = Vec::new();

        // First pass: discover all files to get total count
        let mut files_to_parse: Vec<PathBuf> = Vec::new();

        let walker = WalkBuilder::new(&self.root_path)
            .git_ignore(self.config.respect_gitignore)
            .git_global(self.config.respect_gitignore)
            .git_exclude(self.config.respect_gitignore)
            .hidden(false) // Don't skip hidden files - let gitignore handle it
            .build();

        for entry in walker.flatten() {
            let path = entry.path();

            // Skip directories
            if !path.is_file() {
                continue;
            }

            // Check if language is supported - unsupported files are simply ignored
            if registry.detect_language(path).is_none() {
                continue;
            }

            // Check file size - files we can't access or are too large are skipped
            let metadata = match std::fs::metadata(path) {
                Ok(m) => m,
                Err(e) => {
                    errors.push((path.to_path_buf(), e.to_string()));
                    self.record_skipped_file();
                    continue;
                }
            };

            if metadata.len() > self.config.max_file_size {
                self.record_skipped_file();
                continue;
            }

            files_to_parse.push(path.to_path_buf());
            self.last_status.files_total += 1;

            // Send discovery progress
            self.send_progress(self.last_status.clone());
        }

        tracing::info!(
            "Discovered {} files to parse in {}",
            files_to_parse.len(),
            self.root_path.display()
        );

        // Second pass: parse files
        for path in files_to_parse {
            self.last_status.current_file = Some(path.clone());
            self.send_progress(self.last_status.clone());

            // Get language config
            let Some(lang_config) = registry.detect_language(&path) else {
                self.last_status.files_skipped += 1;
                continue;
            };

            // Parse the file
            match self.parse_file_internal(&path, lang_config) {
                Ok(parsed) => {
                    self.files.insert(path.clone(), Arc::new(parsed));
                    self.last_status.files_parsed += 1;
                }
                Err(e) => {
                    errors.push((path.clone(), e.to_string()));
                    self.last_status.files_errored += 1;
                }
            }

            // Yield to allow other tasks to run
            tokio::task::yield_now().await;
        }

        // Third pass: embed chunks
        let embedding_config = self.config.embedding.clone();
        self.run_embedding_phase(&embedding_config, &mut errors)
            .await?;

        // Send final status
        self.last_status.current_file = None;
        self.send_progress(self.last_status.clone());

        let total_time_ms = start.elapsed().as_millis() as u64;

        Ok(ScanResult {
            root_path: self.root_path.clone(),
            files_parsed: self.last_status.files_parsed,
            files_skipped: self.last_status.files_skipped,
            errors,
            total_time_ms,
        })
    }

    /// Run the embedding phase for all parsed files (internal)
    async fn run_embedding_phase(
        &mut self,
        config: &EmbeddingModelConfig,
        errors: &mut Vec<(PathBuf, String)>,
    ) -> Result<()> {
        let files_count = self.files.len();
        if files_count == 0 {
            return Ok(());
        }

        tracing::info!("Loading embedding model...");
        self.ensure_embedding_model_loaded(config).await?;

        tracing::info!("Embedding {} parsed files...", files_count);
        let paths_to_embed: Vec<PathBuf> = self.files.keys().cloned().collect();

        // Count total chunks across all files for progress tracking in IndexStatus
        self.last_status.chunks_total = paths_to_embed
            .iter()
            .filter_map(|p| self.files.get(p))
            .map(|parsed| chunk_file(parsed.clone()).len())
            .sum();
        tracing::info!("Total chunks to embed: {}", self.last_status.chunks_total);

        for path in paths_to_embed {
            self.last_status.current_file = Some(path.clone());
            self.send_progress(self.last_status.clone());

            self.embed_file_chunks(&path, errors).await;

            self.last_status.files_embedded += 1;
            self.send_progress(self.last_status.clone());

            tokio::task::yield_now().await;
        }

        tracing::info!(
            "Embedding complete: {} files, {} chunks",
            self.last_status.files_embedded,
            self.chunk_graph.chunks().len()
        );

        Ok(())
    }

    /// Embed all chunks for a single file and add to graph.
    /// Updates `last_status.chunks_embedded` as chunks are processed.
    async fn embed_file_chunks(&mut self, path: &Path, errors: &mut Vec<(PathBuf, String)>) {
        let Some(parsed) = self.files.get(path) else {
            return;
        };

        let Some(ref model) = self.embedding_model else {
            return;
        };

        // Remove old chunks for this file before adding new ones
        self.chunk_graph.remove_file(path);

        let chunks = chunk_file(parsed.clone());

        for mut chunk in chunks {
            let Some(content) = chunk.content() else {
                continue;
            };

            // Log progress using IndexStatus (1-indexed for display)
            let current = self.last_status.chunks_embedded + 1;
            let total = self.last_status.chunks_total;
            let symbol_path = chunk.symbol_path();
            tracing::info!("Embedding {}/{}: {}", current, total, symbol_path);

            match model.embed_text(content).await {
                Ok(result) => {
                    chunk.embedding = Some(result.embedding);
                    self.chunk_graph.add(chunk);
                    self.last_status.chunks_embedded += 1;
                }
                Err(e) => {
                    errors.push((path.to_path_buf(), format!("Embedding error: {}", e)));
                }
            }
        }
    }

    /// Ensure the embedding model is loaded
    async fn ensure_embedding_model_loaded(&mut self, config: &EmbeddingModelConfig) -> Result<()> {
        if self.embedding_model.is_some() {
            return Ok(());
        }

        let embed_config = EmbeddingConfig {
            model_source: ModelSource::HuggingFace {
                repo: config.repo.clone(),
                filename: Some(config.filename.clone()),
                folder: None,
            },
            normalize_embeddings: true,
            max_sequence_length: None, // Use model's context size
            debug: false,
        };

        let mut model = EmbeddingModel::new(embed_config).await.map_err(|e| {
            TreeSitterError::embedding_error(format!("Failed to create embedding model: {}", e))
        })?;

        model.load_model().await.map_err(|e| {
            TreeSitterError::embedding_error(format!("Failed to load embedding model: {}", e))
        })?;

        self.embedding_model = Some(Arc::new(model));
        Ok(())
    }

    /// Parse a single file
    fn parse_file_internal(
        &self,
        path: &Path,
        lang_config: &'static crate::language::LanguageConfig,
    ) -> Result<ParsedFile> {
        // Read file content
        let content = std::fs::read_to_string(path)?;

        // Create parser
        let mut parser = tree_sitter::Parser::new();
        let language = lang_config.language();

        parser
            .set_language(&language)
            .map_err(|e| TreeSitterError::parse_error(path.to_path_buf(), e.to_string()))?;

        // Parse
        let tree = parser.parse(&content, None).ok_or_else(|| {
            TreeSitterError::parse_error(path.to_path_buf(), "Parse returned None")
        })?;

        // Compute content hash for cache invalidation
        let content_hash: [u8; 16] = md5::compute(content.as_bytes()).into();

        Ok(ParsedFile::new(
            path.to_path_buf(),
            content,
            tree,
            content_hash,
        ))
    }

    /// Record a file as skipped during discovery
    ///
    /// Increments both files_total and files_skipped, then sends progress update.
    fn record_skipped_file(&mut self) {
        self.last_status.files_total += 1;
        self.last_status.files_skipped += 1;
        self.send_progress(self.last_status.clone());
    }

    /// Get a parsed file by path
    ///
    /// Returns None if the file is not in the index.
    /// The path can be absolute or relative to the root path.
    pub fn get(&self, path: impl AsRef<Path>) -> Option<&ParsedFile> {
        let path = path.as_ref();

        // Try exact path first
        if let Some(parsed) = self.files.get(path) {
            return Some(parsed.as_ref());
        }

        // Try relative to root path
        let full_path = self.root_path.join(path);
        self.files.get(&full_path).map(|arc| arc.as_ref())
    }

    /// Force re-parse of a file, update the index, and re-embed if enabled
    ///
    /// This method:
    /// 1. Re-parses the file with tree-sitter
    /// 2. Clears old chunks for this file from the graph
    /// 3. If embedding is enabled, embeds the new chunks and adds them to the graph
    pub async fn refresh(&mut self, path: impl AsRef<Path>) -> Result<ParsedFile> {
        let path = path.as_ref();

        // Detect language
        let registry = LanguageRegistry::global();
        let lang_config = registry
            .detect_language(path)
            .ok_or_else(|| TreeSitterError::unsupported_language(path.to_path_buf()))?;

        // Parse the file
        let parsed = Arc::new(self.parse_file_internal(path, lang_config)?);

        // Update index
        self.files.insert(path.to_path_buf(), parsed.clone());

        // Clear old chunks and re-embed
        self.chunk_graph.remove_file(path);
        let embedding_config = self.config.embedding.clone();
        self.ensure_embedding_model_loaded(&embedding_config)
            .await?;
        let mut errors = Vec::new();
        // Set up chunk tracking for this single file refresh
        self.last_status.chunks_total = chunk_file(parsed.clone()).len();
        self.last_status.chunks_embedded = 0;
        self.embed_file_chunks(path, &mut errors).await;

        Ok((*parsed).clone())
    }

    /// Remove a file from the index
    pub fn remove(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref();
        self.files.remove(path);

        // Also try removing with root path prefix
        let full_path = self.root_path.join(path);
        self.files.remove(&full_path);
    }

    /// Get all indexed file paths
    pub fn files(&self) -> Vec<PathBuf> {
        self.files.keys().cloned().collect()
    }

    /// Get index statistics
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            total_files: self.files.len(),
            total_chunks: self.chunk_graph.chunks().len(),
        }
    }

    /// Clear all parsed files from the index
    pub fn clear(&mut self) {
        self.files.clear();
        self.chunk_graph = ChunkGraph::new();
        self.last_status = IndexStatus::new(self.root_path.clone());
    }

    /// Get the current status of the index
    pub fn status(&self) -> IndexStatus {
        self.last_status.clone()
    }

    /// Check if a path is in the index
    pub fn contains(&self, path: impl AsRef<Path>) -> bool {
        let path = path.as_ref();
        self.files.contains_key(path) || self.files.contains_key(&self.root_path.join(path))
    }

    /// Get the number of indexed files
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Embed arbitrary text using the index's embedding model.
    ///
    /// This is useful for semantic search where you want to find chunks
    /// similar to a given query string. The embedding model is loaded
    /// if not already loaded.
    ///
    /// Returns the embedding vector that can be used with `SimilarityQuery::embedding()`.
    pub async fn embed_text(&mut self, text: &str) -> Result<Vec<f32>> {
        let config = self.config.embedding.clone();
        self.ensure_embedding_model_loaded(&config).await?;

        let model = self
            .embedding_model
            .as_ref()
            .ok_or_else(|| TreeSitterError::embedding_error("Embedding model not loaded"))?;

        let result = model.embed_text(text).await.map_err(|e| {
            TreeSitterError::embedding_error(format!("Failed to embed text: {}", e))
        })?;

        Ok(result.embedding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{run_progress_test, setup_test_dir, ProgressCollector};
    use tempfile::TempDir;

    /// Minimum expected embedding dimensions from typical embedding models.
    /// Most code embedding models produce vectors of 768+ dimensions.
    const MIN_EMBEDDING_DIMENSIONS: usize = 256;

    #[test]
    fn test_index_config_default() {
        let config = IndexConfig::default();

        assert_eq!(config.max_file_size, DEFAULT_MAX_FILE_SIZE);
        assert_eq!(config.parse_timeout_ms, DEFAULT_PARSE_TIMEOUT_MS);
        assert!(config.respect_gitignore);
        // Embedding is always configured - verify it matches EmbeddingModelConfig defaults
        let default_embed = EmbeddingModelConfig::default();
        assert_eq!(config.embedding.repo, default_embed.repo);
        assert_eq!(config.embedding.filename, default_embed.filename);
    }

    #[test]
    fn test_context_new() {
        let context = IndexContext::new("/some/path");

        assert_eq!(context.root_path(), Path::new("/some/path"));
        assert!(context.is_empty());
    }

    #[test]
    fn test_context_with_config() {
        let config = IndexConfig {
            max_file_size: 1024,
            ..Default::default()
        };

        let context = IndexContext::new("/some/path").with_config(config);

        assert_eq!(context.config().max_file_size, 1024);
    }

    #[tokio::test]
    async fn test_scan_directory() {
        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());

        let result = context.scan().await.unwrap();

        assert_eq!(result.root_path, dir.path());
        assert_eq!(result.files_parsed, 4); // main.rs, lib.rs, config.json, README.md
                                            // unsupported.xyz is ignored (not a supported language), so not counted as skipped
                                            // files_skipped only counts supported files that couldn't be parsed
        assert!(result.files_skipped <= result.files_parsed);
    }

    #[tokio::test]
    async fn test_scan_with_size_limit_skips_large_files() {
        let dir = setup_test_dir();
        // Set a very small size limit so some files get skipped
        let config = IndexConfig {
            max_file_size: 5, // Only 5 bytes allowed
            ..Default::default()
        };
        let mut context = IndexContext::new(dir.path()).with_config(config);

        let result = context.scan().await.unwrap();

        // All supported files exceed 5 bytes, so they should be skipped
        assert!(result.files_skipped >= 1);
        // Verify status reflects the same counts
        let status = context.status();
        assert_eq!(status.files_parsed, result.files_parsed);
        assert_eq!(status.files_skipped, result.files_skipped);
    }

    #[tokio::test]
    async fn test_scan_single_file() {
        let dir = setup_test_dir();
        let single_file = dir.path().join("main.rs");
        let mut context = IndexContext::new(&single_file);

        let result = context.scan().await.unwrap();

        assert_eq!(result.root_path, single_file);
        assert_eq!(result.files_parsed, 1);
        assert_eq!(result.files_skipped, 0);

        // Verify we can get the parsed file
        let parsed = context.get(&single_file);
        assert!(parsed.is_some());
        assert!(parsed.unwrap().source.contains("fn main"));
    }

    #[tokio::test]
    async fn test_scan_with_progress() {
        let dir = setup_test_dir();
        let collector = ProgressCollector::new();

        let mut context = IndexContext::new(dir.path()).with_progress(collector.callback());

        context.scan().await.unwrap();

        // Should have received multiple progress updates
        assert!(collector.count() > 0);
    }

    #[tokio::test]
    async fn test_get_file() {
        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());

        context.scan().await.unwrap();

        let parsed = context.get(dir.path().join("main.rs"));
        assert!(parsed.is_some());
        let parsed = parsed.unwrap();
        assert!(parsed.source.contains("fn main"));
        assert!(!parsed.has_errors());
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());

        context.scan().await.unwrap();

        let parsed = context.get(dir.path().join("nonexistent.rs"));
        assert!(parsed.is_none());
    }

    #[tokio::test]
    async fn test_refresh() {
        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());

        context.scan().await.unwrap();

        // Modify the file
        std::fs::write(
            dir.path().join("main.rs"),
            "fn main() { println!(\"hello\"); }",
        )
        .unwrap();

        // Refresh should re-parse
        let parsed = context.refresh(dir.path().join("main.rs")).await.unwrap();
        assert!(parsed.source.contains("println"));
    }

    #[tokio::test]
    async fn test_refresh_clears_old_chunks() {
        use crate::chunk::SemanticChunk;

        /// Distinctive marker value for fake embedding to identify it after refresh
        const FAKE_EMBEDDING_MARKER: f32 = f32::MAX;

        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());
        context.scan().await.unwrap();

        let path = dir.path().join("main.rs");

        // Record how many chunks exist after initial scan
        let initial_chunk_count = context.chunk_graph().chunks_for_file(&path).len();
        assert!(
            initial_chunk_count > 0,
            "Should have chunks after scan with embedding"
        );

        // Manually add a fake chunk with a distinctive embedding
        let parsed = context.get(&path).unwrap();
        let fake_embedding = vec![FAKE_EMBEDDING_MARKER; 3];
        let fake_chunk = SemanticChunk::from_parsed(Arc::new(parsed.clone()), 0, 8)
            .with_embedding(fake_embedding);
        context.chunk_graph_mut().add(fake_chunk);

        let with_fake_count = context.chunk_graph().chunks_for_file(&path).len();
        assert_eq!(with_fake_count, initial_chunk_count + 1);

        // Refresh should clear old chunks and re-embed
        context.refresh(&path).await.unwrap();

        // Should have new chunks (from re-embedding), but the fake chunk should be gone
        let after_refresh_count = context.chunk_graph().chunks_for_file(&path).len();
        assert!(after_refresh_count > 0, "Should have chunks after refresh");

        // Verify the fake chunk (with the marker embedding) is gone
        let has_fake = context
            .chunk_graph()
            .chunks_for_file(&path)
            .iter()
            .any(|c| {
                c.embedding
                    .as_ref()
                    .map(|e| e.first() == Some(&FAKE_EMBEDDING_MARKER))
                    .unwrap_or(false)
            });
        assert!(!has_fake, "Fake chunk should be cleared after refresh");
    }

    #[tokio::test]
    async fn test_remove() {
        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());

        context.scan().await.unwrap();

        let path = dir.path().join("main.rs");

        // File should exist
        assert!(context.get(&path).is_some());

        // Remove it
        context.remove(&path);

        // File should be gone from index
        assert!(context.get(&path).is_none());
    }

    #[tokio::test]
    async fn test_files() {
        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());

        context.scan().await.unwrap();

        let files = context.files();
        assert_eq!(files.len(), 4);
    }

    #[tokio::test]
    async fn test_stats() {
        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());

        context.scan().await.unwrap();

        let stats = context.stats();
        assert_eq!(stats.total_files, 4);
        // Embedding is always enabled, so we should have chunks
        assert!(
            stats.total_chunks > 0,
            "Should have chunks with embedding enabled"
        );
    }

    #[tokio::test]
    async fn test_clear() {
        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());

        context.scan().await.unwrap();
        assert!(!context.is_empty());

        context.clear();
        assert!(context.is_empty());
    }

    #[tokio::test]
    async fn test_status() {
        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());

        // Before scan
        let status = context.status();
        assert!(!status.is_complete());
        assert_eq!(status.files_total, 0);

        // After scan
        context.scan().await.unwrap();
        let status = context.status();
        assert!(status.is_complete());
        assert_eq!(status.files_parsed, 4);
    }

    #[tokio::test]
    async fn test_contains() {
        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());

        context.scan().await.unwrap();

        assert!(context.contains(dir.path().join("main.rs")));
        assert!(!context.contains(dir.path().join("nonexistent.rs")));
    }

    #[tokio::test]
    async fn test_len_and_is_empty() {
        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());

        assert!(context.is_empty());
        assert_eq!(context.len(), 0);

        context.scan().await.unwrap();

        assert!(!context.is_empty());
        assert_eq!(context.len(), 4);
    }

    #[tokio::test]
    async fn test_embed_text() {
        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());

        // Scan to initialize the embedding model
        context.scan().await.unwrap();

        // Embed some text
        let embedding = context.embed_text("fn main() { }").await.unwrap();

        // Embedding should be a non-empty vector
        assert!(!embedding.is_empty());
        // Typical embedding models produce vectors of 768 or more dimensions
        assert!(embedding.len() >= MIN_EMBEDDING_DIMENSIONS);
    }

    #[tokio::test]
    async fn test_embed_text_loads_model_lazily() {
        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());

        // Don't scan - just call embed_text directly
        // This tests that embed_text loads the model if needed
        let embedding = context.embed_text("test code").await.unwrap();

        assert!(!embedding.is_empty());
    }

    #[tokio::test]
    async fn test_scan_nonexistent_path() {
        let mut context = IndexContext::new("/nonexistent/path/that/does/not/exist");

        let result = context.scan().await;
        assert!(result.is_err());
    }

    // =============================================================================
    // Helper method tests
    // =============================================================================

    #[test]
    fn test_record_skipped_file_increments_counters() {
        let mut context = IndexContext::new("/test");

        let initial_total = context.last_status.files_total;
        let initial_skipped = context.last_status.files_skipped;

        context.record_skipped_file();

        assert_eq!(context.last_status.files_total, initial_total + 1);
        assert_eq!(context.last_status.files_skipped, initial_skipped + 1);
    }

    #[test]
    fn test_record_skipped_file_with_progress_callback() {
        use std::sync::{Arc, Mutex};

        let updates = Arc::new(Mutex::new(Vec::new()));
        let updates_clone = updates.clone();

        let mut context = IndexContext::new("/test").with_progress(move |status| {
            updates_clone.lock().unwrap().push(status);
        });

        context.record_skipped_file();

        let collected = updates.lock().unwrap();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].files_total, 1);
        assert_eq!(collected[0].files_skipped, 1);
    }

    // =============================================================================
    // IndexStatus tests
    // =============================================================================

    #[test]
    fn test_index_status_new() {
        let status = IndexStatus::new(PathBuf::from("/test"));

        assert_eq!(status.root_path, PathBuf::from("/test"));
        assert_eq!(status.files_total, 0);
        assert_eq!(status.files_parsed, 0);
        assert_eq!(status.files_skipped, 0);
        assert_eq!(status.files_errored, 0);
        assert!(status.current_file.is_none());
    }

    #[test]
    fn test_index_status_files_processed() {
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.files_parsed = 5;
        status.files_skipped = 2;
        status.files_errored = 1;

        assert_eq!(status.files_processed(), 8);
    }

    #[test]
    fn test_index_status_progress() {
        let mut status = IndexStatus::new(PathBuf::from("/test"));

        // No files yet - progress is None
        assert_eq!(status.progress(), None);

        // Set total and some parsed
        status.files_total = 10;
        status.files_parsed = 5;

        assert_eq!(status.progress(), Some(0.5));

        // All processed
        status.files_parsed = 8;
        status.files_skipped = 2;

        assert_eq!(status.progress(), Some(1.0));
    }

    #[test]
    fn test_index_status_is_complete() {
        let mut status = IndexStatus::new(PathBuf::from("/test"));

        // No files - not complete
        assert!(!status.is_complete());

        // Has files but not all processed
        status.files_total = 10;
        status.files_parsed = 5;
        assert!(!status.is_complete());

        // All processed (no embedding)
        status.files_parsed = 8;
        status.files_skipped = 2;
        assert!(status.is_complete());
    }

    #[test]
    fn test_index_status_is_parsing_complete() {
        let mut status = IndexStatus::new(PathBuf::from("/test"));

        // No files - not complete
        assert!(!status.is_parsing_complete());

        // Has files but not all processed
        status.files_total = 10;
        status.files_parsed = 5;
        assert!(!status.is_parsing_complete());

        // All processed
        status.files_parsed = 8;
        status.files_skipped = 2;
        assert!(status.is_parsing_complete());
    }

    #[test]
    fn test_index_status_is_embedding_complete() {
        let mut status = IndexStatus::new(PathBuf::from("/test"));

        // No files parsed yet - not complete
        status.files_parsed = 0;
        assert!(!status.is_embedding_complete());

        // Some files parsed but not embedded
        status.files_parsed = 5;
        status.files_embedded = 2;
        assert!(!status.is_embedding_complete());

        // All parsed files embedded
        status.files_embedded = 5;
        assert!(status.is_embedding_complete());
    }

    #[test]
    fn test_index_status_is_complete_with_embedding() {
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.files_total = 10;
        status.files_parsed = 8;
        status.files_skipped = 2;

        // Parsing complete, no embedding started - complete
        assert!(status.is_parsing_complete());
        assert!(status.is_complete());

        // Start embedding (now in progress, not complete)
        status.files_embedded = 4;
        assert!(!status.is_complete());

        // Embedding finished
        status.files_embedded = 8;
        assert!(status.is_embedding_complete());
        assert!(status.is_complete());
    }

    #[test]
    fn test_index_status_progress_with_embedding() {
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.files_total = 10;

        // No progress yet
        assert_eq!(status.progress(), Some(0.0));

        // Half parsed, no embedding started yet
        status.files_parsed = 5;
        assert_eq!(status.progress(), Some(0.5));

        // All parsed, no embedding started (100% of parse phase)
        status.files_parsed = 10;
        assert_eq!(status.progress(), Some(1.0));

        // Start embedding (50% parse + 0% embed = 50%)
        status.files_embedded = 1;
        // (1.0 + 0.1) / 2 = 0.55
        assert!((status.progress().unwrap() - 0.55).abs() < 0.01);

        // Half embedded (75% overall)
        status.files_embedded = 5;
        assert_eq!(status.progress(), Some(0.75));

        // All embedded (100% overall)
        status.files_embedded = 10;
        assert_eq!(status.progress(), Some(1.0));
    }

    // =============================================================================
    // Progress callback tests
    // =============================================================================

    #[tokio::test]
    async fn test_progress_callback_receives_updates() {
        let result = run_progress_test().await;

        // Should have received multiple updates
        assert!(
            result.updates.len() >= 2,
            "Expected at least 2 updates, got {}",
            result.updates.len()
        );
    }

    #[tokio::test]
    async fn test_progress_callback_files_total_grows() {
        let result = run_progress_test().await;

        // files_total should grow during discovery
        let totals: Vec<usize> = result.updates.iter().map(|u| u.files_total).collect();

        // Should eventually have files
        assert!(
            totals.iter().any(|&t| t > 0),
            "files_total should grow during scan"
        );

        // Should be monotonically increasing
        for window in totals.windows(2) {
            assert!(
                window[1] >= window[0],
                "files_total should not decrease: {} -> {}",
                window[0],
                window[1]
            );
        }
    }

    #[tokio::test]
    async fn test_progress_callback_files_parsed_grows() {
        let result = run_progress_test().await;

        // files_parsed should grow during parsing
        let parsed: Vec<usize> = result.updates.iter().map(|u| u.files_parsed).collect();

        // Should eventually have parsed files
        assert!(
            parsed.iter().any(|&p| p > 0),
            "files_parsed should grow during scan"
        );

        // Should be monotonically increasing
        for window in parsed.windows(2) {
            assert!(
                window[1] >= window[0],
                "files_parsed should not decrease: {} -> {}",
                window[0],
                window[1]
            );
        }
    }

    #[tokio::test]
    async fn test_progress_callback_has_current_file_during_parsing() {
        let result = run_progress_test().await;

        // Should have updates with current_file set
        let with_current_file: Vec<_> = result
            .updates
            .iter()
            .filter(|u| u.current_file.is_some())
            .collect();

        assert!(
            !with_current_file.is_empty(),
            "Should have updates with current_file set during parsing"
        );
    }

    #[tokio::test]
    async fn test_progress_callback_final_is_complete() {
        let result = run_progress_test().await;
        let final_status = result.updates.last().unwrap();

        // Final status should be complete
        assert!(
            final_status.is_complete(),
            "Final status should be complete"
        );

        // current_file should be None when done
        assert!(
            final_status.current_file.is_none(),
            "current_file should be None when complete"
        );
    }

    #[tokio::test]
    async fn test_progress_callback_final_matches_result() {
        let result = run_progress_test().await;
        let final_status = result.updates.last().unwrap();

        // Final callback status should match scan result
        assert_eq!(
            final_status.files_parsed, result.scan_result.files_parsed,
            "Callback files_parsed should match result"
        );
        assert_eq!(
            final_status.files_errored,
            result.scan_result.errors.len(),
            "Callback files_errored should match result.errors.len()"
        );
    }

    // =============================================================================
    // Status() synchronous method tests - symmetry with progress callbacks
    // =============================================================================

    #[tokio::test]
    async fn test_status_matches_final_callback() {
        let dir = setup_test_dir();
        let collector = ProgressCollector::new();

        let mut context = IndexContext::new(dir.path()).with_progress(collector.callback());
        context.scan().await.unwrap();

        let updates = collector.updates();
        let final_callback = updates.last().unwrap();
        let sync_status = context.status();

        // Synchronous status() should match final callback status
        assert_eq!(sync_status.files_total, final_callback.files_total);
        assert_eq!(sync_status.files_parsed, final_callback.files_parsed);
        assert_eq!(sync_status.files_skipped, final_callback.files_skipped);
        assert_eq!(sync_status.files_errored, final_callback.files_errored);
        assert_eq!(sync_status.root_path, final_callback.root_path);
    }

    #[test]
    fn test_status_before_scan() {
        let context = IndexContext::new("/some/path");
        let status = context.status();

        assert_eq!(status.files_total, 0);
        assert_eq!(status.files_parsed, 0);
        assert!(!status.is_complete());
    }

    #[tokio::test]
    async fn test_status_after_scan() {
        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());
        context.scan().await.unwrap();

        let status = context.status();

        assert!(status.is_complete());
        assert_eq!(status.files_parsed, context.len());
    }

    #[tokio::test]
    async fn test_status_after_clear() {
        let dir = setup_test_dir();
        let mut context = IndexContext::new(dir.path());
        context.scan().await.unwrap();

        assert!(context.status().is_complete());

        context.clear();

        let status = context.status();
        assert!(!status.is_complete());
        assert_eq!(status.files_total, 0);
    }

    // =============================================================================
    // Edge cases for progress callbacks
    // =============================================================================

    #[tokio::test]
    async fn test_progress_callback_with_no_supported_files() {
        let dir = TempDir::new().unwrap();
        // Empty directory - no supported files
        std::fs::write(dir.path().join("unsupported.xyz"), "unknown").unwrap();

        let collector = ProgressCollector::new();
        let mut context = IndexContext::new(dir.path()).with_progress(collector.callback());
        context.scan().await.unwrap();

        let updates = collector.updates();
        let final_status = updates.last().unwrap();

        // Should still complete with no files parsed
        assert_eq!(final_status.files_total, 0);
        assert_eq!(final_status.files_parsed, 0);
    }

    #[tokio::test]
    async fn test_progress_callback_with_single_file() {
        let dir = setup_test_dir();
        let single_file = dir.path().join("main.rs");
        let collector = ProgressCollector::new();

        let mut context = IndexContext::new(&single_file).with_progress(collector.callback());
        context.scan().await.unwrap();

        let updates = collector.updates();
        let final_status = updates.last().unwrap();

        // Should have parsed the single file
        assert_eq!(final_status.files_parsed, 1);
        assert!(final_status.is_complete());
    }

    #[tokio::test]
    async fn test_progress_callback_not_called_without_setting() {
        let dir = setup_test_dir();

        // Context without progress callback
        let mut context = IndexContext::new(dir.path());
        let result = context.scan().await;

        // Should complete successfully without a callback
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_scans_call_progress_each_time() {
        let dir = setup_test_dir();
        let collector = ProgressCollector::new();

        let mut context = IndexContext::new(dir.path()).with_progress(collector.callback());

        // First scan
        context.scan().await.unwrap();
        let first_count = collector.count();
        assert!(first_count > 0);

        // Clear and scan again
        context.clear();
        context.scan().await.unwrap();
        let second_count = collector.count();

        // Should have more calls after second scan
        assert!(second_count > first_count);
    }

    // =============================================================================
    // Additional IndexStatus method tests
    // =============================================================================

    #[test]
    fn test_index_status_progress_edge_cases() {
        // Zero total returns None
        let status = IndexStatus::new(PathBuf::from("/test"));
        assert_eq!(status.progress(), None);

        // Partial progress
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.files_total = 100;
        status.files_parsed = 25;
        assert_eq!(status.progress(), Some(0.25));

        // 75% with mixed parsed/skipped/errored
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.files_total = 100;
        status.files_parsed = 50;
        status.files_skipped = 20;
        status.files_errored = 5;
        assert_eq!(status.progress(), Some(0.75));

        // 100% complete
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.files_total = 100;
        status.files_parsed = 100;
        assert_eq!(status.progress(), Some(1.0));
    }

    #[test]
    fn test_index_status_is_complete_edge_cases() {
        // Empty status is not complete (needs scanning first)
        let status = IndexStatus::new(PathBuf::from("/test"));
        assert!(!status.is_complete());

        // Has total but nothing processed
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.files_total = 10;
        assert!(!status.is_complete());

        // Partial processing
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.files_total = 10;
        status.files_parsed = 5;
        assert!(!status.is_complete());

        // Complete with all parsed
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.files_total = 10;
        status.files_parsed = 10;
        assert!(status.is_complete());

        // Complete with mix of parsed/skipped/errored
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.files_total = 10;
        status.files_parsed = 6;
        status.files_skipped = 3;
        status.files_errored = 1;
        assert!(status.is_complete());
    }

    #[test]
    fn test_index_status_files_processed_combinations() {
        let mut status = IndexStatus::new(PathBuf::from("/test"));

        // All zeros
        assert_eq!(status.files_processed(), 0);

        // Only parsed
        status.files_parsed = 10;
        assert_eq!(status.files_processed(), 10);

        // Only skipped
        status.files_parsed = 0;
        status.files_skipped = 5;
        assert_eq!(status.files_processed(), 5);

        // Only errored
        status.files_skipped = 0;
        status.files_errored = 3;
        assert_eq!(status.files_processed(), 3);

        // All combined
        status.files_parsed = 10;
        status.files_skipped = 5;
        status.files_errored = 3;
        assert_eq!(status.files_processed(), 18);
    }

    #[test]
    fn test_index_status_default() {
        let status = IndexStatus::default();

        assert_eq!(status.files_total, 0);
        assert_eq!(status.files_parsed, 0);
        assert_eq!(status.files_skipped, 0);
        assert_eq!(status.files_errored, 0);
        assert!(status.current_file.is_none());
        assert_eq!(status.root_path, PathBuf::new());
    }
}
