//! Shared test helpers for the `app` MCP server end-to-end tests.
//!
//! Provides a recording [`SpyShell`] implementing the `AppShell` seam, and an
//! rmcp `Peer<RoleServer>` minted against a closed transport so tests can
//! build a real `RequestContext` and drive `AppService::call_tool` without a
//! live GUI or a full transport pair.

#![allow(dead_code)] // shared by multiple test modules

use std::borrow::Cow;
use std::future::Future;
use std::sync::{Arc, Mutex};

use rmcp::model::{CallToolRequestParams, CallToolResult, NumberOrString};
use rmcp::service::{serve_directly, Peer, RequestContext, RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::Value;
use swissarmyhammer_app_service::{AboutInfo, AppService, AppShell};

/// A recording [`AppShell`] used to assert which shell method the service
/// drove for each op.
///
/// Each call appends a tag to `calls`; `quit` / `about` / `help` push
/// `"quit"`, `"about"`, `"help"` respectively. `show_about` returns the canned
/// [`AboutInfo`] the harness was built with, and `show_help` returns the
/// canned help target.
pub struct SpyShell {
    /// Ordered log of shell method tags, one per call.
    pub calls: Mutex<Vec<&'static str>>,
    /// The about info `show_about` hands back.
    pub about: AboutInfo,
    /// The help target `show_help` hands back.
    pub help_target: String,
}

impl SpyShell {
    /// Build a spy with canned about info and help target.
    pub fn new(about: AboutInfo, help_target: impl Into<String>) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            about,
            help_target: help_target.into(),
        }
    }

    /// Snapshot the recorded call tags in order.
    pub fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }
}

impl AppShell for SpyShell {
    fn quit(&self) {
        self.calls.lock().unwrap().push("quit");
    }

    fn show_about(&self) -> AboutInfo {
        self.calls.lock().unwrap().push("about");
        self.about.clone()
    }

    fn show_help(&self) -> String {
        self.calls.lock().unwrap().push("help");
        self.help_target.clone()
    }
}

/// A fully wired `app` service over a recording spy, kept alive for a test.
///
/// Holds the `Arc<SpyShell>` so tests can read back the recorded calls after
/// driving the service.
pub struct Harness {
    /// The shared spy the service routes through.
    pub shell: Arc<SpyShell>,
}

impl Harness {
    /// Build a harness with default canned about info / help target.
    pub fn new() -> Self {
        Self::with_shell(SpyShell::new(
            AboutInfo {
                name: "kanban-app".to_string(),
                version: "9.9.9".to_string(),
            },
            "https://help.example/docs",
        ))
    }

    /// Build a harness around a caller-supplied spy.
    pub fn with_shell(shell: SpyShell) -> Self {
        Self {
            shell: Arc::new(shell),
        }
    }

    /// Build an `AppService` over the harness's spy shell.
    pub fn service(&self) -> AppService {
        AppService::new(Arc::clone(&self.shell) as Arc<dyn AppShell>)
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

/// Build a default `RequestContext` for the `app` service. The service's verb
/// handlers do not read anything out of the context, but the rmcp signature
/// still requires one.
pub fn request_context() -> RequestContext<RoleServer> {
    RequestContext::new(NumberOrString::Number(0), mint_peer())
}

/// Invoke an `app` tool verb through the service's `ServerHandler` surface and
/// return the parsed `serde_json::Value` payload on success.
///
/// The `op` parameter is load-bearing in debug builds: it must match
/// `arguments["op"]` so a typo in the call site is caught immediately.
pub async fn call_tool(service: &AppService, op: &str, arguments: Value) -> Result<Value, McpError> {
    debug_assert_eq!(
        arguments.get("op").and_then(Value::as_str),
        Some(op),
        "call_tool: op parameter must match arguments[\"op\"]",
    );
    let context = request_context();
    let mut request = CallToolRequestParams::new(Cow::Borrowed("app"));
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
        .expect("app tool should return structured content")
}
