//! Entity-level command implementations: update field, delete, tag update,
//! attachment delete.

use super::run_op;
use crate::context::KanbanContext;
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
