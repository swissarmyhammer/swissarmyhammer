//! End-to-end integration test for the **discovery / layer-precedence
//! capability**, driven through real plugins.
//!
//! This is the capability-level companion to `discovery.rs`. Where
//! `discovery.rs` drives the mechanics of bundle resolution and layer
//! stacking, this single test proves the one capability
//! the layering machinery exists to deliver: when the *same plugin id* lives in
//! two writable layers, the higher-precedence (project) copy is the one that
//! runs — and when that copy is removed, the lower-precedence (user) copy
//! re-emerges as the active one.
//!
//! It follows the reference shape of `files_dispatch_e2e.rs`: a real V8
//! isolate, real registered servers, and an observable effect — here, *which
//! distinct server name is live* — that can only be true if discovery and
//! layer reconciliation resolved the layers correctly.
//!
//! # The two copies
//!
//! Two bundles sharing the directory name `shared` are laid down in both the
//! user-layer and the project-layer temp roots — a bundle's identity is its
//! directory name, so the shared directory name is the shared plugin id. The
//! two copies are deliberately *different*:
//!
//! - the user copy registers a server named `from-user`;
//! - the project copy registers a server named `from-project`.
//!
//! Only one copy of a shadowed id loads, so which server name becomes live is
//! an unambiguous read on which copy is active.
//!
//! # What a passing run proves
//!
//! 1. With both copies present, `discover_and_load_all` loads exactly one
//!    plugin and `from-project` answers a real `rmcp` `echo` call while
//!    `from-user` never came live — the project layer shadowed the user layer.
//! 2. After the project copy is removed from disk, the layer watcher fires and
//!    the *same host* falls back to the user copy: `from-user` now answers a
//!    real `echo` call while `from-project` is gone — the lower layer
//!    re-emerged as active.
//!
//! If layer precedence is broken — the user copy wins, or both copies load, or
//! the fallback after removal does not happen — at least one assertion fails.
//!
//! # Isolation
//!
//! The test owns its own user- and project-layer [`tempfile::TempDir`] roots
//! and a fresh [`PluginHost`]; nothing is `static` and no temp dir is reused.
//! The layer watcher carries a real filesystem debounce, so every wait is
//! bounded by a timeout — a regression fails fast instead of hanging CI.

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
///
/// Each interaction is wrapped in it so a wedged isolate fails the test fast
/// instead of hanging CI.
const TIMEOUT: Duration = Duration::from_secs(20);

/// How long the test will poll the live registry for a watcher-driven layer
/// change.
///
/// The watcher debounce window plus an isolate teardown-and-load is well under
/// this; the slack absorbs slow CI filesystems without letting a genuine hang
/// block the suite.
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
/// The probe plugins register this genuine `#[tool]` handler, so an assertion
/// dispatches against real `rmcp` machinery rather than a mock.
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

/// Writes a TypeScript-only probe plugin bundle — just an `index.ts` entry —
/// into `layer_root/plugins/<dir_name>/`.
///
/// The bundle's identity is its bundle directory name (`dir_name`) and its
/// entry module is the conventional `index.ts`. The entry imports the SDK,
/// declares a `Plugin` subclass whose `load` registers `server` against the
/// host-exposed `rust` module `rust_module`, and exports a `load` lifecycle
/// function. Two
/// copies sharing the same `dir_name` written into different layers — so they
/// share an identity — with different `server` names are how the test reads
/// which copy is active.
fn write_layer_copy(layer_root: &Path, dir_name: &str, server: &str, rust_module: &str) {
    let plugin_dir = layer_root.join("plugins").join(dir_name);
    std::fs::create_dir_all(&plugin_dir).expect("plugin directory should be created");

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
    std::fs::write(plugin_dir.join("index.ts"), entry).expect("index.ts should be written");
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
    .unwrap_or_else(|error| panic!("server '{server}' should be the active copy, got {error:?}"));
    assert!(
        rendered(&result).contains(marker),
        "server '{server}' must serve a real rmcp call, got {}",
        rendered(&result)
    );
}

/// Asserts a call to `(server, "echo")` fails — the copy that would register
/// it is not the active one, so its server name is not in the live registry.
async fn assert_not_live(host: &PluginHost, server: &str) {
    let error = tokio::time::timeout(
        TIMEOUT,
        host.call(CallerId::HostInternal, server, "echo", json!({})),
    )
    .await
    .expect("a dispatch call should not hang")
    .expect_err("the non-active copy's server must not be live");
    assert!(
        matches!(error, Error::UnknownServer | Error::ServerUnavailable),
        "a non-live server must fail as UnknownServer/ServerUnavailable, got {error:?}"
    );
}

/// Polls until a call to `(server, "echo")` succeeds, or fails the test after
/// [`SETTLE`].
///
/// Used to wait out the watcher debounce plus an isolate load: the named server
/// becoming live is the observable that the fallback copy's `load()` has run.
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

/// The discovery / layering capability, end to end: the project copy of a
/// shadowed id wins, and removing it lets the user copy re-emerge.
///
/// This single test stitches the layering capability together:
///
/// - a bundle with the directory name `shared` — and thus the id `shared` —
///   is laid down in both the user and project temp layers, each copy
///   registering a distinct server name;
/// - `discover_and_load_all` resolves the shadowed id to one active copy — the
///   project copy — observed by `from-project` answering a real `echo` call
///   while `from-user` never came live;
/// - the project copy is removed from disk: the layer watcher fires and the
///   same host falls back to the user copy, observed by `from-user` now
///   answering a real `echo` call while `from-project` is gone.
#[tokio::test]
async fn project_layer_shadows_user_and_removal_falls_back_to_user() {
    // Per-test isolation: every layer root is this test's own `TempDir`.
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");

    // The same directory name `shared` — hence the same id — in both layers,
    // behaving differently: the user copy registers `from-user`, the project
    // copy registers `from-project`. Which server name becomes live tells the
    // test which copy is active.
    write_layer_copy(user.path(), "shared", "from-user", "user-mod");
    write_layer_copy(project.path(), "shared", "from-project", "project-mod");

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );
    // Both copies' `rust` modules are exposed up front so neither copy's
    // `register` can fail for a reason unrelated to layer precedence, and the
    // watcher-driven fallback load never races a missing module.
    tokio::time::timeout(
        TIMEOUT,
        host.expose_rust_module("user-mod", echo_module().await),
    )
    .await
    .expect("expose_rust_module should not hang")
    .expect("exposing the user rust module should succeed");
    tokio::time::timeout(
        TIMEOUT,
        host.expose_rust_module("project-mod", echo_module().await),
    )
    .await
    .expect("expose_rust_module should not hang")
    .expect("exposing the project rust module should succeed");

    // First discovery: both copies are present, so the project layer must
    // shadow the user layer.
    let loaded = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("discovering the shared plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "a shadowed id must resolve to exactly one active copy, not two"
    );

    // The project copy is the active one: its server answers a real `echo`
    // call, and the shadowed user copy's server never came live.
    assert_live(&host, "from-project", "project layer wins").await;
    assert_not_live(&host, "from-user").await;

    // Start the layer watcher so a change to the writable roots reconciles the
    // host in place.
    let _watcher = tokio::time::timeout(TIMEOUT, host.watch_plugins::<SwissarmyhammerConfig>())
        .await
        .expect("starting the watcher should not hang")
        .expect("the watcher should start");
    // Let the OS watcher register before mutating the tree.
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Remove the project-layer copy from disk. The watcher fires a `Removed`
    // for the active layer, so the host falls back to the user copy — the only
    // copy of the id `shared` left on disk.
    std::fs::remove_dir_all(project.path().join("plugins").join("shared"))
        .expect("removing the project-layer copy should succeed");

    // The user copy re-emerges as active: its server answers a real `echo`
    // call, and the removed project copy's server is no longer live.
    wait_until_live(&host, "from-user").await;
    assert_live(&host, "from-user", "user layer re-emerges").await;
    assert_not_live(&host, "from-project").await;
}
