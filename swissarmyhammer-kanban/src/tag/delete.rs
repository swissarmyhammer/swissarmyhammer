//! DeleteTag command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::tag_parser;
use crate::types::TagId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Delete a tag (removes `#name` from all task descriptions and deletes the tag file)
#[operation(
    verb = "delete",
    noun = "tag",
    description = "Delete a tag and remove from all tasks"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct DeleteTag {
    /// The tag ID (ULID) to delete
    pub id: TagId,
}

impl DeleteTag {
    pub fn new(id: impl Into<TagId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteTag {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: std::result::Result<Value, KanbanError> = async {
            // Read tag to get its name
            let tag = ctx.read_tag(&self.id).await?;
            let tag_name = &tag.name;

            // Remove #name text from all task descriptions
            let task_ids = ctx.list_task_ids().await?;
            for id in task_ids {
                let mut task = ctx.read_task(&id).await?;
                let new_desc = tag_parser::remove_tag(&task.description, tag_name);
                if new_desc != task.description {
                    task.description = new_desc;
                    ctx.write_task(&task).await?;
                }
            }

            // Delete tag file
            ctx.delete_tag_file(&self.id).await?;

            Ok(serde_json::json!({
                "deleted": true,
                "id": self.id.to_string(),
                "name": tag_name
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
