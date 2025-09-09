//! Memo creation tool for MCP operations
//!
//! This module provides the CreateMemoTool for creating new memos through the MCP protocol.

use crate::mcp::memo_types::CreateMemoRequest;
use crate::mcp::shared_utils::McpErrorHandler;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;

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

        tracing::debug!("Creating/replacing memo with title: {}", request.title);

        // Note: Both title and content can be empty - storage layer supports this

        let mut memo_storage = context.memo_storage.write().await;
        let title = match swissarmyhammer_memoranda::MemoTitle::new(request.title) {
            Ok(title) => title,
            Err(e) => {
                return Ok(BaseToolImpl::create_error_response(
                    format!("Invalid title: {}", e),
                    None,
                ))
            }
        };

        // Check if memo already exists
        let existing_memo = match memo_storage.get(&title).await {
            Ok(memo) => memo,
            Err(e) => {
                return Err(McpErrorHandler::handle_error(
                    swissarmyhammer_common::SwissArmyHammerError::Other { message: format!("Storage error: {}", e) },
                    "check existing memo",
                ));
            }
        };

        let (_memo, _action) = if existing_memo.is_some() {
            // Memo exists, replace it
            match memo_storage.update(&title, request.content.into()).await {
                Ok(memo) => {
                    tracing::info!("Replaced memo {}", memo.title);
                    (memo, "replaced")
                }
                Err(e) => {
                    return Err(McpErrorHandler::handle_error(
                        swissarmyhammer_common::SwissArmyHammerError::Other { message: format!("Storage error: {}", e) },
                        "replace memo",
                    ));
                }
            }
        } else {
            // Memo doesn't exist, create it
            match memo_storage.create(title, request.content.into()).await {
                Ok(memo) => {
                    tracing::info!("Created memo {}", memo.title);
                    (memo, "created")
                }
                Err(e) => {
                    return Err(McpErrorHandler::handle_error(
                        swissarmyhammer_common::SwissArmyHammerError::Other { message: format!("Storage error: {}", e) },
                        "create memo",
                    ));
                }
            }
        };

        Ok(BaseToolImpl::create_success_response("OK".to_string()))
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

        // Verify the response is simply "OK"
        let response_text = call_result.content[0].as_text().unwrap().text.as_str();
        assert_eq!(response_text, "OK");
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

    #[tokio::test]
    async fn test_create_memo_tool_execute_replacement() {
        let tool = CreateMemoTool::new();
        let context = create_test_context().await;

        // First, create a memo
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "title".to_string(),
            serde_json::Value::String("Test Replacement Memo".to_string()),
        );
        arguments.insert(
            "content".to_string(),
            serde_json::Value::String("Original content".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        let response_text = call_result.content[0].as_text().unwrap().text.as_str();
        assert_eq!(response_text, "OK");

        // Now, replace the same memo with new content
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "title".to_string(),
            serde_json::Value::String("Test Replacement Memo".to_string()),
        );
        arguments.insert(
            "content".to_string(),
            serde_json::Value::String("Replaced content".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        let response_text = call_result.content[0].as_text().unwrap().text.as_str();
        assert_eq!(response_text, "OK");
    }

    #[tokio::test]
    async fn test_create_memo_tool_execute_replacement_preserves_creation_time() {
        let tool = CreateMemoTool::new();
        let context = create_test_context().await;

        // First, create a memo
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "title".to_string(),
            serde_json::Value::String("Time Test Memo".to_string()),
        );
        arguments.insert(
            "content".to_string(),
            serde_json::Value::String("Original content".to_string()),
        );

        let result1 = tool.execute(arguments, &context).await;
        assert!(result1.is_ok());

        // Add a small delay to ensure different update time
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Now, replace the same memo with new content
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "title".to_string(),
            serde_json::Value::String("Time Test Memo".to_string()),
        );
        arguments.insert(
            "content".to_string(),
            serde_json::Value::String("Replaced content".to_string()),
        );

        let result2 = tool.execute(arguments, &context).await;
        assert!(result2.is_ok());

        // Verify that the memo was replaced (response is still just "OK")
        let call_result = result2.unwrap();
        let response_text = call_result.content[0].as_text().unwrap().text.as_str();
        assert_eq!(response_text, "OK");
    }
}
