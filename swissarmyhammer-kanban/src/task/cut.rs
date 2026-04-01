//! CutTask operation — snapshot a task's fields for the clipboard, then delete it.
//!
//! This is a destructive operation: the task is deleted after its fields
//! are captured. The clipboard write happens in the Command layer.

use crate::clipboard;
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Cut a task: snapshot its fields for the clipboard, then delete it.
///
/// Returns the serialized clipboard payload and the deleted task's title.
/// The Command layer writes the payload to the system clipboard.
#[operation(
    verb = "cut",
    noun = "task",
    description = "Cut a task (copy to clipboard and delete)"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct CutTask {
    /// The task ID to cut.
    pub id: TaskId,
}

impl CutTask {
    /// Create a new CutTask operation.
    pub fn new(id: impl Into<TaskId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for CutTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;

            // Read the task to snapshot its fields
            let entity = ectx
                .read("task", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;
            let title = entity.get_str("title").unwrap_or("").to_string();

            // Snapshot all fields as clipboard JSON
            let fields = serde_json::to_value(&entity.fields)?;
            let clipboard_json =
                clipboard::serialize_to_clipboard("task", self.id.as_str(), "cut", fields);

            // Remove this task from the depends_on list of all other tasks
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

            // Delete the task.
            // The entity layer automatically trashes attachment files
            // referenced by attachment-type fields.
            ectx.delete("task", self.id.as_str()).await?;

            Ok(serde_json::json!({
                "cut": true,
                "id": self.id.to_string(),
                "title": title,
                "clipboard_json": clipboard_json,
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
    use crate::clipboard;
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
    async fn test_cut_task_returns_clipboard_json_and_deletes() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Cut me")
            .with_description("Will be cut")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = CutTask::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["cut"], true);
        assert_eq!(result["id"], task_id);
        assert_eq!(result["title"], "Cut me");

        // Verify clipboard JSON is valid
        let clipboard_json = result["clipboard_json"].as_str().unwrap();
        let payload = clipboard::deserialize_from_clipboard(clipboard_json)
            .expect("should deserialize clipboard payload");
        assert_eq!(payload.swissarmyhammer_clipboard.entity_type, "task");
        assert_eq!(payload.swissarmyhammer_clipboard.mode, "cut");
        assert_eq!(payload.swissarmyhammer_clipboard.fields["title"], "Cut me");

        // Verify the task is deleted
        let ectx = ctx.entity_context().await.unwrap();
        let tasks = ectx.list("task").await.unwrap();
        assert!(tasks.is_empty(), "task should be deleted after cut");
    }

    #[tokio::test]
    async fn test_cut_removes_from_dependencies() {
        let (_temp, ctx) = setup().await;

        let result1 = AddTask::new("Task 1")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id1 = result1["id"].as_str().unwrap();

        let result2 = AddTask::new("Task 2")
            .with_depends_on(vec![crate::types::TaskId::from_string(id1)])
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id2 = result2["id"].as_str().unwrap();

        // Cut task 1
        CutTask::new(id1).execute(&ctx).await.into_result().unwrap();

        // Task 2 should no longer depend on task 1
        let ectx = ctx.entity_context().await.unwrap();
        let task2 = ectx.read("task", id2).await.unwrap();
        assert!(task2.get_string_list("depends_on").is_empty());
    }

    #[tokio::test]
    async fn test_cut_nonexistent_task_fails() {
        let (_temp, ctx) = setup().await;

        let result = CutTask::new("nonexistent")
            .execute(&ctx)
            .await
            .into_result();
        assert!(result.is_err());
    }
}
