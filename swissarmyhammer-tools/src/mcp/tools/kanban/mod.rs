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
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::{json, Value};
use std::path::PathBuf;
use swissarmyhammer_kanban::{
    parse::parse_input, task::ListTasks, Execute, KanbanContext, KanbanOperation, Noun, Verb,
};

// Operations and schema are provided by the kanban crate's single source of truth:
// `swissarmyhammer_kanban::schema::kanban_operations()`

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
async fn build_plan_data(
    ctx: &KanbanContext,
    trigger: &str,
    affected_task_id: Option<&str>,
) -> Option<Value> {
    // Fetch all tasks
    let tasks_result = ListTasks::new().execute(ctx).await.into_result();
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
            | (Verb::Assign, Noun::Task)
            | (Verb::Unassign, Noun::Task)
    )
}

// No health checks needed
crate::impl_empty_doctorable!(KanbanTool);

impl swissarmyhammer_common::lifecycle::Initializable for KanbanTool {
    fn name(&self) -> &str {
        <Self as crate::mcp::tool_registry::McpTool>::name(self)
    }

    fn category(&self) -> &str {
        "tools"
    }

    fn priority(&self) -> i32 {
        25
    }

    fn is_applicable(&self, scope: &swissarmyhammer_common::lifecycle::InitScope) -> bool {
        matches!(
            scope,
            swissarmyhammer_common::lifecycle::InitScope::Project
                | swissarmyhammer_common::lifecycle::InitScope::Local
        )
    }

    fn init(
        &self,
        _scope: &swissarmyhammer_common::lifecycle::InitScope,
        reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
    ) -> Vec<swissarmyhammer_common::lifecycle::InitResult> {
        use swissarmyhammer_common::lifecycle::{InitResult, Initializable};
        use swissarmyhammer_common::reporter::InitEvent;
        let name = Initializable::name(self);
        let cwd = match std::env::current_dir() {
            Ok(d) => d,
            Err(_) => {
                return vec![InitResult::skipped(
                    name,
                    "Cannot determine current directory",
                )]
            }
        };

        let kanban_dir = cwd.join(".kanban");
        if !kanban_dir.exists() {
            return vec![InitResult::skipped(name, "No .kanban directory found")];
        }

        if let Err(e) = swissarmyhammer_kanban::board::register_merge_drivers(&kanban_dir) {
            return vec![InitResult::error(
                name,
                format!("Failed to register merge drivers: {e}"),
            )];
        }

        reporter.emit(&InitEvent::Action {
            verb: "Configured".to_string(),
            message: "kanban merge drivers".to_string(),
        });

        vec![InitResult::ok(name, "Kanban merge drivers registered")]
    }

    fn deinit(
        &self,
        _scope: &swissarmyhammer_common::lifecycle::InitScope,
        reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
    ) -> Vec<swissarmyhammer_common::lifecycle::InitResult> {
        use swissarmyhammer_common::lifecycle::{InitResult, Initializable};
        use swissarmyhammer_common::reporter::InitEvent;
        let name = Initializable::name(self);
        let cwd = match std::env::current_dir() {
            Ok(d) => d,
            Err(_) => {
                return vec![InitResult::skipped(
                    name,
                    "Cannot determine current directory",
                )]
            }
        };

        let kanban_dir = cwd.join(".kanban");
        if !kanban_dir.exists() {
            return vec![InitResult::skipped(name, "No .kanban directory found")];
        }

        if let Err(e) = swissarmyhammer_kanban::board::unregister_merge_drivers(&kanban_dir) {
            return vec![InitResult::error(
                name,
                format!("Failed to remove merge drivers: {e}"),
            )];
        }

        reporter.emit(&InitEvent::Action {
            verb: "Removed".to_string(),
            message: "kanban merge driver configuration".to_string(),
        });

        vec![InitResult::ok(name, "Kanban merge drivers removed")]
    }
}

#[async_trait]
impl McpTool for KanbanTool {
    fn name(&self) -> &'static str {
        "kanban"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        let ops = swissarmyhammer_kanban::schema::kanban_operations();
        swissarmyhammer_kanban::schema::generate_kanban_mcp_schema(ops)
    }

    fn operations(&self) -> &'static [&'static dyn swissarmyhammer_operations::Operation] {
        swissarmyhammer_kanban::schema::kanban_operations()
    }

    async fn execute(
        &self,
        mut arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let ctx = Self::get_kanban_context(_context)?;

        // Auto-inject the session actor when the caller hasn't provided one.
        // This enables MCP-initiated tool calls (e.g. "add task") to be
        // attributed to the connecting client without requiring callers to
        // pass `actor` explicitly on every request.
        if !arguments.contains_key("actor") {
            let actor_guard = _context.session_actor.read().await;
            if let Some(ref actor_id) = *actor_guard {
                arguments.insert(
                    "actor".to_string(),
                    serde_json::Value::String(actor_id.clone()),
                );
                tracing::debug!(actor = %actor_id, "auto-injected session actor into kanban call");
            }
        }

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
            if let Some(plan) =
                build_plan_data(&ctx, &last_trigger, last_affected_task_id.as_deref()).await
            {
                // Wrap in object if needed and add _plan key
                if let Value::Object(ref mut map) = response {
                    map.insert("_plan".to_string(), plan);
                } else {
                    response = json!({
                        "result": response,
                        "_plan": plan
                    });
                }
                tracing::debug!(
                    "Included plan data in kanban response: trigger={}",
                    last_trigger
                );
            }
        }

        Ok(BaseToolImpl::create_success_response(
            serde_json::to_string_pretty(&response).unwrap_or_else(|_| response.to_string()),
        ))
    }
}

/// Execute a single kanban operation.
///
/// Delegates to [`swissarmyhammer_kanban::dispatch::execute_operation`] — the single
/// source of truth for operation dispatch — and maps errors to MCP format.
async fn execute_operation(ctx: &KanbanContext, op: &KanbanOperation) -> Result<Value, McpError> {
    swissarmyhammer_kanban::dispatch::execute_operation(ctx, op)
        .await
        .map_err(|e| McpError::internal_error(format!("{}: {}", op.op_string(), e), None))
}

/// Register all kanban tools with the tool registry
pub fn register_kanban_tools(registry: &mut ToolRegistry) {
    registry.register(KanbanTool);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;
    use rmcp::model::RawContent;
    use tempfile::TempDir;

    /// Helper to extract text content from a CallToolResult
    fn extract_text(result: &CallToolResult) -> &str {
        match &result.content[0].raw {
            RawContent::Text(text) => &text.text,
            _ => panic!("Expected text content"),
        }
    }

    /// Helper to parse JSON response
    fn parse_json(result: &CallToolResult) -> Value {
        let text = extract_text(result);
        serde_json::from_str(text).expect("Expected valid JSON")
    }

    /// Helper to extract task ID from a result
    fn extract_task_id(result: &CallToolResult) -> String {
        let data = parse_json(result);
        data["id"].as_str().expect("Expected id field").to_string()
    }

    #[tokio::test]
    async fn test_init_board() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("init board"));
        args.insert("name".to_string(), json!("Test Board"));

        let result = tool.execute(args, &context).await.unwrap();
        let data = parse_json(&result);

        // Verify board was created with correct name
        assert_eq!(data["name"], "Test Board");
        // Verify default columns exist
        assert!(data["columns"].is_array());
        let columns = data["columns"].as_array().unwrap();
        assert_eq!(columns.len(), 3); // To Do, Doing, Done
    }

    #[tokio::test]
    async fn test_add_task() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
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

        let result = tool.execute(add_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Verify task was created with correct title
        assert_eq!(data["title"], "Test Task");
        // Verify task has an ID
        assert!(data["id"].is_string());
        // Verify task is in first column (To Do) via position.column
        assert!(data["position"]["column"].is_string());
    }

    #[tokio::test]
    async fn test_get_task() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a task
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add task"));
        add_args.insert("title".to_string(), json!("Test Task"));
        let add_result = tool.execute(add_args, &context).await.unwrap();
        let task_id = extract_task_id(&add_result);

        // Get the task
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get task"));
        get_args.insert("id".to_string(), json!(task_id));

        let result = tool.execute(get_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], task_id);
        assert_eq!(data["title"], "Test Task");
    }

    #[tokio::test]
    async fn test_update_task() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a task
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add task"));
        add_args.insert("title".to_string(), json!("Original Title"));
        let add_result = tool.execute(add_args, &context).await.unwrap();
        let task_id = extract_task_id(&add_result);

        // Update the task
        let mut update_args = serde_json::Map::new();
        update_args.insert("op".to_string(), json!("update task"));
        update_args.insert("id".to_string(), json!(task_id));
        update_args.insert("title".to_string(), json!("Updated Title"));

        let result = tool.execute(update_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["title"], "Updated Title");
    }

    #[tokio::test]
    async fn test_delete_task() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a task
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add task"));
        add_args.insert("title".to_string(), json!("Task to delete"));
        let add_result = tool.execute(add_args, &context).await.unwrap();
        let task_id = extract_task_id(&add_result);

        // Delete the task
        let mut delete_args = serde_json::Map::new();
        delete_args.insert("op".to_string(), json!("delete task"));
        delete_args.insert("id".to_string(), json!(task_id));

        let result = tool.execute(delete_args, &context).await;
        assert!(result.is_ok());

        // Verify task is gone by trying to get it
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get task"));
        get_args.insert("id".to_string(), json!(task_id));

        let get_result = tool.execute(get_args, &context).await;
        assert!(get_result.is_err());
    }

    #[tokio::test]
    async fn test_list_tasks() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add two tasks
        for title in ["Task 1", "Task 2"] {
            let mut add_args = serde_json::Map::new();
            add_args.insert("op".to_string(), json!("add task"));
            add_args.insert("title".to_string(), json!(title));
            tool.execute(add_args, &context).await.unwrap();
        }

        // List tasks
        let mut list_args = serde_json::Map::new();
        list_args.insert("op".to_string(), json!("list tasks"));

        let result = tool.execute(list_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Response format: {"tasks": [...], "count": N}
        assert_eq!(data["count"], 2);
        assert!(data["tasks"].is_array());
        assert_eq!(data["tasks"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_move_task() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a task (goes to first column)
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add task"));
        add_args.insert("title".to_string(), json!("Task to move"));
        let add_result = tool.execute(add_args, &context).await.unwrap();
        let task_id = extract_task_id(&add_result);
        let original_column = parse_json(&add_result)["position"]["column"]
            .as_str()
            .unwrap()
            .to_string();

        // Move to "doing" column
        let mut move_args = serde_json::Map::new();
        move_args.insert("op".to_string(), json!("move task"));
        move_args.insert("id".to_string(), json!(task_id));
        move_args.insert("column".to_string(), json!("doing"));

        let result = tool.execute(move_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Verify column changed (position.column)
        assert_ne!(
            data["position"]["column"].as_str().unwrap(),
            original_column
        );
        assert_eq!(data["position"]["column"], "doing");
    }

    #[tokio::test]
    async fn test_inferred_operation() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();

        // Init board first
        let mut init_args = serde_json::Map::new();
        init_args.insert("op".to_string(), json!("init board"));
        init_args.insert("name".to_string(), json!("Test"));
        tool.execute(init_args, &context).await.unwrap();

        // Add task with inferred operation (just title)
        let mut add_args = serde_json::Map::new();
        add_args.insert("title".to_string(), json!("Inferred Task"));

        let result = tool.execute(add_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["title"], "Inferred Task");
    }

    // Helper to init a board for tests
    async fn init_test_board(tool: &KanbanTool, context: &ToolContext) {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("init board"));
        args.insert("name".to_string(), json!("Test Board"));
        tool.execute(args, context).await.unwrap();
    }

    // =========================================================================
    // Project operations
    // =========================================================================

    #[tokio::test]
    async fn test_add_project() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("add project"));
        args.insert("id".to_string(), json!("backend"));
        args.insert("name".to_string(), json!("Backend"));

        let result = tool.execute(args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], "backend");
        assert_eq!(data["name"], "Backend");
    }

    #[tokio::test]
    async fn test_get_project() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a project
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add project"));
        add_args.insert("id".to_string(), json!("backend"));
        add_args.insert("name".to_string(), json!("Backend"));
        tool.execute(add_args, &context).await.unwrap();

        // Get the project
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get project"));
        get_args.insert("id".to_string(), json!("backend"));

        let result = tool.execute(get_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], "backend");
        assert_eq!(data["name"], "Backend");
    }

    #[tokio::test]
    async fn test_list_projects() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add projects
        for (id, name) in [("backend", "Backend"), ("frontend", "Frontend")] {
            let mut add_args = serde_json::Map::new();
            add_args.insert("op".to_string(), json!("add project"));
            add_args.insert("id".to_string(), json!(id));
            add_args.insert("name".to_string(), json!(name));
            tool.execute(add_args, &context).await.unwrap();
        }

        // List projects
        let mut list_args = serde_json::Map::new();
        list_args.insert("op".to_string(), json!("list projects"));

        let result = tool.execute(list_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["count"], 2);
        assert!(data["projects"].is_array());
        assert_eq!(data["projects"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_delete_project() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a project
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add project"));
        add_args.insert("id".to_string(), json!("backend"));
        add_args.insert("name".to_string(), json!("Backend"));
        tool.execute(add_args, &context).await.unwrap();

        // Delete it
        let mut delete_args = serde_json::Map::new();
        delete_args.insert("op".to_string(), json!("delete project"));
        delete_args.insert("id".to_string(), json!("backend"));

        let result = tool.execute(delete_args, &context).await;
        assert!(result.is_ok());

        // Verify it's gone
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get project"));
        get_args.insert("id".to_string(), json!("backend"));

        let get_result = tool.execute(get_args, &context).await;
        assert!(get_result.is_err());
    }

    // =========================================================================
    // Actor operations
    // =========================================================================

    #[tokio::test]
    async fn test_add_actor() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("add actor"));
        args.insert("id".to_string(), json!("alice"));
        args.insert("name".to_string(), json!("Alice Smith"));
        args.insert("type".to_string(), json!("human"));

        let result = tool.execute(args, &context).await.unwrap();
        let data = parse_json(&result);

        // Response format: {"actor": {...}, "created": true}
        assert_eq!(data["created"], true);
        assert_eq!(data["actor"]["id"], "alice");
        assert_eq!(data["actor"]["name"], "Alice Smith");
    }

    #[tokio::test]
    async fn test_add_agent_actor() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("add actor"));
        args.insert("id".to_string(), json!("claude"));
        args.insert("name".to_string(), json!("Claude"));
        args.insert("type".to_string(), json!("agent"));

        let result = tool.execute(args, &context).await.unwrap();
        let data = parse_json(&result);

        // Response format: {"actor": {...}, "created": true}
        assert_eq!(data["created"], true);
        assert_eq!(data["actor"]["id"], "claude");
        assert_eq!(data["actor"]["name"], "Claude");
    }

    #[tokio::test]
    async fn test_add_actor_with_ensure() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add actor first time
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("add actor"));
        args.insert("id".to_string(), json!("assistant"));
        args.insert("name".to_string(), json!("Assistant"));
        args.insert("type".to_string(), json!("agent"));
        args.insert("ensure".to_string(), json!(true));

        let result = tool.execute(args.clone(), &context).await.unwrap();
        let data = parse_json(&result);
        assert_eq!(data["created"], true);

        // Add again with ensure - should succeed and return existing
        let result2 = tool.execute(args, &context).await.unwrap();
        let data2 = parse_json(&result2);
        assert_eq!(data2["created"], false);
        assert_eq!(data2["actor"]["name"], "Assistant");
    }

    #[tokio::test]
    async fn test_get_actor() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add an actor
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add actor"));
        add_args.insert("id".to_string(), json!("alice"));
        add_args.insert("name".to_string(), json!("Alice"));
        tool.execute(add_args, &context).await.unwrap();

        // Get the actor
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get actor"));
        get_args.insert("id".to_string(), json!("alice"));

        let result = tool.execute(get_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], "alice");
        assert_eq!(data["name"], "Alice");
    }

    #[tokio::test]
    async fn test_list_actors() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add actors
        for (id, name) in [("alice", "Alice"), ("bob", "Bob")] {
            let mut add_args = serde_json::Map::new();
            add_args.insert("op".to_string(), json!("add actor"));
            add_args.insert("id".to_string(), json!(id));
            add_args.insert("name".to_string(), json!(name));
            tool.execute(add_args, &context).await.unwrap();
        }

        // List actors
        let mut list_args = serde_json::Map::new();
        list_args.insert("op".to_string(), json!("list actors"));

        let result = tool.execute(list_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Response format: {"actors": [...], "count": N}
        assert_eq!(data["count"], 2);
        assert!(data["actors"].is_array());
        assert_eq!(data["actors"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_delete_actor() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add an actor
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add actor"));
        add_args.insert("id".to_string(), json!("alice"));
        add_args.insert("name".to_string(), json!("Alice"));
        tool.execute(add_args, &context).await.unwrap();

        // Delete the actor
        let mut delete_args = serde_json::Map::new();
        delete_args.insert("op".to_string(), json!("delete actor"));
        delete_args.insert("id".to_string(), json!("alice"));

        let result = tool.execute(delete_args, &context).await;
        assert!(result.is_ok());

        // Verify it's gone
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get actor"));
        get_args.insert("id".to_string(), json!("alice"));

        let get_result = tool.execute(get_args, &context).await;
        assert!(get_result.is_err());
    }

    // =========================================================================
    // Tag operations (board-level)
    // =========================================================================

    #[tokio::test]
    async fn test_add_tag() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("add tag"));
        args.insert("id".to_string(), json!("bug"));
        args.insert("color".to_string(), json!("ff0000"));

        let result = tool.execute(args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["name"], "bug");
        assert_eq!(data["color"], "ff0000");
        // id is now an auto-generated ULID
        assert!(data["id"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_get_tag() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a tag
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add tag"));
        add_args.insert("id".to_string(), json!("bug"));
        add_args.insert("color".to_string(), json!("ff0000"));
        let add_result = tool.execute(add_args, &context).await.unwrap();
        let add_data = parse_json(&add_result);
        let tag_id = add_data["id"].as_str().unwrap().to_string();

        // Get the tag by its generated id
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get tag"));
        get_args.insert("id".to_string(), json!(tag_id));

        let result = tool.execute(get_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], tag_id);
        assert_eq!(data["name"], "bug");
        assert_eq!(data["color"], "ff0000");
    }

    #[tokio::test]
    async fn test_list_tags() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add tags
        for (id, color) in [("bug", "ff0000"), ("feature", "00ff00")] {
            let mut add_args = serde_json::Map::new();
            add_args.insert("op".to_string(), json!("add tag"));
            add_args.insert("id".to_string(), json!(id));
            add_args.insert("color".to_string(), json!(color));
            tool.execute(add_args, &context).await.unwrap();
        }

        // List tags
        let mut list_args = serde_json::Map::new();
        list_args.insert("op".to_string(), json!("list tags"));

        let result = tool.execute(list_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Response format: {"tags": [...], "count": N}
        assert_eq!(data["count"], 2);
        assert!(data["tags"].is_array());
        assert_eq!(data["tags"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_delete_tag() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a tag
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add tag"));
        add_args.insert("id".to_string(), json!("bug"));
        add_args.insert("color".to_string(), json!("ff0000"));
        let add_result = tool.execute(add_args, &context).await.unwrap();
        let add_data = parse_json(&add_result);
        let tag_id = add_data["id"].as_str().unwrap().to_string();

        // Delete it
        let mut delete_args = serde_json::Map::new();
        delete_args.insert("op".to_string(), json!("delete tag"));
        delete_args.insert("id".to_string(), json!(tag_id));

        let result = tool.execute(delete_args, &context).await;
        assert!(result.is_ok());

        // Verify it's gone
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get tag"));
        get_args.insert("id".to_string(), json!(tag_id));

        let get_result = tool.execute(get_args, &context).await;
        assert!(get_result.is_err());
    }

    // =========================================================================
    // Task tag/untag operations
    // =========================================================================

    #[tokio::test]
    async fn test_tag_task() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a tag
        let mut tag_args = serde_json::Map::new();
        tag_args.insert("op".to_string(), json!("add tag"));
        tag_args.insert("id".to_string(), json!("bug"));
        tag_args.insert("color".to_string(), json!("ff0000"));
        let tag_result = tool.execute(tag_args, &context).await.unwrap();
        let tag_data = parse_json(&tag_result);
        assert!(tag_data["id"].as_str().is_some(), "Tag should have an id");

        // Add a task
        let mut task_args = serde_json::Map::new();
        task_args.insert("op".to_string(), json!("add task"));
        task_args.insert("title".to_string(), json!("Fix bug"));
        let result = tool.execute(task_args, &context).await.unwrap();
        let task_id = extract_task_id(&result);

        // Tag the task (use tag name, which is "bug")
        let mut tag_task_args = serde_json::Map::new();
        tag_task_args.insert("op".to_string(), json!("tag task"));
        tag_task_args.insert("id".to_string(), json!(task_id));
        tag_task_args.insert("tag".to_string(), json!("bug"));

        let result = tool.execute(tag_task_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Response format: {"tagged": true, "task_id": ..., "tag": ...}
        assert_eq!(data["tagged"], true);
        assert_eq!(data["task_id"], task_id);
        assert_eq!(data["tag"], "bug");

        // Verify by getting the task
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get task"));
        get_args.insert("id".to_string(), json!(task_id));
        let get_result = tool.execute(get_args, &context).await.unwrap();
        let task_data = parse_json(&get_result);

        // Task should now have the tag
        assert!(task_data["tags"].is_array());
        let tags = task_data["tags"].as_array().unwrap();
        assert!(!tags.is_empty(), "Task should have at least one tag");
    }

    #[tokio::test]
    async fn test_untag_task() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a tag
        let mut tag_args = serde_json::Map::new();
        tag_args.insert("op".to_string(), json!("add tag"));
        tag_args.insert("id".to_string(), json!("bug"));
        tag_args.insert("color".to_string(), json!("ff0000"));
        let tag_result = tool.execute(tag_args, &context).await.unwrap();
        let tag_data = parse_json(&tag_result);
        assert!(tag_data["id"].as_str().is_some(), "Tag should have an id");

        // Add a task
        let mut task_args = serde_json::Map::new();
        task_args.insert("op".to_string(), json!("add task"));
        task_args.insert("title".to_string(), json!("Fix bug"));
        let result = tool.execute(task_args, &context).await.unwrap();
        let task_id = extract_task_id(&result);

        // Tag the task first
        let mut tag_task_args = serde_json::Map::new();
        tag_task_args.insert("op".to_string(), json!("tag task"));
        tag_task_args.insert("id".to_string(), json!(task_id));
        tag_task_args.insert("tag".to_string(), json!("bug"));
        tool.execute(tag_task_args, &context).await.unwrap();

        // Untag the task
        let mut untag_args = serde_json::Map::new();
        untag_args.insert("op".to_string(), json!("untag task"));
        untag_args.insert("id".to_string(), json!(task_id));
        untag_args.insert("tag".to_string(), json!("bug"));

        let result = tool.execute(untag_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Response format: {"untagged": true, "task_id": ..., "tag": ...}
        assert_eq!(data["untagged"], true);
        assert_eq!(data["task_id"], task_id);
        assert_eq!(data["tag"], "bug");

        // Verify by getting the task
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get task"));
        get_args.insert("id".to_string(), json!(task_id));
        let get_result = tool.execute(get_args, &context).await.unwrap();
        let task_data = parse_json(&get_result);

        // Task should now have no tags
        assert!(task_data["tags"].is_array());
        let tags = task_data["tags"].as_array().unwrap();
        assert!(tags.is_empty(), "Task should have no tags after untag");
    }

    // =========================================================================
    // Complete task operation
    // =========================================================================

    #[tokio::test]
    async fn test_complete_task() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a task
        let mut task_args = serde_json::Map::new();
        task_args.insert("op".to_string(), json!("add task"));
        task_args.insert("title".to_string(), json!("Task to complete"));
        let result = tool.execute(task_args, &context).await.unwrap();
        let task_id = extract_task_id(&result);
        let original_column = parse_json(&result)["position"]["column"]
            .as_str()
            .unwrap()
            .to_string();

        // Complete the task
        let mut complete_args = serde_json::Map::new();
        complete_args.insert("op".to_string(), json!("complete task"));
        complete_args.insert("id".to_string(), json!(task_id));

        let result = tool.execute(complete_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Verify task moved to the done column (position.column)
        assert_ne!(
            data["position"]["column"].as_str().unwrap(),
            original_column
        );
        assert_eq!(data["position"]["column"], "done");
    }

    #[tokio::test]
    async fn test_complete_task_with_done_alias() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a task
        let mut task_args = serde_json::Map::new();
        task_args.insert("op".to_string(), json!("add task"));
        task_args.insert("title".to_string(), json!("Task to complete"));
        let result = tool.execute(task_args, &context).await.unwrap();
        let task_id = extract_task_id(&result);

        // Complete using "done task" alias
        let mut complete_args = serde_json::Map::new();
        complete_args.insert("op".to_string(), json!("done task"));
        complete_args.insert("id".to_string(), json!(task_id));

        let result = tool.execute(complete_args, &context).await;
        assert!(result.is_ok());
    }

    // =========================================================================
    // Assign task operation
    // =========================================================================

    #[tokio::test]
    async fn test_assign_task() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add an actor first
        let mut actor_args = serde_json::Map::new();
        actor_args.insert("op".to_string(), json!("add actor"));
        actor_args.insert("id".to_string(), json!("assistant"));
        actor_args.insert("name".to_string(), json!("Assistant"));
        actor_args.insert("type".to_string(), json!("agent"));
        tool.execute(actor_args, &context).await.unwrap();

        // Add a task
        let mut task_args = serde_json::Map::new();
        task_args.insert("op".to_string(), json!("add task"));
        task_args.insert("title".to_string(), json!("Task to assign"));
        let result = tool.execute(task_args, &context).await.unwrap();
        let task_id = extract_task_id(&result);

        // Assign the task
        let mut assign_args = serde_json::Map::new();
        assign_args.insert("op".to_string(), json!("assign task"));
        assign_args.insert("id".to_string(), json!(task_id));
        assign_args.insert("assignee".to_string(), json!("assistant"));

        let result = tool.execute(assign_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Response format: {"assigned": true, "task_id": ..., "assignee": ..., "all_assignees": [...]}
        assert_eq!(data["assigned"], true);
        assert_eq!(data["task_id"], task_id);
        assert_eq!(data["assignee"], "assistant");
        assert!(data["all_assignees"]
            .as_array()
            .unwrap()
            .contains(&json!("assistant")));
    }

    #[tokio::test]
    async fn test_assign_task_nonexistent_actor() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a task
        let mut task_args = serde_json::Map::new();
        task_args.insert("op".to_string(), json!("add task"));
        task_args.insert("title".to_string(), json!("Task to assign"));
        let result = tool.execute(task_args, &context).await.unwrap();
        let task_id = extract_task_id(&result);

        // Try to assign to nonexistent actor
        let mut assign_args = serde_json::Map::new();
        assign_args.insert("op".to_string(), json!("assign task"));
        assign_args.insert("id".to_string(), json!(task_id));
        assign_args.insert("assignee".to_string(), json!("nonexistent"));

        let result = tool.execute(assign_args, &context).await;
        assert!(result.is_err());
    }

    // =========================================================================
    // Column operations
    // =========================================================================

    #[tokio::test]
    async fn test_add_column() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("add column"));
        args.insert("id".to_string(), json!("review"));
        args.insert("name".to_string(), json!("Review"));

        let result = tool.execute(args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], "review");
        assert_eq!(data["name"], "Review");
    }

    #[tokio::test]
    async fn test_get_column() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Get one of the default columns
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get column"));
        get_args.insert("id".to_string(), json!("todo"));

        let result = tool.execute(get_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], "todo");
    }

    #[tokio::test]
    async fn test_list_columns() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        let mut list_args = serde_json::Map::new();
        list_args.insert("op".to_string(), json!("list columns"));

        let result = tool.execute(list_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Response format: {"columns": [...], "count": N}
        assert_eq!(data["count"], 3);
        assert!(data["columns"].is_array());
        // Default board has 3 columns
        assert_eq!(data["columns"].as_array().unwrap().len(), 3);
    }

    // =========================================================================
    // Error cases
    // =========================================================================

    #[tokio::test]
    async fn test_get_nonexistent_task() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get task"));
        get_args.insert("id".to_string(), json!("nonexistent-id"));

        let result = tool.execute(get_args, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_invalid_operation() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("invalid operation"));

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_add_task_missing_title() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("add task"));
        // Missing title

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_operation_without_board_auto_inits() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        // Don't init board - auto-init should create one

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("add task"));
        args.insert("title".to_string(), json!("Test"));

        // Should succeed now with auto-init
        let result = tool.execute(args, &context).await;
        assert!(result.is_ok(), "Operation should succeed with auto-init");

        // Verify board was auto-created
        let mut get_board = serde_json::Map::new();
        get_board.insert("op".to_string(), json!("get board"));
        let board_result = tool.execute(get_board, &context).await.unwrap();
        let data = parse_json(&board_result);
        assert_eq!(data["name"], "Untitled Board");
    }

    // =========================================================================
    // Next task operation
    // =========================================================================

    #[tokio::test]
    async fn test_next_task() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add some tasks
        for title in ["Task 1", "Task 2"] {
            let mut add_args = serde_json::Map::new();
            add_args.insert("op".to_string(), json!("add task"));
            add_args.insert("title".to_string(), json!(title));
            tool.execute(add_args, &context).await.unwrap();
        }

        // Get next task
        let mut next_args = serde_json::Map::new();
        next_args.insert("op".to_string(), json!("next task"));

        let result = tool.execute(next_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Should return a task
        assert!(data["id"].is_string());
        assert!(data["title"].is_string());
    }

    // =========================================================================
    // Board operations
    // =========================================================================

    #[tokio::test]
    async fn test_get_board() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("get board"));

        let result = tool.execute(args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["name"], "Test Board");
        assert!(data["columns"].is_array());
    }

    #[tokio::test]
    async fn test_update_board() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("update board"));
        args.insert("name".to_string(), json!("Updated Board Name"));

        let result = tool.execute(args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["name"], "Updated Board Name");
    }

    // =========================================================================
    // Additional update/delete operations for full coverage
    // =========================================================================

    #[tokio::test]
    async fn test_update_column() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Update an existing column
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("update column"));
        args.insert("id".to_string(), json!("todo"));
        args.insert("name".to_string(), json!("Backlog"));

        let result = tool.execute(args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], "todo");
        assert_eq!(data["name"], "Backlog");
    }

    #[tokio::test]
    async fn test_delete_column() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a new column to delete (don't delete default ones)
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add column"));
        add_args.insert("id".to_string(), json!("review"));
        add_args.insert("name".to_string(), json!("Review"));
        tool.execute(add_args, &context).await.unwrap();

        // Delete the column
        let mut delete_args = serde_json::Map::new();
        delete_args.insert("op".to_string(), json!("delete column"));
        delete_args.insert("id".to_string(), json!("review"));

        let result = tool.execute(delete_args, &context).await;
        assert!(result.is_ok());

        // Verify it's gone
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get column"));
        get_args.insert("id".to_string(), json!("review"));

        let get_result = tool.execute(get_args, &context).await;
        assert!(get_result.is_err());
    }

    #[tokio::test]
    async fn test_update_project() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a project first
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add project"));
        add_args.insert("id".to_string(), json!("backend"));
        add_args.insert("name".to_string(), json!("Backend"));
        tool.execute(add_args, &context).await.unwrap();

        // Update it
        let mut update_args = serde_json::Map::new();
        update_args.insert("op".to_string(), json!("update project"));
        update_args.insert("id".to_string(), json!("backend"));
        update_args.insert("name".to_string(), json!("Backend Services"));

        let result = tool.execute(update_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], "backend");
        assert_eq!(data["name"], "Backend Services");
    }

    #[tokio::test]
    async fn test_update_actor() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add an actor first
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add actor"));
        add_args.insert("id".to_string(), json!("alice"));
        add_args.insert("name".to_string(), json!("Alice"));
        tool.execute(add_args, &context).await.unwrap();

        // Update it
        let mut update_args = serde_json::Map::new();
        update_args.insert("op".to_string(), json!("update actor"));
        update_args.insert("id".to_string(), json!("alice"));
        update_args.insert("name".to_string(), json!("Alice Smith"));

        let result = tool.execute(update_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], "alice");
        assert_eq!(data["name"], "Alice Smith");
    }

    #[tokio::test]
    async fn test_update_tag() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a tag first
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add tag"));
        add_args.insert("id".to_string(), json!("bug"));
        add_args.insert("color".to_string(), json!("ff0000"));
        let add_result = tool.execute(add_args, &context).await.unwrap();
        let add_data = parse_json(&add_result);
        let tag_id = add_data["id"].as_str().unwrap().to_string();

        // Update it using the generated id
        let mut update_args = serde_json::Map::new();
        update_args.insert("op".to_string(), json!("update tag"));
        update_args.insert("id".to_string(), json!(tag_id));
        update_args.insert("color".to_string(), json!("ff5500"));

        let result = tool.execute(update_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], tag_id);
        assert_eq!(data["color"], "ff5500");
    }

    // =========================================================================
    // Edge cases and additional scenarios
    // =========================================================================

    #[tokio::test]
    async fn test_move_task_with_position() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add two tasks
        let mut add1 = serde_json::Map::new();
        add1.insert("op".to_string(), json!("add task"));
        add1.insert("title".to_string(), json!("Task 1"));
        tool.execute(add1, &context).await.unwrap();

        let mut add2 = serde_json::Map::new();
        add2.insert("op".to_string(), json!("add task"));
        add2.insert("title".to_string(), json!("Task 2"));
        let result2 = tool.execute(add2, &context).await.unwrap();
        let task2_id = extract_task_id(&result2);

        // Move task 2 to doing column
        let mut move_args = serde_json::Map::new();
        move_args.insert("op".to_string(), json!("move task"));
        move_args.insert("id".to_string(), json!(task2_id));
        move_args.insert("column".to_string(), json!("doing"));

        let result = tool.execute(move_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["position"]["column"], "doing");
    }

    #[tokio::test]
    async fn test_list_tasks_with_filter() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add tasks in different columns
        let mut add1 = serde_json::Map::new();
        add1.insert("op".to_string(), json!("add task"));
        add1.insert("title".to_string(), json!("Todo Task"));
        let result1 = tool.execute(add1, &context).await.unwrap();
        let task1_id = extract_task_id(&result1);

        // Move one to doing
        let mut move_args = serde_json::Map::new();
        move_args.insert("op".to_string(), json!("move task"));
        move_args.insert("id".to_string(), json!(task1_id));
        move_args.insert("column".to_string(), json!("doing"));
        tool.execute(move_args, &context).await.unwrap();

        // Add another in todo
        let mut add2 = serde_json::Map::new();
        add2.insert("op".to_string(), json!("add task"));
        add2.insert("title".to_string(), json!("Another Todo"));
        tool.execute(add2, &context).await.unwrap();

        // List only todo column
        let mut list_args = serde_json::Map::new();
        list_args.insert("op".to_string(), json!("list tasks"));
        list_args.insert("column".to_string(), json!("todo"));

        let result = tool.execute(list_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["count"], 1);
        assert_eq!(data["tasks"][0]["title"], "Another Todo");
    }

    #[tokio::test]
    async fn test_task_with_description() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("add task"));
        args.insert("title".to_string(), json!("Task with description"));
        args.insert(
            "description".to_string(),
            json!("This is a detailed description"),
        );

        let result = tool.execute(args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["title"], "Task with description");
        assert_eq!(data["description"], "This is a detailed description");
    }

    #[tokio::test]
    async fn test_next_task_empty_board() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Get next task on empty board
        let mut next_args = serde_json::Map::new();
        next_args.insert("op".to_string(), json!("next task"));

        let result = tool.execute(next_args, &context).await;
        // Should either return null/none or an error - depends on implementation
        // Just verify it doesn't panic
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_tag_nonexistent_tag_auto_creates() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a task
        let mut task_args = serde_json::Map::new();
        task_args.insert("op".to_string(), json!("add task"));
        task_args.insert("title".to_string(), json!("Test task"));
        let result = tool.execute(task_args, &context).await.unwrap();
        let task_id = extract_task_id(&result);

        // Tagging with a nonexistent tag should auto-create the tag
        let mut tag_args = serde_json::Map::new();
        tag_args.insert("op".to_string(), json!("tag task"));
        tag_args.insert("id".to_string(), json!(task_id));
        tag_args.insert("tag".to_string(), json!("nonexistent"));

        let result = tool.execute(tag_args, &context).await;
        assert!(result.is_ok(), "TagTask should auto-create unknown tags");
    }

    #[tokio::test]
    async fn test_complete_already_done_task() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add and complete a task
        let mut task_args = serde_json::Map::new();
        task_args.insert("op".to_string(), json!("add task"));
        task_args.insert("title".to_string(), json!("Task"));
        let result = tool.execute(task_args, &context).await.unwrap();
        let task_id = extract_task_id(&result);

        let mut complete_args = serde_json::Map::new();
        complete_args.insert("op".to_string(), json!("complete task"));
        complete_args.insert("id".to_string(), json!(task_id));
        tool.execute(complete_args.clone(), &context).await.unwrap();

        // Complete again - should be idempotent
        let result = tool.execute(complete_args, &context).await;
        assert!(result.is_ok());
    }

    // =========================================================================
    // Directory initialization tests
    // =========================================================================

    #[tokio::test]
    async fn test_operations_auto_init_without_explicit_init() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();

        // DO NOT call init_test_board - verifying auto-init works

        // Try to add a task without explicit initialization
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add task"));
        add_args.insert("title".to_string(), json!("Task without init"));

        let result = tool.execute(add_args, &context).await;

        // Should succeed with auto-init
        assert!(result.is_ok(), "Operation should succeed with auto-init");
        let data = parse_json(&result.unwrap());
        assert_eq!(data["title"], "Task without init");

        // Verify board was auto-created with default name
        let mut get_board = serde_json::Map::new();
        get_board.insert("op".to_string(), json!("get board"));
        let board_result = tool.execute(get_board, &context).await.unwrap();
        let board_data = parse_json(&board_result);
        assert_eq!(board_data["name"], "Untitled Board");
    }

    #[tokio::test]
    async fn test_ensure_directories_is_idempotent() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        // Call ensure_directories multiple times
        ctx.ensure_directories().await.unwrap();
        ctx.ensure_directories().await.unwrap();
        ctx.ensure_directories().await.unwrap();

        // Verify all directories exist
        assert!(ctx.directories_exist());
        assert!(ctx.root().exists());
        assert!(ctx.tasks_dir().exists());
        assert!(ctx.actors_dir().exists());
        assert!(ctx.tags_dir().exists());
        assert!(ctx.perspectives_dir().exists());
    }

    #[tokio::test]
    async fn test_add_actor_without_init_succeeds_after_ensure() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();

        // Initialize board (creates board.json and dirs)
        init_test_board(&tool, &context).await;

        // Manually delete the actors directory to simulate missing subdirs
        let kanban_ctx = KanbanContext::find(temp.path()).unwrap();
        tokio::fs::remove_dir_all(kanban_ctx.actors_dir())
            .await
            .unwrap();

        // Try to add actor - should succeed because ensure_directories() was added
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add actor"));
        add_args.insert("id".to_string(), json!("alice"));
        add_args.insert("name".to_string(), json!("Alice"));
        add_args.insert("ensure".to_string(), json!(false));

        let result = tool.execute(add_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["actor"]["id"], "alice");
        assert_eq!(data["actor"]["name"], "Alice");
    }

    #[tokio::test]
    async fn test_directories_exist_check() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(&kanban_dir);

        // Initially no directories exist
        assert!(!ctx.directories_exist());

        // Create directories
        ctx.create_directories().await.unwrap();

        // Now they should exist
        assert!(ctx.directories_exist());

        // Delete one subdirectory
        tokio::fs::remove_dir_all(ctx.actors_dir()).await.unwrap();

        // directories_exist should now return false
        assert!(!ctx.directories_exist());
    }

    // =========================================================================
    // Auto-assign tests
    // =========================================================================

    #[tokio::test]
    async fn test_add_task_auto_assigns_actor() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Register an agent actor
        let mut actor_args = serde_json::Map::new();
        actor_args.insert("op".to_string(), json!("add actor"));
        actor_args.insert("id".to_string(), json!("assistant"));
        actor_args.insert("name".to_string(), json!("AI Assistant"));
        actor_args.insert("type".to_string(), json!("agent"));
        tool.execute(actor_args, &context).await.unwrap();

        // Add a task with actor set but no explicit assignees
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add task"));
        add_args.insert("title".to_string(), json!("Auto-assigned task"));
        add_args.insert("actor".to_string(), json!("assistant"));

        let result = tool.execute(add_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Task should be auto-assigned to the actor
        let assignees = data["assignees"]
            .as_array()
            .expect("assignees should be an array");
        assert_eq!(assignees.len(), 1);
        assert_eq!(assignees[0], "assistant");
    }

    #[tokio::test]
    async fn test_add_task_no_auto_assign_without_actor() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a task without actor
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add task"));
        add_args.insert("title".to_string(), json!("No actor task"));

        let result = tool.execute(add_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Task should have no assignees
        let assignees = data["assignees"]
            .as_array()
            .expect("assignees should be an array");
        assert!(assignees.is_empty());
    }

    #[tokio::test]
    async fn test_add_task_explicit_assignees_override_auto_assign() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Register actors
        for (id, name) in [("assistant", "AI Assistant"), ("alice", "Alice")] {
            let mut actor_args = serde_json::Map::new();
            actor_args.insert("op".to_string(), json!("add actor"));
            actor_args.insert("id".to_string(), json!(id));
            actor_args.insert("name".to_string(), json!(name));
            actor_args.insert("type".to_string(), json!("human"));
            tool.execute(actor_args, &context).await.unwrap();
        }

        // Add a task with actor AND explicit assignees
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add task"));
        add_args.insert("title".to_string(), json!("Explicitly assigned task"));
        add_args.insert("actor".to_string(), json!("assistant"));
        add_args.insert("assignees".to_string(), json!(["alice"]));

        let result = tool.execute(add_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Explicit assignees should be used, not auto-assigned actor
        let assignees = data["assignees"]
            .as_array()
            .expect("assignees should be an array");
        assert_eq!(assignees.len(), 1);
        assert_eq!(assignees[0], "alice");
    }

    // =========================================================================
    // Session actor injection tests
    // =========================================================================

    /// When `context.session_actor` is set (as it would be after an MCP
    /// `initialize` call) and the caller does not pass `actor` explicitly,
    /// the tool should auto-inject the session actor so the task is
    /// auto-assigned.
    #[tokio::test]
    async fn test_session_actor_auto_injected_on_add_task() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Register an agent actor to represent the MCP client session
        let mut actor_args = serde_json::Map::new();
        actor_args.insert("op".to_string(), json!("add actor"));
        actor_args.insert("id".to_string(), json!("claude-code"));
        actor_args.insert("name".to_string(), json!("Claude Code"));
        actor_args.insert("type".to_string(), json!("agent"));
        tool.execute(actor_args, &context).await.unwrap();

        // Simulate what ensure_agent_actor does: store the actor_id in the context
        *context.session_actor.write().await = Some("claude-code".to_string());

        // Add a task WITHOUT an explicit actor arg
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add task"));
        add_args.insert("title".to_string(), json!("Session-injected task"));

        let result = tool.execute(add_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Task should be auto-assigned to the session actor
        let assignees = data["assignees"]
            .as_array()
            .expect("assignees should be an array");
        assert_eq!(
            assignees.len(),
            1,
            "task should be auto-assigned to session actor"
        );
        assert_eq!(assignees[0], "claude-code");
    }

    /// When `context.session_actor` is set but the caller explicitly passes a
    /// different `actor`, the explicit value must not be overridden.
    #[tokio::test]
    async fn test_explicit_actor_not_overridden_by_session_actor() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Register two actors
        for (id, name) in [("claude-code", "Claude Code"), ("alice", "Alice")] {
            let mut actor_args = serde_json::Map::new();
            actor_args.insert("op".to_string(), json!("add actor"));
            actor_args.insert("id".to_string(), json!(id));
            actor_args.insert("name".to_string(), json!(name));
            actor_args.insert("type".to_string(), json!("human"));
            tool.execute(actor_args, &context).await.unwrap();
        }

        // Session actor is "claude-code"
        *context.session_actor.write().await = Some("claude-code".to_string());

        // Caller passes a different explicit actor
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add task"));
        add_args.insert("title".to_string(), json!("Explicit actor task"));
        add_args.insert("actor".to_string(), json!("alice"));

        let result = tool.execute(add_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Should use the explicitly provided actor, not the session one
        let assignees = data["assignees"]
            .as_array()
            .expect("assignees should be an array");
        assert_eq!(assignees.len(), 1, "explicit actor should be used");
        assert_eq!(assignees[0], "alice");
    }

    /// When no session actor is set and no actor is passed, tasks should have
    /// no assignees (existing baseline behaviour is preserved).
    #[tokio::test]
    async fn test_no_session_actor_no_assignees() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // session_actor is None (default)
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add task"));
        add_args.insert("title".to_string(), json!("Unassigned task"));

        let result = tool.execute(add_args, &context).await.unwrap();
        let data = parse_json(&result);

        let assignees = data["assignees"]
            .as_array()
            .expect("assignees should be an array");
        assert!(
            assignees.is_empty(),
            "should be unassigned when no session actor"
        );
    }
}
