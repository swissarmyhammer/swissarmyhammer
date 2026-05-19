//! Integration tests for plugin discovery, manifest parsing, layer stacking,
//! and `provides` validation.
//!
//! These tests drive [`PluginHost`] end to end: real plugin bundles — a real
//! `plugin.json` manifest plus a real entry `.ts` file — are written into
//! temporary layer roots, discovered through `swissarmyhammer-directory`'s
//! stacked `plugins/` subdirectory, and loaded into real V8 isolates. The
//! probe plugins register *real* in-process `rmcp` servers, so an assertion
//! observes a genuine round-trip rather than a mock.
//!
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
use serde_json::{json, Value};
use swissarmyhammer_directory::SwissarmyhammerConfig;
use swissarmyhammer_plugin::{CallerId, InProcessServer, McpServer, PluginHost};

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

/// Writes a plugin bundle — `plugin.json` plus an `entry.ts` — into
/// `layer_root/plugins/<dir_name>/`.
///
/// The on-disk directory name (`dir_name`) is deliberately separate from the
/// manifest `id` so tests can prove that identity follows the manifest, not
/// the directory name. The entry imports the SDK, declares a `Plugin` subclass
/// whose `load` runs `body`, and exports a `load` lifecycle function.
fn write_plugin_in_layer(
    layer_root: &Path,
    dir_name: &str,
    manifest_id: &str,
    provides: &[&str],
    entry: &str,
    body: &str,
) {
    let plugin_dir = layer_root.join("plugins").join(dir_name);
    std::fs::create_dir_all(&plugin_dir).expect("plugin directory should be created");

    let provides_json = serde_json::to_string(provides).expect("provides serializes");
    let manifest = format!(
        "{{\n  \"id\": \"{manifest_id}\",\n  \"name\": \"{manifest_id} plugin\",\n  \
         \"version\": \"1.0.0\",\n  \"entry\": \"{entry}\",\n  \"provides\": {provides_json}\n}}\n"
    );
    std::fs::write(plugin_dir.join("plugin.json"), manifest)
        .expect("plugin.json should be written");

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
    // `entry` may name a nested path (e.g. `src/plugin.ts`); create parents.
    let entry_path = plugin_dir.join(entry);
    if let Some(parent) = entry_path.parent() {
        std::fs::create_dir_all(parent).expect("entry parent dir should be created");
    }
    std::fs::write(&entry_path, source).expect("entry file should be written");
}

/// Writes a manifest-less, TypeScript-only plugin bundle — just an `index.ts`
/// entry, no `plugin.json` — into `layer_root/plugins/<dir_name>/`.
///
/// A manifest-less plugin's identity is its bundle directory name, so
/// `dir_name` is both the on-disk directory and the plugin id. The entry
/// imports the SDK, declares a `Plugin` subclass whose `load` runs `body`, and
/// exports a `load` lifecycle function — the same shape a manifest bundle's
/// entry uses, only the file is `index.ts` and there is no manifest.
fn write_manifestless_plugin_in_layer(layer_root: &Path, dir_name: &str, body: &str) {
    let plugin_dir = layer_root.join("plugins").join(dir_name);
    std::fs::create_dir_all(&plugin_dir).expect("plugin directory should be created");

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

/// A manifest-less, TypeScript-only bundle — an `index.ts` entry and no
/// `plugin.json` — staged into the project layer is discovered by
/// `discover_and_load_all` and loaded: its `load()` runs, observed by a real
/// `tools/call` round-trip into the server it registered.
///
/// A manifest-less plugin declares no `provides`, so the host's `provides` gate
/// is skipped for it — a `register` inside its `load()` is not checked against
/// any manifest list. Its identity is its bundle directory name.
#[tokio::test]
async fn discover_and_load_all_loads_a_manifestless_index_ts_plugin() {
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");
    // No `plugin.json`: identity is the bundle directory name, `ts-probe`.
    write_manifestless_plugin_in_layer(
        project.path(),
        "ts-probe",
        "this.register('ts-probe-server', { rust: 'ts-probe-mod' });",
    );

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );
    let probe_mod: Arc<dyn McpServer> = Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed"),
    );
    tokio::time::timeout(TIMEOUT, host.expose_rust_module("ts-probe-mod", probe_mod))
        .await
        .expect("expose_rust_module should not hang")
        .expect("exposing a rust module should succeed");

    let loaded = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("discovery of a manifest-less bundle should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "the manifest-less index.ts bundle must be discovered and loaded"
    );

    // The manifest-less plugin's `load()` ran its `register` with no `provides`
    // gate; the server it provided is live and serves a real rmcp tool call.
    let result = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            "ts-probe-server",
            "echo",
            json!({ "message": "manifest-less" }),
        ),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect("a call into the manifest-less plugin's server should succeed");
    assert!(
        rendered(&result).contains("manifest-less"),
        "the manifest-less plugin's load() must have registered a working server, got {}",
        rendered(&result)
    );
}

/// A probe plugin discovered in the project layer is loaded by
/// `discover_and_load_all`, and its `load()` runs — observed by a real
/// `tools/call` round-trip into the server it registered.
#[tokio::test]
async fn discover_and_load_all_loads_a_discovered_plugin() {
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");
    write_plugin_in_layer(
        project.path(),
        // The disk directory name differs from the manifest id on purpose.
        "probe-dir",
        "probe",
        &["probe-server"],
        "entry.ts",
        "this.register('probe-server', { rust: 'probe-mod' });",
    );

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );
    let probe_mod: Arc<dyn McpServer> = Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed"),
    );
    tokio::time::timeout(TIMEOUT, host.expose_rust_module("probe-mod", probe_mod))
        .await
        .expect("expose_rust_module should not hang")
        .expect("exposing a rust module should succeed");

    let loaded = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("discovery should succeed");
    assert_eq!(loaded.len(), 1, "exactly one plugin should be discovered");

    // The plugin's `load()` ran its `register`, so the server it provided is
    // live and serves a real rmcp tool call.
    let result = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            "probe-server",
            "echo",
            json!({ "message": "discovered" }),
        ),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect("a call into the discovered plugin's server should succeed");
    assert!(
        rendered(&result).contains("discovered"),
        "the discovered plugin's load() must have registered a working server, got {}",
        rendered(&result)
    );
}

/// When the same plugin `id` exists in two layers, the higher-precedence
/// (project) copy is the one that loads — observed by which copy's distinct
/// behavior runs.
#[tokio::test]
async fn layering_picks_the_higher_precedence_copy() {
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");

    // Both layers carry id `shared`; the user copy registers `from-user`, the
    // project copy registers `from-project`. The active copy decides which
    // server name becomes live.
    write_plugin_in_layer(
        user.path(),
        "shared",
        "shared",
        &["from-user"],
        "entry.ts",
        "this.register('from-user', { rust: 'shared-mod' });",
    );
    write_plugin_in_layer(
        project.path(),
        "shared",
        "shared",
        &["from-project"],
        "entry.ts",
        "this.register('from-project', { rust: 'shared-mod' });",
    );

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );
    let shared_mod: Arc<dyn McpServer> = Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed"),
    );
    tokio::time::timeout(TIMEOUT, host.expose_rust_module("shared-mod", shared_mod))
        .await
        .expect("expose_rust_module should not hang")
        .expect("exposing a rust module should succeed");

    let loaded = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("discovery should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "a shadowed id resolves to one active copy, not two"
    );

    // The project copy won: `from-project` is live, `from-user` was never
    // registered because the user copy did not load.
    let project_result = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            "from-project",
            "echo",
            json!({ "message": "project wins" }),
        ),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect("the project-layer copy should be the active one");
    assert!(
        rendered(&project_result).contains("project wins"),
        "the project-layer copy must be the one that loaded, got {}",
        rendered(&project_result)
    );

    let user_err = tokio::time::timeout(
        TIMEOUT,
        host.call(CallerId::HostInternal, "from-user", "echo", json!({})),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect_err("the shadowed user-layer copy must not have loaded");
    assert!(
        matches!(user_err, swissarmyhammer_plugin::Error::UnknownServer),
        "the shadowed copy's server must never become live, got {user_err:?}"
    );
}

/// A plugin staged into the host's read-only builtin layer root is discovered
/// and loaded by `discover_and_load_all` — the builtin layer is a first-class
/// discovery layer, not a one-by-one `host.load()` workaround.
///
/// The host is built with [`PluginHost::for_tests_with_builtin`], so its
/// lowest-precedence discovery layer is a builtin root. A bundle dropped into
/// `<builtin_root>/plugins/` loads and its `load()` runs — observed by a real
/// `tools/call` round-trip into the server it registered.
#[tokio::test]
async fn discover_and_load_all_loads_a_builtin_layer_plugin() {
    let builtin = tempfile::TempDir::new().expect("builtin root temp dir");
    let user = tempfile::TempDir::new().expect("user root temp dir");

    // The bundle lives in the *builtin* layer, nowhere else.
    write_plugin_in_layer(
        builtin.path(),
        "builtin-dir",
        "builtin-svc",
        &["builtin-server"],
        "entry.ts",
        "this.register('builtin-server', { rust: 'builtin-mod' });",
    );

    let host = PluginHost::for_tests_with_builtin(
        builtin.path().to_path_buf(),
        user.path().to_path_buf(),
        None,
    );
    let builtin_mod: Arc<dyn McpServer> = Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed"),
    );
    tokio::time::timeout(TIMEOUT, host.expose_rust_module("builtin-mod", builtin_mod))
        .await
        .expect("expose_rust_module should not hang")
        .expect("exposing a rust module should succeed");

    let loaded = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("discovery should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "the builtin layer must contribute its one bundle to discovery"
    );

    let result = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            "builtin-server",
            "echo",
            json!({ "message": "from the builtin layer" }),
        ),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect("a call into the builtin-layer plugin's server should succeed");
    assert!(
        rendered(&result).contains("from the builtin layer"),
        "the builtin-layer plugin's load() must have registered a working server, got {}",
        rendered(&result)
    );
}

/// The committed `test/builtin/plugins/` fixture tree is discovered when a host
/// points its builtin layer root at it.
///
/// This is the fixture-backed companion to
/// [`discover_and_load_all_loads_a_builtin_layer_plugin`]: rather than staging
/// a temp bundle, it points the builtin layer at the real, committed
/// `test/builtin/` tree at the workspace root. The fixture's `builtin-probe`
/// bundle is self-contained — it registers no server — so the proof it loaded
/// is its discovery-recorded `ReloadStatus::Healthy`.
#[tokio::test]
async fn the_committed_test_builtin_fixture_tree_is_discovered() {
    // `CARGO_MANIFEST_DIR` is `<workspace>/crates/swissarmyhammer-plugin`; the
    // committed fixture tree lives at `<workspace>/test/builtin`.
    let builtin_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("test")
        .join("builtin");
    assert!(
        builtin_root.join("plugins").join("builtin-probe").is_dir(),
        "the committed test/builtin/plugins/ fixture tree must exist at {}",
        builtin_root.display()
    );

    let user = tempfile::TempDir::new().expect("user root temp dir");
    let host = PluginHost::for_tests_with_builtin(builtin_root, user.path().to_path_buf(), None);

    let loaded = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("discovery of the committed builtin fixture should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "the committed fixture tree contributes its one builtin bundle"
    );
    assert_eq!(
        host.reload_status("builtin-probe").await,
        Some(swissarmyhammer_plugin::ReloadStatus::Healthy),
        "the committed builtin-probe fixture must be discovered and loaded"
    );
}

/// When the same plugin `id` lives in the builtin layer and the user layer,
/// the higher-precedence user copy is the one that loads — the read-only
/// builtin layer is the lowest-precedence floor every writable layer stacks on
/// top of.
#[tokio::test]
async fn the_user_layer_shadows_the_builtin_layer() {
    let builtin = tempfile::TempDir::new().expect("builtin root temp dir");
    let user = tempfile::TempDir::new().expect("user root temp dir");

    // Both layers carry id `shared`; the builtin copy registers `from-builtin`,
    // the user copy registers `from-user`. Which server name becomes live is an
    // unambiguous read on which copy is active.
    write_plugin_in_layer(
        builtin.path(),
        "shared",
        "shared",
        &["from-builtin"],
        "entry.ts",
        "this.register('from-builtin', { rust: 'shared-mod' });",
    );
    write_plugin_in_layer(
        user.path(),
        "shared",
        "shared",
        &["from-user"],
        "entry.ts",
        "this.register('from-user', { rust: 'shared-mod' });",
    );

    let host = PluginHost::for_tests_with_builtin(
        builtin.path().to_path_buf(),
        user.path().to_path_buf(),
        None,
    );
    let shared_mod: Arc<dyn McpServer> = Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed"),
    );
    tokio::time::timeout(TIMEOUT, host.expose_rust_module("shared-mod", shared_mod))
        .await
        .expect("expose_rust_module should not hang")
        .expect("exposing a rust module should succeed");

    let loaded = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("discovery should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "a shadowed id resolves to one active copy, not two"
    );

    // The user copy won: `from-user` is live, the builtin copy's `from-builtin`
    // was never registered because the builtin copy did not load.
    let user_result = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            "from-user",
            "echo",
            json!({ "message": "user shadows builtin" }),
        ),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect("the user-layer copy should be the active one");
    assert!(
        rendered(&user_result).contains("user shadows builtin"),
        "the user-layer copy must shadow the builtin copy, got {}",
        rendered(&user_result)
    );

    let builtin_err = tokio::time::timeout(
        TIMEOUT,
        host.call(CallerId::HostInternal, "from-builtin", "echo", json!({})),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect_err("the shadowed builtin-layer copy must not have loaded");
    assert!(
        matches!(builtin_err, swissarmyhammer_plugin::Error::UnknownServer),
        "the shadowed builtin copy's server must never become live, got {builtin_err:?}"
    );
}

/// A plugin whose `load()` registers a server name absent from its manifest's
/// `provides` fails to load with a clear error naming the offending name.
#[tokio::test]
async fn register_of_a_name_absent_from_provides_is_rejected() {
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");
    // The manifest promises `declared` but `load()` registers `sneaky`.
    write_plugin_in_layer(
        project.path(),
        "liar",
        "liar",
        &["declared"],
        "entry.ts",
        "this.register('sneaky', { rust: 'liar-mod' });",
    );

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );
    let liar_mod: Arc<dyn McpServer> = Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed"),
    );
    tokio::time::timeout(TIMEOUT, host.expose_rust_module("liar-mod", liar_mod))
        .await
        .expect("expose_rust_module should not hang")
        .expect("exposing a rust module should succeed");

    let err = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect_err("a plugin registering an undeclared name must fail to load");
    let message = err.to_string();
    assert!(
        message.contains("sneaky"),
        "the error must name the undeclared server, got: {message}"
    );
    assert!(
        message.contains("provides"),
        "the error must mention the manifest's provides list, got: {message}"
    );
}

/// A discovery scan is atomic: when one discovered plugin fails to load, every
/// plugin the scan loaded earlier is rolled back, so a failed scan leaves the
/// host with no plugin from that scan live — and no stale hot-reload state.
///
/// `discover_and_load_all` records a `ReloadStatus::Healthy` for every plugin
/// it loads. The rollback after a mid-scan failure must drop that status too,
/// not just the loaded isolate: otherwise `reload_status` would report a
/// plugin as `Healthy` that is not loaded at all. The test asserts the
/// rolled-back plugin's `reload_status` is `None`.
#[tokio::test]
async fn a_failed_discovery_scan_rolls_back_already_loaded_plugins() {
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");

    // `aaa-good` loads cleanly and registers a working server. Discovery sorts
    // by id, so it is resolved before `zzz-bad` and is loaded first.
    write_plugin_in_layer(
        project.path(),
        "good-dir",
        "aaa-good",
        &["good-server"],
        "entry.ts",
        "this.register('good-server', { rust: 'good-mod' });",
    );
    // `zzz-bad`'s manifest promises `declared` but its `load()` registers
    // `undeclared`, so its load fails mid-scan.
    write_plugin_in_layer(
        project.path(),
        "bad-dir",
        "zzz-bad",
        &["declared"],
        "entry.ts",
        "this.register('undeclared', { rust: 'bad-mod' });",
    );

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );
    let good_mod: Arc<dyn McpServer> = Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed"),
    );
    tokio::time::timeout(TIMEOUT, host.expose_rust_module("good-mod", good_mod))
        .await
        .expect("expose_rust_module should not hang")
        .expect("exposing a rust module should succeed");

    let err = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect_err("a scan with a plugin that fails to load must return Err");
    assert!(
        err.to_string().contains("undeclared"),
        "the surfaced error must be the failing plugin's load error, got: {err}"
    );

    // The first plugin loaded fine, but the atomic scan rolled it back: the
    // server it had registered is no longer live, so a call to it fails. The
    // rollback unregisters the server, which leaves a registry tombstone, so
    // the call fails as `ServerUnavailable` (registered, then disposed) — what
    // matters is that it is no longer serving, not which non-live status it is.
    let good_err = tokio::time::timeout(
        TIMEOUT,
        host.call(CallerId::HostInternal, "good-server", "echo", json!({})),
    )
    .await
    .expect("the dispatch call should not hang")
    .expect_err("the rolled-back plugin's server must not be left live");
    assert!(
        matches!(
            good_err,
            swissarmyhammer_plugin::Error::UnknownServer
                | swissarmyhammer_plugin::Error::ServerUnavailable
        ),
        "a failed scan must leave no plugin from it serving, got: {good_err:?}"
    );

    // The host's own loaded-plugin count is also clean: nothing from the
    // failed scan remains tracked.
    let debug = format!("{host:?}");
    assert!(
        debug.contains("loaded_plugins: 0"),
        "a failed scan must leave the host with no loaded plugins, got: {debug}"
    );

    // Hot-reload state is clean too: the rolled-back `aaa-good` plugin must
    // not be left reporting `Healthy` from `reload_status`. `record_active`
    // had inserted a `Healthy` status when the plugin first loaded; the
    // rollback must have removed it, so the status is now `None`.
    let good_status = host.reload_status("aaa-good").await;
    assert!(
        good_status.is_none(),
        "a rolled-back plugin must leave no reload status, got: {good_status:?}"
    );
}

/// A manifest whose plugin-authored `entry` traverses out of the bundle with
/// `..` is rejected with a clear manifest error, and no isolate is spent on it.
#[tokio::test]
async fn a_manifest_entry_escaping_the_bundle_is_rejected() {
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");

    // Lay out a normal bundle, then overwrite its manifest with one whose
    // `entry` points at a sibling file outside the bundle directory.
    write_plugin_in_layer(
        project.path(),
        "escapee-dir",
        "escapee",
        &["escapee-server"],
        "entry.ts",
        "this.register('escapee-server', { rust: 'escapee-mod' });",
    );
    let plugins_dir = project.path().join("plugins");
    // A file one level above the bundle directory — the escape target.
    std::fs::write(plugins_dir.join("escape.ts"), "// outside the bundle")
        .expect("escape target should be written");
    let escaping_manifest = "{\n  \"id\": \"escapee\",\n  \"name\": \"escapee plugin\",\n  \
         \"version\": \"1.0.0\",\n  \"entry\": \"../escape.ts\",\n  \
         \"provides\": [\"escapee-server\"]\n}\n";
    std::fs::write(
        plugins_dir.join("escapee-dir").join("plugin.json"),
        escaping_manifest,
    )
    .expect("escaping plugin.json should be written");

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );

    let err = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect_err("a manifest entry escaping the bundle must be rejected");
    assert!(
        matches!(err, swissarmyhammer_plugin::Error::Manifest(_)),
        "an escaping entry must surface as Error::Manifest, got: {err:?}"
    );
    let message = err.to_string();
    assert!(
        message.contains("escapes the plugin bundle"),
        "the error must explain the escape, got: {message}"
    );
    assert!(
        message.contains("escapee"),
        "the error must name the offending plugin, got: {message}"
    );

    // Nothing loaded: the rejection happened before any isolate was created.
    let debug = format!("{host:?}");
    assert!(
        debug.contains("loaded_plugins: 0"),
        "a rejected manifest entry must leave the host with no loaded plugins, got: {debug}"
    );
}

/// A `provides` entry that collides with a reserved host server name is
/// rejected at discovery time, before the plugin's isolate is even created.
#[tokio::test]
async fn provides_colliding_with_a_reserved_host_name_is_rejected() {
    let user = tempfile::TempDir::new().expect("user root temp dir");
    let project = tempfile::TempDir::new().expect("project root temp dir");
    // `host-reserved` is exposed by the host below, so a plugin may not claim
    // it in `provides`.
    write_plugin_in_layer(
        project.path(),
        "greedy",
        "greedy",
        &["host-reserved"],
        "entry.ts",
        "this.register('host-reserved', { rust: 'greedy-mod' });",
    );

    let host = PluginHost::for_tests(
        user.path().to_path_buf(),
        Some(project.path().to_path_buf()),
    );
    // The host reserves `host-reserved` by exposing a module under that name.
    let reserved: Arc<dyn McpServer> = Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping a real rmcp handler should succeed"),
    );
    tokio::time::timeout(TIMEOUT, host.expose_rust_module("host-reserved", reserved))
        .await
        .expect("expose_rust_module should not hang")
        .expect("exposing a rust module should succeed");

    let err = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect_err("a provides name colliding with a reserved host name must be rejected");
    let message = err.to_string();
    assert!(
        message.contains("host-reserved"),
        "the error must name the colliding server, got: {message}"
    );
}
