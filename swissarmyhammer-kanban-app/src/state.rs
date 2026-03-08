//! Application state management with multi-board support and MRU persistence.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use swissarmyhammer_commands::{
    builtin_yaml_sources, load_yaml_dir, Command, CommandsRegistry, UIState,
};
use swissarmyhammer_kanban::{KanbanContext, KanbanOperationProcessor};
use tokio::sync::RwLock;

use swissarmyhammer_kanban::actor::AddActor;
use swissarmyhammer_kanban::Execute;

use crate::watcher::{self, BoardWatcher, EntityCache};

const MAX_RECENT_BOARDS: usize = 20;
const CONFIG_DIR_NAME: &str = "swissarmyhammer-kanban";
const CONFIG_FILE_NAME: &str = "config.json";

/// A handle to a single open kanban board.
pub struct BoardHandle {
    pub ctx: Arc<KanbanContext>,
    pub processor: KanbanOperationProcessor,
    /// Entity cache for detecting external file changes with field-level diffing.
    pub entity_cache: EntityCache,
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
        Ok(Self {
            ctx: Arc::new(ctx),
            processor: KanbanOperationProcessor::new(),
            entity_cache,
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
        let avatar = initials_svg_avatar(&realname, &color);

        let cmd = AddActor::new(username.as_str(), realname.as_str())
            .with_ensure()
            .with_color(&color)
            .with_avatar(avatar);

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

        match watcher::start_watching(kanban_root.clone(), cache, move |evt| {
            use tauri::Emitter;
            let event_name = match &evt {
                watcher::WatchEvent::EntityCreated { .. } => "entity-created",
                watcher::WatchEvent::EntityRemoved { .. } => "entity-removed",
                watcher::WatchEvent::EntityFieldChanged { .. } => "entity-field-changed",
            };
            let _ = app_handle.emit(event_name, &evt);
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
pub struct RecentBoard {
    pub path: PathBuf,
    pub name: String,
    pub last_opened: DateTime<Utc>,
}

/// Persisted app configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub recent_boards: Vec<RecentBoard>,
    #[serde(default = "default_keymap_mode")]
    pub keymap_mode: String,
    /// Last active view ID — restored on reload, falls back to first view if invalid.
    #[serde(default)]
    pub active_view_id: Option<String>,
    /// Inspector panel stack as monikers (e.g. ["task:01XYZ"]) — restored on reload,
    /// entries that no longer resolve are silently dropped.
    #[serde(default)]
    pub inspector_stack: Vec<String>,
}

fn default_keymap_mode() -> String {
    "cua".to_string()
}

impl AppConfig {
    /// Load config from disk, returning default if not found.
    pub fn load() -> Self {
        let path = config_file_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save config to disk.
    pub fn save(&self) -> std::io::Result<()> {
        let path = config_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
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

/// The shared application state, managed by Tauri.
pub struct AppState {
    pub boards: RwLock<HashMap<PathBuf, Arc<BoardHandle>>>,
    pub active_board: RwLock<Option<PathBuf>>,
    pub config: RwLock<AppConfig>,
    /// IDs of items in the most recently shown generic context menu.
    /// Used by `handle_menu_event` to distinguish context menu selections
    /// from regular menu commands.
    pub context_menu_ids: RwLock<HashSet<String>>,
    /// Shared UI state (inspector stack, palette, keymap, etc.).
    pub ui_state: Arc<UIState>,
    /// YAML-loaded command definitions. Behind RwLock because user overrides
    /// are merged when switching boards.
    pub commands_registry: RwLock<CommandsRegistry>,
    /// Trait object map from `register_commands()`.
    pub command_impls: HashMap<String, Arc<dyn Command>>,
    /// Current focus scope chain stored by `set_focus`.
    pub focus_scope_chain: RwLock<Vec<String>>,
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

        // Start file watcher if we have an app handle
        if let Some(app) = app_handle {
            handle.start_watcher(app);
        }

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

        {
            let mut boards = self.boards.write().await;
            boards.insert(canonical.clone(), Arc::new(handle));
        }

        // Update MRU
        {
            let mut config = self.config.write().await;
            config.touch_recent(&canonical, &board_name);
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
    "e53e3e", "dd6b20", "d69e2e", "38a169", "319795",
    "3182ce", "5a67d8", "805ad5", "d53f8c", "2b6cb0",
    "c05621", "2f855a", "2c7a7b", "6b46c1", "b83280",
];

/// Derive a deterministic hex color from a username.
fn deterministic_color(username: &str) -> String {
    let hash: u64 = username.bytes().fold(5381u64, |h, b| h.wrapping_mul(33).wrapping_add(b as u64));
    ACTOR_COLORS[(hash as usize) % ACTOR_COLORS.len()].to_string()
}

/// Generate an initials-based SVG avatar as a data URI.
fn initials_svg_avatar(name: &str, color: &str) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let initials: String = name
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .flat_map(|c| c.to_uppercase())
        .collect();
    let initials = if initials.is_empty() {
        "?".to_string()
    } else {
        initials
    };

    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"64\" height=\"64\" viewBox=\"0 0 64 64\">\
         <circle cx=\"32\" cy=\"32\" r=\"32\" fill=\"#{color}\"/>\
         <text x=\"32\" y=\"32\" text-anchor=\"middle\" dy=\".35em\" fill=\"white\" \
         font-family=\"system-ui,sans-serif\" font-size=\"24\" font-weight=\"600\">\
         {initials}</text></svg>",
    );

    format!(
        "data:image/svg+xml;base64,{}",
        STANDARD.encode(svg.as_bytes())
    )
}

/// Get the path to the app config file.
fn config_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(CONFIG_DIR_NAME)
        .join(CONFIG_FILE_NAME)
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

    #[test]
    fn test_initials_svg_avatar_format() {
        let avatar = initials_svg_avatar("Alice Smith", "e53e3e");
        assert!(avatar.starts_with("data:image/svg+xml;base64,"));
        // Decode and check SVG content
        use base64::{engine::general_purpose::STANDARD, Engine};
        let b64 = avatar.strip_prefix("data:image/svg+xml;base64,").unwrap();
        let svg = String::from_utf8(STANDARD.decode(b64).unwrap()).unwrap();
        assert!(svg.contains("AS")); // initials
        assert!(svg.contains("#e53e3e")); // color
    }

    #[test]
    fn test_initials_svg_single_name() {
        let avatar = initials_svg_avatar("Alice", "3182ce");
        use base64::{engine::general_purpose::STANDARD, Engine};
        let b64 = avatar.strip_prefix("data:image/svg+xml;base64,").unwrap();
        let svg = String::from_utf8(STANDARD.decode(b64).unwrap()).unwrap();
        assert!(svg.contains(">A<")); // single initial
    }

    #[test]
    fn test_initials_svg_empty_name() {
        let avatar = initials_svg_avatar("", "3182ce");
        use base64::{engine::general_purpose::STANDARD, Engine};
        let b64 = avatar.strip_prefix("data:image/svg+xml;base64,").unwrap();
        let svg = String::from_utf8(STANDARD.decode(b64).unwrap()).unwrap();
        assert!(svg.contains("?"));
    }
}
