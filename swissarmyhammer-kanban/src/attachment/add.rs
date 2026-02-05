//! AddAttachment command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{Attachment, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

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
            let mut task = ctx.read_task(&self.task_id).await?;

            // Auto-detect MIME type if not provided
            let mime_type = self
                .mime_type
                .clone()
                .or_else(|| detect_mime_type(&self.path));

            // Auto-detect file size if not provided
            let size = self.size.or_else(|| get_file_size(&self.path));

            // Create the attachment
            let mut attachment = Attachment::new(&self.name, &self.path);
            if let Some(mt) = mime_type {
                attachment = attachment.with_mime_type(mt);
            }
            if let Some(s) = size {
                attachment = attachment.with_size(s);
            }

            task.attachments.push(attachment.clone());
            ctx.write_task(&task).await?;

            Ok(serde_json::json!({
                "attachment": attachment,
                "task_id": task.id
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
        let (_temp, ctx) = setup().await;

        // Create a task
        let task_result = AddTask::new("Task with attachments")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Add an attachment
        let result = AddAttachment::new(task_id, "screenshot.png", "./docs/screenshot.png")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["attachment"]["name"], "screenshot.png");
        assert_eq!(result["attachment"]["path"], "./docs/screenshot.png");
        assert_eq!(result["task_id"], task_id);

        // Verify the task has the attachment
        use crate::task::GetTask;
        let task = GetTask::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(task["attachments"].as_array().unwrap().len(), 1);
        assert_eq!(task["attachments"][0]["name"], "screenshot.png");
    }

    #[tokio::test]
    async fn test_add_attachment_with_mime_type() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let result = AddAttachment::new(task_id, "doc.pdf", "./docs/spec.pdf")
            .with_mime_type("application/pdf")
            .with_size(12345)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["attachment"]["mime_type"], "application/pdf");
        assert_eq!(result["attachment"]["size"], 12345);
    }

    #[tokio::test]
    async fn test_add_attachment_auto_detect_mime() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let result = AddAttachment::new(task_id, "image", "./image.png")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Should auto-detect from .png extension
        assert_eq!(result["attachment"]["mime_type"], "image/png");
    }

    #[tokio::test]
    async fn test_add_attachment_to_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let result = AddAttachment::new("nonexistent", "file.txt", "./file.txt")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::TaskNotFound { .. })));
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
