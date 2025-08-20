use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::types::ShowTodoRequest;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use serde_json::json;
use swissarmyhammer::todo::TodoStorage;

/// MCP tool for showing todo items
#[derive(Default)]
pub struct ShowTodoTool;

impl ShowTodoTool {
    /// Creates a new instance of the ShowTodoTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for ShowTodoTool {
    fn name(&self) -> &'static str {
        "todo_show"
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
                "item": {
                    "type": "string",
                    "description": "Either a specific ULID or \"next\" to show the next incomplete item"
                }
            },
            "required": ["todo_list", "item"]
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
        let request: ShowTodoRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Apply rate limiting
        context
            .rate_limiter
            .check_rate_limit("unknown", "todo_show", 1)
            .map_err(|e| {
                tracing::warn!("Rate limit exceeded for todo show: {}", e);
                McpError::invalid_params(e.to_string(), None)
            })?;

        tracing::debug!(
            "Showing todo item '{}' from list: {}",
            request.item,
            request.todo_list
        );

        // Validate todo list name and item identifier
        McpValidation::validate_not_empty(&request.todo_list, "todo list name")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate todo list name"))?;

        McpValidation::validate_not_empty(&request.item, "item identifier")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate item identifier"))?;

        // Create storage instance
        let storage = TodoStorage::new_default()
            .map_err(|e| McpErrorHandler::handle_error(e, "create todo storage"))?;

        // Get the requested todo item
        match storage
            .get_todo_item(&request.todo_list, &request.item)
            .await
        {
            Ok(Some(item)) => {
                tracing::info!("Found todo item {} in list {}", item.id, request.todo_list);
                Ok(BaseToolImpl::create_success_response(
                    json!({
                        "todo_item": {
                            "id": item.id.as_str(),
                            "task": item.task,
                            "context": item.context,
                            "done": item.done
                        },
                        "yaml": format!(
                            "id: {}\ntask: \"{}\"\ncontext: {}\ndone: {}",
                            item.id.as_str(),
                            item.task,
                            match &item.context {
                                Some(ctx) => format!("\"{ctx}\""),
                                None => "null".to_string(),
                            },
                            item.done
                        )
                    })
                    .to_string(),
                ))
            }
            Ok(None) => {
                if request.item == "next" {
                    Ok(BaseToolImpl::create_success_response(
                        json!({
                            "message": format!("No incomplete todo items found in list '{}'", request.todo_list),
                            "todo_item": null
                        }).to_string()
                    ))
                } else {
                    Err(McpError::invalid_request(
                        format!(
                            "Todo item '{}' not found in list '{}'",
                            request.item, request.todo_list
                        ),
                        None,
                    ))
                }
            }
            Err(e) => Err(McpErrorHandler::handle_error(e, "get todo item")),
        }
    }
}
