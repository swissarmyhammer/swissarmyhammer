//! Cross-board task transfer logic.
//!
//! Provides a standalone, Tauri-independent function for transferring or copying
//! a task from one KanbanContext to another.  This lives in `swissarmyhammer-kanban`
//! so it can be exercised by Rust unit tests without any Tauri infrastructure.

use crate::{context::KanbanContext, task_helpers, types::Ordinal};
use serde_json::{json, Value};

/// Transfer or copy a task between two boards.
///
/// # Parameters
/// - `source_ctx` - KanbanContext for the source board (where the task lives now)
/// - `target_ctx` - KanbanContext for the target board (where the task is going)
/// - `task_id`    - ID of the task to transfer/copy
/// - `target_column` - Column ID on the target board to place the task in
/// - `drop_index` - Optional position index in the target column; if `None`, appends at end
/// - `copy_mode`  - When `true`, keep the source task; when `false`, delete it (move)
///
/// # Returns
/// A JSON object `{ id, source_id, transferred, copied }` on success, or a `String`
/// error message on failure.
///
/// # Notes
/// Tags that do not exist on the target board are stripped from the transferred task.
/// The caller is responsible for flushing/emitting entity-change events for both boards.
pub async fn transfer_task(
    source_ctx: &KanbanContext,
    target_ctx: &KanbanContext,
    task_id: &str,
    target_column: &str,
    drop_index: Option<u64>,
    copy_mode: bool,
) -> Result<Value, String> {
    // Read source task
    let source_ectx = source_ctx
        .entity_context()
        .await
        .map_err(|e| e.to_string())?;
    let source_task = source_ectx
        .read("task", task_id)
        .await
        .map_err(|e| format!("Failed to read source task: {}", e))?;

    // Compute ordinal in target column
    let target_ectx = target_ctx
        .entity_context()
        .await
        .map_err(|e| e.to_string())?;
    let ordinal = {
        let all_tasks = target_ectx.list("task").await.map_err(|e| e.to_string())?;
        if let Some(idx) = drop_index {
            let mut col_tasks: Vec<_> = all_tasks
                .into_iter()
                .filter(|t| t.get_str("position_column") == Some(target_column))
                .collect();
            col_tasks.sort_by(|a, b| {
                let oa = a.get_str("position_ordinal").unwrap_or("a0");
                let ob = b.get_str("position_ordinal").unwrap_or("a0");
                oa.cmp(ob)
            });
            task_helpers::compute_ordinal_for_drop(&col_tasks, idx as usize)
        } else {
            let mut last_ord: Option<Ordinal> = None;
            for t in &all_tasks {
                if t.get_str("position_column") == Some(target_column) {
                    let ord = Ordinal::from_string(t.get_str("position_ordinal").unwrap_or("a0"));
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
    };

    // Create new task entity on target board
    let new_id = ulid::Ulid::new().to_string();
    let mut new_task = swissarmyhammer_entity::Entity::new("task", new_id.as_str());

    // Copy fields from source
    let copy_fields = [
        "title",
        "body",
        "assignees",
        "depends_on",
        "priority",
        "estimate",
        "due_date",
        "color",
    ];
    for field in copy_fields {
        if let Some(val) = source_task.get(field) {
            new_task.set(field, val.clone());
        }
    }

    // Strip tags that don't exist in the target board
    if let Some(tags_val) = source_task.get("tags") {
        if let Some(tags_arr) = tags_val.as_array() {
            let target_tags = target_ectx.list("tag").await.unwrap_or_default();
            let target_tag_names: std::collections::HashSet<String> = target_tags
                .iter()
                .filter_map(|t| t.get_str("tag_name").map(|s| s.to_string()))
                .collect();
            let filtered: Vec<Value> = tags_arr
                .iter()
                .filter(|t| {
                    t.as_str()
                        .map(|s| target_tag_names.contains(s))
                        .unwrap_or(false)
                })
                .cloned()
                .collect();
            if !filtered.is_empty() {
                new_task.set("tags", json!(filtered));
            }
        }
    }

    // Set position on target board
    new_task.set("position_column", json!(target_column));
    new_task.set("position_ordinal", json!(ordinal.as_str()));

    // Write to target board
    target_ectx
        .write(&new_task)
        .await
        .map_err(|e| format!("Failed to write target task: {}", e))?;

    // If transfer (not copy), delete source task before returning.
    // Both writes succeed before any events are emitted, avoiding a state where
    // the task appears duplicated if the delete fails.
    if !copy_mode {
        source_ectx
            .delete("task", task_id)
            .await
            .map_err(|e| format!("Failed to delete source task: {}", e))?;
    }

    Ok(json!({
        "id": new_id,
        "source_id": task_id,
        "transferred": !copy_mode,
        "copied": copy_mode,
    }))
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
        let ectx = ctx.entity_context().await.unwrap();
        let mut task = swissarmyhammer_entity::Entity::new("task", id);
        task.set("title", json!(title));
        task.set("position_column", json!(column));
        task.set("position_ordinal", json!("a0"));
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

        transfer_task(&src_ctx, &tgt_ctx, "TASK01", "todo", None, false)
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

        transfer_task(&src_ctx, &tgt_ctx, "TASK02", "todo", None, true)
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

        transfer_task(&src_ctx, &tgt_ctx, "TASK03", "todo", None, true)
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

        transfer_task(&src_ctx, &tgt_ctx, "TASK04", "todo", None, true)
            .await
            .expect("copy should succeed");

        let tgt_tasks = tgt_ctx.list_entities_generic("task").await.unwrap();
        assert_eq!(tgt_tasks.len(), 1);
        // "bug" tag entity does not exist on target — the tags field should be absent/empty.
        // Note: the body "#bug" is copied over, but the tags filter in transfer_task
        // uses tag entities on the target board to filter the `tags` computed field.
        // The body is copied as-is; tag filtering affects the explicit `tags` field only.
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

        transfer_task(&src_ctx, &tgt_ctx, "TASK05", "todo", None, true)
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

        transfer_task(&src_ctx, &tgt_ctx, "TASK06", "todo", Some(0), false)
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

        let result = transfer_task(&src_ctx, &tgt_ctx, "TASK07", "todo", None, false)
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

        let result = transfer_task(&src_ctx, &tgt_ctx, "TASK08", "todo", None, true)
            .await
            .unwrap();

        assert_eq!(result["transferred"].as_bool(), Some(false));
        assert_eq!(result["copied"].as_bool(), Some(true));
    }

    #[tokio::test]
    async fn cross_board_transfer_error_for_nonexistent_task() {
        let (_src_dir, src_ctx) = make_board().await;
        let (_tgt_dir, tgt_ctx) = make_board().await;

        let result = transfer_task(&src_ctx, &tgt_ctx, "NONEXISTENT", "todo", None, false).await;
        assert!(result.is_err(), "should fail for nonexistent task");
    }
}
