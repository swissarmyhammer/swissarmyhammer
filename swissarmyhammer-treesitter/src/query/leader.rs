//! Leader process that owns the tree-sitter index
//!
//! The leader:
//! - Owns the `IndexContext` with all parsed files and embeddings
//! - Listens on a Unix socket for client connections
//! - Handles queries via the tarpc `IndexService` trait
//! - Maintains file watchers to keep the index up to date

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::StreamExt;
use tree_sitter::StreamingIterator;
use tarpc::server::{self, Channel};
use tokio_serde::formats::Bincode;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{watch, RwLock};

use crate::chunk::{cosine_similarity, SemanticChunk, SimilarityQuery};
use crate::index::IndexContext;
use crate::query::election::LeaderGuard;
use crate::query::service::IndexService;
use crate::query::types::{
    Capture, ChunkResult, DuplicateCluster, IndexStatusInfo, QueryError, QueryMatch,
    SimilarChunkResult,
};

/// The index leader server
///
/// Holds the index context and serves queries from clients.
pub struct IndexLeader {
    /// The index context (owned by leader)
    index: Arc<RwLock<IndexContext>>,
    /// Shutdown signal sender
    shutdown_tx: watch::Sender<bool>,
    /// Leader guard (holds the lock)
    _guard: LeaderGuard,
}

impl IndexLeader {
    /// Create a new leader with the given guard and workspace root
    ///
    /// This will scan and index the workspace on creation.
    pub async fn new(guard: LeaderGuard, workspace_root: impl AsRef<Path>) -> crate::Result<Self> {
        let mut index = IndexContext::new(workspace_root);
        index.scan().await?;

        let (shutdown_tx, _) = watch::channel(false);

        Ok(Self {
            index: Arc::new(RwLock::new(index)),
            shutdown_tx,
            _guard: guard,
        })
    }

    /// Run the leader server on the given socket path
    ///
    /// This will listen for client connections and handle queries until shutdown.
    pub async fn run(self, socket_path: &Path) -> crate::Result<()> {
        let _ = std::fs::remove_file(socket_path);

        let listener = UnixListener::bind(socket_path).map_err(crate::TreeSitterError::Io)?;

        tracing::info!("Leader listening on {}", socket_path.display());

        let index = self.index.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, _)) => {
                            spawn_connection_handler(stream, index.clone());
                        }
                        Err(e) => {
                            tracing::warn!("Failed to accept connection: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("Leader shutting down");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Signal the leader to shut down
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    /// Get a reference to the index (for testing)
    #[cfg(test)]
    pub fn index(&self) -> &Arc<RwLock<IndexContext>> {
        &self.index
    }
}

/// Spawn a task to handle a client connection.
///
/// Sets up the tarpc transport over the Unix socket and runs the service.
fn spawn_connection_handler(stream: UnixStream, index: Arc<RwLock<IndexContext>>) {
    let codec_builder = Bincode::default;

    tokio::spawn(async move {
        let framed = tokio_util::codec::Framed::new(
            stream,
            tarpc::tokio_util::codec::LengthDelimitedCodec::new(),
        );
        let transport = tarpc::serde_transport::new(framed, codec_builder());

        let server = IndexServiceServer { index };
        let channel = server::BaseChannel::with_defaults(transport);

        channel
            .execute(server.serve())
            .for_each(|response| async move {
                tokio::spawn(response);
            })
            .await;
    });
}

/// The tarpc service implementation.
///
/// Handles incoming RPC requests from clients by delegating to the shared index.
#[derive(Clone)]
struct IndexServiceServer {
    index: Arc<RwLock<IndexContext>>,
}

impl IndexServiceServer {
    /// Check if the index is ready for queries.
    ///
    /// Returns `QueryError::NotReady` if the index is still building.
    async fn ensure_ready(&self) -> Result<(), QueryError> {
        let index = self.index.read().await;
        if !index.status().is_complete() {
            return Err(QueryError::not_ready());
        }
        Ok(())
    }
}

impl IndexService for IndexServiceServer {
    async fn find_all_duplicates(
        self,
        _: tarpc::context::Context,
        min_similarity: f32,
        min_chunk_bytes: usize,
    ) -> Result<Vec<DuplicateCluster>, QueryError> {
        self.ensure_ready().await?;

        let index = self.index.read().await;
        let graph = index.chunk_graph();

        let chunks: Vec<&SemanticChunk> = graph
            .chunks()
            .iter()
            .filter(|c| c.byte_len() >= min_chunk_bytes && c.has_embedding())
            .collect();

        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        let mut clusters = find_duplicate_clusters(&chunks, min_similarity);

        let results = clusters
            .drain(..)
            .filter(|cluster| cluster.len() > 1)
            .map(|cluster| {
                let avg_sim = compute_average_similarity(&cluster);
                DuplicateCluster {
                    chunks: cluster.iter().map(chunk_to_result).collect(),
                    avg_similarity: avg_sim,
                }
            })
            .collect();

        Ok(results)
    }

    async fn find_duplicates_in_file(
        self,
        _: tarpc::context::Context,
        file: PathBuf,
        min_similarity: f32,
    ) -> Result<Vec<SimilarChunkResult>, QueryError> {
        self.ensure_ready().await?;

        let index = self.index.read().await;
        let graph = index.chunk_graph();

        let file_chunks = graph.chunks_for_file(&file);
        if file_chunks.is_empty() {
            return Err(QueryError::file_not_found(&file));
        }

        let mut results = Vec::new();
        for chunk in file_chunks {
            if !chunk.has_embedding() {
                continue;
            }

            let query = SimilarityQuery::chunk(chunk.clone())
                .min_similarity(min_similarity)
                .top_k(100);

            for sim_chunk in graph.query(query) {
                if sim_chunk.chunk.path() == Some(file.as_path()) {
                    continue;
                }

                results.push(SimilarChunkResult {
                    chunk: chunk_to_result(&sim_chunk.chunk),
                    similarity: sim_chunk.similarity,
                });
            }
        }

        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }

    async fn semantic_search(
        self,
        _: tarpc::context::Context,
        text: String,
        top_k: usize,
        min_similarity: f32,
    ) -> Result<Vec<SimilarChunkResult>, QueryError> {
        self.ensure_ready().await?;

        // Embed the query text (requires write lock for lazy model loading)
        let query_embedding = {
            let mut index = self.index.write().await;
            index
                .embed_text(&text)
                .await
                .map_err(|e| QueryError::embedding_error(e.to_string()))?
        };

        // Search using the embedding (read lock is sufficient)
        let index = self.index.read().await;
        let query = SimilarityQuery::embedding(query_embedding)
            .top_k(top_k)
            .min_similarity(min_similarity);

        let results = index
            .chunk_graph()
            .query(query)
            .into_iter()
            .map(|sim_chunk| SimilarChunkResult {
                chunk: chunk_to_result(&sim_chunk.chunk),
                similarity: sim_chunk.similarity,
            })
            .collect();

        Ok(results)
    }

    async fn tree_sitter_query(
        self,
        _: tarpc::context::Context,
        query: String,
        files: Option<Vec<PathBuf>>,
        language: Option<String>,
    ) -> Result<Vec<QueryMatch>, QueryError> {
        self.ensure_ready().await?;

        let index = self.index.read().await;
        let file_paths = files.unwrap_or_else(|| index.files());
        let registry = crate::language::LanguageRegistry::global();

        let mut results = Vec::new();

        for path in file_paths {
            if let Some(parsed) = index.get(&path) {
                if let Some(lang_config) = registry.detect_language(&path) {
                    if language.as_ref().is_some_and(|l| l != lang_config.name) {
                        continue;
                    }
                    if let Some(matches) = run_ts_query(&query, parsed, lang_config, &path)? {
                        results.extend(matches);
                    }
                }
            }
        }

        Ok(results)
    }

    async fn list_files(self, _: tarpc::context::Context) -> Vec<PathBuf> {
        let index = self.index.read().await;
        index.files()
    }

    async fn status(self, _: tarpc::context::Context) -> IndexStatusInfo {
        let index = self.index.read().await;
        let status = index.status();

        IndexStatusInfo {
            files_total: status.files_total,
            files_indexed: status.files_parsed,
            files_embedded: status.files_embedded,
            is_ready: status.is_complete(),
            root_path: index.root_path().to_path_buf(),
        }
    }

    async fn invalidate_file(
        self,
        _: tarpc::context::Context,
        file: PathBuf,
    ) -> Result<(), QueryError> {
        let mut index = self.index.write().await;
        index
            .refresh(&file)
            .await
            .map_err(|e| QueryError::internal(e.to_string()))?;
        Ok(())
    }
}

/// Run a tree-sitter query on a parsed file.
///
/// Compiles the query, executes it against the parsed AST, and returns matches.
fn run_ts_query(
    query: &str,
    parsed: &crate::ParsedFile,
    lang_config: &crate::language::LanguageConfig,
    path: &Path,
) -> Result<Option<Vec<QueryMatch>>, QueryError> {
    let ts_query = tree_sitter::Query::new(&lang_config.language(), query)
        .map_err(|e| QueryError::invalid_query(format!("Query error: {}", e)))?;

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&ts_query, parsed.root_node(), parsed.source.as_bytes());

    let mut results = Vec::new();
    while let Some(m) = matches.next() {
        let captures: Vec<Capture> = m
            .captures
            .iter()
            .map(|cap| {
                let node = cap.node;
                Capture {
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
                file: path.to_path_buf(),
                captures,
            });
        }
    }

    Ok(if results.is_empty() {
        None
    } else {
        Some(results)
    })
}

/// Convert a SemanticChunk to a serializable ChunkResult.
///
/// Extracts position and content information from the chunk for serialization.
fn chunk_to_result(chunk: &SemanticChunk) -> ChunkResult {
    let (start_line, end_line) = chunk
        .node()
        .map(|n| (n.start_position().row, n.end_position().row))
        .unwrap_or((0, 0));

    let (start_byte, end_byte) = match &chunk.source {
        crate::chunk::ChunkSource::Parsed {
            start_byte,
            end_byte,
            ..
        } => (*start_byte, *end_byte),
        crate::chunk::ChunkSource::Text(s) => (0, s.len()),
    };

    ChunkResult {
        file: chunk.path().unwrap_or(Path::new("")).to_path_buf(),
        text: chunk.content().unwrap_or("").to_string(),
        start_byte,
        end_byte,
        start_line,
        end_line,
    }
}

/// Find clusters of duplicate chunks using similarity threshold.
///
/// Uses union-find to group chunks with similarity above the threshold.
/// Chunks from the same file are not clustered together.
fn find_duplicate_clusters(
    chunks: &[&SemanticChunk],
    min_similarity: f32,
) -> Vec<Vec<SemanticChunk>> {
    let n = chunks.len();
    if n == 0 {
        return Vec::new();
    }

    let mut parent: Vec<usize> = (0..n).collect();

    fn find(parent: &mut [usize], i: usize) -> usize {
        if parent[i] != i {
            parent[i] = find(parent, parent[i]);
        }
        parent[i]
    }

    fn union(parent: &mut [usize], i: usize, j: usize) {
        let pi = find(parent, i);
        let pj = find(parent, j);
        if pi != pj {
            parent[pi] = pj;
        }
    }

    for i in 0..n {
        for j in (i + 1)..n {
            if let (Some(emb_i), Some(emb_j)) = (&chunks[i].embedding, &chunks[j].embedding) {
                let sim = cosine_similarity(emb_i, emb_j);
                if sim >= min_similarity && chunks[i].path() != chunks[j].path() {
                    union(&mut parent, i, j);
                }
            }
        }
    }

    let mut groups: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        groups.entry(root).or_default().push(i);
    }

    groups
        .into_values()
        .map(|indices| indices.into_iter().map(|i| chunks[i].clone()).collect())
        .collect()
}

/// Compute average pairwise similarity within a cluster.
///
/// Returns 1.0 for single-element clusters, 0.0 if no embeddings are available.
fn compute_average_similarity(cluster: &[SemanticChunk]) -> f32 {
    if cluster.len() < 2 {
        return 1.0;
    }

    let mut total_sim = 0.0;
    let mut count = 0;

    for i in 0..cluster.len() {
        for j in (i + 1)..cluster.len() {
            if let (Some(emb_i), Some(emb_j)) = (&cluster[i].embedding, &cluster[j].embedding) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_to_result() {
        let chunk = SemanticChunk::from_text("test code");
        let result = chunk_to_result(&chunk);

        assert_eq!(result.text, "test code");
        assert_eq!(result.start_byte, 0);
        assert_eq!(result.end_byte, 9);
    }

    #[test]
    fn test_find_duplicate_clusters_empty() {
        let chunks: Vec<&SemanticChunk> = vec![];
        let clusters = find_duplicate_clusters(&chunks, 0.9);
        assert!(clusters.is_empty());
    }

    #[test]
    fn test_compute_average_similarity_single() {
        let chunk = SemanticChunk::from_text("code").with_embedding(vec![1.0, 0.0, 0.0]);
        let cluster = vec![chunk];
        assert_eq!(compute_average_similarity(&cluster), 1.0);
    }

    #[test]
    fn test_compute_average_similarity_identical() {
        let chunk1 = SemanticChunk::from_text("a").with_embedding(vec![1.0, 0.0, 0.0]);
        let chunk2 = SemanticChunk::from_text("b").with_embedding(vec![1.0, 0.0, 0.0]);
        let cluster = vec![chunk1, chunk2];

        let avg = compute_average_similarity(&cluster);
        assert!((avg - 1.0).abs() < 0.001);
    }
}
