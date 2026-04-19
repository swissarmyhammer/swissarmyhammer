//! Paste handler for `(tag, task)` — adds the clipboard tag to the target task.
//!
//! Tags are *associations* between a tag-entity and a task, not
//! self-contained values that move from place to place. Pasting a tag onto
//! a task therefore creates the association without consuming the tag
//! entity from its source — `is_cut` is intentionally ignored. Re-pasting
//! the same tag is a no-op because the underlying [`crate::task::TagTask`]
//! operation appends `#tag` to the body via `tag_parser::append_tag`,
//! which is idempotent.

use super::PasteHandler;
use crate::clipboard::ClipboardPayload;
use crate::context::KanbanContext;
use crate::task::TagTask;
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{parse_moniker, CommandContext, CommandError, Result};

/// Handler for pasting a tag entity onto a task entity.
///
/// Dispatches when the clipboard contains `entity_type == "tag"` and the
/// matched scope-chain moniker is a `task:<id>`. Reads the tag's id from
/// `clipboard.swissarmyhammer_clipboard.entity_id` and the task id from
/// the parsed `target` moniker, then runs [`TagTask`] which auto-creates
/// the tag entity if needed and appends `#tag` to the task body
/// idempotently.
pub struct TagOntoTaskHandler;

#[async_trait]
impl PasteHandler for TagOntoTaskHandler {
    fn matches(&self) -> (&'static str, &'static str) {
        ("tag", "task")
    }

    async fn execute(
        &self,
        clipboard: &ClipboardPayload,
        target: &str,
        ctx: &CommandContext,
    ) -> Result<Value> {
        // Target moniker must be `task:<id>` — the dispatcher only invokes
        // us when the (clipboard_type, target_type) pair matched, so this
        // parse should always succeed; treat a malformed moniker as a
        // dispatcher bug rather than a silent fall-through.
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

        let tag_id = &clipboard.swissarmyhammer_clipboard.entity_id;
        if tag_id.is_empty() {
            return Err(CommandError::SourceEntityMissing(
                "Clipboard tag has no entity id".into(),
            ));
        }

        // Ignore is_cut/mode: tags are associations, not movable entities.
        // Cutting a tag and pasting it onto a task should add the
        // association without deleting the source tag.

        let kanban = ctx.require_extension::<KanbanContext>()?;

        // Validate the target task still exists before we run TagTask.
        // The task here is the entity being mutated (we append `#tag` to
        // its body); if it was deleted between copy and paste, surface
        // SourceEntityMissing so the toast names the specific failure
        // ("Task '<id>' no longer exists") instead of a generic message.
        // Tag→task is a "modify the subject" operation, not a "drop into
        // a container" one — the task is a referenced source entity, not
        // a paste destination.
        let ectx = kanban
            .entity_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
        if ectx.read("task", task_id).await.is_err() {
            return Err(CommandError::SourceEntityMissing(format!(
                "Task '{task_id}' no longer exists"
            )));
        }

        let op = TagTask::new(task_id, tag_id.as_str());
        crate::commands::run_op(&op, &kanban).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::clipboard::{ClipboardData, ClipboardPayload};
    use crate::commands::paste_handlers::PasteMatrix;
    use crate::tag::AddTag;
    use crate::task::AddTask;
    use crate::task_helpers::task_tags;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_operations::Execute;
    use tempfile::TempDir;

    /// Spin up a fresh kanban context with a board initialized.
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

    /// Build a `ClipboardPayload` representing a copied tag.
    ///
    /// `tag_id` lands in `entity_id`; the `fields` snapshot mirrors what
    /// the real copy path produces, but the handler currently only needs
    /// the id — the fields are included for fidelity.
    fn tag_clipboard(tag_id: &str, tag_name: &str, mode: &str) -> ClipboardPayload {
        ClipboardPayload {
            swissarmyhammer_clipboard: ClipboardData {
                entity_type: "tag".into(),
                entity_id: tag_id.into(),
                mode: mode.into(),
                fields: json!({"tag_name": tag_name}),
            },
        }
    }

    /// Build a `CommandContext` carrying the kanban extension.
    ///
    /// The dispatcher would normally populate `target` from the matched
    /// scope moniker; tests here pass it through `execute` directly so
    /// they don't need to spin up the full chain walker.
    fn make_ctx(kanban: &Arc<KanbanContext>) -> CommandContext {
        let mut ctx = CommandContext::new("entity.paste", vec![], None, HashMap::new());
        ctx.set_extension(Arc::clone(kanban));
        ctx
    }

    /// Sanity check: the local test matrix accepts the handler and finds
    /// it under the `(tag, task)` key. This mirrors what the production
    /// `register_paste_handlers()` will do once the orchestrator wires
    /// every handler in. Tests can be run in isolation against this
    /// local matrix without the global registry being populated.
    #[test]
    fn handler_registers_on_local_matrix_under_tag_task_key() {
        let mut matrix = PasteMatrix::default();
        matrix.register(TagOntoTaskHandler);
        assert!(
            matrix.find("tag", "task").is_some(),
            "TagOntoTaskHandler should be findable under ('tag', 'task') after registration"
        );
        assert_eq!(TagOntoTaskHandler.matches(), ("tag", "task"));
    }

    #[tokio::test]
    async fn paste_tag_onto_task_adds_tag() {
        let (_temp, kanban) = setup().await;

        // Create the source tag entity.
        let tag = AddTag::new("urgent")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let tag_id = tag["id"].as_str().unwrap().to_string();

        // Create a task with no tags.
        let task = AddTask::new("Plain task")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = task["id"].as_str().unwrap().to_string();

        // Sanity: task starts untagged.
        let ectx = kanban.entity_context().await.unwrap();
        let entity_before = ectx.read("task", &task_id).await.unwrap();
        assert!(
            task_tags(&entity_before).is_empty(),
            "precondition: task should start with no tags"
        );

        // Paste the tag onto the task.
        let payload = tag_clipboard(&tag_id, "urgent", "copy");
        let target = format!("task:{task_id}");
        let ctx = make_ctx(&kanban);
        TagOntoTaskHandler
            .execute(&payload, &target, &ctx)
            .await
            .unwrap();

        // Postcondition: task body now references #urgent.
        let entity_after = ectx.read("task", &task_id).await.unwrap();
        let tags = task_tags(&entity_after);
        assert!(
            tags.iter().any(|t| t == "urgent"),
            "expected 'urgent' tag on task, got {tags:?}"
        );
    }

    #[tokio::test]
    async fn paste_tag_onto_task_ignores_cut_flag() {
        let (_temp, kanban) = setup().await;

        // Source tag and target task.
        let tag = AddTag::new("priority")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let tag_id = tag["id"].as_str().unwrap().to_string();

        let task = AddTask::new("Recipient")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = task["id"].as_str().unwrap().to_string();

        // Paste with mode == "cut" — handler must not delete the source tag.
        let payload = tag_clipboard(&tag_id, "priority", "cut");
        let target = format!("task:{task_id}");
        let ctx = make_ctx(&kanban);
        TagOntoTaskHandler
            .execute(&payload, &target, &ctx)
            .await
            .unwrap();

        // Tag entity must still exist after a "cut" paste.
        let ectx = kanban.entity_context().await.unwrap();
        let still_there = ectx.read("tag", &tag_id).await;
        assert!(
            still_there.is_ok(),
            "cut tag entity must survive paste — tags are associations, not movable values"
        );

        // Association did get applied.
        let task_entity = ectx.read("task", &task_id).await.unwrap();
        assert!(
            task_tags(&task_entity).iter().any(|t| t == "priority"),
            "tag should still be applied to task even when mode is 'cut'"
        );
    }

    #[tokio::test]
    async fn paste_same_tag_twice_is_idempotent() {
        let (_temp, kanban) = setup().await;

        let tag = AddTag::new("dup")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let tag_id = tag["id"].as_str().unwrap().to_string();

        let task = AddTask::new("Idempotent target")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = task["id"].as_str().unwrap().to_string();

        let payload = tag_clipboard(&tag_id, "dup", "copy");
        let target = format!("task:{task_id}");
        let ctx = make_ctx(&kanban);

        // First paste — adds the association.
        TagOntoTaskHandler
            .execute(&payload, &target, &ctx)
            .await
            .unwrap();
        // Second paste — must not duplicate the tag.
        TagOntoTaskHandler
            .execute(&payload, &target, &ctx)
            .await
            .unwrap();

        // Task should carry the tag exactly once.
        let ectx = kanban.entity_context().await.unwrap();
        let task_entity = ectx.read("task", &task_id).await.unwrap();
        let tags = task_tags(&task_entity);
        let count = tags.iter().filter(|t| t.as_str() == "dup").count();
        assert_eq!(
            count, 1,
            "expected exactly one 'dup' tag, got {count} (full tags: {tags:?})"
        );
    }

    #[tokio::test]
    async fn paste_rejects_non_task_target_moniker() {
        let (_temp, kanban) = setup().await;
        let payload = tag_clipboard("01FAKE", "x", "copy");
        let ctx = make_ctx(&kanban);
        let err = TagOntoTaskHandler
            .execute(&payload, "column:doing", &ctx)
            .await
            .unwrap_err();
        match err {
            CommandError::DestinationInvalid(msg) => {
                assert!(
                    msg.contains("expected a task"),
                    "unexpected error message: {msg}"
                );
            }
            other => panic!("expected DestinationInvalid, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn paste_rejects_unparseable_target_moniker() {
        let (_temp, kanban) = setup().await;
        let payload = tag_clipboard("01FAKE", "x", "copy");
        let ctx = make_ctx(&kanban);
        let err = TagOntoTaskHandler
            .execute(&payload, "not-a-moniker", &ctx)
            .await
            .unwrap_err();
        match err {
            CommandError::DestinationInvalid(msg) => {
                assert!(
                    msg.contains("not a task moniker"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected DestinationInvalid, got: {other:?}"),
        }
    }

    /// Acceptance criterion: when the target task referenced by the
    /// paste moniker no longer exists on the board (e.g. it was deleted
    /// between copy and paste), the handler must surface a structured
    /// `SourceEntityMissing` with a user-readable message naming the
    /// missing task — not a generic "execution failed".
    ///
    /// The task is the referenced subject the handler mutates; treating
    /// a missing subject as `SourceEntityMissing` keeps `DestinationInvalid`
    /// reserved for true container-style destinations (column, board,
    /// project) where the new entity would be placed *into* something.
    #[tokio::test]
    async fn paste_tag_onto_deleted_task_returns_source_entity_missing_error() {
        let (_temp, kanban) = setup().await;

        // Real tag entity on the clipboard so the handler's source-side
        // checks pass; the failure must come from the missing target.
        let tag = AddTag::new("urgent")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let tag_id = tag["id"].as_str().unwrap().to_string();

        let payload = tag_clipboard(&tag_id, "urgent", "copy");
        let ctx = make_ctx(&kanban);

        // Target moniker references a task ULID that was never written
        // to the board — equivalent to it having been deleted between
        // copy and paste from the user's perspective.
        let missing_task_id = "01ZZZZZZZZZZZZZZZZZZZZZZZZ";
        let target = format!("task:{missing_task_id}");

        let err = TagOntoTaskHandler
            .execute(&payload, &target, &ctx)
            .await
            .expect_err("missing target task must produce an error");
        match err {
            CommandError::SourceEntityMissing(msg) => {
                assert!(
                    msg.contains(missing_task_id),
                    "error must name the missing task id; got: {msg}"
                );
                assert!(
                    msg.contains("no longer exists"),
                    "error must explain the failure mode; got: {msg}"
                );
            }
            other => panic!("expected SourceEntityMissing, got: {other:?}"),
        }
    }
}
