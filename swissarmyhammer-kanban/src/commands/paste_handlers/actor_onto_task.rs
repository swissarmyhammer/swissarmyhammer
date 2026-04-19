//! Paste handler: actor → task.
//!
//! Pasting an actor onto a task assigns that actor to the task. Actors are
//! associations rather than first-class movable entities, so this handler
//! deliberately ignores the clipboard's `cut` flag — the source actor is
//! never deleted as part of the paste. Repeated pastes of the same actor
//! onto the same task are no-ops because the underlying [`AssignTask`]
//! operation is itself idempotent.
//!
//! Dispatch key: `("actor", "task")`. Registered into the production
//! [`super::PasteMatrix`] by the orchestrator alongside its sibling
//! handlers.

use super::PasteHandler;
use crate::clipboard::ClipboardPayload;
use crate::commands::run_op;
use crate::context::KanbanContext;
use crate::task::AssignTask;
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{parse_moniker, CommandContext, CommandError, Result};

/// Paste handler that assigns the clipboard's actor to the target task.
///
/// The handler is unit-shaped — it carries no state. All inputs come from
/// the [`ClipboardPayload`], the `target` moniker, and the
/// [`KanbanContext`] extension stored on [`CommandContext`].
pub struct ActorOntoTaskHandler;

#[async_trait]
impl PasteHandler for ActorOntoTaskHandler {
    /// Dispatch key: actor on the clipboard, task under the cursor.
    fn matches(&self) -> (&'static str, &'static str) {
        ("actor", "task")
    }

    /// Append the clipboard's actor to the target task's assignees.
    ///
    /// `target` is expected to be a `task:<id>` moniker. The actor id is
    /// read from `clipboard.swissarmyhammer_clipboard.entity_id`. The
    /// `cut` flag is intentionally ignored — actors are associations,
    /// not entities that "move" between containers, so a cut paste must
    /// not delete the source actor.
    ///
    /// Idempotent: re-pasting the same actor on the same task is a no-op
    /// because [`AssignTask`] only appends an assignee that is not already
    /// present.
    ///
    /// # Errors
    ///
    /// - [`CommandError::ExecutionFailed`] if the `target` moniker is not
    ///   a `task:<id>` pair.
    /// - [`CommandError::ExecutionFailed`] if the `KanbanContext` extension
    ///   is missing from the [`CommandContext`].
    /// - Any error surfaced by the underlying [`AssignTask`] operation
    ///   (e.g., unknown task or actor).
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

        let actor_id = clipboard.swissarmyhammer_clipboard.entity_id.as_str();
        if actor_id.is_empty() {
            return Err(CommandError::SourceEntityMissing(
                "Clipboard actor has no entity id".into(),
            ));
        }

        let kanban = ctx.require_extension::<KanbanContext>()?;

        // Validate the referenced subjects exist before mutating the
        // task. The task is the entity being mutated (its assignees
        // list) and the actor is the value being applied — if either
        // was deleted between copy and paste, surface a structured
        // SourceEntityMissing so the toast names the specific failure
        // instead of a generic "execution failed".
        let ectx = kanban
            .entity_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
        if ectx.read("task", task_id).await.is_err() {
            return Err(CommandError::SourceEntityMissing(format!(
                "Task '{task_id}' no longer exists"
            )));
        }
        if ectx.read("actor", actor_id).await.is_err() {
            return Err(CommandError::SourceEntityMissing(format!(
                "Actor '{actor_id}' no longer exists"
            )));
        }

        let op = AssignTask::new(task_id, actor_id);
        run_op(&op, &kanban).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::AddActor;
    use crate::board::InitBoard;
    use crate::clipboard::ClipboardData;
    use crate::commands::paste_handlers::PasteMatrix;
    use crate::task::AddTask;
    use crate::Execute;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::UIState;

    /// Bring up a temp KanbanContext with an initialised board, plus a
    /// CommandContext whose extension map carries the kanban handle.
    async fn setup() -> (tempfile::TempDir, Arc<KanbanContext>) {
        let temp = tempfile::TempDir::new().unwrap();
        let kanban = Arc::new(KanbanContext::new(temp.path().join(".kanban")));
        InitBoard::new("Test")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        (temp, kanban)
    }

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

    /// Build a `ClipboardPayload` describing an actor on the clipboard.
    /// `mode` is one of `"copy"` / `"cut"` — the handler ignores it, but
    /// our tests cover both branches to lock that behaviour in.
    fn payload_for_actor(actor_id: &str, mode: &str) -> ClipboardPayload {
        ClipboardPayload {
            swissarmyhammer_clipboard: ClipboardData {
                entity_type: "actor".to_string(),
                entity_id: actor_id.to_string(),
                mode: mode.to_string(),
                fields: serde_json::json!({}),
            },
        }
    }

    /// Build a local matrix wired up with just our handler. Mirrors how
    /// the orchestrator will register the handler in production, but
    /// without depending on the global `register_paste_handlers()` call
    /// (which is filled in by sibling cards).
    fn local_matrix() -> PasteMatrix {
        let mut m = PasteMatrix::default();
        m.register(ActorOntoTaskHandler);
        m
    }

    /// Hygiene: the handler's dispatch key is `(actor, task)` and the
    /// matrix can resolve it.
    #[test]
    fn handler_matches_actor_onto_task() {
        let h = ActorOntoTaskHandler;
        assert_eq!(h.matches(), ("actor", "task"));

        let m = local_matrix();
        assert!(m.find("actor", "task").is_some());
        assert!(m.find("task", "actor").is_none());
    }

    /// Pasting an actor onto a task with no current assignees adds the
    /// actor to the assignee list.
    #[tokio::test]
    async fn paste_actor_onto_task_adds_assignee() {
        let (_temp, kanban) = setup().await;

        AddActor::new("alice", "Alice")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task = AddTask::new("Target")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = task["id"].as_str().unwrap();

        // Sanity: the freshly-created task starts with no assignees.
        let ectx = kanban.entity_context().await.unwrap();
        let before = ectx.read("task", task_id).await.unwrap();
        assert!(before.get_string_list("assignees").is_empty());

        let target = format!("task:{task_id}");
        let ctx = make_ctx(&target, &kanban);
        let payload = payload_for_actor("alice", "copy");

        let matrix = local_matrix();
        let handler = matrix.find("actor", "task").expect("handler registered");
        let result = handler.execute(&payload, &target, &ctx).await.unwrap();

        assert_eq!(result["assigned"], true);
        assert_eq!(result["assignee"], "alice");
        assert_eq!(result["task_id"], task_id);

        let after = ectx.read("task", task_id).await.unwrap();
        assert_eq!(
            after.get_string_list("assignees"),
            vec!["alice".to_string()]
        );
    }

    /// The handler must ignore the `cut` flag: actors are associations,
    /// not entities, so a cut-paste must add the assignee *and* leave the
    /// source actor entity intact.
    #[tokio::test]
    async fn paste_actor_onto_task_ignores_cut_flag() {
        let (_temp, kanban) = setup().await;

        AddActor::new("alice", "Alice")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task = AddTask::new("Target")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = task["id"].as_str().unwrap();

        let target = format!("task:{task_id}");
        let ctx = make_ctx(&target, &kanban);
        let payload = payload_for_actor("alice", "cut");

        let matrix = local_matrix();
        let handler = matrix.find("actor", "task").expect("handler registered");
        let result = handler.execute(&payload, &target, &ctx).await.unwrap();

        // Assignment still happened.
        assert_eq!(result["assigned"], true);

        let ectx = kanban.entity_context().await.unwrap();
        let after = ectx.read("task", task_id).await.unwrap();
        assert_eq!(
            after.get_string_list("assignees"),
            vec!["alice".to_string()]
        );

        // The source actor entity must still exist — cut did not delete it.
        let actor = ectx.read("actor", "alice").await;
        assert!(
            actor.is_ok(),
            "cut-paste of an actor must not delete the source actor entity"
        );
    }

    /// Re-pasting the same actor on the same task is a no-op: the
    /// assignee list still contains exactly one occurrence of the actor.
    #[tokio::test]
    async fn paste_same_actor_twice_is_idempotent() {
        let (_temp, kanban) = setup().await;

        AddActor::new("alice", "Alice")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task = AddTask::new("Target")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = task["id"].as_str().unwrap();

        let target = format!("task:{task_id}");
        let ctx = make_ctx(&target, &kanban);
        let payload = payload_for_actor("alice", "copy");

        let matrix = local_matrix();
        let handler = matrix.find("actor", "task").expect("handler registered");

        handler.execute(&payload, &target, &ctx).await.unwrap();
        let second = handler.execute(&payload, &target, &ctx).await.unwrap();

        // The second paste still reports the assignment as logically true,
        // but the assignee list contains a single occurrence of the actor.
        assert_eq!(second["assignee"], "alice");

        let ectx = kanban.entity_context().await.unwrap();
        let after = ectx.read("task", task_id).await.unwrap();
        let assignees = after.get_string_list("assignees");
        assert_eq!(
            assignees,
            vec!["alice".to_string()],
            "duplicate paste must not add the actor twice"
        );
    }

    /// A non-task target moniker should be rejected with a clear error
    /// rather than silently misbehaving.
    #[tokio::test]
    async fn paste_actor_onto_non_task_target_errors() {
        let (_temp, kanban) = setup().await;
        AddActor::new("alice", "Alice")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();

        let target = "column:doing";
        let ctx = make_ctx(target, &kanban);
        let payload = payload_for_actor("alice", "copy");

        let matrix = local_matrix();
        let handler = matrix.find("actor", "task").expect("handler registered");
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
}
