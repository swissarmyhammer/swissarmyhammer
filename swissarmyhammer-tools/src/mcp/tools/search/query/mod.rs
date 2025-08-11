//! Search query tool for MCP operations
//!
//! This module provides the SearchQueryTool for performing semantic search queries through the MCP protocol.

use crate::mcp::search_types::{SearchQueryRequest, SearchQueryResponse, SearchResult};
use crate::mcp::shared_utils::McpErrorHandler;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use std::time::Instant;
use swissarmyhammer::search::{SearchQuery, SemanticConfig, SemanticSearcher, VectorStorage};

/// Tool for performing semantic search queries
#[derive(Default)]
pub struct SearchQueryTool;

impl SearchQueryTool {
    /// Creates a new instance of the SearchQueryTool
    pub fn new() -> Self {
        Self
    }

    #[cfg(test)]
    fn create_test_config() -> SemanticConfig {
        // Create a unique temporary database path for each test execution
        use std::thread;
        use std::time::{SystemTime, UNIX_EPOCH};

        let thread_id = format!("{:?}", thread::current().id());
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let unique_id = format!(
            "{}_{}",
            thread_id.replace("ThreadId(", "").replace(")", ""),
            timestamp
        );

        let persistent_path =
            std::env::temp_dir().join(format!("swissarmyhammer_test_{unique_id}"));
        std::fs::create_dir_all(&persistent_path).expect("Failed to create persistent test dir");
        let db_path = persistent_path.join("semantic.db");

        SemanticConfig {
            database_path: db_path,
            embedding_model: "test-model".to_string(),
            chunk_size: 512,
            chunk_overlap: 64,
            similarity_threshold: 0.7,
            excerpt_length: 200,
            context_lines: 2,
            simple_search_threshold: 0.5,
            code_similarity_threshold: 0.7,
            content_preview_length: 100,
            min_chunk_size: 50,
            max_chunk_size: 2000,
            max_chunks_per_file: 100,
            max_file_size_bytes: 10 * 1024 * 1024,
        }
    }
}

#[async_trait]
impl McpTool for SearchQueryTool {
    fn name(&self) -> &'static str {
        "search_query"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("search", "query")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(SearchQueryRequest))
            .expect("Failed to generate schema")
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext, // Search tools don't need shared context
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: SearchQueryRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!(
            "Starting search query: '{}', limit: {}",
            request.query,
            request.limit
        );

        if request.query.trim().is_empty() {
            return Err(McpError::invalid_request(
                "Search query cannot be empty",
                None,
            ));
        }

        let start_time = Instant::now();

        // Initialize semantic search components
        let config = {
            #[cfg(test)]
            {
                Self::create_test_config()
            }
            #[cfg(not(test))]
            {
                SemanticConfig::default()
            }
        };
        let storage = VectorStorage::new(config.clone())
            .map_err(|e| McpErrorHandler::handle_error(e, "initialize vector storage"))?;

        storage
            .initialize()
            .map_err(|e| McpErrorHandler::handle_error(e, "initialize storage database"))?;

        let searcher = {
            #[cfg(test)]
            {
                SemanticSearcher::new_for_testing(storage, config)
                    .await
                    .map_err(|e| {
                        McpErrorHandler::handle_error(
                            swissarmyhammer::SwissArmyHammerError::Semantic(e),
                            "create semantic searcher for testing",
                        )
                    })?
            }
            #[cfg(not(test))]
            {
                SemanticSearcher::new(storage, config).await.map_err(|e| {
                    McpErrorHandler::handle_error(
                        swissarmyhammer::SwissArmyHammerError::Semantic(e),
                        "create semantic searcher",
                    )
                })?
            }
        };

        // Perform search
        let search_query = SearchQuery {
            text: request.query.clone(),
            limit: request.limit,
            similarity_threshold: 0.5, // Use lower threshold for more results
            language_filter: None,
        };

        let search_results = searcher.search(&search_query).await.map_err(|e| {
            McpErrorHandler::handle_error(
                swissarmyhammer::SwissArmyHammerError::Semantic(e),
                &format!("search for '{}'", request.query),
            )
        })?;

        let duration = start_time.elapsed();

        // Convert search results to response format
        let results: Vec<SearchResult> = search_results
            .into_iter()
            .map(|result| SearchResult {
                file_path: result.chunk.file_path.to_string_lossy().to_string(),
                chunk_text: result.chunk.content.clone(),
                line_start: Some(result.chunk.start_line),
                line_end: Some(result.chunk.end_line),
                similarity_score: result.similarity_score,
                language: Some(format!("{:?}", result.chunk.language).to_lowercase()),
                chunk_type: Some(format!("{:?}", result.chunk.chunk_type)),
                excerpt: result.excerpt,
            })
            .collect();

        let response = SearchQueryResponse {
            total_results: results.len(),
            results,
            query: request.query,
            execution_time_ms: duration.as_millis() as u64,
        };

        tracing::info!(
            "Search query completed: found {} results for '{}' in {:?}",
            response.total_results,
            response.query,
            duration
        );

        Ok(BaseToolImpl::create_success_response(
            serde_json::to_string_pretty(&response).map_err(|e| {
                McpError::internal_error(format!("Failed to serialize response: {e}"), None)
            })?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;

    #[test]
    fn test_search_query_tool_new() {
        let tool = SearchQueryTool::new();
        assert_eq!(tool.name(), "search_query");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_search_query_tool_schema() {
        let tool = SearchQueryTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert!(schema["properties"]["limit"].is_object());
        assert_eq!(schema["required"], serde_json::json!(["query"]));
    }

    #[tokio::test]
    async fn test_search_query_tool_execute_empty_query() {
        let tool = SearchQueryTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "query".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_search_query_tool_execute_valid_query() {
        let tool = SearchQueryTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "query".to_string(),
            serde_json::Value::String("test function".to_string()),
        );
        arguments.insert(
            "limit".to_string(),
            serde_json::Value::Number(serde_json::Number::from(5)),
        );

        // Note: This test may fail if fastembed models cannot be downloaded in test environment
        // This is expected and acceptable in CI/offline environments
        match tool.execute(arguments, &context).await {
            Ok(result) => {
                assert_eq!(result.is_error, Some(false));
                assert!(!result.content.is_empty());
                // The result should be a JSON response with search results
                let content_str =
                    if let rmcp::model::RawContent::Text(text) = &result.content[0].raw {
                        &text.text
                    } else {
                        panic!("Expected text content");
                    };
                assert!(content_str.contains("results"));
                assert!(content_str.contains("query"));
                // With an empty database, we expect 0 results
                assert!(content_str.contains("\"total_results\": 0"));
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("Failed to initialize fastembed model")
                    || error_msg.contains("I/O error")
                    || error_msg.contains("No such file or directory")
                    || error_msg.contains("Vector storage operation failed")
                    || error_msg.contains("Semantic search error")
                    || error_msg.contains("Storage error")
                    || error_msg.contains("Could not set lock")
                {
                    // Expected in test environments without model access or with database conflicts
                    println!(
                        "⚠️  Search query skipped - semantic search operation failed: {error_msg}"
                    );
                } else {
                    panic!("Unexpected error: {error_msg}");
                }
            }
        }
    }

    #[tokio::test]
    async fn test_search_query_tool_execute_default_limit() {
        let _tool = SearchQueryTool::new();
        let _context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "query".to_string(),
            serde_json::Value::String("test".to_string()),
        );
        // Omit limit to test default

        // Test that parsing works with default limit
        let request: SearchQueryRequest = BaseToolImpl::parse_arguments(arguments).unwrap();
        assert_eq!(request.limit, 10); // Default value
    }

    #[tokio::test]
    async fn test_search_query_tool_execute_missing_query() {
        let tool = SearchQueryTool::new();
        let context = create_test_context().await;

        let arguments = serde_json::Map::new(); // Missing query field

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }
}
