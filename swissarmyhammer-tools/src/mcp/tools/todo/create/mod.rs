use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::types::CreateTodoRequest;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use serde_json::json;
use swissarmyhammer::todo::TodoStorage;

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
                "todo_list": {
                    "type": "string",
                    "description": "Name of the todo list file (without extension)"
                },
                "task": {
                    "type": "string",
                    "description": "Brief description of the task to be completed"
                },
                "context": {
                    "type": ["string", "null"],
                    "description": "Optional additional context, notes, or implementation details"
                }
            },
            "required": ["todo_list", "task"]
        })
    }

    fn hidden_from_cli(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        // Parse arguments using base tool implementation
        let request: CreateTodoRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Apply rate limiting
        context
            .rate_limiter
            .check_rate_limit("unknown", "todo_create", 1)
            .map_err(|e| {
                tracing::warn!("Rate limit exceeded for todo creation: {}", e);
                McpError::invalid_params(e.to_string(), None)
            })?;

        tracing::debug!("Creating todo item in list: {}", request.todo_list);

        // Validate todo list name and task
        McpValidation::validate_not_empty(&request.todo_list, "todo list name")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate todo list name"))?;

        McpValidation::validate_not_empty(&request.task, "task description")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate task description"))?;

        // Create storage instance
        let storage = TodoStorage::new_default()
            .map_err(|e| McpErrorHandler::handle_error(e, "create todo storage"))?;

        // Create the todo item
        match storage
            .create_todo_item(&request.todo_list, request.task, request.context)
            .await
        {
            Ok(item) => {
                tracing::info!(
                    "Created todo item {} in list {}",
                    item.id,
                    request.todo_list
                );
                Ok(BaseToolImpl::create_success_response(
                    json!({
                        "message": format!("Created todo item in list '{}'", request.todo_list),
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
            Err(e) => Err(McpErrorHandler::handle_error(e, "create todo item")),
        }
    }
}
