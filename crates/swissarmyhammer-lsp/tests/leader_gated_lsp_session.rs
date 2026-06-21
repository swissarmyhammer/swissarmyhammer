//! Leader-per-workdir: the workspace-root election that gates the LSP session.
//!
//! LSP servers speak stdio only — one client, no listener — so exactly one
//! process per workspace may own the server. The MCP server enforces this by
//! gating the supervisor spawn on a single workspace-root flock election (the
//! same election that owns the index DB, so LSP and index leadership coincide):
//! only the elected leader builds an [`LspSupervisorManager`].
//!
//! This test proves the *election semantics* that gate depends on — exactly one
//! leader per workspace root, a follower that cannot win while the leader holds
//! the flock, and a follower that wins (and may then cold-spawn the supervisor)
//! once the leader exits. The leader-only branch of the spawn itself is unit
//! tested at the gate in `swissarmyhammer-tools`
//! (`server::tests::test_lsp_supervisor_spawn_is_leader_gated`); here we lock in
//! the election behavior that branch keys on, and show the leader-side
//! supervisor build that follows a won election.
//!
//! The workspace is empty (no project markers), so the leader's supervisor
//! starts with zero daemons — fast and deterministic regardless of whether any
//! real language server is installed.

use std::path::Path;

use swissarmyhammer_leader_election::{ElectionConfig, ElectionOutcome, LeaderElection};
use swissarmyhammer_lsp::LspSupervisorManager;
use tempfile::TempDir;

/// Isolate election lock/socket files in `base` so concurrent test runs don't
/// collide on the shared system temp dir.
fn isolated_config(base: &Path) -> ElectionConfig {
    ElectionConfig::new()
        .with_prefix("lsp-leader-test")
        .with_base_dir(base)
}

/// Build and start the supervisor for a workspace — the leader-only action that
/// follows a won election.
async fn spawn_supervisor(workspace_root: &Path) -> LspSupervisorManager {
    let mut supervisor = LspSupervisorManager::new(workspace_root.to_path_buf());
    let _ = supervisor.start().await;
    supervisor
}

#[tokio::test]
async fn one_leader_per_root_follower_can_promote_only_after_leader_exits() {
    let lock_dir = TempDir::new().unwrap();
    let ws = TempDir::new().unwrap();
    let cfg = isolated_config(lock_dir.path());

    // First process wins the single workspace-root election → it is the leader.
    let election_a: LeaderElection = LeaderElection::with_config(ws.path(), cfg.clone());
    let leader_guard = match election_a.elect().unwrap() {
        ElectionOutcome::Leader(g) => g,
        ElectionOutcome::Follower(_) => panic!("first elector on a fresh root must win"),
    };
    // The leader — and only the leader — builds the LSP supervisor.
    let leader_supervisor = spawn_supervisor(ws.path()).await;
    // An empty workspace yields no daemons; the point is the leader owns the
    // supervisor that owns whatever daemons exist.
    assert!(leader_supervisor.daemon_names().is_empty());

    // Second process on the SAME root loses the election → it is a follower.
    let election_b: LeaderElection = LeaderElection::with_config(ws.path(), cfg.clone());
    let follower_guard = match election_b.elect().unwrap() {
        ElectionOutcome::Follower(f) => f,
        ElectionOutcome::Leader(_) => panic!("second elector on a held root must follow"),
    };

    // While the leader holds the flock the follower cannot win, so it never
    // reaches the leader-only supervisor spawn.
    assert!(
        follower_guard.try_promote().unwrap().is_none(),
        "follower must not promote while the leader holds the flock"
    );

    // Leader exits: release the flock (and drop the owned supervisor).
    drop(leader_supervisor);
    drop(leader_guard);

    // The follower re-contests the single election and now wins. Only after this
    // win is it entitled to cold-spawn its own supervisor.
    let promoted_guard = follower_guard
        .try_promote()
        .unwrap()
        .expect("follower must win the election once the leader has exited");
    let promoted_supervisor = spawn_supervisor(ws.path()).await;
    assert!(promoted_supervisor.daemon_names().is_empty());

    drop(promoted_guard);
}
