//! Memo update tool for MCP operations
//!
//! This module provides the UpdateMemoTool for updating memo content by ID through the MCP protocol.

use crate::mcp::memo_types::UpdateMemoRequest;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;

/// Tool for updating a memo's content by its ID
#[derive(Default)]
pub struct UpdateMemoTool;

impl UpdateMemoTool {
    /// Creates a new instance of the UpdateMemoTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for UpdateMemoTool {
    fn name(&self) -> &'static str {
        "memo_update"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("memoranda", "update")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "ULID identifier of the memo to update"
                },
                "content": {
                    "type": "string",
                    "description": "New markdown content for the memo"
                }
            },
            "required": ["id", "content"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: UpdateMemoRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Updating memo with ID: {}", request.id);

        // Validate memo content using shared validation
        crate::mcp::shared_utils::McpValidation::validate_not_empty(
            &request.content,
            "memo content",
        )
        .map_err(|e| {
            crate::mcp::shared_utils::McpErrorHandler::handle_error(e, "validate memo content")
        })?;

        let memo_id = match swissarmyhammer::memoranda::MemoId::from_string(request.id.clone()) {
            Ok(id) => id,
            Err(_) => {
                return Err(McpError::invalid_params(
                    format!("Invalid memo ID format: {}", request.id),
                    None,
                ))
            }
        };

        let memo_storage = context.memo_storage.write().await;
        match memo_storage.update_memo(&memo_id, request.content).await {
            Ok(memo) => {
                tracing::info!("Updated memo {}", memo.id);
                Ok(BaseToolImpl::create_success_response(format!(
                    "Successfully updated memo:\n\nID: {}\nTitle: {}\nUpdated: {}\n\nContent:\n{}",
                    memo.id,
                    memo.title,
                    crate::mcp::shared_utils::McpFormatter::format_timestamp(memo.updated_at),
                    memo.content
                )))
            }
            Err(e) => Err(crate::mcp::shared_utils::McpErrorHandler::handle_error(
                e,
                "update memo",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;

    #[test]
    fn test_update_memo_tool_new() {
        let tool = UpdateMemoTool::new();
        assert_eq!(tool.name(), "memo_update");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_update_memo_tool_schema() {
        let tool = UpdateMemoTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["id"].is_object());
        assert!(schema["properties"]["content"].is_object());
        assert_eq!(schema["required"], serde_json::json!(["id", "content"]));
    }

    #[tokio::test]
    async fn test_update_memo_tool_execute_success() {
        let tool = UpdateMemoTool::new();
        let context = create_test_context().await;

        // First create a memo to update
        let memo_storage = context.memo_storage.write().await;
        let memo = memo_storage
            .create_memo("Test Memo".to_string(), "Original content".to_string())
            .await
            .unwrap();
        drop(memo_storage); // Release the lock

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "id".to_string(),
            serde_json::Value::String(memo.id.to_string()),
        );
        arguments.insert(
            "content".to_string(),
            serde_json::Value::String("Updated content".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());
    }

    #[tokio::test]
    async fn test_update_memo_tool_execute_empty_content() {
        let tool = UpdateMemoTool::new();
        let context = create_test_context().await;

        // First create a memo to update
        let memo_storage = context.memo_storage.write().await;
        let memo = memo_storage
            .create_memo("Test Memo".to_string(), "Original content".to_string())
            .await
            .unwrap();
        drop(memo_storage); // Release the lock

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "id".to_string(),
            serde_json::Value::String(memo.id.to_string()),
        );
        arguments.insert(
            "content".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err()); // Should fail due to validation
    }

    #[tokio::test]
    async fn test_update_memo_tool_execute_invalid_id_format() {
        let tool = UpdateMemoTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "id".to_string(),
            serde_json::Value::String("invalid-id".to_string()),
        );
        arguments.insert(
            "content".to_string(),
            serde_json::Value::String("New content".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_memo_tool_execute_nonexistent_memo() {
        let tool = UpdateMemoTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "id".to_string(),
            serde_json::Value::String("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string()),
        );
        arguments.insert(
            "content".to_string(),
            serde_json::Value::String("New content".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err()); // Should fail because memo doesn't exist
    }

    #[tokio::test]
    async fn test_update_memo_tool_execute_missing_required_field() {
        let tool = UpdateMemoTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "id".to_string(),
            serde_json::Value::String("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string()),
        );
        // Missing content field

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_memo_tool_execute_invalid_argument_type() {
        let tool = UpdateMemoTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "id".to_string(),
            serde_json::Value::Number(serde_json::Number::from(123)),
        );
        arguments.insert(
            "content".to_string(),
            serde_json::Value::String("New content".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }
}
