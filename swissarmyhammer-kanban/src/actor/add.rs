//! AddActor command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::ActorId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Convert an actor entity to JSON output
pub(crate) fn actor_entity_to_json(entity: &Entity) -> Value {
    json!({
        "id": entity.id,
        "name": entity.get_str("name").unwrap_or(""),
    })
}

/// Add a new actor
///
/// Actors are stored as separate files in `.kanban/actors/`.
/// Use `ensure: true` for idempotent registration (returns existing actor if found).
#[operation(
    verb = "add",
    noun = "actor",
    description = "Add a new actor"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct AddActor {
    /// The actor ID (slug)
    pub id: ActorId,
    /// The actor display name
    pub name: String,
    /// If true, return existing actor instead of error if ID exists (idempotent)
    #[serde(default)]
    pub ensure: bool,
    /// Optional display color (6-char hex without #)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Optional avatar (data URI or URL)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
}

impl AddActor {
    /// Create a new AddActor command
    pub fn new(id: impl Into<ActorId>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            ensure: false,
            color: None,
            avatar: None,
        }
    }

    /// Make this registration idempotent (return existing if found)
    pub fn with_ensure(mut self) -> Self {
        self.ensure = true;
        self
    }

    /// Set the display color (6-char hex without #)
    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Set the avatar (data URI or URL)
    pub fn with_avatar(mut self, avatar: impl Into<String>) -> Self {
        self.avatar = Some(avatar.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddActor {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
            let ectx = ctx.entity_context().await?;

            // Check if actor already exists
            if let Ok(mut existing) = ectx.read("actor", self.id.as_str()).await {
                if self.ensure {
                    // Idempotent mode: update any changed mutable fields, then return
                    let mut updated = false;
                    let current_name = existing.get_str("name").unwrap_or("").to_string();
                    if current_name != self.name {
                        existing.set("name", json!(self.name));
                        updated = true;
                    }
                    if let Some(ref color) = self.color {
                        let current = existing.get_str("color").map(|s| s.to_string());
                        if current.as_deref() != Some(color.as_str()) {
                            existing.set("color", json!(color));
                            updated = true;
                        }
                    }
                    if let Some(ref avatar) = self.avatar {
                        let current = existing.get_str("avatar").map(|s| s.to_string());
                        if current.as_deref() != Some(avatar.as_str()) {
                            existing.set("avatar", json!(avatar));
                            updated = true;
                        }
                    }
                    if updated {
                        ectx.write(&existing).await?;
                    }
                    return Ok(json!({
                        "actor": actor_entity_to_json(&existing),
                        "created": false,
                        "updated": updated,
                        "message": if updated { "Actor updated" } else { "Actor already exists" }
                    }));
                }
                return Err(KanbanError::duplicate_id("actor", self.id.to_string()));
            }

            let mut entity = Entity::new("actor", self.id.as_str());
            entity.set("name", json!(self.name));
            if let Some(ref color) = self.color {
                entity.set("color", json!(color));
            }
            if let Some(ref avatar) = self.avatar {
                entity.set("avatar", json!(avatar));
            }

            ectx.write(&entity).await?;

            Ok(json!({
                "actor": actor_entity_to_json(&entity),
                "created": true
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
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
    async fn test_add_actor() {
        let (_temp, ctx) = setup().await;

        let result = AddActor::new("alice", "Alice Smith")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["created"], true);
        assert_eq!(result["actor"]["id"], "alice");
        assert_eq!(result["actor"]["name"], "Alice Smith");
    }

    #[tokio::test]
    async fn test_add_duplicate_actor_errors() {
        let (_temp, ctx) = setup().await;

        AddActor::new("alice", "Alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = AddActor::new("alice", "Alice Duplicate")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::DuplicateId { .. })));
    }

    #[tokio::test]
    async fn test_add_actor_with_ensure_idempotent() {
        let (_temp, ctx) = setup().await;

        // First add
        let result1 = AddActor::new("assistant", "AI Assistant")
            .with_ensure()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result1["created"], true);

        // Second add with ensure and same name - should return existing without update
        let result2 = AddActor::new("assistant", "AI Assistant")
            .with_ensure()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result2["created"], false);
        assert_eq!(result2["updated"], false);
        assert_eq!(result2["actor"]["name"], "AI Assistant");
    }

    #[tokio::test]
    async fn test_ensure_updates_changed_name() {
        let (_temp, ctx) = setup().await;

        // First add
        AddActor::new("assistant", "AI Assistant")
            .with_ensure()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Second add with ensure and different name - should update
        let result = AddActor::new("assistant", "New Name")
            .with_ensure()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["created"], false);
        assert_eq!(result["updated"], true);
        assert_eq!(result["actor"]["name"], "New Name");
        assert_eq!(result["message"], "Actor updated");
    }

    #[tokio::test]
    async fn test_ensure_updates_changed_color() {
        let (_temp, ctx) = setup().await;

        // First add with color
        AddActor::new("alice", "Alice")
            .with_ensure()
            .with_color("ff0000")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Second add with different color - should update
        let result = AddActor::new("alice", "Alice")
            .with_ensure()
            .with_color("00ff00")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["created"], false);
        assert_eq!(result["updated"], true);
        assert_eq!(result["message"], "Actor updated");
    }

    #[tokio::test]
    async fn test_ensure_updates_changed_avatar() {
        let (_temp, ctx) = setup().await;

        // First add with avatar
        AddActor::new("alice", "Alice")
            .with_ensure()
            .with_avatar("data:image/svg+xml;base64,old")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Second add with different avatar - should update
        let result = AddActor::new("alice", "Alice")
            .with_ensure()
            .with_avatar("data:image/svg+xml;base64,new")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["created"], false);
        assert_eq!(result["updated"], true);
    }
}
