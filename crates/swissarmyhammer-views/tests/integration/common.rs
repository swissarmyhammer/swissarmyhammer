//! Shared test helpers for the `views` MCP server end-to-end tests.
//!
//! Mints an rmcp `Peer<RoleServer>` against a closed transport so tests can
//! build a real `RequestContext` and drive `ViewsServer::call_tool` without
//! spinning up a full transport pair. Also wires a complete substrate — a
//! `PerspectiveContext` + a `ViewsContext` + a shared `StoreContext` with a
//! `PerspectiveStore` and `ViewStore` handle each — exactly as production
//! (`swissarmyhammer-kanban`'s `wire_store_substrate`) does, so undo and
//! events behave the same way in tests.

#![allow(dead_code)] // shared by multiple test modules

use std::borrow::Cow;
use std::future::Future;
use std::sync::Arc;

use rmcp::model::{CallToolRequestParams, CallToolResult, NumberOrString};
use rmcp::service::{serve_directly, Peer, RequestContext, RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::Value;
use swissarmyhammer_perspectives::{PerspectiveContext, PerspectiveStore};
use swissarmyhammer_store::{StoreContext, StoreHandle};
use swissarmyhammer_views::{ViewStore, ViewsContext, ViewsServer};
use tempfile::TempDir;
use tokio::sync::RwLock;

/// A transport that yields no messages and closes immediately, used solely
/// to mint a `Peer<RoleServer>` for the `RequestContext` an rmcp call needs.
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

/// Build a default `RequestContext` for the `views` server. The server's verb
/// handlers do not read anything out of the context, but the rmcp signature
/// still requires one.
pub fn request_context() -> RequestContext<RoleServer> {
    RequestContext::new(NumberOrString::Number(0), mint_peer())
}

/// A fully wired perspective + views substrate, kept alive for a test.
///
/// Holds the `TempDir` so the storage root is not reclaimed mid-test, the
/// shared `Arc<StoreContext>` (so tests can drive `undo` directly), and the
/// `Arc<RwLock<…>>` handles the `ViewsServer` dispatches against.
pub struct Harness {
    pub dir: TempDir,
    pub store_ctx: Arc<StoreContext>,
    pub perspectives: Arc<RwLock<PerspectiveContext>>,
    pub views: Arc<RwLock<ViewsContext>>,
}

impl Harness {
    /// Build the substrate, mirroring `wire_store_substrate`'s perspective and
    /// view wiring: one `StoreContext`, a `PerspectiveStore`/`ViewStore`
    /// registered with the shared context, and `set_store_handle` +
    /// `set_store_context` on each context so writes push onto the undo stack.
    pub async fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let store_ctx = Arc::new(StoreContext::new(dir.path().to_path_buf()));

        // Perspective context + store.
        let perspectives_dir = dir.path().join("perspectives");
        let perspective_ctx = PerspectiveContext::open(&perspectives_dir).await.unwrap();
        let perspective_store = PerspectiveStore::new(&perspectives_dir);
        let p_handle = Arc::new(StoreHandle::new(Arc::new(perspective_store)));
        store_ctx.register(p_handle.clone()).await;

        let perspectives = {
            let mut pctx = perspective_ctx;
            pctx.set_store_handle(p_handle);
            pctx.set_store_context(Arc::clone(&store_ctx));
            Arc::new(RwLock::new(pctx))
        };

        // Views context + store.
        let views_dir = dir.path().join("views");
        let views_ctx = ViewsContext::open(&views_dir).build().await.unwrap();
        let view_store = ViewStore::new(&views_dir);
        let v_handle = Arc::new(StoreHandle::new(Arc::new(view_store)));
        store_ctx.register(v_handle.clone()).await;

        let views = {
            let mut vctx = views_ctx;
            vctx.set_store_handle(v_handle);
            vctx.set_store_context(Arc::clone(&store_ctx));
            Arc::new(RwLock::new(vctx))
        };

        Self {
            dir,
            store_ctx,
            perspectives,
            views,
        }
    }

    /// Build a `ViewsServer` over the harness's shared contexts.
    pub fn server(&self) -> ViewsServer {
        ViewsServer::new(Arc::clone(&self.perspectives), Arc::clone(&self.views))
    }
}

/// Invoke a `views` tool verb through the server's `ServerHandler` surface and
/// return the parsed `serde_json::Value` payload on success.
///
/// The `op` parameter is load-bearing in debug builds: it must match
/// `arguments["op"]` so a typo in the call site is caught immediately.
pub async fn call_tool(
    server: &ViewsServer,
    op: &str,
    arguments: Value,
) -> Result<Value, McpError> {
    debug_assert_eq!(
        arguments.get("op").and_then(Value::as_str),
        Some(op),
        "call_tool: op parameter must match arguments[\"op\"]",
    );
    let context = request_context();
    let mut request = CallToolRequestParams::new(Cow::Borrowed("views"));
    if let Value::Object(map) = arguments {
        request = request.with_arguments(map);
    }
    let result = server.call_tool(request, context).await?;
    Ok(extract_structured(&result))
}

/// Pull the `structured_content` payload out of a [`CallToolResult`].
pub fn extract_structured(result: &CallToolResult) -> Value {
    result
        .structured_content
        .clone()
        .expect("views tool should return structured content")
}
