//! RPC server implementation for the tree-sitter index service
//!
//! This module contains `IndexServiceServer` which implements the tarpc
//! `IndexService` trait to handle incoming queries from clients.

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Maximum number of similar chunks to return when finding duplicates for a file
const DUPLICATES_TOP_K: usize = 100;

use tokio::sync::RwLock;
use tree_sitter::StreamingIterator;

use crate::chunk::{cosine_similarity, SemanticChunk, SimilarityQuery};
use crate::index::IndexContext;
use crate::query::service::IndexService;
use crate::query::types::{
    check_ready, Capture, ChunkResult, DuplicateCluster, IndexStatusInfo, QueryError, QueryMatch,
    SimilarChunkResult,
};

/// The tarpc service implementation.
///
/// Handles incoming RPC requests from clients by delegating to the shared index.
#[derive(Clone)]
pub struct IndexServiceServer {
    pub(crate) index: Arc<RwLock<IndexContext>>,
}

impl IndexServiceServer {
    /// Create a new service server wrapping the given index context
    pub fn new(index: Arc<RwLock<IndexContext>>) -> Self {
        Self { index }
    }
}

impl IndexService for IndexServiceServer {
    async fn find_all_duplicates(
        self,
        _: tarpc::context::Context,
        min_similarity: f32,
        min_chunk_bytes: usize,
    ) -> Result<Vec<DuplicateCluster>, QueryError> {
        let index = self.index.read().await;
        check_ready(index.status().is_complete())?;
        Ok(find_all_duplicates_impl(index.chunk_graph(), min_similarity, min_chunk_bytes))
    }

    async fn find_duplicates_in_file(
        self,
        _: tarpc::context::Context,
        file: PathBuf,
        min_similarity: f32,
    ) -> Result<Vec<SimilarChunkResult>, QueryError> {
        let index = self.index.read().await;
        check_ready(index.status().is_complete())?;
        find_duplicates_in_file_impl(index.chunk_graph(), &file, min_similarity)
    }

    async fn semantic_search(
        self,
        _: tarpc::context::Context,
        text: String,
        top_k: usize,
        min_similarity: f32,
    ) -> Result<Vec<SimilarChunkResult>, QueryError> {
        // Embed the query text (requires write lock for lazy model loading)
        let query_embedding = {
            let mut index = self.index.write().await;
            check_ready(index.status().is_complete())?;
            index
                .embed_text(&text)
                .await
                .map_err(|e| QueryError::embedding_error(e.to_string()))?
        };

        // Search using the embedding (read lock is sufficient)
        let index = self.index.read().await;
        Ok(semantic_search_impl(index.chunk_graph(), query_embedding, top_k, min_similarity))
    }

    async fn tree_sitter_query(
        self,
        _: tarpc::context::Context,
        query: String,
        files: Option<Vec<PathBuf>>,
        language: Option<String>,
    ) -> Result<Vec<QueryMatch>, QueryError> {
        let index = self.index.read().await;
        check_ready(index.status().is_complete())?;
        tree_sitter_query_impl(&index, &query, files, language)
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

// ============================================================================
// Public implementation functions for use by unified::Workspace
// ============================================================================

/// Find all duplicate code clusters across the index.
///
/// Used by `Workspace` in leader mode.
pub(crate) fn find_all_duplicates_impl(
    graph: &crate::chunk::ChunkGraph,
    min_similarity: f32,
    min_chunk_bytes: usize,
) -> Vec<DuplicateCluster> {
    let chunks: Vec<&SemanticChunk> = graph
        .chunks()
        .iter()
        .filter(|c| c.byte_len() >= min_chunk_bytes && c.has_embedding())
        .collect();

    if chunks.is_empty() {
        return Vec::new();
    }

    let mut clusters = find_duplicate_clusters(&chunks, min_similarity);

    clusters
        .drain(..)
        .filter(|cluster| cluster.len() > 1)
        .map(|cluster| {
            let avg_sim = compute_average_similarity(&cluster);
            DuplicateCluster {
                chunks: cluster.iter().map(chunk_to_result).collect(),
                avg_similarity: avg_sim,
            }
        })
        .collect()
}

/// Find duplicates for chunks in a specific file.
///
/// Used by `Workspace` in leader mode.
pub(crate) fn find_duplicates_in_file_impl(
    graph: &crate::chunk::ChunkGraph,
    file: &Path,
    min_similarity: f32,
) -> Result<Vec<SimilarChunkResult>, QueryError> {
    let file_chunks = graph.chunks_for_file(file);
    if file_chunks.is_empty() {
        return Err(QueryError::file_not_found(file));
    }

    let mut results = Vec::new();
    for chunk in file_chunks {
        if !chunk.has_embedding() {
            continue;
        }

        let query = SimilarityQuery::chunk(chunk.clone())
            .min_similarity(min_similarity)
            .top_k(DUPLICATES_TOP_K);

        for sim_chunk in graph.query(query) {
            if sim_chunk.chunk.path() == Some(file) {
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

/// Semantic search using a pre-computed embedding.
///
/// Used by `Workspace` in leader mode.
pub(crate) fn semantic_search_impl(
    graph: &crate::chunk::ChunkGraph,
    query_embedding: Vec<f32>,
    top_k: usize,
    min_similarity: f32,
) -> Vec<SimilarChunkResult> {
    let query = SimilarityQuery::embedding(query_embedding)
        .top_k(top_k)
        .min_similarity(min_similarity);

    graph
        .query(query)
        .into_iter()
        .map(|sim_chunk| SimilarChunkResult {
            chunk: chunk_to_result(&sim_chunk.chunk),
            similarity: sim_chunk.similarity,
        })
        .collect()
}

/// Execute a tree-sitter query across index files.
///
/// Used by `Workspace` in leader mode.
pub(crate) fn tree_sitter_query_impl(
    index: &IndexContext,
    query: &str,
    files: Option<Vec<PathBuf>>,
    language: Option<String>,
) -> Result<Vec<QueryMatch>, QueryError> {
    let file_paths = files.unwrap_or_else(|| index.files());
    let registry = crate::language::LanguageRegistry::global();

    let mut results = Vec::new();

    for path in file_paths {
        if let Some(parsed) = index.get(&path) {
            if let Some(lang_config) = registry.detect_language(&path) {
                if language.as_ref().is_some_and(|l| l != lang_config.name) {
                    continue;
                }
                if let Some(matches) = run_ts_query(query, parsed, lang_config, &path)? {
                    results.extend(matches);
                }
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::types::QueryErrorKind;

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

    // =========================================================================
    // IndexServiceServer tests
    // =========================================================================

    #[test]
    fn test_index_service_server_new() {
        let context = Arc::new(RwLock::new(IndexContext::new("/tmp")));
        let server = IndexServiceServer::new(context.clone());
        assert!(Arc::ptr_eq(&server.index, &context));
    }

    // =========================================================================
    // IndexService trait implementation tests
    // Note: Full integration tests are in tests/leader_client.rs
    // =========================================================================

    #[tokio::test]
    async fn test_trait_find_all_duplicates_not_ready() {
        let context = Arc::new(RwLock::new(IndexContext::new("/nonexistent")));
        let server = IndexServiceServer::new(context);
        let result = server.find_all_duplicates(tarpc::context::current(), 0.9, 10).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err().kind, QueryErrorKind::NotReady));
    }

    #[tokio::test]
    async fn test_trait_find_duplicates_in_file_not_ready() {
        let context = Arc::new(RwLock::new(IndexContext::new("/nonexistent")));
        let server = IndexServiceServer::new(context);
        let result = server.find_duplicates_in_file(
            tarpc::context::current(),
            PathBuf::from("/test.rs"),
            0.9
        ).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err().kind, QueryErrorKind::NotReady));
    }

    #[tokio::test]
    async fn test_trait_semantic_search_not_ready() {
        let context = Arc::new(RwLock::new(IndexContext::new("/nonexistent")));
        let server = IndexServiceServer::new(context);
        let result = server.semantic_search(
            tarpc::context::current(),
            "test".to_string(),
            10,
            0.5
        ).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err().kind, QueryErrorKind::NotReady));
    }

    #[tokio::test]
    async fn test_trait_tree_sitter_query_not_ready() {
        let context = Arc::new(RwLock::new(IndexContext::new("/nonexistent")));
        let server = IndexServiceServer::new(context);
        let result = server.tree_sitter_query(
            tarpc::context::current(),
            "(identifier) @name".to_string(),
            None,
            None
        ).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err().kind, QueryErrorKind::NotReady));
    }

    #[tokio::test]
    async fn test_trait_list_files_empty() {
        let context = Arc::new(RwLock::new(IndexContext::new("/nonexistent")));
        let server = IndexServiceServer::new(context);
        let files = server.list_files(tarpc::context::current()).await;
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn test_trait_status_not_ready() {
        let context = Arc::new(RwLock::new(IndexContext::new("/nonexistent")));
        let server = IndexServiceServer::new(context);
        let status = server.status(tarpc::context::current()).await;
        assert!(!status.is_ready);
        assert_eq!(status.files_total, 0);
    }

    // =========================================================================
    // Public implementation function tests
    // =========================================================================

    #[test]
    fn test_find_all_duplicates_impl_empty_graph() {
        let graph = crate::chunk::ChunkGraph::new();
        let clusters = find_all_duplicates_impl(&graph, 0.9, 10);
        assert!(clusters.is_empty());
    }

    #[test]
    fn test_find_all_duplicates_impl_no_embeddings() {
        let mut graph = crate::chunk::ChunkGraph::new();
        graph.add(SemanticChunk::from_text("test code without embedding"));
        let clusters = find_all_duplicates_impl(&graph, 0.9, 5);
        assert!(clusters.is_empty());
    }

    #[test]
    fn test_find_all_duplicates_impl_single_chunk() {
        let mut graph = crate::chunk::ChunkGraph::new();
        graph.add(SemanticChunk::from_text("test").with_embedding(vec![1.0, 0.0, 0.0]));
        let clusters = find_all_duplicates_impl(&graph, 0.9, 1);
        assert!(clusters.is_empty());
    }

    #[test]
    fn test_find_duplicates_in_file_impl_file_not_found() {
        let graph = crate::chunk::ChunkGraph::new();
        let result = find_duplicates_in_file_impl(&graph, Path::new("/nonexistent.rs"), 0.9);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().kind,
            crate::query::types::QueryErrorKind::FileNotFound
        ));
    }

    #[test]
    fn test_find_duplicates_in_file_impl_empty_graph() {
        let graph = crate::chunk::ChunkGraph::new();
        let result = find_duplicates_in_file_impl(&graph, Path::new("/test.rs"), 0.9);
        assert!(result.is_err());
    }

    #[test]
    fn test_semantic_search_impl_empty_graph() {
        let graph = crate::chunk::ChunkGraph::new();
        let results = semantic_search_impl(&graph, vec![1.0, 0.0, 0.0], 10, 0.5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_semantic_search_impl_with_chunks() {
        let mut graph = crate::chunk::ChunkGraph::new();
        graph.add(SemanticChunk::from_text("hello world").with_embedding(vec![1.0, 0.0, 0.0]));
        graph.add(SemanticChunk::from_text("goodbye").with_embedding(vec![0.0, 1.0, 0.0]));

        let results = semantic_search_impl(&graph, vec![1.0, 0.0, 0.0], 10, 0.5);
        assert!(!results.is_empty());
        assert!(results[0].similarity > 0.9);
    }

    #[test]
    fn test_semantic_search_impl_min_similarity_filter() {
        let mut graph = crate::chunk::ChunkGraph::new();
        graph.add(SemanticChunk::from_text("test").with_embedding(vec![1.0, 0.0, 0.0]));

        let results = semantic_search_impl(&graph, vec![0.0, 1.0, 0.0], 10, 0.99);
        assert!(results.is_empty());
    }

    #[test]
    fn test_tree_sitter_query_impl_empty_index() {
        let index = IndexContext::new("/nonexistent");
        let results = tree_sitter_query_impl(&index, "(identifier) @name", None, None);
        assert!(results.is_ok());
        assert!(results.unwrap().is_empty());
    }

    #[test]
    fn test_tree_sitter_query_impl_with_language_filter() {
        let index = IndexContext::new("/tmp");
        let results =
            tree_sitter_query_impl(&index, "(identifier) @name", None, Some("rust".to_string()));
        assert!(results.is_ok());
    }

    #[test]
    fn test_tree_sitter_query_impl_with_file_filter() {
        let index = IndexContext::new("/tmp");
        let results = tree_sitter_query_impl(
            &index,
            "(identifier) @name",
            Some(vec![PathBuf::from("/tmp/test.rs")]),
            None,
        );
        assert!(results.is_ok());
    }
}
