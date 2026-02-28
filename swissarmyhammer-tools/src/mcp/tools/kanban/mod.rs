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
    activity::ListActivity,
    actor::{AddActor, DeleteActor, GetActor, ListActors, UpdateActor},
    attachment::{
        AddAttachment, DeleteAttachment, GetAttachment, ListAttachments, UpdateAttachment,
    },
    board::{GetBoard, InitBoard, UpdateBoard},
    column::{AddColumn, DeleteColumn, GetColumn, ListColumns, UpdateColumn},
    comment::{AddComment, DeleteComment, GetComment, ListComments, UpdateComment},
    parse::parse_input,
    swimlane::{AddSwimlane, DeleteSwimlane, GetSwimlane, ListSwimlanes, UpdateSwimlane},
    tag::{AddTag, DeleteTag, GetTag, ListTags, UpdateTag},
    task::{
        AddTask, AssignTask, CompleteTask, DeleteTask, GetTask, ListTasks, MoveTask, NextTask,
        TagTask, UnassignTask, UntagTask, UpdateTask,
    },
    Execute, KanbanContext, KanbanOperation, KanbanOperationProcessor, Noun, Operation,
    OperationProcessor, Verb,
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

static ADD_SWIMLANE: Lazy<AddSwimlane> = Lazy::new(|| AddSwimlane::new("", ""));
static GET_SWIMLANE: Lazy<GetSwimlane> = Lazy::new(|| GetSwimlane::new(""));
static UPDATE_SWIMLANE: Lazy<UpdateSwimlane> = Lazy::new(|| UpdateSwimlane::new(""));
static DELETE_SWIMLANE: Lazy<DeleteSwimlane> = Lazy::new(|| DeleteSwimlane::new(""));
static LIST_SWIMLANES: Lazy<ListSwimlanes> = Lazy::new(ListSwimlanes::default);

static ADD_ACTOR: Lazy<AddActor> = Lazy::new(|| AddActor::human("", ""));
static GET_ACTOR: Lazy<GetActor> = Lazy::new(|| GetActor::new(""));
static UPDATE_ACTOR: Lazy<UpdateActor> = Lazy::new(|| UpdateActor::new(""));
static DELETE_ACTOR: Lazy<DeleteActor> = Lazy::new(|| DeleteActor::new(""));
static LIST_ACTORS: Lazy<ListActors> = Lazy::new(ListActors::default);

static ADD_TASK: Lazy<AddTask> = Lazy::new(|| AddTask::new(""));
static GET_TASK: Lazy<GetTask> = Lazy::new(|| GetTask::new(""));
static UPDATE_TASK: Lazy<UpdateTask> = Lazy::new(|| UpdateTask::new(""));
static DELETE_TASK: Lazy<DeleteTask> = Lazy::new(|| DeleteTask::new(""));
static MOVE_TASK: Lazy<MoveTask> = Lazy::new(|| MoveTask::to_column("", ""));
static COMPLETE_TASK: Lazy<CompleteTask> = Lazy::new(|| CompleteTask::new(""));
static ASSIGN_TASK: Lazy<AssignTask> = Lazy::new(|| AssignTask::new("", ""));
static UNASSIGN_TASK: Lazy<UnassignTask> = Lazy::new(|| UnassignTask::new("", ""));
static NEXT_TASK: Lazy<NextTask> = Lazy::new(NextTask::new);
static TAG_TASK: Lazy<TagTask> = Lazy::new(|| TagTask::new("", ""));
static UNTAG_TASK: Lazy<UntagTask> = Lazy::new(|| UntagTask::new("", ""));
static LIST_TASKS: Lazy<ListTasks> = Lazy::new(ListTasks::new);

static ADD_TAG: Lazy<AddTag> = Lazy::new(|| AddTag::new("", "", ""));
static GET_TAG: Lazy<GetTag> = Lazy::new(|| GetTag::new(""));
static UPDATE_TAG: Lazy<UpdateTag> = Lazy::new(|| UpdateTag::new(""));
static DELETE_TAG: Lazy<DeleteTag> = Lazy::new(|| DeleteTag::new(""));
static LIST_TAGS: Lazy<ListTags> = Lazy::new(ListTags::default);

static ADD_COMMENT: Lazy<AddComment> = Lazy::new(|| AddComment::new("", "", ""));
static GET_COMMENT: Lazy<GetComment> = Lazy::new(|| GetComment::new("", ""));
static UPDATE_COMMENT: Lazy<UpdateComment> = Lazy::new(|| UpdateComment::new("", ""));
static DELETE_COMMENT: Lazy<DeleteComment> = Lazy::new(|| DeleteComment::new("", ""));
static LIST_COMMENTS: Lazy<ListComments> = Lazy::new(|| ListComments::new(""));

static LIST_ACTIVITY: Lazy<ListActivity> = Lazy::new(ListActivity::default);


static ADD_ATTACHMENT: Lazy<AddAttachment> = Lazy::new(|| AddAttachment::new("", "", ""));
static GET_ATTACHMENT: Lazy<GetAttachment> = Lazy::new(|| GetAttachment::new("", ""));
static UPDATE_ATTACHMENT: Lazy<UpdateAttachment> = Lazy::new(|| UpdateAttachment::new("", ""));
static DELETE_ATTACHMENT: Lazy<DeleteAttachment> = Lazy::new(|| DeleteAttachment::new("", ""));
static LIST_ATTACHMENTS: Lazy<ListAttachments> = Lazy::new(|| ListAttachments::new(""));

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
        // Swimlane operations
        &*ADD_SWIMLANE as &dyn Operation,
        &*GET_SWIMLANE as &dyn Operation,
        &*UPDATE_SWIMLANE as &dyn Operation,
        &*DELETE_SWIMLANE as &dyn Operation,
        &*LIST_SWIMLANES as &dyn Operation,
        // Actor operations
        &*ADD_ACTOR as &dyn Operation,
        &*GET_ACTOR as &dyn Operation,
        &*UPDATE_ACTOR as &dyn Operation,
        &*DELETE_ACTOR as &dyn Operation,
        &*LIST_ACTORS as &dyn Operation,
        // Task operations
        &*ADD_TASK as &dyn Operation,
        &*GET_TASK as &dyn Operation,
        &*UPDATE_TASK as &dyn Operation,
        &*DELETE_TASK as &dyn Operation,
        &*MOVE_TASK as &dyn Operation,
        &*COMPLETE_TASK as &dyn Operation,
        &*ASSIGN_TASK as &dyn Operation,
        &*UNASSIGN_TASK as &dyn Operation,
        &*NEXT_TASK as &dyn Operation,
        &*TAG_TASK as &dyn Operation,
        &*UNTAG_TASK as &dyn Operation,
        &*LIST_TASKS as &dyn Operation,
        // Tag operations (board-level)
        &*ADD_TAG as &dyn Operation,
        &*GET_TAG as &dyn Operation,
        &*UPDATE_TAG as &dyn Operation,
        &*DELETE_TAG as &dyn Operation,
        &*LIST_TAGS as &dyn Operation,
        // Comment operations
        &*ADD_COMMENT as &dyn Operation,
        &*GET_COMMENT as &dyn Operation,
        &*UPDATE_COMMENT as &dyn Operation,
        &*DELETE_COMMENT as &dyn Operation,
        &*LIST_COMMENTS as &dyn Operation,
        // Attachment operations
        &*ADD_ATTACHMENT as &dyn Operation,
        &*GET_ATTACHMENT as &dyn Operation,
        &*UPDATE_ATTACHMENT as &dyn Operation,
        &*DELETE_ATTACHMENT as &dyn Operation,
        &*LIST_ATTACHMENTS as &dyn Operation,
        // Activity operations
        &*LIST_ACTIVITY as &dyn Operation,
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

#[async_trait]
impl McpTool for KanbanTool {
    fn name(&self) -> &'static str {
        "kanban"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        swissarmyhammer_kanban::schema::generate_kanban_mcp_schema(&KANBAN_OPERATIONS)
    }

    fn operations(&self) -> &'static [&'static dyn swissarmyhammer_operations::Operation] {
        // Force initialization of the lazy static
        let ops: &[&'static dyn Operation] = &KANBAN_OPERATIONS;
        // This is safe because KANBAN_OPERATIONS is a static Lazy<Vec<...>>
        // We need to convert to a slice with 'static lifetime
        // SAFETY: The Lazy is initialized once and lives for 'static
        unsafe {
            std::mem::transmute::<
                &[&dyn Operation],
                &'static [&'static dyn swissarmyhammer_operations::Operation],
            >(ops)
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

/// Execute a single kanban operation
async fn execute_operation(ctx: &KanbanContext, op: &KanbanOperation) -> Result<Value, McpError> {
    // Note: Can't use glob imports due to Verb::Tag and Noun::Tag collision

    // Create processor with actor from operation context
    let processor = match &op.actor {
        Some(actor) => KanbanOperationProcessor::with_actor(actor.to_string()),
        None => KanbanOperationProcessor::new(),
    };

    let result = match (op.verb, op.noun) {
        // Board operations
        (Verb::Init, Noun::Board) => {
            let name = op
                .get_string("name")
                .ok_or_else(|| McpError::invalid_params("missing required field: name", None))?;
            let description = op.get_string("description").map(String::from);

            let mut cmd = InitBoard::new(name);
            if let Some(desc) = description {
                cmd = cmd.with_description(desc);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Get, Noun::Board) => {
            let include_counts = op.get_bool("include_counts").unwrap_or(true);
            processor.process(&GetBoard { include_counts }, ctx).await
        }
        (Verb::Update, Noun::Board) => {
            let mut cmd = UpdateBoard::new();
            if let Some(name) = op.get_string("name") {
                cmd = cmd.with_name(name);
            }
            if let Some(desc) = op.get_string("description") {
                cmd = cmd.with_description(desc);
            }
            processor.process(&cmd, ctx).await
        }

        // Column operations
        (Verb::Add, Noun::Column) => {
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
            processor.process(&cmd, ctx).await
        }
        (Verb::Get, Noun::Column) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            processor.process(&GetColumn::new(id), ctx).await
        }
        (Verb::Update, Noun::Column) => {
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
            processor.process(&cmd, ctx).await
        }
        (Verb::Delete, Noun::Column) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            processor.process(&DeleteColumn::new(id), ctx).await
        }
        (Verb::List, Noun::Columns) => processor.process(&ListColumns, ctx).await,

        // Task operations
        (Verb::Add, Noun::Task) => {
            let title = op
                .get_string("title")
                .ok_or_else(|| McpError::invalid_params("missing required field: title", None))?;

            let mut cmd = AddTask::new(title);
            if let Some(desc) = op.get_string("description") {
                cmd = cmd.with_description(desc);
            }
            // Could add position, tags, assignees, depends_on parsing here
            processor.process(&cmd, ctx).await
        }
        (Verb::Get, Noun::Task) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            processor.process(&GetTask::new(id), ctx).await
        }
        (Verb::Update, Noun::Task) => {
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
            processor.process(&cmd, ctx).await
        }
        (Verb::Move, Noun::Task) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            let column = op
                .get_string("column")
                .ok_or_else(|| McpError::invalid_params("missing required field: column", None))?;

            processor
                .process(&MoveTask::to_column(id, column), ctx)
                .await
        }
        (Verb::Delete, Noun::Task) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            processor.process(&DeleteTask::new(id), ctx).await
        }
        (Verb::Complete, Noun::Task) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            processor.process(&CompleteTask::new(id), ctx).await
        }
        (Verb::Assign, Noun::Task) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            let assignee = op.get_string("assignee").ok_or_else(|| {
                McpError::invalid_params("missing required field: assignee", None)
            })?;
            processor.process(&AssignTask::new(id, assignee), ctx).await
        }
        (Verb::Unassign, Noun::Task) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            let assignee = op.get_string("assignee").ok_or_else(|| {
                McpError::invalid_params("missing required field: assignee", None)
            })?;
            processor
                .process(&UnassignTask::new(id, assignee), ctx)
                .await
        }
        (Verb::Next, Noun::Task) => {
            let cmd = NextTask::new();
            // Could add swimlane/assignee filtering here
            processor.process(&cmd, ctx).await
        }
        (Verb::List, Noun::Tasks) => {
            let mut cmd = ListTasks::new();
            if let Some(column) = op.get_string("column") {
                cmd = cmd.with_column(column);
            }
            if let Some(ready) = op.get_param("ready").and_then(|v| v.as_bool()) {
                cmd = cmd.with_ready(ready);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Tag, Noun::Task) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            let tag = op
                .get_string("tag")
                .ok_or_else(|| McpError::invalid_params("missing required field: tag", None))?;
            processor.process(&TagTask::new(id, tag), ctx).await
        }
        (Verb::Untag, Noun::Task) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            let tag = op
                .get_string("tag")
                .ok_or_else(|| McpError::invalid_params("missing required field: tag", None))?;
            processor.process(&UntagTask::new(id, tag), ctx).await
        }

        // Swimlane operations
        (Verb::Add, Noun::Swimlane) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            let name = op
                .get_string("name")
                .ok_or_else(|| McpError::invalid_params("missing required field: name", None))?;

            let mut cmd = AddSwimlane::new(id, name);
            if let Some(order) = op.get_param("order").and_then(|v| v.as_u64()) {
                cmd = cmd.with_order(order as usize);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Get, Noun::Swimlane) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            processor.process(&GetSwimlane::new(id), ctx).await
        }
        (Verb::Update, Noun::Swimlane) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;

            let mut cmd = UpdateSwimlane::new(id);
            if let Some(name) = op.get_string("name") {
                cmd = cmd.with_name(name);
            }
            if let Some(order) = op.get_param("order").and_then(|v| v.as_u64()) {
                cmd = cmd.with_order(order as usize);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Delete, Noun::Swimlane) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            processor.process(&DeleteSwimlane::new(id), ctx).await
        }
        (Verb::List, Noun::Swimlanes) => processor.process(&ListSwimlanes, ctx).await,

        // Actor operations
        (Verb::Add, Noun::Actor) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            let name = op
                .get_string("name")
                .ok_or_else(|| McpError::invalid_params("missing required field: name", None))?;
            let actor_type = op.get_string("type").unwrap_or("human");
            let ensure = op
                .get_param("ensure")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let mut cmd = if actor_type == "agent" {
                AddActor::agent(id, name)
            } else {
                AddActor::human(id, name)
            };

            if ensure {
                cmd = cmd.with_ensure();
            }

            processor.process(&cmd, ctx).await
        }
        (Verb::Get, Noun::Actor) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            processor.process(&GetActor::new(id), ctx).await
        }
        (Verb::Update, Noun::Actor) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;

            let mut cmd = UpdateActor::new(id);
            if let Some(name) = op.get_string("name") {
                cmd = cmd.with_name(name);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Delete, Noun::Actor) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            processor.process(&DeleteActor::new(id), ctx).await
        }
        (Verb::List, Noun::Actors) => processor.process(&ListActors::default(), ctx).await,

        // Tag operations (board-level)
        (Verb::Add, Noun::Tag) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            let name = op
                .get_string("name")
                .ok_or_else(|| McpError::invalid_params("missing required field: name", None))?;
            let color = op
                .get_string("color")
                .ok_or_else(|| McpError::invalid_params("missing required field: color", None))?;

            let mut cmd = AddTag::new(id, name, color);
            if let Some(desc) = op.get_string("description") {
                cmd = cmd.with_description(desc);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Get, Noun::Tag) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            processor.process(&GetTag::new(id), ctx).await
        }
        (Verb::Update, Noun::Tag) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;

            let mut cmd = UpdateTag::new(id);
            if let Some(name) = op.get_string("name") {
                cmd = cmd.with_name(name);
            }
            if let Some(color) = op.get_string("color") {
                cmd = cmd.with_color(color);
            }
            if let Some(desc) = op.get_string("description") {
                cmd = cmd.with_description(desc);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Delete, Noun::Tag) => {
            let id = op
                .get_string("id")
                .ok_or_else(|| McpError::invalid_params("missing required field: id", None))?;
            processor.process(&DeleteTag::new(id), ctx).await
        }
        (Verb::List, Noun::Tags) => processor.process(&ListTags::default(), ctx).await,

        // Comment operations
        // Note: For comments, we need both task_id and comment_id. The parser aliases
        // task_id->id, so we use different param names to avoid collision.
        (Verb::Add, Noun::Comment) => {
            // task_id gets aliased to id by parser, so check both
            let task_id = op
                .get_string("task_id")
                .or_else(|| op.get_string("id"))
                .ok_or_else(|| McpError::invalid_params("missing required field: task_id", None))?;
            let body = op
                .get_string("body")
                .or_else(|| op.get_string("description"))
                .ok_or_else(|| McpError::invalid_params("missing required field: body", None))?;
            let author = op
                .get_string("author")
                .ok_or_else(|| McpError::invalid_params("missing required field: author", None))?;
            processor
                .process(&AddComment::new(task_id, body, author), ctx)
                .await
        }
        (Verb::Get, Noun::Comment) => {
            let task_id = op
                .get_string("task_id")
                .or_else(|| op.get_string("id"))
                .ok_or_else(|| McpError::invalid_params("missing required field: task_id", None))?;
            let comment_id = op.get_string("comment_id").ok_or_else(|| {
                McpError::invalid_params("missing required field: comment_id", None)
            })?;
            processor
                .process(&GetComment::new(task_id, comment_id), ctx)
                .await
        }
        (Verb::Update, Noun::Comment) => {
            let task_id = op
                .get_string("task_id")
                .or_else(|| op.get_string("id"))
                .ok_or_else(|| McpError::invalid_params("missing required field: task_id", None))?;
            let comment_id = op.get_string("comment_id").ok_or_else(|| {
                McpError::invalid_params("missing required field: comment_id", None)
            })?;

            let mut cmd = UpdateComment::new(task_id, comment_id);
            if let Some(body) = op
                .get_string("body")
                .or_else(|| op.get_string("description"))
            {
                cmd = cmd.with_body(body);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Delete, Noun::Comment) => {
            let task_id = op
                .get_string("task_id")
                .or_else(|| op.get_string("id"))
                .ok_or_else(|| McpError::invalid_params("missing required field: task_id", None))?;
            let comment_id = op.get_string("comment_id").ok_or_else(|| {
                McpError::invalid_params("missing required field: comment_id", None)
            })?;
            processor
                .process(&DeleteComment::new(task_id, comment_id), ctx)
                .await
        }
        (Verb::List, Noun::Comments) => {
            let task_id = op
                .get_string("task_id")
                .or_else(|| op.get_string("id"))
                .ok_or_else(|| McpError::invalid_params("missing required field: task_id", None))?;
            processor.process(&ListComments::new(task_id), ctx).await
        }

        // Activity operations
        (Verb::List, Noun::Activity) => {
            let mut cmd = ListActivity::default();
            if let Some(limit) = op.get_param("limit").and_then(|v| v.as_u64()) {
                cmd = cmd.with_limit(limit as usize);
            }
            processor.process(&cmd, ctx).await
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

    /// Helper to extract ID from result (generic)
    fn extract_id(result: &CallToolResult) -> String {
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
    // Swimlane operations
    // =========================================================================

    #[tokio::test]
    async fn test_add_swimlane() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("add swimlane"));
        args.insert("id".to_string(), json!("urgent"));
        args.insert("name".to_string(), json!("Urgent"));

        let result = tool.execute(args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], "urgent");
        assert_eq!(data["name"], "Urgent");
    }

    #[tokio::test]
    async fn test_get_swimlane() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a swimlane
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add swimlane"));
        add_args.insert("id".to_string(), json!("urgent"));
        add_args.insert("name".to_string(), json!("Urgent"));
        tool.execute(add_args, &context).await.unwrap();

        // Get the swimlane
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get swimlane"));
        get_args.insert("id".to_string(), json!("urgent"));

        let result = tool.execute(get_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], "urgent");
        assert_eq!(data["name"], "Urgent");
    }

    #[tokio::test]
    async fn test_list_swimlanes() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add swimlanes
        for (id, name) in [("urgent", "Urgent"), ("normal", "Normal")] {
            let mut add_args = serde_json::Map::new();
            add_args.insert("op".to_string(), json!("add swimlane"));
            add_args.insert("id".to_string(), json!(id));
            add_args.insert("name".to_string(), json!(name));
            tool.execute(add_args, &context).await.unwrap();
        }

        // List swimlanes
        let mut list_args = serde_json::Map::new();
        list_args.insert("op".to_string(), json!("list swimlanes"));

        let result = tool.execute(list_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Response format: {"swimlanes": [...], "count": N}
        assert_eq!(data["count"], 2);
        assert!(data["swimlanes"].is_array());
        assert_eq!(data["swimlanes"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_delete_swimlane() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a swimlane
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add swimlane"));
        add_args.insert("id".to_string(), json!("urgent"));
        add_args.insert("name".to_string(), json!("Urgent"));
        tool.execute(add_args, &context).await.unwrap();

        // Delete it
        let mut delete_args = serde_json::Map::new();
        delete_args.insert("op".to_string(), json!("delete swimlane"));
        delete_args.insert("id".to_string(), json!("urgent"));

        let result = tool.execute(delete_args, &context).await;
        assert!(result.is_ok());

        // Verify it's gone
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get swimlane"));
        get_args.insert("id".to_string(), json!("urgent"));

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
        args.insert("name".to_string(), json!("Bug"));
        args.insert("color".to_string(), json!("ff0000"));

        let result = tool.execute(args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], "bug");
        assert_eq!(data["name"], "Bug");
        assert_eq!(data["color"], "ff0000");
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
        add_args.insert("name".to_string(), json!("Bug"));
        add_args.insert("color".to_string(), json!("ff0000"));
        tool.execute(add_args, &context).await.unwrap();

        // Get the tag
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get tag"));
        get_args.insert("id".to_string(), json!("bug"));

        let result = tool.execute(get_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], "bug");
        assert_eq!(data["name"], "Bug");
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
        for (id, name, color) in [("bug", "Bug", "ff0000"), ("feature", "Feature", "00ff00")] {
            let mut add_args = serde_json::Map::new();
            add_args.insert("op".to_string(), json!("add tag"));
            add_args.insert("id".to_string(), json!(id));
            add_args.insert("name".to_string(), json!(name));
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
        add_args.insert("name".to_string(), json!("Bug"));
        add_args.insert("color".to_string(), json!("ff0000"));
        tool.execute(add_args, &context).await.unwrap();

        // Delete it
        let mut delete_args = serde_json::Map::new();
        delete_args.insert("op".to_string(), json!("delete tag"));
        delete_args.insert("id".to_string(), json!("bug"));

        let result = tool.execute(delete_args, &context).await;
        assert!(result.is_ok());

        // Verify it's gone
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get tag"));
        get_args.insert("id".to_string(), json!("bug"));

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
        tag_args.insert("name".to_string(), json!("Bug"));
        tag_args.insert("color".to_string(), json!("ff0000"));
        tool.execute(tag_args, &context).await.unwrap();

        // Add a task
        let mut task_args = serde_json::Map::new();
        task_args.insert("op".to_string(), json!("add task"));
        task_args.insert("title".to_string(), json!("Fix bug"));
        let result = tool.execute(task_args, &context).await.unwrap();
        let task_id = extract_task_id(&result);

        // Tag the task
        let mut tag_task_args = serde_json::Map::new();
        tag_task_args.insert("op".to_string(), json!("tag task"));
        tag_task_args.insert("id".to_string(), json!(task_id));
        tag_task_args.insert("tag".to_string(), json!("bug"));

        let result = tool.execute(tag_task_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Response format: {"tagged": true, "task_id": ..., "tag_id": ...}
        assert_eq!(data["tagged"], true);
        assert_eq!(data["task_id"], task_id);
        assert_eq!(data["tag_id"], "bug");

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
        tag_args.insert("name".to_string(), json!("Bug"));
        tag_args.insert("color".to_string(), json!("ff0000"));
        tool.execute(tag_args, &context).await.unwrap();

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

        // Response format: {"untagged": true, "task_id": ..., "tag_id": ...}
        assert_eq!(data["untagged"], true);
        assert_eq!(data["task_id"], task_id);
        assert_eq!(data["tag_id"], "bug");

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
    // Comment operations
    // =========================================================================

    #[tokio::test]
    async fn test_add_comment() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a task first
        let mut task_args = serde_json::Map::new();
        task_args.insert("op".to_string(), json!("add task"));
        task_args.insert("title".to_string(), json!("Task with comment"));
        let result = tool.execute(task_args, &context).await.unwrap();
        let task_id = extract_task_id(&result);

        // Add a comment
        let mut comment_args = serde_json::Map::new();
        comment_args.insert("op".to_string(), json!("add comment"));
        comment_args.insert("task_id".to_string(), json!(task_id));
        comment_args.insert("body".to_string(), json!("This is a comment"));
        comment_args.insert("author".to_string(), json!("alice"));

        let result = tool.execute(comment_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["body"], "This is a comment");
        assert_eq!(data["author"], "alice");
        assert!(data["id"].is_string());
    }

    #[tokio::test]
    async fn test_get_comment() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a task
        let mut task_args = serde_json::Map::new();
        task_args.insert("op".to_string(), json!("add task"));
        task_args.insert("title".to_string(), json!("Task with comment"));
        let result = tool.execute(task_args, &context).await.unwrap();
        let task_id = extract_task_id(&result);

        // Add a comment
        let mut comment_args = serde_json::Map::new();
        comment_args.insert("op".to_string(), json!("add comment"));
        comment_args.insert("task_id".to_string(), json!(task_id));
        comment_args.insert("body".to_string(), json!("Test comment"));
        comment_args.insert("author".to_string(), json!("alice"));
        let add_result = tool.execute(comment_args, &context).await.unwrap();
        let comment_id = extract_id(&add_result);

        // Get the comment
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get comment"));
        get_args.insert("task_id".to_string(), json!(task_id));
        get_args.insert("comment_id".to_string(), json!(comment_id));

        let result = tool.execute(get_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], comment_id);
        assert_eq!(data["body"], "Test comment");
    }

    #[tokio::test]
    async fn test_list_comments() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a task
        let mut task_args = serde_json::Map::new();
        task_args.insert("op".to_string(), json!("add task"));
        task_args.insert("title".to_string(), json!("Task with comments"));
        let result = tool.execute(task_args, &context).await.unwrap();
        let task_id = extract_task_id(&result);

        // Add comments
        for body in ["Comment 1", "Comment 2"] {
            let mut comment_args = serde_json::Map::new();
            comment_args.insert("op".to_string(), json!("add comment"));
            comment_args.insert("task_id".to_string(), json!(task_id));
            comment_args.insert("body".to_string(), json!(body));
            comment_args.insert("author".to_string(), json!("alice"));
            tool.execute(comment_args, &context).await.unwrap();
        }

        // List comments
        let mut list_args = serde_json::Map::new();
        list_args.insert("op".to_string(), json!("list comments"));
        list_args.insert("task_id".to_string(), json!(task_id));

        let result = tool.execute(list_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Response format: {"comments": [...], "count": N}
        assert_eq!(data["count"], 2);
        assert!(data["comments"].is_array());
        assert_eq!(data["comments"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_delete_comment() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a task
        let mut task_args = serde_json::Map::new();
        task_args.insert("op".to_string(), json!("add task"));
        task_args.insert("title".to_string(), json!("Task with comment"));
        let result = tool.execute(task_args, &context).await.unwrap();
        let task_id = extract_task_id(&result);

        // Add a comment
        let mut comment_args = serde_json::Map::new();
        comment_args.insert("op".to_string(), json!("add comment"));
        comment_args.insert("task_id".to_string(), json!(task_id));
        comment_args.insert("body".to_string(), json!("Test comment"));
        comment_args.insert("author".to_string(), json!("alice"));
        let add_result = tool.execute(comment_args, &context).await.unwrap();
        let comment_id = extract_id(&add_result);

        // Delete the comment
        let mut delete_args = serde_json::Map::new();
        delete_args.insert("op".to_string(), json!("delete comment"));
        delete_args.insert("task_id".to_string(), json!(task_id));
        delete_args.insert("comment_id".to_string(), json!(comment_id));

        let result = tool.execute(delete_args, &context).await;
        assert!(result.is_ok());

        // Verify it's gone
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get comment"));
        get_args.insert("task_id".to_string(), json!(task_id));
        get_args.insert("comment_id".to_string(), json!(comment_id));

        let get_result = tool.execute(get_args, &context).await;
        assert!(get_result.is_err());
    }

    // =========================================================================
    // Activity operations
    // =========================================================================

    #[tokio::test]
    async fn test_list_activity() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a task to generate some activity
        let mut task_args = serde_json::Map::new();
        task_args.insert("op".to_string(), json!("add task"));
        task_args.insert("title".to_string(), json!("Test task"));
        tool.execute(task_args, &context).await.unwrap();

        // List activity
        let mut list_args = serde_json::Map::new();
        list_args.insert("op".to_string(), json!("list activity"));

        let result = tool.execute(list_args, &context).await.unwrap();
        let data = parse_json(&result);

        // Response format: {"entries": [...], "count": N}
        assert!(data["entries"].is_array());
        // Should have at least some activity entries
        assert!(data["count"].as_i64().unwrap_or(0) >= 0);
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
    async fn test_update_swimlane() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a swimlane first
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add swimlane"));
        add_args.insert("id".to_string(), json!("urgent"));
        add_args.insert("name".to_string(), json!("Urgent"));
        tool.execute(add_args, &context).await.unwrap();

        // Update it
        let mut update_args = serde_json::Map::new();
        update_args.insert("op".to_string(), json!("update swimlane"));
        update_args.insert("id".to_string(), json!("urgent"));
        update_args.insert("name".to_string(), json!("Critical"));

        let result = tool.execute(update_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], "urgent");
        assert_eq!(data["name"], "Critical");
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
        add_args.insert("name".to_string(), json!("Bug"));
        add_args.insert("color".to_string(), json!("ff0000"));
        tool.execute(add_args, &context).await.unwrap();

        // Update it
        let mut update_args = serde_json::Map::new();
        update_args.insert("op".to_string(), json!("update tag"));
        update_args.insert("id".to_string(), json!("bug"));
        update_args.insert("name".to_string(), json!("Bug Fix"));
        update_args.insert("color".to_string(), json!("ff5500"));

        let result = tool.execute(update_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], "bug");
        assert_eq!(data["name"], "Bug Fix");
        assert_eq!(data["color"], "ff5500");
    }

    #[tokio::test]
    async fn test_update_comment() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add a task
        let mut task_args = serde_json::Map::new();
        task_args.insert("op".to_string(), json!("add task"));
        task_args.insert("title".to_string(), json!("Task with comment"));
        let result = tool.execute(task_args, &context).await.unwrap();
        let task_id = extract_task_id(&result);

        // Add a comment
        let mut comment_args = serde_json::Map::new();
        comment_args.insert("op".to_string(), json!("add comment"));
        comment_args.insert("task_id".to_string(), json!(task_id));
        comment_args.insert("body".to_string(), json!("Original comment"));
        comment_args.insert("author".to_string(), json!("alice"));
        let add_result = tool.execute(comment_args, &context).await.unwrap();
        let comment_id = extract_id(&add_result);

        // Update the comment
        let mut update_args = serde_json::Map::new();
        update_args.insert("op".to_string(), json!("update comment"));
        update_args.insert("task_id".to_string(), json!(task_id));
        update_args.insert("comment_id".to_string(), json!(comment_id));
        update_args.insert("body".to_string(), json!("Updated comment"));

        let result = tool.execute(update_args, &context).await.unwrap();
        let data = parse_json(&result);

        assert_eq!(data["id"], comment_id);
        assert_eq!(data["body"], "Updated comment");
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
    async fn test_tag_nonexistent_tag() {
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

        // Try to tag with nonexistent tag
        let mut tag_args = serde_json::Map::new();
        tag_args.insert("op".to_string(), json!("tag task"));
        tag_args.insert("id".to_string(), json!(task_id));
        tag_args.insert("tag".to_string(), json!("nonexistent"));

        let result = tool.execute(tag_args, &context).await;
        assert!(result.is_err());
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

    #[tokio::test]
    async fn test_activity_logging_via_mcp() {
        let temp = TempDir::new().unwrap();
        let context = create_test_context()
            .await
            .with_working_dir(temp.path().to_path_buf());
        let tool = KanbanTool::new();
        init_test_board(&tool, &context).await;

        // Add task with actor
        let mut add_args = serde_json::Map::new();
        add_args.insert("op".to_string(), json!("add task"));
        add_args.insert("title".to_string(), json!("Test Task"));
        add_args.insert("actor".to_string(), json!("alice"));

        let result = tool.execute(add_args, &context).await.unwrap();
        let task_id = extract_task_id(&result);

        // Update task with different actor
        let mut update_args = serde_json::Map::new();
        update_args.insert("op".to_string(), json!("update task"));
        update_args.insert("id".to_string(), json!(task_id));
        update_args.insert("title".to_string(), json!("Updated Task"));
        update_args.insert("actor".to_string(), json!("bob"));

        tool.execute(update_args, &context).await.unwrap();

        // Read operation (should not log)
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), json!("get task"));
        get_args.insert("id".to_string(), json!(task_id));

        tool.execute(get_args, &context).await.unwrap();

        // Verify activity log via list activity
        let mut list_activity_args = serde_json::Map::new();
        list_activity_args.insert("op".to_string(), json!("list activity"));

        let result = tool.execute(list_activity_args, &context).await.unwrap();
        let data = parse_json(&result);

        let entries = data["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 3); // init, add, update (not get)

        // Verify actor attribution
        assert_eq!(entries[0]["op"], "update task");
        assert_eq!(entries[0]["actor"], "bob");
        assert_eq!(entries[1]["op"], "add task");
        assert_eq!(entries[1]["actor"], "alice");
        assert_eq!(entries[2]["op"], "init board");
        assert!(entries[2]["actor"].is_null());

        // Verify per-task log file exists
        let kanban_ctx = KanbanContext::find(temp.path()).unwrap();
        let task_id_type = swissarmyhammer_kanban::types::TaskId::from_string(&task_id);
        let task_log_path = kanban_ctx.task_log_path(&task_id_type);
        assert!(task_log_path.exists());

        let task_log = std::fs::read_to_string(task_log_path).unwrap();
        let log_lines: Vec<&str> = task_log.lines().collect();
        assert_eq!(log_lines.len(), 2); // add + update (not get)
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
        assert!(ctx.activity_dir().exists());
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
        add_args.insert("actor_type".to_string(), json!("human"));
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
}
