//! ListSwimlanes command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::swimlane::swimlane_entity_to_json;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List all swimlanes
#[operation(
    verb = "list",
    noun = "swimlanes",
    description = "List all swimlanes ordered by position"
)]
#[derive(Debug, Default, Deserialize)]
pub struct ListSwimlanes;

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListSwimlanes {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            let mut swimlanes = ectx.list("swimlane").await?;
            swimlanes
                .sort_by_key(|s| s.get("order").and_then(|v| v.as_u64()).unwrap_or(0) as usize);

            let swimlanes_json: Vec<Value> =
                swimlanes.iter().map(swimlane_entity_to_json).collect();

            Ok(serde_json::json!({
                "swimlanes": swimlanes_json,
                "count": swimlanes_json.len()
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
    use crate::board::InitBoard;
    use crate::swimlane::AddSwimlane;
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
    async fn test_list_swimlanes_empty() {
        let (_temp, ctx) = setup().await;

        let result = ListSwimlanes.execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["count"], 0);
        assert!(result["swimlanes"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_swimlanes_ordered() {
        let (_temp, ctx) = setup().await;

        AddSwimlane::new("backend", "Backend")
            .with_order(1)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        AddSwimlane::new("frontend", "Frontend")
            .with_order(0)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListSwimlanes.execute(&ctx).await.into_result().unwrap();

        let swimlanes = result["swimlanes"].as_array().unwrap();
        assert_eq!(swimlanes.len(), 2);
        // Should be ordered by `order` field — frontend (0) before backend (1)
        assert_eq!(swimlanes[0]["id"], "frontend");
        assert_eq!(swimlanes[1]["id"], "backend");
        assert_eq!(result["count"], 2);
    }
}
