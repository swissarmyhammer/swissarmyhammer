use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::json;
use swissarmyhammer_todo::MarkCompleteTodoRequest;
use swissarmyhammer_todo::TodoStorage;

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
                "id": {
                    "type": "string",
                    "description": "ULID of the todo item to mark as complete"
                }
            },
            "required": ["id"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        // Parse arguments using base tool implementation
        let request: MarkCompleteTodoRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Marking todo item '{}' complete", request.id);

        // Validate ID
        McpValidation::validate_not_empty(request.id.as_str(), "todo item ID")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate todo item ID"))?;

        // Create storage instance
        let storage = TodoStorage::new_default()
            .map_err(|e| McpErrorHandler::handle_todo_error(e, "create todo storage"))?;

        // The request.id is already a TodoId from the swissarmyhammer-todo crate

        // Mark the item as complete
        match storage.mark_todo_complete(&request.id).await {
            Ok(()) => {
                tracing::info!("Marked todo item {} complete", request.id);
                Ok(BaseToolImpl::create_success_response(
                    json!({
                        "message": format!("Marked todo item '{}' as complete", request.id),
                        "action": "marked_complete",
                        "id": request.id.as_str()
                    })
                    .to_string(),
                ))
            }
            Err(e) => Err(McpErrorHandler::handle_todo_error(
                e,
                "mark todo item complete",
            )),
        }
    }
}
