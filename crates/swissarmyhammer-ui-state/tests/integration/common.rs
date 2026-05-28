//! Shared test helpers for the `ui_state` MCP server end-to-end tests.
//!
//! Provides a [`Harness`] that owns a `TempDir` and a `UiStateServer` over a
//! `UIState` loaded from a temp file (so persisted state is observable and no
//! real home dir is touched), plus an rmcp `Peer<RoleServer>` minted against a
//! closed transport so tests can build a real `RequestContext` and drive
//! `UiStateServer::call_tool` without a live GUI or transport pair.

#![allow(dead_code)] // shared by multiple test modules

use std::borrow::Cow;
use std::future::Future;
use std::sync::Arc;

use rmcp::model::{CallToolRequestParams, CallToolResult, NumberOrString};
use rmcp::service::{serve_directly, Peer, RequestContext, RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::Value;
use swissarmyhammer_ui_state::{UIState, UiStateServer};
use tempfile::TempDir;

/// A fully wired `ui_state` service over a temp-file-backed `UIState`.
///
/// Holds the `TempDir` so the backing file outlives the test, and the shared
/// `Arc<UIState>` so tests can read persisted state back after driving the
/// service.
pub struct Harness {
    /// Temp dir backing the `UIState` config file; kept alive for the test.
    pub _dir: TempDir,
    /// The shared UI state the service mutates.
    pub ui_state: Arc<UIState>,
}

impl Harness {
    /// Build a harness over a fresh temp-file-backed `UIState`.
    pub fn new() -> Self {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("ui-state.yaml");
        let ui_state = Arc::new(UIState::load(path));
        Self {
            _dir: dir,
            ui_state,
        }
    }

    /// Build a `UiStateServer` over the harness's shared `UIState`.
    pub fn service(&self) -> UiStateServer {
        UiStateServer::new(Arc::clone(&self.ui_state))
    }
}

/// A transport that yields no messages and closes immediately, used solely to
/// mint a `Peer<RoleServer>` for the `RequestContext` an rmcp call needs.
struct ClosedTransport;

impl Transport<RoleServer> for ClosedTransport {
    type Error = std::io::Error;

    fn send(
        &mut self,
        _item: TxJsonRpcMessage<RoleServer>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        std::future::ready(Ok(()))
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<RoleServer>>> + Send {
        std::future::ready(None)
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        std::future::ready(Ok(()))
    }
}

/// Mint an inert `Peer<RoleServer>` by briefly serving a placeholder handler
/// over a closed transport.
fn mint_peer() -> Peer<RoleServer> {
    struct PeerProbe;
    impl ServerHandler for PeerProbe {}

    let running = serve_directly(PeerProbe, ClosedTransport, None);
    running.peer().clone()
}

/// Build a default `RequestContext` for the `ui_state` service. The service's
/// verb handlers do not read anything out of the context, but the rmcp
/// signature still requires one.
pub fn request_context() -> RequestContext<RoleServer> {
    RequestContext::new(NumberOrString::Number(0), mint_peer())
}

/// Invoke a `ui_state` tool verb through the service's `ServerHandler` surface
/// and return the parsed `serde_json::Value` payload on success.
///
/// The `op` parameter is load-bearing in debug builds: it must match
/// `arguments["op"]` so a typo in the call site is caught immediately.
pub async fn call_tool(
    service: &UiStateServer,
    op: &str,
    arguments: Value,
) -> Result<Value, McpError> {
    debug_assert_eq!(
        arguments.get("op").and_then(Value::as_str),
        Some(op),
        "call_tool: op parameter must match arguments[\"op\"]",
    );
    let context = request_context();
    let mut request = CallToolRequestParams::new(Cow::Borrowed("ui_state"));
    if let Value::Object(map) = arguments {
        request = request.with_arguments(map);
    }
    let result = service.call_tool(request, context).await?;
    Ok(extract_structured(&result))
}

/// Pull the `structured_content` payload out of a [`CallToolResult`].
pub fn extract_structured(result: &CallToolResult) -> Value {
    result
        .structured_content
        .clone()
        .expect("ui_state tool should return structured content")
}
