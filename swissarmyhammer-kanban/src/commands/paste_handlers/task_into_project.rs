//! Paste handler: `(task, project)` — paste a task onto a project.
//!
//! Pasting a task onto a project creates a new task whose `project` field
//! is set to the target project id. The new task receives a fresh ULID and
//! all other fields are seeded from the clipboard snapshot.
//!
//! Column placement falls back through two cases:
//! - The clipboard snapshot already carries a `position_column` (the
//!   common case — every task has one) — preserve it so the pasted task
//!   lands in the same workflow stage as the source.
//! - The snapshot has no column information — defer to [`AddEntity`]'s
//!   default placement (lowest-order column), which mirrors how a fresh
//!   `entity.add:task` would be placed.
//!
//! The task ordinal is *always* dropped — even when `position_column` is
//! preserved — so [`AddEntity`]'s position helper appends the new task at
//! the bottom of the destination column rather than colliding with the
//! source's slot. Carrying the source ordinal would either duplicate-place
//! on copy (ordinal already taken by the source) or leave the destination
//! sorting depending on a stale value the source no longer needs after a
//! cut.
//!
//! When the clipboard payload was produced by a `cut` (rather than a
//! `copy`), the source task is deleted after the new task has been
//! successfully written. Create-then-delete ordering is intentional: a
//! failed paste must never destroy the source.
//!
//! This handler matches the `("task", "project")` pair in [`PasteMatrix`].
//! Per the parallel-safety note in the implementing card, registration
//! into the production matrix is deferred to the orchestrator's batch
//! step — only the colocated tests register on a local [`PasteMatrix`].
//!
//! [`PasteMatrix`]: super::PasteMatrix

use super::PasteHandler;
use crate::clipboard::ClipboardPayload;
use crate::commands::run_op;
use crate::context::KanbanContext;
use crate::entity::position::POSITION_COLUMN_FIELD;
use crate::entity::AddEntity;
use crate::task::DeleteTask;
use async_trait::async_trait;
use serde_json::{Map, Value};
use std::collections::HashMap;
use swissarmyhammer_commands::{parse_moniker, CommandContext, CommandError};

/// Override-bag keys that are reserved for ordinal handling and must be
/// dropped from the snapshot before forwarding to [`AddEntity`].
///
/// The destination ordinal is always recomputed (append-at-bottom) because
/// inheriting the source's slot would collide on copy and leave an
/// arbitrary slot on cut. Both the dispatcher-convention name (`ordinal`)
/// and the raw field name (`position_ordinal`) are reserved, since the
/// snapshot stores the entity's full field set under the latter.
const ORDINAL_KEYS_TO_DROP: &[&str] = &["ordinal", "position_ordinal"];

/// `(task, project)` paste handler — see module docs.
pub struct TaskIntoProjectHandler;

#[async_trait]
impl PasteHandler for TaskIntoProjectHandler {
    fn matches(&self) -> (&'static str, &'static str) {
        ("task", "project")
    }

    async fn execute(
        &self,
        clipboard: &ClipboardPayload,
        target: &str,
        ctx: &CommandContext,
    ) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        // Parse the project id off the target moniker. Anything that is
        // not a `project:<id>` moniker is a dispatcher bug — the matrix
        // would not have routed the call here — so surface it loudly.
        let project_id = parse_moniker(target)
            .filter(|(kind, _)| *kind == "project")
            .map(|(_, id)| id.to_string())
            .ok_or_else(|| {
                CommandError::DestinationInvalid(format!(
                    "paste target '{target}' is not a project moniker"
                ))
            })?;

        // Validate the destination project still exists. A project
        // deleted between copy and paste otherwise surfaces only as
        // whatever AddEntity does when it sees an unknown project
        // reference downstream — usually an opaque ExecutionFailed.
        // Surfacing DestinationInvalid here lets the toast name the
        // missing project specifically.
        let ectx = kanban
            .entity_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
        if ectx.read("project", &project_id).await.is_err() {
            return Err(CommandError::DestinationInvalid(format!(
                "Project '{project_id}' no longer exists"
            )));
        }

        // Build the override bag from the clipboard snapshot. Drop any
        // ordinal keys so AddEntity re-derives the ordinal from
        // append-at-end. Project is overridden with the target's id;
        // column is preserved when present in the snapshot (so the pasted
        // task stays in its source workflow stage) and otherwise omitted
        // (so AddEntity falls back to the leftmost column).
        let overrides = build_overrides(&clipboard.swissarmyhammer_clipboard.fields, &project_id);

        // Create the new task.
        let create_op = AddEntity::new("task").with_overrides(overrides);
        let created = run_op(&create_op, &kanban).await?;

        // For cut: delete the source after a successful create. Failure
        // to delete (e.g. the source is already gone) is surfaced as a
        // command error rather than swallowed — the caller asked us to
        // move the task and the move is incomplete. Mirrors the
        // create-then-delete ordering used by sibling task paste handlers.
        if clipboard.swissarmyhammer_clipboard.mode == "cut" {
            let delete_op = DeleteTask::new(clipboard.swissarmyhammer_clipboard.entity_id.as_str());
            run_op(&delete_op, &kanban).await?;
        }

        Ok(created)
    }
}

/// Translate the clipboard's field snapshot into the override map
/// [`AddEntity`] expects.
///
/// - The clipboard `fields` value is expected to be a JSON object (the
///   serialized `Entity::fields` map). A non-object snapshot is treated
///   as an empty bag — paste then degrades to "create a default task in
///   the target project" rather than failing, since there is nothing
///   useful to recover from.
/// - The target `project` is injected as an explicit override, replacing
///   any value carried in the snapshot.
/// - All [`ORDINAL_KEYS_TO_DROP`] are stripped so the snapshot's stale
///   ordinal cannot leak through.
/// - The snapshot's [`POSITION_COLUMN_FIELD`] is promoted to the
///   dispatcher-convention `column` key so [`AddEntity::apply_position`]
///   sees it as an explicit column override and skips the leftmost-column
///   fallback. When the snapshot has no column at all, no `column`
///   override is emitted and [`AddEntity`] picks the leftmost column.
fn build_overrides(snapshot: &Value, project_id: &str) -> HashMap<String, Value> {
    let mut overrides: HashMap<String, Value> = match snapshot {
        Value::Object(map) => filtered_overrides(map),
        _ => HashMap::new(),
    };
    overrides.insert("project".to_string(), Value::String(project_id.to_string()));
    overrides
}

/// Build the override map from a snapshot object, dropping reserved
/// ordinal keys and promoting the snapshot's `position_column` (if any)
/// to a `column` override.
///
/// Promotion is necessary because [`AddEntity`] reads the column override
/// from the dispatcher-convention `column` key, not the raw field name.
/// Forwarding `position_column` directly would let it land in
/// [`AddEntity::apply_overrides`] as a plain field set, bypassing
/// [`AddEntity::apply_position`]'s column-validation step.
fn filtered_overrides(snapshot: &Map<String, Value>) -> HashMap<String, Value> {
    let mut out: HashMap<String, Value> = HashMap::new();
    for (key, value) in snapshot {
        if ORDINAL_KEYS_TO_DROP.contains(&key.as_str()) {
            continue;
        }
        if key == POSITION_COLUMN_FIELD {
            // Promote the snapshot's position_column to the dispatcher's
            // `column` override key. Skip if the snapshot value isn't a
            // string — AddEntity expects a string column id and a malformed
            // snapshot should fall through to the leftmost-column default.
            if let Some(col) = value.as_str() {
                out.insert("column".to_string(), Value::String(col.to_string()));
            }
            continue;
        }
        out.insert(key.clone(), value.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::paste_handlers::test_support::{
        clipboard_payload, list_tasks, make_ctx, matrix_with, setup, snapshot_task,
        task_clipboard_from_fields,
    };
    use crate::project::AddProject;
    use crate::task::AddTask;
    use serde_json::json;
    use std::sync::Arc;
    use swissarmyhammer_operations::Execute;

    /// Create a project entity for paste targets.
    async fn add_project(kanban: &Arc<KanbanContext>, id: &str) -> String {
        let result = AddProject::new(id, id)
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        result["id"].as_str().unwrap().to_string()
    }

    // =========================================================================
    // Local matrix registration — verifies dispatch wiring works in isolation.
    // =========================================================================

    /// The handler must be findable by its declared `(clipboard, target)`
    /// pair on a freshly-built [`PasteMatrix`]. This is the colocated
    /// equivalent of the production `PasteMatrix::find` lookup — it lets
    /// the file be tested without touching `register_paste_handlers()`,
    /// per the parallel-safety note in the implementing card.
    #[test]
    fn local_matrix_finds_task_into_project_handler() {
        let matrix = matrix_with(TaskIntoProjectHandler);
        assert!(
            matrix.find("task", "project").is_some(),
            "matrix should resolve (task, project) to TaskIntoProjectHandler"
        );
        assert!(
            matrix.find("tag", "task").is_none(),
            "matrix should not resolve unrelated pairs"
        );
    }

    #[test]
    fn handler_matches_returns_task_project_pair() {
        assert_eq!(TaskIntoProjectHandler.matches(), ("task", "project"));
    }

    // =========================================================================
    // Behavioral tests
    // =========================================================================

    /// Pasting a copied task onto a project creates a new task with the
    /// project field set to the target project id.
    #[tokio::test]
    async fn paste_task_into_project_sets_project_field() {
        let (_temp, kanban) = setup().await;
        let project_id = add_project(&kanban, "backend").await;

        let add = AddTask::new("Source")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let source_id = add["id"].as_str().unwrap().to_string();

        let payload = snapshot_task(kanban.as_ref(), &source_id, "copy").await;
        let ctx = make_ctx(&kanban);

        let result = TaskIntoProjectHandler
            .execute(&payload, &format!("project:{project_id}"), &ctx)
            .await
            .expect("paste should succeed");

        let new_id = result["id"].as_str().expect("created task must have id");
        assert_ne!(new_id, source_id, "pasted task must have a fresh ULID");
        assert_eq!(
            result["project"], project_id,
            "new task's project field must equal the target project id"
        );
        assert_eq!(
            result["title"], "Source",
            "title from clipboard snapshot must carry over"
        );

        // Source is unchanged: copy is non-destructive.
        let tasks = list_tasks(kanban.as_ref()).await;
        assert_eq!(tasks.len(), 2, "copy must leave source intact");
        assert!(
            tasks.iter().any(|t| t.id == source_id),
            "source task must still exist after copy paste"
        );
    }

    /// When the clipboard snapshot carries a `position_column`, the
    /// pasted task lands in that same column (rather than being shoved
    /// into the leftmost column). This pins the spec's "preserve source
    /// column" requirement.
    ///
    /// The source is moved to `doing` before snapshotting so the
    /// snapshot's `position_column` is unambiguously different from the
    /// default leftmost column (`todo`). If the handler dropped the
    /// snapshot's column, the new task would land in `todo` and this
    /// assertion would fail.
    #[tokio::test]
    async fn paste_task_into_project_preserves_source_column() {
        use crate::task::MoveTask;

        let (_temp, kanban) = setup().await;
        let project_id = add_project(&kanban, "backend").await;

        let add = AddTask::new("Source in doing")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let source_id = add["id"].as_str().unwrap().to_string();

        // Move the source into `doing` so the snapshot's column is
        // distinct from the leftmost column.
        MoveTask::to_column(source_id.clone(), "doing")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();

        let payload = snapshot_task(kanban.as_ref(), &source_id, "copy").await;
        // Sanity: the snapshot really does carry position_column = "doing".
        assert_eq!(
            payload.swissarmyhammer_clipboard.fields["position_column"], "doing",
            "test precondition: snapshot must record source's column"
        );

        let ctx = make_ctx(&kanban);
        let result = TaskIntoProjectHandler
            .execute(&payload, &format!("project:{project_id}"), &ctx)
            .await
            .expect("paste should succeed");

        assert_eq!(
            result["position_column"], "doing",
            "new task must inherit the source's column when snapshot has one"
        );
        assert_eq!(
            result["project"], project_id,
            "project field must still be set on the new task"
        );
    }

    /// A cut clipboard payload (`mode == "cut"`) deletes the source after
    /// the new task is written. Create-before-delete is verified
    /// implicitly: if the create failed first, we'd see 0 tasks remaining.
    #[tokio::test]
    async fn paste_cut_task_into_project_deletes_source() {
        let (_temp, kanban) = setup().await;
        let project_id = add_project(&kanban, "backend").await;

        let add = AddTask::new("Cut me")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let source_id = add["id"].as_str().unwrap().to_string();

        let payload = snapshot_task(kanban.as_ref(), &source_id, "cut").await;
        let ctx = make_ctx(&kanban);

        let result = TaskIntoProjectHandler
            .execute(&payload, &format!("project:{project_id}"), &ctx)
            .await
            .expect("cut paste should succeed");

        let new_id = result["id"].as_str().expect("created task must have id");
        assert_ne!(new_id, source_id, "cut must produce a new ULID");
        assert_eq!(
            result["project"], project_id,
            "cut paste must still set the project field"
        );

        let tasks = list_tasks(kanban.as_ref()).await;
        assert_eq!(
            tasks.len(),
            1,
            "after cut paste only the new task should remain (source deleted)"
        );
        assert_eq!(
            tasks[0].id.as_str(),
            new_id,
            "the surviving task must be the newly-pasted one"
        );
    }

    /// Pasting onto a non-`project:` moniker is a dispatcher contract
    /// violation; the handler should reject it loudly rather than
    /// silently coerce.
    #[tokio::test]
    async fn paste_into_non_project_target_errors() {
        let (_temp, kanban) = setup().await;
        let add = AddTask::new("Source")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let source_id = add["id"].as_str().unwrap().to_string();

        let payload = snapshot_task(kanban.as_ref(), &source_id, "copy").await;
        let ctx = make_ctx(&kanban);

        let result = TaskIntoProjectHandler
            .execute(&payload, "task:01OTHER", &ctx)
            .await;
        assert!(
            result.is_err(),
            "non-project target must produce an error: {result:?}"
        );
    }

    /// When the clipboard snapshot has no `position_column` at all (e.g.
    /// constructed by an older copy path or by hand), the handler must
    /// fall back to AddEntity's default placement (leftmost column)
    /// rather than failing or writing a null column.
    #[tokio::test]
    async fn paste_task_into_project_falls_back_to_leftmost_column() {
        let (_temp, kanban) = setup().await;
        let project_id = add_project(&kanban, "backend").await;

        // Build a snapshot by hand that lacks any column information.
        let payload =
            task_clipboard_from_fields("01OLDSOURCE", json!({"title": "Headless task"}), "copy");

        let ctx = make_ctx(&kanban);
        let result = TaskIntoProjectHandler
            .execute(&payload, &format!("project:{project_id}"), &ctx)
            .await
            .expect("paste should succeed even with no column in snapshot");

        // Default board has todo as the leftmost (order 0) column.
        assert_eq!(
            result["position_column"], "todo",
            "snapshot without column must fall back to leftmost column"
        );
        assert_eq!(result["project"], project_id);
        assert_eq!(result["title"], "Headless task");
    }

    /// Stale ordinal carried by the snapshot must not leak through to
    /// the new task — AddEntity must recompute "after the last task in
    /// the destination column" so the new task has its own slot.
    #[tokio::test]
    async fn paste_task_into_project_recomputes_ordinal() {
        let (_temp, kanban) = setup().await;
        let project_id = add_project(&kanban, "backend").await;

        // Pre-existing task in the destination column to make the
        // recomputed ordinal visibly different from a stale "00".
        AddTask::new("Existing")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();

        // Hand-built snapshot with a deliberately-low ordinal.
        let payload = task_clipboard_from_fields(
            "01SOURCE",
            json!({
                "title": "Pasted",
                "position_column": "todo",
                "position_ordinal": "00",
            }),
            "copy",
        );

        let ctx = make_ctx(&kanban);
        let result = TaskIntoProjectHandler
            .execute(&payload, &format!("project:{project_id}"), &ctx)
            .await
            .expect("paste should succeed");

        let new_ord = result["position_ordinal"]
            .as_str()
            .expect("new task must have an ordinal");
        assert_ne!(
            new_ord, "00",
            "ordinal must be recomputed, not copied from the clipboard snapshot"
        );
    }

    /// Project field on the snapshot must be overridden by the target —
    /// pasting onto project B always sets project=B even if the snapshot
    /// claimed project=A.
    #[tokio::test]
    async fn paste_task_into_project_overrides_snapshot_project() {
        let (_temp, kanban) = setup().await;
        let target_project = add_project(&kanban, "backend").await;
        let _other_project = add_project(&kanban, "frontend").await;

        let payload = task_clipboard_from_fields(
            "01SOURCE",
            json!({
                "title": "Mislabelled",
                "project": "frontend",
                "position_column": "todo",
            }),
            "copy",
        );

        let ctx = make_ctx(&kanban);
        let result = TaskIntoProjectHandler
            .execute(&payload, &format!("project:{target_project}"), &ctx)
            .await
            .expect("paste should succeed");

        assert_eq!(
            result["project"], target_project,
            "target project must override snapshot's stale project value"
        );
    }

    /// Cut-mode transactional safety: when the destination project does
    /// not exist the source task must remain on the board.
    ///
    /// The handler validates the destination project up front and surfaces
    /// `DestinationInvalid` before any AddEntity call. The source
    /// `DeleteTask` only fires after a successful create, so this failure
    /// path must leave the source untouched. Pinned here so a future
    /// refactor that drops the pre-check (or moves the delete before the
    /// create) trips the test.
    #[tokio::test]
    async fn task_into_project_cut_preserves_source_when_create_fails() {
        let (_temp, kanban) = setup().await;
        let add = AddTask::new("Source")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let source_id = add["id"].as_str().unwrap().to_string();

        let payload = snapshot_task(kanban.as_ref(), &source_id, "cut").await;
        let ctx = make_ctx(&kanban);

        // Target a project that was never created on this board.
        let result = TaskIntoProjectHandler
            .execute(&payload, "project:ghost-project", &ctx)
            .await;
        assert!(
            matches!(result, Err(CommandError::DestinationInvalid(_))),
            "cut paste onto missing project must surface DestinationInvalid; got {result:?}"
        );

        // Source must remain. Create-then-delete ordering means a failed
        // destination resolution never touches the source.
        let tasks = list_tasks(kanban.as_ref()).await;
        assert_eq!(
            tasks.len(),
            1,
            "source task must remain when destination project does not exist"
        );
        assert_eq!(
            tasks[0].id, source_id,
            "the surviving task must be the source we tried to cut"
        );
    }

    /// `available()` defaults to `true` — paste availability is gated
    /// upstream by the matrix lookup. This is a regression guard so a
    /// future override does not silently disable all `(task, project)`
    /// pastes.
    #[test]
    fn handler_available_defaults_to_true() {
        let payload = clipboard_payload("task", "01SRC", "copy", json!({}));
        let ctx = CommandContext::new(
            "entity.paste",
            vec![],
            None,
            std::collections::HashMap::new(),
        );
        assert!(
            TaskIntoProjectHandler.available(&payload, "project:backend", &ctx),
            "no availability gate is configured; default must remain true"
        );
    }
}
