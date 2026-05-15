//! Entity-level command implementations: update field, delete, tag update,
//! and attachment file operations.

use super::run_op;
use crate::context::KanbanContext;
use crate::focus::resolve_focused_column;
use crate::types::{ColumnId, TaskId};
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

        // When the caller didn't supply an explicit `column`, inspect the
        // scope chain for a focused column/task and synthesize one. This
        // used to live as `resolveFocusedColumnId` in `board-view.tsx`
        // (PR #40 review: column resolution is business logic, not
        // presentation — it belongs in headless Rust). The frontend now
        // fires `entity.add:task` without a pre-computed `column`, and
        // the backend resolves it here from the scope chain the dispatch
        // already carries.
        if !overrides.contains_key("column") {
            if let Some(column) =
                resolve_column_from_scope(&kanban, &entity_type, &ctx.scope_chain).await?
            {
                overrides.insert("column".into(), Value::String(column.as_str().to_string()));
            }
        }

        let op = crate::entity::AddEntity::new(entity_type).with_overrides(overrides);
        run_op(&op, &kanban).await
    }
}

/// Resolve the focused column id implied by the scope chain, via the live
/// task → home-column map when the focus points at a task.
///
/// Delegates the pure branching logic to
/// [`crate::focus::resolve_focused_column`] (covered by headless tests in
/// `tests/resolve_focused_column.rs`); this helper owns only the storage
/// side — consulting [`KanbanContext::entity_context`] to materialize the
/// task → column map when at least one `task:*` moniker sits in the
/// chain.
///
/// Returns `None` when:
/// - the chain carries no column/task context, or
/// - `entity_type` does not declare a `position_column` field — actor,
///   tag, project, column, and board would have any synthesized column
///   silently dropped by [`crate::entity::add::AddEntity::apply_position`]
///   anyway, so we skip the resolution (and any `ectx.list("task")` I/O
///   it would incur) instead of doing the work for nothing.
///
/// In either case the caller falls through to the default placement path
/// (lowest-order column) that [`crate::entity::position::resolve_column`]
/// already owns.
async fn resolve_column_from_scope(
    kanban: &KanbanContext,
    entity_type: &str,
    scope_chain: &[String],
) -> swissarmyhammer_commands::Result<Option<ColumnId>> {
    // Short-circuit: if the chain has no column/task monikers at all, skip
    // the entity-context read entirely. Keeps the dispatch path cheap for
    // scopes like `[window:main]` (the "nothing focused" case).
    let needs_task_map = scope_chain
        .iter()
        .any(|m| m.starts_with("task:") || m.starts_with("column:"));
    if !needs_task_map {
        return Ok(None);
    }

    // Skip the resolution entirely for entity types that don't declare a
    // `position_column` field (actor, tag, project, column, board…). Their
    // [`AddEntity::apply_position`] path silently drops any synthesized
    // `column` override via the same `has_position_column` check, so the
    // resolver — and especially the `ectx.list("task")` round-trip below —
    // would be paying for a value the operation throws away.
    let ectx = kanban
        .entity_context()
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
    let entity_def = match ectx.fields().get_entity(entity_type) {
        Some(def) => def,
        // Unknown entity type: let `AddEntity` surface the "unknown entity
        // type" error during execute() rather than masking it here.
        None => return Ok(None),
    };
    let has_position_column = entity_def
        .fields
        .iter()
        .any(|f| f.as_str() == crate::entity::position::POSITION_COLUMN_FIELD);
    if !has_position_column {
        return Ok(None);
    }

    // Only materialize the task → column map when a `task:*` moniker sits
    // in the chain — a `column:*`-only chain resolves without touching
    // entity storage, so walking `ectx.list("task")` would be wasted I/O.
    let needs_task_lookup = scope_chain.iter().any(|m| m.starts_with("task:"));
    let task_to_column = if needs_task_lookup {
        let tasks = ectx
            .list("task")
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
        tasks
            .iter()
            .filter_map(|t| {
                let col = t.get_str("position_column")?;
                Some((
                    TaskId::from_string(t.id.as_str()),
                    ColumnId::from_string(col),
                ))
            })
            .collect::<HashMap<TaskId, ColumnId>>()
    } else {
        HashMap::new()
    };

    Ok(resolve_focused_column(scope_chain, &task_to_column))
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
            // Attachments live as a multi-value field on their parent task;
            // the dispatch path walks the scope chain innermost-first to find
            // the owning task. When the user right-clicks an attachment chip
            // the scope chain is `[attachment:<id>, task:<id>, column:<id>, ...]`,
            // so `resolve_entity_id("task")` picks up the correct parent. If no
            // task is in scope (e.g. a bare `entity.delete` against a raw
            // attachment moniker with no surrounding context) we surface a
            // loud error rather than guess.
            "attachment" => {
                let task_id = ctx.resolve_entity_id("task").ok_or_else(|| {
                    CommandError::ExecutionFailed(
                        "attachment delete requires a task in scope".into(),
                    )
                })?;
                run_op(
                    &crate::attachment::DeleteAttachment::new(task_id, id),
                    &kanban,
                )
                .await
            }
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
    // DeleteEntityCmd execute — attachment (folded from retired
    // `attachment.delete`). The dispatch path resolves the parent task via
    // the scope chain (`resolve_entity_id("task")`).
    // =========================================================================

    /// End-to-end: right-clicking an attachment chip inside a task produces a
    /// scope chain `[attachment:<path>, task:<tid>, column:<cid>]` where the
    /// attachment moniker carries the absolute filesystem path emitted by
    /// the frontend (`attachment:${attachment.path}`). Firing
    /// `entity.delete` against that target must remove the attachment from
    /// the parent task's attachments list.
    #[tokio::test]
    async fn delete_entity_deletes_attachment_via_scope_chain() {
        let (temp, ctx) = setup().await;

        let task_result = crate::task::AddTask::new("Has attachments")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        let source_file = temp.path().join("note.txt");
        std::fs::write(&source_file, b"hello").unwrap();
        crate::attachment::AddAttachment::new(
            task_id.as_str(),
            "note.txt",
            source_file.to_string_lossy().to_string(),
        )
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

        // Resolve the attachment's stored path — the same value the
        // frontend embeds in the `attachment:${path}` moniker.
        let ectx = ctx.entity_context().await.unwrap();
        let task = ectx.read("task", task_id.as_str()).await.unwrap();
        let attachment_path = task
            .get("attachments")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|entry| entry.get("path"))
            .and_then(|v| v.as_str())
            .expect("attachment path should be present after add")
            .to_string();

        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            Some(format!("attachment:{}", attachment_path)),
            vec![
                format!("attachment:{}", attachment_path),
                format!("task:{}", task_id),
                "column:todo".into(),
            ],
            HashMap::new(),
        );

        let result = DeleteEntityCmd.execute(&cmd_ctx).await;
        assert!(
            result.is_ok(),
            "delete attachment should succeed: {:?}",
            result
        );

        // Verify the attachment is removed from the task's list.
        let ectx = kanban.entity_context().await.unwrap();
        let task = ectx.read("task", task_id.as_str()).await.unwrap();
        let attachments = task.get("attachments");
        let is_empty = attachments.is_none()
            || attachments.unwrap().is_null()
            || attachments.unwrap().as_array().is_none_or(|a| a.is_empty());
        assert!(
            is_empty,
            "task attachments list should be empty after delete, got: {:?}",
            attachments
        );
    }

    /// Without a `task:` moniker in the scope chain there is no parent to
    /// look up, so the dispatch should fail loudly rather than silently
    /// succeed or guess.
    #[tokio::test]
    async fn delete_entity_attachment_missing_task_in_scope_errors() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            Some("attachment:some-id".into()),
            // Scope chain deliberately has no task: moniker.
            vec!["attachment:some-id".into(), "column:todo".into()],
            HashMap::new(),
        );
        let result = DeleteEntityCmd.execute(&cmd_ctx).await;
        match result {
            Err(CommandError::ExecutionFailed(msg)) => {
                assert!(
                    msg.contains("requires a task in scope"),
                    "expected 'requires a task in scope' error, got: {msg}"
                );
            }
            other => panic!(
                "expected ExecutionFailed 'requires a task in scope', got: {:?}",
                other
            ),
        }
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

    // =========================================================================
    // AddEntityCmd — scope-chain driven column resolution
    //
    // These tests assert that `AddEntityCmd` (the backend for the dynamic
    // `entity.add:task` command surfaced in the palette/context menu)
    // resolves the target column from the scope chain when the caller
    // didn't supply an explicit `column` arg. This is the Rust replacement
    // for the React `resolveFocusedColumnId` helper — see PR #40 review.
    // =========================================================================

    /// When the scope chain carries `column:<id>`, a bare `entity.add:task`
    /// (no explicit `column` arg) must land the task in that column. This
    /// is the "column focused, New Task pressed" path.
    #[tokio::test]
    async fn add_entity_task_uses_column_from_scope_chain() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        let mut args = HashMap::new();
        args.insert("entity_type".into(), Value::String("task".into()));

        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            None,
            vec!["column:doing".to_string(), "window:main".to_string()],
            args,
        );

        let result = AddEntityCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            result["position_column"], "doing",
            "focused column in scope chain must win over default placement"
        );
    }

    /// When the scope chain carries `task:<tid>`, a bare `entity.add:task`
    /// must land the new task in the focused task's home column. This is
    /// the "task focused, New Task pressed" path — the resolver consults
    /// the live task → column map and pulls the focused task's column.
    #[tokio::test]
    async fn add_entity_task_uses_focused_tasks_column() {
        let (_temp, ctx) = setup().await;

        // Seed an existing task in "doing" so the scope-chain lookup has
        // a real task to resolve against.
        let mut add = AddTask::new("Existing");
        add.column = Some("doing".to_string());
        let existing = add.execute(&ctx).await.into_result().unwrap();
        let focused_task_id = existing["id"].as_str().unwrap().to_string();

        let kanban = Arc::new(ctx);

        let mut args = HashMap::new();
        args.insert("entity_type".into(), Value::String("task".into()));

        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            None,
            vec![
                format!("task:{}", focused_task_id),
                "column:todo".to_string(),
                "window:main".to_string(),
            ],
            args,
        );

        let result = AddEntityCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            result["position_column"], "doing",
            "focused task's home column must win over the outer column scope"
        );
    }

    /// When the scope chain has no column/task moniker, `AddEntityCmd`
    /// falls through to the default placement path — lowest-order column.
    /// This is the "palette invoked with nothing focused" case.
    #[tokio::test]
    async fn add_entity_task_defaults_to_lowest_order_column_without_focus() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        let mut args = HashMap::new();
        args.insert("entity_type".into(), Value::String("task".into()));

        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            None,
            vec!["window:main".to_string()],
            args,
        );

        let result = AddEntityCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            result["position_column"], "todo",
            "no column/task in scope → lowest-order column"
        );
    }

    /// An explicit `column` arg supplied at dispatch time still wins over
    /// the scope-chain resolution, preserving backward compatibility for
    /// callers (like the grid-view `onAddTask` handler) that compute the
    /// column from domain state rather than focus.
    #[tokio::test]
    async fn add_entity_task_explicit_column_arg_wins_over_scope() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        let mut args = HashMap::new();
        args.insert("entity_type".into(), Value::String("task".into()));
        args.insert("column".into(), Value::String("doing".into()));

        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            None,
            // Scope chain says "todo" via `column:todo`, but the explicit
            // arg overrides it.
            vec!["column:todo".to_string(), "window:main".to_string()],
            args,
        );

        let result = AddEntityCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            result["position_column"], "doing",
            "explicit column arg must override scope-chain resolution"
        );
    }

    /// When the scope chain's only `task:` moniker names a task that
    /// doesn't exist (e.g. stale scope from a deleted task), the resolver
    /// commits to the task lookup, misses the map, and returns `None`.
    /// `AddEntityCmd` then falls through to the lowest-order column
    /// default rather than silently picking up some outer `column:*`.
    #[tokio::test]
    async fn add_entity_task_unknown_task_in_scope_falls_back_to_default() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        let mut args = HashMap::new();
        args.insert("entity_type".into(), Value::String("task".into()));

        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            None,
            vec![
                "task:does-not-exist".to_string(),
                "column:doing".to_string(),
                "window:main".to_string(),
            ],
            args,
        );

        let result = AddEntityCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            result["position_column"], "todo",
            "unknown task moniker must not fall through to an outer column scope"
        );
    }

    /// `entity.add:<non-positional>` (actor, tag, project, column, board) must
    /// succeed when the scope chain carries a `column:*` or `task:*` moniker,
    /// and the resulting entity must have no `position_column` field. The
    /// resolver gates on `EntityDef.fields.contains(POSITION_COLUMN_FIELD)`
    /// and short-circuits before fetching the task map, so this also serves
    /// as a regression for the no-extra-I/O contract called out in the PR
    /// review's perf nit. If someone ever adds `position_column` to actor,
    /// this test will start asserting against a populated value and surface
    /// the contract change.
    #[tokio::test]
    async fn add_entity_non_positional_drops_synthesized_column_silently() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        let mut args = HashMap::new();
        args.insert("entity_type".into(), Value::String("tag".into()));

        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            None,
            // Scope chain implies a focused column, but `tag` doesn't declare
            // `position_column` — the resolver must skip itself entirely and
            // the resulting tag must carry no column field.
            vec!["column:doing".to_string(), "window:main".to_string()],
            args,
        );

        let result = AddEntityCmd.execute(&cmd_ctx).await.unwrap();
        assert!(
            result.get("position_column").is_none() || result["position_column"].is_null(),
            "non-positional entity must not receive a synthesized position_column \
             from the scope chain, got: {:?}",
            result.get("position_column")
        );
        // Sanity: the tag was actually created with the schema default.
        assert_eq!(result["tag_name"], "new-tag");
    }
}
