//! End-to-end integration test for the **hot-reload capability**, driven
//! through a real plugin and a real filesystem watcher.
//!
//! This is the capability-level companion to `hot_reload.rs`. Where
//! `hot_reload.rs` drives the reconcile mechanics — the fingerprint guard,
//! reload-policy gating, layer fallback — this single test proves the one
//! capability hot reload exists to deliver: a plugin author *saves a new
//! version of their plugin's source* and the running host picks it up, with
//! the new version's behavior observable in the *same* `PluginHost` instance,
//! no restart.
//!
//! It follows the reference shape of `files_dispatch_e2e.rs`: a real V8
//! isolate, real registered servers, and an observable effect — here, *which
//! distinct server name is live* — that can only be true if the live host
//! genuinely tore down the old isolate and ran the rewritten source.
//!
//! # The two versions
//!
//! A single probe plugin is written into a project-layer temp root. Its two
//! versions are deliberately *different*:
//!
//! - version 1's `load()` registers a server named `behavior-a`;
//! - version 2's `load()` registers a server named `behavior-b`.
//!
//! Both names are declared in the manifest's `provides` up front, so the
//! reload from v1 to v2 is an in-place reload, not a `provides` expansion.
//!
//! # What a passing run proves
//!
//! 1. After the initial `discover_and_load_all`, `behavior-a` answers a real
//!    `rmcp` `echo` call — version 1 ran.
//! 2. The plugin's `entry.ts` is rewritten on disk to version 2 and the
//!    watcher fires: on the *same host* `behavior-b` answers a real `echo`
//!    call and `behavior-a` is gone — the live host reloaded the plugin in
//!    place, tearing down v1's isolate and running v2's source.
//!
//! If hot reload is broken — the watcher does not fire, the host does not
//! reload, or the old isolate is not disposed — at least one assertion fails.
//!
//! # Isolation
//!
//! The test owns its own [`tempfile::TempDir`] roots and a fresh
//! [`PluginHost`]: hot reload is stateful — the reconcile compares the new
//! source against the *prior load's* ledger and fingerprint — so a fresh host
//! with no shared or `static` state is mandatory. The watcher carries a real
//! filesystem debounce, so every wait is bounded by a timeout — a regression
//! fails fast instead of hanging CI.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

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

/// How long the test will poll the live registry for a watcher-driven reload.
///
/// The watcher debounce window plus an isolate teardown-and-reload is well
/// under this; the slack absorbs slow CI filesystems without letting a genuine
/// hang block the suite.
const SETTLE: Duration = Duration::from_secs(15);

/// Arguments for the probe `rmcp` server's `echo` tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct EchoArgs {
    /// The payload echoed straight back to the caller.
    message: String,
}

/// A real `rmcp` server handler exposing a single flat `echo` tool that
/// returns its `message` argument verbatim.
///
/// Each probe plugin version registers this genuine `#[tool]` handler, so an
/// assertion dispatches against real `rmcp` machinery rather than a mock.
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

/// Builds an `Arc<dyn McpServer>` wrapping a fresh real `rmcp` `EchoServer`.
async fn echo_module() -> Arc<dyn McpServer> {
    Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed"),
    )
}

/// Writes the probe plugin's `plugin.json` into `plugin_dir`.
///
/// The manifest declares both server names the plugin will ever register —
/// version 1's and version 2's — up front, so the reload from v1 to v2 is an
/// in-place reload rather than a `provides` expansion.
fn write_manifest(plugin_dir: &Path, version_a: &str, version_b: &str) {
    let manifest = format!(
        "{{\n  \"id\": \"probe\",\n  \"name\": \"hot reload probe\",\n  \
         \"version\": \"1.0.0\",\n  \"entry\": \"entry.ts\",\n  \
         \"provides\": [\"{version_a}\", \"{version_b}\"]\n}}\n"
    );
    std::fs::write(plugin_dir.join("plugin.json"), manifest)
        .expect("plugin.json should be written");
}

/// Overwrites the probe plugin's `entry.ts` with a version whose `load()`
/// registers `server` against the host-exposed `rust` module `rust_module`.
///
/// This is both the initial write and the "rewrite the source" half of the
/// hot-reload test: writing a new `server` makes the watcher observe a genuine
/// content change and reload.
fn write_version(plugin_dir: &Path, server: &str, rust_module: &str) {
    let entry = format!(
        "import {{ Plugin, makePluginThis }} from '@swissarmyhammer/plugin';\n\
         class P extends Plugin {{\n\
           async load(): Promise<void> {{\n\
             this.register('{server}', {{ rust: '{rust_module}' }});\n\
           }}\n\
         }}\n\
         export async function load(): Promise<unknown> {{\n\
           const p = makePluginThis(new P()) as P;\n\
           await p.load();\n\
           return null;\n\
         }}\n"
    );
    std::fs::write(plugin_dir.join("entry.ts"), entry).expect("entry.ts should be written");
}

/// Renders a `tools/call` result to a string for substring assertions.
fn rendered(value: &Value) -> String {
    serde_json::to_string(value).expect("a tools/call result is serializable")
}

/// Asserts a call to `(server, "echo")` succeeds right now and round-trips the
/// real `rmcp` handler with `marker` echoed back.
async fn assert_live(host: &PluginHost, server: &str, marker: &str) {
    let result = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            server,
            "echo",
            json!({ "message": marker }),
        ),
    )
    .await
    .expect("a dispatch call should not hang")
    .unwrap_or_else(|error| panic!("server '{server}' should be live, got {error:?}"));
    assert!(
        rendered(&result).contains(marker),
        "server '{server}' must serve a real rmcp call, got {}",
        rendered(&result)
    );
}

/// Asserts a call to `(server, "echo")` fails — the version that registered it
/// is no longer the live one.
async fn assert_not_live(host: &PluginHost, server: &str) {
    let error = tokio::time::timeout(
        TIMEOUT,
        host.call(CallerId::HostInternal, server, "echo", json!({})),
    )
    .await
    .expect("a dispatch call should not hang")
    .expect_err("the disposed version's server must not be live");
    assert!(
        matches!(error, Error::UnknownServer | Error::ServerUnavailable),
        "a non-live server must fail as UnknownServer/ServerUnavailable, got {error:?}"
    );
}

/// Polls until a call to `(server, "echo")` succeeds, or fails the test after
/// [`SETTLE`].
///
/// Used to wait out the watcher debounce plus an isolate reload: the named
/// server becoming live is the observable that the new version's `load()` ran.
async fn wait_until_live(host: &PluginHost, server: &str) {
    let deadline = Instant::now() + SETTLE;
    loop {
        let result = tokio::time::timeout(
            TIMEOUT,
            host.call(
                CallerId::HostInternal,
                server,
                "echo",
                json!({ "message": "probe" }),
            ),
        )
        .await
        .expect("a dispatch call should not hang");
        if result.is_ok() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "server '{server}' never became live within {SETTLE:?}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// The hot-reload capability, end to end: rewriting a running plugin's source
/// makes the live host reload it in place, with the new version's behavior
/// observable in the same host.
///
/// This single test stitches the hot-reload capability together:
///
/// - version 1 of the probe plugin is discovered and loaded, observed by
///   `behavior-a` answering a real `echo` call;
/// - the layer watcher is started, then the plugin's `entry.ts` is rewritten
///   to version 2 on disk — the "save a new version" plugin-author action;
/// - the watcher fires and the host reloads the plugin in place: on the same
///   host `behavior-b` becomes live and `behavior-a` is disposed — proof the
///   live host tore down v1's isolate and ran v2's rewritten source.
#[tokio::test]
async fn rewriting_a_running_plugins_source_hot_reloads_it_in_the_same_host() {
    // Per-test isolation: every root is this test's own `TempDir`. A fresh
    // host is mandatory — reload compares against the prior load's ledger.
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");
    let plugin_dir = project.path().join("plugins").join("probe");
    std::fs::create_dir_all(&plugin_dir).expect("probe plugin directory should be created");

    // The manifest declares both versions' server names up front so the reload
    // is an in-place reload, not a `provides` expansion.
    write_manifest(&plugin_dir, "behavior-a", "behavior-b");
    // Version 1: `load()` registers `behavior-a`.
    write_version(&plugin_dir, "behavior-a", "mod-a");

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );
    // Both versions' `rust` modules are exposed up front so the watcher-driven
    // v2 `load()` — which may run the moment the debounce elapses — never
    // races a missing module.
    tokio::time::timeout(
        TIMEOUT,
        host.expose_rust_module("mod-a", echo_module().await),
    )
    .await
    .expect("expose_rust_module should not hang")
    .expect("exposing the v1 rust module should succeed");
    tokio::time::timeout(
        TIMEOUT,
        host.expose_rust_module("mod-b", echo_module().await),
    )
    .await
    .expect("expose_rust_module should not hang")
    .expect("exposing the v2 rust module should succeed");

    // Load version 1 through point-in-time discovery.
    tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("the initial discovery should succeed");

    // Behavior A — version 1 ran: its server answers a real `echo` call.
    assert_live(&host, "behavior-a", "version one is running").await;
    assert_not_live(&host, "behavior-b").await;

    // Start the watcher so a save to the plugin's source reloads it in place.
    let _watcher = tokio::time::timeout(TIMEOUT, host.watch_plugins::<SwissarmyhammerConfig>())
        .await
        .expect("starting the watcher should not hang")
        .expect("the watcher should start");
    // Let the OS watcher register before mutating the tree.
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Rewrite the plugin's source to version 2 — the plugin author saving a
    // new version of their plugin while the host runs.
    write_version(&plugin_dir, "behavior-b", "mod-b");

    // Behavior B — the watcher fired and the live host reloaded the plugin in
    // place: version 2's server is live and version 1's is disposed. This can
    // only be true if the same `PluginHost` tore down v1's isolate and ran the
    // rewritten v2 source.
    wait_until_live(&host, "behavior-b").await;
    assert_live(&host, "behavior-b", "version two is running").await;
    assert_not_live(&host, "behavior-a").await;
}
