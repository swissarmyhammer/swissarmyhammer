//! Entity-level command implementations: update field, delete, tag update,
//! and attachment file operations.

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

/// Open a file with the OS default application.
///
/// Resolves the file path from the scope chain (`attachment:{path}`).
/// Uses the `open` crate for cross-platform support.
pub struct AttachmentOpenCmd;

#[async_trait]
impl Command for AttachmentOpenCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.resolve_entity_id("attachment").is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let path = ctx
            .resolve_entity_id("attachment")
            .ok_or_else(|| CommandError::MissingArg("attachment in scope chain".into()))?
            .to_string();
        tokio::task::spawn_blocking({
            let p = path.clone();
            move || open::that(&p)
        })
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?
        .map_err(|e| CommandError::ExecutionFailed(format!("failed to open {}: {}", path, e)))?;
        Ok(serde_json::json!({ "opened": path }))
    }
}

/// Reveal a file in the OS file manager.
///
/// Resolves the file path from the scope chain (`attachment:{path}`).
/// Uses platform-specific commands:
/// - macOS: `open -R <path>` (selects the file in Finder)
/// - Linux: `xdg-open <parent>` (opens the parent directory)
/// - Windows: `explorer /select,<path>` (selects the file in Explorer)
pub struct AttachmentRevealCmd;

/// Spawn the platform-specific "reveal in file manager" command.
///
/// Returns the exit status of the spawned process. Each platform uses a
/// different binary and argument convention, so we branch at compile time
/// with `#[cfg(target_os)]`.
fn reveal_in_file_manager(path: &str) -> std::io::Result<std::process::ExitStatus> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .status()
    }
    #[cfg(target_os = "linux")]
    {
        // xdg-open cannot select a specific file, so open the parent directory.
        let parent = std::path::Path::new(path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        std::process::Command::new("xdg-open").arg(parent).status()
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path))
            .status()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            format!("reveal-in-file-manager is not supported on this platform"),
        ))
    }
}

#[async_trait]
impl Command for AttachmentRevealCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.resolve_entity_id("attachment").is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let path = ctx
            .resolve_entity_id("attachment")
            .ok_or_else(|| CommandError::MissingArg("attachment in scope chain".into()))?
            .to_string();
        tokio::task::spawn_blocking({
            let p = path.clone();
            move || reveal_in_file_manager(&p)
        })
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?
        .map_err(|e| CommandError::ExecutionFailed(format!("failed to reveal {}: {}", path, e)))?;
        Ok(serde_json::json!({ "revealed": path }))
    }
}
