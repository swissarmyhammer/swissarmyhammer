//! The in-process transport: an [`McpServer`] backed by an in-memory
//! [`rmcp::ServerHandler`].
//!
//! [`InProcessServer`] wraps any `rmcp` server handler — typically one built
//! with the `#[tool_router]` / `#[tool]` / `#[tool_handler]` macros — and
//! exposes it as a platform [`McpServer`]. Calls flow straight to the wrapped
//! handler with no serialization and no inter-process traffic, so this is the
//! transport host Rust code uses to register its own tools with the platform.

use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use rmcp::model::{CallToolRequestParams, NumberOrString, PaginatedRequestParams};
use rmcp::service::{serve_directly, Peer, RequestContext, RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::RoleServer;
use rmcp::ServerHandler;
use serde_json::Value;

use crate::error::Error;
use crate::server::{CallerId, McpServer, ToolMetadata};

/// An [`McpServer`] backed directly by an in-memory [`rmcp::ServerHandler`].
///
/// `InProcessServer` is the platform's zero-IPC transport. It owns a shared
/// handle to an `rmcp` server handler and dispatches every [`McpServer`] call
/// straight into that handler's `rmcp` methods — there is no child process, no
/// socket, and no JSON-RPC framing on the call path. The wrapped handler is
/// the platform's view of a tool provider implemented in host Rust.
///
/// `rmcp`'s request-handling methods (`call_tool`, `list_tools`) require a
/// [`RequestContext<RoleServer>`], which in turn needs a `Peer<RoleServer>`.
/// `rmcp` only mints a `Peer` from inside its service machinery, so the
/// constructors briefly run the handler through [`serve_directly`] over an
/// immediately-closing in-memory transport solely to obtain a `Peer` value.
/// That peer is an inert routing token here: a flat `#[tool]` handler that
/// does not call back to a client never touches it.
///
/// The tool list is enumerated once at construction and cached, because
/// [`McpServer::tools`] is synchronous and cannot drive `rmcp`'s asynchronous
/// `list_tools`. The constructors are therefore `async`.
pub struct InProcessServer<S> {
    /// The wrapped `rmcp` server handler, shared so it can outlive a single call.
    inner: Arc<S>,
    /// A `Peer<RoleServer>` used only to build the `RequestContext` each call needs.
    peer: Peer<RoleServer>,
    /// The handler's tool list, enumerated once at construction.
    tools: Vec<ToolMetadata>,
}

/// A transport that yields no messages and closes immediately.
///
/// [`serve_directly`] is the only public path to a `Peer<RoleServer>`, and it
/// requires a transport. This transport's [`receive`](Transport::receive)
/// returns `None` on the first poll, so `rmcp`'s service loop terminates at
/// once: the background task exits cleanly and the `Peer` it produced remains
/// a valid, standalone value.
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

impl<S> InProcessServer<S>
where
    S: ServerHandler + Send + Sync + 'static,
{
    /// Wraps `inner`, taking ownership of the handler.
    ///
    /// This is `async` because it enumerates the handler's tools once, up
    /// front, so the synchronous [`McpServer::tools`] can serve the cached
    /// list. See [`from_arc`](Self::from_arc) for the shared-handle form.
    ///
    /// # Parameters
    ///
    /// - `inner` — the `rmcp` server handler to expose through the platform.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`](crate::error::Error) when the handler's
    /// `list_tools` reports a failure.
    pub async fn new(inner: S) -> crate::error::Result<Self> {
        Self::from_arc(Arc::new(inner)).await
    }

    /// Wraps an already-shared handler.
    ///
    /// Equivalent to [`new`](Self::new) for a handler the caller already holds
    /// behind an [`Arc`].
    ///
    /// # Parameters
    ///
    /// - `inner` — the shared `rmcp` server handler to expose.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`](crate::error::Error) when the handler's
    /// `list_tools` reports a failure.
    pub async fn from_arc(inner: Arc<S>) -> crate::error::Result<Self> {
        let peer = mint_peer();
        let tools = enumerate_tools(inner.as_ref(), &peer).await?;
        Ok(Self { inner, peer, tools })
    }

    /// Builds the [`RequestContext`] an `rmcp` call needs, carrying `caller`.
    ///
    /// The returned context's `extensions` hold the [`CallerId`], so an `rmcp`
    /// `#[tool]` handler can recover it with an `Extension<CallerId>` extractor
    /// or by reading `ctx.extensions` directly.
    fn request_context(&self, caller: CallerId) -> RequestContext<RoleServer> {
        let mut context = RequestContext::new(NumberOrString::Number(0), self.peer.clone());
        context.extensions.insert(caller);
        context
    }
}

/// Mints a `Peer<RoleServer>` by briefly serving a placeholder handler.
///
/// `rmcp` exposes no standalone `Peer` constructor; [`serve_directly`] is the
/// only public route, and it consumes a handler plus a transport. The handler
/// served here is a throwaway — the real wrapped handler is never run through
/// the service loop — and [`ClosedTransport`] makes that loop exit at once.
fn mint_peer() -> Peer<RoleServer> {
    /// A trivial handler used only so [`serve_directly`] has something to run.
    struct PeerProbe;
    impl ServerHandler for PeerProbe {}

    let running = serve_directly(PeerProbe, ClosedTransport, None);
    running.peer().clone()
}

/// Enumerates `handler`'s tools into platform [`ToolMetadata`].
///
/// Drives `rmcp`'s asynchronous `list_tools`, then wraps each returned
/// `rmcp::model::Tool` — preserving `name`, `description`, `inputSchema`, and
/// `_meta` — as [`ToolMetadata`].
async fn enumerate_tools<S>(
    handler: &S,
    peer: &Peer<RoleServer>,
) -> crate::error::Result<Vec<ToolMetadata>>
where
    S: ServerHandler + Send + Sync + 'static,
{
    let context = RequestContext::new(NumberOrString::Number(0), peer.clone());
    let listed = handler
        .list_tools(None::<PaginatedRequestParams>, context)
        .await
        .map_err(map_rmcp_error)?;
    Ok(listed.tools.into_iter().map(ToolMetadata::new).collect())
}

/// Maps an `rmcp` error into the platform [`Error`].
///
/// `rmcp`'s `ErrorData` distinguishes a missing tool — code
/// [`METHOD_NOT_FOUND`](rmcp::model::ErrorCode::METHOD_NOT_FOUND) — from every
/// other failure. A missing tool becomes [`Error::UnknownTool`]; anything else
/// becomes [`Error::ServerUnavailable`], since the handler is registered but
/// could not serve the request.
fn map_rmcp_error(error: rmcp::ErrorData) -> Error {
    if error.code == rmcp::model::ErrorCode::METHOD_NOT_FOUND {
        Error::UnknownTool
    } else {
        Error::ServerUnavailable
    }
}

#[async_trait]
impl<S> McpServer for InProcessServer<S>
where
    S: ServerHandler + Send + Sync + 'static,
{
    /// Returns the tool list enumerated from the wrapped handler at construction.
    fn tools(&self) -> Vec<ToolMetadata> {
        self.tools.clone()
    }

    /// Invokes `tool` on the wrapped `rmcp` handler with no serialization.
    ///
    /// Builds an `rmcp` `CallToolRequestParams` from `tool` and `input`,
    /// threads `caller` through the request context's `extensions`, and calls
    /// the handler's `call_tool` directly. The returned `CallToolResult` is
    /// converted to a `serde_json::Value` — the same shape an MCP `tools/call`
    /// response carries on the wire — without ever crossing a transport.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownTool`] when `tool` is not in the wrapped
    /// handler's tool list, or [`Error::ServerUnavailable`] when the handler's
    /// `call_tool` itself reports a failure.
    async fn invoke(
        &self,
        caller: CallerId,
        tool: &str,
        input: Value,
    ) -> crate::error::Result<Value> {
        if !self.tools.iter().any(|t| t.name() == tool) {
            return Err(Error::UnknownTool);
        }

        let mut request = CallToolRequestParams::new(tool.to_string());
        request.arguments = input.as_object().cloned();

        let context = self.request_context(caller);
        let result = self
            .inner
            .call_tool(request, context)
            .await
            .map_err(map_rmcp_error)?;

        serde_json::to_value(result).map_err(|_| Error::ServerUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rmcp::handler::server::common::Extension;
    use rmcp::handler::server::router::tool::ToolRouter;
    use rmcp::handler::server::wrapper::Parameters;
    use rmcp::schemars::{self, JsonSchema};
    use rmcp::{tool, tool_handler, tool_router};
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    use crate::dispatcher::Dispatcher;
    use crate::registry::ServerRegistry;
    use crate::server::PluginId;

    /// Arguments for the test echo tool.
    #[derive(Debug, Serialize, Deserialize, JsonSchema)]
    struct EchoArgs {
        /// The payload echoed straight back to the caller.
        message: String,
    }

    /// A real `rmcp` server handler built with the macro stack.
    ///
    /// It exposes a single flat `echo` tool that returns its `message`
    /// argument verbatim, plus a `whoami` tool that reports the [`CallerId`]
    /// the in-process transport inserted into the request context. Both tools
    /// are genuine `#[tool]` handlers — nothing here is hand-rolled.
    #[derive(Clone)]
    struct EchoServer {
        /// The macro-generated tool router for this handler.
        tool_router: ToolRouter<Self>,
    }

    #[tool_router(router = tool_router)]
    impl EchoServer {
        /// Builds an [`EchoServer`] with its tool router wired up.
        fn new() -> Self {
            Self {
                tool_router: Self::tool_router(),
            }
        }

        /// Echoes the `message` argument straight back to the caller.
        #[tool(name = "echo", description = "Echoes its message argument back.")]
        async fn echo(&self, Parameters(args): Parameters<EchoArgs>) -> String {
            args.message
        }

        /// Reports the [`CallerId`] carried in the request context.
        #[tool(name = "whoami", description = "Reports the caller identity.")]
        async fn whoami(&self, Extension(caller): Extension<CallerId>) -> String {
            format!("{caller:?}")
        }
    }

    #[tool_handler(router = self.tool_router)]
    impl ServerHandler for EchoServer {}

    /// Renders a `tools/call` result to a string for substring assertions.
    fn rendered(value: &Value) -> String {
        serde_json::to_string(value).expect("a tools/call result is serializable")
    }

    #[tokio::test]
    async fn invoke_round_trips_a_tools_call_to_the_rmcp_handler() {
        let server = InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed");

        let result = server
            .invoke(
                CallerId::HostInternal,
                "echo",
                json!({ "message": "hello in-process" }),
            )
            .await
            .expect("invoke of a real rmcp tool should succeed");

        assert!(
            rendered(&result).contains("hello in-process"),
            "the echoed payload should reach back through the adapter, got {}",
            rendered(&result)
        );
    }

    #[tokio::test]
    async fn tools_lists_the_wrapped_handlers_tools() {
        let server = InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed");

        let names: Vec<String> = server
            .tools()
            .into_iter()
            .map(|t| t.name().to_string())
            .collect();

        assert!(names.contains(&"echo".to_string()), "echo tool listed");
        assert!(names.contains(&"whoami".to_string()), "whoami tool listed");
    }

    #[tokio::test]
    async fn invoke_threads_the_caller_into_request_context_extensions() {
        let server = InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed");
        let caller = CallerId::Plugin(PluginId::new("plugin-x"));

        let result = server
            .invoke(caller.clone(), "whoami", json!({}))
            .await
            .expect("invoke of the whoami tool should succeed");

        assert!(
            rendered(&result).contains("plugin-x"),
            "the handler should observe the exact caller the adapter inserted, got {}",
            rendered(&result)
        );
    }

    #[tokio::test]
    async fn unknown_tool_yields_unknown_tool_error() {
        let server = InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed");

        let err = server
            .invoke(CallerId::HostInternal, "no-such-tool", json!({}))
            .await
            .expect_err("invoking a missing tool should fail");

        assert!(
            matches!(err, Error::UnknownTool),
            "a missing tool should map to UnknownTool, got {err:?}"
        );
    }

    #[tokio::test]
    async fn dispatches_through_the_registry_and_dispatcher() {
        let server: Arc<dyn McpServer> = Arc::new(
            InProcessServer::new(EchoServer::new())
                .await
                .expect("wrapping a real rmcp handler should succeed"),
        );
        let mut registry = ServerRegistry::new();
        registry
            .register("echo-srv".to_string(), server)
            .expect("registering a fresh name should succeed");
        let dispatcher = Dispatcher::new(Arc::new(registry));

        let result = dispatcher
            .call(
                CallerId::HostInternal,
                "echo-srv",
                "echo",
                json!({ "message": "via dispatcher" }),
            )
            .await
            .expect("dispatch to a registered in-process server should succeed");

        assert!(
            rendered(&result).contains("via dispatcher"),
            "the call should round-trip the registry and dispatcher, got {}",
            rendered(&result)
        );
    }
}
