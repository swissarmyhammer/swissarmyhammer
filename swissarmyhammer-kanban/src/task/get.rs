//! GetTask command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::task_helpers::task_entity_to_rich_json;
use crate::types::TaskId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get a task by ID with computed fields
#[operation(
    verb = "get",
    noun = "task",
    description = "Retrieve a task by ID with computed fields"
)]
#[derive(Debug, Deserialize)]
pub struct GetTask {
    /// The task ID to retrieve
    pub id: TaskId,
}

impl GetTask {
    /// Create a new GetTask command
    pub fn new(id: impl Into<TaskId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            let entity = ectx.read("task", self.id.as_str()).await?;

            let all_columns = ctx.read_all_columns().await?;
            let all_tasks = ectx.list("task").await?;

            let terminal_column = all_columns
                .iter()
                .max_by_key(|c| c.order)
                .map(|c| c.id.as_str())
                .unwrap_or("done");

            Ok(task_entity_to_rich_json(&entity, &all_tasks, terminal_column))
        }
        .await
        {
            Ok(value) => ExecutionResult::Unlogged { value },
            Err(error) => ExecutionResult::Failed {
                error,
                log_entry: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::task::AddTask;
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
    async fn test_get_task() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Test task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = GetTask::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["title"], "Test task");
        assert_eq!(result["ready"], true);
        assert!(result["blocked_by"].as_array().unwrap().is_empty());
        assert!(result["blocks"].as_array().unwrap().is_empty());
    }
}
