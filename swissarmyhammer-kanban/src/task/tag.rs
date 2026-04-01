//! TagTask command — appends `#tag` to task description

use crate::auto_color;
use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::tag::tag_name_exists_entity;
use crate::tag_parser;
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Add a tag to a task by appending `#tag` to its description.
///
/// The `tag` field is the tag name/slug (e.g. "bug").
/// If the Tag object doesn't exist yet, it is auto-created with an auto-color.
#[operation(verb = "tag", noun = "task", description = "Add a tag to a task")]
#[derive(Debug, Deserialize, Serialize)]
pub struct TagTask {
    /// The task ID to tag
    pub id: TaskId,
    /// The tag name (slug) to add (e.g. "bug")
    pub tag: String,
}

impl TagTask {
    pub fn new(id: impl Into<TaskId>, tag: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            tag: tag.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for TagTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: std::result::Result<Value, KanbanError> = async {
            let slug = tag_parser::normalize_slug(&self.tag);

            let ectx = ctx.entity_context().await?;

            // Auto-create Tag entity if it doesn't exist
            if !tag_name_exists_entity(&ectx, &slug).await {
                let color = auto_color::auto_color(&slug).to_string();
                let tag_id = ulid::Ulid::new().to_string();
                let mut tag_entity = Entity::new("tag", tag_id.as_str());
                tag_entity.set("tag_name", json!(slug));
                tag_entity.set("color", json!(color));
                ectx.write(&tag_entity).await?;
            }
            let mut entity = ectx.read("task", self.id.as_str()).await?;

            // Append #tag to body if not already present
            let body = entity.get_str("body").unwrap_or("").to_string();
            let new_body = tag_parser::append_tag(&body, &slug);
            if new_body != body {
                entity.set("body", serde_json::json!(new_body));
                ectx.write(&entity).await?;
            }

            Ok(serde_json::json!({
                "tagged": true,
                "task_id": self.id.to_string(),
                "tag": slug
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
            .get("task_id")
            .and_then(|v| v.as_str())
            .map(|id| vec![id.to_string()])
            .unwrap_or_default()
    }
}
