//! DeleteTag command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::tag::shared::apply_tag_edit_to_all_tasks;
use crate::tag_parser;
use crate::types::TagId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

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
    /// Create a new DeleteTag command for the given tag ID.
    pub fn new(id: impl Into<TagId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteTag {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let result: std::result::Result<Value, KanbanError> = async {
            let ectx = ctx.entity_context().await?;

            // Read tag entity to get its name
            let entity = ectx
                .read("tag", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;
            let tag_name = entity.get_str("tag_name").unwrap_or("").to_string();
            // `add tag` stores the name verbatim, but a body always carries the
            // NORMALIZED slug — that is what every tagging path writes and what
            // `parse_tags` reads back. Normalize before editing bodies, exactly
            // as `cut tag` and `paste tag` do, or a tag whose stored name needs
            // normalizing (`"Bug Fix"`, `"v2.0"`) leaves its markers behind.
            let slug = tag_parser::normalize_slug(&tag_name);

            // Remove #slug text from all task bodies
            apply_tag_edit_to_all_tasks(&ectx, |body| tag_parser::remove_tag(body, &slug)).await?;

            // Delete tag entity
            ectx.delete("tag", self.id.as_str()).await?;

            Ok(serde_json::json!({
                "deleted": true,
                "id": self.id.to_string(),
                "name": tag_name
            }))
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
    use crate::tag::AddTag;
    use crate::task::AddTask;
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

    /// Deleting a tag strips its markers from every task body.
    #[tokio::test]
    async fn test_delete_tag_strips_markers_from_task_bodies() {
        let (_temp, ctx) = setup().await;

        let tag = AddTag::new("bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tag_id = tag["id"].as_str().unwrap().to_string();

        let task = AddTask::new("Fix login")
            .with_description("Login broken #bug please fix")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task["id"].as_str().unwrap().to_string();

        DeleteTag::new(tag_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let ectx = ctx.entity_context().await.unwrap();
        let body = ectx.read("task", &task_id).await.unwrap();
        let body = body.get_str("body").unwrap_or("").to_string();
        assert!(!body.contains("#bug"), "marker must be gone from: {body}");
        assert!(
            body.contains("Login broken") && body.contains("please fix"),
            "the prose must survive: {body}"
        );
    }

    /// A tag stored under a name that needs normalizing (`add tag` keeps the
    /// name verbatim) still has its markers stripped. A body always carries the
    /// NORMALIZED slug, so the delete must normalize the stored name before it
    /// edits bodies.
    #[tokio::test]
    async fn test_delete_tag_with_unnormalized_stored_name_strips_markers() {
        let (_temp, ctx) = setup().await;

        let tag = AddTag::new("Bug Fix")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tag_id = tag["id"].as_str().unwrap().to_string();

        let task = AddTask::new("Fix login")
            .with_tags(vec!["Bug Fix".to_string()])
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task["id"].as_str().unwrap().to_string();

        let ectx = ctx.entity_context().await.unwrap();
        let seeded = ectx.read("task", &task_id).await.unwrap();
        assert!(
            seeded.get_str("body").unwrap_or("").contains("#Bug-Fix"),
            "the body must carry the normalized marker, or the delete proves nothing: {:?}",
            seeded.get_str("body")
        );

        DeleteTag::new(tag_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // A fresh entity context — the one used for the seed assertion caches
        // what it read, so it would hand back the pre-delete body.
        let ectx = ctx.entity_context().await.unwrap();
        let body = ectx.read("task", &task_id).await.unwrap();
        let body = body.get_str("body").unwrap_or("").to_string();
        assert!(
            !body.contains("#Bug-Fix"),
            "marker must be gone from: {body}"
        );
    }
}
