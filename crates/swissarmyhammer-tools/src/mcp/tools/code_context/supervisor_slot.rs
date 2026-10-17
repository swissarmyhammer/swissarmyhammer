//! The one replaceable slot that holds the LSP supervisor of this process.
//!
//! Only the workspace leader runs LSP servers. A leader that steps down must
//! stop its servers, and a process that becomes leader again must start a new
//! supervisor. A `OnceLock` cannot do this: its second `set` fails, so the
//! new supervisor drops and `kill_on_drop` kills the new servers. This slot
//! can be replaced and emptied.
//!
//! The slot also holds the abort handles of the tasks that use the sessions
//! of its supervisor (the diagnostics watcher, the request server, the
//! diagnostics feeds). When the supervisor leaves the slot, those tasks stop
//! with it.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use swissarmyhammer_lsp::LspSupervisorManager;
use tokio::task::AbortHandle;

/// Shared handle to an LSP supervisor.
///
/// The supervisor is behind an async mutex. A start holds the lock until it
/// ends, so a shutdown that locks it waits for a start in progress.
pub(crate) type SharedLspSupervisor = Arc<tokio::sync::Mutex<LspSupervisorManager>>;

/// The content of the slot: the supervisor of the current leader tenure and
/// the tasks that use its sessions.
struct SlotState {
    /// The supervisor of the current leader tenure. `None` when this process
    /// is not the leader.
    supervisor: Option<SharedLspSupervisor>,
    /// The tasks that must stop when `supervisor` leaves the slot.
    tasks: Vec<AbortHandle>,
}

/// A supervisor that left the slot, with the tasks that must stop with it.
struct Retired {
    /// The supervisor to shut down.
    supervisor: SharedLspSupervisor,
    /// The tasks to abort.
    tasks: Vec<AbortHandle>,
}

/// The one slot of this process.
static LSP_SUPERVISOR_SLOT: Mutex<SlotState> = Mutex::new(SlotState {
    supervisor: None,
    tasks: Vec::new(),
});

/// Lock the slot. A poisoned lock still holds valid state (each change is one
/// assignment), so the guard is recovered.
fn slot() -> MutexGuard<'static, SlotState> {
    LSP_SUPERVISOR_SLOT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Take the supervisor and its tasks out of the slot, if there is one.
fn take_retired(state: &mut SlotState) -> Option<Retired> {
    let supervisor = state.supervisor.take()?;
    Some(Retired {
        supervisor,
        tasks: std::mem::take(&mut state.tasks),
    })
}

/// Abort the tasks of a retired supervisor, then shut the supervisor down.
///
/// The lock waits for a start in progress, so no server that the start
/// spawns stays alive after this returns.
async fn stop(retired: Retired) {
    for task in &retired.tasks {
        task.abort();
    }
    retired.supervisor.lock().await.shutdown().await;
}

/// Return the supervisor in the slot, or `None` when this process runs no
/// LSP supervisor (a follower, or a leader that stepped down).
pub(crate) fn lsp_supervisor() -> Option<SharedLspSupervisor> {
    slot().supervisor.clone()
}

/// Put `supervisor` in the slot. A supervisor that was in the slot is shut
/// down, and its tasks are aborted.
pub(crate) async fn install_lsp_supervisor(supervisor: SharedLspSupervisor) {
    let replaced = {
        let mut state = slot();
        let replaced = take_retired(&mut state);
        state.supervisor = Some(supervisor);
        replaced
    };
    if let Some(replaced) = replaced {
        tracing::info!("code-context: replacing the LSP supervisor; shutting down the old one");
        stop(replaced).await;
    }
}

/// Tie `task` to `owner`. The task is aborted when `owner` leaves the slot.
///
/// When `owner` is not in the slot now (a step-down came first), the task is
/// aborted at once. Thus a task that starts after a step-down cannot stay
/// alive.
pub(crate) fn track_lsp_task(owner: &SharedLspSupervisor, task: AbortHandle) {
    let mut state = slot();
    let owner_in_slot = state
        .supervisor
        .as_ref()
        .is_some_and(|current| Arc::ptr_eq(current, owner));
    if owner_in_slot {
        state.tasks.push(task);
    } else {
        task.abort();
    }
}

/// Take the supervisor out of the slot and shut it down.
///
/// The tasks of the supervisor are aborted. The shutdown waits for a start in
/// progress, so no LSP server of this process stays alive after this returns.
/// With an empty slot this does nothing.
///
/// Call this when the leader steps down and when `sah serve` exits.
pub async fn shutdown_lsp_supervisor() {
    let retired = take_retired(&mut slot());
    if let Some(retired) = retired {
        tracing::info!("code-context: shutting down the LSP supervisor");
        stop(retired).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// The time a test waits to see that a shutdown is still blocked.
    const BLOCKED_PROBE: Duration = Duration::from_millis(100);

    /// A new supervisor over an empty temp directory, behind the shared
    /// handle.
    fn new_supervisor(root: &std::path::Path) -> SharedLspSupervisor {
        Arc::new(tokio::sync::Mutex::new(LspSupervisorManager::new(
            root.to_path_buf(),
        )))
    }

    /// Whether the slot holds exactly `supervisor`.
    fn slot_holds(supervisor: &SharedLspSupervisor) -> bool {
        lsp_supervisor().is_some_and(|current| Arc::ptr_eq(&current, supervisor))
    }

    /// The time a test waits for an aborted task to end.
    const ABORT_WAIT: Duration = Duration::from_secs(5);

    /// A task that runs until something aborts it.
    fn pending_task() -> tokio::task::JoinHandle<()> {
        tokio::spawn(std::future::pending::<()>())
    }

    /// Whether `task` ends as cancelled within [`ABORT_WAIT`].
    async fn was_aborted(task: tokio::task::JoinHandle<()>) -> bool {
        matches!(
            tokio::time::timeout(ABORT_WAIT, task).await,
            Ok(Err(e)) if e.is_cancelled()
        )
    }

    #[tokio::test]
    async fn install_puts_the_supervisor_in_the_slot() {
        let dir = tempfile::tempdir().unwrap();
        let supervisor = new_supervisor(dir.path());

        install_lsp_supervisor(Arc::clone(&supervisor)).await;

        assert!(slot_holds(&supervisor));
        shutdown_lsp_supervisor().await;
    }

    /// A promotion after a step-down puts a new supervisor in the slot. The
    /// old one is shut down, and the new one stays live.
    #[tokio::test]
    async fn install_shuts_down_the_replaced_supervisor() {
        let dir = tempfile::tempdir().unwrap();
        let old = new_supervisor(dir.path());
        let new = new_supervisor(dir.path());

        install_lsp_supervisor(Arc::clone(&old)).await;
        install_lsp_supervisor(Arc::clone(&new)).await;

        assert!(old.lock().await.is_shut_down());
        assert!(slot_holds(&new));
        assert!(!new.lock().await.is_shut_down());
        shutdown_lsp_supervisor().await;
    }

    #[tokio::test]
    async fn shutdown_empties_the_slot_and_shuts_down_the_supervisor() {
        let dir = tempfile::tempdir().unwrap();
        let supervisor = new_supervisor(dir.path());
        install_lsp_supervisor(Arc::clone(&supervisor)).await;

        shutdown_lsp_supervisor().await;

        assert!(lsp_supervisor().is_none());
        assert!(supervisor.lock().await.is_shut_down());
    }

    #[tokio::test]
    async fn shutdown_aborts_the_tracked_tasks() {
        let dir = tempfile::tempdir().unwrap();
        let supervisor = new_supervisor(dir.path());
        install_lsp_supervisor(Arc::clone(&supervisor)).await;
        let task = pending_task();
        track_lsp_task(&supervisor, task.abort_handle());

        shutdown_lsp_supervisor().await;

        assert!(was_aborted(task).await, "the task must be aborted");
    }

    /// A task that a start tracks after a step-down took its supervisor out
    /// of the slot is aborted at once.
    #[tokio::test]
    async fn track_aborts_a_task_whose_supervisor_left_the_slot() {
        let dir = tempfile::tempdir().unwrap();
        let supervisor = new_supervisor(dir.path());
        install_lsp_supervisor(Arc::clone(&supervisor)).await;
        shutdown_lsp_supervisor().await;
        let task = pending_task();

        track_lsp_task(&supervisor, task.abort_handle());

        assert!(was_aborted(task).await, "the task must be aborted");
    }

    /// A shutdown waits for a start in progress. The start holds the lock of
    /// the supervisor, so the shutdown runs only after the start ends.
    #[tokio::test]
    async fn shutdown_waits_for_a_start_in_progress() {
        let dir = tempfile::tempdir().unwrap();
        let supervisor = new_supervisor(dir.path());
        install_lsp_supervisor(Arc::clone(&supervisor)).await;
        let start_in_progress = Arc::clone(&supervisor).lock_owned().await;

        let shutdown = tokio::spawn(shutdown_lsp_supervisor());
        tokio::time::sleep(BLOCKED_PROBE).await;
        assert!(
            !shutdown.is_finished(),
            "the shutdown must wait for the start"
        );
        drop(start_in_progress);
        shutdown.await.unwrap();

        assert!(supervisor.lock().await.is_shut_down());
    }
}
