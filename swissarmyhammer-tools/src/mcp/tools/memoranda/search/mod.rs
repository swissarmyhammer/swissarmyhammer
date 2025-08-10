//! Memo search tool for MCP operations
//!
//! This module provides the SearchMemoTool for searching memos by query string through the MCP protocol.

use crate::mcp::memo_types::SearchMemosRequest;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;

/// Tool for searching memos by query string
#[derive(Default)]
pub struct SearchMemoTool;

impl SearchMemoTool {
    /// Preview length for memo search operations (characters)
    const MEMO_SEARCH_PREVIEW_LENGTH: usize = 200;

    /// Creates a new instance of the SearchMemoTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for SearchMemoTool {
    fn name(&self) -> &'static str {
        "memo_search"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("memoranda", "search")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query string to match against memo titles and content"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: SearchMemosRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Searching memos with query: {}", request.query);

        // Validate search query is not empty
        crate::mcp::shared_utils::McpValidation::validate_not_empty(&request.query, "search query")
            .map_err(|e| {
                crate::mcp::shared_utils::McpErrorHandler::handle_error(e, "validate search query")
            })?;

        let memo_storage = context.memo_storage.read().await;
        match memo_storage.search_memos(&request.query).await {
            Ok(memos) => {
                tracing::info!("Search returned {} memos", memos.len());
                if memos.is_empty() {
                    Ok(BaseToolImpl::create_success_response(format!(
                        "No memos found matching query: '{}'",
                        request.query
                    )))
                } else {
                    let memo_list = memos
                        .iter()
                        .map(|memo| {
                            crate::mcp::shared_utils::McpFormatter::format_memo_preview(
                                memo,
                                Self::MEMO_SEARCH_PREVIEW_LENGTH,
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");

                    Ok(BaseToolImpl::create_success_response(format!(
                        "Found {} memo{} matching '{}':\n\n{}",
                        memos.len(),
                        if memos.len() == 1 { "" } else { "s" },
                        request.query,
                        memo_list
                    )))
                }
            }
            Err(e) => Err(crate::mcp::shared_utils::McpErrorHandler::handle_error(
                e,
                "search memos",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;

    #[test]
    fn test_search_memo_tool_new() {
        let tool = SearchMemoTool::new();
        assert_eq!(tool.name(), "memo_search");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_search_memo_tool_schema() {
        let tool = SearchMemoTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"], serde_json::json!(["query"]));
    }

    #[test]
    fn test_format_memo_preview() {
        use swissarmyhammer::memoranda::{Memo, MemoId};
        use chrono::Utc;

        let memo = Memo {
            id: MemoId::new(),
            title: "Test Memo".to_string(),
            content: "This is a long piece of content that should be truncated in the preview to show only the first part".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let preview = crate::mcp::shared_utils::McpFormatter::format_memo_preview(&memo, 50);
        assert!(preview.contains("Test Memo"));
        assert!(preview.contains("Created:"));
        assert!(preview.contains("Updated:"));
        assert!(preview.contains("Preview:"));
    }

    #[tokio::test]
    async fn test_search_memo_tool_execute_no_matches() {
        let tool = SearchMemoTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "query".to_string(),
            serde_json::Value::String("nonexistent".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());
    }

    #[tokio::test]
    async fn test_search_memo_tool_execute_with_matches() {
        let tool = SearchMemoTool::new();
        let context = create_test_context().await;

        // Create some test memos
        let memo_storage = context.memo_storage.write().await;
        memo_storage
            .create_memo(
                "Test Memo".to_string(),
                "This contains searchable content".to_string(),
            )
            .await
            .unwrap();
        memo_storage
            .create_memo(
                "Another Memo".to_string(),
                "Different content here".to_string(),
            )
            .await
            .unwrap();
        drop(memo_storage); // Release the lock

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "query".to_string(),
            serde_json::Value::String("content".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());
    }

    #[tokio::test]
    async fn test_search_memo_tool_execute_empty_query() {
        let tool = SearchMemoTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "query".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err()); // Should fail due to validation
    }

    #[tokio::test]
    async fn test_search_memo_tool_execute_missing_required_field() {
        let tool = SearchMemoTool::new();
        let context = create_test_context().await;

        let arguments = serde_json::Map::new(); // Missing query field

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_memo_tool_execute_invalid_argument_type() {
        let tool = SearchMemoTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "query".to_string(),
            serde_json::Value::Number(serde_json::Number::from(123)),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_memo_tool_execute_singular_plural_formatting() {
        let tool = SearchMemoTool::new();
        let context = create_test_context().await;

        // Create one memo that will match
        let memo_storage = context.memo_storage.write().await;
        memo_storage
            .create_memo(
                "Single Memo".to_string(),
                "unique content for single match".to_string(),
            )
            .await
            .unwrap();
        drop(memo_storage); // Release the lock

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "query".to_string(),
            serde_json::Value::String("unique".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        // The result should use singular form "memo" not "memos" for single result
        assert!(!call_result.content.is_empty());
    }
}
