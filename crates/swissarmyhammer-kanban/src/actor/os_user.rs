//! OS-level user identity resolution for actor-less callers.

use crate::actor::AddActor;
use crate::context::KanbanContext;
use crate::error::Result;
use crate::types::ActorId;
use swissarmyhammer_operations::Execute;

/// Idempotently ensure an actor entity for the current OS user and return
/// its id.
///
/// The id is the slugified OS username; the display name is the OS real
/// name, falling back to the username when unavailable. Registration goes
/// through [`crate::actor::AddActor`] with `ensure: true`, so repeated
/// calls return the same actor without erroring.
///
/// # Errors
///
/// Returns an error only when the underlying entity write fails (e.g. an
/// uninitialized board) — identity resolution itself never errors.
pub(crate) async fn ensure_os_user_actor(ctx: &KanbanContext) -> Result<ActorId> {
    let username = whoami::username();
    let id = swissarmyhammer_common::slug(&username);

    let realname = whoami::realname();
    let name = if realname.trim().is_empty() {
        username
    } else {
        realname
    };

    AddActor::new(id.as_str(), name)
        .with_ensure()
        .execute(ctx)
        .await
        .into_result()?;

    Ok(ActorId::from_string(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
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

    /// The OS user actor is created on first call (slugified username id,
    /// non-empty display name) and the same id comes back on repeat calls.
    #[tokio::test]
    async fn test_ensure_os_user_actor_is_idempotent() {
        let (_temp, ctx) = setup().await;

        let expected_id = swissarmyhammer_common::slug(&whoami::username());

        let first = ensure_os_user_actor(&ctx).await.unwrap();
        assert_eq!(first.as_str(), expected_id);

        // The actor entity exists with a non-empty display name.
        let ectx = ctx.entity_context().await.unwrap();
        let actor = ectx.read("actor", &expected_id).await.unwrap();
        assert!(!actor.get_str("name").unwrap_or("").is_empty());

        // Repeat call returns the same id without erroring.
        let second = ensure_os_user_actor(&ctx).await.unwrap();
        assert_eq!(second, first);
    }
}
