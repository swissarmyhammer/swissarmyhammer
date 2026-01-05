use crate::mcp::progress_notifications::generate_progress_token;
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

    fn cli_name(&self) -> &'static str {
        "complete"
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        // Parse arguments using base tool implementation
        let request: MarkCompleteTodoRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Marking todo item '{}' complete", request.id);

        // Generate progress token for notifications
        let progress_token = generate_progress_token();

        // Validate ID
        McpValidation::validate_not_empty(request.id.as_str(), "todo item ID")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate todo item ID"))?;

        // Create storage instance using working_dir if available, otherwise use default
        let storage = if let Some(ref working_dir) = context.working_dir {
            TodoStorage::new_with_working_dir(working_dir.clone())
        } else {
            TodoStorage::new_default()
        }
        .map_err(|e| McpErrorHandler::handle_todo_error(e, "create todo storage"))?;

        // The request.id is already a TodoId from the swissarmyhammer-todo crate

        // Mark the item as complete
        match storage.mark_todo_complete(&request.id).await {
            Ok(()) => {
                tracing::info!("Marked todo item {} complete", request.id);

                // Send progress notification for todo completion
                if let Some(sender) = &context.progress_sender {
                    if let Err(e) = sender.send_progress_with_metadata(
                        &progress_token,
                        Some(100),
                        "Todo completed",
                        json!({
                            "action": "todo_completed",
                            "todo_id": request.id.as_str()
                        }),
                    ) {
                        tracing::warn!("Failed to send todo completion notification: {}", e);
                    }
                }

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
