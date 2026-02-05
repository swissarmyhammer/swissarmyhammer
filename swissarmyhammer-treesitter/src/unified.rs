//! Workspace with automatic leader/client mode using SQLite storage
//!
//! The `Workspace` struct transparently handles whether this process is the leader
//! (performing writes to the index) or a reader (querying the index).
//!
//! # Storage
//!
//! The index is stored in a SQLite database in WAL mode, allowing one writer (leader)
//! and multiple concurrent readers (non-leaders). The leader performs batch writes
//! per file during indexing.
//!
//! # Example
//!
//! ```ignore
//! use swissarmyhammer_treesitter::Workspace;
//!
//! // Open workspace and set up progress callback before building
//! let workspace = Workspace::new("/path/to/workspace")
//!     .with_progress(|status| {
//!         println!("Progress: {}/{} files", status.files_parsed, status.files_total);
//!     })
//!     .open()
//!     .await?;
//!
//! // Build the index (only works if we're the leader)
//! workspace.build().await?;
//!
//! // Query the workspace - works the same regardless of mode
//! let status = workspace.status().await?;
//! let files = workspace.list_files().await?;
//! let results = workspace.semantic_search("fn main", 10, 0.7).await?;
//! ```

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ignore::WalkBuilder;
use tokio::sync::RwLock as TokioRwLock;

use swissarmyhammer_leader_election::{ElectionConfig, ElectionError, LeaderElection, LeaderGuard};

use crate::chunk::{cosine_similarity, SemanticChunk};
use crate::db::{database_path, EmbeddedChunkRecord, IndexDatabase};
use crate::index::{compute_file_hash, IndexConfig, IndexContext, IndexStatus, ProgressCallback};
use crate::query::{
    check_ready, ChunkResult, DuplicateCluster, IndexStatusInfo, QueryError, QueryMatch,
    SimilarChunkResult,
};
use crate::{Result, TreeSitterError};

/// Maximum number of similar chunks to return when finding duplicates for a file
const DUPLICATES_TOP_K: usize = 100;

/// Internal mode of the workspace
enum WorkspaceMode {
    /// This process owns the index and writes to the database
    /// Note: With background indexing, this variant is not currently constructed.
    /// Leader work happens in a detached background task. Kept for future use.
    #[allow(dead_code)]
    Leader {
        /// The database for persistent storage (internally thread-safe)
        db: Arc<IndexDatabase>,
        /// Guard that holds the leader lock (released on drop)
        _guard: LeaderGuard,
    },
    /// This process reads from the database only
    Reader {
        /// Read-only database connection
        db: Arc<IndexDatabase>,
    },
}

/// Builder for creating a Workspace with configuration
pub struct WorkspaceBuilder {
    workspace_root: PathBuf,
    election_config: ElectionConfig,
    index_config: IndexConfig,
    progress_callback: Option<ProgressCallback>,
}

impl WorkspaceBuilder {
    /// Create a new workspace builder for the given path
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
            election_config: ElectionConfig::default(),
            index_config: IndexConfig::default(),
            progress_callback: None,
        }
    }

    /// Set a progress callback for indexing operations
    ///
    /// The callback is called during the build phase with status updates.
    pub fn with_progress<F>(mut self, callback: F) -> Self
    where
        F: Fn(IndexStatus) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Arc::new(callback));
        self
    }

    /// Set custom election configuration
    pub fn with_election_config(mut self, config: ElectionConfig) -> Self {
        self.election_config = config;
        self
    }

    /// Set custom index configuration
    pub fn with_index_config(mut self, config: IndexConfig) -> Self {
        self.index_config = config;
        self
    }

    /// Open the workspace, establishing leader/reader mode
    ///
    /// This does NOT start indexing. Call `build()` to index the workspace.
    pub async fn open(self) -> Result<Workspace> {
        Workspace::open_internal(
            self.workspace_root,
            self.election_config,
            self.index_config,
            self.progress_callback,
        )
        .await
    }
}

/// A tree-sitter workspace with automatic leader/client mode.
///
/// `Workspace` transparently handles whether this process is the leader
/// (holding the index and writing to the database) or a reader (querying the database).
///
/// # Leader Election
///
/// When you call `Workspace::open()`:
/// 1. First, it tries to become the leader
/// 2. If another process is leader, it becomes a reader
/// 3. Call `build()` to start indexing (only leaders can build)
///
/// # Example
///
/// ```ignore
/// // Set up workspace with progress callback
/// let workspace = Workspace::new("/path/to/workspace")
///     .with_progress(|status| {
///         println!("{}/{} files", status.files_parsed, status.files_total);
///     })
///     .open()
///     .await?;
///
/// // Build the index (only works for leader)
/// workspace.build().await?;
///
/// // Query the workspace
/// let status = workspace.status().await?;
/// let files = workspace.list_files().await?;
/// ```
pub struct Workspace {
    mode: WorkspaceMode,
    election: LeaderElection,
    workspace_root: PathBuf,
    /// Index context - always present, configured with callbacks
    context: Arc<TokioRwLock<IndexContext>>,
    /// Whether build() has been called
    is_built: std::sync::atomic::AtomicBool,
}

impl Workspace {
    /// Create a new workspace builder for the given path
    ///
    /// Use the builder to configure callbacks before opening:
    /// ```ignore
    /// let workspace = Workspace::new("/path")
    ///     .with_progress(|status| println!("{:?}", status))
    ///     .open()
    ///     .await?;
    /// ```
    pub fn new(workspace_root: impl AsRef<Path>) -> WorkspaceBuilder {
        WorkspaceBuilder::new(workspace_root)
    }

    /// Open a workspace, automatically handling leader election.
    ///
    /// Returns immediately as a Reader. If no leader exists, spawns a background
    /// task to build the index. The index is eventually consistent - queries return
    /// current database state which may be incomplete during initial indexing.
    ///
    /// For more control, use `Workspace::new().with_progress(...).open()`.
    pub async fn open(workspace_root: impl AsRef<Path>) -> Result<Self> {
        Self::open_internal(
            workspace_root.as_ref().to_path_buf(),
            ElectionConfig::default(),
            IndexConfig::default(),
            None,
        )
        .await
    }

    /// Open a workspace with custom configuration.
    ///
    /// Returns immediately as a Reader. If no leader exists, spawns a background
    /// task to build the index.
    pub async fn open_with_config(
        workspace_root: impl AsRef<Path>,
        election_config: ElectionConfig,
        index_config: Option<IndexConfig>,
    ) -> Result<Self> {
        Self::open_internal(
            workspace_root.as_ref().to_path_buf(),
            election_config,
            index_config.unwrap_or_default(),
            None,
        )
        .await
    }

    /// Wait for database to be ready by attempting to open and query.
    /// Uses exponential backoff with a maximum timeout.
    async fn wait_for_database_ready(db_path: &Path, timeout: std::time::Duration) -> Result<()> {
        let start = std::time::Instant::now();
        let mut backoff = std::time::Duration::from_millis(50);

        loop {
            if start.elapsed() > timeout {
                return Err(TreeSitterError::database_error(
                    "Timeout waiting for database schema to be created",
                ));
            }

            // Try to open readonly and verify schema
            if let Ok(db) = IndexDatabase::open_readonly(db_path) {
                // Verify schema exists by attempting a query
                if db.file_count().is_ok() {
                    tracing::debug!("Database schema verified ready");
                    return Ok(());
                }
            }

            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(std::time::Duration::from_millis(500));
        }
    }

    /// Spawn a background indexer task that builds the index once and exits.
    fn spawn_background_indexer(
        workspace_root: PathBuf,
        db: Arc<IndexDatabase>,
        context: IndexContext,
        skip_paths: HashSet<PathBuf>,
        guard: LeaderGuard,
    ) {
        tokio::spawn(async move {
            let _guard = guard; // Hold guard for duration of task

            let mut ctx = context.with_database(db);

            match ctx.scan_with_skip(skip_paths).await {
                Ok(result) => {
                    tracing::info!(
                        "Background indexing complete for {}: {} files parsed, {} skipped",
                        workspace_root.display(),
                        result.files_parsed,
                        result.files_skipped
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Background indexing failed for {}: {}",
                        workspace_root.display(),
                        e
                    );
                }
            }

            // Guard drops here, releasing leader lock
            // Task exits
        });
    }

    /// Internal open implementation
    async fn open_internal(
        workspace_root: PathBuf,
        election_config: ElectionConfig,
        index_config: IndexConfig,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<Self> {
        // Ensure root .gitignore contains tree-sitter database entries
        crate::db::ensure_root_gitignore(&workspace_root).map_err(|e| {
            TreeSitterError::database_error(format!("Failed to update root .gitignore: {}", e))
        })?;

        let election = LeaderElection::with_config(&workspace_root, election_config);
        let db_path = database_path(&workspace_root);

        // Create the index context with configuration
        let mut context = IndexContext::new(&workspace_root).with_config(index_config);
        if let Some(callback) = progress_callback {
            context = context.with_progress_callback(callback);
        }

        // Try to become the leader (non-blocking)
        match election.try_become_leader() {
            Ok(guard) => {
                tracing::info!(
                    "Becoming tree-sitter index leader for {}",
                    workspace_root.display()
                );

                // Create database (file + schema) synchronously
                let leader_db = IndexDatabase::open_readwrite(&db_path).map_err(|e| {
                    TreeSitterError::database_error(format!("Failed to create database: {}", e))
                })?;
                let leader_db = Arc::new(leader_db);

                // Compute skip set for background task
                let skip_paths = Self::compute_unchanged_files_static(&workspace_root, &leader_db)?;

                // Spawn background indexer (takes ownership of guard)
                Self::spawn_background_indexer(
                    workspace_root.clone(),
                    leader_db.clone(),
                    context,
                    skip_paths,
                    guard,
                );

                // Open our own readonly database for Reader mode
                let reader_db = IndexDatabase::open_readonly(&db_path).map_err(|e| {
                    TreeSitterError::database_error(format!(
                        "Failed to open reader database: {}",
                        e
                    ))
                })?;

                let ctx = IndexContext::new(&workspace_root);

                Ok(Self {
                    mode: WorkspaceMode::Reader {
                        db: Arc::new(reader_db),
                    },
                    election,
                    workspace_root,
                    context: Arc::new(TokioRwLock::new(ctx)),
                    is_built: std::sync::atomic::AtomicBool::new(false),
                })
            }
            Err(ElectionError::LockHeld) => {
                // Another process is the leader
                tracing::debug!("Another process is leader, waiting for database to be ready");

                // Wait for database to be ready with timeout
                Self::wait_for_database_ready(&db_path, std::time::Duration::from_secs(5)).await?;

                let db = IndexDatabase::open_readonly(&db_path).map_err(|e| {
                    TreeSitterError::database_error(format!(
                        "Failed to open database in read-only mode: {}",
                        e
                    ))
                })?;

                let ctx = IndexContext::new(&workspace_root);

                Ok(Self {
                    mode: WorkspaceMode::Reader { db: Arc::new(db) },
                    election,
                    workspace_root,
                    context: Arc::new(TokioRwLock::new(ctx)),
                    is_built: std::sync::atomic::AtomicBool::new(false),
                })
            }
            Err(e) => Err(TreeSitterError::connection_error(format!(
                "Failed to acquire index leader lock: {}",
                e
            ))),
        }
    }

    /// Build the index by scanning and parsing files.
    ///
    /// This method:
    /// - Only works if this workspace is the leader
    /// - Scans all files in the workspace
    /// - Parses them with tree-sitter
    /// - Generates embeddings
    /// - Writes results to the database
    ///
    /// Uses incremental indexing: files that haven't changed since the last
    /// index build (based on content hash) are skipped.
    ///
    /// Progress is reported through the callback set with `with_progress()`.
    ///
    /// # Errors
    ///
    /// Returns `TreeSitterError::NotLeader` if called on a reader workspace.
    pub async fn build(&self) -> Result<()> {
        // Check if we're the leader
        let db = match &self.mode {
            WorkspaceMode::Leader { db, .. } => db.clone(),
            WorkspaceMode::Reader { .. } => {
                return Err(TreeSitterError::not_leader(
                    "Cannot build index: another process is the leader",
                ));
            }
        };

        // Compute skip set: files that haven't changed since last index
        let skip_paths = self.compute_unchanged_files(&db)?;
        let skip_count = skip_paths.len();
        if skip_count > 0 {
            tracing::info!("{} files unchanged, skipping re-index", skip_count);
        }

        // Scan and parse files, skipping unchanged ones
        // Database writes happen immediately during embedding in embed_file_chunks
        let mut context = self.context.write().await;
        let result = context.scan_with_skip(skip_paths).await?;

        tracing::info!(
            "Index scan complete: {} files parsed, {} skipped, {} errors in {}ms",
            result.files_parsed,
            result.files_skipped,
            result.errors.len(),
            result.total_time_ms
        );

        self.is_built
            .store(true, std::sync::atomic::Ordering::SeqCst);

        Ok(())
    }

    /// Static version of compute_unchanged_files for background indexer.
    fn compute_unchanged_files_static(
        workspace_root: &Path,
        db: &Arc<IndexDatabase>,
    ) -> Result<HashSet<PathBuf>> {
        let mut skip_paths = HashSet::new();
        let registry = crate::language::LanguageRegistry::global();

        let walker = WalkBuilder::new(workspace_root)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .hidden(false)
            .build();

        for entry in walker.flatten() {
            let path = entry.path();
            if !path.is_file() || registry.detect_language(path).is_none() {
                continue;
            }

            if let Some(canonical) = Self::check_file_unchanged(path, db) {
                skip_paths.insert(canonical);
            }
        }

        Ok(skip_paths)
    }

    /// Compute the set of files that haven't changed since the last index build
    ///
    /// Walks the workspace, computes content hashes, and checks against the database.
    /// Returns canonical paths of unchanged files to skip during scanning.
    fn compute_unchanged_files(&self, db: &Arc<IndexDatabase>) -> Result<HashSet<PathBuf>> {
        Self::compute_unchanged_files_static(&self.workspace_root, db)
    }

    /// Check if a file is unchanged in the database, returning its canonical path if so
    fn check_file_unchanged(path: &Path, db: &Arc<IndexDatabase>) -> Option<PathBuf> {
        let hash = compute_file_hash(path).ok()?;
        let is_current = db.file_is_current(path, &hash).unwrap_or(false);
        if is_current {
            Some(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()))
        } else {
            None
        }
    }

    /// Check if this instance is the leader
    pub fn is_leader(&self) -> bool {
        matches!(self.mode, WorkspaceMode::Leader { .. })
    }

    /// Check if the index has been built
    pub fn is_built(&self) -> bool {
        self.is_built.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Get the workspace root
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Get the socket path for this workspace's index (for backward compatibility)
    pub fn socket_path(&self) -> &Path {
        self.election.socket_path()
    }

    /// Get the database path for this workspace
    pub fn database_path(&self) -> PathBuf {
        database_path(&self.workspace_root)
    }

    /// Get current index status.
    pub async fn status(&self) -> std::result::Result<IndexStatusInfo, QueryError> {
        // Always query the database - it's the source of truth for indexed data
        let db = self.db();
        let file_count = db.file_count().unwrap_or(0);
        let embedded_count = db.embedded_chunk_count().unwrap_or(0);

        Ok(IndexStatusInfo {
            files_total: file_count,
            files_indexed: file_count,
            files_embedded: if embedded_count > 0 { file_count } else { 0 },
            is_ready: file_count > 0,
            root_path: self.workspace_root.clone(),
        })
    }

    /// Get a reference to the database regardless of leader/reader mode.
    fn db(&self) -> &Arc<IndexDatabase> {
        match &self.mode {
            WorkspaceMode::Leader { db, .. } => db,
            WorkspaceMode::Reader { db } => db,
        }
    }

    /// List all files in the index.
    pub async fn list_files(&self) -> std::result::Result<Vec<PathBuf>, QueryError> {
        let db = self.db();
        db.list_files()
            .map_err(|e| QueryError::internal(e.to_string()))
    }

    /// Find all duplicate code clusters across the project.
    ///
    /// Searches the indexed codebase for semantically similar code chunks using
    /// cosine similarity of embeddings. Returns clusters of duplicate code.
    ///
    /// # Arguments
    /// * `min_similarity` - Minimum cosine similarity threshold (0.0 to 1.0)
    /// * `min_chunk_bytes` - Minimum chunk size to consider
    pub async fn find_all_duplicates(
        &self,
        min_similarity: f32,
        min_chunk_bytes: usize,
    ) -> std::result::Result<Vec<DuplicateCluster>, QueryError> {
        // Always query from database - it's the source of truth for indexed data
        let db = self.db();
        let chunks = db.get_all_embedded_chunks().map_err(db_to_query_error)?;
        Ok(find_all_duplicates_from_records(
            &chunks,
            min_similarity,
            min_chunk_bytes,
        ))
    }

    /// Find duplicates for chunks in a specific file.
    ///
    /// Returns chunks from other files similar to chunks in the given file.
    pub async fn find_duplicates_in_file(
        &self,
        file: PathBuf,
        min_similarity: f32,
    ) -> std::result::Result<Vec<SimilarChunkResult>, QueryError> {
        // Always query from database - it's the source of truth for indexed data
        let db = self.db();
        let all_chunks = db.get_all_embedded_chunks().map_err(db_to_query_error)?;
        find_duplicates_in_file_from_records(&all_chunks, &file, min_similarity)
    }

    /// Semantic search - find chunks similar to the given text.
    ///
    /// Embeds the query and finds the most similar indexed chunks.
    /// Note: This requires the embedding model, which is only available in leader mode.
    pub async fn semantic_search(
        &self,
        text: String,
        top_k: usize,
        min_similarity: f32,
    ) -> std::result::Result<Vec<SimilarChunkResult>, QueryError> {
        match &self.mode {
            WorkspaceMode::Leader { .. } => {
                // Need write lock to potentially load embedding model
                let mut ctx = self.context.write().await;
                check_index_ready(&ctx.status())?;

                let query_embedding = ctx
                    .embed_text(&text)
                    .await
                    .map_err(|e| QueryError::embedding_error(e.to_string()))?;

                // Query the database, not the in-memory graph
                let db = self.db();
                let chunks = db.get_all_embedded_chunks().map_err(db_to_query_error)?;
                Ok(semantic_search_from_records(
                    &chunks,
                    &query_embedding,
                    top_k,
                    min_similarity,
                ))
            }
            WorkspaceMode::Reader { .. } => {
                // Reader mode cannot embed text (no model loaded)
                Err(QueryError::embedding_error(
                    "Semantic search not available in reader mode (no embedding model)",
                ))
            }
        }
    }

    /// Execute a tree-sitter query and return matches.
    ///
    /// The query is an S-expression pattern like `(function_item name: (identifier) @name)`.
    ///
    /// Note: This only works in leader mode as it requires the parsed AST.
    pub async fn tree_sitter_query(
        &self,
        query: String,
        files: Option<Vec<PathBuf>>,
        language: Option<String>,
    ) -> std::result::Result<Vec<QueryMatch>, QueryError> {
        match &self.mode {
            WorkspaceMode::Leader { .. } => {
                let ctx = self.context.read().await;
                check_index_ready(&ctx.status())?;
                tree_sitter_query_impl(&ctx, &query, files, language)
            }
            WorkspaceMode::Reader { .. } => {
                // Reader mode cannot run tree-sitter queries (no parsed AST)
                Err(QueryError::internal(
                    "Tree-sitter queries not available in reader mode (no parsed AST)",
                ))
            }
        }
    }

    /// Invalidate a file (force re-parse and re-embed).
    /// Invalidate a file is not supported with background indexing.
    ///
    /// The index is eventually consistent - file changes will be picked up
    /// when a new process starts and detects the changed content hash.
    pub async fn invalidate_file(&self, _file: PathBuf) -> std::result::Result<(), QueryError> {
        Err(QueryError::internal(
            "invalidate_file not supported with background indexing - index is eventually consistent",
        ))
    }
}

// ============================================================================
// Implementation helpers
// ============================================================================

/// Trait for items that can be clustered by embedding similarity
trait Clusterable {
    /// Get the embedding vector for similarity comparison
    fn embedding(&self) -> Option<&[f32]>;
    /// Get the file path for ensuring duplicates are from different files
    fn file_path(&self) -> Option<&Path>;
    /// Get the byte size of this item
    fn byte_len(&self) -> usize;
}

/// Trait for converting items to ChunkResult
trait ToChunkResult {
    /// Convert this item to a ChunkResult
    fn to_chunk_result(&self) -> ChunkResult;
}

impl Clusterable for SemanticChunk {
    fn embedding(&self) -> Option<&[f32]> {
        self.embedding.as_deref()
    }
    fn file_path(&self) -> Option<&Path> {
        self.path()
    }
    fn byte_len(&self) -> usize {
        SemanticChunk::byte_len(self)
    }
}

impl ToChunkResult for SemanticChunk {
    fn to_chunk_result(&self) -> ChunkResult {
        let (start_line, end_line) = self
            .node()
            .map(|n| (n.start_position().row, n.end_position().row))
            .unwrap_or((0, 0));

        let (start_byte, end_byte) = match &self.source {
            crate::chunk::ChunkSource::Parsed {
                start_byte,
                end_byte,
                ..
            } => (*start_byte, *end_byte),
            crate::chunk::ChunkSource::Text(s) => (0, s.len()),
        };

        ChunkResult {
            file: self.path().unwrap_or(Path::new("")).to_path_buf(),
            text: self.content().unwrap_or("").to_string(),
            start_byte,
            end_byte,
            start_line,
            end_line,
        }
    }
}

impl Clusterable for EmbeddedChunkRecord {
    fn embedding(&self) -> Option<&[f32]> {
        Some(&self.embedding)
    }
    fn file_path(&self) -> Option<&Path> {
        Some(&self.path)
    }
    fn byte_len(&self) -> usize {
        self.end_byte - self.start_byte
    }
}

impl ToChunkResult for EmbeddedChunkRecord {
    fn to_chunk_result(&self) -> ChunkResult {
        // Try to read the file and extract the text range
        let (text, start_line, end_line) = std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|content| {
                // Extract the byte range
                let text = content
                    .get(self.start_byte..self.end_byte)
                    .unwrap_or("")
                    .to_string();

                // Calculate line numbers by counting newlines before start_byte
                let before_start = content.get(..self.start_byte).unwrap_or("");
                let start_line = before_start.matches('\n').count();

                // Count newlines in the chunk for end_line
                let newlines_in_chunk = text.matches('\n').count();
                let end_line = start_line + newlines_in_chunk;

                Some((text, start_line, end_line))
            })
            .unwrap_or_else(|| (String::new(), 0, 0));

        ChunkResult {
            file: self.path.clone(),
            text,
            start_byte: self.start_byte,
            end_byte: self.end_byte,
            start_line,
            end_line,
        }
    }
}

/// Union-find helper: find root with path compression
fn union_find_root(parent: &mut [usize], i: usize) -> usize {
    if parent[i] != i {
        parent[i] = union_find_root(parent, parent[i]);
    }
    parent[i]
}

/// Union-find helper: merge two sets
fn union_find_merge(parent: &mut [usize], i: usize, j: usize) {
    let pi = union_find_root(parent, i);
    let pj = union_find_root(parent, j);
    if pi != pj {
        parent[pi] = pj;
    }
}

/// Check if index is ready for queries, allowing empty workspaces
///
/// Empty workspaces (0 files) are considered ready since there's nothing to process.
fn check_index_ready(status: &crate::index::IndexStatus) -> std::result::Result<(), QueryError> {
    if status.files_total > 0 {
        check_ready(status.is_complete())
    } else {
        Ok(())
    }
}

/// Sort similarity results by similarity score in descending order
fn sort_by_similarity_desc(results: &mut [SimilarChunkResult]) {
    results.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Convert database error to QueryError
fn db_to_query_error<E: std::fmt::Display>(e: E) -> QueryError {
    QueryError::internal(e.to_string())
}

/// Filter records matching a specific file path
fn filter_records_for_file<'a>(
    chunks: &'a [EmbeddedChunkRecord],
    file: &Path,
) -> Vec<&'a EmbeddedChunkRecord> {
    chunks.iter().filter(|c| c.path == file).collect()
}

/// Filter records NOT matching a specific file path
fn filter_records_excluding_file<'a>(
    chunks: &'a [EmbeddedChunkRecord],
    file: &Path,
) -> Vec<&'a EmbeddedChunkRecord> {
    chunks.iter().filter(|c| c.path != file).collect()
}

/// Find clusters of similar items using union-find algorithm
fn cluster_by_similarity<T: Clusterable>(items: &[&T], min_similarity: f32) -> Vec<Vec<usize>> {
    let n = items.len();
    if n == 0 {
        return Vec::new();
    }

    let mut parent: Vec<usize> = (0..n).collect();

    for i in 0..n {
        for j in (i + 1)..n {
            if let (Some(emb_i), Some(emb_j)) = (items[i].embedding(), items[j].embedding()) {
                let sim = cosine_similarity(emb_i, emb_j);
                // Only cluster items from different files
                if sim >= min_similarity && items[i].file_path() != items[j].file_path() {
                    union_find_merge(&mut parent, i, j);
                }
            }
        }
    }

    let mut groups: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for i in 0..n {
        let root = union_find_root(&mut parent, i);
        groups.entry(root).or_default().push(i);
    }

    groups.into_values().collect()
}

/// Compute average pairwise similarity within a cluster
fn compute_cluster_similarity<T: Clusterable>(items: &[&T]) -> f32 {
    if items.len() < 2 {
        return 1.0;
    }

    let mut total_sim = 0.0;
    let mut count = 0;

    for i in 0..items.len() {
        for j in (i + 1)..items.len() {
            if let (Some(emb_i), Some(emb_j)) = (items[i].embedding(), items[j].embedding()) {
                total_sim += cosine_similarity(emb_i, emb_j);
                count += 1;
            }
        }
    }

    if count > 0 {
        total_sim / count as f32
    } else {
        0.0
    }
}

/// Generic function to find all duplicate clusters from a collection of items
///
/// This works with any type that implements both Clusterable and ToChunkResult.
fn find_all_duplicates_generic<T: Clusterable + ToChunkResult>(
    items: Vec<&T>,
    min_similarity: f32,
) -> Vec<DuplicateCluster> {
    if items.is_empty() {
        return Vec::new();
    }

    let cluster_indices = cluster_by_similarity(&items, min_similarity);

    cluster_indices
        .into_iter()
        .filter(|indices| indices.len() > 1)
        .map(|indices| {
            let cluster_items: Vec<&T> = indices.iter().map(|&i| items[i]).collect();
            let avg_sim = compute_cluster_similarity(&cluster_items);
            DuplicateCluster {
                chunks: cluster_items.iter().map(|c| c.to_chunk_result()).collect(),
                avg_similarity: avg_sim,
            }
        })
        .collect()
}

/// Find all duplicate clusters from database records
fn find_all_duplicates_from_records(
    chunks: &[EmbeddedChunkRecord],
    min_similarity: f32,
    min_chunk_bytes: usize,
) -> Vec<DuplicateCluster> {
    let filtered: Vec<&EmbeddedChunkRecord> = chunks
        .iter()
        .filter(|c| c.byte_len() >= min_chunk_bytes)
        .collect();

    find_all_duplicates_generic(filtered, min_similarity)
}

/// Finalize similarity results: sort by similarity and optionally truncate
fn finalize_similarity_results(results: &mut Vec<SimilarChunkResult>, limit: Option<usize>) {
    sort_by_similarity_desc(results);
    if let Some(max) = limit {
        results.truncate(max);
    }
}

/// Find duplicates for a specific file from database records
fn find_duplicates_in_file_from_records(
    all_chunks: &[EmbeddedChunkRecord],
    file: &Path,
    min_similarity: f32,
) -> std::result::Result<Vec<SimilarChunkResult>, QueryError> {
    let file_chunks = filter_records_for_file(all_chunks, file);

    if file_chunks.is_empty() {
        return Err(QueryError::file_not_found(file));
    }

    let other_chunks = filter_records_excluding_file(all_chunks, file);

    let mut results = Vec::new();
    for file_chunk in &file_chunks {
        for other in &other_chunks {
            let sim = cosine_similarity(&file_chunk.embedding, &other.embedding);
            if sim >= min_similarity {
                results.push(SimilarChunkResult {
                    chunk: other.to_chunk_result(),
                    similarity: sim,
                });
            }
        }
    }

    finalize_similarity_results(&mut results, Some(DUPLICATES_TOP_K));
    Ok(results)
}

/// Semantic search using database records
fn semantic_search_from_records(
    chunks: &[EmbeddedChunkRecord],
    query_embedding: &[f32],
    top_k: usize,
    min_similarity: f32,
) -> Vec<SimilarChunkResult> {
    let mut results: Vec<SimilarChunkResult> = chunks
        .iter()
        .filter_map(|chunk| {
            let sim = cosine_similarity(query_embedding, &chunk.embedding);
            if sim >= min_similarity {
                Some(SimilarChunkResult {
                    chunk: chunk.to_chunk_result(),
                    similarity: sim,
                })
            } else {
                None
            }
        })
        .collect();

    finalize_similarity_results(&mut results, Some(top_k));
    results
}

/// Execute a tree-sitter query
fn tree_sitter_query_impl(
    index: &IndexContext,
    query: &str,
    files: Option<Vec<PathBuf>>,
    language: Option<String>,
) -> std::result::Result<Vec<QueryMatch>, QueryError> {
    use tree_sitter::StreamingIterator;

    let file_paths = files.unwrap_or_else(|| index.files());
    let registry = crate::language::LanguageRegistry::global();

    let mut results = Vec::new();

    for path in file_paths {
        if let Some(parsed) = index.get(&path) {
            if let Some(lang_config) = registry.detect_language(&path) {
                if language.as_ref().is_some_and(|l| l != lang_config.name) {
                    continue;
                }

                let ts_query = tree_sitter::Query::new(&lang_config.language(), query)
                    .map_err(|e| QueryError::invalid_query(format!("Query error: {}", e)))?;

                let mut cursor = tree_sitter::QueryCursor::new();
                let mut matches =
                    cursor.matches(&ts_query, parsed.root_node(), parsed.source.as_bytes());

                while let Some(m) = matches.next() {
                    let captures: Vec<crate::query::Capture> = m
                        .captures
                        .iter()
                        .map(|cap| {
                            let node = cap.node;
                            crate::query::Capture {
                                name: ts_query.capture_names()[cap.index as usize].to_string(),
                                kind: node.kind().to_string(),
                                text: parsed
                                    .get_text(node.start_byte(), node.end_byte())
                                    .unwrap_or("")
                                    .to_string(),
                                start_byte: node.start_byte(),
                                end_byte: node.end_byte(),
                                start_line: node.start_position().row,
                                end_line: node.end_position().row,
                            }
                        })
                        .collect();

                    if !captures.is_empty() {
                        results.push(QueryMatch {
                            file: path.clone(),
                            captures,
                        });
                    }
                }
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// High similarity threshold for duplicate detection tests
    const TEST_HIGH_SIMILARITY_THRESHOLD: f32 = 0.9;
    /// Medium similarity threshold for duplicate detection tests
    const TEST_MEDIUM_SIMILARITY_THRESHOLD: f32 = 0.5;
    /// Minimum chunk size for duplicate detection tests
    const TEST_MIN_CHUNK_BYTES: usize = 10;
    /// Small minimum chunk size for tests with small code samples
    const TEST_SMALL_MIN_CHUNK_BYTES: usize = 5;

    // Test timeouts and delays for background indexing tests
    /// Timeout for successful database verification (1 second)
    const TEST_DB_READY_TIMEOUT_SUCCESS: std::time::Duration = std::time::Duration::from_secs(1);
    /// Timeout for expected timeout failures (100 milliseconds)
    const TEST_DB_READY_TIMEOUT_FAIL: std::time::Duration = std::time::Duration::from_millis(100);
    /// Delay before creating database in retry test (200 milliseconds)
    const TEST_DB_CREATE_DELAY: std::time::Duration = std::time::Duration::from_millis(200);
    /// Timeout for retry test (2 seconds)
    const TEST_DB_READY_TIMEOUT_RETRY: std::time::Duration = std::time::Duration::from_secs(2);
    /// Time to wait for background indexing to complete (2 seconds)
    const TEST_BACKGROUND_INDEX_WAIT: std::time::Duration = std::time::Duration::from_secs(2);
    /// Extended time to wait for background indexing and lock release (3 seconds)
    const TEST_BACKGROUND_INDEX_WAIT_LONG: std::time::Duration = std::time::Duration::from_secs(3);

    // =========================================================================
    // Tests for clustering helper functions
    // =========================================================================

    #[test]
    fn test_union_find_root_single_element() {
        let mut parent = vec![0];
        assert_eq!(union_find_root(&mut parent, 0), 0);
    }

    #[test]
    fn test_union_find_root_finds_root() {
        let mut parent = vec![1, 2, 2]; // 0 -> 1 -> 2, 2 is root
        assert_eq!(union_find_root(&mut parent, 0), 2);
        // After path compression, 0 should point directly to 2
        assert_eq!(parent[0], 2);
    }

    #[test]
    fn test_union_find_merge_separate_sets() {
        let mut parent = vec![0, 1, 2]; // Three separate sets
        union_find_merge(&mut parent, 0, 1);
        // 0 and 1 should now be in the same set
        assert_eq!(
            union_find_root(&mut parent, 0),
            union_find_root(&mut parent, 1)
        );
    }

    #[test]
    fn test_union_find_merge_same_set() {
        let mut parent = vec![1, 1]; // Both in same set
        union_find_merge(&mut parent, 0, 1);
        // Should not change anything
        assert_eq!(
            union_find_root(&mut parent, 0),
            union_find_root(&mut parent, 1)
        );
    }

    /// Test item for clustering tests
    struct TestClusterable {
        embedding: Vec<f32>,
        path: PathBuf,
        size: usize,
    }

    impl Clusterable for TestClusterable {
        fn embedding(&self) -> Option<&[f32]> {
            Some(&self.embedding)
        }
        fn file_path(&self) -> Option<&Path> {
            Some(&self.path)
        }
        fn byte_len(&self) -> usize {
            self.size
        }
    }

    // =========================================================================
    // Tests for Clusterable and ToChunkResult traits
    // =========================================================================

    #[test]
    fn test_clusterable_byte_len() {
        let item = TestClusterable {
            embedding: vec![1.0, 0.0],
            path: PathBuf::from("/test.rs"),
            size: 100,
        };
        assert_eq!(item.byte_len(), 100);
    }

    #[test]
    fn test_embedded_chunk_record_clusterable() {
        let record = EmbeddedChunkRecord {
            path: PathBuf::from("/test.rs"),
            start_byte: 10,
            end_byte: 50,
            embedding: vec![1.0, 0.0, 0.0],
            symbol_path: "test::func".to_string(),
        };
        assert_eq!(record.byte_len(), 40);
        assert_eq!(record.file_path(), Some(Path::new("/test.rs")));
        assert!(record.embedding().is_some());
    }

    #[test]
    fn test_embedded_chunk_record_to_chunk_result() {
        let record = EmbeddedChunkRecord {
            path: PathBuf::from("/test.rs"),
            start_byte: 10,
            end_byte: 50,
            embedding: vec![1.0, 0.0, 0.0],
            symbol_path: "test::func".to_string(),
        };
        let result = record.to_chunk_result();
        assert_eq!(result.file, PathBuf::from("/test.rs"));
        assert_eq!(result.start_byte, 10);
        assert_eq!(result.end_byte, 50);
        assert_eq!(result.start_line, 0); // Not stored in database
        assert_eq!(result.end_line, 0);
        assert!(result.text.is_empty()); // Text not stored in database
    }

    #[test]
    fn test_finalize_similarity_results_sorts_descending() {
        let mut results = vec![
            SimilarChunkResult {
                chunk: ChunkResult {
                    file: PathBuf::from("/a.rs"),
                    text: String::new(),
                    start_byte: 0,
                    end_byte: 10,
                    start_line: 0,
                    end_line: 0,
                },
                similarity: 0.5,
            },
            SimilarChunkResult {
                chunk: ChunkResult {
                    file: PathBuf::from("/b.rs"),
                    text: String::new(),
                    start_byte: 0,
                    end_byte: 10,
                    start_line: 0,
                    end_line: 0,
                },
                similarity: 0.9,
            },
            SimilarChunkResult {
                chunk: ChunkResult {
                    file: PathBuf::from("/c.rs"),
                    text: String::new(),
                    start_byte: 0,
                    end_byte: 10,
                    start_line: 0,
                    end_line: 0,
                },
                similarity: 0.7,
            },
        ];

        finalize_similarity_results(&mut results, None);

        assert_eq!(results[0].similarity, 0.9);
        assert_eq!(results[1].similarity, 0.7);
        assert_eq!(results[2].similarity, 0.5);
    }

    #[test]
    fn test_finalize_similarity_results_truncates() {
        let mut results = vec![
            SimilarChunkResult {
                chunk: ChunkResult {
                    file: PathBuf::from("/a.rs"),
                    text: String::new(),
                    start_byte: 0,
                    end_byte: 10,
                    start_line: 0,
                    end_line: 0,
                },
                similarity: 0.9,
            },
            SimilarChunkResult {
                chunk: ChunkResult {
                    file: PathBuf::from("/b.rs"),
                    text: String::new(),
                    start_byte: 0,
                    end_byte: 10,
                    start_line: 0,
                    end_line: 0,
                },
                similarity: 0.8,
            },
            SimilarChunkResult {
                chunk: ChunkResult {
                    file: PathBuf::from("/c.rs"),
                    text: String::new(),
                    start_byte: 0,
                    end_byte: 10,
                    start_line: 0,
                    end_line: 0,
                },
                similarity: 0.7,
            },
        ];

        finalize_similarity_results(&mut results, Some(2));

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].similarity, 0.9);
        assert_eq!(results[1].similarity, 0.8);
    }

    #[test]
    fn test_cluster_by_similarity_empty() {
        let items: Vec<&TestClusterable> = vec![];
        let clusters = cluster_by_similarity(&items, 0.9);
        assert!(clusters.is_empty());
    }

    #[test]
    fn test_cluster_by_similarity_single_item() {
        let item = TestClusterable {
            embedding: vec![1.0, 0.0, 0.0],
            path: PathBuf::from("/a.rs"),
            size: 50,
        };
        let items = vec![&item];
        let clusters = cluster_by_similarity(&items, 0.9);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0], vec![0]);
    }

    #[test]
    fn test_cluster_by_similarity_identical_embeddings_different_files() {
        let item1 = TestClusterable {
            embedding: vec![1.0, 0.0, 0.0],
            path: PathBuf::from("/a.rs"),
            size: 50,
        };
        let item2 = TestClusterable {
            embedding: vec![1.0, 0.0, 0.0],
            path: PathBuf::from("/b.rs"),
            size: 50,
        };
        let items = vec![&item1, &item2];
        let clusters = cluster_by_similarity(&items, 0.9);
        // Should be clustered together (similarity = 1.0)
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].len(), 2);
    }

    #[test]
    fn test_cluster_by_similarity_same_file_not_clustered() {
        let item1 = TestClusterable {
            embedding: vec![1.0, 0.0, 0.0],
            path: PathBuf::from("/a.rs"),
            size: 50,
        };
        let item2 = TestClusterable {
            embedding: vec![1.0, 0.0, 0.0],
            path: PathBuf::from("/a.rs"), // Same file
            size: 50,
        };
        let items = vec![&item1, &item2];
        let clusters = cluster_by_similarity(&items, 0.9);
        // Should NOT be clustered (same file)
        assert_eq!(clusters.len(), 2);
    }

    #[test]
    fn test_cluster_by_similarity_orthogonal_not_clustered() {
        let item1 = TestClusterable {
            embedding: vec![1.0, 0.0, 0.0],
            path: PathBuf::from("/a.rs"),
            size: 50,
        };
        let item2 = TestClusterable {
            embedding: vec![0.0, 1.0, 0.0], // Orthogonal
            path: PathBuf::from("/b.rs"),
            size: 50,
        };
        let items = vec![&item1, &item2];
        let clusters = cluster_by_similarity(&items, 0.9);
        // Should NOT be clustered (similarity = 0)
        assert_eq!(clusters.len(), 2);
    }

    #[test]
    fn test_compute_cluster_similarity_single_item() {
        let item = TestClusterable {
            embedding: vec![1.0, 0.0, 0.0],
            path: PathBuf::from("/a.rs"),
            size: 50,
        };
        let items = vec![&item];
        let sim = compute_cluster_similarity(&items);
        assert_eq!(sim, 1.0); // Single item returns 1.0
    }

    #[test]
    fn test_compute_cluster_similarity_identical() {
        let item1 = TestClusterable {
            embedding: vec![1.0, 0.0, 0.0],
            path: PathBuf::from("/a.rs"),
            size: 50,
        };
        let item2 = TestClusterable {
            embedding: vec![1.0, 0.0, 0.0],
            path: PathBuf::from("/b.rs"),
            size: 50,
        };
        let items = vec![&item1, &item2];
        let sim = compute_cluster_similarity(&items);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_cluster_similarity_orthogonal() {
        let item1 = TestClusterable {
            embedding: vec![1.0, 0.0, 0.0],
            path: PathBuf::from("/a.rs"),
            size: 50,
        };
        let item2 = TestClusterable {
            embedding: vec![0.0, 1.0, 0.0],
            path: PathBuf::from("/b.rs"),
            size: 50,
        };
        let items = vec![&item1, &item2];
        let sim = compute_cluster_similarity(&items);
        assert!(sim.abs() < 0.001); // Orthogonal = 0 similarity
    }

    #[test]
    fn test_clusterable_trait_embedded_chunk_record() {
        let record = EmbeddedChunkRecord {
            path: PathBuf::from("/test.rs"),
            start_byte: 0,
            end_byte: 100,
            embedding: vec![1.0, 2.0, 3.0],
            symbol_path: "test::func".to_string(),
        };
        assert_eq!(record.embedding(), Some([1.0, 2.0, 3.0].as_slice()));
        assert_eq!(record.file_path(), Some(Path::new("/test.rs")));
    }

    // =========================================================================
    // Tests for query helper functions
    // =========================================================================

    #[test]
    fn test_check_index_ready_empty_workspace() {
        let status = crate::index::IndexStatus::new(PathBuf::from("/test"));
        // Empty workspace (0 files) should be considered ready
        assert!(check_index_ready(&status).is_ok());
    }

    #[test]
    fn test_check_index_ready_incomplete() {
        let mut status = crate::index::IndexStatus::new(PathBuf::from("/test"));
        status.files_total = 10;
        status.files_parsed = 5;
        // Incomplete workspace should return error
        assert!(check_index_ready(&status).is_err());
    }

    #[test]
    fn test_check_index_ready_complete() {
        let mut status = crate::index::IndexStatus::new(PathBuf::from("/test"));
        status.phase = crate::index::IndexPhase::Complete;
        status.files_total = 10;
        status.files_parsed = 10;
        // Complete workspace should be ready
        assert!(check_index_ready(&status).is_ok());
    }

    #[test]
    fn test_sort_by_similarity_desc() {
        let mut results = vec![
            SimilarChunkResult {
                chunk: ChunkResult {
                    file: PathBuf::from("/a.rs"),
                    text: String::new(),
                    start_byte: 0,
                    end_byte: 10,
                    start_line: 0,
                    end_line: 0,
                },
                similarity: 0.5,
            },
            SimilarChunkResult {
                chunk: ChunkResult {
                    file: PathBuf::from("/b.rs"),
                    text: String::new(),
                    start_byte: 0,
                    end_byte: 10,
                    start_line: 0,
                    end_line: 0,
                },
                similarity: 0.9,
            },
            SimilarChunkResult {
                chunk: ChunkResult {
                    file: PathBuf::from("/c.rs"),
                    text: String::new(),
                    start_byte: 0,
                    end_byte: 10,
                    start_line: 0,
                    end_line: 0,
                },
                similarity: 0.7,
            },
        ];
        sort_by_similarity_desc(&mut results);
        assert!((results[0].similarity - 0.9).abs() < 0.001);
        assert!((results[1].similarity - 0.7).abs() < 0.001);
        assert!((results[2].similarity - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_db_to_query_error() {
        let error = db_to_query_error("test error message");
        assert!(error.to_string().contains("test error message"));
    }

    #[test]
    fn test_filter_records_for_file() {
        let records = vec![
            EmbeddedChunkRecord {
                path: PathBuf::from("/a.rs"),
                start_byte: 0,
                end_byte: 10,
                embedding: vec![1.0],
                symbol_path: String::new(),
            },
            EmbeddedChunkRecord {
                path: PathBuf::from("/b.rs"),
                start_byte: 0,
                end_byte: 10,
                embedding: vec![1.0],
                symbol_path: String::new(),
            },
            EmbeddedChunkRecord {
                path: PathBuf::from("/a.rs"),
                start_byte: 10,
                end_byte: 20,
                embedding: vec![1.0],
                symbol_path: String::new(),
            },
        ];
        let filtered = filter_records_for_file(&records, Path::new("/a.rs"));
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|r| r.path == Path::new("/a.rs")));
    }

    #[test]
    fn test_filter_records_excluding_file() {
        let records = vec![
            EmbeddedChunkRecord {
                path: PathBuf::from("/a.rs"),
                start_byte: 0,
                end_byte: 10,
                embedding: vec![1.0],
                symbol_path: String::new(),
            },
            EmbeddedChunkRecord {
                path: PathBuf::from("/b.rs"),
                start_byte: 0,
                end_byte: 10,
                embedding: vec![1.0],
                symbol_path: String::new(),
            },
            EmbeddedChunkRecord {
                path: PathBuf::from("/c.rs"),
                start_byte: 0,
                end_byte: 10,
                embedding: vec![1.0],
                symbol_path: String::new(),
            },
        ];
        let filtered = filter_records_excluding_file(&records, Path::new("/a.rs"));
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|r| r.path != Path::new("/a.rs")));
    }

    // =========================================================================
    // Tests for Workspace and other functionality
    // =========================================================================

    #[test]
    fn test_database_path() {
        let root = PathBuf::from("/test/workspace");
        let db_path = database_path(&root);
        assert!(db_path.to_string_lossy().contains(".treesitter-index.db"));
    }

    #[tokio::test]
    async fn test_workspace_open_becomes_leader() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let workspace = Workspace::open(dir.path()).await.unwrap();

        // Wait for background indexing to complete
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        // With background indexing, open() returns Reader mode
        assert!(!workspace.is_leader());

        // Verify indexing is complete by checking status
        let status = workspace.status().await.unwrap();
        assert!(
            status.is_ready,
            "Workspace should be ready after background indexing"
        );

        assert_eq!(workspace.workspace_root(), dir.path());
    }

    #[tokio::test]
    async fn test_workspace_open_empty_directory() {
        let dir = TempDir::new().unwrap();

        let workspace = Workspace::open(dir.path()).await.unwrap();

        // Wait for background indexing to complete
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        // With background indexing, open() returns Reader mode
        assert!(!workspace.is_leader());
        let files = workspace.list_files().await.unwrap();
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn test_workspace_status() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let workspace = Workspace::open(dir.path()).await.unwrap();

        // Wait for background indexing to complete
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        let status = workspace.status().await.unwrap();

        assert!(status.is_ready);
        assert!(status.files_indexed >= 1);
        assert_eq!(status.root_path, dir.path());
    }

    #[tokio::test]
    async fn test_workspace_list_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let workspace = Workspace::open(dir.path()).await.unwrap();

        // Wait for background indexing to complete
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        let files = workspace.list_files().await.unwrap();

        assert!(!files.is_empty());
        assert!(files
            .iter()
            .any(|f| f.to_string_lossy().contains("test.rs")));
    }

    #[tokio::test]
    async fn test_workspace_tree_sitter_query_fails_in_reader_mode() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() { let x = 1; }").unwrap();

        let workspace = Workspace::open(dir.path()).await.unwrap();

        // Wait for background indexing to complete
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        // Tree-sitter queries are not available in Reader mode (no parsed AST)
        let results = workspace
            .tree_sitter_query("(identifier) @name".to_string(), None, None)
            .await;

        assert!(
            results.is_err(),
            "Tree-sitter queries should not be available in Reader mode"
        );
    }

    #[tokio::test]
    async fn test_workspace_invalidate_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {}").unwrap();

        let workspace = Workspace::open(dir.path()).await.unwrap();

        // Wait for background indexing to complete
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        // Modify the file
        std::fs::write(&file_path, "fn main() { println!(\"updated\"); }").unwrap();

        // invalidate_file is not supported with background indexing (Reader mode)
        let result = workspace.invalidate_file(file_path).await;
        assert!(
            result.is_err(),
            "invalidate_file should fail in Reader mode"
        );
    }

    #[test]
    fn test_semantic_chunk_to_chunk_result() {
        let chunk = SemanticChunk::from_text("test code");
        let result = chunk.to_chunk_result();

        assert_eq!(result.text, "test code");
        assert_eq!(result.start_byte, 0);
        assert_eq!(result.end_byte, 9);
    }

    #[tokio::test]
    async fn test_open_returns_workspace() {
        let dir = TempDir::new().unwrap();
        let result = Workspace::open(dir.path()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_open_with_config_custom_election() {
        let dir = TempDir::new().unwrap();
        let election_config = ElectionConfig::default();
        let result = Workspace::open_with_config(dir.path(), election_config, None).await;
        assert!(result.is_ok());

        // Wait for background indexing to complete
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        // With background indexing, open() returns Reader mode
        assert!(!result.unwrap().is_leader());
    }

    #[tokio::test]
    async fn test_is_leader_returns_false_for_reader() {
        let dir = TempDir::new().unwrap();
        let workspace = Workspace::open(dir.path()).await.unwrap();

        // Wait for background indexing to complete
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        // With background indexing, open() returns Reader mode (leader is background task)
        assert!(!workspace.is_leader());
    }

    #[tokio::test]
    async fn test_workspace_root_returns_correct_path() {
        let dir = TempDir::new().unwrap();
        let workspace = Workspace::open(dir.path()).await.unwrap();
        assert_eq!(workspace.workspace_root(), dir.path());
    }

    #[tokio::test]
    async fn test_socket_path_returns_path() {
        let dir = TempDir::new().unwrap();
        let workspace = Workspace::open(dir.path()).await.unwrap();
        let socket_path = workspace.socket_path();
        // Socket path should be a valid path
        assert!(!socket_path.as_os_str().is_empty());
    }

    #[tokio::test]
    async fn test_database_path_returns_correct_location() {
        let dir = TempDir::new().unwrap();
        let workspace = Workspace::open(dir.path()).await.unwrap();
        let db_path = workspace.database_path();
        assert!(db_path.to_string_lossy().contains(".treesitter-index.db"));
        assert!(db_path.starts_with(dir.path()));
    }

    #[tokio::test]
    async fn test_find_all_duplicates_empty_workspace() {
        let dir = TempDir::new().unwrap();
        let workspace = Workspace::open(dir.path()).await.unwrap();
        let result = workspace
            .find_all_duplicates(TEST_HIGH_SIMILARITY_THRESHOLD, TEST_MIN_CHUNK_BYTES)
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_find_all_duplicates_with_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn foo() { println!(\"hello\"); }").unwrap();
        std::fs::write(dir.path().join("b.rs"), "fn bar() { println!(\"world\"); }").unwrap();

        let workspace = Workspace::open(dir.path()).await.unwrap();
        let result = workspace
            .find_all_duplicates(TEST_HIGH_SIMILARITY_THRESHOLD, TEST_SMALL_MIN_CHUNK_BYTES)
            .await;
        assert!(result.is_ok());
        // May or may not find duplicates depending on embeddings
    }

    #[tokio::test]
    async fn test_find_duplicates_in_file_not_found() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let workspace = Workspace::open(dir.path()).await.unwrap();
        let result = workspace
            .find_duplicates_in_file(
                PathBuf::from("/nonexistent.rs"),
                TEST_HIGH_SIMILARITY_THRESHOLD,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_find_duplicates_in_file_with_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() { let x = 1; }").unwrap();

        let workspace = Workspace::open(dir.path()).await.unwrap();

        // Wait for background indexing to complete
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        let result = workspace
            .find_duplicates_in_file(file_path, TEST_MEDIUM_SIMILARITY_THRESHOLD)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_builder_with_progress_callback() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let progress_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let progress_called_clone = progress_called.clone();

        let _workspace = Workspace::new(dir.path())
            .with_progress(move |_status| {
                progress_called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            })
            .open()
            .await
            .unwrap();

        // Wait for background indexing to run
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        // Progress callback should have been called by background task
        assert!(progress_called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_builder_without_auto_build() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let workspace = Workspace::new(dir.path()).open().await.unwrap();

        // With background indexing, open() returns Reader mode immediately
        assert!(!workspace.is_leader());

        // build() should fail on Reader mode
        let result = workspace.build().await;
        assert!(result.is_err());
    }

    // =========================================================================
    // Incremental indexing tests
    // =========================================================================

    #[tokio::test]
    async fn test_incremental_indexing_skips_unchanged_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        // First indexing - background task will parse the file
        let workspace = Workspace::new(dir.path()).open().await.unwrap();
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        let status1 = workspace.status().await.unwrap();
        assert_eq!(status1.files_indexed, 1);

        // Drop workspace to release lock
        drop(workspace);

        // Second indexing - file is unchanged, should skip
        let parse_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let parse_count_clone = parse_count.clone();

        let _workspace2 = Workspace::new(dir.path())
            .with_progress(move |status| {
                // Track when files are being parsed (not skipped)
                if status.files_parsed > 0 {
                    parse_count_clone
                        .store(status.files_parsed, std::sync::atomic::Ordering::SeqCst);
                }
            })
            .open()
            .await
            .unwrap();
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        // File was unchanged, so should not have been re-parsed
        let parsed = parse_count.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(parsed, 0, "Unchanged file should be skipped, not re-parsed");
    }

    #[tokio::test]
    async fn test_incremental_indexing_reparses_changed_files() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {}").unwrap();

        // First indexing
        let workspace = Workspace::new(dir.path()).open().await.unwrap();
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;
        drop(workspace);

        // Modify the file
        std::fs::write(&file_path, "fn main() { println!(\"changed\"); }").unwrap();

        // Second indexing - file changed, should re-parse
        let parse_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let parse_count_clone = parse_count.clone();

        let _workspace2 = Workspace::new(dir.path())
            .with_progress(move |status| {
                parse_count_clone.store(status.files_parsed, std::sync::atomic::Ordering::SeqCst);
            })
            .open()
            .await
            .unwrap();
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        // File was changed, so should have been re-parsed
        let parsed = parse_count.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(parsed, 1, "Changed file should be re-parsed");
    }

    #[tokio::test]
    async fn test_incremental_indexing_mixed_changed_unchanged() {
        let dir = TempDir::new().unwrap();
        let file1 = dir.path().join("unchanged.rs");
        let file2 = dir.path().join("changed.rs");
        std::fs::write(&file1, "fn unchanged() {}").unwrap();
        std::fs::write(&file2, "fn changed() {}").unwrap();

        // First indexing
        let workspace = Workspace::new(dir.path()).open().await.unwrap();
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;
        let status1 = workspace.status().await.unwrap();
        assert_eq!(status1.files_indexed, 2);
        drop(workspace);

        // Modify only one file
        std::fs::write(&file2, "fn changed() { println!(\"modified\"); }").unwrap();

        // Second indexing
        let max_parsed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let max_parsed_clone = max_parsed.clone();

        let _workspace2 = Workspace::new(dir.path())
            .with_progress(move |status| {
                let current = max_parsed_clone.load(std::sync::atomic::Ordering::SeqCst);
                if status.files_parsed > current {
                    max_parsed_clone
                        .store(status.files_parsed, std::sync::atomic::Ordering::SeqCst);
                }
            })
            .open()
            .await
            .unwrap();
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        // Only the changed file should be re-parsed
        let parsed = max_parsed.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(parsed, 1, "Only the changed file should be re-parsed");
    }

    #[tokio::test]
    async fn test_incremental_indexing_new_file_added() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("existing.rs"), "fn existing() {}").unwrap();

        // First indexing
        let workspace = Workspace::new(dir.path()).open().await.unwrap();
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;
        drop(workspace);

        // Add a new file
        std::fs::write(dir.path().join("new.rs"), "fn new_func() {}").unwrap();

        // Second indexing
        let workspace2 = Workspace::new(dir.path()).open().await.unwrap();
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        // Both files should now be indexed
        let status = workspace2.status().await.unwrap();
        assert_eq!(status.files_indexed, 2, "Both files should be indexed");
    }

    #[tokio::test]
    async fn test_check_file_unchanged_returns_none_for_new_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {}").unwrap();

        // Open database without indexing
        let db_path = database_path(dir.path());
        let db = Arc::new(IndexDatabase::open_readwrite(&db_path).unwrap());

        // File not in database - should return None
        let result = Workspace::check_file_unchanged(&file_path, &db);
        assert!(
            result.is_none(),
            "New file should not be marked as unchanged"
        );
    }

    #[tokio::test]
    async fn test_check_file_unchanged_returns_path_for_unchanged() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {}").unwrap();

        // Open workspace (spawns background indexer)
        let _workspace = Workspace::new(dir.path()).open().await.unwrap();

        // Wait for background indexing to complete (using test constant defined above)
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        // Get the database
        let db_path = database_path(dir.path());
        let db = Arc::new(IndexDatabase::open_readwrite(&db_path).unwrap());

        // File in database with same hash - should return Some
        let result = Workspace::check_file_unchanged(&file_path, &db);
        assert!(
            result.is_some(),
            "Unchanged file should return canonical path"
        );
    }

    #[test]
    fn test_embedded_chunk_record_to_chunk_result_with_valid_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        std::fs::write(&file_path, content).unwrap();

        // Create a record for the println line (bytes 12-35)
        let record = EmbeddedChunkRecord {
            path: file_path.clone(),
            start_byte: 12,
            end_byte: 35,
            embedding: vec![1.0, 2.0, 3.0],
            symbol_path: "test.rs::main".to_string(),
        };

        let result = record.to_chunk_result();
        assert_eq!(result.file, file_path);
        assert_eq!(result.text, "    println!(\"hello\");\n");
        assert_eq!(result.start_byte, 12);
        assert_eq!(result.end_byte, 35);
        assert_eq!(result.start_line, 1); // Second line (0-indexed)
        assert_eq!(result.end_line, 2); // Ends on third line
    }

    #[test]
    fn test_embedded_chunk_record_to_chunk_result_missing_file() {
        let record = EmbeddedChunkRecord {
            path: PathBuf::from("/nonexistent/file.rs"),
            start_byte: 0,
            end_byte: 10,
            embedding: vec![1.0],
            symbol_path: "missing".to_string(),
        };

        let result = record.to_chunk_result();
        assert_eq!(result.text, ""); // Should return empty string
        assert_eq!(result.start_line, 0);
        assert_eq!(result.end_line, 0);
    }

    #[test]
    fn test_embedded_chunk_record_to_chunk_result_invalid_byte_range() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "short").unwrap();

        // Byte range beyond file length
        let record = EmbeddedChunkRecord {
            path: file_path,
            start_byte: 100,
            end_byte: 200,
            embedding: vec![1.0],
            symbol_path: "test".to_string(),
        };

        let result = record.to_chunk_result();
        assert_eq!(result.text, ""); // Should handle gracefully
    }

    #[test]
    fn test_embedded_chunk_record_to_chunk_result_no_newlines() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        let content = "single line without newline";
        std::fs::write(&file_path, content).unwrap();

        let record = EmbeddedChunkRecord {
            path: file_path,
            start_byte: 0,
            end_byte: content.len(),
            embedding: vec![1.0],
            symbol_path: "test".to_string(),
        };

        let result = record.to_chunk_result();
        assert_eq!(result.text, content);
        assert_eq!(result.start_line, 0);
        assert_eq!(result.end_line, 0); // No newlines
    }

    #[test]
    fn test_embedded_chunk_record_to_chunk_result_at_file_boundary() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        let content = "line1\nline2\nline3\n";
        std::fs::write(&file_path, content).unwrap();

        // Test chunk at start of file
        let record = EmbeddedChunkRecord {
            path: file_path.clone(),
            start_byte: 0,
            end_byte: 6,
            embedding: vec![1.0],
            symbol_path: "test".to_string(),
        };

        let result = record.to_chunk_result();
        assert_eq!(result.text, "line1\n");
        assert_eq!(result.start_line, 0);
        assert_eq!(result.end_line, 1);

        // Test chunk at end of file
        let record = EmbeddedChunkRecord {
            path: file_path,
            start_byte: 12,
            end_byte: content.len(),
            embedding: vec![1.0],
            symbol_path: "test".to_string(),
        };

        let result = record.to_chunk_result();
        assert_eq!(result.text, "line3\n");
        assert_eq!(result.start_line, 2);
        assert_eq!(result.end_line, 3);
    }

    #[tokio::test]
    async fn test_wait_for_database_ready_succeeds() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");

        // Create database with schema
        let _db = IndexDatabase::open_readwrite(&db_path).unwrap();

        // Should return immediately since database exists
        let result =
            Workspace::wait_for_database_ready(&db_path, TEST_DB_READY_TIMEOUT_SUCCESS).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_wait_for_database_ready_timeout() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("nonexistent.db");

        // Should timeout since database doesn't exist
        let result = Workspace::wait_for_database_ready(&db_path, TEST_DB_READY_TIMEOUT_FAIL).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wait_for_database_ready_retries() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");

        // Spawn a task that creates the database after a delay
        let db_path_clone = db_path.clone();
        tokio::spawn(async move {
            tokio::time::sleep(TEST_DB_CREATE_DELAY).await;
            let _db = IndexDatabase::open_readwrite(&db_path_clone).unwrap();
        });

        // Should succeed after retrying
        let result =
            Workspace::wait_for_database_ready(&db_path, TEST_DB_READY_TIMEOUT_RETRY).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_open_spawns_background_indexer() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        // First open should spawn background indexer
        let workspace = Workspace::open(dir.path()).await.unwrap();

        // Should return as Reader mode immediately
        assert!(!workspace.is_leader());

        // Database should exist (created synchronously)
        assert!(workspace.database_path().exists());

        // Give background task time to index
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT).await;

        // Check that file was indexed
        let status = workspace.status().await.unwrap();
        assert!(
            status.files_indexed > 0,
            "Background indexer should have indexed files"
        );
    }

    #[tokio::test]
    async fn test_open_follower_waits_for_database() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        // First open starts background indexing
        let _first = Workspace::open(dir.path()).await.unwrap();

        // Second open should wait for database to be ready, then open as follower
        let second = Workspace::open(dir.path()).await.unwrap();
        assert!(!second.is_leader());
        assert!(second.database_path().exists());
    }

    #[tokio::test]
    async fn test_background_indexer_releases_lock() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        // Open workspace (spawns background indexer)
        let _workspace = Workspace::open(dir.path()).await.unwrap();

        // Wait for background indexing to complete
        tokio::time::sleep(TEST_BACKGROUND_INDEX_WAIT_LONG).await;

        // Should be able to become leader again (lock released)
        let election = LeaderElection::new(dir.path());
        let result = election.try_become_leader();
        assert!(
            result.is_ok(),
            "Lock should be released after background indexing completes"
        );
    }
}
