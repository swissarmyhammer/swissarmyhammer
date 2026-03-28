//! MoveTask command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::task_helpers::{compute_ordinal_for_neighbors, task_entity_to_json};
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
    /// Place before this task ID (ordinal computed from neighbors)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_id: Option<TaskId>,
    /// Place after this task ID (ordinal computed from neighbors)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_id: Option<TaskId>,
}

impl MoveTask {
    /// Create a MoveTask command to move to a column (appended at end)
    pub fn to_column(id: impl Into<TaskId>, column: impl Into<ColumnId>) -> Self {
        Self {
            id: id.into(),
            column: column.into(),
            swimlane: None,
            ordinal: None,
            before_id: None,
            after_id: None,
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
            before_id: None,
            after_id: None,
        }
    }

    /// Create a MoveTask with explicit ordinal placement
    pub fn with_ordinal(mut self, ordinal: impl Into<String>) -> Self {
        self.ordinal = Some(ordinal.into());
        self
    }

    /// Place before the given task ID (ordinal computed from neighbors).
    pub fn with_before(mut self, id: impl Into<TaskId>) -> Self {
        self.before_id = Some(id.into());
        self
    }

    /// Place after the given task ID (ordinal computed from neighbors).
    pub fn with_after(mut self, id: impl Into<TaskId>) -> Self {
        self.after_id = Some(id.into());
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

            // Priority: explicit ordinal > before_id/after_id placement > append
            let ordinal = if let Some(ref ord) = self.ordinal {
                Ordinal::from_string(ord)
            } else if self.before_id.is_some() || self.after_id.is_some() {
                // Load and sort all tasks in target column (excluding moved task)
                let all_tasks = ectx.list("task").await?;
                let mut col_tasks: Vec<_> = all_tasks
                    .into_iter()
                    .filter(|t| {
                        t.get_str("position_column") == Some(self.column.as_str())
                            && t.id.as_str() != self.id.as_str()
                    })
                    .collect();
                col_tasks.sort_by(|a, b| {
                    let oa = a
                        .get_str("position_ordinal")
                        .unwrap_or(Ordinal::DEFAULT_STR);
                    let ob = b
                        .get_str("position_ordinal")
                        .unwrap_or(Ordinal::DEFAULT_STR);
                    oa.cmp(ob)
                });

                if let Some(ref ref_id) = self.before_id {
                    let ref_idx = col_tasks
                        .iter()
                        .position(|t| t.id.as_str() == ref_id.as_str());
                    match ref_idx {
                        Some(0) => {
                            let ref_ord = Ordinal::from_string(
                                col_tasks[0]
                                    .get_str("position_ordinal")
                                    .unwrap_or(Ordinal::DEFAULT_STR),
                            );
                            compute_ordinal_for_neighbors(None, Some(&ref_ord))
                        }
                        Some(idx) => {
                            let pred_ord = Ordinal::from_string(
                                col_tasks[idx - 1]
                                    .get_str("position_ordinal")
                                    .unwrap_or(Ordinal::DEFAULT_STR),
                            );
                            let ref_ord = Ordinal::from_string(
                                col_tasks[idx]
                                    .get_str("position_ordinal")
                                    .unwrap_or(Ordinal::DEFAULT_STR),
                            );
                            compute_ordinal_for_neighbors(Some(&pred_ord), Some(&ref_ord))
                        }
                        None => {
                            // Reference not found — append at end
                            compute_ordinal_for_neighbors(
                                col_tasks
                                    .last()
                                    .map(|t| {
                                        Ordinal::from_string(
                                            t.get_str("position_ordinal")
                                                .unwrap_or(Ordinal::DEFAULT_STR),
                                        )
                                    })
                                    .as_ref(),
                                None,
                            )
                        }
                    }
                } else if let Some(ref ref_id) = self.after_id {
                    let ref_idx = col_tasks
                        .iter()
                        .position(|t| t.id.as_str() == ref_id.as_str());
                    match ref_idx {
                        Some(idx) if idx == col_tasks.len() - 1 => {
                            let ref_ord = Ordinal::from_string(
                                col_tasks[idx]
                                    .get_str("position_ordinal")
                                    .unwrap_or(Ordinal::DEFAULT_STR),
                            );
                            compute_ordinal_for_neighbors(Some(&ref_ord), None)
                        }
                        Some(idx) => {
                            let ref_ord = Ordinal::from_string(
                                col_tasks[idx]
                                    .get_str("position_ordinal")
                                    .unwrap_or(Ordinal::DEFAULT_STR),
                            );
                            let succ_ord = Ordinal::from_string(
                                col_tasks[idx + 1]
                                    .get_str("position_ordinal")
                                    .unwrap_or(Ordinal::DEFAULT_STR),
                            );
                            compute_ordinal_for_neighbors(Some(&ref_ord), Some(&succ_ord))
                        }
                        None => {
                            // Reference not found — append at end
                            compute_ordinal_for_neighbors(
                                col_tasks
                                    .last()
                                    .map(|t| {
                                        Ordinal::from_string(
                                            t.get_str("position_ordinal")
                                                .unwrap_or(Ordinal::DEFAULT_STR),
                                        )
                                    })
                                    .as_ref(),
                                None,
                            )
                        }
                    }
                } else {
                    unreachable!()
                }
            } else {
                let all_tasks = ectx.list("task").await?;
                let mut last_ordinal: Option<Ordinal> = None;

                for t in &all_tasks {
                    if t.id == self.id.as_str() {
                        continue;
                    }
                    let t_col = t.get_str("position_column").unwrap_or("");
                    let t_swim = t.get_str("position_swimlane");
                    if t_col == self.column.as_str()
                        && t_swim == self.swimlane.as_ref().map(|s| s.as_str())
                    {
                        let ord = Ordinal::from_string(
                            t.get_str("position_ordinal")
                                .unwrap_or(Ordinal::DEFAULT_STR),
                        );
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

    /// Helper: add a task and return its ID.
    async fn add_task(ctx: &KanbanContext, title: &str) -> String {
        let result = AddTask::new(title)
            .execute(ctx)
            .await
            .into_result()
            .unwrap();
        result["id"].as_str().unwrap().to_string()
    }

    /// Helper: read a task's ordinal.
    async fn get_ordinal(ctx: &KanbanContext, id: &str) -> String {
        let ectx = ctx.entity_context().await.unwrap();
        let entity = ectx.read("task", id).await.unwrap();
        entity
            .get_str("position_ordinal")
            .unwrap_or("80")
            .to_string()
    }

    #[tokio::test]
    async fn test_move_task_before_id() {
        let (_temp, ctx) = setup().await;

        let a = add_task(&ctx, "A").await;
        let b = add_task(&ctx, "B").await;
        let c = add_task(&ctx, "C").await;

        // Move C before A
        MoveTask::to_column(c.as_str(), "todo")
            .with_before(a.as_str())
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let ord_c = get_ordinal(&ctx, &c).await;
        let ord_a = get_ordinal(&ctx, &a).await;
        assert!(
            ord_c < ord_a,
            "C ({}) should sort before A ({})",
            ord_c,
            ord_a
        );

        // B should be unchanged and after A
        let ord_b = get_ordinal(&ctx, &b).await;
        assert!(
            ord_a < ord_b,
            "A ({}) should sort before B ({})",
            ord_a,
            ord_b
        );
    }

    #[tokio::test]
    async fn test_move_task_after_id() {
        let (_temp, ctx) = setup().await;

        let a = add_task(&ctx, "A").await;
        let _b = add_task(&ctx, "B").await;
        let c = add_task(&ctx, "C").await;

        // Move A after C
        MoveTask::to_column(a.as_str(), "todo")
            .with_after(c.as_str())
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let ord_a = get_ordinal(&ctx, &a).await;
        let ord_c = get_ordinal(&ctx, &c).await;
        assert!(
            ord_a > ord_c,
            "A ({}) should sort after C ({})",
            ord_a,
            ord_c
        );
    }

    #[tokio::test]
    async fn test_move_task_before_id_between() {
        let (_temp, ctx) = setup().await;

        let a = add_task(&ctx, "A").await;
        let b = add_task(&ctx, "B").await;
        let c = add_task(&ctx, "C").await;

        // Move C before B (between A and B)
        MoveTask::to_column(c.as_str(), "todo")
            .with_before(b.as_str())
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let ord_a = get_ordinal(&ctx, &a).await;
        let ord_c = get_ordinal(&ctx, &c).await;
        let ord_b = get_ordinal(&ctx, &b).await;
        assert!(ord_a < ord_c, "A ({}) < C ({})", ord_a, ord_c);
        assert!(ord_c < ord_b, "C ({}) < B ({})", ord_c, ord_b);
    }

    #[tokio::test]
    async fn test_move_task_before_id_not_found_appends() {
        let (_temp, ctx) = setup().await;

        let a = add_task(&ctx, "A").await;

        // Move A with before_id referencing nonexistent task — should append
        let result = MoveTask::to_column(a.as_str(), "todo")
            .with_before("nonexistent")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["position"]["column"], "todo");
    }

    #[tokio::test]
    async fn test_move_task_ordinal_takes_precedence() {
        let (_temp, ctx) = setup().await;

        let a = add_task(&ctx, "A").await;
        let b = add_task(&ctx, "B").await;

        // Get B's ordinal so we can verify before_id was NOT used
        let ord_b = get_ordinal(&ctx, &b).await;

        // Set both ordinal and before_id — ordinal should win.
        // Use Ordinal::after(B) so we get a valid ordinal that's clearly after B,
        // while before_id would place us before B.
        let target_ord = Ordinal::after(&Ordinal::from_string(&ord_b));
        let mut cmd = MoveTask::to_column(a.as_str(), "todo").with_before(b.as_str());
        cmd.ordinal = Some(target_ord.as_str().to_string());

        cmd.execute(&ctx).await.into_result().unwrap();

        let ord_a = get_ordinal(&ctx, &a).await;
        // Ordinal wins: A should be AFTER B, not before it
        assert!(
            ord_a > ord_b,
            "ordinal should take precedence over before_id: A ({}) > B ({})",
            ord_a,
            ord_b
        );
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
