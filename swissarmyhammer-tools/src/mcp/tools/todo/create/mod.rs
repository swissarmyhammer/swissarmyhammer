use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::json;
use swissarmyhammer_todo::CreateTodoRequest;
use swissarmyhammer_todo::TodoStorage;

/// MCP tool for creating new todo items
#[derive(Default)]
pub struct CreateTodoTool;

impl CreateTodoTool {
    /// Creates a new instance of the CreateTodoTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for CreateTodoTool {
    fn name(&self) -> &'static str {
        "todo_create"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "Brief description of the task to be completed"
                },
                "context": {
                    "type": ["string", "null"],
                    "description": "Optional additional context, notes, or implementation details"
                }
            },
            "required": ["task"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        // Parse arguments using base tool implementation
        let request: CreateTodoRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Creating todo item");

        // Validate task
        McpValidation::validate_not_empty(&request.task, "task description")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate task description"))?;

        // Create storage instance
        let storage = TodoStorage::new_default()
            .map_err(|e| McpErrorHandler::handle_todo_error(e, "create todo storage"))?;

        // Create the todo item
        match storage
            .create_todo_item(request.task, request.context)
            .await
        {
            Ok(item) => {
                tracing::info!("Created todo item {}", item.id);
                Ok(BaseToolImpl::create_success_response(
                    json!({
                        "message": "Created todo item",
                        "todo_item": {
                            "id": item.id.as_str(),
                            "task": item.task,
                            "context": item.context,
                            "done": item.done
                        }
                    })
                    .to_string(),
                ))
            }
            Err(e) => Err(McpErrorHandler::handle_todo_error(e, "create todo item")),
        }
    }
}
