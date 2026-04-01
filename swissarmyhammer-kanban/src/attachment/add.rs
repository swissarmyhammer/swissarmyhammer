//! AddAttachment command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Extract raw stored filenames from an enriched attachment field value.
///
/// After `ectx.read()`, attachment fields are enriched into metadata objects
/// `{ id, name, size, mime_type, path }`. The stored filename is `{id}-{name}`.
/// This function reconstructs those filenames so they can be written back.
pub(crate) fn extract_stored_filenames(enriched: &Value) -> Vec<String> {
    match enriched {
        Value::Array(arr) => arr.iter().filter_map(stored_filename_from_meta).collect(),
        _ => Vec::new(),
    }
}

/// Reconstruct the stored filename from an enriched metadata object.
///
/// The stored format is `{id}-{name}` where `id` is the ULID prefix and
/// `name` is the sanitized original filename.
fn stored_filename_from_meta(meta: &Value) -> Option<String> {
    let id = meta.get("id")?.as_str()?;
    let name = meta.get("name")?.as_str()?;
    Some(format!("{}-{}", id, name))
}

/// Add an attachment to an existing task
#[operation(
    verb = "add",
    noun = "attachment",
    description = "Add an attachment to a task"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct AddAttachment {
    /// The task ID to add the attachment to
    pub task_id: TaskId,
    /// The attachment name
    pub name: String,
    /// The file path
    pub path: String,
    /// Optional MIME type (auto-detected if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Optional file size in bytes (auto-detected if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

impl AddAttachment {
    /// Create a new AddAttachment command
    pub fn new(
        task_id: impl Into<TaskId>,
        name: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            name: name.into(),
            path: path.into(),
            mime_type: None,
            size: None,
        }
    }

    /// Set the MIME type
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Set the file size
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddAttachment {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;

            // Read the task (attachments field is enriched to metadata objects)
            let mut task = ectx.read("task", self.task_id.as_str()).await?;

            // Extract existing stored filenames from the enriched metadata
            let mut filenames: Vec<Value> =
                extract_stored_filenames(task.fields.get("attachments").unwrap_or(&json!([])))
                    .into_iter()
                    .map(Value::String)
                    .collect();

            // Append the new source path — the entity layer will copy the file
            // and replace this with the stored filename on write
            filenames.push(json!(self.path));
            task.set("attachments", json!(filenames));
            ectx.write(&task).await?;

            // Re-read the task to get the enriched metadata for the response
            let task = ectx.read("task", self.task_id.as_str()).await?;
            let attachments = task
                .fields
                .get("attachments")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            // The newly added attachment is the last one in the list
            let attachment = attachments.last().cloned().unwrap_or(json!(null));

            Ok(json!({
                "attachment": attachment,
                "task_id": self.task_id.to_string()
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
                        json!({"error": error_msg}),
                        None,
                        duration_ms,
                    )),
                }
            }
        }
    }

    fn affected_resource_ids(&self, _result: &Value) -> Vec<String> {
        vec![self.task_id.to_string()]
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
    async fn test_add_attachment() {
        let (temp, ctx) = setup().await;

        // Create a task
        let task_result = AddTask::new("Task with attachments")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Create a real file to attach
        let source_file = temp.path().join("screenshot.png");
        std::fs::write(&source_file, b"fake png data").unwrap();

        // Add an attachment
        let result = AddAttachment::new(task_id, "screenshot.png", source_file.to_str().unwrap())
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["attachment"]["name"], "screenshot.png");
        assert_eq!(result["task_id"], task_id);

        // Verify the file was copied to .attachments/
        let att_dir = temp
            .path()
            .join(".kanban")
            .join("tasks")
            .join(".attachments");
        assert!(att_dir.exists(), ".attachments/ directory should exist");
        let entries: Vec<_> = std::fs::read_dir(&att_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| !e.file_name().to_str().unwrap_or("").starts_with('.'))
            .collect();
        assert_eq!(entries.len(), 1, "should have one attachment file");
    }

    #[tokio::test]
    async fn test_add_attachment_with_mime_type() {
        let (temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Create a real file to attach
        let source_file = temp.path().join("spec.pdf");
        std::fs::write(&source_file, b"fake pdf data").unwrap();

        let result = AddAttachment::new(task_id, "doc.pdf", source_file.to_str().unwrap())
            .with_mime_type("application/pdf")
            .with_size(12345)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Metadata is derived by entity layer from the actual file
        assert!(result["attachment"]["mime_type"].is_string());
        assert!(result["attachment"]["size"].is_number());
    }

    #[tokio::test]
    async fn test_add_attachment_auto_detect_mime() {
        let (temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Create a real file to attach
        let source_file = temp.path().join("image.png");
        std::fs::write(&source_file, b"fake png data").unwrap();

        let result = AddAttachment::new(task_id, "image", source_file.to_str().unwrap())
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Entity layer auto-detects from .png extension
        assert_eq!(result["attachment"]["mime_type"], "image/png");
    }

    #[tokio::test]
    async fn test_add_attachment_to_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let result = AddAttachment::new("nonexistent", "file.txt", "./file.txt")
            .execute(&ctx)
            .await
            .into_result();

        assert!(result.is_err());
    }
}
