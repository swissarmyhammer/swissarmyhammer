//! AddActor command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::{Actor, ActorId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Actor type for creation
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorType {
    Human,
    Agent,
}

/// Add a new actor (person or agent)
///
/// Actors are stored as separate files in `.kanban/actors/`.
/// Use `ensure: true` for idempotent registration (returns existing actor if found).
#[operation(
    verb = "add",
    noun = "actor",
    description = "Add a new actor (person or agent)"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct AddActor {
    /// The actor ID (slug)
    pub id: ActorId,
    /// The actor display name
    pub name: String,
    /// The actor type (human or agent)
    #[serde(rename = "type")]
    pub actor_type: ActorType,
    /// If true, return existing actor instead of error if ID exists (idempotent)
    #[serde(default)]
    pub ensure: bool,
}

impl AddActor {
    /// Create a new AddActor command for a human
    pub fn human(id: impl Into<ActorId>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            actor_type: ActorType::Human,
            ensure: false,
        }
    }

    /// Create a new AddActor command for an agent
    pub fn agent(id: impl Into<ActorId>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            actor_type: ActorType::Agent,
            ensure: false,
        }
    }

    /// Make this registration idempotent (return existing if found)
    pub fn with_ensure(mut self) -> Self {
        self.ensure = true;
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddActor {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
            // Check if actor already exists
            if ctx.actor_exists(&self.id).await {
                if self.ensure {
                    // Idempotent mode: return existing actor
                    let actor = ctx.read_actor(&self.id).await?;
                    return Ok(serde_json::json!({
                        "actor": actor,
                        "created": false,
                        "message": "Actor already exists"
                    }));
                }
                return Err(KanbanError::duplicate_id("actor", self.id.to_string()));
            }

            let actor = match self.actor_type {
                ActorType::Human => Actor::Human {
                    id: self.id.clone(),
                    name: self.name.clone(),
                },
                ActorType::Agent => Actor::Agent {
                    id: self.id.clone(),
                    name: self.name.clone(),
                },
            };

            ctx.write_actor(&actor).await?;

            Ok(serde_json::json!({
                "actor": actor,
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
    async fn test_add_human_actor() {
        let (_temp, ctx) = setup().await;

        let result = AddActor::human("alice", "Alice Smith")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["created"], true);
        assert_eq!(result["actor"]["type"], "human");
        assert_eq!(result["actor"]["id"], "alice");
        assert_eq!(result["actor"]["name"], "Alice Smith");
    }

    #[tokio::test]
    async fn test_add_agent_actor() {
        let (_temp, ctx) = setup().await;

        let result = AddActor::agent("assistant", "AI Assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["created"], true);
        assert_eq!(result["actor"]["type"], "agent");
        assert_eq!(result["actor"]["id"], "assistant");
    }

    #[tokio::test]
    async fn test_add_duplicate_actor_errors() {
        let (_temp, ctx) = setup().await;

        AddActor::human("alice", "Alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = AddActor::human("alice", "Alice Duplicate")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::DuplicateId { .. })));
    }

    #[tokio::test]
    async fn test_add_actor_with_ensure_idempotent() {
        let (_temp, ctx) = setup().await;

        // First add
        let result1 = AddActor::agent("assistant", "AI Assistant")
            .with_ensure()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result1["created"], true);

        // Second add with ensure - should return existing
        let result2 = AddActor::agent("assistant", "Different Name")
            .with_ensure()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result2["created"], false);
        assert_eq!(result2["actor"]["name"], "AI Assistant"); // Original name preserved
    }
}
