//! Paste handler: `(column, board)` — duplicate a column onto a board.
//!
//! Pasting a column onto a board creates a new column whose field values
//! are seeded from the clipboard snapshot (`name`, plus any other styling
//! fields the column carries). The new column is appended at the end of
//! the destination board's column ordering — the source column's `order`
//! is intentionally dropped so the paste does not collide with an existing
//! column's slot or shuffle the board's layout.
//!
//! This handler duplicates the **column structure only**. Tasks that lived
//! in the source column are not copied — task duplication is a separate
//! concern handled by the `(task, *)` handlers and would require either a
//! cross-board read of the source's task list (impossible from a clipboard
//! snapshot, which carries only the column entity itself) or a multi-step
//! workflow. Keeping this handler narrow matches the rest of the matrix:
//! one entity pairing per file, one operation per handler.
//!
//! When the clipboard payload was produced by a `cut` (rather than a
//! `copy`), the source column is deleted after the new column has been
//! successfully written. A failed create therefore leaves the source
//! column untouched — cut is "create-then-delete", never
//! "delete-then-create". The follow-up [`DeleteColumn`] propagates errors
//! to the caller (e.g. `ColumnNotEmpty` when the source still has tasks)
//! rather than being swallowed, since silently leaving a duplicate would
//! hide the partial-move state from the user.
//!
//! This handler matches the `("column", "board")` pair in [`PasteMatrix`].
//! Per the parallel-safety override in the implementing card, the
//! production registration line `m.register(ColumnIntoBoardHandler);` is
//! intentionally **deferred** — the orchestrator batch-registers all
//! sibling paste handlers in a single step. The colocated tests below
//! exercise the handler against a local matrix so the file is testable
//! in isolation.
//!
//! [`PasteMatrix`]: super::PasteMatrix
//! [`DeleteColumn`]: crate::column::DeleteColumn

use super::PasteHandler;
use crate::clipboard::ClipboardPayload;
use crate::column::DeleteColumn;
use crate::commands::run_op;
use crate::context::KanbanContext;
use crate::entity::AddEntity;
use crate::error::Result as KanbanResult;
use async_trait::async_trait;
use serde_json::{Map, Value};
use std::collections::HashMap;
use swissarmyhammer_commands::{CommandContext, CommandError, Result};
use swissarmyhammer_entity::EntityContext;

/// Snapshot keys that must NOT be forwarded onto the new column.
///
/// `order` is dropped because the new column is always appended at the
/// end of the destination board — re-using the source column's order
/// would either collide with an existing column or insert the paste at a
/// surprising slot in the layout.
///
/// `id` and `entity_type` are dropped because the snapshot may include
/// them as bookkeeping fields; the new column receives a fresh ULID from
/// [`AddEntity`] and its entity type is fixed to `"column"`.
const FIELDS_TO_DROP: &[&str] = &["order", "id", "entity_type"];

/// Pastes a column onto a board, appending it at the end of the column
/// ordering.
///
/// Matches the `(clipboard_type, target_type)` pair `("column", "board")`
/// in the [`super::PasteMatrix`]. The dispatcher selects this handler when
/// the clipboard holds a column and the innermost matching scope frame is
/// a `board:` moniker.
pub struct ColumnIntoBoardHandler;

impl ColumnIntoBoardHandler {
    /// Compute the next available `order` value for a new column.
    ///
    /// "Next" is the highest existing `order` plus one, or `0` when the
    /// board has no columns yet. Mirrors the unsupplied-order branch of
    /// [`crate::column::AddColumn`] so the paste-created column lands in
    /// the same slot a fresh `column.add` would take.
    async fn next_order(ectx: &EntityContext) -> KanbanResult<u64> {
        let columns = ectx.list("column").await?;
        Ok(columns
            .iter()
            .filter_map(|c| c.get("order").and_then(|v| v.as_u64()))
            .max()
            .map(|o| o + 1)
            .unwrap_or(0))
    }
}

#[async_trait]
impl PasteHandler for ColumnIntoBoardHandler {
    fn matches(&self) -> (&'static str, &'static str) {
        ("column", "board")
    }

    /// Create a new column from the clipboard snapshot, appended at the
    /// end of the board's column ordering, then delete the source column
    /// on cut.
    ///
    /// Field handling mirrors the sibling `*_into_board` handlers: the
    /// clipboard's `fields` snapshot is forwarded as overrides to
    /// [`AddEntity`], with the source `order` dropped so the new column
    /// appends at the end. Bookkeeping fields (`id`, `entity_type`) that
    /// may ride along in the snapshot are also stripped — the new column
    /// receives a fresh ULID and its entity type is fixed.
    ///
    /// Cut variants delete the source after the destination has been
    /// successfully written. A failed create therefore never deletes
    /// the source. The follow-up [`DeleteColumn`] propagates errors to
    /// the caller (e.g. `ColumnNotEmpty` when the source still has
    /// tasks) rather than being swallowed.
    ///
    /// Tasks in the source column are not copied — see the module docs.
    async fn execute(
        &self,
        clipboard: &ClipboardPayload,
        _target: &str,
        ctx: &CommandContext,
    ) -> Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        let ectx = kanban
            .entity_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        // Validate the source column referenced by the clipboard still
        // exists. The snapshot's `fields` are forwarded as overrides
        // for the new column, so a deleted source still notionally
        // succeeds — but a `cut` paste would then call DeleteColumn on
        // a column that doesn't exist and surface a generic error. By
        // checking up front we can name the failure cleanly: the
        // user's clipboard refers to a column that's no longer there.
        let source_id = clipboard.swissarmyhammer_clipboard.entity_id.as_str();
        if !source_id.is_empty() && ectx.read("column", source_id).await.is_err() {
            return Err(CommandError::SourceEntityMissing(format!(
                "Column '{source_id}' no longer exists"
            )));
        }

        let order = Self::next_order(&ectx)
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        let mut overrides = build_overrides(&clipboard.swissarmyhammer_clipboard.fields);
        overrides.insert("order".to_string(), Value::from(order));

        // Create destination first — surface any failure to the caller
        // before touching the source. This guarantees a failed paste
        // never deletes the original.
        let add_op = AddEntity::new("column").with_overrides(overrides);
        let created = run_op(&add_op, &kanban).await?;

        // Cut: delete the source after the new column is successfully
        // written. Mirrors the sibling `task_into_*` handlers — propagate
        // the delete error so the caller knows the move is incomplete
        // rather than silently leaving a duplicate.
        if clipboard.swissarmyhammer_clipboard.mode == "cut" {
            let delete_op =
                DeleteColumn::new(clipboard.swissarmyhammer_clipboard.entity_id.as_str());
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
///   as an empty bag — paste then degrades to "create a default column"
///   rather than failing, since there is nothing useful to recover from.
/// - All [`FIELDS_TO_DROP`] are stripped so the new column appends at the
///   end and receives a fresh identity.
fn build_overrides(snapshot: &Value) -> HashMap<String, Value> {
    match snapshot {
        Value::Object(map) => filtered_overrides(map),
        _ => HashMap::new(),
    }
}

/// Build the override map from a snapshot object, dropping reserved keys.
fn filtered_overrides(snapshot: &Map<String, Value>) -> HashMap<String, Value> {
    snapshot
        .iter()
        .filter(|(k, _)| !FIELDS_TO_DROP.contains(&k.as_str()))
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
    use crate::column::AddColumn;
    use crate::commands::paste_handlers::PasteMatrix;
    use crate::task::AddTask;
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
    /// extension. The scope chain is empty — `target` is passed in to the
    /// handler call directly, mirroring how `PasteEntityCmd` invokes a
    /// handler.
    fn make_ctx(kanban: &Arc<KanbanContext>) -> CommandContext {
        let mut ctx = CommandContext::new("entity.paste", vec![], None, HashMap::new());
        ctx.set_extension(Arc::clone(kanban));
        ctx
    }

    /// Snapshot a column's fields into a `ClipboardPayload` with the given
    /// `mode` ("copy" or "cut").
    async fn snapshot_column(
        kanban: &KanbanContext,
        column_id: &str,
        mode: &str,
    ) -> ClipboardPayload {
        let ectx = kanban.entity_context().await.unwrap();
        let entity = ectx.read("column", column_id).await.unwrap();
        let fields = serde_json::to_value(&entity.fields).unwrap();
        let json = serialize_to_clipboard("column", column_id, mode, fields);
        deserialize_from_clipboard(&json).expect("snapshot must round-trip")
    }

    /// Read every column currently on the board.
    async fn list_columns(kanban: &KanbanContext) -> Vec<swissarmyhammer_entity::Entity> {
        kanban
            .entity_context()
            .await
            .unwrap()
            .list("column")
            .await
            .unwrap()
    }

    /// Build a populated PasteMatrix for tests — registers only the
    /// handler under test so the tests don't depend on the production
    /// `register_paste_handlers()` (which is wired by the orchestrator
    /// in a separate batch step).
    fn test_matrix() -> PasteMatrix {
        let mut m = PasteMatrix::default();
        m.register(ColumnIntoBoardHandler);
        m
    }

    // =========================================================================
    // matches() / find()
    // =========================================================================

    #[test]
    fn handler_matches_column_board_pair() {
        assert_eq!(ColumnIntoBoardHandler.matches(), ("column", "board"));
    }

    #[test]
    fn local_matrix_finds_column_into_board_handler() {
        let m = test_matrix();
        assert!(
            m.find("column", "board").is_some(),
            "matrix should resolve (column, board) to ColumnIntoBoardHandler"
        );
        assert!(
            m.find("column", "column").is_none(),
            "matrix should not resolve unrelated pairs"
        );
    }

    /// `available()` defaults to `true` — paste availability is gated
    /// upstream by the matrix lookup. Regression guard so a future
    /// override does not silently disable the pairing.
    #[test]
    fn handler_available_defaults_to_true() {
        let payload = ClipboardPayload {
            swissarmyhammer_clipboard: ClipboardData {
                entity_type: "column".into(),
                entity_id: "doing".into(),
                mode: "copy".into(),
                fields: json!({}),
            },
        };
        let ctx = CommandContext::new("entity.paste", vec![], None, HashMap::new());
        assert!(
            ColumnIntoBoardHandler.available(&payload, "board:my-board", &ctx),
            "no availability gate is configured; default must remain true"
        );
    }

    // =========================================================================
    // execute()
    // =========================================================================

    /// Acceptance criterion: pasting a column onto a board creates a new
    /// column carrying the source's `name` field.
    #[tokio::test]
    async fn paste_column_into_board_creates_column() {
        let (_temp, kanban) = setup().await;

        // The default board has a "Doing" column from InitBoard.
        let payload = snapshot_column(kanban.as_ref(), "doing", "copy").await;
        let ctx = make_ctx(&kanban);

        let result = ColumnIntoBoardHandler
            .execute(&payload, "board:my-board", &ctx)
            .await
            .expect("paste should succeed");

        // A new column — distinct ULID id — lands on the board carrying
        // the source's name.
        let new_id = result["id"].as_str().expect("created column must have id");
        assert_ne!(new_id, "doing", "pasted column must have a fresh ULID");
        assert_eq!(
            result["name"], "Doing",
            "name from clipboard snapshot must carry over"
        );

        // The source column is unchanged: copy is non-destructive, and
        // the new column is in addition to the originals.
        let columns = list_columns(kanban.as_ref()).await;
        assert!(
            columns.iter().any(|c| c.id.as_str() == "doing"),
            "source column 'doing' must still exist after copy paste"
        );
        assert!(
            columns.iter().any(|c| c.id.as_str() == new_id),
            "new column with fresh id must exist on the board"
        );
    }

    /// Acceptance criterion: pasting a column duplicates the column
    /// structure only — tasks in the source column are not copied to the
    /// new column.
    #[tokio::test]
    async fn paste_column_into_board_does_not_copy_tasks() {
        let (_temp, kanban) = setup().await;

        // Seed the source column with three tasks. The default
        // `AddTask` lands a task in the leftmost column ("todo"); add
        // them there so the source has tasks to (not) copy.
        for title in &["A", "B", "C"] {
            AddTask::new(*title)
                .execute(kanban.as_ref())
                .await
                .into_result()
                .unwrap();
        }

        // Sanity-check the source has the tasks we just added.
        let ectx = kanban.entity_context().await.unwrap();
        let tasks_before = ectx.list("task").await.unwrap();
        let source_count = tasks_before
            .iter()
            .filter(|t| t.get_str("position_column") == Some("todo"))
            .count();
        assert_eq!(
            source_count, 3,
            "test setup must place 3 tasks in source column"
        );

        let payload = snapshot_column(kanban.as_ref(), "todo", "copy").await;
        let ctx = make_ctx(&kanban);

        let result = ColumnIntoBoardHandler
            .execute(&payload, "board:my-board", &ctx)
            .await
            .expect("paste should succeed");
        let new_id = result["id"]
            .as_str()
            .expect("created column must have id")
            .to_string();

        // The new column has zero tasks — task duplication is a separate
        // concern.
        let tasks_after = ectx.list("task").await.unwrap();
        let new_count = tasks_after
            .iter()
            .filter(|t| t.get_str("position_column") == Some(new_id.as_str()))
            .count();
        assert_eq!(
            new_count, 0,
            "newly-pasted column must have zero tasks; got {new_count}"
        );

        // Source still has its three tasks.
        let still_in_source = tasks_after
            .iter()
            .filter(|t| t.get_str("position_column") == Some("todo"))
            .count();
        assert_eq!(
            still_in_source, 3,
            "source column must retain its tasks after copy paste"
        );
    }

    /// Acceptance criterion: the new column is appended at the end of
    /// the board's column ordering — its `order` is greater than every
    /// pre-existing column's `order`.
    #[tokio::test]
    async fn paste_column_appended_at_end() {
        let (_temp, kanban) = setup().await;

        // Capture the highest existing order before the paste.
        let columns_before = list_columns(kanban.as_ref()).await;
        let max_before = columns_before
            .iter()
            .filter_map(|c| c.get("order").and_then(|v| v.as_u64()))
            .max()
            .expect("default board must have at least one column");

        let payload = snapshot_column(kanban.as_ref(), "doing", "copy").await;
        let ctx = make_ctx(&kanban);

        let result = ColumnIntoBoardHandler
            .execute(&payload, "board:my-board", &ctx)
            .await
            .expect("paste should succeed");

        let new_order = result["order"]
            .as_u64()
            .expect("created column must have a numeric order");
        assert!(
            new_order > max_before,
            "new column's order ({new_order}) must exceed prior max ({max_before})"
        );

        // Cross-check by reading every column back: no other column
        // shares or exceeds the new one.
        let columns_after = list_columns(kanban.as_ref()).await;
        for col in &columns_after {
            if col.get("order").and_then(|v| v.as_u64()) == Some(new_order) {
                // The new column itself is allowed; everything else must
                // be strictly lower.
                assert_eq!(
                    col.id.as_str(),
                    result["id"].as_str().unwrap(),
                    "no other column may share the new column's order"
                );
            }
        }
    }

    /// Acceptance criterion: a `cut` clipboard payload deletes the source
    /// column after the new column is written. Create-before-delete
    /// ordering is verified implicitly: if the create failed, we'd never
    /// reach the delete.
    #[tokio::test]
    async fn paste_cut_column_deletes_source() {
        let (_temp, kanban) = setup().await;

        // Use a freshly-added empty column as the source so the cut's
        // `DeleteColumn` is not blocked by `ColumnNotEmpty`. The default
        // board's columns may have implicit task placement we don't
        // control here; an explicit empty column makes the delete
        // unambiguous.
        AddColumn::new("staging", "Staging")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();

        let payload = snapshot_column(kanban.as_ref(), "staging", "cut").await;
        let ctx = make_ctx(&kanban);

        let result = ColumnIntoBoardHandler
            .execute(&payload, "board:my-board", &ctx)
            .await
            .expect("cut paste should succeed for an empty source column");

        let new_id = result["id"]
            .as_str()
            .expect("created column must have id")
            .to_string();
        assert_ne!(new_id, "staging", "cut paste must produce a fresh ULID");

        let ectx = kanban.entity_context().await.unwrap();
        assert!(
            ectx.read("column", "staging").await.is_err(),
            "cut mode must delete the source column"
        );

        // The new column survives.
        assert!(
            ectx.read("column", &new_id).await.is_ok(),
            "newly-pasted column must remain after cut paste"
        );
    }
}
