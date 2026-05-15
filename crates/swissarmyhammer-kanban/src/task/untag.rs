//! UntagTask command — removes `#tag` from task description

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::tag_parser;
use crate::task_helpers::task_tags;
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Remove a tag from a task by removing `#tag` from its description.
///
/// The `tag` field is the tag name/slug (e.g. "bug").
#[operation(
    verb = "untag",
    noun = "task",
    description = "Remove a tag from a task"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UntagTask {
    /// The task ID to untag
    pub id: TaskId,
    /// The tag name (slug) to remove
    pub tag: String,
}

impl UntagTask {
    pub fn new(id: impl Into<TaskId>, tag: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            tag: tag.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UntagTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;

            // Resolve tag: may be a slug ("bug") or an entity ID (ULID).
            // If it looks like a ULID and we can read the tag entity, use its tag_name.
            let slug =
                if self.tag.len() == 26 && self.tag.chars().all(|c| c.is_ascii_alphanumeric()) {
                    // Looks like a ULID — try to resolve to tag_name
                    match ectx.read("tag", &self.tag).await {
                        Ok(tag_entity) => tag_parser::normalize_slug(
                            tag_entity.get_str("tag_name").unwrap_or(&self.tag),
                        ),
                        Err(_) => tag_parser::normalize_slug(&self.tag),
                    }
                } else {
                    tag_parser::normalize_slug(&self.tag)
                };
            let mut entity = ectx.read("task", self.id.as_str()).await?;

            // Check if tag is present in body
            let was_present = task_tags(&entity).iter().any(|t| t == &slug);

            // Remove #tag from body
            let body = entity.get_str("body").unwrap_or("").to_string();
            let new_body = tag_parser::remove_tag(&body, &slug);
            if new_body != body {
                entity.set("body", serde_json::json!(new_body));
                ectx.write(&entity).await?;
            }

            Ok(serde_json::json!({
                "untagged": was_present,
                "task_id": self.id.to_string(),
                "tag": slug
            }))
        }
        .await;

        match result {
            Ok(value) => ExecutionResult::Success { value },
            Err(error) => ExecutionResult::Failed { error },
        }
    }
}
