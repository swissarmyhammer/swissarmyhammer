//! AddTag command

use crate::auto_color;
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_entity::Entity;
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

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;

            // Check if a tag with this name already exists
            if find_tag_entity_by_name(ectx, &self.name).await.is_some() {
                return Err(KanbanError::duplicate_id("tag", self.name.clone()));
            }

            let color = self
                .color
                .clone()
                .unwrap_or_else(|| auto_color::auto_color(&self.name).to_string());

            let tag_id = ulid::Ulid::new().to_string();
            let mut entity = Entity::new("tag", tag_id.as_str());
            entity.set("tag_name", json!(self.name));
            entity.set("color", json!(color));
            if let Some(desc) = &self.description {
                entity.set("description", json!(desc));
            }

            ectx.write(&entity).await?;

            Ok(tag_entity_to_json(&entity))
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

/// Convert a tag Entity to the API JSON format
pub(crate) fn tag_entity_to_json(entity: &Entity) -> Value {
    json!({
        "id": entity.id,
        "name": entity.get_str("tag_name").unwrap_or(""),
        "description": entity.get_str("description").unwrap_or(""),
        "color": entity.get_str("color").unwrap_or(""),
    })
}

/// Find a tag entity by its human-readable name (slug)
pub(crate) async fn find_tag_entity_by_name(
    ectx: &swissarmyhammer_entity::EntityContext,
    name: &str,
) -> Option<Entity> {
    let tags = ectx.list("tag").await.ok()?;
    tags.into_iter()
        .find(|t| t.get_str("tag_name") == Some(name))
}

/// Check if a tag with the given name exists
pub(crate) async fn tag_name_exists_entity(
    ectx: &swissarmyhammer_entity::EntityContext,
    name: &str,
) -> bool {
    find_tag_entity_by_name(ectx, name).await.is_some()
}
