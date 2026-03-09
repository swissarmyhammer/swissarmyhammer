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
                specs_by_command
                    .entry(spec.command.clone())
                    .or_insert(spec);
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
        info!(
            count = self.daemons.len(),
            "Shutting down all LSP daemons"
        );
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
}
