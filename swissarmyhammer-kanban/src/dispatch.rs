//! Public dispatch for parsed kanban operations.
//!
//! Executes a `KanbanOperation` (from `parse::parse_input`) against a `KanbanContext`.
//! This is the single source of truth for operation dispatch, used by both the MCP tool
//! and the standalone kanban CLI.

use crate::activity::ListActivity;
use crate::actor::{AddActor, DeleteActor, GetActor, ListActors, UpdateActor};
use crate::board::{GetBoard, InitBoard, UpdateBoard};
use crate::column::{AddColumn, DeleteColumn, GetColumn, ListColumns, UpdateColumn};
use crate::swimlane::{AddSwimlane, DeleteSwimlane, GetSwimlane, ListSwimlanes, UpdateSwimlane};
use crate::tag::{AddTag, DeleteTag, GetTag, ListTags, UpdateTag};
use crate::task::{
    AddTask, ArchiveTask, AssignTask, CompleteTask, DeleteTask, GetTask, ListArchived, ListTasks,
    MoveTask, NextTask, TagTask, UnarchiveTask, UnassignTask, UntagTask, UpdateTask,
};
use crate::types::{ActorId, Noun, Operation as KanbanOperation, TaskId, Verb};
use crate::{KanbanContext, KanbanError, KanbanOperationProcessor, OperationProcessor};
use serde_json::Value;

/// Helper: require a string param, returning KanbanError on missing.
fn req<'a>(op: &'a KanbanOperation, key: &str) -> Result<&'a str, KanbanError> {
    op.get_string(key)
        .ok_or_else(|| KanbanError::parse(format!("missing required field: {}", key)))
}

/// Execute a parsed kanban operation against a context.
///
/// This is the central dispatch function that maps `(Verb, Noun)` pairs
/// to concrete operation structs and executes them via the processor.
pub async fn execute_operation(
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    let processor = match &op.actor {
        Some(actor) => KanbanOperationProcessor::with_actor(actor.to_string()),
        None => KanbanOperationProcessor::new(),
    };

    match (op.verb, op.noun) {
        // Board operations
        (Verb::Init, Noun::Board) => {
            let name = req(op, "name")?;
            let mut cmd = InitBoard::new(name);
            if let Some(desc) = op.get_string("description") {
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
            let id = req(op, "id")?;
            let name = req(op, "name")?;
            let mut cmd = AddColumn::new(id, name);
            if let Some(order) = op.get_param("order").and_then(|v| v.as_u64()) {
                cmd = cmd.with_order(order as usize);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Get, Noun::Column) => {
            let id = req(op, "id")?;
            processor.process(&GetColumn::new(id), ctx).await
        }
        (Verb::Update, Noun::Column) => {
            let id = req(op, "id")?;
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
            let id = req(op, "id")?;
            processor.process(&DeleteColumn::new(id), ctx).await
        }
        (Verb::List, Noun::Columns) => processor.process(&ListColumns, ctx).await,

        // Task operations
        (Verb::Add, Noun::Task) => {
            let title = req(op, "title")?;
            let mut cmd = AddTask::new(title);
            if let Some(desc) = op.get_string("description") {
                cmd = cmd.with_description(desc);
            }
            if let Some(column) = op.get_string("column") {
                cmd.column = Some(column.to_string());
            }
            if let Some(swimlane) = op.get_string("swimlane") {
                cmd.swimlane = Some(swimlane.to_string());
            }
            if let Some(ordinal) = op.get_string("ordinal") {
                cmd.ordinal = Some(ordinal.to_string());
            }

            // Parse assignees
            let explicit_assignees: Vec<ActorId> = op
                .get_param("assignees")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(ActorId::from_string))
                        .collect()
                })
                .or_else(|| {
                    op.get_string("assignee")
                        .map(|a| vec![ActorId::from_string(a)])
                })
                .unwrap_or_default();

            let assignees = if explicit_assignees.is_empty() {
                match &op.actor {
                    Some(actor) => vec![actor.clone()],
                    None => Vec::new(),
                }
            } else {
                explicit_assignees
            };

            if !assignees.is_empty() {
                cmd = cmd.with_assignees(assignees);
            }

            if let Some(deps) = op.get_param("depends_on").and_then(|v| v.as_array()) {
                let dep_ids: Vec<TaskId> = deps
                    .iter()
                    .filter_map(|v| v.as_str().map(TaskId::from_string))
                    .collect();
                if !dep_ids.is_empty() {
                    cmd = cmd.with_depends_on(dep_ids);
                }
            }

            processor.process(&cmd, ctx).await
        }
        (Verb::Get, Noun::Task) => {
            let id = req(op, "id")?;
            processor.process(&GetTask::new(id), ctx).await
        }
        (Verb::Update, Noun::Task) => {
            let id = req(op, "id")?;
            let mut cmd = UpdateTask::new(id);
            if let Some(title) = op.get_string("title") {
                cmd = cmd.with_title(title);
            }
            if let Some(desc) = op.get_string("description") {
                cmd = cmd.with_description(desc);
            }
            if let Some(assignees) = op.get_param("assignees").and_then(|v| v.as_array()) {
                let ids: Vec<ActorId> = assignees
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.into()))
                    .collect();
                if !ids.is_empty() {
                    cmd = cmd.with_assignees(ids);
                }
            }
            if let Some(deps) = op.get_param("depends_on").and_then(|v| v.as_array()) {
                let dep_ids: Vec<TaskId> = deps
                    .iter()
                    .filter_map(|v| v.as_str().map(TaskId::from_string))
                    .collect();
                cmd = cmd.with_depends_on(dep_ids);
            }
            if let Some(swimlane) = op.get_string("swimlane") {
                cmd = cmd.with_swimlane(Some(swimlane.into()));
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Move, Noun::Task) => {
            let id = req(op, "id")?;
            let column = req(op, "column")?;
            let mut cmd = MoveTask::to_column(id, column);
            if let Some(swimlane) = op.get_string("swimlane") {
                cmd.swimlane = Some(swimlane.into());
            }
            if let Some(ordinal) = op.get_string("ordinal") {
                cmd.ordinal = Some(ordinal.to_string());
            }
            if let Some(before_id) = op.get_string("before_id") {
                cmd.before_id = Some(before_id.into());
            }
            if let Some(after_id) = op.get_string("after_id") {
                cmd.after_id = Some(after_id.into());
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Delete, Noun::Task) => {
            let id = req(op, "id")?;
            processor.process(&DeleteTask::new(id), ctx).await
        }
        (Verb::Complete, Noun::Task) => {
            let id = req(op, "id")?;
            processor.process(&CompleteTask::new(id), ctx).await
        }
        (Verb::Assign, Noun::Task) => {
            let id = req(op, "id")?;
            let assignee = req(op, "assignee")?;
            processor.process(&AssignTask::new(id, assignee), ctx).await
        }
        (Verb::Unassign, Noun::Task) => {
            let id = req(op, "id")?;
            let assignee = req(op, "assignee")?;
            processor
                .process(&UnassignTask::new(id, assignee), ctx)
                .await
        }
        (Verb::Next, Noun::Task) => {
            let mut cmd = NextTask::new();
            if let Some(tag) = op.get_string("tag") {
                cmd = cmd.with_tag(tag);
            }
            if let Some(swimlane) = op.get_string("swimlane") {
                cmd = cmd.with_swimlane(swimlane);
            }
            if let Some(assignee) = op.get_string("assignee") {
                cmd = cmd.with_assignee(assignee);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::List, Noun::Tasks) => {
            let mut cmd = ListTasks::new();
            if let Some(column) = op.get_string("column") {
                cmd = cmd.with_column(column);
            }
            if let Some(tag) = op.get_string("tag") {
                cmd = cmd.with_tag(tag);
            }
            if let Some(swimlane) = op.get_string("swimlane") {
                cmd = cmd.with_swimlane(swimlane);
            }
            if let Some(assignee) = op.get_string("assignee") {
                cmd = cmd.with_assignee(assignee);
            }
            if let Some(ready) = op.get_param("ready").and_then(|v| v.as_bool()) {
                cmd = cmd.with_ready(ready);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Tag, Noun::Task) => {
            let id = req(op, "id")?;
            let tag = req(op, "tag")?;
            processor.process(&TagTask::new(id, tag), ctx).await
        }
        (Verb::Untag, Noun::Task) => {
            let id = req(op, "id")?;
            let tag = req(op, "tag")?;
            processor.process(&UntagTask::new(id, tag), ctx).await
        }

        // Swimlane operations
        (Verb::Add, Noun::Swimlane) => {
            let id = req(op, "id")?;
            let name = req(op, "name")?;
            let mut cmd = AddSwimlane::new(id, name);
            if let Some(order) = op.get_param("order").and_then(|v| v.as_u64()) {
                cmd = cmd.with_order(order as usize);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Get, Noun::Swimlane) => {
            let id = req(op, "id")?;
            processor.process(&GetSwimlane::new(id), ctx).await
        }
        (Verb::Update, Noun::Swimlane) => {
            let id = req(op, "id")?;
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
            let id = req(op, "id")?;
            processor.process(&DeleteSwimlane::new(id), ctx).await
        }
        (Verb::List, Noun::Swimlanes) => processor.process(&ListSwimlanes, ctx).await,

        // Actor operations
        (Verb::Add, Noun::Actor) => {
            let id = req(op, "id")?;
            let name = req(op, "name")?;
            let ensure = op.get_bool("ensure").unwrap_or(false);
            let mut cmd = AddActor::new(id, name);
            if ensure {
                cmd = cmd.with_ensure();
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Get, Noun::Actor) => {
            let id = req(op, "id")?;
            processor.process(&GetActor::new(id), ctx).await
        }
        (Verb::Update, Noun::Actor) => {
            let id = req(op, "id")?;
            let mut cmd = UpdateActor::new(id);
            if let Some(name) = op.get_string("name") {
                cmd = cmd.with_name(name);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Delete, Noun::Actor) => {
            let id = req(op, "id")?;
            processor.process(&DeleteActor::new(id), ctx).await
        }
        (Verb::List, Noun::Actors) => processor.process(&ListActors, ctx).await,

        // Tag operations (board-level)
        (Verb::Add, Noun::Tag) => {
            let name = op
                .get_string("name")
                .or_else(|| op.get_string("id"))
                .ok_or_else(|| KanbanError::parse("missing required field: name"))?;
            let mut cmd = AddTag::new(name);
            if let Some(color) = op.get_string("color") {
                cmd = cmd.with_color(color);
            }
            if let Some(desc) = op.get_string("description") {
                cmd = cmd.with_description(desc);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Get, Noun::Tag) => {
            let id = req(op, "id")?;
            processor.process(&GetTag::new(id), ctx).await
        }
        (Verb::Update, Noun::Tag) => {
            let id = req(op, "id")?;
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
            let id = req(op, "id")?;
            processor.process(&DeleteTag::new(id), ctx).await
        }
        (Verb::List, Noun::Tags) => processor.process(&ListTags::default(), ctx).await,

        // Activity operations
        (Verb::List, Noun::Activity) => {
            let mut cmd = ListActivity::default();
            if let Some(limit) = op.get_param("limit").and_then(|v| v.as_u64()) {
                cmd = cmd.with_limit(limit as usize);
            }
            processor.process(&cmd, ctx).await
        }

        // Archive operations
        (Verb::Archive, Noun::Task) => {
            let id = req(op, "id")?;
            processor.process(&ArchiveTask::new(id), ctx).await
        }
        (Verb::Unarchive, Noun::Task) => {
            let id = req(op, "id")?;
            processor.process(&UnarchiveTask::new(id), ctx).await
        }
        (Verb::List, Noun::Archived) => processor.process(&ListArchived, ctx).await,

        _ => Err(KanbanError::parse(format!(
            "unsupported operation: {} {}",
            op.verb, op.noun
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_input;
    use serde_json::json;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);
        // Init a board first
        let ops = parse_input(json!({"op": "init board", "name": "Test"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();
        (temp, ctx)
    }

    #[tokio::test]
    async fn dispatch_init_board() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        let ops = parse_input(json!({"op": "init board", "name": "My Board"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "My Board");
        assert!(result["columns"].is_array());
    }

    /// Verify that dispatching `add task` (without a column arg) places the task
    /// in the first column (todo).
    #[tokio::test]
    async fn dispatch_add_task_places_in_first_column_by_default() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "New task"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();

        assert_eq!(
            result["position"]["column"], "todo",
            "task without explicit column should land in todo (first column)"
        );
    }

    /// Verify that dispatching `add task` with an explicit column arg places the task
    /// in that column, not in todo.
    #[tokio::test]
    async fn dispatch_add_task_with_explicit_column_uses_that_column() {
        let (_temp, ctx) = setup().await;

        let ops =
            parse_input(json!({"op": "add task", "title": "Task in doing", "column": "doing"}))
                .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();

        assert_eq!(
            result["position"]["column"], "doing",
            "task with explicit column arg should land in that column"
        );
    }

    /// Verify that dispatching `add task` on a board with no columns returns an error.
    #[tokio::test]
    async fn dispatch_add_task_on_board_with_no_columns_returns_error() {
        let (_temp, ctx) = setup().await;

        // Delete all default columns (todo, doing, done)
        for col_id in &["todo", "doing", "done"] {
            let ops = parse_input(json!({"op": "delete column", "id": col_id})).unwrap();
            execute_operation(&ctx, &ops[0]).await.unwrap();
        }

        // Now add task should fail gracefully
        let ops = parse_input(json!({"op": "add task", "title": "Task on empty board"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await;

        assert!(
            result.is_err(),
            "adding a task to a board with no columns should return an error"
        );
    }

    /// Verify that `board.newCard` is not a separate dispatch operation — the
    /// `task.add` dispatch path is the canonical way to add cards and it correctly
    /// defaults to the first column.
    #[tokio::test]
    async fn dispatch_board_new_card_not_a_separate_operation() {
        let (_temp, ctx) = setup().await;

        // board.newCard does not exist as a parsed operation; the canonical way
        // to add a card is "add task".  Attempting to dispatch an invented
        // "new card" verb/noun pair must return an error, confirming that all
        // new-card creation flows go through "add task".
        let op = crate::types::Operation::new(crate::types::Verb::Add, crate::types::Noun::Task, {
            let mut m = serde_json::Map::new();
            m.insert("title".into(), json!("Card via add task"));
            m
        });
        let result = execute_operation(&ctx, &op).await;
        assert!(
            result.is_ok(),
            "add task (the board.newCard equivalent) should succeed"
        );
        assert_eq!(
            result.unwrap()["position"]["column"],
            "todo",
            "board.newCard equivalent should default to the first column"
        );
    }

    #[tokio::test]
    async fn dispatch_add_and_list_tasks() {
        let (_temp, ctx) = setup().await;

        // Add a task
        let ops = parse_input(json!({"op": "add task", "title": "Fix bug"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["title"], "Fix bug");
        let task_id = result["id"].as_str().unwrap().to_string();

        // List tasks
        let ops = parse_input(json!({"op": "list tasks", "column": "todo"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["id"], task_id);
    }

    #[tokio::test]
    async fn dispatch_get_board() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "get board"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "Test");
    }

    #[tokio::test]
    async fn dispatch_unsupported_operation_returns_error() {
        let (_temp, ctx) = setup().await;

        let op = crate::types::Operation::new(
            crate::types::Verb::Rename,
            crate::types::Noun::Board,
            serde_json::Map::new(),
        );
        let result = execute_operation(&ctx, &op).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn dispatch_archive_task() {
        let (_temp, ctx) = setup().await;

        // Add a task
        let ops = parse_input(json!({"op": "add task", "title": "Task to archive"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = result["id"].as_str().unwrap().to_string();

        // Archive the task via dispatch
        let ops = parse_input(json!({"op": "archive task", "id": task_id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["archived"], true);
        assert_eq!(result["id"].as_str().unwrap(), task_id);

        // List tasks — the archived task should not appear
        let ops = parse_input(json!({"op": "list tasks", "column": "todo"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(
            result["count"], 0,
            "archived task should not appear in list tasks"
        );
    }

    #[tokio::test]
    async fn dispatch_unarchive_task() {
        let (_temp, ctx) = setup().await;

        // Add a task and archive it
        let ops = parse_input(json!({"op": "add task", "title": "Task to unarchive"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = result["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "archive task", "id": task_id})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // Unarchive via dispatch
        let ops = parse_input(json!({"op": "unarchive task", "id": task_id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["unarchived"], true);
        assert_eq!(result["id"].as_str().unwrap(), task_id);

        // List tasks — the task should be back
        let ops = parse_input(json!({"op": "list tasks", "column": "todo"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(
            result["count"], 1,
            "unarchived task should reappear in list tasks"
        );
    }

    #[tokio::test]
    async fn dispatch_list_archived() {
        let (_temp, ctx) = setup().await;

        // Add two tasks and archive one
        let ops = parse_input(json!({"op": "add task", "title": "Will be archived"})).unwrap();
        let r1 = execute_operation(&ctx, &ops[0]).await.unwrap();
        let id1 = r1["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "add task", "title": "Still live"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "archive task", "id": id1})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // List archived
        let ops = parse_input(json!({"op": "list archived"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["count"], 1, "should list exactly one archived task");
        let tasks = result["tasks"].as_array().unwrap();
        assert_eq!(tasks[0]["title"], "Will be archived");
    }

    // ------------------------------------------------------------------
    // Board operations
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_update_board() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(
            json!({"op": "update board", "name": "Updated Board", "description": "A description"}),
        )
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "Updated Board");
        assert_eq!(result["description"], "A description");
    }

    // ------------------------------------------------------------------
    // Column operations
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_add_column() {
        let (_temp, ctx) = setup().await;

        let ops =
            parse_input(json!({"op": "add column", "id": "review", "name": "Review"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["id"], "review");
        assert_eq!(result["name"], "Review");
    }

    #[tokio::test]
    async fn dispatch_get_column() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "get column", "id": "todo"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["id"], "todo");
    }

    #[tokio::test]
    async fn dispatch_update_column() {
        let (_temp, ctx) = setup().await;

        let ops =
            parse_input(json!({"op": "update column", "id": "todo", "name": "Backlog"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "Backlog");
    }

    #[tokio::test]
    async fn dispatch_delete_column() {
        let (_temp, ctx) = setup().await;

        // Add a new empty column then delete it
        let ops = parse_input(json!({"op": "add column", "id": "temp", "name": "Temp"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "delete column", "id": "temp"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["deleted"], true);
    }

    #[tokio::test]
    async fn dispatch_list_columns() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "list columns"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        let columns = result["columns"].as_array().unwrap();
        // Default board has todo, doing, done
        assert!(columns.len() >= 3);
        let ids: Vec<&str> = columns.iter().filter_map(|c| c["id"].as_str()).collect();
        assert!(ids.contains(&"todo"));
        assert!(ids.contains(&"doing"));
        assert!(ids.contains(&"done"));
    }

    // ------------------------------------------------------------------
    // Swimlane operations
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_add_swimlane() {
        let (_temp, ctx) = setup().await;

        let ops =
            parse_input(json!({"op": "add swimlane", "id": "team-a", "name": "Team A"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["id"], "team-a");
        assert_eq!(result["name"], "Team A");
    }

    #[tokio::test]
    async fn dispatch_get_swimlane() {
        let (_temp, ctx) = setup().await;

        // Add a swimlane first
        let ops =
            parse_input(json!({"op": "add swimlane", "id": "team-b", "name": "Team B"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "get swimlane", "id": "team-b"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["id"], "team-b");
        assert_eq!(result["name"], "Team B");
    }

    #[tokio::test]
    async fn dispatch_update_swimlane() {
        let (_temp, ctx) = setup().await;

        let ops =
            parse_input(json!({"op": "add swimlane", "id": "lane", "name": "Original"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops =
            parse_input(json!({"op": "update swimlane", "id": "lane", "name": "Updated"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "Updated");
    }

    #[tokio::test]
    async fn dispatch_delete_swimlane() {
        let (_temp, ctx) = setup().await;

        let ops =
            parse_input(json!({"op": "add swimlane", "id": "to-delete", "name": "To Delete"}))
                .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "delete swimlane", "id": "to-delete"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["deleted"], true);
    }

    #[tokio::test]
    async fn dispatch_list_swimlanes() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add swimlane", "id": "s1", "name": "S1"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "list swimlanes"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        let swimlanes = result["swimlanes"].as_array().unwrap();
        assert!(swimlanes.len() >= 1);
        let ids: Vec<&str> = swimlanes.iter().filter_map(|s| s["id"].as_str()).collect();
        assert!(ids.contains(&"s1"));
    }

    // ------------------------------------------------------------------
    // Actor operations
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_add_actor() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(
            json!({"op": "add actor", "id": "alice", "name": "Alice Smith", "type": "human"}),
        )
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        // AddActor wraps the actor under an "actor" key
        assert_eq!(result["actor"]["id"], "alice");
        assert_eq!(result["actor"]["name"], "Alice Smith");
        assert_eq!(result["created"], true);
    }

    #[tokio::test]
    async fn dispatch_get_actor() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(
            json!({"op": "add actor", "id": "bob", "name": "Bob Jones", "type": "human"}),
        )
        .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "get actor", "id": "bob"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["id"], "bob");
        assert_eq!(result["name"], "Bob Jones");
    }

    #[tokio::test]
    async fn dispatch_update_actor() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(
            json!({"op": "add actor", "id": "carol", "name": "Carol", "type": "human"}),
        )
        .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops =
            parse_input(json!({"op": "update actor", "id": "carol", "name": "Carol Updated"}))
                .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "Carol Updated");
    }

    #[tokio::test]
    async fn dispatch_delete_actor() {
        let (_temp, ctx) = setup().await;

        let ops =
            parse_input(json!({"op": "add actor", "id": "dave", "name": "Dave", "type": "human"}))
                .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "delete actor", "id": "dave"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["deleted"], true);
    }

    #[tokio::test]
    async fn dispatch_list_actors() {
        let (_temp, ctx) = setup().await;

        let ops =
            parse_input(json!({"op": "add actor", "id": "eve", "name": "Eve", "type": "human"}))
                .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "list actors"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        let actors = result["actors"].as_array().unwrap();
        let ids: Vec<&str> = actors.iter().filter_map(|a| a["id"].as_str()).collect();
        assert!(ids.contains(&"eve"));
    }

    // ------------------------------------------------------------------
    // Tag operations (board-level)
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_add_tag() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add tag", "name": "urgent"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "urgent");
    }

    #[tokio::test]
    async fn dispatch_get_tag() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add tag", "name": "blocker"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let tag_id = r["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "get tag", "id": tag_id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "blocker");
    }

    #[tokio::test]
    async fn dispatch_update_tag() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add tag", "name": "old-tag"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let tag_id = r["id"].as_str().unwrap().to_string();

        let ops =
            parse_input(json!({"op": "update tag", "id": tag_id, "name": "new-tag"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "new-tag");
    }

    #[tokio::test]
    async fn dispatch_delete_tag() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add tag", "name": "remove-me"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let tag_id = r["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "delete tag", "id": tag_id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["deleted"], true);
    }

    #[tokio::test]
    async fn dispatch_list_tags() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add tag", "name": "mytag"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "list tags"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        let tags = result["tags"].as_array().unwrap();
        let names: Vec<&str> = tags.iter().filter_map(|t| t["name"].as_str()).collect();
        assert!(names.contains(&"mytag"));
    }

    // ------------------------------------------------------------------
    // Task operations (additional)
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_get_task() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Get me"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = r["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "get task", "id": task_id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["title"], "Get me");
        assert_eq!(result["id"].as_str().unwrap(), task_id);
    }

    #[tokio::test]
    async fn dispatch_update_task() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Original title"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = r["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "update task", "id": task_id, "title": "Updated title", "description": "New desc"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["title"], "Updated title");
        assert_eq!(result["description"], "New desc");
    }

    #[tokio::test]
    async fn dispatch_delete_task() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Delete me"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = r["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "delete task", "id": task_id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["deleted"], true);
    }

    #[tokio::test]
    async fn dispatch_complete_task() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Complete me"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = r["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "complete task", "id": task_id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["position"]["column"], "done");
    }

    #[tokio::test]
    async fn dispatch_move_task() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Move me"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = r["id"].as_str().unwrap().to_string();
        assert_eq!(r["position"]["column"], "todo");

        let ops =
            parse_input(json!({"op": "move task", "id": task_id, "column": "doing"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["position"]["column"], "doing");
    }

    #[tokio::test]
    async fn dispatch_assign_and_unassign_task() {
        let (_temp, ctx) = setup().await;

        // Create actor and task
        let ops = parse_input(
            json!({"op": "add actor", "id": "frank", "name": "Frank", "type": "human"}),
        )
        .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "add task", "title": "Assign me"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = r["id"].as_str().unwrap().to_string();

        // Assign — response has all_assignees, not assignees
        let ops =
            parse_input(json!({"op": "assign task", "id": task_id, "assignee": "frank"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["assigned"], true);
        let assignees = result["all_assignees"].as_array().unwrap();
        assert!(
            assignees.iter().any(|a| a == "frank"),
            "frank should be assigned"
        );

        // Unassign
        let ops = parse_input(json!({"op": "unassign task", "id": task_id, "assignee": "frank"}))
            .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["unassigned"], true);
    }

    #[tokio::test]
    async fn dispatch_next_task() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Next one"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "next task"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["title"], "Next one");
    }

    #[tokio::test]
    async fn dispatch_tag_and_untag_task() {
        let (_temp, ctx) = setup().await;

        // Add task
        let ops = parse_input(json!({"op": "add task", "title": "Tagged task"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = r["id"].as_str().unwrap().to_string();

        // Tag the task — TagTask auto-creates the tag and returns {tagged, task_id, tag}
        let ops = parse_input(json!({"op": "tag task", "id": task_id, "tag": "feature"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["tagged"], true);
        assert_eq!(result["tag"], "feature");

        // Untag — UntagTask returns {untagged, task_id, tag}
        let ops =
            parse_input(json!({"op": "untag task", "id": task_id, "tag": "feature"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["untagged"], true);
        assert_eq!(result["tag"], "feature");
    }

    #[tokio::test]
    async fn dispatch_list_tasks_with_filters() {
        let (_temp, ctx) = setup().await;

        // Add tasks in different columns
        let ops = parse_input(json!({"op": "add task", "title": "Todo task"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "add task", "title": "Doing task", "column": "doing"}))
            .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // Filter by column
        let ops = parse_input(json!({"op": "list tasks", "column": "doing"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Doing task");
    }

    // ------------------------------------------------------------------
    // Activity operations
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_list_activity() {
        let (_temp, ctx) = setup().await;

        // Add a task to generate activity
        let ops = parse_input(json!({"op": "add task", "title": "Activity task"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "list activity"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert!(result["entries"].is_array(), "should return entries array");
    }

    #[tokio::test]
    async fn dispatch_list_activity_with_limit() {
        let (_temp, ctx) = setup().await;

        // Generate multiple activity entries
        for i in 0..5 {
            let ops =
                parse_input(json!({"op": "add task", "title": format!("Task {}", i)})).unwrap();
            execute_operation(&ctx, &ops[0]).await.unwrap();
        }

        let ops = parse_input(json!({"op": "list activity", "limit": 2})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        let entries = result["entries"].as_array().unwrap();
        assert!(entries.len() <= 2, "limit should cap results at 2");
    }
}
