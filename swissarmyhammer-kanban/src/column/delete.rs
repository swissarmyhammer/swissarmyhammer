//! DeleteColumn command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::ColumnId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Delete a column (fails if it has tasks)
#[operation(
    verb = "delete",
    noun = "column",
    description = "Delete an empty column"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct DeleteColumn {
    /// The column ID to delete
    pub id: ColumnId,
}

impl DeleteColumn {
    pub fn new(id: impl Into<ColumnId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteColumn {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
            let ectx = ctx.entity_context().await?;

            // Check column exists (read will error if not found)
            ectx.read("column", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;

            // Check for tasks in this column
            let tasks = ectx.list("task").await?;
            let task_count = tasks
                .iter()
                .filter(|t| t.get_str("position_column") == Some(self.id.as_str()))
                .count();

            if task_count > 0 {
                return Err(KanbanError::ColumnNotEmpty {
                    id: self.id.to_string(),
                    count: task_count,
                });
            }

            ectx.delete("column", self.id.as_str()).await?;

            Ok(serde_json::json!({
                "deleted": true,
                "id": self.id.to_string()
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
    use crate::column::add::AddColumn;
    use crate::column::get::GetColumn;
    use crate::error::KanbanError;
    use crate::task::AddTask;
    use tempfile::TempDir;

    /// Create a temporary kanban context with a board initialized.
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
    async fn test_delete_column_empty() {
        let (_temp, ctx) = setup().await;

        AddColumn::new("staging", "Staging")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = DeleteColumn::new("staging")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["deleted"], true);
        assert_eq!(result["id"], "staging");

        // Verify it is gone
        let get_result = GetColumn::new("staging").execute(&ctx).await.into_result();
        assert!(matches!(
            get_result,
            Err(KanbanError::ColumnNotFound { .. })
        ));
    }

    #[tokio::test]
    async fn test_delete_column_not_found() {
        let (_temp, ctx) = setup().await;

        let result = DeleteColumn::new("nonexistent")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::ColumnNotFound { .. })));
    }

    #[tokio::test]
    async fn test_delete_column_with_tasks_fails() {
        let (_temp, ctx) = setup().await;

        // Add a task to the "todo" column (the default column)
        AddTask::new("A task in todo")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Attempting to delete the non-empty "todo" column should fail
        let result = DeleteColumn::new("todo").execute(&ctx).await.into_result();

        assert!(matches!(result, Err(KanbanError::ColumnNotEmpty { .. })));
    }
}
