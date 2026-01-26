//! AVP Context - Manages the AVP directory, logging, and agent access.
//!
//! The AVP directory (configured via `AvpConfig::DIR_NAME`) is created at the
//! git repository root and contains:
//! - `avp.log` - Append-only log of hook events
//! - `validators/` - Project-specific validators
//! - `.gitignore` - Excludes log files from version control
//!
//! User-level validators can be placed in `~/<AVP_DIR>/validators/`.
//!
//! The context also provides access to an ACP Agent for validator execution.
//! In production, this is a ClaudeAgent created lazily. In tests, a PlaybackAgent
//! can be injected via `with_agent()`.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use agent_client_protocol::{Agent, SessionNotification};
use chrono::Utc;
use claude_agent::CreateAgentConfig;
use swissarmyhammer_directory::{AvpConfig, DirectoryConfig, ManagedDirectory};
use tokio::sync::{broadcast, Mutex};

use crate::error::AvpError;

/// Log file name within the AVP directory.
const LOG_FILE_NAME: &str = "avp.log";

/// Decision outcome for a hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Hook allowed the action to proceed.
    Allow,
    /// Hook blocked the action.
    Block,
    /// Hook encountered an error.
    Error,
}

impl std::fmt::Display for Decision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Decision::Allow => write!(f, "allow"),
            Decision::Block => write!(f, "block"),
            Decision::Error => write!(f, "error"),
        }
    }
}

/// A hook event to log.
#[derive(Debug)]
pub struct HookEvent<'a> {
    /// The hook type (e.g., "PreToolUse", "PostToolUse").
    pub hook_type: &'a str,
    /// The decision outcome.
    pub decision: Decision,
    /// Optional details (tool name, reason, etc.).
    pub details: Option<String>,
}

/// A validator execution event to log.
#[derive(Debug)]
pub struct ValidatorEvent<'a> {
    /// The validator name.
    pub name: &'a str,
    /// Whether the validator passed.
    pub passed: bool,
    /// The validator message.
    pub message: &'a str,
    /// The hook type that triggered this validator.
    pub hook_type: &'a str,
}

/// Holds the agent and notification channel.
struct AgentHandle {
    agent: Arc<dyn Agent + Send + Sync>,
    notifications: broadcast::Sender<SessionNotification>,
}

/// AVP Context - manages the AVP directory, logging, and agent access.
///
/// All AVP directory logic is centralized here. The directory is created
/// at the git repository root using the shared `swissarmyhammer-directory` crate.
///
/// The context tracks both project-level and user-level directories:
/// - Project: `./<AVP_DIR>/` at git root
/// - User: `~/<AVP_DIR>/` in home directory
///
/// The context also provides access to an ACP Agent for validator execution.
/// The agent is created lazily on first access, or can be injected for testing.
pub struct AvpContext {
    /// Managed directory at git root (<AVP_DIR>)
    project_dir: ManagedDirectory<AvpConfig>,

    /// Managed directory at user home (~/<AVP_DIR>), if available
    home_dir: Option<ManagedDirectory<AvpConfig>>,

    /// Shared log file handle (None if logging failed to initialize).
    log_file: Option<Arc<StdMutex<File>>>,

    /// Agent handle (lazily created or injected)
    agent_handle: Arc<Mutex<Option<AgentHandle>>>,
}

impl std::fmt::Debug for AvpContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AvpContext")
            .field("project_dir", &self.project_dir.root())
            .field("home_dir", &self.home_dir.as_ref().map(|d| d.root()))
            .field("has_log_file", &self.log_file.is_some())
            .field("has_agent", &"<async>")
            .finish()
    }
}

impl AvpContext {
    /// Initialize AVP context by finding git root and creating the AVP directory.
    ///
    /// This will:
    /// 1. Create AVP directory at git root (via swissarmyhammer-directory)
    /// 2. Create .gitignore in the AVP directory if it doesn't exist
    /// 3. Open log file for appending
    /// 4. Optionally connect to user AVP directory
    ///
    /// The agent is created lazily on first access.
    ///
    /// Returns Err if not in a git repository.
    pub fn init() -> Result<Self, AvpError> {
        // Create project directory at git root
        let project_dir = ManagedDirectory::<AvpConfig>::from_git_root().map_err(|e| {
            AvpError::Context(format!(
                "failed to create {} directory: {}",
                AvpConfig::DIR_NAME,
                e
            ))
        })?;

        // Try to create user directory (optional, may fail if no home dir)
        let home_dir = ManagedDirectory::<AvpConfig>::from_user_home().ok();

        // Open log file for appending (best-effort)
        let log_file = open_log_file(project_dir.root());

        Ok(Self {
            project_dir,
            home_dir,
            log_file,
            agent_handle: Arc::new(Mutex::new(None)),
        })
    }

    /// Create an AVP context with an injected agent.
    ///
    /// This is primarily for testing with PlaybackAgent or other test agents.
    /// The agent is used immediately without lazy creation.
    pub fn with_agent(
        agent: Arc<dyn Agent + Send + Sync>,
        notifications: broadcast::Receiver<SessionNotification>,
    ) -> Result<Self, AvpError> {
        // Create project directory at git root
        let project_dir = ManagedDirectory::<AvpConfig>::from_git_root().map_err(|e| {
            AvpError::Context(format!(
                "failed to create {} directory: {}",
                AvpConfig::DIR_NAME,
                e
            ))
        })?;

        // Try to create user directory (optional, may fail if no home dir)
        let home_dir = ManagedDirectory::<AvpConfig>::from_user_home().ok();

        // Open log file for appending (best-effort)
        let log_file = open_log_file(project_dir.root());

        // Create a sender from the receiver (for resubscription)
        // We need to create a new channel and the injected receiver becomes the first subscriber
        let (tx, _) = broadcast::channel(256);

        // Spawn a task to forward notifications from the injected receiver to our sender
        let tx_clone = tx.clone();
        let mut rx = notifications;
        tokio::spawn(async move {
            while let Ok(notification) = rx.recv().await {
                let _ = tx_clone.send(notification);
            }
        });

        Ok(Self {
            project_dir,
            home_dir,
            log_file,
            agent_handle: Arc::new(Mutex::new(Some(AgentHandle {
                agent,
                notifications: tx,
            }))),
        })
    }

    /// Get the agent for validator execution.
    ///
    /// Creates an ephemeral ClaudeAgent on first access if not already created.
    /// Returns a reference to the agent and a notification receiver.
    pub async fn agent(
        &self,
    ) -> Result<
        (
            Arc<dyn Agent + Send + Sync>,
            broadcast::Receiver<SessionNotification>,
        ),
        AvpError,
    > {
        let mut guard = self.agent_handle.lock().await;

        if guard.is_none() {
            tracing::debug!("Creating ephemeral ClaudeAgent for validator execution...");
            let start = std::time::Instant::now();

            // Create ephemeral agent (haiku model, no session persistence)
            let config = CreateAgentConfig::builder().ephemeral(true).build();

            let (agent, notifications) = claude_agent::create_agent(config)
                .await
                .map_err(|e| AvpError::Agent(format!("Failed to create agent: {}", e)))?;

            // Create a broadcast channel for notifications
            let (tx, _) = broadcast::channel(256);
            let tx_clone = tx.clone();

            // Forward notifications from the agent to our channel
            tokio::spawn(async move {
                let mut rx = notifications;
                while let Ok(notification) = rx.recv().await {
                    let _ = tx_clone.send(notification);
                }
            });

            tracing::debug!(
                "Ephemeral ClaudeAgent created in {:.2}s",
                start.elapsed().as_secs_f64()
            );

            *guard = Some(AgentHandle {
                agent: Arc::new(agent),
                notifications: tx,
            });
        }

        let handle = guard.as_ref().unwrap();
        Ok((Arc::clone(&handle.agent), handle.notifications.subscribe()))
    }

    /// Get the project AVP directory path.
    pub fn avp_dir(&self) -> &Path {
        self.project_dir.root()
    }

    /// Get the project validators directory path (./<AVP_DIR>/validators).
    ///
    /// Returns the path even if it doesn't exist yet.
    pub fn project_validators_dir(&self) -> PathBuf {
        self.project_dir.subdir("validators")
    }

    /// Get the user validators directory path (~/<AVP_DIR>/validators).
    ///
    /// Returns None if user directory is not available.
    pub fn home_validators_dir(&self) -> Option<PathBuf> {
        self.home_dir.as_ref().map(|d| d.subdir("validators"))
    }

    /// Ensure the project validators directory exists.
    ///
    /// Creates the directory if it doesn't exist.
    pub fn ensure_project_validators_dir(&self) -> Result<PathBuf, AvpError> {
        self.project_dir
            .ensure_subdir("validators")
            .map_err(|e| AvpError::Context(format!("failed to create validators directory: {}", e)))
    }

    /// Ensure the user validators directory exists.
    ///
    /// Creates the directory if it doesn't exist.
    /// Returns None if user directory is not available.
    pub fn ensure_home_validators_dir(&self) -> Option<Result<PathBuf, AvpError>> {
        self.home_dir.as_ref().map(|d| {
            d.ensure_subdir("validators").map_err(|e| {
                AvpError::Context(format!("failed to create user validators directory: {}", e))
            })
        })
    }

    /// Get all validator directories that exist.
    ///
    /// Returns directories in precedence order (user first, then project).
    pub fn existing_validator_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        // User directory (lower precedence)
        if let Some(home_dir) = self.home_validators_dir() {
            if home_dir.exists() {
                dirs.push(home_dir);
            }
        }

        // Project directory (higher precedence)
        let project_dir = self.project_validators_dir();
        if project_dir.exists() {
            dirs.push(project_dir);
        }

        dirs
    }

    /// Log a hook event.
    ///
    /// Format: `2024-01-23T10:15:32.123Z PreToolUse decision=allow tool=Bash`
    pub fn log_event(&self, event: &HookEvent) {
        let Some(log_file) = &self.log_file else {
            return;
        };

        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ");
        let details_str = event
            .details
            .as_ref()
            .map(|d| format!(" {}", d))
            .unwrap_or_default();

        let line = format!(
            "{} {} decision={}{}\n",
            timestamp, event.hook_type, event.decision, details_str
        );

        if let Ok(mut file) = log_file.lock() {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }

    /// Log a validator execution event.
    ///
    /// Format: `2024-01-23T10:15:32.123Z VALIDATOR rust-coding passed hook=PostToolUse "No issues found"`
    pub fn log_validator(&self, event: &ValidatorEvent) {
        let Some(log_file) = &self.log_file else {
            return;
        };

        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ");
        let status = if event.passed { "passed" } else { "FAILED" };

        // Truncate message for log readability
        let message = if event.message.len() > 100 {
            format!("{}...", &event.message[..97])
        } else {
            event.message.to_string()
        };

        let line = format!(
            "{} VALIDATOR {} {} hook={} \"{}\"\n",
            timestamp, event.name, status, event.hook_type, message
        );

        if let Ok(mut file) = log_file.lock() {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }
}

/// Open log file for appending.
fn open_log_file(avp_dir: &Path) -> Option<Arc<StdMutex<File>>> {
    let log_path = avp_dir.join(LOG_FILE_NAME);
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok()
        .map(|f| Arc::new(StdMutex::new(f)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_decision_display() {
        assert_eq!(format!("{}", Decision::Allow), "allow");
        assert_eq!(format!("{}", Decision::Block), "block");
        assert_eq!(format!("{}", Decision::Error), "error");
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_context_with_git_root() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let result = AvpContext::init();

        // Restore original directory
        std::env::set_current_dir(&original_dir).unwrap();

        assert!(result.is_ok());
        let ctx = result.unwrap();
        assert!(ctx.avp_dir().exists());
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_context_validators_dir() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = AvpContext::init().unwrap();

        // Validators dir path should be returned even if it doesn't exist
        let validators_path = ctx.project_validators_dir();
        assert!(validators_path.ends_with("validators"));

        // Ensure creates it
        let ensured_path = ctx.ensure_project_validators_dir().unwrap();
        assert!(ensured_path.exists());

        std::env::set_current_dir(&original_dir).unwrap();
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_context_not_in_git_repo() {
        let temp = TempDir::new().unwrap();
        // No .git directory

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let result = AvpContext::init();

        std::env::set_current_dir(&original_dir).unwrap();

        assert!(result.is_err());
    }
}
