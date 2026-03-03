//! AddTag command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::{Tag, TagId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Add a new tag to the board
#[operation(verb = "add", noun = "tag", description = "Add a new tag to the board")]
#[derive(Debug, Deserialize, Serialize)]
pub struct AddTag {
    /// The tag ID (slug)
    pub id: TagId,
    /// The tag display name
    pub name: String,
    /// 6-character hex color code (without #)
    pub color: String,
    /// Optional description
    pub description: Option<String>,
}

impl AddTag {
    pub fn new(id: impl Into<TagId>, name: impl Into<String>, color: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            color: color.into(),
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddTag {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
            // Check if tag already exists using file-based storage
            if ctx.tag_exists(&self.id).await {
                return Err(KanbanError::duplicate_id("tag", self.id.to_string()));
            }

            let mut tag = Tag::new(self.id.0.clone(), &self.name, &self.color);
            if let Some(desc) = &self.description {
                tag = tag.with_description(desc);
            }

            // Write tag to file
            ctx.write_tag(&tag).await?;

            Ok(serde_json::to_value(&tag)?)
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
