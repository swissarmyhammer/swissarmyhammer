//! UpdateTag command


use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::TagId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

/// Update a tag
#[operation(verb = "update", noun = "tag", description = "Update a tag's name, color, or description")]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateTag {
    /// The tag ID to update
    pub id: TagId,
    /// New tag name
    pub name: Option<String>,
    /// New color (6-character hex without #)
    pub color: Option<String>,
    /// New description
    pub description: Option<String>,
}

impl UpdateTag {
    pub fn new(id: impl Into<TagId>) -> Self {
        Self {
            id: id.into(),
            name: None,
            color: None,
            description: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateTag {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: std::result::Result<Value, KanbanError> = async {
            // Read tag from file
            let mut tag = ctx.read_tag(&self.id).await?;

            if let Some(name) = &self.name {
                tag.name = name.clone();
            }
            if let Some(color) = &self.color {
                tag.color = color.clone();
            }
            if let Some(description) = &self.description {
                tag.description = Some(description.clone());
            }

            // Write updated tag back to file
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
