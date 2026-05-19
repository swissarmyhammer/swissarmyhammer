//! End-to-end integration test for the **unload-disposal capability**, driven
//! through a real plugin.
//!
//! This is the capability-level companion to the `unload` cases in
//! `plugin_host.rs`. Where those drive the ledger mechanics one handle kind at
//! a time, this single test proves the one capability `unload` exists to
//! deliver: after a plugin is unloaded, *everything it registered is gone* —
//! calls into the server it registered fail, and the callbacks it handed the
//! host can no longer fire, because the isolate that backed both has been torn
//! down.
//!
//! It follows the reference shape of `files_dispatch_e2e.rs`: a real V8
//! isolate, a real registered server, and observable effects — a real `rmcp`
//! `echo` round-trip, and the dispatch errors that prove disposal — that can
//! only hold if `unload` genuinely reclaimed the plugin.
//!
//! # What the probe plugin registers
//!
//! The probe plugin's `load()` does two things, so `unload` has both handle
//! kinds to dispose:
//!
//! - `register('weather', { rust: 'weather-mod' })` — activates a real
//!   in-process `rmcp` server. While loaded, a `tools/call` into `weather`
//!   round-trips that real handler.
//! - `this.__transport.callbackDispatch({ ... })` — hands the host two
//!   functions. The SDK marshals each into a `$callback` marker and the host
//!   records one ledger handle per marker; the stored functions live in the
//!   plugin's isolate callback table.
//!
//! # What a passing run proves
//!
//! 1. While the plugin is loaded, `weather` answers a real `echo` call and the
//!    plugin's ledger holds three handles — the one server plus the two
//!    callbacks.
//! 2. After `unload`, a call into `weather` fails with
//!    [`Error::ServerUnavailable`] — the name was registered and then disposed,
//!    distinct from a name that never existed — and the plugin's ledger is gone
//!    entirely: the callback handles were drained and disposed against the
//!    isolate, and the isolate itself was torn down, so neither the server nor
//!    the callbacks can serve another request.
//!
//! If `unload` disposal is broken — the server stays live, the callbacks are
//! left in the ledger, or the isolate is not reclaimed — at least one
//! assertion fails.
//!
//! # Isolation
//!
//! The test owns its own [`tempfile::TempDir`] layer root and a fresh
//! [`PluginHost`]; nothing is `static` and no temp dir is reused. Every
//! cross-thread interaction is bounded by a timeout so a wedged isolate fails
//! the test fast instead of hanging CI.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_directory::SwissarmyhammerConfig;
use swissarmyhammer_plugin::{CallerId, Error, InProcessServer, McpServer, PluginHost};

/// A generous upper bound on any single host interaction.
const TIMEOUT: Duration = Duration::from_secs(20);

/// Arguments for the probe `rmcp` server's `echo` tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct EchoArgs {
    /// The payload echoed straight back to the caller.
    message: String,
}

/// A real `rmcp` server handler exposing a single flat `echo` tool that
/// returns its `message` argument verbatim.
///
/// The probe plugin registers this genuine `#[tool]` handler, so the
/// before-unload assertion dispatches against real `rmcp` machinery rather
/// than a mock.
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
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for EchoServer {}

/// Renders a `tools/call` result to a string for substring assertions.
fn rendered(value: &Value) -> String {
    serde_json::to_string(value).expect("a tools/call result is serializable")
}

/// Writes the probe plugin bundle — a TypeScript-only `index.ts` entry —
/// into `<layer_root>/plugins/weather-probe/`.
///
/// The bundle's identity is its bundle directory name (`weather-probe`) and
/// its entry module is the conventional `index.ts`. The entry imports the SDK,
/// declares a `Plugin` subclass whose `load()` registers the real `rmcp` server
/// `weather` and hands the host two functions in a callback-bearing payload,
/// then exports a `load` lifecycle function. A plugin that registers *both*
/// handle kinds is what makes the test prove `unload` disposes a server *and*
/// callbacks in one capability.
///
/// The bundle is staged under the layer's `plugins/` directory so the platform
/// discovers it through `discover_and_load_all`.
fn write_probe_plugin(layer_root: &Path) {
    let plugin_dir = layer_root
        .join(swissarmyhammer_plugin::PLUGINS_SUBDIR)
        .join("weather-probe");
    std::fs::create_dir_all(&plugin_dir).expect("probe plugin directory should be created");

    // `load()` activates the real `weather` server and hands the host two
    // callback functions, so the ledger holds one server handle and two
    // callback handles when the plugin is loaded.
    let entry = "import { Plugin, makePluginThis } from '@swissarmyhammer/plugin';\n\
         class P extends Plugin {\n\
           async load(): Promise<void> {\n\
             this.register('weather', { rust: 'weather-mod' });\n\
             this.__transport.callbackDispatch({\n\
               onForecast: () => 'forecast',\n\
               onAlert: () => 'alert',\n\
             });\n\
           }\n\
         }\n\
         export async function load(): Promise<unknown> {\n\
           const p = makePluginThis(new P()) as P;\n\
           await p.load();\n\
           return null;\n\
         }\n";
    std::fs::write(plugin_dir.join("index.ts"), entry).expect("probe index.ts should be written");
}

/// The unload-disposal capability, end to end: a plugin that registered a
/// server and callbacks is fully reclaimed by `unload`.
///
/// This single test stitches the disposal capability together:
///
/// - the probe plugin's `load()` registers a real `rmcp` server `weather` and
///   hands the host two callback functions;
/// - while loaded, `weather` answers a real `echo` call and the ledger records
///   all three handles — proof both the server and the callbacks are live;
/// - after `unload`, a call into `weather` fails with
///   [`Error::ServerUnavailable`] and the plugin's ledger is gone — proof the
///   server was disposed, the callbacks were disposed against the isolate, and
///   the isolate was torn down, so nothing the plugin registered can fire.
#[tokio::test]
async fn unload_disposes_a_real_plugins_server_and_callbacks() {
    // Per-test isolation: the layer root is this test's own `TempDir`.
    let layer = tempfile::TempDir::new().expect("plugin layer temp dir");
    write_probe_plugin(layer.path());

    let host = PluginHost::for_tests(layer.path().to_path_buf(), None);
    // Expose the real `rmcp` server the plugin will activate as `weather`.
    let weather_mod: Arc<dyn McpServer> = Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed"),
    );
    tokio::time::timeout(TIMEOUT, host.expose_rust_module("weather-mod", weather_mod))
        .await
        .expect("expose_rust_module should not hang")
        .expect("exposing the rust module should succeed");

    // Discover and load the probe plugin: the host scans the layer's
    // `plugins/` directory, resolves the bundle's `index.ts` entry, and runs
    // `load()` — which registers `weather` and hands the host two callbacks.
    let mut loaded = tokio::time::timeout(
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
    let plugin_id = loaded.pop().expect("the discovered plugin's id");

    // While loaded: the registered server answers a real `rmcp` `echo` call.
    let result = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            "weather",
            "echo",
            json!({ "message": "before unload" }),
        ),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect("a call into the plugin's registered server should succeed while loaded");
    assert!(
        rendered(&result).contains("before unload"),
        "the registered server must serve a real rmcp call while loaded, got {}",
        rendered(&result)
    );

    // While loaded: the ledger holds three handles — the one server and the
    // two callbacks the plugin handed the host.
    let ledger_len = tokio::time::timeout(TIMEOUT, host.ledger_len(&plugin_id))
        .await
        .expect("ledger_len should not hang")
        .expect("the loaded plugin should have a ledger");
    assert_eq!(
        ledger_len, 3,
        "the loaded plugin must hold one server handle and two callback handles"
    );

    // Unload the plugin: the host disposes every registration and tears the
    // isolate down.
    tokio::time::timeout(TIMEOUT, host.unload(&plugin_id))
        .await
        .expect("unload should not hang")
        .expect("unloading the plugin should succeed");

    // After unload: a call into the disposed server fails with
    // `ServerUnavailable` — the name was registered and then disposed, which
    // is deliberately distinct from a name that never existed.
    let server_err = tokio::time::timeout(
        TIMEOUT,
        host.call(CallerId::HostInternal, "weather", "echo", json!({})),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect_err("a call into the unloaded plugin's server must fail");
    assert!(
        matches!(server_err, Error::ServerUnavailable),
        "an unloaded plugin's disposed server must fail with ServerUnavailable, \
         got {server_err:?}"
    );

    // After unload: the plugin's ledger is gone entirely. The callback handles
    // were drained and disposed against the isolate, and the isolate itself
    // was torn down — so the callbacks the plugin handed the host can never
    // fire again.
    let ledger_after = tokio::time::timeout(TIMEOUT, host.ledger_len(&plugin_id))
        .await
        .expect("ledger_len should not hang");
    assert!(
        ledger_after.is_none(),
        "an unloaded plugin must have no ledger — its server and callback \
         handles must all be disposed, got {ledger_after:?}"
    );
}
