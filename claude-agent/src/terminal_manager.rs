//! Terminal session management for ACP compliance
//!
//! This module provides comprehensive terminal session management following
//! the Agent Client Protocol (ACP) specification.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use swissarmyhammer_common::rate_limiter::{RateLimiter, RateLimiterConfig};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

/// Manages terminal sessions for command execution
#[derive(Debug, Clone)]
pub struct TerminalManager {
    pub terminals: Arc<RwLock<HashMap<String, TerminalSession>>>,
    rate_limiter: Arc<RateLimiter>,
    pub client_capabilities: Arc<RwLock<Option<agent_client_protocol::ClientCapabilities>>>,
}

/// Terminal lifecycle state
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalState {
    /// Terminal created but process not yet started
    Created,
    /// Process is currently running
    Running,
    /// Process completed with exit status
    Finished,
    /// Process killed by signal
    Killed,
    /// Resources released, terminal ID invalidated
    Released,
}

/// Default graceful shutdown timeout in seconds
pub const DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT_SECS: u64 = 5;

/// Newtype wrapper for graceful shutdown timeout duration
///
/// Provides type safety to prevent mixing up timeout durations with other Duration values.
/// This ensures that timeout configurations cannot be accidentally confused with other
/// time-based parameters in the system.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GracefulShutdownTimeout(Duration);

impl GracefulShutdownTimeout {
    /// Create a new graceful shutdown timeout
    pub fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// Get the timeout as a Duration
    pub fn as_duration(&self) -> Duration {
        self.0
    }
}

impl Default for GracefulShutdownTimeout {
    fn default() -> Self {
        Self(Duration::from_secs(DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT_SECS))
    }
}

/// Configuration for terminal timeout behavior
///
/// Controls graceful shutdown timeout for SIGKILL escalation during terminal cleanup.
///
/// IMPORTANT: Do not add execution timeouts for commands.
/// Commands should be allowed to run until completion or explicit user termination.
/// Only graceful shutdown timeout is appropriate for cleanup scenarios.
/// Execution timeouts create poor developer experience for legitimate long-running operations.
#[derive(Debug, Clone, Default)]
pub struct TimeoutConfig {
    /// Graceful shutdown timeout before escalating to SIGKILL
    pub graceful_shutdown_timeout: GracefulShutdownTimeout,
}

/// Represents a terminal session with working directory and environment
#[derive(Debug)]
pub struct TerminalSession {
    pub process: Option<Arc<RwLock<Child>>>,
    pub working_dir: std::path::PathBuf,
    pub environment: HashMap<String, String>,
    // ACP-compliant fields for terminal/create method
    pub command: Option<String>,
    pub args: Vec<String>,
    pub session_id: Option<String>,
    pub output_byte_limit: u64,
    pub output_buffer: Arc<RwLock<Vec<u8>>>,
    pub buffer_truncated: Arc<RwLock<bool>>,
    pub exit_status: Arc<RwLock<Option<ExitStatus>>>,
    pub state: Arc<RwLock<TerminalState>>,
    pub output_task: Option<JoinHandle<()>>,
    pub timeout_config: TimeoutConfig,
}

/// ACP-compliant request parameters for terminal/create method
///
/// This struct defines all the parameters needed to create a new terminal session
/// following the Anthropic Computer Protocol (ACP) specification.
#[derive(Debug, Clone, Deserialize)]
pub struct TerminalCreateParams {
    /// Session identifier that must exist and be a valid ULID format
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Command to execute in the terminal (e.g., "bash", "python", "echo")
    pub command: String,
    /// Optional command line arguments as a vector of strings
    pub args: Option<Vec<String>>,
    /// Optional environment variables to set for the terminal session
    pub env: Option<Vec<EnvVariable>>,
    /// Optional working directory path (must be absolute if provided)
    pub cwd: Option<String>,
    /// Optional byte limit for terminal output buffering (defaults to system limit)
    #[serde(rename = "outputByteLimit")]
    pub output_byte_limit: Option<u64>,
}

/// Environment variable specification for terminal creation
///
/// Represents a single environment variable to be set in the terminal session.
/// Environment variables override system defaults when names conflict.
#[derive(Debug, Clone, Deserialize)]
pub struct EnvVariable {
    /// Environment variable name (cannot be empty)
    pub name: String,
    /// Environment variable value
    pub value: String,
}

/// ACP-compliant response for terminal/create method
///
/// Returns the unique identifier for the newly created terminal session.
/// This terminal ID can be used for subsequent terminal operations.
#[derive(Debug, Serialize)]
pub struct TerminalCreateResponse {
    /// Unique terminal identifier (ULID format)
    #[serde(rename = "terminalId")]
    pub terminal_id: String,
}

/// ACP-compliant request parameters for terminal/output method
#[derive(Debug, Deserialize)]
pub struct TerminalOutputParams {
    /// Session identifier
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Terminal identifier
    #[serde(rename = "terminalId")]
    pub terminal_id: String,
}

/// ACP-compliant response for terminal/output method
#[derive(Debug, Serialize)]
pub struct TerminalOutputResponse {
    /// Terminal output as UTF-8 string
    pub output: String,
    /// Whether output has been truncated from the beginning
    pub truncated: bool,
    /// Exit status (only present when process has completed)
    #[serde(rename = "exitStatus", skip_serializing_if = "Option::is_none")]
    pub exit_status: Option<ExitStatus>,
}

/// Exit status information for completed processes
#[derive(Debug, Serialize, Clone)]
pub struct ExitStatus {
    /// Exit code (0 for success, non-zero for error)
    #[serde(rename = "exitCode")]
    pub exit_code: Option<i32>,
    /// Signal name if process was terminated by signal
    pub signal: Option<String>,
}

/// ACP-compliant request parameters for terminal/release method
#[derive(Debug, Deserialize)]
pub struct TerminalReleaseParams {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "terminalId")]
    pub terminal_id: String,
}

impl TerminalManager {
    /// Create a new terminal manager
    pub fn new() -> Self {
        Self::with_rate_limiter(RateLimiter::new())
    }

    /// Create a new terminal manager with custom rate limiter
    pub fn with_rate_limiter(rate_limiter: RateLimiter) -> Self {
        Self {
            terminals: Arc::new(RwLock::new(HashMap::new())),
            rate_limiter: Arc::new(rate_limiter),
            client_capabilities: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new terminal manager with custom rate limiter configuration
    pub fn with_rate_limiter_config(config: RateLimiterConfig) -> Self {
        Self::with_rate_limiter(RateLimiter::with_config(config))
    }

    /// Set client capabilities from initialize request
    pub async fn set_client_capabilities(
        &self,
        capabilities: agent_client_protocol::ClientCapabilities,
    ) {
        let mut caps = self.client_capabilities.write().await;
        *caps = Some(capabilities);
    }

    /// Check if client has terminal capability
    #[allow(dead_code)]
    async fn has_terminal_capability(&self) -> bool {
        let caps = self.client_capabilities.read().await;
        match &*caps {
            Some(caps) => caps.terminal,
            None => false,
        }
    }

    /// Validate that client has terminal capability, return error if not
    async fn validate_terminal_capability(&self) -> crate::Result<()> {
        let caps = self.client_capabilities.read().await;
        tracing::debug!("Terminal manager validating capabilities: {:?}", caps.as_ref().map(|c| c.terminal));
        match &*caps {
            Some(caps) if caps.terminal => Ok(()),
            Some(_) => Err(crate::AgentError::Protocol(
                "Client does not support terminal capability. Set client_capabilities.terminal = true during initialization.".to_string(),
            )),
            None => Err(crate::AgentError::Protocol(
                "No client capabilities available. Client must send initialize request with capabilities.".to_string(),
            )),
        }
    }

    /// Generate ACP-compliant terminal ID with "term_" prefix
    fn generate_terminal_id(&self) -> String {
        format!("term_{}", ulid::Ulid::new())
    }

    /// Create a new terminal session (skips capability check - caller must validate)
    pub(crate) async fn create_terminal_unchecked(&self, working_dir: Option<String>) -> crate::Result<String> {
        // Check rate limit (cost 1 - terminal creation is a standard operation)
        // Use a default client_id for non-session-based creates
        self.rate_limiter
            .check_rate_limit("default", "terminal_create", 1)
            .map_err(|e| crate::AgentError::ToolExecution(e.to_string()))?;

        let terminal_id = self.generate_terminal_id();
        let working_dir = working_dir
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
            });

        // Store terminal session
        let terminal_session = TerminalSession {
            process: None,
            working_dir,
            environment: std::env::vars().collect(),
            command: None,
            args: Vec::new(),
            session_id: None,
            output_byte_limit: 1_048_576, // 1MB default
            output_buffer: Arc::new(RwLock::new(Vec::new())),
            buffer_truncated: Arc::new(RwLock::new(false)),
            exit_status: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(TerminalState::Created)),
            output_task: None,
            timeout_config: TimeoutConfig::default(),
        };
        self.terminals
            .write()
            .await
            .insert(terminal_id.clone(), terminal_session);

        Ok(terminal_id)
    }

    /// Create a new terminal session
    pub async fn create_terminal(&self, working_dir: Option<String>) -> crate::Result<String> {
        // Check terminal capability
        self.validate_terminal_capability().await?;

        self.create_terminal_unchecked(working_dir).await
    }

    /// Create ACP-compliant terminal session with command and all parameters
    ///
    /// This method creates a new terminal session following the Anthropic Computer Protocol
    /// specification. It validates the session ID, resolves the working directory,
    /// prepares environment variables, and creates the terminal with proper output buffering.
    ///
    /// # Arguments
    /// * `session_manager` - Manager for session validation and retrieval
    /// * `params` - Terminal creation parameters including command, args, env, etc.
    ///
    /// # Returns
    /// * `Ok(String)` - The unique terminal ID (ULID format) on success
    /// * `Err(AgentError)` - Protocol error for invalid parameters or session issues
    pub async fn create_terminal_with_command(
        &self,
        session_manager: &crate::session::SessionManager,
        params: TerminalCreateParams,
    ) -> crate::Result<String> {
        // 0. Check terminal capability
        self.validate_terminal_capability().await?;

        // 1. Check rate limit (cost 1 - terminal creation is a standard operation)
        self.rate_limiter
            .check_rate_limit(&params.session_id, "terminal_create", 1)
            .map_err(|e| crate::AgentError::ToolExecution(e.to_string()))?;

        // 2. Validate session ID
        self.validate_session_id(session_manager, &params.session_id)
            .await?;

        // 3. Generate ACP-compliant terminal ID
        let terminal_id = self.generate_terminal_id();

        // 4. Resolve working directory (use session cwd if not specified)
        let working_dir = self
            .resolve_working_directory(session_manager, &params.session_id, params.cwd.as_deref())
            .await?;

        // 5. Prepare environment variables
        let environment = self.prepare_environment(params.env.unwrap_or_default())?;

        // 6. Create enhanced terminal session
        let session = TerminalSession {
            process: None,
            working_dir,
            environment,
            command: Some(params.command),
            args: params.args.unwrap_or_default(),
            session_id: Some(params.session_id),
            output_byte_limit: params.output_byte_limit.unwrap_or(1_048_576), // 1MB default
            output_buffer: Arc::new(RwLock::new(Vec::new())),
            buffer_truncated: Arc::new(RwLock::new(false)),
            exit_status: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(TerminalState::Created)),
            output_task: None,
            timeout_config: TimeoutConfig::default(),
        };

        // 7. Register terminal
        let mut terminals = self.terminals.write().await;
        terminals.insert(terminal_id.clone(), session);

        tracing::info!("Created ACP terminal session: {}", terminal_id);
        Ok(terminal_id)
    }

    /// Validate session ID exists and is properly formatted
    async fn validate_session_id(
        &self,
        session_manager: &crate::session::SessionManager,
        session_id: &str,
    ) -> crate::Result<()> {
        let parsed_session_id = crate::session::SessionId::parse(session_id).map_err(|e| {
            crate::AgentError::Protocol(format!("Invalid session ID format: {}", e))
        })?;

        session_manager
            .get_session(&parsed_session_id)?
            .ok_or_else(|| {
                crate::AgentError::Protocol(format!("Session not found: {}", session_id))
            })?;

        Ok(())
    }

    /// Resolve working directory from session or parameter
    pub async fn resolve_working_directory(
        &self,
        session_manager: &crate::session::SessionManager,
        session_id: &str,
        cwd_param: Option<&str>,
    ) -> crate::Result<std::path::PathBuf> {
        if let Some(cwd) = cwd_param {
            // Use provided working directory, validate it's absolute
            let path = std::path::PathBuf::from(cwd);
            if !path.is_absolute() {
                return Err(crate::AgentError::Protocol(format!(
                    "Working directory must be absolute path: {}",
                    cwd
                )));
            }
            Ok(path)
        } else {
            // Use session's working directory
            let parsed_session_id = crate::session::SessionId::parse(session_id).map_err(|e| {
                crate::AgentError::Protocol(format!("Invalid session ID format: {}", e))
            })?;

            let session = session_manager
                .get_session(&parsed_session_id)?
                .ok_or_else(|| {
                    crate::AgentError::Protocol(format!("Session not found: {}", session_id))
                })?;

            Ok(session.cwd)
        }
    }

    /// Prepare environment variables by merging custom with system environment
    pub fn prepare_environment(
        &self,
        env_vars: Vec<EnvVariable>,
    ) -> crate::Result<HashMap<String, String>> {
        let mut environment: HashMap<String, String> = std::env::vars().collect();

        // Apply custom environment variables, overriding system ones
        for env_var in env_vars {
            if env_var.name.is_empty() {
                return Err(crate::AgentError::Protocol(
                    "Environment variable name cannot be empty".to_string(),
                ));
            }
            environment.insert(env_var.name, env_var.value);
        }

        Ok(environment)
    }

    /// Execute a command in the specified terminal session
    pub async fn execute_command(&self, terminal_id: &str, command: &str) -> crate::Result<String> {
        // Check terminal capability
        self.validate_terminal_capability().await?;

        // Check rate limit (cost 2 - command execution is slightly more expensive)
        self.rate_limiter
            .check_rate_limit(terminal_id, "terminal_execute", 2)
            .map_err(|e| crate::AgentError::ToolExecution(e.to_string()))?;

        let mut terminals = self.terminals.write().await;
        let session = terminals.get_mut(terminal_id).ok_or_else(|| {
            crate::AgentError::ToolExecution(format!("Terminal {} not found", terminal_id))
        })?;

        tracing::info!("Executing command in terminal {}: {}", terminal_id, command);

        // Parse command and arguments
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(crate::AgentError::ToolExecution(
                "Empty command".to_string(),
            ));
        }

        let program = parts[0];
        let args = &parts[1..];

        // Transition to Running state
        *session.state.write().await = TerminalState::Running;

        // Execute command
        let output = Command::new(program)
            .args(args)
            .current_dir(&session.working_dir)
            .envs(&session.environment)
            .output()
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!("Failed to execute command: {}", e))
            })?;

        // Transition to Finished state and set exit status
        let exit_status = ExitStatus {
            exit_code: output.status.code(),
            signal: None,
        };
        session.set_exit_status(exit_status).await;
        *session.state.write().await = TerminalState::Finished;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let result = if output.status.success() {
            if stdout.is_empty() {
                "Command completed successfully (exit code: 0)".to_string()
            } else {
                format!("Command output:\n{}", stdout)
            }
        } else {
            let exit_code = output.status.code().unwrap_or(-1);
            if stderr.is_empty() {
                format!("Command failed (exit code: {})", exit_code)
            } else {
                format!("Command failed (exit code: {}):\n{}", exit_code, stderr)
            }
        };

        tracing::info!(
            "Command completed with exit code: {:?}",
            output.status.code()
        );
        Ok(result)
    }

    /// Change the working directory for a terminal session
    pub async fn change_directory(&self, terminal_id: &str, path: &str) -> crate::Result<String> {
        let mut terminals = self.terminals.write().await;
        let session = terminals.get_mut(terminal_id).ok_or_else(|| {
            crate::AgentError::ToolExecution(format!("Terminal {} not found", terminal_id))
        })?;

        let new_path = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            session.working_dir.join(path)
        };

        if new_path.exists() && new_path.is_dir() {
            session.working_dir = new_path.canonicalize().map_err(|e| {
                crate::AgentError::ToolExecution(format!("Failed to resolve path: {}", e))
            })?;

            tracing::info!("Changed directory to: {}", session.working_dir.display());
            Ok(format!(
                "Changed directory to: {}",
                session.working_dir.display()
            ))
        } else {
            Err(crate::AgentError::ToolExecution(format!(
                "Directory does not exist: {}",
                path
            )))
        }
    }

    /// Release a terminal session (ACP terminal/release method)
    ///
    /// This method implements the ACP terminal/release specification:
    /// 1. Kill running process if still active
    /// 2. Clean up process handles and output tasks
    /// 3. Keep terminal in storage for output/status queries
    /// 4. Mark terminal as Released to prevent further operations
    /// 5. Return null result on successful release
    ///
    /// Note: Terminal remains queryable after release for output and status.
    /// This allows clients to retrieve final output and exit status even after
    /// releasing the terminal resources.
    pub async fn release_terminal(
        &self,
        session_manager: &crate::session::SessionManager,
        params: TerminalReleaseParams,
    ) -> crate::Result<serde_json::Value> {
        // 0. Check terminal capability
        self.validate_terminal_capability().await?;

        // 1. Check rate limit (cost 1 - releasing is a standard operation)
        self.rate_limiter
            .check_rate_limit(&params.session_id, "terminal_release", 1)
            .map_err(|e| crate::AgentError::ToolExecution(e.to_string()))?;

        // 2. Validate session ID
        self.validate_session_id(session_manager, &params.session_id)
            .await?;

        // 3. Get terminal from registry (keep it in storage)
        let terminals = self.terminals.read().await;
        let session = terminals.get(&params.terminal_id).ok_or_else(|| {
            crate::AgentError::Protocol(format!("Terminal not found: {}", params.terminal_id))
        })?;

        // 4. Release terminal resources (but keep output/status)
        session.release().await?;

        tracing::info!("Released terminal session: {}", params.terminal_id);

        // 5. Return null result per ACP specification
        Ok(serde_json::Value::Null)
    }

    /// Get a terminal session by ID for operations that require non-released terminal
    ///
    /// This method retrieves a terminal session by its ID, performing all necessary
    /// validation checks including session ID validation and terminal release status.
    ///
    /// # Arguments
    ///
    /// * `session_manager` - Manager for session validation
    /// * `session_id` - The session ID that owns the terminal
    /// * `terminal_id` - The terminal ID to retrieve
    ///
    /// # Returns
    ///
    /// Returns a read guard to the terminals HashMap and validates the terminal exists
    /// and is not released. The caller must hold the read lock for as long as they
    /// need to access the terminal.
    ///
    /// # Errors
    ///
    /// * `AgentError::Protocol` - Invalid session ID, session not found, terminal not found, or terminal released
    async fn get_terminal<'a>(
        &'a self,
        session_manager: &crate::session::SessionManager,
        session_id: &str,
        terminal_id: &str,
    ) -> crate::Result<tokio::sync::RwLockReadGuard<'a, HashMap<String, TerminalSession>>> {
        // 1. Validate session ID
        self.validate_session_id(session_manager, session_id)
            .await?;

        // 2. Get terminal session
        let terminals = self.terminals.read().await;

        // 3. Validate terminal exists
        let session = terminals.get(terminal_id).ok_or_else(|| {
            crate::AgentError::Protocol(format!("Terminal not found: {}", terminal_id))
        })?;

        // 4. Validate terminal is not released
        session.validate_not_released().await?;

        Ok(terminals)
    }

    /// Get a terminal session by ID for read-only operations (allows released terminals)
    ///
    /// This method retrieves a terminal session for read-only operations like getting
    /// output or status. Unlike `get_terminal`, this method allows access to released
    /// terminals since output and status remain available after release.
    ///
    /// # Arguments
    ///
    /// * `session_manager` - Manager for session validation
    /// * `session_id` - The session ID that owns the terminal
    /// * `terminal_id` - The terminal ID to retrieve
    ///
    /// # Returns
    ///
    /// Returns a read guard to the terminals HashMap. Does not validate release status.
    ///
    /// # Errors
    ///
    /// * `AgentError::Protocol` - Invalid session ID, session not found, or terminal not found
    async fn get_terminal_for_query<'a>(
        &'a self,
        session_manager: &crate::session::SessionManager,
        session_id: &str,
        terminal_id: &str,
    ) -> crate::Result<tokio::sync::RwLockReadGuard<'a, HashMap<String, TerminalSession>>> {
        // 1. Validate session ID
        self.validate_session_id(session_manager, session_id)
            .await?;

        // 2. Get terminal session
        let terminals = self.terminals.read().await;

        // 3. Validate terminal exists (but don't check release status)
        terminals.get(terminal_id).ok_or_else(|| {
            crate::AgentError::Protocol(format!("Terminal not found: {}", terminal_id))
        })?;

        Ok(terminals)
    }

    /// Get output from a terminal session (ACP terminal/output method)
    ///
    /// This method allows querying output even from released terminals, since
    /// output buffers and exit status are preserved after release.
    pub async fn get_output(
        &self,
        session_manager: &crate::session::SessionManager,
        params: TerminalOutputParams,
    ) -> crate::Result<TerminalOutputResponse> {
        // Check terminal capability
        self.validate_terminal_capability().await?;

        // Check rate limit (cost 1 - getting output is a standard read operation)
        self.rate_limiter
            .check_rate_limit(&params.session_id, "terminal_output", 1)
            .map_err(|e| crate::AgentError::ToolExecution(e.to_string()))?;

        // Get terminal (allows released terminals)
        let terminals = self
            .get_terminal_for_query(session_manager, &params.session_id, &params.terminal_id)
            .await?;
        let session = terminals.get(&params.terminal_id).unwrap(); // Safe: validated by get_terminal_for_query

        // Get output data
        let output = session.get_output_string().await;
        let truncated = session.is_output_truncated().await;
        let exit_status = session.get_exit_status().await;

        tracing::debug!(
            "Retrieved output for terminal {}: {} bytes, truncated: {}, exit_status: {:?}",
            params.terminal_id,
            output.len(),
            truncated,
            exit_status
        );

        Ok(TerminalOutputResponse {
            output,
            truncated,
            exit_status,
        })
    }

    /// Wait for terminal process to exit (ACP terminal/wait_for_exit method)
    pub async fn wait_for_exit(
        &self,
        session_manager: &crate::session::SessionManager,
        params: TerminalOutputParams,
    ) -> crate::Result<ExitStatus> {
        // Check terminal capability
        self.validate_terminal_capability().await?;

        // Check rate limit (cost 1 - waiting is a standard operation)
        self.rate_limiter
            .check_rate_limit(&params.session_id, "terminal_wait", 1)
            .map_err(|e| crate::AgentError::ToolExecution(e.to_string()))?;

        // Get and validate terminal
        let terminals = self
            .get_terminal(session_manager, &params.session_id, &params.terminal_id)
            .await?;
        let session = terminals.get(&params.terminal_id).unwrap(); // Safe: validated by get_terminal

        // 3. Wait for exit
        let exit_status = session.wait_for_exit().await?;

        tracing::info!(
            "Terminal {} exited with status: {:?}",
            params.terminal_id,
            exit_status
        );

        Ok(exit_status)
    }

    /// Kill a terminal process (ACP terminal/kill method)
    pub async fn kill_terminal(
        &self,
        session_manager: &crate::session::SessionManager,
        params: TerminalOutputParams,
    ) -> crate::Result<()> {
        // Check terminal capability
        self.validate_terminal_capability().await?;

        // Check rate limit (cost 1 - killing is a standard operation)
        self.rate_limiter
            .check_rate_limit(&params.session_id, "terminal_kill", 1)
            .map_err(|e| crate::AgentError::ToolExecution(e.to_string()))?;

        // Get and validate terminal
        let terminals = self
            .get_terminal(session_manager, &params.session_id, &params.terminal_id)
            .await?;
        let session = terminals.get(&params.terminal_id).unwrap(); // Safe: validated by get_terminal

        // 3. Kill process
        session.kill_process().await?;

        tracing::info!("Terminal {} killed", params.terminal_id);

        Ok(())
    }

    /// Clean up all terminals associated with a session
    ///
    /// This method is called when a session is being closed/removed to ensure
    /// all associated terminal processes are properly cleaned up. It:
    /// 1. Finds all terminals belonging to the session
    /// 2. Releases each terminal (kills process, cleans up resources)
    /// 3. Removes terminals from storage
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID whose terminals should be cleaned up
    ///
    /// # Returns
    ///
    /// Returns the number of terminals that were cleaned up
    pub async fn cleanup_session_terminals(&self, session_id: &str) -> crate::Result<usize> {
        let mut terminals_to_cleanup = Vec::new();

        // Find all terminals belonging to this session
        {
            let terminals = self.terminals.read().await;
            for (terminal_id, terminal) in terminals.iter() {
                if let Some(ref term_session_id) = terminal.session_id {
                    if term_session_id == session_id {
                        terminals_to_cleanup.push(terminal_id.clone());
                    }
                }
            }
        }

        let cleanup_count = terminals_to_cleanup.len();

        // Release and remove each terminal
        for terminal_id in terminals_to_cleanup {
            tracing::debug!(
                "Cleaning up terminal {} for session {}",
                terminal_id,
                session_id
            );

            // Release terminal resources (kills process if running)
            {
                let terminals = self.terminals.read().await;
                if let Some(terminal) = terminals.get(&terminal_id) {
                    if let Err(e) = terminal.release().await {
                        tracing::warn!(
                            "Failed to release terminal {} during session cleanup: {}",
                            terminal_id,
                            e
                        );
                    }
                }
            }

            // Remove terminal from storage
            {
                let mut terminals = self.terminals.write().await;
                terminals.remove(&terminal_id);
            }
        }

        if cleanup_count > 0 {
            tracing::info!(
                "Cleaned up {} terminal(s) for session {}",
                cleanup_count,
                session_id
            );
        }

        Ok(cleanup_count)
    }
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalSession {
    /// Add output data to the buffer, enforcing byte limits with character-boundary truncation
    pub async fn add_output(&self, data: &[u8]) {
        let mut buffer = self.output_buffer.write().await;
        let mut truncated = self.buffer_truncated.write().await;

        // Always append the new data first
        buffer.extend_from_slice(data);

        // Then truncate from beginning if we exceed the limit
        let limit = self.output_byte_limit as usize;
        if buffer.len() > limit {
            let excess = buffer.len() - limit;

            // Find a safe UTF-8 boundary to truncate at
            let truncate_point = Self::find_utf8_boundary(&buffer, excess);
            buffer.drain(0..truncate_point);
            *truncated = true;
        }
    }

    /// Find the nearest UTF-8 character boundary at or after the given position
    fn find_utf8_boundary(data: &[u8], min_pos: usize) -> usize {
        let mut pos = min_pos;

        // Move forward until we find a valid UTF-8 boundary
        while pos < data.len() {
            // Check if this position starts a valid UTF-8 sequence
            // UTF-8 start bytes: 0xxxxxxx, 110xxxxx, 1110xxxx, 11110xxx
            // Continuation bytes: 10xxxxxx
            let byte = data[pos];

            // If this is not a continuation byte, it's a valid boundary
            if (byte & 0b1100_0000) != 0b1000_0000 {
                return pos;
            }

            pos += 1;
        }

        // If we reached the end, return the data length
        data.len()
    }

    /// Get output as UTF-8 string
    pub async fn get_output_string(&self) -> String {
        let buffer = self.output_buffer.read().await;
        String::from_utf8_lossy(&buffer).to_string()
    }

    /// Check if output buffer has been truncated
    pub async fn is_output_truncated(&self) -> bool {
        *self.buffer_truncated.read().await
    }

    /// Get current buffer size in bytes
    pub async fn get_buffer_size(&self) -> usize {
        self.output_buffer.read().await.len()
    }

    /// Clear the output buffer
    pub async fn clear_output(&self) {
        self.output_buffer.write().await.clear();
        *self.buffer_truncated.write().await = false;
    }

    /// Get the current exit status
    pub async fn get_exit_status(&self) -> Option<ExitStatus> {
        self.exit_status.read().await.clone()
    }

    /// Set the exit status when process completes
    pub async fn set_exit_status(&self, status: ExitStatus) {
        *self.exit_status.write().await = Some(status);
    }

    /// Get current terminal state
    pub async fn get_state(&self) -> TerminalState {
        self.state.read().await.clone()
    }

    /// Check if terminal is in Released state
    pub async fn is_released(&self) -> bool {
        matches!(*self.state.read().await, TerminalState::Released)
    }

    /// Check if terminal is in Finished state
    pub async fn is_finished(&self) -> bool {
        matches!(*self.state.read().await, TerminalState::Finished)
    }

    /// Validate terminal is not released (for operations that require active terminal)
    pub async fn validate_not_released(&self) -> crate::Result<()> {
        if self.is_released().await {
            return Err(crate::AgentError::Protocol(
                "Terminal has been released".to_string(),
            ));
        }
        Ok(())
    }

    /// Wait for process to exit and return exit status
    ///
    /// ACP terminal/wait_for_exit method implementation:
    /// Blocks until the process completes and returns the exit status
    /// Wait for process to exit and return the exit status
    ///
    /// This method blocks until the process completes and returns the exit status
    /// including exit code and signal information. If the process has already finished,
    /// it returns the cached exit status immediately.
    ///
    /// # Returns
    ///
    /// * `Ok(ExitStatus)` - Exit status with code and optional signal name
    ///
    /// # Errors
    ///
    /// * `AgentError::Protocol` - Terminal has been released or no process running
    /// * `AgentError::ToolExecution` - Failed to wait for process completion
    ///
    /// # Behavior
    ///
    /// - Returns cached exit status if process already finished
    /// - Blocks waiting for process completion if still running
    /// - Updates terminal state to Finished after process exits
    /// - Extracts and stores signal information on Unix systems
    ///
    /// # Example Usage
    ///
    /// ```ignore
    /// let status = terminal.wait_for_exit().await?;
    /// if let Some(code) = status.exit_code {
    ///     println!("Process exited with code: {}", code);
    /// }
    /// if let Some(signal) = status.signal {
    ///     println!("Process killed by signal: {}", signal);
    /// }
    /// ```
    pub async fn wait_for_exit(&self) -> crate::Result<ExitStatus> {
        // Validate terminal is not released
        self.validate_not_released().await?;

        // Check if already finished
        if let Some(status) = self.get_exit_status().await {
            return Ok(status);
        }

        // Check if process exists
        let process = self
            .process
            .as_ref()
            .ok_or_else(|| crate::AgentError::Protocol("No process running".to_string()))?;

        // Wait for process to complete
        let status = {
            let mut proc = process.write().await;
            proc.wait().await.map_err(|e| {
                crate::AgentError::ToolExecution(format!("Failed to wait for process: {}", e))
            })?
        };

        // Convert to our ExitStatus
        let exit_status = ExitStatus {
            exit_code: status.code(),
            signal: Self::get_signal_name(&status),
        };

        // Store exit status and update state
        self.set_exit_status(exit_status.clone()).await;
        *self.state.write().await = TerminalState::Finished;

        Ok(exit_status)
    }

    /// Get signal name from process status
    #[cfg(unix)]
    fn get_signal_name(status: &std::process::ExitStatus) -> Option<String> {
        use std::os::unix::process::ExitStatusExt;
        status.signal().map(|sig| match sig {
            1 => "SIGHUP".to_string(),
            2 => "SIGINT".to_string(),
            9 => "SIGKILL".to_string(),
            15 => "SIGTERM".to_string(),
            _ => format!("signal {}", sig),
        })
    }

    #[cfg(not(unix))]
    fn get_signal_name(_status: &std::process::ExitStatus) -> Option<String> {
        None
    }

    /// Kill the running process with signal handling
    ///
    /// ACP terminal/kill method implementation:
    /// 1. Send SIGTERM for graceful shutdown (Unix only)
    /// 2. Wait for graceful_shutdown_timeout
    /// 3. Send SIGKILL if process still running
    pub async fn kill_process(&self) -> crate::Result<()> {
        // Validate terminal is not released
        self.validate_not_released().await?;

        // Check if already finished
        if self.is_finished().await {
            tracing::debug!("Process already finished, skipping kill");
            return Ok(());
        }

        // Check if process exists
        let process = self
            .process
            .as_ref()
            .ok_or_else(|| crate::AgentError::Protocol("No process running".to_string()))?;

        #[cfg(unix)]
        {
            self.kill_process_unix(process).await?;
        }

        #[cfg(not(unix))]
        {
            self.kill_process_windows(process).await?;
        }

        // Update state
        *self.state.write().await = TerminalState::Killed;

        Ok(())
    }

    #[cfg(unix)]
    async fn kill_process_unix(&self, process: &Arc<RwLock<Child>>) -> crate::Result<()> {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        let pid = {
            let proc = process.read().await;
            proc.id().ok_or_else(|| {
                crate::AgentError::Protocol("Process ID not available".to_string())
            })?
        };

        let pid = Pid::from_raw(pid as i32);

        // Send SIGTERM for graceful shutdown
        tracing::debug!("Sending SIGTERM to process {}", pid);
        kill(pid, Signal::SIGTERM).map_err(|e| {
            crate::AgentError::ToolExecution(format!("Failed to send SIGTERM: {}", e))
        })?;

        // Wait for graceful shutdown with timeout
        let graceful_timeout = self.timeout_config.graceful_shutdown_timeout.as_duration();
        let wait_result = tokio::time::timeout(graceful_timeout, async {
            let mut proc = process.write().await;
            proc.wait().await
        })
        .await;

        match wait_result {
            Ok(Ok(status)) => {
                tracing::debug!("Process terminated gracefully with status: {:?}", status);
                let exit_status = ExitStatus {
                    exit_code: status.code(),
                    signal: Self::get_signal_name(&status),
                };
                self.set_exit_status(exit_status).await;
                Ok(())
            }
            Ok(Err(e)) => Err(crate::AgentError::ToolExecution(format!(
                "Failed to wait for process: {}",
                e
            ))),
            Err(_) => {
                // Timeout - force kill with SIGKILL
                tracing::debug!(
                    "Graceful shutdown timed out, sending SIGKILL to process {}",
                    pid
                );
                kill(pid, Signal::SIGKILL).map_err(|e| {
                    crate::AgentError::ToolExecution(format!("Failed to send SIGKILL: {}", e))
                })?;

                // Wait for forceful kill
                let mut proc = process.write().await;
                let status = proc.wait().await.map_err(|e| {
                    crate::AgentError::ToolExecution(format!("Failed to wait after SIGKILL: {}", e))
                })?;

                let exit_status = ExitStatus {
                    exit_code: status.code(),
                    signal: Some("SIGKILL".to_string()),
                };
                self.set_exit_status(exit_status).await;
                Ok(())
            }
        }
    }

    #[cfg(not(unix))]
    async fn kill_process_windows(&self, process: &Arc<RwLock<Child>>) -> crate::Result<()> {
        // Windows doesn't have signals - use TerminateProcess directly
        let mut proc = process.write().await;
        proc.kill().await.map_err(|e| {
            crate::AgentError::ToolExecution(format!("Failed to kill process: {}", e))
        })?;

        let status = proc.wait().await.map_err(|e| {
            crate::AgentError::ToolExecution(format!("Failed to wait for process: {}", e))
        })?;

        let exit_status = ExitStatus {
            exit_code: status.code(),
            signal: None,
        };
        self.set_exit_status(exit_status).await;
        Ok(())
    }

    /// Release terminal resources
    ///
    /// ACP terminal/release method implementation:
    /// 1. Kill running process if still active
    /// 2. Clean up process handles and output tasks
    /// 3. Keep output buffers and exit status for queries
    /// 4. Mark terminal as Released to prevent further operations
    ///
    /// Note: Output buffers and exit status are preserved to allow
    /// clients to query final output and status after release.
    pub async fn release(&self) -> crate::Result<()> {
        // Kill process if still running
        if let Some(process) = self.process.as_ref() {
            let mut proc = process.write().await;
            let _ = proc.kill().await;
            tracing::debug!("Killed process during terminal release");
        }

        // Abort output task if running
        if let Some(task) = self.output_task.as_ref() {
            task.abort();
        }

        // Mark as released (but keep output buffers and exit status)
        *self.state.write().await = TerminalState::Released;

        tracing::debug!("Terminal resources released (output/status preserved)");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_session_manager() -> crate::session::SessionManager {
        crate::session::SessionManager::new()
    }

    async fn create_terminal_for_testing(
        manager: &TerminalManager,
        session_manager: &crate::session::SessionManager,
    ) -> crate::Result<(String, String)> {
        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        let session_id = session_manager.create_session(cwd, None)?;
        let session_id_str = session_id.to_string();

        let params = TerminalCreateParams {
            session_id: session_id_str.clone(),
            command: "echo".to_string(),
            args: Some(vec!["test".to_string()]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = manager
            .create_terminal_with_command(session_manager, params)
            .await?;

        Ok((session_id_str, terminal_id))
    }

    #[tokio::test]
    async fn test_get_terminal_success() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        // Test get_terminal returns valid terminal
        let terminals = manager
            .get_terminal(&session_manager, &session_id, &terminal_id)
            .await
            .unwrap();

        let session = terminals.get(&terminal_id).unwrap();
        assert_eq!(session.get_state().await, TerminalState::Created);

        // Clean up
        drop(terminals);
        let params = TerminalReleaseParams {
            session_id,
            terminal_id,
        };
        let _ = manager.release_terminal(&session_manager, params).await;
    }

    #[tokio::test]
    async fn test_get_terminal_invalid_session() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let result = manager
            .get_terminal(&session_manager, "01K6DB0000000000000000000", "term_test")
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Session not found"));
    }

    #[tokio::test]
    async fn test_get_terminal_not_found() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        let session_id = session_manager.create_session(cwd, None).unwrap();

        let result = manager
            .get_terminal(
                &session_manager,
                &session_id.to_string(),
                "term_nonexistent",
            )
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Terminal not found"));
    }

    #[tokio::test]
    async fn test_get_terminal_released() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        // Release the terminal
        let release_params = TerminalReleaseParams {
            session_id: session_id.clone(),
            terminal_id: terminal_id.clone(),
        };
        manager
            .release_terminal(&session_manager, release_params)
            .await
            .unwrap();

        // Try to get the released terminal for operations (should fail)
        let result = manager
            .get_terminal(&session_manager, &session_id, &terminal_id)
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Terminal has been released"));
    }

    #[tokio::test]
    async fn test_terminal_state_lifecycle() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        {
            let terminals = manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            let state = session.get_state().await;

            assert_eq!(state, TerminalState::Created);
            assert!(!session.is_released().await);
        }

        // Clean up: release the terminal to avoid resource leak
        let params = TerminalReleaseParams {
            session_id,
            terminal_id,
        };
        let _ = manager.release_terminal(&session_manager, params).await;
    }

    #[tokio::test]
    async fn test_release_terminal_success() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        let params = TerminalReleaseParams {
            session_id,
            terminal_id: terminal_id.clone(),
        };

        let result = manager.release_terminal(&session_manager, params).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::Value::Null);

        // Terminal should still exist in storage but be marked as released
        let terminals = manager.terminals.read().await;
        let session = terminals
            .get(&terminal_id)
            .expect("Terminal should remain in storage after release");
        assert!(
            session.is_released().await,
            "Terminal should be marked as released"
        );
    }

    #[tokio::test]
    async fn test_release_terminal_not_found() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        let session_id = session_manager.create_session(cwd, None).unwrap();

        let params = TerminalReleaseParams {
            session_id: session_id.to_string(),
            terminal_id: "term_nonexistent".to_string(),
        };

        let result = manager.release_terminal(&session_manager, params).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Terminal not found"));
    }

    #[tokio::test]
    async fn test_release_terminal_invalid_session() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let params = TerminalReleaseParams {
            session_id: "01K6DB0000000000000000000".to_string(),
            terminal_id: "term_test".to_string(),
        };

        let result = manager.release_terminal(&session_manager, params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_terminal_session_release_preserves_buffers() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (_session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        {
            let terminals = manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            session.add_output(b"test output").await;
            let buffer_size = session.get_buffer_size().await;
            assert!(buffer_size > 0);
        }

        {
            let terminals = manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            session.release().await.unwrap();
            let buffer_size = session.get_buffer_size().await;
            assert!(
                buffer_size > 0,
                "Output buffer should be preserved after release"
            );
            assert!(session.is_released().await);
        }
    }

    #[tokio::test]
    async fn test_validate_not_released() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (_session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        {
            let terminals = manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            assert!(session.validate_not_released().await.is_ok());
        }

        {
            let terminals = manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            session.release().await.unwrap();
            let result = session.validate_not_released().await;
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Terminal has been released"));
        }
    }

    #[tokio::test]
    async fn test_get_output_on_released_terminal() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        // Add some output before releasing
        {
            let terminals = manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            session.add_output(b"test output before release").await;
        }

        let release_params = TerminalReleaseParams {
            session_id: session_id.clone(),
            terminal_id: terminal_id.clone(),
        };

        manager
            .release_terminal(&session_manager, release_params)
            .await
            .unwrap();

        // Should still be able to get output after release
        let output_params = TerminalOutputParams {
            session_id,
            terminal_id,
        };

        let result = manager.get_output(&session_manager, output_params).await;
        assert!(
            result.is_ok(),
            "Should be able to get output from released terminal"
        );
        let response = result.unwrap();
        assert_eq!(response.output, "test output before release");
    }

    #[tokio::test]
    async fn test_terminal_state_transitions() {
        let session = TerminalSession {
            process: None,
            working_dir: std::path::PathBuf::from("/tmp"),
            environment: HashMap::new(),
            command: Some("echo".to_string()),
            args: vec!["test".to_string()],
            session_id: Some("test".to_string()),
            output_byte_limit: 1024,
            output_buffer: Arc::new(RwLock::new(Vec::new())),
            buffer_truncated: Arc::new(RwLock::new(false)),
            exit_status: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(TerminalState::Created)),
            output_task: None,
            timeout_config: TimeoutConfig::default(),
        };

        assert_eq!(session.get_state().await, TerminalState::Created);
        assert!(!session.is_released().await);
        assert!(!session.is_finished().await);

        *session.state.write().await = TerminalState::Running;
        assert_eq!(session.get_state().await, TerminalState::Running);

        *session.state.write().await = TerminalState::Finished;
        assert!(session.is_finished().await);

        *session.state.write().await = TerminalState::Killed;
        assert_eq!(session.get_state().await, TerminalState::Killed);

        *session.state.write().await = TerminalState::Released;
        assert!(session.is_released().await);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_signal_handling_graceful_termination() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        let session_id = session_manager.create_session(cwd, None).unwrap();

        let params = TerminalCreateParams {
            session_id: session_id.to_string(),
            command: "sleep".to_string(),
            args: Some(vec!["30".to_string()]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = manager
            .create_terminal_with_command(&session_manager, params)
            .await
            .unwrap();

        // Start the process
        {
            let mut terminals = manager.terminals.write().await;
            let session = terminals.get_mut(&terminal_id).unwrap();

            let mut cmd = Command::new("sleep");
            cmd.arg("30")
                .current_dir(&session.working_dir)
                .envs(&session.environment)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            let child = cmd.spawn().unwrap();
            session.process = Some(Arc::new(RwLock::new(child)));
            *session.state.write().await = TerminalState::Running;
        }

        // Kill the process
        let kill_params = TerminalOutputParams {
            session_id: session_id.to_string(),
            terminal_id: terminal_id.clone(),
        };

        let result = manager.kill_terminal(&session_manager, kill_params).await;
        result.expect("kill_terminal should succeed");

        // Verify terminal state is killed
        let terminals = manager.terminals.read().await;
        let session = terminals.get(&terminal_id).unwrap();
        let state = session.get_state().await;
        assert_eq!(state, TerminalState::Killed);

        // Verify exit status has signal information
        let exit_status = session.get_exit_status().await;
        assert!(exit_status.is_some());
        let status = exit_status.unwrap();
        assert!(status.signal.is_some());
    }

    #[tokio::test]
    async fn test_wait_for_exit_already_finished() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        // Manually set exit status to simulate finished process
        {
            let terminals = manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            let status = ExitStatus {
                exit_code: Some(0),
                signal: None,
            };
            session.set_exit_status(status).await;
            *session.state.write().await = TerminalState::Finished;
        }

        // Wait for exit should return immediately with cached status
        let params = TerminalOutputParams {
            session_id,
            terminal_id,
        };

        let result = manager.wait_for_exit(&session_manager, params).await;
        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.exit_code, Some(0));
        assert_eq!(status.signal, None);
    }

    #[tokio::test]
    async fn test_kill_already_finished_process() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        // Manually set to finished state and test kill_process directly
        {
            let terminals = manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            *session.state.write().await = TerminalState::Finished;

            // Test session-level kill (should succeed without process)
            let result = session.kill_process().await;
            assert!(
                result.is_ok(),
                "Session-level kill failed: {:?}",
                result.err()
            );
        }

        // Also test manager-level kill
        let params = TerminalOutputParams {
            session_id,
            terminal_id: terminal_id.clone(),
        };

        let result = manager.kill_terminal(&session_manager, params).await;
        match result {
            Ok(_) => {}
            Err(e) => panic!("Manager-level kill failed: {}", e),
        }
    }

    #[tokio::test]
    async fn test_wait_for_exit_with_running_process() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        let session_id = session_manager.create_session(cwd, None).unwrap();

        let params = TerminalCreateParams {
            session_id: session_id.to_string(),
            command: "echo".to_string(),
            args: Some(vec!["hello".to_string()]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = manager
            .create_terminal_with_command(&session_manager, params)
            .await
            .unwrap();

        // Start the process
        {
            let mut terminals = manager.terminals.write().await;
            let session = terminals.get_mut(&terminal_id).unwrap();

            let mut cmd = Command::new("echo");
            cmd.arg("hello")
                .current_dir(&session.working_dir)
                .envs(&session.environment)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            let child = cmd.spawn().unwrap();
            session.process = Some(Arc::new(RwLock::new(child)));
            *session.state.write().await = TerminalState::Running;
        }

        // Wait for exit
        let wait_params = TerminalOutputParams {
            session_id: session_id.to_string(),
            terminal_id: terminal_id.clone(),
        };

        let result = manager.wait_for_exit(&session_manager, wait_params).await;
        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.exit_code, Some(0));
        assert_eq!(status.signal, None);

        // Verify terminal state is finished
        let terminals = manager.terminals.read().await;
        let session = terminals.get(&terminal_id).unwrap();
        let state = session.get_state().await;
        assert_eq!(state, TerminalState::Finished);
    }

    #[tokio::test]
    async fn test_concurrent_wait_for_exit_calls() {
        let manager = Arc::new(TerminalManager::new());
        let session_manager = Arc::new(create_test_session_manager().await);

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        let session_id = session_manager.create_session(cwd, None).unwrap();

        let params = TerminalCreateParams {
            session_id: session_id.to_string(),
            command: "sleep".to_string(),
            args: Some(vec!["1".to_string()]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = manager
            .create_terminal_with_command(&session_manager, params)
            .await
            .unwrap();

        // Start the process
        {
            let mut terminals = manager.terminals.write().await;
            let session = terminals.get_mut(&terminal_id).unwrap();

            let mut cmd = Command::new("sleep");
            cmd.arg("1")
                .current_dir(&session.working_dir)
                .envs(&session.environment)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            let child = cmd.spawn().unwrap();
            session.process = Some(Arc::new(RwLock::new(child)));
            *session.state.write().await = TerminalState::Running;
        }

        // Create two concurrent wait calls
        let wait_params1 = TerminalOutputParams {
            session_id: session_id.to_string(),
            terminal_id: terminal_id.clone(),
        };
        let wait_params2 = TerminalOutputParams {
            session_id: session_id.to_string(),
            terminal_id: terminal_id.clone(),
        };

        let manager_clone = manager.clone();
        let session_manager_clone = session_manager.clone();

        let wait1 =
            tokio::spawn(
                async move { manager.wait_for_exit(&session_manager, wait_params1).await },
            );

        let wait2 = tokio::spawn(async move {
            manager_clone
                .wait_for_exit(&session_manager_clone, wait_params2)
                .await
        });

        // Both should succeed
        let result1 = wait1.await.unwrap();
        let result2 = wait2.await.unwrap();

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let status1 = result1.unwrap();
        let status2 = result2.unwrap();

        assert_eq!(status1.exit_code, Some(0));
        assert_eq!(status2.exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_wait_for_exit_on_released_terminal() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        // Release the terminal
        let release_params = TerminalReleaseParams {
            session_id: session_id.clone(),
            terminal_id: terminal_id.clone(),
        };

        manager
            .release_terminal(&session_manager, release_params)
            .await
            .unwrap();

        // Try to wait for exit on released terminal
        let wait_params = TerminalOutputParams {
            session_id,
            terminal_id,
        };

        let result = manager.wait_for_exit(&session_manager, wait_params).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Terminal has been released"));
    }

    #[tokio::test]
    async fn test_kill_then_wait_for_exit() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        let session_id = session_manager.create_session(cwd, None).unwrap();

        let params = TerminalCreateParams {
            session_id: session_id.to_string(),
            command: "sleep".to_string(),
            args: Some(vec!["30".to_string()]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = manager
            .create_terminal_with_command(&session_manager, params)
            .await
            .unwrap();

        // Start the process
        {
            let mut terminals = manager.terminals.write().await;
            let session = terminals.get_mut(&terminal_id).unwrap();

            let mut cmd = Command::new("sleep");
            cmd.arg("30")
                .current_dir(&session.working_dir)
                .envs(&session.environment)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            let child = cmd.spawn().unwrap();
            session.process = Some(Arc::new(RwLock::new(child)));
            *session.state.write().await = TerminalState::Running;
        }

        // Kill the process
        let kill_params = TerminalOutputParams {
            session_id: session_id.to_string(),
            terminal_id: terminal_id.clone(),
        };

        let kill_result = manager.kill_terminal(&session_manager, kill_params).await;
        assert!(kill_result.is_ok());

        // Wait for exit should return cached exit status
        let wait_params = TerminalOutputParams {
            session_id: session_id.to_string(),
            terminal_id: terminal_id.clone(),
        };

        let wait_result = manager.wait_for_exit(&session_manager, wait_params).await;
        assert!(wait_result.is_ok());
        let status = wait_result.unwrap();

        // Process was killed, so should have exit status set
        assert!(status.exit_code.is_some() || status.signal.is_some());

        // Verify terminal state is killed
        let terminals = manager.terminals.read().await;
        let session = terminals.get(&terminal_id).unwrap();
        let state = session.get_state().await;
        assert_eq!(state, TerminalState::Killed);
    }

    // UTF-8 Processing Tests

    fn create_test_terminal_session() -> TerminalSession {
        TerminalSession {
            process: None,
            working_dir: std::path::PathBuf::from("/tmp"),
            environment: HashMap::new(),
            command: Some("test".to_string()),
            args: vec![],
            session_id: Some("test_session".to_string()),
            output_byte_limit: 100, // Small limit for testing truncation
            output_buffer: Arc::new(RwLock::new(Vec::new())),
            buffer_truncated: Arc::new(RwLock::new(false)),
            exit_status: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(TerminalState::Created)),
            output_task: None,
            timeout_config: TimeoutConfig::default(),
        }
    }

    #[tokio::test]
    async fn test_utf8_1_byte_ascii_characters() {
        let session = create_test_terminal_session();
        let text = "Hello World!";
        session.add_output(text.as_bytes()).await;

        let output = session.get_output_string().await;
        assert_eq!(output, text);
        assert!(!session.is_output_truncated().await);
    }

    #[tokio::test]
    async fn test_utf8_2_byte_characters() {
        let session = create_test_terminal_session();
        // Latin Extended, Cyrillic, etc.
        let text = "Привет мир"; // Russian "Hello World"
        session.add_output(text.as_bytes()).await;

        let output = session.get_output_string().await;
        assert_eq!(output, text);
        assert!(!session.is_output_truncated().await);
    }

    #[tokio::test]
    async fn test_utf8_3_byte_characters() {
        let session = create_test_terminal_session();
        // CJK characters
        let text = "你好世界"; // Chinese "Hello World"
        session.add_output(text.as_bytes()).await;

        let output = session.get_output_string().await;
        assert_eq!(output, text);
        assert!(!session.is_output_truncated().await);
    }

    #[tokio::test]
    async fn test_utf8_4_byte_characters() {
        let session = create_test_terminal_session();
        // Emoji and other 4-byte characters
        let text = "Hello 👋 World 🌍!";
        session.add_output(text.as_bytes()).await;

        let output = session.get_output_string().await;
        assert_eq!(output, text);
        assert!(!session.is_output_truncated().await);
    }

    #[tokio::test]
    async fn test_utf8_mixed_width_characters() {
        let session = create_test_terminal_session();
        // Mix of 1, 2, 3, and 4 byte characters
        let text = "ASCII Привет 你好 👋";
        session.add_output(text.as_bytes()).await;

        let output = session.get_output_string().await;
        assert_eq!(output, text);
        assert!(!session.is_output_truncated().await);
    }

    #[tokio::test]
    async fn test_utf8_truncation_at_ascii_boundary() {
        let session = create_test_terminal_session();
        // Add more than 100 bytes of ASCII text
        let text = "a".repeat(150);
        session.add_output(text.as_bytes()).await;

        let output = session.get_output_string().await;
        assert!(session.is_output_truncated().await);
        assert!(output.len() <= 100);
        // Verify output is valid UTF-8
        assert!(std::str::from_utf8(output.as_bytes()).is_ok());
    }

    #[tokio::test]
    async fn test_utf8_truncation_preserves_multibyte_characters() {
        let session = create_test_terminal_session();
        // Create text with 3-byte characters that will exceed limit
        let text = "你".repeat(40); // Each character is 3 bytes = 120 bytes total
        session.add_output(text.as_bytes()).await;

        let output = session.get_output_string().await;
        assert!(session.is_output_truncated().await);
        // Output should be valid UTF-8 with complete characters only
        assert!(std::str::from_utf8(output.as_bytes()).is_ok());
        // Each character is 3 bytes, so we should have at most 33 complete characters
        // (99 bytes = 33 * 3, staying under 100 byte limit)
        assert!(output.chars().count() <= 34);
    }

    #[tokio::test]
    async fn test_utf8_truncation_at_emoji_boundary() {
        let session = create_test_terminal_session();
        // Create text with 4-byte emoji that will exceed limit
        let text = "👋".repeat(30); // Each emoji is 4 bytes = 120 bytes total
        session.add_output(text.as_bytes()).await;

        let output = session.get_output_string().await;
        assert!(session.is_output_truncated().await);
        // Output should be valid UTF-8 with complete emoji only
        assert!(std::str::from_utf8(output.as_bytes()).is_ok());
        // Each emoji is 4 bytes, so we should have at most 25 complete emoji
        // (100 bytes = 25 * 4)
        assert!(output.chars().count() <= 25);
    }

    #[tokio::test]
    async fn test_utf8_boundary_detection_continuation_byte() {
        let session = create_test_terminal_session();
        // Test truncation happens at a safe boundary, not mid-character
        let text = "a".repeat(98) + "你"; // 98 ASCII + 1 3-byte char = 101 bytes
        session.add_output(text.as_bytes()).await;

        let output = session.get_output_string().await;
        assert!(session.is_output_truncated().await);
        // The 3-byte character should be preserved or removed entirely, not split
        assert!(std::str::from_utf8(output.as_bytes()).is_ok());
    }

    #[tokio::test]
    async fn test_utf8_incremental_output_with_truncation() {
        let session = create_test_terminal_session();
        // Add output in multiple chunks
        session.add_output("Hello ".as_bytes()).await;
        session.add_output("World ".as_bytes()).await;
        session.add_output("你好 ".as_bytes()).await;
        session.add_output("👋".as_bytes()).await;

        let output = session.get_output_string().await;
        assert!(!session.is_output_truncated().await);
        assert!(std::str::from_utf8(output.as_bytes()).is_ok());
        assert!(output.contains("Hello"));
        assert!(output.contains("👋"));
    }

    #[tokio::test]
    async fn test_utf8_incremental_output_exceeding_limit() {
        let session = create_test_terminal_session();
        // Add output that incrementally exceeds the limit
        for _ in 0..10 {
            session.add_output("Hello World! ".as_bytes()).await;
        }

        let output = session.get_output_string().await;
        assert!(session.is_output_truncated().await);
        assert!(output.len() <= 100);
        assert!(std::str::from_utf8(output.as_bytes()).is_ok());
    }

    #[test]
    fn test_find_utf8_boundary_at_ascii() {
        let data = b"Hello World";
        let boundary = TerminalSession::find_utf8_boundary(data, 5);
        assert_eq!(boundary, 5);
    }

    #[test]
    fn test_find_utf8_boundary_at_2_byte_start() {
        // Cyrillic 'П' = 0xD0 0x9F
        let data = b"Hello\xD0\x9F\xD1\x80\xD0\xB8\xD0\xB2\xD0\xB5\xD1\x82";
        let boundary = TerminalSession::find_utf8_boundary(data, 5);
        assert_eq!(boundary, 5); // Should be at start of 'П'
    }

    #[test]
    fn test_find_utf8_boundary_at_continuation_byte() {
        // Cyrillic 'П' = 0xD0 0x9F, test position at continuation byte
        let data = b"Hello\xD0\x9F\xD1\x80";
        let boundary = TerminalSession::find_utf8_boundary(data, 6);
        // Position 6 is the continuation byte of 'П', should move to 7 (next char)
        assert_eq!(boundary, 7);
    }

    #[test]
    fn test_find_utf8_boundary_at_3_byte_character() {
        // Chinese '你' = 0xE4 0xBD 0xA0
        let data = b"Hi\xE4\xBD\xA0";
        let boundary = TerminalSession::find_utf8_boundary(data, 2);
        assert_eq!(boundary, 2); // Should be at start of '你'
    }

    #[test]
    fn test_find_utf8_boundary_mid_3_byte_character() {
        // Chinese '你' = 0xE4 0xBD 0xA0
        let data = b"Hi\xE4\xBD\xA0\xE5\xA5\xBD"; // "Hi你好"
        let boundary = TerminalSession::find_utf8_boundary(data, 3);
        // Position 3 is continuation byte, should move to 5 (next char)
        assert_eq!(boundary, 5);
    }

    #[test]
    fn test_find_utf8_boundary_at_4_byte_emoji() {
        // Waving hand emoji '👋' = 0xF0 0x9F 0x91 0x8B
        let data = b"Hi\xF0\x9F\x91\x8B";
        let boundary = TerminalSession::find_utf8_boundary(data, 2);
        assert_eq!(boundary, 2); // Should be at start of emoji
    }

    #[test]
    fn test_find_utf8_boundary_mid_4_byte_emoji() {
        // Waving hand emoji '👋' = 0xF0 0x9F 0x91 0x8B
        let data = b"Hi\xF0\x9F\x91\x8B!";
        let boundary = TerminalSession::find_utf8_boundary(data, 3);
        // Position 3 is continuation byte, should move to 6 (after emoji)
        assert_eq!(boundary, 6);
    }

    #[test]
    fn test_find_utf8_boundary_at_end_of_data() {
        let data = b"Hello";
        let boundary = TerminalSession::find_utf8_boundary(data, 10);
        assert_eq!(boundary, 5); // Should return data length
    }

    #[tokio::test]
    async fn test_utf8_clear_output_resets_truncation_flag() {
        let session = create_test_terminal_session();
        // Add enough data to trigger truncation
        session.add_output("a".repeat(150).as_bytes()).await;
        assert!(session.is_output_truncated().await);

        // Clear output
        session.clear_output().await;

        // Add new data under limit
        session.add_output(b"Hello").await;
        assert!(!session.is_output_truncated().await);
        assert_eq!(session.get_output_string().await, "Hello");
    }

    #[tokio::test]
    async fn test_utf8_output_with_null_bytes() {
        let session = create_test_terminal_session();
        // Add data with null bytes
        let data = b"Hello\x00World";
        session.add_output(data).await;

        let output = session.get_output_string().await;
        // from_utf8_lossy should handle this
        assert!(!output.is_empty());
    }

    #[tokio::test]
    async fn test_utf8_large_buffer_size_tracking() {
        let session = create_test_terminal_session();
        let text1 = "Hello";
        let text2 = " World";

        session.add_output(text1.as_bytes()).await;
        assert_eq!(session.get_buffer_size().await, text1.len());

        session.add_output(text2.as_bytes()).await;
        assert_eq!(session.get_buffer_size().await, text1.len() + text2.len());
    }

    #[tokio::test]
    async fn test_utf8_truncation_with_boundary_exactly_at_limit() {
        let session = create_test_terminal_session();
        // Create exactly 100 bytes of valid UTF-8
        let text = "a".repeat(100);
        session.add_output(text.as_bytes()).await;

        let output = session.get_output_string().await;
        assert!(!session.is_output_truncated().await);
        assert_eq!(output.len(), 100);
    }

    #[tokio::test]
    async fn test_utf8_truncation_one_byte_over_limit() {
        let session = create_test_terminal_session();
        // Create 101 bytes
        let text = "a".repeat(101);
        session.add_output(text.as_bytes()).await;

        let output = session.get_output_string().await;
        assert!(session.is_output_truncated().await);
        assert!(output.len() <= 100);
    }

    #[tokio::test]
    async fn test_cleanup_session_terminals() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        let session_id = session_manager.create_session(cwd, None).unwrap();
        let session_id_str = session_id.to_string();

        // Create multiple terminals for this session
        let mut terminal_ids = Vec::new();
        for i in 0..3 {
            let params = TerminalCreateParams {
                session_id: session_id_str.clone(),
                command: "echo".to_string(),
                args: Some(vec![format!("test{}", i)]),
                env: None,
                cwd: None,
                output_byte_limit: None,
            };

            let terminal_id = manager
                .create_terminal_with_command(&session_manager, params)
                .await
                .unwrap();
            terminal_ids.push(terminal_id);
        }

        // Verify all terminals exist
        {
            let terminals = manager.terminals.read().await;
            assert_eq!(terminals.len(), 3);
            for terminal_id in &terminal_ids {
                assert!(terminals.contains_key(terminal_id));
            }
        }

        // Clean up session terminals
        let cleanup_count = manager
            .cleanup_session_terminals(&session_id_str)
            .await
            .unwrap();
        assert_eq!(cleanup_count, 3);

        // Verify all terminals were removed
        {
            let terminals = manager.terminals.read().await;
            assert_eq!(terminals.len(), 0);
        }
    }

    #[tokio::test]
    async fn test_cleanup_session_terminals_mixed_sessions() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));

        // Create two sessions
        let session_id1 = session_manager.create_session(cwd.clone(), None).unwrap();
        let session_id1_str = session_id1.to_string();
        let session_id2 = session_manager.create_session(cwd, None).unwrap();
        let session_id2_str = session_id2.to_string();

        // Create terminals for session 1
        let mut session1_terminal_ids = Vec::new();
        for i in 0..2 {
            let params = TerminalCreateParams {
                session_id: session_id1_str.clone(),
                command: "echo".to_string(),
                args: Some(vec![format!("session1-{}", i)]),
                env: None,
                cwd: None,
                output_byte_limit: None,
            };

            let terminal_id = manager
                .create_terminal_with_command(&session_manager, params)
                .await
                .unwrap();
            session1_terminal_ids.push(terminal_id);
        }

        // Create terminals for session 2
        let mut session2_terminal_ids = Vec::new();
        for i in 0..2 {
            let params = TerminalCreateParams {
                session_id: session_id2_str.clone(),
                command: "echo".to_string(),
                args: Some(vec![format!("session2-{}", i)]),
                env: None,
                cwd: None,
                output_byte_limit: None,
            };

            let terminal_id = manager
                .create_terminal_with_command(&session_manager, params)
                .await
                .unwrap();
            session2_terminal_ids.push(terminal_id);
        }

        // Verify all terminals exist
        {
            let terminals = manager.terminals.read().await;
            assert_eq!(terminals.len(), 4);
        }

        // Clean up only session 1 terminals
        let cleanup_count = manager
            .cleanup_session_terminals(&session_id1_str)
            .await
            .unwrap();
        assert_eq!(cleanup_count, 2);

        // Verify only session 1 terminals were removed
        {
            let terminals = manager.terminals.read().await;
            assert_eq!(terminals.len(), 2);

            // Session 1 terminals should be gone
            for terminal_id in &session1_terminal_ids {
                assert!(!terminals.contains_key(terminal_id));
            }

            // Session 2 terminals should still exist
            for terminal_id in &session2_terminal_ids {
                assert!(terminals.contains_key(terminal_id));
            }
        }
    }

    #[tokio::test]
    async fn test_cleanup_session_terminals_no_terminals() {
        let manager = TerminalManager::new();

        // Clean up terminals for a session that has none
        let cleanup_count = manager
            .cleanup_session_terminals("01K6DB0000000000000000000")
            .await
            .unwrap();
        assert_eq!(cleanup_count, 0);
    }

    #[tokio::test]
    async fn test_cleanup_session_terminals_with_running_processes() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        let session_id = session_manager.create_session(cwd, None).unwrap();
        let session_id_str = session_id.to_string();

        // Create a terminal with a long-running process
        let params = TerminalCreateParams {
            session_id: session_id_str.clone(),
            command: "sleep".to_string(),
            args: Some(vec!["30".to_string()]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = manager
            .create_terminal_with_command(&session_manager, params)
            .await
            .unwrap();

        // Start the process
        {
            let mut terminals = manager.terminals.write().await;
            let session = terminals.get_mut(&terminal_id).unwrap();

            let mut cmd = Command::new("sleep");
            cmd.arg("30")
                .current_dir(&session.working_dir)
                .envs(&session.environment)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            let child = cmd.spawn().unwrap();
            session.process = Some(Arc::new(RwLock::new(child)));
            *session.state.write().await = TerminalState::Running;
        }

        // Clean up session terminals (should kill the running process)
        let cleanup_count = manager
            .cleanup_session_terminals(&session_id_str)
            .await
            .unwrap();
        assert_eq!(cleanup_count, 1);

        // Verify terminal was removed
        {
            let terminals = manager.terminals.read().await;
            assert_eq!(terminals.len(), 0);
        }
    }

    // Concurrent Terminal Operations Tests

    #[tokio::test]
    async fn test_concurrent_terminal_creates() {
        let manager = Arc::new(TerminalManager::new());
        let session_manager = Arc::new(create_test_session_manager().await);

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        let session_id = session_manager.create_session(cwd, None).unwrap();
        let session_id_str = session_id.to_string();

        // Create multiple terminals concurrently
        let mut handles = Vec::new();
        for i in 0..10 {
            let manager_clone = manager.clone();
            let session_manager_clone = session_manager.clone();
            let session_id_clone = session_id_str.clone();

            let handle = tokio::spawn(async move {
                let params = TerminalCreateParams {
                    session_id: session_id_clone,
                    command: "echo".to_string(),
                    args: Some(vec![format!("test{}", i)]),
                    env: None,
                    cwd: None,
                    output_byte_limit: None,
                };

                manager_clone
                    .create_terminal_with_command(&session_manager_clone, params)
                    .await
            });

            handles.push(handle);
        }

        // Wait for all creates to complete
        let mut terminal_ids = Vec::new();
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
            terminal_ids.push(result.unwrap());
        }

        // Verify all terminals were created
        assert_eq!(terminal_ids.len(), 10);
        let terminals = manager.terminals.read().await;
        assert_eq!(terminals.len(), 10);

        // Verify all terminal IDs are unique
        let unique_ids: std::collections::HashSet<_> = terminal_ids.iter().collect();
        assert_eq!(unique_ids.len(), 10);
    }

    #[tokio::test]
    async fn test_concurrent_get_output() {
        let manager = Arc::new(TerminalManager::new());
        let session_manager = Arc::new(create_test_session_manager().await);

        let (session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        // Add some output
        {
            let terminals = manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            session.add_output(b"test output").await;
        }

        // Concurrently get output from multiple tasks
        let mut handles = Vec::new();
        for _ in 0..10 {
            let manager_clone = manager.clone();
            let session_manager_clone = session_manager.clone();
            let params = TerminalOutputParams {
                session_id: session_id.clone(),
                terminal_id: terminal_id.clone(),
            };

            let handle = tokio::spawn(async move {
                manager_clone
                    .get_output(&session_manager_clone, params)
                    .await
            });

            handles.push(handle);
        }

        // All reads should succeed with the same output
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
            let response = result.unwrap();
            assert_eq!(response.output, "test output");
        }
    }

    #[tokio::test]
    async fn test_concurrent_output_additions() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (_session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        // Concurrently add output from multiple tasks
        let mut handles = Vec::new();
        for i in 0..10 {
            let terminals = manager.terminals.clone();
            let terminal_id_clone = terminal_id.clone();

            let handle = tokio::spawn(async move {
                let terminals = terminals.read().await;
                let session = terminals.get(&terminal_id_clone).unwrap();
                session.add_output(format!("line{}\n", i).as_bytes()).await;
            });

            handles.push(handle);
        }

        // Wait for all additions to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all output was added
        let terminals = manager.terminals.read().await;
        let session = terminals.get(&terminal_id).unwrap();
        let output = session.get_output_string().await;

        // All 10 lines should be present (in some order due to concurrency)
        for i in 0..10 {
            assert!(
                output.contains(&format!("line{}", i)),
                "Output should contain line{}",
                i
            );
        }
    }

    #[tokio::test]
    async fn test_concurrent_release_and_query() {
        let manager = Arc::new(TerminalManager::new());
        let session_manager = Arc::new(create_test_session_manager().await);

        let (session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        // Add some output before testing
        {
            let terminals = manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            session.add_output(b"test output").await;
        }

        // Spawn concurrent tasks: one releases, others query output
        let mut release_handles = Vec::new();
        let mut query_handles = Vec::new();

        // Release task
        {
            let manager_clone = manager.clone();
            let session_manager_clone = session_manager.clone();
            let params = TerminalReleaseParams {
                session_id: session_id.clone(),
                terminal_id: terminal_id.clone(),
            };

            let handle = tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                manager_clone
                    .release_terminal(&session_manager_clone, params)
                    .await
            });

            release_handles.push(handle);
        }

        // Query tasks
        for _ in 0..5 {
            let manager_clone = manager.clone();
            let session_manager_clone = session_manager.clone();
            let params = TerminalOutputParams {
                session_id: session_id.clone(),
                terminal_id: terminal_id.clone(),
            };

            let handle = tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
                manager_clone
                    .get_output(&session_manager_clone, params)
                    .await
            });

            query_handles.push(handle);
        }

        // Release should succeed
        for handle in release_handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        // All queries should succeed (queries work on released terminals)
        for handle in query_handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_concurrent_kills_on_same_terminal() {
        let manager = Arc::new(TerminalManager::new());
        let session_manager = Arc::new(create_test_session_manager().await);

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        let session_id = session_manager.create_session(cwd, None).unwrap();
        let session_id_str = session_id.to_string();

        let params = TerminalCreateParams {
            session_id: session_id_str.clone(),
            command: "sleep".to_string(),
            args: Some(vec!["30".to_string()]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = manager
            .create_terminal_with_command(&session_manager, params)
            .await
            .unwrap();

        // Start the process
        {
            let mut terminals = manager.terminals.write().await;
            let session = terminals.get_mut(&terminal_id).unwrap();

            let mut cmd = Command::new("sleep");
            cmd.arg("30")
                .current_dir(&session.working_dir)
                .envs(&session.environment)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            let child = cmd.spawn().unwrap();
            session.process = Some(Arc::new(RwLock::new(child)));
            *session.state.write().await = TerminalState::Running;
        }

        // Spawn multiple concurrent kill operations
        let mut handles = Vec::new();
        for _ in 0..3 {
            let manager_clone = manager.clone();
            let session_manager_clone = session_manager.clone();
            let params = TerminalOutputParams {
                session_id: session_id_str.clone(),
                terminal_id: terminal_id.clone(),
            };

            let handle = tokio::spawn(async move {
                manager_clone
                    .kill_terminal(&session_manager_clone, params)
                    .await
            });

            handles.push(handle);
        }

        // At least one should succeed, others should either succeed or handle gracefully
        let mut success_count = 0;
        for handle in handles {
            let result = handle.await.unwrap();
            if result.is_ok() {
                success_count += 1;
            }
        }

        assert!(success_count >= 1, "At least one kill should succeed");

        // Verify terminal is in killed state
        let terminals = manager.terminals.read().await;
        let session = terminals.get(&terminal_id).unwrap();
        let state = session.get_state().await;
        assert_eq!(state, TerminalState::Killed);
    }

    #[tokio::test]
    async fn test_concurrent_operations_on_different_terminals() {
        let manager = Arc::new(TerminalManager::new());
        let session_manager = Arc::new(create_test_session_manager().await);

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        let session_id = session_manager.create_session(cwd, None).unwrap();
        let session_id_str = session_id.to_string();

        // Create multiple terminals
        let mut terminal_ids = Vec::new();
        for i in 0..5 {
            let params = TerminalCreateParams {
                session_id: session_id_str.clone(),
                command: "echo".to_string(),
                args: Some(vec![format!("test{}", i)]),
                env: None,
                cwd: None,
                output_byte_limit: None,
            };

            let terminal_id = manager
                .create_terminal_with_command(&session_manager, params)
                .await
                .unwrap();
            terminal_ids.push(terminal_id);
        }

        // Perform concurrent operations on different terminals
        let mut add_handles = Vec::new();
        let mut get_handles = Vec::new();
        let mut release_handles = Vec::new();

        // Add output to each terminal concurrently
        for (i, terminal_id) in terminal_ids.iter().enumerate() {
            let terminals = manager.terminals.clone();
            let terminal_id_clone = terminal_id.clone();

            let handle = tokio::spawn(async move {
                let terminals = terminals.read().await;
                let session = terminals.get(&terminal_id_clone).unwrap();
                session
                    .add_output(format!("output for terminal {}\n", i).as_bytes())
                    .await;
            });

            add_handles.push(handle);
        }

        // Get output from each terminal concurrently
        for terminal_id in terminal_ids.iter() {
            let manager_clone = manager.clone();
            let session_manager_clone = session_manager.clone();
            let params = TerminalOutputParams {
                session_id: session_id_str.clone(),
                terminal_id: terminal_id.clone(),
            };

            let handle = tokio::spawn(async move {
                manager_clone
                    .get_output(&session_manager_clone, params)
                    .await
            });

            get_handles.push(handle);
        }

        // Release each terminal concurrently
        for terminal_id in terminal_ids.iter() {
            let manager_clone = manager.clone();
            let session_manager_clone = session_manager.clone();
            let params = TerminalReleaseParams {
                session_id: session_id_str.clone(),
                terminal_id: terminal_id.clone(),
            };

            let handle = tokio::spawn(async move {
                manager_clone
                    .release_terminal(&session_manager_clone, params)
                    .await
            });

            release_handles.push(handle);
        }

        // Wait for all add operations
        for handle in add_handles {
            handle.await.unwrap();
        }

        // All get operations should succeed
        for handle in get_handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        // All release operations should succeed
        for handle in release_handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        // Verify all terminals are released
        let terminals = manager.terminals.read().await;
        for terminal_id in terminal_ids.iter() {
            let session = terminals.get(terminal_id).unwrap();
            assert!(session.is_released().await);
        }
    }

    #[tokio::test]
    async fn test_concurrent_cleanup_and_query() {
        let manager = Arc::new(TerminalManager::new());
        let session_manager = Arc::new(create_test_session_manager().await);

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        let session_id = session_manager.create_session(cwd, None).unwrap();
        let session_id_str = session_id.to_string();

        // Create multiple terminals
        let mut terminal_ids = Vec::new();
        for i in 0..5 {
            let params = TerminalCreateParams {
                session_id: session_id_str.clone(),
                command: "echo".to_string(),
                args: Some(vec![format!("test{}", i)]),
                env: None,
                cwd: None,
                output_byte_limit: None,
            };

            let terminal_id = manager
                .create_terminal_with_command(&session_manager, params)
                .await
                .unwrap();
            terminal_ids.push(terminal_id.clone());

            // Add some output
            let terminals = manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            session.add_output(b"test output").await;
        }

        let mut cleanup_handle = None;
        let mut query_handles = Vec::new();

        // Spawn cleanup task
        {
            let manager_clone = manager.clone();
            let session_id_clone = session_id_str.clone();

            let handle = tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                manager_clone
                    .cleanup_session_terminals(&session_id_clone)
                    .await
            });

            cleanup_handle = Some(handle);
        }

        // Spawn concurrent query tasks (some may fail after cleanup)
        for terminal_id in terminal_ids.iter() {
            let manager_clone = manager.clone();
            let session_manager_clone = session_manager.clone();
            let params = TerminalOutputParams {
                session_id: session_id_str.clone(),
                terminal_id: terminal_id.clone(),
            };

            let handle = tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
                manager_clone
                    .get_output(&session_manager_clone, params)
                    .await
            });

            query_handles.push(handle);
        }

        // Wait for cleanup to complete
        let cleanup_result = cleanup_handle.unwrap().await.unwrap();
        assert!(cleanup_result.is_ok());

        // Wait for query tasks (some may succeed, some may fail depending on timing)
        for handle in query_handles {
            let _ = handle.await.unwrap();
        }

        // After cleanup, all terminals should be removed
        let terminals = manager.terminals.read().await;
        assert_eq!(terminals.len(), 0);
    }
}
