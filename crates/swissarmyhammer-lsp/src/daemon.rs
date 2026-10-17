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
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, Command};
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

use crate::client::{LspJsonRpcClient, SharedLspClient};
use crate::error::LspError;
use crate::framing::{self, WriteStep};
use crate::session::{LspSession, PathFilter};
use crate::types::{LspDaemonState, OwnedLspServerSpec};

/// Predicate that decides whether a line of LSP-server stderr should be
/// suppressed from debug logs.
///
/// LSP servers can be noisy on stderr (progress chatter, info banners). The
/// daemon drains stderr in the background and applies this filter so callers
/// can suppress known-noise lines. The filter is injected by the owner (via
/// [`LspSupervisorManager::with_stderr_filter`](crate::LspSupervisorManager::with_stderr_filter))
/// rather than hard-wired, so this crate stays free of any configuration-source
/// dependency. When no filter is set, every stderr line is logged.
pub type StderrFilter = Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// Maximum consecutive failures before we stop retrying.
const MAX_CONSECUTIVE_FAILURES: u32 = 5;

/// Backoff durations in seconds: 1, 2, 4, 8, 16, 32, 60, 60, ...
const BACKOFF_BASE: u64 = 1;
const BACKOFF_CAP_SECS: u64 = 60;

/// Timeout for the graceful shutdown phase before SIGKILL.
const SHUTDOWN_GRACE_SECS: u64 = 5;

/// The time a server must stay up after a good start before the daemon
/// clears its consecutive failure count.
///
/// A good `initialize` alone does not prove a recovery: a server that
/// initializes and then crashes would otherwise restart with no limit.
const MIN_HEALTHY_UPTIME: Duration = Duration::from_secs(5 * 60);

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
    /// The single owned session for this server, built over the same shared
    /// `client` handle. It owns the shared open-document set and is the one
    /// type through which in-process consumers issue requests and keep
    /// documents open across them — no other type spawns a client.
    session: LspSession<LspJsonRpcClient>,
    /// Consecutive failure count for backoff calculation.
    ///
    /// A good start does not clear it. Only a server that stays up for
    /// [`min_healthy_uptime`](Self::min_healthy_uptime) clears it (see
    /// [`health_check`](Self::health_check)), so a server that crashes after
    /// each good `initialize` reaches [`MAX_CONSECUTIVE_FAILURES`].
    consecutive_failures: u32,
    /// The time of the last good start, while the server runs. `None` when
    /// no server runs.
    healthy_since: Option<Instant>,
    /// Whether [`restart_with_backoff`](Self::restart_with_backoff) has logged
    /// that it stopped at the restart limit. The error is logged one time,
    /// not at each health interval.
    restart_limit_reported: bool,
    /// Observable state — subscribers get notified on every transition.
    state_tx: watch::Sender<LspDaemonState>,
    /// Read-side of the state watch (cloneable for external consumers).
    state_rx: watch::Receiver<LspDaemonState>,
    /// Grace period for [`shutdown`](Self::shutdown) before the child is killed
    /// and reaped. Defaults to [`SHUTDOWN_GRACE_SECS`]; tests override it via
    /// [`set_shutdown_grace`](Self::set_shutdown_grace) so they don't wait out
    /// the full production grace period when probing the timeout path.
    shutdown_grace: Duration,
    /// The uptime after which a running server clears the failure count.
    /// Defaults to [`MIN_HEALTHY_UPTIME`]; tests override it via
    /// [`set_min_healthy_uptime`](Self::set_min_healthy_uptime).
    min_healthy_uptime: Duration,
    /// Optional predicate suppressing matching LSP-server stderr lines.
    ///
    /// Injected by the owner so this crate doesn't depend on any config source.
    /// `None` logs every stderr line.
    stderr_filter: Option<StderrFilter>,
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
        let client: SharedLspClient = Arc::new(Mutex::new(None));
        // One session per server, sharing the daemon's single client handle.
        let language_id = spec
            .language_ids
            .first()
            .cloned()
            .unwrap_or_else(|| "plaintext".to_string());
        let session = LspSession::new(Arc::clone(&client), language_id);
        Self {
            spec,
            workspace_root,
            child: None,
            client,
            session,
            consecutive_failures: 0,
            healthy_since: None,
            restart_limit_reported: false,
            state_tx,
            state_rx,
            shutdown_grace: Duration::from_secs(SHUTDOWN_GRACE_SECS),
            min_healthy_uptime: MIN_HEALTHY_UPTIME,
            stderr_filter: None,
        }
    }

    /// Set the stderr-noise filter for this daemon.
    ///
    /// The predicate receives each line the LSP server writes to stderr and
    /// returns `true` to suppress it from debug logs. Injecting the filter here
    /// keeps the configuration source (e.g. code-context's stderr-filter config)
    /// in the owning crate instead of this one.
    pub fn set_stderr_filter(&mut self, filter: StderrFilter) {
        self.stderr_filter = Some(filter);
    }

    /// Set the filter of the paths that this daemon's session must not send
    /// to the server.
    ///
    /// The filter goes to the one session of this daemon, so every consumer
    /// that holds a clone of [`session`](Self::session) obeys it. See
    /// [`LspSession::set_path_filter`].
    pub fn set_path_filter(&mut self, filter: PathFilter) {
        self.session.set_path_filter(filter);
    }

    /// Override the graceful-shutdown grace period.
    ///
    /// Test-only seam: the production default is [`SHUTDOWN_GRACE_SECS`], but
    /// tests that deliberately drive the timeout branch (a child that ignores
    /// the closed pipes and never exits) set a short period so they assert the
    /// timeout → kill-and-reap behavior in milliseconds instead of seconds.
    #[cfg(test)]
    pub(crate) fn set_shutdown_grace(&mut self, grace: Duration) {
        self.shutdown_grace = grace;
    }

    /// Override the uptime after which a running server clears the failure
    /// count.
    ///
    /// Test-only seam: the production default is [`MIN_HEALTHY_UPTIME`]
    /// (minutes), so tests set a short period to probe the reset.
    #[cfg(test)]
    pub(crate) fn set_min_healthy_uptime(&mut self, uptime: Duration) {
        self.min_healthy_uptime = uptime;
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

    /// Return a cloneable handle to this daemon's single LSP session.
    ///
    /// The session owns the shared open-document set and routes
    /// requests/notifications through the same client this daemon manages.
    /// Every clone shares one open-doc set, so multiple in-process consumers
    /// (the code-context indexer and query ops, diagnostics) never disagree
    /// about what the server believes is open. This daemon owns exactly one
    /// session; no other type spawns a client.
    pub fn session(&self) -> LspSession<LspJsonRpcClient> {
        self.session.clone()
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

        // A new server replaces the old one: stop the old process first, so
        // it does not stay alive or stay as a zombie.
        if let Some(mut old_child) = self.child.take() {
            self.kill_and_reap(&mut old_child).await;
        }

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

                // Drain stderr in the background, suppressing lines the
                // injected filter matches (the owner supplies the filter so
                // this crate carries no configuration-source dependency).
                if let Some(stderr) = child.stderr.take() {
                    let cmd = self.spec.command.clone();
                    let filter = self.stderr_filter.clone();
                    tokio::spawn(async move {
                        use tokio::io::{AsyncBufReadExt, BufReader};
                        let mut lines = BufReader::new(stderr).lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let filtered = filter.as_ref().map(|f| f(&line)).unwrap_or(false);
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

                // A fresh process knows nothing about what a prior incarnation
                // had open, so the session's open-doc set must start empty —
                // otherwise post-restart opens would be suppressed as stale
                // duplicates and the new server would never learn the document.
                self.session.reset_documents();

                // Store the client in the shared Arc<Mutex<Option<...>>>
                if let Ok(mut guard) = self.client.lock() {
                    *guard = client;
                }
                self.child = Some(child);
                // A good start does not clear the failure count: the server
                // must first stay up for the minimum healthy uptime.
                self.healthy_since = Some(Instant::now());
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
                self.kill_and_reap(&mut child).await;
                self.record_failure(reason.clone());
                Err(e)
            }
            Err(_) => {
                let timeout = self.spec.startup_timeout();
                let reason = format!("initialize timed out after {timeout:?}");
                error!(cmd = &self.spec.command, "LSP initialize timed out");
                self.kill_and_reap(&mut child).await;
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
                // The process is gone; the session must not keep believing its
                // documents are open (a restart would otherwise suppress the
                // re-opens as stale duplicates).
                self.session.reset_documents();
                self.child = None;
                self.record_failure(reason);
                false
            }
            Ok(None) => {
                self.clear_failures_after_healthy_uptime();
                true
            }
            Err(e) => {
                let reason = format!("try_wait error: {e}");
                warn!(cmd = self.spec.command, %e, "Error checking LSP server health");
                if let Ok(mut guard) = self.client.lock() {
                    *guard = None;
                }
                // The process is gone; the session must not keep believing its
                // documents are open (a restart would otherwise suppress the
                // re-opens as stale duplicates).
                self.session.reset_documents();
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
    ///
    /// At the limit the daemon stops: it starts no server, and it logs the
    /// error one time. A later call at the limit returns the same error and
    /// logs nothing, so the health loop does not repeat the log at each
    /// interval. [`force_restart`](Self::force_restart) starts again.
    pub async fn restart_with_backoff(&mut self) -> Result<(), LspError> {
        if self.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
            let reason = format!(
                "too many consecutive failures ({}), giving up",
                self.consecutive_failures
            );
            if !self.restart_limit_reported {
                self.restart_limit_reported = true;
                error!(cmd = self.spec.command, %reason, "LSP server restart limit reached");
            }
            return Err(LspError::HandshakeFailed(reason));
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
        self.clear_failures();
        self.start().await
    }

    /// Gracefully shut down the LSP server.
    ///
    /// Drops the JSON-RPC client (closing stdin/stdout pipes), then waits for
    /// the child process to exit. If it does not exit within
    /// [`SHUTDOWN_GRACE_SECS`], the process is killed and reaped.
    pub async fn shutdown(&mut self) {
        // Drop the client first — this closes the stdin/stdout pipes,
        // which signals the LSP server to exit.
        if let Ok(mut guard) = self.client.lock() {
            *guard = None;
        }
        // The server is going away; forget what it had open so a later restart
        // re-announces documents instead of suppressing them as duplicates.
        self.session.reset_documents();
        self.healthy_since = None;

        let mut child = match self.child.take() {
            Some(c) => c,
            None => {
                self.set_state(LspDaemonState::NotStarted);
                return;
            }
        };

        self.set_state(LspDaemonState::ShuttingDown);

        let result =
            tokio::time::timeout(self.shutdown_grace, Self::graceful_shutdown(&mut child)).await;

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
                    "Shutdown timed out, killing the LSP server"
                );
                self.kill_and_reap(&mut child).await;
            }
        }

        self.set_state(LspDaemonState::NotStarted);
    }

    /// Kill `child` and wait for its exit, so no process or zombie of the
    /// server stays.
    async fn kill_and_reap(&self, child: &mut Child) {
        if let Err(e) = child.kill().await {
            warn!(cmd = self.spec.command, %e, "Failed to kill the LSP server");
        }
    }

    // -- internal helpers -------------------------------------------------

    /// Transition to a new state and notify watchers.
    fn set_state(&self, state: LspDaemonState) {
        debug!(cmd = self.spec.command, ?state, "State transition");
        let _ = self.state_tx.send(state);
    }

    /// Clear the failure count and the record of the restart-limit log.
    fn clear_failures(&mut self) {
        self.consecutive_failures = 0;
        self.restart_limit_reported = false;
    }

    /// Clear the failure count when the server has stayed up for the minimum
    /// healthy uptime since its last good start.
    fn clear_failures_after_healthy_uptime(&mut self) {
        let healthy_long_enough = self
            .healthy_since
            .is_some_and(|since| since.elapsed() >= self.min_healthy_uptime);
        if healthy_long_enough && self.consecutive_failures > 0 {
            debug!(
                cmd = self.spec.command,
                failures = self.consecutive_failures,
                "LSP server stayed healthy; clearing the failure count"
            );
            self.clear_failures();
        }
    }

    /// Record a failure, incrementing the consecutive counter.
    fn record_failure(&mut self, reason: String) {
        self.healthy_since = None;
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
    async fn graceful_shutdown(child: &mut Child) -> Result<(), LspError> {
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
///
/// The frame and the error words come from [`framing`](crate::framing), the
/// same as the sync client's writer, so both put the same bytes on the wire.
pub async fn send_jsonrpc_message<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    message: &serde_json::Value,
) -> Result<(), LspError> {
    let frame = framing::encode(message)?;
    writer
        .write_all(frame.header.as_bytes())
        .await
        .map_err(|e| WriteStep::Header.error(e))?;
    writer
        .write_all(frame.body.as_bytes())
        .await
        .map_err(|e| WriteStep::Body.error(e))?;
    writer.flush().await.map_err(|e| WriteStep::Flush.error(e))
}

/// Read a single JSON-RPC message, parsing the `Content-Length` header.
///
/// The header block goes through [`framing::HeaderBlock`] and the body decode
/// through [`framing`](crate::framing), the same as the sync client's reader.
/// Header fields other than Content-Length (e.g. Content-Type) are ignored.
/// A bad header line gives its error only after the whole header block is
/// read, so the next call starts at the next frame.
pub async fn read_jsonrpc_message<R: AsyncBufReadExt + Unpin>(
    reader: &mut R,
) -> Result<serde_json::Value, LspError> {
    let mut block = framing::HeaderBlock::default();
    let length = loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .await
            .map_err(|e| LspError::JsonRpc(format!("read header line: {e}")))?;
        if n == 0 {
            return Err(framing::eof_in_headers());
        }
        if let Some(length) = block.feed(&line) {
            break length?;
        }
    };

    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|e| LspError::JsonRpc(format!("read body: {e}")))?;
    framing::decode_body(&body)
}

// -- tests ----------------------------------------------------------------

#[cfg(test)]
mod tests;
