//! DeleteProject command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::ProjectId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Delete a project (fails if tasks reference it)
#[operation(
    verb = "delete",
    noun = "project",
    description = "Delete a project (fails if tasks reference it)"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct DeleteProject {
    /// The project ID to delete
    pub id: ProjectId,
}

impl DeleteProject {
    /// Create a new DeleteProject command
    pub fn new(id: impl Into<ProjectId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteProject {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
            let ectx = ctx.entity_context().await?;

            // Check project exists (read will error if not found)
            ectx.read("project", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;

            // Check for tasks referencing this project
            let tasks = ectx.list("task").await?;
            let task_count = tasks
                .iter()
                .filter(|t| t.get_str("project") == Some(self.id.as_str()))
                .count();

            if task_count > 0 {
                return Err(KanbanError::ProjectHasTasks {
                    id: self.id.to_string(),
                    count: task_count,
                });
            }

            ectx.delete("project", self.id.as_str()).await?;

            Ok(serde_json::json!({
                "deleted": true,
                "id": self.id.to_string()
            }))
        }
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(value) => ExecutionResult::Logged {
                value: value.clone(),
                log_entry: LogEntry::new(self.op_string(), input, value, None, duration_ms),
            },
            Err(error) => {
                let error_msg = error.to_string();
                ExecutionResult::Failed {
                    error,
                    log_entry: Some(LogEntry::new(
                        self.op_string(),
                        input,
                        serde_json::json!({"error": error_msg}),
                        None,
                        duration_ms,
                    )),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::error::KanbanError;
    use crate::project::add::AddProject;
    use crate::project::get::GetProject;
    use tempfile::TempDir;

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

    #[tokio::test]
    async fn test_delete_project_empty() {
        let (_temp, ctx) = setup().await;

        AddProject::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = DeleteProject::new("backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["deleted"], true);
        assert_eq!(result["id"], "backend");

        // Verify it is gone
        let get_result = GetProject::new("backend").execute(&ctx).await.into_result();
        assert!(matches!(
            get_result,
            Err(KanbanError::ProjectNotFound { .. })
        ));
    }

    #[tokio::test]
    async fn test_delete_project_not_found() {
        let (_temp, ctx) = setup().await;

        let result = DeleteProject::new("nonexistent")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::ProjectNotFound { .. })));
    }

    #[tokio::test]
    async fn test_delete_project_with_tasks_fails() {
        let (_temp, ctx) = setup().await;

        let add_result = AddProject::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result();
        eprintln!("AddProject result: {:?}", add_result);
        add_result.unwrap();

        // Verify project was created
        let ectx = ctx.entity_context().await.unwrap();
        let proj = ectx.read("project", "backend").await;
        eprintln!("Project after create: {:?}", proj);

        // Add a task that references this project
        let mut task = swissarmyhammer_entity::Entity::new("task", "test-task-1");
        task.set("title", serde_json::json!("A task"));
        task.set("project", serde_json::json!("backend"));
        task.set("position_column", serde_json::json!("todo"));
        task.set("position_ordinal", serde_json::json!("1000"));
        let write_result = ectx.write(&task).await;
        eprintln!("Task write result: {:?}", write_result);
        write_result.unwrap();

        // Read back the task to verify project field was stored
        let read_task = ectx.read("task", "test-task-1").await.unwrap();
        eprintln!("Task after read: {:?}", read_task);
        eprintln!("Task project field: {:?}", read_task.get_str("project"));

        // Attempting to delete the project should fail
        let result = DeleteProject::new("backend")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::ProjectHasTasks { .. })));
    }
}
