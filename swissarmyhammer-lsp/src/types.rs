//! Core types for LSP server management

use std::fmt;

use serde::{Deserialize, Serialize};
use std::time::Duration;
use swissarmyhammer_project_detection::ProjectType;

/// Specification for an LSP server that can be auto-detected and managed.
pub struct LspServerSpec {
    /// Which project types this server handles
    pub project_types: &'static [ProjectType],
    /// Binary name to invoke (looked up via `which`)
    pub command: &'static str,
    /// Command-line arguments
    pub args: &'static [&'static str],
    /// LSP language identifiers this server handles
    pub language_ids: &'static [&'static str],
    /// File extensions this server handles (without dot)
    pub file_extensions: &'static [&'static str],
    /// Optional initialization options factory
    pub initialization_options: Option<fn() -> serde_json::Value>,
    /// How long to wait for server startup
    pub startup_timeout: Duration,
    /// Interval between health checks
    pub health_check_interval: Duration,
    /// Human-readable install instructions shown on failure
    pub install_hint: &'static str,
}

/// Owned version of LspServerSpec for runtime-loaded configurations from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnedLspServerSpec {
    /// Which project types this server handles
    pub project_types: Vec<ProjectType>,
    /// Binary name to invoke (looked up via `which`)
    pub command: String,
    /// Command-line arguments
    pub args: Vec<String>,
    /// LSP language identifiers this server handles
    pub language_ids: Vec<String>,
    /// File extensions this server handles (without dot)
    pub file_extensions: Vec<String>,
    /// How long to wait for server startup (in seconds, stored for YAML serialization)
    #[serde(default = "default_startup_timeout")]
    pub startup_timeout_secs: u64,
    /// Interval between health checks (in seconds, stored for YAML serialization)
    #[serde(default = "default_health_check_interval")]
    pub health_check_interval_secs: u64,
    /// Human-readable install instructions shown on failure
    pub install_hint: String,
}

fn default_startup_timeout() -> u64 {
    30
}

fn default_health_check_interval() -> u64 {
    60
}

impl OwnedLspServerSpec {
    /// Get startup timeout as Duration
    pub fn startup_timeout(&self) -> Duration {
        Duration::from_secs(self.startup_timeout_secs)
    }

    /// Get health check interval as Duration
    pub fn health_check_interval(&self) -> Duration {
        Duration::from_secs(self.health_check_interval_secs)
    }
}

impl fmt::Debug for LspServerSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LspServerSpec")
            .field("project_types", &self.project_types)
            .field("command", &self.command)
            .field("args", &self.args)
            .field("language_ids", &self.language_ids)
            .field("file_extensions", &self.file_extensions)
            .field(
                "initialization_options",
                if self.initialization_options.is_some() {
                    &"Some(...)"
                } else {
                    &"None"
                },
            )
            .field("startup_timeout", &self.startup_timeout)
            .field("health_check_interval", &self.health_check_interval)
            .field("install_hint", &self.install_hint)
            .finish()
    }
}

impl fmt::Display for OwnedLspServerSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (command: {})", self.command, self.command)
    }
}

/// Runtime state of a managed LSP daemon
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LspDaemonState {
    /// Not yet started
    NotStarted,
    /// Starting up, waiting for initialize response
    Starting,
    /// Running and healthy, with the child process PID and start timestamp (millis since epoch)
    Running {
        pid: u32,
        since_epoch_ms: u64,
    },
    /// Server process died or health check failed
    Failed {
        reason: String,
        attempts: u32,
    },
    /// Binary not found on PATH
    NotFound,
    /// Shutting down gracefully
    ShuttingDown,
}

/// Status snapshot for a single daemon, suitable for external queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    /// The command name of the LSP server
    pub command: String,
    /// Current state
    pub state: LspDaemonState,
}
