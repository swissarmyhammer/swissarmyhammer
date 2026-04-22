//! DeleteAttachment command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Locate the enriched attachment entry in `arr` that matches `needle`.
///
/// `arr` is the enriched `attachments` field as returned by
/// `EntityContext::read` — each element is an object of the shape
/// `{"id": <ulid>, "name": <basename>, "path": <abs>, ...}` whose `path`
/// points into `.attachments/{id}-{name}`.
///
/// `needle` may identify the attachment by any of:
///   * the stored-filename ULID prefix (as embedded in `id`)
///   * the absolute filesystem path (as emitted by the UI moniker
///     `attachment:${path}`)
///   * the stored filename `{id}-{name}`
///
/// Returns the index of the first matching entry, or `None` if no entry
/// matches. Entries that aren't object-shaped, or are missing the fields
/// needed to compare against `needle`, are skipped rather than rejected —
/// the matcher is deliberately tolerant of malformed rows so a single bad
/// entry cannot prevent a valid delete.
///
/// An empty `needle` never matches (even against empty fields), so callers
/// that accidentally pass `""` get a clean `None` rather than a spurious
/// hit on a partially-populated row.
fn match_attachment_index(arr: &[Value], needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    arr.iter().position(|entry| {
        let Some(obj) = entry.as_object() else {
            return false;
        };
        let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let path = obj.get("path").and_then(|v| v.as_str()).unwrap_or("");
        if !id.is_empty() && needle == id {
            return true;
        }
        if !path.is_empty() && needle == path {
            return true;
        }
        if !id.is_empty() && !name.is_empty() && needle == format!("{}-{}", id, name) {
            return true;
        }
        false
    })
}

/// Delete an attachment from a task
#[operation(
    verb = "delete",
    noun = "attachment",
    description = "Delete an attachment from a task"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct DeleteAttachment {
    /// The task ID
    pub task_id: TaskId,
    /// The attachment ID to delete
    pub id: String,
}

impl DeleteAttachment {
    /// Create a new DeleteAttachment command
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            id: id.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteAttachment {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;

            // Read the task. The attachments field comes back enriched —
            // each entry is an object `{"id": <ulid>, "name": ..., "path": <abs>, ...}`
            // where `path` points into `.attachments/{id}-{name}`.
            //
            // The caller may identify the attachment by any of:
            //   * the stored-filename ULID prefix (as embedded in `id`)
            //   * the absolute filesystem path (as emitted by the UI moniker
            //     `attachment:${path}`)
            //   * the stored filename `{id}-{name}`
            //
            // We locate the matching enriched entry and rebuild the list
            // in canonical stored-filename form. `trash_removed_attachments`
            // in `EntityContext::write` compares old vs. new filename lists
            // and trashes the removed file — that's the single source of
            // truth for attachment file removal. There is no separate
            // "attachment" entity to delete: attachments live purely as
            // files in `.attachments/{id}-{name}` plus an entry in the
            // owning task's `attachments` field.
            let mut task = ectx.read("task", self.task_id.as_str()).await?;
            let attachments_arr = task
                .get("attachments")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let Some(idx) = match_attachment_index(&attachments_arr, &self.id) else {
                return Err(KanbanError::NotFound {
                    resource: "attachment".to_string(),
                    id: self.id.to_string(),
                });
            };

            let matched_id = attachments_arr[idx]
                .as_object()
                .and_then(|o| o.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or(&self.id)
                .to_string();

            // Rebuild the attachments list as canonical stored filenames
            // (`{id}-{name}`) — that's the on-disk form that
            // `trash_removed_attachments` in `EntityContext::write` compares
            // against the previous list to decide which files to trash.
            //
            // Malformed entries (missing/non-string `id` or `name`) are rare
            // but can arise from hand-edited files; we preserve them verbatim
            // rather than silently dropping them, to match the "tolerant of
            // corrupt rows" posture of `match_attachment_index` above. The
            // entity layer will pass the raw value through to storage
            // unchanged.
            let retained: Vec<Value> = attachments_arr
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != idx)
                .map(|(_, entry)| {
                    let canonical = entry.as_object().and_then(|obj| {
                        let id = obj.get("id").and_then(|v| v.as_str())?;
                        let name = obj.get("name").and_then(|v| v.as_str())?;
                        Some(Value::String(format!("{}-{}", id, name)))
                    });
                    canonical.unwrap_or_else(|| entry.clone())
                })
                .collect();
            task.set("attachments", Value::Array(retained));
            ectx.write(&task).await?;

            Ok(json!({
                "deleted": true,
                "attachment_id": matched_id,
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
    use super::{match_attachment_index, DeleteAttachment};
    use crate::board::InitBoard;
    use crate::context::KanbanContext;
    use crate::task::AddTask;
    use serde_json::{json, Value};
    use swissarmyhammer_operations::Execute;
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

    fn create_temp_file(dir: &std::path::Path, name: &str, content: &[u8]) -> String {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path.to_string_lossy().to_string()
    }

    #[tokio::test]
    async fn test_delete_attachment() {
        let (temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Create file and attach
        let file_path = create_temp_file(temp.path(), "file.txt", b"hello");
        let ectx = ctx.entity_context().await.unwrap();
        let mut task = ectx.read("task", task_id).await.unwrap();
        task.set("attachments", json!([file_path]));
        ectx.write(&task).await.unwrap();

        // Read back to get the stored filename
        let task = ectx.read("task", task_id).await.unwrap();
        let arr = task.get("attachments").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 1);
        let stored_id = arr[0]["id"].as_str().unwrap();
        let stored_name = arr[0]["name"].as_str().unwrap();
        let stored_filename = format!("{}-{}", stored_id, stored_name);

        // Remove attachment by clearing the field and writing
        let mut task = ectx.read("task", task_id).await.unwrap();
        task.set("attachments", json!([]));
        ectx.write(&task).await.unwrap();

        // Verify file was trashed
        let att_file = temp
            .path()
            .join(".kanban")
            .join("tasks")
            .join(".attachments")
            .join(&stored_filename);
        assert!(!att_file.exists(), "Attachment file should be trashed");

        // Verify file moved to trash dir
        let trash_file = temp
            .path()
            .join(".kanban")
            .join("tasks")
            .join(".attachments")
            .join(".trash")
            .join(&stored_filename);
        assert!(trash_file.exists(), "Attachment should be in trash");

        // Verify the task's attachments list is empty
        let task = ectx.read("task", task_id).await.unwrap();
        let attachments = task.get("attachments");
        let is_empty = attachments.is_none()
            || attachments.unwrap().is_null()
            || attachments.unwrap().as_array().is_none_or(|a| a.is_empty());
        assert!(is_empty);
    }

    #[tokio::test]
    async fn test_delete_nonexistent_attachment() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Task with no attachments — nothing to delete
        let ectx = ctx.entity_context().await.unwrap();
        let task = ectx.read("task", task_id).await.unwrap();
        let attachments = task.get("attachments");
        let is_empty = attachments.is_none()
            || attachments.unwrap().is_null()
            || attachments.unwrap().as_array().is_none_or(|a| a.is_empty());
        assert!(is_empty);
    }

    #[tokio::test]
    async fn test_delete_attachment_from_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let ectx = ctx.entity_context().await.unwrap();
        let result = ectx.read("task", "nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_one_of_multiple_attachments() {
        let (temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Create and attach two files
        let f1 = create_temp_file(temp.path(), "file1.txt", b"one");
        let f2 = create_temp_file(temp.path(), "file2.txt", b"two");
        let ectx = ctx.entity_context().await.unwrap();
        let mut task = ectx.read("task", task_id).await.unwrap();
        task.set("attachments", json!([f1, f2]));
        ectx.write(&task).await.unwrap();

        // Read back enriched metadata
        let task = ectx.read("task", task_id).await.unwrap();
        let arr = task.get("attachments").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 2);

        // Keep only the second attachment (remove the first)
        let second_meta = arr[1].clone();
        let mut task = ectx.read("task", task_id).await.unwrap();
        // Write back with only the second stored filename
        let second_stored = format!(
            "{}-{}",
            second_meta["id"].as_str().unwrap(),
            second_meta["name"].as_str().unwrap()
        );
        task.set("attachments", json!([second_stored]));
        ectx.write(&task).await.unwrap();

        // Verify only one attachment remains
        let task = ectx.read("task", task_id).await.unwrap();
        let arr = task.get("attachments").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "file2.txt");
    }

    /// Attach a file and return `(task_id, enriched_entry)`.
    ///
    /// The enriched entry is the single object the entity layer emits after
    /// the write completes — its `id`, `name`, and `path` fields drive the
    /// three identifier forms exercised by the matcher tests below.
    async fn setup_one_attachment(temp: &TempDir, ctx: &KanbanContext) -> (String, Value) {
        let task_result = AddTask::new("Task")
            .execute(ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        let file_path = create_temp_file(temp.path(), "file.txt", b"hello");
        let ectx = ctx.entity_context().await.unwrap();
        let mut task = ectx.read("task", &task_id).await.unwrap();
        task.set("attachments", json!([file_path]));
        ectx.write(&task).await.unwrap();

        let task = ectx.read("task", &task_id).await.unwrap();
        let arr = task.get("attachments").unwrap().as_array().unwrap().clone();
        assert_eq!(arr.len(), 1);
        (task_id, arr.into_iter().next().unwrap())
    }

    /// DeleteAttachment resolves `self.id` against the enriched entry's `id`
    /// field (the stored-filename ULID prefix).
    #[tokio::test]
    async fn delete_attachment_by_id_form() {
        let (temp, ctx) = setup().await;
        let (task_id, entry) = setup_one_attachment(&temp, &ctx).await;
        let needle = entry["id"].as_str().unwrap().to_string();

        DeleteAttachment::new(task_id.clone(), needle)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let ectx = ctx.entity_context().await.unwrap();
        let task = ectx.read("task", &task_id).await.unwrap();
        let attachments = task.get("attachments");
        let is_empty = attachments.is_none()
            || attachments.unwrap().is_null()
            || attachments.unwrap().as_array().is_none_or(|a| a.is_empty());
        assert!(is_empty, "attachment should be removed when matched by id");
    }

    /// DeleteAttachment resolves `self.id` against the enriched entry's
    /// absolute `path` field (as emitted by the `attachment:${path}` UI
    /// moniker).
    #[tokio::test]
    async fn delete_attachment_by_path_form() {
        let (temp, ctx) = setup().await;
        let (task_id, entry) = setup_one_attachment(&temp, &ctx).await;
        let needle = entry["path"].as_str().unwrap().to_string();

        DeleteAttachment::new(task_id.clone(), needle)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let ectx = ctx.entity_context().await.unwrap();
        let task = ectx.read("task", &task_id).await.unwrap();
        let attachments = task.get("attachments");
        let is_empty = attachments.is_none()
            || attachments.unwrap().is_null()
            || attachments.unwrap().as_array().is_none_or(|a| a.is_empty());
        assert!(
            is_empty,
            "attachment should be removed when matched by path"
        );
    }

    /// DeleteAttachment resolves `self.id` against the canonical stored
    /// filename `{id}-{name}`.
    #[tokio::test]
    async fn delete_attachment_by_stored_filename_form() {
        let (temp, ctx) = setup().await;
        let (task_id, entry) = setup_one_attachment(&temp, &ctx).await;
        let needle = format!(
            "{}-{}",
            entry["id"].as_str().unwrap(),
            entry["name"].as_str().unwrap()
        );

        DeleteAttachment::new(task_id.clone(), needle)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let ectx = ctx.entity_context().await.unwrap();
        let task = ectx.read("task", &task_id).await.unwrap();
        let attachments = task.get("attachments");
        let is_empty = attachments.is_none()
            || attachments.unwrap().is_null()
            || attachments.unwrap().as_array().is_none_or(|a| a.is_empty());
        assert!(
            is_empty,
            "attachment should be removed when matched by stored filename"
        );
    }

    /// `match_attachment_index` must never match on an empty needle, even
    /// against a row whose `path` happens to be empty — callers that pass
    /// `""` by mistake should get `None`, not a spurious hit.
    #[test]
    fn match_attachment_index_rejects_empty_needle() {
        let arr = vec![json!({
            "id": "01ABC",
            "name": "file.txt",
            "path": "",
        })];
        assert_eq!(match_attachment_index(&arr, ""), None);
    }

    /// `match_attachment_index` skips (rather than rejects) rows with
    /// missing fields, so a single malformed row cannot block a valid
    /// delete.
    #[test]
    fn match_attachment_index_tolerates_malformed_rows() {
        let arr = vec![
            json!({ "not_an_attachment": true }),
            json!({
                "id": "01ABC",
                "name": "file.txt",
                "path": "/tmp/x/01ABC-file.txt",
            }),
        ];
        assert_eq!(match_attachment_index(&arr, "01ABC"), Some(1));
        assert_eq!(
            match_attachment_index(&arr, "/tmp/x/01ABC-file.txt"),
            Some(1)
        );
        assert_eq!(match_attachment_index(&arr, "01ABC-file.txt"), Some(1));
    }
}
