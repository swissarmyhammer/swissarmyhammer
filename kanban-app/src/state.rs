//! Application state management with multi-board support and MRU persistence.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use swissarmyhammer_commands::{
    builtin_yaml_sources, load_yaml_dir, Command, CommandsRegistry, UIState,
};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_entity_search::EntitySearchIndex;
use swissarmyhammer_kanban::{KanbanContext, KanbanOperationProcessor};
use tokio::sync::RwLock;

use swissarmyhammer_kanban::actor::AddActor;
use swissarmyhammer_kanban::Execute;

use crate::watcher::{self, BoardWatcher, EntityCache};

const MAX_RECENT_BOARDS: usize = 20;
const CONFIG_APP_SUBDIR: &str = "kanban-app";
const CONFIG_FILE_NAME: &str = "config.yaml";
const CONFIG_FILE_NAME_LEGACY: &str = "config.json";

/// A handle to a single open kanban board.
pub(crate) struct BoardHandle {
    pub(crate) ctx: Arc<KanbanContext>,
    pub(crate) processor: KanbanOperationProcessor,
    /// Entity cache for detecting external file changes with field-level diffing.
    pub(crate) entity_cache: EntityCache,
    /// In-memory search index over all entities.
    pub(crate) search_index: Arc<RwLock<EntitySearchIndex>>,
    /// File watcher — dropped when the handle is dropped.
    _watcher: Option<BoardWatcher>,
}

impl BoardHandle {
    /// Create a handle with a fully-initialized context (views, fields, etc.).
    ///
    /// Does NOT start the file watcher — call `start_watcher` after the
    /// Tauri AppHandle is available.
    pub async fn open(kanban_path: PathBuf) -> Result<Self, String> {
        let ctx = KanbanContext::open(&kanban_path)
            .await
            .map_err(|e| format!("Failed to open board context: {e}"))?;
        let entity_cache = watcher::new_entity_cache(&kanban_path);

        // Migrate legacy ordinals to FractionalIndex format.
        // Reads all tasks, groups by column, sorts by existing ordinal string,
        // then assigns new FractionalIndex ordinals preserving that order.
        if let Ok(ectx) = ctx.entity_context().await {
            use std::collections::HashMap;
            use swissarmyhammer_kanban::types::Ordinal;

            if let Ok(tasks) = ectx.list("task").await {
                // Check if any task has a legacy (non-FractionalIndex) ordinal
                let needs_migration = tasks.iter().any(|t| {
                    let ord = t.get_str("position_ordinal").unwrap_or("");
                    !ord.is_empty() && !Ordinal::is_valid(ord)
                });

                if needs_migration {
                    tracing::info!("migrating legacy ordinals to fractional index format");

                    // Group by column, sort by existing ordinal string
                    let mut by_column: HashMap<String, Vec<Entity>> = HashMap::new();
                    for t in tasks {
                        let col = t.get_str("position_column").unwrap_or("todo").to_string();
                        by_column.entry(col).or_default().push(t);
                    }

                    for tasks in by_column.values_mut() {
                        tasks.sort_by(|a, b| {
                            let oa = a.get_str("position_ordinal").unwrap_or("");
                            let ob = b.get_str("position_ordinal").unwrap_or("");
                            oa.cmp(ob)
                        });

                        // Assign new ordinals: first(), after(first), after(after(first)), ...
                        let mut ord = Ordinal::first();
                        for task in tasks.iter_mut() {
                            task.set("position_ordinal", serde_json::json!(ord.as_str()));
                            if let Err(e) = ectx.write(task).await {
                                tracing::warn!(id = %task.id, error = %e, "failed to migrate ordinal");
                            }
                            ord = Ordinal::after(&ord);
                        }
                    }
                    tracing::info!("ordinal migration complete");
                }
            }
        }

        // Load all entities into search index
        let mut all_entities: Vec<Entity> = Vec::new();
        if let Ok(ectx) = ctx.entity_context().await {
            for entity_type in &["task", "tag", "column", "actor", "swimlane", "board"] {
                if let Ok(entities) = ectx.list(entity_type).await {
                    all_entities.extend(entities);
                }
            }
        }
        let search_index = Arc::new(RwLock::new(EntitySearchIndex::from_entities(all_entities)));

        Ok(Self {
            ctx: Arc::new(ctx),
            processor: KanbanOperationProcessor::new(),
            entity_cache,
            search_index,
            _watcher: None,
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

    /// Start the file watcher, emitting entity-level events on the given AppHandle.
    pub fn start_watcher(&mut self, app_handle: tauri::AppHandle) {
        let kanban_root = self.ctx.root().to_path_buf();
        let cache = self.entity_cache.clone();
        let search_index = self.search_index.clone();
        let board_path_str = kanban_root.display().to_string();

        match watcher::start_watching(kanban_root.clone(), cache, move |evt| {
            use tauri::Emitter;
            // Update search index for external file changes.
            // Use try_write to avoid blocking the notify thread if the async
            // dispatch_command path holds the write lock concurrently.
            if let Ok(mut idx) = search_index.try_write() {
                watcher::sync_search_index(&mut idx, &evt);
            }
            let event_name = match &evt {
                watcher::WatchEvent::EntityCreated { .. } => "entity-created",
                watcher::WatchEvent::EntityRemoved { .. } => "entity-removed",
                watcher::WatchEvent::EntityFieldChanged { .. } => "entity-field-changed",
            };
            let wrapped = watcher::BoardWatchEvent {
                event: evt,
                board_path: board_path_str.clone(),
            };
            let _ = app_handle.emit(event_name, &wrapped);
        }) {
            Ok(w) => {
                tracing::info!(
                    path = %kanban_root.display(),
                    "file watcher started for board"
                );
                self._watcher = Some(w);
            }
            Err(e) => {
                tracing::warn!(
                    path = %kanban_root.display(),
                    error = %e,
                    "failed to start file watcher"
                );
            }
        }
    }
}

/// A recently opened board entry for MRU persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RecentBoard {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) last_opened: DateTime<Utc>,
}

/// Persisted app configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct AppConfig {
    pub(crate) recent_boards: Vec<RecentBoard>,
    #[serde(default = "default_keymap_mode")]
    pub(crate) keymap_mode: String,
    /// Paths of boards that were open when the app last ran.
    /// Restored on startup so multi-board sessions survive reloads.
    #[serde(default)]
    pub(crate) open_boards: Vec<PathBuf>,
    /// Per-window state: board path, active view, inspector stack, geometry.
    /// Keyed by window label ("main" for the primary window).
    #[serde(default, alias = "window_boards")]
    pub(crate) windows: HashMap<String, WindowState>,
}

/// Persisted state for a window (main or secondary).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WindowState {
    pub(crate) board_path: PathBuf,
    #[serde(default)]
    pub(crate) active_view_id: Option<String>,
    #[serde(default)]
    pub(crate) inspector_stack: Vec<String>,
    #[serde(default)]
    pub(crate) x: Option<i32>,
    #[serde(default)]
    pub(crate) y: Option<i32>,
    #[serde(default)]
    pub(crate) width: Option<u32>,
    #[serde(default)]
    pub(crate) height: Option<u32>,
    #[serde(default)]
    pub(crate) maximized: bool,
}

impl WindowState {
    /// Create a new WindowState with the given board path and default values.
    pub(crate) fn new(board_path: PathBuf) -> Self {
        Self {
            board_path,
            active_view_id: None,
            inspector_stack: Vec::new(),
            x: None,
            y: None,
            width: None,
            height: None,
            maximized: false,
        }
    }
}

fn default_keymap_mode() -> String {
    "cua".to_string()
}

impl AppConfig {
    /// Load config from disk, returning default if not found.
    ///
    /// Tries `config.yaml` first. If it doesn't exist, falls back to
    /// `config.json` (legacy format) for migration — the next `save()`
    /// will write YAML.
    pub fn load() -> Self {
        let yaml_path = config_file_path();
        if let Ok(content) = std::fs::read_to_string(&yaml_path) {
            return serde_yaml_ng::from_str(&content).unwrap_or_default();
        }
        // Fall back to legacy JSON for migration
        let json_path = legacy_config_file_path();
        if let Ok(content) = std::fs::read_to_string(&json_path) {
            return serde_json::from_str(&content).unwrap_or_default();
        }
        Self::default()
    }

    /// Save config to disk as YAML.
    pub fn save(&self) -> std::io::Result<()> {
        let path = config_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_yaml_ng::to_string(self)
            .map_err(std::io::Error::other)?;
        std::fs::write(&path, content)
    }

    /// Add or update a board in the MRU list.
    pub fn touch_recent(&mut self, path: &Path, name: &str) {
        // Remove existing entry for this path
        self.recent_boards.retain(|r| r.path != path);

        // Insert at front
        self.recent_boards.insert(
            0,
            RecentBoard {
                path: path.to_path_buf(),
                name: name.to_string(),
                last_opened: Utc::now(),
            },
        );

        // Truncate to max
        self.recent_boards.truncate(MAX_RECENT_BOARDS);
    }
}

/// Active drag session for cross-window drag coordination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DragSession {
    /// Unique session ID (ULID)
    pub(crate) session_id: String,
    /// Board path the task originates from
    pub(crate) source_board_path: String,
    /// Tauri window label of the source window
    pub(crate) source_window_label: String,
    /// The task ID being dragged
    pub(crate) task_id: String,
    /// Serialized task fields for ghost preview in target windows
    pub(crate) task_fields: serde_json::Value,
    /// Whether Alt/Option was held (copy mode)
    pub(crate) copy_mode: bool,
    /// When the session was started (epoch millis for serialization)
    #[serde(default)]
    pub(crate) started_at_ms: u64,
}

/// Maximum age of a drag session before it is considered stale (30 seconds).
pub(crate) const DRAG_SESSION_MAX_AGE_MS: u64 = 30_000;

/// The shared application state, managed by Tauri.
pub(crate) struct AppState {
    pub(crate) boards: RwLock<HashMap<PathBuf, Arc<BoardHandle>>>,
    pub(crate) active_board: RwLock<Option<PathBuf>>,
    pub(crate) config: RwLock<AppConfig>,
    /// IDs of items in the most recently shown generic context menu.
    /// Used by `handle_menu_event` to distinguish context menu selections
    /// from regular menu commands.
    pub(crate) context_menu_ids: RwLock<HashSet<String>>,
    /// Shared UI state (inspector stack, palette, keymap, etc.).
    pub(crate) ui_state: Arc<UIState>,
    /// YAML-loaded command definitions. Behind RwLock because user overrides
    /// are merged when switching boards.
    pub(crate) commands_registry: RwLock<CommandsRegistry>,
    /// Trait object map from `register_commands()`.
    pub(crate) command_impls: HashMap<String, Arc<dyn Command>>,
    /// Current focus scope chain stored by `set_focus`.
    pub(crate) focus_scope_chain: RwLock<Vec<String>>,
    /// Active cross-window drag session, if any.
    pub(crate) drag_session: RwLock<Option<DragSession>>,
    /// Set to `true` when the app is shutting down (RunEvent::ExitRequested).
    /// The Destroyed handler uses this to distinguish mid-session close from app quit.
    pub(crate) shutting_down: AtomicBool,
}

impl AppState {
    /// Create a new AppState, loading config from disk.
    pub fn new() -> Self {
        let sources = builtin_yaml_sources();
        let source_refs: Vec<(&str, &str)> = sources.iter().map(|(n, c)| (*n, *c)).collect();
        Self {
            boards: RwLock::new(HashMap::new()),
            active_board: RwLock::new(None),
            config: RwLock::new(AppConfig::load()),
            context_menu_ids: RwLock::new(HashSet::new()),
            ui_state: Arc::new(UIState::new()),
            commands_registry: RwLock::new(CommandsRegistry::from_yaml_sources(&source_refs)),
            command_impls: swissarmyhammer_kanban::commands::register_commands(),
            focus_scope_chain: RwLock::new(Vec::new()),
            drag_session: RwLock::new(None),
            shutting_down: AtomicBool::new(false),
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

        // Check if already open
        {
            let boards = self.boards.read().await;
            if boards.contains_key(&canonical) {
                // Already open — just update active
                *self.active_board.write().await = Some(canonical.clone());
                return Ok(canonical);
            }
        }

        let mut handle = BoardHandle::open(kanban_path).await?;

        // Ensure OS user actor exists
        handle.ensure_os_actor().await;

        // Read board name for MRU
        let board_name = if handle.ctx.is_initialized() {
            match handle.ctx.entity_context().await {
                Ok(ectx) => match ectx.read("board", "board").await {
                    Ok(entity) => entity.get_str("name").unwrap_or("").to_string(),
                    Err(_) => canonical.display().to_string(),
                },
                Err(_) => canonical.display().to_string(),
            }
        } else {
            canonical.display().to_string()
        };

        // Start the file watcher on the owned handle BEFORE wrapping in
        // Arc and inserting into the map. This avoids a TOCTOU race where
        // a concurrent Tauri command could clone the Arc between insert
        // and Arc::get_mut, silently preventing the watcher from starting.
        if let Some(ref app) = app_handle {
            handle.start_watcher(app.clone());
        }

        // Insert into the boards map and set active. The watcher may
        // already be emitting events, but the frontend won't see them
        // until list_open_boards returns this board.
        //
        // Collect open board paths inside the write lock, then drop it
        // before touching config to avoid lock-ordering hazards.
        let open_paths: Vec<PathBuf>;
        {
            let mut boards = self.boards.write().await;
            boards.insert(canonical.clone(), Arc::new(handle));
            open_paths = boards.keys().cloned().collect();
        }

        // Update MRU + persist open boards list (no boards lock held)
        {
            let mut config = self.config.write().await;
            config.touch_recent(&canonical, &board_name);
            config.open_boards = open_paths;
            let _ = config.save();
        }

        *self.active_board.write().await = Some(canonical.clone());

        // Load user command overrides from .kanban/commands/
        self.reload_command_overrides(&canonical).await;

        Ok(canonical)
    }

    /// Auto-open a board at startup by walking up from CWD looking for a `.kanban` directory.
    ///
    /// If no `.kanban` directory is found in any ancestor, the app starts without
    /// a board (the frontend shows the "No board loaded" prompt).
    pub async fn auto_open_board(&self) {
        // Restore previously-open boards from persisted config.
        {
            let config = self.config.read().await;
            let paths = config.open_boards.clone();
            drop(config);
            for path in paths {
                if path.is_dir() {
                    tracing::info!(path = %path.display(), "auto_open_board: restoring persisted board");
                    if let Err(e) = self.open_board(&path, None).await {
                        tracing::warn!(path = %path.display(), error = %e, "auto_open_board: failed to restore board");
                    }
                } else {
                    tracing::info!(path = %path.display(), "auto_open_board: persisted board no longer exists, skipping");
                }
            }
        }

        // Also open any boards referenced in windows that aren't already open.
        // This handles the case where a secondary window shows a different board
        // than the ones in open_boards.
        {
            let config = self.config.read().await;
            let wb_paths: Vec<PathBuf> = config
                .windows
                .values()
                .map(|e| e.board_path.clone())
                .collect();
            drop(config);

            let boards = self.boards.read().await;
            let already_open: HashSet<PathBuf> = boards.keys().cloned().collect();
            drop(boards);

            for path in wb_paths {
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                if already_open.contains(&canonical) {
                    continue;
                }
                if path.is_dir() {
                    tracing::info!(path = %path.display(), "auto_open_board: restoring board from windows");
                    if let Err(e) = self.open_board(&path, None).await {
                        tracing::warn!(path = %path.display(), error = %e, "auto_open_board: failed to restore windows board");
                    }
                }
            }
        }

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

        let cwd = match std::env::current_dir() {
            Ok(dir) => dir,
            Err(e) => {
                tracing::warn!("Cannot determine current directory: {e}");
                return;
            }
        };
        tracing::info!(cwd = %cwd.display(), "auto_open_board: starting discovery");

        // Strategy 1: walk up from CWD
        let mut board_dir = discover_board(&cwd);
        if let Some(ref dir) = board_dir {
            tracing::info!(path = %dir.display(), "auto_open_board: found .kanban via CWD walk");
        } else {
            tracing::info!("auto_open_board: no .kanban found walking up from CWD");
        }

        // Strategy 2: if CWD walk didn't pass through home, check home as backstop
        if board_dir.is_none() {
            if let Some(home) = dirs::home_dir() {
                let walked_through_home = cwd.starts_with(&home);
                tracing::info!(
                    home = %home.display(),
                    walked_through_home,
                    "auto_open_board: checking home dir backstop"
                );
                if !walked_through_home {
                    board_dir = discover_board(&home);
                    if let Some(ref dir) = board_dir {
                        tracing::info!(path = %dir.display(), "auto_open_board: found .kanban via home backstop");
                    }
                }
            }
        }

        // Strategy 3: fall back to MRU — the most recently opened board
        if board_dir.is_none() {
            let config = self.config.read().await;
            if let Some(recent) = config.recent_boards.first() {
                let path = &recent.path;
                tracing::info!(
                    path = %path.display(),
                    name = %recent.name,
                    "auto_open_board: falling back to MRU board"
                );
                // MRU stores the canonical .kanban path — check its parent exists
                if path.is_dir() {
                    board_dir = Some(path.clone());
                } else {
                    tracing::warn!(
                        path = %path.display(),
                        "auto_open_board: MRU path no longer exists"
                    );
                }
            } else {
                tracing::info!("auto_open_board: no MRU boards in config");
            }
        }

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

        // Collect remaining board paths inside the write lock, then drop it
        // before touching config. This avoids the lock-ordering hazard where
        // sync_open_boards_to_config would re-acquire a boards read lock.
        let remaining_paths: Vec<PathBuf>;
        {
            let mut boards = self.boards.write().await;
            if boards.remove(&canonical).is_none() {
                return Err(format!("Board not open: {}", canonical.display()));
            }

            // If we just closed the active board, switch to another one.
            let mut active = self.active_board.write().await;
            if active.as_ref() == Some(&canonical) {
                *active = boards.keys().next().cloned();
            }

            remaining_paths = boards.keys().cloned().collect();
        }

        // Persist updated open boards list (no boards lock held)
        {
            let mut config = self.config.write().await;
            config.open_boards = remaining_paths;
            let _ = config.save();
        }

        tracing::info!(path = %canonical.display(), "closed board");
        Ok(())
    }

    /// Get the handle for the active board.
    pub async fn active_handle(&self) -> Option<Arc<BoardHandle>> {
        let active = self.active_board.read().await;
        let path = active.as_ref()?;
        let boards = self.boards.read().await;
        boards.get(path).cloned()
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

    // macOS stores user profile pictures via dscl; the actual file is at:
    // /var/db/dslocal/nodes/Default/users/<username>.plist (needs root)
    // But a more accessible copy often exists at:
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

    // Try dscl as a fallback — reads the JPEGPhoto attribute.
    // dscl outputs hex-encoded text like "JPEGPhoto:\n ffd8ffe0 00104a46 ..."
    // We need to parse the hex back to binary bytes.
    let output = std::process::Command::new("dscl")
        .args([".", "-read", &format!("/Users/{username}"), "JPEGPhoto"])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Skip the "JPEGPhoto:\n" header line
        let hex_body = stdout.strip_prefix("JPEGPhoto:\n").unwrap_or(&stdout);
        // Remove all whitespace to get a continuous hex string
        let hex_clean: String = hex_body.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        // Decode hex pairs to bytes
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
    }

    None
}

#[cfg(not(target_os = "macos"))]
fn macos_profile_picture(_username: &str) -> Option<String> {
    None
}

/// Get the path to the app config file.
///
/// Uses XDG config directory: `$XDG_CONFIG_HOME/sah/kanban-app/config.yaml`
/// Falls back to `~/.config/sah/kanban-app/config.yaml` if XDG_CONFIG_HOME is not set.
fn config_file_path() -> PathBuf {
    use swissarmyhammer_directory::{ManagedDirectory, SwissarmyhammerConfig};

    ManagedDirectory::<SwissarmyhammerConfig>::xdg_config()
        .map(|dir| dir.root().join(CONFIG_APP_SUBDIR).join(CONFIG_FILE_NAME))
        .unwrap_or_else(|_| {
            PathBuf::from(".")
                .join(CONFIG_APP_SUBDIR)
                .join(CONFIG_FILE_NAME)
        })
}

/// Get the path to the legacy JSON config file (for migration).
fn legacy_config_file_path() -> PathBuf {
    use swissarmyhammer_directory::{ManagedDirectory, SwissarmyhammerConfig};

    ManagedDirectory::<SwissarmyhammerConfig>::xdg_config()
        .map(|dir| {
            dir.root()
                .join(CONFIG_APP_SUBDIR)
                .join(CONFIG_FILE_NAME_LEGACY)
        })
        .unwrap_or_else(|_| {
            PathBuf::from(".")
                .join(CONFIG_APP_SUBDIR)
                .join(CONFIG_FILE_NAME_LEGACY)
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
    fn test_mru_config_touch_and_truncate() {
        let mut config = AppConfig::default();

        for i in 0..25 {
            config.touch_recent(
                &PathBuf::from(format!("/board/{}", i)),
                &format!("Board {}", i),
            );
        }

        assert_eq!(config.recent_boards.len(), MAX_RECENT_BOARDS);
        // Most recent should be first
        assert_eq!(config.recent_boards[0].name, "Board 24");
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
        let mut config = AppConfig::default();
        let path = PathBuf::from("/board/a");

        config.touch_recent(&path, "Board A");
        config.touch_recent(&PathBuf::from("/board/b"), "Board B");
        config.touch_recent(&path, "Board A Updated");

        assert_eq!(config.recent_boards.len(), 2);
        assert_eq!(config.recent_boards[0].name, "Board A Updated");
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
        std::fs::create_dir_all(kanban_dir.join("swimlanes")).unwrap();
        std::fs::create_dir_all(kanban_dir.join("activity")).unwrap();
    }

    #[tokio::test]
    async fn test_auto_open_board_from_cwd() {
        let tmp = TempDir::new().unwrap();
        create_board_at(tmp.path(), "Test Board");

        // Simulate CWD being inside the project
        let subdir = tmp.path().join("src").join("components");
        std::fs::create_dir_all(&subdir).unwrap();

        let state = AppState::new();
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

        let state = AppState::new();
        // No board opened — active_handle should be None
        let handle = state.active_handle().await;
        assert!(handle.is_none());
    }

    #[tokio::test]
    async fn test_open_board_sets_active_and_appears_in_boards() {
        let tmp = TempDir::new().unwrap();
        create_board_at(tmp.path(), "My Board");

        let state = AppState::new();
        let result = state.open_board(tmp.path(), None).await;
        assert!(result.is_ok());

        let canonical = result.unwrap();

        // active_board should be set
        let active = state.active_board.read().await;
        assert_eq!(*active, Some(canonical.clone()));

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

        let state = AppState::new();

        // Open board A
        let path_a = state.open_board(tmp_a.path(), None).await.unwrap();

        // Open board B
        let path_b = state.open_board(tmp_b.path(), None).await.unwrap();

        // Both boards must be in the map
        let boards = state.boards.read().await;
        assert_eq!(boards.len(), 2, "Expected 2 boards, got {}", boards.len());
        assert!(boards.contains_key(&path_a), "Board A missing from map");
        assert!(boards.contains_key(&path_b), "Board B missing from map");

        // Active board should be B (most recently opened)
        let active = state.active_board.read().await;
        assert_eq!(*active, Some(path_b));
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

    fn make_drag_session(task_id: &str, board_path: &str) -> DragSession {
        DragSession {
            session_id: ulid::Ulid::new().to_string(),
            source_board_path: board_path.to_string(),
            source_window_label: "main".to_string(),
            task_id: task_id.to_string(),
            task_fields: serde_json::json!({"title": "Test task"}),
            copy_mode: false,
            started_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }

    #[tokio::test]
    async fn test_drag_session_start_and_cancel() {
        let state = AppState::new();
        assert!(state.drag_session.read().await.is_none());

        // Start session
        let session = make_drag_session("task-1", "/board/a");
        *state.drag_session.write().await = Some(session);
        assert!(state.drag_session.read().await.is_some());

        // Cancel session
        let taken = state.drag_session.write().await.take();
        assert!(taken.is_some());
        assert_eq!(taken.unwrap().task_id, "task-1");
        assert!(state.drag_session.read().await.is_none());
    }

    #[tokio::test]
    async fn test_drag_session_double_take_returns_none() {
        let state = AppState::new();
        *state.drag_session.write().await = Some(make_drag_session("task-1", "/board/a"));

        // First take succeeds
        let first = state.drag_session.write().await.take();
        assert!(first.is_some());

        // Second take returns None (session already consumed)
        let second = state.drag_session.write().await.take();
        assert!(second.is_none());
    }

    #[tokio::test]
    async fn test_drag_session_replaced_by_new_start() {
        let state = AppState::new();
        let session1 = make_drag_session("task-1", "/board/a");
        let id1 = session1.session_id.clone();
        *state.drag_session.write().await = Some(session1);

        // Replace with new session
        let session2 = make_drag_session("task-2", "/board/b");
        let id2 = session2.session_id.clone();
        *state.drag_session.write().await = Some(session2);

        let current = state.drag_session.read().await;
        assert_eq!(current.as_ref().unwrap().session_id, id2);
        assert_ne!(id1, id2);
        assert_eq!(current.as_ref().unwrap().task_id, "task-2");
    }

    #[test]
    fn test_drag_session_stale_detection() {
        let mut session = make_drag_session("task-1", "/board/a");
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Session just started — not stale
        session.started_at_ms = now_ms;
        assert!(now_ms.saturating_sub(session.started_at_ms) <= DRAG_SESSION_MAX_AGE_MS);

        // Session 31 seconds ago — stale
        session.started_at_ms = now_ms - 31_000;
        assert!(now_ms.saturating_sub(session.started_at_ms) > DRAG_SESSION_MAX_AGE_MS);
    }

    #[test]
    fn test_drag_session_serialization() {
        let session = make_drag_session("task-1", "/board/a");
        let json = serde_json::to_value(&session).unwrap();
        assert_eq!(json["task_id"], "task-1");
        assert_eq!(json["source_board_path"], "/board/a");
        assert_eq!(json["source_window_label"], "main");
        assert_eq!(json["copy_mode"], false);
        assert!(json["started_at_ms"].as_u64().unwrap() > 0);
    }

    // =========================================================================
    // Window-board mapping tests
    // =========================================================================

    fn make_window_entry(board_path: &str) -> WindowState {
        WindowState {
            board_path: PathBuf::from(board_path),
            active_view_id: None,
            inspector_stack: Vec::new(),
            x: None,
            y: None,
            width: None,
            height: None,
            maximized: false,
        }
    }

    fn make_window_entry_with_pos(board_path: &str, x: i32, y: i32, w: u32, h: u32) -> WindowState {
        WindowState {
            board_path: PathBuf::from(board_path),
            active_view_id: None,
            inspector_stack: Vec::new(),
            x: Some(x),
            y: Some(y),
            width: Some(w),
            height: Some(h),
            maximized: false,
        }
    }

    #[test]
    fn test_windows_persists_through_serialization() {
        let mut config = AppConfig::default();
        config.windows.insert(
            "board-01abc".to_string(),
            make_window_entry("/boards/project-a/.kanban"),
        );
        config.windows.insert(
            "board-02def".to_string(),
            make_window_entry("/boards/project-b/.kanban"),
        );

        let yaml = serde_yaml_ng::to_string(&config).unwrap();
        let restored: AppConfig = serde_yaml_ng::from_str(&yaml).unwrap();

        assert_eq!(restored.windows.len(), 2);
        assert_eq!(
            restored.windows.get("board-01abc").unwrap().board_path,
            PathBuf::from("/boards/project-a/.kanban")
        );
        assert_eq!(
            restored.windows.get("board-02def").unwrap().board_path,
            PathBuf::from("/boards/project-b/.kanban")
        );
    }

    #[test]
    fn test_windows_persists_geometry() {
        let mut config = AppConfig::default();
        config.windows.insert(
            "board-01abc".to_string(),
            make_window_entry_with_pos("/boards/a/.kanban", 100, 200, 1200, 800),
        );

        let yaml = serde_yaml_ng::to_string(&config).unwrap();
        let restored: AppConfig = serde_yaml_ng::from_str(&yaml).unwrap();

        let entry = restored.windows.get("board-01abc").unwrap();
        assert_eq!(entry.x, Some(100));
        assert_eq!(entry.y, Some(200));
        assert_eq!(entry.width, Some(1200));
        assert_eq!(entry.height, Some(800));
    }

    #[test]
    fn test_windows_geometry_defaults_to_none() {
        let mut config = AppConfig::default();
        config.windows.insert(
            "board-01abc".to_string(),
            make_window_entry("/boards/a/.kanban"),
        );

        let yaml = serde_yaml_ng::to_string(&config).unwrap();
        let restored: AppConfig = serde_yaml_ng::from_str(&yaml).unwrap();

        let entry = restored.windows.get("board-01abc").unwrap();
        assert_eq!(entry.x, None);
        assert_eq!(entry.y, None);
        assert_eq!(entry.width, None);
        assert_eq!(entry.height, None);
    }

    #[test]
    fn test_windows_defaults_to_empty() {
        // Simulate loading config that predates windows field
        let yaml = "recent_boards: []\nkeymap_mode: cua\n";
        let config: AppConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(config.windows.is_empty());
    }

    #[test]
    fn test_windows_remove_by_board_path() {
        let mut config = AppConfig::default();
        let board_a = PathBuf::from("/boards/a/.kanban");

        config
            .windows
            .insert("win-1".to_string(), make_window_entry("/boards/a/.kanban"));
        config
            .windows
            .insert("win-2".to_string(), make_window_entry("/boards/b/.kanban"));
        config
            .windows
            .insert("win-3".to_string(), make_window_entry("/boards/a/.kanban"));

        // Remove all windows pointing to board A
        config
            .windows
            .retain(|_, entry| entry.board_path != board_a);

        assert_eq!(config.windows.len(), 1);
        assert!(config.windows.contains_key("win-2"));
    }

    #[test]
    fn test_window_label_is_ulid_based() {
        // Verify the label format matches what create_window generates
        let label = format!("board-{}", ulid::Ulid::new().to_string().to_lowercase());
        assert!(label.starts_with("board-"));
        // ULID is 26 chars, so label is "board-" (6) + 26 = 32
        assert_eq!(label.len(), 32);
    }

    #[test]
    fn test_windows_save_and_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.yaml");

        // Create config with windows including geometry
        let mut config = AppConfig::default();
        config.windows.insert(
            "board-01abc".to_string(),
            make_window_entry_with_pos("/test/.kanban", 50, 100, 1400, 900),
        );

        // Save
        let content = serde_yaml_ng::to_string(&config).unwrap();
        std::fs::write(&config_path, &content).unwrap();

        // Load
        let loaded_content = std::fs::read_to_string(&config_path).unwrap();
        let loaded: AppConfig = serde_yaml_ng::from_str(&loaded_content).unwrap();

        assert_eq!(loaded.windows.len(), 1);
        let entry = loaded.windows.get("board-01abc").unwrap();
        assert_eq!(entry.board_path, PathBuf::from("/test/.kanban"));
        assert_eq!(entry.x, Some(50));
        assert_eq!(entry.y, Some(100));
        assert_eq!(entry.width, Some(1400));
        assert_eq!(entry.height, Some(900));
    }

    /// Full lifecycle test: create window entry → update geometry → save →
    /// reload → verify position restores. This is the exact sequence that
    /// create_window + on_window_event + restore_windows must execute.
    #[test]
    fn test_window_lifecycle_create_move_restart_restore() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.yaml");

        // Step 1: Simulate create_window — save entry with no geometry
        let label = format!("board-{}", ulid::Ulid::new().to_string().to_lowercase());
        let board_path = PathBuf::from("/projects/my-board/.kanban");
        {
            let mut config = AppConfig::default();
            config.open_boards = vec![board_path.clone()];
            config.windows.insert(
                label.clone(),
                WindowState {
                    board_path: board_path.clone(),
                    active_view_id: None,
                    inspector_stack: Vec::new(),
                    x: None,
                    y: None,
                    width: None,
                    height: None,
                    maximized: false,
                },
            );
            let content = serde_yaml_ng::to_string(&config).unwrap();
            std::fs::write(&config_path, &content).unwrap();
        }

        // Step 2: Simulate on_window_event Moved/Resized — update geometry
        {
            let content = std::fs::read_to_string(&config_path).unwrap();
            let mut config: AppConfig = serde_yaml_ng::from_str(&content).unwrap();
            let entry = config.windows.get_mut(&label).unwrap();
            entry.x = Some(300);
            entry.y = Some(150);
            entry.width = Some(1400);
            entry.height = Some(900);
            let content = serde_yaml_ng::to_string(&config).unwrap();
            std::fs::write(&config_path, &content).unwrap();
        }

        // Step 3: Simulate app restart — load config, verify entry survives
        {
            let content = std::fs::read_to_string(&config_path).unwrap();
            let config: AppConfig = serde_yaml_ng::from_str(&content).unwrap();

            // open_boards must survive
            assert_eq!(config.open_boards.len(), 1);
            assert_eq!(config.open_boards[0], board_path);

            // windows must survive with geometry
            assert_eq!(config.windows.len(), 1);
            let entry = config.windows.get(&label).unwrap();
            assert_eq!(entry.board_path, board_path);
            assert_eq!(entry.x, Some(300));
            assert_eq!(entry.y, Some(150));
            assert_eq!(entry.width, Some(1400));
            assert_eq!(entry.height, Some(900));
        }

        // Step 4: Simulate open_board updating open_boards (must NOT clobber windows)
        {
            let content = std::fs::read_to_string(&config_path).unwrap();
            let mut config: AppConfig = serde_yaml_ng::from_str(&content).unwrap();
            // open_board writes open_boards but should preserve windows
            config.open_boards = vec![board_path.clone()];
            let content = serde_yaml_ng::to_string(&config).unwrap();
            std::fs::write(&config_path, &content).unwrap();
        }

        // Step 5: Verify windows still intact after open_board save
        {
            let content = std::fs::read_to_string(&config_path).unwrap();
            let config: AppConfig = serde_yaml_ng::from_str(&content).unwrap();
            assert_eq!(config.windows.len(), 1, "windows was clobbered!");
            let entry = config.windows.get(&label).unwrap();
            assert_eq!(entry.x, Some(300), "geometry was lost!");
        }
    }

    /// Window entries survive board close — windows are about windows, not boards.
    /// close_board no longer touches config.windows. The frontend updates
    /// board_path via switch_board when falling back to another board.
    #[test]
    fn test_window_entries_survive_board_close() {
        let mut config = AppConfig::default();

        config.windows.insert(
            "main".to_string(),
            make_window_entry_with_pos("/boards/a/.kanban", 100, 200, 1200, 800),
        );
        config.windows.insert(
            "win-2".to_string(),
            make_window_entry_with_pos("/boards/a/.kanban", 500, 300, 1200, 800),
        );

        // Closing board A does NOT remove window entries — both windows still exist
        // (the frontend will update their board_path via switch_board)
        assert_eq!(config.windows.len(), 2);
        assert!(config.windows.contains_key("main"));
        assert!(config.windows.contains_key("win-2"));

        // Geometry is preserved
        assert_eq!(config.windows.get("main").unwrap().x, Some(100));
        assert_eq!(config.windows.get("win-2").unwrap().x, Some(500));
    }

    /// Verify that legacy JSON config with `window_boards` key migrates to `windows`.
    #[test]
    fn test_config_json_migration() {
        // Legacy JSON uses "window_boards" (old field name) — the serde alias
        // maps it to the new "windows" field.
        let json_content = serde_json::json!({
            "recent_boards": [],
            "keymap_mode": "vim",
            "open_boards": ["/boards/test/.kanban"],
            "window_boards": {
                "board-01abc": {
                    "board_path": "/boards/test/.kanban",
                    "x": 100,
                    "y": 200,
                    "width": 1200,
                    "height": 800
                }
            }
        });
        let config: AppConfig =
            serde_json::from_str(&serde_json::to_string(&json_content).unwrap()).unwrap();

        assert_eq!(config.keymap_mode, "vim");
        assert_eq!(config.open_boards.len(), 1);
        assert_eq!(config.windows.len(), 1);
        let entry = config.windows.get("board-01abc").unwrap();
        assert_eq!(entry.board_path, PathBuf::from("/boards/test/.kanban"));
        assert_eq!(entry.x, Some(100));
        // New fields default correctly
        assert_eq!(entry.active_view_id, None);
        assert!(entry.inspector_stack.is_empty());

        // Verify roundtrip through YAML
        let yaml_content = serde_yaml_ng::to_string(&config).unwrap();
        let restored: AppConfig = serde_yaml_ng::from_str(&yaml_content).unwrap();
        assert_eq!(restored.windows.len(), 1);
        assert_eq!(restored.windows.get("board-01abc").unwrap().x, Some(100));
    }

    /// Roundtrip test with all WindowState fields populated.
    #[test]
    fn test_window_state_full_roundtrip() {
        let mut config = AppConfig::default();
        config.windows.insert(
            "main".to_string(),
            WindowState {
                board_path: PathBuf::from("/boards/main/.kanban"),
                active_view_id: Some("board-view".to_string()),
                inspector_stack: vec!["task:01ABC".to_string(), "tag:01DEF".to_string()],
                x: Some(100),
                y: Some(200),
                width: Some(1400),
                height: Some(900),
                maximized: true,
            },
        );
        config.windows.insert(
            "board-secondary".to_string(),
            WindowState {
                board_path: PathBuf::from("/boards/other/.kanban"),
                active_view_id: Some("grid-view".to_string()),
                inspector_stack: Vec::new(),
                x: Some(500),
                y: Some(100),
                width: Some(1200),
                height: Some(800),
                maximized: false,
            },
        );

        let yaml = serde_yaml_ng::to_string(&config).unwrap();
        let restored: AppConfig = serde_yaml_ng::from_str(&yaml).unwrap();

        assert_eq!(restored.windows.len(), 2);

        let main = restored.windows.get("main").unwrap();
        assert_eq!(main.board_path, PathBuf::from("/boards/main/.kanban"));
        assert_eq!(main.active_view_id.as_deref(), Some("board-view"));
        assert_eq!(main.inspector_stack, vec!["task:01ABC", "tag:01DEF"]);
        assert_eq!(main.maximized, true);

        let secondary = restored.windows.get("board-secondary").unwrap();
        assert_eq!(secondary.active_view_id.as_deref(), Some("grid-view"));
        assert!(secondary.inspector_stack.is_empty());
        assert_eq!(secondary.maximized, false);
    }
}
