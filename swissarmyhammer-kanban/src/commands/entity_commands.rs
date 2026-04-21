//! Entity-level command implementations: update field, delete, tag update,
//! and attachment file operations.

use super::run_op;
use crate::context::KanbanContext;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use swissarmyhammer_commands::{parse_moniker, Command, CommandContext, CommandError};

/// Create a new entity of any type using field-default values.
///
/// This is the backend for the dynamic `entity.add:{type}` palette /
/// context-menu command. The dispatch layer rewrites `entity.add:{type}`
/// into canonical `entity.add` with `entity_type: <type>` merged into the
/// arg bag (see `match_dynamic_prefix` in `kanban-app/src/commands.rs`).
///
/// Required arg: `entity_type`. All other args are forwarded as field
/// overrides to `AddEntity`; unknown or positional-only keys are silently
/// dropped by the operation (so the frontend can supply a generic arg bag
/// without having to know which fields each entity type actually declares).
pub struct AddEntityCmd;

#[async_trait]
impl Command for AddEntityCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.arg("entity_type")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty())
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let entity_type = ctx.require_arg_str("entity_type")?.to_string();

        // Everything except `entity_type` flows through as a field
        // override. The operation itself filters out unknown / positional
        // keys — we don't need to know the entity's schema here.
        let mut overrides: HashMap<String, Value> = HashMap::new();
        for (key, value) in ctx.args.iter() {
            if key == "entity_type" {
                continue;
            }
            overrides.insert(key.clone(), value.clone());
        }

        let op = crate::entity::AddEntity::new(entity_type).with_overrides(overrides);
        run_op(&op, &kanban).await
    }
}

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

/// Entity types that opt out of the cross-cutting `entity.delete` command.
///
/// Boards are singletons managed by `file.closeBoard` / `file.newBoard` /
/// `file.openBoard` — deleting a board through a cross-cutting row-level
/// command would leave the app in an undefined state. The cross-cutting
/// emitter still creates a `ResolvedCommand` for every known entity, but
/// `DeleteEntityCmd::available()` returns false for these types so the
/// final `retain(|c| c.available)` pass in `commands_for_scope` drops
/// them from the surface.
const DELETE_OPT_OUT_TYPES: &[&str] = &["board"];

/// Entity types that opt out of the cross-cutting `entity.archive` command.
///
/// Same rationale as [`DELETE_OPT_OUT_TYPES`]: archiving a board moves the
/// board file into `.archive/` with no code path that treats the result as
/// meaningful. Dispatch would silently succeed and leave the app in an
/// undefined state, which is worse than a loud error.
const ARCHIVE_OPT_OUT_TYPES: &[&str] = &["board"];

/// Return true when the given target moniker belongs to one of the listed
/// entity types. Invalid monikers (missing colon, empty parts) return false
/// — availability is not the right place to surface parse errors; execute
/// handles that with `CommandError::InvalidMoniker`.
fn target_matches_entity_type(target: Option<&str>, types: &[&str]) -> bool {
    target
        .and_then(parse_moniker)
        .is_some_and(|(entity_type, _)| types.contains(&entity_type))
}

/// Delete any entity by its target moniker.
///
/// Available when a target moniker is set and the entity type is not in
/// [`DELETE_OPT_OUT_TYPES`]. Dispatches to the correct delete operation
/// based on the entity type parsed from the moniker.
pub struct DeleteEntityCmd;

#[async_trait]
impl Command for DeleteEntityCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.target.is_some()
            && !target_matches_entity_type(ctx.target.as_deref(), DELETE_OPT_OUT_TYPES)
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
            "project" => run_op(&crate::project::DeleteProject::new(id), &kanban).await,
            _ => Err(CommandError::ExecutionFailed(format!(
                "unknown entity type for delete: '{}'",
                entity_type
            ))),
        }
    }
}

/// Archive any entity by its target moniker.
///
/// Available when a target moniker is set, does not already carry an
/// `:archive` suffix, and the entity type is not in
/// [`ARCHIVE_OPT_OUT_TYPES`]. Dispatches to `EntityContext::archive()`
/// based on the entity type parsed from the moniker.
pub struct ArchiveEntityCmd;

#[async_trait]
impl Command for ArchiveEntityCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.target
            .as_deref()
            .is_some_and(|t| !t.ends_with(":archive"))
            && !target_matches_entity_type(ctx.target.as_deref(), ARCHIVE_OPT_OUT_TYPES)
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
        ctx.target
            .as_deref()
            .is_some_and(|t| t.ends_with(":archive"))
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let raw_moniker = ctx
            .target
            .as_deref()
            .ok_or_else(|| CommandError::MissingArg("target".into()))?;
        // Strip the ":archive" suffix added by the archive view
        let moniker = raw_moniker.strip_suffix(":archive").unwrap_or(raw_moniker);
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

/// Delete an attachment from a task.
///
/// Always available (all parameters come from args).
/// Required args: `task_id`, `id`.
pub struct AttachmentDeleteCmd;

#[async_trait]
impl Command for AttachmentDeleteCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let task_id = ctx.require_arg_str("task_id")?;
        let id = ctx.require_arg_str("id")?;
        let op = crate::attachment::DeleteAttachment::new(task_id, id);

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
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::CommandContext;
    use swissarmyhammer_operations::Execute;
    use tempfile::TempDir;

    /// Initialize a board and return a (TempDir, KanbanContext) pair.
    /// TempDir is returned to keep the temp directory alive for the duration of the test.
    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);
        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        (temp, ctx)
    }

    /// Build a CommandContext with a KanbanContext extension and optional target moniker.
    fn make_ctx(
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
    // DeleteEntityCmd availability
    // =========================================================================

    #[test]
    fn delete_entity_available_when_target_set() {
        let ctx = CommandContext::new(
            "entity.delete",
            vec![],
            Some("task:01ABC".into()),
            HashMap::new(),
        );
        let cmd = DeleteEntityCmd;
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn delete_entity_not_available_without_target() {
        let ctx = CommandContext::new("entity.delete", vec![], None, HashMap::new());
        let cmd = DeleteEntityCmd;
        assert!(!cmd.available(&ctx));
    }

    #[test]
    fn delete_entity_not_available_for_board() {
        // Boards opt out of the cross-cutting delete — see DELETE_OPT_OUT_TYPES.
        let ctx = CommandContext::new(
            "entity.delete",
            vec![],
            Some("board:main".into()),
            HashMap::new(),
        );
        let cmd = DeleteEntityCmd;
        assert!(!cmd.available(&ctx));
    }

    // =========================================================================
    // DeleteEntityCmd execute — task, tag, column, actor, unknown
    // =========================================================================

    #[tokio::test]
    async fn delete_entity_deletes_task() {
        let (_temp, ctx) = setup().await;
        let add_result = AddTask::new("To delete")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap().to_string();

        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            Some(format!("task:{}", task_id)),
            vec![],
            HashMap::new(),
        );
        let result = DeleteEntityCmd.execute(&cmd_ctx).await;
        assert!(result.is_ok(), "delete task should succeed: {:?}", result);

        // Verify the task is gone
        let ectx = kanban.entity_context().await.unwrap();
        assert!(ectx.list("task").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_entity_deletes_tag() {
        let (_temp, ctx) = setup().await;
        let add_result = AddTag::new("bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tag_id = add_result["id"].as_str().unwrap().to_string();

        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            Some(format!("tag:{}", tag_id)),
            vec![],
            HashMap::new(),
        );
        let result = DeleteEntityCmd.execute(&cmd_ctx).await;
        assert!(result.is_ok(), "delete tag should succeed: {:?}", result);
    }

    #[tokio::test]
    async fn delete_entity_deletes_column() {
        let (_temp, ctx) = setup().await;

        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            Some("column:todo".into()),
            vec![],
            HashMap::new(),
        );
        let result = DeleteEntityCmd.execute(&cmd_ctx).await;
        assert!(result.is_ok(), "delete column should succeed: {:?}", result);
    }

    #[tokio::test]
    async fn delete_entity_deletes_project() {
        let (_temp, ctx) = setup().await;
        let add_result = crate::project::AddProject::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let project_id = add_result["id"].as_str().unwrap().to_string();

        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            Some(format!("project:{}", project_id)),
            vec![],
            HashMap::new(),
        );
        let result = DeleteEntityCmd.execute(&cmd_ctx).await;
        assert!(
            result.is_ok(),
            "delete project should succeed: {:?}",
            result
        );

        // Verify the project is gone
        let ectx = kanban.entity_context().await.unwrap();
        let projects = ectx.list("project").await.unwrap();
        assert!(
            projects.iter().all(|p| p.id.as_str() != project_id),
            "project {} should be removed from entity context",
            project_id
        );
    }

    #[tokio::test]
    async fn delete_entity_deletes_actor() {
        let (_temp, ctx) = setup().await;
        let add_result = crate::actor::AddActor::new("alice", "Alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let actor_id = add_result["actor"]["id"].as_str().unwrap().to_string();

        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            Some(format!("actor:{}", actor_id)),
            vec![],
            HashMap::new(),
        );
        let result = DeleteEntityCmd.execute(&cmd_ctx).await;
        assert!(result.is_ok(), "delete actor should succeed: {:?}", result);
    }

    #[tokio::test]
    async fn delete_entity_unknown_type_returns_error() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            Some("widget:some-id".into()),
            vec![],
            HashMap::new(),
        );
        let result = DeleteEntityCmd.execute(&cmd_ctx).await;
        assert!(result.is_err(), "unknown entity type should fail");
    }

    #[tokio::test]
    async fn delete_entity_fails_without_kanban_context() {
        let ctx = CommandContext::new(
            "entity.delete",
            vec![],
            Some("task:01ABC".into()),
            HashMap::new(),
        );
        let result = DeleteEntityCmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without KanbanContext");
    }

    #[tokio::test]
    async fn delete_entity_fails_without_target() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(Arc::clone(&kanban), None, vec![], HashMap::new());
        let result = DeleteEntityCmd.execute(&cmd_ctx).await;
        assert!(result.is_err(), "should fail without target");
    }

    #[tokio::test]
    async fn delete_entity_invalid_moniker_returns_error() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        // Provide invalid moniker (no colon)
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            Some("nocolon".into()),
            vec![],
            HashMap::new(),
        );
        let result = DeleteEntityCmd.execute(&cmd_ctx).await;
        assert!(result.is_err(), "invalid moniker should fail");
    }

    // =========================================================================
    // ArchiveEntityCmd availability
    // =========================================================================

    #[test]
    fn archive_entity_available_when_target_set() {
        let ctx = CommandContext::new(
            "entity.archive",
            vec![],
            Some("task:01ABC".into()),
            HashMap::new(),
        );
        assert!(ArchiveEntityCmd.available(&ctx));
    }

    #[test]
    fn archive_entity_not_available_without_target() {
        let ctx = CommandContext::new("entity.archive", vec![], None, HashMap::new());
        assert!(!ArchiveEntityCmd.available(&ctx));
    }

    #[test]
    fn archive_entity_not_available_for_archived_entity() {
        let ctx = CommandContext::new(
            "entity.archive",
            vec![],
            Some("task:01ABC:archive".into()),
            HashMap::new(),
        );
        assert!(!ArchiveEntityCmd.available(&ctx));
    }

    #[test]
    fn archive_entity_not_available_for_board() {
        // Boards opt out of the cross-cutting archive — see ARCHIVE_OPT_OUT_TYPES.
        let ctx = CommandContext::new(
            "entity.archive",
            vec![],
            Some("board:main".into()),
            HashMap::new(),
        );
        assert!(!ArchiveEntityCmd.available(&ctx));
    }

    // =========================================================================
    // ArchiveEntityCmd execute
    // =========================================================================

    #[tokio::test]
    async fn archive_entity_archives_task() {
        let (_temp, ctx) = setup().await;
        let add_result = AddTask::new("Archive me")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap().to_string();

        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            Some(format!("task:{}", task_id)),
            vec![],
            HashMap::new(),
        );
        let result = ArchiveEntityCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["archived"], true);
        assert_eq!(result["title"].as_str().unwrap(), "Archive me");

        // Verify task is gone from live list
        let ectx = kanban.entity_context().await.unwrap();
        assert!(ectx.list("task").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn archive_entity_archives_non_task_entity() {
        let (_temp, ctx) = setup().await;
        // Use a column (non-task entity) — should call ectx.archive() directly
        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            Some("column:todo".into()),
            vec![],
            HashMap::new(),
        );
        let result = ArchiveEntityCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["archived"], true);
    }

    #[tokio::test]
    async fn archive_entity_fails_without_target() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(Arc::clone(&kanban), None, vec![], HashMap::new());
        let result = ArchiveEntityCmd.execute(&cmd_ctx).await;
        assert!(result.is_err(), "should fail without target");
    }

    #[tokio::test]
    async fn archive_entity_invalid_moniker_returns_error() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            Some("nocolon".into()),
            vec![],
            HashMap::new(),
        );
        let result = ArchiveEntityCmd.execute(&cmd_ctx).await;
        assert!(result.is_err(), "invalid moniker should fail");
    }

    // =========================================================================
    // UnarchiveEntityCmd availability
    // =========================================================================

    #[test]
    fn unarchive_entity_available_when_target_has_archived_suffix() {
        let ctx = CommandContext::new(
            "entity.unarchive",
            vec![],
            Some("task:01ABC:archive".into()),
            HashMap::new(),
        );
        assert!(UnarchiveEntityCmd.available(&ctx));
    }

    #[test]
    fn unarchive_entity_not_available_for_live_entity() {
        let ctx = CommandContext::new(
            "entity.unarchive",
            vec![],
            Some("task:01ABC".into()),
            HashMap::new(),
        );
        assert!(!UnarchiveEntityCmd.available(&ctx));
    }

    #[test]
    fn unarchive_entity_not_available_without_target() {
        let ctx = CommandContext::new("entity.unarchive", vec![], None, HashMap::new());
        assert!(!UnarchiveEntityCmd.available(&ctx));
    }

    // =========================================================================
    // UnarchiveEntityCmd execute
    // =========================================================================

    #[tokio::test]
    async fn unarchive_entity_unarchives_task() {
        let (_temp, ctx) = setup().await;
        let add_result = AddTask::new("Restore me")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap().to_string();

        // Archive the task first via the operation
        crate::task::ArchiveTask::new(task_id.as_str())
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            Some(format!("task:{}:archive", task_id)),
            vec![],
            HashMap::new(),
        );
        let result = UnarchiveEntityCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["unarchived"], true);

        // Verify task is back in the live list
        let ectx = kanban.entity_context().await.unwrap();
        assert_eq!(ectx.list("task").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn unarchive_entity_unarchives_non_task_entity() {
        let (_temp, ctx) = setup().await;
        // Archive a column first, then unarchive it
        let ectx = ctx.entity_context().await.unwrap();
        ectx.archive("column", "todo").await.unwrap();

        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            Some("column:todo:archive".into()),
            vec![],
            HashMap::new(),
        );
        let result = UnarchiveEntityCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["unarchived"], true);
    }

    #[tokio::test]
    async fn unarchive_entity_fails_without_target() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(Arc::clone(&kanban), None, vec![], HashMap::new());
        let result = UnarchiveEntityCmd.execute(&cmd_ctx).await;
        assert!(result.is_err(), "should fail without target");
    }

    // =========================================================================
    // TagUpdateCmd availability
    // =========================================================================

    #[test]
    fn tag_update_available_when_tag_in_scope() {
        let ctx = CommandContext::new("tag.update", vec!["tag:01ABC".into()], None, HashMap::new());
        assert!(TagUpdateCmd.available(&ctx));
    }

    #[test]
    fn tag_update_not_available_without_tag_scope() {
        let ctx = CommandContext::new(
            "tag.update",
            vec!["task:01ABC".into()],
            None,
            HashMap::new(),
        );
        assert!(!TagUpdateCmd.available(&ctx));
    }

    #[test]
    fn tag_update_not_available_with_empty_scope() {
        let ctx = CommandContext::new("tag.update", vec![], None, HashMap::new());
        assert!(!TagUpdateCmd.available(&ctx));
    }

    // =========================================================================
    // TagUpdateCmd execute
    // =========================================================================

    #[tokio::test]
    async fn tag_update_renames_tag() {
        let (_temp, ctx) = setup().await;
        let add_result = AddTag::new("bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tag_id = add_result["id"].as_str().unwrap().to_string();

        let kanban = Arc::new(ctx);
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("defect".into()));
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            None,
            vec![format!("tag:{}", tag_id)],
            args,
        );

        let result = TagUpdateCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["name"].as_str().unwrap(), "defect");
    }

    #[tokio::test]
    async fn tag_update_changes_color() {
        let (_temp, ctx) = setup().await;
        let add_result = AddTag::new("bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tag_id = add_result["id"].as_str().unwrap().to_string();

        let kanban = Arc::new(ctx);
        let mut args = HashMap::new();
        args.insert("color".into(), Value::String("ff0000".into()));
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            None,
            vec![format!("tag:{}", tag_id)],
            args,
        );

        let result = TagUpdateCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["color"].as_str().unwrap(), "ff0000");
    }

    #[tokio::test]
    async fn tag_update_changes_description() {
        let (_temp, ctx) = setup().await;
        let add_result = AddTag::new("bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tag_id = add_result["id"].as_str().unwrap().to_string();

        let kanban = Arc::new(ctx);
        let mut args = HashMap::new();
        args.insert("description".into(), Value::String("A known bug".into()));
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            None,
            vec![format!("tag:{}", tag_id)],
            args,
        );

        let result = TagUpdateCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["description"].as_str().unwrap(), "A known bug");
    }

    #[tokio::test]
    async fn tag_update_fails_without_tag_in_scope() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("new-name".into()));
        // No tag in scope
        let cmd_ctx = make_ctx(Arc::clone(&kanban), None, vec![], args);
        let result = TagUpdateCmd.execute(&cmd_ctx).await;
        assert!(result.is_err(), "should fail without tag in scope");
    }

    // =========================================================================
    // AttachmentOpenCmd availability
    // =========================================================================

    #[test]
    fn attachment_open_available_when_attachment_in_scope() {
        let ctx = CommandContext::new(
            "attachment.open",
            vec!["attachment:/tmp/file.txt".into()],
            None,
            HashMap::new(),
        );
        assert!(AttachmentOpenCmd.available(&ctx));
    }

    #[test]
    fn attachment_open_not_available_without_attachment_scope() {
        let ctx = CommandContext::new(
            "attachment.open",
            vec!["task:01ABC".into()],
            None,
            HashMap::new(),
        );
        assert!(!AttachmentOpenCmd.available(&ctx));
    }

    #[test]
    fn attachment_open_not_available_with_empty_scope() {
        let ctx = CommandContext::new("attachment.open", vec![], None, HashMap::new());
        assert!(!AttachmentOpenCmd.available(&ctx));
    }

    // =========================================================================
    // AttachmentRevealCmd availability
    // =========================================================================

    #[test]
    fn attachment_reveal_available_when_attachment_in_scope() {
        let ctx = CommandContext::new(
            "attachment.reveal",
            vec!["attachment:/tmp/file.txt".into()],
            None,
            HashMap::new(),
        );
        assert!(AttachmentRevealCmd.available(&ctx));
    }

    #[test]
    fn attachment_reveal_not_available_without_attachment_scope() {
        let ctx = CommandContext::new(
            "attachment.reveal",
            vec!["task:01ABC".into()],
            None,
            HashMap::new(),
        );
        assert!(!AttachmentRevealCmd.available(&ctx));
    }

    // =========================================================================
    // AttachmentOpenCmd execute — error when attachment path not in scope
    // =========================================================================

    #[tokio::test]
    async fn attachment_open_fails_without_attachment_scope() {
        let ctx = CommandContext::new("attachment.open", vec![], None, HashMap::new());
        let result = AttachmentOpenCmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without attachment in scope");
    }

    // =========================================================================
    // AttachmentRevealCmd execute — error when attachment path not in scope
    // =========================================================================

    #[tokio::test]
    async fn attachment_reveal_fails_without_attachment_scope() {
        let ctx = CommandContext::new("attachment.reveal", vec![], None, HashMap::new());
        let result = AttachmentRevealCmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without attachment in scope");
    }
}
