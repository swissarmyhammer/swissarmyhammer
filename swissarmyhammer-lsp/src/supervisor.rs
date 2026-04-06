//! Multi-daemon supervisor for a workspace.
//!
//! [`LspSupervisorManager`] detects projects in a workspace directory, looks up
//! matching LSP server specs from the registry, and spawns an [`LspDaemon`] for
//! each. It exposes aggregate status, per-server force-restart, and a
//! coordinated shutdown.

use std::collections::HashMap;
use std::path::PathBuf;

use tracing::{info, warn};

use crate::daemon::LspDaemon;
use crate::error::LspError;
use crate::registry::servers_for_project;
use crate::types::{DaemonStatus, LspDaemonState, OwnedLspServerSpec};

/// Manages all LSP daemons for a single workspace.
pub struct LspSupervisorManager {
    /// Workspace root path.
    workspace_root: PathBuf,
    /// Map from command name to its daemon.
    daemons: HashMap<String, LspDaemon>,
}

impl std::fmt::Debug for LspSupervisorManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspSupervisorManager")
            .field("workspace_root", &self.workspace_root)
            .field("daemon_count", &self.daemons.len())
            .finish()
    }
}

impl LspSupervisorManager {
    /// Create a new supervisor manager (no daemons started yet).
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            daemons: HashMap::new(),
        }
    }

    /// Detect projects in the workspace and start an LSP daemon for each
    /// matching server spec.
    ///
    /// Uses `swissarmyhammer_project_detection::detect_projects()` to discover
    /// project types, then looks up servers via `servers_for_project()`. Each
    /// unique server command gets one daemon (deduped by command name).
    pub async fn start(&mut self) -> Vec<Result<(), LspError>> {
        let projects =
            match swissarmyhammer_project_detection::detect_projects(&self.workspace_root, None) {
                Ok(p) => p,
                Err(e) => {
                    warn!(%e, "Failed to detect projects in workspace");
                    return vec![Err(LspError::ProjectDetection(format!(
                        "{}: {e}",
                        self.workspace_root.display()
                    )))];
                }
            };

        info!(
            workspace = %self.workspace_root.display(),
            projects = projects.len(),
            "Detected projects"
        );

        // Collect unique server specs (dedupe by command name)
        let mut specs_by_command: HashMap<String, OwnedLspServerSpec> = HashMap::new();
        for project in &projects {
            for spec in servers_for_project(project.project_type) {
                specs_by_command.entry(spec.command.clone()).or_insert(spec);
            }
        }

        // Start a daemon for each unique server
        let mut results = Vec::new();
        for (command, spec) in specs_by_command {
            if self.daemons.contains_key(&command) {
                // Already running — skip
                continue;
            }
            let mut daemon = LspDaemon::new(spec, self.workspace_root.clone());
            let outcome = daemon.start().await;
            self.daemons.insert(command, daemon);
            results.push(outcome);
        }

        results
    }

    /// Return the status of all managed daemons.
    pub fn status(&self) -> Vec<DaemonStatus> {
        self.daemons
            .iter()
            .map(|(cmd, daemon)| DaemonStatus {
                command: cmd.clone(),
                state: daemon.state(),
            })
            .collect()
    }

    /// Force-restart a specific daemon by command name.
    ///
    /// Resets the backoff counter and attempts a fresh start. Returns `Err` if
    /// no daemon with that command name is managed.
    pub async fn force_restart(&mut self, command: &str) -> Result<(), LspError> {
        let daemon = self
            .daemons
            .get_mut(command)
            .ok_or_else(|| LspError::DaemonNotFound(command.to_string()))?;
        daemon.force_restart().await
    }

    /// Gracefully shut down all managed daemons.
    pub async fn shutdown(&mut self) {
        info!(count = self.daemons.len(), "Shutting down all LSP daemons");
        for (cmd, daemon) in self.daemons.iter_mut() {
            info!(cmd, "Shutting down LSP daemon");
            daemon.shutdown().await;
        }
    }

    /// Run health checks on all daemons, attempting restart for any that have
    /// died.
    ///
    /// This is intended to be called periodically (e.g. from a tokio task).
    pub async fn health_check_all(&mut self) {
        let commands: Vec<String> = self.daemons.keys().cloned().collect();
        for cmd in commands {
            if let Some(daemon) = self.daemons.get_mut(&cmd) {
                if !daemon.health_check() {
                    // Only attempt restart if state is Failed (not NotFound, NotStarted, etc.)
                    if matches!(daemon.state(), LspDaemonState::Failed { .. }) {
                        let _ = daemon.restart_with_backoff().await;
                    }
                }
            }
        }
    }

    /// Get a reference to a specific daemon by command name.
    pub fn get_daemon(&self, command: &str) -> Option<&LspDaemon> {
        self.daemons.get(command)
    }

    /// Get a mutable reference to a specific daemon by command name.
    pub fn get_daemon_mut(&mut self, command: &str) -> Option<&mut LspDaemon> {
        self.daemons.get_mut(command)
    }

    /// Return the command names of all managed daemons.
    pub fn daemon_names(&self) -> Vec<String> {
        self.daemons.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supervisor_new() {
        let mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));
        assert!(mgr.status().is_empty());
    }

    #[test]
    fn test_supervisor_no_daemon_for_unknown_command() {
        let mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));
        assert!(mgr.get_daemon("nonexistent").is_none());
    }

    #[test]
    fn test_daemon_names_empty() {
        let mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));
        assert!(mgr.daemon_names().is_empty());
    }

    #[tokio::test]
    async fn test_force_restart_unknown_command() {
        let mut mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));
        let result = mgr.force_restart("nonexistent").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LspError::DaemonNotFound(_)));
    }

    #[tokio::test]
    async fn test_shutdown_empty() {
        let mut mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));
        // Should not panic on empty daemon set
        mgr.shutdown().await;
    }

    // -- helper -----------------------------------------------------------

    use crate::types::OwnedLspServerSpec;

    /// Build a minimal OwnedLspServerSpec with a nonexistent binary.
    fn fake_spec(command: &str) -> OwnedLspServerSpec {
        OwnedLspServerSpec {
            project_types: vec![],
            command: command.to_string(),
            args: vec![],
            language_ids: vec!["test".to_string()],
            file_extensions: vec![],
            startup_timeout_secs: 1,
            health_check_interval_secs: 1,
            install_hint: String::new(),
            icon: None,
        }
    }

    /// Insert an un-started daemon into the supervisor for the given spec.
    fn insert_daemon(mgr: &mut LspSupervisorManager, spec: OwnedLspServerSpec) {
        let cmd = spec.command.clone();
        let daemon = LspDaemon::new(spec, mgr.workspace_root.clone());
        mgr.daemons.insert(cmd, daemon);
    }

    // -- start (spawn_all) tests ------------------------------------------

    #[tokio::test]
    async fn test_start_empty_workspace_detects_no_projects() {
        // An empty temp directory has no project markers, so start() should
        // produce no daemons and return an empty results vec.
        let tmp = tempfile::tempdir().unwrap();
        let mut mgr = LspSupervisorManager::new(tmp.path().to_path_buf());
        let results = mgr.start().await;
        // No projects detected means no server specs found, so no spawn attempts.
        assert!(
            results.is_empty(),
            "expected no spawn results for empty dir"
        );
        assert!(mgr.status().is_empty());
        assert!(mgr.daemon_names().is_empty());
    }

    #[tokio::test]
    async fn test_start_with_project_marker_attempts_spawn() {
        // Create a temp dir with a Cargo.toml so project detection finds a Rust project.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        let mut mgr = LspSupervisorManager::new(tmp.path().to_path_buf());
        let results = mgr.start().await;

        // We should get at least one result (the attempt to start rust-analyzer).
        // It may succeed or fail depending on whether rust-analyzer is installed,
        // but the supervisor should have recorded a daemon entry.
        assert!(
            !results.is_empty(),
            "expected at least one spawn attempt for Rust project"
        );
        assert!(
            !mgr.daemon_names().is_empty(),
            "expected daemon entries after start"
        );
    }

    #[tokio::test]
    async fn test_start_deduplicates_same_server() {
        // Two project markers that resolve to the same LSP server should only
        // produce one daemon entry.
        let tmp = tempfile::tempdir().unwrap();
        // Cargo.toml in root and in a nested dir — both are Rust projects
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join("Cargo.toml"),
            "[package]\nname = \"b\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        let mut mgr = LspSupervisorManager::new(tmp.path().to_path_buf());
        let _ = mgr.start().await;

        // Count unique command names — each command should appear at most once
        let names = mgr.daemon_names();
        let unique: std::collections::HashSet<&String> = names.iter().collect();
        assert_eq!(
            names.len(),
            unique.len(),
            "daemon names should be unique (no duplicates)"
        );
    }

    #[tokio::test]
    async fn test_start_nonexistent_workspace() {
        // A nonexistent directory should produce a ProjectDetection error
        let mut mgr = LspSupervisorManager::new(PathBuf::from("/nonexistent/workspace/path"));
        let results = mgr.start().await;
        assert_eq!(results.len(), 1, "expected exactly one error result");
        assert!(results[0].is_err());
        match &results[0] {
            Err(LspError::ProjectDetection(msg)) => {
                assert!(
                    msg.contains("/nonexistent/workspace/path"),
                    "error should mention the workspace path"
                );
            }
            other => panic!("expected ProjectDetection error, got: {other:?}"),
        }
    }

    // -- shutdown tests ---------------------------------------------------

    #[tokio::test]
    async fn test_shutdown_transitions_daemons_to_not_started() {
        let mut mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));
        insert_daemon(&mut mgr, fake_spec("fake-lsp-a"));
        insert_daemon(&mut mgr, fake_spec("fake-lsp-b"));

        assert_eq!(mgr.daemon_names().len(), 2);

        mgr.shutdown().await;

        // After shutdown, all daemons should be in NotStarted state
        // (they were never started, so shutdown is a no-op that leaves them NotStarted)
        for status in mgr.status() {
            assert_eq!(
                status.state,
                LspDaemonState::NotStarted,
                "daemon {} should be NotStarted after shutdown",
                status.command
            );
        }
    }

    #[tokio::test]
    async fn test_shutdown_preserves_daemon_entries() {
        let mut mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));
        insert_daemon(&mut mgr, fake_spec("fake-lsp-a"));

        mgr.shutdown().await;

        // The daemon entry should still exist (shutdown does not remove daemons)
        assert_eq!(mgr.daemon_names().len(), 1);
        assert!(mgr.get_daemon("fake-lsp-a").is_some());
    }

    // -- health_check_all tests -------------------------------------------

    #[tokio::test]
    async fn test_health_check_all_no_daemons() {
        let mut mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));
        // Should not panic with empty daemon set
        mgr.health_check_all().await;
    }

    #[tokio::test]
    async fn test_health_check_all_not_started_daemons_no_restart() {
        let mut mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));
        insert_daemon(&mut mgr, fake_spec("fake-lsp-a"));

        // NotStarted daemons should not trigger a restart attempt
        mgr.health_check_all().await;

        let status = mgr.status();
        assert_eq!(status.len(), 1);
        // State remains NotStarted since health_check returns false for no child,
        // but supervisor only restarts Failed daemons
        assert_eq!(status[0].state, LspDaemonState::NotStarted);
    }

    #[tokio::test]
    async fn test_health_check_all_failed_daemon_attempts_restart() {
        let mut mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));
        insert_daemon(&mut mgr, fake_spec("__nonexistent_binary_xyz__"));

        // Manually transition the daemon to Failed state so health_check_all
        // will attempt a restart.
        if let Some(daemon) = mgr.daemons.get_mut("__nonexistent_binary_xyz__") {
            // Simulate a failure by calling start() which will fail with BinaryNotFound
            let _ = daemon.start().await;
        }

        // Verify it's in a non-Running state (NotFound since binary doesn't exist)
        let state_before = mgr
            .get_daemon("__nonexistent_binary_xyz__")
            .unwrap()
            .state();
        assert!(
            matches!(state_before, LspDaemonState::NotFound),
            "expected NotFound, got: {state_before:?}"
        );

        // health_check_all only restarts Failed daemons, not NotFound ones
        mgr.health_check_all().await;

        // State should remain NotFound (not attempted restart)
        let state_after = mgr
            .get_daemon("__nonexistent_binary_xyz__")
            .unwrap()
            .state();
        assert!(
            matches!(state_after, LspDaemonState::NotFound),
            "NotFound daemons should not be restarted by health_check_all"
        );
    }

    #[tokio::test]
    async fn test_health_check_all_only_restarts_failed_state() {
        // Verifies that health_check_all discriminates between daemon states:
        // only Failed triggers restart_with_backoff, other non-healthy states
        // (NotStarted, NotFound, ShuttingDown) are left alone.
        let mut mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));
        insert_daemon(&mut mgr, fake_spec("not-started-lsp"));
        insert_daemon(&mut mgr, fake_spec("__also_nonexistent__"));

        // Make one daemon NotFound by attempting start with bad binary
        if let Some(daemon) = mgr.daemons.get_mut("__also_nonexistent__") {
            let _ = daemon.start().await;
        }

        mgr.health_check_all().await;

        // not-started-lsp should still be NotStarted
        assert_eq!(
            mgr.get_daemon("not-started-lsp").unwrap().state(),
            LspDaemonState::NotStarted
        );
        // __also_nonexistent__ should still be NotFound
        assert!(matches!(
            mgr.get_daemon("__also_nonexistent__").unwrap().state(),
            LspDaemonState::NotFound
        ));
    }

    // -- force_restart tests ----------------------------------------------

    #[tokio::test]
    async fn test_force_restart_known_daemon_with_bad_binary() {
        let mut mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));
        insert_daemon(&mut mgr, fake_spec("__nonexistent_force_restart__"));

        // force_restart should fail because the binary doesn't exist
        let result = mgr.force_restart("__nonexistent_force_restart__").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LspError::BinaryNotFound { .. }
        ));

        // Daemon should be in NotFound state after failed restart
        assert!(matches!(
            mgr.get_daemon("__nonexistent_force_restart__")
                .unwrap()
                .state(),
            LspDaemonState::NotFound
        ));
    }

    // -- status / get_daemon tests ----------------------------------------

    #[tokio::test]
    async fn test_status_reports_all_daemons() {
        let mut mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));
        insert_daemon(&mut mgr, fake_spec("lsp-alpha"));
        insert_daemon(&mut mgr, fake_spec("lsp-beta"));
        insert_daemon(&mut mgr, fake_spec("lsp-gamma"));

        let statuses = mgr.status();
        assert_eq!(statuses.len(), 3);

        let commands: std::collections::HashSet<String> =
            statuses.iter().map(|s| s.command.clone()).collect();
        assert!(commands.contains("lsp-alpha"));
        assert!(commands.contains("lsp-beta"));
        assert!(commands.contains("lsp-gamma"));
    }

    #[test]
    fn test_get_daemon_mut_returns_mutable_ref() {
        let mut mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));
        insert_daemon(&mut mgr, fake_spec("mutable-lsp"));

        let daemon = mgr.get_daemon_mut("mutable-lsp");
        assert!(daemon.is_some());
        // Verify we can access daemon methods through the mutable ref
        assert_eq!(daemon.unwrap().command(), "mutable-lsp");
    }

    #[test]
    fn test_debug_impl() {
        let mut mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));
        insert_daemon(&mut mgr, fake_spec("debug-lsp"));

        let debug_str = format!("{:?}", mgr);
        assert!(debug_str.contains("LspSupervisorManager"));
        assert!(debug_str.contains("daemon_count: 1"));
    }

    #[tokio::test]
    async fn test_start_skip_already_running_daemon() {
        // If a daemon is already inserted for a command, start() should skip it
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        let mut mgr = LspSupervisorManager::new(tmp.path().to_path_buf());

        // Pre-insert a daemon for rust-analyzer
        insert_daemon(&mut mgr, fake_spec("rust-analyzer"));

        // start() should detect Rust project and find rust-analyzer server,
        // but skip spawning because it's already in the daemons map
        let results = mgr.start().await;

        // Should get no results because the existing daemon was skipped
        assert!(
            results.is_empty(),
            "expected no spawn results when daemon already exists, got {} results",
            results.len()
        );
    }

    #[tokio::test]
    async fn test_health_check_all_with_failed_daemon_attempts_restart() {
        let mut mgr = LspSupervisorManager::new(PathBuf::from("/tmp/test"));

        // Use "true" which exists but exits immediately, causing handshake failure
        let spec = OwnedLspServerSpec {
            project_types: vec![],
            command: "true".to_string(),
            args: vec![],
            language_ids: vec!["test".to_string()],
            file_extensions: vec![],
            startup_timeout_secs: 1,
            health_check_interval_secs: 1,
            install_hint: String::new(),
            icon: None,
        };
        let cmd = spec.command.clone();
        let mut daemon = LspDaemon::new(spec, mgr.workspace_root.clone());

        // Start the daemon - it will fail because "true" exits immediately
        // and produces no LSP handshake, putting it in Failed state
        let _ = daemon.start().await;
        assert!(
            matches!(daemon.state(), LspDaemonState::Failed { .. }),
            "expected Failed state, got: {:?}",
            daemon.state()
        );
        mgr.daemons.insert(cmd, daemon);

        // health_check_all should attempt restart_with_backoff on Failed daemons
        mgr.health_check_all().await;

        // After restart attempt, state should be Failed again (another handshake failure)
        let state = mgr.get_daemon("true").unwrap().state();
        assert!(
            matches!(state, LspDaemonState::Failed { .. }),
            "expected Failed after restart attempt, got: {state:?}"
        );
    }
}
