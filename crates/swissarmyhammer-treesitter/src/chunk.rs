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

    /// Extract parsed fields: (parsed_file, start_byte, end_byte).
    /// Returns None for Text variant. Used to reduce destructuring duplication.
    fn parsed_fields(&self) -> Option<(&Arc<ParsedFile>, usize, usize)> {
        match self {
            Self::Parsed {
                start_byte,
                end_byte,
                parsed_file,
            } => Some((parsed_file, *start_byte, *end_byte)),
            Self::Text(_) => None,
        }
    }

    /// File path (None for text sources)
    pub fn path(&self) -> Option<&Path> {
        self.parsed_fields().map(|(pf, _, _)| pf.path.as_path())
    }

    /// Byte length of this chunk
    pub fn byte_len(&self) -> usize {
        match self.parsed_fields() {
            Some((_, start, end)) => end.saturating_sub(start),
            None => match self {
                Self::Text(s) => s.len(),
                _ => 0,
            },
        }
    }

    /// Tree-sitter node for this chunk (None for text sources)
    pub fn node(&self) -> Option<tree_sitter::Node<'_>> {
        self.parsed_fields()
            .and_then(|(pf, start, end)| pf.tree.root_node().descendant_for_byte_range(start, end))
    }

    /// Parent node of this chunk's node (None for text sources)
    pub fn parent_node(&self) -> Option<tree_sitter::Node<'_>> {
        self.node().and_then(|n| n.parent())
    }

    /// Text content of this chunk
    pub fn content(&self) -> Option<&str> {
        match self.parsed_fields() {
            Some((pf, start, end)) => pf.get_text(start, end),
            None => match self {
                Self::Text(s) => Some(s.as_str()),
                _ => None,
            },
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

    /// Extract comparison key for Parsed variant: (path, start_byte, end_byte).
    /// Returns None for Text variant.
    fn parsed_key(&self) -> Option<(&Path, usize, usize)> {
        self.parsed_fields()
            .map(|(pf, start, end)| (pf.path.as_path(), start, end))
    }
}

impl PartialEq for ChunkSource {
    fn eq(&self, other: &Self) -> bool {
        match (self.parsed_key(), other.parsed_key()) {
            (Some(k1), Some(k2)) => k1 == k2,
            (None, None) => match (self, other) {
                (Self::Text(t1), Self::Text(t2)) => t1 == t2,
                _ => false,
            },
            _ => false,
        }
    }
}

impl Eq for ChunkSource {}

impl std::hash::Hash for ChunkSource {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if let Some((path, start, end)) = self.parsed_key() {
            0u8.hash(state);
            path.hash(state);
            start.hash(state);
            end.hash(state);
        } else if let Self::Text(s) = self {
            1u8.hash(state);
            s.hash(state);
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
        match (self.parsed_key(), other.parsed_key()) {
            (Some(k1), Some(k2)) => k1.cmp(&k2),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => match (self, other) {
                (Self::Text(t1), Self::Text(t2)) => t1.cmp(t2),
                _ => std::cmp::Ordering::Equal,
            },
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

    /// Get a human-readable symbol path for this chunk.
    ///
    /// Returns something like "file.rs::StructName::method_name" for nested definitions,
    /// or "file.rs::function_name" for top-level items.
    pub fn symbol_path(&self) -> String {
        let file_name = self.file_name_or_default();
        let Some(node) = self.node() else {
            return file_name;
        };

        // Get source bytes for utf8_text extraction
        let source_bytes = self.source_bytes();
        let names = collect_symbol_names(node, source_bytes);
        if names.is_empty() {
            format!("{}::{}", file_name, node.kind())
        } else {
            format!("{}::{}", file_name, names.join("::"))
        }
    }

    /// Get the source bytes for this chunk's file.
    /// Returns empty slice for text-only sources.
    fn source_bytes(&self) -> &[u8] {
        match &self.source {
            ChunkSource::Parsed { parsed_file, .. } => parsed_file.source.as_bytes(),
            ChunkSource::Text(_) => &[],
        }
    }

    fn file_name_or_default(&self) -> String {
        self.path()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "<text>".to_string())
    }

    /// Cosine similarity with another chunk (0.0 if either lacks embedding)
    pub fn similarity_to(&self, other: &SemanticChunk) -> f32 {
        match (&self.embedding, &other.embedding) {
            (Some(a), Some(b)) => cosine_similarity(a, b),
            _ => 0.0,
        }
    }
}

pub use model_embedding::cosine_similarity;

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
    /// Create a new query with the given source and default settings.
    fn new(source: QuerySource) -> Self {
        Self {
            source,
            top_k: DEFAULT_TOP_K,
            min_similarity: DEFAULT_MIN_SIMILARITY,
        }
    }

    /// Query by a single chunk
    pub fn chunk(chunk: SemanticChunk) -> Self {
        Self::new(QuerySource::Chunk(chunk))
    }

    /// Query by multiple chunks
    pub fn chunks(chunks: Vec<SemanticChunk>) -> Self {
        Self::new(QuerySource::Chunks(chunks))
    }

    /// Query by file path
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::new(QuerySource::File(path.into()))
    }

    /// Query by raw embedding vector
    pub fn embedding(embedding: Vec<f32>) -> Self {
        Self::new(QuerySource::Embedding(embedding))
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
        // Sort by similarity descending first
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Dedup keeping first occurrence (highest similarity) using a seen set
        let mut seen = std::collections::HashSet::new();
        results.retain(|r| seen.insert(r.chunk.source.clone()));

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

/// Node kinds that represent meaningful semantic units worth embedding.
///
/// These are the "definition" level constructs across languages - functions,
/// classes, structs, etc. We skip low-level nodes like identifiers, literals,
/// and operators since they don't carry standalone semantic meaning.
const EMBEDDABLE_NODE_KINDS: &[&str] = &[
    // Rust
    "function_item",
    "impl_item",
    "struct_item",
    "enum_item",
    "trait_item",
    "mod_item",
    "macro_definition",
    "const_item",
    "static_item",
    "type_item",
    // Python
    "function_definition",
    "class_definition",
    "decorated_definition",
    // JavaScript/TypeScript
    "function_declaration",
    "function_expression",
    "arrow_function",
    "class_declaration",
    "method_definition",
    "generator_function_declaration",
    "export_statement",
    // Go
    "function_declaration",
    "method_declaration",
    "type_declaration",
    "type_spec",
    // Java
    "method_declaration",
    "class_declaration",
    "interface_declaration",
    "enum_declaration",
    "constructor_declaration",
    // C/C++
    "function_definition",
    "struct_specifier",
    "class_specifier",
    "enum_specifier",
    "namespace_definition",
    // Ruby
    "method",
    "class",
    "module",
    "singleton_method",
    // PHP
    "function_definition",
    "method_declaration",
    "class_declaration",
    "interface_declaration",
    "trait_declaration",
    // Swift
    "function_declaration",
    "class_declaration",
    "struct_declaration",
    "enum_declaration",
    "protocol_declaration",
    // Kotlin
    "function_declaration",
    "class_declaration",
    "object_declaration",
    // Scala
    "function_definition",
    "class_definition",
    "object_definition",
    "trait_definition",
    // Elixir
    "call", // def, defp, defmodule are calls in Elixir's AST
    // Haskell
    "function",
    "type_signature",
    // Lua
    "function_declaration",
    "local_function",
    // Bash
    "function_definition",
    // SQL
    "create_function_statement",
    "create_procedure",
    "create_table_statement",
    "create_view_statement",
];

/// Check if a node kind should be embedded.
fn is_embeddable_kind(kind: &str) -> bool {
    EMBEDDABLE_NODE_KINDS.contains(&kind)
}

/// Container node kinds that provide naming context (impl, class, module, etc.)
const CONTAINER_KINDS: &[&str] = &[
    "impl_item",
    "class_definition",
    "class_declaration",
    "module",
    "mod_item",
    "namespace_definition",
    "interface_declaration",
    "trait_item",
];

/// Collect symbol names from a node up through its ancestors.
/// Returns names in order from outermost to innermost (e.g., ["Struct", "method"]).
fn collect_symbol_names(node: tree_sitter::Node<'_>, source: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    collect_names_recursive(node, source, &mut names);
    names.reverse();
    names
}

fn collect_names_recursive(node: tree_sitter::Node<'_>, source: &[u8], names: &mut Vec<String>) {
    if let Some(name) = extract_node_name(node, source) {
        names.push(name);
    }
    // Walk up to find parent containers, skipping intermediate nodes like declaration_list
    let mut current = node;
    while let Some(parent) = current.parent() {
        let pk = parent.kind();
        if is_embeddable_kind(pk) || CONTAINER_KINDS.contains(&pk) {
            collect_names_recursive(parent, source, names);
            break;
        }
        current = parent;
    }
}

/// Extract the name identifier from a node.
fn extract_node_name(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<String> {
    // Try common name fields
    for field in &["name", "identifier", "declarator"] {
        if let Some(name) = try_extract_name_field(node, field, source) {
            return Some(name);
        }
    }
    // Special case for impl blocks
    if node.kind() == "impl_item" {
        return extract_impl_type_name(node, source);
    }
    None
}

/// Validate that text is a simple identifier suitable for symbol paths.
/// Rejects text with newlines or exceeding the max length.
fn is_valid_symbol_text(text: &str, max_len: usize) -> bool {
    !text.contains('\n') && text.len() < max_len
}

fn try_extract_name_field(
    node: tree_sitter::Node<'_>,
    field: &str,
    source: &[u8],
) -> Option<String> {
    let name_node = node.child_by_field_name(field)?;
    let text = name_node.utf8_text(source).ok()?;
    // Only accept simple identifiers (no whitespace, reasonable length)
    if !text.contains(' ') && is_valid_symbol_text(text, 100) {
        Some(text.to_string())
    } else {
        None
    }
}

fn extract_impl_type_name(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<String> {
    let type_node = node.child_by_field_name("type")?;
    let text = type_node.utf8_text(source).ok()?;
    if is_valid_symbol_text(text, 50) {
        Some(format!("impl {}", text))
    } else {
        None
    }
}

/// Extract chunks from a parsed file.
///
/// Only extracts nodes whose kind is in the allowlist of meaningful
/// semantic units (functions, classes, structs, etc.).
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
    // Only add this node as a chunk if it's a meaningful semantic unit
    if is_embeddable_kind(node.kind()) {
        let chunk =
            SemanticChunk::from_parsed(parsed_file.clone(), node.start_byte(), node.end_byte());
        chunks.push(chunk);
    }

    // Always recurse into children to find nested definitions
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
    fn test_chunk_file_filters_to_meaningful_nodes() {
        let parsed = create_parsed_file("fn main() {} fn other() {}");
        let chunks = chunk_file(parsed);

        // Should only create chunks for function_item nodes, not identifiers etc.
        assert_eq!(chunks.len(), 2, "Expected 2 function_item chunks");

        // Verify they're actually function_items
        for chunk in &chunks {
            let node = chunk.node().expect("chunk should have node");
            assert_eq!(node.kind(), "function_item");
        }
    }

    #[test]
    fn test_is_embeddable_kind_rust() {
        assert!(is_embeddable_kind("function_item"));
        assert!(is_embeddable_kind("impl_item"));
        assert!(is_embeddable_kind("struct_item"));
        assert!(is_embeddable_kind("enum_item"));
        assert!(is_embeddable_kind("trait_item"));
        assert!(!is_embeddable_kind("identifier"));
        assert!(!is_embeddable_kind("string_literal"));
        assert!(!is_embeddable_kind("source_file"));
    }

    #[test]
    fn test_is_embeddable_kind_python() {
        assert!(is_embeddable_kind("function_definition"));
        assert!(is_embeddable_kind("class_definition"));
        assert!(!is_embeddable_kind("expression_statement"));
    }

    #[test]
    fn test_is_embeddable_kind_javascript() {
        assert!(is_embeddable_kind("function_declaration"));
        assert!(is_embeddable_kind("arrow_function"));
        assert!(is_embeddable_kind("class_declaration"));
        assert!(!is_embeddable_kind("call_expression"));
    }

    #[test]
    fn test_chunk_file_finds_nested_definitions() {
        // impl block containing methods
        let source = r#"
impl Foo {
    fn bar() {}
    fn baz() {}
}
"#;
        let parsed = create_parsed_file(source);
        let chunks = chunk_file(parsed);

        // Should find: impl_item + 2 function_items
        assert_eq!(chunks.len(), 3, "Expected impl_item and 2 function_items");

        let kinds: Vec<_> = chunks.iter().map(|c| c.node().unwrap().kind()).collect();
        assert!(kinds.contains(&"impl_item"));
        assert_eq!(kinds.iter().filter(|k| **k == "function_item").count(), 2);
    }

    #[test]
    fn test_symbol_path_function() {
        let parsed = create_parsed_file("fn main() {}");
        let chunk = SemanticChunk::from_parsed(parsed, 0, 12);
        let path = chunk.symbol_path();
        assert!(path.contains("test.rs"), "Should contain filename");
        assert!(path.contains("main"), "Should contain function name");
    }

    #[test]
    fn test_symbol_path_text_chunk() {
        let chunk = SemanticChunk::from_text("search query");
        assert_eq!(chunk.symbol_path(), "<text>");
    }

    #[test]
    fn test_symbol_path_nested_method() {
        let source = "impl Foo { fn bar() {} }";
        let parsed = create_parsed_file(source);
        // Find the function_item within the impl
        let chunks = chunk_file(parsed);
        let method_chunk = chunks
            .iter()
            .find(|c| c.node().map(|n| n.kind()) == Some("function_item"));
        assert!(method_chunk.is_some());
        let path = method_chunk.unwrap().symbol_path();
        // Should have both the impl type AND the method name
        assert!(path.contains("Foo"), "Should contain impl type: {}", path);
        assert!(path.contains("bar"), "Should contain method name: {}", path);
        // Full path should be like "test.rs::impl Foo::bar"
        assert!(
            path.contains("impl Foo::bar"),
            "Should have full path: {}",
            path
        );
    }

    // === Parsed variant byte-range extraction edge cases ===

    #[test]
    fn test_chunk_source_parsed_empty_byte_range() {
        // When start == end, byte_len should be 0
        let parsed = create_parsed_file("fn main() {}");
        let source = ChunkSource::parsed(parsed, 5, 5);
        assert_eq!(source.byte_len(), 0);
        // content() at an empty range should return empty string (valid range)
        assert_eq!(source.content(), Some(""));
    }

    #[test]
    fn test_chunk_source_parsed_inverted_byte_range() {
        // When start > end, byte_len saturates to 0
        let parsed = create_parsed_file("fn main() {}");
        let source = ChunkSource::parsed(parsed, 10, 5);
        assert_eq!(source.byte_len(), 0);
        // content() with inverted range returns None (start > end is invalid)
        assert_eq!(source.content(), None);
    }

    #[test]
    fn test_chunk_source_parsed_out_of_bounds_byte_range() {
        // When end exceeds source length, content() returns None
        let parsed = create_parsed_file("fn main() {}"); // 12 bytes
        let source = ChunkSource::parsed(parsed, 0, 999);
        // byte_len reports the arithmetic length even if out of bounds
        assert_eq!(source.byte_len(), 999);
        // content() returns None because end > source.len()
        assert_eq!(source.content(), None);
    }

    #[test]
    fn test_chunk_source_parsed_parent_node() {
        // function_item inside an impl block should have a parent
        let source = "impl Foo { fn bar() {} }";
        let parsed = create_parsed_file(source);
        // Byte range covering the fn bar() {} part
        let chunks = chunk_file(parsed);
        let method_chunk = chunks
            .iter()
            .find(|c| c.node().map(|n| n.kind()) == Some("function_item"))
            .unwrap();
        // The method's parent_node should exist (it's inside impl_item's declaration_list)
        let parent = method_chunk.source.parent_node();
        assert!(
            parent.is_some(),
            "Method inside impl should have a parent node"
        );
    }

    #[test]
    fn test_chunk_source_text_parent_node_is_none() {
        let source = ChunkSource::text("some code");
        assert!(source.parent_node().is_none());
    }

    // === ChunkSource equality and ordering ===

    #[test]
    fn test_chunk_source_equality_same_parsed() {
        let parsed = create_parsed_file("fn main() {}");
        let s1 = ChunkSource::parsed(parsed.clone(), 0, 12);
        let s2 = ChunkSource::parsed(parsed, 0, 12);
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_chunk_source_equality_different_ranges() {
        let parsed = create_parsed_file("fn main() {}");
        let s1 = ChunkSource::parsed(parsed.clone(), 0, 6);
        let s2 = ChunkSource::parsed(parsed, 6, 12);
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_chunk_source_equality_parsed_vs_text() {
        let parsed = create_parsed_file("fn main() {}");
        let ps = ChunkSource::parsed(parsed, 0, 12);
        let ts = ChunkSource::text("fn main() {}");
        // Parsed and text sources are never equal even with same content
        assert_ne!(ps, ts);
    }

    #[test]
    fn test_chunk_source_equality_same_text() {
        let s1 = ChunkSource::text("hello");
        let s2 = ChunkSource::text("hello");
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_chunk_source_equality_different_text() {
        let s1 = ChunkSource::text("hello");
        let s2 = ChunkSource::text("world");
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_chunk_source_ordering_parsed_before_text() {
        // Parsed sources sort before Text sources
        let parsed = create_parsed_file("fn main() {}");
        let ps = ChunkSource::parsed(parsed, 0, 12);
        let ts = ChunkSource::text("fn main() {}");
        assert!(ps < ts, "Parsed should sort before text");
    }

    #[test]
    fn test_chunk_source_ordering_text_vs_text() {
        let s1 = ChunkSource::text("aardvark");
        let s2 = ChunkSource::text("zebra");
        assert!(s1 < s2);
        assert!(s2 > s1);
    }

    #[test]
    fn test_chunk_source_hash_consistency() {
        // Same source should produce consistent hash results (used in HashSets)
        use std::collections::HashSet;
        let parsed = create_parsed_file("fn main() {}");
        let s1 = ChunkSource::parsed(parsed.clone(), 0, 12);
        let s2 = ChunkSource::parsed(parsed, 0, 12);

        let mut set = HashSet::new();
        set.insert(s1);
        // Inserting an equal source should not grow the set
        assert!(!set.insert(s2));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_chunk_source_hash_text_in_set() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ChunkSource::text("hello"));
        assert!(!set.insert(ChunkSource::text("hello")));
        assert!(set.insert(ChunkSource::text("world")));
        assert_eq!(set.len(), 2);
    }

    // === ChunkGraph additional methods ===

    #[test]
    fn test_chunk_graph_chunks_for_file() {
        let mut graph = ChunkGraph::new();
        let parsed_a = create_parsed_file("fn a() {}");
        // Create a different parsed file on a different path
        let registry = crate::language::LanguageRegistry::global();
        let config = registry.get_by_name("rust").unwrap();
        let language = config.language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).unwrap();
        let source_b = "fn b() {}";
        let tree_b = parser.parse(source_b, None).unwrap();
        let hash_b = md5::compute(source_b.as_bytes());
        let parsed_b = Arc::new(ParsedFile::new(
            PathBuf::from("/other.rs"),
            source_b.to_string(),
            tree_b,
            hash_b.into(),
        ));

        graph.add(SemanticChunk::from_parsed(parsed_a, 0, 9));
        graph.add(SemanticChunk::from_parsed(parsed_b, 0, 9));
        graph.add(SemanticChunk::from_text("unrelated"));

        // chunks_for_file should return only chunks for the requested path
        let chunks = graph.chunks_for_file(Path::new("/test.rs"));
        assert_eq!(chunks.len(), 1);

        let chunks_b = graph.chunks_for_file(Path::new("/other.rs"));
        assert_eq!(chunks_b.len(), 1);

        // Text chunks have no path, so they're not returned for any file
        let chunks_none = graph.chunks_for_file(Path::new("/nonexistent.rs"));
        assert_eq!(chunks_none.len(), 0);
    }

    #[test]
    fn test_chunk_graph_clear() {
        let mut graph = ChunkGraph::new();
        let parsed = create_parsed_file("fn main() {}");
        graph.add(SemanticChunk::from_parsed(parsed, 0, 12));
        graph.add(SemanticChunk::from_text("some text"));
        assert_eq!(graph.len(), 2);

        graph.clear();
        assert!(graph.is_empty());
        assert_eq!(graph.len(), 0);
    }

    #[test]
    fn test_chunk_graph_chunks_accessor() {
        let mut graph = ChunkGraph::new();
        let parsed = create_parsed_file("fn main() {}");
        graph.add(SemanticChunk::from_parsed(parsed, 0, 12));
        graph.add(SemanticChunk::from_text("search"));

        let all_chunks = graph.chunks();
        assert_eq!(all_chunks.len(), 2);
    }

    // === SimilarityQuery variants ===

    #[test]
    fn test_similarity_query_chunks_variant() {
        // Test the Chunks (plural) query variant
        let mut graph = ChunkGraph::new();
        let parsed = create_parsed_file("fn main() {} fn other() {}");
        graph.add(
            SemanticChunk::from_parsed(parsed.clone(), 0, 12).with_embedding(vec![1.0, 0.0, 0.0]),
        );
        graph.add(SemanticChunk::from_parsed(parsed, 13, 26).with_embedding(vec![0.5, 0.5, 0.0]));

        // Query using multiple source chunks
        let query_chunk1 = SemanticChunk::from_text("main").with_embedding(vec![1.0, 0.0, 0.0]);
        let query_chunk2 = SemanticChunk::from_text("other").with_embedding(vec![0.5, 0.5, 0.0]);
        let results = graph
            .query(SimilarityQuery::chunks(vec![query_chunk1, query_chunk2]).min_similarity(0.9));
        // Should find both chunks that are similar to either query
        assert!(!results.is_empty());
    }

    #[test]
    fn test_similarity_query_embedding_variant() {
        // Test the Embedding (raw vector) query variant
        let mut graph = ChunkGraph::new();
        let parsed = create_parsed_file("fn main() {}");
        graph.add(SemanticChunk::from_parsed(parsed, 0, 12).with_embedding(vec![1.0, 0.0, 0.0]));

        let results =
            graph.query(SimilarityQuery::embedding(vec![1.0, 0.0, 0.0]).min_similarity(0.9));
        assert_eq!(results.len(), 1);
        assert!((results[0].similarity - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_similarity_query_file_variant() {
        // Test the File path query variant
        let mut graph = ChunkGraph::new();
        let parsed = create_parsed_file("fn main() {}");

        // Add chunk from /test.rs (the default in create_parsed_file)
        let chunk =
            SemanticChunk::from_parsed(parsed.clone(), 0, 12).with_embedding(vec![1.0, 0.0, 0.0]);
        graph.add(chunk.clone());

        // Add a non-similar chunk also from /test.rs
        graph.add(SemanticChunk::from_parsed(parsed, 0, 6).with_embedding(vec![0.0, 1.0, 0.0]));

        // Query using the file path — this uses all chunks from /test.rs as query source
        // and excludes them from results, so the result should be empty
        let results = graph.query(SimilarityQuery::file("/test.rs").min_similarity(0.1));
        // All chunks from /test.rs are excluded from results
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_similarity_query_no_embeddings_returns_empty() {
        // Query chunk without embedding returns no results
        let mut graph = ChunkGraph::new();
        let parsed = create_parsed_file("fn main() {}");
        // Add chunk WITHOUT embedding
        graph.add(SemanticChunk::from_parsed(parsed, 0, 12));

        // Query chunk also WITHOUT embedding
        let query_chunk = SemanticChunk::from_text("main");
        let results = graph.query(SimilarityQuery::chunk(query_chunk).min_similarity(0.5));
        assert!(results.is_empty(), "No results when query has no embedding");
    }

    #[test]
    fn test_semantic_chunk_similarity_no_embedding() {
        // similarity_to() returns 0.0 when either chunk lacks embedding
        let chunk_with = SemanticChunk::from_text("a").with_embedding(vec![1.0, 0.0]);
        let chunk_without = SemanticChunk::from_text("b");

        assert_eq!(chunk_with.similarity_to(&chunk_without), 0.0);
        assert_eq!(chunk_without.similarity_to(&chunk_with), 0.0);
        assert_eq!(chunk_without.similarity_to(&chunk_without), 0.0);
    }

    // === chunk_file edge cases ===

    #[test]
    fn test_chunk_file_empty_source() {
        // Empty source produces no chunks
        let parsed = create_parsed_file("");
        let chunks = chunk_file(parsed);
        assert!(chunks.is_empty(), "Empty source should produce no chunks");
    }

    #[test]
    fn test_chunk_file_no_embeddable_nodes() {
        // Source with only comments/whitespace — no function or struct definitions
        let source = "// just a comment\n";
        let parsed = create_parsed_file(source);
        let chunks = chunk_file(parsed);
        assert!(
            chunks.is_empty(),
            "Comment-only source should produce no chunks"
        );
    }

    #[test]
    fn test_chunk_file_byte_ranges_are_valid() {
        // Each chunk's byte range should be a valid slice of the source
        let source = "fn alpha() {} fn beta() { let x = 1; }";
        let parsed = create_parsed_file(source);
        let chunks = chunk_file(parsed);

        assert!(!chunks.is_empty());
        for chunk in &chunks {
            // content() must not be None — the byte range must be valid
            assert!(
                chunk.content().is_some(),
                "Chunk byte range should be valid: {:?}",
                chunk.source
            );
            // byte_len must match actual content length
            let content_len = chunk.content().unwrap().len();
            assert_eq!(
                chunk.byte_len(),
                content_len,
                "byte_len should match content length"
            );
        }
    }

    #[test]
    fn test_chunk_file_with_struct_and_impl() {
        let source = r#"struct Foo {
    x: i32,
}

impl Foo {
    fn new(x: i32) -> Self {
        Foo { x }
    }
}
"#;
        let parsed = create_parsed_file(source);
        let chunks = chunk_file(parsed);

        let kinds: Vec<_> = chunks.iter().map(|c| c.node().unwrap().kind()).collect();

        // Should find struct_item, impl_item, and function_item
        assert!(
            kinds.contains(&"struct_item"),
            "Should find struct: {:?}",
            kinds
        );
        assert!(
            kinds.contains(&"impl_item"),
            "Should find impl: {:?}",
            kinds
        );
        assert!(
            kinds.contains(&"function_item"),
            "Should find fn: {:?}",
            kinds
        );
    }

    #[test]
    fn test_chunk_source_parsed_node_at_exact_function_range() {
        // Verify node() returns the correct tree-sitter node for a known byte range
        let source = "fn foo() {} fn bar() {}";
        let parsed = create_parsed_file(source);

        // fn foo() {} is bytes 0..11
        let s = ChunkSource::parsed(parsed.clone(), 0, 11);
        let node = s.node().expect("Should find a node");
        assert_eq!(node.kind(), "function_item");

        // fn bar() {} is bytes 12..23
        let s2 = ChunkSource::parsed(parsed, 12, 23);
        let node2 = s2.node().expect("Should find a node");
        assert_eq!(node2.kind(), "function_item");
    }
}
