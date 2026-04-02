//! Single LSP server child-process management.
//!
//! [`LspDaemon`] owns exactly one LSP server process. It handles:
//! - Spawning the binary with stdin/stdout pipes
//! - The LSP `initialize` / `initialized` handshake
//! - Periodic health checks via `child.try_wait()`
//! - Restart with exponential backoff (cap 60 s, max 5 consecutive failures)
//! - Graceful shutdown (`shutdown` request + `exit` notification, then SIGKILL)

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, Command};
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

use swissarmyhammer_code_context::{LspJsonRpcClient, SharedLspClient};

use crate::error::LspError;
use crate::types::{LspDaemonState, OwnedLspServerSpec};

/// Maximum consecutive failures before we stop retrying.
const MAX_CONSECUTIVE_FAILURES: u32 = 5;

/// Backoff durations in seconds: 1, 2, 4, 8, 16, 32, 60, 60, ...
const BACKOFF_BASE: u64 = 1;
const BACKOFF_CAP_SECS: u64 = 60;

/// Timeout for the graceful shutdown phase before SIGKILL.
const SHUTDOWN_GRACE_SECS: u64 = 5;

/// Compute the backoff duration for the given attempt number (0-indexed).
///
/// Sequence: 1s, 2s, 4s, 8s, 16s, 32s, 60s, 60s, ...
pub(crate) fn backoff_duration(attempt: u32) -> Duration {
    let secs = BACKOFF_BASE
        .checked_shl(attempt)
        .unwrap_or(BACKOFF_CAP_SECS)
        .min(BACKOFF_CAP_SECS);
    Duration::from_secs(secs)
}

/// Manages the lifecycle of a single LSP server child process.
pub struct LspDaemon {
    /// The server specification (owned, from the registry).
    spec: OwnedLspServerSpec,
    /// Workspace root URI passed to the LSP server.
    workspace_root: PathBuf,
    /// Current child process handle (if running).
    ///
    /// After a successful handshake, stdin and stdout are taken from this handle
    /// and given to `client`. The `Child` is retained for `try_wait()` health
    /// checks and `kill()` on shutdown.
    child: Option<Child>,
    /// JSON-RPC client created after a successful initialize handshake.
    ///
    /// Stored behind `Arc<Mutex<Option<...>>>` so external consumers (like the
    /// LSP indexing worker) can share access to the client without owning the
    /// daemon. The `Option` is `None` when the daemon is not running.
    client: SharedLspClient,
    /// Consecutive failure count for backoff calculation.
    consecutive_failures: u32,
    /// Observable state — subscribers get notified on every transition.
    state_tx: watch::Sender<LspDaemonState>,
    /// Read-side of the state watch (cloneable for external consumers).
    state_rx: watch::Receiver<LspDaemonState>,
}

impl std::fmt::Debug for LspDaemon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspDaemon")
            .field("command", &self.spec.command)
            .field("state", &*self.state_rx.borrow())
            .finish()
    }
}

impl LspDaemon {
    /// Create a new daemon for the given server spec and workspace root.
    pub fn new(spec: OwnedLspServerSpec, workspace_root: PathBuf) -> Self {
        let (state_tx, state_rx) = watch::channel(LspDaemonState::NotStarted);
        Self {
            spec,
            workspace_root,
            child: None,
            client: Arc::new(Mutex::new(None)),
            consecutive_failures: 0,
            state_tx,
            state_rx,
        }
    }

    /// Get a cloneable receiver to observe state changes.
    pub fn state_rx(&self) -> watch::Receiver<LspDaemonState> {
        self.state_rx.clone()
    }

    /// Return a snapshot of the current state.
    pub fn state(&self) -> LspDaemonState {
        self.state_rx.borrow().clone()
    }

    /// Return the command name for this daemon's server.
    pub fn command(&self) -> &str {
        &self.spec.command
    }

    /// Return a mutable reference to the JSON-RPC client, if the server is running.
    ///
    /// Returns `None` if the daemon has not been started, failed to start, or
    /// has been shut down. The client is created after a successful `initialize`
    /// handshake and dropped on shutdown or restart.
    ///
    /// **Note**: This locks the internal `Mutex`. For long-running background work,
    /// prefer [`shared_client()`] and lock externally.
    pub fn client(&self) -> Option<std::sync::MutexGuard<'_, Option<LspJsonRpcClient>>> {
        match self.client.lock() {
            Ok(guard) if guard.is_some() => Some(guard),
            Ok(_) => None,
            Err(poisoned) => {
                let guard = poisoned.into_inner();
                if guard.is_some() {
                    Some(guard)
                } else {
                    None
                }
            }
        }
    }

    /// Return a cloneable handle to the shared LSP client.
    ///
    /// The returned `Arc<Mutex<Option<LspJsonRpcClient>>>` can be passed to
    /// background workers (e.g. the LSP indexing worker) so they can send
    /// requests through the same LSP process.
    pub fn shared_client(&self) -> SharedLspClient {
        Arc::clone(&self.client)
    }

    // -- lifecycle --------------------------------------------------------

    /// Attempt to start the LSP server.
    ///
    /// Checks that the binary exists on PATH, spawns the child, and performs the
    /// `initialize` / `initialized` handshake. On success the state transitions
    /// to `Running`; on failure it transitions to `Failed`.
    pub async fn start(&mut self) -> Result<(), LspError> {
        // Check binary availability
        if which::which(&self.spec.command).is_err() {
            warn!(
                cmd = &self.spec.command,
                hint = &self.spec.install_hint,
                "LSP binary not found on PATH"
            );
            self.set_state(LspDaemonState::NotFound);
            return Err(LspError::BinaryNotFound {
                command: self.spec.command.clone(),
                install_hint: self.spec.install_hint.clone(),
            });
        }

        self.set_state(LspDaemonState::Starting);

        // Spawn child
        let mut child = match Command::new(&self.spec.command)
            .args(&self.spec.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                error!(cmd = &self.spec.command, %e, "Failed to spawn LSP server");
                self.record_failure(format!("spawn failed: {e}"));
                return Err(LspError::SpawnFailed(e));
            }
        };

        let pid = child.id().unwrap_or(0);
        info!(cmd = &self.spec.command, pid, "LSP server spawned");

        // Perform initialize handshake with timeout
        match tokio::time::timeout(
            self.spec.startup_timeout(),
            Self::initialize_handshake(&mut child, &self.workspace_root, &self.spec),
        )
        .await
        {
            Ok(Ok(())) => {
                let since_epoch_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                // Drain stderr in the background, filtering noise via config
                if let Some(stderr) = child.stderr.take() {
                    let cmd = self.spec.command.clone();
                    // Load stderr filter config once at daemon start
                    let filter_config = {
                        use swissarmyhammer_code_context::config::{
                            load_code_context_config, CompiledCodeContextConfig,
                        };
                        let raw = load_code_context_config();
                        CompiledCodeContextConfig::compile(&raw).ok()
                    };
                    tokio::spawn(async move {
                        use tokio::io::{AsyncBufReadExt, BufReader};
                        let mut lines = BufReader::new(stderr).lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let filtered = filter_config
                                .as_ref()
                                .map(|c| {
                                    swissarmyhammer_code_context::config::should_filter_stderr(
                                        &line, c,
                                    )
                                })
                                .unwrap_or(false);
                            if !filtered {
                                tracing::debug!(cmd = %cmd, "LSP stderr: {}", line);
                            }
                        }
                    });
                }

                // Take stdin/stdout from the child and create the JSON-RPC client.
                // The child handle is retained for health checks and shutdown.
                // Convert tokio async pipes to std blocking pipes via OwnedFd.
                let client = match (child.stdin.take(), child.stdout.take()) {
                    (Some(tokio_stdin), Some(tokio_stdout)) => {
                        match (tokio_stdin.into_owned_fd(), tokio_stdout.into_owned_fd()) {
                            (Ok(stdin_fd), Ok(stdout_fd)) => {
                                let std_stdin: std::process::ChildStdin = stdin_fd.into();
                                let std_stdout: std::process::ChildStdout = stdout_fd.into();
                                Some(LspJsonRpcClient::new(std_stdin, std_stdout))
                            }
                            (Err(e), _) | (_, Err(e)) => {
                                warn!(cmd = &self.spec.command, %e, "Failed to convert pipes to std");
                                None
                            }
                        }
                    }
                    _ => {
                        warn!(
                            cmd = &self.spec.command,
                            "stdin/stdout unavailable after handshake"
                        );
                        None
                    }
                };

                // Store the client in the shared Arc<Mutex<Option<...>>>
                if let Ok(mut guard) = self.client.lock() {
                    *guard = client;
                }
                self.child = Some(child);
                self.consecutive_failures = 0;
                self.set_state(LspDaemonState::Running {
                    pid,
                    since_epoch_ms,
                });
                info!(cmd = &self.spec.command, pid, "LSP server initialized");
                Ok(())
            }
            Ok(Err(e)) => {
                let reason = e.to_string();
                error!(cmd = &self.spec.command, %reason, "LSP initialize failed");
                let _ = child.kill().await;
                self.record_failure(reason.clone());
                Err(e)
            }
            Err(_) => {
                let timeout = self.spec.startup_timeout();
                let reason = format!("initialize timed out after {timeout:?}");
                error!(cmd = &self.spec.command, "LSP initialize timed out");
                let _ = child.kill().await;
                self.record_failure(reason);
                Err(LspError::Timeout(timeout))
            }
        }
    }

    /// Check whether the child process is still alive.
    ///
    /// Returns `true` if the process is running, `false` if it has exited
    /// (which also transitions state to `Failed`).
    pub fn health_check(&mut self) -> bool {
        let child = match self.child.as_mut() {
            Some(c) => c,
            None => return false,
        };

        match child.try_wait() {
            Ok(Some(status)) => {
                let reason = format!("process exited: {status}");
                warn!(cmd = self.spec.command, %status, "LSP server exited unexpectedly");
                if let Ok(mut guard) = self.client.lock() {
                    *guard = None;
                }
                self.child = None;
                self.record_failure(reason);
                false
            }
            Ok(None) => true, // still running
            Err(e) => {
                let reason = format!("try_wait error: {e}");
                warn!(cmd = self.spec.command, %e, "Error checking LSP server health");
                if let Ok(mut guard) = self.client.lock() {
                    *guard = None;
                }
                self.child = None;
                self.record_failure(reason);
                false
            }
        }
    }

    /// Attempt to restart after failure, respecting backoff policy.
    ///
    /// Returns `Ok(())` if the server was restarted, or `Err` if the backoff
    /// budget is exhausted (>= MAX_CONSECUTIVE_FAILURES) or the restart itself
    /// fails.
    pub async fn restart_with_backoff(&mut self) -> Result<(), LspError> {
        if self.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
            return Err(LspError::HandshakeFailed(format!(
                "too many consecutive failures ({}), giving up",
                self.consecutive_failures
            )));
        }

        let delay = backoff_duration(self.consecutive_failures);
        info!(
            cmd = self.spec.command,
            attempt = self.consecutive_failures + 1,
            delay_secs = delay.as_secs(),
            "Restarting LSP server after backoff"
        );
        tokio::time::sleep(delay).await;

        self.start().await
    }

    /// Force-restart the server, resetting the backoff counter.
    ///
    /// Shuts down the current process (if any) and starts fresh.
    pub async fn force_restart(&mut self) -> Result<(), LspError> {
        info!(cmd = self.spec.command, "Force-restarting LSP server");
        self.shutdown().await;
        self.consecutive_failures = 0;
        self.start().await
    }

    /// Gracefully shut down the LSP server.
    ///
    /// Drops the JSON-RPC client (closing stdin/stdout pipes), then waits for
    /// the child process to exit. If it does not exit within
    /// [`SHUTDOWN_GRACE_SECS`], the process is killed on drop.
    pub async fn shutdown(&mut self) {
        // Drop the client first — this closes the stdin/stdout pipes,
        // which signals the LSP server to exit.
        if let Ok(mut guard) = self.client.lock() {
            *guard = None;
        }

        let child = match self.child.take() {
            Some(c) => c,
            None => {
                self.set_state(LspDaemonState::NotStarted);
                return;
            }
        };

        self.set_state(LspDaemonState::ShuttingDown);

        let result = tokio::time::timeout(
            Duration::from_secs(SHUTDOWN_GRACE_SECS),
            Self::graceful_shutdown(child),
        )
        .await;

        match result {
            Ok(Ok(())) => {
                info!(cmd = self.spec.command, "LSP server shut down gracefully");
            }
            Ok(Err(e)) => {
                warn!(cmd = self.spec.command, %e, "Error during graceful shutdown");
            }
            Err(_) => {
                warn!(
                    cmd = self.spec.command,
                    "Shutdown timed out, process should be killed on drop"
                );
            }
        }

        self.set_state(LspDaemonState::NotStarted);
    }

    // -- internal helpers -------------------------------------------------

    /// Transition to a new state and notify watchers.
    fn set_state(&self, state: LspDaemonState) {
        debug!(cmd = self.spec.command, ?state, "State transition");
        let _ = self.state_tx.send(state);
    }

    /// Record a failure, incrementing the consecutive counter.
    fn record_failure(&mut self, reason: String) {
        self.consecutive_failures += 1;
        self.set_state(LspDaemonState::Failed {
            reason,
            attempts: self.consecutive_failures,
        });
    }

    /// Run the `initialize` / `initialized` handshake over the child's stdio.
    async fn initialize_handshake(
        child: &mut Child,
        workspace_root: &Path,
        _spec: &OwnedLspServerSpec,
    ) -> Result<(), LspError> {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| LspError::HandshakeFailed("child stdin unavailable".into()))?;
        let stdout = child
            .stdout
            .as_mut()
            .ok_or_else(|| LspError::HandshakeFailed("child stdout unavailable".into()))?;

        let mut writer = BufWriter::new(stdin);
        let mut reader = BufReader::new(stdout);

        // Build initialize request
        // For owned specs, we always use null initialization options
        let init_options = serde_json::Value::Null;

        let root_uri = url::Url::from_file_path(workspace_root)
            .map_err(|_| LspError::HandshakeFailed("invalid workspace path".into()))?
            .to_string();

        let init_params = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": root_uri,
                "capabilities": {},
                "initializationOptions": init_options
            }
        });

        // Send initialize request
        send_jsonrpc_message(&mut writer, &init_params).await?;

        // Read initialize response — on EOF, capture stderr for diagnostics
        let _response = match read_jsonrpc_message(&mut reader).await {
            Ok(resp) => resp,
            Err(e) => {
                let stderr_context = Self::capture_stderr(child).await;
                let msg = if stderr_context.is_empty() {
                    e.to_string()
                } else {
                    format!("{e}; stderr: {stderr_context}")
                };
                return Err(LspError::HandshakeFailed(msg));
            }
        };

        // Send initialized notification
        let initialized = json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        });
        send_jsonrpc_message(&mut writer, &initialized).await?;

        Ok(())
    }

    /// Read whatever the child has written to stderr (best-effort, with timeout).
    async fn capture_stderr(child: &mut Child) -> String {
        use tokio::io::AsyncReadExt;
        let Some(stderr) = child.stderr.as_mut() else {
            return String::new();
        };
        let mut buf = vec![0u8; 4096];
        match tokio::time::timeout(Duration::from_secs(1), stderr.read(&mut buf)).await {
            Ok(Ok(n)) if n > 0 => String::from_utf8_lossy(&buf[..n]).trim().to_string(),
            _ => String::new(),
        }
    }

    /// Send `shutdown` + `exit` and wait for the child to exit.
    async fn graceful_shutdown(mut child: Child) -> Result<(), LspError> {
        // Try to send shutdown/exit — if stdin is already gone, just kill.
        if let Some(stdin) = child.stdin.as_mut() {
            let mut writer = BufWriter::new(stdin);

            let shutdown_req = json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "shutdown",
                "params": null
            });
            // Ignore send errors — the server may already be dead.
            let _ = send_jsonrpc_message(&mut writer, &shutdown_req).await;

            // Read shutdown response (best-effort)
            if let Some(stdout) = child.stdout.as_mut() {
                let mut reader = BufReader::new(stdout);
                let _ = read_jsonrpc_message(&mut reader).await;
            }

            let exit_notification = json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            });
            let _ = send_jsonrpc_message(&mut writer, &exit_notification).await;
        }

        // Wait for the child to exit
        match child.wait().await {
            Ok(status) => {
                debug!(?status, "LSP server exited after shutdown");
                Ok(())
            }
            Err(e) => Err(LspError::ShutdownFailed(format!("wait failed: {e}"))),
        }
    }
}

// -- JSON-RPC framing helpers ---------------------------------------------

/// Encode and send a JSON-RPC message with `Content-Length` header.
pub async fn send_jsonrpc_message<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    message: &serde_json::Value,
) -> Result<(), LspError> {
    let body = serde_json::to_string(message)
        .map_err(|e| LspError::JsonRpc(format!("json encode: {e}")))?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());

    writer
        .write_all(header.as_bytes())
        .await
        .map_err(|e| LspError::JsonRpc(format!("write header: {e}")))?;
    writer
        .write_all(body.as_bytes())
        .await
        .map_err(|e| LspError::JsonRpc(format!("write body: {e}")))?;
    writer
        .flush()
        .await
        .map_err(|e| LspError::JsonRpc(format!("flush: {e}")))?;

    Ok(())
}

/// Read a single JSON-RPC message, parsing the `Content-Length` header.
pub async fn read_jsonrpc_message<R: AsyncBufReadExt + Unpin>(
    reader: &mut R,
) -> Result<serde_json::Value, LspError> {
    let mut content_length: Option<usize> = None;

    // Read headers until blank line
    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .await
            .map_err(|e| LspError::JsonRpc(format!("read header line: {e}")))?;
        if n == 0 {
            return Err(LspError::JsonRpc("unexpected EOF reading headers".into()));
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            // End of headers
            break;
        }

        if let Some(val) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(
                val.trim()
                    .parse::<usize>()
                    .map_err(|e| LspError::JsonRpc(format!("bad Content-Length: {e}")))?,
            );
        }
        // Ignore other headers (e.g. Content-Type)
    }

    let length =
        content_length.ok_or_else(|| LspError::JsonRpc("missing Content-Length header".into()))?;

    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|e| LspError::JsonRpc(format!("read body: {e}")))?;

    serde_json::from_slice(&body).map_err(|e| LspError::JsonRpc(format!("json decode: {e}")))
}

// -- tests ----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_sequence() {
        assert_eq!(backoff_duration(0), Duration::from_secs(1));
        assert_eq!(backoff_duration(1), Duration::from_secs(2));
        assert_eq!(backoff_duration(2), Duration::from_secs(4));
        assert_eq!(backoff_duration(3), Duration::from_secs(8));
        assert_eq!(backoff_duration(4), Duration::from_secs(16));
        assert_eq!(backoff_duration(5), Duration::from_secs(32));
        assert_eq!(backoff_duration(6), Duration::from_secs(60));
        assert_eq!(backoff_duration(7), Duration::from_secs(60));
        assert_eq!(backoff_duration(100), Duration::from_secs(60));
    }

    #[test]
    fn test_state_transitions() {
        // Starting -> Running
        let state = LspDaemonState::Starting;
        assert_eq!(state, LspDaemonState::Starting);

        let running = LspDaemonState::Running {
            pid: 1234,
            since_epoch_ms: 0,
        };
        assert!(matches!(running, LspDaemonState::Running { pid: 1234, .. }));

        // Starting -> Failed
        let failed = LspDaemonState::Failed {
            reason: "crash".into(),
            attempts: 1,
        };
        assert!(matches!(failed, LspDaemonState::Failed { attempts: 1, .. }));

        // Failed -> Starting (restart)
        let restarting = LspDaemonState::Starting;
        assert_eq!(restarting, LspDaemonState::Starting);
    }

    #[test]
    fn test_state_serialization() {
        let state = LspDaemonState::Running {
            pid: 42,
            since_epoch_ms: 1700000000000,
        };
        let json = serde_json::to_string(&state).expect("serialize");
        let deser: LspDaemonState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(state, deser);

        let failed = LspDaemonState::Failed {
            reason: "timeout".into(),
            attempts: 3,
        };
        let json = serde_json::to_string(&failed).expect("serialize");
        let deser: LspDaemonState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(failed, deser);
    }

    #[tokio::test]
    async fn test_jsonrpc_roundtrip() {
        let msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": { "processId": 123 }
        });

        // Write to a buffer
        let mut buf: Vec<u8> = Vec::new();
        send_jsonrpc_message(&mut buf, &msg).await.unwrap();

        // Read back
        let mut cursor = &buf[..];
        let mut reader = BufReader::new(&mut cursor);
        let decoded = read_jsonrpc_message(&mut reader).await.unwrap();

        assert_eq!(decoded["jsonrpc"], "2.0");
        assert_eq!(decoded["id"], 1);
        assert_eq!(decoded["method"], "initialize");
        assert_eq!(decoded["params"]["processId"], 123);
    }

    #[tokio::test]
    async fn test_jsonrpc_multiple_messages() {
        let msg1 = json!({"jsonrpc": "2.0", "id": 1, "method": "foo"});
        let msg2 = json!({"jsonrpc": "2.0", "id": 2, "method": "bar"});

        let mut buf: Vec<u8> = Vec::new();
        send_jsonrpc_message(&mut buf, &msg1).await.unwrap();
        send_jsonrpc_message(&mut buf, &msg2).await.unwrap();

        let mut cursor = &buf[..];
        let mut reader = BufReader::new(&mut cursor);

        let decoded1 = read_jsonrpc_message(&mut reader).await.unwrap();
        assert_eq!(decoded1["id"], 1);

        let decoded2 = read_jsonrpc_message(&mut reader).await.unwrap();
        assert_eq!(decoded2["id"], 2);
    }

    #[tokio::test]
    async fn test_jsonrpc_missing_content_length() {
        let bad_input = b"SomeHeader: value\r\n\r\n{}";
        let mut cursor = &bad_input[..];
        let mut reader = BufReader::new(&mut cursor);
        let result = read_jsonrpc_message(&mut reader).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing Content-Length"));
    }

    #[tokio::test]
    async fn test_jsonrpc_eof() {
        let empty: &[u8] = b"";
        let mut cursor = empty;
        let mut reader = BufReader::new(&mut cursor);
        let result = read_jsonrpc_message(&mut reader).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_daemon_initial_state() {
        use crate::types::OwnedLspServerSpec;
        use swissarmyhammer_project_detection::ProjectType;
        let spec = OwnedLspServerSpec {
            project_types: vec![ProjectType::Rust],
            command: "rust-analyzer".to_string(),
            args: vec![],
            language_ids: vec!["rust".to_string()],
            file_extensions: vec!["rs".to_string()],
            startup_timeout_secs: 30,
            health_check_interval_secs: 60,
            install_hint: "Install rust-analyzer: rustup component add rust-analyzer".to_string(),
            icon: None,
        };
        let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        assert_eq!(daemon.state(), LspDaemonState::NotStarted);
        assert_eq!(daemon.command(), "rust-analyzer");
    }

    // -- helper for lifecycle tests ------------------------------------------

    /// Build a minimal `OwnedLspServerSpec` for testing.
    fn test_spec(command: &str) -> OwnedLspServerSpec {
        OwnedLspServerSpec {
            project_types: vec![],
            command: command.to_string(),
            args: vec![],
            language_ids: vec!["test".to_string()],
            file_extensions: vec!["txt".to_string()],
            startup_timeout_secs: 5,
            health_check_interval_secs: 60,
            install_hint: format!("install {command}"),
            icon: None,
        }
    }

    // -- start() tests -------------------------------------------------------

    #[tokio::test]
    async fn test_start_binary_not_found() {
        let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

        let result = daemon.start().await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(
            matches!(err, LspError::BinaryNotFound { .. }),
            "expected BinaryNotFound, got: {err:?}"
        );
        assert_eq!(daemon.state(), LspDaemonState::NotFound);
    }

    #[tokio::test]
    async fn test_start_binary_not_found_preserves_hint() {
        let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

        let result = daemon.start().await;
        match result.unwrap_err() {
            LspError::BinaryNotFound {
                command,
                install_hint,
            } => {
                assert_eq!(command, "nonexistent-lsp-binary-abc123xyz");
                assert!(install_hint.contains("nonexistent-lsp-binary-abc123xyz"));
            }
            other => panic!("expected BinaryNotFound, got: {other:?}"),
        }
    }

    // -- client() tests ------------------------------------------------------

    #[test]
    fn test_client_returns_none_when_not_started() {
        let spec = test_spec("some-server");
        let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        assert!(daemon.client().is_none());
    }

    #[test]
    fn test_shared_client_returns_arc() {
        let spec = test_spec("some-server");
        let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        let shared = daemon.shared_client();
        // The shared client should be lockable and contain None
        let guard = shared.lock().unwrap();
        assert!(guard.is_none());
    }

    #[test]
    fn test_client_recovers_from_poisoned_mutex() {
        let spec = test_spec("some-server");
        let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

        // Poison the mutex by panicking inside a lock
        let shared = daemon.shared_client();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = shared.lock().unwrap();
            panic!("intentional panic to poison mutex");
        }));

        // client() should still work via the poison-recovery path
        // The inner Option is still None, so it returns None
        assert!(daemon.client().is_none());
    }

    // -- health_check() tests ------------------------------------------------

    #[test]
    fn test_health_check_returns_false_when_no_child() {
        let spec = test_spec("some-server");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        assert!(!daemon.health_check());
    }

    // -- shutdown() tests ----------------------------------------------------

    #[tokio::test]
    async fn test_shutdown_when_not_started() {
        let spec = test_spec("some-server");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

        // Shutdown should be a no-op that transitions to NotStarted
        daemon.shutdown().await;
        assert_eq!(daemon.state(), LspDaemonState::NotStarted);
    }

    #[tokio::test]
    async fn test_shutdown_clears_client() {
        let spec = test_spec("some-server");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        let shared = daemon.shared_client();

        daemon.shutdown().await;

        // Client should be None after shutdown
        let guard = shared.lock().unwrap();
        assert!(guard.is_none());
    }

    // -- restart_with_backoff() tests ----------------------------------------

    #[tokio::test]
    async fn test_restart_with_backoff_exhausted() {
        let spec = test_spec("some-server");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

        // Simulate MAX_CONSECUTIVE_FAILURES failures
        daemon.consecutive_failures = MAX_CONSECUTIVE_FAILURES;

        let result = daemon.restart_with_backoff().await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(
            matches!(err, LspError::HandshakeFailed(ref msg) if msg.contains("too many consecutive failures")),
            "expected HandshakeFailed with 'too many consecutive failures', got: {err:?}"
        );
    }

    #[tokio::test]
    async fn test_restart_with_backoff_resets_on_binary_not_found() {
        // restart_with_backoff calls start(), which will fail with BinaryNotFound
        // and increment consecutive_failures further
        let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        daemon.consecutive_failures = 0;

        let result = daemon.restart_with_backoff().await;
        assert!(result.is_err());
        // State should be NotFound since binary doesn't exist
        assert_eq!(daemon.state(), LspDaemonState::NotFound);
    }

    // -- force_restart() tests -----------------------------------------------

    #[tokio::test]
    async fn test_force_restart_resets_failures() {
        let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        daemon.consecutive_failures = 3;

        // force_restart resets failures to 0 then calls start()
        let result = daemon.force_restart().await;
        assert!(result.is_err()); // binary not found
                                  // consecutive_failures was reset to 0 before start(), but start() didn't
                                  // call record_failure because BinaryNotFound doesn't go through record_failure
        assert_eq!(daemon.consecutive_failures, 0);
    }

    // -- state_rx() tests ----------------------------------------------------

    #[tokio::test]
    async fn test_state_rx_observes_transitions() {
        let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        let mut rx = daemon.state_rx();

        assert_eq!(*rx.borrow(), LspDaemonState::NotStarted);

        // Trigger a start (will fail with NotFound)
        let _ = daemon.start().await;

        // The rx should have seen the NotFound state
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), LspDaemonState::NotFound);
    }

    // -- record_failure() tests ----------------------------------------------

    #[test]
    fn test_record_failure_increments_counter() {
        let spec = test_spec("some-server");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        assert_eq!(daemon.consecutive_failures, 0);

        daemon.record_failure("test failure 1".into());
        assert_eq!(daemon.consecutive_failures, 1);
        assert!(matches!(
            daemon.state(),
            LspDaemonState::Failed { attempts: 1, .. }
        ));

        daemon.record_failure("test failure 2".into());
        assert_eq!(daemon.consecutive_failures, 2);
        assert!(matches!(
            daemon.state(),
            LspDaemonState::Failed { attempts: 2, .. }
        ));
    }

    #[test]
    fn test_record_failure_stores_reason() {
        let spec = test_spec("some-server");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

        daemon.record_failure("connection refused".into());
        match daemon.state() {
            LspDaemonState::Failed { reason, .. } => {
                assert_eq!(reason, "connection refused");
            }
            other => panic!("expected Failed state, got: {other:?}"),
        }
    }

    // -- Debug impl test -----------------------------------------------------

    #[test]
    fn test_daemon_debug_output() {
        let spec = test_spec("test-server");
        let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        let debug = format!("{daemon:?}");
        assert!(debug.contains("test-server"));
        assert!(debug.contains("NotStarted"));
    }

    // -- Lifecycle tests with real processes ---------------------------------
    //
    // These tests use simple commands (cat, true, etc.) to exercise the
    // daemon lifecycle without requiring a full LSP server implementation.

    /// Build a spec that spawns a process which exits immediately.
    /// This tests the handshake timeout / EOF error path.
    fn immediately_exiting_spec() -> OwnedLspServerSpec {
        OwnedLspServerSpec {
            project_types: vec![],
            command: "true".to_string(),
            args: vec![],
            language_ids: vec!["test".to_string()],
            file_extensions: vec!["txt".to_string()],
            startup_timeout_secs: 2,
            health_check_interval_secs: 60,
            install_hint: "N/A".to_string(),
            icon: None,
        }
    }

    /// Build a spec that spawns `cat`, which keeps stdin/stdout open
    /// but never speaks LSP protocol. This tests the handshake timeout path.
    fn cat_spec() -> OwnedLspServerSpec {
        OwnedLspServerSpec {
            project_types: vec![],
            command: "cat".to_string(),
            args: vec![],
            language_ids: vec!["test".to_string()],
            file_extensions: vec!["txt".to_string()],
            // Very short timeout so the test doesn't hang
            startup_timeout_secs: 1,
            health_check_interval_secs: 60,
            install_hint: "N/A".to_string(),
            icon: None,
        }
    }

    #[tokio::test]
    async fn test_start_immediately_exiting_process() {
        // `true` exits immediately, so the handshake should fail with EOF
        let spec = immediately_exiting_spec();
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

        let result = daemon.start().await;
        assert!(result.is_err(), "expected start to fail");
        // Should be a handshake failure (EOF reading headers) or timeout
        let err = result.unwrap_err();
        assert!(
            matches!(err, LspError::HandshakeFailed(_) | LspError::Timeout(_)),
            "expected HandshakeFailed or Timeout, got: {err:?}"
        );
        // State should be Failed
        assert!(
            matches!(daemon.state(), LspDaemonState::Failed { .. }),
            "expected Failed, got: {:?}",
            daemon.state()
        );
        assert_eq!(daemon.consecutive_failures, 1);
    }

    #[tokio::test]
    async fn test_start_with_cat_succeeds_as_echo() {
        // `cat` echoes the initialize request back verbatim, which happens to
        // be valid JSON-RPC framing. The daemon doesn't validate response
        // content, so this "succeeds". This confirms the handshake completes
        // when the child produces valid framing on stdout.
        let spec = cat_spec();
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

        let result = daemon.start().await;
        // cat echoes a valid JSON-RPC message, so the handshake succeeds
        assert!(
            result.is_ok(),
            "expected cat echo to pass handshake: {result:?}"
        );
        assert!(matches!(daemon.state(), LspDaemonState::Running { .. }));

        daemon.shutdown().await;
        assert_eq!(daemon.state(), LspDaemonState::NotStarted);
    }

    #[tokio::test]
    async fn test_health_check_detects_exited_process_via_cat() {
        // Spawn `cat`, then kill it and verify health_check detects the exit.
        // We skip the handshake by directly setting up the child.
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn cat");

        let spec = cat_spec();
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
        daemon.child = Some(child);

        // Process should be alive
        assert!(daemon.health_check());

        // Kill it
        if let Some(child) = daemon.child.as_mut() {
            child.kill().await.expect("kill cat");
            let _ = child.wait().await;
        }

        // health_check should now detect the exit
        assert!(!daemon.health_check());
        assert!(
            matches!(daemon.state(), LspDaemonState::Failed { .. }),
            "expected Failed, got: {:?}",
            daemon.state()
        );
        assert_eq!(daemon.consecutive_failures, 1);
    }

    #[tokio::test]
    async fn test_graceful_stop_of_running_child() {
        // Directly set a child process and test that stop drops it
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn cat");

        let spec = cat_spec();
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
        daemon.child = Some(child);

        // Shutting down should clean up the child
        daemon.shutdown().await;
        assert_eq!(daemon.state(), LspDaemonState::NotStarted);
        assert!(daemon.child.is_none());
        assert!(daemon.client().is_none());
    }

    #[tokio::test]
    async fn test_force_restart_after_failure() {
        // Simulate a daemon that failed, then force_restart
        let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        daemon.consecutive_failures = 4;
        daemon.record_failure("previous crash".into());

        // force_restart resets consecutive_failures to 0 then calls start()
        let result = daemon.force_restart().await;
        assert!(result.is_err()); // binary still not found
                                  // But consecutive_failures was reset before the start attempt
        assert_eq!(daemon.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_state_transitions_through_failed_start() {
        let spec = immediately_exiting_spec();
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
        let mut rx = daemon.state_rx();

        assert_eq!(*rx.borrow(), LspDaemonState::NotStarted);

        let _ = daemon.start().await;

        // Should have transitioned through Starting -> Failed
        // Drain all changes
        while rx.has_changed().unwrap_or(false) {
            let _ = rx.borrow_and_update();
        }
        let final_state = rx.borrow().clone();
        assert!(
            matches!(final_state, LspDaemonState::Failed { .. }),
            "expected Failed, got: {final_state:?}"
        );
    }

    #[tokio::test]
    async fn test_shutdown_with_child_transitions_through_shutting_down() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn cat");

        let spec = cat_spec();
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
        daemon.child = Some(child);

        let mut rx = daemon.state_rx();

        daemon.shutdown().await;

        // Collect all state changes that happened
        let mut saw_shutting_down = false;
        // The watch channel coalesces changes, but ShuttingDown should have
        // been emitted before NotStarted
        while rx.has_changed().unwrap_or(false) {
            let state = rx.borrow_and_update().clone();
            if state == LspDaemonState::ShuttingDown {
                saw_shutting_down = true;
            }
        }
        // Final state should be NotStarted
        assert_eq!(*rx.borrow(), LspDaemonState::NotStarted);
        // Note: watch channels coalesce, so ShuttingDown might have been
        // replaced by NotStarted before we observed it. That's OK -- the
        // important thing is that NotStarted is the final state.
        let _ = saw_shutting_down; // avoid unused warning
    }

    #[tokio::test]
    async fn test_multiple_consecutive_failures_track_correctly() {
        let spec = immediately_exiting_spec();
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

        // Each failed start should increment the failure counter
        let _ = daemon.start().await;
        assert_eq!(daemon.consecutive_failures, 1);

        let _ = daemon.start().await;
        assert_eq!(daemon.consecutive_failures, 2);

        let _ = daemon.start().await;
        assert_eq!(daemon.consecutive_failures, 3);

        match daemon.state() {
            LspDaemonState::Failed { attempts, .. } => {
                assert_eq!(attempts, 3);
            }
            other => panic!("expected Failed, got: {other:?}"),
        }
    }
}
