//! Claude CLI process management for persistent stream-json communication
//!
//! This module provides process management capabilities for spawning and maintaining
//! persistent `claude` CLI processes that communicate via the stream-json protocol.
//!
//! # Architecture
//!
//! The module provides two main types:
//!
//! - [`ClaudeProcessManager`]: Manages a collection of claude processes, one per session.
//!   Provides session-level operations like spawn, get, terminate, and session existence checks.
//!
//! - [`ClaudeProcess`]: Represents a single persistent claude CLI child process.
//!   Provides low-level I/O operations (read/write lines), process lifecycle management,
//!   and status checking.
//!
//! # Stream-JSON Protocol
//!
//! The claude CLI is spawned with the flags to enable stream-json communication.
//!
//!
//! Messages are exchanged as newline-delimited JSON objects conforming to the
//! JSON-RPC 2.0 specification for Agent Communication Protocol (ACP).
//!
//! # Thread Safety
//!
//! [`ClaudeProcessManager`] is thread-safe and can be safely shared across threads using `Arc`.
//! It uses `Arc<RwLock<HashMap>>` internally to allow concurrent read access for session lookups
//! while serializing write operations (spawn/terminate).
//!
//! Individual [`ClaudeProcess`] instances are wrapped in `Arc<Mutex<>>` to allow exclusive
//! access for I/O operations, preventing data races when reading/writing to stdin/stdout.
//!
//! # Usage Example
//!
//! ```ignore
//! use claude_agent::claude_process::{ClaudeProcessManager, SpawnConfig};
//! use claude_agent::session::SessionId;
//! use std::path::PathBuf;
//!
//! # async fn example() -> claude_agent::Result<()> {
//! let manager = ClaudeProcessManager::new();
//! let session_id = SessionId::new();
//! let acp_session_id = agent_client_protocol::schema::SessionId::new("test".to_string());
//!
//! // Spawn a new process using SpawnConfig builder
//! let config = SpawnConfig::builder()
//!     .session_id(session_id.clone())
//!     .acp_session_id(acp_session_id)
//!     .cwd(PathBuf::from("/tmp"))
//!     .build();
//! manager.spawn_for_session(config).await?;
//!
//! // Get the process and interact with it
//! let process = manager.get_process(&session_id)?;
//! let mut proc = process.lock().await;
//!
//! // Write a JSON-RPC message
//! proc.write_line(r#"{"jsonrpc":"2.0","method":"initialize","params":{},"id":1}"#).await?;
//!
//! // Read the response
//! if let Some(response) = proc.read_line().await? {
//!     println!("Received: {}", response);
//! }
//!
//! drop(proc); // Release lock before terminating
//!
//! // Terminate when done
//! manager.terminate_session(&session_id).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Error Handling
//!
//! Operations return [`crate::Result<T>`] which wraps [`crate::AgentError`]:
//!
//! - `AgentError::Internal`: Process spawn failures, I/O errors, binary not found
//! - `AgentError::Session`: Session already exists, session not found
//!
//! # Process Lifecycle
//!
//! 1. **Spawn**: `ClaudeProcess::spawn()` creates a new child process with stdin/stdout/stderr pipes
//! 2. **Active**: Process runs persistently, accepting JSON messages on stdin and emitting on stdout
//! 3. **Shutdown**: `shutdown()` drops stdin (signaling EOF), waits for graceful exit with 5s timeout,
//!    then force-kills if necessary
//!
//! Processes are automatically cleaned up when terminated via the manager, but callers must ensure
//! no `Arc<Mutex<ClaudeProcess>>` references are held when calling `terminate_session()`.

use crate::session::SessionId;
use crate::{AgentError, Result};
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use swissarmyhammer_common::Pretty;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use typed_builder::TypedBuilder;

/// Configuration for spawning a Claude process.
///
/// Uses builder pattern to allow flexible configuration without
/// breaking changes when new options are added.
#[derive(Debug, Clone, TypedBuilder)]
pub struct SpawnConfig {
    /// Session ID for this Claude process
    pub session_id: SessionId,
    /// ACP protocol session ID (for protocol translation)
    pub acp_session_id: agent_client_protocol::schema::SessionId,
    /// Working directory for the process
    pub cwd: PathBuf,
    /// Optional Claude agent mode (e.g., "code")
    #[builder(default)]
    pub agent_mode: Option<String>,
    /// Optional system prompt to replace Claude's default
    #[builder(default)]
    pub system_prompt: Option<String>,
    /// MCP servers to configure
    #[builder(default)]
    pub mcp_servers: Vec<crate::config::McpServerConfig>,
    /// Ephemeral mode: uses haiku model and no session persistence
    #[builder(default)]
    pub ephemeral: bool,
    /// Override for Claude's built-in tools. When set to Some(""), disables all built-in tools.
    /// This is used for validator agents that should only have MCP-provided tools.
    #[builder(default)]
    pub tools_override: Option<String>,
}

/// Claude CLI command-line arguments for stream-json communication
const CLAUDE_CLI_ARGS: &[&str] = &[
    "--verbose", // REQUIRED for stream-json output format
    "--print",   // print mode (non-interactive)
    "--input-format",
    "stream-json", // accept newline-delimited JSON on stdin
    "--output-format",
    "stream-json",                    // emit newline-delimited JSON on stdout
    "--dangerously-skip-permissions", // ACP server handles permission checks
    // Load filesystem settings so the board's `.claude/settings.json` (project)
    // and `.claude/settings.local.json` (local) are honored. In `--print` mode
    // the CLI loads NO settings sources by default, so without this the board's
    // `permissions.deny`/`allow` rules are silently ignored. `user` is included
    // to match the sources interactive Claude loads by default. Note: even under
    // `--dangerously-skip-permissions`, `permissions.deny` is a hard rule the CLI
    // still enforces — once these sources are actually loaded.
    "--setting-sources",
    "user,project,local",
    "--include-partial-messages", // Emit partial messages for immediate streaming
    "--no-session-persistence",   // Disable built-in session persistence (we manage it ourselves)
    // NOTE: This causes Claude to send a duplicate final combined message and empty terminator
    // We filter these out in the streaming loop (skip large chunks and empty chunks)
    "--replay-user-messages", // Re-emit user messages for transcript recording
];

/// Manages multiple persistent claude CLI processes, one per session
///
/// # Thread Safety
///
/// This type is thread-safe and can be safely shared across threads using `Arc<ClaudeProcessManager>`.
///
/// The internal `processes` map uses `Arc<RwLock<HashMap>>` which provides:
/// - **Concurrent reads**: Multiple threads can simultaneously check session existence or retrieve processes
/// - **Exclusive writes**: Spawn and terminate operations acquire exclusive write locks, preventing races
///
/// Individual processes are wrapped in `Arc<Mutex<ClaudeProcess>>` to ensure exclusive access
/// for I/O operations. Callers must acquire the mutex lock before reading/writing to a process.
///
/// # Important
///
/// When calling `terminate_session()`, ensure no `Arc<Mutex<ClaudeProcess>>` references are held,
/// as termination requires exclusive ownership. Drop all process references before terminating.
#[derive(Debug)]
pub struct ClaudeProcessManager {
    processes: Arc<RwLock<HashMap<SessionId, Arc<Mutex<ClaudeProcess>>>>>,
}

impl ClaudeProcessManager {
    /// Create a new process manager
    pub fn new() -> Self {
        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Spawn a new claude process for the given session
    /// # Arguments
    /// * `session_id` - Session identifier
    /// * `config` - Spawn configuration including session_id, cwd, agent_mode, etc.
    ///
    /// # Errors
    /// Returns error if:
    /// - Session already has a process
    /// - Failed to spawn claude binary
    /// - Process spawn fails
    pub async fn spawn_for_session(&self, config: SpawnConfig) -> Result<()> {
        // Check if session already exists - use write lock to prevent race
        let mut processes = self.processes.write().map_err(|_| {
            AgentError::Internal("Failed to acquire write lock on processes".to_string())
        })?;

        if processes.contains_key(&config.session_id) {
            // Process already exists, this is fine - just return success
            tracing::debug!("Process already exists for session {}", config.session_id);
            return Ok(());
        }

        let session_id = config.session_id;

        // Spawn new process with config
        let process = ClaudeProcess::spawn(config).map_err(|e| {
            tracing::error!(
                "Failed to spawn claude process for session {}: {}",
                session_id,
                e
            );
            e
        })?;

        // Insert into map
        processes.insert(session_id, Arc::new(Mutex::new(process)));

        tracing::info!("Spawned claude process for session {}", session_id);
        Ok(())
    }

    /// Get the process for a session, spawning one if it doesn't exist
    ///
    /// # Arguments
    /// * `session_id` - Session identifier
    /// * `cwd` - Working directory for the Claude process if spawning is needed
    /// # Errors
    /// Returns error if no process exists for this session
    pub fn get_process(&self, session_id: &SessionId) -> Result<Arc<Mutex<ClaudeProcess>>> {
        let processes = self.processes.read().map_err(|_| {
            AgentError::Internal("Failed to acquire read lock on processes".to_string())
        })?;

        processes.get(session_id).cloned().ok_or_else(|| {
            AgentError::Internal(format!(
                "No Claude process exists for session {}. Process must be spawned first.",
                session_id
            ))
        })
    }

    /// Spawn a new Claude process for a session
    ///
    /// # Arguments
    /// * `config` - Spawn configuration including session_id, cwd, agent_mode, etc.
    ///
    /// # Errors
    /// Returns error if spawning fails
    pub async fn spawn_process(&self, config: SpawnConfig) -> Result<Arc<Mutex<ClaudeProcess>>> {
        tracing::info!(
            "Spawning Claude process for session {} in {}, agent_mode={:?}, system_prompt={}, ephemeral={}",
            config.session_id,
            config.cwd.display(),
            config.agent_mode,
            config.system_prompt
                .as_ref()
                .map(|s| format!("{} chars", s.len()))
                .unwrap_or_else(|| "None".to_string()),
            config.ephemeral
        );

        let session_id = config.session_id;
        self.spawn_for_session(config).await?;

        // Get the newly spawned process
        let processes = self.processes.read().map_err(|_| {
            AgentError::Internal("Failed to acquire read lock on processes".to_string())
        })?;

        tracing::info!(
            "Spawned new Claude process for session {} (total active: {})",
            session_id,
            processes.len()
        );

        processes.get(&session_id).cloned().ok_or_else(|| {
            AgentError::Internal("Process spawn succeeded but not found in map".to_string())
        })
    }

    /// Terminate a session's process
    ///
    /// # Errors
    /// Returns error if session does not exist or shutdown fails
    pub async fn terminate_session(&self, session_id: &SessionId) -> Result<()> {
        // Remove from map
        let process = {
            let mut processes = self.processes.write().map_err(|_| {
                AgentError::Internal("Failed to acquire write lock on processes".to_string())
            })?;
            processes.remove(session_id)
        };

        if let Some(process_arc) = process {
            // Take ownership and shutdown
            let process = Arc::try_unwrap(process_arc).map_err(|_| {
                AgentError::Internal("Process still has multiple references".to_string())
            })?;
            let process = process.into_inner();

            process.shutdown().await?;
            tracing::info!("Terminated claude process for session {}", session_id);
            Ok(())
        } else {
            Err(AgentError::Session(format!(
                "No process for session {}",
                session_id
            )))
        }
    }

    /// Check if a session has a process
    pub async fn has_session(&self, session_id: &SessionId) -> bool {
        self.processes
            .read()
            .ok()
            .map(|processes| processes.contains_key(session_id))
            .unwrap_or(false)
    }
}

impl Default for ClaudeProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

/// A persistent claude CLI process for stream-json communication
#[derive(Debug)]
pub struct ClaudeProcess {
    session_id: SessionId,
    child: Child,
    stdin: ManuallyDrop<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    stderr: BufReader<ChildStderr>,
    /// Flag to prevent Drop from killing if shutdown was called
    shutdown_called: bool,
}

impl ClaudeProcess {
    /// Spawn a new claude process with stream-json flags
    ///
    /// # Arguments
    /// * `config` - Spawn configuration containing session_id, cwd, agent_mode, system_prompt, etc.
    ///
    /// # Errors
    /// Returns error if:
    /// - claude binary not found
    /// - Process spawn fails
    /// - stdin/stdout/stderr not available
    pub fn spawn(config: SpawnConfig) -> Result<Self> {
        let test_context = std::thread::current().name().map(|n| n.to_string());
        let claude_session_uuid = uuid::Uuid::new_v4().to_string();

        Self::log_spawn_info(&config, &claude_session_uuid);

        let mut command = Self::build_base_command(&claude_session_uuid);
        Self::configure_agent_mode(&mut command, &config);
        Self::configure_system_prompt(&mut command, &config);
        Self::configure_ephemeral_mode(&mut command, &config);
        Self::configure_tools_override(&mut command, &config);
        Self::configure_mcp_servers(&mut command, &config);
        Self::log_command(&command);

        let cmd = Self::execute_spawn(&mut command, &config)?;
        Self::create_process_instance(config.session_id, cmd, test_context)
    }

    /// Log spawn configuration info.
    fn log_spawn_info(config: &SpawnConfig, claude_session_uuid: &str) {
        tracing::info!(
            "ClaudeProcess::spawn for session {} with Claude UUID {}, {} MCP servers, ephemeral={}",
            config.session_id,
            claude_session_uuid,
            config.mcp_servers.len(),
            config.ephemeral
        );
    }

    /// Build the base command with required args.
    fn build_base_command(claude_session_uuid: &str) -> Command {
        let mut command = Command::new("claude");
        command
            .args(CLAUDE_CLI_ARGS)
            .arg("--session-id")
            .arg(claude_session_uuid)
            .env("CLAUDE_ACP", "1")
            // Allow spawning Claude from within a Claude Code session
            .env_remove("CLAUDECODE");
        command
    }

    /// Configure agent mode if specified.
    fn configure_agent_mode(command: &mut Command, config: &SpawnConfig) {
        if let Some(ref mode) = config.agent_mode {
            tracing::info!("Spawning Claude with agent mode: {}", mode);
            command.arg("--agent").arg(mode);
        }
    }

    /// Configure system prompt if specified.
    fn configure_system_prompt(command: &mut Command, config: &SpawnConfig) {
        if let Some(ref prompt) = config.system_prompt {
            tracing::info!(
                "Spawning Claude with SwissArmyHammer system prompt ({} chars)",
                prompt.len()
            );
            command.arg("--system-prompt").arg(prompt);
        }
    }

    /// Configure ephemeral mode settings.
    fn configure_ephemeral_mode(command: &mut Command, config: &SpawnConfig) {
        if config.ephemeral {
            tracing::info!("Spawning Claude in ephemeral mode (haiku, no session persistence)");
            command.arg("--model").arg("haiku");
        }
    }

    /// Configure tools override if specified.
    /// When set to Some(""), passes `--tools ""` to disable all built-in tools.
    fn configure_tools_override(command: &mut Command, config: &SpawnConfig) {
        if let Some(ref tools) = config.tools_override {
            tracing::info!("Spawning Claude with tools override: {:?}", tools);
            command.arg("--tools").arg(tools);
        }
    }

    /// Configure MCP servers if specified.
    fn configure_mcp_servers(command: &mut Command, config: &SpawnConfig) {
        if config.mcp_servers.is_empty() {
            return;
        }

        tracing::info!(
            "Building MCP config for Claude with {} servers",
            config.mcp_servers.len()
        );

        let mcp_servers_obj = Self::build_mcp_servers_map(&config.mcp_servers);
        let mcp_config = serde_json::json!({ "mcpServers": mcp_servers_obj });

        Self::write_mcp_config_file(command, config, &mcp_config);
    }

    /// Build MCP servers JSON map.
    fn build_mcp_servers_map(
        servers: &[crate::config::McpServerConfig],
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut mcp_servers_obj = serde_json::Map::new();
        for server in servers {
            match server {
                crate::config::McpServerConfig::Http(http) => {
                    mcp_servers_obj.insert(
                        http.name.clone(),
                        serde_json::json!({
                            "type": "http",
                            "url": http.url,
                            "headers": {}
                        }),
                    );
                }
                crate::config::McpServerConfig::Sse(sse) => {
                    mcp_servers_obj.insert(
                        sse.name.clone(),
                        serde_json::json!({
                            "type": "sse",
                            "url": sse.url,
                            "headers": {}
                        }),
                    );
                }
                crate::config::McpServerConfig::Stdio(_) => {
                    tracing::warn!("Stdio MCP servers not supported for Claude CLI");
                }
            }
        }
        mcp_servers_obj
    }

    /// Write MCP config to temp file and add command args.
    fn write_mcp_config_file(
        command: &mut Command,
        config: &SpawnConfig,
        mcp_config: &serde_json::Value,
    ) {
        let temp_dir = std::env::temp_dir();
        let mcp_config_path =
            temp_dir.join(format!("claude_mcp_config_{}.json", config.session_id));

        if let Err(e) = std::fs::write(
            &mcp_config_path,
            serde_json::to_string_pretty(mcp_config).unwrap(),
        ) {
            tracing::error!("Failed to write MCP config: {}", e);
        } else {
            tracing::info!("Wrote MCP config to {}", Pretty(&mcp_config_path));
            command.arg("--mcp-config").arg(&mcp_config_path);
            command.arg("--strict-mcp-config");
        }
    }

    /// Log the complete command being executed.
    fn log_command(command: &Command) {
        #[derive(serde::Serialize, Debug)]
        struct CommandInfo {
            program: String,
            args: Vec<String>,
        }
        let cmd_info = CommandInfo {
            program: command.as_std().get_program().to_string_lossy().to_string(),
            args: command
                .as_std()
                .get_args()
                .map(|s| s.to_string_lossy().to_string())
                .collect(),
        };
        tracing::info!("🚀 Spawning Claude CLI: {}", Pretty(&cmd_info));
    }

    /// Execute the spawn and handle errors.
    fn execute_spawn(command: &mut Command, config: &SpawnConfig) -> Result<Child> {
        command
            .current_dir(&config.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AgentError::Internal(
                        "claude binary not found in PATH. Please ensure claude CLI is installed."
                            .to_string(),
                    )
                } else {
                    AgentError::Internal(format!("Failed to spawn claude process: {}", e))
                }
            })
    }

    /// Create the ClaudeProcess instance from spawned child.
    fn create_process_instance(
        session_id: SessionId,
        mut cmd: Child,
        test_context: Option<String>,
    ) -> Result<Self> {
        let stdin = cmd.stdin.take().ok_or_else(|| {
            AgentError::Internal("Failed to capture claude process stdin".to_string())
        })?;

        let stdout = cmd.stdout.take().ok_or_else(|| {
            AgentError::Internal("Failed to capture claude process stdout".to_string())
        })?;

        let stderr = cmd.stderr.take().ok_or_else(|| {
            AgentError::Internal("Failed to capture claude process stderr".to_string())
        })?;

        let pid = cmd.id();
        tracing::info!(
            "Claude process spawned: session={}, pid={:?}, test={:?}",
            session_id,
            pid,
            test_context
        );

        Ok(Self {
            session_id,
            child: cmd,
            stdin: ManuallyDrop::new(stdin),
            stdout: BufReader::new(stdout),
            stderr: BufReader::new(stderr),
            shutdown_called: false,
        })
    }

    /// Write a line to the process stdin
    ///
    /// # Errors
    /// Returns error if write or flush fails
    pub async fn write_line(&mut self, line: &str) -> Result<()> {
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| AgentError::Internal(format!("Failed to write to claude stdin: {}", e)))?;

        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|e| AgentError::Internal(format!("Failed to write newline: {}", e)))?;

        self.stdin
            .flush()
            .await
            .map_err(|e| AgentError::Internal(format!("Failed to flush claude stdin: {}", e)))?;

        tracing::trace!("Wrote line to session {}: {}", self.session_id, line);
        Ok(())
    }

    /// Read a line from the process stdout
    ///
    /// Returns None if EOF (process terminated)
    ///
    /// # Errors
    /// Returns error if read fails (but not on EOF)
    pub async fn read_line(&mut self) -> Result<Option<String>> {
        let mut line = String::new();
        let bytes_read = self.stdout.read_line(&mut line).await.map_err(|e| {
            AgentError::Internal(format!("Failed to read from claude stdout: {}", e))
        })?;

        if bytes_read == 0 {
            tracing::debug!("EOF on claude stdout for session {}", self.session_id);
            return Ok(None);
        }

        // Remove trailing newline
        let line = line.trim_end().to_string();
        tracing::trace!("Read line from session {}: {}", self.session_id, line);
        Ok(Some(line))
    }

    /// Read a line from the process stderr
    ///
    /// Returns None if EOF
    ///
    /// # Errors
    /// Returns error if read fails (but not on EOF)
    pub async fn read_stderr_line(&mut self) -> Result<Option<String>> {
        let mut line = String::new();
        let bytes_read = self.stderr.read_line(&mut line).await.map_err(|e| {
            AgentError::Internal(format!("Failed to read from claude stderr: {}", e))
        })?;

        if bytes_read == 0 {
            return Ok(None);
        }

        // Remove trailing newline
        let line = line.trim_end().to_string();
        tracing::trace!(
            "Read stderr line from session {}: {}",
            self.session_id,
            line
        );
        Ok(Some(line))
    }

    /// Check if the process is still alive
    pub async fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(status)) => {
                tracing::debug!(
                    "Claude process for session {} exited with status: {}",
                    self.session_id,
                    status
                );
                false
            }
            Ok(None) => true,
            Err(e) => {
                tracing::error!(
                    "Error checking claude process status for session {}: {}",
                    self.session_id,
                    e
                );
                false
            }
        }
    }

    /// Gracefully shutdown the process
    ///
    /// Attempts graceful termination first, then force kills if needed
    ///
    /// # Errors
    /// Returns error if force kill fails
    pub async fn shutdown(mut self) -> Result<()> {
        tracing::debug!(
            "Shutting down claude process for session {}",
            self.session_id
        );

        // Mark that shutdown was called to prevent Drop from running
        self.shutdown_called = true;

        // Manually drop stdin to signal EOF to the process
        unsafe {
            ManuallyDrop::drop(&mut self.stdin);
        }

        // Try to wait for graceful exit with timeout
        // Use try_wait in a loop to avoid blocking and retain access to child
        let start = std::time::Instant::now();
        let timeout_duration = Duration::from_secs(5);

        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    tracing::info!(
                        "Claude process for session {} exited gracefully with status: {}",
                        self.session_id,
                        status
                    );
                    return Ok(());
                }
                Ok(None) => {
                    // Still running, check timeout
                    if start.elapsed() >= timeout_duration {
                        tracing::warn!(
                            "Claude process for session {} did not exit gracefully, force killing",
                            self.session_id
                        );
                        // Force kill the process
                        if let Err(e) = self.child.kill().await {
                            tracing::error!(
                                "Failed to force kill claude process for session {}: {}",
                                self.session_id,
                                e
                            );
                            return Err(AgentError::Internal(format!(
                                "Failed to force kill process: {}",
                                e
                            )));
                        }
                        // Wait for the killed process to clean up
                        if let Err(e) = self.child.wait().await {
                            tracing::error!(
                                "Failed to wait after killing claude process for session {}: {}",
                                self.session_id,
                                e
                            );
                        }
                        return Ok(());
                    }
                    // Sleep briefly before checking again
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    tracing::error!(
                        "Error checking claude process status for session {}: {}",
                        self.session_id,
                        e
                    );
                    return Err(AgentError::Internal(format!(
                        "Failed to check process status: {}",
                        e
                    )));
                }
            }
        }
    }

    /// Get the session ID for this process
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }
}

impl Drop for ClaudeProcess {
    fn drop(&mut self) {
        // If shutdown was already called, don't kill again
        if self.shutdown_called {
            tracing::debug!(
                "Dropping ClaudeProcess for session {}, shutdown already called",
                self.session_id
            );
            return;
        }

        tracing::debug!(
            "Dropping ClaudeProcess for session {}, force-killing process",
            self.session_id
        );

        // CRITICAL: Force-kill the child process immediately
        // We can't use async here (Drop must be sync), so we use start_kill()
        // which sends SIGKILL immediately without waiting
        if let Err(e) = self.child.start_kill() {
            // Only log if the process wasn't already dead
            if e.kind() != std::io::ErrorKind::InvalidInput {
                tracing::error!(
                    "Failed to force-kill claude process for session {} during drop: {}",
                    self.session_id,
                    e
                );
            }
        } else {
            tracing::info!(
                "Force-killed claude process for session {} during drop",
                self.session_id
            );
        }

        // Note: We don't wait for the process to exit here because Drop must be non-blocking
        // The OS will clean up the zombie process
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HttpHeader, HttpTransport, McpServerConfig, SseTransport};

    #[tokio::test]
    async fn test_process_manager_new() {
        let manager = ClaudeProcessManager::new();
        let session_id = SessionId::new();
        assert!(!manager.has_session(&session_id).await);
    }

    #[tokio::test]
    async fn test_terminate_nonexistent_session() {
        let manager = ClaudeProcessManager::new();
        let session_id = SessionId::new();

        let result = manager.terminate_session(&session_id).await;
        assert!(result.is_err());
        if let Err(AgentError::Session(msg)) = result {
            assert!(msg.contains("No process for session"));
        } else {
            panic!("Expected Session error");
        }
    }

    /// Helper to build a minimal `SpawnConfig` with the supplied MCP servers
    /// for testing the MCP-related command configuration in isolation.
    fn spawn_config_with_servers(servers: Vec<McpServerConfig>) -> SpawnConfig {
        SpawnConfig::builder()
            .session_id(SessionId::new())
            .acp_session_id(agent_client_protocol::schema::SessionId::new(
                "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            ))
            .cwd(std::env::temp_dir())
            .mcp_servers(servers)
            .build()
    }

    /// `build_mcp_servers_map` must convert each HTTP MCP server entry
    /// into a JSON `{"type":"http","url":"...","headers":{}}` block keyed
    /// by the server name — this is exactly the shape the Claude CLI
    /// expects in its `--mcp-config` file.
    #[test]
    fn test_build_mcp_servers_map_with_http_server() {
        let servers = vec![McpServerConfig::Http(HttpTransport {
            transport_type: "http".to_string(),
            name: "per-board-kanban".to_string(),
            url: "http://127.0.0.1:54321/mcp".to_string(),
            headers: vec![HttpHeader {
                name: "X-Test".to_string(),
                value: "ignored-by-builder".to_string(),
            }],
        })];

        let map = ClaudeProcess::build_mcp_servers_map(&servers);
        let entry = map
            .get("per-board-kanban")
            .expect("HTTP server must be keyed by name");
        assert_eq!(entry["type"], "http");
        assert_eq!(entry["url"], "http://127.0.0.1:54321/mcp");
        // The current builder always emits an empty `headers` object; lock
        // that in so a regression that starts injecting headers under the
        // wrong key is caught.
        assert!(entry["headers"].is_object());
    }

    /// SSE servers must be emitted with `"type":"sse"` so the Claude CLI
    /// picks the correct transport.
    #[test]
    fn test_build_mcp_servers_map_with_sse_server() {
        let servers = vec![McpServerConfig::Sse(SseTransport {
            transport_type: "sse".to_string(),
            name: "events-server".to_string(),
            url: "https://events.example.com/sse".to_string(),
            headers: vec![],
        })];

        let map = ClaudeProcess::build_mcp_servers_map(&servers);
        let entry = map
            .get("events-server")
            .expect("SSE server must be present");
        assert_eq!(entry["type"], "sse");
        assert_eq!(entry["url"], "https://events.example.com/sse");
    }

    /// Empty input must produce an empty map (and downstream this means
    /// `configure_mcp_servers` skips the temp file and the CLI flags).
    #[test]
    fn test_build_mcp_servers_map_empty() {
        let map = ClaudeProcess::build_mcp_servers_map(&[]);
        assert!(map.is_empty());
    }

    /// With a non-empty MCP server list, `configure_mcp_servers` must add
    /// both `--mcp-config <file>` and `--strict-mcp-config` to the spawn
    /// command. Without `--strict-mcp-config`, the Claude CLI would merge
    /// in ambient global/project MCP config and pick up the wrong `kanban`
    /// server — which is the bug this task fixes.
    #[test]
    fn test_configure_mcp_servers_adds_strict_flag_for_non_empty_list() {
        let config = spawn_config_with_servers(vec![McpServerConfig::Http(HttpTransport {
            transport_type: "http".to_string(),
            name: "per-board-kanban".to_string(),
            url: "http://127.0.0.1:54321/mcp".to_string(),
            headers: vec![],
        })]);

        let mut command = Command::new("claude");
        ClaudeProcess::configure_mcp_servers(&mut command, &config);

        let args: Vec<String> = command
            .as_std()
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        assert!(
            args.iter().any(|a| a == "--mcp-config"),
            "Expected --mcp-config flag, got args: {:?}",
            args
        );
        assert!(
            args.iter().any(|a| a == "--strict-mcp-config"),
            "Expected --strict-mcp-config flag, got args: {:?}",
            args
        );

        // The `--mcp-config` flag must be followed by an existing file
        // path holding the JSON config the CLI will read.
        let cfg_pos = args
            .iter()
            .position(|a| a == "--mcp-config")
            .expect("--mcp-config not found in args");
        let cfg_path = args
            .get(cfg_pos + 1)
            .expect("--mcp-config must be followed by a file path");
        let cfg_path = std::path::PathBuf::from(cfg_path);
        assert!(
            cfg_path.exists(),
            "MCP config file written by configure_mcp_servers must exist on disk: {}",
            cfg_path.display()
        );

        let written = std::fs::read_to_string(&cfg_path).expect("MCP config file must be readable");
        let parsed: serde_json::Value =
            serde_json::from_str(&written).expect("MCP config file must be valid JSON");
        let server_entry = parsed
            .get("mcpServers")
            .and_then(|m| m.get("per-board-kanban"))
            .expect("Written MCP config must include the per-session server");
        assert_eq!(server_entry["type"], "http");
        assert_eq!(server_entry["url"], "http://127.0.0.1:54321/mcp");

        // Clean up the temp file so repeat test runs don't leave debris.
        let _ = std::fs::remove_file(&cfg_path);
    }

    /// With an empty MCP server list, `configure_mcp_servers` must add
    /// neither `--mcp-config` nor `--strict-mcp-config`. (If it did, the
    /// CLI would still be flagged strict and would reject the ambient
    /// config — but the point here is that an empty list means "no MCP
    /// configuration at all".)
    #[test]
    fn test_configure_mcp_servers_skips_flags_for_empty_list() {
        let config = spawn_config_with_servers(vec![]);

        let mut command = Command::new("claude");
        ClaudeProcess::configure_mcp_servers(&mut command, &config);

        let args: Vec<String> = command
            .as_std()
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        assert!(
            !args.iter().any(|a| a == "--mcp-config"),
            "Empty MCP server list must not add --mcp-config, got args: {:?}",
            args
        );
        assert!(
            !args.iter().any(|a| a == "--strict-mcp-config"),
            "Empty MCP server list must not add --strict-mcp-config, got args: {:?}",
            args
        );
    }

    /// The spawned headless `claude` runs in `--print` mode, which does NOT
    /// load project/local `.claude/settings.json` unless `--setting-sources`
    /// is passed. Without it the board's `permissions.deny`/`allow` rules are
    /// silently ignored, so the agent runs tools the board meant to block.
    /// The base command must pass `--setting-sources` with a value covering
    /// `project` (board `.claude/settings.json`) and `local`
    /// (`.claude/settings.local.json`) — matching the sources interactive
    /// Claude loads by default.
    #[test]
    fn test_base_command_loads_filesystem_setting_sources() {
        let command = ClaudeProcess::build_base_command("test-session-uuid");
        let args: Vec<String> = command
            .as_std()
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        let pos = args
            .iter()
            .position(|a| a == "--setting-sources")
            .unwrap_or_else(|| panic!("Expected --setting-sources flag, got args: {args:?}"));
        let value = args
            .get(pos + 1)
            .expect("--setting-sources must be followed by a value");
        assert!(
            value.split(',').any(|s| s == "project"),
            "--setting-sources value must include 'project', got: {value:?}"
        );
        assert!(
            value.split(',').any(|s| s == "local"),
            "--setting-sources value must include 'local', got: {value:?}"
        );
    }

    /// Regression guard: adding `--setting-sources` must not disturb the core
    /// stream-json contract the ACP pipeline depends on. The base command must
    /// still pass `--print` and the stream-json input/output format flags.
    #[test]
    fn test_base_command_retains_core_streamjson_args() {
        let command = ClaudeProcess::build_base_command("test-session-uuid");
        let args: Vec<String> = command
            .as_std()
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        assert!(
            args.iter().any(|a| a == "--print"),
            "missing --print, got args: {args:?}"
        );

        let in_pos = args
            .iter()
            .position(|a| a == "--input-format")
            .expect("missing --input-format");
        assert_eq!(
            args.get(in_pos + 1).map(String::as_str),
            Some("stream-json"),
            "--input-format must be followed by stream-json, got args: {args:?}"
        );

        let out_pos = args
            .iter()
            .position(|a| a == "--output-format")
            .expect("missing --output-format");
        assert_eq!(
            args.get(out_pos + 1).map(String::as_str),
            Some("stream-json"),
            "--output-format must be followed by stream-json, got args: {args:?}"
        );
    }
}
