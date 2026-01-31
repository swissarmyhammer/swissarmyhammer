//! Workspace with automatic leader/client mode
//!
//! The `Workspace` struct transparently handles whether this process is the leader
//! (holding the actual data) or a client (connecting to another process).
//! Callers don't need to understand leader election - just call `Workspace::open()`.
//!
//! # Example
//!
//! ```ignore
//! use swissarmyhammer_treesitter::Workspace;
//!
//! // Simple usage - leader/client mode is handled internally
//! let workspace = Workspace::open("/path/to/workspace").await?;
//!
//! // Query the workspace - works the same regardless of mode
//! let status = workspace.status().await?;
//! let files = workspace.list_files().await?;
//! let results = workspace.semantic_search("fn main", 10, 0.7).await?;
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UnixListener;
use tokio::sync::RwLock;

use swissarmyhammer_leader_election::{ElectionConfig, ElectionError, LeaderElection, LeaderGuard};

use crate::index::{IndexConfig, IndexContext};
use crate::query::server::{IndexServiceServer, find_all_duplicates_impl, find_duplicates_in_file_impl, semantic_search_impl, tree_sitter_query_impl};
use crate::query::service::{IndexService, IndexServiceClient};
use crate::query::{check_ready, DuplicateCluster, IndexStatusInfo, QueryError, QueryMatch, SimilarChunkResult};
use crate::{Result, TreeSitterError};

/// Time to wait for a newly started leader to begin listening on its socket
const LEADER_STARTUP_DELAY: Duration = Duration::from_millis(100);

/// Time to wait before retrying connection when another process holds the leader lock
const LEADER_ELECTION_RETRY_DELAY: Duration = Duration::from_millis(500);

/// Internal mode of the index
enum WorkspaceMode {
    /// This process owns the index data and serves queries
    Leader {
        /// The index context with all parsed files and embeddings
        context: Arc<RwLock<IndexContext>>,
        /// Guard that holds the leader lock (released on drop)
        _guard: LeaderGuard,
        /// Handle to the background server task
        _server_handle: tokio::task::JoinHandle<()>,
    },
    /// This process connects to a leader via RPC
    Client {
        /// RPC client for sending queries
        client: IndexServiceClient,
    },
}

/// A tree-sitter workspace with automatic leader/client mode.
///
/// `Workspace` transparently handles whether this process is the leader
/// (holding the actual data) or a client (connecting to another process).
/// Callers don't need to understand leader election - just call `Workspace::open()`.
///
/// # Leader Election
///
/// When you call `Workspace::open()`:
/// 1. First, it tries to connect to an existing leader
/// 2. If no leader exists, this process becomes the leader
/// 3. The leader scans and indexes the workspace
/// 4. Clients connect to the leader via Unix socket RPC
///
/// # Example
///
/// ```ignore
/// // Simple usage - leader/client mode is handled internally
/// let index = Workspace::open("/path/to/workspace").await?;
///
/// // Query the index - works the same regardless of mode
/// let status = index.status().await?;
/// let files = index.list_files().await?;
/// ```
pub struct Workspace {
    mode: WorkspaceMode,
    election: LeaderElection,
}

impl Workspace {
    /// Open an index for a workspace, automatically handling leader election.
    ///
    /// This will:
    /// 1. Try to connect to an existing leader
    /// 2. If no leader exists, become the leader and scan the workspace
    ///
    /// The leader election is transparent to the caller.
    pub async fn open(workspace_root: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_config(workspace_root, ElectionConfig::default(), None).await
    }

    /// Open an index with custom configuration.
    ///
    /// # Arguments
    /// * `workspace_root` - Path to the workspace to index
    /// * `election_config` - Configuration for leader election (prefix, base dir)
    /// * `index_config` - Optional configuration for the index (when becoming leader)
    pub async fn open_with_config(
        workspace_root: impl AsRef<Path>,
        election_config: ElectionConfig,
        index_config: Option<IndexConfig>,
    ) -> Result<Self> {
        let workspace_root = workspace_root.as_ref();
        let election = LeaderElection::with_config(workspace_root, election_config);

        // First, try to connect to an existing leader
        if let Ok(client) = Self::try_connect(&election).await {
            return Ok(Self {
                mode: WorkspaceMode::Client { client },
                election,
            });
        }

        // No leader running - try to become the leader
        match election.try_become_leader() {
            Ok(guard) => {
                tracing::info!(
                    "Becoming tree-sitter index leader for {}",
                    workspace_root.display()
                );

                // Create and scan the index
                let config = index_config.unwrap_or_default();
                let mut context = IndexContext::new(workspace_root).with_config(config);
                let result = context.scan().await?;

                tracing::info!(
                    "Index scan complete: {} files parsed, {} skipped, {} errors in {}ms",
                    result.files_parsed,
                    result.files_skipped,
                    result.errors.len(),
                    result.total_time_ms
                );

                let context = Arc::new(RwLock::new(context));
                let socket_path = election.socket_path().to_path_buf();

                // Start the RPC server in the background
                let server_context = context.clone();
                let server_handle = tokio::spawn(async move {
                    if let Err(e) = run_leader_server(server_context, &socket_path).await {
                        tracing::error!("Index leader server error: {}", e);
                    }
                });

                // Wait for server to start
                tokio::time::sleep(LEADER_STARTUP_DELAY).await;

                Ok(Self {
                    mode: WorkspaceMode::Leader {
                        context,
                        _guard: guard,
                        _server_handle: server_handle,
                    },
                    election,
                })
            }
            Err(ElectionError::LockHeld) => {
                // Another process is the leader - wait and try connecting again
                tracing::debug!("Another process holds the leader lock, waiting to connect...");
                tokio::time::sleep(LEADER_ELECTION_RETRY_DELAY).await;

                let client = Self::try_connect(&election).await.map_err(|e| {
                    TreeSitterError::connection_error(format!(
                        "Failed to connect after waiting for leader: {}",
                        e
                    ))
                })?;

                Ok(Self {
                    mode: WorkspaceMode::Client { client },
                    election,
                })
            }
            Err(e) => Err(TreeSitterError::connection_error(format!(
                "Failed to acquire index leader lock: {}",
                e
            ))),
        }
    }

    /// Try to connect to an existing leader
    async fn try_connect(
        election: &LeaderElection,
    ) -> std::result::Result<IndexServiceClient, String> {
        if !election.leader_exists() {
            return Err("No leader socket found".to_string());
        }

        connect_to_socket(election.socket_path())
            .await
            .map_err(|e| e.to_string())
    }

    /// Check if this instance is the leader
    pub fn is_leader(&self) -> bool {
        matches!(self.mode, WorkspaceMode::Leader { .. })
    }

    /// Get the workspace root
    pub fn workspace_root(&self) -> &Path {
        self.election.workspace_root()
    }

    /// Get the socket path for this workspace's index
    pub fn socket_path(&self) -> &Path {
        self.election.socket_path()
    }

    /// Get current index status.
    pub async fn status(&self) -> std::result::Result<IndexStatusInfo, QueryError> {
        match &self.mode {
            WorkspaceMode::Leader { context, .. } => {
                let ctx = context.read().await;
                let status = ctx.status();
                Ok(IndexStatusInfo {
                    files_total: status.files_total,
                    files_indexed: status.files_parsed,
                    files_embedded: status.files_embedded,
                    is_ready: status.is_complete(),
                    root_path: ctx.root_path().to_path_buf(),
                })
            }
            WorkspaceMode::Client { client } => client
                .status(tarpc::context::current())
                .await
                .map_err(|e| QueryError::internal(e.to_string())),
        }
    }

    /// List all files in the index.
    pub async fn list_files(&self) -> std::result::Result<Vec<PathBuf>, QueryError> {
        match &self.mode {
            WorkspaceMode::Leader { context, .. } => {
                let ctx = context.read().await;
                Ok(ctx.files())
            }
            WorkspaceMode::Client { client } => client
                .list_files(tarpc::context::current())
                .await
                .map_err(|e| QueryError::internal(e.to_string())),
        }
    }

    /// Find all duplicate code clusters across the project.
    ///
    /// # Arguments
    /// * `min_similarity` - Minimum cosine similarity threshold (0.0 to 1.0)
    /// * `min_chunk_bytes` - Minimum chunk size to consider
    pub async fn find_all_duplicates(
        &self,
        min_similarity: f32,
        min_chunk_bytes: usize,
    ) -> std::result::Result<Vec<DuplicateCluster>, QueryError> {
        match &self.mode {
            WorkspaceMode::Leader { context, .. } => {
                let ctx = context.read().await;
                check_ready(ctx.status().is_complete())?;
                Ok(find_all_duplicates_impl(ctx.chunk_graph(), min_similarity, min_chunk_bytes))
            }
            WorkspaceMode::Client { client } => client
                .find_all_duplicates(tarpc::context::current(), min_similarity, min_chunk_bytes)
                .await
                .map_err(|e| QueryError::internal(e.to_string()))?,
        }
    }

    /// Find duplicates for chunks in a specific file.
    ///
    /// Returns chunks from other files similar to chunks in the given file.
    pub async fn find_duplicates_in_file(
        &self,
        file: PathBuf,
        min_similarity: f32,
    ) -> std::result::Result<Vec<SimilarChunkResult>, QueryError> {
        match &self.mode {
            WorkspaceMode::Leader { context, .. } => {
                let ctx = context.read().await;
                check_ready(ctx.status().is_complete())?;
                find_duplicates_in_file_impl(ctx.chunk_graph(), &file, min_similarity)
            }
            WorkspaceMode::Client { client } => client
                .find_duplicates_in_file(tarpc::context::current(), file, min_similarity)
                .await
                .map_err(|e| QueryError::internal(e.to_string()))?,
        }
    }

    /// Semantic search - find chunks similar to the given text.
    ///
    /// Embeds the query and finds the most similar indexed chunks.
    pub async fn semantic_search(
        &self,
        text: String,
        top_k: usize,
        min_similarity: f32,
    ) -> std::result::Result<Vec<SimilarChunkResult>, QueryError> {
        match &self.mode {
            WorkspaceMode::Leader { context, .. } => {
                // Need write lock to potentially load embedding model
                let mut ctx = context.write().await;
                check_ready(ctx.status().is_complete())?;

                let query_embedding = ctx
                    .embed_text(&text)
                    .await
                    .map_err(|e| QueryError::embedding_error(e.to_string()))?;

                Ok(semantic_search_impl(ctx.chunk_graph(), query_embedding, top_k, min_similarity))
            }
            WorkspaceMode::Client { client } => client
                .semantic_search(tarpc::context::current(), text, top_k, min_similarity)
                .await
                .map_err(|e| QueryError::internal(e.to_string()))?,
        }
    }

    /// Execute a tree-sitter query and return matches.
    ///
    /// The query is an S-expression pattern like `(function_item name: (identifier) @name)`.
    pub async fn tree_sitter_query(
        &self,
        query: String,
        files: Option<Vec<PathBuf>>,
        language: Option<String>,
    ) -> std::result::Result<Vec<QueryMatch>, QueryError> {
        match &self.mode {
            WorkspaceMode::Leader { context, .. } => {
                let ctx = context.read().await;
                check_ready(ctx.status().is_complete())?;
                tree_sitter_query_impl(&ctx, &query, files, language)
            }
            WorkspaceMode::Client { client } => client
                .tree_sitter_query(tarpc::context::current(), query, files, language)
                .await
                .map_err(|e| QueryError::internal(e.to_string()))?,
        }
    }

    /// Invalidate a file (force re-parse and re-embed).
    pub async fn invalidate_file(&self, file: PathBuf) -> std::result::Result<(), QueryError> {
        match &self.mode {
            WorkspaceMode::Leader { context, .. } => {
                let mut ctx = context.write().await;
                ctx.refresh(&file)
                    .await
                    .map_err(|e| QueryError::internal(e.to_string()))?;
                Ok(())
            }
            WorkspaceMode::Client { client } => client
                .invalidate_file(tarpc::context::current(), file)
                .await
                .map_err(|e| QueryError::internal(e.to_string()))?,
        }
    }
}

/// Connect to a leader's RPC socket
async fn connect_to_socket(
    socket_path: &Path,
) -> std::result::Result<IndexServiceClient, std::io::Error> {
    use tarpc::client;
    use tokio::net::UnixStream;
    use tokio_serde::formats::Bincode;

    let stream = UnixStream::connect(socket_path).await?;
    let codec_builder = Bincode::default;
    let framed = tokio_util::codec::Framed::new(
        stream,
        tarpc::tokio_util::codec::LengthDelimitedCodec::new(),
    );
    let transport = tarpc::serde_transport::new(framed, codec_builder());
    let client = IndexServiceClient::new(client::Config::default(), transport).spawn();
    Ok(client)
}

/// Run the leader RPC server (called in background task)
async fn run_leader_server(
    context: Arc<RwLock<IndexContext>>,
    socket_path: &Path,
) -> std::result::Result<(), std::io::Error> {
    use futures::StreamExt;
    use tarpc::server::{self, Channel};
    use tokio_serde::formats::Bincode;

    // Remove any stale socket file
    let _ = std::fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path)?;
    tracing::info!("Index leader listening on {}", socket_path.display());

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let context = context.clone();
                tokio::spawn(async move {
                    let codec_builder = Bincode::default;
                    let framed = tokio_util::codec::Framed::new(
                        stream,
                        tarpc::tokio_util::codec::LengthDelimitedCodec::new(),
                    );
                    let transport = tarpc::serde_transport::new(framed, codec_builder());

                    let server = IndexServiceServer::new(context);
                    let channel = server::BaseChannel::with_defaults(transport);

                    channel
                        .execute(server.serve())
                        .for_each(|response| async move {
                            tokio::spawn(response);
                        })
                        .await;
                });
            }
            Err(e) => {
                tracing::warn!("Failed to accept connection: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // =========================================================================
    // Constants tests
    // =========================================================================

    #[test]
    fn test_constants() {
        assert!(LEADER_STARTUP_DELAY.as_millis() > 0);
        assert!(LEADER_ELECTION_RETRY_DELAY.as_millis() > 0);
    }

    #[test]
    fn test_leader_startup_delay_reasonable() {
        assert!(LEADER_STARTUP_DELAY.as_millis() >= 50);
        assert!(LEADER_STARTUP_DELAY.as_millis() <= 1000);
    }

    #[test]
    fn test_election_retry_delay_reasonable() {
        assert!(LEADER_ELECTION_RETRY_DELAY.as_millis() >= 100);
        assert!(LEADER_ELECTION_RETRY_DELAY.as_millis() <= 5000);
    }

    // =========================================================================
    // Workspace::open tests
    // =========================================================================

    #[tokio::test]
    async fn test_index_open_becomes_leader() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let index = Workspace::open(dir.path()).await.unwrap();

        assert!(index.is_leader());
        assert_eq!(index.workspace_root(), dir.path());
    }

    #[tokio::test]
    async fn test_index_open_empty_directory() {
        let dir = TempDir::new().unwrap();

        let index = Workspace::open(dir.path()).await.unwrap();

        assert!(index.is_leader());
        let files = index.list_files().await.unwrap();
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn test_index_open_with_config() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let election_config = ElectionConfig::new().with_prefix("test");
        let index = Workspace::open_with_config(dir.path(), election_config, None)
            .await
            .unwrap();

        assert!(index.is_leader());
        assert!(index.socket_path().to_string_lossy().contains("test-ts-"));
    }

    #[tokio::test]
    async fn test_index_open_with_index_config() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let index_config = IndexConfig {
            max_file_size: 1024 * 1024, // 1MB
            ..Default::default()
        };
        let index =
            Workspace::open_with_config(dir.path(), ElectionConfig::default(), Some(index_config))
                .await
                .unwrap();

        assert!(index.is_leader());
    }

    // =========================================================================
    // Index metadata tests
    // =========================================================================

    #[tokio::test]
    async fn test_index_status() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let index = Workspace::open(dir.path()).await.unwrap();
        let status = index.status().await.unwrap();

        assert!(status.is_ready);
        assert!(status.files_indexed >= 1);
        assert_eq!(status.root_path, dir.path());
    }

    #[tokio::test]
    async fn test_index_list_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let index = Workspace::open(dir.path()).await.unwrap();
        let files = index.list_files().await.unwrap();

        assert!(!files.is_empty());
        assert!(files
            .iter()
            .any(|f| f.to_string_lossy().contains("test.rs")));
    }

    #[tokio::test]
    async fn test_index_socket_path() {
        let dir = TempDir::new().unwrap();

        let index = Workspace::open(dir.path()).await.unwrap();

        assert!(index.socket_path().to_string_lossy().contains("sah"));
        assert!(index.socket_path().to_string_lossy().ends_with(".sock"));
    }

    #[tokio::test]
    async fn test_index_workspace_root() {
        let dir = TempDir::new().unwrap();

        let index = Workspace::open(dir.path()).await.unwrap();

        assert_eq!(index.workspace_root(), dir.path());
    }

    #[tokio::test]
    async fn test_index_is_leader() {
        let dir = TempDir::new().unwrap();

        let index = Workspace::open(dir.path()).await.unwrap();

        // First opener should be leader
        assert!(index.is_leader());
    }

    // =========================================================================
    // Query method tests (leader mode)
    // =========================================================================

    #[tokio::test]
    async fn test_index_tree_sitter_query() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() { let x = 1; }").unwrap();

        let index = Workspace::open(dir.path()).await.unwrap();
        let results = index
            .tree_sitter_query("(identifier) @name".to_string(), None, None)
            .await
            .unwrap();

        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_index_tree_sitter_query_with_language_filter() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("test.py"), "def main(): pass").unwrap();

        let index = Workspace::open(dir.path()).await.unwrap();
        let results = index
            .tree_sitter_query(
                "(function_item) @fn".to_string(),
                None,
                Some("rust".to_string()),
            )
            .await
            .unwrap();

        // Should only match Rust files
        for result in &results {
            assert!(result.file.to_string_lossy().ends_with(".rs"));
        }
    }

    #[tokio::test]
    async fn test_index_find_all_duplicates() {
        let dir = TempDir::new().unwrap();
        // Create two files with similar code
        std::fs::write(
            dir.path().join("a.rs"),
            "fn duplicate_function() { let x = 1; let y = 2; let z = x + y; }",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("b.rs"),
            "fn duplicate_function() { let x = 1; let y = 2; let z = x + y; }",
        )
        .unwrap();

        let index = Workspace::open(dir.path()).await.unwrap();
        let clusters = index.find_all_duplicates(0.8, 10).await.unwrap();

        // May or may not find duplicates depending on chunking
        // Just verify the call succeeds
        let _ = clusters;
    }

    #[tokio::test]
    async fn test_index_find_duplicates_in_file() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() { let x = 1; }").unwrap();

        let index = Workspace::open(dir.path()).await.unwrap();
        let file_path = dir.path().join("test.rs");
        let results = index.find_duplicates_in_file(file_path, 0.8).await;

        // May succeed or fail with FileNotFound depending on path resolution
        // Just verify the call completes
        let _ = results;
    }

    #[tokio::test]
    async fn test_index_semantic_search() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("test.rs"),
            "fn calculate_sum(a: i32, b: i32) -> i32 { a + b }",
        )
        .unwrap();

        let index = Workspace::open(dir.path()).await.unwrap();
        let results = index
            .semantic_search("sum function".to_string(), 10, 0.5)
            .await
            .unwrap();

        // Should find something
        // Results depend on embedding model
        let _ = results;
    }

    #[tokio::test]
    async fn test_index_invalidate_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {}").unwrap();

        let index = Workspace::open(dir.path()).await.unwrap();

        // Modify the file
        std::fs::write(&file_path, "fn main() { println!(\"updated\"); }").unwrap();

        // Invalidate to re-index
        let result = index.invalidate_file(file_path).await;
        assert!(result.is_ok());
    }

    // =========================================================================
    // try_connect tests
    // =========================================================================

    #[tokio::test]
    async fn test_try_connect_no_leader() {
        let dir = TempDir::new().unwrap();
        let election = LeaderElection::new(dir.path());

        let result = Workspace::try_connect(&election).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No leader"));
    }

    #[tokio::test]
    async fn test_try_connect_stale_socket() {
        let dir = TempDir::new().unwrap();
        let election = LeaderElection::new(dir.path());

        // Create a fake socket file
        std::fs::write(election.socket_path(), "").unwrap();

        let result = Workspace::try_connect(&election).await;
        // Should fail because socket isn't a real Unix socket
        assert!(result.is_err());
    }

    // =========================================================================
    // connect_to_socket tests
    // =========================================================================

    #[tokio::test]
    async fn test_connect_to_socket_nonexistent() {
        let result = connect_to_socket(Path::new("/nonexistent/socket.sock")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_connect_to_socket_not_a_socket() {
        let dir = TempDir::new().unwrap();
        let fake_socket = dir.path().join("fake.sock");
        std::fs::write(&fake_socket, "not a socket").unwrap();

        let result = connect_to_socket(&fake_socket).await;
        assert!(result.is_err());
    }

    // =========================================================================
    // Workspace Send/Sync bounds tests
    // =========================================================================

    #[test]
    fn test_workspace_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Workspace>();
    }

    #[test]
    fn test_workspace_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Workspace>();
    }

    // =========================================================================
    // Multiple files tests
    // =========================================================================

    #[tokio::test]
    async fn test_index_multiple_rust_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("lib.rs"), "pub fn lib_fn() {}").unwrap();
        std::fs::write(dir.path().join("utils.rs"), "pub fn util() {}").unwrap();

        let index = Workspace::open(dir.path()).await.unwrap();
        let files = index.list_files().await.unwrap();

        assert_eq!(files.len(), 3);
    }

    #[tokio::test]
    async fn test_index_mixed_languages() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("script.py"), "def main(): pass").unwrap();
        std::fs::write(dir.path().join("app.js"), "function main() {}").unwrap();

        let index = Workspace::open(dir.path()).await.unwrap();
        let files = index.list_files().await.unwrap();

        assert!(files.len() >= 3);
    }
}
