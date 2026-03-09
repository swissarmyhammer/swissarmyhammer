//! UpdateEntityField command

use crate::auto_color;
use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::tag::tag_name_exists_entity;
use crate::tag_parser;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Update a single field on any entity.
///
/// Generic command that works with any entity type (task, tag, actor, etc.).
/// Validates the field name against the entity's schema, reads the entity,
/// sets (or removes if null) the field, and writes it back.
#[operation(
    verb = "update",
    noun = "entity field",
    description = "Update a single field on any entity"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateEntityField {
    /// The entity type (e.g. "task", "tag", "actor", "column")
    pub entity_type: String,
    /// The entity ID
    pub id: String,
    /// The field name to update
    pub field_name: String,
    /// The new value (null to remove the field)
    pub value: Value,
}

impl UpdateEntityField {
    /// Create a new UpdateEntityField command.
    pub fn new(
        entity_type: impl Into<String>,
        id: impl Into<String>,
        field_name: impl Into<String>,
        value: Value,
    ) -> Self {
        Self {
            entity_type: entity_type.into(),
            id: id.into(),
            field_name: field_name.into(),
            value,
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateEntityField {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: std::result::Result<Value, KanbanError> = async {
            let ectx = ctx.entity_context().await?;

            // Validate field_name against the entity's schema
            let entity_def = ectx
                .entity_def(&self.entity_type)
                .map_err(KanbanError::from_entity_error)?;
            if !entity_def
                .fields
                .iter()
                .any(|f| f.as_str() == self.field_name)
            {
                return Err(KanbanError::InvalidValue {
                    field: self.field_name.clone(),
                    message: format!(
                        "field '{}' is not defined for entity type '{}'",
                        self.field_name, self.entity_type
                    ),
                });
            }

            let mut entity = ectx
                .read(&self.entity_type, &self.id)
                .await
                .map_err(KanbanError::from_entity_error)?;

            if self.value.is_null() {
                entity.remove(&self.field_name);
            } else {
                entity.set(&self.field_name, self.value.clone());
            }

            ectx.write(&entity).await?;

            // Auto-create tag entities when a task body field is updated
            if self.entity_type == "task" {
                let fields_ctx = ectx.fields();
                let entity_def = fields_ctx.get_entity("task");
                let is_body_field = entity_def
                    .map(|def| def.body_field.as_deref() == Some(&self.field_name))
                    .unwrap_or(false);
                if is_body_field {
                    let body_text = self.value.as_str().unwrap_or("");
                    let tags = tag_parser::parse_tags(body_text);
                    for tag_name in &tags {
                        if !tag_name_exists_entity(ectx, tag_name).await {
                            let color = auto_color::auto_color(tag_name).to_string();
                            let tag_id = ulid::Ulid::new().to_string();
                            let mut tag_entity = Entity::new("tag", tag_id.as_str());
                            tag_entity.set("tag_name", json!(tag_name));
                            tag_entity.set("color", json!(color));
                            ectx.write(&tag_entity).await?;
                        }
                    }
                }
            }

            Ok(entity.to_json())
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

    fn affected_resource_ids(&self, _result: &Value) -> Vec<String> {
        vec![self.id.clone()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::context::KanbanContext;
    use crate::task::AddTask;
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
    async fn test_update_entity_field_set_value() {
        let (_temp, ctx) = setup().await;

        // Create a task first
        let task_result = AddTask::new("Original title")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // Update the title field
        let cmd = UpdateEntityField::new("task", &task_id, "title", serde_json::json!("New title"));
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["title"], "New title");
        assert_eq!(result["id"], task_id);
    }

    #[tokio::test]
    async fn test_update_entity_field_remove_value() {
        let (_temp, ctx) = setup().await;

        // Create a task with a description
        let task_result = AddTask::new("Test task")
            .with_description("Some description")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // Remove the body field by setting it to null
        let cmd = UpdateEntityField::new("task", &task_id, "body", Value::Null);
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        // The body field should be absent from the result
        assert!(result.get("body").is_none() || result["body"].is_null());
    }

    #[tokio::test]
    async fn test_update_entity_field_invalid_field() {
        let (_temp, ctx) = setup().await;

        // Create a task
        let task_result = AddTask::new("Test task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // Try to update a field that doesn't exist on tasks
        let cmd = UpdateEntityField::new(
            "task",
            &task_id,
            "nonexistent_field",
            serde_json::json!("value"),
        );
        let result = cmd.execute(&ctx).await.into_result();

        assert!(result.is_err(), "Should fail for undefined field");
    }

    #[tokio::test]
    async fn test_update_body_auto_creates_tag_entities() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Tag test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // Update the body to include a #hashtag
        let cmd = UpdateEntityField::new(
            "task",
            &task_id,
            "body",
            serde_json::json!("Fix the #autotest issue"),
        );
        cmd.execute(&ctx).await.into_result().unwrap();

        // The tag entity should now exist
        let ectx = ctx.entity_context().await.unwrap();
        let tags = ectx.list("tag").await.unwrap();
        let found = tags
            .iter()
            .any(|t| t.get_str("tag_name") == Some("autotest"));
        assert!(found, "Tag entity 'autotest' should have been auto-created");
    }

    #[tokio::test]
    async fn test_update_body_does_not_duplicate_existing_tags() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Tag test")
            .with_description("Already has #existing")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // Tag 'existing' was auto-created by AddTask. Update body with same tag.
        let cmd = UpdateEntityField::new(
            "task",
            &task_id,
            "body",
            serde_json::json!("Still has #existing tag"),
        );
        cmd.execute(&ctx).await.into_result().unwrap();

        // Should still be exactly one tag entity named 'existing'
        let ectx = ctx.entity_context().await.unwrap();
        let tags = ectx.list("tag").await.unwrap();
        let count = tags
            .iter()
            .filter(|t| t.get_str("tag_name") == Some("existing"))
            .count();
        assert_eq!(count, 1, "Should not duplicate existing tag");
    }

    #[tokio::test]
    async fn test_update_entity_field_entity_not_found() {
        let (_temp, ctx) = setup().await;

        // Try to update a task that doesn't exist
        let cmd = UpdateEntityField::new(
            "task",
            "nonexistent_id",
            "title",
            serde_json::json!("value"),
        );
        let result = cmd.execute(&ctx).await.into_result();

        assert!(result.is_err(), "Should fail for nonexistent entity");
    }
}
