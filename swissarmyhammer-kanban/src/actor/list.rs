//! ListActors command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::Actor;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List all actors
#[operation(
    verb = "list",
    noun = "actors",
    description = "List all actors with optional type filter"
)]
#[derive(Debug, Default, Deserialize)]
pub struct ListActors {
    /// Filter by actor type (human or agent)
    #[serde(rename = "type")]
    pub actor_type: Option<String>,
}

impl ListActors {
    pub fn new() -> Self {
        Self { actor_type: None }
    }

    pub fn humans() -> Self {
        Self {
            actor_type: Some("human".to_string()),
        }
    }

    pub fn agents() -> Self {
        Self {
            actor_type: Some("agent".to_string()),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListActors {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let all_actors = ctx.read_all_actors().await?;

            let actors_json: Vec<Value> = all_actors
                .iter()
                .filter(|a| match &self.actor_type {
                    None => true,
                    Some(t) if t == "human" => matches!(a, Actor::Human { .. }),
                    Some(t) if t == "agent" => matches!(a, Actor::Agent { .. }),
                    Some(_) => true, // Unknown type, include all
                })
                .map(|a| {
                    let mut v = serde_json::to_value(a).unwrap_or(Value::Null);
                    v["id"] = serde_json::json!(a.id());
                    v
                })
                .collect();

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

        AddActor::human("alice", "Alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddActor::agent("assistant", "Assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListActors::new().execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["count"], 2);
    }

    #[tokio::test]
    async fn test_list_actors_filter_humans() {
        let (_temp, ctx) = setup().await;

        AddActor::human("alice", "Alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddActor::agent("assistant", "Assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListActors::humans()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["count"], 1);
        assert_eq!(result["actors"][0]["type"], "human");
    }

    #[tokio::test]
    async fn test_list_actors_filter_agents() {
        let (_temp, ctx) = setup().await;

        AddActor::human("alice", "Alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddActor::agent("assistant", "Assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListActors::agents()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["count"], 1);
        assert_eq!(result["actors"][0]["type"], "agent");
    }
}
