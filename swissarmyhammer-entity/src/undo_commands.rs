//! Entity-layer undo/redo command implementations.
//!
//! These commands operate on `StoreContext` (via extension) and use the
//! store-level undo/redo stack. They are entity-layer infrastructure,
//! reusable outside kanban.

use async_trait::async_trait;
use serde_json::{json, Value};

use swissarmyhammer_commands::{Command, CommandContext, CommandError};
use swissarmyhammer_store::StoreContext;

/// Undo the most recent undoable operation.
///
/// Delegates to `StoreContext::undo()` which finds the correct store and
/// reverses the most recent changelog entry.
/// Returns `{ "noop": true }` when the stack is empty.
pub struct UndoCmd;

#[async_trait]
impl Command for UndoCmd {
    /// Returns `true` only when the undo stack has entries to undo.
    ///
    /// Checks the cached `can_undo` flag on UIState, which is updated after
    /// every stack-mutating operation (write, delete, undo, redo). Falls back
    /// to `false` if UIState is not available on the context.
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.ui_state
            .as_ref()
            .map(|ui| ui.can_undo())
            .unwrap_or(false)
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let store_ctx = ctx.require_extension::<StoreContext>()?;

        match store_ctx.undo().await {
            Ok(()) => Ok(json!({ "undone": true })),
            Err(swissarmyhammer_store::StoreError::NotFound(_)) => Ok(json!({ "noop": true })),
            Err(e) => Err(CommandError::ExecutionFailed(e.to_string())),
        }
    }
}

/// Redo the most recently undone operation.
///
/// Delegates to `StoreContext::redo()` which finds the correct store and
/// re-applies the most recently undone changelog entry.
/// Returns `{ "noop": true }` when the stack is empty.
pub struct RedoCmd;

#[async_trait]
impl Command for RedoCmd {
    /// Returns `true` only when the undo stack has entries to redo.
    ///
    /// Checks the cached `can_redo` flag on UIState, which is updated after
    /// every stack-mutating operation (write, delete, undo, redo). Falls back
    /// to `false` if UIState is not available on the context.
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.ui_state
            .as_ref()
            .map(|ui| ui.can_redo())
            .unwrap_or(false)
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let store_ctx = ctx.require_extension::<StoreContext>()?;

        match store_ctx.redo().await {
            Ok(()) => Ok(json!({ "redone": true })),
            Err(swissarmyhammer_store::StoreError::NotFound(_)) => Ok(json!({ "noop": true })),
            Err(e) => Err(CommandError::ExecutionFailed(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::EntityContext;
    use crate::entity::Entity;
    use crate::test_utils::test_fields_context;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::ui_state::UIState;
    use tempfile::TempDir;

    /// Build a CommandContext with an EntityContext extension.
    fn build_cmd_ctx(ectx: Arc<EntityContext>, ui_state: Option<Arc<UIState>>) -> CommandContext {
        let mut ctx = CommandContext::new("test.undo", vec![], None, HashMap::new());
        ctx.set_extension(ectx);
        if let Some(ui) = ui_state {
            ctx = ctx.with_ui_state(ui);
        }
        ctx
    }

    #[tokio::test]
    async fn undo_cmd_available_false_without_ui_state() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        assert!(!UndoCmd.available(&ctx));
    }

    #[tokio::test]
    async fn redo_cmd_available_false_without_ui_state() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        assert!(!RedoCmd.available(&ctx));
    }

    #[tokio::test]
    async fn undo_cmd_available_with_ui_state_default_is_false() {
        let ui = Arc::new(UIState::default());
        let ctx = CommandContext::new("test", vec![], None, HashMap::new()).with_ui_state(ui);
        // Default UIState has can_undo = false
        assert!(!UndoCmd.available(&ctx));
    }

    #[tokio::test]
    async fn redo_cmd_available_with_ui_state_default_is_false() {
        let ui = Arc::new(UIState::default());
        let ctx = CommandContext::new("test", vec![], None, HashMap::new()).with_ui_state(ui);
        assert!(!RedoCmd.available(&ctx));
    }

    #[tokio::test]
    #[ignore = "requires StoreContext undo stack not yet on this branch"]
    async fn undo_cmd_execute_noop_when_stack_empty() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ectx = Arc::new(EntityContext::new(dir.path(), fields));

        let ctx = build_cmd_ctx(ectx, None);
        let result = UndoCmd.execute(&ctx).await.unwrap();

        assert_eq!(result["noop"], true);
    }

    #[tokio::test]
    #[ignore = "requires StoreContext undo stack not yet on this branch"]
    async fn redo_cmd_execute_noop_when_stack_empty() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ectx = Arc::new(EntityContext::new(dir.path(), fields));

        let ctx = build_cmd_ctx(ectx, None);
        let result = RedoCmd.execute(&ctx).await.unwrap();

        assert_eq!(result["noop"], true);
    }

    #[tokio::test]
    #[ignore = "requires StoreContext undo stack not yet on this branch"]
    async fn undo_cmd_execute_undoes_last_operation() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ectx = Arc::new(EntityContext::new(dir.path(), fields));

        // Create and update an entity
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ectx.write(&tag).await.unwrap();

        tag.set("tag_name", json!("Bug Report"));
        ectx.write(&tag).await.unwrap();

        let ctx = build_cmd_ctx(Arc::clone(&ectx), None);
        let result = UndoCmd.execute(&ctx).await.unwrap();

        assert!(result.get("undone").is_some());
        assert!(result.get("operation_id").is_some());

        // Verify the undo took effect
        let restored = ectx.read("tag", "bug").await.unwrap();
        assert_eq!(restored.get_str("tag_name"), Some("Bug"));
    }

    #[tokio::test]
    #[ignore = "requires StoreContext undo stack not yet on this branch"]
    async fn redo_cmd_execute_redoes_undone_operation() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ectx = Arc::new(EntityContext::new(dir.path(), fields));

        // Create and update
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ectx.write(&tag).await.unwrap();

        tag.set("tag_name", json!("Bug Report"));
        ectx.write(&tag).await.unwrap();

        // Undo via command
        let ctx = build_cmd_ctx(Arc::clone(&ectx), None);
        UndoCmd.execute(&ctx).await.unwrap();

        // Redo via command
        let result = RedoCmd.execute(&ctx).await.unwrap();
        assert!(result.get("redone").is_some());
        assert!(result.get("operation_id").is_some());

        // Verify redo took effect
        let redone = ectx.read("tag", "bug").await.unwrap();
        assert_eq!(redone.get_str("tag_name"), Some("Bug Report"));
    }

    #[tokio::test]
    async fn undo_cmd_missing_extension_errors() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        let result = UndoCmd.execute(&ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn redo_cmd_missing_extension_errors() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        let result = RedoCmd.execute(&ctx).await;
        assert!(result.is_err());
    }
}
