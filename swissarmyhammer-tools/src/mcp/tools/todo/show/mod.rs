use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::json;
use swissarmyhammer_todo::ShowTodoRequest;
use swissarmyhammer_todo::TodoStorage;

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
                "item": {
                    "type": "string",
                    "description": "Either a specific ULID or \"next\" to show the next incomplete item"
                }
            },
            "required": ["item"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        // Parse arguments using base tool implementation
        let request: ShowTodoRequest = BaseToolImpl::parse_arguments(arguments)?;



        tracing::debug!("Showing todo item '{}'", request.item);

        // Validate item identifier
        McpValidation::validate_not_empty(&request.item, "item identifier")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate item identifier"))?;

        // Create storage instance
        let storage = TodoStorage::new_default()
            .map_err(|e| McpErrorHandler::handle_todo_error(e, "create todo storage"))?;

        // Get the requested todo item
        match storage
            .get_todo_item(&request.item)
            .await
        {
            Ok(Some(item)) => {
                tracing::info!("Found todo item {}", item.id);
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
                            "message": "No incomplete todo items found",
                            "todo_item": null
                        }).to_string()
                    ))
                } else {
                    Err(McpError::invalid_request(
                        format!("Todo item '{}' not found", request.item),
                        None,
                    ))
                }
            }
            Err(e) => Err(McpErrorHandler::handle_todo_error(e, "get todo item")),
        }
    }
}
