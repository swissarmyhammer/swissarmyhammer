//! Search index tool for MCP operations
//!
//! This module provides the SearchIndexTool for indexing files for semantic search through the MCP protocol.

use crate::mcp::search_types::{SearchIndexRequest, SearchIndexResponse};
use crate::mcp::shared_utils::McpErrorHandler;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use std::time::Instant;
use swissarmyhammer::search::{FileIndexer, SemanticConfig, VectorStorage};

/// Tool for indexing files for semantic search
#[derive(Default)]
pub struct SearchIndexTool;

impl SearchIndexTool {
    /// Creates a new instance of the SearchIndexTool
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
            embedding_model: swissarmyhammer_config::DEFAULT_TEST_EMBEDDING_MODEL.to_string(),
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

        let mut indexer = {
            #[cfg(test)]
            {
                FileIndexer::new_for_testing(storage).await.map_err(|e| {
                    McpErrorHandler::handle_error(
                        swissarmyhammer::SwissArmyHammerError::Semantic(e),
                        "create file indexer for testing",
                    )
                })?
            }
            #[cfg(not(test))]
            {
                FileIndexer::new(storage).await.map_err(|e| {
                    McpErrorHandler::handle_error(
                        swissarmyhammer::SwissArmyHammerError::Semantic(e),
                        "create file indexer",
                    )
                })?
            }
        };

        // Perform indexing for all patterns
        let mut combined_report = None;

        for pattern in &request.patterns {
            tracing::debug!("Processing pattern: {}", pattern);
            let report = indexer
                .index_glob(pattern, request.force)
                .await
                .map_err(|e| {
                    McpErrorHandler::handle_error(
                        swissarmyhammer::SwissArmyHammerError::Semantic(e),
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
        let tool = SearchIndexTool::new();
        let context = create_test_context().await;

        // Create a temporary directory with test files
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let test_dir = temp_dir.path();

        // Create test Rust file
        let test_file = test_dir.join("test.rs");
        std::fs::write(
            &test_file,
            r#"fn main() {
    println!("Hello, world!");
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#,
        )
        .expect("Failed to write test file");

        // Use the test file path in the pattern
        let pattern = format!("{}/*.rs", test_dir.display());
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "patterns".to_string(),
            serde_json::Value::Array(vec![serde_json::Value::String(pattern)]),
        );
        arguments.insert("force".to_string(), serde_json::Value::Bool(false));

        // Note: This test may fail if fastembed models cannot be downloaded in test environment
        // This is expected and acceptable in CI/offline environments
        match tool.execute(arguments, &context).await {
            Ok(result) => {
                assert_eq!(result.is_error, Some(false));
                assert!(!result.content.is_empty());
                // Verify the response indicates successful indexing
                let content_str =
                    if let rmcp::model::RawContent::Text(text) = &result.content[0].raw {
                        &text.text
                    } else {
                        panic!("Expected text content");
                    };
                assert!(content_str.contains("indexed_files"));
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("Failed to initialize fastembed model")
                    || error_msg.contains("I/O error")
                    || error_msg.contains("No such file or directory")
                {
                    // Expected in test environments without model access
                    tracing::warn!(
                        "⚠️  Search indexing skipped - model initialization failed: {error_msg}"
                    );
                } else {
                    panic!("Unexpected error: {error_msg}");
                }
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
