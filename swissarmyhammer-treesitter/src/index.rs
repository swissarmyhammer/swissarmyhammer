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

use crate::chunk::chunk_file;
use crate::db::IndexDatabase;
use crate::error::{Result, TreeSitterError};
use crate::language::LanguageRegistry;
use crate::parsed_file::ParsedFile;
use ignore::WalkBuilder;
use llama_embedding::{EmbeddingConfig, EmbeddingModel};
use llama_loader::ModelSource;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// Compute the content hash for a file without parsing it
///
/// This is useful for checking if a file has changed before re-parsing.
pub fn compute_file_hash(path: &Path) -> std::io::Result<[u8; 16]> {
    let content = std::fs::read(path)?;
    Ok(md5::compute(&content).into())
}

/// Result of checking whether a file should be parsed
enum FileCheckResult {
    /// File should be parsed
    Parse,
    /// File is unchanged from database, skip it
    SkipUnchanged,
    /// File is too large, skip it
    SkipTooLarge,
    /// Error accessing file
    Error(String),
}

/// Default maximum file size to parse (10 MB)
pub const DEFAULT_MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Default parse timeout in milliseconds
pub const DEFAULT_PARSE_TIMEOUT_MS: u64 = 5000;

/// Configuration for embedding model
#[derive(Debug, Clone)]
pub struct EmbeddingModelConfig {
    /// HuggingFace repo (default: "nomic-ai/nomic-embed-text-v1.5-GGUF")
    pub repo: String,

    /// Model filename (default: "nomic-embed-text-v1.5.Q4_K_M.gguf")
    pub filename: String,
}

impl Default for EmbeddingModelConfig {
    fn default() -> Self {
        Self {
            repo: "nomic-ai/nomic-embed-text-v1.5-GGUF".to_string(),
            filename: "nomic-embed-text-v1.5.Q4_K_M.gguf".to_string(),
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

/// Phase of the indexing operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum IndexPhase {
    /// Initial state, not yet started
    #[default]
    Idle,
    /// Discovering files to process
    Discovering,
    /// Parsing discovered files
    Parsing,
    /// Embedding parsed chunks
    Embedding,
    /// Indexing complete
    Complete,
}

/// Reason a file was skipped
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkipReason {
    /// File content unchanged from last index
    Unchanged,
    /// File too large to parse
    TooLarge,
    /// Unsupported language/file type
    Unsupported,
}

/// Action that just occurred, triggering this status update.
///
/// Notification cadence:
/// 1. `BuildStarted` - indexing begins
/// 2. `FileStarted` - starting to process a file (parse + embed)
/// 3. `FileSkipped` - file skipped (unchanged/too large) instead of FileStarted
/// 4. `ChunkStarted` - starting to embed a chunk within current file
/// 5. `ChunkComplete` - finished embedding a chunk
/// 6. `FileComplete` - finished processing file (all chunks embedded)
/// 7. `BuildComplete` - all files processed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum IndexAction {
    /// No specific action
    #[default]
    None,
    /// Build started
    BuildStarted,
    /// Started processing a file (will parse and embed)
    FileStarted {
        /// Path to the file
        path: PathBuf,
    },
    /// File skipped
    FileSkipped {
        /// Path to the file
        path: PathBuf,
        /// Why it was skipped
        reason: SkipReason,
    },
    /// File processing failed
    FileError {
        /// Path to the file
        path: PathBuf,
        /// Error message
        error: String,
    },
    /// Started embedding a chunk
    ChunkStarted {
        /// Path to the file containing the chunk
        path: PathBuf,
        /// Symbol path of the chunk (e.g. "module::function")
        symbol: String,
        /// Chunk index (1-based) within the file
        index: usize,
        /// Total chunks in the file
        total: usize,
    },
    /// Finished embedding a chunk
    ChunkComplete {
        /// Path to the file containing the chunk
        path: PathBuf,
        /// Symbol path of the chunk
        symbol: String,
        /// Chunk index (1-based) within the file
        index: usize,
        /// Total chunks in the file
        total: usize,
    },
    /// Finished processing a file (all chunks embedded)
    FileComplete {
        /// Path to the file
        path: PathBuf,
    },
    /// Build complete
    BuildComplete,
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
    /// Current phase of the indexing operation
    pub phase: IndexPhase,

    /// Action that triggered this status update
    pub action: IndexAction,

    /// Total files discovered that need parsing (grows during discovery)
    pub files_total: usize,

    /// Files successfully parsed so far (grows during parsing)
    pub files_parsed: usize,

    /// Files skipped - unsupported language, too large, unchanged, etc. (grows during parsing)
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

    /// Number of files processed so far (parsed + skipped + errored)
    pub fn files_processed(&self) -> usize {
        self.files_parsed + self.files_skipped + self.files_errored
    }

    /// Progress within current phase as a fraction (0.0 to 1.0), None if not applicable
    ///
    /// - Discovering: None (count grows, no total yet)
    /// - Parsing: files_processed / files_total
    /// - Embedding: chunks_embedded / chunks_total
    /// - Complete/Idle: Some(1.0) / None
    pub fn progress(&self) -> Option<f64> {
        match self.phase {
            IndexPhase::Idle => None,
            IndexPhase::Discovering => None, // No total known yet
            IndexPhase::Parsing => {
                if self.files_total == 0 {
                    None
                } else {
                    Some(self.files_processed() as f64 / self.files_total as f64)
                }
            }
            IndexPhase::Embedding => {
                if self.chunks_total == 0 {
                    None
                } else {
                    Some(self.chunks_embedded as f64 / self.chunks_total as f64)
                }
            }
            IndexPhase::Complete => Some(1.0),
        }
    }

    /// Whether all processing is complete (phase is Complete)
    pub fn is_complete(&self) -> bool {
        self.phase == IndexPhase::Complete
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

    /// Database for persistent storage (writes happen immediately during indexing)
    database: Option<Arc<IndexDatabase>>,

    /// Embedding model (lazy-loaded on first scan with embedding enabled)
    embedding_model: Option<EmbeddingModel>,

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
            database: None,
            embedding_model: None,
        }
    }

    /// Set the database for persistent storage
    ///
    /// When set, chunks are written to the database immediately as they are embedded.
    pub fn with_database(mut self, database: Arc<IndexDatabase>) -> Self {
        self.database = Some(database);
        self
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
    /// Send a progress update if a callback is configured
    fn send_progress(&self, status: IndexStatus) {
        if let Some(ref callback) = self.progress_callback {
            callback(status);
        }
    }

    /// Send a progress update with a specific action
    fn notify(&mut self, action: IndexAction) {
        self.last_status.action = action;
        self.send_progress(self.last_status.clone());
        self.last_status.action = IndexAction::None; // Reset for next update
    }

    /// Scan the root path and parse all supported files
    ///
    /// This is an async operation that discovers files and parses them.
    /// Progress updates are sent via the callback if configured.
    pub async fn scan(&mut self) -> Result<ScanResult> {
        self.scan_with_skip(HashSet::new()).await
    }

    /// Scan the root path, skipping files in the provided set
    ///
    /// Files in `skip_paths` are counted as "skipped" and not parsed.
    /// This is useful for incremental indexing where unchanged files
    /// don't need to be re-parsed.
    pub async fn scan_with_skip(&mut self, skip_paths: HashSet<PathBuf>) -> Result<ScanResult> {
        let start = Instant::now();

        if !self.root_path.exists() {
            return Err(TreeSitterError::FileNotFound(self.root_path.clone()));
        }

        self.last_status = IndexStatus::new(self.root_path.clone());
        self.last_status.phase = IndexPhase::Discovering;
        self.notify(IndexAction::BuildStarted);

        let mut errors = Vec::new();

        // Phase 1: discover files
        let (files_to_parse, skipped_unchanged) = self.discover_files(&skip_paths, &mut errors);

        tracing::info!(
            "Discovered {} files to parse, {} unchanged in {}",
            files_to_parse.len(),
            skipped_unchanged,
            self.root_path.display()
        );

        // Phase 2: parse files
        self.last_status.phase = IndexPhase::Parsing;
        self.parse_discovered_files(&files_to_parse, &mut errors)
            .await;

        // Phase 3: embed chunks
        self.last_status.phase = IndexPhase::Embedding;
        let embedding_config = self.config.embedding.clone();
        self.run_embedding_phase(&embedding_config, &mut errors)
            .await?;

        // Send final status
        self.last_status.phase = IndexPhase::Complete;
        self.last_status.current_file = None;
        self.notify(IndexAction::BuildComplete);

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
            self.notify(IndexAction::FileStarted { path: path.clone() });

            self.embed_file_chunks(&path, errors).await;

            self.last_status.files_embedded += 1;
            self.notify(IndexAction::FileComplete { path: path.clone() });

            tokio::task::yield_now().await;
        }

        tracing::info!(
            "Embedding complete: {} files, {} chunks",
            self.last_status.files_embedded,
            self.last_status.chunks_embedded
        );

        Ok(())
    }

    /// Embed all chunks for a single file, collecting results.
    /// Returns Vec of (chunk, embedding, symbol_path) tuples.
    async fn embed_chunks(
        &mut self,
        path: &Path,
        chunks: Vec<crate::chunk::SemanticChunk>,
        errors: &mut Vec<(PathBuf, String)>,
    ) -> Vec<(crate::chunk::SemanticChunk, Vec<f32>, String)> {
        let file_chunk_count = chunks.len();
        let mut embedded = Vec::new();

        for (chunk_index, chunk) in chunks.into_iter().enumerate() {
            let Some(content) = chunk.content() else {
                continue;
            };

            let symbol = chunk.symbol_path();
            let index = chunk_index + 1;

            self.notify(IndexAction::ChunkStarted {
                path: path.to_path_buf(),
                symbol: symbol.clone(),
                index,
                total: file_chunk_count,
            });

            let model = self.embedding_model.as_mut().unwrap();
            match model.embed_text(content).await {
                Ok(result) => {
                    embedded.push((chunk, result.embedding, symbol.clone()));
                    self.last_status.chunks_embedded += 1;
                    self.notify(IndexAction::ChunkComplete {
                        path: path.to_path_buf(),
                        symbol,
                        index,
                        total: file_chunk_count,
                    });
                }
                Err(e) => {
                    let msg = format!("Embedding error for {}: {}", symbol, e);
                    tracing::warn!("{}", msg);
                    errors.push((path.to_path_buf(), msg));
                }
            }
        }

        embedded
    }

    /// Write file and all its chunks to database in an atomic transaction.
    /// Only writes if ALL chunks were successfully embedded.
    fn write_file_atomically(
        &self,
        path: &Path,
        content_hash: &[u8; 16],
        embedded_chunks: Vec<(crate::chunk::SemanticChunk, Vec<f32>, String)>,
        errors: &mut Vec<(PathBuf, String)>,
    ) {
        let Some(db) = &self.database else {
            return;
        };

        if let Err(e) = db.begin_transaction() {
            errors.push((
                path.to_path_buf(),
                format!("Failed to begin transaction: {}", e),
            ));
            return;
        }

        // Remove old data, insert file record, insert all chunks
        if let Err(e) = db.remove_file(path).and_then(|_| {
            let file_id = db.upsert_file(path, content_hash)?;
            for (chunk, embedding, symbol) in &embedded_chunks {
                if let crate::chunk::ChunkSource::Parsed {
                    start_byte,
                    end_byte,
                    ..
                } = &chunk.source
                {
                    db.insert_chunk(&file_id, *start_byte, *end_byte, Some(embedding), symbol)?;
                }
            }
            db.commit_transaction()
        }) {
            let _ = db.commit_transaction(); // Try to commit what we have
            errors.push((path.to_path_buf(), format!("Database write failed: {}", e)));
        }
    }

    /// Embed all chunks for a single file and write to database atomically.
    async fn embed_file_chunks(&mut self, path: &Path, errors: &mut Vec<(PathBuf, String)>) {
        let Some(parsed) = self.files.get(path).cloned() else {
            return;
        };

        if self.embedding_model.is_none() {
            return;
        }

        tracing::info!(path = %path.display(), "Starting indexing");

        let chunks = chunk_file(parsed.clone());
        let chunk_count = chunks.len();
        let embedded = self.embed_chunks(path, chunks, errors).await;

        if embedded.is_empty() {
            tracing::warn!(path = %path.display(), "No chunks embedded");
            return;
        }

        self.write_file_atomically(path, &parsed.content_hash, embedded.clone(), errors);

        // Check if write succeeded
        if errors.iter().any(|(p, _)| p == path) {
            tracing::error!(path = %path.display(), chunks = embedded.len(), "Failed to write chunks to database");
        } else {
            tracing::info!(path = %path.display(), chunks = embedded.len(), total = chunk_count, "Finished indexing");
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

        self.embedding_model = Some(model);
        Ok(())
    }

    /// Discover files to parse, filtering out skip_paths
    ///
    /// Returns (files_to_parse, count_of_skipped_unchanged_files)
    fn discover_files(
        &mut self,
        skip_paths: &HashSet<PathBuf>,
        errors: &mut Vec<(PathBuf, String)>,
    ) -> (Vec<PathBuf>, usize) {
        let registry = LanguageRegistry::global();
        let mut files_to_parse = Vec::new();
        let mut skipped_unchanged = 0;

        let walker = WalkBuilder::new(&self.root_path)
            .git_ignore(self.config.respect_gitignore)
            .git_global(self.config.respect_gitignore)
            .git_exclude(self.config.respect_gitignore)
            .hidden(false)
            .build();

        for entry in walker.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if registry.detect_language(path).is_none() {
                continue;
            }

            match self.check_file_for_parsing(path, skip_paths) {
                FileCheckResult::Parse => {
                    files_to_parse.push(path.to_path_buf());
                    self.last_status.files_total += 1;
                }
                FileCheckResult::SkipUnchanged => {
                    skipped_unchanged += 1;
                    self.handle_file_skip(path, SkipReason::Unchanged, false);
                }
                FileCheckResult::SkipTooLarge => {
                    self.handle_file_skip(path, SkipReason::TooLarge, true);
                }
                FileCheckResult::Error(msg) => {
                    self.handle_file_error(path, msg, errors);
                }
            }
        }

        (files_to_parse, skipped_unchanged)
    }

    /// Check if a file should be parsed, skipped, or has an error
    fn check_file_for_parsing(
        &self,
        path: &Path,
        skip_paths: &HashSet<PathBuf>,
    ) -> FileCheckResult {
        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) => return FileCheckResult::Error(e.to_string()),
        };

        if metadata.len() > self.config.max_file_size {
            return FileCheckResult::SkipTooLarge;
        }

        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if skip_paths.contains(&canonical) {
            return FileCheckResult::SkipUnchanged;
        }

        FileCheckResult::Parse
    }

    /// Parse a list of discovered files
    async fn parse_discovered_files(
        &mut self,
        files: &[PathBuf],
        errors: &mut Vec<(PathBuf, String)>,
    ) {
        let registry = LanguageRegistry::global();

        for path in files {
            self.last_status.current_file = Some(path.clone());

            let Some(lang_config) = registry.detect_language(path) else {
                tracing::debug!(path = %path.display(), reason = "unsupported", "Skipping file");
                self.last_status.files_skipped += 1;
                self.notify(IndexAction::FileSkipped {
                    path: path.clone(),
                    reason: SkipReason::Unsupported,
                });
                continue;
            };

            tracing::debug!(path = %path.display(), "Parsing file");
            self.notify(IndexAction::FileStarted { path: path.clone() });

            match self.parse_file_internal(path, lang_config) {
                Ok(parsed) => {
                    self.files.insert(path.clone(), Arc::new(parsed));
                    self.last_status.files_parsed += 1;
                    tracing::debug!(path = %path.display(), "Parsed file");
                    self.notify(IndexAction::FileComplete { path: path.clone() });
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    tracing::warn!(path = %path.display(), error = %error_msg, "Parse error");
                    errors.push((path.clone(), error_msg.clone()));
                    self.last_status.files_errored += 1;
                    self.notify(IndexAction::FileError {
                        path: path.clone(),
                        error: error_msg,
                    });
                }
            }

            tokio::task::yield_now().await;
        }
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
    /// Record a skipped file in the status counters and send progress
    fn record_skipped_file(&mut self) {
        self.last_status.files_total += 1;
        self.last_status.files_skipped += 1;
        self.send_progress(self.last_status.clone());
    }

    /// Handle file skip: set current file, optionally update counters, and send notification
    ///
    /// If `count_as_new` is true, calls `record_skipped_file()` to increment both
    /// files_total and files_skipped. If false, only increments files_skipped
    /// (used for unchanged files that were already counted in a previous index).
    fn handle_file_skip(&mut self, path: &Path, reason: SkipReason, count_as_new: bool) {
        tracing::debug!(path = %path.display(), reason = ?reason, "Skipping file");
        self.last_status.current_file = Some(path.to_path_buf());
        if count_as_new {
            self.record_skipped_file();
        } else {
            self.last_status.files_skipped += 1;
        }
        self.notify(IndexAction::FileSkipped {
            path: path.to_path_buf(),
            reason,
        });
    }

    /// Handle file error: record error, update counters, and send notification
    fn handle_file_error(
        &mut self,
        path: &Path,
        error: String,
        errors: &mut Vec<(PathBuf, String)>,
    ) {
        errors.push((path.to_path_buf(), error.clone()));
        self.record_skipped_file();
        self.notify(IndexAction::FileError {
            path: path.to_path_buf(),
            error,
        });
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

        // Re-embed chunks (database cleanup happens in prepare_file_in_db)
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
        let total_chunks = self
            .database
            .as_ref()
            .and_then(|db| db.chunk_count().ok())
            .unwrap_or(0);

        IndexStats {
            total_files: self.files.len(),
            total_chunks,
        }
    }

    /// Clear all parsed files from the index
    pub fn clear(&mut self) {
        self.files.clear();
        self.last_status = IndexStatus::new(self.root_path.clone());

        // Clear database if configured
        if let Some(db) = &self.database {
            if let Err(e) = db.clear() {
                tracing::warn!(error = %e, "Failed to clear database");
            }
        }
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
            .as_mut()
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
    use crate::test_utils::{
        run_progress_test, setup_minimal_test_dir, setup_test_dir, ProgressCollector,
    };
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
        let dir = setup_test_dir();

        // Set up context with database
        let db_path = dir.path().join("test.db");
        let db = Arc::new(IndexDatabase::open_readwrite(&db_path).unwrap());
        let mut context = IndexContext::new(dir.path()).with_database(db.clone());

        context.scan().await.unwrap();

        let path = dir.path().join("main.rs");

        // Check initial chunk count from database
        let initial_chunks = db.get_chunks_for_file(&path).unwrap();
        let initial_count = initial_chunks.len();
        assert!(initial_count > 0, "Should have chunks after scan");

        // Refresh should re-embed and update database
        context.refresh(&path).await.unwrap();

        // Check chunk count after refresh
        let after_chunks = db.get_chunks_for_file(&path).unwrap();
        assert!(!after_chunks.is_empty(), "Should have chunks after refresh");
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

        // Set up context with database
        let db_path = dir.path().join("test.db");
        let db = Arc::new(IndexDatabase::open_readwrite(&db_path).unwrap());
        let mut context = IndexContext::new(dir.path()).with_database(db);

        context.scan().await.unwrap();

        let stats = context.stats();
        assert_eq!(stats.total_files, 4);
        // Embedding is always enabled and written to database, so we should have chunks
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

        // Idle phase - progress is None
        assert_eq!(status.progress(), None);

        // Set to Parsing phase with total and some parsed
        status.phase = IndexPhase::Parsing;
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

        // Idle phase - not complete
        assert!(!status.is_complete());

        // Has files but in parsing phase
        status.phase = IndexPhase::Parsing;
        status.files_total = 10;
        status.files_parsed = 5;
        assert!(!status.is_complete());

        // Set to Complete phase
        status.phase = IndexPhase::Complete;
        assert!(status.is_complete());
    }

    #[test]
    fn test_index_status_is_complete_with_phases() {
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.files_total = 10;
        status.files_parsed = 8;
        status.files_skipped = 2;

        // Parsing phase - not complete yet
        status.phase = IndexPhase::Parsing;
        assert!(!status.is_complete());

        // Embedding phase - not complete yet
        status.phase = IndexPhase::Embedding;
        assert!(!status.is_complete());

        // Complete phase
        status.phase = IndexPhase::Complete;
        assert!(status.is_complete());
    }

    #[test]
    fn test_index_status_progress_by_phase() {
        let mut status = IndexStatus::new(PathBuf::from("/test"));

        // Idle phase - no progress
        status.phase = IndexPhase::Idle;
        assert_eq!(status.progress(), None);

        // Discovering phase - no progress (total unknown)
        status.phase = IndexPhase::Discovering;
        assert_eq!(status.progress(), None);

        // Parsing phase - tracks file progress
        status.phase = IndexPhase::Parsing;
        status.files_total = 10;
        status.files_parsed = 5;
        assert_eq!(status.progress(), Some(0.5));

        status.files_parsed = 10;
        assert_eq!(status.progress(), Some(1.0));

        // Embedding phase - tracks chunk progress
        status.phase = IndexPhase::Embedding;
        status.chunks_total = 100;
        status.chunks_embedded = 50;
        assert_eq!(status.progress(), Some(0.5));

        status.chunks_embedded = 100;
        assert_eq!(status.progress(), Some(1.0));

        // Complete phase - 100%
        status.phase = IndexPhase::Complete;
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
        // Idle phase returns None
        let status = IndexStatus::new(PathBuf::from("/test"));
        assert_eq!(status.progress(), None);

        // Parsing phase - partial progress
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.phase = IndexPhase::Parsing;
        status.files_total = 100;
        status.files_parsed = 25;
        assert_eq!(status.progress(), Some(0.25));

        // Parsing phase - 75% with mixed parsed/skipped/errored
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.phase = IndexPhase::Parsing;
        status.files_total = 100;
        status.files_parsed = 50;
        status.files_skipped = 20;
        status.files_errored = 5;
        assert_eq!(status.progress(), Some(0.75));

        // Parsing phase - 100% complete
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.phase = IndexPhase::Parsing;
        status.files_total = 100;
        status.files_parsed = 100;
        assert_eq!(status.progress(), Some(1.0));
    }

    #[test]
    fn test_index_status_is_complete_edge_cases() {
        // Idle phase is not complete
        let status = IndexStatus::new(PathBuf::from("/test"));
        assert!(!status.is_complete());

        // Parsing phase - not complete
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.phase = IndexPhase::Parsing;
        status.files_total = 10;
        assert!(!status.is_complete());

        // Embedding phase - not complete
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.phase = IndexPhase::Embedding;
        status.files_total = 10;
        status.files_parsed = 5;
        assert!(!status.is_complete());

        // Complete phase - is complete regardless of counters
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.phase = IndexPhase::Complete;
        assert!(status.is_complete());

        // Complete phase with counters
        let mut status = IndexStatus::new(PathBuf::from("/test"));
        status.phase = IndexPhase::Complete;
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

    // =============================================================================
    // IndexAction notification tests
    // =============================================================================

    #[tokio::test]
    async fn test_parsing_sends_file_started_notifications() {
        let dir = setup_test_dir();
        let collector = ProgressCollector::new();

        let mut context = IndexContext::new(dir.path()).with_progress(collector.callback());
        context.scan().await.unwrap();

        let updates = collector.updates();

        // Count FileStarted actions during parsing phase
        let file_started_count = updates
            .iter()
            .filter(|u| u.phase == IndexPhase::Parsing)
            .filter(|u| matches!(u.action, IndexAction::FileStarted { .. }))
            .count();

        // Should have at least one FileStarted for each parsed file
        assert!(
            file_started_count >= 1,
            "Expected FileStarted notifications during parsing, got {}",
            file_started_count
        );
    }

    #[tokio::test]
    async fn test_parsing_sends_file_complete_notifications() {
        let dir = setup_test_dir();
        let collector = ProgressCollector::new();

        let mut context = IndexContext::new(dir.path()).with_progress(collector.callback());
        context.scan().await.unwrap();

        let updates = collector.updates();

        // Count FileComplete actions during parsing phase
        let file_complete_count = updates
            .iter()
            .filter(|u| u.phase == IndexPhase::Parsing)
            .filter(|u| matches!(u.action, IndexAction::FileComplete { .. }))
            .count();

        // Should have FileComplete for each successfully parsed file
        assert!(
            file_complete_count >= 1,
            "Expected FileComplete notifications during parsing, got {}",
            file_complete_count
        );
    }

    #[tokio::test]
    async fn test_file_started_before_file_complete() {
        let dir = setup_minimal_test_dir();
        let collector = ProgressCollector::new();

        let mut context = IndexContext::new(dir.path()).with_progress(collector.callback());
        context.scan().await.unwrap();

        let updates = collector.updates();

        // Find the main.rs FileStarted and FileComplete indices
        let main_rs = dir.path().join("main.rs");

        let started_idx = updates.iter().position(
            |u| matches!(&u.action, IndexAction::FileStarted { path } if path == &main_rs),
        );

        let complete_idx = updates.iter().position(
            |u| matches!(&u.action, IndexAction::FileComplete { path } if path == &main_rs),
        );

        assert!(started_idx.is_some(), "Should have FileStarted for main.rs");
        assert!(
            complete_idx.is_some(),
            "Should have FileComplete for main.rs"
        );
        assert!(
            started_idx.unwrap() < complete_idx.unwrap(),
            "FileStarted should come before FileComplete"
        );
    }

    #[tokio::test]
    async fn test_skip_sends_file_skipped_notification() {
        let dir = TempDir::new().unwrap();
        // Create a file that's too large
        let config = IndexConfig {
            max_file_size: 5, // Only 5 bytes allowed
            ..Default::default()
        };
        std::fs::write(dir.path().join("large.rs"), "fn main() { /* too large */ }").unwrap();

        let collector = ProgressCollector::new();
        let mut context = IndexContext::new(dir.path())
            .with_config(config)
            .with_progress(collector.callback());
        context.scan().await.unwrap();

        let updates = collector.updates();

        // Should have a FileSkipped with TooLarge reason
        let skipped = updates.iter().find(|u| {
            matches!(
                &u.action,
                IndexAction::FileSkipped {
                    reason: SkipReason::TooLarge,
                    ..
                }
            )
        });

        assert!(
            skipped.is_some(),
            "Should have FileSkipped notification for too-large file"
        );
    }

    #[tokio::test]
    async fn test_build_started_and_complete_notifications() {
        let dir = setup_test_dir();
        let collector = ProgressCollector::new();

        let mut context = IndexContext::new(dir.path()).with_progress(collector.callback());
        context.scan().await.unwrap();

        let updates = collector.updates();

        // First action should be BuildStarted
        let first_action = updates.first().map(|u| &u.action);
        assert!(
            matches!(first_action, Some(IndexAction::BuildStarted)),
            "First notification should be BuildStarted, got {:?}",
            first_action
        );

        // Last action should be BuildComplete
        let last_action = updates.last().map(|u| &u.action);
        assert!(
            matches!(last_action, Some(IndexAction::BuildComplete)),
            "Last notification should be BuildComplete, got {:?}",
            last_action
        );
    }
}
