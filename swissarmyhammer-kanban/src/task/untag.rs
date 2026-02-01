//! UntagTask command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{TagId, TaskId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Remove a tag from a task
#[operation(verb = "untag", noun = "task", description = "Remove a tag from a task")]
#[derive(Debug, Deserialize)]
pub struct UntagTask {
    /// The task ID to untag
    pub id: TaskId,
    /// The tag ID to remove
    pub tag: TagId,
}

impl UntagTask {
    pub fn new(id: impl Into<TaskId>, tag: impl Into<TagId>) -> Self {
        Self {
            id: id.into(),
            tag: tag.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UntagTask {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut task = ctx.read_task(&self.id).await?;

        // Remove tag if present
        let was_present = task.tags.contains(&self.tag);
        task.tags.retain(|t| t != &self.tag);

        if was_present {
            ctx.write_task(&task).await?;
        }

        Ok(serde_json::json!({
            "untagged": was_present,
            "task_id": self.id.to_string(),
            "tag_id": self.tag.to_string()
        }))
    }
}
