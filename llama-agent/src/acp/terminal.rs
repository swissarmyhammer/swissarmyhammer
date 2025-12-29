//! Terminal management for ACP
//!
//! This module handles terminal process management exposed via ACP protocol.

use agent_client_protocol::ClientCapabilities;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use super::error::TerminalError;

/// Terminal state tracking
///
/// Represents the lifecycle of a terminal process from creation to cleanup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalState {
    /// Terminal created but not yet running
    Created,
    /// Process is actively running
    Running,
    /// Process finished with exit code
    Finished(i32),
    /// Process was forcibly killed
    Killed,
    /// Terminal resources have been released and cleaned up
    Released,
}

/// A terminal session managing a single process
struct TerminalSession {
    /// The child process
    process: Child,
    /// Buffer for output from the process (shared with async capture tasks)
    output_buffer: Arc<Mutex<Vec<u8>>>,
    /// Position of last read in the buffer
    last_read_pos: usize,
    /// Current state of the terminal
    state: TerminalState,
    /// Flag indicating if output has been truncated due to buffer limits
    output_truncated: Arc<Mutex<bool>>,
    /// Graceful shutdown timeout before escalating to SIGKILL
    graceful_shutdown_timeout: Duration,
}

/// Terminal identifier type
pub type TerminalId = String;

/// Request to create a new terminal
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTerminalRequest {
    /// Command to execute
    pub command: String,
}

/// Response from creating a terminal
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTerminalResponse {
    /// The ID of the created terminal
    pub terminal_id: TerminalId,
}

/// Request to get terminal output
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOutputRequest {
    /// The terminal to get output from
    pub terminal_id: TerminalId,
}

/// Response containing terminal output
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOutputResponse {
    /// Output since last read
    pub output: String,
    /// Whether output has been truncated due to buffer size limits
    pub truncated: bool,
}

/// Request to wait for terminal exit
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitForExitRequest {
    /// The terminal to wait for
    pub terminal_id: TerminalId,
}

/// Response with exit status
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitForExitResponse {
    /// Exit code (0 for success, non-zero for error)
    pub exit_code: Option<i32>,
    /// Signal name if process was terminated by signal
    pub signal: Option<String>,
}

/// Request to get terminal state
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTerminalRequest {
    /// The terminal to get state for
    pub terminal_id: TerminalId,
}

/// Response containing terminal state
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTerminalResponse {
    /// The terminal ID
    pub terminal_id: TerminalId,
    /// Current state of the terminal
    pub state: String,
}

/// Request to kill a terminal process
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillTerminalRequest {
    /// The terminal to kill
    pub terminal_id: TerminalId,
}

/// Response from killing a terminal
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillTerminalResponse {
    /// Whether the process was successfully killed
    pub killed: bool,
}

/// Terminal manager
pub struct TerminalManager {
    /// Active terminals indexed by ID
    terminals: HashMap<TerminalId, TerminalSession>,
    /// Maximum buffer size for output (default 1MB)
    max_buffer_size: usize,
    /// Graceful shutdown timeout before escalating to SIGKILL
    graceful_shutdown_timeout: Duration,
    /// Client capabilities from ACP initialize request
    client_capabilities: ClientCapabilities,
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalManager {
    /// Create a new terminal manager with default buffer size (1MB)
    pub fn new() -> Self {
        Self::with_capabilities(ClientCapabilities::default())
    }

    /// Create a new terminal manager with specified buffer size
    pub fn with_buffer_size(max_buffer_size: usize) -> Self {
        Self::with_config(max_buffer_size, Duration::from_secs(5))
    }

    /// Create a new terminal manager with specified buffer size and timeout
    pub fn with_config(max_buffer_size: usize, graceful_shutdown_timeout: Duration) -> Self {
        Self::with_config_and_capabilities(
            max_buffer_size,
            graceful_shutdown_timeout,
            ClientCapabilities::default(),
        )
    }

    /// Create a new terminal manager with specified client capabilities
    pub fn with_capabilities(client_capabilities: ClientCapabilities) -> Self {
        Self::with_config_and_capabilities(1_048_576, Duration::from_secs(5), client_capabilities)
    }

    /// Create a new terminal manager with full configuration including capabilities
    pub fn with_config_and_capabilities(
        max_buffer_size: usize,
        graceful_shutdown_timeout: Duration,
        client_capabilities: ClientCapabilities,
    ) -> Self {
        Self {
            terminals: HashMap::new(),
            max_buffer_size,
            graceful_shutdown_timeout,
            client_capabilities,
        }
    }

    /// Update client capabilities from initialize request
    pub fn set_client_capabilities(&mut self, client_capabilities: ClientCapabilities) {
        self.client_capabilities = client_capabilities;
    }

    /// Check if client has terminal capability
    fn check_terminal_capability(&self) -> Result<(), TerminalError> {
        if !self.client_capabilities.terminal {
            return Err(TerminalError::CapabilityNotSupported);
        }
        Ok(())
    }

    /// Generate ACP-compliant terminal ID with "term_" prefix
    fn generate_terminal_id(&self) -> String {
        format!("term_{}", ulid::Ulid::new())
    }

    /// Create a terminal
    pub async fn create_terminal(
        &mut self,
        req: CreateTerminalRequest,
    ) -> Result<CreateTerminalResponse, TerminalError> {
        // Check client capability
        self.check_terminal_capability()?;

        // Parse command and args
        let mut parts = req.command.split_whitespace();
        let cmd = parts
            .next()
            .ok_or_else(|| TerminalError::ExecutionFailed("Empty command".to_string()))?;
        let args: Vec<&str> = parts.collect();

        // Spawn process
        let mut child = Command::new(cmd)
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| TerminalError::CreationFailed(e.to_string()))?;

        // Generate ACP-compliant terminal ID
        let terminal_id = self.generate_terminal_id();

        // Create shared output buffer and truncation flag
        let output_buffer = Arc::new(Mutex::new(Vec::new()));
        let output_truncated = Arc::new(Mutex::new(false));

        // Spawn async task to capture stdout
        if let Some(mut stdout) = child.stdout.take() {
            let buffer = Arc::clone(&output_buffer);
            let truncated = Arc::clone(&output_truncated);
            let max_size = self.max_buffer_size;
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                loop {
                    match stdout.read(&mut buf).await {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            let mut buffer_guard = buffer.lock().await;
                            buffer_guard.extend_from_slice(&buf[..n]);
                            // Truncate if buffer exceeds max size
                            if buffer_guard.len() > max_size {
                                let excess = buffer_guard.len() - max_size;
                                buffer_guard.drain(..excess);
                                // Set truncation flag
                                *truncated.lock().await = true;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        // Spawn async task to capture stderr
        if let Some(mut stderr) = child.stderr.take() {
            let buffer = Arc::clone(&output_buffer);
            let truncated = Arc::clone(&output_truncated);
            let max_size = self.max_buffer_size;
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                loop {
                    match stderr.read(&mut buf).await {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            let mut buffer_guard = buffer.lock().await;
                            buffer_guard.extend_from_slice(&buf[..n]);
                            // Truncate if buffer exceeds max size
                            if buffer_guard.len() > max_size {
                                let excess = buffer_guard.len() - max_size;
                                buffer_guard.drain(..excess);
                                // Set truncation flag
                                *truncated.lock().await = true;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        // Store session
        let session = TerminalSession {
            process: child,
            output_buffer,
            last_read_pos: 0,
            state: TerminalState::Running,
            output_truncated,
            graceful_shutdown_timeout: self.graceful_shutdown_timeout,
        };

        self.terminals.insert(terminal_id.clone(), session);

        Ok(CreateTerminalResponse { terminal_id })
    }

    /// Get terminal output
    pub async fn get_output(
        &mut self,
        req: TerminalOutputRequest,
    ) -> Result<TerminalOutputResponse, TerminalError> {
        // Check client capability
        self.check_terminal_capability()?;

        let session = self
            .terminals
            .get_mut(&req.terminal_id)
            .ok_or_else(|| TerminalError::NotFound(req.terminal_id.clone()))?;

        // Access the shared output buffer
        let buffer_guard = session.output_buffer.lock().await;

        // Return output since last read
        let new_output = &buffer_guard[session.last_read_pos..];
        let output = String::from_utf8_lossy(new_output).to_string();

        // Update last read position
        let new_pos = buffer_guard.len();
        drop(buffer_guard);
        session.last_read_pos = new_pos;

        // Get truncation status
        let truncated = *session.output_truncated.lock().await;

        Ok(TerminalOutputResponse { output, truncated })
    }

    /// Wait for terminal exit
    pub async fn wait_for_exit(
        &mut self,
        req: WaitForExitRequest,
    ) -> Result<WaitForExitResponse, TerminalError> {
        // Check client capability
        self.check_terminal_capability()?;

        let session = self
            .terminals
            .get_mut(&req.terminal_id)
            .ok_or_else(|| TerminalError::NotFound(req.terminal_id.clone()))?;

        // Wait for process to exit
        let status = session.process.wait().await?;

        let exit_code = status.code();
        let signal = Self::get_signal_name(&status);

        // Update state with exit code (use 0 if None for state tracking)
        session.state = TerminalState::Finished(exit_code.unwrap_or(0));

        Ok(WaitForExitResponse { exit_code, signal })
    }

    /// Extract signal name from exit status (Unix only)
    #[cfg(unix)]
    fn get_signal_name(status: &std::process::ExitStatus) -> Option<String> {
        use std::os::unix::process::ExitStatusExt;
        status.signal().map(|sig| match sig {
            1 => "SIGHUP".to_string(),
            2 => "SIGINT".to_string(),
            3 => "SIGQUIT".to_string(),
            6 => "SIGABRT".to_string(),
            9 => "SIGKILL".to_string(),
            15 => "SIGTERM".to_string(),
            _ => format!("signal {}", sig),
        })
    }

    /// Extract signal name from exit status (non-Unix)
    #[cfg(not(unix))]
    fn get_signal_name(_status: &std::process::ExitStatus) -> Option<String> {
        None
    }

    /// Get the current state of a terminal
    pub fn get_state(&self, terminal_id: &TerminalId) -> Result<TerminalState, TerminalError> {
        self.terminals
            .get(terminal_id)
            .map(|session| session.state.clone())
            .ok_or_else(|| TerminalError::NotFound(terminal_id.clone()))
    }

    /// Get terminal information by ID
    pub fn get_terminal(
        &self,
        req: GetTerminalRequest,
    ) -> Result<GetTerminalResponse, TerminalError> {
        // Check client capability
        self.check_terminal_capability()?;

        let state = self.get_state(&req.terminal_id)?;

        let state_str = match state {
            TerminalState::Created => "created".to_string(),
            TerminalState::Running => "running".to_string(),
            TerminalState::Finished(code) => format!("finished:{}", code),
            TerminalState::Killed => "killed".to_string(),
            TerminalState::Released => "released".to_string(),
        };

        Ok(GetTerminalResponse {
            terminal_id: req.terminal_id,
            state: state_str,
        })
    }

    /// Kill a terminal process with graceful shutdown
    ///
    /// This method implements a two-phase termination strategy:
    /// 1. Send SIGTERM (Unix) or attempt graceful termination (Windows) to allow cleanup
    /// 2. Wait for graceful_shutdown_timeout
    /// 3. Send SIGKILL (Unix) or forceful termination (Windows) if process still running
    ///
    /// This matches the claude-agent reference implementation for graceful process termination.
    pub async fn kill_terminal(
        &mut self,
        req: KillTerminalRequest,
    ) -> Result<KillTerminalResponse, TerminalError> {
        // Check client capability
        self.check_terminal_capability()?;

        let session = self
            .terminals
            .get_mut(&req.terminal_id)
            .ok_or_else(|| TerminalError::NotFound(req.terminal_id.clone()))?;

        // Check if process is already finished
        match session.state {
            TerminalState::Finished(_) | TerminalState::Killed | TerminalState::Released => {
                return Ok(KillTerminalResponse { killed: false });
            }
            _ => {}
        }

        let graceful_timeout = session.graceful_shutdown_timeout;

        #[cfg(unix)]
        {
            Self::kill_process_unix(session, graceful_timeout).await?;
        }

        #[cfg(not(unix))]
        {
            Self::kill_process_windows(session, graceful_timeout).await?;
        }

        // Update state
        session.state = TerminalState::Killed;

        Ok(KillTerminalResponse { killed: true })
    }

    /// Kill process on Unix with SIGTERM -> SIGKILL escalation
    #[cfg(unix)]
    async fn kill_process_unix(
        session: &mut TerminalSession,
        graceful_timeout: Duration,
    ) -> Result<(), TerminalError> {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        let pid = session
            .process
            .id()
            .ok_or_else(|| TerminalError::InvalidState("Process ID not available".to_string()))?;

        let pid = Pid::from_raw(pid as i32);

        // Send SIGTERM for graceful shutdown
        tracing::debug!("Sending SIGTERM to process {}", pid);
        kill(pid, Signal::SIGTERM)
            .map_err(|e| TerminalError::KillFailed(format!("Failed to send SIGTERM: {}", e)))?;

        // Wait for graceful shutdown with timeout
        let wait_result = tokio::time::timeout(graceful_timeout, session.process.wait()).await;

        match wait_result {
            Ok(Ok(status)) => {
                tracing::debug!("Process terminated gracefully with status: {:?}", status);
                session.state = TerminalState::Finished(status.code().unwrap_or(0));
                Ok(())
            }
            Ok(Err(e)) => Err(TerminalError::Io(e)),
            Err(_) => {
                // Timeout - force kill with SIGKILL
                tracing::debug!(
                    "Graceful shutdown timed out after {:?}, sending SIGKILL to process {}",
                    graceful_timeout,
                    pid
                );
                kill(pid, Signal::SIGKILL).map_err(|e| {
                    TerminalError::KillFailed(format!("Failed to send SIGKILL: {}", e))
                })?;

                // Wait for forceful kill
                let status = session.process.wait().await?;

                tracing::debug!("Process forcefully killed with status: {:?}", status);
                session.state = TerminalState::Killed;
                Ok(())
            }
        }
    }

    /// Kill process on Windows (immediate termination)
    #[cfg(not(unix))]
    async fn kill_process_windows(
        session: &mut TerminalSession,
        _graceful_timeout: Duration,
    ) -> Result<(), TerminalError> {
        // Windows doesn't support SIGTERM, so we use kill() directly
        // Note: This is less graceful than Unix but matches platform capabilities
        tracing::debug!("Killing Windows process");
        session.process.kill().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_terminal() {
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager = TerminalManager::with_capabilities(caps);

        let req = CreateTerminalRequest {
            command: "echo hello".to_string(),
        };

        let response = manager.create_terminal(req).await;
        assert!(response.is_ok());

        let terminal_id = response.unwrap().terminal_id;
        assert!(!terminal_id.is_empty());
        assert!(
            terminal_id.starts_with("term_"),
            "Terminal ID should have term_ prefix"
        );
        assert!(manager.terminals.contains_key(&terminal_id));
    }

    #[tokio::test]
    async fn test_get_output() {
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager = TerminalManager::with_capabilities(caps);

        let req = CreateTerminalRequest {
            command: "echo hello".to_string(),
        };

        let create_response = manager.create_terminal(req).await.unwrap();
        let terminal_id = create_response.terminal_id;

        // Give the process time to run
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let output_req = TerminalOutputRequest {
            terminal_id: terminal_id.clone(),
        };

        let output_response = manager.get_output(output_req).await;
        assert!(output_response.is_ok());

        let response = output_response.unwrap();
        assert!(response.output.contains("hello"));
        assert!(!response.truncated);
    }

    #[tokio::test]
    async fn test_wait_for_exit() {
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager = TerminalManager::with_capabilities(caps);

        let req = CreateTerminalRequest {
            command: "echo hello".to_string(),
        };

        let create_response = manager.create_terminal(req).await.unwrap();
        let terminal_id = create_response.terminal_id;

        let wait_req = WaitForExitRequest {
            terminal_id: terminal_id.clone(),
        };

        let exit_response = manager.wait_for_exit(wait_req).await;
        assert!(exit_response.is_ok());

        let response = exit_response.unwrap();
        assert_eq!(response.exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_terminal_not_found() {
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager = TerminalManager::with_capabilities(caps);

        let output_req = TerminalOutputRequest {
            terminal_id: "nonexistent".to_string(),
        };

        let result = manager.get_output(output_req).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Terminal not found"));
    }

    #[tokio::test]
    async fn test_empty_command() {
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager = TerminalManager::with_capabilities(caps);

        let req = CreateTerminalRequest {
            command: "".to_string(),
        };

        let result = manager.create_terminal(req).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty command"));
    }

    #[tokio::test]
    async fn test_invalid_command() {
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager = TerminalManager::with_capabilities(caps);

        let req = CreateTerminalRequest {
            command: "nonexistent_command_that_should_not_exist".to_string(),
        };

        let result = manager.create_terminal(req).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        eprintln!("Actual error message: {}", err_msg);
        assert!(
            err_msg.contains("Failed to create terminal")
                || err_msg.contains("Failed to spawn process")
        );
    }

    #[tokio::test]
    async fn test_output_buffer_incremental_read() {
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager = TerminalManager::with_capabilities(caps);

        let req = CreateTerminalRequest {
            command: "sh -c 'echo first && sleep 0.05 && echo second'".to_string(),
        };

        let create_response = manager.create_terminal(req).await.unwrap();
        let terminal_id = create_response.terminal_id;

        // First read
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        let output_req1 = TerminalOutputRequest {
            terminal_id: terminal_id.clone(),
        };
        let response1 = manager.get_output(output_req1).await.unwrap();

        // Second read
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let output_req2 = TerminalOutputRequest {
            terminal_id: terminal_id.clone(),
        };
        let response2 = manager.get_output(output_req2).await.unwrap();

        // First read should have first output
        assert!(response1.output.contains("first"));
        // Second read should have second output (not duplicated first)
        if !response2.output.is_empty() {
            assert!(response2.output.contains("second"));
            assert!(!response2.output.contains("first"));
        }
    }

    #[tokio::test]
    async fn test_get_state() {
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager = TerminalManager::with_capabilities(caps);

        let req = CreateTerminalRequest {
            command: "echo hello".to_string(),
        };

        let create_response = manager.create_terminal(req).await.unwrap();
        let terminal_id = create_response.terminal_id;

        // Check state after creation - should be Running
        let state = manager.get_state(&terminal_id).unwrap();
        assert_eq!(state, TerminalState::Running);

        // Wait for process to exit
        let wait_req = WaitForExitRequest {
            terminal_id: terminal_id.clone(),
        };
        let exit_response = manager.wait_for_exit(wait_req).await.unwrap();
        assert_eq!(exit_response.exit_code, Some(0));

        // Check state after exit - should be Finished
        let state = manager.get_state(&terminal_id).unwrap();
        assert_eq!(state, TerminalState::Finished(0));
    }

    #[tokio::test]
    async fn test_get_state_not_found() {
        let caps = ClientCapabilities::default().terminal(true);
        let manager = TerminalManager::with_capabilities(caps);

        let result = manager.get_state(&"nonexistent".to_string());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Terminal not found"));
    }

    #[tokio::test]
    async fn test_get_terminal() {
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager = TerminalManager::with_capabilities(caps);

        let req = CreateTerminalRequest {
            command: "echo hello".to_string(),
        };

        let create_response = manager.create_terminal(req).await.unwrap();
        let terminal_id = create_response.terminal_id;

        // Get terminal info
        let get_req = GetTerminalRequest {
            terminal_id: terminal_id.clone(),
        };

        let get_response = manager.get_terminal(get_req).unwrap();
        assert_eq!(get_response.terminal_id, terminal_id);
        assert_eq!(get_response.state, "running");
    }

    #[tokio::test]
    async fn test_get_terminal_not_found() {
        let caps = ClientCapabilities::default().terminal(true);
        let manager = TerminalManager::with_capabilities(caps);

        let get_req = GetTerminalRequest {
            terminal_id: "nonexistent".to_string(),
        };

        let result = manager.get_terminal(get_req);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Terminal not found"));
    }

    #[tokio::test]
    async fn test_get_terminal_after_exit() {
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager = TerminalManager::with_capabilities(caps);

        let req = CreateTerminalRequest {
            command: "echo hello".to_string(),
        };

        let create_response = manager.create_terminal(req).await.unwrap();
        let terminal_id = create_response.terminal_id;

        // Wait for process to exit
        let wait_req = WaitForExitRequest {
            terminal_id: terminal_id.clone(),
        };
        manager.wait_for_exit(wait_req).await.unwrap();

        // Get terminal info after exit
        let get_req = GetTerminalRequest {
            terminal_id: terminal_id.clone(),
        };

        let get_response = manager.get_terminal(get_req).unwrap();
        assert_eq!(get_response.terminal_id, terminal_id);
        assert_eq!(get_response.state, "finished:0");
    }

    #[tokio::test]
    async fn test_kill_terminal() {
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager = TerminalManager::with_capabilities(caps);

        // Create a long-running process
        let req = CreateTerminalRequest {
            command: "sleep 10".to_string(),
        };

        let create_response = manager.create_terminal(req).await.unwrap();
        let terminal_id = create_response.terminal_id;

        // Verify process is running
        let state = manager.get_state(&terminal_id).unwrap();
        assert_eq!(state, TerminalState::Running);

        // Kill the process
        let kill_req = KillTerminalRequest {
            terminal_id: terminal_id.clone(),
        };

        let kill_response = manager.kill_terminal(kill_req).await.unwrap();
        assert!(kill_response.killed);

        // Verify state is now Killed
        let state = manager.get_state(&terminal_id).unwrap();
        assert_eq!(state, TerminalState::Killed);
    }

    #[tokio::test]
    async fn test_kill_terminal_not_found() {
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager = TerminalManager::with_capabilities(caps);

        let kill_req = KillTerminalRequest {
            terminal_id: "nonexistent".to_string(),
        };

        let result = manager.kill_terminal(kill_req).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Terminal not found"));
    }

    #[tokio::test]
    async fn test_kill_terminal_already_finished() {
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager = TerminalManager::with_capabilities(caps);

        let req = CreateTerminalRequest {
            command: "echo hello".to_string(),
        };

        let create_response = manager.create_terminal(req).await.unwrap();
        let terminal_id = create_response.terminal_id;

        // Wait for process to exit
        let wait_req = WaitForExitRequest {
            terminal_id: terminal_id.clone(),
        };
        manager.wait_for_exit(wait_req).await.unwrap();

        // Try to kill already finished process
        let kill_req = KillTerminalRequest {
            terminal_id: terminal_id.clone(),
        };

        let kill_response = manager.kill_terminal(kill_req).await.unwrap();
        assert!(!kill_response.killed);
    }

    #[tokio::test]
    async fn test_terminal_manager_with_custom_buffer_size() {
        // Create manager with custom buffer size
        let custom_buffer_size = 512;
        let caps = ClientCapabilities::default().terminal(true);
        let manager = TerminalManager::with_config_and_capabilities(
            custom_buffer_size,
            Duration::from_secs(5),
            caps.clone(),
        );
        assert_eq!(manager.max_buffer_size, custom_buffer_size);

        // Verify default constructor still uses 1MB
        let default_manager = TerminalManager::with_capabilities(caps);
        assert_eq!(default_manager.max_buffer_size, 1_048_576);
    }

    #[tokio::test]
    async fn test_terminal_manager_with_custom_timeout() {
        // Create manager with custom timeout
        let custom_buffer_size = 1024;
        let custom_timeout = Duration::from_secs(10);
        let caps = ClientCapabilities::default().terminal(true);
        let manager =
            TerminalManager::with_config_and_capabilities(custom_buffer_size, custom_timeout, caps);

        assert_eq!(manager.max_buffer_size, custom_buffer_size);
        assert_eq!(manager.graceful_shutdown_timeout, custom_timeout);
    }

    #[tokio::test]
    async fn test_terminal_session_inherits_timeout() {
        let custom_timeout = Duration::from_secs(3);
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager = TerminalManager::with_config_and_capabilities(1024, custom_timeout, caps);

        let req = CreateTerminalRequest {
            command: "echo test".to_string(),
        };

        let create_response = manager.create_terminal(req).await.unwrap();
        let terminal_id = create_response.terminal_id;

        // Verify the session has the configured timeout
        let session = manager.terminals.get(&terminal_id).unwrap();
        assert_eq!(session.graceful_shutdown_timeout, custom_timeout);
    }

    #[tokio::test]
    async fn test_output_truncation_flag() {
        // Create manager with very small buffer to trigger truncation
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager =
            TerminalManager::with_config_and_capabilities(50, Duration::from_secs(5), caps);

        let req = CreateTerminalRequest {
            command: "sh -c 'for i in 1 2 3 4 5 6 7 8 9 10; do echo \"Line $i with some text to fill buffer\"; done'".to_string(),
        };

        let create_response = manager.create_terminal(req).await.unwrap();
        let terminal_id = create_response.terminal_id;

        // Give the process time to generate output
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let output_req = TerminalOutputRequest {
            terminal_id: terminal_id.clone(),
        };

        let response = manager.get_output(output_req).await.unwrap();

        // With a 50 byte buffer and multiple lines of output, truncation should occur
        assert!(
            response.truncated,
            "Output should be truncated with small buffer"
        );

        // Output should not be empty
        assert!(!response.output.is_empty());

        // Buffer size should be limited
        assert!(response.output.len() <= 50);
    }

    #[tokio::test]
    async fn test_no_truncation_with_small_output() {
        // Create manager with reasonable buffer size
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager =
            TerminalManager::with_config_and_capabilities(1024, Duration::from_secs(5), caps);

        let req = CreateTerminalRequest {
            command: "echo hello".to_string(),
        };

        let create_response = manager.create_terminal(req).await.unwrap();
        let terminal_id = create_response.terminal_id;

        // Give the process time to run
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let output_req = TerminalOutputRequest {
            terminal_id: terminal_id.clone(),
        };

        let response = manager.get_output(output_req).await.unwrap();

        // Small output should not trigger truncation
        assert!(!response.truncated, "Small output should not be truncated");
        assert!(response.output.contains("hello"));
    }

    #[test]
    fn test_terminal_structures_serialization_camelcase() {
        // Test CreateTerminalResponse
        let response = CreateTerminalResponse {
            terminal_id: "term_123".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(
            json.contains("terminalId"),
            "CreateTerminalResponse should serialize terminal_id as terminalId (camelCase). Found: {}",
            json
        );
        assert!(
            !json.contains("terminal_id"),
            "CreateTerminalResponse should NOT use snake_case terminal_id. Found: {}",
            json
        );

        // Test TerminalOutputRequest
        let request = TerminalOutputRequest {
            terminal_id: "term_123".to_string(),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(
            json.contains("terminalId"),
            "TerminalOutputRequest should serialize terminal_id as terminalId (camelCase). Found: {}",
            json
        );
        assert!(
            !json.contains("terminal_id"),
            "TerminalOutputRequest should NOT use snake_case terminal_id. Found: {}",
            json
        );

        // Test WaitForExitRequest
        let request = WaitForExitRequest {
            terminal_id: "term_123".to_string(),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(
            json.contains("terminalId"),
            "WaitForExitRequest should serialize terminal_id as terminalId (camelCase). Found: {}",
            json
        );
        assert!(
            !json.contains("terminal_id"),
            "WaitForExitRequest should NOT use snake_case terminal_id. Found: {}",
            json
        );

        // Test WaitForExitResponse with exitCode
        let response = WaitForExitResponse {
            exit_code: Some(0),
            signal: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(
            json.contains("exitCode"),
            "WaitForExitResponse should serialize exit_code as exitCode (camelCase). Found: {}",
            json
        );
        assert!(
            !json.contains("exit_code"),
            "WaitForExitResponse should NOT use snake_case exit_code. Found: {}",
            json
        );

        // Test GetTerminalRequest
        let request = GetTerminalRequest {
            terminal_id: "term_123".to_string(),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(
            json.contains("terminalId"),
            "GetTerminalRequest should serialize terminal_id as terminalId (camelCase). Found: {}",
            json
        );
        assert!(
            !json.contains("terminal_id"),
            "GetTerminalRequest should NOT use snake_case terminal_id. Found: {}",
            json
        );

        // Test GetTerminalResponse
        let response = GetTerminalResponse {
            terminal_id: "term_123".to_string(),
            state: "running".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(
            json.contains("terminalId"),
            "GetTerminalResponse should serialize terminal_id as terminalId (camelCase). Found: {}",
            json
        );
        assert!(
            !json.contains("terminal_id"),
            "GetTerminalResponse should NOT use snake_case terminal_id. Found: {}",
            json
        );

        // Test KillTerminalRequest
        let request = KillTerminalRequest {
            terminal_id: "term_123".to_string(),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(
            json.contains("terminalId"),
            "KillTerminalRequest should serialize terminal_id as terminalId (camelCase). Found: {}",
            json
        );
        assert!(
            !json.contains("terminal_id"),
            "KillTerminalRequest should NOT use snake_case terminal_id. Found: {}",
            json
        );
    }

    #[tokio::test]
    async fn test_capture_command_output_comprehensive() {
        let caps = ClientCapabilities::default().terminal(true);
        let mut manager = TerminalManager::with_capabilities(caps);

        // Test command that produces multi-line output
        // Using printf to generate multiple lines in a single command
        let req = CreateTerminalRequest {
            command: "printf Line1\\nLine2\\nLine3\\nLine4\\n".to_string(),
        };

        let create_response = manager.create_terminal(req).await;
        assert!(create_response.is_ok(), "Failed to create terminal");

        let terminal_id = create_response.unwrap().terminal_id;

        // Give the process time to run and complete
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // Capture the output
        let output_req = TerminalOutputRequest {
            terminal_id: terminal_id.clone(),
        };

        let output_response = manager.get_output(output_req).await;
        assert!(output_response.is_ok(), "Failed to get output");

        let response = output_response.unwrap();

        // Verify all output lines are captured
        assert!(
            response.output.contains("Line1"),
            "Output should contain Line1. Actual: {:?}",
            response.output
        );
        assert!(
            response.output.contains("Line2"),
            "Output should contain Line2. Actual: {:?}",
            response.output
        );
        assert!(
            response.output.contains("Line3"),
            "Output should contain Line3. Actual: {:?}",
            response.output
        );
        assert!(
            response.output.contains("Line4"),
            "Output should contain Line4. Actual: {:?}",
            response.output
        );

        // Verify output is not truncated for this small amount
        assert!(
            !response.truncated,
            "Output should not be truncated for small output"
        );

        // Verify the output is not empty
        assert!(!response.output.is_empty(), "Output should not be empty");
    }

    #[tokio::test]
    async fn test_capability_enforcement_create_terminal() {
        // Create manager without terminal capability
        let caps = ClientCapabilities::default(); // terminal defaults to false
        let mut manager = TerminalManager::with_capabilities(caps);

        let req = CreateTerminalRequest {
            command: "echo test".to_string(),
        };

        let result = manager.create_terminal(req).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TerminalError::CapabilityNotSupported
        ));
    }

    #[tokio::test]
    async fn test_capability_enforcement_get_output() {
        // Create manager without terminal capability
        let caps = ClientCapabilities::default();
        let mut manager = TerminalManager::with_capabilities(caps);

        let req = TerminalOutputRequest {
            terminal_id: "term_123".to_string(),
        };

        let result = manager.get_output(req).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TerminalError::CapabilityNotSupported
        ));
    }

    #[tokio::test]
    async fn test_capability_enforcement_wait_for_exit() {
        // Create manager without terminal capability
        let caps = ClientCapabilities::default();
        let mut manager = TerminalManager::with_capabilities(caps);

        let req = WaitForExitRequest {
            terminal_id: "term_123".to_string(),
        };

        let result = manager.wait_for_exit(req).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TerminalError::CapabilityNotSupported
        ));
    }

    #[tokio::test]
    async fn test_capability_enforcement_get_terminal() {
        // Create manager without terminal capability
        let caps = ClientCapabilities::default();
        let manager = TerminalManager::with_capabilities(caps);

        let req = GetTerminalRequest {
            terminal_id: "term_123".to_string(),
        };

        let result = manager.get_terminal(req);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TerminalError::CapabilityNotSupported
        ));
    }

    #[tokio::test]
    async fn test_capability_enforcement_kill_terminal() {
        // Create manager without terminal capability
        let caps = ClientCapabilities::default();
        let mut manager = TerminalManager::with_capabilities(caps);

        let req = KillTerminalRequest {
            terminal_id: "term_123".to_string(),
        };

        let result = manager.kill_terminal(req).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TerminalError::CapabilityNotSupported
        ));
    }
}
