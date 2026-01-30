//! Client for connecting to the tree-sitter index leader
//!
//! The client connects to the leader process via Unix socket and sends queries.
//! If no leader exists, it can attempt to become the leader itself.
//!
//! # Testing
//!
//! The async RPC methods require a running leader for full testing.
//! Unit tests verify error handling and the public API compiles correctly.
//! Integration tests in `tests/` cover the full client-leader interaction.

use std::path::{Path, PathBuf};

use tarpc::client;
use tokio::net::UnixStream;
use tokio_serde::formats::Bincode;

use crate::query::election::LeaderElection;
use crate::query::service::IndexServiceClient;
use crate::query::types::{
    DuplicateCluster, IndexStatusInfo, QueryError, QueryMatch, SimilarChunkResult,
};

/// Client for querying the tree-sitter index.
///
/// Connects to the leader process and delegates queries via tarpc RPC.
/// Use [`IndexClient::connect`] to connect to an existing leader.
pub struct IndexClient {
    inner: IndexServiceClient,
    election: LeaderElection,
}

impl IndexClient {
    /// Connect to an existing leader.
    ///
    /// Returns `ClientError::NoLeader` if no leader is running.
    /// Returns `ClientError::ConnectionFailed` if the socket exists but connection fails.
    pub async fn connect(workspace_root: impl AsRef<Path>) -> Result<Self, ClientError> {
        let election = LeaderElection::new(workspace_root);

        if !election.leader_exists() {
            return Err(ClientError::NoLeader);
        }

        let inner = connect_to_socket(election.socket_path()).await?;

        Ok(Self { inner, election })
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
    ) -> Result<Vec<DuplicateCluster>, QueryError> {
        self.inner
            .find_all_duplicates(tarpc::context::current(), min_similarity, min_chunk_bytes)
            .await
            .map_err(|e| QueryError::internal(e.to_string()))?
    }

    /// Find duplicates for chunks in a specific file.
    ///
    /// Returns chunks from other files similar to chunks in the given file.
    pub async fn find_duplicates_in_file(
        &self,
        file: PathBuf,
        min_similarity: f32,
    ) -> Result<Vec<SimilarChunkResult>, QueryError> {
        self.inner
            .find_duplicates_in_file(tarpc::context::current(), file, min_similarity)
            .await
            .map_err(|e| QueryError::internal(e.to_string()))?
    }

    /// Semantic search - find chunks similar to the given text.
    ///
    /// Embeds the query and finds the most similar indexed chunks.
    pub async fn semantic_search(
        &self,
        text: String,
        top_k: usize,
        min_similarity: f32,
    ) -> Result<Vec<SimilarChunkResult>, QueryError> {
        self.inner
            .semantic_search(tarpc::context::current(), text, top_k, min_similarity)
            .await
            .map_err(|e| QueryError::internal(e.to_string()))?
    }

    /// Execute a tree-sitter query and return matches.
    ///
    /// The query is an S-expression pattern like `(function_item name: (identifier) @name)`.
    pub async fn tree_sitter_query(
        &self,
        query: String,
        files: Option<Vec<PathBuf>>,
        language: Option<String>,
    ) -> Result<Vec<QueryMatch>, QueryError> {
        self.inner
            .tree_sitter_query(tarpc::context::current(), query, files, language)
            .await
            .map_err(|e| QueryError::internal(e.to_string()))?
    }

    /// List all files in the index.
    pub async fn list_files(&self) -> Result<Vec<PathBuf>, ClientError> {
        self.inner
            .list_files(tarpc::context::current())
            .await
            .map_err(|e| ClientError::Rpc(e.to_string()))
    }

    /// Get current index status.
    pub async fn status(&self) -> Result<IndexStatusInfo, ClientError> {
        self.inner
            .status(tarpc::context::current())
            .await
            .map_err(|e| ClientError::Rpc(e.to_string()))
    }

    /// Invalidate a file (force re-parse and re-embed).
    pub async fn invalidate_file(&self, file: PathBuf) -> Result<(), QueryError> {
        self.inner
            .invalidate_file(tarpc::context::current(), file)
            .await
            .map_err(|e| QueryError::internal(e.to_string()))?
    }

    /// Get the socket path for the current workspace.
    pub fn socket_path(&self) -> &Path {
        self.election.socket_path()
    }

    /// Get the workspace root path.
    pub fn workspace_root(&self) -> &Path {
        self.election.workspace_root()
    }
}

/// Errors that can occur when using the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    /// No leader is running.
    NoLeader,
    /// Failed to connect to the leader.
    ConnectionFailed(String),
    /// RPC call failed.
    Rpc(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoLeader => write!(f, "No leader is running"),
            Self::ConnectionFailed(msg) => write!(f, "Failed to connect: {}", msg),
            Self::Rpc(msg) => write!(f, "RPC error: {}", msg),
        }
    }
}

impl std::error::Error for ClientError {}

async fn connect_to_socket(socket_path: &Path) -> Result<IndexServiceClient, ClientError> {
    let stream = UnixStream::connect(socket_path)
        .await
        .map_err(|e| ClientError::ConnectionFailed(e.to_string()))?;

    let codec_builder = Bincode::default;
    let framed = tokio_util::codec::Framed::new(
        stream,
        tarpc::tokio_util::codec::LengthDelimitedCodec::new(),
    );
    let transport = tarpc::serde_transport::new(framed, codec_builder());
    let client = IndexServiceClient::new(client::Config::default(), transport).spawn();

    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // =========================================================================
    // ClientError tests
    // =========================================================================

    #[test]
    fn test_client_error_no_leader() {
        let err = ClientError::NoLeader;
        assert!(err.to_string().contains("No leader"));
        assert_eq!(err, ClientError::NoLeader);
    }

    #[test]
    fn test_client_error_connection_failed() {
        let err = ClientError::ConnectionFailed("timeout".to_string());
        assert!(err.to_string().contains("timeout"));
        assert!(matches!(err, ClientError::ConnectionFailed(_)));
    }

    #[test]
    fn test_client_error_rpc() {
        let err = ClientError::Rpc("internal".to_string());
        assert!(err.to_string().contains("internal"));
        assert!(matches!(err, ClientError::Rpc(_)));
    }

    #[test]
    fn test_client_error_traits() {
        // Test Error trait
        let err: &dyn std::error::Error = &ClientError::NoLeader;
        assert!(!err.to_string().is_empty());

        // Test Clone
        let err = ClientError::ConnectionFailed("x".to_string());
        assert_eq!(err.clone(), err);

        // Test PartialEq
        assert_eq!(ClientError::NoLeader, ClientError::NoLeader);
        assert_ne!(ClientError::NoLeader, ClientError::Rpc("x".to_string()));

        // Test Debug
        assert!(!format!("{:?}", ClientError::NoLeader).is_empty());
    }

    // =========================================================================
    // IndexClient::connect tests
    // =========================================================================

    #[tokio::test]
    async fn test_connect_no_leader() {
        let dir = TempDir::new().unwrap();
        let result = IndexClient::connect(dir.path()).await;
        assert!(matches!(result, Err(ClientError::NoLeader)));
    }

    #[tokio::test]
    async fn test_connect_stale_socket() {
        let dir = TempDir::new().unwrap();
        let election = LeaderElection::new(dir.path());
        std::fs::write(election.socket_path(), "").unwrap();

        let result = IndexClient::connect(dir.path()).await;
        assert!(matches!(result, Err(ClientError::ConnectionFailed(_))));
    }

    // =========================================================================
    // Type tests - verify Send/Sync bounds
    // =========================================================================

    #[test]
    fn test_index_client_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<IndexClient>();
        assert_sync::<IndexClient>();
    }

    #[test]
    fn test_client_error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ClientError>();
        assert_sync::<ClientError>();
    }

    // =========================================================================
    // Method signature tests - verify the API compiles
    // Full integration tests require a running leader (see tests/integration/)
    // =========================================================================

    /// Verify find_all_duplicates signature compiles
    #[allow(dead_code)]
    async fn api_find_all_duplicates(c: &IndexClient) {
        let _ = c.find_all_duplicates(0.9, 100).await;
    }

    /// Verify find_duplicates_in_file signature compiles
    #[allow(dead_code)]
    async fn api_find_duplicates_in_file(c: &IndexClient) {
        let _ = c.find_duplicates_in_file(PathBuf::from("/test"), 0.9).await;
    }

    /// Verify semantic_search signature compiles
    #[allow(dead_code)]
    async fn api_semantic_search(c: &IndexClient) {
        let _ = c.semantic_search("query".to_string(), 10, 0.8).await;
    }

    /// Verify tree_sitter_query signature compiles
    #[allow(dead_code)]
    async fn api_tree_sitter_query(c: &IndexClient) {
        let _ = c.tree_sitter_query("(identifier)".to_string(), None, None).await;
    }

    /// Verify list_files signature compiles
    #[allow(dead_code)]
    async fn api_list_files(c: &IndexClient) {
        let _ = c.list_files().await;
    }

    /// Verify status signature compiles
    #[allow(dead_code)]
    async fn api_status(c: &IndexClient) {
        let _ = c.status().await;
    }

    /// Verify invalidate_file signature compiles
    #[allow(dead_code)]
    async fn api_invalidate_file(c: &IndexClient) {
        let _ = c.invalidate_file(PathBuf::from("/test")).await;
    }

    /// Verify socket_path signature compiles
    #[allow(dead_code)]
    fn api_socket_path(c: &IndexClient) {
        let _: &Path = c.socket_path();
    }

    /// Verify workspace_root signature compiles
    #[allow(dead_code)]
    fn api_workspace_root(c: &IndexClient) {
        let _: &Path = c.workspace_root();
    }
}
