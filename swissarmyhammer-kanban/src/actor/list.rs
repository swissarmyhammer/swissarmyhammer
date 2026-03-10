//! ListActors command

use crate::actor::actor_entity_to_json;
use crate::context::KanbanContext;
use crate::error::KanbanError;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List all actors
#[operation(verb = "list", noun = "actors", description = "List all actors")]
#[derive(Debug, Default, Deserialize)]
pub struct ListActors;

impl ListActors {
    /// Create a new ListActors command
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListActors {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            let all_actors = ectx.list("actor").await?;

            let actors_json: Vec<Value> = all_actors.iter().map(actor_entity_to_json).collect();

            Ok(serde_json::json!({
                "actors": actors_json,
                "count": actors_json.len()
            }))
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

        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_list_actors_empty() {
        let (_temp, ctx) = setup().await;

        let result = ListActors::new().execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["count"], 0);
        assert!(result["actors"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_actors() {
        let (_temp, ctx) = setup().await;

        AddActor::new("alice", "Alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddActor::new("assistant", "Assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListActors::new().execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["count"], 2);
    }
}
