//! Integration tests for hot reload driven by the directory watcher.
//!
//! These tests drive [`PluginHost`] end to end with a *real*
//! `swissarmyhammer-directory` [`Watcher`](swissarmyhammer_directory::Watcher):
//! a probe plugin bundle is written into a temporary project-layer plugins
//! root, the host is told to watch that root, and the plugin's source (or its
//! presence in a layer) is mutated on disk. The watcher fires, the host
//! translates the [`StackedEvent`](swissarmyhammer_directory::StackedEvent)
//! into a load / reload / unload, and the test observes the new behavior in
//! the *same* host — exactly the "write source, observe behavior; rewrite
//! source, observe new behavior" shape the architecture doc prescribes.
//!
//! Observation is the live [`ServerRegistry`]: each probe plugin version
//! registers a *distinct* server name, so which name is live tells the test
//! which version is active. The registered servers are real in-process `rmcp`
//! servers — no mocks.
//!
//! These tests are timing-sensitive: a real filesystem watcher with a real
//! debounce window sits in the loop. Every wait is bounded, so a regression
//! fails fast instead of hanging CI.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use serde::{Deserialize, Serialize};
use serde_json::json;
use swissarmyhammer_directory::SwissarmyhammerConfig;
use swissarmyhammer_plugin::{
    CallerId, Error, InProcessServer, McpServer, PluginHost, ReloadStatus,
};

/// A generous upper bound on any single host interaction.
const TIMEOUT: Duration = Duration::from_secs(20);

/// How long a test will poll the live registry for a watcher-driven change.
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

/// Writes a probe plugin bundle — a single `index.ts` entry — into
/// `layer_root/plugins/<dir_name>/`.
///
/// A plugin's identity is its bundle directory name, so `dir_name` is also the
/// plugin id. The entry imports the SDK, declares a `Plugin` subclass whose
/// `load` runs `body`, and exports a `load` lifecycle function.
fn write_bundle(layer_root: &Path, dir_name: &str, body: &str) {
    let plugin_dir = layer_root.join("plugins").join(dir_name);
    std::fs::create_dir_all(&plugin_dir).expect("plugin directory should be created");
    write_entry(&plugin_dir, body);
}

/// Overwrites the `index.ts` of an already-written bundle with a new `load`
/// body — the "rewrite the source" half of a hot-reload test.
fn write_entry(plugin_dir: &Path, body: &str) {
    let source = format!(
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
    std::fs::write(plugin_dir.join("index.ts"), source).expect("index.ts should be written");
}

/// Polls until a call to `(server, "echo")` succeeds, or fails the test after
/// [`SETTLE`].
///
/// Used to wait out the watcher debounce plus an isolate reload: the named
/// server becoming live is the observable that the new plugin version's
/// `load()` has run.
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
        if Instant::now() >= deadline {
            panic!("server '{server}' never became live within {SETTLE:?}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Polls until a call to `(server, "echo")` no longer succeeds, or fails the
/// test after [`SETTLE`].
///
/// The complement of [`wait_until_live`]: a server going non-live is the
/// observable that the old plugin version was disposed.
async fn wait_until_gone(host: &PluginHost, server: &str) {
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
        if result.is_err() {
            return;
        }
        if Instant::now() >= deadline {
            panic!("server '{server}' was still live after {SETTLE:?}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Asserts a call to `(server, "echo")` succeeds right now and round-trips the
/// real `rmcp` handler.
async fn assert_live(host: &PluginHost, server: &str) {
    let result = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            server,
            "echo",
            json!({ "message": "live-check" }),
        ),
    )
    .await
    .expect("a dispatch call should not hang")
    .unwrap_or_else(|error| panic!("server '{server}' should be live, got {error:?}"));
    let rendered = serde_json::to_string(&result).expect("a tools/call result serializes");
    assert!(
        rendered.contains("live-check"),
        "server '{server}' must serve a real rmcp call, got {rendered}"
    );
}

/// Asserts a call to `(server, "echo")` fails — the server is not live.
async fn assert_not_live(host: &PluginHost, server: &str) {
    let error = tokio::time::timeout(
        TIMEOUT,
        host.call(CallerId::HostInternal, server, "echo", json!({})),
    )
    .await
    .expect("a dispatch call should not hang")
    .expect_err("server should not be live");
    assert!(
        matches!(error, Error::UnknownServer | Error::ServerUnavailable),
        "a non-live server must fail as UnknownServer/ServerUnavailable, got {error:?}"
    );
}

/// Writing a new `load()` body to an active plugin's entry source makes the
/// watcher fire; the host reloads the plugin in place and the *new* version's
/// behavior is observable in the *same* host.
///
/// Version 1 registers `probe-v1`; version 2 registers `probe-v2`. After the
/// reload the v1 server is disposed and the v2 server is live — proof the
/// reload tore down the old ledger and ran fresh source.
#[tokio::test]
async fn rewriting_an_active_plugins_source_reloads_it_in_place() {
    let project = tempfile::TempDir::new().expect("project root temp dir");
    let plugin_dir = project.path().join("plugins").join("probe-dir");
    // Version 1: registers `probe-v1`.
    write_bundle(
        project.path(),
        "probe-dir",
        "this.register('probe-v1', { rust: 'probe-mod' });",
    );

    let host = PluginHost::for_tests(
        tempfile::TempDir::new()
            .expect("user root temp dir")
            .path()
            .to_path_buf(),
        Some(project.path().to_path_buf()),
    );
    tokio::time::timeout(
        TIMEOUT,
        host.expose_rust_module("probe-mod", echo_module().await),
    )
    .await
    .expect("expose_rust_module should not hang")
    .expect("exposing a rust module should succeed");

    // Load version 1 through point-in-time discovery, then start the watcher.
    tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("the initial discovery should succeed");
    assert_live(&host, "probe-v1").await;

    // Expose the v2 rust module up front so the reloaded `load()` — which the
    // watcher may run as soon as the debounce elapses — never races a missing
    // module.
    tokio::time::timeout(
        TIMEOUT,
        host.expose_rust_module("probe-mod-2", echo_module().await),
    )
    .await
    .expect("expose_rust_module should not hang")
    .expect("exposing the v2 rust module should succeed");

    let _watcher = tokio::time::timeout(TIMEOUT, host.watch_plugins::<SwissarmyhammerConfig>())
        .await
        .expect("starting the watcher should not hang")
        .expect("the watcher should start");
    // Let the OS watcher register before mutating the tree.
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Rewrite the source: version 2 registers `probe-v2` instead.
    write_entry(
        &plugin_dir,
        "this.register('probe-v2', { rust: 'probe-mod-2' });",
    );

    // The watcher fires, the host reloads: the v2 server comes live and the v1
    // server is disposed.
    wait_until_live(&host, "probe-v2").await;
    assert_not_live(&host, "probe-v1").await;
}

/// A reload whose new `load()` throws leaves the plugin UNLOADED: there is no
/// fallback to the old version (its isolate was already torn down), the error
/// is surfaced via [`PluginHost::reload_status`], and no server from either
/// version is registered.
#[tokio::test]
async fn a_failed_v2_load_leaves_the_plugin_unloaded_and_surfaces_the_error() {
    let project = tempfile::TempDir::new().expect("project root temp dir");
    let plugin_dir = project.path().join("plugins").join("crash-dir");
    write_bundle(
        project.path(),
        "crash-dir",
        "this.register('crash-v1', { rust: 'crash-mod' });",
    );

    let host = PluginHost::for_tests(
        tempfile::TempDir::new()
            .expect("user root temp dir")
            .path()
            .to_path_buf(),
        Some(project.path().to_path_buf()),
    );
    tokio::time::timeout(
        TIMEOUT,
        host.expose_rust_module("crash-mod", echo_module().await),
    )
    .await
    .expect("expose_rust_module should not hang")
    .expect("exposing a rust module should succeed");

    tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("the initial discovery should succeed");
    assert_live(&host, "crash-v1").await;

    let _watcher = tokio::time::timeout(TIMEOUT, host.watch_plugins::<SwissarmyhammerConfig>())
        .await
        .expect("starting the watcher should not hang")
        .expect("the watcher should start");
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Version 2's `load()` throws before it can register anything.
    write_entry(
        &plugin_dir,
        "throw new Error('v2 load deliberately fails');",
    );

    // The v1 server is disposed as the reload tears the old isolate down, and
    // v2 never comes up — so `crash-v1` goes non-live and stays non-live.
    wait_until_gone(&host, "crash-v1").await;
    assert_not_live(&host, "crash-v1").await;

    // The failure is surfaced, not silent: the plugin's status records it.
    let deadline = Instant::now() + SETTLE;
    let status = loop {
        match host.reload_status("crash-dir").await {
            Some(ReloadStatus::Failed { error }) => break error,
            _ if Instant::now() >= deadline => {
                panic!("the failed reload was never surfaced as ReloadStatus::Failed");
            }
            _ => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    };
    assert!(
        status.contains("v2 load deliberately fails"),
        "the surfaced status must carry the v2 load error, got: {status}"
    );
}

/// Removing the project-layer copy of a plugin that also exists in the user
/// layer makes the watcher fire a `Removed`; the host falls back to the user
/// copy, which becomes the active one.
#[tokio::test]
async fn removing_the_active_layer_falls_back_to_the_lower_layer() {
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");

    // A bundle named `shared` in both layers, so both have id `shared`. The
    // user copy registers `from-user`; the project copy registers
    // `from-project`. Project shadows user, so `from-project` is active first.
    write_bundle(
        user.path(),
        "shared",
        "this.register('from-user', { rust: 'shared-mod-user' });",
    );
    write_bundle(
        project.path(),
        "shared",
        "this.register('from-project', { rust: 'shared-mod-project' });",
    );

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );
    tokio::time::timeout(
        TIMEOUT,
        host.expose_rust_module("shared-mod-user", echo_module().await),
    )
    .await
    .expect("expose_rust_module should not hang")
    .expect("exposing the user rust module should succeed");
    tokio::time::timeout(
        TIMEOUT,
        host.expose_rust_module("shared-mod-project", echo_module().await),
    )
    .await
    .expect("expose_rust_module should not hang")
    .expect("exposing the project rust module should succeed");

    tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("the initial discovery should succeed");
    // Project wins initially.
    assert_live(&host, "from-project").await;
    assert_not_live(&host, "from-user").await;

    let _watcher = tokio::time::timeout(TIMEOUT, host.watch_plugins::<SwissarmyhammerConfig>())
        .await
        .expect("starting the watcher should not hang")
        .expect("the watcher should start");
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Remove the project-layer copy: the watcher fires a Removed for the
    // active layer, so the host falls back to the user copy.
    std::fs::remove_dir_all(project.path().join("plugins").join("shared"))
        .expect("removing the project copy should succeed");

    wait_until_live(&host, "from-user").await;
    assert_not_live(&host, "from-project").await;
}

/// Modifying a *shadowed* lower-layer copy of a plugin id must NOT reload the
/// active higher-layer copy.
///
/// The id `shared` exists in both layers; the project copy shadows the user
/// copy, so the project copy is active. The user copy is then edited. Because
/// the watcher re-runs full discovery and reconciles every id on any event,
/// the host's `reconcile_id` is reached for `shared` even though its active
/// (project) copy did not change — the content-fingerprint guard is what must
/// make that reconcile a no-op.
///
/// The observation is deliberately race-free. The active project copy is
/// loaded with a `rust` module that `activate_rust_module` *consumes* out of
/// the host's module table on first registration. So a *spurious* reload would
/// re-run the project copy's `load()`, find `project-mod` already gone, and
/// fail with `UnknownServer` — leaving the plugin unloaded with a `Failed`
/// status. The fix therefore turns a would-be `Failed` into a steady `Healthy`
/// and a continuously-live `project-active` server: the test asserts exactly
/// that, so a regression is caught as a hard failure rather than a flaky one.
#[tokio::test]
async fn modifying_a_shadowed_layer_does_not_reload_the_active_copy() {
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");

    // A bundle named `shared` in both layers, so both have id `shared`. The
    // project copy is active and registers `project-active`; the shadowed user
    // copy registers `user-shadowed` and is not live.
    write_bundle(
        user.path(),
        "shared",
        "this.register('user-shadowed', { rust: 'user-mod' });",
    );
    write_bundle(
        project.path(),
        "shared",
        "this.register('project-active', { rust: 'project-mod' });",
    );

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );
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

    tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("the initial discovery should succeed");
    // The project copy wins; its server is live and its status is healthy.
    assert_live(&host, "project-active").await;
    assert_not_live(&host, "user-shadowed").await;
    assert!(
        matches!(
            host.reload_status("shared").await,
            Some(ReloadStatus::Healthy)
        ),
        "the freshly loaded project copy must report Healthy",
    );

    let _watcher = tokio::time::timeout(TIMEOUT, host.watch_plugins::<SwissarmyhammerConfig>())
        .await
        .expect("starting the watcher should not hang")
        .expect("the watcher should start");
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Edit the *shadowed* user copy's entry source. This is a `Modified` of a
    // non-winning layer: the active project copy's bytes are untouched.
    write_entry(
        &user.path().join("plugins").join("shared"),
        "this.register('user-shadowed', { rust: 'user-mod-2' });",
    );

    // Wait out the watcher debounce and a full reconcile cycle. A spurious
    // reload would have surfaced as `Failed` by now; correct behavior leaves
    // the status steady at `Healthy`.
    let deadline = Instant::now() + SETTLE;
    while Instant::now() < deadline {
        let status = host.reload_status("shared").await;
        assert!(
            matches!(status, Some(ReloadStatus::Healthy)),
            "modifying a shadowed layer must not reload the active copy, \
             but its status became {status:?}",
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // The active project copy is still serving — never torn down, never
    // re-`load()`ed — and the shadowed copy never came live.
    assert_live(&host, "project-active").await;
    assert_not_live(&host, "user-shadowed").await;
}

/// A genuine `Modified` of the *active* copy still reloads it: the active
/// project copy's own entry source is edited and the watcher reloads it in
/// place, with the new version's behavior observable in the same host.
///
/// This is the companion to
/// [`modifying_a_shadowed_layer_does_not_reload_the_active_copy`]: together
/// they pin the fingerprint guard from both sides — a shadowed-copy change is
/// a no-op, an active-copy change still reloads.
#[tokio::test]
async fn modifying_the_active_copy_still_reloads_it() {
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");
    let active_dir = project.path().join("plugins").join("shared");

    // A bundle named `shared` in both layers, so both have id `shared`; the
    // project copy is active.
    write_bundle(
        user.path(),
        "shared",
        "this.register('user-shadowed', { rust: 'user-mod' });",
    );
    write_bundle(
        project.path(),
        "shared",
        "this.register('active-v1', { rust: 'active-mod-1' });",
    );

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );
    tokio::time::timeout(
        TIMEOUT,
        host.expose_rust_module("user-mod", echo_module().await),
    )
    .await
    .expect("expose_rust_module should not hang")
    .expect("exposing the user rust module should succeed");
    tokio::time::timeout(
        TIMEOUT,
        host.expose_rust_module("active-mod-1", echo_module().await),
    )
    .await
    .expect("expose_rust_module should not hang")
    .expect("exposing the v1 rust module should succeed");

    tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("the initial discovery should succeed");
    assert_live(&host, "active-v1").await;

    // Expose the v2 module up front so the reloaded `load()` never races a
    // missing module.
    tokio::time::timeout(
        TIMEOUT,
        host.expose_rust_module("active-mod-2", echo_module().await),
    )
    .await
    .expect("expose_rust_module should not hang")
    .expect("exposing the v2 rust module should succeed");

    let _watcher = tokio::time::timeout(TIMEOUT, host.watch_plugins::<SwissarmyhammerConfig>())
        .await
        .expect("starting the watcher should not hang")
        .expect("the watcher should start");
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Edit the *active* project copy's own entry source: its fingerprint
    // changes, so the reconcile must reload it in place.
    write_entry(
        &active_dir,
        "this.register('active-v2', { rust: 'active-mod-2' });",
    );

    // The watcher fires, the host reloads: the v2 server comes live and the v1
    // server is disposed.
    wait_until_live(&host, "active-v2").await;
    assert_not_live(&host, "active-v1").await;
}
