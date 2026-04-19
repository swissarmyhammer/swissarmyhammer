//! Paste handler: attachment → task.
//!
//! Pasting an attachment onto a task adds the attachment file to the
//! task's attachment list. The attachment moniker uses the file path as
//! its `entity_id` (see `builtin/entities/attachment.yaml`); the display
//! name, MIME type, and size are carried in the clipboard `fields`
//! snapshot populated by the copy path.
//!
//! This handler deliberately ignores the clipboard's `cut`/`copy` mode:
//! attachments belong to a task by association, not as standalone movable
//! entities, and "cutting" an attachment in the UI today still leaves the
//! source association in place — pasting onto a different task creates a
//! new association on the destination while the original remains. If a
//! true "move" semantic is required later, this handler can be extended
//! to delete the source association after a successful paste.
//!
//! Duplicate attachments on the same task are intentionally allowed:
//! [`AddAttachment`] mints a fresh attachment entity id on every call,
//! so the resulting `attachment` entity is always distinct even when the
//! underlying file path matches an existing one.
//!
//! Dispatch key: `("attachment", "task")`.

use super::PasteHandler;
use crate::attachment::AddAttachment;
use crate::clipboard::ClipboardPayload;
use crate::commands::run_op;
use crate::context::KanbanContext;
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{parse_moniker, CommandContext, CommandError, Result};

/// Paste handler that attaches the clipboard's file to the target task.
///
/// Unit-shaped — all inputs come from the [`ClipboardPayload`], the
/// `target` moniker, and the [`KanbanContext`] extension on
/// [`CommandContext`].
pub struct AttachmentOntoTaskHandler;

impl AttachmentOntoTaskHandler {
    /// Derive the display name for the attachment.
    ///
    /// Prefers the `name` field carried by the clipboard snapshot (the
    /// real copy path populates `attachment_name` and the legacy `name`
    /// alias). Falls back to the trailing path component when neither is
    /// present, then finally to the raw path string — guaranteeing the
    /// `AddAttachment::name` argument is never empty.
    fn resolve_name(fields: &Value, path: &str) -> String {
        if let Some(obj) = fields.as_object() {
            for key in ["name", "attachment_name"] {
                if let Some(name) = obj.get(key).and_then(|v| v.as_str()) {
                    if !name.is_empty() {
                        return name.to_string();
                    }
                }
            }
        }
        std::path::Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| path.to_string())
    }

    /// Read the optional MIME type from the clipboard fields snapshot.
    ///
    /// Accepts either the canonical `attachment_mime_type` field name or
    /// the shorter `mime_type` alias used by the clipboard wire format.
    /// Empty strings are treated as missing so [`AddAttachment`] falls
    /// back to its extension-based detection.
    fn resolve_mime_type(fields: &Value) -> Option<String> {
        let obj = fields.as_object()?;
        for key in ["mime_type", "attachment_mime_type"] {
            if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
        None
    }

    /// Read the optional file size from the clipboard fields snapshot.
    ///
    /// Accepts either the canonical `attachment_size` field name or the
    /// shorter `size` alias. Returns `None` when neither key is a u64,
    /// letting [`AddAttachment`] re-`stat` the file at write time.
    fn resolve_size(fields: &Value) -> Option<u64> {
        let obj = fields.as_object()?;
        for key in ["size", "attachment_size"] {
            if let Some(n) = obj.get(key).and_then(|v| v.as_u64()) {
                return Some(n);
            }
        }
        None
    }
}

#[async_trait]
impl PasteHandler for AttachmentOntoTaskHandler {
    /// Dispatch key: attachment on the clipboard, task under the cursor.
    fn matches(&self) -> (&'static str, &'static str) {
        ("attachment", "task")
    }

    /// Add the clipboard's attachment file to the target task.
    ///
    /// `target` is expected to be a `task:<id>` moniker. The attachment
    /// path is read from `clipboard.swissarmyhammer_clipboard.entity_id`
    /// — attachment monikers carry the file path as their id (see
    /// `builtin/entities/attachment.yaml`). The display name, MIME type,
    /// and size are pulled from the `fields` snapshot populated by the
    /// copy path; sensible fallbacks (filename, extension-based MIME,
    /// `stat` size) kick in when fields are missing.
    ///
    /// `cut`/`copy` mode is intentionally ignored: attachments are
    /// associations rather than movable entities, so a cut paste creates
    /// the new association without touching the source task's list.
    ///
    /// # Errors
    ///
    /// - [`CommandError::ExecutionFailed`] if the `target` moniker is not
    ///   a `task:<id>` pair.
    /// - [`CommandError::ExecutionFailed`] if the clipboard's `entity_id`
    ///   (attachment path) is empty.
    /// - [`CommandError::ExecutionFailed`] if the `KanbanContext` extension
    ///   is missing from the [`CommandContext`].
    /// - Any error surfaced by the underlying [`AddAttachment`] operation
    ///   (e.g., unknown task or unreadable file).
    async fn execute(
        &self,
        clipboard: &ClipboardPayload,
        target: &str,
        ctx: &CommandContext,
    ) -> Result<Value> {
        let (target_type, task_id) = parse_moniker(target).ok_or_else(|| {
            CommandError::DestinationInvalid(format!(
                "paste target '{target}' is not a task moniker"
            ))
        })?;
        if target_type != "task" {
            return Err(CommandError::DestinationInvalid(format!(
                "paste target '{target}' is a {target_type}, expected a task"
            )));
        }

        let path = clipboard.swissarmyhammer_clipboard.entity_id.as_str();
        if path.trim().is_empty() {
            return Err(CommandError::SourceEntityMissing(
                "Clipboard attachment has no source path".into(),
            ));
        }
        // Path safety is enforced by the entity layer: AddAttachment copies
        // the source file into the board's `.attachments/` directory via
        // EntityContext::resolve_attachment_value → io::copy_attachment.
        // The destination (where the bytes land) is always sandboxed under
        // the board's storage root. The source path is intentionally allowed
        // to be absolute or relative — that's how a user attaches files
        // from anywhere on disk (e.g. `/tmp/screenshot.png`). Rejecting
        // traversal sequences here would break legitimate uses without
        // adding security: the entity layer never *writes* to the source
        // path, only reads from it, and read access is already governed
        // by filesystem permissions.

        let fields = &clipboard.swissarmyhammer_clipboard.fields;
        let name = Self::resolve_name(fields, path);

        let kanban = ctx.require_extension::<KanbanContext>()?;

        // Validate the target task and the source file before
        // delegating to AddAttachment. Without these guards a missing
        // task surfaces as a generic ExecutionFailed wrapping the
        // entity-layer's "entity not found" string, and an unreadable
        // source path surfaces as a generic IO error — both rendered
        // identically in the toast. Splitting them lets the toast name
        // the specific failure mode.
        let ectx = kanban
            .entity_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
        if ectx.read("task", task_id).await.is_err() {
            return Err(CommandError::SourceEntityMissing(format!(
                "Task '{task_id}' no longer exists"
            )));
        }
        if !std::path::Path::new(path).exists() {
            return Err(CommandError::SourceEntityMissing(format!(
                "Attachment file '{path}' is not readable"
            )));
        }

        let mut op = AddAttachment::new(task_id, name, path);
        if let Some(mime) = Self::resolve_mime_type(fields) {
            op = op.with_mime_type(mime);
        }
        if let Some(size) = Self::resolve_size(fields) {
            op = op.with_size(size);
        }

        run_op(&op, &kanban).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::clipboard::ClipboardData;
    use crate::commands::paste_handlers::PasteMatrix;
    use crate::task::AddTask;
    use crate::Execute;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::UIState;
    use tempfile::TempDir;

    /// Bring up a temp KanbanContext with an initialised board.
    async fn setup() -> (TempDir, Arc<KanbanContext>) {
        let temp = TempDir::new().unwrap();
        let kanban = Arc::new(KanbanContext::new(temp.path().join(".kanban")));
        InitBoard::new("Test")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        (temp, kanban)
    }

    /// Build a CommandContext carrying the kanban extension and a UI state.
    fn make_ctx(target: &str, kanban: &Arc<KanbanContext>) -> CommandContext {
        let mut ctx = CommandContext::new(
            "entity.paste",
            Vec::new(),
            Some(target.to_string()),
            HashMap::new(),
        );
        ctx.set_extension(Arc::clone(kanban));
        ctx.ui_state = Some(Arc::new(UIState::new()));
        ctx
    }

    /// Build a `ClipboardPayload` describing an attachment on the
    /// clipboard. The attachment moniker uses the file path as its id;
    /// `fields` mirrors what the real copy path produces (display name,
    /// MIME type, size).
    fn payload_for_attachment(
        path: &str,
        name: &str,
        mime_type: Option<&str>,
        size: Option<u64>,
        mode: &str,
    ) -> ClipboardPayload {
        let mut fields = serde_json::Map::new();
        fields.insert("name".into(), json!(name));
        if let Some(mime) = mime_type {
            fields.insert("mime_type".into(), json!(mime));
        }
        if let Some(s) = size {
            fields.insert("size".into(), json!(s));
        }
        ClipboardPayload {
            swissarmyhammer_clipboard: ClipboardData {
                entity_type: "attachment".into(),
                entity_id: path.into(),
                mode: mode.into(),
                fields: Value::Object(fields),
            },
        }
    }

    /// Build a local matrix wired up with just our handler. Mirrors how
    /// the orchestrator will register the handler in production but
    /// keeps the tests independent of the global `register_paste_handlers()`
    /// call, which is filled in by sibling cards.
    fn local_matrix() -> PasteMatrix {
        let mut m = PasteMatrix::default();
        m.register(AttachmentOntoTaskHandler);
        m
    }

    /// Create a temp file with content and return its absolute path as a
    /// `String`. The handler is exercised against real files because
    /// [`AddAttachment`] copies the file into `.kanban/.attachments/` and
    /// `stat`s it for size detection.
    fn write_temp_file(dir: &std::path::Path, name: &str, content: &[u8]) -> String {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path.to_string_lossy().into_owned()
    }

    /// Hygiene: the handler's dispatch key is `(attachment, task)` and the
    /// matrix can resolve it via that key.
    #[test]
    fn handler_matches_attachment_onto_task() {
        let h = AttachmentOntoTaskHandler;
        assert_eq!(h.matches(), ("attachment", "task"));

        let m = local_matrix();
        assert!(m.find("attachment", "task").is_some());
        assert!(m.find("task", "attachment").is_none());
    }

    /// Pasting an attachment onto a task with no current attachments adds
    /// the file to the task's attachment list.
    #[tokio::test]
    async fn paste_attachment_onto_task_adds_attachment() {
        let (temp, kanban) = setup().await;

        let task = AddTask::new("Target")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = task["id"].as_str().unwrap().to_string();

        // Sanity: the freshly-created task starts with no attachments.
        let ectx = kanban.entity_context().await.unwrap();
        let before = ectx.read("task", &task_id).await.unwrap();
        assert!(
            before
                .get("attachments")
                .and_then(|v| v.as_array())
                .map(|a| a.is_empty())
                .unwrap_or(true),
            "precondition: task should start with no attachments"
        );

        let path = write_temp_file(temp.path(), "screenshot.png", b"fake png data");
        let target = format!("task:{task_id}");
        let ctx = make_ctx(&target, &kanban);
        let payload =
            payload_for_attachment(&path, "screenshot.png", Some("image/png"), Some(13), "copy");

        let matrix = local_matrix();
        let handler = matrix
            .find("attachment", "task")
            .expect("handler registered");
        let result = handler.execute(&payload, &target, &ctx).await.unwrap();

        // Result reports the new attachment entity and target task.
        assert_eq!(result["task_id"], task_id);
        let attachment = result["attachment"]
            .as_object()
            .expect("result.attachment must be an object");
        assert_eq!(attachment["name"], "screenshot.png");
        assert_eq!(attachment["path"], path);
        assert!(
            attachment["id"].as_str().is_some(),
            "new attachment must carry an id"
        );

        // Postcondition: the task now lists exactly one attachment, and
        // its enriched metadata matches what we pasted.
        let after = ectx.read("task", &task_id).await.unwrap();
        let arr = after
            .get("attachments")
            .and_then(|v| v.as_array())
            .expect("task must have attachments array after paste");
        assert_eq!(arr.len(), 1, "expected exactly one attachment after paste");
        assert_eq!(arr[0]["name"], "screenshot.png");
    }

    /// The source task that produced the clipboard entry must still
    /// carry the attachment after the paste — copying an attachment is
    /// non-destructive on the source.
    #[tokio::test]
    async fn paste_attachment_preserves_original() {
        let (temp, kanban) = setup().await;

        // Source task: pre-seed with the attachment so we can assert the
        // attachment list survives the paste onto a different task.
        let source = AddTask::new("Source")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let source_id = source["id"].as_str().unwrap().to_string();

        let dest = AddTask::new("Destination")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let dest_id = dest["id"].as_str().unwrap().to_string();

        let path = write_temp_file(temp.path(), "spec.pdf", b"original content");
        AddAttachment::new(source_id.as_str(), "spec.pdf", &path)
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();

        // Snapshot the source's attachment count before pasting.
        let ectx = kanban.entity_context().await.unwrap();
        let source_before = ectx.read("task", &source_id).await.unwrap();
        let count_before = source_before
            .get("attachments")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        assert_eq!(count_before, 1, "source should start with one attachment");

        // Paste onto the destination task.
        let target = format!("task:{dest_id}");
        let ctx = make_ctx(&target, &kanban);
        let payload =
            payload_for_attachment(&path, "spec.pdf", Some("application/pdf"), Some(16), "copy");

        let matrix = local_matrix();
        let handler = matrix
            .find("attachment", "task")
            .expect("handler registered");
        handler.execute(&payload, &target, &ctx).await.unwrap();

        // Source task must still list its original attachment unchanged.
        let source_after = ectx.read("task", &source_id).await.unwrap();
        let count_after = source_after
            .get("attachments")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        assert_eq!(
            count_after, 1,
            "source attachment list must be untouched by the paste"
        );

        // And the destination acquired the new attachment.
        let dest_after = ectx.read("task", &dest_id).await.unwrap();
        let dest_arr = dest_after
            .get("attachments")
            .and_then(|v| v.as_array())
            .expect("destination must have attachments array after paste");
        assert_eq!(dest_arr.len(), 1, "destination must gain one attachment");
        assert_eq!(dest_arr[0]["name"], "spec.pdf");
    }

    /// Pasting with `mode == "cut"` must behave exactly like a copy:
    /// the source association is left intact and the destination gains
    /// the file. Attachments are associations, not movable entities.
    #[tokio::test]
    async fn paste_attachment_ignores_cut_flag() {
        let (temp, kanban) = setup().await;

        let source = AddTask::new("Cut source")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let source_id = source["id"].as_str().unwrap().to_string();

        let dest = AddTask::new("Cut destination")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let dest_id = dest["id"].as_str().unwrap().to_string();

        let path = write_temp_file(temp.path(), "diagram.png", b"diagram bytes");
        AddAttachment::new(source_id.as_str(), "diagram.png", &path)
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();

        // Paste with mode == "cut" — handler must not mutate the source's
        // attachment list and must still add the file to the destination.
        let target = format!("task:{dest_id}");
        let ctx = make_ctx(&target, &kanban);
        let payload =
            payload_for_attachment(&path, "diagram.png", Some("image/png"), Some(13), "cut");

        let matrix = local_matrix();
        let handler = matrix
            .find("attachment", "task")
            .expect("handler registered");
        handler.execute(&payload, &target, &ctx).await.unwrap();

        let ectx = kanban.entity_context().await.unwrap();

        // Source still has the attachment.
        let source_after = ectx.read("task", &source_id).await.unwrap();
        let source_arr = source_after
            .get("attachments")
            .and_then(|v| v.as_array())
            .expect("source must still have attachments array after cut paste");
        assert_eq!(
            source_arr.len(),
            1,
            "cut paste must not remove the attachment from the source task"
        );
        assert_eq!(source_arr[0]["name"], "diagram.png");

        // Destination gained the attachment.
        let dest_after = ectx.read("task", &dest_id).await.unwrap();
        let dest_arr = dest_after
            .get("attachments")
            .and_then(|v| v.as_array())
            .expect("destination must have attachments array after paste");
        assert_eq!(dest_arr.len(), 1, "destination must gain the attachment");
    }

    /// A non-task target moniker should be rejected with a clear error
    /// rather than silently misbehaving.
    #[tokio::test]
    async fn paste_attachment_onto_non_task_target_errors() {
        let (temp, kanban) = setup().await;
        let path = write_temp_file(temp.path(), "x.txt", b"x");

        let target = "column:doing";
        let ctx = make_ctx(target, &kanban);
        let payload = payload_for_attachment(&path, "x.txt", None, None, "copy");

        let matrix = local_matrix();
        let handler = matrix
            .find("attachment", "task")
            .expect("handler registered");
        let err = handler
            .execute(&payload, target, &ctx)
            .await
            .expect_err("non-task target must fail");
        match err {
            CommandError::DestinationInvalid(msg) => {
                assert!(msg.contains("expected a task"), "got: {msg}");
            }
            other => panic!("expected DestinationInvalid, got {other:?}"),
        }
    }

    /// An empty entity_id (path) must be rejected up-front so the
    /// underlying [`AddAttachment`] doesn't see a meaningless empty path.
    #[tokio::test]
    async fn paste_attachment_with_empty_path_errors() {
        let (_temp, kanban) = setup().await;
        let task = AddTask::new("Target")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = task["id"].as_str().unwrap().to_string();

        let target = format!("task:{task_id}");
        let ctx = make_ctx(&target, &kanban);
        let payload = payload_for_attachment("", "x", None, None, "copy");

        let matrix = local_matrix();
        let handler = matrix
            .find("attachment", "task")
            .expect("handler registered");
        let err = handler
            .execute(&payload, &target, &ctx)
            .await
            .expect_err("empty path must fail");
        match err {
            CommandError::SourceEntityMissing(msg) => {
                assert!(
                    msg.contains("no source path"),
                    "empty source path must surface SourceEntityMissing; got: {msg}"
                );
            }
            other => panic!("expected SourceEntityMissing, got {other:?}"),
        }
    }

    /// When the attachment file referenced by the clipboard payload no
    /// longer exists on disk, the handler must surface a structured
    /// `SourceEntityMissing` naming the missing path so the toast names
    /// the specific failure rather than a generic IO error.
    #[tokio::test]
    async fn paste_attachment_with_missing_source_file_errors() {
        let (_temp, kanban) = setup().await;
        let task = AddTask::new("Target")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = task["id"].as_str().unwrap().to_string();

        let target = format!("task:{task_id}");
        let ctx = make_ctx(&target, &kanban);

        // Path that was never written — equivalent to a file that the
        // user's clipboard snapshot referenced but has since been
        // removed from disk.
        let missing_path = "/tmp/nonexistent/attachment-from-paste-test.png";
        let payload = payload_for_attachment(missing_path, "missing.png", None, None, "copy");

        let matrix = local_matrix();
        let handler = matrix
            .find("attachment", "task")
            .expect("handler registered");
        let err = handler
            .execute(&payload, &target, &ctx)
            .await
            .expect_err("missing source file must produce an error");
        match err {
            CommandError::SourceEntityMissing(msg) => {
                assert!(
                    msg.contains(missing_path),
                    "error must name the missing path; got: {msg}"
                );
                assert!(
                    msg.contains("not readable"),
                    "error must explain the failure mode; got: {msg}"
                );
            }
            other => panic!("expected SourceEntityMissing, got {other:?}"),
        }
    }

    /// When the clipboard `fields` snapshot omits the `name`, the
    /// handler must derive a non-empty display name from the path's
    /// trailing component.
    #[test]
    fn resolve_name_falls_back_to_path_basename() {
        let fields = json!({});
        let name = AttachmentOntoTaskHandler::resolve_name(&fields, "/tmp/dir/example.png");
        assert_eq!(name, "example.png");
    }

    /// Empty-string field values must not satisfy the lookup — the
    /// handler should fall through to the basename fallback.
    #[test]
    fn resolve_name_skips_empty_field() {
        let fields = json!({"name": ""});
        let name = AttachmentOntoTaskHandler::resolve_name(&fields, "/tmp/dir/example.png");
        assert_eq!(name, "example.png");
    }

    /// Both the canonical and short MIME-type field names should be
    /// honoured, with the short alias taking precedence (it's what the
    /// clipboard wire format emits today).
    #[test]
    fn resolve_mime_type_reads_either_field_name() {
        let short = json!({"mime_type": "image/png"});
        assert_eq!(
            AttachmentOntoTaskHandler::resolve_mime_type(&short),
            Some("image/png".to_string())
        );
        let long = json!({"attachment_mime_type": "application/pdf"});
        assert_eq!(
            AttachmentOntoTaskHandler::resolve_mime_type(&long),
            Some("application/pdf".to_string())
        );
        let none = json!({});
        assert_eq!(AttachmentOntoTaskHandler::resolve_mime_type(&none), None);
    }

    /// Same alias coverage for the size field.
    #[test]
    fn resolve_size_reads_either_field_name() {
        let short = json!({"size": 42_u64});
        assert_eq!(AttachmentOntoTaskHandler::resolve_size(&short), Some(42));
        let long = json!({"attachment_size": 99_u64});
        assert_eq!(AttachmentOntoTaskHandler::resolve_size(&long), Some(99));
        let none = json!({});
        assert_eq!(AttachmentOntoTaskHandler::resolve_size(&none), None);
    }
}
