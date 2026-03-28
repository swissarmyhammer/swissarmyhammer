//! Archive, unarchive, and list-archived commands for tasks.
//!
//! `ArchiveTask` mirrors DeleteTask behavior: cleans up dependency references before
//! archiving so any tasks that depended on the archived task have it removed from
//! their `depends_on` lists.
//!
//! `UnarchiveTask` restores a previously archived task back to live storage.
//!
//! `ListArchived` returns all archived tasks with a count.

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::task_helpers::task_entity_to_json;
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

/// Restore an archived task back to the live task list.
///
/// Moves the task's data file from the archive directory back to live storage
/// and appends an "unarchive" changelog entry. The task will reappear in
/// `list task` results after this operation.
#[operation(
    verb = "unarchive",
    noun = "task",
    description = "Restore an archived task"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UnarchiveTask {
    /// The task ID to restore from the archive
    pub id: TaskId,
}

impl UnarchiveTask {
    /// Create a new UnarchiveTask command
    pub fn new(id: impl Into<TaskId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UnarchiveTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;

            // Restore the task from the archive
            ectx.unarchive("task", self.id.as_str()).await?;

            // Read the restored entity to return its data
            let entity = ectx
                .read("task", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;
            let title = entity.get_str("title").unwrap_or("").to_string();

            Ok(serde_json::json!({
                "unarchived": true,
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

/// List all archived tasks.
///
/// Returns tasks from the archive directory. These are tasks that were
/// archived via `ArchiveTask` and are no longer visible in normal task listings.
/// Each entry is the full task JSON as produced by `task_entity_to_json`.
#[operation(verb = "list", noun = "archived", description = "List archived tasks")]
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ListArchived;

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListArchived {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            let archived = ectx.list_archived("task").await?;

            let tasks: Vec<Value> = archived.iter().map(task_entity_to_json).collect();
            let count = tasks.len();

            Ok(serde_json::json!({
                "tasks": tasks,
                "count": count
            }))
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

    /// Archive then unarchive a task — it should reappear in the live list.
    #[tokio::test]
    async fn test_unarchive_task() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Task to unarchive")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap().to_string();

        // Archive it
        ArchiveTask::new(&*task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Verify it's gone from the live list
        let ectx = ctx.entity_context().await.unwrap();
        assert!(ectx.list("task").await.unwrap().is_empty());

        // Unarchive it
        let result = UnarchiveTask::new(&*task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["unarchived"], true);
        assert_eq!(result["id"].as_str().unwrap(), task_id);
        assert_eq!(result["title"], "Task to unarchive");

        // Verify task is back in the live list
        let tasks = ectx.list("task").await.unwrap();
        assert_eq!(tasks.len(), 1, "unarchived task should be in the live list");
        assert_eq!(tasks[0].get_str("title").unwrap(), "Task to unarchive");
    }

    /// Archive several tasks and verify ListArchived returns them all with count.
    #[tokio::test]
    async fn test_list_archived() {
        let (_temp, ctx) = setup().await;

        // Create 3 tasks
        let r1 = AddTask::new("Archived One")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let r2 = AddTask::new("Archived Two")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddTask::new("Still live")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let id1 = r1["id"].as_str().unwrap();
        let id2 = r2["id"].as_str().unwrap();

        // Archive the first two
        ArchiveTask::new(id1)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        ArchiveTask::new(id2)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // ListArchived should return exactly 2
        let result = ListArchived.execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["count"], 2, "should list 2 archived tasks");
        let tasks = result["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 2);

        let titles: Vec<&str> = tasks.iter().map(|t| t["title"].as_str().unwrap()).collect();
        assert!(titles.contains(&"Archived One"));
        assert!(titles.contains(&"Archived Two"));
        assert!(!titles.contains(&"Still live"));
    }
}
