//! Memo retrieval tool for MCP operations
//!
//! This module provides the GetMemoTool for retrieving a memo by its unique ID through the MCP protocol.

use crate::mcp::memo_types::GetMemoRequest;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;

/// Tool for retrieving a memo by its unique ID
#[derive(Default)]
pub struct GetMemoTool;

impl GetMemoTool {
    /// Creates a new instance of the GetMemoTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for GetMemoTool {
    fn name(&self) -> &'static str {
        "memo_get"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("memoranda", "get")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "ULID identifier of the memo to retrieve"
                }
            },
            "required": ["id"]
        })
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some("memo")
    }

    fn cli_name(&self) -> &'static str {
        "get"
    }

    fn cli_about(&self) -> Option<&'static str> {
        Some("Retrieve a memo by its unique ID")
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: GetMemoRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Getting memo with ID: {}", request.id);

        let memo_id = match swissarmyhammer::memoranda::MemoId::from_string(request.id.clone()) {
            Ok(id) => id,
            Err(_) => {
                return Err(McpError::invalid_params(
                    format!("Invalid memo ID format: {}", request.id),
                    None,
                ))
            }
        };

        let memo_storage = context.memo_storage.read().await;
        match memo_storage.get_memo(&memo_id).await {
            Ok(memo) => {
                tracing::info!("Retrieved memo {}", memo.id);
                Ok(BaseToolImpl::create_success_response(format!(
                    "Memo found:\n\n🆔 ID: {}\nTitle: {}\n📅 Created: {}\n🔄 Updated: {}\n\nContent:\n{}",
                    memo.id,
                    memo.title,
                    crate::mcp::shared_utils::McpFormatter::format_timestamp(memo.created_at),
                    crate::mcp::shared_utils::McpFormatter::format_timestamp(memo.updated_at),
                    memo.content
                )))
            }
            Err(e) => Err(crate::mcp::shared_utils::McpErrorHandler::handle_error(
                e, "get memo",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;

    #[test]
    fn test_get_memo_tool_new() {
        let tool = GetMemoTool::new();
        assert_eq!(tool.name(), "memo_get");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_get_memo_tool_schema() {
        let tool = GetMemoTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["id"].is_object());
        assert_eq!(schema["required"], serde_json::json!(["id"]));
    }

    #[tokio::test]
    async fn test_get_memo_tool_execute_success() {
        let tool = GetMemoTool::new();
        let context = create_test_context().await;

        // First create a memo to retrieve
        let memo_storage = context.memo_storage.write().await;
        let memo = memo_storage
            .create_memo("Test Memo".to_string(), "Test content".to_string())
            .await
            .unwrap();
        drop(memo_storage); // Release the lock

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "id".to_string(),
            serde_json::Value::String(memo.id.to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());
    }

    #[tokio::test]
    async fn test_get_memo_tool_execute_invalid_id_format() {
        let tool = GetMemoTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "id".to_string(),
            serde_json::Value::String("invalid-id".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_memo_tool_execute_nonexistent_memo() {
        let tool = GetMemoTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "id".to_string(),
            serde_json::Value::String("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err()); // Should fail because memo doesn't exist
    }

    #[tokio::test]
    async fn test_get_memo_tool_execute_missing_required_field() {
        let tool = GetMemoTool::new();
        let context = create_test_context().await;

        let arguments = serde_json::Map::new(); // Missing id field

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_memo_tool_execute_invalid_argument_type() {
        let tool = GetMemoTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "id".to_string(),
            serde_json::Value::Number(serde_json::Number::from(123)),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }
}
