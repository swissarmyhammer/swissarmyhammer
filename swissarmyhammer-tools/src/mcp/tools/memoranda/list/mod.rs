//! Memo listing tool for MCP operations
//!
//! This module provides the ListMemoTool for listing all memos through the MCP protocol.

use crate::mcp::memo_types::ListMemosRequest;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;

/// Tool for listing all memos
#[derive(Default)]
pub struct ListMemoTool;

impl ListMemoTool {
    /// Preview length for memo list operations (characters)
    const MEMO_LIST_PREVIEW_LENGTH: usize = 100;

    /// Creates a new instance of the ListMemoTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for ListMemoTool {
    fn name(&self) -> &'static str {
        "memo_list"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("memoranda", "list")
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
        let _request: ListMemosRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Listing all memos");

        let memo_storage = context.memo_storage.read().await;
        match memo_storage.list().await {
            Ok(memos) => {
                tracing::info!("Retrieved {} memos", memos.len());
                if memos.is_empty() {
                    let summary =
                        crate::mcp::shared_utils::McpFormatter::format_list_summary("memo", 0, 0);
                    Ok(BaseToolImpl::create_success_response(summary))
                } else {
                    let memo_list = memos
                        .iter()
                        .map(|memo| {
                            crate::mcp::shared_utils::McpFormatter::format_memo_preview(
                                memo,
                                Self::MEMO_LIST_PREVIEW_LENGTH,
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");

                    let summary = crate::mcp::shared_utils::McpFormatter::format_list_summary(
                        "memo",
                        memos.len(),
                        memos.len(),
                    );
                    Ok(BaseToolImpl::create_success_response(format!(
                        "{summary}:\n\n{memo_list}"
                    )))
                }
            }
            Err(e) => Err(crate::mcp::shared_utils::McpErrorHandler::handle_error(
                swissarmyhammer_common::SwissArmyHammerError::Other { message: format!("Storage error: {}", e) },
                "list memos",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;

    #[test]
    fn test_list_memo_tool_new() {
        let tool = ListMemoTool::new();
        assert_eq!(tool.name(), "memo_list");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_list_memo_tool_schema() {
        let tool = ListMemoTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"], serde_json::json!({}));
        assert_eq!(schema["required"], serde_json::json!([]));
    }

    #[test]
    fn test_format_memo_preview() {
        use swissarmyhammer_memoranda::{Memo, MemoContent, MemoTitle};

        let title = MemoTitle::new("Test Memo".to_string()).unwrap();
        let content = MemoContent::new("This is a long piece of content that should be truncated in the preview to show only the first part".to_string());
        let memo = Memo::new(title, content);

        let preview = crate::mcp::shared_utils::McpFormatter::format_memo_preview(&memo, 50);
        assert!(preview.contains("Test Memo"));
        assert!(preview.contains("Created:"));
        assert!(preview.contains("Updated:"));
        assert!(preview.contains("Preview:"));
    }

    #[tokio::test]
    async fn test_list_memo_tool_execute_empty_list() {
        let tool = ListMemoTool::new();
        let context = create_test_context().await;

        let arguments = serde_json::Map::new();

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());
    }

    #[tokio::test]
    async fn test_list_memo_tool_execute_with_memos() {
        let tool = ListMemoTool::new();
        let context = create_test_context().await;

        // Create some test memos
        let mut memo_storage = context.memo_storage.write().await;
        memo_storage
            .create(
                swissarmyhammer_memoranda::MemoTitle::new("First Memo".to_string()).unwrap(),
                swissarmyhammer_memoranda::MemoContent::new("First content".to_string()),
            )
            .await
            .unwrap();
        memo_storage
            .create(
                swissarmyhammer_memoranda::MemoTitle::new("Second Memo".to_string()).unwrap(),
                swissarmyhammer_memoranda::MemoContent::new("Second content".to_string()),
            )
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
    async fn test_list_memo_tool_execute_with_invalid_arguments() {
        let tool = ListMemoTool::new();
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
