//! DeleteTag command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::tag_parser;
use crate::types::TagId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Delete a tag (removes `#name` from all task descriptions and deletes the tag entity)
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
            let ectx = ctx.entity_context().await?;

            // Read tag entity to get its name
            let entity = ectx.read("tag", self.id.as_str()).await?;
            let tag_name = entity.get_str("tag_name").unwrap_or("").to_string();

            // Remove #name text from all task bodies
            let all_tasks = ectx.list("task").await?;
            for mut task in all_tasks {
                let body = task.get_str("body").unwrap_or("").to_string();
                let new_body = tag_parser::remove_tag(&body, &tag_name);
                if new_body != body {
                    task.set("body", json!(new_body));
                    ectx.write(&task).await?;
                }
            }

            // Delete tag entity
            ectx.delete("tag", self.id.as_str()).await?;

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
