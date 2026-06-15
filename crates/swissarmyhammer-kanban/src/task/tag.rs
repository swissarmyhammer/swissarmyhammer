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
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

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
        let result: std::result::Result<Value, KanbanError> = async {
            let ectx = ctx.entity_context().await?;

            // Resolve tag: may be a slug ("bug") or an entity ID (ULID).
            // If it looks like a ULID and we can read the tag entity, use its tag_name.
            let slug =
                if self.tag.len() == 26 && self.tag.chars().all(|c| c.is_ascii_alphanumeric()) {
                    match ectx.read("tag", &self.tag).await {
                        Ok(tag_entity) => tag_parser::normalize_slug(
                            tag_entity.get_str("tag_name").unwrap_or(&self.tag),
                        ),
                        Err(_) => tag_parser::normalize_slug(&self.tag),
                    }
                } else {
                    tag_parser::normalize_slug(&self.tag)
                };

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

            // Thin ack — success implies the tag took effect; `get task` is
            // the escape hatch for the post-op tag list.
            Ok(crate::task_helpers::task_mutation_ack(&entity))
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
    use crate::board::InitBoard;
    use crate::task::{AddTask, GetTask};
    use crate::task_helpers::assert_task_mutation_ack;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        (temp, ctx)
    }

    /// `tag task` returns exactly the thin ack; the tag's presence is
    /// asserted via `get task` (stored state, not response echo).
    #[tokio::test]
    async fn test_tag_task_returns_thin_ack() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Tag me")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = TagTask::new(task_id, "bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_task_mutation_ack(&result, task_id);

        let task = GetTask::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert!(
            task["tags"].as_array().unwrap().contains(&json!("bug")),
            "tag must be applied to the stored task, got: {}",
            task["tags"]
        );
    }

    /// Re-tagging with the same tag is idempotent and still returns the ack.
    #[tokio::test]
    async fn test_tag_task_idempotent_returns_thin_ack() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Tag me twice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        TagTask::new(task_id, "bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let result = TagTask::new(task_id, "bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_task_mutation_ack(&result, task_id);

        let task = GetTask::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(
            task["tags"].as_array().unwrap().len(),
            1,
            "duplicate tag must not be appended twice"
        );
    }
}
