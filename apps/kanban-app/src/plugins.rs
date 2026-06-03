//! Plugin platform integration for the kanban desktop app.
//!
//! The kanban app embeds the SwissArmyHammer plugin platform
//! ([`swissarmyhammer_plugin::PluginHost`]) so user-authored and shipped
//! plugins load as part of the application context. This module is the thin
//! glue that wires the platform to the app:
//!
//! - **Builtin layer** — the `builtin/plugins/` tree under the repository root
//!   is compiled into the binary via [`include_dir!`] and extracted to a cache
//!   directory at startup. The cache directory is then handed to the host as
//!   its read-only builtin layer root, so the builtin bundles are discovered
//!   through `discover_and_load_all` — a first-class, lowest-precedence
//!   discovery layer — rather than loaded one bundle at a time.
//! - **User layer** — the host's writable user-layer root is
//!   `$XDG_CONFIG_HOME/kanban` (resolved by
//!   [`swissarmyhammer_directory::KanbanConfig`]); plugin bundles live under its
//!   `plugins/` subdirectory. There is **no project layer**.
//! - **Exposed servers** — the in-process `kanban` tool from
//!   `swissarmyhammer-tools` is exposed to the host via `expose_rust_module`
//!   (reusing the `swissarmyhammer-tools` tool-exposure path) so plugins can
//!   drive real kanban operations.
//!
//! [`PluginPlatform`] bundles the live [`PluginHost`] with the
//! [`PluginWatcher`] keeping hot reload alive; [`AppState`](crate::state::AppState)
//! owns one. Construction is split: [`PluginPlatform::build`] produces a host
//! with the `kanban` module exposed and every plugin loaded, while
//! [`PluginPlatform::start_watcher`] starts the hot-reload watcher once the
//! Tauri `AppHandle` exists — mirroring how board watchers start in
//! [`AppState::start_watchers`](crate::state::AppState::start_watchers).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use include_dir::{include_dir, Dir};
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_config::ModelConfig;
use swissarmyhammer_directory::KanbanConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_plugin::host::PluginWatcher;
use swissarmyhammer_plugin::{PluginHost, PLUGINS_SUBDIR};
use swissarmyhammer_tools::mcp::plugin_bridge::build_tool_modules;
use swissarmyhammer_tools::mcp::ToolHandlers;
use swissarmyhammer_tools::{register_kanban_tools, ToolContext, ToolRegistry};
use tokio::sync::{Mutex as TokioMutex, RwLock};

/// The builtin plugin bundles shipped with the kanban app.
///
/// The repository's top-level `builtin/plugins/` tree holds every builtin
/// bundle, and the kanban app now exposes the services those bundles need
/// (the in-process `kanban` tool plus the command-service modules wired by
/// [`PluginPlatform::wire_command_services`]: `store`, `entity`, `ui_state`,
/// `focus`, and `commands`). Every top-level subdirectory of
/// `builtin/plugins/` is one plugin bundle: `kanban-builtin-probe` (the
/// read-only builtin-layer probe) plus the 7 command plugins
/// (`task-commands`, `kanban-misc-commands`, `file-commands`,
/// `perspective-commands`, `entity-commands`, `ui-commands`,
/// `app-shell-commands`) that register the cut-over's command surface.
///
/// At startup the embedded tree is extracted ([`extract_builtin_plugins`])
/// into the `plugins/` subdirectory of a cache directory — one per-bundle
/// subdirectory each — and that cache directory is handed to the host as
/// its read-only builtin layer root, so `discover_and_load_all` discovers
/// each bundle as a first-class builtin-layer plugin.
static BUILTIN_PLUGINS: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../builtin/plugins");

/// The module id the in-process `kanban` operation tool is exposed under.
///
/// A plugin addresses it with `register(name, { rust: "kanban" })`; the builtin
/// probe plugin does exactly this. Only referenced by the test harness — in
/// production the id is the kanban tool's own name, set by
/// [`build_tool_modules`].
#[cfg(test)]
const KANBAN_MODULE_ID: &str = "kanban";

/// The live plugin platform owned by the application state.
///
/// Bundles the [`PluginHost`] — which carries the loaded builtin and user-layer
/// plugins and the exposed `kanban` server — with the [`PluginWatcher`] that
/// keeps hot reload running. The watcher is `None` until
/// [`start_watcher`](Self::start_watcher) is called from the Tauri `setup`
/// hook, because the watcher only needs to start once the app is fully up.
pub(crate) struct PluginPlatform {
    /// The application's plugin host: builtin + user-layer plugins, with the
    /// in-process `kanban` tool exposed as a Rust module.
    host: PluginHost,

    /// The host's writable user-layer plugin root — `$XDG_CONFIG_HOME/kanban`.
    /// The hot-reload watcher watches its `plugins/` subdirectory.
    user_root: PathBuf,

    /// The hot-reload watcher; kept alive so the host reacts to plugin files
    /// changing on disk. `None` until [`start_watcher`](Self::start_watcher).
    watcher: Option<PluginWatcher>,

    /// The wired command service, populated by
    /// [`wire_command_services`](Self::wire_command_services). `None` until
    /// production wiring runs (test fixtures and the degraded empty platform
    /// leave it `None`).
    command_service: Option<std::sync::Arc<CommandService>>,
}

impl PluginPlatform {
    /// Builds the plugin platform: a host with the `kanban` tool exposed and
    /// ready for additional module wiring + plugin discovery.
    ///
    /// The bundled builtin plugins are first extracted to `builtin_cache`, and
    /// that cache directory is handed to [`PluginHost::new`] as the host's
    /// read-only **builtin layer root** — the lowest-precedence discovery
    /// layer. The in-process `kanban` tool is exposed *before* any plugin is
    /// loaded, so a plugin that activates `{ rust: "kanban" }` always finds the
    /// module already exposed.
    ///
    /// The optional `project_root` is the host's writable **project layer**, the
    /// highest-precedence discovery layer (project shadows user shadows
    /// builtin). A per-board host passes the board's `.kanban` directory so its
    /// project plugins resolve at `<board_dir>/.kanban/plugins/<id>/`; the
    /// global fallback host passes `None`, so it carries only the builtin and
    /// user layers shared process-wide.
    ///
    /// Plugins are NOT discovered here — call
    /// [`discover_plugins`](Self::discover_plugins) after any additional
    /// modules are exposed (production wires the command-service modules in
    /// between via [`wire_command_services`](Self::wire_command_services)).
    ///
    /// # Parameters
    ///
    /// - `user_root` — the writable user-layer root (`$XDG_CONFIG_HOME/kanban`).
    /// - `builtin_cache` — the directory the bundled builtin plugins are
    ///   extracted into; it becomes the host's builtin layer root.
    /// - `project_root` — the writable project-layer root, or `None` for a host
    ///   with no project layer. Discovery joins `plugins/` onto this root, so a
    ///   per-board host passes `<board_dir>/.kanban` to resolve project plugins
    ///   at `<board_dir>/.kanban/plugins/<id>/`.
    /// - `tool_working_dir` — the working directory the exposed `kanban` tool
    ///   resolves its `.kanban` board against.
    ///
    /// # Errors
    ///
    /// Returns the platform error string when the builtin plugins cannot be
    /// extracted or the `kanban` module cannot be exposed.
    pub(crate) async fn build(
        user_root: PathBuf,
        builtin_cache: PathBuf,
        project_root: Option<PathBuf>,
        tool_working_dir: PathBuf,
    ) -> Result<Self, String> {
        // Extract the embedded builtin bundles into the cache, then build the
        // host with that cache as its builtin layer root. The builtin layer is
        // a first-class discovery layer: builtins are discovered, not loaded
        // one by one. The project layer (when supplied) stacks on top of the
        // shared builtin + user layers, so a per-board host's project plugins
        // shadow user/builtin copies of the same id for that board only.
        extract_builtin_plugins(&builtin_cache)?;
        let host = PluginHost::new(
            Some(builtin_cache),
            user_root.clone(),
            project_root,
            false,
            user_root.clone(),
        );

        // Expose the `kanban` module before discovery runs, so the builtin
        // probe plugin's `{ rust: "kanban" }` activation never races a missing
        // module.
        expose_kanban_module(&host, tool_working_dir).await?;

        Ok(Self {
            host,
            user_root,
            watcher: None,
            command_service: None,
        })
    }

    /// Discover and load every plugin from the builtin and user layers.
    ///
    /// Split out from [`build`] so a production caller can expose additional
    /// in-process MCP modules (the `commands` service and its sibling
    /// modules — see [`Self::wire_command_services`]) between
    /// `expose_kanban_module` and discovery. A plugin that activates one of
    /// those new modules (`{ rust: "commands" }`, …) at `load()` time
    /// would race a missing module if discovery ran first.
    ///
    /// Tests that don't exercise the command service call this directly
    /// after [`build`] without going through [`wire_command_services`].
    pub(crate) async fn discover_plugins(&self) -> Result<(), String> {
        self.host
            .discover_and_load_all::<KanbanConfig>()
            .await
            .map(|_| ())
            .map_err(|e| format!("failed to discover builtin and user-layer plugins: {e}"))
    }

    /// Expose the production in-process MCP modules (`store`, `entity`,
    /// `ui_state`, `focus`, and `commands` with the store-backed
    /// transaction seam) on this platform's host and stash the returned
    /// `CommandService`.
    ///
    /// Call after [`build`] and before [`discover_plugins`]. The `window`
    /// module is conditional on `window_shell` (`None` skips it — the
    /// kanban app supplies `None` from `AppState::new` because the Tauri
    /// `AppHandle` only exists from the `setup_app` hook).
    ///
    /// Returns the wired `Arc<CommandService>` for the caller to thread
    /// onto `AppState` if desired; the platform also stores a clone
    /// internally (see [`Self::command_service`]).
    pub(crate) async fn wire_command_services(
        &mut self,
        ui_state: std::sync::Arc<swissarmyhammer_ui_state::UIState>,
        window_shell: Option<std::sync::Arc<dyn swissarmyhammer_window_service::WindowShell>>,
        app_shell: Option<std::sync::Arc<dyn swissarmyhammer_app_service::AppShell>>,
    ) -> Result<std::sync::Arc<CommandService>, String> {
        let service = crate::command_services::install_app_command_services(
            &self.host,
            ui_state,
            window_shell,
            app_shell,
        )
        .await?;
        self.command_service = Some(std::sync::Arc::clone(&service));
        Ok(service)
    }

    /// Expose the `AppHandle`-backed `window` and `app` modules on this
    /// platform's host.
    ///
    /// Call from the Tauri `setup_app` hook, after [`wire_command_services`]
    /// (which wires every non-`AppHandle` module, deferring `window` / `app`)
    /// and BEFORE [`discover_plugins`]. The `WindowShell` / `AppShell` seams
    /// require a live `AppHandle`, which only exists at setup; exposing them
    /// here lets the `file-commands` / `ui-commands` / `kanban-misc-commands` /
    /// `app-shell-commands` builtin plugins satisfy `ensureServices` so the
    /// atomic `discover_and_load_all` loads ALL 7 builtin command plugins.
    pub(crate) async fn expose_apphandle_modules(
        &self,
        window_shell: Option<std::sync::Arc<dyn swissarmyhammer_window_service::WindowShell>>,
        app_shell: Option<std::sync::Arc<dyn swissarmyhammer_app_service::AppShell>>,
    ) -> Result<(), String> {
        crate::command_services::expose_apphandle_modules(&self.host, window_shell, app_shell).await
    }

    /// The wired `Arc<CommandService>`, or `None` when
    /// [`wire_command_services`] has not been called (test fixtures and the
    /// degraded empty platform).
    #[allow(dead_code)]
    pub(crate) fn command_service(&self) -> Option<std::sync::Arc<CommandService>> {
        self.command_service.clone()
    }

    /// Builds an empty plugin platform rooted at `user_root`, loading nothing.
    ///
    /// Synchronous: the host is built with [`PluginHost::for_tests`] (the
    /// platform's infallible, no-builtins constructor) over `user_root` and no
    /// project layer; no module is exposed and no plugin is loaded. Used as the
    /// production degraded fallback when [`build`](Self::build) fails, so the
    /// app still starts with an inert plugin host rather than crashing.
    pub(crate) fn empty(user_root: PathBuf) -> Self {
        let host = PluginHost::for_tests(user_root.clone(), None);
        Self {
            host,
            user_root,
            watcher: None,
            command_service: None,
        }
    }

    /// Builds an empty plugin platform for tests that do not exercise plugins.
    ///
    /// Synchronous so the kanban app's plain `#[test]` constructors keep
    /// working. Delegates to [`empty`](Self::empty) over a fresh temp user
    /// root. Tests that *do* exercise plugins use [`build`](Self::build).
    #[cfg(test)]
    pub(crate) fn for_tests_empty() -> Self {
        let user_root =
            std::env::temp_dir().join(format!("kanban-plugins-test-{}", ulid::Ulid::new()));
        Self::empty(user_root)
    }

    /// Starts the hot-reload watcher on the user-layer `plugins/` directory.
    ///
    /// Call this from the Tauri `setup` hook — alongside
    /// [`AppState::start_watchers`](crate::state::AppState::start_watchers) —
    /// once the app is up. Idempotent: a second call replaces the watcher.
    ///
    /// A failure to start the watcher is logged rather than propagated: the
    /// already-loaded plugins keep running, only hot reload is unavailable.
    pub(crate) async fn start_watcher(&mut self) {
        match self.host.watch_plugins::<KanbanConfig>().await {
            Ok(watcher) => {
                tracing::info!(
                    root = %self.user_root.join("plugins").display(),
                    "kanban plugin hot-reload watcher started"
                );
                self.watcher = Some(watcher);
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to start kanban plugin watcher");
            }
        }
    }

    /// The application's plugin host.
    ///
    /// Used by the generic MCP transport handlers in `commands.rs`
    /// (`command_tool_call`, `mcp_subscribe`) to route `tools/call` requests
    /// and subscribe to the `NotificationBridge`, and by the plugin
    /// integration tests, which drive the host directly. Most production
    /// code reaches plugins through the host the platform already wired
    /// during [`build`](Self::build).
    pub(crate) fn host(&self) -> &PluginHost {
        &self.host
    }
}

/// Exposes the in-process `kanban` operation tool to `host` as a Rust module.
///
/// A minimal [`ToolRegistry`] holding only the kanban tools is built and paired
/// with a [`ToolContext`] whose `working_dir` is `tool_working_dir` — the kanban
/// tool resolves its board at `<working_dir>/.kanban`. The tool is wrapped via
/// the `swissarmyhammer-tools` [`build_tool_modules`] adapter — the same path
/// `McpServer::expose_tools_to_plugin_host` uses — and handed to the host under
/// its tool name with `expose_rust_module`.
///
/// # Errors
///
/// Returns the platform error string when `expose_rust_module` rejects a
/// module id (in practice, an id already exposed).
async fn expose_kanban_module(host: &PluginHost, tool_working_dir: PathBuf) -> Result<(), String> {
    let mut registry = ToolRegistry::new();
    register_kanban_tools(&mut registry);
    let registry = Arc::new(RwLock::new(registry));

    // The kanban tool only needs `working_dir` (to locate its `.kanban` board)
    // and the registry handle; git is not used, so `git_ops` is `None`.
    let git_ops = Arc::new(TokioMutex::new(None::<GitOperations>));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ModelConfig::default());
    let context = ToolContext::new(tool_handlers, git_ops, agent_config)
        .with_tool_registry(Arc::clone(&registry))
        .with_working_dir(tool_working_dir);
    let context = Arc::new(context);

    for (id, module) in build_tool_modules(registry, context).await {
        host.expose_rust_module(id, module)
            .await
            .map_err(|e| format!("failed to expose tool module to plugin host: {e}"))?;
    }
    Ok(())
}

/// Extracts the compiled-in builtin plugins into `cache_dir` so it can serve
/// as the host's read-only builtin layer root.
///
/// Plugin discovery resolves bundles under `<root>/plugins/<bundle>/`, so each
/// top-level subdirectory of [`BUILTIN_PLUGINS`] is one plugin bundle and is
/// extracted into its own per-bundle subdirectory of the layer root's
/// `plugins/` directory. Handing `cache_dir` itself to the host as the builtin
/// layer root then makes `discover_and_load_all` find every bundle. The cache
/// directory is removed and recreated first, so a previous extraction from an
/// older binary cannot leave a stale bundle behind.
///
/// # Errors
///
/// Returns the error string when the cache directory cannot be reset, a
/// bundle has no usable directory name, or the embedded tree cannot be
/// written to disk.
fn extract_builtin_plugins(cache_dir: &Path) -> Result<(), String> {
    if cache_dir.exists() {
        std::fs::remove_dir_all(cache_dir)
            .map_err(|e| format!("failed to clear builtin plugin cache: {e}"))?;
    }
    let plugins_dir = cache_dir.join(PLUGINS_SUBDIR);
    std::fs::create_dir_all(&plugins_dir)
        .map_err(|e| format!("failed to create builtin plugin cache: {e}"))?;

    // Each top-level subdirectory of BUILTIN_PLUGINS is one plugin bundle
    // (`kanban-builtin-probe`, `task-commands`, …) and must land at
    // `plugins/<bundle-name>/<files>` so the host discovers each as a
    // first-class bundle.
    //
    // `Dir::extract(base)` joins the FULL embedded entry path — which already
    // carries the `<bundle-name>/` prefix (e.g. `entity-commands/index.ts`) —
    // onto `base`, so the base must be `plugins_dir`, NOT a per-bundle subdir
    // (that would double-nest to `plugins/<bundle>/<bundle>/...`). `extract`
    // only `create_dir_all`s for `Dir` ENTRIES, so a flat bundle (just an
    // `index.ts`, no subdir) has no entry that creates its own
    // `plugins/<bundle>/` parent — `fs::write` would then fail with ENOENT.
    // Pre-create `plugins/<bundle-name>/` here to cover that flat case before
    // extracting into `plugins_dir`.
    for bundle in BUILTIN_PLUGINS.dirs() {
        let bundle_name = bundle
            .path()
            .file_name()
            .ok_or_else(|| {
                format!(
                    "builtin plugin entry has no bundle name: {}",
                    bundle.path().display()
                )
            })?
            .to_owned();
        let bundle_dir = plugins_dir.join(&bundle_name);
        std::fs::create_dir_all(&bundle_dir).map_err(|e| {
            format!(
                "failed to create builtin plugin cache directory {}: {e}",
                bundle_dir.display()
            )
        })?;
        bundle.extract(&plugins_dir).map_err(|e| {
            format!(
                "failed to extract builtin plugin bundle {}: {e}",
                bundle_name.to_string_lossy()
            )
        })?;
    }
    Ok(())
}

/// The module id the kanban operation tool is exposed under.
///
/// Used by the integration tests that drive the exposed server by name.
#[cfg(test)]
pub(crate) const fn kanban_module_id() -> &'static str {
    KANBAN_MODULE_ID
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use serde_json::json;
    use swissarmyhammer_plugin::{CallerId, ReloadStatus};
    use tempfile::TempDir;

    use super::kanban_module_id;
    use crate::state::AppState;

    /// A generous upper bound on any single host interaction.
    const TIMEOUT: Duration = Duration::from_secs(20);

    /// How long a watcher-driven load is polled for before the test fails.
    const SETTLE: Duration = Duration::from_secs(15);

    /// Writes a genuine probe plugin bundle into `plugins_dir/<id>/`.
    ///
    /// The bundle is a real TS-only bundle: just an `index.ts`. The plugin's
    /// `load()` runs real plugin code (a `log` call) without registering a
    /// server — exactly what is needed to prove a layer genuinely *loads* a
    /// plugin (isolate created, lifecycle run) without contending for the
    /// single-activation `kanban` Rust module, which the builtin probe
    /// already consumes.
    ///
    /// `id` is the bundle directory name and so the plugin's identity.
    fn write_probe_plugin(plugins_dir: &std::path::Path, id: &str) {
        let plugin_dir = plugins_dir.join(id);
        std::fs::create_dir_all(&plugin_dir).expect("probe plugin directory");
        let entry = format!(
            "import {{ Plugin }} from '@swissarmyhammer/plugin';\n\
             export default class P extends Plugin {{\n\
               async load(): Promise<void> {{\n\
                 this.log.info('{id} loaded');\n\
               }}\n\
             }}\n"
        );
        std::fs::write(plugin_dir.join("index.ts"), entry).expect("probe index.ts");
    }

    /// Constructing `AppState` over temp roots loads the builtin probe plugin
    /// and a user-layer probe plugin, and the builtin plugin can drive the
    /// exposed `kanban` server to a real effect.
    ///
    /// This is the integration test the task's TDD step calls for. It builds
    /// the real plugin platform via `AppState::new_for_test_with_plugins` — the
    /// `new_for_test` pattern parameterized for a temp `~/.config/kanban`, a
    /// temp builtin-extraction cache, and a temp kanban working directory —
    /// then asserts:
    ///
    /// - the **builtin** probe (bundled into the binary, extracted, loaded as
    ///   the read-only builtin layer) is live and drives a *real kanban effect*
    ///   through the host-exposed `kanban` server;
    /// - the **user-layer** probe (discovered from `<user_root>/plugins/`)
    ///   loaded — its discovery-recorded reload status is `Healthy`.
    #[tokio::test]
    async fn app_state_loads_builtin_and_user_layer_plugins() {
        let user_root = TempDir::new().expect("user root temp dir");
        let builtin_cache = TempDir::new().expect("builtin cache temp dir");
        let board_dir = TempDir::new().expect("kanban board temp dir");

        // Drop a user-layer probe into <user_root>/plugins/ before construction
        // so `discover_and_load_all` finds it during `AppState::new_*`.
        let user_plugins = user_root.path().join("plugins");
        write_probe_plugin(&user_plugins, "kanban-user-probe");

        let state = AppState::new_for_test_with_plugins(
            user_root.path().to_path_buf(),
            builtin_cache.path().to_path_buf(),
            board_dir.path().to_path_buf(),
        )
        .await
        .expect("AppState should build with the plugin platform");

        let platform = state.plugin_platform.lock().await;

        // The user-layer probe — discovered from <user_root>/plugins/ — loaded:
        // discovery recorded it with a `Healthy` reload status.
        assert_eq!(
            platform.host().reload_status("kanban-user-probe").await,
            Some(ReloadStatus::Healthy),
            "the user-layer probe plugin must have been discovered and loaded"
        );

        // The builtin probe — bundled into the binary, extracted, loaded as the
        // read-only builtin layer — discovered healthy.
        assert_eq!(
            platform.host().reload_status("kanban-builtin-probe").await,
            Some(ReloadStatus::Healthy),
            "the builtin probe plugin must have been discovered and loaded"
        );

        // The builtin probe is live: it answers a real `kanban` call through the
        // host-exposed `kanban` server (the probe registers it under the
        // canonical name `kanban`, shared with the command plugins). `init board`
        // is a real kanban effect: it creates a `.kanban` board on disk.
        let result = tokio::time::timeout(
            TIMEOUT,
            platform.host().call(
                CallerId::HostInternal,
                kanban_module_id(),
                kanban_module_id(),
                json!({ "op": "init board", "name": "Builtin Board" }),
            ),
        )
        .await
        .expect("a kanban call should not hang")
        .expect("the builtin probe should drive the exposed kanban server");
        let rendered = serde_json::to_string(&result).expect("a kanban result is serializable");
        assert!(
            rendered.contains("Builtin Board"),
            "the builtin probe must drive a real kanban effect, got {rendered}"
        );
        assert!(
            board_dir.path().join(".kanban").is_dir(),
            "a real .kanban board must have been created by the kanban tool"
        );
    }

    /// A plugin dropped into the user `plugins/` directory after the hot-reload
    /// watcher starts is picked up and loaded into the running host.
    ///
    /// This reuses the hot-reload watcher the kanban app starts from its Tauri
    /// `setup` hook — here driven directly via `AppState::start_plugin_watcher`
    /// — and proves the watcher reconciles a freshly-added user-layer bundle:
    /// the dropped plugin's discovery-recorded reload status becomes `Healthy`.
    #[tokio::test]
    async fn watcher_picks_up_a_plugin_dropped_into_user_layer() {
        let user_root = TempDir::new().expect("user root temp dir");
        let builtin_cache = TempDir::new().expect("builtin cache temp dir");
        let board_dir = TempDir::new().expect("kanban board temp dir");

        // The user `plugins/` directory starts empty.
        let user_plugins = user_root.path().join("plugins");
        std::fs::create_dir_all(&user_plugins).expect("user plugins dir");

        let state = AppState::new_for_test_with_plugins(
            user_root.path().to_path_buf(),
            builtin_cache.path().to_path_buf(),
            board_dir.path().to_path_buf(),
        )
        .await
        .expect("AppState should build with the plugin platform");

        // Start the hot-reload watcher, then let the OS watcher register before
        // mutating the tree.
        state.start_plugin_watcher().await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Drop a new plugin into the watched user layer.
        write_probe_plugin(&user_plugins, "kanban-dropped-probe");

        // Poll until the dropped plugin is loaded — its discovery-recorded
        // reload status becoming `Healthy` is proof the watcher fired and the
        // host loaded the new bundle in place.
        let deadline = Instant::now() + SETTLE;
        loop {
            let loaded = {
                let platform = state.plugin_platform.lock().await;
                platform.host().reload_status("kanban-dropped-probe").await
                    == Some(ReloadStatus::Healthy)
            };
            if loaded {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the dropped plugin was never loaded by the watcher within {SETTLE:?}"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    // ── Per-board PluginHost integration tests ──────────────────────────────
    //
    // These prove the per-board host architecture: each open board gets its own
    // `PluginPlatform` (host + registries + CommandService), keyed by board
    // path on its `BoardHandle`, with the global platform as the fallback for
    // boardless windows. They drive the REAL pipeline — `AppState::open_board`
    // builds each per-board platform from the shared roots, exactly as
    // production does — not a fixture.

    /// A builtin command id every per-board host must carry, registered by the
    /// bundled `task-commands` plugin (`builtin/plugins/task-commands`).
    ///
    /// Derived from the first baseline entry so the two never drift — both name
    /// the `task-commands` plugin's representative command.
    const BUILTIN_COMMAND_ID: &str = BUILTIN_COMMAND_BASELINE[0];

    /// One representative command id per builtin command plugin — proof that
    /// ALL 7 plugins discovered and registered (each id comes from a distinct
    /// bundle, and the four after `entity.add` activate the `views` / `window` /
    /// `app` backends that previously failed `ensureServices`):
    ///
    /// - `task.move`        → task-commands       (`commands`, `kanban`)
    /// - `entity.add`       → entity-commands     (`commands`, `entity`)
    /// - `perspective.filter` → perspective-commands (`commands`, **views**)
    /// - `file.switchBoard` → file-commands       (`commands`, **window**)
    /// - `ui.palette.open`  → ui-commands         (`commands`, ui_state, **window**, focus)
    /// - `view.set`         → kanban-misc-commands (`commands`, kanban, **window**, **views**)
    /// - `app.quit`         → app-shell-commands  (`commands`, **app**, ui_state, store)
    const BUILTIN_COMMAND_BASELINE: &[&str] = &[
        "task.move",
        "entity.add",
        "perspective.filter",
        "file.switchBoard",
        "ui.palette.open",
        "view.set",
        "app.quit",
    ];

    /// The 7 builtin command-plugin bundle ids under `builtin/plugins/`
    /// (excluding the read-only `kanban-builtin-probe`).
    const BUILTIN_COMMAND_PLUGINS: &[&str] = &[
        "task-commands",
        "entity-commands",
        "perspective-commands",
        "file-commands",
        "ui-commands",
        "kanban-misc-commands",
        "app-shell-commands",
    ];

    /// Assert a platform's host carries the full builtin command baseline — one
    /// command from each of the 7 builtin command plugins.
    async fn assert_builtin_baseline(platform: &super::PluginPlatform, ctx: &str) {
        let ids = list_command_ids(platform).await;
        for id in BUILTIN_COMMAND_BASELINE {
            assert!(
                ids.contains(*id),
                "{ctx} must carry the builtin baseline command {id:?}; got {ids:?}"
            );
        }
    }

    /// Call `list command` on a platform's host and return the set of command
    /// ids it reports. Mirrors the production `command_tool_call` path
    /// (`host.call(HostInternal, "commands", "command", { op: "list command" })`).
    async fn list_command_ids(
        platform: &super::PluginPlatform,
    ) -> std::collections::HashSet<String> {
        let result = tokio::time::timeout(
            TIMEOUT,
            platform.host().call(
                CallerId::HostInternal,
                "commands",
                "command",
                json!({ "op": "list command" }),
            ),
        )
        .await
        .expect("list command should not hang")
        .expect("the commands module answers list command");

        command_ids_from_call_result(&result)
    }

    /// Extract the set of command ids from a `command` tool `list command`
    /// result.
    ///
    /// `PluginHost::call` returns the raw rmcp `CallToolResult` JSON: the
    /// payload is a `content` array whose first text part carries the
    /// `{ ok, commands: [...] }` object as a JSON string. Parse that text part,
    /// then pull each command's `id`. Falls back to a top-level / structured
    /// `commands` array so the helper is robust to either response shape.
    fn command_ids_from_call_result(
        result: &serde_json::Value,
    ) -> std::collections::HashSet<String> {
        let commands = result
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|parts| parts.iter().find_map(|p| p.get("text")?.as_str()))
            .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
            .and_then(|parsed| parsed.get("commands").cloned())
            .or_else(|| {
                result
                    .get("structuredContent")
                    .and_then(|sc| sc.get("commands"))
                    .cloned()
            })
            .or_else(|| result.get("commands").cloned());

        commands
            .as_ref()
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| c.get("id").and_then(|v| v.as_str()))
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Open a board rooted at a fresh temp dir on `state`, returning the temp
    /// dir (kept alive by the caller) and the canonical `.kanban` path the
    /// board was registered under.
    async fn open_temp_board(state: &AppState) -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().expect("board temp dir");
        let canonical = state
            .open_board(dir.path(), None)
            .await
            .expect("open_board should succeed");
        (dir, canonical)
    }

    /// Build an `AppState` with real shared plugin roots so `open_board` builds
    /// a per-board platform for each board. The user plugin layer starts empty
    /// (only the bundled builtins load), which is enough to prove the builtin
    /// command baseline and per-board isolation.
    async fn app_state_with_plugin_roots() -> (TempDir, TempDir, TempDir, AppState) {
        let user_root = TempDir::new().expect("user root temp dir");
        let builtin_cache = TempDir::new().expect("builtin cache temp dir");
        let global_board_dir = TempDir::new().expect("global tool working dir");
        std::fs::create_dir_all(user_root.path().join("plugins")).expect("user plugins dir");

        let state = AppState::new_for_test_with_plugins(
            user_root.path().to_path_buf(),
            builtin_cache.path().to_path_buf(),
            global_board_dir.path().to_path_buf(),
        )
        .await
        .expect("AppState should build with the plugin platform");

        (user_root, builtin_cache, global_board_dir, state)
    }

    /// Two boards opened in one `AppState` each get a DISTINCT per-board
    /// `PluginPlatform` / `CommandService`, and each carries the builtin
    /// command baseline.
    #[tokio::test]
    async fn two_boards_get_distinct_platforms_with_builtin_baseline() {
        let (_user_root, _builtin_cache, _global_dir, state) = app_state_with_plugin_roots().await;

        let (_dir_a, path_a) = open_temp_board(&state).await;
        let (_dir_b, path_b) = open_temp_board(&state).await;
        assert_ne!(path_a, path_b, "the two boards must be distinct");

        let boards = state.boards.read().await;
        let handle_a = boards.get(&path_a).expect("board A is open").clone();
        let handle_b = boards.get(&path_b).expect("board B is open").clone();
        drop(boards);

        let platform_a = handle_a
            .platform()
            .expect("board A has a per-board platform");
        let platform_b = handle_b
            .platform()
            .expect("board B has a per-board platform");

        // The CommandService instances are DISTINCT objects — registries are
        // isolated per board.
        let svc_a = platform_a
            .lock()
            .await
            .command_service()
            .expect("board A wired a command service");
        let svc_b = platform_b
            .lock()
            .await
            .command_service()
            .expect("board B wired a command service");
        assert!(
            !std::sync::Arc::ptr_eq(&svc_a, &svc_b),
            "each board must own a distinct CommandService instance"
        );

        // Each board carries the FULL builtin command baseline — all 7 plugins,
        // including the four that need the `views` / `window` / `app` backends.
        assert_builtin_baseline(&*platform_a.lock().await, "board A").await;
        assert_builtin_baseline(&*platform_b.lock().await, "board B").await;
    }

    /// The global host and every per-board host load ALL 7 builtin command
    /// plugins and register the full builtin command baseline.
    ///
    /// This is the acceptance for the wiring card: `discover_and_load_all` is
    /// atomic, so a single unwired backend (`views` / `window` / `app`) would
    /// roll back ALL 7 plugins and leave an empty command registry. A passing
    /// run proves every backend each plugin's `ensureServices` requires is
    /// exposed before discovery on both host kinds.
    #[tokio::test]
    async fn all_seven_builtin_command_plugins_load_with_full_baseline() {
        let (_user_root, _builtin_cache, _global_dir, state) = app_state_with_plugin_roots().await;

        // The global fallback host: every builtin command plugin discovered
        // healthy, and the full baseline is registered.
        {
            let global = state.plugin_platform.lock().await;
            for bundle in BUILTIN_COMMAND_PLUGINS {
                assert_eq!(
                    global.host().reload_status(bundle).await,
                    Some(ReloadStatus::Healthy),
                    "global host must load the builtin command plugin {bundle:?}"
                );
            }
            assert_builtin_baseline(&global, "the global host").await;
        }

        // A per-board host built at board-open time carries the same baseline.
        let (_dir, path) = open_temp_board(&state).await;
        let handle = {
            let boards = state.boards.read().await;
            boards.get(&path).expect("board is open").clone()
        };
        let platform = handle.platform().expect("board has a per-board platform");
        let platform = platform.lock().await;
        for bundle in BUILTIN_COMMAND_PLUGINS {
            assert_eq!(
                platform.host().reload_status(bundle).await,
                Some(ReloadStatus::Healthy),
                "per-board host must load the builtin command plugin {bundle:?}"
            );
        }
        assert_builtin_baseline(&platform, "the per-board host").await;
    }

    /// Closing board A drops its per-board host without affecting board B: the
    /// command path resolved for board B still answers `list command`.
    #[tokio::test]
    async fn closing_a_board_drops_its_host_without_affecting_the_other() {
        let (_user_root, _builtin_cache, _global_dir, state) = app_state_with_plugin_roots().await;

        let (_dir_a, path_a) = open_temp_board(&state).await;
        let (_dir_b, path_b) = open_temp_board(&state).await;

        // Board B keeps a live, working host before and after A closes.
        let handle_b = {
            let boards = state.boards.read().await;
            boards.get(&path_b).expect("board B is open").clone()
        };
        let platform_b = handle_b.platform().expect("board B has a platform");
        assert!(
            list_command_ids(&*platform_b.lock().await)
                .await
                .contains(BUILTIN_COMMAND_ID),
            "board B answers before board A closes"
        );

        // Close board A — its handle (and so its per-board platform) is dropped.
        // `close_board` keys by the canonical `.kanban` path (the same key
        // `open_board` registered under), so pass `path_a`, not the board dir.
        state
            .close_board(&path_a)
            .await
            .expect("closing board A succeeds");
        {
            let boards = state.boards.read().await;
            assert!(
                !boards.contains_key(&path_a),
                "board A must be removed from the open set"
            );
            assert!(
                boards.contains_key(&path_b),
                "board B must remain open after board A closes"
            );
        }

        // Board B's host is unaffected: it still answers list command.
        assert!(
            list_command_ids(&*platform_b.lock().await)
                .await
                .contains(BUILTIN_COMMAND_ID),
            "board B must keep answering after board A's host is dropped"
        );
    }

    /// Register a command directly into a platform's host command registry via
    /// the production `commands` module `register command` op, mirroring how a
    /// project plugin would register one. The `execute` callback id only has to
    /// resolve at dispatch time, not at register/list time, so a placeholder is
    /// enough to prove the command lands in (and is listed by) THIS host.
    async fn register_command_into(platform: &super::PluginPlatform, id: &str) {
        tokio::time::timeout(
            TIMEOUT,
            platform.host().call(
                CallerId::HostInternal,
                "commands",
                "command",
                json!({
                    "op": "register command",
                    "id": id,
                    "name": id,
                    "execute": { "$callback": "isolation-test-callback" },
                }),
            ),
        )
        .await
        .expect("register command should not hang")
        .expect("the commands module answers register command");
    }

    /// REGISTRY ISOLATION: a command registered into board A's host is visible
    /// ONLY in board A's `list command`, never board B's.
    ///
    /// This is the central per-board guarantee — registries are isolated, not
    /// merely distinct objects. We register straight into one board's host
    /// command service (the same path a project plugin's `register command`
    /// takes) so the test holds NOW, before project-plugin loading lands; once
    /// it does, a project plugin exercises the identical path.
    #[tokio::test]
    async fn a_command_registered_in_one_board_is_invisible_to_the_other() {
        let (_user_root, _builtin_cache, _global_dir, state) = app_state_with_plugin_roots().await;

        let (_dir_a, path_a) = open_temp_board(&state).await;
        let (_dir_b, path_b) = open_temp_board(&state).await;

        let (handle_a, handle_b) = {
            let boards = state.boards.read().await;
            (
                boards.get(&path_a).expect("board A is open").clone(),
                boards.get(&path_b).expect("board B is open").clone(),
            )
        };
        let platform_a = handle_a
            .platform()
            .expect("board A has a per-board platform");
        let platform_b = handle_b
            .platform()
            .expect("board B has a per-board platform");

        // A unique id that is NOT part of the builtin baseline, so seeing it in
        // a host's `list command` proves it came from this registration.
        const ISOLATED_ID: &str = "isolation.probe.only-in-board-a";

        register_command_into(&*platform_a.lock().await, ISOLATED_ID).await;

        let ids_a = list_command_ids(&*platform_a.lock().await).await;
        let ids_b = list_command_ids(&*platform_b.lock().await).await;

        assert!(
            ids_a.contains(ISOLATED_ID),
            "board A's host must list the command registered into it; got {ids_a:?}"
        );
        assert!(
            !ids_b.contains(ISOLATED_ID),
            "board B's host must NOT see a command registered into board A — \
             registries are isolated; got {ids_b:?}"
        );
        // Sanity: board B still carries the shared builtin baseline, so the
        // absence above is isolation, not an empty registry.
        assert!(
            ids_b.contains(BUILTIN_COMMAND_ID),
            "board B must still carry the builtin baseline; got {ids_b:?}"
        );
    }

    // ── Project plugin layer (per-board) integration tests ──────────────────
    //
    // These prove the per-board PROJECT layer: a plugin bundle checked into a
    // board's `<board_dir>/.kanban/plugins/<id>/` loads in THAT board's host
    // only, stacked over the shared user + builtin layers, and a project copy
    // shadows a user copy of the same id for that board. They drive the REAL
    // pipeline — `open_board` builds each per-board host with its project layer
    // rooted at the board's `.kanban` — not a fixture.

    /// Write a project plugin bundle that registers a single command `command_id`
    /// into the board's `commands` registry, at `<board_dir>/.kanban/plugins/<id>/`.
    ///
    /// The bundle is a real TS-only plugin: it `ensureServices(["commands"])`
    /// then `registerCommands` one command, exactly the path the builtin command
    /// plugins take. Seeing `command_id` in a host's `list command` is proof the
    /// project plugin genuinely loaded and is functional in that host. The
    /// `execute` callback only has to resolve at dispatch time, so a trivial
    /// no-op body is enough to prove registration + listing.
    ///
    /// `board_dir` is the board's working directory (the parent of `.kanban`);
    /// `id` is the bundle directory name and so the plugin's identity.
    fn write_project_command_plugin(board_dir: &std::path::Path, id: &str, command_id: &str) {
        let plugins_dir = board_dir.join(".kanban").join("plugins");
        let plugin_dir = plugins_dir.join(id);
        std::fs::create_dir_all(&plugin_dir).expect("project plugin directory");
        let entry = format!(
            "import {{ Plugin, ensureServices, registerCommands }} from '@swissarmyhammer/plugin';\n\
             export default class P extends Plugin {{\n\
               async load(): Promise<void> {{\n\
                 await ensureServices(this, ['commands']);\n\
                 await registerCommands(this, [{{\n\
                   id: '{command_id}',\n\
                   name: '{command_id}',\n\
                   execute: async () => ({{ ok: true }}),\n\
                 }}]);\n\
                 this.log.info('{id} loaded');\n\
               }}\n\
             }}\n"
        );
        std::fs::write(plugin_dir.join("index.ts"), entry).expect("project plugin index.ts");
    }

    /// Open a board on `state` whose project plugin layer has been seeded BEFORE
    /// open, so `open_board`'s per-board host discovers the project plugin.
    ///
    /// Returns the board temp dir (kept alive by the caller) and the canonical
    /// `.kanban` path the board registered under. `seed` runs against the board
    /// directory while the board is still on disk-only, before `open_board`
    /// builds the per-board platform — that build is when project-layer discovery
    /// runs, so the bundle must already exist.
    async fn open_temp_board_seeded(
        state: &AppState,
        seed: impl FnOnce(&std::path::Path),
    ) -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().expect("board temp dir");
        seed(dir.path());
        let canonical = state
            .open_board(dir.path(), None)
            .await
            .expect("open_board should succeed");
        (dir, canonical)
    }

    /// CROSS-BOARD ISOLATION: a project plugin checked into board A's
    /// `<board_dir>/.kanban/plugins/<id>/` loads in board A's host ONLY — its
    /// command is visible in board A's `list command` and absent from board B's.
    ///
    /// This is the per-board project-layer proof the per-window host card
    /// deferred to this card: the project layer is rooted at each board's
    /// `.kanban`, so a bundle dropped into board A's project layer cannot leak
    /// into board B. Board B carries the shared builtin baseline (proving the
    /// absence is isolation, not an empty registry).
    #[tokio::test]
    async fn a_project_plugin_loads_in_its_board_only() {
        let (_user_root, _builtin_cache, _global_dir, state) = app_state_with_plugin_roots().await;

        // A command id that is NOT part of the builtin baseline, so seeing it in
        // a host's `list command` proves it came from board A's project plugin.
        const PROJECT_COMMAND: &str = "project.probe.only-in-board-a";

        // Board A is seeded with a project plugin BEFORE open, so its per-board
        // host discovers the project layer at <board_a>/.kanban/plugins/.
        let (_dir_a, path_a) = open_temp_board_seeded(&state, |board_dir| {
            write_project_command_plugin(board_dir, "board-a-project-plugin", PROJECT_COMMAND);
        })
        .await;
        // Board B has no project plugin.
        let (_dir_b, path_b) = open_temp_board(&state).await;
        assert_ne!(path_a, path_b, "the two boards must be distinct");

        let (handle_a, handle_b) = {
            let boards = state.boards.read().await;
            (
                boards.get(&path_a).expect("board A is open").clone(),
                boards.get(&path_b).expect("board B is open").clone(),
            )
        };
        let platform_a = handle_a
            .platform()
            .expect("board A has a per-board platform");
        let platform_b = handle_b
            .platform()
            .expect("board B has a per-board platform");

        let ids_a = list_command_ids(&*platform_a.lock().await).await;
        let ids_b = list_command_ids(&*platform_b.lock().await).await;

        assert!(
            ids_a.contains(PROJECT_COMMAND),
            "board A's host must load its project plugin and list its command \
             (project layer rooted at <board_a>/.kanban); got {ids_a:?}"
        );
        assert!(
            !ids_b.contains(PROJECT_COMMAND),
            "board B's host must NOT see board A's project plugin command — the \
             project layer is per-board; got {ids_b:?}"
        );
        // Sanity: board B still carries the shared builtin baseline, so the
        // absence above is per-board project isolation, not an empty registry.
        assert!(
            ids_b.contains(BUILTIN_COMMAND_ID),
            "board B must still carry the builtin baseline; got {ids_b:?}"
        );
    }

    /// PROJECT SHADOWS USER: when a plugin id exists in BOTH the shared user
    /// layer and a board's project layer, the project copy wins for that board.
    ///
    /// Both copies share the bundle id `shadowed-plugin` but register DIFFERENT
    /// command ids. The board's host must list the PROJECT copy's command and
    /// NOT the user copy's — proving project shadows user (the discovery
    /// precedence) end-to-end through the per-board host.
    #[tokio::test]
    async fn a_project_plugin_shadows_a_user_plugin_with_the_same_id() {
        const SHARED_ID: &str = "shadowed-plugin";
        const USER_COMMAND: &str = "shadow.probe.user-copy";
        const PROJECT_COMMAND: &str = "shadow.probe.project-copy";

        // Seed the SHARED user layer with a copy of `shadowed-plugin` BEFORE the
        // AppState (and so the per-board hosts) is built. Reuse the project-plugin
        // writer by pointing it at a fake board dir whose `.kanban/plugins` IS the
        // user root's `plugins/` — the bundle shape is identical across layers.
        let user_root = TempDir::new().expect("user root temp dir");
        let builtin_cache = TempDir::new().expect("builtin cache temp dir");
        let global_board_dir = TempDir::new().expect("global tool working dir");
        let user_plugins = user_root.path().join("plugins");
        std::fs::create_dir_all(&user_plugins).expect("user plugins dir");
        write_user_command_plugin(&user_plugins, SHARED_ID, USER_COMMAND);

        let state = AppState::new_for_test_with_plugins(
            user_root.path().to_path_buf(),
            builtin_cache.path().to_path_buf(),
            global_board_dir.path().to_path_buf(),
        )
        .await
        .expect("AppState should build with the plugin platform");

        // Board's project layer carries a SAME-ID copy registering a different
        // command, so a win is observable by which command id is listed.
        let (_dir, path) = open_temp_board_seeded(&state, |board_dir| {
            write_project_command_plugin(board_dir, SHARED_ID, PROJECT_COMMAND);
        })
        .await;

        let handle = {
            let boards = state.boards.read().await;
            boards.get(&path).expect("board is open").clone()
        };
        let platform = handle.platform().expect("board has a per-board platform");
        let ids = list_command_ids(&*platform.lock().await).await;

        assert!(
            ids.contains(PROJECT_COMMAND),
            "the project copy of {SHARED_ID:?} must win and register its command; got {ids:?}"
        );
        assert!(
            !ids.contains(USER_COMMAND),
            "the user copy of {SHARED_ID:?} must be shadowed by the project copy; got {ids:?}"
        );
    }

    /// Write a user-layer plugin bundle that registers a single command, at
    /// `<user_plugins>/<id>/`. Same bundle shape as
    /// [`write_project_command_plugin`], just rooted at the user layer's
    /// `plugins/` directory — used to stage a same-id copy the project layer
    /// shadows.
    fn write_user_command_plugin(user_plugins: &std::path::Path, id: &str, command_id: &str) {
        let plugin_dir = user_plugins.join(id);
        std::fs::create_dir_all(&plugin_dir).expect("user plugin directory");
        let entry = format!(
            "import {{ Plugin, ensureServices, registerCommands }} from '@swissarmyhammer/plugin';\n\
             export default class P extends Plugin {{\n\
               async load(): Promise<void> {{\n\
                 await ensureServices(this, ['commands']);\n\
                 await registerCommands(this, [{{\n\
                   id: '{command_id}',\n\
                   name: '{command_id}',\n\
                   execute: async () => ({{ ok: true }}),\n\
                 }}]);\n\
                 this.log.info('{id} loaded');\n\
               }}\n\
             }}\n"
        );
        std::fs::write(plugin_dir.join("index.ts"), entry).expect("user plugin index.ts");
    }

    /// A boardless / unknown window falls back to the global platform: the
    /// window → board resolver returns `None`, and the global host still
    /// carries the builtin command baseline.
    #[tokio::test]
    async fn boardless_window_falls_back_to_global_platform() {
        let (_user_root, _builtin_cache, _global_dir, state) = app_state_with_plugin_roots().await;

        // Open one board, but query an unknown window label that has no
        // assignment in UIState.
        let (_dir, _path) = open_temp_board(&state).await;
        assert!(
            state
                .board_handle_for_window("window-with-no-board")
                .await
                .is_none(),
            "an unknown window label must resolve to no board (global fallback)"
        );

        // The global platform — the fallback host for boardless windows — still
        // carries the builtin command baseline.
        let global = state.plugin_platform.lock().await;
        assert!(
            list_command_ids(&global).await.contains(BUILTIN_COMMAND_ID),
            "the global fallback host must carry the builtin command baseline"
        );
    }
}
