//! Paste handler: attachment → attachment.
//!
//! Pasting an attachment when the right-clicked target is *itself* an
//! `attachment:<path>` moniker dispatches into the parent task (resolved
//! from the scope chain) and reuses [`AttachmentOntoTaskHandler`].
//!
//! This is the dispatch shim that lets right-clicking an existing
//! attachment chip and choosing **Paste** add a clipboard attachment to
//! the same task that owns the chip — without a registered
//! `(attachment, attachment)` pair the cross-cutting `entity.paste`
//! emission for the innermost attachment scope would resolve to "no
//! handler" and the menu entry would never surface.
//!
//! The semantics intentionally match `attachment_onto_task`: a copy mode
//! adds a fresh association on the parent task while leaving the source
//! intact, and a cut mode does the same — the destructive remove already
//! ran in `CutEntityCmd::execute` before paste fires.
//!
//! Dispatch key: `("attachment", "attachment")`.

use super::attachment_onto_task::AttachmentOntoTaskHandler;
use super::PasteHandler;
use crate::clipboard::ClipboardPayload;
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{CommandContext, CommandError, Result};

/// Dispatch shim that forwards `(attachment, attachment)` paste
/// invocations to the parent task via [`AttachmentOntoTaskHandler`].
///
/// Unit-shaped — all routing decisions come from the [`CommandContext`]'s
/// scope chain (parent task lookup) and the existing handler's logic.
pub struct AttachmentOntoAttachmentHandler;

#[async_trait]
impl PasteHandler for AttachmentOntoAttachmentHandler {
    /// Dispatch key: attachment on the clipboard, attachment row under
    /// the cursor.
    fn matches(&self) -> (&'static str, &'static str) {
        ("attachment", "attachment")
    }

    /// Paste an attachment from the clipboard onto the parent task of
    /// the right-clicked attachment row.
    ///
    /// `target` is the inner attachment moniker (`attachment:<path>`).
    /// The owning task is resolved via `ctx.resolve_entity_id("task")` —
    /// every attachment row's `FocusScope` chain carries the parent task
    /// moniker, so this is the load-bearing identity. Once the task is
    /// known, the paste forwards verbatim to
    /// [`AttachmentOntoTaskHandler::execute`].
    ///
    /// # Errors
    ///
    /// - [`CommandError::MissingScope`] if no `task:` moniker is in scope
    ///   — the dispatch path cannot identify the destination task.
    /// - Any error surfaced by [`AttachmentOntoTaskHandler::execute`].
    async fn execute(
        &self,
        clipboard: &ClipboardPayload,
        _target: &str,
        ctx: &CommandContext,
    ) -> Result<Value> {
        let task_id = ctx
            .resolve_entity_id("task")
            .ok_or_else(|| CommandError::MissingScope("task".into()))?;
        let task_target = format!("task:{task_id}");
        AttachmentOntoTaskHandler
            .execute(clipboard, &task_target, ctx)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::paste_handlers::test_support::{attachment_clipboard, matrix_with, setup};
    use crate::task::AddTask;
    use crate::Execute;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::{CommandContext, UIState};

    /// Build a `CommandContext` whose scope chain looks like the real
    /// frontend's attachment row focus chain, with the supplied `target`
    /// moniker set so the handler can read it back.
    fn make_attachment_ctx(
        task_id: &str,
        attachment_path: &str,
        kanban: &Arc<crate::context::KanbanContext>,
    ) -> CommandContext {
        let scope = vec![
            format!("attachment:{attachment_path}"),
            format!("task:{task_id}"),
            "column:todo".into(),
        ];
        let mut ctx = CommandContext::new(
            "entity.paste",
            scope,
            Some(format!("attachment:{attachment_path}")),
            HashMap::new(),
        );
        ctx.set_extension(Arc::clone(kanban));
        ctx.ui_state = Some(Arc::new(UIState::new()));
        ctx
    }

    /// Hygiene: the dispatch key is `(attachment, attachment)` and the
    /// matrix can resolve it via that key.
    #[test]
    fn handler_matches_attachment_onto_attachment() {
        let h = AttachmentOntoAttachmentHandler;
        assert_eq!(h.matches(), ("attachment", "attachment"));

        let m = matrix_with(AttachmentOntoAttachmentHandler);
        assert!(m.find("attachment", "attachment").is_some());
        assert!(m.find("attachment", "task").is_none());
    }

    /// Pasting an attachment onto an existing attachment chip (with the
    /// parent task in scope) lands the new attachment on that task —
    /// confirms the shim forwards correctly to `attachment_onto_task`.
    #[tokio::test]
    async fn paste_attachment_onto_attachment_lands_on_parent_task() {
        let (temp, kanban) = setup().await;

        let task = AddTask::new("Target")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = task["id"].as_str().unwrap().to_string();

        // Source file the clipboard payload references.
        let source_path = temp.path().join("clip.png");
        std::fs::write(&source_path, b"clip bytes").unwrap();
        let source_path_str = source_path.to_string_lossy().into_owned();

        // The right-clicked attachment moniker — for this test it can be
        // any path; the handler ignores it and dispatches via the parent
        // task in scope.
        let existing_path = "/tmp/some-existing.png";
        let ctx = make_attachment_ctx(&task_id, existing_path, &kanban);
        let payload = attachment_clipboard(
            &source_path_str,
            "clip.png",
            Some("image/png"),
            Some(10),
            "copy",
        );

        let target = format!("attachment:{existing_path}");
        let result = AttachmentOntoAttachmentHandler
            .execute(&payload, &target, &ctx)
            .await
            .unwrap();
        assert_eq!(result["task_id"], task_id);

        let ectx = kanban.entity_context().await.unwrap();
        let after = ectx.read("task", &task_id).await.unwrap();
        let arr = after
            .get("attachments")
            .and_then(|v| v.as_array())
            .expect("task must have attachments after paste");
        assert_eq!(arr.len(), 1, "exactly one attachment after paste");
        assert_eq!(arr[0]["name"], "clip.png");
    }

    /// Without a `task:` moniker in scope the handler must fail loudly
    /// rather than guess — symmetric with how `entity.delete` on an
    /// orphan attachment errors.
    #[tokio::test]
    async fn paste_attachment_onto_attachment_without_task_in_scope_errors() {
        let (_temp, kanban) = setup().await;
        let mut ctx = CommandContext::new(
            "entity.paste",
            vec!["attachment:/tmp/x.png".into(), "column:todo".into()],
            Some("attachment:/tmp/x.png".into()),
            HashMap::new(),
        );
        ctx.set_extension(Arc::clone(&kanban));
        ctx.ui_state = Some(Arc::new(UIState::new()));

        let payload = attachment_clipboard("/tmp/y.png", "y.png", None, None, "copy");
        let err = AttachmentOntoAttachmentHandler
            .execute(&payload, "attachment:/tmp/x.png", &ctx)
            .await
            .expect_err("missing parent task must fail");
        match err {
            CommandError::MissingScope(scope) => assert_eq!(scope, "task"),
            other => panic!("expected MissingScope(\"task\"), got {other:?}"),
        }
    }
}
