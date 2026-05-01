//! Polymorphic clipboard command implementations: copy, cut, paste.
//!
//! All three commands are cross-cutting: their primary param is
//! `from: target`, so the scope_commands emitter fires them once per
//! entity moniker in the scope chain. Each command parses `ctx.target`
//! into an `(entity_type, id)` pair and dispatches polymorphically —
//! copy snapshots the entity's fields via `EntityContext::read`, cut
//! delegates to entity-specific destructive operations (task delete,
//! tag untag), and paste walks a `PasteMatrix` of
//! `(clipboard_type, target_type)` handlers.
//!
//! Known entity types: task, tag, column, board, actor, project,
//! attachment. Copy is available for any known type. Cut is available
//! for types that have a destructive operation defined (task, tag);
//! other types fall through to "not available" rather than faking
//! delete semantics the UI never exercised.

use super::paste_handlers::{register_paste_handlers, PasteMatrix};
use super::run_op;
use crate::clipboard::{self, ClipboardProviderExt};
use crate::context::KanbanContext;
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{parse_moniker, Command, CommandContext, CommandError};

/// Entity types that have a known copy path (generic via
/// `EntityContext::read`). Must stay in sync with the entity definitions
/// under `swissarmyhammer-kanban/builtin/entities/*.yaml`.
const COPYABLE_ENTITY_TYPES: &[&str] = &[
    "task",
    "tag",
    "column",
    "board",
    "actor",
    "project",
    "attachment",
];

/// Check whether a target moniker names a known entity type that can be
/// copied. Returns `true` when the moniker parses and the entity type is
/// in `COPYABLE_ENTITY_TYPES`; `false` otherwise (including when the
/// target is `None` or contains an `:archive` suffix).
fn target_is_copyable(target: Option<&str>) -> bool {
    let Some(t) = target else {
        return false;
    };
    // Archive-view monikers (e.g. `task:X:archive`) are a read-only lens;
    // copy/cut are not offered on archived entities.
    if t.ends_with(":archive") {
        return false;
    }
    parse_moniker(t)
        .map(|(entity_type, _)| COPYABLE_ENTITY_TYPES.contains(&entity_type))
        .unwrap_or(false)
}

/// Write clipboard JSON to the system clipboard provider and flag the UI
/// state with the copied entity's type so availability guards can gate
/// paste correctly.
async fn write_to_clipboard(
    ctx: &CommandContext,
    clipboard_json: &str,
    entity_type: &str,
) -> swissarmyhammer_commands::Result<()> {
    if let Ok(clipboard) = ctx.require_extension::<ClipboardProviderExt>() {
        clipboard
            .0
            .write_text(clipboard_json)
            .await
            .map_err(|e| CommandError::ExecutionFailed(format!("clipboard write failed: {e}")))?;
    }
    if let Some(ref ui) = ctx.ui_state {
        ui.set_clipboard_entity_type(entity_type);
    }
    Ok(())
}

/// Snapshot the `entity_type:id` entity's fields as a clipboard JSON
/// string. Used by both copy and cut execution paths — both need the
/// same payload shape, they only differ in whether the source is also
/// deleted afterwards.
async fn snapshot_entity_to_clipboard(
    kanban: &KanbanContext,
    entity_type: &str,
    entity_id: &str,
    mode: &str,
) -> swissarmyhammer_commands::Result<String> {
    let ectx = kanban
        .entity_context()
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
    let entity = ectx
        .read(entity_type, entity_id)
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
    let fields = serde_json::to_value(&entity.fields)
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
    Ok(clipboard::serialize_to_clipboard(
        entity_type,
        entity_id,
        mode,
        fields,
    ))
}

/// Copy the targeted entity to the system clipboard.
///
/// Cross-cutting command: reads `ctx.target`, parses the entity type and
/// id, snapshots all fields via `EntityContext::read`, and writes the
/// clipboard-format JSON to the system clipboard provider. Works
/// polymorphically for any known entity type — the copy itself is a
/// read-only operation, so entity-specific logic is unnecessary.
pub struct CopyEntityCmd;

#[async_trait]
impl Command for CopyEntityCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        target_is_copyable(ctx.target.as_deref())
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let moniker = ctx
            .target
            .as_deref()
            .ok_or_else(|| CommandError::MissingArg("target".into()))?;
        let (entity_type, entity_id) =
            parse_moniker(moniker).ok_or_else(|| CommandError::InvalidMoniker(moniker.into()))?;

        if !COPYABLE_ENTITY_TYPES.contains(&entity_type) {
            return Err(CommandError::ExecutionFailed(format!(
                "unknown entity type for copy: '{entity_type}'"
            )));
        }

        let clipboard_json =
            snapshot_entity_to_clipboard(&kanban, entity_type, entity_id, "copy").await?;
        write_to_clipboard(ctx, &clipboard_json, entity_type).await?;

        Ok(serde_json::json!({
            "copied": true,
            "id": entity_id,
            "entity_type": entity_type,
            "clipboard_json": clipboard_json,
        }))
    }
}

/// Cut the targeted entity: copy it to the clipboard, then run the
/// entity-specific destructive operation.
///
/// Cross-cutting command: reads `ctx.target` to identify the entity.
/// Dispatches on entity type — task and tag have dedicated cut
/// operations (delete the task / untag it from the task in scope).
/// Other entity types have no destructive cut semantics defined and
/// return `available() == false` rather than pretend to support an
/// operation the UI never surfaced.
pub struct CutEntityCmd;

#[async_trait]
impl Command for CutEntityCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        let Some(target) = ctx.target.as_deref() else {
            return false;
        };
        if target.ends_with(":archive") {
            return false;
        }
        let Some((entity_type, _)) = parse_moniker(target) else {
            return false;
        };
        match entity_type {
            "task" => true,
            // Cutting a tag means "untag this tag from the task that
            // shares the scope chain". Without a task in scope there is
            // no destructive operation to perform.
            "tag" => ctx.has_in_scope("task"),
            _ => false,
        }
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let moniker = ctx
            .target
            .as_deref()
            .ok_or_else(|| CommandError::MissingArg("target".into()))?;
        let (entity_type, entity_id) =
            parse_moniker(moniker).ok_or_else(|| CommandError::InvalidMoniker(moniker.into()))?;

        let (result, ui_entity_type) = match entity_type {
            "task" => {
                let op = crate::task::CutTask::new(entity_id);
                (run_op(&op, &kanban).await?, "task")
            }
            "tag" => {
                let task_id = ctx
                    .resolve_entity_id("task")
                    .ok_or_else(|| CommandError::MissingScope("task".into()))?;
                // Resolve the tag slug so CutTag can re-locate the tag
                // entity by name — it keys off the slug found on the
                // task body, not the entity id.
                let ectx = kanban
                    .entity_context()
                    .await
                    .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
                let tag_entity = ectx
                    .read("tag", entity_id)
                    .await
                    .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
                let tag_name = tag_entity
                    .get_str("tag_name")
                    .unwrap_or(entity_id)
                    .to_string();
                let op = crate::tag::CutTag::new(task_id, tag_name);
                (run_op(&op, &kanban).await?, "tag")
            }
            other => {
                return Err(CommandError::ExecutionFailed(format!(
                    "cut is not supported for entity type '{other}'"
                )));
            }
        };

        if let Some(clipboard_json) = result["clipboard_json"].as_str() {
            write_to_clipboard(ctx, clipboard_json, ui_entity_type).await?;
        }

        Ok(result)
    }
}

/// Paste whatever is on the clipboard onto the targeted entity.
///
/// Cross-cutting command: reads `ctx.target` to pick the destination
/// moniker, reads `UIState::clipboard_entity_type()` (for availability
/// gating) and the clipboard text (for execution). Dispatches through a
/// [`PasteMatrix`] keyed by `(clipboard_type, target_type)`. Registers
/// the production matrix lazily on first use via `once_cell` so callers
/// don't have to plumb a matrix through context.
pub struct PasteEntityCmd {
    matrix: PasteMatrix,
}

impl PasteEntityCmd {
    /// Create a PasteEntityCmd with the production paste-handler matrix.
    pub fn new() -> Self {
        Self {
            matrix: register_paste_handlers(),
        }
    }
}

impl Default for PasteEntityCmd {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for PasteEntityCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        let Some(ui) = ctx.ui_state.as_ref() else {
            return false;
        };
        if !ui.has_clipboard() {
            return false;
        }
        let Some(clipboard_type) = ui.clipboard_entity_type() else {
            return false;
        };
        let Some(target) = ctx.target.as_deref() else {
            return false;
        };
        let Some((target_type, _)) = parse_moniker(target) else {
            return false;
        };
        self.matrix.find(&clipboard_type, target_type).is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let moniker = ctx
            .target
            .as_deref()
            .ok_or_else(|| CommandError::MissingArg("target".into()))?;
        let (target_type, _) =
            parse_moniker(moniker).ok_or_else(|| CommandError::InvalidMoniker(moniker.into()))?;

        // Read clipboard text and peek at entity_type to find the handler.
        let clipboard_ext = ctx.require_extension::<ClipboardProviderExt>()?;
        let clipboard_text = clipboard_ext
            .0
            .read_text()
            .await
            .map_err(|e| CommandError::ExecutionFailed(format!("clipboard read failed: {e}")))?
            .ok_or_else(|| CommandError::ExecutionFailed("clipboard is empty".into()))?;

        let payload = clipboard::deserialize_from_clipboard(&clipboard_text).ok_or_else(|| {
            CommandError::ExecutionFailed(
                "clipboard does not contain a swissarmyhammer payload".into(),
            )
        })?;
        let clipboard_type = payload.swissarmyhammer_clipboard.entity_type.clone();

        let handler = self
            .matrix
            .find(&clipboard_type, target_type)
            .ok_or_else(|| {
                CommandError::ExecutionFailed(format!(
                    "no paste handler for clipboard type '{clipboard_type}' onto target type \
                     '{target_type}'"
                ))
            })?;

        handler.execute(&payload, moniker, ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::AddActor;
    use crate::board::InitBoard;
    use crate::clipboard::{ClipboardProviderExt, InMemoryClipboard};
    use crate::project::AddProject;
    use crate::tag::AddTag;
    use crate::task::AddTask;
    use crate::Execute;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::UIState;

    async fn setup() -> (
        tempfile::TempDir,
        Arc<KanbanContext>,
        Arc<ClipboardProviderExt>,
        Arc<UIState>,
    ) {
        let temp = tempfile::TempDir::new().unwrap();
        let kanban = Arc::new(KanbanContext::new(temp.path().join(".kanban")));
        InitBoard::new("Test")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let clipboard = Arc::new(ClipboardProviderExt(Arc::new(InMemoryClipboard::new())));
        let ui = Arc::new(UIState::new());
        (temp, kanban, clipboard, ui)
    }

    fn make_ctx(
        command_id: &str,
        scope: &[&str],
        target: Option<&str>,
        kanban: &Arc<KanbanContext>,
        clipboard: &Arc<ClipboardProviderExt>,
        ui: &Arc<UIState>,
    ) -> CommandContext {
        let mut ctx = CommandContext::new(
            command_id,
            scope.iter().map(|s| s.to_string()).collect(),
            target.map(|s| s.to_string()),
            HashMap::new(),
        );
        ctx.set_extension(Arc::clone(kanban));
        ctx.set_extension(Arc::clone(clipboard));
        ctx.ui_state = Some(Arc::clone(ui));
        ctx
    }

    // =========================================================================
    // Copy availability — driven by `target`
    // =========================================================================

    #[tokio::test]
    async fn copy_available_with_task_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx(
            "entity.copy",
            &[],
            Some("task:01X"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(CopyEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn copy_available_with_tag_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx(
            "entity.copy",
            &[],
            Some("tag:01X"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(CopyEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn copy_available_with_project_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx(
            "entity.copy",
            &[],
            Some("project:foo"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(CopyEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn copy_available_with_column_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx(
            "entity.copy",
            &[],
            Some("column:todo"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(CopyEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn copy_available_with_actor_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx(
            "entity.copy",
            &[],
            Some("actor:alice"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(CopyEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn copy_not_available_without_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx("entity.copy", &[], None, &kanban, &clipboard, &ui);
        assert!(!CopyEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn copy_not_available_for_unknown_type() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx(
            "entity.copy",
            &[],
            Some("widget:foo"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(!CopyEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn copy_not_available_for_archive_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx(
            "entity.copy",
            &[],
            Some("task:01X:archive"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(!CopyEntityCmd.available(&ctx));
    }

    // =========================================================================
    // Copy execution — works on any known entity type via target
    // =========================================================================

    #[tokio::test]
    async fn copy_entity_works_on_tag_via_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let add = AddTag::new("bug")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let tag_id = add["id"].as_str().unwrap();

        let ctx = make_ctx(
            "entity.copy",
            &[],
            Some(&format!("tag:{tag_id}")),
            &kanban,
            &clipboard,
            &ui,
        );
        let result = CopyEntityCmd.execute(&ctx).await.unwrap();
        assert_eq!(result["copied"], true);
        assert_eq!(result["entity_type"], "tag");
        assert_eq!(result["id"], tag_id);

        assert!(ui.has_clipboard());
        assert_eq!(ui.clipboard_entity_type().as_deref(), Some("tag"));

        let text = clipboard.0.read_text().await.unwrap().unwrap();
        let payload = clipboard::deserialize_from_clipboard(&text).unwrap();
        assert_eq!(payload.swissarmyhammer_clipboard.entity_type, "tag");
        assert_eq!(payload.swissarmyhammer_clipboard.entity_id, tag_id);
        assert_eq!(
            payload.swissarmyhammer_clipboard.fields["tag_name"], "bug",
            "clipboard payload must include the tag's fields"
        );
    }

    #[tokio::test]
    async fn copy_entity_works_on_task_via_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let add = AddTask::new("My task")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = add["id"].as_str().unwrap();

        let ctx = make_ctx(
            "entity.copy",
            &[],
            Some(&format!("task:{task_id}")),
            &kanban,
            &clipboard,
            &ui,
        );
        let result = CopyEntityCmd.execute(&ctx).await.unwrap();
        assert_eq!(result["copied"], true);
        assert_eq!(result["entity_type"], "task");

        let text = clipboard.0.read_text().await.unwrap().unwrap();
        let payload = clipboard::deserialize_from_clipboard(&text).unwrap();
        assert_eq!(payload.swissarmyhammer_clipboard.entity_type, "task");
        assert_eq!(payload.swissarmyhammer_clipboard.entity_id, task_id);
        assert_eq!(payload.swissarmyhammer_clipboard.fields["title"], "My task");
    }

    #[tokio::test]
    async fn copy_entity_works_on_project_via_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let add = AddProject::new("backend", "Backend")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let project_id = add["id"].as_str().unwrap();

        let ctx = make_ctx(
            "entity.copy",
            &[],
            Some(&format!("project:{project_id}")),
            &kanban,
            &clipboard,
            &ui,
        );
        let result = CopyEntityCmd.execute(&ctx).await.unwrap();
        assert_eq!(result["copied"], true);
        assert_eq!(result["entity_type"], "project");

        let text = clipboard.0.read_text().await.unwrap().unwrap();
        let payload = clipboard::deserialize_from_clipboard(&text).unwrap();
        assert_eq!(payload.swissarmyhammer_clipboard.entity_type, "project");
        assert_eq!(payload.swissarmyhammer_clipboard.entity_id, project_id);
    }

    #[tokio::test]
    async fn copy_entity_works_on_column_via_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;

        let ctx = make_ctx(
            "entity.copy",
            &[],
            Some("column:todo"),
            &kanban,
            &clipboard,
            &ui,
        );
        let result = CopyEntityCmd.execute(&ctx).await.unwrap();
        assert_eq!(result["copied"], true);
        assert_eq!(result["entity_type"], "column");
        assert_eq!(result["id"], "todo");

        let text = clipboard.0.read_text().await.unwrap().unwrap();
        let payload = clipboard::deserialize_from_clipboard(&text).unwrap();
        assert_eq!(payload.swissarmyhammer_clipboard.entity_type, "column");
        assert_eq!(payload.swissarmyhammer_clipboard.entity_id, "todo");
    }

    #[tokio::test]
    async fn copy_entity_works_on_actor_via_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let add = AddActor::new("alice", "Alice")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let actor_id = add["actor"]["id"].as_str().unwrap();

        let ctx = make_ctx(
            "entity.copy",
            &[],
            Some(&format!("actor:{actor_id}")),
            &kanban,
            &clipboard,
            &ui,
        );
        let result = CopyEntityCmd.execute(&ctx).await.unwrap();
        assert_eq!(result["copied"], true);
        assert_eq!(result["entity_type"], "actor");

        let text = clipboard.0.read_text().await.unwrap().unwrap();
        let payload = clipboard::deserialize_from_clipboard(&text).unwrap();
        assert_eq!(payload.swissarmyhammer_clipboard.entity_type, "actor");
        assert_eq!(payload.swissarmyhammer_clipboard.entity_id, actor_id);
    }

    #[tokio::test]
    async fn copy_entity_fails_for_unknown_entity() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx(
            "entity.copy",
            &[],
            Some("task:does-not-exist"),
            &kanban,
            &clipboard,
            &ui,
        );
        let result = CopyEntityCmd.execute(&ctx).await;
        assert!(result.is_err(), "copying a missing task should fail");
    }

    // =========================================================================
    // Cut availability — driven by `target`
    // =========================================================================

    #[tokio::test]
    async fn cut_available_with_task_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx(
            "entity.cut",
            &[],
            Some("task:01X"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(CutEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn cut_available_with_tag_target_and_task_in_scope() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx(
            "entity.cut",
            &["tag:01X", "task:01T", "column:todo"],
            Some("tag:01X"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(CutEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn cut_not_available_with_tag_target_without_task_in_scope() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx(
            "entity.cut",
            &["tag:01X", "column:todo"],
            Some("tag:01X"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(
            !CutEntityCmd.available(&ctx),
            "cut tag requires a task in scope to untag from"
        );
    }

    #[tokio::test]
    async fn cut_not_available_for_project_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx(
            "entity.cut",
            &[],
            Some("project:foo"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(
            !CutEntityCmd.available(&ctx),
            "project has no destructive cut defined"
        );
    }

    #[tokio::test]
    async fn cut_not_available_without_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx("entity.cut", &[], None, &kanban, &clipboard, &ui);
        assert!(!CutEntityCmd.available(&ctx));
    }

    #[tokio::test]
    async fn cut_not_available_for_archive_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx(
            "entity.cut",
            &[],
            Some("task:01X:archive"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(!CutEntityCmd.available(&ctx));
    }

    // =========================================================================
    // Cut execution
    // =========================================================================

    #[tokio::test]
    async fn cut_task_via_target_deletes_and_puts_on_clipboard() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let add = AddTask::new("Cut me")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = add["id"].as_str().unwrap();

        let ctx = make_ctx(
            "entity.cut",
            &[],
            Some(&format!("task:{task_id}")),
            &kanban,
            &clipboard,
            &ui,
        );
        let result = CutEntityCmd.execute(&ctx).await.unwrap();
        assert_eq!(result["cut"], true);
        assert!(ui.has_clipboard());

        // Task should be deleted
        let ectx = kanban.entity_context().await.unwrap();
        assert!(ectx.read("task", task_id).await.is_err());

        let text = clipboard.0.read_text().await.unwrap().unwrap();
        let payload = clipboard::deserialize_from_clipboard(&text).unwrap();
        assert_eq!(payload.swissarmyhammer_clipboard.entity_type, "task");
    }

    #[tokio::test]
    async fn cut_tag_via_target_untags_task_and_puts_on_clipboard() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let add = AddTask::new("Tagged")
            .with_description("Fix #bug")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = add["id"].as_str().unwrap();

        let ectx = kanban.entity_context().await.unwrap();
        let tag = crate::tag::find_tag_entity_by_name(&ectx, "bug")
            .await
            .unwrap();
        let tag_id = tag.id.to_string();

        let ctx = make_ctx(
            "entity.cut",
            &[
                &format!("tag:{tag_id}"),
                &format!("task:{task_id}"),
                "column:todo",
            ],
            Some(&format!("tag:{tag_id}")),
            &kanban,
            &clipboard,
            &ui,
        );
        let result = CutEntityCmd.execute(&ctx).await.unwrap();
        assert_eq!(result["cut"], true);
        assert_eq!(result["tag"], "bug");

        // Tag removed from task body
        let task = ectx.read("task", task_id).await.unwrap();
        assert!(!task.get_str("body").unwrap_or("").contains("#bug"));

        let text = clipboard.0.read_text().await.unwrap().unwrap();
        let payload = clipboard::deserialize_from_clipboard(&text).unwrap();
        assert_eq!(payload.swissarmyhammer_clipboard.entity_type, "tag");
    }

    #[tokio::test]
    async fn cut_project_via_target_returns_error() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx(
            "entity.cut",
            &[],
            Some("project:foo"),
            &kanban,
            &clipboard,
            &ui,
        );
        let result = CutEntityCmd.execute(&ctx).await;
        assert!(result.is_err(), "cut for unsupported type must error");
    }

    // =========================================================================
    // Paste availability — driven by `target`
    // =========================================================================

    #[tokio::test]
    async fn paste_task_available_with_column_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        ui.set_clipboard_entity_type("task");
        let ctx = make_ctx(
            "entity.paste",
            &[],
            Some("column:todo"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(PasteEntityCmd::new().available(&ctx));
    }

    #[tokio::test]
    async fn paste_tag_available_with_task_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        ui.set_clipboard_entity_type("tag");
        let ctx = make_ctx(
            "entity.paste",
            &[],
            Some("task:01X"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(PasteEntityCmd::new().available(&ctx));
    }

    #[tokio::test]
    async fn paste_tag_not_available_on_column_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        ui.set_clipboard_entity_type("tag");
        let ctx = make_ctx(
            "entity.paste",
            &[],
            Some("column:todo"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(
            !PasteEntityCmd::new().available(&ctx),
            "tag paste has no (tag, column) handler"
        );
    }

    #[tokio::test]
    async fn paste_not_available_without_clipboard() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx(
            "entity.paste",
            &[],
            Some("column:todo"),
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(!PasteEntityCmd::new().available(&ctx));
    }

    #[tokio::test]
    async fn paste_not_available_without_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        ui.set_clipboard_entity_type("task");
        let ctx = make_ctx("entity.paste", &[], None, &kanban, &clipboard, &ui);
        assert!(!PasteEntityCmd::new().available(&ctx));
    }

    // =========================================================================
    // Paste execution — dispatches through PasteMatrix
    // =========================================================================

    #[tokio::test]
    async fn paste_task_into_column_via_target_creates_new_task() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let add = AddTask::new("Source")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = add["id"].as_str().unwrap();

        // Copy the task first
        let copy_ctx = make_ctx(
            "entity.copy",
            &[],
            Some(&format!("task:{task_id}")),
            &kanban,
            &clipboard,
            &ui,
        );
        CopyEntityCmd.execute(&copy_ctx).await.unwrap();

        // Paste into doing column via target
        let paste_ctx = make_ctx(
            "entity.paste",
            &[],
            Some("column:doing"),
            &kanban,
            &clipboard,
            &ui,
        );
        let result = PasteEntityCmd::new().execute(&paste_ctx).await.unwrap();

        let new_id = result["id"].as_str().unwrap();
        assert_ne!(new_id, task_id, "pasted task must have new ID");

        let ectx = kanban.entity_context().await.unwrap();
        assert_eq!(ectx.list("task").await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn paste_tag_onto_task_via_target_tags_it() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let add = AddTask::new("Target")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = add["id"].as_str().unwrap();

        // Seed a tag and copy it
        let tag_add = AddTag::new("urgent")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let tag_id = tag_add["id"].as_str().unwrap();

        let copy_ctx = make_ctx(
            "entity.copy",
            &[],
            Some(&format!("tag:{tag_id}")),
            &kanban,
            &clipboard,
            &ui,
        );
        CopyEntityCmd.execute(&copy_ctx).await.unwrap();

        // Paste onto the task via target
        let paste_ctx = make_ctx(
            "entity.paste",
            &[],
            Some(&format!("task:{task_id}")),
            &kanban,
            &clipboard,
            &ui,
        );
        let result = PasteEntityCmd::new().execute(&paste_ctx).await.unwrap();
        // The (tag, task) paste handler delegates to `TagTask`, whose result
        // shape is `{"tagged": true, "task_id": ..., "tag": <slug>}`. We
        // assert against the underlying op's contract — wrapping it would
        // require every paste handler to translate its result.
        assert_eq!(result["tagged"], true);
        assert_eq!(result["tag"], "urgent");

        let ectx = kanban.entity_context().await.unwrap();
        let task = ectx.read("task", task_id).await.unwrap();
        assert!(task.get_str("body").unwrap_or("").contains("#urgent"));
    }

    #[tokio::test]
    async fn paste_fails_when_no_handler_for_pair() {
        let (_temp, kanban, clipboard, ui) = setup().await;

        // Put a tag on the clipboard
        let clip = clipboard::serialize_to_clipboard(
            "tag",
            "01FAKE",
            "copy",
            serde_json::json!({"tag_name": "bug"}),
        );
        clipboard.0.write_text(&clip).await.unwrap();
        ui.set_clipboard_entity_type("tag");

        // Paste onto a column — no (tag, column) handler
        let ctx = make_ctx(
            "entity.paste",
            &[],
            Some("column:todo"),
            &kanban,
            &clipboard,
            &ui,
        );
        let result = PasteEntityCmd::new().execute(&ctx).await;
        assert!(result.is_err(), "paste should fail without a handler");
    }
}
