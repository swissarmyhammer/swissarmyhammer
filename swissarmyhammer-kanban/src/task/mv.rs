//! MoveTask command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::task_helpers::task_entity_to_json;
use crate::types::{ColumnId, Ordinal, SwimlaneId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Move a task to a new position
#[operation(
    verb = "move",
    noun = "task",
    description = "Move a task to a different column or position"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct MoveTask {
    /// The task ID to move
    pub id: TaskId,
    /// Target column
    pub column: ColumnId,
    /// Optional target swimlane
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swimlane: Option<SwimlaneId>,
    /// Optional ordinal — if None, appends at end of column
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ordinal: Option<String>,
}

impl MoveTask {
    /// Create a MoveTask command to move to a column (appended at end)
    pub fn to_column(id: impl Into<TaskId>, column: impl Into<ColumnId>) -> Self {
        Self {
            id: id.into(),
            column: column.into(),
            swimlane: None,
            ordinal: None,
        }
    }

    /// Create a MoveTask command with column and swimlane (appended at end)
    pub fn to_column_and_swimlane(
        id: impl Into<TaskId>,
        column: impl Into<ColumnId>,
        swimlane: impl Into<SwimlaneId>,
    ) -> Self {
        Self {
            id: id.into(),
            column: column.into(),
            swimlane: Some(swimlane.into()),
            ordinal: None,
        }
    }

    /// Create a MoveTask with explicit ordinal placement
    pub fn with_ordinal(mut self, ordinal: impl Into<String>) -> Self {
        self.ordinal = Some(ordinal.into());
        self
    }
}

/// Auto-create a column entity if it doesn't exist. Returns a title-cased name from the slug.
fn slug_to_name(slug: &str) -> String {
    slug.split(['-', '_'])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for MoveTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value, KanbanError> = async {
            let ectx = ctx.entity_context().await?;
            let mut entity = ectx.read("task", self.id.as_str()).await?;

            // Auto-create column if it doesn't exist
            if ectx.read("column", self.column.as_str()).await.is_err() {
                let columns = ectx.list("column").await?;
                let order = columns
                    .iter()
                    .filter_map(|c| c.get("order").and_then(|v| v.as_u64()))
                    .max()
                    .map(|o| o as usize + 1)
                    .unwrap_or(0);
                let name = slug_to_name(self.column.as_str());
                let mut col_entity = Entity::new("column", self.column.as_str());
                col_entity.set("name", json!(name));
                col_entity.set("order", json!(order));
                ectx.write(&col_entity).await?;
            }

            // Auto-create swimlane if it doesn't exist
            if let Some(ref swimlane_id) = self.swimlane {
                if ectx.read("swimlane", swimlane_id.as_str()).await.is_err() {
                    let swimlanes = ectx.list("swimlane").await?;
                    let order = swimlanes
                        .iter()
                        .filter_map(|s| s.get("order").and_then(|v| v.as_u64()))
                        .max()
                        .map(|o| o as usize + 1)
                        .unwrap_or(0);
                    let name = slug_to_name(swimlane_id.as_str());
                    let mut sl_entity = Entity::new("swimlane", swimlane_id.as_str());
                    sl_entity.set("name", json!(name));
                    sl_entity.set("order", json!(order));
                    ectx.write(&sl_entity).await?;
                }
            }

            // Use explicit ordinal if provided, otherwise auto-calculate (append at end)
            let ordinal = if let Some(ref ord) = self.ordinal {
                Ordinal::from_string(ord)
            } else {
                let all_tasks = ectx.list("task").await?;
                let mut last_ordinal: Option<Ordinal> = None;

                for t in &all_tasks {
                    if t.id == self.id.as_str() {
                        continue; // Skip the task being moved
                    }
                    let t_col = t.get_str("position_column").unwrap_or("");
                    let t_swim = t.get_str("position_swimlane");
                    if t_col == self.column.as_str()
                        && t_swim == self.swimlane.as_ref().map(|s| s.as_str())
                    {
                        let ord =
                            Ordinal::from_string(t.get_str("position_ordinal").unwrap_or("a0"));
                        last_ordinal = Some(match last_ordinal {
                            None => ord,
                            Some(ref o) if ord > *o => ord,
                            Some(o) => o,
                        });
                    }
                }

                match last_ordinal {
                    Some(last) => Ordinal::after(&last),
                    None => Ordinal::first(),
                }
            };

            // Update position fields
            entity.set("position_column", json!(self.column.as_str()));
            match &self.swimlane {
                Some(s) => entity.set("position_swimlane", json!(s.as_str())),
                None => {
                    entity.remove("position_swimlane");
                }
            }
            entity.set("position_ordinal", json!(ordinal.as_str()));

            ectx.write(&entity).await?;
            Ok(task_entity_to_json(&entity))
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

    fn affected_resource_ids(&self, result: &Value) -> Vec<String> {
        result
            .get("id")
            .and_then(|v| v.as_str())
            .map(|id| vec![id.to_string()])
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::task::AddTask;
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
    async fn test_move_task_to_column() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = MoveTask::to_column(task_id, "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["position"]["column"], "done");
    }

    #[tokio::test]
    async fn test_move_task_auto_creates_column() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = MoveTask::to_column(task_id, "in-review")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["position"]["column"], "in-review");

        // Verify the column was created as an entity
        let ectx = ctx.entity_context().await.unwrap();
        let col_entity = ectx.read("column", "in-review").await.unwrap();
        assert_eq!(col_entity.get_str("name").unwrap(), "In Review");
    }
}
