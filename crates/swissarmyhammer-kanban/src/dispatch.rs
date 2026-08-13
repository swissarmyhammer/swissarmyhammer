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

/// Normalize a forgiving list-of-refs param value to a flat list of ref strings.
///
/// Every collection param on the task ops (`depends_on`, `tags`, `assignees`,
/// `attachments`) shares this shape tolerance, because the slim wire schema
/// gives clients no array type-hint and they routinely send a scalar. Accepts:
/// - a JSON array of strings;
/// - a single JSON string holding one ref;
/// - a stringified JSON array (`"[\"01K…\"]"`), which is parsed back into its
///   elements; a string that does not parse as a JSON array of strings is
///   treated as one ref.
///
/// Anything else — a number, a bool, an object, or an array holding a
/// non-string — is malformed and errors. It is never silently dropped, because
/// on an update a dropped param reads as "no change" and the caller has no way
/// to tell its input was thrown away. `field` names the param in the error.
fn ref_list(field: &str, value: &Value) -> Result<Vec<String>, KanbanError> {
    let malformed = || {
        KanbanError::parse(format!(
            "{field} must be a ref string or an array of ref strings, got: {value}"
        ))
    };
    if let Some(arr) = value.as_array() {
        return arr
            .iter()
            .map(|v| v.as_str().map(str::to_string).ok_or_else(malformed))
            .collect();
    }
    if let Some(s) = value.as_str() {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(s) {
            return Ok(parsed);
        }
        return Ok(vec![s.to_string()]);
    }
    Err(malformed())
}

/// Read a forgiving list-of-refs param, returning `Ok(None)` when it is absent.
///
/// An explicit empty array yields `Some(vec![])` — the difference between
/// "leave alone" and "clear" that replace-style params depend on.
fn list_param(op: &KanbanOperation, key: &str) -> Result<Option<Vec<String>>, KanbanError> {
    match op.get_param(key) {
        Some(value) => Ok(Some(ref_list(key, value)?)),
        None => Ok(None),
    }
}

/// Normalize a forgiving `depends_on` param to canonical full ULIDs.
///
/// Shape tolerance comes from [`ref_list`]. Every element then routes through
/// [`resolve_task_ref`], so a short id, `^<short>`, unique prefix, lowercase, or
/// full ULID all resolve to the canonical 26-char ULID. An unresolvable ref is
/// an error, never a silent drop. Returns `Ok(None)` when the param is absent.
async fn resolve_depends_on(
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Option<Vec<TaskId>>, KanbanError> {
    let Some(refs) = list_param(op, "depends_on")? else {
        return Ok(None);
    };
    let mut resolved = Vec::with_capacity(refs.len());
    for raw in refs {
        let full = resolve_task_ref(ctx, &raw).await?;
        resolved.push(TaskId::from_string(full));
    }
    Ok(Some(resolved))
}

/// Read the forgiving `tags` param as a list of raw tag refs.
///
/// Resolution (name, full ULID, `^<short>`, short id) happens inside the
/// commands, in the one shared path `tag task` also uses — the dispatch layer
/// only normalizes the wire shape. The singular `tag` is accepted as a
/// one-element alias, because that is the key `tag task` teaches; this arm is
/// only reached for `add`/`update`, so `tag task`'s own `tag` param is
/// untouched. Returns `Ok(None)` when neither key is present.
fn tag_refs(op: &KanbanOperation) -> Result<Option<Vec<String>>, KanbanError> {
    if let Some(refs) = list_param(op, "tags")? {
        return Ok(Some(refs));
    }
    list_param(op, "tag")
}

/// Read the forgiving `attachments` param.
///
/// The entity layer accepts two element shapes for an attachment field: a
/// source path string to copy in, and the enriched `{id, name, …}` object that
/// `get task` hands back. Both are accepted here, and they may be mixed, so a
/// client can read a task, add one new file path, and send the whole list back.
/// A scalar string and a stringified array follow [`ref_list`].
///
/// An object the entity layer could not resolve is rejected rather than passed
/// on: it would resolve to nothing and vanish from the stored list, wiping the
/// task's attachments while the caller is told the update succeeded. Returns
/// `Ok(None)` when the param is absent.
fn attachment_param(op: &KanbanOperation) -> Result<Option<Value>, KanbanError> {
    let Some(value) = op.get_param("attachments") else {
        return Ok(None);
    };
    if let Some(arr) = value.as_array() {
        if arr.iter().any(Value::is_object) {
            let elements = arr
                .iter()
                .map(attachment_element)
                .collect::<Result<Vec<Value>, KanbanError>>()?;
            return Ok(Some(Value::Array(elements)));
        }
    }
    Ok(Some(serde_json::json!(ref_list("attachments", value)?)))
}

/// Validate one element of an `attachments` array.
///
/// A string is a source path, taken as-is. An object must carry string `id` and
/// `name` — the pair the entity layer rebuilds the stored filename from. Any
/// other shape errors, naming what was expected.
fn attachment_element(value: &Value) -> Result<Value, KanbanError> {
    let resolvable = value.is_string()
        || value.as_object().is_some_and(|obj| {
            ["id", "name"]
                .iter()
                .all(|key| obj.get(*key).and_then(Value::as_str).is_some())
        });
    if resolvable {
        return Ok(value.clone());
    }
    Err(KanbanError::parse(format!(
        "attachments entries must be a source path string or an attachment object \
         carrying string `id` and `name`, got: {value}"
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

/// Read the explicit assignee refs from an operation, before resolution.
///
/// `assignees` takes the same forgiving shapes as every other ref-list param
/// (see [`ref_list`]). The singular `assignee` is an alias read through the
/// same [`list_param`] path, so it accepts every shape the plural key does —
/// the alias names the key, it does not narrow the shape. Returns `Ok(None)`
/// only when neither key is present — an explicit empty array is
/// `Some(vec![])`, which `update task` uses to unassign.
fn explicit_assignee_refs(op: &KanbanOperation) -> Result<Option<Vec<String>>, KanbanError> {
    if let Some(refs) = list_param(op, "assignees")? {
        return Ok(Some(refs));
    }
    list_param(op, "assignee")
}

/// Normalize the explicit `assignees` param to registered actor ids.
///
/// Shape tolerance comes from [`explicit_assignee_refs`]. Every element is then
/// checked against the board's actors, and an id that names no actor is a
/// [`KanbanError::ActorNotFound`], never a silent drop — the same contract
/// [`resolve_depends_on`] gives an unknown task ref, and the one `assign task`
/// has always enforced. The whole list is resolved before any of it is applied,
/// so one bad ref rejects the write instead of applying part of it.
///
/// Unlike a task ref, an actor id is a caller-chosen slug rather than a ULID,
/// so there is no short form, prefix, or `^` sigil to expand: resolution is an
/// exact-id existence check.
///
/// Without this check the fields layer prunes the dangling id on write and the
/// caller is acked with `ok` while the assignment is thrown away.
///
/// Returns `Ok(None)` when neither `assignees` nor `assignee` is present.
async fn resolve_explicit_assignees(
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Option<Vec<ActorId>>, KanbanError> {
    let Some(refs) = explicit_assignee_refs(op)? else {
        return Ok(None);
    };
    let ectx = ctx.entity_context().await?;
    let mut resolved = Vec::with_capacity(refs.len());
    for raw in refs {
        ectx.read("actor", &raw)
            .await
            .map_err(KanbanError::from_entity_error)?;
        resolved.push(ActorId::from_string(raw));
    }
    Ok(Some(resolved))
}

/// Assignees for a new task: the explicit list, falling back to the operation's
/// actor when no assignee was named.
///
/// The fallback never errors. `actor` names the caller for attribution, not an
/// assignment the caller asked for, so an unregistered caller still creates the
/// task — the fallback is skipped instead. Skipping it, rather than passing the
/// unknown id on, keeps the create's echoed `assignees` equal to what is
/// stored: [`AddTask`] echoes the entity it built, while the fields layer
/// prunes an unregistered id during the write, so passing it on would ack an
/// assignee the board does not hold.
async fn resolve_assignees(
    ctx: &KanbanContext,
    op: &KanbanOperation,
) -> Result<Vec<ActorId>, KanbanError> {
    let explicit = resolve_explicit_assignees(ctx, op)
        .await?
        .unwrap_or_default();
    if !explicit.is_empty() {
        return Ok(explicit);
    }
    let Some(actor) = op.actor.as_ref() else {
        return Ok(Vec::new());
    };
    let ectx = ctx.entity_context().await?;
    if ectx.read("actor", actor.as_str()).await.is_err() {
        return Ok(Vec::new());
    }
    Ok(vec![actor.clone()])
}

/// Build and execute an `AddTask` command from operation parameters.
///
/// Parses title (required), description, column, ordinal, assignees,
/// depends_on, tags, project, and the user-set dates from the operation.
/// Assignees fall back to the operation's actor when no explicit assignee
/// list is provided.
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

    let assignees = resolve_assignees(ctx, op).await?;
    if !assignees.is_empty() {
        cmd = cmd.with_assignees(assignees);
    }

    if let Some(dep_ids) = resolve_depends_on(ctx, op).await? {
        if !dep_ids.is_empty() {
            cmd = cmd.with_depends_on(dep_ids);
        }
    }

    if let Some(refs) = tag_refs(op)? {
        cmd = cmd.with_tags(refs);
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
/// Parses id (required), title, description, assignees, depends_on, tags,
/// attachments, and project from the operation.
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
    // A present-but-empty list is a clear, not a no-op — the caller asked for
    // "no assignees" and must get it.
    if let Some(ids) = resolve_explicit_assignees(ctx, op).await? {
        cmd = cmd.with_assignees(ids);
    }
    if let Some(dep_ids) = resolve_depends_on(ctx, op).await? {
        cmd = cmd.with_depends_on(dep_ids);
    }
    if let Some(refs) = tag_refs(op)? {
        cmd = cmd.with_tags(refs);
    }
    if let Some(attachments) = attachment_param(op)? {
        cmd = cmd.with_attachments(attachments);
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
mod tests;
