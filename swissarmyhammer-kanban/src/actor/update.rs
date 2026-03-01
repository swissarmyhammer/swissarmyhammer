//! UpdateActor command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{Actor, ActorId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Update an actor
#[operation(
    verb = "update",
    noun = "actor",
    description = "Update an actor's name"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateActor {
    /// The actor ID to update
    pub id: ActorId,
    /// New actor name
    pub name: Option<String>,
}

impl UpdateActor {
    pub fn new(id: impl Into<ActorId>) -> Self {
        Self {
            id: id.into(),
            name: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateActor {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let actor = ctx.read_actor(&self.id).await?;

            let updated_actor = if let Some(name) = &self.name {
                // Update the name while preserving the type
                match actor {
                    Actor::Human { id, .. } => Actor::Human {
                        id,
                        name: name.clone(),
                    },
                    Actor::Agent { id, .. } => Actor::Agent {
                        id,
                        name: name.clone(),
                    },
                }
            } else {
                actor
            };

            ctx.write_actor(&updated_actor).await?;

            let mut result = serde_json::to_value(&updated_actor)?;
            result["id"] = serde_json::json!(updated_actor.id());
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
    use crate::actor::AddActor;
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
    async fn test_update_actor_name() {
        let (_temp, ctx) = setup().await;

        AddActor::human("alice", "Alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = UpdateActor::new("alice")
            .with_name("Alice Smith")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["name"], "Alice Smith");
        assert_eq!(result["type"], "human"); // Type preserved
    }

    #[tokio::test]
    async fn test_update_nonexistent_actor() {
        let (_temp, ctx) = setup().await;

        let result = UpdateActor::new("nonexistent")
            .with_name("New Name")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::ActorNotFound { .. })));
    }
}
