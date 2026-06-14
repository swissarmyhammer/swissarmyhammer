//! Shared test helpers for the `store` MCP server end-to-end tests.
//!
//! Mints an rmcp `Peer<RoleServer>` against a closed transport so tests
//! can build a real `RequestContext` and drive `StoreServer::call_tool`
//! without spinning up a full transport pair. Also provides a tiny
//! [`MockStore`] for tests that need real `TrackedStore` writes.

#![allow(dead_code)] // shared by multiple test modules

use std::borrow::Cow;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::model::{CallToolRequestParams, CallToolResult, NumberOrString};
use rmcp::service::{serve_directly, Peer, RequestContext, RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::Value;
use swissarmyhammer_store::server::StoreServer;
use swissarmyhammer_store::{StoreHandle, TrackedStore};

/// A transport that yields no messages and closes immediately, used
/// solely to mint a `Peer<RoleServer>` for the `RequestContext` an rmcp
/// call needs.
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

/// Mint an inert `Peer<RoleServer>` by briefly serving a placeholder
/// handler over a closed transport.
fn mint_peer() -> Peer<RoleServer> {
    struct PeerProbe;
    impl ServerHandler for PeerProbe {}

    let running = serve_directly(PeerProbe, ClosedTransport, None);
    running.peer().clone()
}

/// Build a default `RequestContext` for the `store` server. The
/// server's verb handlers do not read anything out of the context, but
/// the rmcp signature still requires one.
pub fn request_context() -> RequestContext<RoleServer> {
    RequestContext::new(NumberOrString::Number(0), mint_peer())
}

/// Invoke a `store` tool verb through the server's `ServerHandler`
/// surface and return the parsed `serde_json::Value` payload on
/// success.
///
/// The `op` parameter is load-bearing in debug builds: it must match
/// `arguments["op"]` so a typo in the call site is caught immediately.
pub async fn call_tool(
    server: &StoreServer,
    op: &str,
    arguments: Value,
) -> Result<Value, McpError> {
    debug_assert_eq!(
        arguments.get("op").and_then(Value::as_str),
        Some(op),
        "call_tool: op parameter must match arguments[\"op\"]",
    );
    let context = request_context();
    let mut request = CallToolRequestParams::new(Cow::Borrowed("store"));
    if let Value::Object(map) = arguments {
        request = request.with_arguments(map);
    }
    let result = server.call_tool(request, context).await?;
    Ok(extract_structured(&result))
}

/// Pull the `structured_content` payload out of a [`CallToolResult`].
///
/// Every `store` verb returns a structured response; the helper unwraps
/// the `Option` to keep call sites short.
pub fn extract_structured(result: &CallToolResult) -> Value {
    result
        .structured_content
        .clone()
        .expect("store tool should return structured content")
}

/// A minimal `TrackedStore` for integration tests.
///
/// Items are plain strings whose first line is the item id. The store's
/// human-readable name comes from the basename of its root directory,
/// matching the default convention used by the rest of the crate.
pub struct MockStore {
    root: PathBuf,
}

impl MockStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl swissarmyhammer_store::store::sealed::Sealed for MockStore {}

impl TrackedStore for MockStore {
    type Item = String;
    type ItemId = String;

    fn root(&self) -> &Path {
        &self.root
    }

    fn item_id(&self, item: &String) -> String {
        item.lines().next().unwrap_or("unknown").to_string()
    }

    fn serialize(&self, item: &String) -> swissarmyhammer_store::error::Result<String> {
        Ok(item.clone())
    }

    fn deserialize(
        &self,
        _id: &String,
        text: &str,
    ) -> swissarmyhammer_store::error::Result<String> {
        Ok(text.to_string())
    }

    fn extension(&self) -> &str {
        "txt"
    }
}

/// Build an `Arc<StoreHandle<MockStore>>` over a fresh directory.
pub fn make_mock_handle(root: &Path) -> Arc<StoreHandle<MockStore>> {
    let store = Arc::new(MockStore::new(root.to_path_buf()));
    Arc::new(StoreHandle::new(store))
}
