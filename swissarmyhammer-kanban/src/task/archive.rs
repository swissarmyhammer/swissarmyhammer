//! ArchiveTask command
//!
//! Mirrors DeleteTask behavior: cleans up dependency references before archiving
//! so any tasks that depended on the archived task have it removed from their
//! `depends_on` lists.

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Archive a task, cleaning up any dependency references in other tasks.
///
/// When a task is archived, other tasks that have it in their `depends_on`
/// list will have it removed — the same cleanup that `DeleteTask` performs.
/// This ensures blocked tasks become unblocked after archiving.
#[operation(
    verb = "archive",
    noun = "task",
    description = "Archive a task and clean up dependencies"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct ArchiveTask {
    /// The task ID to archive
    pub id: TaskId,
}

impl ArchiveTask {
    /// Create a new ArchiveTask command
    pub fn new(id: impl Into<TaskId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ArchiveTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;

            // Read the task first to verify it exists and get its data
            let entity = ectx
                .read("task", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;
            let title = entity.get_str("title").unwrap_or("").to_string();

            // Remove this task from the depends_on list of all other tasks
            // (same cleanup as DeleteTask — archive is just delete with different storage)
            let all_tasks = ectx.list("task").await?;
            for mut t in all_tasks {
                if t.id == self.id.as_str() {
                    continue;
                }

                let deps = t.get_string_list("depends_on");
                if deps.contains(&self.id.to_string()) {
                    let new_deps: Vec<String> =
                        deps.into_iter().filter(|d| d != self.id.as_str()).collect();
                    t.set("depends_on", serde_json::to_value(&new_deps)?);
                    ectx.write(&t).await?;
                }
            }

            // Move the task to the archive directory
            ectx.archive("task", self.id.as_str()).await?;

            Ok(serde_json::json!({
                "archived": true,
                "id": self.id.to_string(),
                "title": title
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

    fn affected_resource_ids(&self, result: &Value) -> Vec<String> {
        result
            .get("id")
            .and_then(|v| v.as_str())
            .map(|id| vec![id.to_string()])
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::task::{AddTask, NextTask};
    use crate::types::TaskId;
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
    async fn test_archive_task() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Task to archive")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = ArchiveTask::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["archived"], true);
        assert_eq!(result["title"], "Task to archive");

        // Verify task is no longer in the live task list
        let ectx = ctx.entity_context().await.unwrap();
        let tasks = ectx.list("task").await.unwrap();
        assert!(tasks.is_empty());
    }

    /// When a task is archived, any other tasks that have it in their
    /// `depends_on` list should have it removed.
    #[tokio::test]
    async fn archive_task_cleans_dependencies() {
        let (_temp, ctx) = setup().await;

        // Create Task A (blocker)
        let result_a = AddTask::new("Task A - blocker")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id_a = result_a["id"].as_str().unwrap().to_string();

        // Create Task B depending on Task A
        let result_b = AddTask::new("Task B - depends on A")
            .with_depends_on(vec![TaskId::from_string(&id_a)])
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id_b = result_b["id"].as_str().unwrap().to_string();

        // Archive Task A
        ArchiveTask::new(&*id_a)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Verify Task B's depends_on is now empty
        let ectx = ctx.entity_context().await.unwrap();
        let task_b = ectx.read("task", &*id_b).await.unwrap();
        assert!(
            task_b.get_string_list("depends_on").is_empty(),
            "Task B should have no dependencies after Task A is archived"
        );
    }

    /// After archiving the blocker, the previously blocked task should become ready.
    #[tokio::test]
    async fn archive_task_dependent_becomes_ready() {
        let (_temp, ctx) = setup().await;

        // Create Task A (blocker)
        let result_a = AddTask::new("Task A - blocker")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id_a = result_a["id"].as_str().unwrap().to_string();

        // Create Task B depending on Task A
        AddTask::new("Task B - depends on A")
            .with_depends_on(vec![TaskId::from_string(&id_a)])
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Before archiving: NextTask should return Task A (Task B is blocked)
        let next = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(
            next["title"], "Task A - blocker",
            "NextTask should return the blocker before archiving"
        );

        // Archive Task A
        ArchiveTask::new(&*id_a)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // After archiving: NextTask should return Task B (now unblocked)
        let next = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(
            next["title"], "Task B - depends on A",
            "Task B should be ready (returned by NextTask) after Task A is archived"
        );
        assert_eq!(
            next["ready"], true,
            "Task B should report ready=true after Task A is archived"
        );
    }
}
