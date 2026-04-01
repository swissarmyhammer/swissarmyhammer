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

        // For tasks, dispatch to UnarchiveTask which goes through the operation
        // processor for proper transaction/changelog support (enables undo/redo).
        // For other entity types, call EntityContext::unarchive() directly.
        if entity_type == "task" {
            return run_op(&crate::task::UnarchiveTask::new(id), &kanban).await;
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::context::KanbanContext;
    use crate::tag::AddTag;
    use crate::task::AddTask;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::{Command, CommandContext};
    use swissarmyhammer_operations::Execute;
    use tempfile::TempDir;

    /// Initialize a board and return the temp dir + shared KanbanContext.
    async fn setup() -> (TempDir, Arc<KanbanContext>) {
        let temp = TempDir::new().unwrap();
        let kanban = Arc::new(KanbanContext::new(temp.path().join(".kanban")));
        InitBoard::new("Test")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        (temp, kanban)
    }

    /// Build a CommandContext with a target moniker, optional scope chain, and args.
    fn make_ctx_with_target(
        kanban: Arc<KanbanContext>,
        target: Option<String>,
        scope: Vec<String>,
        args: HashMap<String, Value>,
    ) -> CommandContext {
        let mut ctx = CommandContext::new("test", scope, target, args);
        ctx.set_extension(kanban);
        ctx
    }

    // =========================================================================
    // DeleteEntityCmd
    // =========================================================================

    #[tokio::test]
    async fn delete_entity_available_when_target_set() {
        let ctx = CommandContext::new("test", vec![], Some("task:01X".into()), HashMap::new());
        assert!(DeleteEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn delete_entity_not_available_without_target() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        assert!(!DeleteEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn delete_entity_removes_task() {
        let (_temp, kanban) = setup().await;

        // Create a task
        let result = AddTask::new("To delete")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = result["id"].as_str().unwrap();

        // Delete it via DeleteEntityCmd
        let ctx = make_ctx_with_target(
            Arc::clone(&kanban),
            Some(format!("task:{task_id}")),
            vec![],
            HashMap::new(),
        );
        let del_result = DeleteEntityCmd.execute(&ctx).await;
        assert!(del_result.is_ok(), "delete should succeed");

        // Verify it's gone
        let list = crate::task::ListTasks::new()
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let tasks = list["tasks"].as_array().unwrap();
        assert!(
            !tasks.iter().any(|t| t["id"].as_str() == Some(task_id)),
            "deleted task should not appear in list"
        );
    }

    #[tokio::test]
    async fn delete_entity_removes_tag() {
        let (_temp, kanban) = setup().await;

        let result = AddTag::new("bug")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let tag_id = result["id"].as_str().unwrap();

        let ctx = make_ctx_with_target(
            Arc::clone(&kanban),
            Some(format!("tag:{tag_id}")),
            vec![],
            HashMap::new(),
        );
        let del_result = DeleteEntityCmd.execute(&ctx).await;
        assert!(del_result.is_ok(), "delete tag should succeed");
    }

    #[tokio::test]
    async fn delete_entity_unknown_type_errors() {
        let (_temp, kanban) = setup().await;

        let ctx = make_ctx_with_target(
            Arc::clone(&kanban),
            Some("widget:123".into()),
            vec![],
            HashMap::new(),
        );
        let result = DeleteEntityCmd.execute(&ctx).await;
        assert!(result.is_err(), "unknown entity type should error");
    }

    // =========================================================================
    // ArchiveEntityCmd
    // =========================================================================

    #[tokio::test]
    async fn archive_entity_available_when_target_set() {
        let ctx = CommandContext::new("test", vec![], Some("task:01X".into()), HashMap::new());
        assert!(ArchiveEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn archive_entity_not_available_without_target() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        assert!(!ArchiveEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn archive_task_via_command() {
        let (_temp, kanban) = setup().await;

        let result = AddTask::new("To archive")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = result["id"].as_str().unwrap();

        let ctx = make_ctx_with_target(
            Arc::clone(&kanban),
            Some(format!("task:{task_id}")),
            vec![],
            HashMap::new(),
        );
        let archive_result = ArchiveEntityCmd.execute(&ctx).await;
        assert!(archive_result.is_ok(), "archive should succeed");

        // Task should no longer appear in active list
        let list = crate::task::ListTasks::new()
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let tasks = list["tasks"].as_array().unwrap();
        assert!(
            !tasks.iter().any(|t| t["id"].as_str() == Some(task_id)),
            "archived task should not appear in active list"
        );
    }

    // =========================================================================
    // UnarchiveEntityCmd
    // =========================================================================

    #[tokio::test]
    async fn unarchive_entity_available_when_target_set() {
        let ctx = CommandContext::new("test", vec![], Some("task:01X".into()), HashMap::new());
        assert!(UnarchiveEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn unarchive_entity_not_available_without_target() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        assert!(!UnarchiveEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn unarchive_restores_task() {
        let (_temp, kanban) = setup().await;

        // Create and archive a task
        let result = AddTask::new("Round trip")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = result["id"].as_str().unwrap();

        // Archive it
        let ctx = make_ctx_with_target(
            Arc::clone(&kanban),
            Some(format!("task:{task_id}")),
            vec![],
            HashMap::new(),
        );
        ArchiveEntityCmd.execute(&ctx).await.unwrap();

        // Unarchive it
        let ctx = make_ctx_with_target(
            Arc::clone(&kanban),
            Some(format!("task:{task_id}")),
            vec![],
            HashMap::new(),
        );
        let unarchive_result = UnarchiveEntityCmd.execute(&ctx).await;
        assert!(unarchive_result.is_ok(), "unarchive should succeed");

        // Task should re-appear in active list
        let list = crate::task::ListTasks::new()
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let tasks = list["tasks"].as_array().unwrap();
        assert!(
            tasks.iter().any(|t| t["id"].as_str() == Some(task_id)),
            "unarchived task should be back in active list"
        );
    }

    // =========================================================================
    // TagUpdateCmd
    // =========================================================================

    #[tokio::test]
    async fn tag_update_available_with_tag_in_scope() {
        let ctx = CommandContext::new("test", vec!["tag:01X".into()], None, HashMap::new());
        assert!(TagUpdateCmd.available(&ctx));
    }

    #[tokio::test]
    async fn tag_update_not_available_without_tag_scope() {
        let ctx = CommandContext::new("test", vec!["task:01X".into()], None, HashMap::new());
        assert!(!TagUpdateCmd.available(&ctx));
    }

    #[tokio::test]
    async fn tag_update_modifies_name() {
        let (_temp, kanban) = setup().await;

        let result = AddTag::new("bug")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let tag_id = result["id"].as_str().unwrap();

        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("defect".into()));

        let ctx = make_ctx_with_target(
            Arc::clone(&kanban),
            None,
            vec![format!("tag:{tag_id}")],
            args,
        );
        let update_result = TagUpdateCmd.execute(&ctx).await;
        assert!(update_result.is_ok(), "tag update should succeed");

        // Verify the tag was renamed
        let tag = crate::tag::GetTag::new(tag_id)
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        assert_eq!(tag["name"].as_str(), Some("defect"));
    }

    #[tokio::test]
    async fn tag_update_modifies_color() {
        let (_temp, kanban) = setup().await;

        let result = AddTag::new("feature")
            .with_color("00ff00")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let tag_id = result["id"].as_str().unwrap();

        let mut args = HashMap::new();
        args.insert("color".into(), Value::String("ff0000".into()));

        let ctx = make_ctx_with_target(
            Arc::clone(&kanban),
            None,
            vec![format!("tag:{tag_id}")],
            args,
        );
        TagUpdateCmd.execute(&ctx).await.unwrap();

        let tag = crate::tag::GetTag::new(tag_id)
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        assert_eq!(tag["color"].as_str(), Some("ff0000"));
    }

    #[tokio::test]
    async fn tag_update_modifies_description() {
        let (_temp, kanban) = setup().await;

        let result = AddTag::new("urgent")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let tag_id = result["id"].as_str().unwrap();

        let mut args = HashMap::new();
        args.insert(
            "description".into(),
            Value::String("High priority items".into()),
        );

        let ctx = make_ctx_with_target(
            Arc::clone(&kanban),
            None,
            vec![format!("tag:{tag_id}")],
            args,
        );
        TagUpdateCmd.execute(&ctx).await.unwrap();

        let tag = crate::tag::GetTag::new(tag_id)
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        assert_eq!(tag["description"].as_str(), Some("High priority items"));
    }

    // =========================================================================
    // AttachmentDeleteCmd
    // =========================================================================

    #[tokio::test]
    async fn attachment_delete_available_with_both_args() {
        let mut args = HashMap::new();
        args.insert("task_id".into(), Value::String("01T".into()));
        args.insert("id".into(), Value::String("01A".into()));

        let ctx = CommandContext::new("test", vec![], None, args);
        assert!(AttachmentDeleteCmd.available(&ctx));
    }

    #[tokio::test]
    async fn attachment_delete_not_available_missing_id() {
        let mut args = HashMap::new();
        args.insert("task_id".into(), Value::String("01T".into()));

        let ctx = CommandContext::new("test", vec![], None, args);
        assert!(!AttachmentDeleteCmd.available(&ctx));
    }

    #[tokio::test]
    async fn attachment_delete_not_available_missing_task_id() {
        let mut args = HashMap::new();
        args.insert("id".into(), Value::String("01A".into()));

        let ctx = CommandContext::new("test", vec![], None, args);
        assert!(!AttachmentDeleteCmd.available(&ctx));
    }

    #[tokio::test]
    async fn attachment_delete_removes_attachment() {
        let (_temp, kanban) = setup().await;

        // Create a task
        let task_result = AddTask::new("With attachment")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Add an attachment
        let att_result =
            crate::attachment::AddAttachment::new(task_id, "file.txt", "/tmp/file.txt")
                .execute(kanban.as_ref())
                .await
                .into_result()
                .unwrap();
        let att_id = att_result["attachment"]["id"].as_str().unwrap();

        // Delete via AttachmentDeleteCmd
        let mut args = HashMap::new();
        args.insert("task_id".into(), Value::String(task_id.into()));
        args.insert("id".into(), Value::String(att_id.into()));

        let ctx = make_ctx_with_target(Arc::clone(&kanban), None, vec![], args);
        let del_result = AttachmentDeleteCmd.execute(&ctx).await;
        assert!(del_result.is_ok(), "attachment delete should succeed");

        // Verify attachment is gone
        let list = crate::attachment::ListAttachments::new(task_id)
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let attachments = list["attachments"].as_array().unwrap();
        assert!(
            !attachments.iter().any(|a| a["id"].as_str() == Some(att_id)),
            "deleted attachment should not appear in list"
        );
    }
}
