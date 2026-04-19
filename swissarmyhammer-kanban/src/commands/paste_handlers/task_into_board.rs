//! Paste-handler for `(task, board)` — drop a task onto a board with no
//! specific column in the scope chain.
//!
//! When the user copies a task and pastes onto a board (or any scope that
//! resolves to `board:` without a more specific `column:`), the new task
//! lands in the board's *leftmost* column — the one with the lowest
//! `order` value, matching the "tasks land in 'todo' by default"
//! convention enforced by [`crate::entity::position::resolve_column`].
//!
//! Cut variants delete the source task after the new task has been
//! created. Create-then-delete ordering is intentional: the destination
//! must succeed before any data is destroyed, so a paste error never
//! orphans the source. The follow-up delete is logged-and-continued on
//! failure (consistent with sibling `*_into_*` handlers — see
//! `01KPG7FDDG75EFABQ47Y198ZZJ`).

use super::PasteHandler;
use crate::clipboard::ClipboardPayload;
use crate::commands::run_op;
use crate::context::KanbanContext;
use crate::entity::AddEntity;
use crate::error::Result as KanbanResult;
use crate::task::DeleteTask;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use swissarmyhammer_commands::{CommandContext, CommandError, Result};
use swissarmyhammer_entity::EntityContext;

/// Reserved positional override keys that must be re-derived per paste.
///
/// Mirrors the same list used by [`crate::commands::paste_handlers::task_into_column`]:
/// the destination column is set explicitly from the leftmost-column
/// resolution, and the ordinal is dropped so [`AddEntity`]'s position
/// helper appends the new task at the bottom of the destination column.
/// The raw field-name forms (`position_column`, `position_ordinal`) are
/// included as well, since clipboard snapshots store the entity's full
/// field set under those names.
const POSITION_KEYS_TO_DROP: &[&str] =
    &["column", "ordinal", "position_column", "position_ordinal"];

/// Sync filesystem probe used by [`TaskIntoBoardHandler::available`].
///
/// Returns `true` when the columns directory exists and contains at
/// least one file. Used as a coarse gate so the dispatcher can skip
/// pasting onto a board that has no columns at all without a sync /
/// async runtime adapter.
fn columns_dir_has_entries(kanban: &KanbanContext) -> bool {
    let dir = kanban.columns_dir();
    let Ok(read) = std::fs::read_dir(&dir) else {
        return false;
    };
    read.flatten()
        .any(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
}

/// Pastes a task into a board's leftmost column.
///
/// Matches the `(clipboard_type, target_type)` pair `("task", "board")` in
/// the [`super::PasteMatrix`]. The dispatcher selects this handler when the
/// clipboard holds a task and the innermost matching scope frame is a
/// `board:` moniker (i.e. no more-specific `column:` is in scope).
pub struct TaskIntoBoardHandler;

impl TaskIntoBoardHandler {
    /// Find the leftmost column on the current board.
    ///
    /// "Leftmost" is the column with the lowest `order` field. Returns
    /// `None` when the board has no columns at all — callers
    /// (`available()` and `execute()`) treat that as the no-op /
    /// unavailable case rather than panicking.
    async fn leftmost_column_id(ectx: &EntityContext) -> KanbanResult<Option<String>> {
        let columns = ectx.list("column").await?;
        Ok(columns
            .iter()
            .min_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
            .map(|c| c.id.to_string()))
    }
}

#[async_trait]
impl PasteHandler for TaskIntoBoardHandler {
    fn matches(&self) -> (&'static str, &'static str) {
        ("task", "board")
    }

    /// Returns `false` when the board has no columns to paste into.
    ///
    /// Without at least one column there is nowhere to drop the task —
    /// the dispatcher should keep walking the scope chain rather than
    /// claim the paste and then fail. The check is intentionally a
    /// synchronous filesystem probe of the columns directory rather
    /// than a `list("column")` round-trip: `available()` is sync, and
    /// blocking on the async entity-context inside the dispatcher's
    /// gate would deadlock the runtime in single-threaded contexts.
    /// Missing-or-empty directory means no columns; any other state
    /// (file in the directory) is treated optimistically as
    /// "potentially available" and falls through to `execute()` for
    /// the precise check.
    fn available(
        &self,
        _clipboard: &ClipboardPayload,
        _target: &str,
        ctx: &CommandContext,
    ) -> bool {
        let Ok(kanban) = ctx.require_extension::<KanbanContext>() else {
            return false;
        };
        columns_dir_has_entries(&kanban)
    }

    /// Create a new task in the leftmost column using the clipboard's
    /// field snapshot, then delete the source task on cut.
    ///
    /// Field handling mirrors [`super::task_into_column`]: the
    /// clipboard's `fields` snapshot is forwarded as overrides to
    /// [`AddEntity`], `column` is set to the resolved leftmost column,
    /// and any stale `ordinal` from the source position is dropped so
    /// the new task appends at the bottom of the destination column.
    ///
    /// Cut variants delete the source after the destination has been
    /// successfully written. A failed create therefore never deletes
    /// the source.
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

        let column = Self::leftmost_column_id(&ectx)
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?
            .ok_or_else(|| {
                CommandError::DestinationInvalid("Board has no columns to paste a task into".into())
            })?;

        // Build override bag from the clipboard's field snapshot,
        // dropping reserved positional keys so the leftmost-column
        // override and append-at-end ordinal logic win over any stale
        // values carried from the source entity.
        let mut overrides: HashMap<String, Value> = HashMap::new();
        if let Some(obj) = clipboard.swissarmyhammer_clipboard.fields.as_object() {
            for (key, value) in obj {
                if POSITION_KEYS_TO_DROP.contains(&key.as_str()) {
                    continue;
                }
                overrides.insert(key.clone(), value.clone());
            }
        }
        overrides.insert("column".to_string(), Value::String(column));

        // Create destination first — surface any failure to the caller
        // before touching the source. This guarantees a failed paste
        // never deletes the original.
        let add_op = AddEntity::new("task").with_overrides(overrides);
        let created = run_op(&add_op, &kanban).await?;

        // Cut: delete the source after the new task is successfully
        // written. Mirrors [`super::task_into_column`] — propagate the
        // delete error so the caller knows the move is incomplete
        // rather than silently leaving a duplicate.
        if clipboard.swissarmyhammer_clipboard.mode == "cut" {
            let delete_op = DeleteTask::new(clipboard.swissarmyhammer_clipboard.entity_id.as_str());
            run_op(&delete_op, &kanban).await?;
        }

        Ok(created)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::clipboard::{
        deserialize_from_clipboard, serialize_to_clipboard, ClipboardProviderExt, InMemoryClipboard,
    };
    use crate::commands::paste_handlers::PasteMatrix;
    use crate::task::AddTask;
    use crate::Execute;
    use std::sync::Arc;
    use swissarmyhammer_commands::UIState;
    use swissarmyhammer_entity::Entity;

    /// Build a kanban context, in-memory clipboard provider, and UI state
    /// for the handler tests. The board is *not* initialised here so each
    /// test can opt into the column shape it needs (default columns,
    /// custom positions, or empty).
    async fn setup() -> (
        tempfile::TempDir,
        Arc<KanbanContext>,
        Arc<ClipboardProviderExt>,
        Arc<UIState>,
    ) {
        let temp = tempfile::TempDir::new().unwrap();
        let kanban = Arc::new(KanbanContext::new(temp.path().join(".kanban")));
        let clipboard = Arc::new(ClipboardProviderExt(Arc::new(InMemoryClipboard::new())));
        let ui = Arc::new(UIState::new());
        (temp, kanban, clipboard, ui)
    }

    /// Initialise the default todo/doing/done columns via the standard
    /// board init path. Used by tests that just need "a board with
    /// columns".
    async fn init_default_board(kanban: &Arc<KanbanContext>) {
        InitBoard::new("Test")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
    }

    /// Replace whatever columns exist on the board with `columns`,
    /// expressed as `(id, order)` pairs. Used by tests that need
    /// non-default column orderings (e.g. positions 0/100/200 to make
    /// "leftmost" unambiguous).
    async fn install_columns(kanban: &Arc<KanbanContext>, columns: &[(&str, u64)]) {
        let ectx = kanban.entity_context().await.unwrap();
        // Drop any pre-existing columns first so the test starts from a
        // known state. The default board may or may not have been
        // initialised by the caller — both paths must converge on the
        // shape the test asked for.
        for col in ectx.list("column").await.unwrap() {
            ectx.delete("column", col.id.as_str()).await.unwrap();
        }
        for (id, order) in columns {
            let mut entity = Entity::new("column", *id);
            entity.set("name", serde_json::json!(id));
            entity.set("order", serde_json::json!(*order));
            ectx.write(&entity).await.unwrap();
        }
    }

    /// Build a CommandContext wired with the kanban context, clipboard
    /// provider, and UI state extensions needed by the handler.
    fn make_ctx(
        scope: &[&str],
        kanban: &Arc<KanbanContext>,
        clipboard: &Arc<ClipboardProviderExt>,
        ui: &Arc<UIState>,
    ) -> CommandContext {
        let mut ctx = CommandContext::new(
            "entity.paste",
            scope.iter().map(|s| s.to_string()).collect(),
            None,
            HashMap::new(),
        );
        ctx.set_extension(Arc::clone(kanban));
        ctx.set_extension(Arc::clone(clipboard));
        ctx.ui_state = Some(Arc::clone(ui));
        ctx
    }

    /// Copy a task and return the resulting `ClipboardPayload`. Mirrors
    /// what `CopyCmd` would write to the system clipboard, but skips the
    /// extension-roundtrip so tests can build payloads without driving
    /// the full copy command.
    fn payload_from_task(task_id: &str, fields: serde_json::Value, mode: &str) -> ClipboardPayload {
        let json = serialize_to_clipboard("task", task_id, mode, fields);
        deserialize_from_clipboard(&json).expect("payload roundtrip must succeed")
    }

    /// Build a populated PasteMatrix for tests — registers only the
    /// handler under test so the tests don't depend on the production
    /// `register_paste_handlers()` (which is wired by the orchestrator
    /// in a separate batch step).
    fn test_matrix() -> PasteMatrix {
        let mut m = PasteMatrix::default();
        m.register(TaskIntoBoardHandler);
        m
    }

    // =========================================================================
    // matches() / find()
    // =========================================================================

    #[test]
    fn handler_matches_task_board_pair() {
        assert_eq!(TaskIntoBoardHandler.matches(), ("task", "board"));
    }

    #[test]
    fn matrix_dispatch_resolves_handler_by_pair() {
        let m = test_matrix();
        assert!(m.find("task", "board").is_some());
        assert!(m.find("task", "column").is_none());
        assert!(m.find("tag", "board").is_none());
    }

    // =========================================================================
    // available()
    // =========================================================================

    #[tokio::test]
    async fn paste_task_into_empty_board_unavailable() {
        // Acceptance criterion: when the board has zero columns,
        // `available()` returns false so the dispatcher can fall through
        // to the next scope frame instead of failing the paste.
        let (_temp, kanban, clipboard, ui) = setup().await;
        install_columns(&kanban, &[]).await;
        let ctx = make_ctx(&["board:my-board"], &kanban, &clipboard, &ui);

        let payload = payload_from_task("01SOURCE", serde_json::json!({"title": "t"}), "copy");

        assert!(
            !TaskIntoBoardHandler.available(&payload, "board:my-board", &ctx),
            "empty board must not advertise the paste as available"
        );
    }

    #[tokio::test]
    async fn paste_task_into_populated_board_available() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        init_default_board(&kanban).await;
        let ctx = make_ctx(&["board:my-board"], &kanban, &clipboard, &ui);

        let payload = payload_from_task("01SOURCE", serde_json::json!({"title": "t"}), "copy");

        assert!(
            TaskIntoBoardHandler.available(&payload, "board:my-board", &ctx),
            "board with columns must advertise the paste as available"
        );
    }

    // =========================================================================
    // execute()
    // =========================================================================

    #[tokio::test]
    async fn paste_task_into_board_uses_leftmost_column() {
        // Acceptance criterion: with columns at positions 0, 100, 200
        // the new task lands in the position-0 column. This pins the
        // "lowest order wins" semantics so a future column-creation
        // change that re-numbers positions doesn't accidentally drop
        // pasted tasks elsewhere.
        let (_temp, kanban, clipboard, ui) = setup().await;
        install_columns(&kanban, &[("first", 0), ("middle", 100), ("last", 200)]).await;
        let ctx = make_ctx(&["board:my-board"], &kanban, &clipboard, &ui);

        let payload = payload_from_task(
            "01SOURCE",
            serde_json::json!({"title": "Pasted task"}),
            "copy",
        );

        let result = TaskIntoBoardHandler
            .execute(&payload, "board:my-board", &ctx)
            .await
            .expect("paste must succeed when leftmost column exists");

        assert_eq!(
            result["position_column"], "first",
            "pasted task must land in the lowest-order column"
        );
        assert_eq!(result["title"], "Pasted task");
    }

    #[tokio::test]
    async fn paste_copy_task_into_board_does_not_delete_source() {
        // Copy mode is non-destructive — the source task must remain on
        // the board after the paste creates the new one.
        let (_temp, kanban, clipboard, ui) = setup().await;
        init_default_board(&kanban).await;

        let source = AddTask::new("Source")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let source_id = source["id"].as_str().unwrap().to_string();

        let ctx = make_ctx(&["board:my-board"], &kanban, &clipboard, &ui);
        let payload = payload_from_task(&source_id, serde_json::json!({"title": "Source"}), "copy");

        TaskIntoBoardHandler
            .execute(&payload, "board:my-board", &ctx)
            .await
            .unwrap();

        let ectx = kanban.entity_context().await.unwrap();
        assert!(
            ectx.read("task", &source_id).await.is_ok(),
            "copy mode must leave the source task in place"
        );
        assert_eq!(
            ectx.list("task").await.unwrap().len(),
            2,
            "copy mode must produce a new task in addition to the source"
        );
    }

    #[tokio::test]
    async fn paste_cut_task_into_board_deletes_source() {
        // Cut mode moves the task — the source must be deleted after
        // the new task is created. Create-before-delete ordering is
        // verified implicitly: if the new task didn't exist, we'd see
        // 0 tasks instead of 1.
        let (_temp, kanban, clipboard, ui) = setup().await;
        init_default_board(&kanban).await;

        let source = AddTask::new("Source to cut")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let source_id = source["id"].as_str().unwrap().to_string();

        let ctx = make_ctx(&["board:my-board"], &kanban, &clipboard, &ui);
        let payload = payload_from_task(
            &source_id,
            serde_json::json!({"title": "Source to cut"}),
            "cut",
        );

        let result = TaskIntoBoardHandler
            .execute(&payload, "board:my-board", &ctx)
            .await
            .unwrap();

        let new_id = result["id"].as_str().unwrap();
        assert_ne!(new_id, source_id, "cut paste must produce a new task id");

        let ectx = kanban.entity_context().await.unwrap();
        assert!(
            ectx.read("task", &source_id).await.is_err(),
            "cut mode must delete the source task"
        );
        let remaining = ectx.list("task").await.unwrap();
        assert_eq!(
            remaining.len(),
            1,
            "exactly one task must remain after cut paste"
        );
        assert_eq!(remaining[0].id, new_id, "remaining task must be the paste");
    }

    #[tokio::test]
    async fn paste_drops_stale_ordinal_from_clipboard() {
        // The clipboard snapshot carries the source task's ordinal. If
        // we forwarded it through, the new task would sort at the same
        // position — colliding with the source on copy, or landing
        // arbitrarily on cut. Force the override bag to drop the
        // ordinal so AddEntity recomputes "after the last existing
        // task" in the destination column.
        let (_temp, kanban, clipboard, ui) = setup().await;
        init_default_board(&kanban).await;

        // Pre-existing task in the destination column to make the
        // recomputed ordinal visibly different from a stale one.
        AddTask::new("Existing")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();

        let ctx = make_ctx(&["board:my-board"], &kanban, &clipboard, &ui);
        let payload = payload_from_task(
            "01SOURCE",
            serde_json::json!({
                "title": "Pasted",
                "position_column": "doing",
                "position_ordinal": "00",
                "ordinal": "00",
            }),
            "copy",
        );

        let result = TaskIntoBoardHandler
            .execute(&payload, "board:my-board", &ctx)
            .await
            .unwrap();

        assert_eq!(
            result["position_column"], "todo",
            "stale position_column must be overridden by leftmost-column resolution"
        );
        let new_ord = result["position_ordinal"]
            .as_str()
            .expect("new task must have an ordinal");
        assert_ne!(
            new_ord, "00",
            "ordinal must be recomputed, not copied from the clipboard snapshot"
        );
    }
}
