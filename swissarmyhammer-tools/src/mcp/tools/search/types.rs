//! Request and response types for search MCP operations

use serde::{Deserialize, Serialize};

/// Request to index files for semantic search
///
/// # Examples
///
/// Index all Rust files:
/// ```ignore
/// SearchIndexRequest {
///     patterns: vec!["**/*.rs".to_string()],
///     force: false,
/// }
/// ```
///
/// Force re-index specific files:
/// ```ignore
/// SearchIndexRequest {
///     patterns: vec!["src/main.rs".to_string(), "src/lib.rs".to_string()],
///     force: true,
/// }
/// ```
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SearchIndexRequest {
    /// Glob patterns or files to index (supports both "**/*.rs" and expanded file lists)
    pub patterns: Vec<String>,
    /// Force re-indexing of all files
    #[serde(default)]
    pub force: bool,
}

/// Response from indexing files for semantic search
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SearchIndexResponse {
    /// Success message summary
    pub message: String,
    /// Number of files successfully indexed
    pub indexed_files: usize,
    /// Number of files skipped (no changes detected)
    pub skipped_files: usize,
    /// Total number of code chunks generated
    pub total_chunks: usize,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
}

/// Request to perform semantic search query
///
/// # Examples
///
/// Basic search:
/// ```ignore
/// SearchQueryRequest {
///     query: "error handling".to_string(),
///     limit: 10,
/// }
/// ```
///
/// Search for async functions:
/// ```ignore
/// SearchQueryRequest {
///     query: "async function implementation".to_string(),
///     limit: 5,
/// }
/// ```
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SearchQueryRequest {
    /// Search query string
    pub query: String,
    /// Number of results to return
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

fn default_search_limit() -> usize {
    10
}

/// Individual search result
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SearchResult {
    /// Path to the file containing the match
    pub file_path: String,
    /// Matching code chunk content
    pub chunk_text: String,
    /// Starting line number (1-based)
    pub line_start: Option<usize>,
    /// Ending line number (1-based)
    pub line_end: Option<usize>,
    /// Similarity score (0.0 to 1.0)
    pub similarity_score: f32,
    /// Programming language of the file
    pub language: Option<String>,
    /// Type of code chunk (Function, Class, etc.)
    pub chunk_type: Option<String>,
    /// Excerpt with highlighted matches
    pub excerpt: String,
}

/// Response from semantic search query
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SearchQueryResponse {
    /// List of search results
    pub results: Vec<SearchResult>,
    /// Original search query
    pub query: String,
    /// Total number of results found
    pub total_results: usize,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_index_request_serialization() {
        let request = SearchIndexRequest {
            patterns: vec!["**/*.rs".to_string(), "src/**/*.py".to_string()],
            force: true,
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: SearchIndexRequest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(request.patterns, deserialized.patterns);
        assert_eq!(request.force, deserialized.force);
    }

    #[test]
    fn test_search_query_request_default_limit() {
        let json = r#"{"query": "test"}"#;
        let request: SearchQueryRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.query, "test");
        assert_eq!(request.limit, 10); // Default value
    }

    #[test]
    fn test_search_query_request_custom_limit() {
        let json = r#"{"query": "test", "limit": 5}"#;
        let request: SearchQueryRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.query, "test");
        assert_eq!(request.limit, 5);
    }

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResult {
            file_path: "src/main.rs".to_string(),
            chunk_text: "fn main() {}".to_string(),
            line_start: Some(42),
            line_end: Some(44),
            similarity_score: 0.85,
            language: Some("rust".to_string()),
            chunk_type: Some("Function".to_string()),
            excerpt: "...fn main() {...".to_string(),
        };

        let serialized = serde_json::to_string(&result).unwrap();
        let deserialized: SearchResult = serde_json::from_str(&serialized).unwrap();

        assert_eq!(result.file_path, deserialized.file_path);
        assert_eq!(result.chunk_text, deserialized.chunk_text);
        assert_eq!(result.line_start, deserialized.line_start);
        assert_eq!(result.line_end, deserialized.line_end);
        assert_eq!(result.similarity_score, deserialized.similarity_score);
        assert_eq!(result.language, deserialized.language);
        assert_eq!(result.chunk_type, deserialized.chunk_type);
        assert_eq!(result.excerpt, deserialized.excerpt);
    }
}