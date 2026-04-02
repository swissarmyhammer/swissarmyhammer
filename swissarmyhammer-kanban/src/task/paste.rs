//! PasteTask operation — create a new task from clipboard JSON.
//!
//! Deserializes a clipboard payload, creates a new task entity with a fresh
//! ID, copies relevant fields, and places it in the specified column/position.

use crate::auto_color;
use crate::clipboard;
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::tag::tag_name_exists_entity;
use crate::task_helpers::{compute_ordinal_for_neighbors, task_entity_to_json};
use crate::types::{ColumnId, Ordinal, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Paste a task from clipboard JSON into the specified column.
///
/// The `clipboard_json` field is populated by the Command layer from the
/// system clipboard. A new task is created with a fresh ULID; position
/// fields are computed from the target column and optional `after_id`.
#[operation(
    verb = "paste",
    noun = "task",
    description = "Paste a task from the clipboard"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct PasteTask {
    /// Target column for the pasted task.
    pub column: ColumnId,
    /// Place the pasted task after this task ID. If `None`, appends at end.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_id: Option<TaskId>,
    /// Clipboard JSON string containing the entity snapshot.
    pub clipboard_json: String,
}

impl PasteTask {
    /// Create a new PasteTask operation.
    pub fn new(
        column: impl Into<ColumnId>,
        after_id: Option<TaskId>,
        clipboard_json: String,
    ) -> Self {
        Self {
            column: column.into(),
            after_id,
            clipboard_json,
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for PasteTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            // Deserialize and validate clipboard payload
            let payload =
                clipboard::deserialize_from_clipboard(&self.clipboard_json).ok_or_else(|| {
                    KanbanError::parse("clipboard does not contain valid swissarmyhammer data")
                })?;

            let data = &payload.swissarmyhammer_clipboard;
            if data.entity_type != "task" {
                return Err(KanbanError::parse(format!(
                    "clipboard contains '{}', expected 'task'",
                    data.entity_type
                )));
            }

            let ectx = ctx.entity_context().await?;

            // Compute ordinal for the new task
            let ordinal = if let Some(ref after_task_id) = self.after_id {
                // Load all tasks in the target column, sorted by ordinal
                let all_tasks = ectx.list("task").await?;
                let mut col_tasks: Vec<_> = all_tasks
                    .into_iter()
                    .filter(|t| t.get_str("position_column") == Some(self.column.as_str()))
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

                let ref_idx = col_tasks
                    .iter()
                    .position(|t| t.id.as_str() == after_task_id.as_str());
                match ref_idx {
                    Some(idx) if idx == col_tasks.len() - 1 => {
                        // After the last task — append
                        let ref_ord = Ordinal::from_string(
                            col_tasks[idx]
                                .get_str("position_ordinal")
                                .unwrap_or(Ordinal::DEFAULT_STR),
                        );
                        compute_ordinal_for_neighbors(Some(&ref_ord), None)
                    }
                    Some(idx) => {
                        // Between ref and successor
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
                // No after_id — append at end of column
                let all_tasks = ectx.list("task").await?;
                let mut last_ordinal: Option<Ordinal> = None;

                for t in &all_tasks {
                    let t_col = t.get_str("position_column").unwrap_or("");
                    if t_col == self.column.as_str() {
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

            // Create new entity with fresh ID
            let new_id = TaskId::new();
            let mut entity = Entity::new("task", new_id.as_str());

            // Copy fields from clipboard snapshot
            let fields_obj = data
                .fields
                .as_object()
                .ok_or_else(|| KanbanError::parse("clipboard fields is not an object"))?;

            // Fields to copy from the source entity
            let copy_fields = [
                "title",
                "body",
                "assignees",
                "depends_on",
                "position_swimlane",
            ];

            for field_name in &copy_fields {
                if let Some(value) = fields_obj.get(*field_name) {
                    // Skip null values and empty strings
                    if !value.is_null() {
                        entity.set(*field_name, value.clone());
                    }
                }
            }

            // Set position fields for the new location
            entity.set("position_column", json!(self.column.as_str()));
            entity.set("position_ordinal", json!(ordinal.as_str()));

            // Ensure title is set (fallback to "Pasted task" if missing)
            if entity.get_str("title").is_none() {
                entity.set("title", json!("Pasted task"));
            }

            // Ensure body is set
            if entity.get("body").is_none() {
                entity.set("body", json!(""));
            }

            ectx.write(&entity).await?;

            // Auto-create Tag entities for any #tag patterns in the body
            let body = entity.get_str("body").unwrap_or("");
            let tags = crate::tag_parser::parse_tags(body);
            for tag_name in &tags {
                if !tag_name_exists_entity(&ectx, tag_name).await {
                    let color = auto_color::auto_color(tag_name).to_string();
                    let tag_id = ulid::Ulid::new().to_string();
                    let mut tag_entity = Entity::new("tag", tag_id.as_str());
                    tag_entity.set("tag_name", json!(tag_name));
                    tag_entity.set("color", json!(color));
                    ectx.write(&tag_entity).await?;
                }
            }

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
    use crate::clipboard;
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

    /// Helper: add a task and return its ID.
    async fn add_task(ctx: &KanbanContext, title: &str) -> String {
        let result = AddTask::new(title)
            .execute(ctx)
            .await
            .into_result()
            .unwrap();
        result["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn test_paste_creates_new_task_from_clipboard() {
        let (_temp, ctx) = setup().await;

        // Create clipboard JSON manually
        let fields = json!({
            "title": "Original task",
            "body": "Task body with #feature",
            "assignees": ["alice"],
        });
        let clipboard_json = clipboard::serialize_to_clipboard("task", "01OLD", "copy", fields);

        let result = PasteTask::new("todo", None, clipboard_json)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["title"], "Original task");
        assert_eq!(result["description"], "Task body with #feature");
        assert_eq!(result["position"]["column"], "todo");

        // Verify the new task has a different ID from the source
        let new_id = result["id"].as_str().unwrap();
        assert_ne!(new_id, "01OLD");

        // Verify the task actually exists on disk
        let ectx = ctx.entity_context().await.unwrap();
        let entity = ectx.read("task", new_id).await.unwrap();
        assert_eq!(entity.get_str("title").unwrap(), "Original task");
    }

    #[tokio::test]
    async fn test_paste_with_after_id_positions_correctly() {
        let (_temp, ctx) = setup().await;

        let id_a = add_task(&ctx, "A").await;
        let id_b = add_task(&ctx, "B").await;

        // Paste after A (before B)
        let fields = json!({"title": "Pasted"});
        let clipboard_json = clipboard::serialize_to_clipboard("task", "01SRC", "copy", fields);

        let result = PasteTask::new("todo", Some(TaskId::from_string(&id_a)), clipboard_json)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let pasted_ordinal = result["position"]["ordinal"].as_str().unwrap();

        // Get ordinals of A and B
        let ectx = ctx.entity_context().await.unwrap();
        let a = ectx.read("task", &id_a).await.unwrap();
        let b = ectx.read("task", &id_b).await.unwrap();
        let ord_a = a
            .get_str("position_ordinal")
            .unwrap_or(Ordinal::DEFAULT_STR);
        let ord_b = b
            .get_str("position_ordinal")
            .unwrap_or(Ordinal::DEFAULT_STR);

        assert!(
            pasted_ordinal > ord_a,
            "pasted ({}) should be after A ({})",
            pasted_ordinal,
            ord_a
        );
        assert!(
            pasted_ordinal < ord_b,
            "pasted ({}) should be before B ({})",
            pasted_ordinal,
            ord_b
        );
    }

    #[tokio::test]
    async fn test_paste_invalid_clipboard_returns_error() {
        let (_temp, ctx) = setup().await;

        let result = PasteTask::new("todo", None, "not valid json".to_string())
            .execute(&ctx)
            .await
            .into_result();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("clipboard"),
            "error should mention clipboard: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_paste_wrong_entity_type_returns_error() {
        let (_temp, ctx) = setup().await;

        let fields = json!({"name": "A tag"});
        let clipboard_json = clipboard::serialize_to_clipboard("tag", "01TAG", "copy", fields);

        let result = PasteTask::new("todo", None, clipboard_json)
            .execute(&ctx)
            .await
            .into_result();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("tag"),
            "error should mention wrong type: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_paste_after_last_task_appends() {
        let (_temp, ctx) = setup().await;

        let id_a = add_task(&ctx, "A").await;
        let id_b = add_task(&ctx, "B").await;

        // Paste after B (the last task)
        let fields = json!({"title": "After last"});
        let clipboard_json = clipboard::serialize_to_clipboard("task", "01SRC", "copy", fields);

        let result = PasteTask::new("todo", Some(TaskId::from_string(&id_b)), clipboard_json)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let pasted_ordinal = result["position"]["ordinal"].as_str().unwrap();

        let ectx = ctx.entity_context().await.unwrap();
        let b = ectx.read("task", &id_b).await.unwrap();
        let ord_b = b
            .get_str("position_ordinal")
            .unwrap_or(Ordinal::DEFAULT_STR);

        assert!(
            pasted_ordinal > ord_b,
            "pasted ({}) should be after B ({})",
            pasted_ordinal,
            ord_b
        );

        // A still exists and ordinal unchanged
        let a = ectx.read("task", &id_a).await.unwrap();
        assert!(a.get_str("title").is_some());
    }

    #[tokio::test]
    async fn test_paste_copies_depends_on() {
        let (_temp, ctx) = setup().await;

        let dep_id = add_task(&ctx, "Dependency").await;

        let fields = json!({
            "title": "Dependent task",
            "depends_on": [dep_id],
        });
        let clipboard_json = clipboard::serialize_to_clipboard("task", "01SRC", "copy", fields);

        let result = PasteTask::new("todo", None, clipboard_json)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // depends_on should be preserved
        let deps = result["depends_on"].as_array().unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0], dep_id);
    }

    #[tokio::test]
    async fn test_paste_affected_resource_ids() {
        let (_temp, ctx) = setup().await;

        let fields = json!({"title": "Paste me"});
        let clipboard_json = clipboard::serialize_to_clipboard("task", "01SRC", "copy", fields);

        let op = PasteTask::new("todo", None, clipboard_json);
        let exec_result = op.execute(&ctx).await;
        let value = exec_result.into_result().unwrap();

        let ids = op.affected_resource_ids(&value);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], value["id"].as_str().unwrap());
    }

    #[tokio::test]
    async fn test_paste_after_id_not_found_appends() {
        let (_temp, ctx) = setup().await;

        let _id_a = add_task(&ctx, "A").await;

        // Use a nonexistent after_id — should append at end
        let fields = json!({"title": "Appended"});
        let clipboard_json = clipboard::serialize_to_clipboard("task", "01SRC", "copy", fields);

        let result = PasteTask::new(
            "todo",
            Some(TaskId::from_string("01NONEXISTENT")),
            clipboard_json,
        )
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

        assert_eq!(result["title"], "Appended");
        assert_eq!(result["position"]["column"], "todo");
    }

    #[tokio::test]
    async fn test_paste_multiple_times_creates_distinct_tasks() {
        let (_temp, ctx) = setup().await;

        let fields = json!({"title": "Duplicated"});
        let clipboard_json = clipboard::serialize_to_clipboard("task", "01SRC", "copy", fields);

        let result1 = PasteTask::new("todo", None, clipboard_json.clone())
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let result2 = PasteTask::new("todo", None, clipboard_json)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let id1 = result1["id"].as_str().unwrap();
        let id2 = result2["id"].as_str().unwrap();
        assert_ne!(id1, id2, "each paste should create a task with a unique ID");

        // Both should exist
        let ectx = ctx.entity_context().await.unwrap();
        let tasks = ectx.list("task").await.unwrap();
        assert_eq!(tasks.len(), 2);
    }
}
