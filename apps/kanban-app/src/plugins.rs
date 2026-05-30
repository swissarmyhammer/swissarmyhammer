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
    /// module already exposed. There is **no project layer** — the kanban
    /// app has only the builtin and user layers.
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
        tool_working_dir: PathBuf,
    ) -> Result<Self, String> {
        // Extract the embedded builtin bundles into the cache, then build the
        // host with that cache as its builtin layer root. The builtin layer is
        // a first-class discovery layer: builtins are discovered, not loaded
        // one by one. No project layer — the kanban app has only builtin +
        // user.
        extract_builtin_plugins(&builtin_cache)?;
        let host = PluginHost::new(
            Some(builtin_cache),
            user_root.clone(),
            None,
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
    ) -> Result<std::sync::Arc<CommandService>, String> {
        let service = crate::command_services::install_app_command_services(
            &self.host,
            ui_state,
            window_shell,
        )
        .await?;
        self.command_service = Some(std::sync::Arc::clone(&service));
        Ok(service)
    }

    /// The wired `Arc<CommandService>`, or `None` when
    /// [`wire_command_services`] has not been called (test fixtures and the
    /// degraded empty platform).
    #[allow(dead_code)]
    pub(crate) fn command_service(
        &self,
    ) -> Option<std::sync::Arc<CommandService>> {
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
    // (`kanban-builtin-probe`, `task-commands`, …). Extract each into its
    // own `plugins/<bundle-name>/` subdirectory so the host sees them as
    // first-class bundles. `Dir::extract` writes the directory's CONTENTS
    // into the target — pointed at a per-bundle subdir, that produces
    // the expected `plugins/<bundle>/<files>` layout.
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
        bundle.extract(&bundle_dir).map_err(|e| {
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
            "import {{ Plugin, makePluginThis }} from '@swissarmyhammer/plugin';\n\
             class P extends Plugin {{\n\
               async load(): Promise<void> {{\n\
                 this.log.info('{id} loaded');\n\
               }}\n\
             }}\n\
             export async function load(): Promise<unknown> {{\n\
               const p = makePluginThis(new P()) as P;\n\
               await p.load();\n\
               return null;\n\
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
        // read-only builtin layer — is live: it answers a real `kanban` call
        // through the host-exposed `kanban` server. `init board` is a real
        // kanban effect: it creates a `.kanban` board on disk.
        let result = tokio::time::timeout(
            TIMEOUT,
            platform.host().call(
                CallerId::HostInternal,
                "kanban-builtin-probe",
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
}
