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
use crate::project::{AddProject, DeleteProject, GetProject, ListProjects, UpdateProject};
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

/// Dispatch board operations (init, get, update).
async fn execute_board_operation(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    match op.verb {
        Verb::Init => {
            let name = req(op, "name")?;
            let mut cmd = InitBoard::new(name);
            if let Some(desc) = op.get_string("description") {
                cmd = cmd.with_description(desc);
            }
            processor.process(&cmd, ctx).await
        }
        Verb::Get => {
            let include_counts = op.get_bool("include_counts").unwrap_or(true);
            processor.process(&GetBoard { include_counts }, ctx).await
        }
        Verb::Update => {
            let mut cmd = UpdateBoard::new();
            if let Some(name) = op.get_string("name") {
                cmd = cmd.with_name(name);
            }
            if let Some(desc) = op.get_string("description") {
                cmd = cmd.with_description(desc);
            }
            processor.process(&cmd, ctx).await
        }
        _ => Err(KanbanError::parse(format!(
            "unsupported operation: {} {}",
            op.verb, op.noun
        ))),
    }
}

/// Dispatch column operations (add, get, update, delete, list).
async fn execute_column_operation(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    match (op.verb, op.noun) {
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
        _ => Err(KanbanError::parse(format!(
            "unsupported operation: {} {}",
            op.verb, op.noun
        ))),
    }
}

/// Build and execute an `AddTask` command from operation parameters.
///
/// Parses title (required), description, column, ordinal, assignees, and
/// depends_on from the operation. Assignees fall back to the operation's actor
/// when no explicit assignee list is provided.
/// Resolve assignees from explicit list, single value, or operation actor fallback.
fn resolve_assignees(op: &KanbanOperation) -> Vec<ActorId> {
    let explicit: Vec<ActorId> = op
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

    if explicit.is_empty() {
        op.actor.iter().cloned().collect()
    } else {
        explicit
    }
}

async fn dispatch_add_task(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    let title = req(op, "title")?;
    let mut cmd = AddTask::new(title);
    if let Some(desc) = op.get_string("description") {
        cmd = cmd.with_description(desc);
    }
    if let Some(column) = op.get_string("column") {
        cmd.column = Some(column.to_string());
    }
    if let Some(ordinal) = op.get_string("ordinal") {
        cmd.ordinal = Some(ordinal.to_string());
    }

    let assignees = resolve_assignees(op);
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

    if let Some(project) = op.get_string("project") {
        cmd = cmd.with_project(project);
    }

    // User-set date fields. Empty strings are not supported at create time —
    // they'd be rejected by `AddTask`'s validator, which is the correct
    // behaviour (a create can't "clear" a field that doesn't exist yet).
    //
    // Non-string, non-null JSON values (e.g. `42`, `true`) are coerced to
    // their string form and forwarded so the downstream date parser produces
    // a clear error. Silently dropping them (as `op.get_string` does) would
    // leave the caller with no feedback about a type mismatch.
    if let Some(due) = date_param_to_add(op, "due") {
        cmd = cmd.with_due(due);
    }
    if let Some(scheduled) = date_param_to_add(op, "scheduled") {
        cmd = cmd.with_scheduled(scheduled);
    }

    processor.process(&cmd, ctx).await
}

/// Build and execute an `UpdateTask` command from operation parameters.
///
/// Parses id (required), title, description, assignees, depends_on, and
/// project from the operation.
async fn dispatch_update_task(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
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
    if let Some(project) = op.get_string("project") {
        cmd = cmd.with_project(project);
    }

    // User-set date fields: tri-state.
    //   - param absent  → don't touch (builder already defaults to None).
    //   - JSON null     → clear (`Some(None)`).
    //   - empty string  → clear (same as null).
    //   - date string   → set (validated by `UpdateTask`).
    cmd.due = date_param_to_update(op, "due");
    cmd.scheduled = date_param_to_update(op, "scheduled");

    processor.process(&cmd, ctx).await
}

/// Translate an operation parameter into the tri-state date update form.
///
/// Returns `None` (leave untouched) when the param is absent. Returns
/// `Some(None)` (clear) when the param is present as JSON `null` or an
/// empty/whitespace-only string. Returns `Some(Some(value))` otherwise,
/// deferring date-format validation to `UpdateTask`'s apply layer.
fn date_param_to_update(op: &KanbanOperation, key: &str) -> Option<Option<String>> {
    let value = op.get_param(key)?;
    if value.is_null() {
        return Some(None);
    }
    if let Some(s) = value.as_str() {
        if s.trim().is_empty() {
            return Some(None);
        }
        return Some(Some(s.to_string()));
    }
    // Non-string, non-null values fall through to Some(Some(...)) so that
    // downstream parsing produces a clear error message.
    Some(Some(value.to_string()))
}

/// Translate an operation parameter into an add-task date value.
///
/// `AddTask` has no tri-state — a date is either set or unset. Returns
/// `None` when the param is absent or JSON `null` (treated as "unset" at
/// create time). Returns `Some(raw)` for a string value. Non-string,
/// non-null values (e.g. `42`, `true`) are coerced to their string form
/// and forwarded so the downstream date parser produces a useful error —
/// without this, `op.get_string` would silently drop them and callers
/// would get no feedback that their type was wrong.
fn date_param_to_add(op: &KanbanOperation, key: &str) -> Option<String> {
    let value = op.get_param(key)?;
    if value.is_null() {
        return None;
    }
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    Some(value.to_string())
}

/// Dispatch task CRUD operations: add, get, update, delete, complete.
///
/// Delegates to [`dispatch_add_task`] and [`dispatch_update_task`] for the
/// longer Add and Update arms; handles Get, Delete, and Complete inline.
async fn execute_task_crud_operation(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    match op.verb {
        Verb::Add => dispatch_add_task(processor, ctx, op).await,
        Verb::Get => {
            let id = req(op, "id")?;
            processor.process(&GetTask::new(id), ctx).await
        }
        Verb::Update => dispatch_update_task(processor, ctx, op).await,
        Verb::Delete => {
            let id = req(op, "id")?;
            processor.process(&DeleteTask::new(id), ctx).await
        }
        Verb::Complete => {
            let id = req(op, "id")?;
            processor.process(&CompleteTask::new(id), ctx).await
        }
        _ => Err(KanbanError::parse(format!(
            "unsupported operation: {} {}",
            op.verb, op.noun
        ))),
    }
}

/// Dispatch task movement operations: move, archive, unarchive.
async fn execute_task_movement_operation(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    match op.verb {
        Verb::Move => {
            let id = req(op, "id")?;
            let column = req(op, "column")?;
            let mut cmd = MoveTask::to_column(id, column);
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
        Verb::Archive => {
            let id = req(op, "id")?;
            processor.process(&ArchiveTask::new(id), ctx).await
        }
        Verb::Unarchive => {
            let id = req(op, "id")?;
            processor.process(&UnarchiveTask::new(id), ctx).await
        }
        _ => Err(KanbanError::parse(format!(
            "unsupported operation: {} {}",
            op.verb, op.noun
        ))),
    }
}

/// Dispatch task assignment and tagging operations: assign, unassign, tag, untag.
async fn execute_task_assignment_operation(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    match op.verb {
        Verb::Assign => {
            let id = req(op, "id")?;
            let assignee = req(op, "assignee")?;
            processor.process(&AssignTask::new(id, assignee), ctx).await
        }
        Verb::Unassign => {
            let id = req(op, "id")?;
            let assignee = req(op, "assignee")?;
            processor
                .process(&UnassignTask::new(id, assignee), ctx)
                .await
        }
        Verb::Tag => {
            let id = req(op, "id")?;
            let tag = req(op, "tag")?;
            processor.process(&TagTask::new(id, tag), ctx).await
        }
        Verb::Untag => {
            let id = req(op, "id")?;
            let tag = req(op, "tag")?;
            processor.process(&UntagTask::new(id, tag), ctx).await
        }
        _ => Err(KanbanError::parse(format!(
            "unsupported operation: {} {}",
            op.verb, op.noun
        ))),
    }
}

/// Dispatch task query operations: list, next.
async fn execute_task_query_operation(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    match op.verb {
        Verb::Next => {
            let mut cmd = NextTask::new();
            if let Some(filter) = op.get_string("filter") {
                cmd = cmd.with_filter(filter);
            }
            processor.process(&cmd, ctx).await
        }
        Verb::List => {
            let mut cmd = ListTasks::new();
            if let Some(column) = op.get_string("column") {
                cmd = cmd.with_column(column);
            }
            if let Some(filter) = op.get_string("filter") {
                cmd = cmd.with_filter(filter);
            }
            processor.process(&cmd, ctx).await
        }
        _ => Err(KanbanError::parse(format!(
            "unsupported operation: {} {}",
            op.verb, op.noun
        ))),
    }
}

/// Dispatch task operations by delegating to category-specific handlers.
///
/// Routes each verb to one of: CRUD, movement, assignment/tagging, or query.
async fn execute_task_operation(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    match op.verb {
        Verb::Add | Verb::Get | Verb::Update | Verb::Delete | Verb::Complete => {
            execute_task_crud_operation(processor, ctx, op).await
        }
        Verb::Move | Verb::Archive | Verb::Unarchive => {
            execute_task_movement_operation(processor, ctx, op).await
        }
        Verb::Assign | Verb::Unassign | Verb::Tag | Verb::Untag => {
            execute_task_assignment_operation(processor, ctx, op).await
        }
        Verb::Next | Verb::List => execute_task_query_operation(processor, ctx, op).await,
        _ => Err(KanbanError::parse(format!(
            "unsupported operation: {} {}",
            op.verb, op.noun
        ))),
    }
}

/// Dispatch actor operations (add, get, update, delete, list).
async fn execute_actor_operation(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    match (op.verb, op.noun) {
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
        _ => Err(KanbanError::parse(format!(
            "unsupported operation: {} {}",
            op.verb, op.noun
        ))),
    }
}

/// Dispatch board-level tag operations (add, get, update, delete, list).
async fn execute_tag_operation(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    match (op.verb, op.noun) {
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
        _ => Err(KanbanError::parse(format!(
            "unsupported operation: {} {}",
            op.verb, op.noun
        ))),
    }
}

/// Dispatch project operations (add, get, update, delete, list).
async fn execute_project_operation(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    match (op.verb, op.noun) {
        (Verb::Add, Noun::Project) => dispatch_add_project(processor, ctx, op).await,
        (Verb::Get, Noun::Project) => {
            processor
                .process(&GetProject::new(req(op, "id")?), ctx)
                .await
        }
        (Verb::Update, Noun::Project) => dispatch_update_project(processor, ctx, op).await,
        (Verb::Delete, Noun::Project) => {
            processor
                .process(&DeleteProject::new(req(op, "id")?), ctx)
                .await
        }
        (Verb::List, Noun::Projects) => processor.process(&ListProjects, ctx).await,
        _ => Err(KanbanError::parse(format!(
            "unsupported operation: {} {}",
            op.verb, op.noun
        ))),
    }
}

async fn dispatch_add_project(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    let mut cmd = AddProject::new(req(op, "id")?, req(op, "name")?);
    if let Some(d) = op.get_string("description") {
        cmd = cmd.with_description(d);
    }
    if let Some(c) = op.get_string("color") {
        cmd = cmd.with_color(c);
    }
    processor.process(&cmd, ctx).await
}

async fn dispatch_update_project(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    let mut cmd = UpdateProject::new(req(op, "id")?);
    if let Some(n) = op.get_string("name") {
        cmd = cmd.with_name(n);
    }
    if let Some(d) = op.get_string("description") {
        cmd = cmd.with_description(d);
    }
    if let Some(c) = op.get_string("color") {
        cmd = cmd.with_color(c);
    }
    processor.process(&cmd, ctx).await
}

/// Parse a JSON array param into a `Vec<T>`, returning a `KanbanError` on failure.
fn parse_json_array<T: serde::de::DeserializeOwned>(
    op: &KanbanOperation,
    key: &str,
) -> Result<Option<Vec<T>>, KanbanError> {
    match op.get_param(key) {
        Some(val) => {
            let items = serde_json::from_value(val.clone())
                .map_err(|e| KanbanError::parse(format!("invalid {}: {}", key, e)))?;
            Ok(Some(items))
        }
        None => Ok(None),
    }
}

/// Dispatch perspective operations (add, get, update, delete, list).
async fn execute_perspective_operation(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    match (op.verb, op.noun) {
        (Verb::Add, Noun::Perspective) => dispatch_add_perspective(processor, ctx, op).await,
        (Verb::Get, Noun::Perspective) => {
            processor
                .process(&GetPerspective::new(req(op, "id")?), ctx)
                .await
        }
        (Verb::Update, Noun::Perspective) => dispatch_update_perspective(processor, ctx, op).await,
        (Verb::Delete, Noun::Perspective) => {
            processor
                .process(&DeletePerspective::new(req(op, "id")?), ctx)
                .await
        }
        (Verb::List, Noun::Perspectives) => processor.process(&ListPerspectives::new(), ctx).await,
        _ => Err(KanbanError::parse(format!(
            "unsupported operation: {} {}",
            op.verb, op.noun
        ))),
    }
}

async fn dispatch_add_perspective(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    let mut cmd = AddPerspective::new(req(op, "name")?, req(op, "view")?);
    if let Some(f) = parse_json_array(op, "fields")? {
        cmd = cmd.with_fields(f);
    }
    if let Some(v) = op.get_string("filter") {
        cmd = cmd.with_filter(v);
    }
    if let Some(v) = op.get_string("group") {
        cmd = cmd.with_group(v);
    }
    if let Some(s) = parse_json_array(op, "sort")? {
        cmd = cmd.with_sort(s);
    }
    processor.process(&cmd, ctx).await
}

async fn dispatch_update_perspective(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    let mut cmd = UpdatePerspective::new(req(op, "id")?);
    if let Some(n) = op.get_string("name") {
        cmd = cmd.with_name(n);
    }
    if let Some(v) = op.get_string("view") {
        cmd = cmd.with_view(v);
    }
    if let Some(f) = parse_json_array(op, "fields")? {
        cmd = cmd.with_fields(f);
    }
    if op.params.contains_key("filter") {
        cmd = cmd.with_filter(op.get_string("filter").map(|s| s.to_string()));
    }
    if op.params.contains_key("group") {
        cmd = cmd.with_group(op.get_string("group").map(|s| s.to_string()));
    }
    if let Some(s) = parse_json_array(op, "sort")? {
        cmd = cmd.with_sort(s);
    }
    processor.process(&cmd, ctx).await
}

/// Dispatch attachment operations (add, get, update, delete, list).
async fn execute_attachment_operation(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    match op.verb {
        Verb::Add => dispatch_add_attachment(processor, ctx, op).await,
        Verb::Get => {
            processor
                .process(
                    &GetAttachment::new(req(op, "task_id")?, req(op, "id")?),
                    ctx,
                )
                .await
        }
        Verb::Update => dispatch_update_attachment(processor, ctx, op).await,
        Verb::Delete => {
            processor
                .process(
                    &DeleteAttachment::new(req(op, "task_id")?, req(op, "id")?),
                    ctx,
                )
                .await
        }
        Verb::List => {
            processor
                .process(&ListAttachments::new(req(op, "task_id")?), ctx)
                .await
        }
        _ => Err(KanbanError::parse(format!(
            "unsupported operation: {} {}",
            op.verb, op.noun
        ))),
    }
}

async fn dispatch_add_attachment(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    let mut cmd = AddAttachment::new(req(op, "task_id")?, req(op, "name")?, req(op, "path")?);
    if let Some(mime) = op.get_string("mime_type") {
        cmd = cmd.with_mime_type(mime);
    }
    if let Some(size) = op.get_param("size").and_then(|v| v.as_u64()) {
        cmd = cmd.with_size(size);
    }
    processor.process(&cmd, ctx).await
}

async fn dispatch_update_attachment(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    let mut cmd = UpdateAttachment::new(req(op, "task_id")?, req(op, "id")?);
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

    match op.noun {
        Noun::Board => execute_board_operation(&processor, ctx, op).await,
        Noun::Column | Noun::Columns => execute_column_operation(&processor, ctx, op).await,
        Noun::Task | Noun::Tasks => execute_task_operation(&processor, ctx, op).await,
        Noun::Actor | Noun::Actors => execute_actor_operation(&processor, ctx, op).await,
        Noun::Tag | Noun::Tags => execute_tag_operation(&processor, ctx, op).await,
        Noun::Project | Noun::Projects => execute_project_operation(&processor, ctx, op).await,
        Noun::Perspective | Noun::Perspectives => {
            execute_perspective_operation(&processor, ctx, op).await
        }
        Noun::Attachment | Noun::Attachments => {
            execute_attachment_operation(&processor, ctx, op).await
        }
        Noun::Archived => processor.process(&ListArchived, ctx).await,
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
    use crate::types::Ordinal;
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

    // ── Perspective operations ─────────────────────────────────────

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

    // ------------------------------------------------------------------
    // Dispatch: add task with optional fields
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_add_task_with_description() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(
            json!({"op": "add task", "title": "Described", "description": "Some detail"}),
        )
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["title"], "Described");
        assert_eq!(result["description"], "Some detail");
    }

    #[tokio::test]
    async fn dispatch_add_task_with_ordinal() {
        // Caller-supplied ordinals must be well-formed FractionalIndex
        // encodings — legacy strings like "a5" are rejected at the
        // validation boundary rather than silently stored.
        let (_temp, ctx) = setup().await;

        let ordinal = Ordinal::DEFAULT_STR;
        let ops =
            parse_input(json!({"op": "add task", "title": "Ordered", "ordinal": ordinal}))
                .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["title"], "Ordered");
        assert_eq!(result["position"]["ordinal"], ordinal);
    }

    #[tokio::test]
    async fn dispatch_add_task_with_assignees_array() {
        let (_temp, ctx) = setup().await;

        // Add an actor
        let ops = parse_input(
            json!({"op": "add actor", "id": "alice", "name": "Alice", "type": "human"}),
        )
        .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops =
            parse_input(json!({"op": "add task", "title": "Assigned", "assignees": ["alice"]}))
                .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["title"], "Assigned");
        let assignees = result["assignees"].as_array().unwrap();
        assert!(assignees.iter().any(|a| a == "alice"));
    }

    #[tokio::test]
    async fn dispatch_add_task_with_single_assignee() {
        let (_temp, ctx) = setup().await;

        let ops =
            parse_input(json!({"op": "add actor", "id": "bob", "name": "Bob", "type": "human"}))
                .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops =
            parse_input(json!({"op": "add task", "title": "Single Assignee", "assignee": "bob"}))
                .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["title"], "Single Assignee");
        let assignees = result["assignees"].as_array().unwrap();
        assert!(assignees.iter().any(|a| a == "bob"));
    }

    #[tokio::test]
    async fn dispatch_add_task_with_actor_auto_assigns() {
        let (_temp, ctx) = setup().await;

        // Add actor first
        let ops = parse_input(
            json!({"op": "add actor", "id": "agent", "name": "Agent", "type": "agent"}),
        )
        .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // Provide actor in the operation itself (not in assignees)
        let mut op =
            crate::types::Operation::new(crate::types::Verb::Add, crate::types::Noun::Task, {
                let mut m = serde_json::Map::new();
                m.insert("title".into(), json!("Auto-assigned"));
                m
            });
        op.actor = Some("agent".into());
        let result = execute_operation(&ctx, &op).await.unwrap();
        let assignees = result["assignees"].as_array().unwrap();
        assert!(
            assignees.iter().any(|a| a == "agent"),
            "actor should be auto-assigned when no explicit assignees"
        );
    }

    #[tokio::test]
    async fn dispatch_add_task_with_depends_on() {
        let (_temp, ctx) = setup().await;

        // Add first task
        let ops = parse_input(json!({"op": "add task", "title": "Dep target"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let dep_id = r["id"].as_str().unwrap().to_string();

        // Add task depending on first
        let ops =
            parse_input(json!({"op": "add task", "title": "Dependent", "depends_on": [dep_id]}))
                .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        let deps = result["depends_on"].as_array().unwrap();
        assert!(deps.iter().any(|d| d.as_str() == Some(&dep_id)));
    }

    // ------------------------------------------------------------------
    // Dispatch: update task with optional fields
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_update_task_with_assignees() {
        let (_temp, ctx) = setup().await;

        let ops =
            parse_input(json!({"op": "add actor", "id": "zara", "name": "Zara", "type": "human"}))
                .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "add task", "title": "Reassign"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = r["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "update task", "id": task_id, "assignees": ["zara"]}))
            .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        let assignees = result["assignees"].as_array().unwrap();
        assert!(assignees.iter().any(|a| a == "zara"));
    }

    #[tokio::test]
    async fn dispatch_update_task_with_depends_on() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Target dep"})).unwrap();
        let r1 = execute_operation(&ctx, &ops[0]).await.unwrap();
        let dep_id = r1["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "add task", "title": "Updatable"})).unwrap();
        let r2 = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = r2["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "update task", "id": task_id, "depends_on": [dep_id]}))
            .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        let deps = result["depends_on"].as_array().unwrap();
        assert!(deps.iter().any(|d| d.as_str() == Some(&dep_id)));
    }

    // ------------------------------------------------------------------
    // Dispatch: move task with optional fields
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_move_task_with_ordinal() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Ordinal move"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = r["id"].as_str().unwrap().to_string();

        let ops = parse_input(
            json!({"op": "move task", "id": task_id, "column": "doing", "ordinal": "z9"}),
        )
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["position"]["column"], "doing");
        // Ordinal is passed through to MoveTask
        assert!(result["position"]["ordinal"].as_str().is_some());
    }

    #[tokio::test]
    async fn dispatch_move_task_with_before_id() {
        let (_temp, ctx) = setup().await;

        // Add two tasks in doing column
        let ops =
            parse_input(json!({"op": "add task", "title": "First", "column": "doing"})).unwrap();
        let r1 = execute_operation(&ctx, &ops[0]).await.unwrap();
        let id1 = r1["id"].as_str().unwrap().to_string();

        let ops =
            parse_input(json!({"op": "add task", "title": "Second", "column": "doing"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // Add a task in todo, then move before id1
        let ops = parse_input(json!({"op": "add task", "title": "Mover"})).unwrap();
        let r3 = execute_operation(&ctx, &ops[0]).await.unwrap();
        let id3 = r3["id"].as_str().unwrap().to_string();

        let ops =
            parse_input(json!({"op": "move task", "id": id3, "column": "doing", "before_id": id1}))
                .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["position"]["column"], "doing");
    }

    #[tokio::test]
    async fn dispatch_move_task_with_after_id() {
        let (_temp, ctx) = setup().await;

        let ops =
            parse_input(json!({"op": "add task", "title": "Anchor", "column": "doing"})).unwrap();
        let r1 = execute_operation(&ctx, &ops[0]).await.unwrap();
        let id1 = r1["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "add task", "title": "After mover"})).unwrap();
        let r2 = execute_operation(&ctx, &ops[0]).await.unwrap();
        let id2 = r2["id"].as_str().unwrap().to_string();

        let ops =
            parse_input(json!({"op": "move task", "id": id2, "column": "doing", "after_id": id1}))
                .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["position"]["column"], "doing");
    }

    // ------------------------------------------------------------------
    // Dispatch: next task with filters
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_next_task_with_tag_filter() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Untagged"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "add task", "title": "Tagged task"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = r["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "tag task", "id": task_id, "tag": "priority"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "next task", "filter": "#priority"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["title"], "Tagged task");
    }

    #[tokio::test]
    async fn dispatch_next_task_with_assignee_filter() {
        let (_temp, ctx) = setup().await;

        let ops =
            parse_input(json!({"op": "add actor", "id": "dev", "name": "Dev", "type": "human"}))
                .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "add task", "title": "Assigned next"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = r["id"].as_str().unwrap().to_string();

        let ops =
            parse_input(json!({"op": "assign task", "id": task_id, "assignee": "dev"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "next task", "filter": "@dev"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["title"], "Assigned next");
    }

    // ------------------------------------------------------------------
    // Dispatch: list tasks with all filter types
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_list_tasks_with_tag_filter() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Tagged list"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = r["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "tag task", "id": task_id, "tag": "bug"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "list tasks", "tag": "bug"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["count"], 1);
    }

    #[tokio::test]
    async fn dispatch_list_tasks_with_assignee_filter() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(
            json!({"op": "add actor", "id": "worker", "name": "Worker", "type": "human"}),
        )
        .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "add task", "title": "Worker task"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let task_id = r["id"].as_str().unwrap().to_string();

        let ops =
            parse_input(json!({"op": "assign task", "id": task_id, "assignee": "worker"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "list tasks", "assignee": "worker"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["count"], 1);
    }

    #[tokio::test]
    async fn dispatch_list_tasks_with_ready_filter() {
        let (_temp, ctx) = setup().await;

        // Add a task with a dependency (not ready)
        let ops = parse_input(json!({"op": "add task", "title": "Blocker"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let blocker_id = r["id"].as_str().unwrap().to_string();

        let ops =
            parse_input(json!({"op": "add task", "title": "Blocked", "depends_on": [blocker_id]}))
                .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // List only ready tasks
        let ops = parse_input(json!({"op": "list tasks", "filter": "#READY"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        // Only the blocker should be ready
        let titles: Vec<&str> = result["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["title"].as_str())
            .collect();
        assert!(titles.contains(&"Blocker"), "Blocker should be ready");
        assert!(
            !titles.contains(&"Blocked"),
            "Blocked task should not be ready"
        );
    }

    // ------------------------------------------------------------------
    // Dispatch: column with order
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_add_column_with_order() {
        let (_temp, ctx) = setup().await;

        let ops =
            parse_input(json!({"op": "add column", "id": "review", "name": "Review", "order": 1}))
                .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["id"], "review");
        assert_eq!(result["order"], 1);
    }

    #[tokio::test]
    async fn dispatch_update_column_with_order() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "update column", "id": "todo", "order": 5})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["order"], 5);
    }

    // ------------------------------------------------------------------
    // Dispatch: tag with optional fields
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_add_tag_with_color() {
        let (_temp, ctx) = setup().await;

        let ops =
            parse_input(json!({"op": "add tag", "name": "red-tag", "color": "ff0000"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "red-tag");
        assert_eq!(result["color"], "ff0000");
    }

    #[tokio::test]
    async fn dispatch_add_tag_with_description() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(
            json!({"op": "add tag", "name": "documented", "description": "A documented tag"}),
        )
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "documented");
        assert_eq!(result["description"], "A documented tag");
    }

    #[tokio::test]
    async fn dispatch_add_tag_by_id_field() {
        let (_temp, ctx) = setup().await;

        // The dispatch code also accepts "id" as a fallback for "name"
        let ops = parse_input(json!({"op": "add tag", "id": "id-based-tag"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "id-based-tag");
    }

    #[tokio::test]
    async fn dispatch_update_tag_with_color() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add tag", "name": "colorful"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let tag_id = r["id"].as_str().unwrap().to_string();

        let ops =
            parse_input(json!({"op": "update tag", "id": tag_id, "color": "00ff00"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["color"], "00ff00");
    }

    #[tokio::test]
    async fn dispatch_update_tag_with_description() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add tag", "name": "desc-tag"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let tag_id = r["id"].as_str().unwrap().to_string();

        let ops =
            parse_input(json!({"op": "update tag", "id": tag_id, "description": "Updated desc"}))
                .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["description"], "Updated desc");
    }

    // ------------------------------------------------------------------
    // Dispatch: actor with ensure
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_add_actor_with_ensure() {
        let (_temp, ctx) = setup().await;

        // First add
        let ops = parse_input(
            json!({"op": "add actor", "id": "ensured", "name": "Ensured", "ensure": true}),
        )
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["actor"]["id"], "ensured");
        assert_eq!(result["created"], true);

        // Second add with ensure should not fail
        let ops = parse_input(
            json!({"op": "add actor", "id": "ensured", "name": "Ensured Again", "ensure": true}),
        )
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["actor"]["id"], "ensured");
        assert_eq!(result["created"], false);
    }

    // ------------------------------------------------------------------
    // Dispatch: init board with description
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_init_board_with_description() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        let ops = parse_input(
            json!({"op": "init board", "name": "Described Board", "description": "A nice board"}),
        )
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "Described Board");
        assert_eq!(result["description"], "A nice board");
    }

    // ------------------------------------------------------------------
    // Dispatch: get board with include_counts=false
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_get_board_without_counts() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "get board", "include_counts": false})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "Test");
    }

    // ------------------------------------------------------------------
    // Dispatch: req helper error
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_missing_required_field_returns_error() {
        let (_temp, ctx) = setup().await;

        // get column without id
        let op = crate::types::Operation::new(
            crate::types::Verb::Get,
            crate::types::Noun::Column,
            serde_json::Map::new(),
        );
        let result = execute_operation(&ctx, &op).await;
        assert!(result.is_err(), "should fail without required 'id' field");
    }

    // ------------------------------------------------------------------
    // Dispatch: processor with actor
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_with_actor_sets_processor() {
        let (_temp, ctx) = setup().await;

        let mut op =
            crate::types::Operation::new(crate::types::Verb::Add, crate::types::Noun::Task, {
                let mut m = serde_json::Map::new();
                m.insert("title".into(), json!("Actor task"));
                m
            });
        op.actor = Some("test-actor".into());
        let result = execute_operation(&ctx, &op).await.unwrap();
        assert_eq!(result["title"], "Actor task");
    }

    // -----------------------------------------------------------------------
    // Date field dispatch tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn dispatch_add_task_accepts_due_and_scheduled() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({
            "op": "add task",
            "title": "Dated task",
            "due": "2026-04-30",
            "scheduled": "2026-04-15",
        }))
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();

        assert_eq!(result["due"], "2026-04-30");
        assert_eq!(result["scheduled"], "2026-04-15");
    }

    #[tokio::test]
    async fn dispatch_add_task_rejects_invalid_date() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({
            "op": "add task",
            "title": "Bad date",
            "due": "not-a-date",
        }))
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await;
        assert!(result.is_err(), "invalid due must be rejected");
    }

    #[tokio::test]
    async fn dispatch_add_task_rejects_non_string_date() {
        // Non-string JSON values for `due` must not silently vanish — they need
        // to produce a clear downstream parse error, mirroring the behaviour of
        // `dispatch_update_task`. Otherwise a caller that accidentally sends
        // `42` or `true` would silently get no date set with no feedback.
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({
            "op": "add task",
            "title": "Bad date type",
            "due": 42,
        }))
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await;
        assert!(
            result.is_err(),
            "non-string due must be rejected, got: {result:?}"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.to_lowercase().contains("due"),
            "error should mention the failing field, got: {err}"
        );
    }

    #[tokio::test]
    async fn dispatch_add_task_rejects_non_string_scheduled() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({
            "op": "add task",
            "title": "Bad scheduled type",
            "scheduled": true,
        }))
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await;
        assert!(
            result.is_err(),
            "non-string scheduled must be rejected, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn dispatch_update_task_sets_due() {
        let (_temp, ctx) = setup().await;

        let add_ops = parse_input(json!({"op": "add task", "title": "Set due"})).unwrap();
        let add = execute_operation(&ctx, &add_ops[0]).await.unwrap();
        let id = add["id"].as_str().unwrap();

        let update_ops = parse_input(json!({
            "op": "update task",
            "id": id,
            "due": "2026-05-01",
        }))
        .unwrap();
        let result = execute_operation(&ctx, &update_ops[0]).await.unwrap();
        assert_eq!(result["due"], "2026-05-01");
    }

    #[tokio::test]
    async fn dispatch_update_task_clears_due_with_null() {
        let (_temp, ctx) = setup().await;

        let add_ops = parse_input(json!({
            "op": "add task",
            "title": "Clear me",
            "due": "2026-05-01",
        }))
        .unwrap();
        let add = execute_operation(&ctx, &add_ops[0]).await.unwrap();
        let id = add["id"].as_str().unwrap();
        assert_eq!(add["due"], "2026-05-01");

        let update_ops = parse_input(json!({
            "op": "update task",
            "id": id,
            "due": null,
        }))
        .unwrap();
        let result = execute_operation(&ctx, &update_ops[0]).await.unwrap();
        assert!(
            result["due"].is_null(),
            "due must be null after clearing via null"
        );
    }

    #[tokio::test]
    async fn dispatch_update_task_clears_scheduled_with_empty_string() {
        let (_temp, ctx) = setup().await;

        let add_ops = parse_input(json!({
            "op": "add task",
            "title": "Clear me",
            "scheduled": "2026-05-01",
        }))
        .unwrap();
        let add = execute_operation(&ctx, &add_ops[0]).await.unwrap();
        let id = add["id"].as_str().unwrap();

        let update_ops = parse_input(json!({
            "op": "update task",
            "id": id,
            "scheduled": "",
        }))
        .unwrap();
        let result = execute_operation(&ctx, &update_ops[0]).await.unwrap();
        assert!(
            result["scheduled"].is_null(),
            "scheduled must be null after clearing via empty string"
        );
    }

    #[tokio::test]
    async fn dispatch_update_task_ignores_missing_date_fields() {
        let (_temp, ctx) = setup().await;

        let add_ops = parse_input(json!({
            "op": "add task",
            "title": "Keep my date",
            "due": "2026-05-01",
        }))
        .unwrap();
        let add = execute_operation(&ctx, &add_ops[0]).await.unwrap();
        let id = add["id"].as_str().unwrap();

        // Update a different field; date must be preserved.
        let update_ops = parse_input(json!({
            "op": "update task",
            "id": id,
            "title": "New title",
        }))
        .unwrap();
        let result = execute_operation(&ctx, &update_ops[0]).await.unwrap();
        assert_eq!(result["title"], "New title");
        assert_eq!(
            result["due"], "2026-05-01",
            "missing date param must not touch the field"
        );
    }

    #[tokio::test]
    async fn dispatch_get_task_emits_all_date_fields() {
        let (_temp, ctx) = setup().await;

        let add_ops = parse_input(json!({
            "op": "add task",
            "title": "All dates",
            "due": "2026-05-01",
            "scheduled": "2026-04-15",
        }))
        .unwrap();
        let add = execute_operation(&ctx, &add_ops[0]).await.unwrap();
        let id = add["id"].as_str().unwrap();

        let get_ops = parse_input(json!({"op": "get task", "id": id})).unwrap();
        let result = execute_operation(&ctx, &get_ops[0]).await.unwrap();

        assert_eq!(result["due"], "2026-05-01");
        assert_eq!(result["scheduled"], "2026-04-15");
        // System dates are populated by the changelog-backed derivations.
        assert!(
            result["created"].is_string(),
            "created must be populated after write"
        );
        assert!(
            result["updated"].is_string(),
            "updated must be populated after write"
        );
    }

    #[tokio::test]
    async fn dispatch_list_tasks_emits_date_fields() {
        let (_temp, ctx) = setup().await;

        let add_ops = parse_input(json!({
            "op": "add task",
            "title": "In list",
            "due": "2026-05-01",
        }))
        .unwrap();
        execute_operation(&ctx, &add_ops[0]).await.unwrap();

        let list_ops = parse_input(json!({"op": "list tasks"})).unwrap();
        let result = execute_operation(&ctx, &list_ops[0]).await.unwrap();

        let tasks = result["tasks"].as_array().expect("tasks array");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["due"], "2026-05-01");
        assert!(tasks[0]["scheduled"].is_null());
        assert!(tasks[0].get("created").is_some());
    }
}
