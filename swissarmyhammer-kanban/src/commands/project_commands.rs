//! Project-related command implementations: add and delete.

use super::run_op;
use crate::context::KanbanContext;
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{Command, CommandContext, CommandError};

/// Add a new project to the board.
///
/// Always available (no scope prerequisites). Generates a slug ID from
/// the project name. Optional args: `name`, `description`, `color`.
pub struct AddProjectCmd;

#[async_trait]
impl Command for AddProjectCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let name = ctx
            .arg("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "New project".to_string());

        // Generate a slug ID from the name: lowercase, replace non-alphanumeric with hyphens
        let id: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .to_string();

        let mut op = crate::project::AddProject::new(id, &name);

        if let Some(description) = ctx.arg("description").and_then(|v| v.as_str()) {
            op = op.with_description(description);
        }
        if let Some(color) = ctx.arg("color").and_then(|v| v.as_str()) {
            op = op.with_color(color);
        }

        run_op(&op, &kanban).await
    }
}

/// Delete a project by its ID from the scope chain or args.
///
/// Available when `project` is in the scope chain or an `id` arg is
/// provided. Fails if tasks reference the project.
pub struct DeleteProjectCmd;

#[async_trait]
impl Command for DeleteProjectCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("project") || ctx.arg("id").and_then(|v| v.as_str()).is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let project_id = ctx
            .resolve_entity_id("project")
            .or_else(|| ctx.arg("id").and_then(|v| v.as_str()))
            .ok_or_else(|| CommandError::MissingScope("project".into()))?;

        let op = crate::project::DeleteProject::new(project_id);
        run_op(&op, &kanban).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::context::KanbanContext;
    use crate::project::AddProject;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::CommandContext;
    use swissarmyhammer_operations::Execute;
    use tempfile::TempDir;

    /// Initialize a board and return a (TempDir, KanbanContext) pair.
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

    /// Build a CommandContext with a KanbanContext extension and optional scope/args.
    fn make_ctx(
        kanban: Arc<KanbanContext>,
        scope: Vec<String>,
        args: HashMap<String, Value>,
    ) -> CommandContext {
        let mut ctx = CommandContext::new("test", scope, None, args);
        ctx.set_extension(kanban);
        ctx
    }

    // =========================================================================
    // AddProjectCmd
    // =========================================================================

    #[test]
    fn add_project_always_available() {
        let ctx = CommandContext::new("project.add", vec![], None, HashMap::new());
        assert!(AddProjectCmd.available(&ctx));
    }

    #[tokio::test]
    async fn add_project_creates_with_defaults() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(Arc::clone(&kanban), vec![], HashMap::new());

        let result = AddProjectCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["name"], "New project");
        assert!(!result["id"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn add_project_creates_with_name() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("Backend".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), vec![], args);

        let result = AddProjectCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["name"], "Backend");
        assert_eq!(result["id"], "backend");
    }

    #[tokio::test]
    async fn add_project_creates_with_all_fields() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("Frontend".into()));
        args.insert("description".into(), Value::String("UI work".into()));
        args.insert("color".into(), Value::String("ff0000".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), vec![], args);

        let result = AddProjectCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["name"], "Frontend");
        assert_eq!(result["description"], "UI work");
        assert_eq!(result["color"], "ff0000");
    }

    // =========================================================================
    // DeleteProjectCmd
    // =========================================================================

    #[test]
    fn delete_project_available_with_project_in_scope() {
        let ctx = CommandContext::new(
            "project.delete",
            vec!["project:backend".into()],
            None,
            HashMap::new(),
        );
        assert!(DeleteProjectCmd.available(&ctx));
    }

    #[test]
    fn delete_project_available_with_id_arg() {
        let mut args = HashMap::new();
        args.insert("id".into(), Value::String("backend".into()));
        let ctx = CommandContext::new("project.delete", vec![], None, args);
        assert!(DeleteProjectCmd.available(&ctx));
    }

    #[test]
    fn delete_project_not_available_without_scope_or_arg() {
        let ctx = CommandContext::new("project.delete", vec![], None, HashMap::new());
        assert!(!DeleteProjectCmd.available(&ctx));
    }

    #[tokio::test]
    async fn delete_project_deletes_from_scope() {
        let (_temp, ctx) = setup().await;

        // Create a project first
        AddProject::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            vec!["project:backend".into()],
            HashMap::new(),
        );

        let result = DeleteProjectCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["deleted"], true);
    }

    #[tokio::test]
    async fn delete_project_fails_without_scope_or_arg() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(Arc::clone(&kanban), vec![], HashMap::new());
        let result = DeleteProjectCmd.execute(&cmd_ctx).await;
        assert!(result.is_err());
    }
}
