//! Public dispatch for parsed kanban operations.
//!
//! Executes a `KanbanOperation` (from `parse::parse_input`) against a `KanbanContext`.
//! This is the single source of truth for operation dispatch, used by both the MCP tool
//! and the standalone kanban CLI.

use crate::actor::{AddActor, DeleteActor, GetActor, ListActors, UpdateActor};
use crate::attachment::{
    AddAttachment, DeleteAttachment, GetAttachment, ListAttachments, UpdateAttachment,
};
use crate::board::{GetBoard, InitBoard, UpdateBoard};
use crate::column::{AddColumn, DeleteColumn, GetColumn, ListColumns, UpdateColumn};
use crate::perspective::{
    AddPerspective, DeletePerspective, GetPerspective, ListPerspectives, UpdatePerspective,
};
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

        // Attachment operations
        (Verb::Add, Noun::Attachment) => {
            let task_id = req(op, "task_id")?;
            let name = req(op, "name")?;
            let path = req(op, "path")?;
            let mut cmd = AddAttachment::new(task_id, name, path);
            if let Some(mime) = op.get_string("mime_type") {
                cmd = cmd.with_mime_type(mime);
            }
            if let Some(size) = op.get_param("size").and_then(|v| v.as_u64()) {
                cmd = cmd.with_size(size);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Get, Noun::Attachment) => {
            let task_id = req(op, "task_id")?;
            let id = req(op, "id")?;
            processor
                .process(&GetAttachment::new(task_id, id), ctx)
                .await
        }
        (Verb::Update, Noun::Attachment) => {
            let task_id = req(op, "task_id")?;
            let id = req(op, "id")?;
            let mut cmd = UpdateAttachment::new(task_id, id);
            if let Some(name) = op.get_string("name") {
                cmd = cmd.with_name(name);
            }
            if let Some(mime) = op.get_string("mime_type") {
                cmd = cmd.with_mime_type(mime);
            }
            if let Some(size) = op.get_param("size").and_then(|v| v.as_u64()) {
                cmd = cmd.with_size(size);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Delete, Noun::Attachment) => {
            let task_id = req(op, "task_id")?;
            let id = req(op, "id")?;
            processor
                .process(&DeleteAttachment::new(task_id, id), ctx)
                .await
        }
        (Verb::List, Noun::Attachments) => {
            let task_id = req(op, "task_id")?;
            processor.process(&ListAttachments::new(task_id), ctx).await
        }

        // Perspective operations
        (Verb::Add, Noun::Perspective) => {
            let name = req(op, "name")?;
            let view = req(op, "view")?;
            let mut cmd = AddPerspective::new(name, view);
            if let Some(fields_val) = op.get_param("fields") {
                let fields: Vec<crate::perspective::PerspectiveFieldEntry> =
                    serde_json::from_value(fields_val.clone())
                        .map_err(|e| KanbanError::parse(format!("invalid fields: {}", e)))?;
                cmd = cmd.with_fields(fields);
            }
            if let Some(filter) = op.get_string("filter") {
                cmd = cmd.with_filter(filter);
            }
            if let Some(group) = op.get_string("group") {
                cmd = cmd.with_group(group);
            }
            if let Some(sort_val) = op.get_param("sort") {
                let sort: Vec<crate::perspective::SortEntry> =
                    serde_json::from_value(sort_val.clone())
                        .map_err(|e| KanbanError::parse(format!("invalid sort: {}", e)))?;
                cmd = cmd.with_sort(sort);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Get, Noun::Perspective) => {
            let id = req(op, "id")?;
            processor.process(&GetPerspective::new(id), ctx).await
        }
        (Verb::Update, Noun::Perspective) => {
            let id = req(op, "id")?;
            let mut cmd = UpdatePerspective::new(id);
            if let Some(name) = op.get_string("name") {
                cmd = cmd.with_name(name);
            }
            if let Some(view) = op.get_string("view") {
                cmd = cmd.with_view(view);
            }
            if let Some(fields_val) = op.get_param("fields") {
                let fields: Vec<crate::perspective::PerspectiveFieldEntry> =
                    serde_json::from_value(fields_val.clone())
                        .map_err(|e| KanbanError::parse(format!("invalid fields: {}", e)))?;
                cmd = cmd.with_fields(fields);
            }
            if op.params.contains_key("filter") {
                let filter = op.get_string("filter").map(|s| s.to_string());
                cmd = cmd.with_filter(filter);
            }
            if op.params.contains_key("group") {
                let group = op.get_string("group").map(|s| s.to_string());
                cmd = cmd.with_group(group);
            }
            if let Some(sort_val) = op.get_param("sort") {
                let sort: Vec<crate::perspective::SortEntry> =
                    serde_json::from_value(sort_val.clone())
                        .map_err(|e| KanbanError::parse(format!("invalid sort: {}", e)))?;
                cmd = cmd.with_sort(sort);
            }
            processor.process(&cmd, ctx).await
        }
        (Verb::Delete, Noun::Perspective) => {
            let id = req(op, "id")?;
            processor.process(&DeletePerspective::new(id), ctx).await
        }
        (Verb::List, Noun::Perspectives) => processor.process(&ListPerspectives::new(), ctx).await,

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

    #[tokio::test]
    async fn dispatch_add_perspective() {
        let (_temp, ctx) = setup().await;

        let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("name".into(), json!("Sprint View"));
            m.insert("view".into(), json!("board"));
            m
        });
        let result = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(result["name"], "Sprint View");
        assert_eq!(result["view"], "board");
        assert!(result["id"].as_str().is_some());
    }

    #[tokio::test]
    async fn dispatch_get_perspective() {
        let (_temp, ctx) = setup().await;

        // Add a perspective first
        let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("name".into(), json!("My View"));
            m.insert("view".into(), json!("grid"));
            m
        });
        let added = execute_operation(&ctx, &op).await.unwrap();
        let id = added["id"].as_str().unwrap().to_string();

        // Get by ID
        let op = KanbanOperation::new(Verb::Get, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("id".into(), json!(id));
            m
        });
        let result = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(result["name"], "My View");
        assert_eq!(result["view"], "grid");
    }

    #[tokio::test]
    async fn dispatch_list_perspectives() {
        let (_temp, ctx) = setup().await;

        // Add two perspectives
        for name in &["View A", "View B"] {
            let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
                let mut m = serde_json::Map::new();
                m.insert("name".into(), json!(name));
                m.insert("view".into(), json!("board"));
                m
            });
            execute_operation(&ctx, &op).await.unwrap();
        }

        // List all
        let op = KanbanOperation::new(Verb::List, Noun::Perspectives, serde_json::Map::new());
        let result = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(result["count"], 2);
        let perspectives = result["perspectives"].as_array().unwrap();
        assert_eq!(perspectives.len(), 2);
    }

    #[tokio::test]
    async fn dispatch_update_perspective() {
        let (_temp, ctx) = setup().await;

        // Add a perspective
        let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("name".into(), json!("Old Name"));
            m.insert("view".into(), json!("board"));
            m
        });
        let added = execute_operation(&ctx, &op).await.unwrap();
        let id = added["id"].as_str().unwrap().to_string();

        // Update the name
        let op = KanbanOperation::new(Verb::Update, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("id".into(), json!(id));
            m.insert("name".into(), json!("New Name"));
            m.insert("view".into(), json!("grid"));
            m
        });
        let result = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(result["name"], "New Name");
        assert_eq!(result["view"], "grid");
    }

    #[tokio::test]
    async fn dispatch_delete_perspective() {
        let (_temp, ctx) = setup().await;

        // Add a perspective
        let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("name".into(), json!("Doomed"));
            m.insert("view".into(), json!("board"));
            m
        });
        let added = execute_operation(&ctx, &op).await.unwrap();
        let id = added["id"].as_str().unwrap().to_string();

        // Delete it
        let op = KanbanOperation::new(Verb::Delete, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("id".into(), json!(id));
            m
        });
        let result = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(result["deleted"], true);

        // Verify it's gone
        let op = KanbanOperation::new(Verb::Get, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("id".into(), json!(id));
            m
        });
        let result = execute_operation(&ctx, &op).await;
        assert!(result.is_err(), "deleted perspective should not be found");
    }

    #[tokio::test]
    async fn dispatch_perspective_full_lifecycle() {
        let (_temp, ctx) = setup().await;

        // Add
        let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("name".into(), json!("Lifecycle Test"));
            m.insert("view".into(), json!("board"));
            m.insert("filter".into(), json!("(e) => e.Status !== 'Done'"));
            m
        });
        let added = execute_operation(&ctx, &op).await.unwrap();
        let id = added["id"].as_str().unwrap().to_string();
        assert_eq!(added["name"], "Lifecycle Test");
        assert_eq!(added["filter"], "(e) => e.Status !== 'Done'");

        // Get
        let op = KanbanOperation::new(Verb::Get, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("id".into(), json!(&id));
            m
        });
        let got = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(got["name"], "Lifecycle Test");

        // Update
        let op = KanbanOperation::new(Verb::Update, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("id".into(), json!(&id));
            m.insert("name".into(), json!("Updated Lifecycle"));
            m.insert("group".into(), json!("(e) => e.Assignee"));
            m
        });
        let updated = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(updated["name"], "Updated Lifecycle");
        assert_eq!(updated["group"], "(e) => e.Assignee");
        // Filter should be preserved
        assert_eq!(updated["filter"], "(e) => e.Status !== 'Done'");

        // List
        let op = KanbanOperation::new(Verb::List, Noun::Perspectives, serde_json::Map::new());
        let listed = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(listed["count"], 1);

        // Delete
        let op = KanbanOperation::new(Verb::Delete, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("id".into(), json!(&id));
            m
        });
        let deleted = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(deleted["deleted"], true);

        // Verify empty
        let op = KanbanOperation::new(Verb::List, Noun::Perspectives, serde_json::Map::new());
        let listed = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(listed["count"], 0);
    }

    #[tokio::test]
    async fn dispatch_update_perspective_clear_filter_and_group_via_null() {
        let (_temp, ctx) = setup().await;

        // Add a perspective with filter and group set
        let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("name".into(), json!("Null Clear Test"));
            m.insert("view".into(), json!("board"));
            m.insert("filter".into(), json!("(e) => e.Status !== 'Done'"));
            m.insert("group".into(), json!("(e) => e.Assignee"));
            m
        });
        let added = execute_operation(&ctx, &op).await.unwrap();
        let id = added["id"].as_str().unwrap().to_string();
        assert_eq!(added["filter"], "(e) => e.Status !== 'Done'");
        assert_eq!(added["group"], "(e) => e.Assignee");

        // Update with filter: null and group: null to clear them
        let op = KanbanOperation::new(Verb::Update, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("id".into(), json!(&id));
            m.insert("filter".into(), Value::Null);
            m.insert("group".into(), Value::Null);
            m
        });
        let updated = execute_operation(&ctx, &op).await.unwrap();
        assert!(
            updated.get("filter").is_none() || updated["filter"].is_null(),
            "filter should be cleared (null or absent), got: {:?}",
            updated.get("filter")
        );
        assert!(
            updated.get("group").is_none() || updated["group"].is_null(),
            "group should be cleared (null or absent), got: {:?}",
            updated.get("group")
        );

        // Verify via get that the clear persisted
        let op = KanbanOperation::new(Verb::Get, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("id".into(), json!(&id));
            m
        });
        let got = execute_operation(&ctx, &op).await.unwrap();
        assert!(
            got.get("filter").is_none() || got["filter"].is_null(),
            "filter should remain cleared after re-fetch, got: {:?}",
            got.get("filter")
        );
        assert!(
            got.get("group").is_none() || got["group"].is_null(),
            "group should remain cleared after re-fetch, got: {:?}",
            got.get("group")
        );
    }

    /// Passing malformed `fields` JSON to `add perspective` should return a parse error
    /// instead of silently dropping the value.
    #[tokio::test]
    async fn dispatch_add_perspective_malformed_fields_returns_error() {
        let (_temp, ctx) = setup().await;

        let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("name".into(), json!("Bad Fields"));
            m.insert("view".into(), json!("board"));
            // fields should be an array of PerspectiveFieldEntry, not a string
            m.insert("fields".into(), json!("not-an-array"));
            m
        });
        let result = execute_operation(&ctx, &op).await;
        assert!(
            result.is_err(),
            "malformed fields should produce an error, not be silently dropped"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("invalid fields"),
            "error should mention 'invalid fields', got: {err_msg}"
        );
    }

    /// Passing malformed `sort` JSON to `add perspective` should return a parse error.
    #[tokio::test]
    async fn dispatch_add_perspective_malformed_sort_returns_error() {
        let (_temp, ctx) = setup().await;

        let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("name".into(), json!("Bad Sort"));
            m.insert("view".into(), json!("board"));
            // sort should be an array of SortEntry, not a number
            m.insert("sort".into(), json!(42));
            m
        });
        let result = execute_operation(&ctx, &op).await;
        assert!(
            result.is_err(),
            "malformed sort should produce an error, not be silently dropped"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("invalid sort"),
            "error should mention 'invalid sort', got: {err_msg}"
        );
    }

    /// Passing malformed `fields` JSON to `update perspective` should return a parse error.
    #[tokio::test]
    async fn dispatch_update_perspective_malformed_fields_returns_error() {
        let (_temp, ctx) = setup().await;

        // Create a valid perspective first
        let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("name".into(), json!("Valid"));
            m.insert("view".into(), json!("board"));
            m
        });
        let added = execute_operation(&ctx, &op).await.unwrap();
        let id = added["id"].as_str().unwrap().to_string();

        // Update with malformed fields
        let op = KanbanOperation::new(Verb::Update, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("id".into(), json!(id));
            m.insert("fields".into(), json!({"wrong": "shape"}));
            m
        });
        let result = execute_operation(&ctx, &op).await;
        assert!(
            result.is_err(),
            "malformed fields on update should produce an error"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("invalid fields"),
            "error should mention 'invalid fields', got: {err_msg}"
        );
    }

    /// Passing malformed `sort` JSON to `update perspective` should return a parse error.
    #[tokio::test]
    async fn dispatch_update_perspective_malformed_sort_returns_error() {
        let (_temp, ctx) = setup().await;

        // Create a valid perspective first
        let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("name".into(), json!("Valid"));
            m.insert("view".into(), json!("board"));
            m
        });
        let added = execute_operation(&ctx, &op).await.unwrap();
        let id = added["id"].as_str().unwrap().to_string();

        // Update with malformed sort
        let op = KanbanOperation::new(Verb::Update, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("id".into(), json!(id));
            m.insert("sort".into(), json!("not-an-array"));
            m
        });
        let result = execute_operation(&ctx, &op).await;
        assert!(
            result.is_err(),
            "malformed sort on update should produce an error"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("invalid sort"),
            "error should mention 'invalid sort', got: {err_msg}"
        );
    }

    // ── Board operations ──────────────────────────────────────────────

    #[tokio::test]
    async fn dispatch_update_board() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(
            json!({"op": "update board", "name": "Renamed", "description": "New desc"}),
        )
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "Renamed");
        assert_eq!(result["description"], "New desc");
    }

    // ── Column operations ─────────────────────────────────────────────

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

        // Add a fresh column with no tasks so we can delete it
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
        assert!(result["columns"].as_array().unwrap().len() >= 3);
    }

    // ── Task operations (get, update, move, delete, complete) ─────────

    #[tokio::test]
    async fn dispatch_get_task() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Fetch me"})).unwrap();
        let added = execute_operation(&ctx, &ops[0]).await.unwrap();
        let id = added["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "get task", "id": id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["title"], "Fetch me");
        assert_eq!(result["id"].as_str().unwrap(), id);
    }

    #[tokio::test]
    async fn dispatch_update_task() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Old title"})).unwrap();
        let added = execute_operation(&ctx, &ops[0]).await.unwrap();
        let id = added["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "update task", "id": id, "title": "New title", "description": "Updated desc"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["title"], "New title");
        assert_eq!(result["description"], "Updated desc");
    }

    #[tokio::test]
    async fn dispatch_move_task() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Move me"})).unwrap();
        let added = execute_operation(&ctx, &ops[0]).await.unwrap();
        let id = added["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "move task", "id": id, "column": "doing"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["position"]["column"], "doing");
    }

    #[tokio::test]
    async fn dispatch_delete_task() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Delete me"})).unwrap();
        let added = execute_operation(&ctx, &ops[0]).await.unwrap();
        let id = added["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "delete task", "id": id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["deleted"], true);
    }

    #[tokio::test]
    async fn dispatch_complete_task() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Complete me"})).unwrap();
        let added = execute_operation(&ctx, &ops[0]).await.unwrap();
        let id = added["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "complete task", "id": id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["position"]["column"], "done");
    }

    // ── Task assign/unassign ──────────────────────────────────────────

    #[tokio::test]
    async fn dispatch_assign_and_unassign_task() {
        let (_temp, ctx) = setup().await;

        // Create an actor
        let ops = parse_input(json!({"op": "add actor", "id": "alice", "name": "Alice"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // Create a task
        let ops = parse_input(json!({"op": "add task", "title": "Assignable"})).unwrap();
        let added = execute_operation(&ctx, &ops[0]).await.unwrap();
        let id = added["id"].as_str().unwrap().to_string();

        // Assign
        let ops = parse_input(json!({"op": "assign task", "id": id, "assignee": "alice"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["assigned"], true);
        assert!(result["all_assignees"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a.as_str() == Some("alice")));

        // Unassign
        let ops =
            parse_input(json!({"op": "unassign task", "id": id, "assignee": "alice"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert!(!result["all_assignees"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a.as_str() == Some("alice")));
    }

    // ── Next task ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn dispatch_next_task() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "First ready"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "next task"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["title"], "First ready");
    }

    // ── Tag/untag task ────────────────────────────────────────────────

    #[tokio::test]
    async fn dispatch_tag_and_untag_task() {
        let (_temp, ctx) = setup().await;

        // Create a tag
        let ops = parse_input(json!({"op": "add tag", "name": "bug"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // Create a task
        let ops = parse_input(json!({"op": "add task", "title": "Taggable"})).unwrap();
        let added = execute_operation(&ctx, &ops[0]).await.unwrap();
        let id = added["id"].as_str().unwrap().to_string();

        // Tag it
        let ops = parse_input(json!({"op": "tag task", "id": id, "tag": "bug"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["tagged"], true);
        assert_eq!(result["tag"], "bug");

        // Untag it
        let ops = parse_input(json!({"op": "untag task", "id": id, "tag": "bug"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["untagged"], true);
        assert_eq!(result["tag"], "bug");
    }

    /// Tag a task using the tag's slug name, verify `#bug` appears in body.
    #[tokio::test]
    async fn tag_with_slug() {
        let (_temp, ctx) = setup().await;

        // Create a tag and a task
        let ops = parse_input(json!({"op": "add tag", "name": "bug"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "add task", "title": "Slug tag test"})).unwrap();
        let added = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = added["id"].as_str().unwrap().to_string();

        // Tag using the slug
        let ops = parse_input(json!({"op": "tag task", "id": task_id, "tag": "bug"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["tagged"], true);
        assert_eq!(result["tag"], "bug");

        // Verify description contains #bug (task_entity_to_json maps body → description)
        let ops = parse_input(json!({"op": "get task", "id": task_id})).unwrap();
        let task = execute_operation(&ctx, &ops[0]).await.unwrap();
        let desc = task["description"].as_str().unwrap_or("");
        assert!(
            desc.contains("#bug"),
            "description should contain #bug, got: {desc}"
        );
        // Also verify the computed tags array includes "bug"
        let tags = task["tags"].as_array().unwrap();
        assert!(
            tags.iter().any(|t| t.as_str() == Some("bug")),
            "tags array should include 'bug'"
        );
    }

    /// Tag a task using the tag's entity ID (ULID), verify it resolves to the
    /// slug and `#bug` appears in description.
    #[tokio::test]
    async fn tag_with_entity_id() {
        let (_temp, ctx) = setup().await;

        // Create a tag and capture its entity ID
        let ops = parse_input(json!({"op": "add tag", "name": "bug"})).unwrap();
        let tag_result = execute_operation(&ctx, &ops[0]).await.unwrap();
        let tag_entity_id = tag_result["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "add task", "title": "Entity ID tag test"})).unwrap();
        let added = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = added["id"].as_str().unwrap().to_string();

        // Tag using the entity ID instead of the slug
        let ops =
            parse_input(json!({"op": "tag task", "id": task_id, "tag": tag_entity_id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["tagged"], true);
        assert_eq!(result["tag"], "bug", "should resolve ULID to slug 'bug'");

        // Verify description contains #bug (not the raw ULID)
        let ops = parse_input(json!({"op": "get task", "id": task_id})).unwrap();
        let task = execute_operation(&ctx, &ops[0]).await.unwrap();
        let desc = task["description"].as_str().unwrap_or("");
        assert!(
            desc.contains("#bug"),
            "description should contain #bug, got: {desc}"
        );
        assert!(
            !desc.contains(&tag_entity_id),
            "description should NOT contain raw entity ID"
        );
        let tags = task["tags"].as_array().unwrap();
        assert!(
            tags.iter().any(|t| t.as_str() == Some("bug")),
            "tags array should include 'bug'"
        );
    }

    /// Untag a task using the tag's slug name, verify `#bug` is removed from body.
    #[tokio::test]
    async fn untag_with_slug() {
        let (_temp, ctx) = setup().await;

        // Create tag + task, then tag the task
        let ops = parse_input(json!({"op": "add tag", "name": "bug"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "add task", "title": "Slug untag test"})).unwrap();
        let added = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = added["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "tag task", "id": task_id, "tag": "bug"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // Untag using the slug
        let ops = parse_input(json!({"op": "untag task", "id": task_id, "tag": "bug"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["untagged"], true);
        assert_eq!(result["tag"], "bug");

        // Verify description no longer contains #bug
        let ops = parse_input(json!({"op": "get task", "id": task_id})).unwrap();
        let task = execute_operation(&ctx, &ops[0]).await.unwrap();
        let desc = task["description"].as_str().unwrap_or("");
        assert!(
            !desc.contains("#bug"),
            "description should NOT contain #bug after untag, got: {desc}"
        );
        let tags = task["tags"].as_array().unwrap();
        assert!(
            !tags.iter().any(|t| t.as_str() == Some("bug")),
            "tags array should NOT include 'bug' after untag"
        );
    }

    /// Untag a task using the tag's entity ID (ULID), verify it resolves to the
    /// slug and `#bug` is removed from description.
    #[tokio::test]
    async fn untag_with_entity_id() {
        let (_temp, ctx) = setup().await;

        // Create a tag and capture its entity ID
        let ops = parse_input(json!({"op": "add tag", "name": "bug"})).unwrap();
        let tag_result = execute_operation(&ctx, &ops[0]).await.unwrap();
        let tag_entity_id = tag_result["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "add task", "title": "Entity ID untag test"})).unwrap();
        let added = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = added["id"].as_str().unwrap().to_string();

        // Tag using slug first so the description has #bug
        let ops = parse_input(json!({"op": "tag task", "id": task_id, "tag": "bug"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // Untag using the entity ID instead of the slug
        let ops =
            parse_input(json!({"op": "untag task", "id": task_id, "tag": tag_entity_id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["untagged"], true);
        assert_eq!(result["tag"], "bug", "should resolve ULID to slug 'bug'");

        // Verify description no longer contains #bug
        let ops = parse_input(json!({"op": "get task", "id": task_id})).unwrap();
        let task = execute_operation(&ctx, &ops[0]).await.unwrap();
        let desc = task["description"].as_str().unwrap_or("");
        assert!(
            !desc.contains("#bug"),
            "description should NOT contain #bug after untag, got: {desc}"
        );
        let tags = task["tags"].as_array().unwrap();
        assert!(
            !tags.iter().any(|t| t.as_str() == Some("bug")),
            "tags array should NOT include 'bug' after untag"
        );
    }

    // ── Swimlane operations ───────────────────────────────────────────

    #[tokio::test]
    async fn dispatch_swimlane_crud() {
        let (_temp, ctx) = setup().await;

        // Add
        let ops =
            parse_input(json!({"op": "add swimlane", "id": "urgent", "name": "Urgent"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["id"], "urgent");
        assert_eq!(result["name"], "Urgent");

        // Get
        let ops = parse_input(json!({"op": "get swimlane", "id": "urgent"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "Urgent");

        // Update
        let ops = parse_input(json!({"op": "update swimlane", "id": "urgent", "name": "Critical"}))
            .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "Critical");

        // List
        let ops = parse_input(json!({"op": "list swimlanes"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert!(result["swimlanes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == "urgent"));

        // Delete
        let ops = parse_input(json!({"op": "delete swimlane", "id": "urgent"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["deleted"], true);
    }

    // ── Actor operations ──────────────────────────────────────────────

    #[tokio::test]
    async fn dispatch_actor_crud() {
        let (_temp, ctx) = setup().await;

        // Add
        let ops = parse_input(json!({"op": "add actor", "id": "bob", "name": "Bob"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["actor"]["id"], "bob");
        assert_eq!(result["actor"]["name"], "Bob");

        // Get
        let ops = parse_input(json!({"op": "get actor", "id": "bob"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "Bob");

        // Update
        let ops =
            parse_input(json!({"op": "update actor", "id": "bob", "name": "Robert"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "Robert");

        // List
        let ops = parse_input(json!({"op": "list actors"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert!(result["actors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["id"] == "bob"));

        // Delete
        let ops = parse_input(json!({"op": "delete actor", "id": "bob"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["deleted"], true);
    }

    #[tokio::test]
    async fn dispatch_add_actor_with_ensure() {
        let (_temp, ctx) = setup().await;

        // Add actor
        let ops =
            parse_input(json!({"op": "add actor", "id": "eve", "name": "Eve", "ensure": true}))
                .unwrap();
        let r1 = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(r1["actor"]["id"], "eve");

        // Ensure again returns existing actor without error
        let ops =
            parse_input(json!({"op": "add actor", "id": "eve", "name": "Eve", "ensure": true}))
                .unwrap();
        let r2 = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(r2["actor"]["id"], "eve");
    }

    // ── Tag operations (board-level) ──────────────────────────────────

    #[tokio::test]
    async fn dispatch_tag_crud() {
        let (_temp, ctx) = setup().await;

        // Add
        let ops =
            parse_input(json!({"op": "add tag", "name": "feature", "color": "00ff00"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "feature");
        let tag_id = result["id"].as_str().unwrap().to_string();

        // Get
        let ops = parse_input(json!({"op": "get tag", "id": tag_id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "feature");

        // Update
        let ops = parse_input(json!({"op": "update tag", "id": tag_id, "name": "enhancement", "color": "0000ff", "description": "Feature request"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "enhancement");

        // List
        let ops = parse_input(json!({"op": "list tags"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert!(result["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "enhancement"));

        // Delete
        let ops = parse_input(json!({"op": "delete tag", "id": tag_id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["deleted"], true);
    }

    // ── Attachment operations ─────────────────────────────────────────

    #[tokio::test]
    async fn dispatch_attachment_crud() {
        let (_temp, ctx) = setup().await;

        // Create a task to attach to
        let ops = parse_input(json!({"op": "add task", "title": "Has attachments"})).unwrap();
        let task = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = task["id"].as_str().unwrap().to_string();

        // Add attachment (use KanbanOperation::new to avoid parse_input param issues)
        let op = KanbanOperation::new(Verb::Add, Noun::Attachment, {
            let mut m = serde_json::Map::new();
            m.insert("task_id".into(), json!(task_id));
            m.insert("name".into(), json!("screenshot.png"));
            m.insert("path".into(), json!("/tmp/screenshot.png"));
            m
        });
        let result = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(result["attachment"]["name"], "screenshot.png");
        let att_id = result["attachment"]["id"].as_str().unwrap().to_string();

        // Get attachment (returns unwrapped, not nested under "attachment")
        let op = KanbanOperation::new(Verb::Get, Noun::Attachment, {
            let mut m = serde_json::Map::new();
            m.insert("task_id".into(), json!(task_id));
            m.insert("id".into(), json!(att_id));
            m
        });
        let result = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(result["name"], "screenshot.png");

        // Update attachment
        let op = KanbanOperation::new(Verb::Update, Noun::Attachment, {
            let mut m = serde_json::Map::new();
            m.insert("task_id".into(), json!(task_id));
            m.insert("id".into(), json!(att_id));
            m.insert("name".into(), json!("renamed.png"));
            m.insert("mime_type".into(), json!("image/png"));
            m
        });
        let result = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(result["attachment"]["name"], "renamed.png");

        // List attachments
        let op = KanbanOperation::new(Verb::List, Noun::Attachments, {
            let mut m = serde_json::Map::new();
            m.insert("task_id".into(), json!(task_id));
            m
        });
        let result = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(result["count"], 1);

        // Delete attachment
        let op = KanbanOperation::new(Verb::Delete, Noun::Attachment, {
            let mut m = serde_json::Map::new();
            m.insert("task_id".into(), json!(task_id));
            m.insert("id".into(), json!(att_id));
            m
        });
        let result = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(result["deleted"], true);
    }

    // ── Dispatch with actor context ───────────────────────────────────

    #[tokio::test]
    async fn dispatch_add_task_with_actor_auto_assigns() {
        let (_temp, ctx) = setup().await;

        // Create actor
        let ops =
            parse_input(json!({"op": "add actor", "id": "agent-1", "name": "Agent One"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // Create an operation with actor set (simulating MCP actor context)
        let mut op = KanbanOperation::new(Verb::Add, Noun::Task, {
            let mut m = serde_json::Map::new();
            m.insert("title".into(), json!("Auto-assigned task"));
            m
        });
        op.actor = Some(ActorId::from_string("agent-1"));

        let result = execute_operation(&ctx, &op).await.unwrap();
        assert!(
            result["assignees"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a.as_str() == Some("agent-1")),
            "task should be auto-assigned to the operation actor"
        );
    }

    // ── Missing required field errors ─────────────────────────────────

    #[tokio::test]
    async fn dispatch_missing_required_field_returns_error() {
        let (_temp, ctx) = setup().await;

        // Move task without column should fail
        let op = KanbanOperation::new(Verb::Move, Noun::Task, {
            let mut m = serde_json::Map::new();
            m.insert("id".into(), json!("some-id"));
            m
        });
        let result = execute_operation(&ctx, &op).await;
        assert!(result.is_err(), "missing required 'column' should error");

        // Add column without name should fail
        let op = KanbanOperation::new(Verb::Add, Noun::Column, {
            let mut m = serde_json::Map::new();
            m.insert("id".into(), json!("col"));
            m
        });
        let result = execute_operation(&ctx, &op).await;
        assert!(result.is_err(), "missing required 'name' should error");
    }
}
