//! Entity-level command implementations: update field, delete, tag update,
//! attachment delete, paste.

use super::run_op;
use crate::context::KanbanContext;
use crate::types::Ordinal;
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{parse_moniker, Command, CommandContext, CommandError};

/// Update a single field on any entity.
///
/// Always available (all parameters come from args).
/// Required args: `entity_type`, `id`, `field_name`, `value`.
pub struct UpdateEntityFieldCmd;

#[async_trait]
impl Command for UpdateEntityFieldCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let entity_type = ctx.require_arg_str("entity_type")?;
        let id = ctx.require_arg_str("id")?;
        let field_name = ctx.require_arg_str("field_name")?;
        let value = ctx
            .arg("value")
            .cloned()
            .ok_or_else(|| CommandError::MissingArg("value".into()))?;

        let op = crate::entity::UpdateEntityField::new(entity_type, id, field_name, value);

        run_op(&op, &kanban).await
    }
}

/// Delete any entity by its target moniker.
///
/// Available when a target moniker is set. Dispatches to the correct
/// delete operation based on the entity type parsed from the moniker.
pub struct DeleteEntityCmd;

#[async_trait]
impl Command for DeleteEntityCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.target.is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let moniker = ctx
            .target
            .as_deref()
            .ok_or_else(|| CommandError::MissingArg("target".into()))?;
        let (entity_type, id) =
            parse_moniker(moniker).ok_or_else(|| CommandError::InvalidMoniker(moniker.into()))?;

        match entity_type {
            "task" => run_op(&crate::task::DeleteTask::new(id), &kanban).await,
            "tag" => run_op(&crate::tag::DeleteTag::new(id), &kanban).await,
            "column" => run_op(&crate::column::DeleteColumn::new(id), &kanban).await,
            "actor" => run_op(&crate::actor::DeleteActor::new(id), &kanban).await,
            "swimlane" => run_op(&crate::swimlane::DeleteSwimlane::new(id), &kanban).await,
            _ => Err(CommandError::ExecutionFailed(format!(
                "unknown entity type for delete: '{}'",
                entity_type
            ))),
        }
    }
}

/// Archive any entity by its target moniker.
///
/// Available when a target moniker is set. Dispatches to EntityContext::archive()
/// based on the entity type parsed from the moniker.
pub struct ArchiveEntityCmd;

#[async_trait]
impl Command for ArchiveEntityCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.target.is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let moniker = ctx
            .target
            .as_deref()
            .ok_or_else(|| CommandError::MissingArg("target".into()))?;
        let (entity_type, id) =
            parse_moniker(moniker).ok_or_else(|| CommandError::InvalidMoniker(moniker.into()))?;

        // For tasks, dispatch to ArchiveTask which handles dependency cleanup
        // (same as DeleteEntityCmd dispatches to DeleteTask for tasks).
        // For other entity types, call EntityContext::archive() directly.
        if entity_type == "task" {
            return run_op(&crate::task::ArchiveTask::new(id), &kanban).await;
        }

        let ectx = kanban
            .entity_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        ectx.archive(entity_type, id)
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        Ok(serde_json::json!({"archived": true}))
    }
}

/// Restore any entity from the archive by its target moniker.
///
/// Available when a target moniker is set. Dispatches to EntityContext::unarchive()
/// based on the entity type parsed from the moniker.
pub struct UnarchiveEntityCmd;

#[async_trait]
impl Command for UnarchiveEntityCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.target.is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let moniker = ctx
            .target
            .as_deref()
            .ok_or_else(|| CommandError::MissingArg("target".into()))?;
        let (entity_type, id) =
            parse_moniker(moniker).ok_or_else(|| CommandError::InvalidMoniker(moniker.into()))?;

        let ectx = kanban
            .entity_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        ectx.unarchive(entity_type, id)
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        Ok(serde_json::json!({"unarchived": true}))
    }
}

/// Update a tag's name, color, or description.
///
/// Available when `tag` is in the scope chain.
/// Optional args: `name`, `color`, `description`.
pub struct TagUpdateCmd;

#[async_trait]
impl Command for TagUpdateCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("tag")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let tag_id = ctx
            .resolve_entity_id("tag")
            .ok_or_else(|| CommandError::MissingScope("tag".into()))?;

        let mut op = crate::tag::UpdateTag::new(tag_id);

        if let Some(name) = ctx.arg("name").and_then(|v| v.as_str()) {
            op = op.with_name(name);
        }
        if let Some(color) = ctx.arg("color").and_then(|v| v.as_str()) {
            op = op.with_color(color);
        }
        if let Some(description) = ctx.arg("description").and_then(|v| v.as_str()) {
            op = op.with_description(description);
        }

        run_op(&op, &kanban).await
    }
}

/// Paste from clipboard: create a new task from the clipboard snapshot.
///
/// Available when the UIState clipboard is populated AND (column or board
/// is in the scope chain). Position: after the focused task if one exists,
/// otherwise first position in the target column. Clipboard persists after
/// paste (can paste multiple times).
pub struct PasteCmd;

#[async_trait]
impl Command for PasteCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        let has_clipboard = ctx
            .ui_state
            .as_ref()
            .and_then(|ui| ui.clipboard())
            .is_some();
        has_clipboard && (ctx.has_in_scope("column") || ctx.has_in_scope("board"))
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("no UIState".into()))?;

        let clip = ui
            .clipboard()
            .ok_or_else(|| CommandError::ExecutionFailed("clipboard is empty".into()))?;

        // Determine target column: from scope chain, or first column if only board in scope
        let column_id = if let Some(col) = ctx.resolve_entity_id("column") {
            col.to_string()
        } else {
            // Only board in scope — load columns and pick the first one
            let columns = kanban
                .list_entities_generic("column")
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
            let mut columns = columns;
            columns.sort_by(|a, b| {
                let oa = a.get_str("order").unwrap_or("0");
                let ob = b.get_str("order").unwrap_or("0");
                oa.cmp(ob)
            });
            columns
                .first()
                .map(|c| c.id.to_string())
                .ok_or_else(|| CommandError::ExecutionFailed("no columns on board".into()))?
        };

        // Determine position: after focused task if one exists, otherwise first position
        let all_tasks = kanban
            .list_entities_generic("task")
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
        let mut col_tasks: Vec<_> = all_tasks
            .into_iter()
            .filter(|t| t.get_str("position_column") == Some(&column_id))
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

        let ordinal = if let Some(focused_task_id) = ctx.resolve_entity_id("task") {
            // Place after the focused task
            let ref_idx = col_tasks
                .iter()
                .position(|t| t.id.as_str() == focused_task_id);
            match ref_idx {
                Some(idx) if idx == col_tasks.len() - 1 => {
                    // After the last task
                    let ref_ord = Ordinal::from_string(
                        col_tasks[idx]
                            .get_str("position_ordinal")
                            .unwrap_or(Ordinal::DEFAULT_STR),
                    );
                    crate::task_helpers::compute_ordinal_for_neighbors(Some(&ref_ord), None)
                }
                Some(idx) => {
                    // Between focused task and its successor
                    let ref_ord = Ordinal::from_string(
                        col_tasks[idx]
                            .get_str("position_ordinal")
                            .unwrap_or(Ordinal::DEFAULT_STR),
                    );
                    let succ_ord = Ordinal::from_string(
                        col_tasks[idx + 1]
                            .get_str("position_ordinal")
                            .unwrap_or(Ordinal::DEFAULT_STR),
                    );
                    crate::task_helpers::compute_ordinal_for_neighbors(
                        Some(&ref_ord),
                        Some(&succ_ord),
                    )
                }
                None => {
                    // Focused task not in this column — first position
                    crate::task_helpers::compute_ordinal_for_neighbors(
                        None,
                        col_tasks
                            .first()
                            .map(|t| {
                                Ordinal::from_string(
                                    t.get_str("position_ordinal")
                                        .unwrap_or(Ordinal::DEFAULT_STR),
                                )
                            })
                            .as_ref(),
                    )
                }
            }
        } else {
            // No focused task — first position
            crate::task_helpers::compute_ordinal_for_neighbors(
                None,
                col_tasks
                    .first()
                    .map(|t| {
                        Ordinal::from_string(
                            t.get_str("position_ordinal")
                                .unwrap_or(Ordinal::DEFAULT_STR),
                        )
                    })
                    .as_ref(),
            )
        };

        // Build AddTask from clipboard fields
        let title = clip
            .fields
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Pasted task")
            .to_string();

        let mut op = crate::task::AddTask::new(title);
        op.column = Some(column_id);
        op.ordinal = Some(ordinal.as_str().to_string());

        if let Some(desc) = clip.fields.get("description").and_then(|v| v.as_str()) {
            op.description = Some(desc.to_string());
        }

        let result = run_op(&op, &kanban).await?;

        // Extract the new task ID from the result
        let new_task_id = result
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Clipboard persists after paste (don't clear it)

        Ok(serde_json::json!({
            "pasted": new_task_id,
            "from_clipboard": clip.entity_id,
        }))
    }
}

/// Delete an attachment from a task.
///
/// Available when the scope chain or args provide a task context.
/// Required args: `task_id`, `id` (attachment ID).
pub struct AttachmentDeleteCmd;

#[async_trait]
impl Command for AttachmentDeleteCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        // Available when task_id and id args are provided
        ctx.arg("task_id").is_some() && ctx.arg("id").is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let task_id = ctx.require_arg_str("task_id")?;
        let attachment_id = ctx.require_arg_str("id")?;

        let op = crate::attachment::DeleteAttachment::new(task_id, attachment_id);

        run_op(&op, &kanban).await
    }
}
