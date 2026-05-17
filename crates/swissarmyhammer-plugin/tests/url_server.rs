//! Integration tests for the [`UrlServer`] HTTP MCP transport.
//!
//! These tests drive a *real* HTTP MCP endpoint: each test stands up an
//! `rmcp` [`StreamableHttpService`] — the genuine server side of the MCP
//! Streamable HTTP transport — mounted on an in-process `axum` server bound
//! to a loopback port. A [`UrlServer`] then connects to that endpoint over
//! real HTTP, and the test asserts the observable result: a tool round-trip
//! in one case, a clean transport failure in the other.
//!
//! The fixture's `echo` tool records the HTTP request it was called with —
//! the configured authorization header in particular — into a shared slot,
//! so the round-trip test can prove the [`UrlServer`] attached the headers
//! the registration carried.
//!
//! [`UrlServer`]: swissarmyhammer_plugin::UrlServer
//! [`StreamableHttpService`]: rmcp::transport::StreamableHttpService

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::StreamableHttpService;
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use swissarmyhammer_plugin::{CallerId, Error, McpServer, UrlServer};
use tokio::net::TcpListener;

/// A generous upper bound on any single HTTP interaction.
///
/// Every await in these tests is wrapped in this timeout: a server that
/// hangs, or a transport that never reports a closed connection, must fail
/// the test fast rather than blocking CI indefinitely.
const TIMEOUT: Duration = Duration::from_secs(20);

/// The authorization header value the round-trip test registers and expects
/// the fixture to observe verbatim.
const TEST_AUTH_HEADER: &str = "Bearer url-server-test-token";

/// The single HTTP request the fixture's `echo` tool last observed.
///
/// The fixture pushes the request's `authorization` header value here every
/// time `echo` runs, so a test can prove the [`UrlServer`] forwarded the
/// configured headers on the `tools/call` request.
type RecordedAuth = Arc<Mutex<Option<String>>>;

/// Arguments for the fixture's `echo` tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct EchoArgs {
    /// The payload echoed straight back to the caller.
    message: String,
}

/// The fixture's `rmcp` server handler, served over HTTP.
///
/// A flat handler with one `echo` tool. It holds the macro-generated tool
/// router and a shared [`RecordedAuth`] slot; the `echo` tool records the
/// inbound HTTP request's authorization header into that slot so the test
/// can assert on the request shape the [`UrlServer`] produced.
#[derive(Clone)]
struct FixtureServer {
    /// The macro-generated tool router for this handler.
    tool_router: ToolRouter<Self>,
    /// The slot the `echo` tool records the observed auth header into.
    recorded_auth: RecordedAuth,
}

#[tool_router(router = tool_router)]
impl FixtureServer {
    /// Builds a [`FixtureServer`] that records into `recorded_auth`.
    fn new(recorded_auth: RecordedAuth) -> Self {
        Self {
            tool_router: Self::tool_router(),
            recorded_auth,
        }
    }

    /// Echoes the `message` argument back, recording the request's auth header.
    ///
    /// The HTTP request's [`Parts`](http::request::Parts) are injected into
    /// the request context by [`StreamableHttpService`]; this tool reads the
    /// `authorization` header out of them and stores it in the shared slot
    /// before returning the echoed payload.
    #[tool(name = "echo", description = "Echoes its message argument back.")]
    async fn echo(
        &self,
        Parameters(args): Parameters<EchoArgs>,
        Extension(parts): Extension<http::request::Parts>,
    ) -> String {
        let auth = parts
            .headers
            .get(http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        *self
            .recorded_auth
            .lock()
            .expect("the recorded-auth slot is never poisoned") = auth;
        args.message
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for FixtureServer {}

/// A running in-process HTTP MCP endpoint plus the request slot it records to.
///
/// Holds the bound URL the [`UrlServer`] connects to and the shared
/// [`RecordedAuth`] slot the fixture's `echo` tool writes the observed
/// authorization header into. Dropping it aborts the background server task.
struct Endpoint {
    /// The MCP endpoint URL, e.g. `http://127.0.0.1:PORT/mcp`.
    url: String,
    /// The slot the fixture records each `echo` call's auth header into.
    recorded_auth: RecordedAuth,
    /// The background `axum` server task; aborted on drop.
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        self.server.abort();
    }
}

/// Stands up a real HTTP MCP endpoint backed by an `rmcp` `StreamableHttpService`.
///
/// Mounts the [`StreamableHttpService`] — the genuine server half of the MCP
/// Streamable HTTP transport — at `/mcp` on an `axum` router, binds it to a
/// loopback port chosen by the OS, and serves it on a background task. The
/// returned [`Endpoint`] carries the URL to connect to and the slot the
/// fixture records requests into.
async fn start_endpoint() -> Endpoint {
    let recorded_auth: RecordedAuth = Arc::new(Mutex::new(None));
    let factory_auth = Arc::clone(&recorded_auth);

    let service = StreamableHttpService::new(
        move || Ok(FixtureServer::new(Arc::clone(&factory_auth))),
        Arc::new(LocalSessionManager::default()),
        Default::default(),
    );

    let router = axum::Router::new().route_service("/mcp", service);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("binding a loopback port for the fixture endpoint should succeed");
    let addr = listener
        .local_addr()
        .expect("a bound listener should report its address");
    let url = format!("http://{addr}/mcp");

    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    Endpoint {
        url,
        recorded_auth,
        server,
    }
}

/// Connects a [`UrlServer`] to `url` with the test auth header, failing fast.
async fn connect_url_server(url: &str) -> UrlServer {
    let mut headers = Map::new();
    headers.insert(
        http::header::AUTHORIZATION.to_string(),
        Value::String(TEST_AUTH_HEADER.to_string()),
    );

    tokio::time::timeout(TIMEOUT, UrlServer::connect(url.to_string(), Some(headers)))
        .await
        .expect("connecting to the fixture HTTP endpoint should not hang")
        .expect("connecting to a real HTTP MCP endpoint should succeed")
}

#[tokio::test]
async fn invoke_round_trips_a_tools_call_over_http() {
    let endpoint = start_endpoint().await;
    let server = connect_url_server(&endpoint.url).await;

    let names: Vec<String> = server
        .tools()
        .into_iter()
        .map(|tool| tool.name().to_string())
        .collect();
    assert!(
        names.contains(&"echo".to_string()),
        "the endpoint's tools/list should surface the echo tool, got {names:?}"
    );

    let result = tokio::time::timeout(
        TIMEOUT,
        server.invoke(
            CallerId::HostInternal,
            "echo",
            json!({ "message": "hello over http" }),
        ),
    )
    .await
    .expect("a tools/call against the HTTP endpoint should not hang")
    .expect("invoking the echo tool over HTTP should succeed");

    let rendered = serde_json::to_string(&result).expect("a tools/call result is serializable");
    assert!(
        rendered.contains("hello over http"),
        "the echoed payload should round-trip back over HTTP, got {rendered}"
    );

    let recorded = endpoint
        .recorded_auth
        .lock()
        .expect("the recorded-auth slot is never poisoned")
        .clone();
    assert_eq!(
        recorded.as_deref(),
        Some(TEST_AUTH_HEADER),
        "the tools/call request should have carried the configured authorization header"
    );
}

#[tokio::test]
async fn invoke_forwards_the_arguments_map_unchanged() {
    let endpoint = start_endpoint().await;
    let server = connect_url_server(&endpoint.url).await;

    let result = tokio::time::timeout(
        TIMEOUT,
        server.invoke(
            CallerId::HostInternal,
            "echo",
            json!({ "message": "argument fidelity" }),
        ),
    )
    .await
    .expect("a tools/call against the HTTP endpoint should not hang")
    .expect("invoking the echo tool over HTTP should succeed");

    let rendered = serde_json::to_string(&result).expect("a tools/call result is serializable");
    assert!(
        rendered.contains("argument fidelity"),
        "the arguments map should reach the tool unchanged, got {rendered}"
    );
}

#[tokio::test]
async fn unknown_tool_yields_unknown_tool_error() {
    let endpoint = start_endpoint().await;
    let server = connect_url_server(&endpoint.url).await;

    let err = tokio::time::timeout(
        TIMEOUT,
        server.invoke(CallerId::HostInternal, "no-such-tool", json!({})),
    )
    .await
    .expect("invoking a missing tool should not hang")
    .expect_err("invoking a tool the endpoint does not expose should fail");

    assert!(
        matches!(err, Error::UnknownTool),
        "a tool absent from the endpoint's tools/list should map to UnknownTool, got {err:?}"
    );
}

#[tokio::test]
async fn connecting_to_an_unreachable_url_yields_server_unavailable() {
    // Port 1 on loopback has no listener: the TCP connection is refused, so
    // the rmcp HTTP transport's initialize handshake fails at connect time.
    // `UrlServer` is not `Debug`, so collapse the Ok side to a marker before
    // asserting on the error variant.
    let outcome = tokio::time::timeout(
        TIMEOUT,
        UrlServer::connect("http://127.0.0.1:1/mcp".to_string(), None),
    )
    .await
    .expect("connecting to an unreachable URL must not hang")
    .map(|_| "connected");

    let err = outcome.expect_err("connecting to an unreachable URL should fail");
    assert!(
        matches!(err, Error::ServerUnavailable),
        "an unreachable endpoint should map to ServerUnavailable, got {err:?}"
    );
}
