//! tarpc service definition for the tree-sitter index RPC
//!
//! This defines the RPC interface between leader and client processes.

use crate::query::types::{
    DuplicateCluster, IndexStatusInfo, QueryError, QueryMatch, SimilarChunkResult,
};
use std::path::PathBuf;

/// tarpc service trait for tree-sitter index queries
///
/// The leader implements this trait to handle queries from clients.
/// tarpc generates both client and server code from this definition.
#[tarpc::service]
pub trait IndexService {
    /// Find all duplicate code clusters across the entire project
    ///
    /// Returns clusters of chunks that are highly similar to each other.
    /// Each cluster represents code that appears in multiple places.
    async fn find_all_duplicates(
        min_similarity: f32,
        min_chunk_bytes: usize,
    ) -> Result<Vec<DuplicateCluster>, QueryError>;

    /// Find duplicates for chunks in a specific file
    ///
    /// Returns chunks from other files that are similar to chunks in the given file.
    async fn find_duplicates_in_file(
        file: PathBuf,
        min_similarity: f32,
    ) -> Result<Vec<SimilarChunkResult>, QueryError>;

    /// Semantic search - find chunks similar to the given text
    ///
    /// Embeds the query text and finds the most similar chunks in the index.
    async fn semantic_search(
        text: String,
        top_k: usize,
        min_similarity: f32,
    ) -> Result<Vec<SimilarChunkResult>, QueryError>;

    /// Execute a tree-sitter query and return matches
    ///
    /// The query is an S-expression pattern (e.g., "(function_item name: (identifier) @name)").
    /// If files is None, searches all files. If language is provided, filters to that language.
    async fn tree_sitter_query(
        query: String,
        files: Option<Vec<PathBuf>>,
        language: Option<String>,
    ) -> Result<Vec<QueryMatch>, QueryError>;

    /// List all files in the index
    async fn list_files() -> Vec<PathBuf>;

    /// Get current index status
    async fn status() -> IndexStatusInfo;

    /// Invalidate a file (force re-parse and re-embed)
    async fn invalidate_file(file: PathBuf) -> Result<(), QueryError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_trait_exists() {
        // This test verifies the service trait compiles correctly.
        // The actual functionality is tested in integration tests.
        fn _assert_service_trait<T: IndexService>() {}
    }
}
