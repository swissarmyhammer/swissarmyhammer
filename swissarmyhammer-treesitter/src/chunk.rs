//! Semantic chunking for parsed files
//!
//! Chunks can come from parsed files (with AST access) or raw text (for queries).

use crate::parsed_file::ParsedFile;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// === Constants ===

/// Default top_k for similarity queries
pub const DEFAULT_TOP_K: usize = 10;
/// Default minimum similarity threshold
pub const DEFAULT_MIN_SIMILARITY: f32 = 0.8;

/// Source of a chunk - parsed file or raw text
///
/// Parsed sources have AST access via tree-sitter. Text sources are used
/// for semantic search queries with arbitrary strings.
#[derive(Debug, Clone)]
pub enum ChunkSource {
    /// From a parsed file with AST access
    Parsed {
        /// Start byte offset in source
        start_byte: usize,
        /// End byte offset in source
        end_byte: usize,
        /// The parsed file (contains path, source, and tree)
        parsed_file: Arc<ParsedFile>,
    },
    /// Raw text (for semantic search queries)
    Text(String),
}

impl ChunkSource {
    /// Create a chunk source from a parsed file with byte range
    pub fn parsed(parsed_file: Arc<ParsedFile>, start_byte: usize, end_byte: usize) -> Self {
        Self::Parsed {
            start_byte,
            end_byte,
            parsed_file,
        }
    }

    /// Create a chunk source from raw text
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text(content.into())
    }

    /// File path (None for text sources)
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Parsed { parsed_file, .. } => Some(&parsed_file.path),
            Self::Text(_) => None,
        }
    }

    /// Byte length of this chunk
    pub fn byte_len(&self) -> usize {
        match self {
            Self::Parsed {
                start_byte,
                end_byte,
                ..
            } => end_byte.saturating_sub(*start_byte),
            Self::Text(s) => s.len(),
        }
    }

    /// Tree-sitter node for this chunk (None for text sources)
    pub fn node(&self) -> Option<tree_sitter::Node<'_>> {
        match self {
            Self::Parsed {
                start_byte,
                end_byte,
                parsed_file,
            } => parsed_file
                .tree
                .root_node()
                .descendant_for_byte_range(*start_byte, *end_byte),
            Self::Text(_) => None,
        }
    }

    /// Parent node of this chunk's node (None for text sources)
    pub fn parent_node(&self) -> Option<tree_sitter::Node<'_>> {
        self.node().and_then(|n| n.parent())
    }

    /// Text content of this chunk
    pub fn content(&self) -> Option<&str> {
        match self {
            Self::Parsed {
                start_byte,
                end_byte,
                parsed_file,
            } => parsed_file.get_text(*start_byte, *end_byte),
            Self::Text(s) => Some(s.as_str()),
        }
    }

    /// Check if this is a parsed source (with AST access)
    pub fn is_parsed(&self) -> bool {
        matches!(self, Self::Parsed { .. })
    }

    /// Check if this is a text-only source
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text(_))
    }
}

impl PartialEq for ChunkSource {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Parsed {
                    start_byte: s1,
                    end_byte: e1,
                    parsed_file: p1,
                },
                Self::Parsed {
                    start_byte: s2,
                    end_byte: e2,
                    parsed_file: p2,
                },
            ) => p1.path == p2.path && s1 == s2 && e1 == e2,
            (Self::Text(t1), Self::Text(t2)) => t1 == t2,
            _ => false,
        }
    }
}

impl Eq for ChunkSource {}

impl std::hash::Hash for ChunkSource {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Parsed {
                start_byte,
                end_byte,
                parsed_file,
            } => {
                0u8.hash(state);
                parsed_file.path.hash(state);
                start_byte.hash(state);
                end_byte.hash(state);
            }
            Self::Text(s) => {
                1u8.hash(state);
                s.hash(state);
            }
        }
    }
}

impl PartialOrd for ChunkSource {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ChunkSource {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (
                Self::Parsed {
                    start_byte: s1,
                    end_byte: e1,
                    parsed_file: p1,
                },
                Self::Parsed {
                    start_byte: s2,
                    end_byte: e2,
                    parsed_file: p2,
                },
            ) => (&p1.path, s1, e1).cmp(&(&p2.path, s2, e2)),
            (Self::Text(t1), Self::Text(t2)) => t1.cmp(t2),
            (Self::Parsed { .. }, Self::Text(_)) => std::cmp::Ordering::Less,
            (Self::Text(_), Self::Parsed { .. }) => std::cmp::Ordering::Greater,
        }
    }
}

/// A semantic chunk with optional embedding
///
/// Content is accessed via `content()` which retrieves from the source.
#[derive(Debug, Clone)]
pub struct SemanticChunk {
    /// The source of this chunk (parsed file or text)
    pub source: ChunkSource,
    /// Embedding vector (populated after embedding model processes this chunk)
    pub embedding: Option<Vec<f32>>,
}

impl SemanticChunk {
    /// Create from a chunk source
    pub fn new(source: ChunkSource) -> Self {
        Self {
            source,
            embedding: None,
        }
    }

    /// Create from raw text (convenience)
    pub fn from_text(content: impl Into<String>) -> Self {
        Self::new(ChunkSource::text(content))
    }

    /// Create from a parsed file range (convenience)
    pub fn from_parsed(parsed_file: Arc<ParsedFile>, start_byte: usize, end_byte: usize) -> Self {
        Self::new(ChunkSource::parsed(parsed_file, start_byte, end_byte))
    }

    /// Text content of this chunk
    pub fn content(&self) -> Option<&str> {
        self.source.content()
    }

    /// File path (None for text sources)
    pub fn path(&self) -> Option<&Path> {
        self.source.path()
    }

    /// Tree-sitter node (None for text sources)
    pub fn node(&self) -> Option<tree_sitter::Node<'_>> {
        self.source.node()
    }

    /// Parent node (None for text sources)
    pub fn parent_node(&self) -> Option<tree_sitter::Node<'_>> {
        self.source.parent_node()
    }

    /// Byte length of this chunk
    pub fn byte_len(&self) -> usize {
        self.source.byte_len()
    }

    /// Set embedding vector
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }

    /// Check if this chunk has an embedding
    pub fn has_embedding(&self) -> bool {
        self.embedding.is_some()
    }

    /// Cosine similarity with another chunk (0.0 if either lacks embedding)
    pub fn similarity_to(&self, other: &SemanticChunk) -> f32 {
        match (&self.embedding, &other.embedding) {
            (Some(a), Some(b)) => cosine_similarity(a, b),
            _ => 0.0,
        }
    }
}

/// SIMD-accelerated cosine similarity between two vectors
///
/// Returns similarity in range [-1.0, 1.0] where 1.0 = identical.
/// Returns 0.0 for mismatched lengths or empty vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    use simsimd::SpatialSimilarity;

    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    match f32::cosine(a, b) {
        Some(distance) => 1.0 - distance as f32,
        None => 0.0,
    }
}

/// A similarity match result containing the chunk and its similarity score
#[derive(Debug, Clone)]
pub struct SimilarChunk {
    /// The matched chunk
    pub chunk: SemanticChunk,
    /// Similarity score (cosine similarity, -1.0 to 1.0)
    pub similarity: f32,
}

/// Query for finding similar chunks (builder pattern)
///
/// # Example
///
/// ```ignore
/// let q = SimilarityQuery::file("/src/main.rs")
///     .top_k(5)
///     .min_similarity(0.9);
/// let results = graph.query(q);
/// ```
#[derive(Debug, Clone)]
pub struct SimilarityQuery {
    /// What to search for
    pub source: QuerySource,
    /// Maximum results to return
    pub top_k: usize,
    /// Minimum similarity threshold (0.0 to 1.0)
    pub min_similarity: f32,
}

/// Query source variants
#[derive(Debug, Clone)]
pub enum QuerySource {
    /// Query by a single chunk
    Chunk(SemanticChunk),
    /// Query by multiple chunks (finds similar to any)
    Chunks(Vec<SemanticChunk>),
    /// Query by file path (uses all chunks from that file)
    File(PathBuf),
    /// Query by raw embedding vector
    Embedding(Vec<f32>),
}

impl SimilarityQuery {
    /// Query by a single chunk
    pub fn chunk(chunk: SemanticChunk) -> Self {
        Self {
            source: QuerySource::Chunk(chunk),
            top_k: DEFAULT_TOP_K,
            min_similarity: DEFAULT_MIN_SIMILARITY,
        }
    }

    /// Query by multiple chunks
    pub fn chunks(chunks: Vec<SemanticChunk>) -> Self {
        Self {
            source: QuerySource::Chunks(chunks),
            top_k: DEFAULT_TOP_K,
            min_similarity: DEFAULT_MIN_SIMILARITY,
        }
    }

    /// Query by file path
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self {
            source: QuerySource::File(path.into()),
            top_k: DEFAULT_TOP_K,
            min_similarity: DEFAULT_MIN_SIMILARITY,
        }
    }

    /// Query by raw embedding vector
    pub fn embedding(embedding: Vec<f32>) -> Self {
        Self {
            source: QuerySource::Embedding(embedding),
            top_k: DEFAULT_TOP_K,
            min_similarity: DEFAULT_MIN_SIMILARITY,
        }
    }

    /// Set maximum results to return
    pub fn top_k(mut self, top_k: usize) -> Self {
        self.top_k = top_k;
        self
    }

    /// Set minimum similarity threshold (0.0 to 1.0)
    pub fn min_similarity(mut self, min_similarity: f32) -> Self {
        self.min_similarity = min_similarity;
        self
    }

    fn resolve(
        &self,
        graph: &ChunkGraph,
    ) -> (Vec<Vec<f32>>, std::collections::HashSet<ChunkSource>) {
        match &self.source {
            QuerySource::Chunk(chunk) => {
                let embs = chunk.embedding.clone().into_iter().collect();
                let exclude = std::iter::once(chunk.source.clone()).collect();
                (embs, exclude)
            }
            QuerySource::Chunks(chunks) => {
                let embs = chunks.iter().filter_map(|c| c.embedding.clone()).collect();
                let exclude = chunks.iter().map(|c| c.source.clone()).collect();
                (embs, exclude)
            }
            QuerySource::File(path) => {
                let file_chunks: Vec<_> = graph
                    .chunks
                    .iter()
                    .filter(|c| c.source.path() == Some(path.as_path()))
                    .collect();
                let embs = file_chunks
                    .iter()
                    .filter_map(|c| c.embedding.clone())
                    .collect();
                let exclude = file_chunks.iter().map(|c| c.source.clone()).collect();
                (embs, exclude)
            }
            QuerySource::Embedding(emb) => (vec![emb.clone()], std::collections::HashSet::new()),
        }
    }
}

/// Graph of semantic chunks with similarity queries
#[derive(Debug, Clone, Default)]
pub struct ChunkGraph {
    chunks: Vec<SemanticChunk>,
}

impl ChunkGraph {
    /// Create a new empty chunk graph
    pub fn new() -> Self {
        Self { chunks: Vec::new() }
    }

    /// Add a semantic chunk to the graph
    pub fn add(&mut self, chunk: SemanticChunk) {
        self.chunks.push(chunk);
    }

    /// Remove all chunks from a file path
    pub fn remove_file(&mut self, path: &Path) {
        self.chunks.retain(|c| c.source.path() != Some(path));
    }

    /// Get all chunks
    pub fn chunks(&self) -> &[SemanticChunk] {
        &self.chunks
    }

    /// Get chunks from a specific file
    pub fn chunks_for_file(&self, path: &Path) -> Vec<&SemanticChunk> {
        self.chunks
            .iter()
            .filter(|c| c.source.path() == Some(path))
            .collect()
    }

    /// Query for similar chunks
    pub fn query(&self, query: SimilarityQuery) -> Vec<SimilarChunk> {
        let (embeddings, exclude_sources) = query.resolve(self);

        if embeddings.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<SimilarChunk> = self
            .chunks
            .iter()
            .filter(|c| !exclude_sources.contains(&c.source))
            .filter_map(|c| self.compute_similarity(c, &embeddings, query.min_similarity))
            .collect();

        self.sort_and_truncate(&mut results, query.top_k);
        results
    }

    fn compute_similarity(
        &self,
        candidate: &SemanticChunk,
        embeddings: &[Vec<f32>],
        min_sim: f32,
    ) -> Option<SimilarChunk> {
        let cand_emb = candidate.embedding.as_ref()?;
        let best_sim = embeddings
            .iter()
            .map(|q_emb| cosine_similarity(q_emb, cand_emb))
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))?;

        if best_sim >= min_sim {
            Some(SimilarChunk {
                chunk: candidate.clone(),
                similarity: best_sim,
            })
        } else {
            None
        }
    }

    fn sort_and_truncate(&self, results: &mut Vec<SimilarChunk>, top_k: usize) {
        results.sort_by(|a, b| match a.chunk.source.cmp(&b.chunk.source) {
            std::cmp::Ordering::Equal => b
                .similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal),
            other => other,
        });
        results.dedup_by(|a, b| a.chunk.source == b.chunk.source);

        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(top_k);
    }

    /// Number of chunks in the graph
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    /// Whether the graph is empty
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Clear all chunks
    pub fn clear(&mut self) {
        self.chunks.clear();
    }
}

/// Extract chunks from a parsed file
///
/// Every AST node becomes a chunk. Size limits are handled at embedding
/// time based on the embedding model's constraints.
pub fn chunk_file(parsed_file: Arc<ParsedFile>) -> Vec<SemanticChunk> {
    let mut chunks = Vec::new();
    let root = parsed_file.root_node();
    extract_chunks_recursive(&mut chunks, root, &parsed_file);
    chunks
}

fn extract_chunks_recursive(
    chunks: &mut Vec<SemanticChunk>,
    node: tree_sitter::Node<'_>,
    parsed_file: &Arc<ParsedFile>,
) {
    // Add this node as a chunk
    let chunk = SemanticChunk::from_parsed(parsed_file.clone(), node.start_byte(), node.end_byte());
    chunks.push(chunk);

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_chunks_recursive(chunks, child, parsed_file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::LanguageRegistry;

    fn create_parsed_file(source: &str) -> Arc<ParsedFile> {
        let registry = LanguageRegistry::global();
        let config = registry.get_by_name("rust").unwrap();
        let language = config.language();

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).unwrap();

        let tree = parser.parse(source, None).unwrap();
        let hash = md5::compute(source.as_bytes());

        Arc::new(ParsedFile::new(
            PathBuf::from("/test.rs"),
            source.to_string(),
            tree,
            hash.into(),
        ))
    }

    #[test]
    fn test_chunk_source_parsed() {
        let parsed = create_parsed_file("fn main() {}");
        let source = ChunkSource::parsed(parsed.clone(), 0, 12);

        assert_eq!(source.path(), Some(Path::new("/test.rs")));
        assert_eq!(source.byte_len(), 12);
        assert!(source.is_parsed());
        assert!(!source.is_text());
        assert_eq!(source.content(), Some("fn main() {}"));
    }

    #[test]
    fn test_chunk_source_text() {
        let source = ChunkSource::text("hello world");

        assert_eq!(source.path(), None);
        assert_eq!(source.byte_len(), 11);
        assert!(!source.is_parsed());
        assert!(source.is_text());
        assert_eq!(source.content(), Some("hello world"));
    }

    #[test]
    fn test_chunk_source_node() {
        let parsed = create_parsed_file("fn main() {}");
        let source = ChunkSource::parsed(parsed, 0, 12);

        let node = source.node();
        assert!(node.is_some());
        assert_eq!(node.unwrap().kind(), "function_item");
    }

    #[test]
    fn test_chunk_source_text_has_no_node() {
        let source = ChunkSource::text("fn main() {}");
        assert!(source.node().is_none());
    }

    #[test]
    fn test_semantic_chunk_from_parsed() {
        let parsed = create_parsed_file("fn main() {}");
        let chunk = SemanticChunk::from_parsed(parsed, 0, 12);

        assert_eq!(chunk.content(), Some("fn main() {}"));
        assert_eq!(chunk.path(), Some(Path::new("/test.rs")));
        assert!(chunk.node().is_some());
    }

    #[test]
    fn test_semantic_chunk_from_text() {
        let chunk = SemanticChunk::from_text("search query");

        assert_eq!(chunk.content(), Some("search query"));
        assert_eq!(chunk.path(), None);
        assert!(chunk.node().is_none());
    }

    #[test]
    fn test_semantic_chunk_with_embedding() {
        let chunk = SemanticChunk::from_text("code").with_embedding(vec![1.0, 0.0, 0.0]);

        assert!(chunk.has_embedding());
        assert_eq!(chunk.embedding.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn test_semantic_chunk_similarity() {
        let chunk1 = SemanticChunk::from_text("a").with_embedding(vec![1.0, 0.0, 0.0]);
        let chunk2 = SemanticChunk::from_text("b").with_embedding(vec![1.0, 0.0, 0.0]);

        assert!((chunk1.similarity_to(&chunk2) - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity() {
        assert!((cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 0.0001);
        assert!((cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]) - 0.0).abs() < 0.0001);
        assert_eq!(cosine_similarity(&[1.0], &[1.0, 0.0]), 0.0);
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn test_chunk_graph_add_and_query() {
        let mut graph = ChunkGraph::new();
        let parsed = create_parsed_file("fn a() {} fn b() {}");

        let chunk1 =
            SemanticChunk::from_parsed(parsed.clone(), 0, 9).with_embedding(vec![1.0, 0.0, 0.0]);
        let chunk2 = SemanticChunk::from_parsed(parsed, 10, 19).with_embedding(vec![0.9, 0.1, 0.0]);

        graph.add(chunk1.clone());
        graph.add(chunk2);

        let results = graph.query(SimilarityQuery::chunk(chunk1).min_similarity(0.5));
        assert_eq!(results.len(), 1);
        assert!(results[0].similarity > 0.9);
    }

    #[test]
    fn test_chunk_graph_query_with_text() {
        let mut graph = ChunkGraph::new();
        let parsed = create_parsed_file("fn main() {}");

        graph.add(SemanticChunk::from_parsed(parsed, 0, 12).with_embedding(vec![1.0, 0.0, 0.0]));

        let query_chunk =
            SemanticChunk::from_text("find main").with_embedding(vec![0.95, 0.05, 0.0]);
        let results = graph.query(SimilarityQuery::chunk(query_chunk).min_similarity(0.9));

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_chunk_graph_remove_file() {
        let mut graph = ChunkGraph::new();
        let parsed = create_parsed_file("fn a() {}");

        graph.add(SemanticChunk::from_parsed(parsed, 0, 9).with_embedding(vec![1.0]));
        assert_eq!(graph.len(), 1);

        graph.remove_file(Path::new("/test.rs"));
        assert!(graph.is_empty());
    }

    #[test]
    fn test_query_builder() {
        let q = SimilarityQuery::file("/test.rs")
            .top_k(5)
            .min_similarity(0.9);
        assert_eq!(q.top_k, 5);
        assert_eq!(q.min_similarity, 0.9);
    }

    #[test]
    fn test_chunk_file_creates_chunks_for_all_nodes() {
        let parsed = create_parsed_file("fn main() {} fn other() {}");
        let chunks = chunk_file(parsed);

        // Should create chunks for every AST node
        assert!(!chunks.is_empty());
        // At minimum: source_file, two function_items, their children
        assert!(chunks.len() > 2);
    }
}
