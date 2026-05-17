//! In-process ACP agent served over a loopback WebSocket.
//!
//! The ACP agent runs *inside* the kanban-app process — there is no external
//! agent subprocess. (claude-agent spawning the `claude` CLI internally to do
//! its own work is fine; that is the backend's concern, not a transport hop.)
//!
//! # Topology
//!
//! ```text
//!   webview (TS ACP client)  ──ws://127.0.0.1:<port>──▶  AgentWebSocketServer
//!                                                              │
//!                                              swissarmyhammer_agent::create_agent
//!                                                              │
//!                                                   ┌──────────┴──────────┐
//!                                              claude-agent          llama-agent
//!                                                        (chosen at runtime by
//!                                                         ModelConfig::executor_type)
//! ```
//!
//! Tauri IPC is intentionally *not* on the ACP data path. The data path is a
//! plain WebSocket so the webview's ACP client speaks JSON-RPC directly to the
//! in-process agent.
//!
//! # Transport adaptation
//!
//! ACP's [`Lines`] transport consumes a newline-delimited JSON byte stream
//! modelled as a `Stream<Item = io::Result<String>>` plus a
//! `Sink<String, Error = io::Error>`. A WebSocket connection already frames
//! messages, so each WebSocket text frame carries exactly one JSON-RPC
//! message: incoming text frames map straight to line strings and outgoing
//! line strings map straight to text frames. No byte-level newline scanning
//! is needed.
//!
//! The agent component returned by `create_agent` is a `ConnectTo<Client>`
//! (the ACP 0.11 builder/handler runtime). Running it as the *server* side of
//! the WebSocket is `ConnectTo::<Agent>::connect_to(transport, agent)`: the
//! transport serves the `Agent` role and forwards to the agent component,
//! whose counterpart is `Client`.

use std::io;
use std::net::SocketAddr;

use agent_client_protocol::{Agent, ConnectTo, Lines};
use futures_util::{SinkExt, StreamExt};
use swissarmyhammer_agent::{create_agent, AcpAgentHandle};
use swissarmyhammer_config::model::ModelConfig;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

/// A loopback WebSocket server that serves an in-process ACP agent.
///
/// Each accepted connection gets its own freshly-built ACP agent. The server
/// binds an ephemeral port on `127.0.0.1`; callers read the chosen port back
/// via [`local_addr`](Self::local_addr) and hand `ws://127.0.0.1:<port>/` to
/// the webview's ACP client.
pub struct AgentWebSocketServer {
    listener: TcpListener,
    model_config: ModelConfig,
}

impl AgentWebSocketServer {
    /// Bind a loopback WebSocket server on an ephemeral port using the default
    /// model configuration (Claude Code).
    ///
    /// The OS assigns the port; [`local_addr`](Self::local_addr) reports it.
    /// `create_agent` dispatches on the configuration's executor type at
    /// runtime, so a future caller that needs a different backend simply
    /// supplies a different [`ModelConfig`] here — no compile-time gate.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if binding the loopback TCP socket fails.
    pub async fn bind() -> io::Result<Self> {
        Self::bind_with(ModelConfig::default()).await
    }

    /// Bind a loopback WebSocket server on an ephemeral port for a specific
    /// model configuration.
    ///
    /// This is the constructor the model-selection command surface uses: the
    /// webview picks a model, the backend resolves its [`ModelConfig`], and
    /// hands it here. Every connection accepted by [`run`](Self::run) builds
    /// its agent from this configuration, and `create_agent` dispatches on the
    /// configuration's executor type (Claude Code vs. local llama) at runtime.
    ///
    /// The OS assigns the port; [`local_addr`](Self::local_addr) reports it.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if binding the loopback TCP socket fails.
    pub async fn bind_with(model_config: ModelConfig) -> io::Result<Self> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        Ok(Self {
            listener,
            model_config,
        })
    }

    /// The loopback address the server is bound to, including the OS-assigned
    /// port.
    pub fn local_addr(&self) -> SocketAddr {
        self.listener
            .local_addr()
            .expect("a bound TcpListener always has a local address")
    }

    /// Run the accept loop until the listener errors.
    ///
    /// Each accepted connection is handled on its own spawned task so a slow
    /// or stuck agent never blocks new connections. This future only returns
    /// if accepting a connection fails irrecoverably.
    ///
    /// # Concurrency and security posture
    ///
    /// The accept loop spawns one task per inbound connection with no cap.
    /// This is intentional for a loopback-only server: the webview opens a
    /// single ACP connection and the OS-assigned port is never advertised, so
    /// the realistic fan-out is one. A bounded connection pool would add
    /// machinery for a contention case that does not arise here.
    ///
    /// The loopback socket has no per-connection auth: there is no origin
    /// check and no token handshake, so any local process that discovers the
    /// OS-assigned ephemeral port could connect and drive an in-process agent.
    /// The accepted risk is exactly that — a co-resident local process. It is
    /// mitigated only by loopback-only binding, which keeps the server off the
    /// network and matches a single-user desktop-app threat model.
    ///
    /// Hardening this with a per-launch auth token (mint a secret, embed it in
    /// the `ws://` URL handed to the webview, and reject connections that do
    /// not present it) is deferred, real work tracked separately as kanban
    /// task `01KRV7GFHKD1FFGNY8C6X8BZZ4`.
    pub async fn run(self) {
        let Self {
            listener,
            model_config,
        } = self;
        tracing::info!(addr = %listener.local_addr().map(|a| a.to_string()).unwrap_or_default(),
            "ACP agent WebSocket server listening");

        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    let model_config = model_config.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, model_config).await {
                            tracing::warn!(peer = %peer, error = %e,
                                "ACP agent WebSocket connection ended with error");
                        }
                    });
                }
                Err(e) => {
                    tracing::error!(error = %e, "ACP agent WebSocket accept failed; stopping");
                    return;
                }
            }
        }
    }
}

/// Handle one WebSocket connection: upgrade, build the agent in-process, and
/// run the ACP protocol over the socket until the client disconnects.
///
/// # Errors
///
/// Returns an error if the WebSocket handshake fails or the ACP protocol loop
/// terminates abnormally. A failure to *build* the agent is not an error of
/// this function — it is reported to the client as a JSON-RPC error and the
/// connection is then closed cleanly.
async fn handle_connection(stream: TcpStream, model_config: ModelConfig) -> io::Result<()> {
    let ws = tokio_tungstenite::accept_async(stream)
        .await
        .map_err(io::Error::other)?;

    let handle = match create_agent(&model_config, None).await {
        Ok(handle) => handle,
        Err(e) => {
            tracing::warn!(error = %e, "failed to build in-process ACP agent");
            return reject_connection(ws, &e.to_string()).await;
        }
    };

    serve_agent(ws, handle).await
}

/// Run an already-built ACP agent as the server side of `ws`.
///
/// Adapts the WebSocket to ACP's [`Lines`] transport and drives the agent
/// component until the connection closes.
async fn serve_agent(ws: WebSocketStream<TcpStream>, handle: AcpAgentHandle) -> io::Result<()> {
    let transport = lines_transport(ws);

    // The agent component is a `ConnectTo<Client>`. Serving it over the
    // transport means the transport plays the `Agent` role (its counterpart
    // is `Client`) and forwards client traffic to the agent component.
    ConnectTo::<Agent>::connect_to(transport, handle.agent)
        .await
        .map_err(|e| io::Error::other(e.to_string()))
}

/// Adapt a WebSocket stream into ACP's [`Lines`] transport.
///
/// Incoming text frames become JSON-RPC line strings; non-text frames (binary,
/// ping/pong) are dropped because ACP traffic is always UTF-8 JSON. Outgoing
/// line strings become text frames. I/O and protocol errors surface through
/// the stream/sink as [`io::Error`], which ACP's dispatch loop maps onto the
/// connection's shutdown path.
fn lines_transport(
    ws: WebSocketStream<TcpStream>,
) -> Lines<
    impl futures_util::Sink<String, Error = io::Error> + Send + 'static,
    impl futures_util::Stream<Item = io::Result<String>> + Send + 'static,
> {
    let (sink, stream) = ws.split();

    let incoming = stream.filter_map(|frame| async move {
        match frame {
            Ok(Message::Text(text)) => Some(Ok(text.as_str().to_owned())),
            // Binary/ping/pong/close frames carry no JSON-RPC payload. Drop
            // them so the ACP parser only ever sees protocol messages; the
            // close handshake is handled by the underlying WebSocket stream.
            Ok(_) => None,
            Err(e) => Some(Err(io::Error::other(e))),
        }
    });

    let outgoing = sink
        .sink_map_err(io::Error::other)
        .with(|line: String| async move { Ok(Message::text(line)) });

    Lines::new(outgoing, incoming)
}

/// Report an agent-construction failure to the client as a JSON-RPC error,
/// then close the WebSocket cleanly.
///
/// The agent could not be built, so there is no ACP runtime to own the
/// request/response cycle. This reads the client's first request frame,
/// echoes its `id`, and answers with a JSON-RPC error so the client sees a
/// defined failure instead of a silently dropped connection.
async fn reject_connection(mut ws: WebSocketStream<TcpStream>, reason: &str) -> io::Result<()> {
    // Echo the id of the client's first request so the error is correlated.
    // Absent or unreadable input yields a null id, which is still valid
    // JSON-RPC for an error reply.
    let id = match ws.next().await {
        Some(Ok(Message::Text(text))) => serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("id").cloned())
            .unwrap_or(serde_json::Value::Null),
        _ => serde_json::Value::Null,
    };

    let error = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            // -32603 is the JSON-RPC "internal error" code.
            "code": -32603,
            "message": format!("in-process ACP agent unavailable: {reason}"),
        }
    });

    let send = ws.send(Message::text(error.to_string())).await;
    let close = ws.close(None).await;
    send.and(close).map_err(io::Error::other)
}
