//! Kanban board management tool
//!
//! This module provides a single MCP tool for kanban board operations.
//! The tool exposes its operations via the Operation trait for CLI generation.
//!
//! # Plan Notifications
//!
//! When tasks are modified (add, update, delete, move, complete), the tool emits
//! plan notifications through the `plan_sender` channel in `ToolContext`. These
//! notifications contain the complete task list in a format compatible with ACP
//! (Agent Client Protocol) plan updates.
//!
//! Per ACP spec: "Complete plan lists must be resent with each update; clients
//! will replace prior plans entirely."

use crate::mcp::plan_notifications::{PlanEntry, PlanEntryPriority, PlanEntryStatus};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext, ToolRegistry};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::{json, Value};
use std::path::PathBuf;
use swissarmyhammer_kanban::{
    board::{GetBoard, InitBoard, UpdateBoard},
    column::{AddColumn, DeleteColumn, GetColumn, ListColumns, UpdateColumn},
    parse::parse_input,
    task::{AddTask, CompleteTask, DeleteTask, GetTask, ListTasks, MoveTask, NextTask, UpdateTask},
    Execute, KanbanContext, KanbanOperation, Noun, Operation, Verb,
};

// Static operation instances for metadata access
// These are used by the CLI to generate subcommands from operation metadata

static INIT_BOARD: Lazy<InitBoard> = Lazy::new(|| InitBoard::new(""));
static GET_BOARD: Lazy<GetBoard> = Lazy::new(GetBoard::default);
static UPDATE_BOARD: Lazy<UpdateBoard> = Lazy::new(UpdateBoard::new);

static ADD_COLUMN: Lazy<AddColumn> = Lazy::new(|| AddColumn::new("", ""));
static GET_COLUMN: Lazy<GetColumn> = Lazy::new(|| GetColumn::new(""));
static UPDATE_COLUMN: Lazy<UpdateColumn> = Lazy::new(|| UpdateColumn::new(""));
static DELETE_COLUMN: Lazy<DeleteColumn> = Lazy::new(|| DeleteColumn::new(""));
static LIST_COLUMNS: Lazy<ListColumns> = Lazy::new(ListColumns::default);

static ADD_TASK: Lazy<AddTask> = Lazy::new(|| AddTask::new(""));
static GET_TASK: Lazy<GetTask> = Lazy::new(|| GetTask::new(""));
static UPDATE_TASK: Lazy<UpdateTask> = Lazy::new(|| UpdateTask::new(""));
static DELETE_TASK: Lazy<DeleteTask> = Lazy::new(|| DeleteTask::new(""));
static MOVE_TASK: Lazy<MoveTask> = Lazy::new(|| MoveTask::to_column("", ""));
static COMPLETE_TASK: Lazy<CompleteTask> = Lazy::new(|| CompleteTask::new(""));
static NEXT_TASK: Lazy<NextTask> = Lazy::new(NextTask::new);
static LIST_TASKS: Lazy<ListTasks> = Lazy::new(ListTasks::new);

/// All kanban operations for CLI generation
static KANBAN_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        // Board operations
        &*INIT_BOARD as &dyn Operation,
        &*GET_BOARD as &dyn Operation,
        &*UPDATE_BOARD as &dyn Operation,
        // Column operations
        &*ADD_COLUMN as &dyn Operation,
        &*GET_COLUMN as &dyn Operation,
        &*UPDATE_COLUMN as &dyn Operation,
        &*DELETE_COLUMN as &dyn Operation,
        &*LIST_COLUMNS as &dyn Operation,
        // Task operations
        &*ADD_TASK as &dyn Operation,
        &*GET_TASK as &dyn Operation,
        &*UPDATE_TASK as &dyn Operation,
        &*DELETE_TASK as &dyn Operation,
        &*MOVE_TASK as &dyn Operation,
        &*COMPLETE_TASK as &dyn Operation,
        &*NEXT_TASK as &dyn Operation,
        &*LIST_TASKS as &dyn Operation,
    ]
});

/// MCP tool for kanban board operations
#[derive(Default)]
pub struct KanbanTool;

impl KanbanTool {
    /// Creates a new instance of the KanbanTool
    pub fn new() -> Self {
        Self
    }

    /// Get the kanban context from the tool context
    fn get_kanban_context(context: &ToolContext) -> Result<KanbanContext, McpError> {
        let working_dir = context
            .working_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));

        let kanban_dir = working_dir.join(".kanban");

        Ok(KanbanContext::new(kanban_dir))
    }
}

/// Convert a kanban task JSON value to a PlanEntry
///
/// Maps kanban task structure to ACP-compatible plan entry format:
/// - task.title → PlanEntry.content
/// - task.position.column → PlanEntry.status (done → Completed, doing → InProgress, else → Pending)
/// - All entries get Medium priority (kanban doesn't track priority)
fn task_to_plan_entry(task: &Value) -> PlanEntry {
    let column = task["position"]["column"].as_str().unwrap_or("todo");

    let status = match column {
        "done" => PlanEntryStatus::Completed,
        "doing" => PlanEntryStatus::InProgress,
        _ => PlanEntryStatus::Pending,
    };

    let id = task["id"].as_str().unwrap_or("").to_string();
    let title = task["title"].as_str().unwrap_or("").to_string();

    let mut entry = PlanEntry::new(id, title, status, PlanEntryPriority::Medium);

    if let Some(desc) = task["description"].as_str() {
        entry = entry.with_notes(desc);
    }

    entry.with_column(column)
}

/// Build plan data from current kanban tasks
///
/// Returns a JSON object containing the complete plan in a format that can be
/// converted to ACP Plan by the agent. The plan is embedded in tool responses
/// under the `_plan` key.
///
/// Per ACP spec: "Complete plan lists must be resent with each update"
async fn build_plan_data(ctx: &KanbanContext, trigger: &str, affected_task_id: Option<&str>) -> Option<Value> {
    // Fetch all tasks
    let tasks_result = ListTasks::new().execute(ctx).await;
    let tasks = match tasks_result {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("Failed to list tasks for plan: {}", e);
            return None;
        }
    };

    // Convert tasks to plan entries
    let entries: Vec<Value> = tasks
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|task| {
            let entry = task_to_plan_entry(task);
            json!({
                "content": entry.content,
                "status": match entry.status {
                    PlanEntryStatus::Pending => "pending",
                    PlanEntryStatus::InProgress => "in_progress",
                    PlanEntryStatus::Completed => "completed",
                },
                "priority": match entry.priority {
                    PlanEntryPriority::High => "high",
                    PlanEntryPriority::Medium => "medium",
                    PlanEntryPriority::Low => "low",
                },
                "_meta": {
                    "id": entry.id,
                    "column": entry.column,
                    "notes": entry.notes,
                }
            })
        })
        .collect();

    let mut plan = json!({
        "entries": entries,
        "_meta": {
            "source": "swissarmyhammer_kanban",
            "trigger": trigger,
        }
    });

    if let Some(id) = affected_task_id {
        plan["_meta"]["affected_task_id"] = json!(id);
    }

    Some(plan)
}

/// Check if an operation modifies tasks (and should trigger plan notification)
fn is_task_modifying_operation(verb: Verb, noun: Noun) -> bool {
    matches!(
        (verb, noun),
        (Verb::Add, Noun::Task)
            | (Verb::Update, Noun::Task)
            | (Verb::Delete, Noun::Task)
            | (Verb::Move, Noun::Task)
            | (Verb::Complete, Noun::Task)
    )
}

// No health checks needed
crate::impl_empty_doctorable!(KanbanTool);

#[async_trait]
impl McpTool for KanbanTool {
    fn name(&self) -> &'static str {
        "kanban"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": true,
            "description": "Kanban board operations. Accepts verb+noun operations like 'add task', 'move task', 'list tasks', etc.",
            "properties": {
                "op": {
                    "type": "string",
                    "description": "Operation to perform (e.g., 'add task', 'move task', 'list tasks', 'init board')"
                },
                "id": {
                    "type": "string",
                    "description": "ID of the task or column to operate on"
                },
                "title": {
                    "type": "string",
                    "description": "Title for new tasks"
                },
                "name": {
                    "type": "string",
                    "description": "Name for boards or columns"
                },
                "description": {
                    "type": "string",
                    "description": "Description for tasks or boards"
                },
                "column": {
                    "type": "string",
                    "description": "Target column ID for move operations or filtering"
                }
            }
        })
    }

    fn operations(&self) -> &'static [&'static dyn swissarmyhammer_operations::Operation] {
        // Force initialization of the lazy static
        let ops: &[&'static dyn Operation] = &KANBAN_OPERATIONS;
        // This is safe because KANBAN_OPERATIONS is a static Lazy<Vec<...>>
        // We need to convert to a slice with 'static lifetime
        // SAFETY: The Lazy is initialized once and lives for 'static
        unsafe {
            std::mem::transmute::<&[&dyn Operation], &'static [&'static dyn swissarmyhammer_operations::Operation]>(ops)
        }
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let ctx = Self::get_kanban_context(_context)?;

        // Parse the input to get operations
        let input = Value::Object(arguments);
        let operations = parse_input(input).map_err(|e| {
            McpError::invalid_params(format!("Failed to parse kanban operation: {}", e), None)
        })?;

        // Execute each operation and collect results
        let mut results = Vec::new();
        let mut should_include_plan = false;
        let mut last_affected_task_id: Option<String> = None;
        let mut last_trigger = String::new();

        for op in &operations {
            let result = execute_operation(&ctx, op).await?;

            // Track if we need to include plan in response
            if is_task_modifying_operation(op.verb, op.noun) {
                should_include_plan = true;
                last_trigger = op.op_string();

                // Extract the affected task ID from the result if available
                if let Some(id) = result["id"].as_str() {
                    last_affected_task_id = Some(id.to_string());
                }
            }

            results.push(result);
        }

        // Build response with plan data if any task-modifying operations were executed
        let mut response = if results.len() == 1 {
            results.into_iter().next().unwrap()
        } else {
            json!(results)
        };

        // Include plan data in response for task-modifying operations
        // This enables ACP agents to emit Plan notifications
        if should_include_plan {
            if let Some(plan) = build_plan_data(&ctx, &last_trigger, last_affected_task_id.as_deref()).await {
                // Wrap in object if needed and add _plan key
                if let Value::Object(ref mut map) = response {
                    map.insert("_plan".to_string(), plan);
                } else {
                    response = json!({
                        "result": response,
                        "_plan": plan
                    });
                }
                tracing::debug!("Included plan data in kanban response: trigger={}", last_trigger);
            }
        }

        Ok(BaseToolImpl::create_success_response(
            serde_json::to_string_pretty(&response).unwrap_or_else(|_| response.to_string()),
        ))
    }
}

/// Execute a single kanban operation
async fn execute_operation(ctx: &KanbanContext, op: &KanbanOperation) -> Result<Value, McpError> {
    use Noun::*;
    use Verb::*;

    let result = match (op.verb, op.noun) {
        // Board operations
        (Init, Board) => {
            let name = op
                .get_string("name")
                .ok_or_else(|| McpError::invalid_params("missing required field: name", None))?;
            let description = op.get_string("description").map(String::from);

            let mut cmd = InitBoard::new(name);
            if let Some(desc) = description {
                cmd = cmd.with_description(desc);
            }
            cmd.execute(ctx).await
        }
        (Get, Board) => GetBoard.execute(ctx).await,
        (Update, Board) => {
            let mut cmd = UpdateBoard::new();
            if let Some(name) = op.get_string("name") {
                cmd = cmd.with_name(name);
            }
            if let Some(desc) = op.get_string("description") {
                cmd = cmd.with_description(desc);
            }
            cmd.execute(ctx).await
        }

        // Column operations
        (Add, Column) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            let name = op
                .get_string("name")
                .ok_or_else(|| McpError::invalid_params("missing required field: name", None))?;

            let mut cmd = AddColumn::new(id, name);
            if let Some(order) = op.get_param("order").and_then(|v| v.as_u64()) {
                cmd = cmd.with_order(order as usize);
            }
            cmd.execute(ctx).await
        }
        (Get, Column) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            GetColumn::new(id).execute(ctx).await
        }
        (Update, Column) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;

            let mut cmd = UpdateColumn::new(id);
            if let Some(name) = op.get_string("name") {
                cmd = cmd.with_name(name);
            }
            if let Some(order) = op.get_param("order").and_then(|v| v.as_u64()) {
                cmd = cmd.with_order(order as usize);
            }
            cmd.execute(ctx).await
        }
        (Delete, Column) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            DeleteColumn::new(id).execute(ctx).await
        }
        (List, Columns) => ListColumns.execute(ctx).await,

        // Task operations
        (Add, Task) => {
            let title = op
                .get_string("title")
                .ok_or_else(|| McpError::invalid_params("missing required field: title", None))?;

            let mut cmd = AddTask::new(title);
            if let Some(desc) = op.get_string("description") {
                cmd = cmd.with_description(desc);
            }
            // Could add position, tags, assignees, depends_on parsing here
            cmd.execute(ctx).await
        }
        (Get, Task) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            GetTask::new(id).execute(ctx).await
        }
        (Update, Task) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;

            let mut cmd = UpdateTask::new(id);
            if let Some(title) = op.get_string("title") {
                cmd = cmd.with_title(title);
            }
            if let Some(desc) = op.get_string("description") {
                cmd = cmd.with_description(desc);
            }
            cmd.execute(ctx).await
        }
        (Move, Task) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            let column = op
                .get_string("column")
                .ok_or_else(|| McpError::invalid_params("missing required field: column", None))?;

            MoveTask::to_column(id, column).execute(ctx).await
        }
        (Delete, Task) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            DeleteTask::new(id).execute(ctx).await
        }
        (Complete, Task) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            CompleteTask::new(id).execute(ctx).await
        }
        (Next, Task) => {
            let cmd = NextTask::new();
            // Could add swimlane/assignee filtering here
            cmd.execute(ctx).await
        }
        (List, Tasks) => {
            let mut cmd = ListTasks::new();
            if let Some(column) = op.get_string("column") {
                cmd = cmd.with_column(column);
            }
            if let Some(ready) = op.get_param("ready").and_then(|v| v.as_bool()) {
                cmd = cmd.with_ready(ready);
            }
            cmd.execute(ctx).await
        }

        // Unsupported operations
        _ => {
            return Err(McpError::invalid_params(
                format!("unsupported operation: {} {}", op.verb, op.noun),
                None,
            ));
        }
    };

    result.map_err(|e| McpError::internal_error(format!("{}: {}", op.op_string(), e), None))
}

/// Register all kanban tools with the tool registry
pub fn register_kanban_tools(registry: &mut ToolRegistry) {
    registry.register(KanbanTool);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_init_board() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context().await.with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("init board"));
        args.insert("name".to_string(), json!("Test Board"));

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_task() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context().await.with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();

        // First init the board
        let mut init_args = serde_json::Map::new();
        init_args.insert("op".to_string(), json!("init board"));
        init_args.insert("name".to_string(), json!("Test"));
        tool.execute(init_args, &context).await.unwrap();

        // Then add a task
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add task"));
        add_args.insert("title".to_string(), json!("Test Task"));

        let result = tool.execute(add_args, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_inferred_operation() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context().await.with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();

        // Init board first
        let mut init_args = serde_json::Map::new();
        init_args.insert("op".to_string(), json!("init board"));
        init_args.insert("name".to_string(), json!("Test"));
        tool.execute(init_args, &context).await.unwrap();

        // Add task with inferred operation (just title)
        let mut add_args = serde_json::Map::new();
        add_args.insert("title".to_string(), json!("Inferred Task"));

        let result = tool.execute(add_args, &context).await;
        assert!(result.is_ok());
    }
}
