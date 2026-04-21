//! Application state management with multi-board support and MRU persistence.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use swissarmyhammer_commands::{
    builtin_yaml_sources, load_yaml_dir, Command, CommandsRegistry, UIState,
};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_entity_search::EntitySearchIndex;
use swissarmyhammer_kanban::clipboard::ClipboardProvider;
use swissarmyhammer_kanban::KanbanContext;
use tauri::menu::{CheckMenuItem, MenuItem};
use tokio::sync::RwLock;

use swissarmyhammer_kanban::actor::AddActor;
use swissarmyhammer_kanban::Execute;

use crate::watcher;
use swissarmyhammer_entity::EntityCache;

const CONFIG_APP_SUBDIR: &str = "kanban-app";
const UI_STATE_FILE_NAME: &str = "ui-state.yaml";

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
}

impl Drop for BoardHandle {
    fn drop(&mut self) {
        if let Some(task) = self.bridge_task.take() {
            task.abort();
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

/// Register a per-entity-type store for each entity type discovered on disk.
/// Wires the shared `StoreContext` into `EntityContext` so writes/deletes push
/// onto the undo stack, then creates an `EntityTypeStore` for every entity
/// def and registers it with both contexts.
async fn register_entity_stores(
    ctx: &KanbanContext,
    store_context: &Arc<swissarmyhammer_store::StoreContext>,
) {
    let Ok(ectx) = ctx.entity_context().await else {
        return;
    };
    ectx.set_store_context(Arc::clone(store_context));

    let fields_ctx = ectx.fields();
    for entity_def in fields_ctx.all_entities() {
        let entity_type = entity_def.name.as_str();
        let field_defs = fields_ctx.fields_for_entity(entity_type);
        let owned_defs: Vec<_> = field_defs.into_iter().cloned().collect();
        let entity_type_store = swissarmyhammer_entity::EntityTypeStore::new(
            ectx.entity_dir(entity_type),
            entity_type,
            Arc::new(entity_def.clone()),
            Arc::new(owned_defs),
        );
        let handle = Arc::new(swissarmyhammer_store::StoreHandle::new(Arc::new(
            entity_type_store,
        )));
        ectx.register_store(entity_type, handle.clone()).await;
        store_context.register(handle).await;
    }
}

/// Register the perspective store for undo/redo and wire it into
/// `PerspectiveContext` so writes delegate to it and push onto the undo stack.
async fn register_perspective_store(
    ctx: &KanbanContext,
    store_context: &Arc<swissarmyhammer_store::StoreContext>,
    kanban_path: &Path,
) {
    let perspectives_dir = kanban_path.join("perspectives");
    let perspective_store = swissarmyhammer_perspectives::PerspectiveStore::new(&perspectives_dir);
    let handle = Arc::new(swissarmyhammer_store::StoreHandle::new(Arc::new(
        perspective_store,
    )));
    store_context.register(handle.clone()).await;

    if let Ok(pctx) = ctx.perspective_context().await {
        let mut pctx = pctx.write().await;
        pctx.set_store_handle(handle);
        pctx.set_store_context(Arc::clone(store_context));
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
    pub async fn open(kanban_path: PathBuf) -> Result<Self, String> {
        let ctx = KanbanContext::open(&kanban_path)
            .await
            .map_err(|e| format!("Failed to open board context: {e}"))?;

        let store_context = Arc::new(swissarmyhammer_store::StoreContext::new(
            kanban_path.to_path_buf(),
        ));
        register_entity_stores(&ctx, &store_context).await;
        register_perspective_store(&ctx, &store_context, &kanban_path).await;

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

        Ok(Self {
            ctx: Arc::new(ctx),
            store_context,
            entity_cache,
            search_index,
            bridge_task: None,
        })
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
        // Invariant: `start_watcher` must be called after `register_perspective_store`
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

        tracing::info!(
            path = %kanban_root.display(),
            has_perspective_rx = perspective_rx.is_some(),
            "entity-cache bridge starting for board"
        );
        let handle = tokio::spawn(watcher::run_bridge(
            ctx,
            cache,
            app_handle,
            board_path_str,
            search_index,
            perspective_rx,
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
    /// Trait object map from `register_commands()`.
    pub(crate) command_impls: HashMap<String, Arc<dyn Command>>,
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
}

impl AppState {
    /// Create a new AppState, loading config from disk.
    ///
    /// Restores the inspector stack from the persisted config into UIState
    /// so the backend is the single source of truth from startup.
    pub fn new() -> Self {
        Self::with_ui_state_path(ui_state_file_path())
    }

    /// Create AppState with a specific UIState persistence path.
    ///
    /// Used by tests to avoid polluting the real config file.
    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self::with_ui_state_path(
            std::env::temp_dir().join(format!("kanban-test-{}.yaml", ulid::Ulid::new())),
        )
    }

    /// Internal constructor with an explicit UIState persistence path.
    fn with_ui_state_path(ui_state_path: PathBuf) -> Self {
        let sources = builtin_yaml_sources();
        let source_refs: Vec<(&str, &str)> = sources.iter().map(|(n, c)| (*n, *c)).collect();
        let ui_state = Arc::new(UIState::load(ui_state_path));

        Self {
            boards: RwLock::new(HashMap::new()),
            ui_state,
            commands_registry: RwLock::new(CommandsRegistry::from_yaml_sources(&source_refs)),
            command_impls: swissarmyhammer_kanban::commands::register_commands(),
            menu_items: Mutex::new(HashMap::new()),
            shutting_down: AtomicBool::new(false),
            deep_link_handled: AtomicBool::new(false),
        }
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

        let mut handle = BoardHandle::open(kanban_path).await?;
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
    async fn reload_command_overrides(&self, kanban_path: &Path) {
        let commands_dir = kanban_path.join("commands");
        let user_sources = load_yaml_dir(&commands_dir);
        if user_sources.is_empty() {
            return;
        }
        let refs: Vec<(&str, &str)> = user_sources
            .iter()
            .map(|(n, c)| (n.as_str(), c.as_str()))
            .collect();

        // Rebuild from scratch: builtins + user overrides
        let builtin = builtin_yaml_sources();
        let builtin_refs: Vec<(&str, &str)> = builtin.iter().map(|(n, c)| (*n, *c)).collect();
        let mut registry = CommandsRegistry::from_yaml_sources(&builtin_refs);
        registry.merge_yaml_sources(&refs);

        *self.commands_registry.write().await = registry;
        tracing::info!(dir = %commands_dir.display(), count = user_sources.len(), "loaded user command overrides");
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
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
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
fn try_mru_fallback(ui_state: &swissarmyhammer_commands::UIState) -> Option<PathBuf> {
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

/// Get the path to the UIState persistence file.
///
/// Uses XDG config directory: `$XDG_CONFIG_HOME/sah/kanban-app/ui-state.yaml`
fn ui_state_file_path() -> PathBuf {
    use swissarmyhammer_directory::{ManagedDirectory, SwissarmyhammerConfig};

    ManagedDirectory::<SwissarmyhammerConfig>::xdg_config()
        .map(|dir| dir.root().join(CONFIG_APP_SUBDIR).join(UI_STATE_FILE_NAME))
        .unwrap_or_else(|_| {
            PathBuf::from(".")
                .join(CONFIG_APP_SUBDIR)
                .join(UI_STATE_FILE_NAME)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
        let ui_state = swissarmyhammer_commands::UIState::new();

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
        let ui_state = swissarmyhammer_commands::UIState::new();

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

    fn make_drag_session(task_id: &str, board_path: &str) -> swissarmyhammer_commands::DragSession {
        swissarmyhammer_commands::DragSession {
            session_id: ulid::Ulid::new().to_string(),
            from: swissarmyhammer_commands::DragSource::FocusChain {
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
}
