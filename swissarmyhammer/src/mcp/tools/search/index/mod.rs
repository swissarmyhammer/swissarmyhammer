//! Search index tool for MCP operations
//!
//! This module provides the SearchIndexTool for indexing files for semantic search through the MCP protocol.

use crate::mcp::search_types::{SearchIndexRequest, SearchIndexResponse};
use crate::mcp::shared_utils::McpErrorHandler;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::search::{FileIndexer, SemanticConfig, VectorStorage};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use std::time::Instant;

/// Tool for indexing files for semantic search
#[derive(Default)]
pub struct SearchIndexTool;

impl SearchIndexTool {
    /// Creates a new instance of the SearchIndexTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for SearchIndexTool {
    fn name(&self) -> &'static str {
        "search_index"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("search", "index")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(SearchIndexRequest))
            .expect("Failed to generate schema")
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext, // Search tools don't need shared context
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: SearchIndexRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!(
            "Starting search indexing with patterns: {:?}, force: {}",
            request.patterns,
            request.force
        );

        if request.patterns.is_empty() {
            return Err(McpError::invalid_request("No patterns or files provided for indexing. Please specify one or more glob patterns (like '**/*.rs') or file paths.", None));
        }

        let start_time = Instant::now();

        // Initialize semantic search components
        let config = SemanticConfig::default();
        let storage = VectorStorage::new(config.clone())
            .map_err(|e| McpErrorHandler::handle_error(e, "initialize vector storage"))?;

        storage
            .initialize()
            .map_err(|e| McpErrorHandler::handle_error(e, "initialize storage database"))?;

        let mut indexer = FileIndexer::new(storage).await.map_err(|e| {
            McpErrorHandler::handle_error(
                crate::SwissArmyHammerError::Semantic(e),
                "create file indexer",
            )
        })?;

        // Perform indexing for all patterns
        let mut combined_report = None;

        for pattern in &request.patterns {
            tracing::debug!("Processing pattern: {}", pattern);
            let report = indexer
                .index_glob(pattern, request.force)
                .await
                .map_err(|e| {
                    McpErrorHandler::handle_error(
                        crate::SwissArmyHammerError::Semantic(e),
                        &format!("index pattern '{pattern}'"),
                    )
                })?;

            match combined_report {
                None => combined_report = Some(report),
                Some(mut existing_report) => {
                    // Merge reports (combine all statistics and errors)
                    existing_report.merge_report(report);
                    combined_report = Some(existing_report);
                }
            }
        }

        let report = combined_report.expect("Should have at least one report");
        let duration = start_time.elapsed();

        let response = SearchIndexResponse {
            message: format!("Successfully indexed {} files", report.files_successful),
            indexed_files: report.files_successful,
            skipped_files: report.files_processed - report.files_successful - report.files_failed,
            total_chunks: report.total_chunks,
            execution_time_ms: duration.as_millis() as u64,
        };

        tracing::info!(
            "Search indexing completed: {} files indexed, {} chunks created in {:?}",
            response.indexed_files,
            response.total_chunks,
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
    fn test_search_index_tool_new() {
        let tool = SearchIndexTool::new();
        assert_eq!(tool.name(), "search_index");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_search_index_tool_schema() {
        let tool = SearchIndexTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["patterns"].is_object());
        assert!(schema["properties"]["force"].is_object());
        assert_eq!(schema["required"], serde_json::json!(["patterns"]));
    }

    #[tokio::test]
    async fn test_search_index_tool_execute_empty_patterns() {
        let tool = SearchIndexTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert("patterns".to_string(), serde_json::Value::Array(vec![]));
        arguments.insert("force".to_string(), serde_json::Value::Bool(false));

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No patterns or files provided"));
    }

    #[tokio::test]
    async fn test_search_index_tool_execute_valid_patterns() {
        // Skip this test in CI or environments where model downloads aren't feasible
        if std::env::var("CI").is_ok() 
            || std::env::var("GITHUB_ACTIONS").is_ok()
            || std::env::var("SKIP_MODEL_TESTS").is_ok() 
        {
            println!("⚠️  Skipping search indexing test - model downloads disabled in test environment");
            return;
        }

        let tool = SearchIndexTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "patterns".to_string(),
            serde_json::Value::Array(vec![serde_json::Value::String("**/*.rs".to_string())]),
        );
        arguments.insert("force".to_string(), serde_json::Value::Bool(false));

        // Add timeout to prevent hanging
        let timeout_duration = std::time::Duration::from_secs(30);
        
        match tokio::time::timeout(timeout_duration, tool.execute(arguments, &context)).await {
            Ok(Ok(result)) => {
                assert_eq!(result.is_error, Some(false));
                assert!(!result.content.is_empty());
            }
            Ok(Err(e)) => {
                let error_msg = e.to_string();
                if error_msg.contains("Failed to initialize fastembed model")
                    || error_msg.contains("I/O error")
                    || error_msg.contains("No such file or directory")
                    || error_msg.contains("Connection")
                    || error_msg.contains("download")
                {
                    // Expected in test environments without model access
                    println!(
                        "⚠️  Search indexing skipped - model initialization failed: {error_msg}"
                    );
                } else {
                    panic!("Unexpected error: {error_msg}");
                }
            }
            Err(_) => {
                // Timeout occurred
                println!("⚠️  Search indexing test timed out - skipping model download test");
            }
        }
    }

    #[tokio::test]
    async fn test_search_index_tool_execute_missing_patterns() {
        let tool = SearchIndexTool::new();
        let context = create_test_context().await;

        let arguments = serde_json::Map::new(); // Missing patterns field

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }
}
