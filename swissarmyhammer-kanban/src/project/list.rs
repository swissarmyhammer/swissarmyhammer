//! ListProjects command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::project::add::project_entity_to_json;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List all projects ordered by their `order` field.
#[operation(
    verb = "list",
    noun = "projects",
    description = "List all projects ordered by position"
)]
#[derive(Debug, Default, Deserialize)]
pub struct ListProjects;

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListProjects {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            let mut projects = ectx.list("project").await?;
            projects.sort_by_key(|p| p.get("order").and_then(|v| v.as_u64()).unwrap_or(0) as usize);

            let projects_json: Vec<Value> = projects.iter().map(project_entity_to_json).collect();

            Ok(serde_json::json!({
                "projects": projects_json,
                "count": projects_json.len()
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
    use crate::project::add::AddProject;
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
    async fn test_list_projects_empty() {
        let (_temp, ctx) = setup().await;

        let result = ListProjects.execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["count"], 0);
        assert_eq!(result["projects"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_list_projects_includes_count() {
        let (_temp, ctx) = setup().await;

        AddProject::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddProject::new("frontend", "Frontend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListProjects.execute(&ctx).await.into_result().unwrap();

        let count = result["count"].as_u64().unwrap();
        let projects_len = result["projects"].as_array().unwrap().len() as u64;
        assert_eq!(count, projects_len);
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_list_projects_sorted_by_order() {
        let (_temp, ctx) = setup().await;

        AddProject::new("zz-last", "ZZ Last")
            .with_order(100)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        AddProject::new("aa-first", "AA First")
            .with_order(0)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListProjects.execute(&ctx).await.into_result().unwrap();

        let projects = result["projects"].as_array().unwrap();
        let orders: Vec<u64> = projects
            .iter()
            .map(|p| p["order"].as_u64().unwrap_or(0))
            .collect();

        // Verify that the list is sorted ascending by order
        let mut sorted = orders.clone();
        sorted.sort();
        assert_eq!(orders, sorted);
    }
}
