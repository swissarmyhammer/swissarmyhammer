//! CopyTask operation — snapshot a task's fields for the clipboard.
//!
//! This is a read-only operation that serializes the task entity into
//! clipboard JSON format. The actual clipboard write happens in the
//! Command layer (clipboard_commands.rs).

use crate::clipboard;
use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Copy a task's fields to clipboard-ready JSON.
///
/// Returns the serialized clipboard payload as a JSON string in the result.
/// The Command layer is responsible for writing this to the system clipboard.
#[operation(
    verb = "copy",
    noun = "task",
    description = "Copy a task to the clipboard"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct CopyTask {
    /// The task ID to copy.
    pub id: TaskId,
}

impl CopyTask {
    /// Create a new CopyTask operation.
    pub fn new(id: impl Into<TaskId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for CopyTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            let entity = ectx
                .read("task", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;

            // Snapshot all fields as a JSON object
            let fields = serde_json::to_value(&entity.fields)?;
            let clipboard_json =
                clipboard::serialize_to_clipboard("task", self.id.as_str(), "copy", fields);

            // Standard identity envelope plus the clipboard-specific payload
            // (the clipboard payload IS the information the caller needs).
            let mut ack = crate::task_helpers::task_mutation_ack(&entity);
            ack["copied"] = serde_json::json!(true);
            ack["clipboard_json"] = serde_json::json!(clipboard_json);
            Ok(ack)
        }
        .await
        {
            Ok(value) => ExecutionResult::Success { value },
            Err(error) => ExecutionResult::Failed { error },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::clipboard;
    use crate::task::AddTask;
    use crate::task_helpers::assert_task_mutation_ack_with;
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
    async fn test_copy_task_returns_clipboard_json() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Copy me")
            .with_description("Some description #bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = CopyTask::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Standard identity envelope plus the clipboard-specific payload.
        assert_task_mutation_ack_with(&result, task_id, &["copied", "clipboard_json"]);
        assert_eq!(result["copied"], true);

        // Verify the clipboard_json is valid and contains the task data
        let clipboard_json = result["clipboard_json"].as_str().unwrap();
        let payload = clipboard::deserialize_from_clipboard(clipboard_json)
            .expect("should deserialize clipboard payload");

        assert_eq!(payload.swissarmyhammer_clipboard.entity_type, "task");
        assert_eq!(payload.swissarmyhammer_clipboard.entity_id, task_id);
        assert_eq!(payload.swissarmyhammer_clipboard.mode, "copy");
        assert_eq!(payload.swissarmyhammer_clipboard.fields["title"], "Copy me");
    }

    #[tokio::test]
    async fn test_copy_nonexistent_task_fails() {
        let (_temp, ctx) = setup().await;

        let result = CopyTask::new("nonexistent")
            .execute(&ctx)
            .await
            .into_result();
        assert!(result.is_err());
    }
}
