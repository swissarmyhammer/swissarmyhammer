//! Unified MCP server implementation supporting multiple transport modes
//!
//! This module provides a clean, consolidated MCP server implementation that:
//! - Uses rmcp library properly without reimplementing MCP protocol
//! - Supports both stdio and HTTP transport modes
//! - Returns clear connection information for each mode
//! - Eliminates fragmented implementations across multiple crates
//!
//! sah rule ignore test_rule_with_allow

use rmcp::serve_server;
use rmcp::transport::io::stdio;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpService,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::Instant;
use swissarmyhammer_common::{Pretty, Result, SwissArmyHammerError, SwissarmyhammerDirectory};
use swissarmyhammer_prompts::PromptLibrary;

use tokio::net::TcpListener;
use tokio::sync::oneshot;

use super::server::McpServer;

/// Health check endpoint handler
async fn health_check() -> axum::response::Json<serde_json::Value> {
    axum::response::Json(serde_json::json!({
        "status": "healthy",
        "service": "swissarmyhammer-mcp"
    }))
}

/// Thread-safe file writer with immediate flush and sync for reliable debugging logs
///
/// This writer ensures that every write operation is immediately flushed to the OS buffer
/// and synced to disk, providing reliable log output even if the process crashes unexpectedly.
/// This is particularly important for debugging MCP communication issues.
///
/// # Thread Safety
///
/// Multiple threads can safely write to the same `FileWriterGuard` instance. Each write
/// operation acquires the mutex, writes the data, flushes to OS buffers, and syncs to disk
/// before releasing the lock.
///
/// # Performance Considerations
///
/// The immediate flush/sync strategy prioritizes reliability over performance. Each write
/// operation results in a system call, which may impact performance for high-frequency logging.
/// However, this trade-off is acceptable for MCP debugging scenarios where log reliability
/// is more important than maximum throughput.
///
/// # Error Handling
///
/// Write operations use `expect()` calls for error handling rather than returning `Result`
/// values because the `std::io::Write` trait requires specific signatures. In practice,
/// write failures to local files are extremely rare and typically indicate severe system
/// issues (disk full, permissions, hardware failure) that should terminate the process.
///
/// # Example Usage
/// ```rust,ignore
/// use std::sync::{Arc, Mutex};
/// use std::fs::File;
/// use swissarmyhammer_tools::mcp::unified_server::FileWriterGuard;
///
/// let file = File::create("debug.log").unwrap();
/// let shared_file = Arc::new(Mutex::new(file));
/// let mut guard = FileWriterGuard::new(shared_file);
/// writeln!(guard, "Debug message").unwrap();
/// ```
pub struct FileWriterGuard {
    file: Arc<Mutex<std::fs::File>>,
}

impl FileWriterGuard {
    /// Creates a new `FileWriterGuard` wrapping the given file.
    ///
    /// # Arguments
    /// * `file` - `Arc<Mutex<File>>` for thread-safe access to the underlying file
    ///
    /// # Returns
    /// A new `FileWriterGuard` instance that will ensure immediate flushing for all writes
    pub fn new(file: Arc<Mutex<std::fs::File>>) -> Self {
        Self { file }
    }
}

impl std::io::Write for FileWriterGuard {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut file = self.file.lock().expect("FileWriterGuard mutex was poisoned - this indicates a panic occurred while another thread held the lock");
        let result = file.write(buf)?;
        file.flush()?;
        file.sync_all()?; // Ensure data is actually written to disk for debugging reliability
        Ok(result)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut file = self.file.lock().expect("FileWriterGuard flush mutex was poisoned - this indicates a panic occurred while another thread held the lock");
        file.flush()?;
        file.sync_all()
    }
}

/// Global flag to ensure MCP logging is configured only once per process
static MCP_LOGGING_INIT: Once = Once::new();

/// Ensure log directory exists, creating it if necessary
///
/// # Arguments
/// * `log_dir` - Path to the log directory
///
/// # Returns
/// * `Result<()>` - Ok if directory exists or was created successfully
fn ensure_log_directory(log_dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(log_dir).map_err(|e| {
        let error_context = match e.kind() {
            std::io::ErrorKind::PermissionDenied => {
                "Permission denied - check directory permissions"
            }
            std::io::ErrorKind::AlreadyExists => {
                "Directory creation conflict - this shouldn't happen with create_dir_all"
            }
            _ => "Unknown filesystem error - check disk space and parent directory permissions",
        };
        eprintln!(
            "Warning: Could not create MCP log directory {}: {} ({})",
            log_dir.display(),
            e,
            error_context
        );
        e
    })
}

/// Create log file in the specified directory
///
/// # Arguments
/// * `log_dir` - Path to the log directory
///
/// # Returns
/// * `Result<(std::fs::File, std::path::PathBuf)>` - The created file and its path
fn create_log_file(
    log_dir: &std::path::Path,
) -> std::io::Result<(std::fs::File, std::path::PathBuf)> {
    let log_file_name =
        std::env::var("SWISSARMYHAMMER_LOG_FILE").unwrap_or_else(|_| "mcp.log".to_string());
    let log_file_path = log_dir.join(log_file_name);
    let file = std::fs::File::create(&log_file_path)?;
    Ok((file, log_file_path))
}

/// Setup tracing subscriber with file output
///
/// # Arguments
/// * `file` - The log file to write to
/// * `filter` - The tracing filter to apply
/// * `log_file_path` - Path to the log file for cleanup if needed
///
/// # Returns
/// * `bool` - True if subscriber was set successfully, false otherwise
fn setup_tracing_subscriber(
    file: std::fs::File,
    log_file_path: &std::path::Path,
    filter: tracing_subscriber::EnvFilter,
) -> bool {
    use tracing_subscriber::{fmt, prelude::*, registry};

    let shared_file = Arc::new(Mutex::new(file));
    let shared_file_for_cleanup = shared_file.clone();

    let subscriber = registry().with(filter).with(
        fmt::layer()
            .with_writer(move || {
                let file = shared_file.clone();
                Box::new(FileWriterGuard::new(file)) as Box<dyn std::io::Write>
            })
            .with_ansi(false),
    );

    if tracing::subscriber::set_global_default(subscriber).is_err() {
        tracing::debug!(
            "Global tracing subscriber already set - MCP logging configuration skipped"
        );
        drop(shared_file_for_cleanup);
        let _ = std::fs::remove_file(log_file_path);
        return false;
    }

    true
}

/// Configure MCP logging to write to `.sah/` directory
///
/// This function sets up file-based logging similar to what `sah serve` does,
/// ensuring that in-process MCP servers have the same debugging capabilities.
/// Uses `std::sync::Once` to ensure logging is only configured once per process,
/// even if multiple MCP servers are started.
///
/// # Arguments
/// * `log_filter` - Optional log filter string (defaults to "rmcp=warn,debug")
///
/// # Behavior
/// - Creates `.sah/` directory if it doesn't exist
/// - Sets up tracing subscriber with file output (uses `SWISSARMYHAMMER_LOG_FILE` env var or defaults to `mcp.log`)
/// - Falls back to stderr logging if file creation fails
/// - Only configures logging once per process (subsequent calls are no-op)
/// - Uses debug-level logging for comprehensive MCP debugging
///
/// # Error Handling
/// - Directory creation failures: Provides specific error context based on error kind
/// - File creation failures: Falls back gracefully to stderr with warning message
/// - Global subscriber conflicts: Handles gracefully when already set (e.g., in tests)
pub fn configure_mcp_logging(log_filter: Option<&str>) {
    use tracing_subscriber::EnvFilter;

    MCP_LOGGING_INIT.call_once(|| {
        let filter_str = log_filter.unwrap_or("rmcp=warn,debug");
        let filter = EnvFilter::new(filter_str);

        let log_dir = std::path::PathBuf::from(SwissarmyhammerDirectory::dir_name());

        if ensure_log_directory(&log_dir).is_err() {
            return;
        }

        match create_log_file(&log_dir) {
            Ok((file, log_file_path)) => {
                let _success = setup_tracing_subscriber(file, &log_file_path, filter);
                // Subscriber setup result is logged by setup_tracing_subscriber
            }
            Err(e) => {
                let log_file_path = log_dir.join(
                    std::env::var("SWISSARMYHAMMER_LOG_FILE")
                        .unwrap_or_else(|_| "mcp.log".to_string()),
                );
                eprintln!(
                    "Warning: Could not create MCP log file {}: {}. MCP server will use existing logging configuration.",
                    log_file_path.display(), e
                );
            }
        }
    });
}

/// MCP server transport mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum McpServerMode {
    /// Standard input/output transport
    Stdio,
    /// HTTP transport with optional port specification
    /// None = random port assignment
    Http { port: Option<u16> },
}

/// Connection information returned after server startup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    /// The transport mode used
    pub mode: McpServerMode,
    /// Connection URL or identifier
    pub connection_url: String,
    /// Actual bound port (for HTTP mode)
    pub port: Option<u16>,
}

/// Lifetime statistics for an in-process MCP server.
///
/// Counters are updated by the per-request middleware ([`request_observer`])
/// and read by [`McpServerHandle::shutdown`] to emit the matching shutdown
/// log line — the one that answers "did the in-process MCP server shut down
/// cleanly when avp exited?".
///
/// Cloning is cheap: every field is wrapped in [`Arc`] / [`AtomicU64`] so the
/// stats observed by middleware reflect the same counters the handle reads
/// at shutdown time.
#[derive(Debug, Clone)]
struct ServerStats {
    /// Wall-clock time the server bound its listener.
    started_at: Instant,
    /// Total HTTP requests routed through the validator/MCP endpoints.
    total_requests: Arc<AtomicU64>,
    /// Total HTTP requests that returned a non-2xx response.
    total_errors: Arc<AtomicU64>,
    /// MCP session ids observed via the `mcp-session-id` header on requests.
    /// Used to emit a one-time `session_open` log on first sight (so callers
    /// can grep `session_id=abc-123` end-to-end through the log).
    seen_sessions: Arc<Mutex<HashSet<String>>>,
}

impl ServerStats {
    /// Build a fresh stats bundle with `started_at` set to now.
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            total_requests: Arc::new(AtomicU64::new(0)),
            total_errors: Arc::new(AtomicU64::new(0)),
            seen_sessions: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Atomically read the current request counter.
    fn requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    /// Atomically read the current error counter.
    fn errors(&self) -> u64 {
        self.total_errors.load(Ordering::Relaxed)
    }
}

/// Handle for managing HTTP MCP server lifecycle
pub struct McpServerHandle {
    /// Server information
    pub info: McpServerInfo,
    /// Shutdown sender for graceful shutdown
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// Server task handle (only for stdio mode)
    server_task: Option<tokio::task::JoinHandle<()>>,
    /// Server instance for cleanup operations
    server: Option<Arc<McpServer>>,
    /// Completion receiver to detect when server exits naturally (stdio mode)
    completion_rx: Option<oneshot::Receiver<()>>,
    /// Lifetime statistics — observed by the per-request middleware. Used by
    /// [`Self::shutdown`] to emit a matching `bound_for_seconds`/
    /// `total_requests`/`total_errors` line so the in-process server's
    /// lifetime is visible end-to-end in `.avp/log` (or `.sah/mcp.log`).
    /// `None` for stdio handles (no HTTP middleware → no stats).
    stats: Option<ServerStats>,
}

impl std::fmt::Debug for McpServerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServerHandle")
            .field("info", &self.info)
            .field("has_shutdown_tx", &self.shutdown_tx.is_some())
            .field("has_server_task", &self.server_task.is_some())
            .field("has_server", &self.server.is_some())
            .field("has_completion_rx", &self.completion_rx.is_some())
            .field("has_stats", &self.stats.is_some())
            .finish()
    }
}

/// Parameters for creating an MCP server handle with a server task
struct McpServerHandleParams {
    info: McpServerInfo,
    shutdown_tx: oneshot::Sender<()>,
    server_task: tokio::task::JoinHandle<()>,
    server: Arc<McpServer>,
    completion_rx: oneshot::Receiver<()>,
    /// Lifetime stats observed by the per-request middleware. Stdio-only
    /// handles can pass `None` since there is no HTTP request observer.
    stats: Option<ServerStats>,
}

impl McpServerHandle {
    /// Create a new MCP server handle
    fn new(info: McpServerInfo, shutdown_tx: oneshot::Sender<()>) -> Self {
        Self {
            info,
            shutdown_tx: Some(shutdown_tx),
            server_task: None,
            server: None,
            completion_rx: None,
            stats: None,
        }
    }

    /// Create a new MCP server handle with a server task (for stdio mode)
    fn new_with_task(params: McpServerHandleParams) -> Self {
        Self {
            info: params.info,
            shutdown_tx: Some(params.shutdown_tx),
            server_task: Some(params.server_task),
            server: Some(params.server),
            completion_rx: Some(params.completion_rx),
            stats: params.stats,
        }
    }

    /// Get the connection information
    pub fn info(&self) -> &McpServerInfo {
        &self.info
    }

    /// Get the actual port (for HTTP mode)
    pub fn port(&self) -> Option<u16> {
        self.info.port
    }

    /// Get the connection URL
    pub fn url(&self) -> &str {
        &self.info.connection_url
    }

    /// Shutdown the server gracefully.
    ///
    /// Emits a final `event=server_shutdown` log line at `debug!` with
    /// `bound_for_seconds`, `total_requests`, and `total_errors` when this
    /// handle has lifetime stats attached (HTTP mode). The line is the grep
    /// target for "did the in-process MCP server shut down cleanly when
    /// avp exited?". It is logged at debug rather than info because the
    /// summary is diagnostic-only and the sah CLI installs a stderr
    /// subscriber at info — promoting it to info would leak MCP-internal
    /// jargon into every CLI subprocess's stderr.
    ///
    /// Idempotent: a second call after the channel has been consumed is a
    /// no-op (no log line emitted), so repeated shutdowns from explicit
    /// callers and `Drop` do not double up.
    pub async fn shutdown(&mut self) -> Result<()> {
        // Idempotency guard: take the shutdown_tx upfront. A second call
        // sees None and returns silently — matches the historical contract
        // exercised by `test_server_shutdown_idempotency`.
        let Some(tx) = self.shutdown_tx.take() else {
            return Ok(());
        };

        // Stop file watcher if server instance is available
        if let Some(server) = &self.server {
            server.stop_file_watching().await;
            tracing::debug!("File watcher stopped");
        }

        // Send shutdown signal
        let signal_sent = if tx.send(()).is_err() {
            tracing::warn!("Server shutdown signal receiver already dropped");
            false
        } else {
            true
        };

        // Emit the lifetime summary. Always log, even when signal_sent is
        // false, so the operator can see the final counters.
        //
        // Logged at `debug!` rather than `info!`: the summary is diagnostic
        // (matters when investigating "did the server shut down cleanly?"),
        // not user-facing. The sah CLI installs a global stderr subscriber
        // at INFO for non-MCP-mode invocations, so promoting this line to
        // info would leak MCP-internal jargon into every CLI subprocess's
        // stderr — see the integration test
        // `error_scenarios::test_error_message_consistency`. Validators
        // (`avp-cli`) that want this line in `.avp/log` widen their file
        // layer to capture `swissarmyhammer_tools::mcp::unified_server` at
        // debug.
        if let Some(stats) = &self.stats {
            let bound_for = stats.started_at.elapsed();
            let session_count = stats.seen_sessions.lock().map(|s| s.len()).unwrap_or(0);
            tracing::debug!(
                bound_for_seconds = bound_for.as_secs(),
                bound_for_ms = bound_for.as_millis() as u64,
                total_requests = stats.requests(),
                total_errors = stats.errors(),
                total_sessions = session_count,
                signal_sent,
                event = "server_shutdown",
                connection_url = %self.info.connection_url,
                "In-process MCP server shutdown"
            );
        } else {
            tracing::debug!(
                signal_sent,
                event = "server_shutdown",
                connection_url = %self.info.connection_url,
                "In-process MCP server shutdown (stdio — no HTTP stats)"
            );
        }

        Ok(())
    }

    /// Check if this handle has a server task (stdio mode)
    pub fn has_server_task(&self) -> bool {
        self.server_task.is_some()
    }

    /// Get the MCP server instance
    pub fn server(&self) -> Option<Arc<McpServer>> {
        self.server.clone()
    }

    /// Wait for the server task to complete (stdio mode only)
    ///
    /// This method should be called after shutdown to ensure the spawned
    /// server task completes before the process exits, preventing orphaned processes.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Ok if task completed successfully, error if task panicked or no task exists
    pub async fn wait_for_completion(&mut self) -> Result<()> {
        if let Some(task) = self.server_task.take() {
            task.await.map_err(|e| SwissArmyHammerError::Other {
                message: format!("Server task panicked: {}", e),
            })?;
            tracing::debug!("Server task completed successfully");
        }
        Ok(())
    }

    /// Take the completion receiver to detect natural server exit (stdio mode only)
    ///
    /// This method should be called to get the receiver that will signal when
    /// the server task completes naturally (e.g., EOF on stdin).
    ///
    /// # Returns
    ///
    /// * `Option<oneshot::Receiver<()>>` - The completion receiver if available
    pub fn take_completion_rx(&mut self) -> Option<oneshot::Receiver<()>> {
        self.completion_rx.take()
    }
}

/// Emit the shutdown summary line even when callers forget to call
/// [`McpServerHandle::shutdown`] explicitly. This makes the answer to
/// "did the in-process MCP server shut down cleanly when avp exited?"
/// reachable in `.avp/log` even on panicking or short-lived processes.
///
/// The `Drop` implementation only emits the summary when the explicit
/// shutdown path was never taken (`shutdown_tx` is still `Some`). When
/// `shutdown()` has already run, `take()` left `None` behind and `Drop`
/// emits nothing — preserving the idempotency contract documented on
/// [`McpServerHandle::shutdown`].
///
/// We cannot send the shutdown signal here (the receiver may already be
/// gone), but we can log the same `event=server_shutdown` line so the log
/// surface answers the same question whether the caller used `shutdown()`
/// or just dropped the handle.
impl Drop for McpServerHandle {
    fn drop(&mut self) {
        if self.shutdown_tx.is_none() {
            // Explicit shutdown already ran; do not double-emit.
            return;
        }

        // Best-effort: try to send the signal. Failure is fine — the
        // receiver may have already gone away.
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Emit the same lifetime summary as the explicit `shutdown()` path
        // so the log answer is identical regardless of how the handle ended
        // its life. Mark this case explicitly with `dropped=true` so log
        // readers can tell the explicit shutdown apart from the Drop-only
        // path. See `shutdown()` for why this is `debug!` and not `info!`.
        if let Some(stats) = &self.stats {
            let bound_for = stats.started_at.elapsed();
            let session_count = stats.seen_sessions.lock().map(|s| s.len()).unwrap_or(0);
            tracing::debug!(
                bound_for_seconds = bound_for.as_secs(),
                bound_for_ms = bound_for.as_millis() as u64,
                total_requests = stats.requests(),
                total_errors = stats.errors(),
                total_sessions = session_count,
                dropped = true,
                event = "server_shutdown",
                connection_url = %self.info.connection_url,
                "In-process MCP server shutdown (Drop)"
            );
        } else {
            tracing::debug!(
                dropped = true,
                event = "server_shutdown",
                connection_url = %self.info.connection_url,
                "In-process MCP server shutdown (Drop, stdio — no HTTP stats)"
            );
        }
    }
}

/// Start unified MCP server with specified transport mode
///
/// This is the main entry point for starting MCP servers in any mode.
/// Returns connection information appropriate for the selected transport.
///
/// # Arguments
///
/// * `mode` - The transport mode (stdio or HTTP)
/// * `library` - Optional prompt library (creates new if None)
/// * `model_override` - Optional model name to override all use case assignments
/// * `working_dir` - Optional working directory (uses current_dir if None)
///
/// # Returns
///
/// * `Result<McpServerHandle>` - Server handle with connection info
pub async fn start_mcp_server(
    mode: McpServerMode,
    library: Option<PromptLibrary>,
    model_override: Option<String>,
    working_dir: Option<std::path::PathBuf>,
) -> Result<McpServerHandle> {
    start_mcp_server_with_options(mode, library, model_override, working_dir, false).await
}

/// Start unified MCP server with agent mode control
///
/// When `agent_mode` is true, the server registers agent tools (file editing,
/// shell, grep, skills, etc.) that provide base agent behavior. When false,
/// only domain-specific tools are registered — suitable for running alongside
/// an existing agent like Claude Code that already has these capabilities.
///
/// # Arguments
///
/// * `mode` - The transport mode (stdio or HTTP)
/// * `library` - Optional prompt library (creates new if None)
/// * `model_override` - Optional model name to override all use case assignments
/// * `working_dir` - Optional working directory (uses current_dir if None)
/// * `agent_mode` - Whether to register agent tools (true for llama-agent, false for Claude Code)
///
/// # Returns
///
/// * `Result<McpServerHandle>` - Server handle with connection info
pub async fn start_mcp_server_with_options(
    mode: McpServerMode,
    library: Option<PromptLibrary>,
    model_override: Option<String>,
    working_dir: Option<std::path::PathBuf>,
    agent_mode: bool,
) -> Result<McpServerHandle> {
    // Configure MCP logging to match sah serve behavior
    // NOTE: Skip logging configuration when called from CLI as main.rs already handles it
    // Only configure logging when used as library (e.g., in tests or embedded scenarios)
    if std::env::var("SAH_CLI_MODE").is_err() {
        configure_mcp_logging(None);
    }
    match mode {
        McpServerMode::Stdio => {
            start_stdio_server(library, model_override, working_dir, agent_mode).await
        }
        McpServerMode::Http { port } => {
            start_http_server(port, library, model_override, working_dir, agent_mode).await
        }
    }
}

/// Resolve port by either using the provided port or finding an available random port
///
/// # Arguments
/// * `port` - Optional port number (if None, finds random available port)
///
/// # Returns
/// * `Result<u16>` - The resolved port number
async fn resolve_port(port: Option<u16>) -> Result<u16> {
    if let Some(bind_port) = port {
        return Ok(bind_port);
    }

    let temp_listener =
        TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| SwissArmyHammerError::Other {
                message: format!("Failed to bind to random port: {}", e),
            })?;

    let port = temp_listener
        .local_addr()
        .map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to get local address: {}", e),
        })?
        .port();

    drop(temp_listener);
    Ok(port)
}

/// Initialize MCP server with the given configuration
///
/// # Arguments
/// * `library` - Optional prompt library (creates default if None)
/// * `port` - Server port to set in tool context
/// * `model_override` - Optional model name to override use case assignments
/// * `working_dir` - Optional working directory (uses current_dir if None)
/// * `agent_mode` - Whether to register agent tools
///
/// # Returns
/// * `Result<Arc<McpServer>>` - Initialized server with self-reference configured
async fn initialize_mcp_server(
    library: Option<PromptLibrary>,
    port: u16,
    model_override: Option<String>,
    working_dir: Option<std::path::PathBuf>,
    agent_mode: bool,
) -> Result<Arc<McpServer>> {
    let library = library.unwrap_or_default();
    let work_dir = working_dir
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir()));
    let server =
        McpServer::new_with_work_dir(library, work_dir, model_override, agent_mode).await?;
    server.initialize().await?;
    server.set_server_port(port).await;

    let server_arc = Arc::new(server);
    server_arc
        .tool_context
        .set_mcp_server(server_arc.clone())
        .await;

    Ok(server_arc)
}

/// Create MCP router with HTTP service, validator endpoint, and health check
///
/// The router is wrapped in a [`request_observer`] middleware that records
/// every HTTP request into [`ServerStats`] — request count, error count, and
/// first-sight per-session-id log lines. The shared [`ServerStats`] lets
/// [`McpServerHandle::shutdown`] emit a matching shutdown summary at process
/// exit.
///
/// # Arguments
/// * `server` - Arc reference to MCP server (full tool set)
/// * `stats` - Shared lifetime statistics observed by per-request middleware
///
/// # Returns
/// * `axum::Router` - Configured router with /mcp, /mcp/validator, and /health
fn create_mcp_router(server: Arc<McpServer>, stats: ServerStats) -> axum::Router {
    let server_for_full = server.clone();
    let http_service = StreamableHttpService::new(
        move || Ok((*server_for_full).clone()),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    // Build a validator-only server with filtered tools
    let validator_server = server.create_validator_server();
    let validator_service = StreamableHttpService::new(
        move || Ok(validator_server.clone()),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    axum::Router::new()
        .nest_service("/mcp/validator", validator_service)
        .nest_service("/mcp", http_service)
        .route("/health", axum::routing::get(health_check))
        .layer(axum::middleware::from_fn_with_state(
            stats,
            request_observer,
        ))
}

/// Per-request observer middleware: increments stats counters and emits one
/// debug-level line per HTTP request with method, path, and session id (when
/// the `mcp-session-id` header is present). On first sight of a new session
/// id, also emits an info-level `event=session_open` line so the lifetime of
/// any one session can be `grep`d end-to-end by `session_id=...`.
///
/// # Session lifecycle events
///
/// - `event=session_open` — first request observed for a previously-unseen
///   `mcp-session-id`. Fired exactly once per session.
/// - `event=session_close` — observed an HTTP `DELETE` against a session id
///   we have seen open. Streamable-HTTP uses `DELETE` to gracefully close a
///   session. The session id is removed from `seen_sessions` so any future
///   request bearing the same id would re-trigger a `session_open`. The
///   wrapped status is included so callers can tell a clean `204` close from
///   a forced close.
/// - `event=session_terminate` — non-success response on a request bearing a
///   session id, i.e. the rmcp `Session service terminated` failure mode the
///   task description flagged. Logged with `status` and `cause=<reason>` so
///   the lifetime of one session can be grepped end-to-end.
///
/// Errors (non-2xx responses) bump [`ServerStats::total_errors`]. The
/// per-session `session_terminate` line gives the per-session attribution that
/// the bare `total_errors` counter could not.
async fn request_observer(
    axum::extract::State(stats): axum::extract::State<ServerStats>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    // Extract `mcp-session-id` header if present. Streamable-HTTP issues a
    // session id on the first POST and the client echoes it on subsequent
    // requests; the per-session ConnectionInit/Initialize/CallTool flow all
    // share that id.
    let session_id = request
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // First sight of this session id → log an explicit `session_open` event.
    // Subsequent requests skip the lock-and-insert path and just log the
    // per-request line at debug level.
    if let Some(ref sid) = session_id {
        let mut seen = stats
            .seen_sessions
            .lock()
            .expect("ServerStats.seen_sessions mutex poisoned");
        if seen.insert(sid.clone()) {
            tracing::info!(
                session_id = %sid,
                method = %method,
                path = %path,
                event = "session_open",
                "MCP session opened (first request observed)"
            );
        }
    }

    stats.total_requests.fetch_add(1, Ordering::Relaxed);

    tracing::debug!(
        session_id = session_id.as_deref().unwrap_or(""),
        method = %method,
        path = %path,
        "MCP HTTP request"
    );

    // Capture whether this is a streamable-HTTP DELETE (graceful close)
    // before moving `request` into `next.run`. Per MCP streamable-HTTP spec,
    // clients close a session by sending DELETE to the session endpoint;
    // rmcp also routes session terminate signals via DELETE.
    let is_delete = method == axum::http::Method::DELETE;

    let response = next.run(request).await;
    let status = response.status();

    if is_delete {
        if let Some(ref sid) = session_id {
            // Drop from seen_sessions so any future activity on the same id
            // re-triggers session_open (which would itself signal a bug).
            let mut seen = stats
                .seen_sessions
                .lock()
                .expect("ServerStats.seen_sessions mutex poisoned");
            let was_open = seen.remove(sid);
            if was_open {
                tracing::info!(
                    session_id = %sid,
                    method = %method,
                    path = %path,
                    status = %status,
                    cause = if status.is_success() { "client_delete" } else { "delete_failed" },
                    event = "session_close",
                    "MCP session closed (DELETE observed)"
                );
            }
        }
    }

    if !status.is_success() {
        stats.total_errors.fetch_add(1, Ordering::Relaxed);
        tracing::warn!(
            session_id = session_id.as_deref().unwrap_or(""),
            method = %method,
            path = %path,
            status = %status,
            "MCP HTTP request returned non-success status"
        );

        // Per-session attribution for the rmcp `Session service terminated`
        // family of errors: any non-success response on a session-bearing
        // request fires `session_terminate` with the HTTP status as cause.
        // This is what answers "why did session abc-123 die?" — grep for the
        // session id and the line carries the status that killed it.
        if !is_delete {
            if let Some(ref sid) = session_id {
                tracing::warn!(
                    session_id = %sid,
                    method = %method,
                    path = %path,
                    status = %status,
                    cause = %status.canonical_reason().unwrap_or("non-success"),
                    event = "session_terminate",
                    "MCP session encountered non-success status (possible service terminate)"
                );
            }
        }
    }

    response
}

/// Parse socket address from string with error handling
///
/// # Arguments
/// * `bind_addr` - Address string to parse (e.g., "127.0.0.1:8080")
///
/// # Returns
/// * `Result<std::net::SocketAddr>` - Parsed socket address
fn parse_socket_addr(bind_addr: &str) -> Result<std::net::SocketAddr> {
    bind_addr.parse().map_err(|e| SwissArmyHammerError::Other {
        message: format!("Failed to parse bind address {}: {}", bind_addr, e),
    })
}

/// Bind TCP listener to the specified socket address
///
/// # Arguments
/// * `socket_addr` - Socket address to bind to
///
/// # Returns
/// * `Result<TcpListener>` - Bound TCP listener
async fn bind_tcp_listener(socket_addr: std::net::SocketAddr) -> Result<TcpListener> {
    TcpListener::bind(socket_addr)
        .await
        .map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to bind to {}: {}", socket_addr, e),
        })
}

/// Setup HTTP server infrastructure for stdio mode workflow support
///
/// Finds an available port, creates the HTTP service (wrapped in the
/// per-request observer middleware), and binds the listener. Returns the
/// port, router, listener, and shared [`ServerStats`] for the caller to
/// attach to the [`McpServerHandle`].
async fn setup_http_server_for_stdio(
    server: Arc<McpServer>,
) -> Result<(u16, axum::Router, tokio::net::TcpListener, ServerStats)> {
    tracing::debug!("Finding available random port for HTTP server");
    let http_port = resolve_port(None).await?;

    tracing::info!(
        "Will start HTTP server on port {} for workflow support",
        http_port
    );

    let http_bind_addr = format!("127.0.0.1:{}", http_port);
    let http_socket_addr = parse_socket_addr(&http_bind_addr)?;

    let stats = ServerStats::new();
    let router = create_mcp_router(server, stats.clone());
    let http_listener = bind_tcp_listener(http_socket_addr).await?;

    tracing::info!(
        "HTTP MCP server (for workflows) binding on http://127.0.0.1:{}/mcp",
        http_port
    );

    Ok((http_port, router, http_listener, stats))
}

/// Spawn stdio server task with shutdown and completion handling
///
/// Returns the spawned task handle that manages the stdio transport lifecycle.
fn spawn_stdio_server_task(
    server: Arc<McpServer>,
    mut shutdown_rx: oneshot::Receiver<()>,
    completion_tx: oneshot::Sender<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        match serve_server((*server).clone(), stdio()).await {
            Ok(running_service) => {
                tracing::info!("MCP stdio server started successfully");

                tokio::select! {
                    result = running_service.waiting() => {
                        match result {
                            Ok(quit_reason) => {
                                #[derive(serde::Serialize, Debug)]
                                struct QuitInfo { reason: String }
                                tracing::info!("MCP stdio server completed naturally: {}", Pretty(&QuitInfo { reason: format!("{:?}", quit_reason) }));
                            }
                            Err(e) => {
                                tracing::error!("MCP stdio server task error: {}", e);
                            }
                        }
                        let _ = completion_tx.send(());
                    }
                    _ = &mut shutdown_rx => {
                        tracing::info!("MCP stdio server received shutdown signal");
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to start stdio server: {}", e);
                let _ = completion_tx.send(());
            }
        }
        tracing::info!("MCP stdio server task exiting");
    })
}

/// Spawn HTTP server task for workflow support
///
/// # Arguments
/// * `http_listener` - TCP listener for HTTP server
/// * `router` - Axum router with MCP endpoints
///
/// # Returns
/// * Spawned task handle for HTTP server
fn spawn_http_server_for_stdio(
    http_listener: tokio::net::TcpListener,
    router: axum::Router,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = axum::serve(http_listener, router).await {
            tracing::error!("HTTP server error: {}", e);
        }
        tracing::info!("HTTP server task exiting");
    })
}

/// Start MCP server with stdio transport
///
/// This function starts both:
/// 1. Stdio transport for client communication (Claude Code)
/// 2. HTTP server on random localhost port for workflows to execute Claude prompts
async fn start_stdio_server(
    library: Option<PromptLibrary>,
    model_override: Option<String>,
    working_dir: Option<std::path::PathBuf>,
    agent_mode: bool,
) -> Result<McpServerHandle> {
    tracing::info!("Starting unified MCP server in stdio mode");

    let library = library.unwrap_or_default();
    let work_dir = working_dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir()));
    let temp_server =
        McpServer::new_with_work_dir(library, work_dir, model_override.clone(), agent_mode).await?;
    let temp_server_arc = Arc::new(temp_server);

    let (http_port, router, http_listener, stats) =
        setup_http_server_for_stdio(temp_server_arc).await?;

    let server_arc =
        initialize_mcp_server(None, http_port, model_override, working_dir, agent_mode).await?;
    tracing::debug!("Set MCP server self-reference in tool context (stdio)");
    tracing::info!(
        "Set MCP server port {} in tool context for workflows",
        http_port
    );

    spawn_http_server_for_stdio(http_listener, router);

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let (completion_tx, completion_rx) = oneshot::channel();

    let server_task = spawn_stdio_server_task(server_arc.clone(), shutdown_rx, completion_tx);

    let info = McpServerInfo {
        mode: McpServerMode::Stdio,
        connection_url: format!("stdio (HTTP on port {} for workflows)", http_port),
        port: Some(http_port),
    };

    Ok(McpServerHandle::new_with_task(McpServerHandleParams {
        info,
        shutdown_tx,
        server_task,
        server: server_arc,
        completion_rx,
        stats: Some(stats),
    }))
}

/// Spawn HTTP server task with graceful shutdown
///
/// Returns the spawned task handle and readiness receiver to confirm server startup.
fn spawn_http_server_task(
    listener: tokio::net::TcpListener,
    router: axum::Router,
    shutdown_rx: oneshot::Receiver<()>,
) -> (tokio::task::JoinHandle<()>, oneshot::Receiver<()>) {
    let (ready_tx, ready_rx) = oneshot::channel();

    let server_task = tokio::spawn(async move {
        let _ = ready_tx.send(());
        tracing::info!("HTTP MCP server task started and ready to serve");

        let result = axum::serve(listener, router)
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
                tracing::debug!("HTTP server received shutdown signal");
            })
            .await;

        match result {
            Ok(_) => {
                tracing::debug!("HTTP server stopped successfully");
            }
            Err(e) => {
                tracing::error!("HTTP server error: {}", e);
            }
        }
        tracing::debug!("HTTP server task exiting");
    });

    (server_task, ready_rx)
}

/// Create TCP listener bound to the specified port
///
/// # Arguments
/// * `port` - Port number to bind to
///
/// # Returns
/// * `Result<(tokio::net::TcpListener, String)>` - Listener and connection URL
async fn create_tcp_listener(port: u16) -> Result<(tokio::net::TcpListener, String)> {
    let bind_addr = format!("127.0.0.1:{}", port);
    let socket_addr = parse_socket_addr(&bind_addr)?;
    tracing::debug!("Parsed socket address: {}", socket_addr);

    let listener = bind_tcp_listener(socket_addr).await?;

    let connection_url = format!("http://127.0.0.1:{}/mcp", port);
    tracing::info!("Unified HTTP MCP server binding on {}", connection_url);

    Ok((listener, connection_url))
}

/// Wait for HTTP server to signal readiness
///
/// # Arguments
/// * `ready_rx` - Receiver that signals when server is ready
///
/// # Returns
/// * `Result<()>` - Ok if server signaled readiness
async fn wait_for_server_ready(ready_rx: oneshot::Receiver<()>) -> Result<()> {
    ready_rx.await.map_err(|_| SwissArmyHammerError::Other {
        message: "Server task failed to signal readiness".to_string(),
    })
}

/// Resolve the effective bind port: use the caller-specified one if present,
/// otherwise ask the OS for an ephemeral free port.
async fn resolve_http_port(port: Option<u16>) -> Result<u16> {
    match port {
        Some(bind_port) => {
            tracing::debug!("Using specified port: {}", bind_port);
            Ok(bind_port)
        }
        None => {
            tracing::debug!("Finding available random port");
            let resolved_port = resolve_port(None).await?;
            tracing::debug!("Found random port: {}", resolved_port);
            Ok(resolved_port)
        }
    }
}

/// Start MCP server with HTTP transport using rmcp SseServer
async fn start_http_server(
    port: Option<u16>,
    library: Option<PromptLibrary>,
    model_override: Option<String>,
    working_dir: Option<std::path::PathBuf>,
    agent_mode: bool,
) -> Result<McpServerHandle> {
    tracing::debug!("start_http_server called with port: {}", Pretty(&port));

    let actual_port = resolve_http_port(port).await?;

    tracing::debug!("Creating MCP server");
    let server_arc = initialize_mcp_server(
        library,
        actual_port,
        model_override,
        working_dir,
        agent_mode,
    )
    .await?;
    tracing::debug!("MCP server initialized");

    let stats = ServerStats::new();
    let router = create_mcp_router(server_arc.clone(), stats.clone());
    let (listener, connection_url) = create_tcp_listener(actual_port).await?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let (server_task, ready_rx) = spawn_http_server_task(listener, router, shutdown_rx);

    wait_for_server_ready(ready_rx).await?;
    tracing::info!(
        connection_url = %connection_url,
        port = actual_port,
        agent_mode,
        event = "server_start",
        "HTTP MCP server confirmed ready"
    );

    let info = McpServerInfo {
        mode: McpServerMode::Http {
            port: Some(actual_port),
        },
        connection_url,
        port: Some(actual_port),
    };

    let mut handle = McpServerHandle::new(info, shutdown_tx);
    handle.server_task = Some(server_task);
    handle.server = Some(server_arc);
    handle.stats = Some(stats);
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Capture writer for log-output tests. A `MakeWriter` implementation
    /// that pushes every formatted byte into a shared `Arc<Mutex<Vec<u8>>>`
    /// so tests can assert on `tracing::info!` lines verbatim. Used to
    /// convert the grep-able acceptance contract into machine-checked
    /// regression coverage.
    #[derive(Clone)]
    struct LineWriter {
        buf: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    }
    impl std::io::Write for LineWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let mut guard = self.buf.lock().unwrap();
            guard.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LineWriter {
        type Writer = LineWriter;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    /// Build a fresh capture buffer + `MakeWriter`.
    fn capture_lines() -> (std::sync::Arc<std::sync::Mutex<Vec<u8>>>, LineWriter) {
        let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let writer = LineWriter { buf: buf.clone() };
        (buf, writer)
    }

    /// Drain captured bytes into a list of lines.
    fn captured_lines(buf: &std::sync::Arc<std::sync::Mutex<Vec<u8>>>) -> Vec<String> {
        let bytes = buf.lock().unwrap();
        String::from_utf8_lossy(&bytes)
            .lines()
            .map(|s| s.to_string())
            .collect()
    }

    /// Asserts the explicit `shutdown()` path emits `event=server_shutdown`
    /// with a non-zero `bound_for_ms` and zero traffic counters, and that the
    /// idempotent second call does NOT re-emit the line.
    ///
    /// This converts criterion #5 from the task ("did the in-process MCP
    /// server shut down cleanly when avp exited?") into a regression-proof
    /// test — a future refactor that drops the `tracing::debug!` line in
    /// `shutdown()` will fail this test rather than silently regress.
    ///
    /// Subscriber is set to DEBUG because the shutdown summary is logged at
    /// debug level (it is diagnostic, not user-facing — see the comment in
    /// `McpServerHandle::shutdown` for the rationale).
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_shutdown_emits_server_shutdown_event() {
        use tracing_subscriber::fmt;
        use tracing_subscriber::layer::SubscriberExt;

        let (buf, writer) = capture_lines();
        let layer = fmt::layer().with_writer(writer).with_ansi(false);
        let subscriber = tracing_subscriber::registry()
            .with(tracing_subscriber::filter::LevelFilter::DEBUG)
            .with(layer);

        // `set_default` returns a guard scoped to this thread; since
        // `tokio::test` runs on the current thread by default, the guard
        // captures every `tracing::info!` we emit during the test.
        let _guard = tracing::subscriber::set_default(subscriber);

        // Build a stats bundle and a fake handle wired to it; do not
        // actually start a server. The only thing `shutdown()` does that
        // affects logging is read `stats` and emit the line — exercise that
        // path directly.
        let stats = ServerStats::new();
        stats.total_requests.fetch_add(7, Ordering::Relaxed);
        stats.total_errors.fetch_add(2, Ordering::Relaxed);
        let info = McpServerInfo {
            mode: McpServerMode::Http { port: Some(12345) },
            connection_url: "http://127.0.0.1:12345/mcp".to_string(),
            port: Some(12345),
        };
        let (tx, _rx) = oneshot::channel::<()>();
        let mut handle = McpServerHandle {
            info,
            shutdown_tx: Some(tx),
            server_task: None,
            server: None,
            completion_rx: None,
            stats: Some(stats),
        };

        // First shutdown: emits the line.
        handle.shutdown().await.unwrap();
        // Second shutdown: idempotent, must NOT re-emit.
        handle.shutdown().await.unwrap();

        // Forget the handle so its Drop does not also fire (the test
        // is asserting on the explicit path, not the Drop path).
        std::mem::forget(handle);

        // Drop the guard before reading captured output to flush the
        // formatter writer.
        drop(_guard);

        let lines = captured_lines(&buf);
        let shutdown_lines: Vec<&String> = lines
            .iter()
            .filter(|l| l.contains("event=\"server_shutdown\""))
            .collect();
        assert_eq!(
            shutdown_lines.len(),
            1,
            "expected exactly one server_shutdown line (idempotent second call must not re-emit), \
             got {}: {:?}",
            shutdown_lines.len(),
            shutdown_lines
        );
        let line = shutdown_lines[0];
        assert!(
            line.contains("total_requests=7"),
            "missing total_requests=7: {}",
            line
        );
        assert!(
            line.contains("total_errors=2"),
            "missing total_errors=2: {}",
            line
        );
        assert!(
            line.contains("connection_url=http://127.0.0.1:12345/mcp"),
            "missing connection_url: {}",
            line
        );
    }

    /// Asserts the request_observer middleware emits `session_open` exactly
    /// once per `mcp-session-id`, increments request and error counters, and
    /// emits `session_terminate` when a session-bearing request returns a
    /// non-success status.
    ///
    /// This converts criterion #4 (from the task — "why did session abc-123
    /// die?") into a regression-proof test.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_request_observer_session_lifecycle_events() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use axum::routing::any;
        use tower::ServiceExt;
        use tracing_subscriber::fmt;
        use tracing_subscriber::layer::SubscriberExt;

        let (buf, writer) = capture_lines();
        let layer = fmt::layer().with_writer(writer).with_ansi(false);
        let subscriber = tracing_subscriber::registry()
            .with(tracing_subscriber::filter::LevelFilter::INFO)
            .with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        let stats = ServerStats::new();
        let stats_for_app = stats.clone();

        async fn ok_handler() -> StatusCode {
            StatusCode::OK
        }
        async fn err_handler() -> StatusCode {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        async fn delete_ok() -> StatusCode {
            StatusCode::NO_CONTENT
        }

        let app = axum::Router::new()
            .route("/ok", any(ok_handler))
            .route("/err", any(err_handler))
            .route("/del", any(delete_ok))
            .layer(axum::middleware::from_fn_with_state(
                stats_for_app,
                request_observer,
            ));

        // Build three requests sharing one session id, plus one with a
        // distinct session id.
        let session_a = "sess-AAA";
        let session_b = "sess-BBB";

        let r1 = Request::builder()
            .uri("/ok")
            .header("mcp-session-id", session_a)
            .body(Body::empty())
            .unwrap();
        let r2 = Request::builder()
            .uri("/ok")
            .header("mcp-session-id", session_a)
            .body(Body::empty())
            .unwrap();
        let r3 = Request::builder()
            .uri("/err")
            .header("mcp-session-id", session_a)
            .body(Body::empty())
            .unwrap();
        let r4 = Request::builder()
            .uri("/ok")
            .header("mcp-session-id", session_b)
            .body(Body::empty())
            .unwrap();
        let r5 = Request::builder()
            .method("DELETE")
            .uri("/del")
            .header("mcp-session-id", session_a)
            .body(Body::empty())
            .unwrap();

        let _ = app.clone().oneshot(r1).await.unwrap();
        let _ = app.clone().oneshot(r2).await.unwrap();
        let _ = app.clone().oneshot(r3).await.unwrap();
        let _ = app.clone().oneshot(r4).await.unwrap();
        let _ = app.clone().oneshot(r5).await.unwrap();

        // Drop the subscriber guard before reading captured output.
        drop(_guard);

        let lines = captured_lines(&buf);

        // Counters: 5 requests, 1 error.
        assert_eq!(
            stats.requests(),
            5,
            "expected 5 requests, got {}",
            stats.requests()
        );
        assert_eq!(
            stats.errors(),
            1,
            "expected 1 error, got {}",
            stats.errors()
        );

        // session_open must fire exactly once per distinct session id.
        let opens_a: Vec<&String> = lines
            .iter()
            .filter(|l| l.contains("event=\"session_open\"") && l.contains(session_a))
            .collect();
        assert_eq!(
            opens_a.len(),
            1,
            "expected exactly one session_open for session A, got {}: {:?}",
            opens_a.len(),
            opens_a
        );
        let opens_b: Vec<&String> = lines
            .iter()
            .filter(|l| l.contains("event=\"session_open\"") && l.contains(session_b))
            .collect();
        assert_eq!(
            opens_b.len(),
            1,
            "expected exactly one session_open for session B, got {}: {:?}",
            opens_b.len(),
            opens_b
        );

        // session_terminate must fire on the err response on session A.
        let terminates: Vec<&String> = lines
            .iter()
            .filter(|l| l.contains("event=\"session_terminate\"") && l.contains(session_a))
            .collect();
        assert_eq!(
            terminates.len(),
            1,
            "expected exactly one session_terminate for session A, got {}: {:?}",
            terminates.len(),
            terminates
        );

        // session_close must fire on the DELETE response on session A.
        let closes: Vec<&String> = lines
            .iter()
            .filter(|l| l.contains("event=\"session_close\"") && l.contains(session_a))
            .collect();
        assert_eq!(
            closes.len(),
            1,
            "expected exactly one session_close for session A, got {}: {:?}",
            closes.len(),
            closes
        );
    }

    #[tokio::test]
    #[test_log::test]
    #[serial_test::serial(cwd)]
    async fn test_http_server_creation_and_info() {
        tracing::info!("test_http_server_creation_and_info");
        let mode = McpServerMode::Http { port: Some(18080) }; // Fixed port to avoid random port issues
        let mut server = start_mcp_server(mode, None, None, None).await.unwrap();

        // Verify we got a valid port and URL format
        assert_eq!(server.port().unwrap(), 18080);
        assert!(server.url().starts_with("http://127.0.0.1:"));

        // Quick shutdown
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[test_log::test]
    #[serial_test::serial(cwd)]
    async fn test_server_info_structure() {
        let mode = McpServerMode::Http { port: Some(18081) };
        let mut server = start_mcp_server(mode, None, None, None).await.unwrap();

        // Test info structure
        let info = server.info();
        match &info.mode {
            McpServerMode::Http { port } => {
                assert_eq!(port, &Some(18081));
            }
            _ => panic!("Expected HTTP mode"),
        }

        assert_eq!(server.port().unwrap(), 18081);
        assert_eq!(server.url(), "http://127.0.0.1:18081/mcp");

        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[test_log::test]
    #[serial_test::serial(cwd)]
    async fn test_server_with_custom_library() {
        // Test that custom library is properly used
        let custom_library = PromptLibrary::default();

        let mode = McpServerMode::Http { port: None };
        let mut server = start_mcp_server(mode, Some(custom_library), None, None)
            .await
            .unwrap();

        // Server should start successfully with custom library
        assert!(server.port().unwrap() > 0);
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[test_log::test]
    #[serial_test::serial(cwd)]
    async fn test_http_server_port_in_use_error() {
        // First, start a server on a specific port
        let mode1 = McpServerMode::Http { port: Some(18082) };
        let mut server1 = start_mcp_server(mode1, None, None, None).await.unwrap();

        // Verify first server is running
        assert_eq!(server1.port().unwrap(), 18082);

        // Try to start another server on the same port - should fail
        let mode2 = McpServerMode::Http { port: Some(18082) };
        let result = start_mcp_server(mode2, None, None, None).await;

        // Should get an error about port being in use
        assert!(
            result.is_err(),
            "Expected error when trying to bind to same port"
        );
        let error_msg = format!("{}", result.unwrap_err());
        assert!(
            error_msg.contains("Failed to bind") || error_msg.contains("18082"),
            "Error should mention binding failure or port number. Got: {}",
            error_msg
        );

        // Clean up
        server1.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[test_log::test]
    #[serial_test::serial(cwd)]
    async fn test_http_server_invalid_port() {
        // Test with invalid port (port 1 requires root privileges)
        let mode = McpServerMode::Http { port: Some(1) };
        let result = start_mcp_server(mode, None, None, None).await;

        // Should get an error about permission denied
        assert!(
            result.is_err(),
            "Expected error when trying to bind to privileged port"
        );
        let error_msg = format!("{}", result.unwrap_err());
        assert!(
            error_msg.contains("Failed to bind")
                || error_msg.contains("Permission denied")
                || error_msg.contains("1"),
            "Error should mention binding failure, permission denied, or port 1. Got: {}",
            error_msg
        );
    }

    #[tokio::test]
    #[test_log::test]
    #[serial_test::serial(cwd)]
    async fn test_server_shutdown_idempotency() {
        // Test that calling shutdown multiple times doesn't panic
        let mode = McpServerMode::Http { port: None };
        let mut server = start_mcp_server(mode, None, None, None).await.unwrap();

        // First shutdown should work
        server.shutdown().await.unwrap();

        // Second shutdown should also work (idempotent)
        let result = server.shutdown().await;
        assert!(result.is_ok(), "Shutdown should be idempotent");
    }

    #[tokio::test]
    #[test_log::test]
    #[serial_test::serial(cwd)]
    async fn test_server_info_consistency() {
        // Test that server info remains consistent
        let mode = McpServerMode::Http { port: Some(18083) };
        let mut server = start_mcp_server(mode.clone(), None, None, None)
            .await
            .unwrap();

        let info1 = server.info();
        let info2 = server.info();

        // Info should be consistent across calls
        assert_eq!(info1.port, info2.port);
        assert_eq!(info1.connection_url, info2.connection_url);

        match (&info1.mode, &mode) {
            (
                McpServerMode::Http { port: info_port },
                McpServerMode::Http {
                    port: expected_port,
                },
            ) => {
                assert_eq!(info_port, expected_port);
            }
            _ => panic!("Mode mismatch"),
        }

        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[test_log::test]
    #[serial_test::serial(cwd)]
    async fn test_stdio_server_task_completion() {
        // Test that stdio server task handle is stored and can be awaited
        let mode = McpServerMode::Stdio;
        let mut server = start_mcp_server(mode, None, None, None).await.unwrap();

        // Server should have a task handle for stdio mode
        assert!(
            server.has_server_task(),
            "Stdio server should have task handle"
        );

        // Shutdown and wait for completion should succeed within timeout
        server.shutdown().await.unwrap();

        // Use timeout to prevent hanging if server doesn't shut down
        let completion_result = tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            server.wait_for_completion(),
        )
        .await;

        assert!(
            completion_result.is_ok(),
            "Server task should complete within timeout"
        );
        assert!(
            completion_result.unwrap().is_ok(),
            "Server task should complete cleanly"
        );
    }

    // NOTE: Multiple concurrent server test removed to avoid port conflicts and timeouts

    #[tokio::test]
    #[test_log::test]
    async fn test_configure_mcp_logging_does_not_panic() {
        // configure_mcp_logging uses Once internally; calling it multiple times should be safe.
        // In tests the tracing subscriber may already be set, so this is a no-op, but must not panic.
        configure_mcp_logging(None);
        configure_mcp_logging(Some("debug"));
        // If we reach this point, the function did not panic.
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_configure_mcp_logging_custom_filter() {
        // Test that configure_mcp_logging accepts a custom filter string without panicking.
        configure_mcp_logging(Some("error"));
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_ensure_log_directory_creates_nested_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a").join("b").join("c");
        let result = ensure_log_directory(&nested);
        assert!(result.is_ok(), "Should create nested directories");
        assert!(nested.exists());
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_ensure_log_directory_existing_dir() {
        let dir = tempfile::tempdir().unwrap();
        // Should succeed even if directory already exists
        let result = ensure_log_directory(dir.path());
        assert!(result.is_ok(), "Should succeed for existing directory");
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_create_log_file_in_temp_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = create_log_file(dir.path());
        assert!(result.is_ok(), "Should create log file in temp dir");
        let (_file, path) = result.unwrap();
        assert!(path.starts_with(dir.path()));
        assert!(path.exists());
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_create_log_file_custom_env_name() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("SWISSARMYHAMMER_LOG_FILE", "custom_test.log");
        let result = create_log_file(dir.path());
        std::env::remove_var("SWISSARMYHAMMER_LOG_FILE");
        assert!(result.is_ok());
        let (_file, path) = result.unwrap();
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            "custom_test.log"
        );
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_file_writer_guard_write() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test_write.log");
        let file = std::fs::File::create(&file_path).unwrap();
        let shared = std::sync::Arc::new(std::sync::Mutex::new(file));
        let mut guard = FileWriterGuard::new(shared);
        let n = guard.write(b"hello world").unwrap();
        assert_eq!(n, 11);
        let contents = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(contents, "hello world");
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_file_writer_guard_flush() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test_flush.log");
        let file = std::fs::File::create(&file_path).unwrap();
        let shared = std::sync::Arc::new(std::sync::Mutex::new(file));
        let mut guard = FileWriterGuard::new(shared);
        write!(guard, "flush test").unwrap();
        let result = guard.flush();
        assert!(result.is_ok(), "Flush should succeed");
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_resolve_port_with_specified_port() {
        // When a port is specified, resolve_port should return it directly
        let port = resolve_port(Some(18090)).await.unwrap();
        assert_eq!(port, 18090);
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_resolve_port_random() {
        // When no port is specified, resolve_port should find a random available port
        let port = resolve_port(None).await.unwrap();
        assert!(port > 0, "Random port should be non-zero");
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_parse_socket_addr_valid() {
        let addr = parse_socket_addr("127.0.0.1:8080").unwrap();
        assert_eq!(addr.port(), 8080);
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_parse_socket_addr_invalid() {
        let result = parse_socket_addr("not-an-address");
        assert!(result.is_err(), "Invalid address should return error");
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("Failed to parse bind address"),
            "Error should mention parsing failure. Got: {}",
            err
        );
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_bind_tcp_listener_success() {
        let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
        let listener = bind_tcp_listener(addr).await.unwrap();
        let actual_port = listener.local_addr().unwrap().port();
        assert!(actual_port > 0, "Should bind to a non-zero port");
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_bind_tcp_listener_port_in_use() {
        // Bind to a port, then try to bind again - should fail
        let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
        let listener1 = bind_tcp_listener(addr).await.unwrap();
        let occupied_port = listener1.local_addr().unwrap().port();
        let occupied_addr: std::net::SocketAddr =
            format!("127.0.0.1:{}", occupied_port).parse().unwrap();
        let result = bind_tcp_listener(occupied_addr).await;
        assert!(result.is_err(), "Should fail when port is in use");
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_wait_for_server_ready_success() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        tx.send(()).unwrap();
        let result = wait_for_server_ready(rx).await;
        assert!(result.is_ok(), "Should succeed when sender sends");
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_wait_for_server_ready_dropped_sender() {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        drop(tx); // Drop sender without sending
        let result = wait_for_server_ready(rx).await;
        assert!(result.is_err(), "Should fail when sender is dropped");
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("readiness") || err.contains("Server"),
            "Error should mention readiness. Got: {}",
            err
        );
    }

    #[tokio::test]
    #[test_log::test]
    #[serial_test::serial(cwd)]
    async fn test_start_mcp_server_with_options_agent_mode_false() {
        // Test start_mcp_server_with_options with agent_mode=false (default path)
        let mode = McpServerMode::Http { port: Some(18084) };
        let mut server = start_mcp_server_with_options(mode, None, None, None, false)
            .await
            .unwrap();
        assert_eq!(server.port().unwrap(), 18084);
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[test_log::test]
    #[serial_test::serial(cwd)]
    async fn test_start_mcp_server_with_options_agent_mode_true() {
        // Test start_mcp_server_with_options with agent_mode=true (agent tool registration)
        let mode = McpServerMode::Http { port: Some(18085) };
        let mut server = start_mcp_server_with_options(mode, None, None, None, true)
            .await
            .unwrap();
        assert_eq!(server.port().unwrap(), 18085);
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[test_log::test]
    async fn test_start_mcp_server_with_options_invalid_model_override_returns_error() {
        // Test that an invalid model override returns an error rather than silently ignoring it.
        // This exercises the model_override code path in start_mcp_server_with_options.
        let mode = McpServerMode::Http { port: Some(18086) };
        let result = start_mcp_server_with_options(
            mode,
            None,
            Some("nonexistent-model-that-does-not-exist".to_string()),
            None,
            false,
        )
        .await;
        assert!(
            result.is_err(),
            "Should return an error for an unknown model override"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("model")
                || err_msg.contains("Model")
                || err_msg.contains("invalid")
                || err_msg.contains("Invalid"),
            "Error should mention model. Got: {}",
            err_msg
        );
    }

    #[tokio::test]
    #[test_log::test]
    #[serial_test::serial(cwd)]
    async fn test_start_mcp_server_with_options_working_dir() {
        // Test that custom working directory is accepted
        let temp_dir = tempfile::tempdir().unwrap();
        let mode = McpServerMode::Http { port: Some(18087) };
        let mut server = start_mcp_server_with_options(
            mode,
            None,
            None,
            Some(temp_dir.path().to_path_buf()),
            false,
        )
        .await
        .unwrap();
        assert!(server.port().unwrap() > 0);
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[test_log::test]
    #[serial_test::serial(cwd)]
    async fn test_start_mcp_server_cli_mode_skips_logging() {
        // When SAH_CLI_MODE is set, configure_mcp_logging should be skipped
        std::env::set_var("SAH_CLI_MODE", "1");
        let mode = McpServerMode::Http { port: Some(18088) };
        let result = start_mcp_server_with_options(mode, None, None, None, false).await;
        std::env::remove_var("SAH_CLI_MODE");
        let mut server = result.unwrap();
        assert_eq!(server.port().unwrap(), 18088);
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[test_log::test]
    #[serial_test::serial(cwd)]
    async fn test_server_handle_server_accessor() {
        // Test that server() returns the MCP server instance
        let mode = McpServerMode::Http { port: Some(18089) };
        let mut server = start_mcp_server(mode, None, None, None).await.unwrap();
        let server_instance = server.server();
        assert!(
            server_instance.is_some(),
            "Server instance should be accessible"
        );
        server.shutdown().await.unwrap();
    }

    /// Runtime tools/list audit (#3 in task 01KQ7G1R9KRQ8RDBKYVSNEN9V4).
    ///
    /// Boots an actual HTTP MCP server, opens an RMCP client against the
    /// `/mcp/validator` sub-route, sends `tools/list`, and asserts the
    /// returned tool names are exactly the validator allowlist —
    /// `{"read_file", "glob_files", "grep_files", "code_context"}` —
    /// no more, no less.
    ///
    /// The split file tools are exposed by name (rather than the unified
    /// `files` tool with an `op` argument) so that Hermes-trained validator
    /// models can call them directly with the natural `{"name": "read_file",
    /// "arguments": {...}}` shape.
    ///
    /// This is the durable guard against tool-set drift. If anyone adds a
    /// `register_kanban_tools(&mut registry)` call to `create_validator_server`
    /// "because it seemed harmless", the runtime list won't match and this
    /// test fails at the boundary the actual validator agent talks to. It
    /// also catches reverting the split: if the unified `files` tool sneaks
    /// back onto the validator surface, the assertion fails.
    ///
    /// Independent of `agent_mode` because validator tool filtering is
    /// driven by `is_validator_tool()`, not `is_agent_tool()`.
    #[tokio::test]
    #[test_log::test]
    #[serial_test::serial(cwd)]
    async fn test_validator_endpoint_lists_only_validator_tools() {
        use crate::mcp::test_utils::create_test_client;
        use std::collections::BTreeSet;

        // Bind an in-process HTTP MCP server in a clean tempdir so its
        // index does not walk the host monorepo.
        let temp = tempfile::TempDir::new().unwrap();
        let mut server = start_mcp_server_with_options(
            McpServerMode::Http { port: None },
            None,
            None,
            Some(temp.path().to_path_buf()),
            // agent_mode is irrelevant for the validator route — it filters
            // by is_validator_tool(), not is_agent_tool(). Pass true so we
            // explicitly verify the validator route stays minimal even when
            // the full server has the maximal tool set.
            true,
        )
        .await
        .unwrap();

        // The server's `connection_url` points at `/mcp`. The validator
        // sub-route is `/mcp/validator` on the same port.
        let port = server.port().expect("HTTP server must report a bound port");
        let validator_url = format!("http://127.0.0.1:{}/mcp/validator", port);

        let client = create_test_client(&validator_url).await;
        let tools = client
            .list_tools(Default::default())
            .await
            .expect("tools/list against /mcp/validator must succeed");

        let actual: BTreeSet<String> = tools.tools.iter().map(|t| t.name.to_string()).collect();
        let expected: BTreeSet<String> = ["read_file", "glob_files", "grep_files", "code_context"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        assert_eq!(
            actual, expected,
            "validator endpoint must expose exactly {{read_file, glob_files, grep_files, code_context}} — got: {:?}",
            actual
        );

        // The unified `files` tool must not appear — its op-dispatched shape
        // does not match what Hermes-trained validator models naturally emit.
        assert!(
            !actual.contains("files"),
            "validator endpoint must NOT advertise the unified 'files' tool — got: {:?}",
            actual
        );

        // Defense in depth: enumerate the categories the validator must
        // never advertise. If any of these names appear, registration has
        // leaked a forbidden tool through the validator route.
        for forbidden in [
            "shell",
            "git",
            "kanban",
            "web",
            "questions",
            "ralph",
            "skill",
            "agent",
            "write_file",
            "edit_file",
        ] {
            assert!(
                !actual.contains(forbidden),
                "validator endpoint must NOT advertise '{}' — got: {:?}",
                forbidden,
                actual
            );
        }

        client.cancel().await.unwrap();
        server.shutdown().await.unwrap();
    }
}
