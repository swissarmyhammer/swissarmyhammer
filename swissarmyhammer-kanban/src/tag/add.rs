//! AddTag command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::Tag;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Add a new tag to the board.
///
/// The `name` is the tag slug (e.g. "bug", "high-priority").
/// A ULID is generated automatically for the tag's stable identity.
/// Color is optional — if omitted, a deterministic auto-color is assigned.
#[operation(verb = "add", noun = "tag", description = "Add a new tag to the board")]
#[derive(Debug, Deserialize, Serialize)]
pub struct AddTag {
    /// The tag name (human-readable slug)
    pub name: String,
    /// 6-character hex color code (without #). Optional — auto-assigned if omitted.
    pub color: Option<String>,
    /// Optional description
    pub description: Option<String>,
}

impl AddTag {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            color: None,
            description: None,
        }
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
impl Execute<KanbanContext, KanbanError> for AddTag {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
            // Check if a tag with this name already exists
            if ctx.tag_name_exists(&self.name).await? {
                return Err(KanbanError::duplicate_id("tag", self.name.clone()));
            }

            let mut tag = Tag::new(&self.name);
            if let Some(color) = &self.color {
                tag = tag.with_color(color);
            }
            if let Some(desc) = &self.description {
                tag = tag.with_description(desc);
            }

            // Write tag to file (filename is ULID)
            ctx.write_tag(&tag).await?;

            let mut result = serde_json::to_value(&tag)?;
            result["id"] = serde_json::json!(&tag.id);
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
}
