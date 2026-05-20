//! End-to-end integration tests for the **hot-reload capability**, driven
//! through a real plugin and a real filesystem watcher.
//!
//! This is the capability-level companion to `hot_reload.rs`. Where
//! `hot_reload.rs` drives the reconcile mechanics — the fingerprint guard,
//! reload-policy gating, layer fallback — this file covers the spec-named
//! hot-reload edge cases that need cross-layer observation through a real
//! `PluginHost`: a plugin author saves a new version of their source, the
//! running host picks it up, and observable platform behavior matches the
//! contract `ideas/plugins/plugin-architecture.md` § Hot Reload promises.
//!
//! Tests follow the reference shape of `files_dispatch_e2e.rs`: a real V8
//! isolate, real registered servers, and an observable effect — *which
//! distinct server name is live*, *which marker the v2 isolate reports*,
//! *what error a mid-reload call sees* — that can only be true if the live
//! host genuinely tore down the old isolate and ran the rewritten source.
//!
//! # Spec edge-case coverage matrix
//!
//! The spec names five edge cases the hot-reload pipeline must handle. Each
//! row points at the test (here or in `hot_reload.rs`) that observes the
//! behavior end to end. "Missing" rows are filled in by tests in this file
//! whose names match the row's third column.
//!
//! | # | Spec edge case                                              | Test                                                                                            |
//! | - | ----------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
//! | 1 | In-flight calls reject with `PluginReloaded`                | `hot_reload_e2e::an_in_flight_call_rejects_with_plugin_reloaded_when_the_isolate_is_disposed`   |
//! | 2 | Registration set changes silently (different set)           | `hot_reload::rewriting_an_active_plugins_source_reloads_it_in_place`                            |
//! | 2 | Registration set changes silently (expansion: `{foo}` → `{foo, bar}`) | `hot_reload_e2e::reloading_to_a_version_that_registers_an_additional_server_picks_it_up` |
//! | 3 | Failed v2 load leaves the plugin unloaded                   | `hot_reload::a_failed_v2_load_leaves_the_plugin_unloaded_and_surfaces_the_error`                |
//! | 4 | Crashed plugin no auto-restart                              | `hot_reload_e2e::a_crashing_plugin_records_a_crashed_status_and_does_not_auto_restart`          |
//! | 5 | Plugin state in class fields is lost on reload              | `hot_reload_e2e::class_field_and_module_level_state_do_not_survive_a_reload`                    |
//!
//! Edges 1 and 4 required platform work — `PluginReloaded` was a dead
//! variant before this task and the crash channel did not exist. Both are
//! now plumbed through; the tests cited above exercise the new surfaces end
//! to end.
//!
//! # Isolation
//!
//! Each test owns its own [`tempfile::TempDir`] roots and a fresh
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
use swissarmyhammer_plugin::{
    CallerId, Error, InProcessServer, McpServer, PluginHost, ReloadStatus,
};

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

/// Overwrites the probe plugin's `index.ts` with a version whose `load()`
/// registers `server` against the host-exposed `rust` module `rust_module`.
///
/// This is both the initial write and the "rewrite the source" half of the
/// hot-reload test: writing a new `server` makes the watcher observe a genuine
/// content change and reload. The bundle's identity is its bundle directory
/// name and the reload from v1 to v2 is always an in-place reload.
///
/// Implemented as a thin wrapper around [`write_with_load_body`] so the
/// `class P extends Plugin { ... }` shell lives in exactly one place — only
/// the body the `load()` method runs varies between callers.
fn write_version(plugin_dir: &Path, server: &str, rust_module: &str) {
    write_with_load_body(
        plugin_dir,
        &format!("this.register('{server}', {{ rust: '{rust_module}' }});"),
    );
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
/// - the layer watcher is started, then the plugin's `index.ts` is rewritten
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

/// Writes a probe `index.ts` whose `load()` runs `body`.
///
/// Bundle identity is the directory name (the file lives at
/// `<plugin_dir>/index.ts`); the body is interpolated into the `load` method of
/// a class extending `Plugin`. Each rewrite changes the file's bytes so the
/// watcher's content-fingerprint guard observes a genuine modification.
fn write_with_load_body(plugin_dir: &Path, body: &str) {
    let entry = format!(
        "import {{ Plugin, makePluginThis }} from '@swissarmyhammer/plugin';\n\
         class P extends Plugin {{\n\
           async load(): Promise<void> {{\n\
             {body}\n\
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

/// Spec edge case 2 (expansion case): a v2 that registers an additional
/// server name picks the new server up without any host-side opt-in.
///
/// The base hot-reload test (`hot_reload::rewriting_an_active_plugins_source_reloads_it_in_place`)
/// covers the "different set" case — v1's `{behavior-a}` replaced by v2's
/// `{behavior-b}`. This test covers the missing "more" case: v1's `{foo}`
/// expanded to v2's `{foo, bar}` after the source is rewritten. No manifest,
/// no `provides`, no install-time declaration — the platform observes the
/// expansion strictly at v2's `load()` time.
///
/// # What a passing run proves
///
/// 1. After the initial load, server `foo` answers and `bar` does not exist.
/// 2. The source is rewritten so `load()` registers both `foo` (against a
///    fresh module id) *and* `bar`.
/// 3. The watcher fires, the host reloads, and both `foo` and `bar` answer
///    real `echo` calls in the same `PluginHost` — proving the registration
///    set silently expanded.
#[tokio::test]
async fn reloading_to_a_version_that_registers_an_additional_server_picks_it_up() {
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");
    let plugin_dir = project.path().join("plugins").join("probe-expansion");
    std::fs::create_dir_all(&plugin_dir).expect("probe plugin directory should be created");

    // v1's load registers `foo` against `mod-foo-v1`.
    write_with_load_body(&plugin_dir, "this.register('foo', { rust: 'mod-foo-v1' });");

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );

    // Pre-expose every `{ rust }` module either version activates. Each
    // activation is single-use — v2 needs a fresh `foo` module because v1
    // consumed `mod-foo-v1`.
    for id in ["mod-foo-v1", "mod-foo-v2", "mod-bar"] {
        tokio::time::timeout(TIMEOUT, host.expose_rust_module(id, echo_module().await))
            .await
            .expect("expose_rust_module should not hang")
            .unwrap_or_else(|error| panic!("exposing '{id}' should succeed: {error:?}"));
    }

    tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("the initial discovery should succeed");

    // Initial state: only `foo` is live.
    assert_live(&host, "foo", "v1 running").await;
    assert_not_live(&host, "bar").await;

    let _watcher = tokio::time::timeout(TIMEOUT, host.watch_plugins::<SwissarmyhammerConfig>())
        .await
        .expect("starting the watcher should not hang")
        .expect("the watcher should start");
    tokio::time::sleep(Duration::from_millis(300)).await;

    // v2 expands the set: `foo` re-registered against the fresh module, plus
    // a brand-new `bar`. Concretely, an author saving their plugin source.
    write_with_load_body(
        &plugin_dir,
        "this.register('foo', { rust: 'mod-foo-v2' });\n\
         this.register('bar', { rust: 'mod-bar' });",
    );

    // After the watcher-driven reload, both `foo` and `bar` answer. `bar`
    // becoming addressable in the same host proves the registration set
    // expanded with no host-side opt-in.
    wait_until_live(&host, "bar").await;
    assert_live(&host, "foo", "v2 foo").await;
    assert_live(&host, "bar", "v2 bar").await;
}

/// Spec edge case 5: plugin state in class fields (and module-level state) is
/// lost on reload.
///
/// "Lost" is the natural consequence of the platform tearing down v1's V8
/// isolate before evaluating v2's source in a fresh isolate. This test makes
/// that contract observable end to end: v2's `load()` reads what would be v1's
/// state and uses what it sees to decide which server name to register, so the
/// live registry tells the test whether the contract held.
///
/// # The two versions
///
/// - v1's `load()` writes `'v1-touched'` onto `this` as a real own property
///   (using `Object.defineProperty` so the assignment lands on the underlying
///   `P` instance rather than going through the SDK Proxy's dispatcher trap)
///   and into `globalThis.LEAK`, then registers `v1-server`.
/// - v2's `load()` checks both sources with `Object.hasOwn(this, 'field')`
///   and `'LEAK' in globalThis` — neither read can be intercepted by the SDK
///   Proxy, so an `undefined`-vs-dispatcher false positive is impossible. The
///   result names which (if any) source leaked.
///
/// A passing run sees `state-clean` live in the same `PluginHost` after the
/// watcher fires — proof v2 ran in a fresh isolate with no inherited state.
#[tokio::test]
async fn class_field_and_module_level_state_do_not_survive_a_reload() {
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");
    let plugin_dir = project.path().join("plugins").join("probe-state");
    std::fs::create_dir_all(&plugin_dir).expect("probe plugin directory should be created");

    // v1's load mutates per-instance state AND a module-level global, then
    // registers a marker server so the test can observe v1 ran. The instance
    // write uses `Object.defineProperty` rather than `this.field = ...` so
    // the property lands as an own property of the underlying `P` — assigning
    // through the SDK Proxy is the same Reflect.set path but using
    // defineProperty makes the intent explicit.
    write_with_load_body(
        &plugin_dir,
        "Object.defineProperty(this, 'field', { value: 'v1-touched', \
         writable: true, enumerable: true, configurable: true });\n\
         (globalThis as unknown as { LEAK?: string }).LEAK = 'v1-touched';\n\
         this.register('v1-server', { rust: 'mod-v1' });",
    );

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );

    for id in [
        "mod-v1",
        "mod-v2-clean",
        "mod-v2-both",
        "mod-v2-field",
        "mod-v2-global",
    ] {
        tokio::time::timeout(TIMEOUT, host.expose_rust_module(id, echo_module().await))
            .await
            .expect("expose_rust_module should not hang")
            .unwrap_or_else(|error| panic!("exposing '{id}' should succeed: {error:?}"));
    }

    tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("the initial discovery should succeed");

    assert_live(&host, "v1-server", "v1 ran").await;

    let _watcher = tokio::time::timeout(TIMEOUT, host.watch_plugins::<SwissarmyhammerConfig>())
        .await
        .expect("starting the watcher should not hang")
        .expect("the watcher should start");
    tokio::time::sleep(Duration::from_millis(300)).await;

    // v2 reads what v1 wrote. With proper isolate teardown both reads return
    // `undefined`, so v2 registers `state-clean`. Any survivor flips the
    // registered name to `state-leaked` — observable in the same registry —
    // so a regression points at the exact reload contract that broke.
    // v2 probes both sources WITHOUT going through the SDK Proxy's dispatcher
    // fallback. `Object.hasOwn(this, 'field')` triggers the Proxy's default
    // `has` trap, which forwards to the underlying `P` instance — so a fresh
    // `new P()` reports no own `field` regardless of how the Proxy treats
    // gets. `'LEAK' in globalThis` is a real `has` check on the isolate's
    // global object. Both flip to `true` only if real state survived.
    write_with_load_body(
        &plugin_dir,
        "const fieldLeaked = Object.hasOwn(this, 'field');\n\
         const globalLeaked = 'LEAK' in globalThis;\n\
         if (!fieldLeaked && !globalLeaked) {\n\
           this.register('state-clean', { rust: 'mod-v2-clean' });\n\
         } else if (fieldLeaked && globalLeaked) {\n\
           this.register('field-and-global-leaked', { rust: 'mod-v2-both' });\n\
         } else if (fieldLeaked) {\n\
           this.register('field-leaked', { rust: 'mod-v2-field' });\n\
         } else {\n\
           this.register('global-leaked', { rust: 'mod-v2-global' });\n\
         }",
    );

    // Wait for whichever marker v2 picks to come live — `state-clean` means
    // the spec held, the other three name which source survived. Polling
    // returns as soon as the watcher-driven reload completes; the assertions
    // then pin down the exact outcome.
    let observed = wait_until_any_live(
        &host,
        &[
            "state-clean",
            "field-and-global-leaked",
            "field-leaked",
            "global-leaked",
        ],
    )
    .await;
    // `state-clean` means the spec held; any other marker names exactly which
    // source survived the reload.
    assert_eq!(
        observed, "state-clean",
        "v2 should observe undefined for both `this.field` and `globalThis.LEAK` \
         and register `state-clean`; instead the surviving state caused it to \
         register `{observed}`"
    );
    assert_live(&host, "state-clean", "fresh isolate").await;
    assert_not_live(&host, "v1-server").await;
}

/// Spec edge case 1: in-flight calls reject with `PluginReloaded`.
///
/// During the hot-reload window — between v1's unregister and v2's `register`
/// of the same name — a call to that name must reject with
/// [`Error::PluginReloaded`], the variant the spec promises so callers know
/// to retry once v2 settles. Before this task that variant existed but was
/// never emitted; the registry now stages a `Reloading` marker for every
/// name v1 holds and the host's `call` translates it to `PluginReloaded`.
///
/// # The test's deterministic reload window
///
/// Driving a *live* race against the watcher is fragile (the debounce window
/// is timing-sensitive). Instead the test exercises the same code path the
/// watcher would — `mark_reloading` → unregister → re-register — directly
/// through the registry. Concretely:
///
/// 1. v1 is loaded and `mid-reload` is live, callable through a real `echo`.
/// 2. The registry is asked to `mark_reloading("mid-reload")`. v1's server is
///    still in the registry, but `resolve` now returns `Reloading`.
/// 3. A `host.call` against `mid-reload` returns `Error::PluginReloaded`,
///    exactly the error the spec promises an in-flight caller observes.
/// 4. `clear_reloading` ends the window; the call returns to live behavior.
///
/// This isolates the contract (`Reloading` resolves to `PluginReloaded`) from
/// the watcher debounce, while still using the real `PluginHost`, real
/// registry, and real registered server. The full v1→v2 swap is already
/// covered by `rewriting_a_running_plugins_source_hot_reloads_it_in_the_same_host`.
#[tokio::test]
async fn an_in_flight_call_rejects_with_plugin_reloaded_when_the_isolate_is_disposed() {
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");
    let plugin_dir = project.path().join("plugins").join("probe-mid-reload");
    std::fs::create_dir_all(&plugin_dir).expect("probe plugin directory should be created");

    write_version(&plugin_dir, "mid-reload", "mod-mid");

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );

    tokio::time::timeout(
        TIMEOUT,
        host.expose_rust_module("mod-mid", echo_module().await),
    )
    .await
    .expect("expose_rust_module should not hang")
    .expect("exposing 'mod-mid' should succeed");

    tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("the initial discovery should succeed");

    // Sanity check: the name is live before the reload window opens.
    assert_live(&host, "mid-reload", "v1 live").await;

    // Open the reload window directly. Equivalent to the first step of
    // `reload_active` (after `server_names_held_by` captures the name) and
    // before `unload_active` runs — except deterministic, no watcher debounce.
    host.mark_reloading_for_test("mid-reload").await;

    // The contract: a `host.call` during the window rejects with
    // `PluginReloaded`, not `UnknownServer` or `ServerUnavailable`.
    let mid_window = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            "mid-reload",
            "echo",
            json!({ "message": "should not see this" }),
        ),
    )
    .await
    .expect("a dispatch call should not hang")
    .expect_err("a call during the reload window must fail");
    assert!(
        matches!(mid_window, Error::PluginReloaded),
        "an in-flight call during the reload window must reject with \
         Error::PluginReloaded; got {mid_window:?}"
    );

    // Closing the reload window restores normal resolution against the
    // still-registered server — the test never disposed it.
    host.clear_reloading_for_test("mid-reload").await;
    assert_live(&host, "mid-reload", "after window closes").await;
}

/// Spec edge case 4: a crashed plugin records `ReloadStatus::Crashed` and
/// does **not** auto-restart.
///
/// A post-load isolate crash is distinct from a load-time failure (edge case
/// 3, covered by `hot_reload::a_failed_v2_load_leaves_the_plugin_unloaded_and_surfaces_the_error`):
/// the v2 load completed cleanly, the plugin served at least one call, and
/// the isolate died later. The platform's contract is bookkeeping: the
/// plugin's registrations are disposed, the runtime is dropped, and a
/// `ReloadStatus::Crashed { error }` is recorded against the disk id so a
/// host UI / settings layer can surface it. The watcher only fires on file
/// changes, so "no auto-restart" is structurally true — the test confirms
/// the platform itself takes no recovery action by sampling the status after
/// a deliberate quiet period.
///
/// # Driving a crash deterministically
///
/// A "real" V8-internal crash (OOM, near-heap-limit) is non-deterministic
/// and slow to provoke; the test instead exercises the bookkeeping path
/// directly through [`PluginHost::record_crashed`], which is the same entry
/// point a host's crash-detection hook would call. This isolates the
/// contract (status + dispose + no-restart) from the V8-internal mechanism
/// while still using the real `PluginHost`, real registered server, and
/// real ledger.
#[tokio::test]
async fn a_crashing_plugin_records_a_crashed_status_and_does_not_auto_restart() {
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");
    let plugin_dir = project.path().join("plugins").join("probe-crash");
    std::fs::create_dir_all(&plugin_dir).expect("probe plugin directory should be created");

    write_version(&plugin_dir, "pre-crash", "mod-crash");

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );

    tokio::time::timeout(
        TIMEOUT,
        host.expose_rust_module("mod-crash", echo_module().await),
    )
    .await
    .expect("expose_rust_module should not hang")
    .expect("exposing 'mod-crash' should succeed");

    let plugin_ids = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("the initial discovery should succeed");
    let plugin_id = plugin_ids
        .into_iter()
        .next()
        .expect("discovery should load the crash-probe plugin");

    // Pre-crash baseline: the plugin is live and `ReloadStatus::Healthy`.
    assert_live(&host, "pre-crash", "pre-crash live").await;
    assert!(
        matches!(
            host.reload_status("probe-crash").await,
            Some(ReloadStatus::Healthy)
        ),
        "a freshly-loaded plugin must report Healthy before the crash",
    );

    // The watcher is started so the test can later assert that the platform
    // does NOT auto-restart even while the watcher is live. If the platform
    // ever decided to retry a crashed plugin on its own — through the
    // watcher, a timer, or any other channel — this watcher would expose it.
    let _watcher = tokio::time::timeout(TIMEOUT, host.watch_plugins::<SwissarmyhammerConfig>())
        .await
        .expect("starting the watcher should not hang")
        .expect("the watcher should start");

    // Drive the crash through the platform's crash-reporting entry point. A
    // real host's crash-detection hook (a worker thread that observed
    // `Error::RuntimeStopped`, a near-heap-limit callback, …) calls this
    // same function; the test bypasses the mechanism so it can assert the
    // bookkeeping contract.
    tokio::time::timeout(
        TIMEOUT,
        host.record_crashed(&plugin_id, "isolate died: simulated OOM"),
    )
    .await
    .expect("record_crashed should not hang")
    .expect("recording the crash should succeed");

    // Contract part 1: registrations are gone. The server name is tombstoned
    // (it was once live), so a call surfaces as `ServerUnavailable` rather
    // than `UnknownServer`.
    let post_crash = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            "pre-crash",
            "echo",
            json!({ "message": "after crash" }),
        ),
    )
    .await
    .expect("a dispatch call should not hang")
    .expect_err("a crashed plugin's server must not answer");
    assert!(
        matches!(post_crash, Error::ServerUnavailable),
        "post-crash call must reject with ServerUnavailable, got {post_crash:?}"
    );

    // Contract part 2: the status is Crashed and carries the surfaced error.
    let status = host
        .reload_status("probe-crash")
        .await
        .expect("a crashed plugin must have a reload status recorded");
    match status {
        ReloadStatus::Crashed { error } => {
            assert!(
                error.contains("simulated OOM"),
                "Crashed should carry the supplied error text, got {error:?}"
            );
        }
        other => panic!("expected ReloadStatus::Crashed, got {other:?}"),
    }

    // Contract part 3: no auto-restart. After a meaningful quiet period the
    // platform must have taken no recovery action — the server stays gone,
    // the status stays Crashed. A regression that introduced any retry path
    // (a timer, the watcher firing on something other than a file change,
    // a load_active_copy on Crashed status) would flip one of these.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let still_crashed = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            "pre-crash",
            "echo",
            json!({ "message": "still down" }),
        ),
    )
    .await
    .expect("a dispatch call should not hang")
    .expect_err("a crashed plugin's server must still not answer after a quiet period");
    assert!(
        matches!(still_crashed, Error::ServerUnavailable),
        "post-quiet-period call must still reject with ServerUnavailable, got {still_crashed:?}"
    );
    assert!(
        matches!(
            host.reload_status("probe-crash").await,
            Some(ReloadStatus::Crashed { .. })
        ),
        "status must still be Crashed after the quiet period"
    );
}

/// Polls until any name in `names` answers an `echo` call, or fails after
/// [`SETTLE`]. Returns the first name that came live so the caller can pin
/// down which of several mutually-exclusive branches a v2 picked.
async fn wait_until_any_live(host: &PluginHost, names: &[&str]) -> String {
    let deadline = Instant::now() + SETTLE;
    loop {
        for name in names {
            let result = tokio::time::timeout(
                TIMEOUT,
                host.call(
                    CallerId::HostInternal,
                    name,
                    "echo",
                    json!({ "message": "probe" }),
                ),
            )
            .await
            .expect("a dispatch call should not hang");
            if result.is_ok() {
                return (*name).to_string();
            }
        }
        assert!(
            Instant::now() < deadline,
            "none of {names:?} became live within {SETTLE:?}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
