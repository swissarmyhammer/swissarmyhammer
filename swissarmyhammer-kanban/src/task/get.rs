//! GetTask command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::task_helpers::{enrich_task_entity, task_entity_to_rich_json};
use crate::types::TaskId;
use crate::virtual_tags::default_virtual_tag_registry;
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
            let entity = ectx
                .read("task", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;

            let all_columns = ectx.list("column").await?;
            let all_tasks = ectx.list("task").await?;

            let terminal_column = all_columns
                .iter()
                .max_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
                .map(|c| c.id.as_str())
                .unwrap_or("done");

            let registry = default_virtual_tag_registry();
            let mut entity = entity;
            enrich_task_entity(&mut entity, &all_tasks, terminal_column, registry);

            Ok(task_entity_to_rich_json(&entity))
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

    // -----------------------------------------------------------------------
    // Date field round-trip tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_task_returns_user_set_dates() {
        let (_temp, ctx) = setup().await;

        let add = AddTask::new("Task with dates")
            .with_due("2026-04-30")
            .with_scheduled("2026-04-15")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id = add["id"].as_str().unwrap();

        let result = GetTask::new(id).execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["due"], "2026-04-30");
        assert_eq!(result["scheduled"], "2026-04-15");
    }

    #[tokio::test]
    async fn test_get_task_returns_null_for_unset_dates() {
        let (_temp, ctx) = setup().await;

        let add = AddTask::new("Plain task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id = add["id"].as_str().unwrap();

        let result = GetTask::new(id).execute(&ctx).await.into_result().unwrap();

        assert!(result["due"].is_null(), "unset due must be null");
        assert!(
            result["scheduled"].is_null(),
            "unset scheduled must be null"
        );
    }

    #[tokio::test]
    async fn test_get_task_returns_created_and_updated_timestamps() {
        let (_temp, ctx) = setup().await;

        let add = AddTask::new("Tracked task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id = add["id"].as_str().unwrap();

        let result = GetTask::new(id).execute(&ctx).await.into_result().unwrap();

        // `created` and `updated` come from the changelog. After a single
        // write they should both be non-null and equal (same write event).
        let created = result["created"].as_str();
        let updated = result["updated"].as_str();
        assert!(
            created.is_some(),
            "created must be populated from the changelog"
        );
        assert!(
            updated.is_some(),
            "updated must be populated from the changelog"
        );
    }

    #[tokio::test]
    async fn test_get_task_updated_changes_after_update() {
        use crate::task::UpdateTask;
        let (_temp, ctx) = setup().await;

        let add = AddTask::new("Will change")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id = add["id"].as_str().unwrap();

        let first = GetTask::new(id).execute(&ctx).await.into_result().unwrap();
        let first_updated = first["updated"]
            .as_str()
            .expect("first updated should exist")
            .to_string();

        // Sleep just enough to ensure the changelog timestamp advances.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        UpdateTask::new(id)
            .with_title("Changed")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let second = GetTask::new(id).execute(&ctx).await.into_result().unwrap();
        let second_updated = second["updated"]
            .as_str()
            .expect("second updated should exist")
            .to_string();

        assert_ne!(
            first_updated, second_updated,
            "updated should advance after an update write"
        );
    }

    #[tokio::test]
    async fn test_get_task_started_and_completed_after_column_moves() {
        use crate::task::MoveTask;
        let (_temp, ctx) = setup().await;

        let add = AddTask::new("Flow task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id = add["id"].as_str().unwrap();

        // Still in todo → started and completed should both be null.
        let before = GetTask::new(id).execute(&ctx).await.into_result().unwrap();
        assert!(
            before["started"].is_null(),
            "started should be null before moving out of todo"
        );
        assert!(
            before["completed"].is_null(),
            "completed should be null before reaching done"
        );

        // Move to doing → started populated; completed still null.
        MoveTask::to_column(id, "doing")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let mid = GetTask::new(id).execute(&ctx).await.into_result().unwrap();
        assert!(
            mid["started"].is_string(),
            "started should be a timestamp after moving to doing"
        );
        assert!(
            mid["completed"].is_null(),
            "completed should still be null while in doing"
        );

        // Move to done → completed populated.
        MoveTask::to_column(id, "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let end = GetTask::new(id).execute(&ctx).await.into_result().unwrap();
        assert!(
            end["completed"].is_string(),
            "completed should be a timestamp after moving to done"
        );
    }

    #[tokio::test]
    async fn test_get_task_completed_cleared_when_task_reopened() {
        use crate::task::MoveTask;
        let (_temp, ctx) = setup().await;

        let add = AddTask::new("Reopen task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id = add["id"].as_str().unwrap();

        // Move to done.
        MoveTask::to_column(id, "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let done = GetTask::new(id).execute(&ctx).await.into_result().unwrap();
        assert!(
            done["completed"].is_string(),
            "completed should be populated in done column"
        );

        // Reopen by moving back to doing.
        MoveTask::to_column(id, "doing")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let reopened = GetTask::new(id).execute(&ctx).await.into_result().unwrap();
        assert!(
            reopened["completed"].is_null(),
            "completed should be null after moving out of done"
        );
    }
}
