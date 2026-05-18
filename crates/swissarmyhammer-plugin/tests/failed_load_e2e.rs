//! End-to-end integration test for the **failed-load capability**, driven
//! through a real plugin whose `load()` throws.
//!
//! This is the capability-level companion to the failure cases in
//! `discovery.rs` and `plugin_host.rs`. Where those drive individual rejection
//! mechanics — an undeclared `register`, an escaping manifest entry — this
//! single test proves the one capability the load path must guarantee for
//! *any* failure: a plugin whose `load()` throws leaves the host **exactly as
//! it found it**. The error is surfaced to the caller, no zombie isolate is
//! retained, and no half-built server the plugin managed to register before
//! throwing stays live.
//!
//! It follows the reference shape of `files_dispatch_e2e.rs`: a real V8
//! isolate and a real registered server, with observable effects — the
//! surfaced error, the host's loaded-plugin count, and a dispatch error — that
//! can only hold if the load path cleaned up after itself.
//!
//! # The probe plugin
//!
//! The probe plugin's `load()` does two things, in order:
//!
//! 1. `register('half-built', { rust: 'half-built-mod' })` — a *real*
//!    registration of a real `rmcp` server, made successfully;
//! 2. `throw new Error(...)` — fails the load *after* that registration.
//!
//! Registering before throwing is deliberate: it is the only way to prove the
//! load path *rolls back* a registration a failing plugin had already made. A
//! plugin that threw before registering anything would prove nothing about
//! half-built state.
//!
//! # What a passing run proves
//!
//! 1. `discover_and_load_all` returns `Err`, and the surfaced error carries
//!    the plugin's thrown message — the failure is reported, not swallowed.
//! 2. The host's `Debug` reports `loaded_plugins: 0` — the failed plugin's
//!    isolate was torn down, not left running as a zombie.
//! 3. A call into `half-built` — the server the plugin *did* register before
//!    it threw — fails: the registration was rolled back, so no half-built
//!    server is left serving.
//!
//! If failed-load cleanup is broken — the error is swallowed, the isolate is
//! retained, or the pre-throw registration survives — at least one assertion
//! fails.
//!
//! # Isolation
//!
//! The test owns its own user- and project-layer [`tempfile::TempDir`] roots
//! and a fresh [`PluginHost`]; nothing is `static` and no temp dir is reused.
//! Every cross-thread interaction is bounded by a timeout so a wedged isolate
//! fails the test fast instead of hanging CI.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use serde::{Deserialize, Serialize};
use serde_json::json;
use swissarmyhammer_directory::SwissarmyhammerConfig;
use swissarmyhammer_plugin::{CallerId, Error, InProcessServer, McpServer, PluginHost};

/// A generous upper bound on any single host interaction.
const TIMEOUT: Duration = Duration::from_secs(20);

/// The message the probe plugin's `load()` throws — asserted to appear in the
/// error the host surfaces.
const THROW_MESSAGE: &str = "probe load deliberately fails after registering";

/// Arguments for the probe `rmcp` server's `echo` tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct EchoArgs {
    /// The payload echoed straight back to the caller.
    message: String,
}

/// A real `rmcp` server handler exposing a single flat `echo` tool that
/// returns its `message` argument verbatim.
///
/// The probe plugin registers this genuine `#[tool]` handler before it
/// throws, so the test's "no half-built server" assertion observes a real
/// registration being rolled back — not a mock.
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

/// Writes the probe plugin bundle — `plugin.json` plus an `entry.ts` — into
/// `layer_root/plugins/crasher/`.
///
/// The entry imports the SDK, declares a `Plugin` subclass whose `load()`
/// first registers the real `rmcp` server `half-built` and *then* throws, and
/// exports a `load` lifecycle function. Registering before throwing is what
/// makes the bundle prove the load path rolls back a registration a failing
/// plugin had already made.
fn write_crashing_plugin(layer_root: &Path) {
    let plugin_dir = layer_root.join("plugins").join("crasher");
    std::fs::create_dir_all(&plugin_dir).expect("plugin directory should be created");

    let manifest = "{\n  \
         \"id\": \"crasher\",\n  \
         \"name\": \"failed load probe\",\n  \
         \"version\": \"1.0.0\",\n  \
         \"entry\": \"entry.ts\",\n  \
         \"provides\": [\"half-built\"]\n}\n";
    std::fs::write(plugin_dir.join("plugin.json"), manifest)
        .expect("probe plugin.json should be written");

    // `load()` registers a real server successfully, then throws. The host
    // must roll the registration back and surface the thrown message.
    let entry = format!(
        "import {{ Plugin, makePluginThis }} from '@swissarmyhammer/plugin';\n\
         class P extends Plugin {{\n\
           async load(): Promise<void> {{\n\
             this.register('half-built', {{ rust: 'half-built-mod' }});\n\
             throw new Error({message});\n\
           }}\n\
         }}\n\
         export async function load(): Promise<unknown> {{\n\
           const p = makePluginThis(new P()) as P;\n\
           await p.load();\n\
           return null;\n\
         }}\n",
        message = serde_json::to_string(THROW_MESSAGE).expect("a string always serializes to JSON"),
    );
    std::fs::write(plugin_dir.join("entry.ts"), entry).expect("probe entry.ts should be written");
}

/// The failed-load capability, end to end: a plugin whose `load()` throws
/// leaves the host with no zombie isolate and no half-built server.
///
/// This single test stitches the failed-load cleanup capability together:
///
/// - the probe plugin's `load()` registers a real `rmcp` server and *then*
///   throws — so the test can prove the pre-throw registration is rolled back;
/// - `discover_and_load_all` returns `Err` carrying the plugin's thrown
///   message — the failure is surfaced, not swallowed;
/// - the host's `Debug` reports `loaded_plugins: 0` — the failed plugin's
///   isolate was torn down, leaving no zombie;
/// - a call into `half-built`, the server the plugin did register before it
///   threw, fails — the registration was rolled back, so no half-built server
///   is left serving.
#[tokio::test]
async fn a_plugin_whose_load_throws_leaves_no_zombie_isolate_or_half_built_server() {
    // Per-test isolation: every layer root is this test's own `TempDir`.
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");
    write_crashing_plugin(project.path());

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );
    // Expose the real `rmcp` server the plugin's `load()` registers before it
    // throws, so the registration genuinely succeeds and the test observes a
    // real registration being rolled back.
    let half_built_mod: Arc<dyn McpServer> = Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed"),
    );
    tokio::time::timeout(
        TIMEOUT,
        host.expose_rust_module("half-built-mod", half_built_mod),
    )
    .await
    .expect("expose_rust_module should not hang")
    .expect("exposing the rust module should succeed");

    // Discovery loads the probe plugin, whose `load()` registers a server and
    // then throws — the load must fail.
    let error = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect_err("a plugin whose load() throws must fail to load");

    // Assertion 1 — the failure is surfaced, not swallowed: the error carries
    // the message the plugin's `load()` threw.
    assert!(
        error.to_string().contains(THROW_MESSAGE),
        "the surfaced error must carry the plugin's thrown message, got: {error}"
    );

    // Assertion 2 — no zombie isolate: the failed plugin's isolate was torn
    // down, so the host tracks no loaded plugin.
    let debug = format!("{host:?}");
    assert!(
        debug.contains("loaded_plugins: 0"),
        "a failed load must leave the host with no loaded plugins, got: {debug}"
    );

    // Assertion 3 — no half-built server: `half-built` was registered by the
    // plugin before it threw, but the failed load rolled that registration
    // back, so a call into it no longer reaches a live server.
    let server_err = tokio::time::timeout(
        TIMEOUT,
        host.call(CallerId::HostInternal, "half-built", "echo", json!({})),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect_err("the failed plugin's pre-throw registration must not stay live");
    assert!(
        matches!(server_err, Error::UnknownServer | Error::ServerUnavailable),
        "a rolled-back registration must leave no live server, got: {server_err:?}"
    );
}
