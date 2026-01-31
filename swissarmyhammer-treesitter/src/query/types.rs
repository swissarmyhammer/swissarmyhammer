//! Serializable types for the query protocol
//!
//! These types are used to communicate between leader and client processes.
//! They are designed to be serializable (no `Arc<Tree>` or other non-Send types).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Serializable chunk result (no AST references)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkResult {
    /// File path containing this chunk
    pub file: PathBuf,
    /// Text content of the chunk
    pub text: String,
    /// Start byte offset in file
    pub start_byte: usize,
    /// End byte offset in file
    pub end_byte: usize,
    /// Start line number (0-indexed)
    pub start_line: usize,
    /// End line number (0-indexed)
    pub end_line: usize,
}

/// Similarity search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarChunkResult {
    /// The matched chunk
    pub chunk: ChunkResult,
    /// Cosine similarity score (0.0 to 1.0)
    pub similarity: f32,
}

/// A cluster of duplicate code chunks
///
/// All chunks in a cluster are highly similar to each other.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateCluster {
    /// All chunks that are duplicates of each other
    pub chunks: Vec<ChunkResult>,
    /// Average pairwise similarity within the cluster
    pub avg_similarity: f32,
}

/// Tree-sitter query match result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryMatch {
    /// File containing the match
    pub file: PathBuf,
    /// Captures from this match
    pub captures: Vec<Capture>,
}

/// A single capture from a tree-sitter query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capture {
    /// Capture name (e.g., "name" from @name)
    pub name: String,
    /// Node kind (e.g., "identifier", "function_item")
    pub kind: String,
    /// Captured text
    pub text: String,
    /// Start byte offset
    pub start_byte: usize,
    /// End byte offset
    pub end_byte: usize,
    /// Start line (0-indexed)
    pub start_line: usize,
    /// End line (0-indexed)
    pub end_line: usize,
}

/// Index status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatusInfo {
    /// Total files discovered
    pub files_total: usize,
    /// Files successfully indexed
    pub files_indexed: usize,
    /// Files with embeddings computed
    pub files_embedded: usize,
    /// Whether the index is ready for queries
    pub is_ready: bool,
    /// Root path being indexed
    pub root_path: PathBuf,
}

/// Error response from the leader
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryError {
    /// Error message
    pub message: String,
    /// Error kind for programmatic handling
    pub kind: QueryErrorKind,
}

/// Kinds of query errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryErrorKind {
    /// Index not ready (still building)
    NotReady,
    /// File not found in index
    FileNotFound,
    /// Invalid query syntax
    InvalidQuery,
    /// Embedding model error
    EmbeddingError,
    /// Internal error
    Internal,
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for QueryError {}

/// Check if the index is ready, returning `QueryError::NotReady` if not.
///
/// This is a convenience function to avoid duplicating the readiness check pattern
/// across multiple query implementations.
///
/// # Example
///
/// ```ignore
/// let ctx = context.read().await;
/// check_ready(ctx.status().is_complete())?;
/// ```
pub fn check_ready(is_complete: bool) -> Result<(), QueryError> {
    if !is_complete {
        return Err(QueryError::not_ready());
    }
    Ok(())
}

impl QueryError {
    /// Create a "not ready" error
    pub fn not_ready() -> Self {
        Self {
            message: "Index not ready".to_string(),
            kind: QueryErrorKind::NotReady,
        }
    }

    /// Create a "file not found" error
    pub fn file_not_found(path: &std::path::Path) -> Self {
        Self {
            message: format!("File not found in index: {}", path.display()),
            kind: QueryErrorKind::FileNotFound,
        }
    }

    /// Create an "invalid query" error
    pub fn invalid_query(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            kind: QueryErrorKind::InvalidQuery,
        }
    }

    /// Create an "embedding error"
    pub fn embedding_error(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            kind: QueryErrorKind::EmbeddingError,
        }
    }

    /// Create an "internal error"
    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            kind: QueryErrorKind::Internal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_check_ready_when_complete() {
        let result = check_ready(true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_ready_when_not_complete() {
        let result = check_ready(false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err.kind, QueryErrorKind::NotReady));
    }

    #[test]
    fn test_query_error_not_ready() {
        let err = QueryError::not_ready();
        assert_eq!(err.message, "Index not ready");
        assert!(matches!(err.kind, QueryErrorKind::NotReady));
    }

    #[test]
    fn test_query_error_file_not_found() {
        let err = QueryError::file_not_found(Path::new("/some/path.rs"));
        assert!(err.message.contains("/some/path.rs"));
        assert!(matches!(err.kind, QueryErrorKind::FileNotFound));
    }

    #[test]
    fn test_query_error_invalid_query() {
        let err = QueryError::invalid_query("bad syntax");
        assert_eq!(err.message, "bad syntax");
        assert!(matches!(err.kind, QueryErrorKind::InvalidQuery));
    }

    #[test]
    fn test_query_error_embedding_error() {
        let err = QueryError::embedding_error("model failed");
        assert_eq!(err.message, "model failed");
        assert!(matches!(err.kind, QueryErrorKind::EmbeddingError));
    }

    #[test]
    fn test_query_error_internal() {
        let err = QueryError::internal("something broke");
        assert_eq!(err.message, "something broke");
        assert!(matches!(err.kind, QueryErrorKind::Internal));
    }

    #[test]
    fn test_query_error_display() {
        let err = QueryError::not_ready();
        assert_eq!(format!("{}", err), "Index not ready");
    }

    #[test]
    fn test_query_error_is_error_trait() {
        let err = QueryError::internal("test");
        // Verify it implements Error trait by using it as one
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_chunk_result_serialization() {
        let chunk = ChunkResult {
            file: PathBuf::from("/test.rs"),
            text: "fn main() {}".to_string(),
            start_byte: 0,
            end_byte: 12,
            start_line: 0,
            end_line: 0,
        };

        let json = serde_json::to_string(&chunk).unwrap();
        let deserialized: ChunkResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.file, chunk.file);
        assert_eq!(deserialized.text, chunk.text);
    }

    #[test]
    fn test_similar_chunk_result_serialization() {
        let result = SimilarChunkResult {
            chunk: ChunkResult {
                file: PathBuf::from("/test.rs"),
                text: "code".to_string(),
                start_byte: 0,
                end_byte: 4,
                start_line: 0,
                end_line: 0,
            },
            similarity: 0.95,
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SimilarChunkResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.similarity, 0.95);
    }

    #[test]
    fn test_duplicate_cluster_serialization() {
        let cluster = DuplicateCluster {
            chunks: vec![
                ChunkResult {
                    file: PathBuf::from("/a.rs"),
                    text: "dup".to_string(),
                    start_byte: 0,
                    end_byte: 3,
                    start_line: 0,
                    end_line: 0,
                },
                ChunkResult {
                    file: PathBuf::from("/b.rs"),
                    text: "dup".to_string(),
                    start_byte: 0,
                    end_byte: 3,
                    start_line: 0,
                    end_line: 0,
                },
            ],
            avg_similarity: 0.98,
        };

        let json = serde_json::to_string(&cluster).unwrap();
        let deserialized: DuplicateCluster = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.chunks.len(), 2);
        assert_eq!(deserialized.avg_similarity, 0.98);
    }

    #[test]
    fn test_query_match_serialization() {
        let match_result = QueryMatch {
            file: PathBuf::from("/test.rs"),
            captures: vec![Capture {
                name: "name".to_string(),
                kind: "identifier".to_string(),
                text: "main".to_string(),
                start_byte: 3,
                end_byte: 7,
                start_line: 0,
                end_line: 0,
            }],
        };

        let json = serde_json::to_string(&match_result).unwrap();
        let deserialized: QueryMatch = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.captures.len(), 1);
        assert_eq!(deserialized.captures[0].name, "name");
    }

    #[test]
    fn test_index_status_info_serialization() {
        let status = IndexStatusInfo {
            files_total: 100,
            files_indexed: 95,
            files_embedded: 90,
            is_ready: true,
            root_path: PathBuf::from("/project"),
        };

        let json = serde_json::to_string(&status).unwrap();
        let deserialized: IndexStatusInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.files_total, 100);
        assert!(deserialized.is_ready);
    }
}
