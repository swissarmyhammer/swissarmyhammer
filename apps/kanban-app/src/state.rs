//! Application state management with multi-board support and MRU persistence.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_entity_search::EntitySearchIndex;
use swissarmyhammer_kanban::clipboard::ClipboardProvider;
use swissarmyhammer_kanban::commands_core::{load_yaml_dir, CommandsRegistry};
use swissarmyhammer_kanban::KanbanContext;
use swissarmyhammer_tools::mcp::unified_server::{
    start_mcp_server_with_options, McpServerHandle, McpServerMode,
};
use swissarmyhammer_ui_state::UIState;
use tauri::menu::{CheckMenuItem, MenuItem};
use tokio::sync::{Mutex as TokioMutex, RwLock};

use swissarmyhammer_kanban::actor::AddActor;
use swissarmyhammer_kanban::Execute;

use crate::plugins::PluginPlatform;
use crate::watcher;
use swissarmyhammer_entity::EntityCache;

/// XDG subdirectory for this consumer's UIState config file. Passed to
/// [`swissarmyhammer_kanban::default_ui_state`], which owns the full
/// `$XDG_CONFIG_HOME/sah/<subdir>/ui-state.yaml` resolution.
const CONFIG_APP_SUBDIR: &str = "kanban-app";

/// System clipboard provider using the Tauri clipboard plugin.
pub struct TauriClipboardProvider {
    app: tauri::AppHandle,
}

impl TauriClipboardProvider {
    /// Create a provider bound to the given Tauri `AppHandle`. The handle is
    /// used on every `read_text`/`write_text` call to reach the clipboard
    /// plugin, so the provider shares the app's lifetime.
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

#[swissarmyhammer_kanban::async_trait]
impl ClipboardProvider for TauriClipboardProvider {
    async fn write_text(&self, text: &str) -> Result<(), String> {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        self.app
            .clipboard()
            .write_text(text)
            .map_err(|e| format!("clipboard write failed: {e}"))
    }

    async fn read_text(&self) -> Result<Option<String>, String> {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        match self.app.clipboard().read_text() {
            Ok(text) => Ok(Some(text)),
            Err(e) => {
                // HACK: Tauri's clipboard plugin doesn't expose typed error variants,
                // so we fall back to string matching to distinguish "clipboard empty /
                // incompatible format" (which is normal) from real failures.  Fragile —
                // revisit if tauri-plugin-clipboard-manager ever adds structured errors.
                let msg = e.to_string();
                if msg.contains("empty") || msg.contains("format") {
                    tracing::warn!(
                        error = %e,
                        "suppressing clipboard error matched by fragile string check"
                    );
                    Ok(None)
                } else {
                    Err(format!("clipboard read failed: {e}"))
                }
            }
        }
    }
}

/// A handle to a single open kanban board.
pub(crate) struct BoardHandle {
    pub(crate) ctx: Arc<KanbanContext>,
    /// Store-level undo/redo context shared across all entity type stores.
    pub(crate) store_context: Arc<swissarmyhammer_store::StoreContext>,
    /// In-memory entity cache shared with the attached `EntityContext`.
    ///
    /// This is the single source of truth for entity state in the workspace —
    /// the same `Arc<EntityCache>` that `KanbanContext::entity_context()`
    /// attached to the context. The bridge subscribes to this cache and
    /// forwards events to Tauri; the filesystem watcher lives inside
    /// `KanbanContext` and pushes external changes through the cache too.
    pub(crate) entity_cache: Arc<EntityCache>,
    /// In-memory search index over all entities.
    pub(crate) search_index: Arc<RwLock<EntitySearchIndex>>,
    /// Handle to the bridge task that subscribes to `entity_cache` and emits
    /// Tauri events. Aborted when the handle is dropped so the bridge
    /// doesn't outlive the board.
    bridge_task: Option<tokio::task::JoinHandle<()>>,
    /// In-process MCP server exposing the full SwissArmyHammer toolset
    /// (kanban, skills/prompts, code-context, files, …) for this board.
    ///
    /// The server is rooted at the board folder, so its `kanban` tool operates
    /// on this board's `.kanban` and its skills/prompts resolve from the
    /// board's `.sah/` workspace. It binds a random loopback HTTP port; the AI
    /// backend reaches it via [`BoardHandle::mcp_url`].
    ///
    /// `Option` so the handle can be taken in `Drop` to drive an async
    /// `shutdown()`. It is always `Some` for a board returned by
    /// [`BoardHandle::open`].
    mcp_server: Option<McpServerHandle>,
    /// This board's own plugin host — a [`PluginPlatform`] with its own
    /// [`swissarmyhammer_plugin::PluginHost`], `ServerRegistry`, command
    /// registry, [`CommandService`](swissarmyhammer_command_service::CommandService),
    /// and notification bridge, isolated from every other board's host.
    ///
    /// Keyed implicitly by board path (the board is itself the
    /// [`AppState::boards`](crate::state::AppState::boards) map key), so a
    /// project plugin loaded for this board can never leak into another board's
    /// registries. Dispatch resolves the calling window's board and routes to
    /// this platform; windows with no board open fall back to the global
    /// [`AppState::plugin_platform`](crate::state::AppState::plugin_platform).
    ///
    /// `Option` so a board can still open if its per-board platform fails to
    /// build (the board then has no project plugins, but its data path is
    /// unaffected and callers fall back to the global platform). Held under
    /// `TokioMutex` to mirror the global platform — the per-board hot-reload
    /// watcher (a separate card) will mutate it after construction.
    platform: Option<TokioMutex<PluginPlatform>>,
}

impl Drop for BoardHandle {
    fn drop(&mut self) {
        if let Some(task) = self.bridge_task.take() {
            task.abort();
        }

        // Drop the per-board plugin platform OFF the Tokio worker pool. Its
        // `PluginHost` is the sole `Arc<HostInner>` owner, so dropping it runs
        // `BridgeRuntime::drop`, which does a blocking thread-`join()` to tear
        // the host's V8 isolate runtime down. `close_board` calls this drop
        // while holding the `boards` write lock from a worker, so doing the
        // blocking join inline would stall a worker (and hold the lock) across
        // runtime teardown — the same hazard the `mcp_server` shutdown below was
        // written to avoid. Hand the owned platform to the shared confinement
        // runtime to be dropped there instead.
        if let Some(platform) = self.platform.take() {
            crate::confine::drop_confined(platform);
        }

        // Shut the per-board MCP server down so closing a board never leaks a
        // bound loopback port. `McpServerHandle::shutdown` is async and `Drop`
        // is sync, so spawn the graceful shutdown onto the current Tokio
        // runtime when one is available. If no runtime is reachable (e.g. the
        // process is already tearing down), dropping the handle still fires
        // `Drop for McpServerHandle`, which best-effort sends the same
        // shutdown signal — so the server stops either way.
        if let Some(mut server) = self.mcp_server.take() {
            match tokio::runtime::Handle::try_current() {
                Ok(rt) => {
                    rt.spawn(async move {
                        if let Err(e) = server.shutdown().await {
                            tracing::warn!(error = %e, "per-board MCP server shutdown failed");
                        }
                    });
                }
                Err(_) => {
                    tracing::debug!(
                        "no Tokio runtime in BoardHandle::drop — relying on \
                         McpServerHandle::drop to signal shutdown"
                    );
                }
            }
        }
    }
}

/// Read the board's display name for MRU, falling back to the canonical path
/// string when the board is uninitialized, has no `board` entity, or the
/// entity lacks a `name` field.
async fn read_board_name(handle: &BoardHandle, canonical: &Path) -> String {
    if !handle.ctx.is_initialized() {
        return canonical.display().to_string();
    }
    let Ok(ectx) = handle.ctx.entity_context().await else {
        return canonical.display().to_string();
    };
    match ectx.read("board", "board").await {
        Ok(entity) => entity.get_str("name").unwrap_or("").to_string(),
        Err(_) => canonical.display().to_string(),
    }
}

/// Load every searchable entity (task, tag, column, actor, board) into a
/// fresh `EntitySearchIndex`.
async fn load_search_index(ctx: &KanbanContext) -> EntitySearchIndex {
    let mut all_entities: Vec<Entity> = Vec::new();
    if let Ok(ectx) = ctx.entity_context().await {
        for entity_type in &["task", "tag", "column", "actor", "board"] {
            if let Ok(entities) = ectx.list(entity_type).await {
                all_entities.extend(entities);
            }
        }
    }
    EntitySearchIndex::from_entities(all_entities)
}

/// Migrate legacy (non-FractionalIndex) task ordinals to the FractionalIndex
/// format. Reads all tasks, groups by column, sorts by existing ordinal
/// string, then assigns new FractionalIndex ordinals preserving that order.
/// No-op when all ordinals are already valid or the task list can't be read.
async fn migrate_legacy_ordinals(ectx: &swissarmyhammer_entity::EntityContext) {
    use std::collections::HashMap;
    use swissarmyhammer_kanban::types::Ordinal;

    let Ok(tasks) = ectx.list("task").await else {
        return;
    };

    let needs_migration = tasks.iter().any(|t| {
        let ord = t.get_str("position_ordinal").unwrap_or("");
        !ord.is_empty() && !Ordinal::is_valid(ord)
    });
    if !needs_migration {
        return;
    }

    tracing::info!("migrating legacy ordinals to fractional index format");

    let mut by_column: HashMap<String, Vec<Entity>> = HashMap::new();
    for t in tasks {
        let col = t.get_str("position_column").unwrap_or("todo").to_string();
        by_column.entry(col).or_default().push(t);
    }

    for column_tasks in by_column.values_mut() {
        column_tasks.sort_by(|a, b| {
            let oa = a.get_str("position_ordinal").unwrap_or("");
            let ob = b.get_str("position_ordinal").unwrap_or("");
            oa.cmp(ob)
        });
        reassign_ordinals_in_column(ectx, column_tasks).await;
    }
    tracing::info!("ordinal migration complete");
}

/// Rewrite `position_ordinal` for each task in the given column-order slice
/// to a fresh FractionalIndex sequence (`first()`, `after(first)`, …).
async fn reassign_ordinals_in_column(
    ectx: &swissarmyhammer_entity::EntityContext,
    tasks: &mut [Entity],
) {
    use swissarmyhammer_kanban::types::Ordinal;

    let mut ord = Ordinal::first();
    for task in tasks.iter_mut() {
        task.set("position_ordinal", serde_json::json!(ord.as_str()));
        if let Err(e) = ectx.write(task).await {
            tracing::warn!(id = %task.id, error = %e, "failed to migrate ordinal");
        }
        ord = Ordinal::after(&ord);
    }
}

impl BoardHandle {
    /// Create a handle with a fully-initialized context (views, fields, etc.).
    ///
    /// Does NOT start the bridge task — call `start_watcher` after the
    /// Tauri `AppHandle` is available so the bridge can emit events.
    ///
    /// `plugin_roots` carries the shared plugin-layer roots (`user_root`,
    /// `builtin_cache`) so this board can build its OWN [`PluginPlatform`]
    /// rooted at the board dir. The SOURCE of plugins is shared (the same
    /// builtin cache + user `plugins/` directory), but the host — and so its
    /// registries — is isolated per board. `None` skips the per-board platform
    /// (callers then fall back to the global one): the kanban app passes the
    /// shared roots, while plain unit-test constructors that don't exercise
    /// plugins pass `None` to avoid extracting builtins and spinning isolates.
    ///
    /// `apphandle_shells` carries the `(window, app)` shells stored on
    /// [`AppState`] at setup. When present, the per-board host exposes the
    /// `window` / `app` modules too, so its `discover_and_load_all` loads the
    /// four `AppHandle`-dependent builtin command plugins alongside the rest.
    /// `None` (e.g. a board opened before `setup_app` stored the shells, or a
    /// test that doesn't exercise those backends) leaves them unexposed.
    pub async fn open(
        kanban_path: PathBuf,
        plugin_roots: Option<&PluginRoots>,
        apphandle_shells: ApphandleShells,
    ) -> Result<Self, String> {
        ensure_sah_workspace(&kanban_path);

        let ctx = KanbanContext::open(&kanban_path)
            .await
            .map_err(|e| format!("Failed to open board context: {e}"))?;

        // INVARIANT: one `Arc<StoreContext>` per app — and therefore one
        // `undo_stack.yaml`. Every `TrackedStore` (entity-type stores,
        // perspective store, view store) registers into THIS context via
        // `Arc::clone(&store_context)`, so `store.undo` / `store.redo` revert
        // across all of them on a single LIFO stack.
        //
        // `wire_store_substrate` is the single source of truth for that
        // wiring — it constructs the one `StoreContext` and registers all
        // three store kinds into it. Never construct a second `StoreContext`
        // for the same board (here or inside the helper) — that would fork
        // the undo stack: entity edits would land on one stack, perspective
        // edits on another, and `undo` would silently revert only the one the
        // caller happened to dispatch to. The substrate guard test at
        // `apps/kanban-app/tests/substrate_guard.rs` calls the SAME helper and
        // `Arc::ptr_eq`-compares the context each subsystem holds against the
        // returned one; if the helper ever splits the substrate, that test
        // fails loudly.
        let store_context = swissarmyhammer_kanban::wire_store_substrate(&ctx).await;

        // Ensure the entity context is initialized — this also constructs
        // and attaches the `EntityCache`. After this call,
        // `ctx.entity_cache()` returns the same `Arc<EntityCache>` that
        // owns the live state.
        let ectx = ctx
            .entity_context()
            .await
            .map_err(|e| format!("Failed to initialize entity context: {e}"))?;

        migrate_legacy_ordinals(&ectx).await;

        let entity_cache = ctx
            .entity_cache()
            .expect("entity_cache must be populated after entity_context() returns Ok");

        // Spawn the filesystem watcher explicitly — long-running processes
        // want to see external edits propagate through the cache. One-shot
        // callers (MCP, CLI, tests) intentionally skip this because
        // building an FSEvents watcher on macOS costs hundreds of ms.
        if let Err(e) = ctx.start_watcher() {
            tracing::warn!(error = %e, "failed to spawn kanban filesystem watcher");
        }

        let search_index = Arc::new(RwLock::new(load_search_index(&ctx).await));

        let mcp_server = start_board_mcp_server(&kanban_path).await;

        // Build this board's OWN plugin host, rooted at the board dir, from the
        // shared builtin cache + user plugin layer. Mirrors the global platform
        // wiring (build → wire_command_services → expose window/app → discover),
        // so a per-board host carries the same builtin command baseline as the
        // global one — only the registries are isolated. `project_root` is
        // intentionally left `None` here (a follow-up card wires it to the board
        // dir); the structure is the deliverable.
        let platform = match plugin_roots {
            Some(roots) => build_board_platform(roots, &kanban_path, apphandle_shells).await,
            None => None,
        };

        Ok(Self {
            ctx: Arc::new(ctx),
            store_context,
            entity_cache,
            search_index,
            bridge_task: None,
            mcp_server,
            platform: platform.map(TokioMutex::new),
        })
    }

    /// This board's per-board plugin platform, or `None` when the board was
    /// opened without shared plugin roots or its platform failed to build.
    ///
    /// Dispatch resolves the calling window's board and reads this platform's
    /// `command_service()` / `host()`; a `None` here (or no board for the
    /// window) means the caller falls back to the global platform.
    pub(crate) fn platform(&self) -> Option<&TokioMutex<PluginPlatform>> {
        self.platform.as_ref()
    }

    /// The board's in-process MCP server URL, e.g.
    /// `http://127.0.0.1:<port>/mcp`.
    ///
    /// The AI backend hands this URL to the in-process agent so the agent
    /// gets the full SwissArmyHammer toolset scoped to this board. Returns
    /// `None` only if the server failed to start at board-open time.
    ///
    /// `#[allow(dead_code)]`: this accessor is the exposure point the AI
    /// backend will read, but the call site lives in the follow-up task
    /// `01KRRN3SP5D1H63TQ8HM7SQZ1F` that wires `ai_start_agent` to consume
    /// this URL. The board-lifecycle integration test already exercises it.
    /// This mirrors the `#![allow(dead_code)]` rationale on `ai/mod.rs`.
    #[allow(dead_code)]
    pub fn mcp_url(&self) -> Option<&str> {
        self.mcp_server.as_ref().map(|s| s.url())
    }

    /// Ensure an actor entity exists for the current OS user.
    ///
    /// Uses `whoami` to detect username/realname, derives a deterministic color,
    /// and generates an initials-based SVG avatar. Idempotent via `ensure: true`.
    pub async fn ensure_os_actor(&self) {
        let username = whoami::username();
        let realname = whoami::realname();
        let color = deterministic_color(&username);

        let mut cmd = AddActor::new(username.as_str(), realname.as_str())
            .with_ensure()
            .with_color(&color);

        // Profile picture lookup involves synchronous file I/O and may
        // shell out to `dscl` on macOS — run on the blocking thread pool
        // to avoid stalling the async executor.
        let uname = username.clone();
        let photo = tokio::task::spawn_blocking(move || macos_profile_picture(&uname))
            .await
            .ok()
            .flatten();
        if let Some(photo) = photo {
            cmd = cmd.with_avatar(photo);
        }

        match cmd.execute(&self.ctx).await.into_result() {
            Ok(result) => {
                let created = result["created"].as_bool().unwrap_or(false);
                if created {
                    tracing::info!(id = %username, name = %realname, "created OS user actor");
                } else {
                    tracing::debug!(id = %username, "OS user actor already exists");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to ensure OS user actor");
            }
        }
    }

    /// Start the entity-cache → Tauri bridge for this board.
    ///
    /// Spawns a background task that subscribes to the entity cache and
    /// forwards every event as a Tauri emit scoped to this board's path.
    /// Idempotent: calling twice replaces the previous bridge task.
    pub fn start_watcher(&mut self, app_handle: tauri::AppHandle) {
        if let Some(task) = self.bridge_task.take() {
            task.abort();
        }
        let kanban_root = self.ctx.root().to_path_buf();
        let ctx = Arc::clone(&self.ctx);
        let cache = Arc::clone(&self.entity_cache);
        let search_index = Arc::clone(&self.search_index);
        let board_path_str = kanban_root.display().to_string();

        // Subscribe to the perspective broadcast channel so the bridge can
        // forward perspective mutations as Tauri events.
        //
        // Invariant: `start_watcher` must be called after `wire_store_substrate`
        // has initialized the PerspectiveContext (via `perspective_context().await`)
        // and before any concurrent perspective writes begin. This ensures:
        //   1. `perspective_context_if_ready()` returns `Some` (context is initialized)
        //   2. `try_read()` succeeds (no write lock held during startup)
        //
        // `try_read()` is used instead of `.read().await` because `start_watcher`
        // is a synchronous function. The lock is only held momentarily to call
        // `subscribe()` — the guard is dropped immediately.
        let perspective_rx = ctx
            .perspective_context_if_ready()
            .and_then(|pctx| pctx.try_read().ok().map(|guard| guard.subscribe()));

        // Same idea for views — `views()` is the non-initializing accessor
        // (the ViewsContext is eagerly constructed in `KanbanContext::open`),
        // so a `try_read` here is safe at startup.
        let view_rx = ctx
            .views()
            .and_then(|views_lock| views_lock.try_read().ok().map(|guard| guard.subscribe()));

        tracing::info!(
            path = %kanban_root.display(),
            has_perspective_rx = perspective_rx.is_some(),
            has_view_rx = view_rx.is_some(),
            "entity-cache bridge starting for board"
        );
        let handle = tokio::spawn(watcher::run_bridge(
            ctx,
            cache,
            app_handle,
            board_path_str,
            search_index,
            perspective_rx,
            view_rx,
        ));
        self.bridge_task = Some(handle);
    }
}

/// A handle to a native menu item, wrapping both regular and check menu items.
///
/// Used to call `set_enabled()` without knowing which concrete type was created.
pub(crate) enum MenuItemHandle {
    Regular(MenuItem<tauri::Wry>),
    Check(CheckMenuItem<tauri::Wry>),
}

impl MenuItemHandle {
    /// Enable or disable this menu item.
    pub(crate) fn set_enabled(&self, enabled: bool) -> tauri::Result<()> {
        match self {
            Self::Regular(item) => item.set_enabled(enabled),
            Self::Check(item) => item.set_enabled(enabled),
        }
    }

    pub(crate) fn set_text(&self, text: &str) -> tauri::Result<()> {
        match self {
            Self::Regular(item) => item.set_text(text),
            Self::Check(item) => item.set_text(text),
        }
    }
}

/// The shared application state, managed by Tauri.
pub(crate) struct AppState {
    pub(crate) boards: RwLock<HashMap<PathBuf, Arc<BoardHandle>>>,
    /// Shared UI state (inspector stack, palette, keymap, drag session, etc.).
    pub(crate) ui_state: Arc<UIState>,
    /// YAML-loaded command definitions. Behind RwLock because user overrides
    /// are merged when switching boards.
    pub(crate) commands_registry: RwLock<CommandsRegistry>,
    /// Cached menu item handles keyed by command ID. Populated when the menu
    /// is built from the command registry, used by `update_menu_enabled_state`
    /// to toggle enabled/disabled without a full menu rebuild.
    pub(crate) menu_items: Mutex<HashMap<String, MenuItemHandle>>,
    /// Set to `true` when the app is shutting down (RunEvent::ExitRequested).
    /// The Destroyed handler uses this to distinguish mid-session close from app quit.
    pub(crate) shutting_down: AtomicBool,
    /// Set to `true` as soon as a `kanban://open/...` deep-link URL is
    /// recognized during cold-start setup. `auto_open_board` reads this flag
    /// and skips session restore when it's set — the user explicitly asked
    /// for a specific board, which must win over whatever was open
    /// previously. `restore_session_windows` also consults it to avoid
    /// resurrecting previous-session windows on top of the one the deep-link
    /// handler focused or created.
    pub(crate) deep_link_handled: AtomicBool,
    // `spatial_registry` and `spatial_state` were REMOVED in Stage 3
    // of the kanban cut-over. The spatial Tauri commands that mutated
    // them are gone; the React side now reaches the focus kernel
    // through the in-process `focus` MCP server, which owns its own
    // `Arc<Mutex<SpatialRegistry>>` / `Arc<Mutex<SpatialState>>` (see
    // `command_services.rs`).
    /// The embedded plugin platform — the [`swissarmyhammer_plugin::PluginHost`]
    /// loaded with the builtin and user-layer plugins, plus the hot-reload
    /// watcher. Joins `commands_registry`, `ui_state`, and `boards` as another
    /// piece of shared application context. Held under `tokio::sync::Mutex`
    /// because the hot-reload watcher is started after construction (from the
    /// Tauri `setup` hook, via [`Self::start_plugin_watcher`]), which mutates
    /// the platform.
    pub(crate) plugin_platform: TokioMutex<PluginPlatform>,
    /// Shared plugin-layer roots used to build each board's OWN
    /// [`PluginPlatform`] at board-open time (see
    /// [`BoardHandle::platform`]). The builtin cache + user `plugins/`
    /// directory are the same for every board — the SOURCE is shared — so the
    /// per-board hosts only isolate the registries, not the plugin files.
    ///
    /// `None` when the directories couldn't be resolved or for unit-test
    /// constructors that don't exercise plugins; boards opened then have no
    /// per-board platform and dispatch falls back to the global one.
    plugin_roots: Option<PluginRoots>,
    /// The `AppHandle`-backed shells (`window` + `app`) the plugin hosts expose
    /// as the `window` / `app` MCP modules.
    ///
    /// Both seams need a live Tauri `AppHandle`, which does not exist at
    /// `AppState::new`; they are constructed and stored here from the
    /// `setup_app` hook (via [`Self::install_apphandle_shells`]) before the
    /// global host's deferred discovery runs. Each per-board host built at
    /// board-open time reads these back so its `discover_and_load_all` also
    /// loads the four `AppHandle`-dependent builtin command plugins. `OnceLock`
    /// because they are written exactly once at setup and read concurrently
    /// from board-open thereafter.
    window_shell: std::sync::OnceLock<Arc<dyn swissarmyhammer_window_service::WindowShell>>,
    app_shell: std::sync::OnceLock<Arc<dyn swissarmyhammer_app_service::AppShell>>,
    /// Running in-process AI agent endpoints, keyed by board path.
    ///
    /// `ai_start_agent` registers one endpoint per board here; `close_board`
    /// stops the matching endpoint and app teardown stops all of them, so an
    /// agent's WebSocket server never outlives its board or the process.
    pub(crate) running_agents: crate::ai::models::RunningAgents,
}

impl AppState {
    /// Create a new AppState, loading config from disk.
    ///
    /// Delegates UI state loading to
    /// [`swissarmyhammer_kanban::default_ui_state`] (which resolves the
    /// XDG config path and reads the YAML, or seeds defaults). The
    /// builtin command stack is composed at this app layer via
    /// [`swissarmyhammer_kanban::compose_registry!`] over the
    /// contributor crates the app pulls in. This struct does not know
    /// the config file format or the default path — it just wires the
    /// pieces together.
    ///
    /// Async because it constructs the embedded plugin platform — the
    /// [`swissarmyhammer_plugin::PluginHost`] loaded with the builtin and
    /// user-layer plugins. The user layer is `$XDG_CONFIG_HOME/kanban/plugins`
    /// (resolved via [`swissarmyhammer_directory::KanbanConfig`]); the builtin
    /// layer is the bundle tree compiled into the binary.
    pub async fn new() -> Self {
        let ui_state = Arc::new(swissarmyhammer_kanban::default_ui_state(CONFIG_APP_SUBDIR));

        // Resolve the shared plugin-layer roots ONCE. The same builtin cache +
        // user `plugins/` directory feed both the global platform and every
        // per-board platform, so the SOURCE of plugins is shared even though
        // each host's registries are isolated.
        let plugin_roots = PluginRoots::resolve(Arc::clone(&ui_state));
        let mut platform = build_plugin_platform(plugin_roots.as_ref()).await;

        // Production wiring: expose every NON-`AppHandle` command-service MCP
        // module (`store`, `entity`, `views`, `ui_state`, `focus`, `commands`)
        // on the host now. The `window` / `app` modules are deferred to the
        // Tauri `setup_app` hook, where the `AppHandle` (and so the
        // `WindowShell` / `AppShell` seams) exists.
        //
        // Plugin discovery is INTENTIONALLY deferred too: `discover_and_load_all`
        // is atomic, and four of the seven builtin command plugins activate the
        // `window` / `app` backends at `ensureServices` time — discovering here
        // (before those backends are exposed) would fail ALL seven. Discovery
        // is driven once from `setup_app` after the shells are wired. A failure
        // to wire degrades to running without the new dispatch path.
        if let Err(e) = platform
            .wire_command_services(Arc::clone(&ui_state), None, None)
            .await
        {
            tracing::warn!(error = %e, "failed to wire command-service modules; \
                                        new dispatch path will not be available");
        }

        Self::with_ui_state(ui_state, platform, plugin_roots)
    }

    /// Create AppState with a freshly loaded UIState written to a
    /// per-test temp path, so unit tests don't clobber the developer's
    /// real config. Still goes through [`UIState::load`] to exercise the
    /// same loader the production path uses.
    ///
    /// Synchronous, and the plugin platform it wires is the empty
    /// [`PluginPlatform::for_tests_empty`] — the kanban app's plain `#[test]`
    /// units (drag sessions, MRU, path resolution) do not exercise plugins.
    /// Tests that *do* drive the plugin platform use
    /// [`new_for_test_with_plugins`](Self::new_for_test_with_plugins).
    #[cfg(test)]
    pub fn new_for_test() -> Self {
        let path = std::env::temp_dir().join(format!("kanban-test-{}.yaml", ulid::Ulid::new()));
        // No shared plugin roots: plain unit tests don't open real boards, and
        // boards opened without roots skip the per-board platform.
        Self::with_ui_state(
            Arc::new(UIState::load(path)),
            PluginPlatform::for_tests_empty(),
            None,
        )
    }

    /// Create AppState whose plugin platform is built over caller-supplied
    /// temp roots, so a `#[tokio::test]` can exercise the real plugin host
    /// without touching the developer's `~/.config/kanban`.
    ///
    /// # Parameters
    ///
    /// - `user_root` — the temp `~/.config/kanban` equivalent; the user plugin
    ///   layer is its `plugins/` subdirectory.
    /// - `builtin_cache` — the temp directory the bundled builtin plugins are
    ///   extracted into.
    /// - `tool_working_dir` — the working directory the exposed `kanban` tool
    ///   resolves its `.kanban` board against.
    #[cfg(test)]
    pub async fn new_for_test_with_plugins(
        user_root: PathBuf,
        builtin_cache: PathBuf,
        tool_working_dir: PathBuf,
    ) -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!("kanban-test-{}.yaml", ulid::Ulid::new()));
        let ui_state = Arc::new(UIState::load(path));
        let mut platform =
            PluginPlatform::build(user_root.clone(), builtin_cache.clone(), tool_working_dir)
                .await?;
        // Mirror production: wire the non-AppHandle command-service modules,
        // then expose `window` / `app` from SPY shells, then discover — so the
        // bundled command plugins (which `ensureServices` against those modules
        // at `load()`) find every backend already exposed and the atomic
        // `discover_and_load_all` loads ALL 7 builtin command plugins. Spy
        // shells stand in for the Tauri-`AppHandle`-backed production shells,
        // which can't be built in a headless test.
        let window_shell: Arc<dyn swissarmyhammer_window_service::WindowShell> =
            Arc::new(tests::SpyWindowShell);
        let app_shell: Arc<dyn swissarmyhammer_app_service::AppShell> =
            Arc::new(tests::SpyAppShell);
        platform
            .wire_command_services(
                Arc::clone(&ui_state),
                Some(Arc::clone(&window_shell)),
                Some(Arc::clone(&app_shell)),
            )
            .await?;
        platform.discover_plugins().await?;
        // Stash the same shared roots so boards opened on this AppState build
        // their own per-board platforms (the per-board-host integration tests
        // rely on this).
        let plugin_roots = Some(PluginRoots {
            user_root,
            builtin_cache,
            ui_state: Arc::clone(&ui_state),
        });
        let state = Self::with_ui_state(ui_state, platform, plugin_roots);
        // Store the spy shells so boards opened on this AppState wire `window` /
        // `app` into their per-board hosts too (mirrors `install_apphandle_shells`).
        let _ = state.window_shell.set(window_shell);
        let _ = state.app_shell.set(app_shell);
        Ok(state)
    }

    /// Internal constructor that takes an already-loaded [`UIState`] and a
    /// fully-built [`PluginPlatform`].
    ///
    /// Every other constructor funnels through here so the wiring (MRU,
    /// window bookkeeping, command registry, plugin platform) sits in exactly
    /// one place. The command registry is composed via
    /// [`swissarmyhammer_kanban::compose_registry!`] over the
    /// contributor crates this app pulls in (generic UI commands from
    /// `swissarmyhammer_commands`, then domain commands from
    /// `swissarmyhammer_kanban`). User overrides from `.kanban/commands/`
    /// layer on top later via [`Self::reload_command_overrides`]. The plugin
    /// platform is built by the caller because its construction is async,
    /// while this funnel stays synchronous.
    fn with_ui_state(
        ui_state: Arc<UIState>,
        plugin_platform: PluginPlatform,
        plugin_roots: Option<PluginRoots>,
    ) -> Self {
        Self {
            boards: RwLock::new(HashMap::new()),
            ui_state,
            // Stage 4 cut-over: the YAML-driven registry is now empty by
            // construction — every `builtin_yaml_sources()` returns `Vec::new()`
            // because `CommandService` (fed by the 7 builtin command plugins
            // at app startup) is the sole source of command metadata. The
            // registry is retained as a synchronous façade for legacy menu /
            // scope-resolution callers while they migrate to the MCP path.
            commands_registry: RwLock::new(swissarmyhammer_kanban::compose_registry![
                swissarmyhammer_focus,
                swissarmyhammer_kanban,
            ]),
            menu_items: Mutex::new(HashMap::new()),
            shutting_down: AtomicBool::new(false),
            deep_link_handled: AtomicBool::new(false),
            plugin_platform: TokioMutex::new(plugin_platform),
            plugin_roots,
            window_shell: std::sync::OnceLock::new(),
            app_shell: std::sync::OnceLock::new(),
            running_agents: crate::ai::models::RunningAgents::new(),
        }
    }

    /// Store the `AppHandle`-backed shells, expose `window` + `app` on the
    /// global plugin host, then run the global host's deferred plugin discovery.
    ///
    /// Call ONCE from the Tauri `setup_app` hook, after the `AppHandle` exists
    /// and BEFORE [`Self::auto_open_board`] (so the stored shells are available
    /// when each per-board host is built at board-open time) and before
    /// [`Self::start_plugin_watcher`].
    ///
    /// This is where the global fallback host finally loads all 7 builtin
    /// command plugins: `AppState::new` deferred discovery because four of them
    /// need the `window` / `app` backends, which only exist now. Idempotent on
    /// the shell storage (the `OnceLock`s only accept the first write); a
    /// discovery failure is logged, not propagated, so a broken plugin layer
    /// never blocks the app from starting.
    pub async fn install_apphandle_shells(
        &self,
        window_shell: Arc<dyn swissarmyhammer_window_service::WindowShell>,
        app_shell: Arc<dyn swissarmyhammer_app_service::AppShell>,
    ) {
        let _ = self.window_shell.set(Arc::clone(&window_shell));
        let _ = self.app_shell.set(Arc::clone(&app_shell));

        let platform = self.plugin_platform.lock().await;
        if let Err(e) = platform
            .expose_apphandle_modules(Some(window_shell), Some(app_shell))
            .await
        {
            tracing::warn!(error = %e, "failed to expose window/app modules on the global host; \
                                        AppHandle-dependent builtin command plugins will not load");
        }
        if let Err(e) = platform.discover_plugins().await {
            tracing::warn!(error = %e, "global plugin discovery failed; \
                                        builtin and user-layer plugins are not loaded");
        }
    }

    /// The stored `AppHandle`-backed shells, if [`Self::install_apphandle_shells`]
    /// has run. Returns `(window, app)` clones for a per-board host build to
    /// expose the same `window` / `app` backends the global host carries.
    fn apphandle_shells(&self) -> ApphandleShells {
        let window = self.window_shell.get()?;
        let app = self.app_shell.get()?;
        Some((Arc::clone(window), Arc::clone(app)))
    }

    /// Start the plugin hot-reload watcher.
    ///
    /// Call this from the Tauri `setup` hook alongside [`Self::start_watchers`]
    /// once the `AppHandle` is available. The watcher reacts to plugin files
    /// changing under `$XDG_CONFIG_HOME/kanban/plugins`. Idempotent.
    pub async fn start_plugin_watcher(&self) {
        self.plugin_platform.lock().await.start_watcher().await;
    }

    /// Open a board at the given path, resolving to its .kanban directory.
    /// Returns the canonical path used as the map key.
    ///
    /// If `app_handle` is provided, starts a file watcher that emits
    /// entity-level events when files change externally.
    pub async fn open_board(
        &self,
        path: &Path,
        app_handle: Option<tauri::AppHandle>,
    ) -> Result<PathBuf, String> {
        tracing::info!("Opening board at {}", path.display());
        let kanban_path = resolve_kanban_path(path).map_err(|e| e.to_string())?;

        let canonical = kanban_path
            .canonicalize()
            .unwrap_or_else(|_| kanban_path.clone());

        if self.touch_if_already_open(&canonical).await {
            return Ok(canonical);
        }

        let mut handle = BoardHandle::open(
            kanban_path,
            self.plugin_roots.as_ref(),
            self.apphandle_shells(),
        )
        .await?;
        handle.ensure_os_actor().await;
        let board_name = read_board_name(&handle, &canonical).await;

        // Start the file watcher on the owned handle BEFORE wrapping in Arc
        // and inserting into the map. Avoids a TOCTOU race where a concurrent
        // Tauri command could clone the Arc between insert and Arc::get_mut,
        // silently preventing the watcher from starting.
        if let Some(ref app) = app_handle {
            handle.start_watcher(app.clone());
        }

        self.register_open_board(&canonical, handle, &board_name)
            .await;
        self.reload_command_overrides(&canonical).await;
        self.sync_undo_redo_state(&canonical).await;

        Ok(canonical)
    }

    /// If the board is already open, bump its MRU position and return `true`.
    async fn touch_if_already_open(&self, canonical: &Path) -> bool {
        let boards = self.boards.read().await;
        if boards.contains_key(canonical) {
            self.ui_state
                .set_most_recent_board(&canonical.display().to_string());
            true
        } else {
            false
        }
    }

    /// Insert the newly-opened board into the map and update UIState MRU
    /// tracking. The watcher may already be emitting events, but the frontend
    /// won't see them until `list_open_boards` returns this board.
    async fn register_open_board(&self, canonical: &Path, handle: BoardHandle, board_name: &str) {
        {
            let mut boards = self.boards.write().await;
            boards.insert(canonical.to_path_buf(), Arc::new(handle));
        }
        let canonical_str = canonical.display().to_string();
        self.ui_state.touch_recent(&canonical_str, board_name);
        // Track as most recently used board so quick capture and commands
        // without an explicit `board_path` default to this board.
        self.ui_state.set_most_recent_board(&canonical_str);
        self.ui_state.add_open_board(&canonical_str);
    }

    /// Sync UIState undo/redo flags from the newly opened board's
    /// `StoreContext` so menu items reflect correct enabled state from the
    /// start.
    async fn sync_undo_redo_state(&self, canonical: &Path) {
        let boards = self.boards.read().await;
        if let Some(handle) = boards.get(canonical) {
            self.ui_state.set_undo_redo_state(
                handle.store_context.can_undo().await,
                handle.store_context.can_redo().await,
            );
        }
    }

    /// Auto-open a board at startup by walking up from CWD looking for a `.kanban` directory.
    ///
    /// If no `.kanban` directory is found in any ancestor, the app starts without
    /// a board (the frontend shows the "No board loaded" prompt).
    pub async fn auto_open_board(&self) {
        // If a deep-link URL was already handled during setup, the user
        // explicitly asked for a specific board — skip session restore and
        // filesystem discovery entirely so we don't resurrect the previous
        // session on top of their choice.
        if self
            .deep_link_handled
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            tracing::info!("auto_open_board: skipping — deep link handled");
            return;
        }

        self.restore_persisted_boards().await;
        self.restore_window_boards().await;

        // If we restored boards, we're done — don't override with CWD discovery.
        {
            let boards = self.boards.read().await;
            if !boards.is_empty() {
                tracing::info!(
                    count = boards.len(),
                    "auto_open_board: restored {} board(s) from config",
                    boards.len()
                );
                return;
            }
        }

        let board_dir = self.discover_board_from_environment();
        match board_dir {
            Some(ref dir) => {
                tracing::info!(path = %dir.display(), "auto_open_board: opening board");
                if let Err(e) = self.open_board(dir, None).await {
                    tracing::warn!(
                        path = %dir.display(),
                        error = %e,
                        "auto_open_board: failed to open board"
                    );
                }
            }
            None => {
                tracing::info!("auto_open_board: no board found, starting without one");
            }
        }
    }

    /// Restore previously-open boards from UIState's `open_boards` list.
    ///
    /// Removes stale entries (empty paths, directories that no longer exist)
    /// and opens all valid board paths.
    async fn restore_persisted_boards(&self) {
        let paths: Vec<PathBuf> = self
            .ui_state
            .open_boards()
            .into_iter()
            .map(PathBuf::from)
            .collect();
        for path in paths {
            if path.as_os_str().is_empty() {
                tracing::warn!("auto_open_board: removing entry with empty path from config");
                self.ui_state.remove_open_board("");
                continue;
            }
            if path.is_dir() {
                tracing::info!(path = %path.display(), "auto_open_board: restoring persisted board");
                if let Err(e) = self.open_board(&path, None).await {
                    tracing::warn!(path = %path.display(), error = %e, "auto_open_board: failed to restore board");
                }
            } else {
                tracing::info!(path = %path.display(), "auto_open_board: persisted board no longer exists, removing");
                self.ui_state.remove_open_board(&path.display().to_string());
            }
        }
    }

    /// Open boards referenced in UIState `window_boards` that aren't already open.
    ///
    /// Handles the case where a secondary window shows a different board
    /// than the ones in `open_boards`.
    async fn restore_window_boards(&self) {
        let wb_paths: Vec<PathBuf> = self
            .ui_state
            .all_window_boards()
            .into_values()
            .map(PathBuf::from)
            .collect();

        let boards = self.boards.read().await;
        let already_open: HashSet<PathBuf> = boards.keys().cloned().collect();
        drop(boards);

        for path in wb_paths {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            if already_open.contains(&canonical) {
                continue;
            }
            if path.is_dir() {
                tracing::info!(path = %path.display(), "auto_open_board: restoring board from UIState window_boards");
                if let Err(e) = self.open_board(&path, None).await {
                    tracing::warn!(path = %path.display(), error = %e, "auto_open_board: failed to restore windows board");
                }
            }
        }
    }

    /// Discover a board from CWD, home directory, or MRU history.
    ///
    /// Tries three strategies in order: (1) walk up from CWD, (2) check
    /// home directory as backstop, (3) fall back to the most recently
    /// used board from config.
    fn discover_board_from_environment(&self) -> Option<PathBuf> {
        let cwd = match std::env::current_dir() {
            Ok(dir) => dir,
            Err(e) => {
                tracing::warn!("Cannot determine current directory: {e}");
                return None;
            }
        };
        tracing::info!(cwd = %cwd.display(), "auto_open_board: starting discovery");

        // Strategy 1: walk up from CWD
        if let Some(dir) = discover_board(&cwd) {
            tracing::info!(path = %dir.display(), "auto_open_board: found .kanban via CWD walk");
            return Some(dir);
        }
        tracing::info!("auto_open_board: no .kanban found walking up from CWD");

        // Strategy 2: if CWD walk didn't pass through home, check home as backstop
        if let Some(dir) = try_home_backstop(&cwd) {
            return Some(dir);
        }

        // Strategy 3: fall back to MRU — the most recently opened board
        try_mru_fallback(&self.ui_state)
    }

    /// Rebuild the commands registry from builtins + user overrides from the
    /// active board's `.kanban/commands/` directory.
    ///
    /// Composes the builtin stack via
    /// [`swissarmyhammer_kanban::compose_yaml_sources!`] (same crate
    /// list and order as [`Self::with_ui_state`]) and appends the user
    /// overrides last so they take precedence by command id.
    async fn reload_command_overrides(&self, kanban_path: &Path) {
        let commands_dir = kanban_path.join("commands");
        let user_sources = load_yaml_dir(&commands_dir);
        if user_sources.is_empty() {
            return;
        }

        let count = user_sources.len();

        let mut sources = swissarmyhammer_kanban::compose_yaml_sources![
            swissarmyhammer_focus,
            swissarmyhammer_kanban,
        ];
        // User overrides apply last with partial-merge-by-id semantics.
        let user_refs: Vec<(&str, &str)> = user_sources
            .iter()
            .map(|(n, c)| (n.as_str(), c.as_str()))
            .collect();
        sources.extend(user_refs);
        let registry =
            swissarmyhammer_kanban::commands_core::CommandsRegistry::from_yaml_sources(&sources);

        *self.commands_registry.write().await = registry;
        tracing::info!(
            dir = %commands_dir.display(),
            count,
            "loaded user command overrides",
        );
    }

    /// Start file watchers for all open boards that don't have one yet.
    ///
    /// Call this from Tauri `setup` after the AppHandle is available,
    /// since boards opened during `auto_open_board` (before Tauri) don't
    /// have watchers.
    pub async fn start_watchers(&self, app_handle: tauri::AppHandle) {
        let mut boards = self.boards.write().await;
        let keys: Vec<PathBuf> = boards.keys().cloned().collect();
        for key in keys {
            if let Some(handle) = boards.get_mut(&key) {
                if let Some(handle_mut) = Arc::get_mut(handle) {
                    handle_mut.start_watcher(app_handle.clone());
                }
            }
        }
    }

    /// Close a board, removing it from the open set.
    ///
    /// If the closed board was the active board, switches to another open board
    /// (if any) or sets active to None.
    pub async fn close_board(&self, path: &Path) -> Result<(), String> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        {
            let mut boards = self.boards.write().await;
            if boards.remove(&canonical).is_none() {
                return Err(format!("Board not open: {}", canonical.display()));
            }

            // If we just closed the most recent board, switch to another one.
            if self.ui_state.most_recent_board().as_deref()
                == Some(&canonical.display().to_string())
            {
                if let Some(next) = boards.keys().next() {
                    self.ui_state
                        .set_most_recent_board(&next.display().to_string());
                }
            }
        }

        // Update UIState board tracking so it stays in sync.
        self.ui_state
            .remove_open_board(&canonical.display().to_string());

        // Stop the board's in-process AI agent endpoint, if one was started
        // via `ai_start_agent`, so its WebSocket server does not outlive the
        // board.
        self.running_agents.stop(&canonical).await;

        tracing::info!(path = %canonical.display(), "closed board");
        Ok(())
    }

    /// Get the handle for the most recently focused board.
    pub async fn active_handle(&self) -> Option<Arc<BoardHandle>> {
        let path_str = self.ui_state.most_recent_board()?;
        let path = PathBuf::from(&path_str)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&path_str));
        let boards = self.boards.read().await;
        boards.get(&path).cloned()
    }

    /// Resolve the calling window's board handle from its label.
    ///
    /// Maps `label` → board path via the per-window assignment persisted in
    /// [`UIState`](swissarmyhammer_ui_state::UIState) (`window_board`), then
    /// canonicalizes that path the same way [`open_board`](Self::open_board)
    /// keys the `boards` map and looks it up. Returns `None` for a boardless
    /// window (no assignment) or an unknown label — the caller then falls back
    /// to the global plugin platform.
    pub(crate) async fn board_handle_for_window(&self, label: &str) -> Option<Arc<BoardHandle>> {
        let path_str = self.ui_state.window_board(label)?;
        let canonical = PathBuf::from(&path_str)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&path_str));
        let boards = self.boards.read().await;
        boards.get(&canonical).cloned()
    }
}

/// The shared plugin-layer roots used to build every [`PluginPlatform`] in the
/// app — the one global platform on [`AppState`] and each board's own platform
/// on [`BoardHandle`].
///
/// The user plugin layer lives under `$XDG_CONFIG_HOME/kanban` and the builtin
/// plugins are extracted into the XDG cache directory; both are SHARED across
/// boards (the per-board hosts only isolate their registries, not the plugin
/// files). `ui_state` is carried so a per-board platform can wire its own
/// command-service modules against the same shared UI state.
#[derive(Clone)]
pub(crate) struct PluginRoots {
    /// The writable user-layer plugin root (`$XDG_CONFIG_HOME/kanban`).
    user_root: PathBuf,
    /// The directory the bundled builtin plugins are extracted into; it becomes
    /// each host's read-only builtin layer root.
    builtin_cache: PathBuf,
    /// Shared UI state, threaded into each per-board platform's
    /// `wire_command_services` call.
    ui_state: Arc<UIState>,
}

impl PluginRoots {
    /// Resolve the shared plugin-layer roots from the XDG directories.
    ///
    /// Returns `None` when either directory can't be resolved — plugins are
    /// then disabled app-wide (no global platform built from real roots, and
    /// boards open without per-board platforms), but board data still works.
    fn resolve(ui_state: Arc<UIState>) -> Option<Self> {
        use swissarmyhammer_directory::{KanbanConfig, ManagedDirectory};

        let user_root = match ManagedDirectory::<KanbanConfig>::xdg_config() {
            Ok(dir) => dir.root().to_path_buf(),
            Err(e) => {
                tracing::warn!(error = %e, "could not resolve kanban config dir; plugins disabled");
                return None;
            }
        };
        let builtin_cache = match ManagedDirectory::<KanbanConfig>::xdg_cache() {
            Ok(dir) => dir.root().join("builtin-plugins"),
            Err(e) => {
                tracing::warn!(error = %e, "could not resolve kanban cache dir; plugins disabled");
                return None;
            }
        };
        Some(Self {
            user_root,
            builtin_cache,
            ui_state,
        })
    }
}

/// Builds the production global plugin platform, falling back to an inert one.
///
/// The global platform is the fallback host for windows with no board open. It
/// is rooted at the shared `user_root` + `builtin_cache` (from `roots`) with
/// the exposed `kanban` tool resolving its board against the current working
/// directory — the same directory `auto_open_board` discovers boards from.
///
/// A failure to resolve the directories (`roots` is `None`) or to load a plugin
/// is logged and the app continues with an empty [`PluginPlatform`]: a broken
/// plugin layer must not stop the kanban app from opening boards.
async fn build_plugin_platform(roots: Option<&PluginRoots>) -> PluginPlatform {
    let Some(roots) = roots else {
        return PluginPlatform::empty(std::env::temp_dir().join("kanban-plugins-disabled"));
    };
    let tool_working_dir = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());

    match PluginPlatform::build(
        roots.user_root.clone(),
        roots.builtin_cache.clone(),
        tool_working_dir,
    )
    .await
    {
        Ok(platform) => {
            tracing::info!(user_root = %roots.user_root.display(), "kanban plugin platform ready");
            platform
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to build kanban plugin platform; plugins disabled");
            PluginPlatform::empty(roots.user_root.clone())
        }
    }
}

/// The `(window, app)` shells a per-board host exposes as the `window` / `app`
/// MCP modules. `None` when [`AppState::install_apphandle_shells`] has not run
/// yet (a board opened before setup) — the per-board host then loads only the
/// non-`AppHandle` builtin plugins.
type ApphandleShells = Option<(
    Arc<dyn swissarmyhammer_window_service::WindowShell>,
    Arc<dyn swissarmyhammer_app_service::AppShell>,
)>;

/// Build a board's OWN plugin platform, rooted at the board dir, from the
/// shared plugin roots.
///
/// Mirrors the global wiring: `build` → `wire_command_services` → expose
/// `window` / `app` (from `apphandle_shells`) → `discover_plugins`. The host is
/// therefore identical in capability to the global one; only its registries are
/// isolated. `tool_working_dir` is the board dir (the parent of
/// `<board>/.kanban`), so this host's exposed `kanban` tool resolves THIS board.
///
/// `project_root` is intentionally NOT set to the board dir yet — that is the
/// next card. Returns `None` (logged) on any failure so a broken plugin layer
/// never blocks opening the board; the caller then falls back to the global
/// platform for this board's windows.
async fn build_board_platform(
    roots: &PluginRoots,
    kanban_path: &Path,
    apphandle_shells: ApphandleShells,
) -> Option<PluginPlatform> {
    let board_dir = kanban_path.parent().unwrap_or(kanban_path).to_path_buf();
    let roots = roots.clone();
    let board_dir_for_task = board_dir.clone();

    // Build + wire + discover on the shared confinement runtime, off the Tokio
    // worker pool.
    //
    // Why off-worker: `wire_command_services` and `discover_plugins` borrow
    // `&PluginPlatform` across `.await` points, and `PluginPlatform` is `Send`
    // but not `Sync` (its `PluginHost` carries the JS isolate state). Holding
    // that `&` across an await makes the surrounding future `!Send`, which would
    // taint `BoardHandle::open` / `open_board` — both of which the menu handlers
    // run inside `tauri::async_runtime::spawn`, where the future must be `Send`.
    // [`crate::confine::spawn_confined`] runs the `!Send` build span on the
    // confinement runtime's blocking pool and returns a `JoinHandle` we `.await`
    // here, so the main worker is freed (not blocked) and only the owned (and
    // `Send`) `PluginPlatform` crosses back, keeping the caller's future `Send`.
    // It is the SAME confinement seam the synchronous `WindowShell` ops and the
    // per-board host teardown use, so the strategy lives in one place. The global
    // platform built in `AppState::new` doesn't need this — that future is
    // awaited directly in `main`, never spawned with a `Send` bound.
    let built = crate::confine::spawn_confined(move |handle: &tokio::runtime::Handle| {
        handle.block_on(build_board_platform_inner(
            &roots,
            &board_dir_for_task,
            apphandle_shells,
        ))
    })
    .await;

    match built {
        Ok(platform) => platform,
        Err(e) => {
            tracing::warn!(
                board = %board_dir.display(),
                error = %e,
                "per-board plugin platform build task panicked"
            );
            None
        }
    }
}

/// Build + wire + discover the per-board platform on the current runtime.
///
/// Split out of [`build_board_platform`] so the `!Send` build span (it borrows
/// `&PluginPlatform` across awaits) runs entirely on the shared confinement
/// runtime that helper routes it onto. Mirrors the global wiring:
/// `build` → `wire_command_services(None, None)` → expose `window` / `app` →
/// `discover_plugins`.
async fn build_board_platform_inner(
    roots: &PluginRoots,
    board_dir: &Path,
    apphandle_shells: ApphandleShells,
) -> Option<PluginPlatform> {
    let mut platform = match PluginPlatform::build(
        roots.user_root.clone(),
        roots.builtin_cache.clone(),
        board_dir.to_path_buf(),
    )
    .await
    {
        Ok(platform) => platform,
        Err(e) => {
            tracing::warn!(
                board = %board_dir.display(),
                error = %e,
                "failed to build per-board plugin platform; board falls back to global host"
            );
            return None;
        }
    };

    if let Err(e) = platform
        .wire_command_services(Arc::clone(&roots.ui_state), None, None)
        .await
    {
        tracing::warn!(
            board = %board_dir.display(),
            error = %e,
            "failed to wire per-board command-service modules"
        );
    }
    // Expose the AppHandle-backed `window` / `app` modules (when the shells are
    // available) BEFORE discovery, so this board's host loads the four
    // AppHandle-dependent builtin command plugins too. A board opened before
    // setup stored the shells gets `None` here and loads the rest.
    let (window_shell, app_shell) = match apphandle_shells {
        Some((w, a)) => (Some(w), Some(a)),
        None => (None, None),
    };
    if let Err(e) = platform
        .expose_apphandle_modules(window_shell, app_shell)
        .await
    {
        tracing::warn!(
            board = %board_dir.display(),
            error = %e,
            "failed to expose per-board window/app modules"
        );
    }
    if let Err(e) = platform.discover_plugins().await {
        tracing::warn!(
            board = %board_dir.display(),
            error = %e,
            "per-board plugin discovery failed"
        );
    }

    tracing::info!(board = %board_dir.display(), "per-board plugin platform ready");
    Some(platform)
}

/// Walk up from `start_dir` looking for a `.kanban` subdirectory.
///
/// Returns the parent directory containing `.kanban` (not the `.kanban` dir itself),
/// or `None` if no ancestor contains one.
pub fn discover_board(start_dir: &Path) -> Option<PathBuf> {
    let mut current = start_dir.to_path_buf();
    loop {
        let candidate = current.join(".kanban");
        tracing::debug!("Checking for board at {}", candidate.display());
        if candidate.is_dir() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Try the home directory as a backstop for board discovery.
///
/// If the CWD walk already passed through `$HOME`, this returns `None`
/// (the walk already checked it). Otherwise, runs `discover_board` on the
/// home directory and returns whatever it finds.
fn try_home_backstop(cwd: &Path) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let walked_through_home = cwd.starts_with(&home);
    tracing::info!(
        home = %home.display(),
        walked_through_home,
        "auto_open_board: checking home dir backstop"
    );
    if walked_through_home {
        return None;
    }
    let board_dir = discover_board(&home);
    if let Some(ref dir) = board_dir {
        tracing::info!(path = %dir.display(), "auto_open_board: found .kanban via home backstop");
    }
    board_dir
}

/// Try the most recently used board as a fallback.
///
/// Reads the MRU list from UIState and returns the first entry whose
/// path still exists on disk, or `None` if no valid MRU entry is found.
fn try_mru_fallback(ui_state: &swissarmyhammer_ui_state::UIState) -> Option<PathBuf> {
    let recent_boards = ui_state.recent_boards();
    let recent = recent_boards.first()?;
    let path = PathBuf::from(&recent.path);
    tracing::info!(
        path = %path.display(),
        name = %recent.name,
        "auto_open_board: falling back to MRU board"
    );
    if path.is_dir() {
        Some(path)
    } else {
        tracing::warn!(path = %path.display(), "auto_open_board: MRU path no longer exists");
        None
    }
}

/// Resolve a user-provided path to a .kanban directory path.
///
/// Rules:
/// - If path ends in `.kanban` and is a directory, use it directly
/// - If path is a directory containing `.kanban/`, use `path/.kanban`
/// - If we're already inside a `.kanban` dir, use it (don't nest)
/// - Otherwise, assume `path/.kanban`
pub fn resolve_kanban_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

    // Already a .kanban directory
    if path.file_name().and_then(|n| n.to_str()) == Some(".kanban") && path.is_dir() {
        return Ok(path);
    }

    // Check if we're inside a .kanban directory (e.g. path is /foo/.kanban/tasks)
    for ancestor in path.ancestors() {
        if ancestor.file_name().and_then(|n| n.to_str()) == Some(".kanban") && ancestor.is_dir() {
            return Ok(ancestor.to_path_buf());
        }
    }

    // Directory that contains .kanban/
    let child = path.join(".kanban");
    if child.is_dir() {
        return Ok(child);
    }

    // Default: will be created at path/.kanban
    Ok(child)
}

/// Make the board folder a SwissArmyHammer workspace via in-process init.
///
/// Runs the root-explicit [`swissarmyhammer_workspace_init`] components rooted
/// at the board folder (the parent of the `.kanban` directory), at project
/// scope, so the in-process agent has the same `.sah/` workspace and builtin
/// skills that `sah init` would produce. This never shells out to `sah` and
/// never mutates the process working directory — the components are rooted at
/// the explicit board path, which is essential in a multi-board desktop
/// process.
///
/// The operation is idempotent, so it is safe to call on every board open.
/// Failures are logged and swallowed: a board must still open even if skill
/// deployment hits a filesystem problem.
fn ensure_sah_workspace(kanban_path: &Path) {
    use swissarmyhammer_common::lifecycle::{InitScope, InitStatus};
    use swissarmyhammer_common::reporter::NullReporter;

    // The board folder is the parent of the `.kanban` directory; `.sah/` is
    // created as its sibling.
    let Some(board_dir) = kanban_path.parent() else {
        tracing::warn!(
            path = %kanban_path.display(),
            "cannot initialize SAH workspace: .kanban path has no parent"
        );
        return;
    };

    let results = swissarmyhammer_workspace_init::run_workspace_init(
        board_dir,
        &InitScope::Project,
        &NullReporter,
    );
    for r in results.iter().filter(|r| r.status == InitStatus::Error) {
        tracing::warn!(
            component = %r.name,
            error = %r.message,
            "SAH workspace init component failed"
        );
    }
    tracing::info!(
        path = %board_dir.display(),
        "ensured SAH workspace for board folder"
    );
}

/// Start the per-board in-process MCP server, rooted at the board folder.
///
/// Serves the **full** SwissArmyHammer toolset over a random loopback HTTP port.
/// The transport mode is `McpServerMode::Http { port: None }`:
/// `start_mcp_server_with_options` only asks the OS for an ephemeral free port
/// when the port is `None` — passing `Some(0)` would leave the reported
/// `connection_url` literally at port `0`, so `None` is the variant that
/// actually yields a usable `http://127.0.0.1:<port>/mcp` URL.
///
/// `agent_mode` is `true`: this registers the agent-only tools (`skill`,
/// `web`, the full-access `files`) on top of the always-on domain tools
/// (`kanban`, `git`, `code_context`, `shell`, …). The task requires the
/// **full** toolset — explicitly "kanban, skills/prompts, code-context,
/// files, etc." With `agent_mode = false`, `register_all_tools` calls
/// `remove_agent_tools()`, which strips `skill`/`web`/full-`files`; that
/// would contradict the full-toolset requirement and leave the board-folder
/// rooting (whose purpose is so "skills/prompts resolve from that board's
/// SAH directory") without the `skill` tool that consumes it. So
/// `start_mcp_server_with_options(.., agent_mode = true)` is used.
///
/// The server's working directory is the board folder — the parent of the
/// `.kanban` directory — so its `kanban` tool operates on this board's
/// `.kanban` and its skills/prompts resolve from the board's `.sah/`
/// workspace (created by [`ensure_sah_workspace`]).
///
/// Returns `None` when the `.kanban` path has no parent or the server fails
/// to bind; failures are logged and swallowed so a filesystem or port problem
/// never blocks a board from opening. The board simply has no AI MCP endpoint
/// in that case, and [`BoardHandle::mcp_url`] returns `None`.
async fn start_board_mcp_server(kanban_path: &Path) -> Option<McpServerHandle> {
    let Some(board_dir) = kanban_path.parent() else {
        tracing::warn!(
            path = %kanban_path.display(),
            "cannot start board MCP server: .kanban path has no parent"
        );
        return None;
    };

    match start_mcp_server_with_options(
        McpServerMode::Http { port: None },
        None,
        None,
        Some(board_dir.to_path_buf()),
        true,
    )
    .await
    {
        Ok(handle) => {
            tracing::info!(
                board = %board_dir.display(),
                url = %handle.url(),
                "started in-process MCP server for board"
            );
            Some(handle)
        }
        Err(e) => {
            tracing::warn!(
                board = %board_dir.display(),
                error = %e,
                "failed to start in-process MCP server for board"
            );
            None
        }
    }
}

/// Curated palette of visually distinct colors for actor avatars.
const ACTOR_COLORS: &[&str] = &[
    "e53e3e", "dd6b20", "d69e2e", "38a169", "319795", "3182ce", "5a67d8", "805ad5", "d53f8c",
    "2b6cb0", "c05621", "2f855a", "2c7a7b", "6b46c1", "b83280",
];

/// Derive a deterministic hex color from a username.
fn deterministic_color(username: &str) -> String {
    let hash: u64 = username
        .bytes()
        .fold(5381u64, |h, b| h.wrapping_mul(33).wrapping_add(b as u64));
    ACTOR_COLORS[(hash as usize) % ACTOR_COLORS.len()].to_string()
}

/// Try to load the macOS user profile picture as a data URI.
///
/// macOS stores user pictures at `/Users/<username>/Library/Caches/com.apple.user-picture/`
/// or via the DSCL directory services. We try the simplest approach: read the JPEG
/// from the standard DS picture path.
#[cfg(target_os = "macos")]
fn macos_profile_picture(username: &str) -> Option<String> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let home = std::env::var("HOME").ok()?;
    let candidates = [
        format!("{home}/Library/Caches/com.apple.user-picture/user-picture.jpeg"),
        format!("{home}/Library/Caches/com.apple.user-picture/user-picture.png"),
        format!("{home}/.face"),
        format!("{home}/.face.icon"),
    ];

    for path in &candidates {
        if let Ok(data) = std::fs::read(path) {
            if data.is_empty() {
                continue;
            }
            let mime = if path.ends_with(".png") {
                "image/png"
            } else {
                "image/jpeg"
            };
            return Some(format!("data:{mime};base64,{}", STANDARD.encode(&data)));
        }
    }

    dscl_jpeg_photo(username)
}

/// Try reading a profile picture from macOS's `dscl` JPEGPhoto attribute.
///
/// Falls back to shelling out to `dscl . -read /Users/<username> JPEGPhoto`,
/// which outputs hex-encoded JPEG data. Returns a data URI if a valid JPEG
/// header is found, or `None` otherwise.
fn dscl_jpeg_photo(username: &str) -> Option<String> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let output = std::process::Command::new("dscl")
        .args([".", "-read", &format!("/Users/{username}"), "JPEGPhoto"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let hex_body = stdout.strip_prefix("JPEGPhoto:\n").unwrap_or(&stdout);
    let hex_clean: String = hex_body.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    let bytes: Vec<u8> = (0..hex_clean.len())
        .step_by(2)
        .filter_map(|i| {
            hex_clean
                .get(i..i + 2)
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
        })
        .collect();
    if bytes.len() > 2 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        return Some(format!(
            "data:image/jpeg;base64,{}",
            STANDARD.encode(&bytes)
        ));
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn macos_profile_picture(_username: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    use swissarmyhammer_app_service::{AboutInfo, AppShell};
    use swissarmyhammer_window_service::{
        ContextMenuItem, CreatedBoard, MonitorInfo, NewWindow, OpenedBoard, WindowPosition,
        WindowShell,
    };

    /// A no-op [`WindowShell`] for the plugin-platform tests.
    ///
    /// The per-window / baseline tests only need the `window` MCP module
    /// EXPOSED so the `file-commands` / `ui-commands` / `kanban-misc-commands`
    /// builtin plugins satisfy `ensureServices` and load — they don't drive any
    /// window op — so every method returns a benign canned value.
    pub(super) struct SpyWindowShell;

    impl WindowShell for SpyWindowShell {
        fn open_new_window(&self, board_path: Option<String>) -> Result<NewWindow, String> {
            Ok(NewWindow {
                label: "spy-window".to_string(),
                board_path,
            })
        }
        fn activate_window(&self, _label: &str) -> Result<(), String> {
            Ok(())
        }
        fn set_window_position(&self, _label: &str, _pos: WindowPosition) -> Result<(), String> {
            Ok(())
        }
        fn get_window_position(&self, _label: &str) -> Result<WindowPosition, String> {
            Ok(WindowPosition { x: 0, y: 0 })
        }
        fn get_monitors(&self) -> Result<Vec<MonitorInfo>, String> {
            Ok(Vec::new())
        }
        fn close_window(&self, _label: &str) -> Result<(), String> {
            Ok(())
        }
        fn open_path(&self, _path: &str) -> Result<(), String> {
            Ok(())
        }
        fn reveal_path(&self, _path: &str) -> Result<(), String> {
            Ok(())
        }
        fn switch_board(&self, _path: &str) -> Result<(), String> {
            Ok(())
        }
        fn close_board(&self, _path: &str) -> Result<(), String> {
            Ok(())
        }
        fn new_board(&self) -> Result<CreatedBoard, String> {
            Ok(CreatedBoard {
                path: "/tmp/spy-board".to_string(),
                name: "spy-board".to_string(),
            })
        }
        fn open_board(&self) -> Result<Option<OpenedBoard>, String> {
            Ok(None)
        }
        fn show_context_menu(
            &self,
            _items: Vec<ContextMenuItem>,
            _window_label: Option<String>,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    /// A no-op [`AppShell`] for the plugin-platform tests, exposed for the same
    /// reason as [`SpyWindowShell`] (so `app-shell-commands` loads).
    pub(super) struct SpyAppShell;

    impl AppShell for SpyAppShell {
        fn quit(&self) {}
        fn show_about(&self) -> AboutInfo {
            AboutInfo {
                name: "kanban-app-test".to_string(),
                version: "0.0.0".to_string(),
            }
        }
        fn show_help(&self) -> String {
            "https://help.example/test".to_string()
        }
    }

    #[test]
    fn test_resolve_existing_kanban_dir() {
        let tmp = TempDir::new().unwrap();
        let kanban_dir = tmp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        // Passing the .kanban dir directly
        let result = resolve_kanban_path(&kanban_dir).unwrap();
        assert_eq!(result, kanban_dir.canonicalize().unwrap());
    }

    #[test]
    fn test_resolve_parent_containing_kanban() {
        let tmp = TempDir::new().unwrap();
        let kanban_dir = tmp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        // Passing the parent directory
        let result = resolve_kanban_path(tmp.path()).unwrap();
        assert_eq!(result, kanban_dir.canonicalize().unwrap());
    }

    #[test]
    fn test_resolve_inside_kanban_dir() {
        let tmp = TempDir::new().unwrap();
        let kanban_dir = tmp.path().join(".kanban");
        let tasks_dir = kanban_dir.join("tasks");
        std::fs::create_dir_all(&tasks_dir).unwrap();

        // Passing a path inside .kanban
        let result = resolve_kanban_path(&tasks_dir).unwrap();
        assert_eq!(result, kanban_dir.canonicalize().unwrap());
    }

    #[test]
    fn test_resolve_no_kanban_yet() {
        let tmp = TempDir::new().unwrap();

        // No .kanban exists — should return path/.kanban
        let result = resolve_kanban_path(tmp.path()).unwrap();
        assert_eq!(result, tmp.path().canonicalize().unwrap().join(".kanban"));
    }

    #[test]
    fn test_resolve_never_nests_kanban() {
        let tmp = TempDir::new().unwrap();
        let kanban_dir = tmp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        // Passing .kanban itself should NOT create .kanban/.kanban
        let result = resolve_kanban_path(&kanban_dir).unwrap();
        assert!(
            !result.ends_with(".kanban/.kanban"),
            "Should never nest .kanban: {:?}",
            result
        );
    }

    #[test]
    fn test_mru_uistate_touch_and_truncate() {
        let ui_state = swissarmyhammer_ui_state::UIState::new();

        for i in 0..25 {
            ui_state.touch_recent(&format!("/board/{}", i), &format!("Board {}", i));
        }

        let boards = ui_state.recent_boards();
        assert_eq!(boards.len(), 20); // MAX_RECENT_BOARDS
                                      // Most recent should be first
        assert_eq!(boards[0].name, "Board 24");
    }

    #[test]
    fn test_discover_board_found_in_cwd() {
        let tmp = TempDir::new().unwrap();
        let kanban_dir = tmp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        let result = discover_board(tmp.path());
        assert_eq!(result, Some(tmp.path().to_path_buf()));
    }

    #[test]
    fn test_discover_board_found_in_ancestor() {
        let tmp = TempDir::new().unwrap();
        let kanban_dir = tmp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        let nested = tmp.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&nested).unwrap();

        let result = discover_board(&nested);
        assert_eq!(result, Some(tmp.path().to_path_buf()));
    }

    #[test]
    fn test_discover_board_not_found() {
        let tmp = TempDir::new().unwrap();
        // No .kanban anywhere
        let result = discover_board(tmp.path());
        assert_eq!(result, None);
    }

    #[test]
    fn test_mru_deduplicates() {
        let ui_state = swissarmyhammer_ui_state::UIState::new();

        ui_state.touch_recent("/board/a", "Board A");
        ui_state.touch_recent("/board/b", "Board B");
        ui_state.touch_recent("/board/a", "Board A Updated");

        let boards = ui_state.recent_boards();
        assert_eq!(boards.len(), 2);
        assert_eq!(boards[0].name, "Board A Updated");
    }

    // =========================================================================
    // Integration tests for auto_open_board
    // =========================================================================

    /// Helper: create a minimal .kanban board structure that the entity system
    /// can load. This means .kanban/boards/board.yaml must exist (the entity
    /// location, not just the legacy root-level board.yaml).
    fn create_board_at(root: &Path, name: &str) {
        let kanban_dir = root.join(".kanban");
        let boards_dir = kanban_dir.join("boards");
        std::fs::create_dir_all(&boards_dir).unwrap();
        std::fs::write(boards_dir.join("board.yaml"), format!("name: {}\n", name)).unwrap();
        // Also create columns dir so the processor doesn't try to auto-init
        std::fs::create_dir_all(kanban_dir.join("columns")).unwrap();
        std::fs::create_dir_all(kanban_dir.join("tasks")).unwrap();
        std::fs::create_dir_all(kanban_dir.join("tags")).unwrap();
        std::fs::create_dir_all(kanban_dir.join("actors")).unwrap();
        std::fs::create_dir_all(kanban_dir.join("perspectives")).unwrap();
    }

    #[tokio::test]
    async fn test_auto_open_board_from_cwd() {
        let tmp = TempDir::new().unwrap();
        create_board_at(tmp.path(), "Test Board");

        // Simulate CWD being inside the project
        let subdir = tmp.path().join("src").join("components");
        std::fs::create_dir_all(&subdir).unwrap();

        let state = AppState::new_for_test();
        // Manually run discovery from the subdir (can't change real CWD in tests)
        let board_dir = discover_board(&subdir);
        assert_eq!(board_dir, Some(tmp.path().to_path_buf()));

        // Open it
        let result = state.open_board(board_dir.as_ref().unwrap(), None).await;
        assert!(result.is_ok(), "open_board failed: {:?}", result.err());

        // Verify active board is set
        let handle = state.active_handle().await;
        assert!(handle.is_some(), "active_handle should be Some after open");
    }

    #[tokio::test]
    async fn test_auto_open_board_no_kanban_dir() {
        let tmp = TempDir::new().unwrap();
        // No .kanban anywhere

        let result = discover_board(tmp.path());
        assert_eq!(result, None);

        let state = AppState::new_for_test();
        // No board opened — active_handle should be None
        let handle = state.active_handle().await;
        assert!(handle.is_none());
    }

    #[tokio::test]
    async fn test_open_board_sets_active_and_appears_in_boards() {
        let tmp = TempDir::new().unwrap();
        create_board_at(tmp.path(), "My Board");

        let state = AppState::new_for_test();
        let result = state.open_board(tmp.path(), None).await;
        assert!(result.is_ok());

        let canonical = result.unwrap();

        // most_recent_board should be set
        assert_eq!(
            state.ui_state.most_recent_board(),
            Some(canonical.display().to_string())
        );

        // boards map should contain the handle
        let boards = state.boards.read().await;
        assert!(boards.contains_key(&canonical));
    }

    #[tokio::test]
    async fn test_open_board_creates_sah_workspace_at_board_folder() {
        // Drives the real production entry point — `AppState::open_board`
        // delegates to `BoardHandle::open`, which calls `ensure_sah_workspace`
        // with the resolved `.kanban` path and roots the workspace init at
        // `kanban_path.parent()` (the board folder).
        //
        // This test fails if either piece of the production wiring regresses:
        //   - if `ensure_sah_workspace` is removed from `BoardHandle::open`,
        //     nothing creates `.sah/` and the assertion below fails;
        //   - if the `.parent()` board-folder math is wrong (e.g. rooting at
        //     `.kanban/` itself), `.sah/` lands inside `.kanban/` rather than
        //     beside it, so `<board>/.sah/skills/plan/SKILL.md` is absent.
        let tmp = TempDir::new().unwrap();
        create_board_at(tmp.path(), "Workspace Board");

        let state = AppState::new_for_test();
        let result = state.open_board(tmp.path(), None).await;
        assert!(result.is_ok(), "open_board failed: {:?}", result.err());

        // The board folder is the parent of `.kanban/`; the SAH workspace must
        // land there, beside `.kanban/`, not inside it.
        let board_dir = tmp.path().canonicalize().unwrap();
        assert!(
            board_dir.join(".sah").is_dir(),
            ".sah/ must be created at the board folder by BoardHandle::open"
        );
        let plan_skill = board_dir
            .join(".sah")
            .join("skills")
            .join("plan")
            .join("SKILL.md");
        assert!(
            plan_skill.is_file(),
            "builtin `plan` skill must be deployed at {} via the open_board production path",
            plan_skill.display()
        );

        // `.sah/` must be a sibling of `.kanban/`, never nested inside it —
        // this is what proves the `kanban_path.parent()` math is correct.
        assert!(
            !board_dir.join(".kanban").join(".sah").exists(),
            ".sah/ must not be created inside .kanban/ — board-folder math is wrong"
        );
    }

    #[tokio::test]
    async fn test_open_second_board_keeps_both_in_list() {
        let tmp_a = TempDir::new().unwrap();
        let tmp_b = TempDir::new().unwrap();
        create_board_at(tmp_a.path(), "Board A");
        create_board_at(tmp_b.path(), "Board B");

        let state = AppState::new_for_test();

        // Open board A
        let path_a = state.open_board(tmp_a.path(), None).await.unwrap();

        // Open board B
        let path_b = state.open_board(tmp_b.path(), None).await.unwrap();

        // Both boards must be in the map
        let boards = state.boards.read().await;
        assert_eq!(boards.len(), 2, "Expected 2 boards, got {}", boards.len());
        assert!(boards.contains_key(&path_a), "Board A missing from map");
        assert!(boards.contains_key(&path_b), "Board B missing from map");

        // Most recent board should be B (most recently opened)
        assert_eq!(
            state.ui_state.most_recent_board(),
            Some(path_b.display().to_string())
        );
    }

    #[test]
    fn test_deterministic_color_is_stable() {
        let c1 = deterministic_color("alice");
        let c2 = deterministic_color("alice");
        assert_eq!(c1, c2);
        // Should be a valid 6-char hex
        assert_eq!(c1.len(), 6);
        assert!(c1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_deterministic_color_varies() {
        let c1 = deterministic_color("alice");
        let c2 = deterministic_color("bob");
        // Different usernames should (usually) get different colors
        // Not guaranteed but very likely with a 15-color palette
        assert_ne!(c1, c2);
    }

    // =========================================================================
    // Drag session tests
    // =========================================================================

    fn make_drag_session(task_id: &str, board_path: &str) -> swissarmyhammer_ui_state::DragSession {
        swissarmyhammer_ui_state::DragSession {
            session_id: ulid::Ulid::new().to_string(),
            from: swissarmyhammer_ui_state::DragSource::FocusChain {
                entity_type: "task".to_string(),
                entity_id: task_id.to_string(),
                fields: serde_json::json!({"title": "Test task"}),
                source_board_path: board_path.to_string(),
                source_window_label: "main".to_string(),
            },
            copy_mode: false,
            started_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }

    #[test]
    fn test_drag_session_start_and_cancel() {
        let state = AppState::new_for_test();
        assert!(state.ui_state.drag_session().is_none());

        // Start session
        let session = make_drag_session("task-1", "/board/a");
        state.ui_state.start_drag(session);
        assert!(state.ui_state.drag_session().is_some());

        // Cancel session
        let taken = state.ui_state.take_drag();
        assert!(taken.is_some());
        assert_eq!(taken.unwrap().entity_id(), Some("task-1"));
        assert!(state.ui_state.drag_session().is_none());
    }

    #[test]
    fn test_drag_session_double_take_returns_none() {
        let state = AppState::new_for_test();
        state
            .ui_state
            .start_drag(make_drag_session("task-1", "/board/a"));

        // First take succeeds
        let first = state.ui_state.take_drag();
        assert!(first.is_some());

        // Second take returns None (session already consumed)
        let second = state.ui_state.take_drag();
        assert!(second.is_none());
    }

    #[test]
    fn test_drag_session_replaced_by_new_start() {
        let state = AppState::new_for_test();
        let session1 = make_drag_session("task-1", "/board/a");
        let id1 = session1.session_id.clone();
        state.ui_state.start_drag(session1);

        // Replace with new session
        let session2 = make_drag_session("task-2", "/board/b");
        let id2 = session2.session_id.clone();
        state.ui_state.start_drag(session2);

        let current = state.ui_state.drag_session();
        assert_eq!(current.as_ref().unwrap().session_id, id2);
        assert_ne!(id1, id2);
        assert_eq!(current.as_ref().unwrap().entity_id(), Some("task-2"));
    }

    #[test]
    fn test_drag_session_stale_detection() {
        let mut session = make_drag_session("task-1", "/board/a");
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Session just started — not stale (max age is 30 seconds = 30_000 ms)
        session.started_at_ms = now_ms;
        assert!(now_ms.saturating_sub(session.started_at_ms) <= 30_000);

        // Session 31 seconds ago — stale
        session.started_at_ms = now_ms - 31_000;
        assert!(now_ms.saturating_sub(session.started_at_ms) > 30_000);
    }

    #[test]
    fn test_drag_session_serialization() {
        let session = make_drag_session("task-1", "/board/a");
        let json = serde_json::to_value(&session).unwrap();
        // The session's source is now nested under `from` as a tagged enum
        // (`kind: "focus_chain"`). The frontend's `drag-session-active`
        // wire payload still ships the legacy flat shape; that wire payload
        // is built by `DragStartCmd` directly and is exercised by the
        // drag_start_cmd_returns_drag_start_result test in mod.rs.
        assert_eq!(json["from"]["kind"], "focus_chain");
        assert_eq!(json["from"]["entity_type"], "task");
        assert_eq!(json["from"]["entity_id"], "task-1");
        assert_eq!(json["from"]["source_board_path"], "/board/a");
        assert_eq!(json["from"]["source_window_label"], "main");
        assert_eq!(json["copy_mode"], false);
        assert!(json["started_at_ms"].as_u64().unwrap() > 0);
    }

    // =========================================================================
    // Window label tests
    // =========================================================================

    #[test]
    fn test_window_label_is_ulid_based() {
        // Verify the label format matches what create_window generates
        let label = format!("board-{}", ulid::Ulid::new().to_string().to_lowercase());
        assert!(label.starts_with("board-"));
        // ULID is 26 chars, so label is "board-" (6) + 26 = 32
        assert_eq!(label.len(), 32);
    }

    // =========================================================================
    // Per-board in-process MCP server tests
    // =========================================================================

    /// Opening a board starts an in-process full-SAH-toolset MCP server rooted
    /// at the board folder. An MCP client connected to the board's URL sees
    /// `tools/list` carrying `kanban` plus other SAH tools, a `kanban`
    /// `add task` call mutates that board's `.kanban`, and closing the board
    /// shuts the server down so its URL stops answering.
    // multi_thread required: this test hosts the in-process board MCP server and
    // drives an RMCP client on the same runtime. A current-thread runtime cannot
    // advance the server's SSE response task while blocked awaiting the client
    // handshake — see `test_client_handshake_is_fast` in `swissarmyhammer-tools`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_open_board_serves_full_sah_mcp_toolset() {
        let tmp = TempDir::new().unwrap();
        let state = AppState::new_for_test();
        let canonical = state
            .open_board(tmp.path(), None)
            .await
            .expect("open_board should succeed");

        // The board folder is the parent of `.kanban/`; the MCP server is
        // rooted there so its `kanban` tool operates on this board.
        let board_dir = tmp.path().canonicalize().unwrap();

        // The board exposes a loopback MCP URL for the AI backend to consume.
        let mcp_url = {
            let boards = state.boards.read().await;
            let handle = boards.get(&canonical).expect("board must be open");
            handle
                .mcp_url()
                .expect("an open board must expose an MCP URL")
                .to_string()
        };
        assert!(
            mcp_url.starts_with("http://127.0.0.1:") && mcp_url.ends_with("/mcp"),
            "MCP URL must be a loopback /mcp endpoint, got {mcp_url}"
        );

        // `tools/list` must carry the FULL SAH toolset — not just `kanban`.
        // Assert the specific tools the task requires so a regression in tool
        // registration (or a flip back to `agent_mode = false`, which would
        // strip `skill`/`web`/full-`files`) is actually caught. The expected
        // names are the `McpTool::name()` strings the server registers:
        //   - `kanban`, `git`, `code_context` — always-on domain tools
        //   - `skill`, `files`, `web` — agent tools, present only because the
        //     board server runs with `agent_mode = true`
        let client = swissarmyhammer_tools::mcp::test_utils::create_test_client(&mcp_url).await;
        let tools = client
            .list_tools(Default::default())
            .await
            .expect("tools/list should succeed");
        let tool_names: Vec<String> = tools.tools.iter().map(|t| t.name.to_string()).collect();
        for expected in ["kanban", "git", "code_context", "skill", "files", "web"] {
            assert!(
                tool_names.iter().any(|n| n == expected),
                "tools/list must include the full SAH tool `{expected}` \
                 (agent_mode = true serves the agent tools), got {tool_names:?}"
            );
        }

        // A `kanban` call routed through this server must mutate THIS board.
        // Init the board first, then add a task.
        let kanban_call = |args: serde_json::Value| {
            rmcp::model::CallToolRequestParams::new("kanban").with_arguments(
                args.as_object()
                    .cloned()
                    .expect("call arguments must be a JSON object"),
            )
        };
        client
            .call_tool(kanban_call(
                serde_json::json!({ "op": "init board", "name": "MCP Board" }),
            ))
            .await
            .expect("kanban init board should succeed over MCP");
        client
            .call_tool(kanban_call(
                serde_json::json!({ "op": "add task", "title": "Served via MCP" }),
            ))
            .await
            .expect("kanban add task should succeed over MCP");

        // The mutation landed in this board's `.kanban/tasks/` directory.
        let tasks_dir = board_dir.join(".kanban").join("tasks");
        let task_files: Vec<_> = std::fs::read_dir(&tasks_dir)
            .expect("tasks dir must exist after add task")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
            .collect();
        assert!(
            !task_files.is_empty(),
            "the MCP `add task` call must have written a task file under {}",
            tasks_dir.display()
        );

        client.cancel().await.unwrap();

        // Closing the board shuts the MCP server down — its URL must stop
        // answering. `close_board` keys on the canonical `.kanban` path that
        // `open_board` returned.
        state
            .close_board(&canonical)
            .await
            .expect("close_board should succeed");

        // Closing drops the `BoardHandle`, which spawns the async MCP-server
        // shutdown onto the runtime. Give that spawned task a moment to take
        // the listener down before probing the URL.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let probe = reqwest::Client::default()
            .get(&mcp_url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await;
        assert!(
            probe.is_err(),
            "the board's MCP server must be stopped after the board closes — \
             {mcp_url} should no longer answer, got {probe:?}"
        );
    }
}
