//! Memo creation tool for MCP operations
//!
//! This module provides the CreateMemoTool for creating new memos through the MCP protocol.

use crate::mcp::memo_types::CreateMemoRequest;
use crate::mcp::shared_utils::McpErrorHandler;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;

/// Tool for creating new memos
#[derive(Default)]
pub struct CreateMemoTool;

impl CreateMemoTool {
    /// Creates a new instance of the CreateMemoTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for CreateMemoTool {
    fn name(&self) -> &'static str {
        "memo_create"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("memoranda", "create")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Title of the memo"
                },
                "content": {
                    "type": "string",
                    "description": "Markdown content of the memo"
                }
            },
            "required": ["title", "content"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: CreateMemoRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Creating memo with title: {}", request.title);

        // Note: Both title and content can be empty - storage layer supports this

        let memo_storage = context.memo_storage.write().await;
        match memo_storage
            .create_memo(request.title, request.content)
            .await
        {
            Ok(memo) => {
                tracing::info!("Created memo {}", memo.id);
                Ok(BaseToolImpl::create_success_response(format!(
                    "Successfully created memo '{}' with ID: {}\n\nTitle: {}\nContent: {}",
                    memo.title, memo.id, memo.title, memo.content
                )))
            }
            Err(e) => Err(McpErrorHandler::handle_error(e, "create memo")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;

    #[test]
    fn test_create_memo_tool_new() {
        let tool = CreateMemoTool::new();
        assert_eq!(tool.name(), "memo_create");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_create_memo_tool_schema() {
        let tool = CreateMemoTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["title"].is_object());
        assert!(schema["properties"]["content"].is_object());
        assert_eq!(schema["required"], serde_json::json!(["title", "content"]));
    }

    #[tokio::test]
    async fn test_create_memo_tool_execute_success() {
        let tool = CreateMemoTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "title".to_string(),
            serde_json::Value::String("Test Memo".to_string()),
        );
        arguments.insert(
            "content".to_string(),
            serde_json::Value::String("This is test content".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());
    }

    #[tokio::test]
    async fn test_create_memo_tool_execute_empty_title_and_content() {
        let tool = CreateMemoTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "title".to_string(),
            serde_json::Value::String("".to_string()),
        );
        arguments.insert(
            "content".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok()); // Empty title and content should be allowed
    }

    #[tokio::test]
    async fn test_create_memo_tool_execute_missing_required_field() {
        let tool = CreateMemoTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "title".to_string(),
            serde_json::Value::String("Test Memo".to_string()),
        );
        // Missing content field

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_memo_tool_execute_invalid_argument_type() {
        let tool = CreateMemoTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "title".to_string(),
            serde_json::Value::Number(serde_json::Number::from(123)),
        );
        arguments.insert(
            "content".to_string(),
            serde_json::Value::String("content".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }
}
