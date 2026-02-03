//! GetActor command


use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::ActorId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get an actor by ID
#[operation(verb = "get", noun = "actor", description = "Get an actor by ID")]
#[derive(Debug, Deserialize)]
pub struct GetActor {
    /// The actor ID to retrieve
    pub id: ActorId,
}

impl GetActor {
    pub fn new(id: impl Into<ActorId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetActor {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let actor = ctx.read_actor(&self.id).await?;
            Ok(serde_json::to_value(actor)?)
        }
        .await
        {
            Ok(value) => ExecutionResult::Unlogged { value },
            Err(error) => ExecutionResult::Failed {
                error,
                log_entry: None,
            },
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

        InitBoard::new("Test").execute(&ctx).await.into_result().unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_get_actor() {
        let (_temp, ctx) = setup().await;

        AddActor::human("alice", "Alice Smith")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = GetActor::new("alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "alice");
        assert_eq!(result["name"], "Alice Smith");
        assert_eq!(result["type"], "human");
    }

    #[tokio::test]
    async fn test_get_nonexistent_actor() {
        let (_temp, ctx) = setup().await;

        let result = GetActor::new("nonexistent")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::ActorNotFound { .. })));
    }
}
