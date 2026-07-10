//! Cross-board task transfer logic.
//!
//! Provides a standalone, Tauri-independent function for transferring or copying
//! a task from one KanbanContext to another.  This lives in `swissarmyhammer-kanban`
//! so it can be exercised by Rust unit tests without any Tauri infrastructure.

use crate::{context::KanbanContext, error::KanbanError, task_helpers, types::Ordinal};
use serde_json::{json, Value};
use swissarmyhammer_entity::{Entity, EntityContext, EntityError};
use thiserror::Error;

/// Failure modes for [`transfer_task`].
///
/// Each variant names the step that failed so callers can match on the failure
/// mode (e.g. distinguish "source task missing" from "could not write the
/// target") rather than parsing an opaque error string.
#[derive(Debug, Error)]
pub enum TransferError {
    /// Opening the source board's entity context failed.
    #[error("failed to open source board: {0}")]
    SourceContext(#[source] KanbanError),

    /// Opening the target board's entity context failed.
    #[error("failed to open target board: {0}")]
    TargetContext(#[source] KanbanError),

    /// Reading the source task to be transferred failed (e.g. it does not exist).
    #[error("failed to read source task: {0}")]
    ReadSource(#[source] EntityError),

    /// Listing the target board's tasks (needed for ordinal placement) failed.
    #[error("failed to list target tasks: {0}")]
    ListTargetTasks(#[source] EntityError),

    /// Writing the new task entity to the target board failed.
    #[error("failed to write target task: {0}")]
    WriteTarget(#[source] EntityError),

    /// Deleting the source task after a (non-copy) move failed.
    #[error("failed to delete source task: {0}")]
    DeleteSource(#[source] EntityError),
}

/// Transfer or copy a task between two boards.
///
/// # Parameters
/// - `source_ctx` - KanbanContext for the source board (where the task lives now)
/// - `target_ctx` - KanbanContext for the target board (where the task is going)
/// - `task_id`    - ID of the task to transfer/copy
/// - `target_column` - Column ID on the target board to place the task in
/// - `drop_index` - Optional position index in the target column (legacy fallback)
/// - `before_id`  - Optional task ID to place before (highest priority placement)
/// - `after_id`   - Optional task ID to place after (highest priority placement)
/// - `copy_mode`  - When `true`, keep the source task; when `false`, delete it (move)
///
/// # Ordinal resolution priority
/// 1. `before_id`/`after_id` — compute ordinal from neighbors
/// 2. `drop_index` — compute from position (legacy fallback)
/// 3. Neither — append at end
///
/// # Returns
/// A JSON object `{ id, source_id, transferred, copied }` on success, or a
/// [`TransferError`] naming the failed step.
///
/// # Notes
/// Tags that do not exist on the target board are stripped from the transferred task.
/// The caller is responsible for flushing/emitting entity-change events for both boards.
#[allow(clippy::too_many_arguments)]
pub async fn transfer_task(
    source_ctx: &KanbanContext,
    target_ctx: &KanbanContext,
    task_id: &str,
    target_column: &str,
    drop_index: Option<u64>,
    before_id: Option<&str>,
    after_id: Option<&str>,
    copy_mode: bool,
) -> Result<Value, TransferError> {
    // Read source task
    let source_ectx = source_ctx
        .entity_context()
        .await
        .map_err(TransferError::SourceContext)?;
    let source_task = source_ectx
        .read("task", task_id)
        .await
        .map_err(TransferError::ReadSource)?;

    // Compute ordinal in target column
    let target_ectx = target_ctx
        .entity_context()
        .await
        .map_err(TransferError::TargetContext)?;
    let ordinal =
        compute_target_ordinal(&target_ectx, target_column, drop_index, before_id, after_id)
            .await?;

    // Create new task entity on target board, copying the source's fields.
    let new_id = ulid::Ulid::new().to_string();
    let mut new_task = Entity::new("task", new_id.as_str());
    copy_task_fields(&source_task, &mut new_task);
    strip_nonexistent_tags(&target_ectx, &mut new_task).await;

    // Set position on target board
    new_task.set("position_column", json!(target_column));
    new_task.set("position_ordinal", json!(ordinal.as_str()));

    // Write to target board
    target_ectx
        .write(&new_task)
        .await
        .map_err(TransferError::WriteTarget)?;

    // If transfer (not copy), delete source task before returning.
    // Both writes succeed before any events are emitted, avoiding a state where
    // the task appears duplicated if the delete fails.
    if !copy_mode {
        source_ectx
            .delete("task", task_id)
            .await
            .map_err(TransferError::DeleteSource)?;
    }

    Ok(json!({
        "id": new_id,
        "source_id": task_id,
        "transferred": !copy_mode,
        "copied": copy_mode,
    }))
}

/// Compute the fractional-index ordinal for the transferred task within
/// `target_column` on the target board.
///
/// Resolution priority matches the documented contract:
/// 1. `before_id`/`after_id` — place relative to a named neighbor (same pattern
///    as `MoveTask` in `task/mv.rs`); a missing reference appends at the end.
/// 2. `drop_index` — legacy position-based placement.
/// 3. Neither — append after the column's current last task.
async fn compute_target_ordinal(
    target_ectx: &EntityContext,
    target_column: &str,
    drop_index: Option<u64>,
    before_id: Option<&str>,
    after_id: Option<&str>,
) -> Result<Ordinal, TransferError> {
    let all_tasks = target_ectx
        .list("task")
        .await
        .map_err(TransferError::ListTargetTasks)?;

    if before_id.is_some() || after_id.is_some() {
        let col_tasks = sorted_column_tasks(all_tasks, target_column);
        if let Some(ref_id) = before_id {
            Ok(ordinal_before(&col_tasks, ref_id))
        } else if let Some(ref_id) = after_id {
            Ok(ordinal_after(&col_tasks, ref_id))
        } else {
            unreachable!()
        }
    } else if let Some(idx) = drop_index {
        let col_tasks = sorted_column_tasks(all_tasks, target_column);
        Ok(task_helpers::compute_ordinal_for_drop(
            &col_tasks,
            idx as usize,
        ))
    } else {
        Ok(append_ordinal(&all_tasks, target_column))
    }
}

/// Filter `tasks` to those in `column` and sort them by their fractional-index
/// `position_ordinal` (default `a0` when unset).
fn sorted_column_tasks(tasks: Vec<Entity>, column: &str) -> Vec<Entity> {
    let mut col_tasks: Vec<_> = tasks
        .into_iter()
        .filter(|t| t.get_str("position_column") == Some(column))
        .collect();
    col_tasks.sort_by(|a, b| {
        let oa = a
            .get_str("position_ordinal")
            .unwrap_or(Ordinal::DEFAULT_STR);
        let ob = b
            .get_str("position_ordinal")
            .unwrap_or(Ordinal::DEFAULT_STR);
        oa.cmp(ob)
    });
    col_tasks
}

/// Read a task's `position_ordinal` (defaulting to `a0`) as an [`Ordinal`].
fn task_ordinal(task: &Entity) -> Ordinal {
    Ordinal::from_string(task.get_str("position_ordinal").unwrap_or("a0"))
}

/// Ordinal placing the new task immediately before `ref_id` in the sorted
/// column. A missing reference appends at the end.
fn ordinal_before(col_tasks: &[Entity], ref_id: &str) -> Ordinal {
    match col_tasks.iter().position(|t| t.id.as_str() == ref_id) {
        Some(0) => {
            task_helpers::compute_ordinal_for_neighbors(None, Some(&task_ordinal(&col_tasks[0])))
        }
        Some(idx) => task_helpers::compute_ordinal_for_neighbors(
            Some(&task_ordinal(&col_tasks[idx - 1])),
            Some(&task_ordinal(&col_tasks[idx])),
        ),
        None => task_helpers::compute_ordinal_for_neighbors(
            col_tasks.last().map(task_ordinal).as_ref(),
            None,
        ),
    }
}

/// Ordinal placing the new task immediately after `ref_id` in the sorted
/// column. A missing reference appends at the end.
fn ordinal_after(col_tasks: &[Entity], ref_id: &str) -> Ordinal {
    match col_tasks.iter().position(|t| t.id.as_str() == ref_id) {
        Some(idx) if idx == col_tasks.len() - 1 => {
            task_helpers::compute_ordinal_for_neighbors(Some(&task_ordinal(&col_tasks[idx])), None)
        }
        Some(idx) => task_helpers::compute_ordinal_for_neighbors(
            Some(&task_ordinal(&col_tasks[idx])),
            Some(&task_ordinal(&col_tasks[idx + 1])),
        ),
        None => task_helpers::compute_ordinal_for_neighbors(
            col_tasks.last().map(task_ordinal).as_ref(),
            None,
        ),
    }
}

/// Ordinal appending the new task after the column's current last task, or
/// [`Ordinal::first`] when the column is empty.
fn append_ordinal(all_tasks: &[Entity], column: &str) -> Ordinal {
    let mut last_ord: Option<Ordinal> = None;
    for t in all_tasks {
        if t.get_str("position_column") == Some(column) {
            let ord = task_ordinal(t);
            last_ord = Some(match last_ord {
                None => ord,
                Some(ref o) if ord > *o => ord,
                Some(o) => o,
            });
        }
    }
    match last_ord {
        Some(last) => Ordinal::after(&last),
        None => Ordinal::first(),
    }
}

/// Copy the transferable scalar/reference fields from `source` onto `target`.
///
/// Position fields are set separately by the caller, and `tags` is intentionally
/// not copied — the body is the source of truth and tags are recomputed on read.
fn copy_task_fields(source: &Entity, target: &mut Entity) {
    const COPY_FIELDS: [&str; 8] = [
        "title",
        "body",
        "assignees",
        "depends_on",
        "priority",
        "estimate",
        "due_date",
        "color",
    ];
    for field in COPY_FIELDS {
        if let Some(val) = source.get(field) {
            target.set(field, val.clone());
        }
    }
}

/// Strip from the task's BODY any `#tag` whose tag entity does not exist on the
/// target board.
///
/// The body is the single source of truth for a task's tags (the `tags` field is
/// computed from `#tag` mentions on every read), so stripping must edit the body
/// — filtering the computed `tags` array would be futile, since it is recomputed
/// from the copied body on the next read.
async fn strip_nonexistent_tags(target_ectx: &EntityContext, task: &mut Entity) {
    let target_tags = target_ectx.list("tag").await.unwrap_or_default();
    let target_tag_names: std::collections::HashSet<String> = target_tags
        .iter()
        .filter_map(|t| t.get_str("tag_name").map(|s| s.to_string()))
        .collect();
    let mut body = task.get_str("body").unwrap_or("").to_string();
    for slug in crate::tag_parser::parse_tags(&body) {
        if !target_tag_names.contains(&slug) {
            body = crate::tag_parser::remove_tag(&body, &slug);
        }
    }
    task.set("body", json!(body));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::tag::AddTag;
    use crate::Execute;
    use tempfile::TempDir;

    /// Create an initialized board with default columns (todo/doing/done) and return (TempDir, KanbanContext).
    ///
    /// `InitBoard` already creates the default columns, so no extra `AddColumn` call is needed.
    async fn make_board() -> (TempDir, KanbanContext) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join(".kanban");
        std::fs::create_dir_all(&root).unwrap();
        let ctx = KanbanContext::open(&root).await.unwrap();
        ctx.create_directories().await.unwrap();
        InitBoard::new("Test Board")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        (dir, ctx)
    }

    /// Write a minimal task entity directly via entity context.
    async fn write_task(ctx: &KanbanContext, id: &str, title: &str, column: &str) {
        write_task_with_ordinal(ctx, id, title, column, "a0").await;
    }

    /// Write a minimal task entity with a specific ordinal.
    async fn write_task_with_ordinal(
        ctx: &KanbanContext,
        id: &str,
        title: &str,
        column: &str,
        ordinal: &str,
    ) {
        let ectx = ctx.entity_context().await.unwrap();
        let mut task = swissarmyhammer_entity::Entity::new("task", id);
        task.set("title", json!(title));
        task.set("position_column", json!(column));
        task.set("position_ordinal", json!(ordinal));
        ectx.write(&task).await.unwrap();
    }

    // =========================================================================
    // transfer_task tests — two real KanbanContexts, no Tauri
    // =========================================================================

    #[tokio::test]
    async fn cross_board_move_task_appears_on_target_removed_from_source() {
        let (_src_dir, src_ctx) = make_board().await;
        let (_tgt_dir, tgt_ctx) = make_board().await;

        write_task(&src_ctx, "TASK01", "Move me", "todo").await;

        transfer_task(
            &src_ctx, &tgt_ctx, "TASK01", "todo", None, None, None, false,
        )
        .await
        .expect("transfer should succeed");

        // Source should be empty
        let src_tasks = src_ctx.list_entities_generic("task").await.unwrap();
        assert!(
            src_tasks.is_empty(),
            "source task should be deleted after move"
        );

        // Target should have the task
        let tgt_tasks = tgt_ctx.list_entities_generic("task").await.unwrap();
        assert_eq!(tgt_tasks.len(), 1);
        assert_eq!(tgt_tasks[0].get_str("title"), Some("Move me"));
        assert_eq!(tgt_tasks[0].get_str("position_column"), Some("todo"));
    }

    #[tokio::test]
    async fn cross_board_copy_task_appears_on_target_stays_on_source() {
        let (_src_dir, src_ctx) = make_board().await;
        let (_tgt_dir, tgt_ctx) = make_board().await;

        write_task(&src_ctx, "TASK02", "Copy me", "todo").await;

        transfer_task(&src_ctx, &tgt_ctx, "TASK02", "todo", None, None, None, true)
            .await
            .expect("copy should succeed");

        // Source should still have the task
        let src_tasks = src_ctx.list_entities_generic("task").await.unwrap();
        assert_eq!(src_tasks.len(), 1, "source task should remain after copy");

        // Target should also have a copy
        let tgt_tasks = tgt_ctx.list_entities_generic("task").await.unwrap();
        assert_eq!(tgt_tasks.len(), 1);
        assert_eq!(tgt_tasks[0].get_str("title"), Some("Copy me"));
    }

    #[tokio::test]
    async fn cross_board_transfer_copies_fields() {
        let (_src_dir, src_ctx) = make_board().await;
        let (_tgt_dir, tgt_ctx) = make_board().await;

        // Write task with text fields (title and body are always preserved;
        // reference fields like assignees are validated against existing actors).
        {
            let ectx = src_ctx.entity_context().await.unwrap();
            let mut task = swissarmyhammer_entity::Entity::new("task", "TASK03");
            task.set("title", json!("Full Task"));
            task.set("body", json!("Detailed description"));
            task.set("position_column", json!("todo"));
            task.set("position_ordinal", json!("a0"));
            ectx.write(&task).await.unwrap();
        }

        transfer_task(&src_ctx, &tgt_ctx, "TASK03", "todo", None, None, None, true)
            .await
            .expect("copy should succeed");

        let tgt_tasks = tgt_ctx.list_entities_generic("task").await.unwrap();
        assert_eq!(tgt_tasks.len(), 1);
        let t = &tgt_tasks[0];
        assert_eq!(t.get_str("title"), Some("Full Task"));
        assert_eq!(t.get_str("body"), Some("Detailed description"));
    }

    #[tokio::test]
    async fn cross_board_transfer_strips_tags_not_on_target() {
        let (_src_dir, src_ctx) = make_board().await;
        let (_tgt_dir, tgt_ctx) = make_board().await;

        // Add "bug" tag entity to source board only so the tag_name filter works.
        // Tags in this system are stored as `#tag` text in the body; the `tags`
        // computed field is populated by the compute engine on read.
        AddTag::new("bug")
            .execute(&src_ctx)
            .await
            .into_result()
            .unwrap();

        // Write task with `#bug` in body — the compute engine will derive tags: ["bug"]
        {
            let ectx = src_ctx.entity_context().await.unwrap();
            let mut task = swissarmyhammer_entity::Entity::new("task", "TASK04");
            task.set("title", json!("Tagged Task"));
            task.set("body", json!("#bug Some work to do"));
            task.set("position_column", json!("todo"));
            task.set("position_ordinal", json!("a0"));
            ectx.write(&task).await.unwrap();
        }

        // Read back to confirm source has the computed tag
        let src_task = src_ctx
            .entity_context()
            .await
            .unwrap()
            .read("task", "TASK04")
            .await
            .unwrap();
        // The compute engine should have populated `tags`
        let src_tags = src_task
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        assert!(
            src_tags > 0,
            "source task should have computed tags from body"
        );

        transfer_task(&src_ctx, &tgt_ctx, "TASK04", "todo", None, None, None, true)
            .await
            .expect("copy should succeed");

        let tgt_tasks = tgt_ctx.list_entities_generic("task").await.unwrap();
        assert_eq!(tgt_tasks.len(), 1);
        // "bug" tag entity does not exist on target, so transfer_task strips
        // `#bug` from the copied BODY (the source of truth for tags). The
        // computed `tags` field, derived from the now-tagless body, is empty.
        let tags = tgt_tasks[0].get("tags");
        let is_empty = tags
            .map(|v| v.as_array().map(|a| a.is_empty()).unwrap_or(true))
            .unwrap_or(true);
        assert!(
            is_empty,
            "tags computed field should be empty on target since tag entity doesn't exist there"
        );
    }

    #[tokio::test]
    async fn cross_board_transfer_preserves_tags_that_exist_on_target() {
        let (_src_dir, src_ctx) = make_board().await;
        let (_tgt_dir, tgt_ctx) = make_board().await;

        // Add "bug" tag entity to BOTH boards
        AddTag::new("bug")
            .execute(&src_ctx)
            .await
            .into_result()
            .unwrap();
        AddTag::new("bug")
            .execute(&tgt_ctx)
            .await
            .into_result()
            .unwrap();

        // Write task with #bug in body
        {
            let ectx = src_ctx.entity_context().await.unwrap();
            let mut task = swissarmyhammer_entity::Entity::new("task", "TASK05");
            task.set("title", json!("Tagged Task"));
            task.set("body", json!("#bug Some work"));
            task.set("position_column", json!("todo"));
            task.set("position_ordinal", json!("a0"));
            ectx.write(&task).await.unwrap();
        }

        transfer_task(&src_ctx, &tgt_ctx, "TASK05", "todo", None, None, None, true)
            .await
            .expect("copy should succeed");

        let tgt_tasks = tgt_ctx.list_entities_generic("task").await.unwrap();
        assert_eq!(tgt_tasks.len(), 1);
        // Since "bug" exists on target board, the computed tags field should include it
        let tags = tgt_tasks[0].get("tags");
        let has_bug = tags
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().any(|t| t.as_str() == Some("bug")))
            .unwrap_or(false);
        assert!(
            has_bug,
            "tag present on both boards should be in computed tags on target"
        );
    }

    #[tokio::test]
    async fn cross_board_transfer_drop_index_places_correctly() {
        let (_src_dir, src_ctx) = make_board().await;
        let (_tgt_dir, tgt_ctx) = make_board().await;

        // Pre-populate target with two tasks
        write_task(&tgt_ctx, "EXISTING1", "First", "todo").await;
        {
            let ectx = tgt_ctx.entity_context().await.unwrap();
            let mut t = swissarmyhammer_entity::Entity::new("task", "EXISTING2");
            t.set("title", json!("Second"));
            t.set("position_column", json!("todo"));
            t.set("position_ordinal", json!("b0"));
            ectx.write(&t).await.unwrap();
        }

        write_task(&src_ctx, "TASK06", "Insert at 0", "todo").await;

        transfer_task(
            &src_ctx,
            &tgt_ctx,
            "TASK06",
            "todo",
            Some(0),
            None,
            None,
            false,
        )
        .await
        .expect("transfer should succeed");

        let mut tgt_tasks = tgt_ctx.list_entities_generic("task").await.unwrap();
        tgt_tasks.sort_by(|a, b| {
            let oa = a.get_str("position_ordinal").unwrap_or("a0");
            let ob = b.get_str("position_ordinal").unwrap_or("a0");
            oa.cmp(ob)
        });
        // The inserted task should have the smallest ordinal
        assert_eq!(tgt_tasks[0].get_str("title"), Some("Insert at 0"));
    }

    #[tokio::test]
    async fn cross_board_transfer_returns_correct_result_fields() {
        let (_src_dir, src_ctx) = make_board().await;
        let (_tgt_dir, tgt_ctx) = make_board().await;

        write_task(&src_ctx, "TASK07", "Result Task", "todo").await;

        let result = transfer_task(
            &src_ctx, &tgt_ctx, "TASK07", "todo", None, None, None, false,
        )
        .await
        .unwrap();

        assert_eq!(result["source_id"].as_str(), Some("TASK07"));
        assert_eq!(result["transferred"].as_bool(), Some(true));
        assert_eq!(result["copied"].as_bool(), Some(false));
        assert!(
            result["id"].as_str().is_some(),
            "new task id must be present"
        );
        assert_ne!(
            result["id"].as_str(),
            Some("TASK07"),
            "new id must differ from source id"
        );
    }

    #[tokio::test]
    async fn cross_board_copy_returns_correct_result_fields() {
        let (_src_dir, src_ctx) = make_board().await;
        let (_tgt_dir, tgt_ctx) = make_board().await;

        write_task(&src_ctx, "TASK08", "Copy Result Task", "todo").await;

        let result = transfer_task(&src_ctx, &tgt_ctx, "TASK08", "todo", None, None, None, true)
            .await
            .unwrap();

        assert_eq!(result["transferred"].as_bool(), Some(false));
        assert_eq!(result["copied"].as_bool(), Some(true));
    }

    #[tokio::test]
    async fn cross_board_transfer_error_for_nonexistent_task() {
        let (_src_dir, src_ctx) = make_board().await;
        let (_tgt_dir, tgt_ctx) = make_board().await;

        let result = transfer_task(
            &src_ctx,
            &tgt_ctx,
            "NONEXISTENT",
            "todo",
            None,
            None,
            None,
            false,
        )
        .await;
        assert!(result.is_err(), "should fail for nonexistent task");
    }

    #[tokio::test]
    async fn cross_board_transfer_before_id_places_task_before_existing() {
        let (_src_dir, src_ctx) = make_board().await;
        let (_tgt_dir, tgt_ctx) = make_board().await;

        // Generate valid fractional index ordinals
        let ord_a = Ordinal::first();
        let ord_b = Ordinal::after(&ord_a);

        // Pre-populate target with two tasks in known order
        write_task_with_ordinal(&tgt_ctx, "TGT_A", "Alpha", "todo", ord_a.as_str()).await;
        write_task_with_ordinal(&tgt_ctx, "TGT_B", "Beta", "todo", ord_b.as_str()).await;

        // Transfer a task from source, placing it before TGT_B
        write_task(&src_ctx, "SRC_01", "Inserted", "todo").await;
        transfer_task(
            &src_ctx,
            &tgt_ctx,
            "SRC_01",
            "todo",
            None,
            Some("TGT_B"),
            None,
            false,
        )
        .await
        .expect("transfer with before_id should succeed");

        // Collect and sort target tasks by ordinal
        let mut tgt_tasks = tgt_ctx.list_entities_generic("task").await.unwrap();
        tgt_tasks.sort_by(|a, b| {
            let oa = a
                .get_str("position_ordinal")
                .unwrap_or(Ordinal::DEFAULT_STR);
            let ob = b
                .get_str("position_ordinal")
                .unwrap_or(Ordinal::DEFAULT_STR);
            oa.cmp(ob)
        });

        assert_eq!(tgt_tasks.len(), 3);
        // Order should be: Alpha, Inserted, Beta
        assert_eq!(tgt_tasks[0].get_str("title"), Some("Alpha"));
        assert_eq!(tgt_tasks[1].get_str("title"), Some("Inserted"));
        assert_eq!(tgt_tasks[2].get_str("title"), Some("Beta"));
    }

    #[tokio::test]
    async fn cross_board_transfer_after_id_places_task_after_existing() {
        let (_src_dir, src_ctx) = make_board().await;
        let (_tgt_dir, tgt_ctx) = make_board().await;

        // Generate valid fractional index ordinals
        let ord_x = Ordinal::first();
        let ord_y = Ordinal::after(&ord_x);

        // Pre-populate target with two tasks in known order
        write_task_with_ordinal(&tgt_ctx, "TGT_X", "Xray", "todo", ord_x.as_str()).await;
        write_task_with_ordinal(&tgt_ctx, "TGT_Y", "Yankee", "todo", ord_y.as_str()).await;

        // Transfer a task from source, placing it after TGT_X
        write_task(&src_ctx, "SRC_02", "Middle", "todo").await;
        transfer_task(
            &src_ctx,
            &tgt_ctx,
            "SRC_02",
            "todo",
            None,
            None,
            Some("TGT_X"),
            false,
        )
        .await
        .expect("transfer with after_id should succeed");

        // Collect and sort target tasks by ordinal
        let mut tgt_tasks = tgt_ctx.list_entities_generic("task").await.unwrap();
        tgt_tasks.sort_by(|a, b| {
            let oa = a
                .get_str("position_ordinal")
                .unwrap_or(Ordinal::DEFAULT_STR);
            let ob = b
                .get_str("position_ordinal")
                .unwrap_or(Ordinal::DEFAULT_STR);
            oa.cmp(ob)
        });

        assert_eq!(tgt_tasks.len(), 3);
        // Order should be: Xray, Middle, Yankee
        assert_eq!(tgt_tasks[0].get_str("title"), Some("Xray"));
        assert_eq!(tgt_tasks[1].get_str("title"), Some("Middle"));
        assert_eq!(tgt_tasks[2].get_str("title"), Some("Yankee"));
    }
}
