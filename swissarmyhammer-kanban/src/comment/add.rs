//! AddComment command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{ActorId, Comment, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

/// Add a comment to a task
#[operation(verb = "add", noun = "comment", description = "Add a comment to a task")]
#[derive(Debug, Deserialize, Serialize)]
pub struct AddComment {
    /// The task ID to comment on
    pub task_id: TaskId,
    /// The comment body
    pub body: String,
    /// The author of the comment
    pub author: ActorId,
}

impl AddComment {
    pub fn new(
        task_id: impl Into<TaskId>,
        body: impl Into<String>,
        author: impl Into<ActorId>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            body: body.into(),
            author: author.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddComment {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let mut task = ctx.read_task(&self.task_id).await?;

            let comment = Comment::new(&self.body, self.author.clone());
            let result = serde_json::to_value(&comment)?;

            task.comments.push(comment);
            ctx.write_task(&task).await?;

            Ok(result)
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

    fn affected_resource_ids(&self, _result: &Value) -> Vec<String> {
        vec![self.task_id.to_string()]
    }
}
