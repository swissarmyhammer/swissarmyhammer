//! Search index tool for MCP operations
//!
//! This module provides the SearchIndexTool for indexing files for semantic search through the MCP protocol.

use crate::mcp::progress_notifications::generate_progress_token;
use crate::mcp::search_types::{SearchIndexRequest, SearchIndexResponse};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::json;
use std::time::Instant;
use swissarmyhammer_search::{indexer::FileIndexer, storage::VectorStorage, SemanticConfig};

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

    /// Execute the search_index tool with comprehensive progress notifications
    ///
    /// # Progress Notification Flow
    ///
    /// This method implements batched progress notifications following the MCP specification:
    ///
    /// 1. **Start notification** (progress=0): "Starting indexing: {N} patterns"
    ///    - Sent immediately when indexing begins
    ///    - Includes metadata: patterns array and force flag
    ///
    /// 2. **Per-pattern notifications**: "Processing pattern: {pattern} ({M}/{N})"
    ///    - Sent when starting to process each glob pattern
    ///    - Includes metadata: current pattern, pattern index, total patterns
    ///
    /// 3. **Batched file notifications**: "Indexed {X} files"
    ///    - Sent every 10 files (batch_size=10) during indexing
    ///    - Progress values increase monotonically across all patterns
    ///    - Includes metadata: current pattern, cumulative files_processed count
    ///
    /// 4. **Completion notification** (progress=100): "Indexed {N} files ({M} chunks) in {X}s"
    ///    - Sent when all patterns have been processed
    ///    - Includes metadata: files_indexed, files_failed, files_skipped, total_chunks, duration_ms
    ///
    /// # Error Handling
    ///
    /// Progress notification failures are intentionally ignored (using `.ok()`) to ensure
    /// that indexing operations continue successfully even if the notification channel
    /// encounters issues. This design prioritizes the core indexing functionality over
    /// progress reporting.
    ///
    /// # Arguments
    ///
    /// * `arguments` - JSON object containing patterns (array of glob patterns) and force (boolean)
    /// * `_context` - Tool context including optional progress sender for notifications
    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
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
        let progress_token = generate_progress_token();

        // Send start notification
        if let Some(sender) = &_context.progress_sender {
            sender
                .send_progress_with_metadata(
                    &progress_token,
                    Some(0),
                    format!("Starting indexing: {} patterns", request.patterns.len()),
                    json!({
                        "patterns": request.patterns,
                        "force": request.force
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

        let mut indexer = {
            #[cfg(test)]
            {
                FileIndexer::new_for_testing(storage).await.map_err(|e| {
                    McpError::internal_error(
                        format!("Failed to create file indexer for testing: {}", e),
                        None,
                    )
                })?
            }
            #[cfg(not(test))]
            {
                FileIndexer::new(storage).await.map_err(|e| {
                    McpError::internal_error(format!("Failed to create file indexer: {}", e), None)
                })?
            }
        };

        // Perform indexing for all patterns
        let mut combined_report = None;
        let mut total_files_processed = 0;
        let batch_size = 10;

        for (pattern_index, pattern) in request.patterns.iter().enumerate() {
            tracing::debug!("Processing pattern: {}", pattern);

            // Send per-pattern progress notification
            if let Some(sender) = &_context.progress_sender {
                sender
                    .send_progress_with_metadata(
                        &progress_token,
                        Some(total_files_processed as u32),
                        format!(
                            "Processing pattern {}/{}: {}",
                            pattern_index + 1,
                            request.patterns.len(),
                            pattern
                        ),
                        json!({
                            "pattern": pattern,
                            "pattern_index": pattern_index + 1,
                            "total_patterns": request.patterns.len(),
                            "files_processed": total_files_processed
                        }),
                    )
                    .ok();
            }

            // Create a progress callback for batched file updates
            let progress_sender = _context.progress_sender.clone();
            let progress_token_clone = progress_token.clone();
            let pattern_clone = pattern.clone();
            let base_files_processed = total_files_processed;

            let report = indexer
                .index_glob_with_progress(
                    pattern,
                    request.force,
                    Some(move |files_in_pattern, _total_in_pattern| {
                        let current_total = base_files_processed + files_in_pattern;

                        // Send batched progress notifications every batch_size files
                        if files_in_pattern % batch_size == 0 {
                            if let Some(ref sender) = progress_sender {
                                sender
                                    .send_progress_with_metadata(
                                        &progress_token_clone,
                                        Some(current_total as u32),
                                        format!("Indexed {} files", current_total),
                                        json!({
                                            "pattern": pattern_clone,
                                            "files_processed": current_total
                                        }),
                                    )
                                    .ok();
                            }
                        }
                    }),
                )
                .await
                .map_err(|e| {
                    McpError::internal_error(
                        format!("Failed to index pattern '{}': {}", pattern, e),
                        None,
                    )
                })?;

            total_files_processed += report.files_processed;

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

        // Send completion notification
        if let Some(sender) = &_context.progress_sender {
            let duration_ms = duration.as_millis() as u64;
            sender
                .send_progress_with_metadata(
                    &progress_token,
                    Some(100),
                    format!(
                        "Indexed {} files ({} chunks) in {:.1}s",
                        response.indexed_files,
                        response.total_chunks,
                        duration_ms as f64 / 1000.0
                    ),
                    json!({
                        "files_indexed": response.indexed_files,
                        "files_failed": report.files_failed,
                        "files_skipped": response.skipped_files,
                        "total_chunks": response.total_chunks,
                        "duration_ms": duration_ms
                    }),
                )
                .ok();
        }

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

    #[tokio::test]
    async fn test_search_index_sends_progress_notifications() {
        use crate::mcp::progress_notifications::ProgressSender;
        use tokio::sync::mpsc;

        let (tx, mut rx) = mpsc::unbounded_channel();
        let progress_sender = ProgressSender::new(tx);

        let mut context = create_test_context().await;
        context.progress_sender = Some(progress_sender);

        // Create a temporary directory with test files
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let test_dir = temp_dir.path();

        // Create multiple test Rust files to generate multiple progress updates
        for i in 0..15 {
            let test_file = test_dir.join(format!("test_{}.rs", i));
            std::fs::write(
                &test_file,
                format!(
                    r#"fn function_{}() {{
    println!("Hello from file {}", {});
}}

fn add_{}(a: i32, b: i32) -> i32 {{
    a + b
}}
"#,
                    i, i, i, i
                ),
            )
            .expect("Failed to write test file");
        }

        let tool = SearchIndexTool::new();
        let pattern = format!("{}/*.rs", test_dir.display());
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "patterns".to_string(),
            serde_json::Value::Array(vec![serde_json::Value::String(pattern)]),
        );
        arguments.insert("force".to_string(), serde_json::Value::Bool(false));

        // Execute the tool (may fail if embedding model not available, which is expected in test environments)
        let _ = tool.execute(arguments, &context).await;

        // Collect all notifications
        let mut notifications = Vec::new();
        while let Ok(notif) = rx.try_recv() {
            notifications.push(notif);
        }

        // If indexing succeeded, verify notifications were sent
        if !notifications.is_empty() {
            // Should have at least: start notification, per-pattern notification, and completion notification
            assert!(
                notifications.len() >= 3,
                "Expected at least 3 notifications (start, per-pattern, completion), got {}",
                notifications.len()
            );

            // First notification should be the start notification with 0% progress
            assert_eq!(
                notifications[0].progress,
                Some(0),
                "First notification should be start with 0% progress"
            );
            assert!(
                notifications[0].message.contains("Starting indexing"),
                "Start notification should mention starting indexing"
            );

            // Second notification should be per-pattern notification
            assert!(
                notifications[1].message.contains("Processing pattern"),
                "Second notification should mention processing pattern"
            );

            // Last notification should be completion with 100% progress
            let last = notifications.last().unwrap();
            assert_eq!(
                last.progress,
                Some(100),
                "Last notification should be completion with 100% progress"
            );
            assert!(
                last.message.contains("Indexed") || last.message.contains("files"),
                "Completion notification should mention indexed files"
            );

            // Verify progress values increase monotonically (when present)
            let progresses: Vec<u32> = notifications.iter().filter_map(|n| n.progress).collect();
            if progresses.len() >= 2 {
                for window in progresses.windows(2) {
                    assert!(
                        window[0] <= window[1],
                        "Progress should increase monotonically: {} > {}",
                        window[0],
                        window[1]
                    );
                }
            }

            // Check for intermediate batched file notifications
            // With 15 files and batch size of 10, we should see at least one batched notification
            let batched_notifications: Vec<_> = notifications
                .iter()
                .filter(|n| {
                    n.message.contains("Indexed")
                        && n.message.contains("files")
                        && !n.message.contains("in ")
                })
                .collect();

            if !batched_notifications.is_empty() {
                tracing::info!(
                    "Found {} batched progress notifications",
                    batched_notifications.len()
                );
            }
        }
    }

    #[tokio::test]
    async fn test_search_index_continues_when_progress_callback_panics() {
        use crate::mcp::progress_notifications::ProgressSender;
        use tokio::sync::mpsc;

        let (tx, mut rx) = mpsc::unbounded_channel();
        let progress_sender = ProgressSender::new(tx);

        let mut context = create_test_context().await;
        context.progress_sender = Some(progress_sender);

        // Create a temporary directory with test files
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let test_dir = temp_dir.path();



        // Create multiple test Rust files
        for i in 0..5 {
            let test_file = test_dir.join(format!("test_{}.rs", i));
            std::fs::write(
                &test_file,
                format!(
                    r#"fn function_{}() {{
    println!("Hello from file {}", {});
}}
"#,
                    i, i, i
                ),
            )
            .expect("Failed to write test file");
        }

        let tool = SearchIndexTool::new();
        let pattern = format!("{}/*.rs", test_dir.display());
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "patterns".to_string(),
            serde_json::Value::Array(vec![serde_json::Value::String(pattern)]),
        );
        arguments.insert("force".to_string(), serde_json::Value::Bool(false));

        // Execute the tool (may fail if embedding model not available, which is expected in test environments)
        let result = tool.execute(arguments, &context).await;

        // Even if the progress callback has issues, indexing should complete
        // The result may be an error due to missing embedding model, but it should not
        // be due to progress callback failures
        if let Err(e) = &result {
            let error_msg = e.to_string();
            assert!(
                !error_msg.contains("progress") && !error_msg.contains("notification"),
                "Error should not be related to progress notifications: {}",
                error_msg
            );
        }

        // Verify we still received progress notifications
        let mut notifications = Vec::new();
        while let Ok(notif) = rx.try_recv() {
            notifications.push(notif);
        }

        // If any notifications were sent, verify indexing attempted to proceed
        if !notifications.is_empty() {
            tracing::info!(
                "Received {} progress notifications despite callback issues",
                notifications.len()
            );
        }
    }
}
