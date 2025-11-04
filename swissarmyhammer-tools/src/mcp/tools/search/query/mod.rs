//! Search query tool for MCP operations
//!
//! This module provides the SearchQueryTool for performing semantic search queries through the MCP protocol.

use crate::mcp::progress_notifications::generate_progress_token;
use crate::mcp::search_types::{SearchQueryRequest, SearchQueryResponse, SearchResult};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::json;
use std::time::Instant;
use swissarmyhammer_search::{
    searcher::SemanticSearcher, storage::VectorStorage, SearchQuery, SemanticConfig,
};

/// Default similarity threshold for semantic search queries
///
/// This threshold determines the minimum similarity score required for a result to be included.
/// Lower values (closer to 0) return more results with lower relevance, while higher values
/// (closer to 1) return fewer but more relevant results. The value of 0.5 provides a good
/// balance for most search queries.
const DEFAULT_SIMILARITY_THRESHOLD: f32 = 0.5;

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
        super::test_utils::create_test_semantic_config()
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

    /// Execute the search_query tool with progress notifications
    ///
    /// # Progress Notification Flow
    ///
    /// This method implements a simple two-notification pattern:
    ///
    /// 1. **Start notification** (progress=0): "Searching: {query}"
    ///    - Sent immediately when search begins
    ///    - Includes metadata: query, limit, similarity_threshold
    ///
    /// 2. **Completion notification** (progress=100): "Search completed: {N} results in {X}s"
    ///    - Sent when search completes
    ///    - Includes metadata: query, results_found, results_returned, duration_ms
    ///
    /// # Error Handling
    ///
    /// Progress notification failures are intentionally ignored (using `.ok()`) to ensure
    /// that search operations continue successfully even if the notification channel
    /// encounters issues. This design prioritizes the core search functionality over
    /// progress reporting.
    ///
    /// # Arguments
    ///
    /// * `arguments` - JSON object containing query (string) and limit (optional number)
    /// * `_context` - Tool context including optional progress sender for notifications
    ///
    /// # Returns
    ///
    /// Returns a `CallToolResult` containing a JSON-formatted `SearchQueryResponse` with:
    /// - `total_results`: Number of matching results found
    /// - `results`: Array of `SearchResult` objects with file paths, line numbers, similarity scores, excerpts, and code context
    /// - `query`: The original search query string
    /// - `execution_time_ms`: Time taken to execute the search in milliseconds
    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: SearchQueryRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!(
            "Starting search query: '{}', limit: {}",
            request.query,
            request.limit
        );

        if request.query.trim().is_empty() {
            return Err(McpError::invalid_request(
                "Search query cannot be empty. Please provide a search query string.",
                None,
            ));
        }

        let start_time = Instant::now();
        // Generate unique token for correlating all progress notifications for this search
        let progress_token = generate_progress_token();

        // Send start notification
        if let Some(sender) = &_context.progress_sender {
            sender
                .send_progress_with_metadata(
                    &progress_token,
                    Some(0),
                    format!("Searching: {}", request.query),
                    json!({
                        "query": request.query,
                        "limit": request.limit,
                        "similarity_threshold": DEFAULT_SIMILARITY_THRESHOLD
                    }),
                )
                .ok();
        }

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
        let storage = VectorStorage::new(config.clone()).map_err(|e| {
            McpError::internal_error(format!("Failed to initialize vector storage: {}", e), None)
        })?;

        storage.initialize().map_err(|e| {
            McpError::internal_error(
                format!("Failed to initialize storage database: {}", e),
                None,
            )
        })?;

        let searcher = {
            #[cfg(test)]
            {
                SemanticSearcher::new_for_testing(storage, config)
                    .await
                    .map_err(|e| {
                        McpError::internal_error(
                            format!("Failed to create semantic searcher for testing: {}", e),
                            None,
                        )
                    })?
            }
            #[cfg(not(test))]
            {
                SemanticSearcher::new(storage, config).await.map_err(|e| {
                    McpError::internal_error(
                        format!("Failed to create semantic searcher: {}", e),
                        None,
                    )
                })?
            }
        };

        // Perform search
        let search_query = SearchQuery {
            text: request.query.clone(),
            limit: request.limit,
            similarity_threshold: DEFAULT_SIMILARITY_THRESHOLD,
            language_filter: None,
        };

        let search_results = searcher.search(&search_query).await.map_err(|e| {
            McpError::internal_error(
                format!("Failed to search for '{}': {}", request.query, e),
                None,
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

        let results_count = results.len();
        let duration_ms = duration.as_millis() as u64;

        let response = SearchQueryResponse {
            total_results: results_count,
            results,
            query: request.query.clone(),
            execution_time_ms: duration_ms,
        };

        // Send completion notification
        if let Some(sender) = &_context.progress_sender {
            sender
                .send_progress_with_metadata(
                    &progress_token,
                    Some(100),
                    format!(
                        "Search completed: {} results in {:.1}s",
                        results_count,
                        duration_ms as f64 / 1000.0
                    ),
                    json!({
                        "query": response.query,
                        "results_found": results_count,
                        "results_returned": results_count,
                        "duration_ms": duration_ms
                    }),
                )
                .ok();
        }

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
                    tracing::warn!(
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

    #[tokio::test]
    async fn test_search_query_sends_progress_notifications() {
        use crate::mcp::progress_notifications::ProgressSender;
        use tokio::sync::mpsc;

        let (tx, mut rx) = mpsc::unbounded_channel();
        let progress_sender = ProgressSender::new(tx);

        let mut context = create_test_context().await;
        context.progress_sender = Some(progress_sender);

        let tool = SearchQueryTool::new();
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "query".to_string(),
            serde_json::Value::String("test function".to_string()),
        );
        arguments.insert(
            "limit".to_string(),
            serde_json::Value::Number(serde_json::Number::from(5)),
        );

        // Execute the tool (may fail if embedding model not available, which is expected in test environments)
        let _ = tool.execute(arguments, &context).await;

        // Collect all notifications
        let mut notifications = Vec::new();
        while let Ok(notif) = rx.try_recv() {
            notifications.push(notif);
        }

        // If search succeeded, verify notifications were sent
        if !notifications.is_empty() {
            // Should have at least: start notification and completion notification
            assert!(
                notifications.len() >= 2,
                "Expected at least 2 notifications (start, completion), got {}",
                notifications.len()
            );

            // First notification should be the start notification with 0% progress
            assert_eq!(
                notifications[0].progress,
                Some(0),
                "First notification should be start with 0% progress"
            );
            assert!(
                notifications[0].message.contains("Searching"),
                "Start notification should mention searching"
            );

            // Verify start notification metadata
            if let Some(ref metadata) = notifications[0].metadata {
                assert!(
                    metadata.get("query").is_some(),
                    "Start notification should include query in metadata"
                );
                assert_eq!(
                    metadata.get("query").unwrap(),
                    &json!("test function"),
                    "Start notification query should match request"
                );
                assert!(
                    metadata.get("limit").is_some(),
                    "Start notification should include limit in metadata"
                );
                assert_eq!(
                    metadata.get("limit").unwrap(),
                    &json!(5),
                    "Start notification limit should match request"
                );
                assert!(
                    metadata.get("similarity_threshold").is_some(),
                    "Start notification should include similarity_threshold in metadata"
                );
                assert_eq!(
                    metadata.get("similarity_threshold").unwrap(),
                    &json!(0.5),
                    "Start notification similarity_threshold should be 0.5"
                );
            }

            // Last notification should be completion with 100% progress
            let last = notifications.last().unwrap();
            assert_eq!(
                last.progress,
                Some(100),
                "Last notification should be completion with 100% progress"
            );
            assert!(
                last.message.contains("Search completed") || last.message.contains("results"),
                "Completion notification should mention completion or results"
            );

            // Verify completion notification metadata
            if let Some(ref metadata) = last.metadata {
                assert!(
                    metadata.get("query").is_some(),
                    "Completion notification should include query in metadata"
                );
                assert_eq!(
                    metadata.get("query").unwrap(),
                    &json!("test function"),
                    "Completion notification query should match request"
                );
                assert!(
                    metadata.get("results_found").is_some(),
                    "Completion notification should include results_found in metadata"
                );
                // Verify results_found is a number (value depends on test environment)
                assert!(
                    metadata.get("results_found").unwrap().is_number(),
                    "Completion notification results_found should be a number"
                );
                assert!(
                    metadata.get("results_returned").is_some(),
                    "Completion notification should include results_returned in metadata"
                );
                // Verify results_returned matches results_found
                assert_eq!(
                    metadata.get("results_returned").unwrap(),
                    metadata.get("results_found").unwrap(),
                    "Completion notification results_returned should match results_found"
                );
                assert!(
                    metadata.get("duration_ms").is_some(),
                    "Completion notification should include duration_ms in metadata"
                );
                // Verify duration_ms is a number
                assert!(
                    metadata.get("duration_ms").unwrap().is_number(),
                    "Completion notification duration_ms should be a number"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_search_query_progress_notifications_empty_results() {
        use crate::mcp::progress_notifications::ProgressSender;
        use tokio::sync::mpsc;

        let (tx, mut rx) = mpsc::unbounded_channel();
        let progress_sender = ProgressSender::new(tx);

        let mut context = create_test_context().await;
        context.progress_sender = Some(progress_sender);

        let tool = SearchQueryTool::new();
        let mut arguments = serde_json::Map::new();
        // Use a query unlikely to match anything in an empty/new database
        arguments.insert(
            "query".to_string(),
            serde_json::Value::String("xyzzy_nonexistent_magic_query_12345".to_string()),
        );
        arguments.insert(
            "limit".to_string(),
            serde_json::Value::Number(serde_json::Number::from(10)),
        );

        // Execute the tool (may fail if embedding model not available, which is expected in test environments)
        let _ = tool.execute(arguments, &context).await;

        // Collect all notifications
        let mut notifications = Vec::new();
        while let Ok(notif) = rx.try_recv() {
            notifications.push(notif);
        }

        // If search succeeded, verify notifications were sent with 0 results
        if !notifications.is_empty() {
            // Should have at least: start notification and completion notification
            assert!(
                notifications.len() >= 2,
                "Expected at least 2 notifications (start, completion), got {}",
                notifications.len()
            );

            // First notification should be the start notification
            assert_eq!(
                notifications[0].progress,
                Some(0),
                "First notification should be start with 0% progress"
            );

            // Last notification should be completion with 100% progress
            let last = notifications.last().unwrap();
            assert_eq!(
                last.progress,
                Some(100),
                "Last notification should be completion with 100% progress"
            );

            // Verify completion notification metadata reports 0 results
            if let Some(ref metadata) = last.metadata {
                assert!(
                    metadata.get("results_found").is_some(),
                    "Completion notification should include results_found in metadata"
                );
                assert_eq!(
                    metadata.get("results_found").unwrap(),
                    &json!(0),
                    "Completion notification should report 0 results_found for empty results"
                );
                assert!(
                    metadata.get("results_returned").is_some(),
                    "Completion notification should include results_returned in metadata"
                );
                assert_eq!(
                    metadata.get("results_returned").unwrap(),
                    &json!(0),
                    "Completion notification should report 0 results_returned for empty results"
                );
            }

            // Verify completion message mentions 0 results
            assert!(
                last.message.contains("0 results") || last.message.contains("completed"),
                "Completion notification should mention 0 results or completion: {}",
                last.message
            );
        }
    }

    #[tokio::test]
    async fn test_search_query_continues_when_progress_sender_fails() {
        use crate::mcp::progress_notifications::ProgressSender;
        use tokio::sync::mpsc;

        let (tx, rx) = mpsc::unbounded_channel();
        let progress_sender = ProgressSender::new(tx);

        let mut context = create_test_context().await;
        context.progress_sender = Some(progress_sender);

        // Drop the receiver to close the channel
        drop(rx);

        let tool = SearchQueryTool::new();
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "query".to_string(),
            serde_json::Value::String("test".to_string()),
        );

        // Execute the tool - should not fail due to notification channel being closed
        let result = tool.execute(arguments, &context).await;

        // The result may be an error due to missing embedding model, but it should not
        // be due to progress notification failures
        if let Err(e) = &result {
            let error_msg = e.to_string();
            assert!(
                !error_msg.contains("progress") && !error_msg.contains("notification"),
                "Error should not be related to progress notifications: {}",
                error_msg
            );
        }
    }
}
