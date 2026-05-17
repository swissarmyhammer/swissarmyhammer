//! Integration tests for [`PluginHost`] — the plugin lifecycle and the
//! per-plugin registration ledger.
//!
//! These tests drive the host end to end: a real plugin bundle is written to a
//! temporary directory, loaded into a real V8 isolate by [`PluginHost::load`],
//! and its `register` / `tools/call` traffic is observed against the host's
//! live [`ServerRegistry`]. The probe plugins register *real* in-process `rmcp`
//! servers, so an assertion observes a genuine round-trip rather than a mock.
//!
//! Every cross-thread interaction is bounded by a timeout so a wedged isolate
//! fails the test fast instead of hanging CI.

use std::sync::Arc;
use std::time::Duration;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_plugin::{CallerId, Error, InProcessServer, McpServer, PluginHost, PluginId};

/// A generous upper bound on any single host interaction.
const TIMEOUT: Duration = Duration::from_secs(20);

/// Arguments for the probe `rmcp` server's `echo` tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct EchoArgs {
    /// The payload echoed straight back to the caller.
    message: String,
}

/// A real `rmcp` server handler built with the macro stack.
///
/// It exposes a single flat `echo` tool that returns its `message` argument
/// verbatim. This is a genuine `#[tool]` handler — the probe plugins register
/// it through the host so the tests dispatch against real `rmcp` machinery.
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

/// Writes a one-file plugin bundle whose `load` export runs `body`.
///
/// The entry imports the SDK, declares a `Plugin` subclass whose `load`
/// contains `body`, and exports a `load` lifecycle function that constructs the
/// subclass — wrapped in the SDK's plugin Proxy — and awaits its `load`. This
/// matches the bundle shape the host's `load(plugin_dir)` expects.
fn write_plugin(dir: &std::path::Path, body: &str) {
    let entry = format!(
        "import {{ Plugin, makePluginThis }} from '@swissarmyhammer/plugin';\n\
         class P extends Plugin {{\n\
           async load(): Promise<void> {{\n{body}\n}}\n\
         }}\n\
         export async function load(): Promise<unknown> {{\n\
           const p = makePluginThis(new P()) as P;\n\
           await p.load();\n\
           return null;\n\
         }}\n"
    );
    std::fs::write(dir.join("entry.ts"), entry).expect("entry.ts should be written");
}

/// `PluginHost::load` runs a probe plugin whose `load()` registers a `rust`
/// module and calls a tool on it; the registered server is reachable through
/// the host's dispatcher and the call observes a real effect.
#[tokio::test]
async fn load_runs_register_and_a_tool_call_on_the_registered_server() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    // The plugin registers the host-exposed `echo-mod` rust module under the
    // name `echo` and immediately calls its `echo` tool.
    write_plugin(
        bundle.path(),
        "this.register('echo', { rust: 'echo-mod' });\n\
         const result = await this.echo.echo({ message: 'hello from plugin' });\n\
         if (JSON.stringify(result).indexOf('hello from plugin') < 0) {\n\
           throw new Error('echo round-trip did not return the payload');\n\
         }",
    );

    let host = PluginHost::for_tests(bundle.path().to_path_buf(), None);
    let echo_mod: Arc<dyn McpServer> = Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed"),
    );
    tokio::time::timeout(TIMEOUT, host.expose_rust_module("echo-mod", echo_mod))
        .await
        .expect("expose_rust_module should not hang")
        .expect("exposing a rust module should succeed");

    let plugin_id = tokio::time::timeout(TIMEOUT, host.load(bundle.path()))
        .await
        .expect("loading the plugin should not hang")
        .expect("the plugin's load should succeed");

    // The plugin's register made `echo` reachable through the host: a call from
    // the host into it round-trips the real rmcp handler.
    let result = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            "echo",
            "echo",
            json!({ "message": "after load" }),
        ),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect("a call into the plugin's registered server should succeed");
    assert!(
        rendered(&result).contains("after load"),
        "the registered server should serve a real rmcp tool call, got {}",
        rendered(&result)
    );

    let _ = plugin_id;
}

/// After `unload`, calls into the plugin's registered server fail with
/// [`Error::ServerUnavailable`] — the name was disposed out from under the
/// caller, not never registered — and the plugin's ledger is empty.
#[tokio::test]
async fn unload_disposes_every_registration_and_empties_the_ledger() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    write_plugin(
        bundle.path(),
        "this.register('weather', { rust: 'weather-mod' });",
    );

    let host = PluginHost::for_tests(bundle.path().to_path_buf(), None);
    let weather: Arc<dyn McpServer> = Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed"),
    );
    tokio::time::timeout(TIMEOUT, host.expose_rust_module("weather-mod", weather))
        .await
        .expect("expose_rust_module should not hang")
        .expect("exposing a rust module should succeed");

    let plugin_id = tokio::time::timeout(TIMEOUT, host.load(bundle.path()))
        .await
        .expect("loading the plugin should not hang")
        .expect("the plugin's load should succeed");

    // Before unload, the ledger records the one registration and the server is
    // reachable.
    let ledger_len = tokio::time::timeout(TIMEOUT, host.ledger_len(&plugin_id))
        .await
        .expect("ledger_len should not hang")
        .expect("the loaded plugin should have a ledger");
    assert_eq!(ledger_len, 1, "the register call should append one handle");

    tokio::time::timeout(TIMEOUT, host.unload(&plugin_id))
        .await
        .expect("unload should not hang")
        .expect("unloading the plugin should succeed");

    // After unload, the disposed server's name is tombstoned: a call into it
    // fails with `ServerUnavailable`, telling the caller the server it was
    // using was disposed out from under it.
    let err = tokio::time::timeout(
        TIMEOUT,
        host.call(CallerId::HostInternal, "weather", "echo", json!({})),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect_err("a call into a disposed server must fail");
    assert!(
        matches!(err, Error::ServerUnavailable),
        "a disposed server must fail with ServerUnavailable, got {err:?}"
    );

    // A name that was never registered still fails with `UnknownServer`, so
    // the disposed and never-existed cases stay distinguishable.
    let unknown_err = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            "never-registered",
            "echo",
            json!({}),
        ),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect_err("a call into a never-registered server must fail");
    assert!(
        matches!(unknown_err, Error::UnknownServer),
        "a never-registered server must fail with UnknownServer, got {unknown_err:?}"
    );

    // The plugin's ledger no longer exists once it is unloaded.
    let ledger_after = tokio::time::timeout(TIMEOUT, host.ledger_len(&plugin_id))
        .await
        .expect("ledger_len should not hang");
    assert!(
        ledger_after.is_none(),
        "an unloaded plugin must have no ledger entry, got {ledger_after:?}"
    );
}

/// `expose_rust_module` followed by a plugin `register({ rust: id })` activates
/// a real in-process `rmcp` server under the plugin's chosen name.
#[tokio::test]
async fn expose_rust_module_then_register_activates_the_in_process_server() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    // The plugin activates the `calculator` rust module under the name `calc`.
    write_plugin(
        bundle.path(),
        "this.register('calc', { rust: 'calculator' });",
    );

    let host = PluginHost::for_tests(bundle.path().to_path_buf(), None);

    // The rust module is a genuine rmcp `InProcessServer`, exposed into the
    // available-modules table — not the live registry.
    let calculator: Arc<dyn McpServer> = Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed"),
    );
    tokio::time::timeout(TIMEOUT, host.expose_rust_module("calculator", calculator))
        .await
        .expect("expose_rust_module should not hang")
        .expect("exposing a rust module should succeed");

    // Before the plugin loads, the module is exposed but NOT live: a call to
    // `calc` is unknown.
    let before = tokio::time::timeout(
        TIMEOUT,
        host.call(CallerId::HostInternal, "calc", "echo", json!({})),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect_err("an un-activated module must not be reachable");
    assert!(
        matches!(before, Error::UnknownServer),
        "expose_rust_module must not make the module live on its own, got {before:?}"
    );

    tokio::time::timeout(TIMEOUT, host.load(bundle.path()))
        .await
        .expect("loading the plugin should not hang")
        .expect("the plugin's load should succeed");

    // After the plugin's `register({ rust: 'calculator' })`, the module is live
    // under `calc` and serves a real rmcp tool call.
    let result = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            "calc",
            "echo",
            json!({ "message": "activated" }),
        ),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect("a call into the activated rust module should succeed");
    assert!(
        rendered(&result).contains("activated"),
        "register({{rust:id}}) must activate the real rmcp server, got {}",
        rendered(&result)
    );
}

/// `PluginHost::new` exists and takes the writable layer roots explicitly; a
/// host built with it loads a plugin the same way `for_tests` does.
#[tokio::test]
async fn new_constructor_takes_explicit_layer_roots() {
    let user_root = tempfile::TempDir::new().expect("user root temp dir");
    let bundle = tempfile::TempDir::new().expect("bundle temp dir");
    write_plugin(bundle.path(), "this.register('svc', { rust: 'svc-mod' });");

    // `new` takes the builtin plugin set plus the writable layer roots; the
    // platform hardcodes no host-specific directory config.
    let host = tokio::time::timeout(
        TIMEOUT,
        PluginHost::new(Vec::new(), user_root.path().to_path_buf(), None),
    )
    .await
    .expect("constructing the host should not hang")
    .expect("a host with no builtins should construct cleanly");
    let svc: Arc<dyn McpServer> = Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed"),
    );
    tokio::time::timeout(TIMEOUT, host.expose_rust_module("svc-mod", svc))
        .await
        .expect("expose_rust_module should not hang")
        .expect("exposing a rust module should succeed");

    let plugin_id = tokio::time::timeout(TIMEOUT, host.load(bundle.path()))
        .await
        .expect("loading the plugin should not hang")
        .expect("the plugin's load should succeed");

    assert!(
        matches!(&plugin_id, PluginId(id) if !id.is_empty()),
        "a loaded plugin must be given a non-empty id"
    );
    let result = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            "svc",
            "echo",
            json!({ "message": "production host" }),
        ),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect("a call through a `new`-built host should succeed");
    assert!(
        rendered(&result).contains("production host"),
        "a host built with `new` must dispatch like any other, got {}",
        rendered(&result)
    );
}

/// A plugin that hands the host functions over the callback-bearing transport
/// path has each function recorded as a ledger `Callback` handle, and `unload`
/// disposes every one of them.
///
/// The probe plugin sends a `callbackDispatch` carrying two functions; the
/// host marshalled them into `$callback` markers and the bridge recorded one
/// ledger handle per marker. After `unload` the plugin has no ledger entry —
/// the callback handles were drained and disposed alongside the isolate.
#[tokio::test]
async fn callbacks_passed_to_the_host_are_tracked_in_the_ledger_and_disposed() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    // The plugin hands the host two functions in one callback-bearing payload.
    write_plugin(
        bundle.path(),
        "this.__transport.callbackDispatch({\n\
           onAdd: () => 'added',\n\
           onMove: () => 'moved',\n\
         });",
    );

    let host = PluginHost::for_tests(bundle.path().to_path_buf(), None);
    let plugin_id = tokio::time::timeout(TIMEOUT, host.load(bundle.path()))
        .await
        .expect("loading the plugin should not hang")
        .expect("the plugin's load should succeed");

    // Each function the plugin passed was marshalled to a `$callback` marker
    // and recorded as one ledger handle.
    let ledger_len = tokio::time::timeout(TIMEOUT, host.ledger_len(&plugin_id))
        .await
        .expect("ledger_len should not hang")
        .expect("the loaded plugin should have a ledger");
    assert_eq!(
        ledger_len, 2,
        "each callback the plugin passed must be recorded as a ledger handle"
    );

    tokio::time::timeout(TIMEOUT, host.unload(&plugin_id))
        .await
        .expect("unload should not hang")
        .expect("unloading the plugin should succeed");

    // After unload the plugin's ledger is gone — its callback handles were
    // drained and disposed.
    let ledger_after = tokio::time::timeout(TIMEOUT, host.ledger_len(&plugin_id))
        .await
        .expect("ledger_len should not hang");
    assert!(
        ledger_after.is_none(),
        "an unloaded plugin must have no ledger entry, got {ledger_after:?}"
    );
}

/// `unload` of a plugin id the host never loaded fails with
/// [`Error::UnknownPlugin`] — distinct from a missing-server failure.
#[tokio::test]
async fn unload_of_an_unknown_plugin_id_fails_with_unknown_plugin() {
    let user_root = tempfile::TempDir::new().expect("user root temp dir");
    let host = PluginHost::for_tests(user_root.path().to_path_buf(), None);

    let stale = PluginId::new("plugin-never-loaded");
    let err = tokio::time::timeout(TIMEOUT, host.unload(&stale))
        .await
        .expect("unload should not hang")
        .expect_err("unloading a never-loaded plugin must fail");
    assert!(
        matches!(err, Error::UnknownPlugin),
        "unload of a stale plugin id must report UnknownPlugin, got {err:?}"
    );
}
