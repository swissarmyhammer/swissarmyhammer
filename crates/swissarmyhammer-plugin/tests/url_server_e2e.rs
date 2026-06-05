//! End-to-end integration test for the **URL (HTTP MCP) transport**, driven
//! through a real plugin.
//!
//! This is the capability-level companion to `url_server.rs`. Where
//! `url_server.rs` exercises the [`UrlServer`] type directly, this test proves
//! the same transport works when a *real plugin* registers it: a discovered
//! probe plugin's `load()` does `this.register("url", { url, headers })`, the
//! host connects a [`UrlServer`] to that endpoint, and the plugin then issues
//! a `tools/call` that crosses the entire pipeline — SDK dispatch Proxy, host
//! dispatcher, `UrlServer` HTTP transport, the real `rmcp` HTTP server, and
//! back.
//!
//! It follows the reference shape of `files_dispatch_e2e.rs`: a real V8
//! isolate, real registered servers, and an effect observed on disk that can
//! only happen if every stage works.
//!
//! # The HTTP endpoint
//!
//! The endpoint is a genuine `rmcp` [`StreamableHttpService`] — the real
//! server half of the MCP Streamable HTTP transport — mounted on an in-process
//! `axum` server bound to a loopback port. Its `echo` tool records the request
//! it observed (tool name, the full arguments map, and the inbound
//! authorization header) into a shared slot, so the test can prove the
//! `tools/call` the plugin issued reached the endpoint with the expected
//! shape.
//!
//! # The two registered servers
//!
//! The probe plugin registers two servers and uses both:
//!
//! - `url` — the transport under test. Its source is `{ url, headers }`
//!   pointing at the loopback HTTP endpoint, with an `Authorization` header.
//! - `fs` — the real in-process `files` tool, reached exactly as
//!   `files_dispatch_e2e.rs` reaches it. It is the *observation channel*: the
//!   plugin writes the echoed payload to disk through it.
//!
//! # What a passing run proves
//!
//! The probe plugin's `load()` calls `echo` on the `url` server with a known
//! message, extracts the echoed text from the `tools/call` result, and writes
//! that text into a probe file via the real `files` tool. The test asserts:
//!
//! 1. the probe file holds exactly the echoed payload — the response reached
//!    the plugin and crossed back through the dispatcher into the isolate;
//! 2. the endpoint recorded the `echo` tool name, the arguments map verbatim,
//!    and the `Authorization` header the registration carried.
//!
//! If the URL transport is broken at any stage, the plugin's `echo` call
//! throws, `load()` fails, and discovery returns an error before any file is
//! written — the test fails.
//!
//! [`UrlServer`]: swissarmyhammer_plugin::UrlServer
//! [`StreamableHttpService`]: rmcp::transport::StreamableHttpService

use std::path::Path;
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
use serde_json::{json, Value};
use swissarmyhammer_directory::SwissarmyhammerConfig;
use swissarmyhammer_plugin::PluginHost;
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_tools::mcp::McpServer;
use tokio::net::TcpListener;

/// A generous upper bound on any single host or HTTP interaction.
///
/// Building the MCP server stands up the full in-process tool registry, so the
/// bound is wider than a bare isolate test would need. Every cross-thread
/// await is wrapped in it so a server that hangs fails the test fast instead
/// of blocking CI.
const TIMEOUT: Duration = Duration::from_secs(60);

/// The probe file the plugin writes the echoed payload into — proof a
/// `tools/call` round-tripped over HTTP and the response reached the plugin.
const PROBE_FILE: &str = "url_echo_probe.txt";

/// The exact message the plugin sends to the endpoint's `echo` tool and
/// expects echoed verbatim back over HTTP.
const ECHO_PAYLOAD: &str = "e2e payload routed through the URL HTTP transport";

/// The authorization header value the probe plugin registers and the test
/// expects the endpoint to observe verbatim.
const TEST_AUTH_HEADER: &str = "Bearer url-server-e2e-token";

/// The request shape the endpoint's `echo` tool last observed.
///
/// The fixture records the tool name, the arguments map, and the inbound
/// `authorization` header here every time `echo` runs, so the test can prove
/// the `tools/call` the plugin issued reached the endpoint with the expected
/// shape.
#[derive(Debug, Clone, Default)]
struct RecordedRequest {
    /// The tool the `tools/call` named — always `echo` in this test.
    tool: String,
    /// The arguments map the `tools/call` carried, as raw JSON.
    arguments: Value,
    /// The inbound HTTP request's `Authorization` header value, if any.
    auth: Option<String>,
}

/// A shared slot the endpoint records each observed request into.
type RecordedSlot = Arc<Mutex<Option<RecordedRequest>>>;

/// Arguments for the endpoint's `echo` tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct EchoArgs {
    /// The payload echoed straight back to the caller.
    message: String,
}

/// The endpoint's `rmcp` server handler, served over HTTP.
///
/// A flat handler with one `echo` tool. It holds the macro-generated tool
/// router and a shared [`RecordedSlot`]; the `echo` tool records the inbound
/// request's shape into that slot so the test can assert on it.
#[derive(Clone)]
struct FixtureServer {
    /// The macro-generated tool router for this handler.
    tool_router: ToolRouter<Self>,
    /// The slot the `echo` tool records each observed request into.
    recorded: RecordedSlot,
}

#[tool_router(router = tool_router)]
impl FixtureServer {
    /// Builds a [`FixtureServer`] that records into `recorded`.
    fn new(recorded: RecordedSlot) -> Self {
        Self {
            tool_router: Self::tool_router(),
            recorded,
        }
    }

    /// Echoes the `message` argument back, recording the request's shape.
    ///
    /// The HTTP request's [`Parts`](http::request::Parts) are injected into
    /// the request context by [`StreamableHttpService`]; this tool reads the
    /// `authorization` header out of them and records it — together with the
    /// tool name and the arguments map — in the shared slot before returning
    /// the echoed payload.
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
            .recorded
            .lock()
            .expect("the recorded-request slot is never poisoned") = Some(RecordedRequest {
            tool: "echo".to_string(),
            arguments: json!({ "message": args.message.clone() }),
            auth,
        });
        args.message
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for FixtureServer {}

/// A running in-process HTTP MCP endpoint plus the request slot it records to.
///
/// Holds the bound URL the plugin's `{ url }` source points at and the shared
/// [`RecordedSlot`] the endpoint's `echo` tool records into. Dropping it
/// aborts the background `axum` server task.
struct Endpoint {
    /// The MCP endpoint URL, e.g. `http://127.0.0.1:PORT/mcp`.
    url: String,
    /// The slot the endpoint records each `echo` call's request into.
    recorded: RecordedSlot,
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
/// loopback port chosen by the OS, and serves it on a background task.
async fn start_endpoint() -> Endpoint {
    let recorded: RecordedSlot = Arc::new(Mutex::new(None));
    let factory_recorded = Arc::clone(&recorded);

    let service = StreamableHttpService::new(
        move || Ok(FixtureServer::new(Arc::clone(&factory_recorded))),
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
        recorded,
        server,
    }
}

/// Builds a real MCP server against an isolated temp working directory.
///
/// The temp `work_dir` keeps the server's bootstrap from walking the real
/// monorepo. The bootstrap registers the unified `files` tool, which is
/// reachable for exposure as the test's observation channel.
async fn build_mcp_server(work_dir: &Path) -> McpServer {
    McpServer::new_with_work_dir(PromptLibrary::new(), work_dir.to_path_buf(), None)
        .await
        .expect("MCP server bootstrap should succeed")
}

/// Encodes `value` as a JSON/TypeScript string literal, quotes included.
///
/// Used to interpolate the endpoint URL, the auth header, and the probe file
/// path into the generated entry module; `serde_json` handles escaping so an
/// unusual value still produces valid source.
fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("a string always serializes to JSON")
}

/// Writes the probe plugin bundle — a TypeScript-only `index.ts` entry —
/// into `<project_root>/plugins/probe/`.
///
/// The bundle's identity is the bundle directory name (`probe`) and its entry
/// module is the conventional `index.ts`.
///
/// The entry module's `load()`:
///
/// 1. registers `url` as a `{ url, headers }` source with an `Authorization`
///    header — the host connects a [`UrlServer`] to the endpoint;
/// 2. registers `fs` as the host-exposed real `files` tool;
/// 3. calls `echo` on the `url` server, crossing a real `tools/call` over HTTP
///    to the endpoint and back;
/// 4. extracts the echoed text from the result and writes it into the probe
///    file through the real `files` tool.
fn write_probe_plugin(project_root: &Path, endpoint_url: &str, probe_path: &Path) {
    let plugin_dir = project_root
        .join(swissarmyhammer_plugin::PLUGINS_SUBDIR)
        .join("probe");
    std::fs::create_dir_all(&plugin_dir).expect("probe plugin directory should be created");

    // The entry module. `load()` registers the HTTP MCP endpoint with an auth
    // header and the real `files` tool, calls `echo` over HTTP, and writes the
    // echoed text to disk. The `indexOf` check makes the plugin fail loudly if
    // the `tools/call` return value is broken, rather than writing an empty
    // file.
    let entry = format!(
        "import {{ Plugin }} from '@swissarmyhammer/plugin';\n\
         \n\
         /** Extracts the echoed text from an `echo` `tools/call` result. */\n\
         function echoedText(result: unknown): string {{\n\
         \x20 const content = (result as {{ content?: Array<{{ text?: string }}> }}).content;\n\
         \x20 if (content === undefined || content.length === 0) {{\n\
         \x20   throw new Error('echo result carried no content');\n\
         \x20 }}\n\
         \x20 const text = content[0].text;\n\
         \x20 if (typeof text !== 'string') {{\n\
         \x20   throw new Error('echo content[0].text was not a string');\n\
         \x20 }}\n\
         \x20 return text;\n\
         }}\n\
         \n\
         export default class ProbePlugin extends Plugin {{\n\
         \x20 async load(): Promise<void> {{\n\
         \x20   // The transport under test: an HTTP MCP endpoint, with auth.\n\
         \x20   this.register('url', {{\n\
         \x20     url: {url},\n\
         \x20     headers: {{ Authorization: {auth} }},\n\
         \x20   }});\n\
         \x20   // The observation channel: the host-exposed real `files` tool.\n\
         \x20   this.register('fs', {{ rust: 'files' }});\n\
         \n\
         \x20   // Call `echo` over HTTP; the result crosses the dispatcher.\n\
         \x20   const echoResult = await this.url.echo({{ message: {payload} }});\n\
         \x20   const echoed = echoedText(echoResult);\n\
         \x20   if (echoed.indexOf({payload}) < 0) {{\n\
         \x20     throw new Error('echo did not return the sent payload');\n\
         \x20   }}\n\
         \n\
         \x20   // Write the echoed text to disk so the test can observe it.\n\
         \x20   await this.fs.files({{\n\
         \x20     op: 'write file',\n\
         \x20     file_path: {probe},\n\
         \x20     content: echoed,\n\
         \x20   }});\n\
         \x20 }}\n\
         }}\n",
        url = json_string(endpoint_url),
        auth = json_string(TEST_AUTH_HEADER),
        probe = json_string(&probe_path.to_string_lossy()),
        payload = json_string(ECHO_PAYLOAD),
    );
    std::fs::write(plugin_dir.join("index.ts"), entry).expect("probe index.ts should be written");
}

/// A discovered probe plugin registers a `{ url }` source and proves a
/// `tools/call` round-trips over HTTP, carrying the configured auth header.
///
/// The plugin's `load()` registers a loopback HTTP MCP endpoint with an
/// `Authorization` header, calls its `echo` tool, and writes the echoed
/// payload to disk through the real `files` tool. The test asserts both the
/// file landed with the echoed payload — proving the response reached the
/// plugin — and that the endpoint recorded the `echo` tool name, the arguments
/// map verbatim, and the auth header — proving the request crossed HTTP with
/// the shape the registration carried.
#[tokio::test]
async fn discovered_plugin_round_trips_a_tools_call_over_the_url_endpoint() {
    // Per-test isolation: every root is this test's own `TempDir`.
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let project_root = tempfile::TempDir::new().expect("project plugin root temp");
    let output_dir = tempfile::TempDir::new().expect("probe output temp");

    // Stand up the real HTTP MCP endpoint the plugin will register.
    let endpoint = start_endpoint().await;
    let probe_path = output_dir.path().join(PROBE_FILE);

    // The probe bundle is laid out under the project layer's `plugins/` dir,
    // where discovery will find it.
    write_probe_plugin(project_root.path(), &endpoint.url, &probe_path);

    // The real in-process tool set, including the unified `files` tool.
    let server = build_mcp_server(work_dir.path()).await;

    // A fresh host, with the project layer pointed at the temp plugin root.
    let host = PluginHost::for_tests(
        work_dir.path().to_path_buf(),
        Some(project_root.path().to_path_buf()),
    );

    // Expose every in-process tool — `files` among them — as an addressable
    // Rust module. No module is live until a plugin activates it.
    tokio::time::timeout(TIMEOUT, server.expose_tools_to_plugin_host(&host))
        .await
        .expect("exposing the in-process tools should not hang")
        .expect("exposing the in-process tools should succeed");

    // Trigger discovery: the host scans the project layer, transpiles the
    // probe's `index.ts`, creates a fresh isolate, and runs `load` — which
    // connects to the HTTP endpoint and drives the `echo` call over HTTP.
    let loaded = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("discovering and loading the probe plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one probe plugin should be discovered and loaded"
    );

    // Assertion 1 — the probe file holds exactly the echoed payload. This can
    // only be true if the `echo` response reached the plugin over HTTP and
    // crossed back through the dispatcher into the isolate.
    let content = std::fs::read_to_string(&probe_path).unwrap_or_else(|error| {
        panic!(
            "the probe file must exist at {} — the echo tools/call did not \
             round-trip over the HTTP endpoint: {error}",
            probe_path.display()
        )
    });
    assert_eq!(
        content, ECHO_PAYLOAD,
        "the probe file must hold the payload echoed back over HTTP — proving \
         the URL transport's response reached the plugin"
    );

    // Assertion 2 — the endpoint recorded the request the plugin issued: the
    // `echo` tool name, the arguments map verbatim, and the auth header the
    // registration carried. This proves the `tools/call` crossed HTTP with
    // the shape `this.register({ url, headers })` described.
    let recorded = endpoint
        .recorded
        .lock()
        .expect("the recorded-request slot is never poisoned")
        .clone()
        .expect("the endpoint should have recorded the plugin's echo request");
    assert_eq!(
        recorded.tool, "echo",
        "the tools/call should have named the echo tool"
    );
    assert_eq!(
        recorded.arguments,
        json!({ "message": ECHO_PAYLOAD }),
        "the tools/call should have carried the plugin's arguments map verbatim"
    );
    assert_eq!(
        recorded.auth.as_deref(),
        Some(TEST_AUTH_HEADER),
        "the tools/call request should have carried the Authorization header \
         the plugin's registration declared"
    );
}
