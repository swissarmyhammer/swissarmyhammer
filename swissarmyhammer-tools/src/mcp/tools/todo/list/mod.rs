use crate::mcp::shared_utils::McpErrorHandler;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::json;
use swissarmyhammer_todo::{ListTodosRequest, TodoStorage};

/// MCP tool for listing todo items
#[derive(Default)]
pub struct ListTodoTool;

impl ListTodoTool {
    /// Creates a new instance of the ListTodoTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for ListTodoTool {
    fn name(&self) -> &'static str {
        "todo_list"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "completed": {
                    "type": ["boolean", "null"],
                    "description": "Filter by completion status (true=completed, false=incomplete, null=all)"
                }
            }
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        // Parse arguments using base tool implementation
        let request: ListTodosRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Listing todos with filter: {:?}", request.completed);

        // Create storage instance using working_dir if available, otherwise use default
        let storage = if let Some(ref working_dir) = _context.working_dir {
            TodoStorage::new_with_working_dir(working_dir.clone())
        } else {
            TodoStorage::new_default()
        }
        .map_err(|e| McpErrorHandler::handle_todo_error(e, "create todo storage"))?;

        // Get the todo list
        match storage.get_todo_list().await {
            Ok(Some(list)) => {
                // Filter by completion status if requested
                let filtered_todos: Vec<_> = match request.completed {
                    None => list.todo.clone(),
                    Some(done) => list
                        .todo
                        .iter()
                        .filter(|t| t.done == done)
                        .cloned()
                        .collect(),
                };

                // Sort: incomplete first, then by ID (which is time-ordered via ULID)
                let mut sorted_todos = filtered_todos;
                sorted_todos.sort_by_key(|t| (t.done, t.id.clone()));

                let completed_count = sorted_todos.iter().filter(|t| t.done).count();
                let pending_count = sorted_todos.len() - completed_count;

                tracing::info!(
                    "Found {} todos ({} pending, {} completed)",
                    sorted_todos.len(),
                    pending_count,
                    completed_count
                );

                Ok(BaseToolImpl::create_success_response(
                    json!({
                        "todos": sorted_todos.iter().map(|item| json!({
                            "id": item.id.as_str(),
                            "task": item.task,
                            "context": item.context,
                            "done": item.done
                        })).collect::<Vec<_>>(),
                        "total": sorted_todos.len(),
                        "completed": completed_count,
                        "pending": pending_count
                    })
                    .to_string(),
                ))
            }
            Ok(None) => {
                tracing::info!("No todo list found");
                Ok(BaseToolImpl::create_success_response(
                    json!({
                        "todos": [],
                        "total": 0,
                        "completed": 0,
                        "pending": 0
                    })
                    .to_string(),
                ))
            }
            Err(e) => Err(McpErrorHandler::handle_todo_error(e, "get todo list")),
        }
    }
}
