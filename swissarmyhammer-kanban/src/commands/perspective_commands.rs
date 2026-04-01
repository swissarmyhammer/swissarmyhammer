//! Perspective-related command implementations.
//!
//! Commands for loading, saving, deleting perspectives and for updating
//! filter/group settings on an active perspective.

use super::run_op;
use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::perspective::{
    AddPerspective, DeletePerspective, GetPerspective, ListPerspectives, UpdatePerspective,
};
use crate::processor::KanbanOperationProcessor;
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{Command, CommandContext, CommandError};
use swissarmyhammer_operations::OperationProcessor;

/// Load a perspective by name, returning its full configuration.
///
/// Requires `name` arg (the perspective name or ID).
pub struct LoadPerspectiveCmd;

#[async_trait]
impl Command for LoadPerspectiveCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.arg("name").and_then(|v| v.as_str()).is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let name = ctx
            .arg("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("name".into()))?;

        let op = GetPerspective::new(name);
        run_op(&op, &kanban).await
    }
}

/// Save a perspective — creates a new one or updates an existing one by name.
///
/// Requires `name` arg. Optional args: `view`, `filter`, `group`.
/// If a perspective with the given name already exists, it is updated.
/// Otherwise a new perspective is created.
pub struct SavePerspectiveCmd;

#[async_trait]
impl Command for SavePerspectiveCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.arg("name").and_then(|v| v.as_str()).is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let name = ctx
            .arg("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("name".into()))?;

        let view = ctx.arg("view").and_then(|v| v.as_str()).unwrap_or("board");

        let filter = ctx.arg("filter").and_then(|v| v.as_str()).map(String::from);
        let group = ctx.arg("group").and_then(|v| v.as_str()).map(String::from);

        // Try to create the perspective first. If a perspective with the same
        // name already exists the storage layer rejects the add atomically
        // (DuplicateName), so we fall back to update. This avoids the TOCTOU
        // race of reading the name, dropping the lock, then writing.
        let processor = KanbanOperationProcessor::new();
        let mut add_op = AddPerspective::new(name, view);
        add_op.filter = filter.clone();
        add_op.group = group.clone();

        match processor.process(&add_op, &kanban).await {
            Ok(val) => Ok(val),
            Err(KanbanError::DuplicateName { .. }) => {
                // Name already taken — look up the ID and update instead.
                let existing_id = {
                    let pctx = kanban
                        .perspective_context()
                        .await
                        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
                    let pctx = pctx.read().await;
                    pctx.get_by_name(name)
                        .map(|p| p.id.clone())
                        .ok_or_else(|| {
                            CommandError::ExecutionFailed(format!(
                                "perspective '{name}' reported as duplicate but not found"
                            ))
                        })?
                };
                let mut op = UpdatePerspective::new(existing_id);
                op.name = Some(name.to_string());
                op.view = Some(view.to_string());
                op.filter = Some(filter);
                op.group = Some(group);
                run_op(&op, &kanban).await
            }
            Err(e) => Err(CommandError::ExecutionFailed(e.to_string())),
        }
    }
}

/// Delete a perspective by name.
///
/// Requires `name` arg (the perspective name or ID).
pub struct DeletePerspectiveCmd;

#[async_trait]
impl Command for DeletePerspectiveCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.arg("name").and_then(|v| v.as_str()).is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let name = ctx
            .arg("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("name".into()))?;

        // Resolve name to ID if necessary
        let id = {
            let pctx = kanban
                .perspective_context()
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
            let pctx = pctx.read().await;
            if let Some(p) = pctx.get_by_name(name) {
                p.id.clone()
            } else if pctx.get_by_id(name).is_some() {
                name.to_string()
            } else {
                return Err(CommandError::ExecutionFailed(format!(
                    "perspective not found: {name}"
                )));
            }
        };

        let op = DeletePerspective::new(id);
        run_op(&op, &kanban).await
    }
}

/// Set the filter on an active perspective.
///
/// Requires `perspective_id` and `filter` args.
pub struct SetFilterCmd;

#[async_trait]
impl Command for SetFilterCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.arg("perspective_id").and_then(|v| v.as_str()).is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let perspective_id = ctx
            .arg("perspective_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("perspective_id".into()))?;

        let filter = ctx
            .arg("filter")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("filter".into()))?;

        let op = UpdatePerspective::new(perspective_id).with_filter(Some(filter.to_string()));
        run_op(&op, &kanban).await
    }
}

/// Clear the filter on an active perspective.
///
/// Requires `perspective_id` arg.
pub struct ClearFilterCmd;

#[async_trait]
impl Command for ClearFilterCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.arg("perspective_id").and_then(|v| v.as_str()).is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let perspective_id = ctx
            .arg("perspective_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("perspective_id".into()))?;

        let op = UpdatePerspective::new(perspective_id).with_filter(None);
        run_op(&op, &kanban).await
    }
}

/// Set the group on an active perspective.
///
/// Requires `perspective_id` and `group` args.
pub struct SetGroupCmd;

#[async_trait]
impl Command for SetGroupCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.arg("perspective_id").and_then(|v| v.as_str()).is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let perspective_id = ctx
            .arg("perspective_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("perspective_id".into()))?;

        let group = ctx
            .arg("group")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("group".into()))?;

        let op = UpdatePerspective::new(perspective_id).with_group(Some(group.to_string()));
        run_op(&op, &kanban).await
    }
}

/// Clear the group on an active perspective.
///
/// Requires `perspective_id` arg.
pub struct ClearGroupCmd;

#[async_trait]
impl Command for ClearGroupCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.arg("perspective_id").and_then(|v| v.as_str()).is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let perspective_id = ctx
            .arg("perspective_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("perspective_id".into()))?;

        let op = UpdatePerspective::new(perspective_id).with_group(None);
        run_op(&op, &kanban).await
    }
}

/// List all perspectives on the board.
///
/// No arguments required. Returns a JSON object with `perspectives` array
/// and `count`.
pub struct ListPerspectivesCmd;

#[async_trait]
impl Command for ListPerspectivesCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        let op = ListPerspectives::new();
        run_op(&op, &kanban).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::context::KanbanContext;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_operations::Execute;
    use tempfile::TempDir;

    /// Create a temp KanbanContext with an initialized board.
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

    /// Build a CommandContext with the given args and a KanbanContext extension.
    fn make_ctx(kanban: Arc<KanbanContext>, args: HashMap<String, Value>) -> CommandContext {
        let mut ctx = CommandContext::new("test", vec![], None, args);
        ctx.set_extension(kanban);
        ctx
    }

    #[tokio::test]
    async fn test_list_perspectives_cmd_empty() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(Arc::clone(&kanban), HashMap::new());

        let result = ListPerspectivesCmd.execute(&cmd_ctx).await.unwrap();
        let perspectives = result["perspectives"].as_array().unwrap();
        assert!(perspectives.is_empty());
        assert_eq!(result["count"], 0);
    }

    #[tokio::test]
    async fn test_list_perspectives_cmd_after_save() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        // Save a perspective via the SavePerspectiveCmd
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("My View".into()));
        args.insert("view".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args);
        SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();

        // Now list
        let cmd_ctx = make_ctx(Arc::clone(&kanban), HashMap::new());
        let result = ListPerspectivesCmd.execute(&cmd_ctx).await.unwrap();
        let perspectives = result["perspectives"].as_array().unwrap();
        assert_eq!(perspectives.len(), 1);
        assert_eq!(result["count"], 1);
        assert_eq!(perspectives[0]["name"], "My View");
        assert_eq!(perspectives[0]["view"], "board");
        // Each perspective should have an id
        assert!(perspectives[0]["id"].as_str().is_some());
    }
}
