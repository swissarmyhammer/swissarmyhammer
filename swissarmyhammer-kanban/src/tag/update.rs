//! UpdateTag command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::tag_parser;
use crate::types::TagId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Update a tag's name, color, or description.
///
/// When the name changes, all task descriptions are bulk-updated
/// to replace `#old-name` with `#new-name`.
#[operation(
    verb = "update",
    noun = "tag",
    description = "Update a tag's name, color, or description"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateTag {
    /// The tag ID (ULID) to update
    pub id: TagId,
    /// New name (slug). When changed, triggers bulk rename in task descriptions.
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
            let old_name = tag.name.clone();

            if let Some(name) = &self.name {
                let normalized = tag_parser::normalize_slug(name);
                if normalized != old_name {
                    // Check that no other tag has this name
                    if let Some(existing) = ctx.find_tag_by_name(&normalized).await? {
                        if existing.id != self.id {
                            return Err(KanbanError::duplicate_id("tag", normalized));
                        }
                    }

                    // Bulk rename #old-name → #new-name in all task descriptions
                    let task_ids = ctx.list_task_ids().await?;
                    for tid in task_ids {
                        let mut task = ctx.read_task(&tid).await?;
                        let new_desc =
                            tag_parser::rename_tag(&task.description, &old_name, &normalized);
                        if new_desc != task.description {
                            task.description = new_desc;
                            ctx.write_task(&task).await?;
                        }
                    }

                    tag.name = normalized;
                }
            }

            if let Some(color) = &self.color {
                tag.color = color.clone();
            }
            if let Some(description) = &self.description {
                tag.description = Some(description.clone());
            }

            // Write updated tag back to same ULID file
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::context::KanbanContext;
    use crate::tag::AddTag;
    use crate::task::{AddTask, UpdateTask};
    use swissarmyhammer_operations::Execute;
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

    #[tokio::test]
    async fn test_rename_tag() {
        let (_temp, ctx) = setup().await;

        let result = AddTag::new("bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tag_id = result["id"].as_str().unwrap().to_string();

        let result = UpdateTag::new(tag_id.clone())
            .with_name("defect")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["name"], "defect");
        assert_eq!(result["id"], tag_id);
    }

    #[tokio::test]
    async fn test_change_tag_color() {
        let (_temp, ctx) = setup().await;

        let result = AddTag::new("bug")
            .with_color("d73a4a")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tag_id = result["id"].as_str().unwrap().to_string();

        let result = UpdateTag::new(tag_id.clone())
            .with_color("ff0000")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["color"], "ff0000");
        assert_eq!(result["name"], "bug"); // name unchanged
    }

    #[tokio::test]
    async fn test_change_tag_description() {
        let (_temp, ctx) = setup().await;

        let result = AddTag::new("bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tag_id = result["id"].as_str().unwrap().to_string();

        let result = UpdateTag::new(tag_id.clone())
            .with_description("Something isn't working")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["description"], "Something isn't working");
    }

    #[tokio::test]
    async fn test_rename_tag_bulk_updates_task_descriptions() {
        let (_temp, ctx) = setup().await;

        // Create a tag
        let tag_result = AddTag::new("bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tag_id = tag_result["id"].as_str().unwrap().to_string();

        // Create a task with #bug in its description
        let task_result = AddTask::new("Fix login")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // Set description with the tag
        UpdateTask::new(task_id.clone())
            .with_description("Login broken #bug please fix")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Rename the tag
        UpdateTag::new(tag_id.clone())
            .with_name("defect")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Read the task back — description should have #defect not #bug
        let task = ctx
            .read_task(&crate::types::TaskId::from_string(&task_id))
            .await
            .unwrap();
        assert!(
            task.description.contains("#defect"),
            "Expected #defect in: {}",
            task.description
        );
        assert!(
            !task.description.contains("#bug"),
            "Should not contain #bug in: {}",
            task.description
        );
    }

    #[tokio::test]
    async fn test_rename_tag_to_duplicate_name_fails() {
        let (_temp, ctx) = setup().await;

        let result = AddTag::new("bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let bug_id = result["id"].as_str().unwrap().to_string();

        AddTag::new("defect")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Try renaming "bug" to "defect" — should fail
        let result = UpdateTag::new(bug_id.clone())
            .with_name("defect")
            .execute(&ctx)
            .await
            .into_result();

        assert!(
            result.is_err(),
            "Should fail when renaming to existing tag name"
        );
    }

    #[tokio::test]
    async fn test_rename_tag_same_name_is_noop() {
        let (_temp, ctx) = setup().await;

        let result = AddTag::new("bug")
            .with_color("d73a4a")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tag_id = result["id"].as_str().unwrap().to_string();

        // "Rename" to the same name — should succeed, no change
        let result = UpdateTag::new(tag_id.clone())
            .with_name("bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["name"], "bug");
        assert_eq!(result["color"], "d73a4a");
    }

    #[tokio::test]
    async fn test_update_name_preserves_color_and_description() {
        let (_temp, ctx) = setup().await;

        let result = AddTag::new("bug")
            .with_color("d73a4a")
            .with_description("Something isn't working")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tag_id = result["id"].as_str().unwrap().to_string();

        // Update only name — color and description should survive
        let result = UpdateTag::new(tag_id.clone())
            .with_name("defect")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["name"], "defect");
        assert_eq!(result["color"], "d73a4a");
        assert_eq!(result["description"], "Something isn't working");

        // Update only color — name and description should survive
        let result = UpdateTag::new(tag_id.clone())
            .with_color("ff0000")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["name"], "defect");
        assert_eq!(result["color"], "ff0000");
        assert_eq!(result["description"], "Something isn't working");

        // Update only description — name and color should survive
        let result = UpdateTag::new(tag_id.clone())
            .with_description("A defect report")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["name"], "defect");
        assert_eq!(result["color"], "ff0000");
        assert_eq!(result["description"], "A defect report");
    }

    #[tokio::test]
    async fn test_empty_string_erases_description() {
        let (_temp, ctx) = setup().await;

        let result = AddTag::new("bug")
            .with_description("Something isn't working")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tag_id = result["id"].as_str().unwrap().to_string();

        // Set description to "" to erase it
        let result = UpdateTag::new(tag_id.clone())
            .with_description("")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["name"], "bug");
        assert_eq!(result["description"], "");
    }
}
