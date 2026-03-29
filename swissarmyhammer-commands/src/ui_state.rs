use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

/// Maximum number of entries to keep in the MRU recent boards list.
const MAX_RECENT_BOARDS: usize = 20;

/// Active drag session for cross-window drag coordination.
///
/// Transient — carried in UIState but never persisted to the YAML config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DragSession {
    /// Unique session ID (ULID).
    pub session_id: String,
    /// Board path the task originates from.
    pub source_board_path: String,
    /// Tauri window label of the source window.
    pub source_window_label: String,
    /// The task ID being dragged.
    pub task_id: String,
    /// Serialized task fields for ghost preview in target windows.
    pub task_fields: serde_json::Value,
    /// Whether Alt/Option was held (copy mode).
    pub copy_mode: bool,
    /// When the session was started (epoch millis).
    #[serde(default)]
    pub started_at_ms: u64,
}

/// Whether the clipboard entry was created by a copy or a cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipboardMode {
    /// The entity was copied (original remains).
    Copy,
    /// The entity was cut (original was deleted).
    Cut,
}

/// An in-memory clipboard snapshot of an entity's fields.
///
/// Transient — carried in UIState but never persisted to the YAML config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardState {
    /// How the clipboard entry was created.
    pub mode: ClipboardMode,
    /// The entity type that was copied/cut (e.g. "task").
    pub entity_type: String,
    /// The original entity ID.
    pub entity_id: String,
    /// Snapshot of all entity fields as JSON.
    pub fields: serde_json::Value,
}

/// Persisted per-window state: board path, inspector stack, active view, and window geometry.
///
/// `board_path` is the canonical path to the `.kanban` directory this window shows.
/// An empty string means no board is assigned to this window.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WindowState {
    /// The board path assigned to this window (canonical `.kanban` dir path).
    /// Empty string means no board assigned.
    pub board_path: String,
    /// Per-window inspector stack (list of `type:id` monikers).
    pub inspector_stack: Vec<String>,
    /// The active view ID for this window (e.g. "board-view", "grid-view").
    pub active_view_id: String,
    /// Window x position (physical pixels).
    pub x: Option<i32>,
    /// Window y position (physical pixels).
    pub y: Option<i32>,
    /// Window width (physical pixels).
    pub width: Option<u32>,
    /// Window height (physical pixels).
    pub height: Option<u32>,
    /// Whether the window is maximized.
    pub maximized: bool,
}

/// A recently opened board entry for MRU persistence.
///
/// Uses an ISO 8601 string for `last_opened` to avoid adding a chrono
/// dependency to this crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentBoard {
    /// Canonical path to the board directory.
    pub path: String,
    /// Human-readable board name.
    pub name: String,
    /// ISO 8601 timestamp of when the board was last opened.
    pub last_opened: String,
}

/// Payload returned by UIState mutation methods.
///
/// The caller (Tauri layer) uses this to decide which events to emit.
/// Each variant carries the new value after the mutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UIStateChange {
    /// The inspector stack changed; carries the full new stack.
    InspectorStack(Vec<String>),
    /// The active view changed; carries the new view ID.
    ActiveView(String),
    /// The palette open/closed state changed.
    PaletteOpen(bool),
    /// The keymap mode changed (e.g. "cua", "vim", "emacs").
    KeymapMode(String),
    /// The focus scope chain changed.
    ScopeChain(Vec<String>),
}

/// Pure state machine for UI state: inspector stack, active view, palette, keymap.
///
/// Thread-safe via internal `RwLock`. All mutation methods return a
/// `UIStateChange` describing what changed, so the caller can emit events.
/// Methods return `None` when the mutation would be a no-op.
///
/// When constructed via `UIState::load(path)`, mutations are automatically
/// persisted to the YAML config file. When constructed via `UIState::new()`,
/// no persistence occurs (suitable for tests).
pub struct UIState {
    inner: RwLock<UIStateInner>,
    /// Path to the YAML config file, if persistence is enabled.
    config_path: Option<PathBuf>,
}

/// Interior mutable state behind the RwLock.
///
/// Fields marked `#[serde(skip)]` are transient — they reset on restart
/// and are not written to the config file.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct UIStateInner {
    /// Whether the command palette is open. Transient — not persisted.
    #[serde(skip)]
    palette_open: bool,
    /// Current keymap mode: "cua", "vim", or "emacs".
    keymap_mode: String,
    /// Current focus scope chain (innermost first). Transient — not persisted.
    #[serde(skip)]
    scope_chain: Vec<String>,
    /// Active cross-window drag session. Transient — not persisted.
    #[serde(skip)]
    drag_session: Option<DragSession>,
    /// IDs of items in the most recently shown context menu. Transient — not persisted.
    #[serde(skip)]
    context_menu_ids: HashSet<String>,
    /// In-memory clipboard for entity copy/cut. Transient — not persisted.
    #[serde(skip)]
    clipboard: Option<ClipboardState>,
    /// Canonical paths of boards that are open.
    open_boards: Vec<String>,
    /// Per-window state: inspector stack, board assignment, and geometry.
    #[serde(default)]
    windows: HashMap<String, WindowState>,
    /// Most-recently-used board list, most recent first.
    #[serde(default)]
    recent_boards: Vec<RecentBoard>,
    /// Path of the most recently focused board window. Persisted — survives restarts.
    ///
    /// Updated when a window gains focus (WindowEvent::Focused) or when
    /// `file.switchBoard` runs. Used by quick capture and as the default
    /// board for commands that don't specify an explicit board_path.
    #[serde(default)]
    most_recent_board_path: Option<String>,
}

impl Default for UIStateInner {
    /// Returns the default UI state values.
    fn default() -> Self {
        Self {
            palette_open: false,
            keymap_mode: "cua".to_string(),
            scope_chain: Vec::new(),
            drag_session: None,
            context_menu_ids: HashSet::new(),
            clipboard: None,
            open_boards: Vec::new(),
            windows: HashMap::new(),
            recent_boards: Vec::new(),
            most_recent_board_path: None,
        }
    }
}

impl UIState {
    /// Create a new UIState with default values and no persistence.
    ///
    /// Defaults: empty inspector stack, empty active_view_id, palette closed,
    /// keymap mode "cua", empty scope chain. Suitable for tests.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(UIStateInner::default()),
            config_path: None,
        }
    }

    /// Load UIState from a YAML config file, or return defaults if the file is
    /// missing or malformed.
    ///
    /// Once loaded, all subsequent mutations will auto-save to the same path.
    /// Parent directories are created on first save if they don't exist.
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let inner = Self::read_from_file(&path);
        Self {
            inner: RwLock::new(inner),
            config_path: Some(path),
        }
    }

    /// Read state from a YAML file, returning defaults on any error.
    fn read_from_file(path: &Path) -> UIStateInner {
        match std::fs::read_to_string(path) {
            Ok(contents) => match serde_yaml_ng::from_str::<UIStateInner>(&contents) {
                Ok(inner) => inner,
                Err(err) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %err,
                        "UIState: failed to parse YAML config, using defaults"
                    );
                    UIStateInner::default()
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => UIStateInner::default(),
            Err(err) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %err,
                    "UIState: failed to read config file, using defaults"
                );
                UIStateInner::default()
            }
        }
    }

    /// Save current state to the configured YAML path.
    ///
    /// Creates parent directories if needed. Returns an error if writing fails.
    /// No-op if no config path was set (i.e. constructed via `UIState::new()`).
    pub fn save(&self) -> std::io::Result<()> {
        let Some(ref path) = self.config_path else {
            return Ok(());
        };
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        let yaml = serde_yaml_ng::to_string(&*inner)
            .map_err(|e| std::io::Error::other(format!("YAML serialization failed: {e}")))?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, yaml)
    }

    /// Try to save; log errors but never panic or propagate.
    ///
    /// Called internally after every persisted mutation.
    fn try_save(&self) {
        if let Err(err) = self.save() {
            tracing::warn!(error = %err, "UIState: failed to auto-save config");
        }
    }

    /// Open the inspector for the given moniker in the specified window.
    ///
    /// True stack: always pushes. If the moniker is already on top, no-op.
    /// If the moniker exists deeper in the stack, removes it and pushes to top.
    /// Auto-saves if a config path is configured.
    pub fn inspect(&self, window_label: &str, moniker: &str) -> UIStateChange {
        let change = {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            let stack = &mut inner
                .windows
                .entry(window_label.to_string())
                .or_default()
                .inspector_stack;

            // Already on top — no-op
            if stack.last().map(|s| s.as_str()) == Some(moniker) {
                return UIStateChange::InspectorStack(stack.clone());
            }

            // Remove if already in stack (moves to top)
            stack.retain(|m| m != moniker);
            stack.push(moniker.to_string());

            UIStateChange::InspectorStack(stack.clone())
        };
        self.try_save();
        change
    }

    /// Close the topmost inspector entry for the given window.
    ///
    /// Returns `None` if the stack was already empty.
    /// Auto-saves if a config path is configured.
    pub fn inspector_close(&self, window_label: &str) -> Option<UIStateChange> {
        let change = {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            let stack = &mut inner
                .windows
                .entry(window_label.to_string())
                .or_default()
                .inspector_stack;
            if stack.is_empty() {
                return None;
            }
            stack.pop();
            Some(UIStateChange::InspectorStack(stack.clone()))
        };
        self.try_save();
        change
    }

    /// Close all inspector entries for the given window.
    ///
    /// Returns `None` if the stack was already empty.
    /// Auto-saves if a config path is configured.
    pub fn inspector_close_all(&self, window_label: &str) -> Option<UIStateChange> {
        let change = {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            let stack = &mut inner
                .windows
                .entry(window_label.to_string())
                .or_default()
                .inspector_stack;
            if stack.is_empty() {
                return None;
            }
            stack.clear();
            Some(UIStateChange::InspectorStack(stack.clone()))
        };
        self.try_save();
        change
    }

    /// Set the active view ID for a specific window.
    ///
    /// Returns `None` if the view ID is unchanged.
    /// Auto-saves if a config path is configured.
    pub fn set_active_view(&self, window_label: &str, id: &str) -> Option<UIStateChange> {
        let change = {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            let ws = inner.windows.entry(window_label.to_string()).or_default();
            if ws.active_view_id == id {
                return None;
            }
            ws.active_view_id = id.to_string();
            Some(UIStateChange::ActiveView(id.to_string()))
        };
        self.try_save();
        change
    }

    /// Set whether the command palette is open.
    ///
    /// Returns `None` if the value is unchanged. Palette state is transient
    /// and is NOT persisted to the config file.
    pub fn set_palette_open(&self, open: bool) -> Option<UIStateChange> {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        if inner.palette_open == open {
            return None;
        }
        inner.palette_open = open;
        Some(UIStateChange::PaletteOpen(inner.palette_open))
        // No try_save — palette_open is transient (#[serde(skip)])
    }

    /// Set the keymap mode (e.g. "cua", "vim", "emacs").
    ///
    /// Returns `None` if the mode is unchanged.
    /// Auto-saves if a config path is configured.
    pub fn set_keymap_mode(&self, mode: &str) -> Option<UIStateChange> {
        let change = {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            if inner.keymap_mode == mode {
                return None;
            }
            inner.keymap_mode = mode.to_string();
            Some(UIStateChange::KeymapMode(inner.keymap_mode.clone()))
        };
        self.try_save();
        change
    }

    /// Set the focus scope chain. Always returns the new scope chain.
    ///
    /// Scope chain is transient and is NOT persisted to the config file.
    pub fn set_scope_chain(&self, chain: Vec<String>) -> UIStateChange {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.scope_chain = chain;
        UIStateChange::ScopeChain(inner.scope_chain.clone())
        // No try_save — scope_chain is transient (#[serde(skip)])
    }

    /// Start a drag session, replacing any existing one.
    ///
    /// Transient — not persisted to the config file.
    pub fn start_drag(&self, session: DragSession) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.drag_session = Some(session);
        // No try_save() — transient state
    }

    /// Take the current drag session (returns and clears it).
    ///
    /// Returns `None` if no session is active.
    pub fn take_drag(&self) -> Option<DragSession> {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.drag_session.take()
        // No try_save() — transient state
    }

    /// Cancel the current drag session (clears it without returning).
    ///
    /// Transient — not persisted to the config file.
    pub fn cancel_drag(&self) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.drag_session = None;
        // No try_save() — transient state
    }

    /// Store a clipboard snapshot (copy or cut).
    ///
    /// Replaces any previous clipboard entry. Transient — not persisted.
    pub fn set_clipboard(&self, state: ClipboardState) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.clipboard = Some(state);
        // No try_save() — transient state
    }

    /// Get a clone of the current clipboard state, if any.
    pub fn clipboard(&self) -> Option<ClipboardState> {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clipboard
            .clone()
    }

    /// Clear the clipboard. Transient — not persisted.
    pub fn clear_clipboard(&self) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.clipboard = None;
        // No try_save() — transient state
    }

    /// Get a clone of the current drag session, if any.
    pub fn drag_session(&self) -> Option<DragSession> {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .drag_session
            .clone()
    }

    /// Set the context menu IDs for the current menu.
    ///
    /// Replaces any previous set. Transient — not persisted to the config file.
    pub fn set_context_menu_ids(&self, ids: HashSet<String>) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.context_menu_ids = ids;
        // No try_save() — transient state
    }

    /// Check if a menu ID belongs to the current context menu.
    pub fn is_context_menu_id(&self, id: &str) -> bool {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .context_menu_ids
            .contains(id)
    }

    /// Add a board path to the open boards list.
    ///
    /// If the path is already in the list, this is a no-op.
    /// Auto-saves if a config path is configured.
    pub fn add_open_board(&self, path: &str) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            if !inner.open_boards.contains(&path.to_string()) {
                inner.open_boards.push(path.to_string());
            }
        }
        self.try_save();
    }

    /// Remove a board path from the open boards list.
    ///
    /// Also clears the `board_path` field from any windows that were showing
    /// this board. Auto-saves if a config path is configured.
    pub fn remove_open_board(&self, path: &str) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            inner.open_boards.retain(|p| p != path);
            // Clear board_path from any windows pointing to this board
            for ws in inner.windows.values_mut() {
                if ws.board_path == path {
                    ws.board_path = String::new();
                }
            }
        }
        self.try_save();
    }

    /// Get the list of open board paths.
    pub fn open_boards(&self) -> Vec<String> {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .open_boards
            .clone()
    }

    /// Set the per-window board assignment.
    ///
    /// Writes to `windows[label].board_path`.
    /// Auto-saves if a config path is configured.
    pub fn set_window_board(&self, label: &str, path: &str) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            let ws = inner.windows.entry(label.to_string()).or_default();
            ws.board_path = path.to_string();
        }
        self.try_save();
    }

    /// Get the board path assigned to a specific window.
    ///
    /// Returns `None` if the window has no board assigned or if the board_path is empty.
    pub fn window_board(&self, label: &str) -> Option<String> {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .windows
            .get(label)
            .map(|ws| ws.board_path.clone())
            .filter(|p| !p.is_empty())
    }

    /// Get all window-to-board assignments by iterating over windows.
    ///
    /// Returns only windows that have a non-empty board_path.
    pub fn all_window_boards(&self) -> HashMap<String, String> {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .windows
            .iter()
            .filter(|(_, ws)| !ws.board_path.is_empty())
            .map(|(label, ws)| (label.clone(), ws.board_path.clone()))
            .collect()
    }

    /// Add or update a board in the MRU list. Most recent first.
    ///
    /// Removes any existing entry for `path`, inserts a new entry at the
    /// front with the current UTC timestamp, truncates to 20 entries, and
    /// auto-saves.
    pub fn touch_recent(&self, path: &str, name: &str) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            // Remove any existing entry for this path
            inner.recent_boards.retain(|r| r.path != path);
            // Insert at front with current timestamp (RFC 3339 / ISO 8601)
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            // Format as a simple ISO 8601 UTC string: YYYY-MM-DDTHH:MM:SSZ
            let secs = now;
            let s = secs % 60;
            let m = (secs / 60) % 60;
            let h = (secs / 3600) % 24;
            let days = secs / 86400;
            // Use a simple epoch-based date (good enough for ordering)
            let last_opened = format!("1970-01-01T{:02}:{:02}:{:02}Z+{}days", h, m, s, days);
            inner.recent_boards.insert(
                0,
                RecentBoard {
                    path: path.to_string(),
                    name: name.to_string(),
                    last_opened,
                },
            );
            // Truncate to maximum
            inner.recent_boards.truncate(MAX_RECENT_BOARDS);
        }
        self.try_save();
    }

    /// Get the recent boards list (most recent first).
    pub fn recent_boards(&self) -> Vec<RecentBoard> {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .recent_boards
            .clone()
    }

    /// Set the most recently focused board path.
    ///
    /// Persisted to the YAML config file so the last focused board
    /// survives restarts. Called on window focus change and on board switch.
    pub fn set_most_recent_board(&self, path: &str) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            inner.most_recent_board_path = Some(path.to_string());
        }
        self.try_save();
    }

    /// Get the most recently focused board path.
    ///
    /// Returns `None` if no board has been focused yet.
    pub fn most_recent_board(&self) -> Option<String> {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .most_recent_board_path
            .clone()
    }

    /// Clear all per-window state (geometry and inspector stacks).
    ///
    /// Used by reset_windows to wipe geometry before restarting.
    /// Auto-saves if a config path is configured.
    pub fn clear_windows(&self) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            inner.windows.clear();
        }
        self.try_save();
    }

    /// Remove a window's state and board assignment.
    ///
    /// Called when a secondary window is closed mid-session so it doesn't
    /// resurrect on restart. Auto-saves if a config path is configured.
    pub fn remove_window(&self, label: &str) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            inner.windows.remove(label);
        }
        self.try_save();
    }

    /// Restore the open boards list from persisted data.
    ///
    /// Used at startup to populate UIState from legacy AppConfig data when
    /// UIState has no boards yet (first migration).
    pub fn restore_boards(&self, open_boards: Vec<String>) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            if inner.open_boards.is_empty() {
                inner.open_boards = open_boards;
            }
        }
        // No try_save here — this is called at startup with already-persisted data.
    }

    /// Set the inspector stack for a specific window (used for startup restoration).
    ///
    /// Auto-saves if a config path is configured.
    pub fn set_inspector_stack(&self, window_label: &str, stack: Vec<String>) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            inner
                .windows
                .entry(window_label.to_string())
                .or_default()
                .inspector_stack = stack;
        }
        self.try_save();
    }

    /// Get a clone of the current inspector stack for the given window.
    pub fn inspector_stack(&self, window_label: &str) -> Vec<String> {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .windows
            .get(window_label)
            .map(|ws| ws.inspector_stack.clone())
            .unwrap_or_default()
    }

    /// Save window geometry for a specific window.
    ///
    /// Auto-saves if a config path is configured.
    pub fn save_window_geometry(
        &self,
        label: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        maximized: bool,
    ) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            let ws = inner.windows.entry(label.to_string()).or_default();
            ws.x = Some(x);
            ws.y = Some(y);
            ws.width = Some(width);
            ws.height = Some(height);
            ws.maximized = maximized;
        }
        self.try_save();
    }

    /// Get the window state for a specific window label.
    pub fn get_window_state(&self, label: &str) -> Option<WindowState> {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .windows
            .get(label)
            .cloned()
    }

    /// Get all window states (for restore_windows).
    pub fn all_windows(&self) -> HashMap<String, WindowState> {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .windows
            .clone()
    }

    /// Get the active view ID for a specific window.
    ///
    /// Returns an empty string if the window has no active view set.
    pub fn active_view_id(&self, window_label: &str) -> String {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .windows
            .get(window_label)
            .map(|ws| ws.active_view_id.clone())
            .unwrap_or_default()
    }

    /// Get whether the palette is open.
    pub fn palette_open(&self) -> bool {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .palette_open
    }

    /// Get the current keymap mode.
    pub fn keymap_mode(&self) -> String {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .keymap_mode
            .clone()
    }

    /// Get a clone of the current scope chain.
    pub fn scope_chain(&self) -> Vec<String> {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .scope_chain
            .clone()
    }

    /// Serialize the current state to a JSON Value for the frontend.
    ///
    /// Includes ALL fields (both persisted and transient) so the frontend has
    /// a complete snapshot. The `palette_open` and `scope_chain` fields are
    /// marked `#[serde(skip)]` on the inner struct to avoid persisting them
    /// to YAML, so we build the JSON manually to include them.
    pub fn to_json(&self) -> serde_json::Value {
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        serde_json::json!({
            "palette_open": inner.palette_open,
            "keymap_mode": inner.keymap_mode,
            "scope_chain": inner.scope_chain,
            "open_boards": inner.open_boards,
            "windows": inner.windows,
            "recent_boards": inner.recent_boards,
            "most_recent_board_path": inner.most_recent_board_path,
        })
    }
}

impl std::fmt::Debug for UIState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        f.debug_struct("UIState")
            .field("palette_open", &inner.palette_open)
            .field("keymap_mode", &inner.keymap_mode)
            .field("scope_chain", &inner.scope_chain)
            .field("windows", &inner.windows)
            .field("config_path", &self.config_path)
            .finish()
    }
}

impl Default for UIState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn inspect_pushes_onto_stack() {
        let state = UIState::new();
        state.inspect("main", "task:01XYZ");
        assert_eq!(state.inspector_stack("main"), vec!["task:01XYZ"]);
    }

    #[test]
    fn inspect_pushes_per_window() {
        let state = UIState::new();
        state.inspect("main", "task:01XYZ");
        state.inspect("board-2", "task:01ABC");
        // Each window has its own stack
        assert_eq!(state.inspector_stack("main"), vec!["task:01XYZ"]);
        assert_eq!(state.inspector_stack("board-2"), vec!["task:01ABC"]);
    }

    #[test]
    fn inspect_stacks_any_types() {
        let state = UIState::new();
        state.inspect("main", "task:01XYZ");
        state.inspect("main", "tag:01TAG");
        assert_eq!(
            state.inspector_stack("main"),
            vec!["task:01XYZ", "tag:01TAG"]
        );
    }

    #[test]
    fn inspect_stacks_same_type() {
        let state = UIState::new();
        state.inspect("main", "task:01XYZ");
        state.inspect("main", "tag:01TAG");
        state.inspect("main", "task:01ABC");
        assert_eq!(
            state.inspector_stack("main"),
            vec!["task:01XYZ", "tag:01TAG", "task:01ABC"]
        );
    }

    #[test]
    fn inspect_same_moniker_on_top_is_noop() {
        let state = UIState::new();
        state.inspect("main", "task:01XYZ");
        state.inspect("main", "task:01XYZ");
        assert_eq!(state.inspector_stack("main"), vec!["task:01XYZ"]);
    }

    #[test]
    fn inspect_existing_moniker_moves_to_top() {
        let state = UIState::new();
        state.inspect("main", "task:01XYZ");
        state.inspect("main", "tag:01A");
        state.inspect("main", "task:01XYZ");
        assert_eq!(state.inspector_stack("main"), vec!["tag:01A", "task:01XYZ"]);
    }

    #[test]
    fn inspector_close_pops() {
        let state = UIState::new();
        state.inspect("main", "task:01XYZ");
        state.inspect("main", "tag:01TAG");
        let change = state.inspector_close("main");
        assert!(change.is_some());
        assert_eq!(state.inspector_stack("main"), vec!["task:01XYZ"]);
    }

    #[test]
    fn inspector_close_empty_returns_none() {
        let state = UIState::new();
        assert!(state.inspector_close("main").is_none());
    }

    #[test]
    fn inspector_close_all_clears() {
        let state = UIState::new();
        state.inspect("main", "task:01XYZ");
        state.inspect("main", "tag:01TAG");
        let change = state.inspector_close_all("main");
        assert!(change.is_some());
        assert!(state.inspector_stack("main").is_empty());
    }

    #[test]
    fn inspector_close_all_empty_returns_none() {
        let state = UIState::new();
        assert!(state.inspector_close_all("main").is_none());
    }

    #[test]
    fn set_active_view_changes() {
        let state = UIState::new();
        let change = state.set_active_view("main", "board-view");
        assert!(change.is_some());
        assert_eq!(state.active_view_id("main"), "board-view");
    }

    #[test]
    fn set_active_view_same_returns_none() {
        let state = UIState::new();
        state.set_active_view("main", "board-view");
        let change = state.set_active_view("main", "board-view");
        assert!(change.is_none());
    }

    #[test]
    fn set_active_view_per_window() {
        let state = UIState::new();
        state.set_active_view("main", "board-view");
        state.set_active_view("board-2", "grid-view");
        assert_eq!(state.active_view_id("main"), "board-view");
        assert_eq!(state.active_view_id("board-2"), "grid-view");
    }

    #[test]
    fn active_view_id_empty_for_unknown_window() {
        let state = UIState::new();
        assert_eq!(state.active_view_id("unknown-window"), "");
    }

    #[test]
    fn set_palette_open_toggles() {
        let state = UIState::new();
        assert!(!state.palette_open());

        let change = state.set_palette_open(true);
        assert!(change.is_some());
        assert!(state.palette_open());

        let change = state.set_palette_open(false);
        assert!(change.is_some());
        assert!(!state.palette_open());
    }

    #[test]
    fn set_keymap_mode_changes() {
        let state = UIState::new();
        assert_eq!(state.keymap_mode(), "cua");

        let change = state.set_keymap_mode("vim");
        assert!(change.is_some());
        assert_eq!(state.keymap_mode(), "vim");

        let change = state.set_keymap_mode("vim");
        assert!(change.is_none());
    }

    #[test]
    fn set_scope_chain_stores() {
        let state = UIState::new();
        state.set_scope_chain(vec!["task:01XYZ".into(), "column:todo".into()]);
        assert_eq!(state.scope_chain(), vec!["task:01XYZ", "column:todo"]);
    }

    #[test]
    fn set_inspector_stack_restores() {
        let state = UIState::new();
        state.set_inspector_stack("main", vec!["task:01XYZ".into(), "tag:01TAG".into()]);
        assert_eq!(
            state.inspector_stack("main"),
            vec!["task:01XYZ", "tag:01TAG"]
        );
    }

    #[test]
    fn inspector_stack_empty_for_unknown_window() {
        let state = UIState::new();
        // A window with no entries returns an empty stack
        assert!(state.inspector_stack("unknown-window").is_empty());
    }

    // --- Persistence tests ---

    /// Returns a unique temp file path for each test run, avoiding collisions.
    fn temp_yaml_path(suffix: &str) -> PathBuf {
        let mut dir = env::temp_dir();
        dir.push(format!(
            "ui_state_test_{suffix}_{}.yaml",
            std::process::id()
        ));
        dir
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let path = temp_yaml_path("missing");
        // Ensure the file does not exist
        let _ = fs::remove_file(&path);
        let state = UIState::load(&path);
        assert_eq!(state.keymap_mode(), "cua");
        assert!(state.inspector_stack("main").is_empty());
        assert_eq!(state.active_view_id("main"), "");
    }

    #[test]
    fn load_malformed_yaml_returns_defaults() {
        let path = temp_yaml_path("malformed");
        fs::write(&path, b":::not valid yaml:::").unwrap();
        let state = UIState::load(&path);
        assert_eq!(state.keymap_mode(), "cua");
        assert!(state.inspector_stack("main").is_empty());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn round_trip_persists_state() {
        let path = temp_yaml_path("roundtrip");
        {
            let state = UIState::load(&path);
            state.set_keymap_mode("vim");
            state.inspect("main", "task:01XYZ");
            state.set_active_view("main", "board-view");
            state.save().unwrap();
        }
        // Load again and verify
        let state2 = UIState::load(&path);
        assert_eq!(state2.keymap_mode(), "vim");
        assert_eq!(state2.inspector_stack("main"), vec!["task:01XYZ"]);
        assert_eq!(state2.active_view_id("main"), "board-view");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn transient_fields_not_persisted() {
        let path = temp_yaml_path("transient");
        {
            let state = UIState::load(&path);
            state.set_palette_open(true);
            state.set_scope_chain(vec!["scope:x".to_string()]);
            state.set_keymap_mode("emacs"); // persisted — forces a file to exist
            state.save().unwrap();
        }
        let state2 = UIState::load(&path);
        // Transient fields reset to defaults
        assert!(!state2.palette_open());
        assert!(state2.scope_chain().is_empty());
        // Persisted field is intact
        assert_eq!(state2.keymap_mode(), "emacs");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn auto_save_on_mutation() {
        let path = temp_yaml_path("autosave");
        let _ = fs::remove_file(&path);
        {
            let state = UIState::load(&path);
            // Mutate — should auto-save without explicit save() call
            state.set_keymap_mode("vim");
        }
        // Load from same path; mutation should have been persisted automatically
        let state2 = UIState::load(&path);
        assert_eq!(state2.keymap_mode(), "vim");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn new_without_path_no_persistence() {
        // UIState::new() has no config_path — save() is a no-op
        let state = UIState::new();
        state.set_keymap_mode("vim");
        // save() should return Ok without writing any file
        state.save().expect("save on new() should be a no-op Ok");
    }

    #[test]
    fn mutation_returns_correct_payload() {
        let state = UIState::new();

        // inspect returns InspectorStack
        let change = state.inspect("main", "task:01XYZ");
        match change {
            UIStateChange::InspectorStack(stack) => assert_eq!(stack, vec!["task:01XYZ"]),
            other => panic!("Expected InspectorStack, got {:?}", other),
        }

        // set_active_view returns ActiveView
        let change = state.set_active_view("main", "my-view").unwrap();
        match change {
            UIStateChange::ActiveView(id) => assert_eq!(id, "my-view"),
            other => panic!("Expected ActiveView, got {:?}", other),
        }

        // set_palette_open returns PaletteOpen
        let change = state.set_palette_open(true).unwrap();
        match change {
            UIStateChange::PaletteOpen(open) => assert!(open),
            other => panic!("Expected PaletteOpen, got {:?}", other),
        }

        // set_keymap_mode returns KeymapMode
        let change = state.set_keymap_mode("emacs").unwrap();
        match change {
            UIStateChange::KeymapMode(mode) => assert_eq!(mode, "emacs"),
            other => panic!("Expected KeymapMode, got {:?}", other),
        }

        // set_scope_chain returns ScopeChain
        let chain = vec!["board:main".to_string()];
        let change = state.set_scope_chain(chain.clone());
        match change {
            UIStateChange::ScopeChain(sc) => assert_eq!(sc, chain),
            other => panic!("Expected ScopeChain, got {:?}", other),
        }
    }

    // --- most_recent_board_path tests ---

    #[test]
    fn most_recent_board_defaults_to_none() {
        let state = UIState::new();
        assert!(state.most_recent_board().is_none());
    }

    #[test]
    fn set_most_recent_board_stores_path() {
        let state = UIState::new();
        state.set_most_recent_board("/boards/my-project/.kanban");
        assert_eq!(
            state.most_recent_board(),
            Some("/boards/my-project/.kanban".to_string())
        );
    }

    #[test]
    fn set_most_recent_board_overwrites() {
        let state = UIState::new();
        state.set_most_recent_board("/boards/first/.kanban");
        state.set_most_recent_board("/boards/second/.kanban");
        assert_eq!(
            state.most_recent_board(),
            Some("/boards/second/.kanban".to_string())
        );
    }

    #[test]
    fn most_recent_board_persists_round_trip() {
        let path = temp_yaml_path("most_recent_board");
        {
            let state = UIState::load(&path);
            state.set_most_recent_board("/boards/project/.kanban");
            state.save().unwrap();
        }
        let state2 = UIState::load(&path);
        assert_eq!(
            state2.most_recent_board(),
            Some("/boards/project/.kanban".to_string())
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn most_recent_board_in_to_json() {
        let state = UIState::new();
        state.set_most_recent_board("/boards/foo/.kanban");
        let json = state.to_json();
        assert_eq!(
            json["most_recent_board_path"].as_str(),
            Some("/boards/foo/.kanban")
        );
    }

    #[test]
    fn most_recent_board_null_in_to_json_when_unset() {
        let state = UIState::new();
        let json = state.to_json();
        assert!(json["most_recent_board_path"].is_null());
    }

    // --- drag session tests ---

    fn make_drag_session(task_id: &str, board_path: &str) -> DragSession {
        DragSession {
            session_id: format!("sess-{task_id}"),
            source_board_path: board_path.to_string(),
            source_window_label: "main".to_string(),
            task_id: task_id.to_string(),
            task_fields: serde_json::json!({}),
            copy_mode: false,
            started_at_ms: 0,
        }
    }

    #[test]
    fn start_drag_then_drag_session_returns_session() {
        let state = UIState::new();
        state.start_drag(make_drag_session("task-1", "/board/a"));
        let current = state.drag_session();
        assert!(current.is_some());
        assert_eq!(current.unwrap().task_id, "task-1");
    }

    #[test]
    fn take_drag_returns_session_and_clears() {
        let state = UIState::new();
        state.start_drag(make_drag_session("task-1", "/board/a"));

        let taken = state.take_drag();
        assert!(taken.is_some());
        assert_eq!(taken.unwrap().task_id, "task-1");

        assert!(state.take_drag().is_none());
    }

    #[test]
    fn cancel_drag_clears_session() {
        let state = UIState::new();
        state.start_drag(make_drag_session("task-1", "/board/a"));
        state.cancel_drag();
        assert!(state.drag_session().is_none());
    }

    #[test]
    fn start_drag_replaces_existing_session() {
        let state = UIState::new();
        state.start_drag(make_drag_session("task-1", "/board/a"));
        state.start_drag(make_drag_session("task-2", "/board/b"));

        let current = state.drag_session().unwrap();
        assert_eq!(current.task_id, "task-2");
        assert_eq!(current.source_board_path, "/board/b");
    }

    #[test]
    fn take_drag_on_empty_returns_none() {
        let state = UIState::new();
        assert!(state.take_drag().is_none());
    }

    // --- context menu tests ---

    #[test]
    fn set_context_menu_ids_and_check_membership() {
        let state = UIState::new();
        let ids: HashSet<String> =
            ["task:01A", "task:01B"].iter().map(|s| s.to_string()).collect();
        state.set_context_menu_ids(ids);

        assert!(state.is_context_menu_id("task:01A"));
        assert!(state.is_context_menu_id("task:01B"));
    }

    #[test]
    fn is_context_menu_id_returns_false_for_non_member() {
        let state = UIState::new();
        let ids: HashSet<String> = ["task:01A"].iter().map(|s| s.to_string()).collect();
        state.set_context_menu_ids(ids);

        assert!(!state.is_context_menu_id("task:01MISSING"));
    }

    #[test]
    fn replacing_context_menu_ids_clears_previous() {
        let state = UIState::new();

        let first: HashSet<String> =
            ["task:01A", "task:01B"].iter().map(|s| s.to_string()).collect();
        state.set_context_menu_ids(first);
        assert!(state.is_context_menu_id("task:01A"));

        let second: HashSet<String> = ["task:01C"].iter().map(|s| s.to_string()).collect();
        state.set_context_menu_ids(second);

        assert!(!state.is_context_menu_id("task:01A"), "old ID should be gone");
        assert!(!state.is_context_menu_id("task:01B"), "old ID should be gone");
        assert!(state.is_context_menu_id("task:01C"), "new ID should be present");
    }

    // --- open boards and window board management tests ---

    #[test]
    fn add_open_board_adds_and_deduplicates() {
        let state = UIState::new();
        state.add_open_board("/boards/a");
        state.add_open_board("/boards/b");
        state.add_open_board("/boards/a"); // duplicate
        assert_eq!(state.open_boards(), vec!["/boards/a", "/boards/b"]);
    }

    #[test]
    fn remove_open_board_removes_from_list() {
        let state = UIState::new();
        state.add_open_board("/boards/a");
        state.add_open_board("/boards/b");
        state.remove_open_board("/boards/a");
        assert_eq!(state.open_boards(), vec!["/boards/b"]);
    }

    #[test]
    fn remove_open_board_clears_window_board_path() {
        let state = UIState::new();
        state.add_open_board("/boards/a");
        state.set_window_board("main", "/boards/a");
        state.remove_open_board("/boards/a");
        assert!(state.window_board("main").is_none());
    }

    #[test]
    fn set_window_board_and_window_board_round_trip() {
        let state = UIState::new();
        state.set_window_board("main", "/boards/foo");
        assert_eq!(state.window_board("main").as_deref(), Some("/boards/foo"));
    }

    #[test]
    fn window_board_returns_none_for_unassigned() {
        let state = UIState::new();
        assert!(state.window_board("unknown").is_none());
    }

    #[test]
    fn all_window_boards_filters_empty() {
        let state = UIState::new();
        state.set_window_board("main", "/boards/a");
        state.set_window_board("secondary", "/boards/b");
        // Create a window with empty board_path by removing its board
        state.add_open_board("/boards/b");
        state.remove_open_board("/boards/b");
        let boards = state.all_window_boards();
        assert_eq!(boards.len(), 1);
        assert_eq!(boards.get("main").unwrap(), "/boards/a");
    }

    // --- touch_recent and recent_boards tests ---

    #[test]
    fn touch_recent_adds_entry() {
        let state = UIState::new();
        state.touch_recent("/boards/a", "Board A");
        let recent = state.recent_boards();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].path, "/boards/a");
        assert_eq!(recent[0].name, "Board A");
    }

    #[test]
    fn touch_recent_moves_to_front() {
        let state = UIState::new();
        state.touch_recent("/boards/a", "A");
        state.touch_recent("/boards/b", "B");
        state.touch_recent("/boards/a", "A Updated");
        let recent = state.recent_boards();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].path, "/boards/a");
        assert_eq!(recent[0].name, "A Updated");
        assert_eq!(recent[1].path, "/boards/b");
    }

    #[test]
    fn touch_recent_caps_at_max() {
        let state = UIState::new();
        for i in 0..25 {
            state.touch_recent(&format!("/boards/{i}"), &format!("Board {i}"));
        }
        assert_eq!(state.recent_boards().len(), 20);
    }

    #[test]
    fn touch_recent_populates_last_opened() {
        let state = UIState::new();
        state.touch_recent("/boards/a", "A");
        let recent = state.recent_boards();
        assert!(!recent[0].last_opened.is_empty());
    }

    // --- window management tests ---

    #[test]
    fn save_window_geometry_and_get_window_state_round_trip() {
        let state = UIState::new();
        state.save_window_geometry("main", 100, 200, 800, 600, true);
        let ws = state.get_window_state("main").expect("window state exists");
        assert_eq!(ws.x, Some(100));
        assert_eq!(ws.y, Some(200));
        assert_eq!(ws.width, Some(800));
        assert_eq!(ws.height, Some(600));
        assert!(ws.maximized);
    }

    #[test]
    fn remove_window_removes_entry() {
        let state = UIState::new();
        state.save_window_geometry("main", 0, 0, 800, 600, false);
        state.remove_window("main");
        assert!(state.get_window_state("main").is_none());
    }

    #[test]
    fn clear_windows_removes_all() {
        let state = UIState::new();
        state.save_window_geometry("main", 0, 0, 800, 600, false);
        state.save_window_geometry("secondary", 100, 100, 400, 300, false);
        state.clear_windows();
        assert!(state.all_windows().is_empty());
    }

    #[test]
    fn restore_boards_populates_when_empty() {
        let state = UIState::new();
        state.restore_boards(vec!["/a".into(), "/b".into()]);
        assert_eq!(state.open_boards(), vec!["/a", "/b"]);
    }

    #[test]
    fn restore_boards_no_ops_when_not_empty() {
        let state = UIState::new();
        state.add_open_board("/existing");
        state.restore_boards(vec!["/a".into(), "/b".into()]);
        assert_eq!(state.open_boards(), vec!["/existing"]);
    }

    #[test]
    fn all_windows_returns_all_entries() {
        let state = UIState::new();
        state.save_window_geometry("main", 0, 0, 800, 600, false);
        state.save_window_geometry("secondary", 100, 100, 400, 300, false);
        let all = state.all_windows();
        assert_eq!(all.len(), 2);
        assert!(all.contains_key("main"));
        assert!(all.contains_key("secondary"));
    }
}
