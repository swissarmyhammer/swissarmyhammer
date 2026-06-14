//! The CLI transport: an [`McpServer`] backed by a spawned child process that
//! speaks MCP JSON-RPC over its stdio.
//!
//! [`CliServer`] is the platform's subprocess transport. It spawns an external
//! command, performs the MCP `initialize` handshake against it as an MCP
//! *client*, and forwards every [`McpServer`] call to that child process as a
//! real `tools/list` or `tools/call` over the pipe. Where [`InProcessServer`]
//! is the zero-IPC transport for tools written in host Rust, `CliServer` is the
//! transport for a tool provider shipped as a standalone executable.
//!
//! The transport is built on the `rmcp` client SDK: the child process is
//! spawned through [`rmcp::transport::TokioChildProcess`], and the handshake
//! and request/response framing are driven by `rmcp`'s
//! [`serve_client`](rmcp::service::serve_client) and the resulting
//! [`Peer<RoleClient>`]. No JSON-RPC framing is hand-rolled here.
//!
//! ## Caller identity does not cross the process boundary
//!
//! [`McpServer::invoke`] takes a [`CallerId`], but MCP's `tools/call` has no
//! standard field for caller identity, so `CliServer` does **not** send it on
//! the wire. The child process sees only the tool name and the arguments map.
//! Caller-scoped access decisions are therefore the host's responsibility for
//! CLI-backed servers; the subprocess cannot make them.
//!
//! ## Subprocess lifecycle
//!
//! The spawned child is owned by the [`RunningService`] that `CliServer` holds.
//! When the `CliServer` is dropped — whether explicitly or by being
//! unregistered from the [`ServerRegistry`] — the `RunningService`'s drop guard
//! cancels the service, which closes the transport and kills the child. The
//! `rmcp` child-process transport additionally kills the child from its own
//! `Drop`, so a dropped `CliServer` never leaks the subprocess.
//!
//! A crashed subprocess is surfaced, not hidden: once the child exits, its
//! stdio pipes close, the `rmcp` `Peer` reports the connection as closed, and
//! every subsequent [`invoke`](CliServer::invoke) fails with
//! [`Error::ServerUnavailable`]. There is no automatic restart — a crash fails
//! cleanly rather than hanging or panicking.
//!
//! [`InProcessServer`]: crate::server::InProcessServer
//! [`ServerRegistry`]: crate::registry::ServerRegistry
//! [`Peer<RoleClient>`]: rmcp::service::Peer

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use rmcp::model::{CallToolRequestParams, Tool};
use rmcp::service::{serve_client, RoleClient, RunningService};
use rmcp::transport::TokioChildProcess;
use serde_json::Value;
use tokio::process::Command;

use crate::error::{Error, Result};
use crate::server::{CallerId, McpServer, ToolMetadata};

/// An [`McpServer`] backed by a child process that speaks MCP over its stdio.
///
/// A `CliServer` owns the spawned subprocess (through the `rmcp`
/// [`RunningService`] it holds) and a client [`Peer`](rmcp::service::Peer) into
/// it. [`tools`](McpServer::tools) is served from a cache filled at connect
/// time and refreshed when the subprocess sends `notifications/tools/list_changed`;
/// [`invoke`](McpServer::invoke) forwards a `tools/call` to the subprocess and
/// awaits the matching response.
///
/// Construct one with [`connect`](CliServer::connect).
pub struct CliServer {
    /// The running `rmcp` client service.
    ///
    /// Holding it keeps the background service loop alive and owns the child
    /// process; dropping it cancels the loop, closes the transport, and kills
    /// the subprocess.
    service: RunningService<RoleClient, ClientHandler>,
    /// The subprocess's process id, captured at spawn time.
    ///
    /// `None` only if the platform could not read it back from the OS. It is
    /// retained for diagnostics and lifecycle assertions; the wire protocol
    /// itself never uses it.
    child_pid: Option<u32>,
}

/// The `rmcp` client handler for a [`CliServer`] connection.
///
/// `rmcp` requires a client handler for every connection; it both supplies the
/// client's `initialize` info and receives server-to-client notifications. This
/// handler does the second job that matters to the platform: when the
/// subprocess announces `notifications/tools/list_changed`, the handler
/// re-lists the subprocess's tools and replaces the shared cache, so a later
/// [`CliServer::tools`] reflects the change.
///
/// The handler holds the shared tool cache and — once the connection is
/// established — a [`Peer`](rmcp::service::Peer) back into the subprocess used
/// to perform that refresh. The peer is wrapped in an [`RwLock`] holding an
/// [`Option`] because `rmcp` constructs the handler *before* it mints the peer:
/// the slot is empty until [`CliServer::connect`] fills it.
#[derive(Clone)]
struct ClientHandler {
    /// The subprocess's tool list, shared with the owning [`CliServer`].
    tools: Arc<RwLock<Vec<ToolMetadata>>>,
    /// A client peer into the subprocess, set once the connection is live.
    peer: Arc<RwLock<Option<rmcp::service::Peer<RoleClient>>>>,
}

impl rmcp::ClientHandler for ClientHandler {
    /// Refreshes the cached tool list when the subprocess's tools change.
    ///
    /// MCP servers emit `notifications/tools/list_changed` when their tool set
    /// changes at runtime. On that notification this handler re-runs
    /// `tools/list` against the subprocess and swaps the result into the shared
    /// cache. A failure to re-list (for example, a subprocess that crashed
    /// between announcing the change and answering the list) is logged and the
    /// previous cache is left in place rather than cleared.
    async fn on_tool_list_changed(&self, _context: rmcp::service::NotificationContext<RoleClient>) {
        // Take a clone of the peer and release the lock at once — the lock is
        // never held across the `list_all_tools` await below.
        let peer = self
            .peer
            .read()
            .expect("CliServer peer lock is never poisoned")
            .clone();
        let Some(peer) = peer else {
            return;
        };
        match peer.list_all_tools().await {
            Ok(listed) => {
                let refreshed = listed.into_iter().map(ToolMetadata::new).collect();
                *self
                    .tools
                    .write()
                    .expect("CliServer tools lock is never poisoned") = refreshed;
            }
            Err(error) => {
                tracing::warn!(
                    %error,
                    "CliServer: failed to refresh tools after list_changed notification"
                );
            }
        }
    }
}

impl CliServer {
    /// Spawns `cli` as a child process and connects to it as an MCP client.
    ///
    /// The first element of `cli` is the executable; the rest are its
    /// arguments. The child is spawned with its stdin and stdout piped (the
    /// MCP transport) and its stderr inherited (so subprocess diagnostics reach
    /// the host's terminal). This call performs the MCP `initialize` handshake,
    /// runs `tools/list` once, and caches the result so the synchronous
    /// [`tools`](McpServer::tools) can serve it.
    ///
    /// # Parameters
    ///
    /// - `cli` — the command and arguments to spawn; must be non-empty.
    /// - `env` — extra environment variables for the child, layered on top of
    ///   the host's environment; `None` leaves the host environment unchanged.
    /// - `cwd` — the child's working directory; `None` inherits the host's.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServerUnavailable`] when `cli` is empty, when the
    /// process cannot be spawned, when the MCP handshake fails, or when the
    /// initial `tools/list` fails.
    pub async fn connect(
        cli: Vec<String>,
        env: Option<HashMap<String, String>>,
        cwd: Option<PathBuf>,
    ) -> Result<Self> {
        let (program, args) = cli.split_first().ok_or(Error::ServerUnavailable)?;

        let mut command = Command::new(program);
        command.args(args);
        if let Some(env) = env {
            command.envs(env);
        }
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }

        let transport = TokioChildProcess::new(command).map_err(|error| {
            tracing::warn!(%error, "CliServer: failed to spawn subprocess");
            Error::ServerUnavailable
        })?;
        let child_pid = transport.id();

        let tools = Arc::new(RwLock::new(Vec::new()));
        let peer_slot = Arc::new(RwLock::new(None));
        let handler = ClientHandler {
            tools: Arc::clone(&tools),
            peer: Arc::clone(&peer_slot),
        };

        let service = serve_client(handler, transport).await.map_err(|error| {
            tracing::warn!(%error, "CliServer: MCP handshake with subprocess failed");
            Error::ServerUnavailable
        })?;

        // The peer only exists once the handshake completes; hand it to the
        // handler so a later list_changed notification can refresh the cache.
        *peer_slot
            .write()
            .expect("CliServer peer lock is never poisoned") = Some(service.peer().clone());

        let listed = service.peer().list_all_tools().await.map_err(|error| {
            tracing::warn!(%error, "CliServer: initial tools/list failed");
            Error::ServerUnavailable
        })?;
        *tools
            .write()
            .expect("CliServer tools lock is never poisoned") =
            listed.into_iter().map(ToolMetadata::new).collect();

        Ok(Self { service, child_pid })
    }

    /// Returns the process id of the spawned subprocess.
    ///
    /// `None` if the operating system did not report a pid for the child. The
    /// pid is intended for diagnostics and lifecycle checks — it is never sent
    /// over the MCP wire.
    pub fn child_pid(&self) -> Option<u32> {
        self.child_pid
    }

    /// Reads the current cached tool list.
    ///
    /// Shared by [`tools`](McpServer::tools) and [`invoke`](McpServer::invoke);
    /// both need a snapshot of the cache that the notification handler may
    /// concurrently replace. The lock is a synchronous [`RwLock`] and is held
    /// only for the duration of this clone, so reading it from an async task
    /// is safe — it never spans an `.await`.
    fn tools_snapshot(&self) -> Vec<ToolMetadata> {
        self.service
            .service()
            .tools
            .read()
            .expect("CliServer tools lock is never poisoned")
            .clone()
    }
}

#[async_trait]
impl McpServer for CliServer {
    /// Returns the subprocess's tool list as last cached.
    ///
    /// The list is filled at [`connect`](CliServer::connect) time from the
    /// subprocess's `tools/list` and refreshed whenever the subprocess sends
    /// `notifications/tools/list_changed`.
    fn tools(&self) -> Vec<ToolMetadata> {
        self.tools_snapshot()
    }

    /// Forwards `tool` and `input` to the subprocess as an MCP `tools/call`.
    ///
    /// The tool name and the `input` arguments map are sent over the stdio
    /// transport unchanged, and the matching `tools/call` response is awaited
    /// and returned as a [`Value`] — the same wire shape an MCP `tools/call`
    /// result carries. `caller` is **not** transmitted: see the module
    /// documentation on caller identity.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownTool`] when `tool` is absent from the
    /// subprocess's cached tool list, or [`Error::ServerUnavailable`] when the
    /// subprocess is no longer serving requests — including the case where it
    /// has crashed and its stdio has closed.
    async fn invoke(&self, _caller: CallerId, tool: &str, input: Value) -> Result<Value> {
        if !self.tools_snapshot().iter().any(|t| t.name() == tool) {
            return Err(Error::UnknownTool);
        }

        let mut request = CallToolRequestParams::new(tool.to_string());
        request.arguments = input.as_object().cloned();

        let result = self
            .service
            .peer()
            .call_tool(request)
            .await
            .map_err(map_service_error)?;

        serde_json::to_value(result).map_err(|_| Error::ServerUnavailable)
    }
}

/// Maps an `rmcp` client [`ServiceError`](rmcp::service::ServiceError) into the
/// platform [`Error`].
///
/// An MCP-level error whose code is
/// [`METHOD_NOT_FOUND`](rmcp::model::ErrorCode::METHOD_NOT_FOUND) means the
/// subprocess does not know the named tool, which becomes [`Error::UnknownTool`].
/// Every other failure — a transport that has closed because the subprocess
/// exited, a cancelled request, an unexpected response — means the subprocess
/// cannot serve the request, which becomes [`Error::ServerUnavailable`].
fn map_service_error(error: rmcp::service::ServiceError) -> Error {
    match error {
        rmcp::service::ServiceError::McpError(data)
            if data.code == rmcp::model::ErrorCode::METHOD_NOT_FOUND =>
        {
            Error::UnknownTool
        }
        _ => Error::ServerUnavailable,
    }
}

/// A no-op assertion that [`ToolMetadata`] still wraps an `rmcp` [`Tool`].
///
/// `connect` relies on `ToolMetadata::new` accepting an `rmcp` `Tool` straight
/// from `list_all_tools`; this binding makes that dependency explicit to the
/// compiler without affecting runtime behavior.
const _: fn(Tool) -> ToolMetadata = ToolMetadata::new;
