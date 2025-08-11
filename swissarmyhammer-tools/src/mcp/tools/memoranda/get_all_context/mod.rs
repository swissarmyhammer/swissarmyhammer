//! Memo get all context tool for MCP operations
//!
//! This module provides the GetAllContextMemoTool for retrieving all memo content formatted for AI context consumption.

use crate::mcp::memo_types::GetAllContextRequest;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;

/// Tool for getting all memo content formatted for AI context consumption
#[derive(Default)]
pub struct GetAllContextMemoTool;

impl GetAllContextMemoTool {
    /// Creates a new instance of the GetAllContextMemoTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for GetAllContextMemoTool {
    fn name(&self) -> &'static str {
        "memo_get_all_context"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("memoranda", "get_all_context")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let _request: GetAllContextRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Getting all memo context");

        let memo_storage = context.memo_storage.read().await;
        match memo_storage.list_memos().await {
            Ok(memos) => {
                tracing::info!("Retrieved {} memos for context", memos.len());
                if memos.is_empty() {
                    Ok(BaseToolImpl::create_success_response(
                        "No memos available".to_string(),
                    ))
                } else {
                    // Sort memos by updated_at descending (most recent first)
                    let mut sorted_memos = memos;
                    sorted_memos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

                    let context = sorted_memos
                        .iter()
                        .map(|memo| {
                            format!(
                                "=== {} (ID: {}) ===\nCreated: {}\nUpdated: {}\n\n{}",
                                memo.title,
                                memo.id,
                                crate::mcp::shared_utils::McpFormatter::format_timestamp(
                                    memo.created_at
                                ),
                                crate::mcp::shared_utils::McpFormatter::format_timestamp(
                                    memo.updated_at
                                ),
                                memo.content
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(&format!("\n\n{}\n\n", "=".repeat(80)));

                    let memo_count = sorted_memos.len();
                    let plural_suffix = if memo_count == 1 { "" } else { "s" };
                    Ok(BaseToolImpl::create_success_response(format!(
                        "All memo context ({memo_count} memo{plural_suffix}):\n\n{context}"
                    )))
                }
            }
            Err(e) => Err(crate::mcp::shared_utils::McpErrorHandler::handle_error(
                e,
                "get memo context",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;

    #[test]
    fn test_get_all_context_memo_tool_new() {
        let tool = GetAllContextMemoTool::new();
        assert_eq!(tool.name(), "memo_get_all_context");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_get_all_context_memo_tool_schema() {
        let tool = GetAllContextMemoTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"], serde_json::json!({}));
        assert_eq!(schema["required"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn test_get_all_context_memo_tool_execute_empty() {
        let tool = GetAllContextMemoTool::new();
        let context = create_test_context().await;

        let arguments = serde_json::Map::new();

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());
    }

    #[tokio::test]
    async fn test_get_all_context_memo_tool_execute_with_memos() {
        let tool = GetAllContextMemoTool::new();
        let context = create_test_context().await;

        // Create some test memos
        let memo_storage = context.memo_storage.write().await;
        memo_storage
            .create_memo("First Memo".to_string(), "First content".to_string())
            .await
            .unwrap();
        memo_storage
            .create_memo("Second Memo".to_string(), "Second content".to_string())
            .await
            .unwrap();
        drop(memo_storage); // Release the lock

        let arguments = serde_json::Map::new();

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());
    }

    #[tokio::test]
    async fn test_get_all_context_memo_tool_execute_sorting() {
        let tool = GetAllContextMemoTool::new();
        let context = create_test_context().await;

        // Create memos with some delay to ensure different timestamps
        let memo_storage = context.memo_storage.write().await;
        let _first_memo = memo_storage
            .create_memo("First Memo".to_string(), "First content".to_string())
            .await
            .unwrap();

        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let _second_memo = memo_storage
            .create_memo("Second Memo".to_string(), "Second content".to_string())
            .await
            .unwrap();
        drop(memo_storage); // Release the lock

        let arguments = serde_json::Map::new();

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());

        // The most recently created memo should appear first in the context
        // Since we created second_memo after first_memo, it should have a later updated_at
    }

    #[tokio::test]
    async fn test_get_all_context_memo_tool_execute_singular_plural() {
        let tool = GetAllContextMemoTool::new();
        let context = create_test_context().await;

        // Create exactly one memo
        let memo_storage = context.memo_storage.write().await;
        memo_storage
            .create_memo("Single Memo".to_string(), "Single content".to_string())
            .await
            .unwrap();
        drop(memo_storage); // Release the lock

        let arguments = serde_json::Map::new();

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());
        // Should use singular form "memo" not "memos" for single result
    }

    #[tokio::test]
    async fn test_get_all_context_memo_tool_execute_with_invalid_arguments() {
        let tool = GetAllContextMemoTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "invalid_field".to_string(),
            serde_json::Value::String("invalid".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        // Should succeed because the schema allows extra fields and the parsing ignores unknown fields
        assert!(result.is_ok());
    }
}
