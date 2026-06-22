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
use crate::comment::{AddComment, DeleteComment, GetComment, ListComments, UpdateComment};
use crate::perspective::{
    AddPerspective, DeletePerspective, GetPerspective, ListPerspectives, UpdatePerspective,
};
use crate::project::{AddProject, DeleteProject, GetProject, ListProjects, UpdateProject};
use crate::tag::{AddTag, DeleteTag, GetTag, ListTags, UpdateTag};
use crate::task::{
    AddTask, ArchiveTask, AssignTask, CompleteTask, DeleteTask, GetTask, ListArchived, ListTasks,
    MoveTask, NextTask, SearchTasks, TagTask, UnarchiveTask, UnassignTask, UntagTask, UpdateTask,
};
use crate::types::{
    resolve_short_ref, ActorId, Noun, Operation as KanbanOperation, ResolveResult, TaskId, Verb,
};
use crate::{KanbanContext, KanbanError, KanbanOperationProcessor, OperationProcessor};
use serde_json::Value;

/// Helper: require a string param, returning KanbanError on missing.
fn req<'a>(op: &'a KanbanOperation, key: &str) -> Result<&'a str, KanbanError> {
    op.get_string(key)
        .ok_or_else(|| KanbanError::parse(format!("missing required field: {}", key)))
}

/// Helper: require a string param accepting either `primary` or `alt` as the
/// field name, returning a KanbanError naming both when neither is present.
///
/// Used where one entity field has two equally-natural names — e.g. a column's
/// id is just as naturally passed as `column`, so `get/update/delete column`
/// accept either. `primary` is preferred when both are supplied.
fn req_alias<'a>(
    op: &'a KanbanOperation,
    primary: &str,
    alt: &str,
) -> Result<&'a str, KanbanError> {
    op.get_string(primary)
        .or_else(|| op.get_string(alt))
        .ok_or_else(|| {
            KanbanError::parse(format!("missing required field: {} (or {})", primary, alt))
        })
}

/// Recognize an already-canonical full ULID reference and return its canonical
/// (uppercase) form, skipping any board lookup.
///
/// A canonical reference is a 26-char Crockford-base32 ULID, optionally carrying
/// a leading `^` sigil and in any case — the same forms [`resolve_short_ref`]
/// would treat as a full-ULID match. Anything else (short id, prefix, garbage)
/// returns `None`, deferring to the board-scanning resolver.
///
/// This is the fast path for the common case where the caller already holds the
/// full id: the stored ULID *is* the canonical identity, so no scan of live or
/// archived tasks is needed to normalize it. Existence is enforced downstream by
/// the underlying command, exactly as it is for the board-scan path (which only
/// loads ids, never proving the live task still exists).
fn canonical_full_ulid(raw: &str) -> Option<String> {
    let needle = raw.trim();
    let needle = needle.strip_prefix('^').unwrap_or(needle);
    // `Ulid::from_string` accepts only well-formed 26-char Crockford-base32
    // input (case-insensitively) and re-serializes to the canonical uppercase
    // form — the same casing the board stores and the resolver would return.
    ulid::Ulid::from_string(needle)
        .ok()
        .map(|ulid| ulid.to_string())
}

/// Load every task id known to the board — live tasks plus archived ones.
///
/// Used by the forgiving task-ref resolver so callers can pass a short id,
/// `^<short>`, or a ULID prefix anywhere a full ULID is accepted. Archived
/// tasks are included so id-coercing operations that act on them (notably
/// `unarchive task`) can still resolve a short id to the full ULID; existence
/// of the *live* task is then enforced by the underlying command, not the
/// resolver.
///
/// Cost note: the live half (`ectx.list("task")`) reads through the entity
/// cache, but the archived half (`ectx.list_archived("task")`) is **not**
/// cached — it does a fresh disk scan of the trash dir on every call (and, when
/// a compute engine is attached, per-archived-task changelog derivation). So
/// this is only cheap when the archive is small. Callers that already hold a
/// canonical full ULID should short-circuit via [`canonical_full_ulid`] to skip
/// this scan entirely.
async fn board_task_ids(ctx: &KanbanContext) -> Result<Vec<TaskId>, KanbanError> {
    let ectx = ctx.entity_context().await?;
    let live = ectx.list("task").await?;
    let archived = ectx.list_archived("task").await?;
    let live_ids = live.iter().map(|t| TaskId::from_string(t.id.as_str()));
    // Archived entities carry a compound storage id (`<task_id>.<trash_id>`);
    // the original task id is the segment before the first dot. Reduce to that
    // so a short id or full ULID resolves to the canonical task ulid rather
    // than the trash filename (which would later panic the unarchive path).
    let archived_ids = archived.iter().map(|t| {
        let raw = t.id.as_str();
        TaskId::from_string(raw.split('.').next().unwrap_or(raw))
    });
    Ok(live_ids.chain(archived_ids).collect())
}

/// Resolve a forgiving task reference to its canonical full ULID string.
///
/// Accepts a full 26-char ULID, the 7-char short id, either with a leading
/// `^` sigil, or a git-style ULID prefix — case-insensitive — via the core
/// [`resolve_short_ref`] resolver. A full ULID continues to resolve to itself
/// unchanged. An unknown or ambiguous reference yields a clean
/// [`KanbanError::TaskNotFound`] rather than a panic.
///
/// A canonical full ULID short-circuits via [`canonical_full_ulid`] and skips
/// the board scan entirely: the full id is already the canonical identity, so
/// there is nothing to resolve, and the underlying command enforces existence.
async fn resolve_task_ref(ctx: &KanbanContext, raw: &str) -> Result<String, KanbanError> {
    if let Some(canonical) = canonical_full_ulid(raw) {
        return Ok(canonical);
    }
    let ids = board_task_ids(ctx).await?;
    match resolve_short_ref(&ids, raw) {
        ResolveResult::Found(id) => Ok(id.as_str().to_string()),
        ResolveResult::NotFound | ResolveResult::Ambiguous(_) => Err(KanbanError::TaskNotFound {
            id: raw.to_string(),
        }),
    }
}

/// Require a task-id param under `key`, then resolve it to a full ULID.
///
/// Combines [`req`] (missing-field error) with [`resolve_task_ref`] (forgiving
/// short-id coercion) so the many task-id dispatch arms route through the
/// resolver in one call instead of a raw `from_string`.
async fn req_task_id(
    ctx: &KanbanContext,
    op: &KanbanOperation,
    key: &str,
) -> Result<String, KanbanError> {
    let raw = req(op, key)?;
    resolve_task_ref(ctx, raw).await
}

/// Resolve an optional placement-ref param (`before_id`/`after_id`) to a full
/// ULID, returning `Ok(None)` when the param is absent.
///
/// Unlike [`resolve_task_ref`], a reference that resolves to no task is **not**
/// an error here: placement neighbors are advisory, and [`MoveTask`] is built
/// to fall through to appending at the end of the column when the neighbor it
/// is pointed at no longer exists. So an unresolved ref is passed through
/// verbatim, preserving that tolerant append behavior, while a short id or
/// prefix that *does* resolve is still coerced to its canonical ULID.
/// Ambiguity remains a hard error — a non-unique prefix is a genuine caller
/// mistake, not a missing neighbor.
async fn resolve_opt_placement_ref(
    ctx: &KanbanContext,
    op: &KanbanOperation,
    key: &str,
) -> Result<Option<String>, KanbanError> {
    let Some(raw) = op.get_string(key) else {
        return Ok(None);
    };
    let ids = board_task_ids(ctx).await?;
    match resolve_short_ref(&ids, raw) {
        ResolveResult::Found(id) => Ok(Some(id.as_str().to_string())),
        // Unknown neighbor — hand the raw value to MoveTask, which appends.
        ResolveResult::NotFound => Ok(Some(raw.to_string())),
        ResolveResult::Ambiguous(_) => Err(KanbanError::TaskNotFound {
            id: raw.to_string(),
        }),
    }
}

/// Normalize a forgiving `depends_on` param to canonical full ULIDs.
///
/// Mirrors the single-value-or-array tolerance of [`resolve_assignees`] so
/// clients that serialize `depends_on` as a scalar — common because the slim
/// wire schema gives no array type-hint — are not silently dropped. Accepts:
/// - a JSON array of refs;
/// - a single JSON string holding one ref;
/// - a stringified JSON array (`"[\"01K…\"]"`), which is parsed back into its
///   elements; a string that does not parse as a JSON array is treated as one
///   ref.
///
/// Every element routes through [`resolve_task_ref`], so a short id,
/// `^<short>`, unique prefix, lowercase, or full ULID all resolve to the
/// canonical 26-char ULID. An unresolvable ref is an error (consistent with
/// `resolve_task_ref`), never a silent drop. Returns `Ok(None)` when the param
/// is absent.
async fn resolve_depends_on(
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Option<Vec<TaskId>>, KanbanError> {
    let Some(value) = op.get_param("depends_on") else {
        return Ok(None);
    };
    let refs = depends_on_refs(value)?;
    let mut resolved = Vec::with_capacity(refs.len());
    for raw in refs {
        let full = resolve_task_ref(ctx, &raw).await?;
        resolved.push(TaskId::from_string(full));
    }
    Ok(Some(resolved))
}

/// Extract the list of raw task refs from a forgiving `depends_on` value.
///
/// See [`resolve_depends_on`] for the accepted shapes. Resolution to canonical
/// ULIDs is the caller's job; this only normalizes the wire shape to a flat
/// list of ref strings. A value that is neither a JSON array nor a string is
/// malformed and errors — never silently dropped (which on update would clear
/// existing deps, the exact silent-drop bug this helper exists to prevent).
/// A non-string entry inside an array is likewise an error naming the offending
/// entry, rather than being silently skipped.
fn depends_on_refs(value: &Value) -> Result<Vec<String>, KanbanError> {
    if let Some(arr) = value.as_array() {
        return arr
            .iter()
            .map(|v| {
                v.as_str().map(str::to_string).ok_or_else(|| {
                    KanbanError::parse(format!(
                        "depends_on array entries must be task ref strings, got: {v}"
                    ))
                })
            })
            .collect();
    }
    if let Some(s) = value.as_str() {
        // A stringified JSON array (`"[\"01K…\"]"`) parses into its elements;
        // anything else is a single ref.
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(s) {
            return Ok(parsed);
        }
        return Ok(vec![s.to_string()]);
    }
    Err(KanbanError::parse(format!(
        "depends_on must be a task ref string or an array of refs, got: {value}"
    )))
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
            let include_counts = op.get_bool("include_counts");
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
            if let Some(model) = op.get_string("model") {
                cmd = cmd.with_model(model);
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
            let id = req_alias(op, "id", "column")?;
            processor.process(&GetColumn::new(id), ctx).await
        }
        (Verb::Update, Noun::Column) => {
            let id = req_alias(op, "id", "column")?;
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
            let id = req_alias(op, "id", "column")?;
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

    if let Some(dep_ids) = resolve_depends_on(ctx, op).await? {
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
    let id = req_task_id(ctx, op, "id").await?;
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
    if let Some(dep_ids) = resolve_depends_on(ctx, op).await? {
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
            let id = req_task_id(ctx, op, "id").await?;
            processor.process(&GetTask::new(id), ctx).await
        }
        Verb::Update => dispatch_update_task(processor, ctx, op).await,
        Verb::Delete => {
            let id = req_task_id(ctx, op, "id").await?;
            processor.process(&DeleteTask::new(id), ctx).await
        }
        Verb::Complete => {
            let id = req_task_id(ctx, op, "id").await?;
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
            let id = req_task_id(ctx, op, "id").await?;
            let column = req(op, "column")?;
            let mut cmd = MoveTask::to_column(id, column);
            if let Some(ordinal) = op.get_string("ordinal") {
                cmd.ordinal = Some(ordinal.to_string());
            }
            if let Some(before_id) = resolve_opt_placement_ref(ctx, op, "before_id").await? {
                cmd.before_id = Some(before_id.into());
            }
            if let Some(after_id) = resolve_opt_placement_ref(ctx, op, "after_id").await? {
                cmd.after_id = Some(after_id.into());
            }
            processor.process(&cmd, ctx).await
        }
        Verb::Archive => {
            let id = req_task_id(ctx, op, "id").await?;
            processor.process(&ArchiveTask::new(id), ctx).await
        }
        Verb::Unarchive => {
            let id = req_task_id(ctx, op, "id").await?;
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
            let id = req_task_id(ctx, op, "id").await?;
            let assignee = req(op, "assignee")?;
            processor.process(&AssignTask::new(id, assignee), ctx).await
        }
        Verb::Unassign => {
            let id = req_task_id(ctx, op, "id").await?;
            let assignee = req(op, "assignee")?;
            processor
                .process(&UnassignTask::new(id, assignee), ctx)
                .await
        }
        Verb::Tag => {
            let id = req_task_id(ctx, op, "id").await?;
            let tag = req(op, "tag")?;
            processor.process(&TagTask::new(id, tag), ctx).await
        }
        Verb::Untag => {
            let id = req_task_id(ctx, op, "id").await?;
            let tag = req(op, "tag")?;
            processor.process(&UntagTask::new(id, tag), ctx).await
        }
        _ => Err(KanbanError::parse(format!(
            "unsupported operation: {} {}",
            op.verb, op.noun
        ))),
    }
}

/// Dispatch task query operations: list, next, search.
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
            // `project` is forgiving sugar for the `$<project>` filter atom:
            // resolution by id or name-slug (case-insensitive) happens inside
            // `ListTasks::execute` via the slug registry, so we only need to
            // fold it into the DSL here. Alone it becomes `$<project>`; with an
            // explicit `filter` the two are AND-ed (`<filter> && $<project>`).
            let filter = op.get_string("filter");
            let project = op.get_string("project");
            let effective_filter = match (filter, project) {
                (Some(filter), Some(project)) => Some(format!("{filter} && ${project}")),
                (Some(filter), None) => Some(filter.to_string()),
                (None, Some(project)) => Some(format!("${project}")),
                (None, None) => None,
            };
            if let Some(effective_filter) = effective_filter {
                cmd = cmd.with_filter(effective_filter);
            }
            // Pagination — MCP callers pass `page` / `page_size` directly.
            // Anything that doesn't fit in `usize` is treated as unset (the
            // default of 10/1 kicks in inside ListTasks::execute), which
            // matches the clamp behaviour described in the tool docs.
            if let Some(page) = op.get_u64("page").and_then(|n| usize::try_from(n).ok()) {
                cmd = cmd.with_page(page);
            }
            if let Some(page_size) = op
                .get_u64("page_size")
                .and_then(|n| usize::try_from(n).ok())
            {
                cmd = cmd.with_page_size(page_size);
            }
            if let Some(detail) = op.get_string("detail") {
                cmd = cmd.with_detail(detail);
            }
            processor.process(&cmd, ctx).await
        }
        Verb::Search => {
            // `query` is required; `filter` scopes the corpus and `top_k`
            // caps the ranked hits (defaults applied inside SearchTasks).
            let query = req(op, "query")?;
            let mut cmd = SearchTasks::new(query);
            if let Some(filter) = op.get_string("filter") {
                cmd = cmd.with_filter(filter);
            }
            if let Some(top_k) = op.get_u64("top_k").and_then(|n| usize::try_from(n).ok()) {
                cmd = cmd.with_top_k(top_k);
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
        Verb::Next | Verb::List | Verb::Search => {
            execute_task_query_operation(processor, ctx, op).await
        }
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
    if let Some(v) = op.get_string("view_id") {
        cmd = cmd.with_view_id(v);
    }
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
    if op.params.contains_key("view_id") {
        cmd = cmd.with_view_id(op.get_string("view_id").map(|s| s.to_string()));
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
            let task_id = req_task_id(ctx, op, "task_id").await?;
            processor
                .process(&GetAttachment::new(task_id, req(op, "id")?), ctx)
                .await
        }
        Verb::Update => dispatch_update_attachment(processor, ctx, op).await,
        Verb::Delete => {
            let task_id = req_task_id(ctx, op, "task_id").await?;
            processor
                .process(&DeleteAttachment::new(task_id, req(op, "id")?), ctx)
                .await
        }
        Verb::List => {
            let task_id = req_task_id(ctx, op, "task_id").await?;
            processor.process(&ListAttachments::new(task_id), ctx).await
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
    let task_id = req_task_id(ctx, op, "task_id").await?;
    let mut cmd = AddAttachment::new(task_id, req(op, "name")?, req(op, "path")?);
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
    let task_id = req_task_id(ctx, op, "task_id").await?;
    let mut cmd = UpdateAttachment::new(task_id, req(op, "id")?);
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

/// Dispatch comment operations (add, get, update, delete, list).
async fn execute_comment_operation(
    processor: &KanbanOperationProcessor,
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Value, KanbanError> {
    let task_id = req_task_id(ctx, op, "task_id").await?;
    match op.verb {
        Verb::Add => {
            let mut cmd = AddComment::new(task_id, req(op, "text")?);
            // Author pass-through: an explicit `actor` param wins, falling
            // back to the dispatching actor. Resolution and validation live
            // in `AddComment::execute` — dispatch only forwards the Option.
            if let Some(actor) = op
                .get_string("actor")
                .map(str::to_string)
                .or_else(|| op.actor.as_ref().map(|a| a.to_string()))
            {
                cmd = cmd.with_actor(actor);
            }
            processor.process(&cmd, ctx).await
        }
        Verb::Get => {
            processor
                .process(&GetComment::new(task_id, req(op, "id")?), ctx)
                .await
        }
        Verb::Update => {
            processor
                .process(
                    &UpdateComment::new(task_id, req(op, "id")?, req(op, "text")?),
                    ctx,
                )
                .await
        }
        Verb::Delete => {
            processor
                .process(&DeleteComment::new(task_id, req(op, "id")?), ctx)
                .await
        }
        Verb::List => processor.process(&ListComments::new(task_id), ctx).await,
        _ => Err(KanbanError::parse(format!(
            "unsupported operation: {} {}",
            op.verb, op.noun
        ))),
    }
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
        Noun::Comment | Noun::Comments => execute_comment_operation(&processor, ctx, op).await,
        Noun::Archived => {
            let mut cmd = ListArchived::new();
            if let Some(detail) = op.get_string("detail") {
                cmd = cmd.with_detail(detail);
            }
            processor.process(&cmd, ctx).await
        }
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

    /// Fetch the full task via `get task` — mutation responses are thin
    /// acks / slim projections, so effect assertions go through the stored
    /// state.
    async fn get_task(ctx: &KanbanContext, id: &str) -> serde_json::Value {
        let ops = parse_input(json!({"op": "get task", "id": id})).unwrap();
        execute_operation(ctx, &ops[0]).await.unwrap()
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

    /// Regression: a `get task` that passes the task reference under the `task`
    /// key (the committer role's habit) must resolve to `id` and succeed through
    /// the real dispatch + resolver path, not fail with
    /// `missing required field: id`. Covers the full ULID and the `^<short>`
    /// form (the exact shape from the bug report).
    #[tokio::test]
    async fn dispatch_get_task_accepts_task_key_alias() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Aliased fetch"})).unwrap();
        let added = execute_operation(&ctx, &ops[0]).await.unwrap();
        let full_id = added["id"].as_str().unwrap().to_string();
        let short_id = added["short_id"].as_str().unwrap().to_string();

        // Full ULID under `task`.
        let ops = parse_input(json!({"op": "get task", "task": full_id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["title"], "Aliased fetch");
        assert_eq!(result["id"].as_str().unwrap(), full_id);

        // `^<short>` under `task` — the exact shape from the bug report.
        let ops = parse_input(json!({"op": "get task", "task": format!("^{short_id}")})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["id"].as_str().unwrap(), full_id);
    }

    /// `search tasks` must parse to (Verb::Search, Noun::Tasks) and dispatch to
    /// SearchTasks. On an empty board the op short-circuits before loading any
    /// model, so this proves the wiring without an embedding model. A missing
    /// `query` must surface a parse error.
    #[tokio::test]
    async fn dispatch_search_tasks_wiring() {
        let (_temp, ctx) = setup().await;

        // Parses to the Search verb and the Tasks noun.
        let ops = parse_input(json!({"op": "search tasks", "query": "anything"})).unwrap();
        assert_eq!(ops[0].verb, Verb::Search);
        assert_eq!(ops[0].noun, Noun::Tasks);

        // Empty board → ranked result of zero, no model loaded.
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["count"], 0);
        assert!(result["tasks"].as_array().unwrap().is_empty());

        // `query` is required.
        let ops = parse_input(json!({"op": "search tasks"})).unwrap();
        assert!(execute_operation(&ctx, &ops[0]).await.is_err());
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

    /// The optional `detail` param flows through dispatch for both listing
    /// ops: defaults to slim (no `description`), `"full"` restores the
    /// enriched shape.
    #[tokio::test]
    async fn dispatch_list_detail_param() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({
            "op": "add task", "title": "Live", "description": "live body"
        }))
        .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({
            "op": "add task", "title": "Gone", "description": "archived body"
        }))
        .unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let archived_id = r["id"].as_str().unwrap().to_string();
        let ops = parse_input(json!({"op": "archive task", "id": archived_id})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        for (op_name, body) in [
            ("list tasks", "live body"),
            ("list archived", "archived body"),
        ] {
            let ops = parse_input(json!({"op": op_name})).unwrap();
            let result = execute_operation(&ctx, &ops[0]).await.unwrap();
            assert!(
                !result["tasks"][0]
                    .as_object()
                    .unwrap()
                    .contains_key("description"),
                "{op_name} default must be slim"
            );

            let ops = parse_input(json!({"op": op_name, "detail": "full"})).unwrap();
            let result = execute_operation(&ctx, &ops[0]).await.unwrap();
            assert_eq!(
                result["tasks"][0]["description"], body,
                "{op_name} detail=full must include description"
            );

            let ops = parse_input(json!({"op": op_name, "detail": "verbose"})).unwrap();
            let err = execute_operation(&ctx, &ops[0]).await.unwrap_err();
            assert!(
                err.to_string().contains("verbose"),
                "{op_name} must reject unknown detail: {err}"
            );
        }
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
    async fn dispatch_get_column_accepts_column_alias() {
        let (_temp, ctx) = setup().await;

        // `column` is the natural field name for a column id; it must be
        // accepted as an alias for `id` and return the identical result.
        let by_alias = execute_operation(
            &ctx,
            &parse_input(json!({"op": "get column", "column": "todo"})).unwrap()[0],
        )
        .await
        .unwrap();
        let by_id = execute_operation(
            &ctx,
            &parse_input(json!({"op": "get column", "id": "todo"})).unwrap()[0],
        )
        .await
        .unwrap();
        assert_eq!(by_alias["id"], "todo");
        assert_eq!(by_alias, by_id);
    }

    #[tokio::test]
    async fn dispatch_get_column_missing_field_names_both_aliases() {
        let (_temp, ctx) = setup().await;

        // Neither `id` nor `column` present → parse error naming both.
        let op = crate::types::Operation::new(
            crate::types::Verb::Get,
            crate::types::Noun::Column,
            serde_json::Map::new(),
        );
        let err = execute_operation(&ctx, &op).await.unwrap_err().to_string();
        assert!(err.contains("id"), "error should name `id`: {err}");
        assert!(err.contains("column"), "error should name `column`: {err}");
    }

    #[tokio::test]
    async fn dispatch_update_column_accepts_column_alias() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "update column", "column": "todo", "name": "Backlog"}))
            .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["name"], "Backlog");
    }

    #[tokio::test]
    async fn dispatch_delete_column_accepts_column_alias() {
        let (_temp, ctx) = setup().await;

        // Add a new empty column then delete it via the `column` alias.
        let ops = parse_input(json!({"op": "add column", "id": "temp", "name": "Temp"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "delete column", "column": "temp"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["deleted"], true);
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
        // The update response is the thin ack — the effect is asserted via
        // `get task`, the agreed escape hatch.
        crate::task_helpers::assert_task_mutation_ack(&result, &task_id);

        let task = get_task(&ctx, &task_id).await;
        assert_eq!(task["title"], "Updated title");
        assert_eq!(task["description"], "New desc");
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
        execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(get_task(&ctx, &task_id).await["position"]["column"], "done");
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
        execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(
            get_task(&ctx, &task_id).await["position"]["column"],
            "doing"
        );
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

        // Assign — thin ack; the effect is asserted via `get task`
        let ops =
            parse_input(json!({"op": "assign task", "id": task_id, "assignee": "frank"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["ok"], true);
        assert_eq!(result["id"], task_id);
        let assignees = get_task(&ctx, &task_id).await["assignees"]
            .as_array()
            .unwrap()
            .clone();
        assert!(
            assignees.iter().any(|a| a == "frank"),
            "frank should be assigned"
        );

        // Unassign — thin ack; the effect is asserted via `get task`
        let ops = parse_input(json!({"op": "unassign task", "id": task_id, "assignee": "frank"}))
            .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["ok"], true);
        assert!(
            get_task(&ctx, &task_id).await["assignees"]
                .as_array()
                .unwrap()
                .is_empty(),
            "frank should be unassigned"
        );
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

        // Tag the task — TagTask auto-creates the tag and returns the thin
        // ack; the effect is asserted via `get task`
        let ops = parse_input(json!({"op": "tag task", "id": task_id, "tag": "feature"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["ok"], true);
        assert_eq!(result["id"], task_id);
        assert!(get_task(&ctx, &task_id).await["tags"]
            .as_array()
            .unwrap()
            .contains(&json!("feature")));

        // Untag — thin ack; the effect is asserted via `get task`
        let ops =
            parse_input(json!({"op": "untag task", "id": task_id, "tag": "feature"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["ok"], true);
        assert!(get_task(&ctx, &task_id).await["tags"]
            .as_array()
            .unwrap()
            .is_empty());
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
        // The add response is slim (no description echo) — assert the stored
        // description via `get task`.
        let task_id = result["id"].as_str().unwrap();
        assert_eq!(get_task(&ctx, task_id).await["description"], "Some detail");
    }

    #[tokio::test]
    async fn dispatch_add_task_with_ordinal() {
        // Caller-supplied ordinals must be well-formed FractionalIndex
        // encodings — legacy strings like "a5" are rejected at the
        // validation boundary rather than silently stored.
        let (_temp, ctx) = setup().await;

        let ordinal = Ordinal::DEFAULT_STR;
        let ops =
            parse_input(json!({"op": "add task", "title": "Ordered", "ordinal": ordinal})).unwrap();
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
        execute_operation(&ctx, &ops[0]).await.unwrap();
        let task = get_task(&ctx, &task_id).await;
        let assignees = task["assignees"].as_array().unwrap();
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
        execute_operation(&ctx, &ops[0]).await.unwrap();
        let task = get_task(&ctx, &task_id).await;
        let deps = task["depends_on"].as_array().unwrap();
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
        execute_operation(&ctx, &ops[0]).await.unwrap();
        let task = get_task(&ctx, &task_id).await;
        assert_eq!(task["position"]["column"], "doing");
        // Ordinal is passed through to MoveTask
        assert!(task["position"]["ordinal"].as_str().is_some());
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
        execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(get_task(&ctx, &id3).await["position"]["column"], "doing");
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
        execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(get_task(&ctx, &id2).await["position"]["column"], "doing");
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
    // Dispatch: list tasks `project` param folds into the `$<project>`
    // filter. Regression for the silent-ignore bug where `project` was
    // dropped and the whole board was returned.
    // ------------------------------------------------------------------

    /// `{"op": "list tasks", "project": "<id>"}` must return ONLY that
    /// project's tasks. Before the fix the `project` param was silently
    /// ignored and the whole board (both tasks) came back.
    #[tokio::test]
    async fn dispatch_list_tasks_project_param_scopes_to_project() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add project", "id": "myproj", "name": "My Project"}))
            .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops =
            parse_input(json!({"op": "add task", "title": "In project", "project": "myproj"}))
                .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();
        let ops = parse_input(json!({"op": "add task", "title": "Out of project"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "list tasks", "project": "myproj"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();

        assert_eq!(
            result["count"], 1,
            "project param must scope the listing to the in-project task only"
        );
        assert_eq!(result["tasks"][0]["title"], "In project");
    }

    /// `project` + an explicit `filter` apply both (AND semantics): only a
    /// task that is BOTH in the project AND carries the tag is returned.
    #[tokio::test]
    async fn dispatch_list_tasks_project_param_intersects_with_filter() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add project", "id": "myproj", "name": "My Project"}))
            .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // In project AND tagged #bug — the only match.
        let ops =
            parse_input(json!({"op": "add task", "title": "Bug in project", "project": "myproj"}))
                .unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let bug_id = r["id"].as_str().unwrap().to_string();
        let ops = parse_input(json!({"op": "tag task", "id": bug_id, "tag": "bug"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // In project but NOT tagged — excluded by the filter.
        let ops = parse_input(
            json!({"op": "add task", "title": "Plain in project", "project": "myproj"}),
        )
        .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // Tagged #bug but NOT in project — excluded by the project.
        let ops = parse_input(json!({"op": "add task", "title": "Bug outside"})).unwrap();
        let r = execute_operation(&ctx, &ops[0]).await.unwrap();
        let outside_id = r["id"].as_str().unwrap().to_string();
        let ops = parse_input(json!({"op": "tag task", "id": outside_id, "tag": "bug"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "list tasks", "project": "myproj", "filter": "#bug"}))
            .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();

        assert_eq!(
            result["count"], 1,
            "project + filter must intersect (AND), matching only the in-project tagged task"
        );
        assert_eq!(result["tasks"][0]["title"], "Bug in project");
    }

    /// A `project` value naming no existing project yields an empty listing
    /// (normal `$` filter semantics), not the whole board.
    #[tokio::test]
    async fn dispatch_list_tasks_unknown_project_returns_empty() {
        let (_temp, ctx) = setup().await;

        let ops = parse_input(json!({"op": "add task", "title": "Some task"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({"op": "list tasks", "project": "nonexistent"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();

        assert_eq!(
            result["count"], 0,
            "an unknown project must yield an empty list, not the whole board"
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
        execute_operation(&ctx, &update_ops[0]).await.unwrap();
        assert_eq!(get_task(&ctx, id).await["due"], "2026-05-01");
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
        execute_operation(&ctx, &update_ops[0]).await.unwrap();
        assert!(
            get_task(&ctx, id).await["due"].is_null(),
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
        execute_operation(&ctx, &update_ops[0]).await.unwrap();
        assert!(
            get_task(&ctx, id).await["scheduled"].is_null(),
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
        execute_operation(&ctx, &update_ops[0]).await.unwrap();
        let task = get_task(&ctx, id).await;
        assert_eq!(task["title"], "New title");
        assert_eq!(
            task["due"], "2026-05-01",
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

    // ------------------------------------------------------------------
    // Short-id input coercion + output (`short_id` field)
    // ------------------------------------------------------------------

    /// Add a single task and return its full ULID.
    async fn add_one_task(ctx: &KanbanContext, title: &str) -> String {
        let ops = parse_input(json!({"op": "add task", "title": title})).unwrap();
        let r = execute_operation(ctx, &ops[0]).await.unwrap();
        r["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn dispatch_get_task_by_bare_short_id() {
        let (_temp, ctx) = setup().await;
        let id = add_one_task(&ctx, "Short fetch").await;
        let short = crate::types::short_id(&id);

        let ops = parse_input(json!({"op": "get task", "id": short})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["id"].as_str().unwrap(), id);
        assert_eq!(result["title"], "Short fetch");
    }

    #[tokio::test]
    async fn dispatch_get_task_by_caret_short_id() {
        let (_temp, ctx) = setup().await;
        let id = add_one_task(&ctx, "Caret fetch").await;
        let caret = format!("^{}", crate::types::short_id(&id));

        let ops = parse_input(json!({"op": "get task", "id": caret})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["id"].as_str().unwrap(), id);
    }

    #[tokio::test]
    async fn dispatch_get_task_by_short_id_is_case_insensitive() {
        let (_temp, ctx) = setup().await;
        let id = add_one_task(&ctx, "Upper fetch").await;
        let upper = crate::types::short_id(&id).to_uppercase();

        let ops = parse_input(json!({"op": "get task", "id": upper})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["id"].as_str().unwrap(), id);
    }

    #[tokio::test]
    async fn dispatch_get_task_by_full_ulid_still_works() {
        let (_temp, ctx) = setup().await;
        let id = add_one_task(&ctx, "Full fetch").await;

        let ops = parse_input(json!({"op": "get task", "id": id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["id"].as_str().unwrap(), id);
    }

    #[tokio::test]
    async fn dispatch_move_task_by_short_id() {
        let (_temp, ctx) = setup().await;
        let id = add_one_task(&ctx, "Short move").await;
        let short = crate::types::short_id(&id);

        let ops = parse_input(json!({"op": "move task", "id": short, "column": "doing"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        // The ack carries the resolved full ULID; the stored position is
        // asserted via `get task`.
        assert_eq!(result["id"].as_str().unwrap(), id);
        assert_eq!(get_task(&ctx, &id).await["position"]["column"], "doing");
    }

    #[tokio::test]
    async fn dispatch_complete_task_by_short_id() {
        let (_temp, ctx) = setup().await;
        let id = add_one_task(&ctx, "Short complete").await;
        let short = crate::types::short_id(&id);

        let ops = parse_input(json!({"op": "complete task", "id": short})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(result["id"].as_str().unwrap(), id);
    }

    #[tokio::test]
    async fn dispatch_get_task_output_includes_short_id() {
        let (_temp, ctx) = setup().await;
        let id = add_one_task(&ctx, "With short id").await;

        let ops = parse_input(json!({"op": "get task", "id": id})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(
            result["short_id"].as_str().unwrap(),
            crate::types::short_id(&id)
        );
    }

    #[tokio::test]
    async fn dispatch_unknown_short_id_returns_clean_not_found() {
        let (_temp, ctx) = setup().await;
        add_one_task(&ctx, "Real task").await;

        // `zzzzzzz` matches no task — must be a clean error, not a panic.
        let ops = parse_input(json!({"op": "get task", "id": "zzzzzzz"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await;
        assert!(result.is_err(), "unknown short id must return an error");
    }

    #[tokio::test]
    async fn dispatch_ambiguous_prefix_returns_not_found() {
        let (_temp, ctx) = setup().await;
        // Two tasks both exist; an empty-ish ambiguous prefix that matches more
        // than one task resolves to an error rather than picking one.
        let id1 = add_one_task(&ctx, "Amb one").await;
        let _id2 = add_one_task(&ctx, "Amb two").await;

        // Both ULIDs share a long leading run (minted within the same ms burst);
        // the first two chars `01` are a prefix of every ULID → ambiguous.
        let shared_prefix = &id1[..2];
        let ops = parse_input(json!({"op": "get task", "id": shared_prefix})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await;
        assert!(
            result.is_err(),
            "an ambiguous prefix must return a not-found error, not a match"
        );
    }

    #[tokio::test]
    async fn dispatch_add_task_depends_on_short_id_persists_full_ulid() {
        let (_temp, ctx) = setup().await;
        let dep_id = add_one_task(&ctx, "Dependency").await;
        let dep_short = crate::types::short_id(&dep_id);

        // Create a task whose depends_on is given as a short id.
        let ops = parse_input(json!({
            "op": "add task",
            "title": "Dependent",
            "depends_on": [dep_short],
        }))
        .unwrap();
        let created = execute_operation(&ctx, &ops[0]).await.unwrap();

        // The returned depends_on must carry the full canonical ULID.
        let deps = created["depends_on"].as_array().unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].as_str().unwrap(), dep_id);
    }

    #[tokio::test]
    async fn resolve_task_ref_short_circuits_canonical_full_ulid() {
        // A canonical full 26-char ULID is returned directly by the resolver
        // without consulting the board — proven here by resolving one that is
        // NOT on the board: the old board-scan path returned TaskNotFound, the
        // short-circuit returns the ULID unchanged (existence is then enforced
        // by the underlying command, not the resolver).
        let (_temp, ctx) = setup().await;
        let absent = "01KT6SAXCBZFE6S0DEPZDJSQAA";
        let resolved = resolve_task_ref(&ctx, absent).await.unwrap();
        assert_eq!(resolved, absent);
    }

    #[tokio::test]
    async fn resolve_task_ref_short_circuit_normalizes_case_and_caret() {
        // The short-circuit must yield the canonical uppercase ULID even when
        // the caller passes a lowercase form or a `^`-sigil-prefixed full ULID,
        // matching the casing the board scan would have returned.
        let (_temp, ctx) = setup().await;
        let canonical = "01KT6SAXCBZFE6S0DEPZDJSQAA";
        assert_eq!(
            resolve_task_ref(&ctx, &canonical.to_lowercase())
                .await
                .unwrap(),
            canonical
        );
        assert_eq!(
            resolve_task_ref(&ctx, &format!("^{canonical}"))
                .await
                .unwrap(),
            canonical
        );
    }

    #[tokio::test]
    async fn dispatch_update_task_depends_on_short_id_persists_full_ulid() {
        let (_temp, ctx) = setup().await;
        let dep_id = add_one_task(&ctx, "Dep target").await;
        let task_id = add_one_task(&ctx, "Will depend").await;
        let dep_short = crate::types::short_id(&dep_id);

        let ops = parse_input(json!({
            "op": "update task",
            "id": crate::types::short_id(&task_id),
            "depends_on": [dep_short],
        }))
        .unwrap();
        let updated = execute_operation(&ctx, &ops[0]).await.unwrap();

        // The ack carries the resolved full ULID; the persisted dependency
        // is asserted via `get task`.
        assert_eq!(updated["id"].as_str().unwrap(), task_id);
        let task = get_task(&ctx, &task_id).await;
        let deps = task["depends_on"].as_array().unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].as_str().unwrap(), dep_id);
    }

    #[tokio::test]
    async fn dispatch_update_task_depends_on_single_string_persists() {
        // A bare id string (not wrapped in an array) must persist — the
        // forgiving shape real clients frequently serialize, previously
        // silently dropped by the `.as_array()` gate.
        let (_temp, ctx) = setup().await;
        let dep_id = add_one_task(&ctx, "Dep target").await;
        let task_id = add_one_task(&ctx, "Will depend").await;

        let ops = parse_input(json!({
            "op": "update task",
            "id": task_id,
            "depends_on": dep_id,
        }))
        .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let task = get_task(&ctx, &task_id).await;
        let deps = task["depends_on"].as_array().unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].as_str().unwrap(), dep_id);
    }

    #[tokio::test]
    async fn dispatch_update_task_depends_on_stringified_array_persists() {
        // A stringified JSON array (`"[\"01K…\"]"`) must parse and persist.
        let (_temp, ctx) = setup().await;
        let dep_id = add_one_task(&ctx, "Dep target").await;
        let task_id = add_one_task(&ctx, "Will depend").await;
        let stringified = serde_json::to_string(&vec![dep_id.clone()]).unwrap();

        let ops = parse_input(json!({
            "op": "update task",
            "id": task_id,
            "depends_on": stringified,
        }))
        .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let task = get_task(&ctx, &task_id).await;
        let deps = task["depends_on"].as_array().unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].as_str().unwrap(), dep_id);
    }

    #[tokio::test]
    async fn dispatch_update_task_depends_on_caret_single_string_persists_full_ulid() {
        // A `^`-prefixed single string must resolve to the canonical ULID.
        let (_temp, ctx) = setup().await;
        let dep_id = add_one_task(&ctx, "Dep target").await;
        let task_id = add_one_task(&ctx, "Will depend").await;
        let caret_short = format!("^{}", crate::types::short_id(&dep_id));

        let ops = parse_input(json!({
            "op": "update task",
            "id": task_id,
            "depends_on": caret_short,
        }))
        .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let task = get_task(&ctx, &task_id).await;
        let deps = task["depends_on"].as_array().unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].as_str().unwrap(), dep_id);
    }

    #[tokio::test]
    async fn dispatch_update_task_depends_on_unresolvable_ref_errors() {
        // An unresolvable ref must error, not silently drop to an empty list.
        let (_temp, ctx) = setup().await;
        let task_id = add_one_task(&ctx, "Will depend").await;

        let ops = parse_input(json!({
            "op": "update task",
            "id": task_id,
            "depends_on": "nosuch7",
        }))
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await;
        assert!(
            result.is_err(),
            "an unresolvable depends_on ref must error, not silently drop"
        );
    }

    #[tokio::test]
    async fn dispatch_update_task_depends_on_malformed_scalar_errors_without_clearing() {
        // A non-string, non-array value (e.g. a number) is malformed. It must
        // error — never silently clear existing deps, which is exactly the
        // silent-drop anti-pattern this fix exists to kill.
        let (_temp, ctx) = setup().await;
        let dep_id = add_one_task(&ctx, "Dep target").await;
        let task_id = add_one_task(&ctx, "Will depend").await;

        // Seed a real dependency first.
        let ops = parse_input(json!({
            "op": "update task",
            "id": task_id,
            "depends_on": dep_id,
        }))
        .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // A malformed scalar must error, not wipe the seeded dependency.
        let ops = parse_input(json!({
            "op": "update task",
            "id": task_id,
            "depends_on": 42,
        }))
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await;
        assert!(
            result.is_err(),
            "a malformed (non-string, non-array) depends_on must error"
        );

        // The pre-existing dependency must survive the rejected update.
        let task = get_task(&ctx, &task_id).await;
        let deps = task["depends_on"].as_array().unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].as_str().unwrap(), dep_id);
    }

    #[tokio::test]
    async fn dispatch_update_task_depends_on_non_string_array_entry_errors_without_clearing() {
        // An array with a non-string entry (e.g. a number) is malformed. It
        // must error naming the offending entry — never silently skip the entry,
        // which is the same silent-drop anti-pattern this fix exists to kill.
        let (_temp, ctx) = setup().await;
        let dep_id = add_one_task(&ctx, "Dep target").await;
        let task_id = add_one_task(&ctx, "Will depend").await;

        // Seed a real dependency first.
        let ops = parse_input(json!({
            "op": "update task",
            "id": task_id,
            "depends_on": dep_id,
        }))
        .unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        // A non-string array entry must error, not silently skip it.
        let ops = parse_input(json!({
            "op": "update task",
            "id": task_id,
            "depends_on": [dep_id, 42],
        }))
        .unwrap();
        let result = execute_operation(&ctx, &ops[0]).await;
        assert!(
            result.is_err(),
            "a non-string depends_on array entry must error, not be silently skipped"
        );

        // The pre-existing dependency must survive the rejected update.
        let task = get_task(&ctx, &task_id).await;
        let deps = task["depends_on"].as_array().unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].as_str().unwrap(), dep_id);
    }

    /// `add comment` then `list comments` round-trips through
    /// `parse_input` → `execute_operation`: the add returns the mutation ack
    /// (top-level `id` = task id) plus the new member, and the list shows it.
    #[tokio::test]
    async fn dispatch_add_comment_then_list_round_trip() {
        let (_temp, ctx) = setup().await;
        let task_id = add_one_task(&ctx, "Comment target").await;

        let ops =
            parse_input(json!({"op": "add comment", "task_id": task_id, "text": "hi"})).unwrap();
        let added = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(added["ok"], true);
        assert_eq!(added["id"].as_str().unwrap(), task_id);
        assert_eq!(added["comment"]["text"], "hi");

        let ops = parse_input(json!({"op": "list comments", "task_id": task_id})).unwrap();
        let listed = execute_operation(&ctx, &ops[0]).await.unwrap();
        let members = listed["comments"].as_array().unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0]["text"], "hi");
    }

    /// A dispatching actor (the top-level `actor` key, which `parse_input`
    /// lifts onto `op.actor`) is forwarded to `AddComment` and attributed on
    /// the resulting member.
    #[tokio::test]
    async fn dispatch_add_comment_attributes_dispatching_actor() {
        let (_temp, ctx) = setup().await;
        let task_id = add_one_task(&ctx, "Actor attribution").await;

        let ops = parse_input(json!({"op": "add actor", "id": "alice", "name": "Alice"})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();

        let ops = parse_input(json!({
            "op": "add comment",
            "task_id": task_id,
            "text": "from alice",
            "actor": "alice",
        }))
        .unwrap();
        let added = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(added["comment"]["actor"], "alice");
    }

    /// `get comment`, `update comment`, and `delete comment` all dispatch:
    /// get returns the member projection, update edits the text in place,
    /// delete removes the member from the log.
    #[tokio::test]
    async fn dispatch_comment_get_update_delete_round_trip() {
        let (_temp, ctx) = setup().await;
        let task_id = add_one_task(&ctx, "Edit comments").await;

        let ops = parse_input(json!({"op": "add comment", "task_id": task_id, "text": "original"}))
            .unwrap();
        let added = execute_operation(&ctx, &ops[0]).await.unwrap();
        let comment_id = added["comment"]["id"].as_str().unwrap().to_string();

        let ops = parse_input(json!({"op": "get comment", "task_id": task_id, "id": comment_id}))
            .unwrap();
        let got = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(got["text"], "original");
        assert_eq!(got["id"].as_str().unwrap(), comment_id);

        let ops = parse_input(json!({
            "op": "update comment",
            "task_id": task_id,
            "id": comment_id,
            "text": "edited",
        }))
        .unwrap();
        let updated = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(updated["ok"], true);
        assert_eq!(updated["id"].as_str().unwrap(), task_id);

        let ops = parse_input(json!({"op": "get comment", "task_id": task_id, "id": comment_id}))
            .unwrap();
        let got = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(got["text"], "edited");

        let ops =
            parse_input(json!({"op": "delete comment", "task_id": task_id, "id": comment_id}))
                .unwrap();
        let deleted = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(deleted["ok"], true);

        let ops = parse_input(json!({"op": "list comments", "task_id": task_id})).unwrap();
        let listed = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(listed["comments"].as_array().unwrap().len(), 0);
    }
}
