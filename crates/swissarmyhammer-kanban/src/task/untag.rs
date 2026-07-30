//! UntagTask command — removes `#tag` from task description

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::task::tags::{apply_tag_refs, TagApply};
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Remove a tag from a task by removing `#tag` from its description.
///
/// The `tag` field is a forgiving tag reference resolved exactly as `tag task`
/// resolves it — a tag name/slug (e.g. "bug"), a full tag ULID, `^<short>`, or
/// a 7-char short id. Removing a tag the task does not carry is a no-op; an id
/// reference that names no tag is an error.
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
            let mut entity = ectx.read("task", self.id.as_str()).await?;

            // Same resolver as `tag task`, inverse application: strip `#slug`
            // from the body.
            let refs = std::slice::from_ref(&self.tag);
            if apply_tag_refs(&ectx, &mut entity, refs, TagApply::Remove).await? {
                ectx.write(&entity).await?;
            }

            // Thin ack — success implies the tag is gone (idempotent);
            // `get task` is the escape hatch for the post-op tag list.
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
    use crate::task::{AddTask, GetTask, TagTask};
    use crate::task_helpers::assert_task_mutation_ack;
    use serde_json::json;
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

    /// `untag task` returns exactly the thin ack; the tag's removal is
    /// asserted via `get task` (stored state, not response echo).
    #[tokio::test]
    async fn test_untag_task_returns_thin_ack() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Untag me")
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

        let result = UntagTask::new(task_id, "bug")
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
            !task["tags"].as_array().unwrap().contains(&json!("bug")),
            "tag must be removed from the stored task, got: {}",
            task["tags"]
        );
    }

    /// Untagging by the tag's ULID resolves to that tag's name and removes it —
    /// the same resolver `tag task` uses.
    #[tokio::test]
    async fn test_untag_task_by_tag_ulid_removes_the_named_tag() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Untag by id")
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

        let tag_id = crate::tag::ListTags::default()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap()["tags"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == json!("bug"))
            .expect("the tag entity was auto-created")["id"]
            .as_str()
            .unwrap()
            .to_string();

        UntagTask::new(task_id, &tag_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let task = GetTask::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert!(
            task["tags"].as_array().unwrap().is_empty(),
            "untag by ULID must remove the resolved tag, got: {}",
            task["tags"]
        );
    }

    /// Untagging a tag that isn't present is idempotent and still acks.
    #[tokio::test]
    async fn test_untag_task_absent_tag_returns_thin_ack() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Nothing to untag")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = UntagTask::new(task_id, "ghost")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_task_mutation_ack(&result, task_id);
    }
}
