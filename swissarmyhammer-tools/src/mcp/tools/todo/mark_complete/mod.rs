use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::types::MarkCompleteTodoRequest;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use serde_json::json;
use swissarmyhammer::todo::{TodoId, TodoStorage};

/// MCP tool for marking todo items as complete
#[derive(Default)]
pub struct MarkCompleteTodoTool;

impl MarkCompleteTodoTool {
    /// Creates a new instance of the MarkCompleteTodoTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for MarkCompleteTodoTool {
    fn name(&self) -> &'static str {
        "todo_mark_complete"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "todo_list": {
                    "type": "string",
                    "description": "Name of the todo list file (without extension)"
                },
                "id": {
                    "type": "string",
                    "description": "ULID of the todo item to mark as complete"
                }
            },
            "required": ["todo_list", "id"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        // Parse arguments using base tool implementation
        let request: MarkCompleteTodoRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Apply rate limiting
        context
            .rate_limiter
            .check_rate_limit("unknown", "todo_mark_complete", 1)
            .map_err(|e| {
                tracing::warn!("Rate limit exceeded for todo mark complete: {}", e);
                McpError::invalid_params(e.to_string(), None)
            })?;

        tracing::debug!("Marking todo item '{}' complete in list: {}", request.id, request.todo_list);

        // Validate todo list name and ID
        McpValidation::validate_not_empty(&request.todo_list, "todo list name")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate todo list name"))?;
        
        McpValidation::validate_not_empty(&request.id, "todo item ID")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate todo item ID"))?;

        // Create storage instance
        let storage = TodoStorage::new_default()
            .map_err(|e| McpErrorHandler::handle_error(e, "create todo storage"))?;

        // Parse the todo ID
        let todo_id = TodoId::from_string(request.id.clone())
            .map_err(|e| McpErrorHandler::handle_error(e, "parse todo ID"))?;

        // Mark the item as complete
        match storage.mark_todo_complete(&request.todo_list, &todo_id).await {
            Ok(()) => {
                tracing::info!("Marked todo item {} complete in list {}", todo_id, request.todo_list);
                Ok(BaseToolImpl::create_success_response(
                    json!({
                        "message": format!("Marked todo item '{}' as complete in list '{}'", todo_id, request.todo_list),
                        "action": "marked_complete",
                        "todo_list": request.todo_list,
                        "id": todo_id.as_str()
                    }).to_string()
                ))
            }
            Err(e) => Err(McpErrorHandler::handle_error(e, "mark todo item complete")),
        }
    }
}