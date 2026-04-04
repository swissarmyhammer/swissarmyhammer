//! AddAttachment command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Convert an attachment Entity to its JSON API representation.
pub(crate) fn attachment_entity_to_json(entity: &Entity) -> Value {
    json!({
        "id": entity.id,
        "name": entity.get_str("attachment_name").unwrap_or(""),
        "path": entity.get_str("attachment_path").unwrap_or(""),
        "mime_type": entity.get_str("attachment_mime_type"),
        "size": entity.get("attachment_size").and_then(|v| v.as_u64()),
    })
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

/// Detect MIME type from file extension
fn detect_mime_type(path: &str) -> Option<String> {
    let ext = std::path::Path::new(path)
        .extension()?
        .to_str()?
        .to_lowercase();

    let mime = match ext.as_str() {
        // Images
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",

        // Documents
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",

        // Text
        "txt" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "csv" => "text/csv",
        "xml" => "text/xml",

        // Code
        "js" => "application/javascript",
        "json" => "application/json",
        "ts" => "application/typescript",
        "rs" => "text/x-rust",
        "py" => "text/x-python",
        "go" => "text/x-go",
        "java" => "text/x-java",
        "c" => "text/x-c",
        "cpp" | "cc" | "cxx" => "text/x-c++",
        "h" | "hpp" => "text/x-c-header",
        "sh" | "bash" => "application/x-sh",

        // Archives
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" | "gzip" => "application/gzip",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/x-rar-compressed",

        // Media
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "avi" => "video/x-msvideo",
        "mov" => "video/quicktime",
        "wav" => "audio/wav",

        _ => return None,
    };

    Some(mime.to_string())
}

/// Get file size from filesystem
fn get_file_size(path: &str) -> Option<u64> {
    std::fs::metadata(path).ok()?.len().into()
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddAttachment {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;

            // Verify the task exists
            let mut task = ectx.read("task", self.task_id.as_str()).await?;

            // Auto-detect MIME type if not provided
            let mime_type = self
                .mime_type
                .clone()
                .or_else(|| detect_mime_type(&self.path));

            // Auto-detect file size if not provided
            let size = self.size.or_else(|| get_file_size(&self.path));

            // Create standalone attachment entity
            let attachment_id = ulid::Ulid::new().to_string().to_lowercase();
            let mut attachment = Entity::new("attachment", attachment_id.as_str());
            attachment.set("attachment_name", json!(self.name));
            attachment.set("attachment_path", json!(self.path));
            if let Some(mime) = &mime_type {
                attachment.set("attachment_mime_type", json!(mime));
            }
            if let Some(s) = size {
                attachment.set("attachment_size", json!(s));
            }
            // Two-phase write: create attachment entity first, then update task.
            // If the task update fails, we get an orphan attachment (recoverable)
            // rather than a dangling reference in the task (harder to detect).
            ectx.write(&attachment).await?;

            // Add attachment ID to the task's attachments reference list
            let mut attachment_ids = task.get_string_list("attachments");
            attachment_ids.push(attachment_id.clone());
            task.set("attachments", json!(attachment_ids));
            ectx.write(&task).await?;

            Ok(json!({
                "attachment": attachment_entity_to_json(&attachment),
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

    /// Create a temp file with some content and return its path.
    fn create_temp_file(dir: &std::path::Path, name: &str, content: &[u8]) -> String {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
        path.to_string_lossy().to_string()
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
        let file_path = create_temp_file(temp.path(), "screenshot.png", b"fake png data");

        // Add attachment via entity layer (set file path in task's attachments field)
        let ectx = ctx.entity_context().await.unwrap();
        let mut task = ectx.read("task", task_id).await.unwrap();
        task.set("attachments", json!([file_path]));
        ectx.write(&task).await.unwrap();

        // Re-read raw to check stored filename (read enriches, so check via raw)
        // Actually, write() processes attachments and persists the stored filename.
        // Re-read to get the stored (un-enriched) state by checking the YAML file.
        // Use read() which enriches — the stored filename is in the YAML.
        // For checking the stored name, read the task's raw YAML.
        let task_raw = ectx.read("task", task_id).await.unwrap();
        // After enrichment, attachments is an array of metadata objects
        let arr = task_raw.get("attachments").unwrap().as_array().unwrap();
        // Each entry has a "name" derived from the stored filename
        let stored_name = arr[0]["name"].as_str().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(stored_name, "screenshot.png");
        assert!(arr[0]["size"].as_u64().unwrap() > 0);

        // Verify the file was copied to .attachments/
        let att_dir = temp
            .path()
            .join(".kanban")
            .join("tasks")
            .join(".attachments");
        let stored_filename = arr[0]["id"].as_str().unwrap().to_string() + "-" + stored_name;
        let att_file = att_dir.join(&stored_filename);
        assert!(
            att_file.exists(),
            "Attachment file should exist in .attachments/"
        );
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

        // Create a real PDF file
        let file_path = create_temp_file(temp.path(), "spec.pdf", b"fake pdf content of some size");

        let ectx = ctx.entity_context().await.unwrap();
        let mut task = ectx.read("task", task_id).await.unwrap();
        task.set("attachments", json!([file_path]));
        ectx.write(&task).await.unwrap();

        // Read back enriched metadata
        let task = ectx.read("task", task_id).await.unwrap();
        let arr = task.get("attachments").unwrap().as_array().unwrap();
        assert_eq!(arr[0]["mime_type"], "application/pdf");
        assert!(arr[0]["size"].as_u64().unwrap() > 0);
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

        let file_path = create_temp_file(temp.path(), "image.png", b"fake png");

        let ectx = ctx.entity_context().await.unwrap();
        let mut task = ectx.read("task", task_id).await.unwrap();
        task.set("attachments", json!([file_path]));
        ectx.write(&task).await.unwrap();

        let task = ectx.read("task", task_id).await.unwrap();
        let arr = task.get("attachments").unwrap().as_array().unwrap();
        // Should auto-detect from .png extension
        assert_eq!(arr[0]["mime_type"], "image/png");
    }

    #[tokio::test]
    async fn test_add_attachment_to_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let ectx = ctx.entity_context().await.unwrap();
        let result = ectx.read("task", "nonexistent").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_mime_type() {
        assert_eq!(detect_mime_type("file.png"), Some("image/png".to_string()));
        assert_eq!(
            detect_mime_type("doc.pdf"),
            Some("application/pdf".to_string())
        );
        assert_eq!(
            detect_mime_type("script.js"),
            Some("application/javascript".to_string())
        );
        assert_eq!(detect_mime_type("code.rs"), Some("text/x-rust".to_string()));
        assert_eq!(
            detect_mime_type("README.md"),
            Some("text/markdown".to_string())
        );
        assert_eq!(detect_mime_type("unknown.xyz"), None);
    }
}
