//! Paste handler: `(task, column)` — paste a task onto a column.
//!
//! Pasting a task onto a column creates a new task in that column whose
//! field values are seeded from the clipboard snapshot. The new task
//! receives a fresh ULID and is appended at the end of the target column
//! (the dispatcher's `position::resolve_ordinal` helper handles the
//! ordinal computation when no explicit `ordinal` override is supplied).
//!
//! When the clipboard payload was produced by a `cut` (rather than a
//! `copy`), the source task is deleted after the new task has been
//! successfully written. A failed create therefore leaves the source
//! task untouched — cut is "create-then-delete", never "delete-then-create".
//!
//! This handler matches the `("task", "column")` pair in [`PasteMatrix`].
//! The colocated tests below exercise it in isolation by registering it
//! on a local matrix; production registration is done by
//! [`super::register_paste_handlers`] once all sibling handler files
//! have landed.

use super::PasteHandler;
use crate::clipboard::ClipboardPayload;
use crate::commands::run_op;
use crate::context::KanbanContext;
use crate::entity::AddEntity;
use crate::task::DeleteTask;
use async_trait::async_trait;
use serde_json::{Map, Value};
use std::collections::HashMap;
use swissarmyhammer_commands::{parse_moniker, CommandContext, CommandError};

/// Reserved positional override keys that must be re-derived per paste.
///
/// `column` comes from the target moniker — the clipboard's recorded
/// column is irrelevant. `ordinal` is dropped so [`AddEntity`]'s position
/// helper appends the new task at the end of the target column rather
/// than inheriting the source's stale ordinal (which, in the destination
/// column, would either collide with an existing task or land in an
/// arbitrary slot).
///
/// The raw field-name forms (`position_column`, `position_ordinal`) are
/// included as well, since clipboard snapshots store the entity's full
/// field set under those names — leaving them in would let the snapshot
/// bypass the override-bag's positional logic and write the source's
/// position straight back onto the new entity.
const POSITION_KEYS_TO_DROP: &[&str] =
    &["column", "ordinal", "position_column", "position_ordinal"];

/// `(task, column)` paste handler — see module docs.
pub struct TaskIntoColumnHandler;

#[async_trait]
impl PasteHandler for TaskIntoColumnHandler {
    fn matches(&self) -> (&'static str, &'static str) {
        ("task", "column")
    }

    async fn execute(
        &self,
        clipboard: &ClipboardPayload,
        target: &str,
        ctx: &CommandContext,
    ) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        // Parse the column id off the target moniker. Anything that
        // isn't a `column:<id>` moniker is a dispatcher bug — the matrix
        // would not have routed the call here — so surface it loudly.
        let column = parse_moniker(target)
            .filter(|(kind, _)| *kind == "column")
            .map(|(_, id)| id.to_string())
            .ok_or_else(|| {
                CommandError::DestinationInvalid(format!(
                    "paste target '{target}' is not a column moniker"
                ))
            })?;

        // Validate the column still exists on the board before we touch
        // anything. Without this guard the inner AddEntity returns an
        // ExecutionFailed wrapping the entity-layer's "column 'X' does
        // not exist" message, which the toast renders generically.
        // Surfacing DestinationInvalid here lets the UI report the
        // specific "Column 'X' no longer exists" failure.
        let ectx = kanban
            .entity_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
        if ectx.read("column", &column).await.is_err() {
            return Err(CommandError::DestinationInvalid(format!(
                "Column '{column}' no longer exists"
            )));
        }

        // Build the override bag from the clipboard snapshot. Drop any
        // reserved positional keys so AddEntity re-derives column from
        // our explicit override and ordinal from append-at-end.
        let overrides = build_overrides(&clipboard.swissarmyhammer_clipboard.fields, &column);

        // Create the new task.
        let create_op = AddEntity::new("task").with_overrides(overrides);
        let created = run_op(&create_op, &kanban).await?;

        // For cut: delete the source after a successful create. Failure
        // to delete (e.g. the source is already gone) is surfaced as a
        // command error rather than swallowed — the caller asked us to
        // move the task and the move is incomplete.
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
///   the target column" rather than failing, since there is nothing
///   useful to recover from.
/// - The target `column` is injected as an explicit override, replacing
///   any value carried in the snapshot.
/// - All [`POSITION_KEYS_TO_DROP`] are stripped before injecting the
///   column so the snapshot's stale position cannot leak through.
fn build_overrides(snapshot: &Value, column: &str) -> HashMap<String, Value> {
    let mut overrides: HashMap<String, Value> = match snapshot {
        Value::Object(map) => filtered_overrides(map),
        _ => HashMap::new(),
    };
    overrides.insert("column".to_string(), Value::String(column.to_string()));
    overrides
}

/// Build the override map from a snapshot object, dropping reserved
/// positional keys so the target column / append-at-end semantics win.
fn filtered_overrides(snapshot: &Map<String, Value>) -> HashMap<String, Value> {
    snapshot
        .iter()
        .filter(|(k, _)| !POSITION_KEYS_TO_DROP.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::clipboard::{
        deserialize_from_clipboard, serialize_to_clipboard, ClipboardData, ClipboardPayload,
    };
    use crate::commands::paste_handlers::PasteMatrix;
    use crate::task::AddTask;
    use crate::types::TaskId;
    use serde_json::json;
    use std::sync::Arc;
    use swissarmyhammer_operations::Execute;

    /// Build a fresh `KanbanContext` on a tempdir with the default board
    /// columns (todo / doing / done) seeded.
    async fn setup() -> (tempfile::TempDir, Arc<KanbanContext>) {
        let temp = tempfile::TempDir::new().unwrap();
        let kanban = Arc::new(KanbanContext::new(temp.path().join(".kanban")));
        InitBoard::new("Test")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        (temp, kanban)
    }

    /// Produce a `CommandContext` carrying the supplied `KanbanContext`
    /// extension. The scope chain is empty — `target` carries the
    /// pasted-onto moniker directly, mirroring how `PasteEntityCmd`
    /// invokes a handler.
    fn make_ctx(kanban: &Arc<KanbanContext>) -> CommandContext {
        let mut ctx = CommandContext::new(
            "entity.paste",
            vec![],
            None,
            std::collections::HashMap::new(),
        );
        ctx.set_extension(Arc::clone(kanban));
        ctx
    }

    /// Snapshot a task's fields into a `ClipboardPayload` with the given
    /// `mode` ("copy" or "cut").
    async fn snapshot_task(kanban: &KanbanContext, task_id: &str, mode: &str) -> ClipboardPayload {
        let ectx = kanban.entity_context().await.unwrap();
        let entity = ectx.read("task", task_id).await.unwrap();
        let fields = serde_json::to_value(&entity.fields).unwrap();
        let json = serialize_to_clipboard("task", task_id, mode, fields);
        deserialize_from_clipboard(&json).expect("snapshot must round-trip")
    }

    /// Read every task currently on the board.
    async fn list_tasks(kanban: &KanbanContext) -> Vec<swissarmyhammer_entity::Entity> {
        kanban
            .entity_context()
            .await
            .unwrap()
            .list("task")
            .await
            .unwrap()
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
    fn local_matrix_finds_task_into_column_handler() {
        let mut matrix = PasteMatrix::default();
        matrix.register(TaskIntoColumnHandler);
        assert!(
            matrix.find("task", "column").is_some(),
            "matrix should resolve (task, column) to TaskIntoColumnHandler"
        );
        assert!(
            matrix.find("tag", "task").is_none(),
            "matrix should not resolve unrelated pairs"
        );
    }

    #[test]
    fn handler_matches_returns_task_column_pair() {
        assert_eq!(TaskIntoColumnHandler.matches(), ("task", "column"));
    }

    // =========================================================================
    // Behavioral tests
    // =========================================================================

    /// Pasting a copied task into a different column creates a new task
    /// in that column with a fresh ID and the source's title carried
    /// over.
    #[tokio::test]
    async fn paste_task_into_column_creates_copy() {
        let (_temp, kanban) = setup().await;
        let add = AddTask::new("Source")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let source_id = add["id"].as_str().unwrap().to_string();

        let payload = snapshot_task(kanban.as_ref(), &source_id, "copy").await;
        let ctx = make_ctx(&kanban);

        let result = TaskIntoColumnHandler
            .execute(&payload, "column:doing", &ctx)
            .await
            .expect("paste should succeed");

        // A new task — distinct id — lands in the target column.
        let new_id = result["id"].as_str().expect("created task must have id");
        assert_ne!(new_id, source_id, "pasted task must have a fresh ULID");
        assert_eq!(result["position_column"], "doing");
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

    /// Tags, assignees, and project carried in the clipboard snapshot
    /// must be applied to the new task. Position fields, by contrast,
    /// must come from the target column rather than the snapshot — this
    /// test pins both invariants in one pass.
    ///
    /// The snapshot is constructed directly (rather than going through
    /// `AddTask`) so the test pins the *handler's* contract: whatever
    /// fields ride in the clipboard payload must be carried into the
    /// new entity. Coupling this test to `AddTask`'s field-acceptance
    /// rules (e.g. validating that an assignee actor exists at the
    /// time of creation) would muddy the assertion — the handler is
    /// supposed to be agnostic to upstream provenance.
    #[tokio::test]
    async fn paste_task_into_column_preserves_fields() {
        let (_temp, kanban) = setup().await;

        // Synthesize a clipboard payload as if `entity.copy` had snapshotted
        // a task that carried these fields. Position fields are deliberately
        // included to verify the handler's drop logic.
        let snapshot_fields = json!({
            "title": "Rich source",
            "body": "Reproduce #bug",
            "assignees": ["alice"],
            "project": "proj-x",
            "tags": ["bug"],
            // Stale position from the source — must be ignored.
            "position_column": "todo",
            "position_ordinal": "80",
        });
        let clipboard_json = serialize_to_clipboard("task", "01OLDSOURCE", "copy", snapshot_fields);
        let payload = deserialize_from_clipboard(&clipboard_json).unwrap();
        let ctx = make_ctx(&kanban);

        let result = TaskIntoColumnHandler
            .execute(&payload, "column:doing", &ctx)
            .await
            .expect("paste should succeed");

        // Snapshot fields carried over.
        assert_eq!(result["title"], "Rich source");
        assert!(
            result["body"].as_str().unwrap_or("").contains("#bug"),
            "body (with embedded #bug tag) must carry over: got {:?}",
            result["body"]
        );
        let assignees = result["assignees"]
            .as_array()
            .expect("assignees must be an array on the new task");
        assert!(
            assignees.iter().any(|v| v == &json!("alice")),
            "assignees must include alice: {assignees:?}"
        );
        assert_eq!(result["project"], "proj-x", "project must carry over");

        // Position came from the target column, not the snapshot.
        assert_eq!(
            result["position_column"], "doing",
            "new task must land in the pasted-onto column, not the source's column"
        );
    }

    /// A cut clipboard payload (`mode == "cut"`) deletes the source
    /// after the new task is written.
    #[tokio::test]
    async fn paste_cut_task_deletes_source() {
        let (_temp, kanban) = setup().await;
        let add = AddTask::new("Cut me")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let source_id = add["id"].as_str().unwrap().to_string();

        let payload = snapshot_task(kanban.as_ref(), &source_id, "cut").await;
        let ctx = make_ctx(&kanban);

        let result = TaskIntoColumnHandler
            .execute(&payload, "column:doing", &ctx)
            .await
            .expect("cut paste should succeed");

        let new_id = result["id"].as_str().expect("created task must have id");
        assert_ne!(new_id, source_id, "cut must produce a new ULID");

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

    /// Pasting onto a non-`column:` moniker is a dispatcher contract
    /// violation; the handler should reject it loudly rather than
    /// silently coerce.
    #[tokio::test]
    async fn paste_into_non_column_target_errors() {
        let (_temp, kanban) = setup().await;
        let add = AddTask::new("Source")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let source_id = add["id"].as_str().unwrap().to_string();

        let payload = snapshot_task(kanban.as_ref(), &source_id, "copy").await;
        let ctx = make_ctx(&kanban);

        let result = TaskIntoColumnHandler
            .execute(&payload, "task:01OTHER", &ctx)
            .await;
        assert!(
            result.is_err(),
            "non-column target must produce an error: {result:?}"
        );
        match result.unwrap_err() {
            CommandError::DestinationInvalid(msg) => {
                assert!(
                    msg.contains("not a column moniker"),
                    "non-column target must surface DestinationInvalid with a \
                     descriptive message; got: {msg}"
                );
            }
            other => panic!("expected DestinationInvalid, got: {other:?}"),
        }
    }

    /// Acceptance criterion: when the destination column referenced by
    /// the paste target no longer exists on the board (deleted between
    /// copy and paste, or never existed), the handler must surface a
    /// structured `DestinationInvalid` with a user-readable message
    /// naming the offending column.
    ///
    /// Without the explicit guard the inner `AddEntity` would still
    /// fail — `entity::position::resolve_column` rejects unknown
    /// columns — but the error would surface as a generic
    /// `ExecutionFailed` wrapping the entity-layer message. The toast
    /// would render that as "Command failed: column 'X' does not exist
    /// on this board", which works but lumps the failure in with every
    /// other execution error. `DestinationInvalid` lets the UI handle
    /// it categorically (and keeps the toast message specific to the
    /// paste-destination failure mode).
    #[tokio::test]
    async fn paste_task_into_nonexistent_column_returns_destination_invalid_error() {
        let (_temp, kanban) = setup().await;

        // Real source task so the clipboard payload is well-formed —
        // the failure must come from the missing destination column,
        // not from a malformed clipboard snapshot.
        let add = AddTask::new("Source")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let source_id = add["id"].as_str().unwrap().to_string();

        let payload = snapshot_task(kanban.as_ref(), &source_id, "copy").await;
        let ctx = make_ctx(&kanban);

        // Target a column id that was never created on this board —
        // equivalent to a column that was deleted between the user's
        // copy and paste from the user's perspective.
        let missing_column = "ghost-column";
        let target = format!("column:{missing_column}");

        let err = TaskIntoColumnHandler
            .execute(&payload, &target, &ctx)
            .await
            .expect_err("nonexistent destination column must produce an error");
        match err {
            CommandError::DestinationInvalid(msg) => {
                assert!(
                    msg.contains(missing_column),
                    "error must name the missing column id; got: {msg}"
                );
                assert!(
                    msg.contains("no longer exists"),
                    "error must explain the failure mode; got: {msg}"
                );
            }
            other => panic!("expected DestinationInvalid, got: {other:?}"),
        }

        // The board state must be untouched — the handler must not
        // create a stub task or otherwise leave debris when it can't
        // find the destination column.
        let tasks = list_tasks(kanban.as_ref()).await;
        assert_eq!(
            tasks.len(),
            1,
            "handler must not create a task when the destination column is missing; \
             only the original source task should remain"
        );
        assert_eq!(
            tasks[0].id, source_id,
            "the surviving task must be the source"
        );
    }

    /// Position fields that ride along in the snapshot must not leak
    /// into the new task — the target column wins. This guards the
    /// `POSITION_KEYS_TO_DROP` filter.
    #[tokio::test]
    async fn snapshot_position_keys_are_overridden_by_target_column() {
        let (_temp, kanban) = setup().await;
        let add = AddTask::new("Source")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let source_id = add["id"].as_str().unwrap().to_string();

        // The default board lands a fresh task in `todo`. We synthesize
        // a clipboard payload whose snapshot pretends the source lived
        // in `done` — the handler must ignore that and place the new
        // task in the target column (`doing`).
        let mut snapshot = snapshot_task(kanban.as_ref(), &source_id, "copy").await;
        if let Value::Object(ref mut map) = snapshot.swissarmyhammer_clipboard.fields {
            map.insert("position_column".into(), Value::String("done".into()));
        }

        let ctx = make_ctx(&kanban);
        let result = TaskIntoColumnHandler
            .execute(&snapshot, "column:doing", &ctx)
            .await
            .expect("paste should succeed");

        assert_eq!(
            result["position_column"], "doing",
            "target column must override snapshot's stale position_column"
        );
    }

    /// `available()` defaults to `true` — paste availability is gated
    /// upstream by the matrix lookup. This is a regression guard so a
    /// future override does not silently disable all `(task, column)`
    /// pastes.
    #[test]
    fn handler_available_defaults_to_true() {
        let payload = ClipboardPayload {
            swissarmyhammer_clipboard: ClipboardData {
                entity_type: "task".into(),
                entity_id: "01SRC".into(),
                mode: "copy".into(),
                fields: json!({}),
            },
        };
        let ctx = CommandContext::new(
            "entity.paste",
            vec![],
            None,
            std::collections::HashMap::new(),
        );
        assert!(
            TaskIntoColumnHandler.available(&payload, "column:doing", &ctx),
            "no availability gate is configured; default must remain true"
        );
    }

    /// `_ = TaskId` import: the cut path uses `DeleteTask::new`, which
    /// accepts `impl Into<TaskId>`. We reach into `TaskId` for the test
    /// to make sure the pattern stays compatible (catches a future
    /// signature change at compile time).
    #[test]
    fn task_id_round_trips_through_delete_op_constructor() {
        let id = TaskId::from_string("01SRC");
        let _op = DeleteTask::new(id);
    }
}
