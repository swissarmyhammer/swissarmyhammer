//! ListComments command

use crate::comment::{comment_member_to_json, task_comments};
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::TaskId;
use serde::Deserialize;
use serde_json::{json, Value};
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List all comments on a task, in creation order
#[operation(
    verb = "list",
    noun = "comments",
    description = "List all comments on a task"
)]
#[derive(Debug, Deserialize)]
pub struct ListComments {
    /// The task ID to list comments for
    pub task_id: TaskId,
}

impl ListComments {
    /// Create a new ListComments command
    pub fn new(task_id: impl Into<TaskId>) -> Self {
        Self {
            task_id: task_id.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListComments {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;
            let task = ectx.read("task", self.task_id.as_str()).await?;

            // Canonical order: member id ascending — ULIDs are time-ordered,
            // so id order is creation order and is stable under edits.
            let mut members = task_comments(&task);
            members.sort_by(|a, b| {
                let a_id = a.get("id").and_then(Value::as_str).unwrap_or("");
                let b_id = b.get("id").and_then(Value::as_str).unwrap_or("");
                a_id.cmp(b_id)
            });
            let comments: Vec<Value> = members.iter().map(comment_member_to_json).collect();

            Ok(json!({
                "comments": comments,
                "count": comments.len(),
                "task_id": self.task_id.to_string(),
            }))
        }
        .await;

        match result {
            Ok(value) => ExecutionResult::Success { value },
            Err(error) => ExecutionResult::Failed { error },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comment::testing::setup_with_task;
    use serde_json::json;

    #[tokio::test]
    async fn test_list_empty_comments() {
        let (_temp, ctx, task_id) = setup_with_task().await;

        let result = ListComments::new(task_id.as_str())
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["comments"], json!([]));
        assert_eq!(result["count"], 0);
        assert_eq!(result["task_id"], task_id);
    }

    /// The canonical order of a comment log is member id ascending — even
    /// when the stored array is scrambled, listing sorts by id.
    #[tokio::test]
    async fn test_list_comments_sorted_by_id_ascending() {
        let (_temp, ctx, task_id) = setup_with_task().await;

        // Write a deliberately scrambled comment log directly to the field.
        let ectx = ctx.entity_context().await.unwrap();
        let mut task = ectx.read("task", &task_id).await.unwrap();
        task.set(
            "comments",
            json!([
                {"id": "01c0000000000000000000000c", "actor": "alice", "text": "third", "timestamp": "2026-01-03T00:00:00+00:00"},
                {"id": "01a0000000000000000000000a", "actor": "alice", "text": "first", "timestamp": "2026-01-01T00:00:00+00:00"},
                {"id": "01b0000000000000000000000b", "actor": "alice", "text": "second", "timestamp": "2026-01-02T00:00:00+00:00"},
            ]),
        );
        ectx.write(&task).await.unwrap();

        let result = ListComments::new(task_id.as_str())
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let members = result["comments"].as_array().unwrap();
        assert_eq!(result["count"], 3);
        let texts: Vec<&str> = members
            .iter()
            .map(|m| m["text"].as_str().unwrap())
            .collect();
        assert_eq!(texts, vec!["first", "second", "third"]);
    }

    #[tokio::test]
    async fn test_list_comments_nonexistent_task_errors() {
        let (_temp, ctx, _task_id) = setup_with_task().await;

        let result = ListComments::new("01nonexistent0000000000000")
            .execute(&ctx)
            .await
            .into_result();
        assert!(result.is_err());
    }
}
