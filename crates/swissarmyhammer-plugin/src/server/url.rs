//! The URL transport: an [`McpServer`] backed by an HTTP-served MCP endpoint.
//!
//! [`UrlServer`] is the platform's remote transport. It connects to an MCP
//! server reachable over HTTP, performs the MCP `initialize` handshake against
//! it as an MCP *client*, and forwards every [`McpServer`] call to that
//! endpoint as a real `tools/list` or `tools/call` over HTTP. Where
//! [`InProcessServer`] is the zero-IPC transport for tools written in host Rust
//! and [`CliServer`] drives a spawned subprocess over stdio, `UrlServer` is the
//! transport for a tool provider that runs as a network service.
//!
//! The transport is built on the `rmcp` client SDK: the HTTP connection is
//! carried by [`rmcp::transport::StreamableHttpClientTransport`] — the client
//! half of MCP's Streamable HTTP transport, backed by a `reqwest` HTTP client —
//! and the handshake and request/response framing are driven by `rmcp`'s
//! [`serve_client`](rmcp::service::serve_client) and the resulting
//! [`Peer<RoleClient>`]. No JSON-RPC framing is hand-rolled here.
//!
//! ## Headers travel on every request
//!
//! The headers supplied at construction — typically an `Authorization` header
//! for a protected endpoint — are handed to the transport as its
//! [`custom_headers`](rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig::custom_headers).
//! `rmcp` attaches them to *every* HTTP request the transport makes: the
//! `initialize` handshake, each `tools/list`, and each `tools/call`. The
//! platform does not re-attach them per call — configuring them once on the
//! transport is what makes them reusable on every request.
//!
//! ## Caller identity does not cross the wire
//!
//! [`McpServer::invoke`] takes a [`CallerId`], but MCP's `tools/call` has no
//! standard field for caller identity, so `UrlServer` does **not** send it on
//! the wire. The remote endpoint sees only the tool name and the arguments
//! map. Caller-scoped access decisions are therefore the host's responsibility
//! for URL-backed servers; the remote endpoint cannot make them.
//!
//! ## Connection lifecycle
//!
//! The HTTP connection is owned by the [`RunningService`] that `UrlServer`
//! holds. When the `UrlServer` is dropped — whether explicitly or by being
//! unregistered from the [`ServerRegistry`] — the `RunningService`'s drop guard
//! cancels the service, which closes the transport.
//!
//! A remote endpoint that becomes unreachable is surfaced, not hidden: once the
//! HTTP transport can no longer reach the server, the `rmcp` `Peer` reports the
//! connection as closed, and every subsequent [`invoke`](UrlServer::invoke)
//! fails with [`Error::ServerUnavailable`]. There is no automatic reconnect — a
//! failure fails cleanly rather than hanging or panicking.
//!
//! [`InProcessServer`]: crate::server::InProcessServer
//! [`CliServer`]: crate::server::CliServer
//! [`ServerRegistry`]: crate::registry::ServerRegistry
//! [`Peer<RoleClient>`]: rmcp::service::Peer

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use http::{HeaderName, HeaderValue};
use rmcp::model::{CallToolRequestParams, Tool};
use rmcp::service::{serve_client, RoleClient, RunningService};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;
use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::server::{CallerId, McpServer, ToolMetadata};

/// An [`McpServer`] backed by an MCP endpoint reached over HTTP.
///
/// A `UrlServer` owns the HTTP connection (through the `rmcp`
/// [`RunningService`] it holds) and a client [`Peer`](rmcp::service::Peer) into
/// the remote endpoint. [`tools`](McpServer::tools) is served from a cache
/// filled at connect time and refreshed when the endpoint sends
/// `notifications/tools/list_changed`; [`invoke`](McpServer::invoke) forwards a
/// `tools/call` to the endpoint over HTTP and awaits the matching response.
///
/// Construct one with [`connect`](UrlServer::connect).
pub struct UrlServer {
    /// The running `rmcp` client service.
    ///
    /// Holding it keeps the background service loop alive and owns the HTTP
    /// transport; dropping it cancels the loop and closes the connection.
    service: RunningService<RoleClient, ClientHandler>,
}

/// The `rmcp` client handler for a [`UrlServer`] connection.
///
/// `rmcp` requires a client handler for every connection; it both supplies the
/// client's `initialize` info and receives server-to-client notifications. This
/// handler does the second job that matters to the platform: when the endpoint
/// announces `notifications/tools/list_changed`, the handler re-lists the
/// endpoint's tools and replaces the shared cache, so a later
/// [`UrlServer::tools`] reflects the change.
///
/// The handler holds the shared tool cache and — once the connection is
/// established — a [`Peer`](rmcp::service::Peer) back into the endpoint used to
/// perform that refresh. The peer is wrapped in an [`RwLock`] holding an
/// [`Option`] because `rmcp` constructs the handler *before* it mints the peer:
/// the slot is empty until [`UrlServer::connect`] fills it.
#[derive(Clone)]
struct ClientHandler {
    /// The endpoint's tool list, shared with the owning [`UrlServer`].
    tools: Arc<RwLock<Vec<ToolMetadata>>>,
    /// A client peer into the endpoint, set once the connection is live.
    peer: Arc<RwLock<Option<rmcp::service::Peer<RoleClient>>>>,
}

impl rmcp::ClientHandler for ClientHandler {
    /// Refreshes the cached tool list when the endpoint's tools change.
    ///
    /// MCP servers emit `notifications/tools/list_changed` when their tool set
    /// changes at runtime. On that notification this handler re-runs
    /// `tools/list` against the endpoint and swaps the result into the shared
    /// cache. A failure to re-list (for example, an endpoint that became
    /// unreachable between announcing the change and answering the list) is
    /// logged and the previous cache is left in place rather than cleared.
    async fn on_tool_list_changed(&self, _context: rmcp::service::NotificationContext<RoleClient>) {
        // Take a clone of the peer and release the lock at once — the lock is
        // never held across the `list_all_tools` await below.
        let peer = self
            .peer
            .read()
            .expect("UrlServer peer lock is never poisoned")
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
                    .expect("UrlServer tools lock is never poisoned") = refreshed;
            }
            Err(error) => {
                tracing::warn!(
                    %error,
                    "UrlServer: failed to refresh tools after list_changed notification"
                );
            }
        }
    }
}

impl UrlServer {
    /// Connects to the MCP endpoint at `url` as an MCP client.
    ///
    /// This builds an `rmcp` Streamable HTTP client transport for `url` with
    /// `headers` applied to every request, performs the MCP `initialize`
    /// handshake, runs `tools/list` once, and caches the result so the
    /// synchronous [`tools`](McpServer::tools) can serve it.
    ///
    /// # Parameters
    ///
    /// - `url` — the HTTP(S) URL of the MCP endpoint to connect to.
    /// - `headers` — extra HTTP headers (such as an `Authorization` header)
    ///   sent on every request to the endpoint; `None` sends no extra headers.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServerUnavailable`] when a header name or value is not
    /// a valid HTTP header, when the endpoint cannot be reached, when the MCP
    /// handshake fails, or when the initial `tools/list` fails.
    pub async fn connect(url: String, headers: Option<Map<String, Value>>) -> Result<Self> {
        let custom_headers = build_headers(headers)?;

        let config =
            StreamableHttpClientTransportConfig::with_uri(url).custom_headers(custom_headers);
        let transport = StreamableHttpClientTransport::from_config(config);

        let tools = Arc::new(RwLock::new(Vec::new()));
        let peer_slot = Arc::new(RwLock::new(None));
        let handler = ClientHandler {
            tools: Arc::clone(&tools),
            peer: Arc::clone(&peer_slot),
        };

        let service = serve_client(handler, transport).await.map_err(|error| {
            tracing::warn!(%error, "UrlServer: MCP handshake with HTTP endpoint failed");
            Error::ServerUnavailable
        })?;

        // The peer only exists once the handshake completes; hand it to the
        // handler so a later list_changed notification can refresh the cache.
        *peer_slot
            .write()
            .expect("UrlServer peer lock is never poisoned") = Some(service.peer().clone());

        let listed = service.peer().list_all_tools().await.map_err(|error| {
            tracing::warn!(%error, "UrlServer: initial tools/list failed");
            Error::ServerUnavailable
        })?;
        *tools
            .write()
            .expect("UrlServer tools lock is never poisoned") =
            listed.into_iter().map(ToolMetadata::new).collect();

        Ok(Self { service })
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
            .expect("UrlServer tools lock is never poisoned")
            .clone()
    }
}

/// Converts the platform's header map into `rmcp`'s typed HTTP header map.
///
/// The platform carries headers as a JSON object of string keys to string
/// values; `rmcp`'s transport wants a `HashMap<HeaderName, HeaderValue>`. This
/// validates each entry: a malformed header name, a non-string value, or a
/// value that is not a valid HTTP header value all fail the conversion.
///
/// # Parameters
///
/// - `headers` — the platform header map; `None` yields an empty map.
///
/// # Errors
///
/// Returns [`Error::ServerUnavailable`] when any entry is not a well-formed
/// HTTP header — a registration that cannot produce a valid request is treated
/// the same as a server that cannot serve one.
fn build_headers(headers: Option<Map<String, Value>>) -> Result<HashMap<HeaderName, HeaderValue>> {
    let Some(headers) = headers else {
        return Ok(HashMap::new());
    };

    let mut converted = HashMap::with_capacity(headers.len());
    for (name, value) in headers {
        let header_name = HeaderName::try_from(&name).map_err(|error| {
            tracing::warn!(%error, header = %name, "UrlServer: invalid HTTP header name");
            Error::ServerUnavailable
        })?;
        let header_value_str = value.as_str().ok_or_else(|| {
            tracing::warn!(header = %name, "UrlServer: HTTP header value is not a string");
            Error::ServerUnavailable
        })?;
        let header_value = HeaderValue::try_from(header_value_str).map_err(|error| {
            tracing::warn!(%error, header = %name, "UrlServer: invalid HTTP header value");
            Error::ServerUnavailable
        })?;
        converted.insert(header_name, header_value);
    }
    Ok(converted)
}

#[async_trait]
impl McpServer for UrlServer {
    /// Returns the endpoint's tool list as last cached.
    ///
    /// The list is filled at [`connect`](UrlServer::connect) time from the
    /// endpoint's `tools/list` and refreshed whenever the endpoint sends
    /// `notifications/tools/list_changed`.
    fn tools(&self) -> Vec<ToolMetadata> {
        self.tools_snapshot()
    }

    /// Forwards `tool` and `input` to the endpoint as an MCP `tools/call`.
    ///
    /// The tool name and the `input` arguments map are sent over HTTP
    /// unchanged — with the configured headers attached, as on every request —
    /// and the matching `tools/call` response is awaited and returned as a
    /// [`Value`], the same wire shape an MCP `tools/call` result carries.
    /// `caller` is **not** transmitted: see the module documentation on caller
    /// identity.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownTool`] when `tool` is absent from the endpoint's
    /// cached tool list, or [`Error::ServerUnavailable`] when the endpoint is
    /// no longer serving requests — including the case where it has become
    /// unreachable and the HTTP connection has closed.
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
/// endpoint does not know the named tool, which becomes [`Error::UnknownTool`].
/// Every other failure — a transport that has closed because the endpoint
/// became unreachable, a cancelled request, an unexpected response — means the
/// endpoint cannot serve the request, which becomes [`Error::ServerUnavailable`].
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
