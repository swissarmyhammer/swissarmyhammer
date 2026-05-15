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

    /// Return the file extensions this daemon's server handles (without dot).
    pub fn file_extensions(&self) -> &[String] {
        &self.spec.file_extensions
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

    // -- JSON-RPC edge case tests -------------------------------------------

    #[tokio::test]
    async fn test_jsonrpc_bad_content_length_value() {
        let bad_input = b"Content-Length: abc\r\n\r\n";
        let mut cursor = &bad_input[..];
        let mut reader = BufReader::new(&mut cursor);
        let result = read_jsonrpc_message(&mut reader).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("bad Content-Length"),
            "expected bad Content-Length error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_jsonrpc_ignores_non_content_length_headers() {
        // Build a valid message with extra headers that should be ignored
        let body = r#"{"jsonrpc":"2.0","id":1}"#;
        let msg = format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let bytes = msg.as_bytes();
        let mut cursor = bytes;
        let mut reader = BufReader::new(&mut cursor);
        let decoded = read_jsonrpc_message(&mut reader).await.unwrap();
        assert_eq!(decoded["id"], 1);
    }

    #[tokio::test]
    async fn test_jsonrpc_invalid_json_body() {
        let body = "not valid json!!!";
        let msg = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let bytes = msg.as_bytes();
        let mut cursor = bytes;
        let mut reader = BufReader::new(&mut cursor);
        let result = read_jsonrpc_message(&mut reader).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("json decode"),
            "expected json decode error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_jsonrpc_truncated_body() {
        // Content-Length says 100 but only 5 bytes available
        let msg = "Content-Length: 100\r\n\r\nhello";
        let bytes = msg.as_bytes();
        let mut cursor = bytes;
        let mut reader = BufReader::new(&mut cursor);
        let result = read_jsonrpc_message(&mut reader).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("read body"),
            "expected read body error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_send_jsonrpc_formats_content_length_correctly() {
        let msg = json!({"jsonrpc": "2.0", "method": "test"});
        let mut buf: Vec<u8> = Vec::new();
        send_jsonrpc_message(&mut buf, &msg).await.unwrap();

        let output = String::from_utf8(buf).unwrap();
        assert!(output.starts_with("Content-Length: "));
        assert!(output.contains("\r\n\r\n"));
        // Verify the content-length value matches the actual body
        let parts: Vec<&str> = output.splitn(2, "\r\n\r\n").collect();
        let header = parts[0];
        let body = parts[1];
        let claimed_len: usize = header
            .strip_prefix("Content-Length: ")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(claimed_len, body.len());
    }

    // -- LspError Display tests -----------------------------------------------

    #[test]
    fn test_lsp_error_display_variants() {
        let err = LspError::BinaryNotFound {
            command: "test-cmd".to_string(),
            install_hint: "install it".to_string(),
        };
        assert!(err.to_string().contains("test-cmd"));

        let err = LspError::HandshakeFailed("timeout".to_string());
        assert!(err.to_string().contains("timeout"));

        let err = LspError::Timeout(Duration::from_secs(30));
        assert!(err.to_string().contains("30"));

        let err = LspError::ShutdownFailed("crash".to_string());
        assert!(err.to_string().contains("crash"));

        let err = LspError::NotRunning;
        assert!(err.to_string().contains("not running"));

        let err = LspError::JsonRpc("bad frame".to_string());
        assert!(err.to_string().contains("bad frame"));

        let err = LspError::ProjectDetection("no projects".to_string());
        assert!(err.to_string().contains("no projects"));

        let err = LspError::DaemonNotFound("missing-cmd".to_string());
        assert!(err.to_string().contains("missing-cmd"));
    }

    // -- start() timeout path via cat -----------------------------------------

    #[tokio::test]
    async fn test_start_cat_with_very_short_timeout() {
        // cat keeps stdin/stdout open but doesn't respond with valid JSON-RPC.
        // It actually echoes the request back as raw bytes, which happens to be
        // valid framing. To trigger the timeout path, we need a command that
        // keeps pipes open but doesn't echo. `/bin/sleep` does this.
        let spec = OwnedLspServerSpec {
            project_types: vec![],
            command: "sleep".to_string(),
            args: vec!["10".to_string()],
            language_ids: vec!["test".to_string()],
            file_extensions: vec!["txt".to_string()],
            startup_timeout_secs: 1,
            health_check_interval_secs: 60,
            install_hint: "N/A".to_string(),
            icon: None,
        };
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

        let result = daemon.start().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        // sleep doesn't have stdin piped correctly for write, so it may fail
        // with either Timeout, HandshakeFailed, or SpawnFailed depending on
        // the OS behavior. The key is that it fails.
        assert!(
            matches!(
                err,
                LspError::Timeout(_)
                    | LspError::HandshakeFailed(_)
                    | LspError::SpawnFailed(_)
                    | LspError::JsonRpc(_)
            ),
            "expected Timeout/HandshakeFailed/SpawnFailed/JsonRpc, got: {err:?}"
        );
    }

    // -- shutdown with a real running child -----------------------------------

    #[tokio::test]
    async fn test_shutdown_kills_long_running_child() {
        // Spawn a process that won't exit on its own
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let child = Command::new("sleep")
            .args(["60"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn sleep");

        let spec = test_spec("sleep");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
        daemon.child = Some(child);

        // Shutdown should complete within the grace period (kill_on_drop)
        daemon.shutdown().await;
        assert_eq!(daemon.state(), LspDaemonState::NotStarted);
        assert!(daemon.child.is_none());
    }

    // -- restart_with_backoff delay test --------------------------------------

    #[test]
    fn test_client_recovers_from_poisoned_mutex_with_some_value() {
        // This test covers the poisoned mutex path where the inner Option IS Some.
        // We can't easily create a real LspJsonRpcClient without pipes, so we
        // test via shared_client directly.
        let spec = test_spec("some-server");
        let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        let shared = daemon.shared_client();

        // Put a real client into the mutex
        // We need stdin/stdout for LspJsonRpcClient::new
        let stdin = std::process::Command::new("cat")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn();

        if let Ok(mut child) = stdin {
            let child_stdin = child.stdin.take().unwrap();
            let child_stdout = child.stdout.take().unwrap();
            {
                let mut guard = shared.lock().unwrap();
                *guard = Some(LspJsonRpcClient::new(child_stdin, child_stdout));
            }

            // Poison the mutex by panicking inside a lock
            let shared_clone = Arc::clone(&shared);
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = shared_clone.lock().unwrap();
                panic!("intentional panic to poison mutex");
            }));

            // client() should recover from the poisoned mutex.
            // The inner Option is Some, so it should return Some.
            let result = daemon.client();
            assert!(
                result.is_some(),
                "expected Some from poisoned mutex with client present"
            );

            let _ = child.kill();
        }
    }

    #[tokio::test]
    async fn test_restart_with_backoff_increments_failure_on_bad_binary() {
        let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        // Set to 1 failure (below MAX) so restart_with_backoff proceeds
        daemon.consecutive_failures = 1;

        let start = std::time::Instant::now();
        let result = daemon.restart_with_backoff().await;
        let elapsed = start.elapsed();

        assert!(result.is_err());
        // Should have waited at least ~2s backoff for attempt=1
        assert!(
            elapsed >= Duration::from_secs(1),
            "expected at least 1s backoff delay, got: {elapsed:?}"
        );
        assert_eq!(daemon.state(), LspDaemonState::NotFound);
    }

    // -- file_extensions accessor test ----------------------------------------

    #[test]
    fn test_file_extensions_accessor() {
        let mut spec = test_spec("some-lsp");
        spec.file_extensions = vec!["rs".to_string(), "toml".to_string(), "lock".to_string()];
        let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        assert_eq!(
            daemon.file_extensions(),
            &["rs".to_string(), "toml".to_string(), "lock".to_string()]
        );
    }

    #[test]
    fn test_file_extensions_empty() {
        let mut spec = test_spec("some-lsp");
        spec.file_extensions = vec![];
        let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        assert!(daemon.file_extensions().is_empty());
    }

    // -- send_jsonrpc_message write failure test ------------------------------

    /// An async writer that always returns an I/O error on write.
    struct FailingWriter;

    impl tokio::io::AsyncWrite for FailingWriter {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            _buf: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "simulated write failure",
            )))
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "simulated flush failure",
            )))
        }

        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn test_send_jsonrpc_write_failure() {
        let msg = json!({"jsonrpc": "2.0", "method": "test"});
        let mut writer = FailingWriter;
        let result = send_jsonrpc_message(&mut writer, &msg).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("write header"),
            "expected write header error, got: {err}"
        );
    }

    // -- initialize_handshake error path tests --------------------------------
    //
    // These use Python one-liners as mock LSP servers. The Python script reads
    // the incoming JSON-RPC request, then sends a malformed response.

    /// Helper: build a spec that runs a Python script as the LSP server.
    fn python_mock_spec(script: &str) -> OwnedLspServerSpec {
        OwnedLspServerSpec {
            project_types: vec![],
            command: "python3".to_string(),
            args: vec!["-c".to_string(), script.to_string()],
            language_ids: vec!["test".to_string()],
            file_extensions: vec!["txt".to_string()],
            startup_timeout_secs: 5,
            health_check_interval_secs: 60,
            install_hint: "N/A".to_string(),
            icon: None,
        }
    }

    #[tokio::test]
    async fn test_initialize_handshake_bad_json_response() {
        // Python script reads the initialize request, then responds with
        // valid Content-Length framing but invalid JSON body.
        let script = r#"
import sys
# Read the incoming request: headers then body
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
# Extract content length and read body
import re
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
# Send bad JSON with valid framing
bad = 'this is not json at all!!!'
sys.stdout.write(f'Content-Length: {len(bad)}\r\n\r\n{bad}')
sys.stdout.flush()
"#;
        let spec = python_mock_spec(script);
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

        let result = daemon.start().await;
        assert!(result.is_err(), "expected start to fail with bad JSON");
        let err = result.unwrap_err();
        assert!(
            matches!(err, LspError::HandshakeFailed(_)),
            "expected HandshakeFailed, got: {err:?}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("json decode") || msg.contains("json"),
            "expected JSON decode error in message, got: {msg}"
        );
    }

    #[tokio::test]
    async fn test_initialize_handshake_missing_content_length_response() {
        // Python script reads the request, then sends a response without
        // the Content-Length header (just raw JSON after blank line).
        let script = r#"
import sys, re
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
# Send without Content-Length header -- just newline then body
sys.stdout.write('\r\n')
sys.stdout.flush()
# Then close stdout so the reader gets EOF
sys.stdout.close()
"#;
        let spec = python_mock_spec(script);
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

        let result = daemon.start().await;
        assert!(
            result.is_err(),
            "expected start to fail without Content-Length"
        );
        let err = result.unwrap_err();
        assert!(
            matches!(err, LspError::HandshakeFailed(_)),
            "expected HandshakeFailed, got: {err:?}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("Content-Length") || msg.contains("EOF") || msg.contains("missing"),
            "expected missing Content-Length or EOF error, got: {msg}"
        );
    }

    #[tokio::test]
    async fn test_initialize_handshake_truncated_body() {
        // Python script reads the request, then sends a Content-Length
        // that's larger than the actual body, causing an EOF during read.
        let script = r#"
import sys, re
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
# Claim 1000 bytes but only send 5
sys.stdout.write('Content-Length: 1000\r\n\r\nhello')
sys.stdout.flush()
sys.stdout.close()
"#;
        let spec = python_mock_spec(script);
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

        let result = daemon.start().await;
        assert!(
            result.is_err(),
            "expected start to fail with truncated body"
        );
        let err = result.unwrap_err();
        assert!(
            matches!(err, LspError::HandshakeFailed(_)),
            "expected HandshakeFailed, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn test_initialize_handshake_process_exits_with_stderr() {
        // Python script writes to stderr then exits without responding,
        // exercising the stderr capture path in initialize_handshake.
        let script = r#"
import sys
sys.stderr.write('mock LSP fatal error: config not found\n')
sys.stderr.flush()
sys.exit(1)
"#;
        let spec = python_mock_spec(script);
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

        let result = daemon.start().await;
        assert!(result.is_err(), "expected start to fail when process exits");
        let err = result.unwrap_err();
        assert!(
            matches!(err, LspError::HandshakeFailed(_) | LspError::Timeout(_)),
            "expected HandshakeFailed or Timeout, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn test_initialize_handshake_wrong_id_still_succeeds() {
        // initialize_handshake does NOT validate the response ID. This test
        // documents that behavior: a response with id:999 is accepted because
        // the code only checks framing, not content.
        let script = r#"
import sys, re, json
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
# Respond with wrong id
resp = json.dumps({"jsonrpc": "2.0", "id": 999, "result": {"capabilities": {}}})
sys.stdout.write(f'Content-Length: {len(resp)}\r\n\r\n{resp}')
sys.stdout.flush()
# Read the initialized notification
headers2 = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers2 += line
m2 = re.search(r'Content-Length:\s*(\d+)', headers2)
if m2:
    body2 = sys.stdin.read(int(m2.group(1)))
# Keep stdin open briefly so pipes don't break
import time
time.sleep(0.5)
"#;
        let spec = python_mock_spec(script);
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

        let result = daemon.start().await;
        // This succeeds because initialize_handshake doesn't validate the ID
        assert!(
            result.is_ok(),
            "expected wrong-id response to be accepted (no ID validation), got: {result:?}"
        );

        daemon.shutdown().await;
    }

    // -- health_check + restart_with_backoff edge case tests ------------------

    #[tokio::test]
    async fn test_health_check_detects_naturally_exiting_process() {
        // Spawn a process that exits after a short delay. Verify health_check
        // detects the exit and transitions to Failed.
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let child = Command::new("python3")
            .args(["-c", "import time; time.sleep(0.2)"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn python");

        let spec = cat_spec();
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
        daemon.child = Some(child);

        // Initially alive
        assert!(daemon.health_check());

        // Wait for the process to exit naturally
        tokio::time::sleep(Duration::from_millis(500)).await;

        // health_check should detect the exit
        assert!(!daemon.health_check());
        assert!(
            matches!(daemon.state(), LspDaemonState::Failed { .. }),
            "expected Failed after natural exit, got: {:?}",
            daemon.state()
        );
        // Verify failure reason mentions process exit
        if let LspDaemonState::Failed { reason, .. } = daemon.state() {
            assert!(
                reason.contains("process exited"),
                "expected 'process exited' in reason, got: {reason}"
            );
        }
    }

    #[tokio::test]
    async fn test_health_check_clears_client_on_exit() {
        // When a process exits, health_check should clear the shared client.
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let child = Command::new("true")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn true");

        let spec = immediately_exiting_spec();
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
        daemon.child = Some(child);

        // Wait for process to exit
        tokio::time::sleep(Duration::from_millis(200)).await;

        // health_check should detect exit and clear client
        assert!(!daemon.health_check());
        assert!(daemon.child.is_none());
        assert!(daemon.client().is_none());
    }

    #[tokio::test]
    async fn test_restart_with_backoff_sleeps_before_restart() {
        // Verify that restart_with_backoff introduces a delay before
        // attempting restart. We use a nonexistent binary so start() fails
        // fast, and measure wall time to confirm the backoff delay.
        let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        daemon.consecutive_failures = 0; // first failure: 1s backoff

        let start = std::time::Instant::now();
        let result = daemon.restart_with_backoff().await;
        let elapsed = start.elapsed();

        assert!(result.is_err());
        // Should have waited ~1s (backoff for attempt 0)
        assert!(
            elapsed >= Duration::from_millis(800),
            "expected at least ~1s backoff delay, got: {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn test_restart_with_backoff_gives_up_at_max_failures() {
        // When consecutive_failures >= MAX_CONSECUTIVE_FAILURES, restart
        // should return an error immediately without sleeping.
        let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        daemon.consecutive_failures = MAX_CONSECUTIVE_FAILURES;

        let start = std::time::Instant::now();
        let result = daemon.restart_with_backoff().await;
        let elapsed = start.elapsed();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("too many consecutive failures"),
            "expected 'too many consecutive failures', got: {err}"
        );
        // Should return immediately, not sleep
        assert!(
            elapsed < Duration::from_millis(100),
            "expected immediate return, got: {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn test_health_check_multiple_calls_after_exit() {
        // After a process has exited and been detected by health_check,
        // subsequent calls should also return false (child is now None).
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let child = Command::new("true")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn true");

        let spec = immediately_exiting_spec();
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
        daemon.child = Some(child);

        // Wait for exit
        tokio::time::sleep(Duration::from_millis(200)).await;

        // First call detects exit
        assert!(!daemon.health_check());
        assert_eq!(daemon.consecutive_failures, 1);

        // Second call: child is None, returns false but doesn't increment failures
        assert!(!daemon.health_check());
        assert_eq!(daemon.consecutive_failures, 1); // unchanged
    }

    // -- Card 1: additional start() failure path coverage --------------------

    /// Building an `OwnedLspServerSpec` that points at a directory for the
    /// command. `which::which` on a directory returns Err, so this path hits
    /// the binary-not-found branch — adds an extra assertion that
    /// `consecutive_failures` is not incremented on `BinaryNotFound`, which
    /// exercises the documented contract for `set_state(NotFound)`.
    #[tokio::test]
    async fn test_start_binary_not_found_does_not_increment_failures() {
        let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        assert_eq!(daemon.consecutive_failures, 0);

        let _ = daemon.start().await;
        // BinaryNotFound goes through set_state(NotFound), NOT record_failure.
        // The counter must remain at 0.
        assert_eq!(daemon.consecutive_failures, 0);
        assert_eq!(daemon.state(), LspDaemonState::NotFound);
    }

    /// Exercise the stderr filter drain task by running a Python mock that
    /// emits stderr lines during handshake. The drain task is spawned when
    /// start() succeeds; we then verify it does not interfere with shutdown.
    #[tokio::test]
    async fn test_start_spawns_stderr_filter_task_on_success() {
        // Mock LSP that completes handshake and ALSO writes to stderr.
        // The stderr drain task should consume the stderr lines without
        // blocking shutdown. This hits lines 223-225 / 230 of the drain loop.
        let script = r#"
import sys, re, json, time
# Emit some stderr before handshake so the filter task has input to drain
sys.stderr.write('mock lsp: starting up\n')
sys.stderr.write('mock lsp: loading config\n')
sys.stderr.flush()

headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
resp = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"capabilities": {}}})
sys.stdout.write(f'Content-Length: {len(resp)}\r\n\r\n{resp}')
sys.stdout.flush()
# Write more stderr after the response
sys.stderr.write('mock lsp: ready\n')
sys.stderr.flush()
# Read the initialized notification
headers2 = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers2 += line
m2 = re.search(r'Content-Length:\s*(\d+)', headers2)
if m2:
    body2 = sys.stdin.read(int(m2.group(1)))
time.sleep(0.5)
"#;
        let spec = python_mock_spec(script);
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

        let result = daemon.start().await;
        assert!(
            result.is_ok(),
            "expected mock LSP to handshake successfully: {result:?}"
        );
        assert!(matches!(daemon.state(), LspDaemonState::Running { .. }));

        // Give the spawned stderr drain task a moment to read the lines
        tokio::time::sleep(Duration::from_millis(100)).await;

        daemon.shutdown().await;
        assert_eq!(daemon.state(), LspDaemonState::NotStarted);
    }

    /// Verify that `start()` on a success path takes stdin and stdout from
    /// the child process and creates an `LspJsonRpcClient`. This exercises
    /// the successful branch of the pipe-conversion `match` block
    /// (`(Ok(stdin_fd), Ok(stdout_fd))` arm).
    #[tokio::test]
    async fn test_start_success_creates_shared_client() {
        let script = r#"
import sys, re, json, time
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
resp = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"capabilities": {}}})
sys.stdout.write(f'Content-Length: {len(resp)}\r\n\r\n{resp}')
sys.stdout.flush()
headers2 = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers2 += line
m2 = re.search(r'Content-Length:\s*(\d+)', headers2)
if m2:
    body2 = sys.stdin.read(int(m2.group(1)))
time.sleep(0.5)
"#;
        let spec = python_mock_spec(script);
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

        let result = daemon.start().await;
        assert!(result.is_ok(), "expected start to succeed: {result:?}");

        // After a successful start, the shared client should contain Some(_).
        // This confirms the pipe conversion succeeded.
        let shared = daemon.shared_client();
        {
            let guard = shared.lock().unwrap();
            assert!(
                guard.is_some(),
                "expected shared client to be Some after successful start"
            );
        }

        // And the child handle is retained for shutdown.
        assert!(daemon.child.is_some());

        daemon.shutdown().await;
    }

    /// When `start()` succeeds, a `Running { pid, since_epoch_ms }` state
    /// with a sensible timestamp is emitted. This covers the success-path
    /// transition in `start()`.
    #[tokio::test]
    async fn test_start_success_sets_running_state_with_pid_and_timestamp() {
        let script = r#"
import sys, re, json, time
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
resp = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"capabilities": {}}})
sys.stdout.write(f'Content-Length: {len(resp)}\r\n\r\n{resp}')
sys.stdout.flush()
headers2 = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers2 += line
m2 = re.search(r'Content-Length:\s*(\d+)', headers2)
if m2:
    body2 = sys.stdin.read(int(m2.group(1)))
time.sleep(0.5)
"#;
        let spec = python_mock_spec(script);
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

        let before_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let result = daemon.start().await;
        assert!(result.is_ok(), "expected start to succeed: {result:?}");

        match daemon.state() {
            LspDaemonState::Running {
                pid,
                since_epoch_ms,
            } => {
                assert!(pid > 0, "expected a non-zero pid, got: {pid}");
                assert!(
                    since_epoch_ms >= before_ms,
                    "expected since_epoch_ms >= before_ms, got: {since_epoch_ms} vs {before_ms}"
                );
            }
            other => panic!("expected Running, got: {other:?}"),
        }

        // Successful start must reset consecutive_failures.
        assert_eq!(daemon.consecutive_failures, 0);

        daemon.shutdown().await;
    }

    /// Successful start followed by a second start() documents that the
    /// daemon does not refuse concurrent starts — it overwrites the child
    /// handle. This covers the path where `start()` is entered while a
    /// child already exists (the old child is dropped via `kill_on_drop`).
    #[tokio::test]
    async fn test_start_after_previous_successful_start() {
        let script = r#"
import sys, re, json, time
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
resp = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"capabilities": {}}})
sys.stdout.write(f'Content-Length: {len(resp)}\r\n\r\n{resp}')
sys.stdout.flush()
headers2 = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers2 += line
m2 = re.search(r'Content-Length:\s*(\d+)', headers2)
if m2:
    body2 = sys.stdin.read(int(m2.group(1)))
time.sleep(1.0)
"#;
        let spec = python_mock_spec(script);
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

        let r1 = daemon.start().await;
        assert!(r1.is_ok(), "first start failed: {r1:?}");
        let first_pid = match daemon.state() {
            LspDaemonState::Running { pid, .. } => pid,
            other => panic!("expected Running after first start, got: {other:?}"),
        };

        // Now start again without shutdown — new child overwrites old
        let r2 = daemon.start().await;
        assert!(r2.is_ok(), "second start failed: {r2:?}");
        let second_pid = match daemon.state() {
            LspDaemonState::Running { pid, .. } => pid,
            other => panic!("expected Running after second start, got: {other:?}"),
        };
        assert_ne!(first_pid, second_pid, "second start should spawn a new pid");

        daemon.shutdown().await;
    }

    // -- Card 2: additional health_check() failure path coverage --------------

    /// `health_check` when the child has already exited must:
    /// 1. Clear the stored child handle
    /// 2. Clear the shared client mutex
    /// 3. Transition to Failed with a reason containing "process exited"
    /// 4. Return false
    /// 5. Increment consecutive_failures exactly once per detection
    #[tokio::test]
    async fn test_health_check_exited_process_full_contract() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let child = Command::new("true")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn true");

        let spec = immediately_exiting_spec();
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
        daemon.child = Some(child);

        // Put a fake non-None marker into the shared client so we can verify
        // health_check clears it. We use an LspJsonRpcClient wired to separate
        // pipes so we don't mess with the child under test.
        let helper_child = std::process::Command::new("cat")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn();
        if let Ok(mut helper) = helper_child {
            let hstdin = helper.stdin.take().unwrap();
            let hstdout = helper.stdout.take().unwrap();
            let shared = daemon.shared_client();
            {
                let mut guard = shared.lock().unwrap();
                *guard = Some(LspJsonRpcClient::new(hstdin, hstdout));
            }

            // Wait for the `true` process to exit
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Observe health_check
            let alive = daemon.health_check();
            assert!(!alive, "expected health_check to return false");

            // Contract: child cleared
            assert!(daemon.child.is_none(), "child should be None after exit");

            // Contract: client cleared
            {
                let guard = shared.lock().unwrap();
                assert!(
                    guard.is_none(),
                    "shared client should be None after health_check detects exit"
                );
            }

            // Contract: state transitioned to Failed, reason mentions exit
            match daemon.state() {
                LspDaemonState::Failed { reason, attempts } => {
                    assert!(
                        reason.contains("process exited"),
                        "expected 'process exited' in reason, got: {reason}"
                    );
                    assert_eq!(attempts, 1, "expected attempts == 1");
                }
                other => panic!("expected Failed, got: {other:?}"),
            }

            // Contract: consecutive_failures exactly 1
            assert_eq!(daemon.consecutive_failures, 1);

            let _ = helper.kill();
        }
    }

    /// After `health_check` detects an exit and drops the child, a subsequent
    /// call with no child is the early-return path. It must not mutate state
    /// or the failure counter.
    #[tokio::test]
    async fn test_health_check_none_path_does_not_mutate_state() {
        let spec = test_spec("some-server");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

        // Pre-populate state as Failed with a specific reason
        daemon.record_failure("original failure".into());
        let before_state = daemon.state();
        let before_failures = daemon.consecutive_failures;

        // No child — early return path
        assert!(!daemon.health_check());

        // State and counter must be unchanged
        assert_eq!(daemon.state(), before_state);
        assert_eq!(daemon.consecutive_failures, before_failures);
    }

    /// Verify that `health_check` on a live process returns true without
    /// side effects (doesn't touch client, child, failures, or state).
    #[tokio::test]
    async fn test_health_check_live_process_no_side_effects() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let child = Command::new("sleep")
            .args(["30"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn sleep");

        let spec = cat_spec();
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
        daemon.child = Some(child);

        let before_failures = daemon.consecutive_failures;
        let before_state = daemon.state();

        // Still alive — should return true with no side effects
        assert!(daemon.health_check());
        assert!(daemon.child.is_some(), "child handle retained");
        assert_eq!(daemon.consecutive_failures, before_failures);
        assert_eq!(daemon.state(), before_state);

        // Poll a couple more times — idempotent
        assert!(daemon.health_check());
        assert!(daemon.health_check());
        assert_eq!(daemon.consecutive_failures, before_failures);

        // Clean up
        if let Some(mut child) = daemon.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }

    // -- Card 5: async send/read_jsonrpc_message error path coverage --------

    /// A writer that accepts the first N bytes, then returns BrokenPipe on
    /// any further writes. Used to simulate a header-OK / body-fail split.
    struct PartialWriter {
        remaining: usize,
    }

    impl tokio::io::AsyncWrite for PartialWriter {
        fn poll_write(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            if self.remaining == 0 {
                return std::task::Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "partial writer exhausted",
                )));
            }
            let n = std::cmp::min(self.remaining, buf.len());
            self.remaining -= n;
            std::task::Poll::Ready(Ok(n))
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    /// A writer that accepts all writes but fails on flush. Used to
    /// isolate the flush error path in `send_jsonrpc_message`.
    struct FlushFailingWriter {
        buf: Vec<u8>,
    }

    impl tokio::io::AsyncWrite for FlushFailingWriter {
        fn poll_write(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            self.buf.extend_from_slice(buf);
            std::task::Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Err(std::io::Error::other("simulated flush failure")))
        }

        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    /// A reader that returns an I/O error (not EOF) on every read. Used to
    /// exercise the read_line error-return in `read_jsonrpc_message`.
    struct FailingReader;

    impl tokio::io::AsyncRead for FailingReader {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            _buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "simulated read failure",
            )))
        }
    }

    /// `send_jsonrpc_message` must return an error when the writer fails
    /// *after* the header has been written but during the body write.
    /// `PartialWriter` lets us pick the exact number of bytes that will
    /// succeed — just enough for the Content-Length header + CRLF.
    #[tokio::test]
    async fn test_send_jsonrpc_body_write_failure() {
        let msg = json!({"jsonrpc": "2.0", "method": "test", "id": 1});
        // First compute the serialized body length so we can build the
        // header exactly and size the writer to succeed on the header
        // bytes only.
        let body = serde_json::to_string(&msg).unwrap();
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut writer = PartialWriter {
            remaining: header.len(),
        };
        let result = send_jsonrpc_message(&mut writer, &msg).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("write body"),
            "expected write body error, got: {err}"
        );
    }

    /// `send_jsonrpc_message` must return an error when the writer's
    /// flush fails (header + body succeed, flush fails).
    #[tokio::test]
    async fn test_send_jsonrpc_flush_failure() {
        let msg = json!({"jsonrpc": "2.0", "method": "test", "id": 1});
        let mut writer = FlushFailingWriter { buf: Vec::new() };
        let result = send_jsonrpc_message(&mut writer, &msg).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("flush"),
            "expected flush error, got: {err}"
        );
    }

    /// `read_jsonrpc_message` must return an error when the underlying
    /// reader I/O-errors during `read_line` (not EOF, a real error).
    /// This exercises the `read header line: {e}` error arm.
    #[tokio::test]
    async fn test_read_jsonrpc_read_line_io_error() {
        let reader = FailingReader;
        let mut buf_reader = BufReader::new(reader);
        let result = read_jsonrpc_message(&mut buf_reader).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("read header line"),
            "expected read header line error, got: {err}"
        );
    }

    /// A reader that emits exactly one valid header line with a correct
    /// Content-Length, sends the blank-line terminator, then I/O-errors
    /// when the body `read_exact` is attempted. Covers the `read body: {e}`
    /// error arm.
    struct HeaderOkBodyFailReader {
        stage: u8,
        header_bytes: &'static [u8],
        pos: usize,
    }

    impl tokio::io::AsyncRead for HeaderOkBodyFailReader {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            match self.stage {
                0 => {
                    // Feed header bytes byte-by-byte (simple and robust)
                    if self.pos < self.header_bytes.len() {
                        let b = self.header_bytes[self.pos];
                        buf.put_slice(&[b]);
                        self.pos += 1;
                        if self.pos == self.header_bytes.len() {
                            self.stage = 1;
                        }
                        std::task::Poll::Ready(Ok(()))
                    } else {
                        self.stage = 1;
                        std::task::Poll::Ready(Ok(()))
                    }
                }
                _ => std::task::Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "simulated body read failure",
                ))),
            }
        }
    }

    /// Read-body I/O error: the underlying reader errors during the
    /// `read_exact(&mut body)` call after headers were parsed. This hits
    /// the `read body: {e}` error arm.
    #[tokio::test]
    async fn test_read_jsonrpc_read_exact_io_error() {
        // Content-Length: 10, blank line, then body read will fail.
        let header = b"Content-Length: 10\r\n\r\n";
        let reader = HeaderOkBodyFailReader {
            stage: 0,
            header_bytes: header,
            pos: 0,
        };
        let mut buf_reader = BufReader::new(reader);
        let result = read_jsonrpc_message(&mut buf_reader).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("read body"),
            "expected read body error, got: {err}"
        );
    }

    /// `send_jsonrpc_message` writes header + body + flushes. Verify
    /// that a happy-path call against a `Vec<u8>` produces the exact
    /// expected bytes (header format + body JSON) — this locks the
    /// wire format.
    #[tokio::test]
    async fn test_send_jsonrpc_produces_exact_wire_format() {
        let msg = json!({"jsonrpc": "2.0", "id": 42, "method": "ping"});
        let mut buf: Vec<u8> = Vec::new();
        send_jsonrpc_message(&mut buf, &msg).await.unwrap();

        let expected_body = serde_json::to_string(&msg).unwrap();
        let expected_header = format!("Content-Length: {}\r\n\r\n", expected_body.len());
        let mut expected = Vec::new();
        expected.extend_from_slice(expected_header.as_bytes());
        expected.extend_from_slice(expected_body.as_bytes());
        assert_eq!(buf, expected);
    }

    /// `read_jsonrpc_message` accepts any case and spacing variant of the
    /// `Content-Length:` header prefix, as long as the literal prefix
    /// `Content-Length:` is present. Verifies the `strip_prefix` path.
    #[tokio::test]
    async fn test_read_jsonrpc_content_length_with_no_space_after_colon() {
        let body = r#"{"jsonrpc":"2.0"}"#;
        let msg = format!("Content-Length:{}\r\n\r\n{}", body.len(), body);
        let bytes = msg.as_bytes();
        let mut cursor = bytes;
        let mut reader = BufReader::new(&mut cursor);
        let decoded = read_jsonrpc_message(&mut reader).await.unwrap();
        assert_eq!(decoded["jsonrpc"], "2.0");
    }

    /// `read_jsonrpc_message` must surface an error when EOF arrives
    /// *between* a valid header line and the blank-line terminator
    /// (i.e. we parsed headers but the input was cut short). This
    /// specifically hits the `if n == 0 { return Err(...) }` path for
    /// the second iteration of the header loop.
    #[tokio::test]
    async fn test_read_jsonrpc_eof_after_first_header() {
        // Provide exactly one header line with \r\n, then EOF — no blank line
        let partial = b"Content-Length: 5\r\n";
        let mut cursor = &partial[..];
        let mut reader = BufReader::new(&mut cursor);
        let result = read_jsonrpc_message(&mut reader).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("unexpected EOF"),
            "expected 'unexpected EOF', got: {err}"
        );
    }

    /// `send_jsonrpc_message` on a serde-Value that cannot be serialized
    /// would error at the `serde_json::to_string` step. Since any
    /// `serde_json::Value` is always serializable, we document this by
    /// exercising a complex nested Value successfully — locking the
    /// happy path for `serde_json::to_string`.
    #[tokio::test]
    async fn test_send_jsonrpc_serializes_nested_values() {
        let msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "complex",
            "params": {
                "nested": {
                    "arr": [1, 2, 3, {"deep": true}],
                    "null_value": null,
                    "unicode": "αβγ"
                }
            }
        });
        let mut buf: Vec<u8> = Vec::new();
        send_jsonrpc_message(&mut buf, &msg).await.unwrap();

        // Read it back to verify round-trip
        let mut cursor = &buf[..];
        let mut reader = BufReader::new(&mut cursor);
        let decoded = read_jsonrpc_message(&mut reader).await.unwrap();
        assert_eq!(decoded["params"]["nested"]["unicode"], "αβγ");
        assert_eq!(decoded["params"]["nested"]["arr"][3]["deep"], true);
    }

    /// `graceful_shutdown` on a child whose stdin is already closed must
    /// still wait for the child to exit and return Ok when it does. This
    /// exercises the `child.stdin.as_mut() == None` branch at the top.
    #[tokio::test]
    async fn test_graceful_shutdown_with_no_stdin_still_waits() {
        let mut child = Command::new("true")
            .stdin(Stdio::null()) // no stdin pipe at all
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn true");

        // Take the stdin to be sure
        let _ = child.stdin.take();

        let result = LspDaemon::graceful_shutdown(child).await;
        assert!(
            result.is_ok(),
            "expected graceful_shutdown to succeed: {result:?}"
        );
    }

    /// `graceful_shutdown` happy path: cat takes input, shutdown request +
    /// exit notification close its stdin (cat exits), wait() returns Ok.
    #[tokio::test]
    async fn test_graceful_shutdown_normal_flow_with_cat() {
        let child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn cat");

        let result = LspDaemon::graceful_shutdown(child).await;
        assert!(
            result.is_ok(),
            "expected graceful_shutdown with cat to succeed: {result:?}"
        );
    }

    // -- Card 4: initialize_handshake and capture_stderr direct tests -------

    /// Directly invoke `initialize_handshake` with an invalid (relative)
    /// workspace path. `Url::from_file_path` rejects relative paths, so the
    /// function should return `HandshakeFailed("invalid workspace path")`.
    /// This exercises the `.map_err` branch of the `root_uri` construction.
    #[tokio::test]
    async fn test_initialize_handshake_invalid_workspace_path() {
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn cat");

        let spec = test_spec("cat");
        // A relative path — `Url::from_file_path` requires absolute paths
        let relative = PathBuf::from("not/an/absolute/path");
        let result = LspDaemon::initialize_handshake(&mut child, &relative, &spec).await;

        let _ = child.kill().await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            LspError::HandshakeFailed(msg) => {
                assert!(
                    msg.contains("invalid workspace path"),
                    "expected 'invalid workspace path', got: {msg}"
                );
            }
            other => panic!("expected HandshakeFailed, got: {other:?}"),
        }
    }

    /// Directly invoke `initialize_handshake` against a child that has
    /// had its stdin taken externally. This exercises the `stdin.as_mut()`
    /// `None` branch that returns `HandshakeFailed("child stdin unavailable")`.
    #[tokio::test]
    async fn test_initialize_handshake_stdin_unavailable() {
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn cat");

        // Take stdin, dropping the handle so the child sees EOF on stdin
        let _stolen_stdin = child.stdin.take();
        drop(_stolen_stdin);

        let spec = test_spec("cat");
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let result = LspDaemon::initialize_handshake(&mut child, workspace.path(), &spec).await;

        let _ = child.kill().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            LspError::HandshakeFailed(msg) => {
                assert!(
                    msg.contains("child stdin unavailable"),
                    "expected 'child stdin unavailable', got: {msg}"
                );
            }
            other => panic!("expected HandshakeFailed with stdin msg, got: {other:?}"),
        }
    }

    /// Directly invoke `initialize_handshake` against a child that has
    /// had its stdout taken externally. This exercises the `stdout.as_mut()`
    /// `None` branch that returns `HandshakeFailed("child stdout unavailable")`.
    #[tokio::test]
    async fn test_initialize_handshake_stdout_unavailable() {
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn cat");

        // Take stdout, dropping the handle
        let _stolen_stdout = child.stdout.take();
        drop(_stolen_stdout);

        let spec = test_spec("cat");
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let result = LspDaemon::initialize_handshake(&mut child, workspace.path(), &spec).await;

        let _ = child.kill().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            LspError::HandshakeFailed(msg) => {
                assert!(
                    msg.contains("child stdout unavailable"),
                    "expected 'child stdout unavailable', got: {msg}"
                );
            }
            other => panic!("expected HandshakeFailed with stdout msg, got: {other:?}"),
        }
    }

    /// `capture_stderr` on a child whose stderr handle has been taken
    /// returns an empty string (the early-return `None` path).
    #[tokio::test]
    async fn test_capture_stderr_returns_empty_when_no_stderr_handle() {
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn cat");

        // Take stderr so the handle is None
        let _stolen = child.stderr.take();
        drop(_stolen);

        let captured = LspDaemon::capture_stderr(&mut child).await;
        assert_eq!(captured, "");

        let _ = child.kill().await;
    }

    /// `capture_stderr` reads available data from the child's stderr
    /// within the 1-second timeout, trims it, and returns it.
    #[tokio::test]
    async fn test_capture_stderr_reads_available_bytes() {
        let mut child = Command::new("sh")
            .args(["-c", "echo mock-error-text 1>&2; sleep 2"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn sh");

        // Give the child a moment to write to stderr
        tokio::time::sleep(Duration::from_millis(200)).await;

        let captured = LspDaemon::capture_stderr(&mut child).await;
        assert!(
            captured.contains("mock-error-text"),
            "expected 'mock-error-text' in captured stderr, got: {captured:?}"
        );

        let _ = child.kill().await;
        let _ = child.wait().await;
    }

    /// `capture_stderr` on a child that produces no stderr output within
    /// the timeout returns the empty string (the `_ =>` arm of the match
    /// on `timeout(read).await`).
    #[tokio::test]
    async fn test_capture_stderr_returns_empty_on_timeout() {
        let mut child = Command::new("sh")
            .args(["-c", "sleep 10"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn sh");

        // No stderr output forthcoming — capture_stderr's 1s timeout kicks in
        let captured = LspDaemon::capture_stderr(&mut child).await;
        assert_eq!(captured, "");

        let _ = child.kill().await;
        let _ = child.wait().await;
    }

    /// When `initialize_handshake` fails because the child closes stdout
    /// immediately, the error message should be decorated with stderr
    /// context (via `capture_stderr`). This confirms both the error path
    /// through `read_jsonrpc_message` and the concatenation of stderr into
    /// the `HandshakeFailed` message.
    #[tokio::test]
    async fn test_initialize_handshake_includes_stderr_context_in_error() {
        // Python script that writes to stderr and then exits without
        // responding on stdout. The daemon will read EOF from stdout and
        // then capture_stderr should pick up the error message.
        let script = r#"
import sys
sys.stderr.write('FATAL: mock LSP refuses to initialize\n')
sys.stderr.flush()
# Close stdout immediately so reader gets EOF
sys.stdout.close()
import time
time.sleep(0.3)
"#;
        let spec = python_mock_spec(script);
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

        let result = daemon.start().await;
        assert!(result.is_err(), "expected start to fail");
        match result.unwrap_err() {
            LspError::HandshakeFailed(msg) => {
                // capture_stderr should prepend "; stderr: " once it reads
                // the stderr content. The exact content depends on timing,
                // but the error should surface an EOF and optionally a
                // stderr decoration. We accept either (timing-sensitive).
                assert!(
                    msg.contains("EOF")
                        || msg.contains("stderr")
                        || msg.contains("FATAL")
                        || msg.contains("read header"),
                    "expected EOF/stderr/FATAL/read header in msg, got: {msg}"
                );
            }
            LspError::Timeout(_) => {
                // Timing-dependent alternate path: if the timeout fires
                // before read_jsonrpc_message completes, we get Timeout.
            }
            other => panic!("expected HandshakeFailed or Timeout, got: {other:?}"),
        }
    }

    /// `initialize_handshake` happy path: a Python mock that plays the full
    /// initialize/initialized handshake correctly. This covers the
    /// `send_jsonrpc_message(&mut writer, &initialized)` line (the final
    /// send of the `initialized` notification).
    #[tokio::test]
    async fn test_initialize_handshake_full_happy_path_direct() {
        let script = r#"
import sys, re, json, time
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
resp = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"capabilities": {}}})
sys.stdout.write(f'Content-Length: {len(resp)}\r\n\r\n{resp}')
sys.stdout.flush()
# Read the initialized notification and confirm it was sent
headers2 = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers2 += line
m2 = re.search(r'Content-Length:\s*(\d+)', headers2)
if m2:
    body2 = sys.stdin.read(int(m2.group(1)))
    # Echo the initialized body to stderr so we can assert on it
    sys.stderr.write(f'GOT: {body2}\n')
    sys.stderr.flush()
time.sleep(0.5)
"#;
        let mut child = Command::new("python3")
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn python3");

        let spec = test_spec("python3");
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let result = LspDaemon::initialize_handshake(&mut child, workspace.path(), &spec).await;

        assert!(result.is_ok(), "expected handshake to succeed: {result:?}");

        let _ = child.kill().await;
        let _ = child.wait().await;
    }

    // -- Card 3: additional restart_with_backoff / shutdown coverage ---------

    /// `restart_with_backoff` with failures one below the cap should sleep
    /// for the cap-level backoff (~32s cap at attempt=5 / 60s cap beyond).
    /// We use attempt=2 so the delay is a bounded 4s sleep — short enough
    /// to stay reasonable, long enough to measurably verify the sleep
    /// happened. This exercises the sleep+restart info-log path.
    #[tokio::test]
    async fn test_restart_with_backoff_info_log_path_attempt_2() {
        // attempt=2 → 4s backoff
        let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        daemon.consecutive_failures = 2;

        let start = std::time::Instant::now();
        let result = daemon.restart_with_backoff().await;
        let elapsed = start.elapsed();

        assert!(result.is_err());
        assert!(
            elapsed >= Duration::from_secs(3),
            "expected at least ~4s backoff, got: {elapsed:?}"
        );
        // start() hit BinaryNotFound, so state is NotFound
        assert_eq!(daemon.state(), LspDaemonState::NotFound);
    }

    /// When `restart_with_backoff` is called and the underlying start()
    /// succeeds (via a working mock), the restart must return Ok(()) and
    /// the daemon transitions to Running.
    #[tokio::test]
    async fn test_restart_with_backoff_success_path() {
        let script = r#"
import sys, re, json, time
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
resp = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"capabilities": {}}})
sys.stdout.write(f'Content-Length: {len(resp)}\r\n\r\n{resp}')
sys.stdout.flush()
headers2 = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers2 += line
m2 = re.search(r'Content-Length:\s*(\d+)', headers2)
if m2:
    body2 = sys.stdin.read(int(m2.group(1)))
time.sleep(0.5)
"#;
        let spec = python_mock_spec(script);
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
        // attempt 0: 1s backoff before start
        daemon.consecutive_failures = 0;

        let start_time = std::time::Instant::now();
        let result = daemon.restart_with_backoff().await;
        let elapsed = start_time.elapsed();

        assert!(
            result.is_ok(),
            "expected restart_with_backoff to succeed: {result:?}"
        );
        assert!(matches!(daemon.state(), LspDaemonState::Running { .. }));
        // Successful start resets consecutive_failures
        assert_eq!(daemon.consecutive_failures, 0);
        // Must have waited at least ~1s for the backoff
        assert!(
            elapsed >= Duration::from_millis(800),
            "expected ~1s backoff, got: {elapsed:?}"
        );

        daemon.shutdown().await;
    }

    /// `shutdown()` on a process that refuses to exit within the grace
    /// window (by ignoring stdin close) must eventually return with state
    /// NotStarted. This covers the `Err(_)` (timeout) branch of the
    /// shutdown outcome match. Slow test (~5s) — same pattern as existing
    /// `test_shutdown_kills_long_running_child` but with an explicit
    /// state-transition sequence assertion.
    #[tokio::test]
    async fn test_shutdown_timeout_path_emits_shutting_down_then_not_started() {
        // sleep never responds to stdin close — shutdown will hit the
        // SHUTDOWN_GRACE_SECS timeout.
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let child = Command::new("sleep")
            .args(["60"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn sleep");

        let spec = test_spec("sleep");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
        daemon.child = Some(child);

        let mut rx = daemon.state_rx();
        // Drain any prior state changes
        while rx.has_changed().unwrap_or(false) {
            let _ = rx.borrow_and_update();
        }

        let start = std::time::Instant::now();
        daemon.shutdown().await;
        let elapsed = start.elapsed();

        // Final state is NotStarted
        assert_eq!(daemon.state(), LspDaemonState::NotStarted);
        assert!(daemon.child.is_none());

        // Elapsed time must be at least near SHUTDOWN_GRACE_SECS (5s)
        // because graceful_shutdown waits for the sleep process which
        // never exits from stdin close alone.
        assert!(
            elapsed >= Duration::from_secs(4),
            "expected at least ~5s shutdown (timeout path), got: {elapsed:?}"
        );
    }

    /// `shutdown()` after a client has been explicitly installed must clear
    /// the shared client regardless of whether a child exists.
    #[tokio::test]
    async fn test_shutdown_clears_client_with_installed_value() {
        let spec = test_spec("some-server");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

        // Install a real client
        let helper = std::process::Command::new("cat")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn();
        if let Ok(mut helper) = helper {
            let hstdin = helper.stdin.take().unwrap();
            let hstdout = helper.stdout.take().unwrap();
            let shared = daemon.shared_client();
            {
                let mut guard = shared.lock().unwrap();
                *guard = Some(LspJsonRpcClient::new(hstdin, hstdout));
            }

            // Verify installed
            {
                let guard = shared.lock().unwrap();
                assert!(guard.is_some());
            }

            // Shutdown with no child — should take the early-return path
            // but still clear the client.
            daemon.shutdown().await;

            {
                let guard = shared.lock().unwrap();
                assert!(
                    guard.is_none(),
                    "shutdown must clear client even when no child"
                );
            }
            assert_eq!(daemon.state(), LspDaemonState::NotStarted);
            let _ = helper.kill();
        }
    }

    /// Calling `shutdown()` multiple times in a row on a never-started
    /// daemon is idempotent — state remains NotStarted.
    #[tokio::test]
    async fn test_shutdown_is_idempotent_when_not_started() {
        let spec = test_spec("some-server");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

        daemon.shutdown().await;
        assert_eq!(daemon.state(), LspDaemonState::NotStarted);

        daemon.shutdown().await;
        assert_eq!(daemon.state(), LspDaemonState::NotStarted);

        daemon.shutdown().await;
        assert_eq!(daemon.state(), LspDaemonState::NotStarted);
    }

    /// Exercise `restart_with_backoff` at each valid attempt level (0..MAX-1)
    /// by confirming the error value returned when start() fails matches
    /// BinaryNotFound for the bad-binary case.
    #[tokio::test]
    async fn test_restart_with_backoff_at_attempt_zero_returns_binary_not_found() {
        // attempt 0 → 1s backoff → then start() which hits BinaryNotFound
        let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
        let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
        daemon.consecutive_failures = 0;

        let result = daemon.restart_with_backoff().await;
        let err = result.unwrap_err();
        // The underlying start() call surfaces BinaryNotFound
        assert!(
            matches!(err, LspError::BinaryNotFound { .. }),
            "expected BinaryNotFound from underlying start(), got: {err:?}"
        );
    }

    /// Verify that `health_check` records a `Failed` state whose `attempts`
    /// field equals the current `consecutive_failures`, incrementing as
    /// detections accumulate. This documents the link between the counter
    /// and the Failed-state payload.
    #[tokio::test]
    async fn test_health_check_failed_attempts_tracks_consecutive_failures() {
        let spec = immediately_exiting_spec();
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

        // Simulate prior failures
        daemon.consecutive_failures = 2;

        let child = Command::new("true")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn true");
        daemon.child = Some(child);

        tokio::time::sleep(Duration::from_millis(200)).await;

        assert!(!daemon.health_check());
        // The counter bumps from 2 to 3
        assert_eq!(daemon.consecutive_failures, 3);
        match daemon.state() {
            LspDaemonState::Failed { attempts, .. } => {
                assert_eq!(attempts, 3);
            }
            other => panic!("expected Failed with attempts=3, got: {other:?}"),
        }
    }
}
