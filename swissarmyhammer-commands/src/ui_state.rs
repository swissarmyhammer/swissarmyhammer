use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

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
    /// Stack of open inspector monikers (e.g. `["task:01XYZ", "tag:01TAG"]`).
    inspector_stack: Vec<String>,
    /// ID of the currently active view.
    active_view_id: String,
    /// Whether the command palette is open. Transient — not persisted.
    #[serde(skip)]
    palette_open: bool,
    /// Current keymap mode: "cua", "vim", or "emacs".
    keymap_mode: String,
    /// Current focus scope chain (innermost first). Transient — not persisted.
    #[serde(skip)]
    scope_chain: Vec<String>,
    /// Canonical paths of boards that are open.
    open_boards: Vec<String>,
    /// The globally active board path.
    active_board_path: Option<String>,
    /// Per-window board assignments (window label → board path).
    window_boards: HashMap<String, String>,
}

impl Default for UIStateInner {
    /// Returns the default UI state values.
    fn default() -> Self {
        Self {
            inspector_stack: Vec::new(),
            active_view_id: String::new(),
            palette_open: false,
            keymap_mode: "cua".to_string(),
            scope_chain: Vec::new(),
            open_boards: Vec::new(),
            active_board_path: None,
            window_boards: HashMap::new(),
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

    /// Open the inspector for the given moniker.
    ///
    /// True stack: always pushes. If the moniker is already on top, no-op.
    /// If the moniker exists deeper in the stack, removes it and pushes to top.
    /// Auto-saves if a config path is configured.
    pub fn inspect(&self, moniker: &str) -> UIStateChange {
        let change = {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());

            // Already on top — no-op
            if inner.inspector_stack.last().map(|s| s.as_str()) == Some(moniker) {
                return UIStateChange::InspectorStack(inner.inspector_stack.clone());
            }

            // Remove if already in stack (moves to top)
            inner.inspector_stack.retain(|m| m != moniker);
            inner.inspector_stack.push(moniker.to_string());

            UIStateChange::InspectorStack(inner.inspector_stack.clone())
        };
        self.try_save();
        change
    }

    /// Close the topmost inspector entry.
    ///
    /// Returns `None` if the stack was already empty.
    /// Auto-saves if a config path is configured.
    pub fn inspector_close(&self) -> Option<UIStateChange> {
        let change = {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            if inner.inspector_stack.is_empty() {
                return None;
            }
            inner.inspector_stack.pop();
            Some(UIStateChange::InspectorStack(inner.inspector_stack.clone()))
        };
        self.try_save();
        change
    }

    /// Close all inspector entries.
    ///
    /// Returns `None` if the stack was already empty.
    /// Auto-saves if a config path is configured.
    pub fn inspector_close_all(&self) -> Option<UIStateChange> {
        let change = {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            if inner.inspector_stack.is_empty() {
                return None;
            }
            inner.inspector_stack.clear();
            Some(UIStateChange::InspectorStack(inner.inspector_stack.clone()))
        };
        self.try_save();
        change
    }

    /// Set the active view ID.
    ///
    /// Returns `None` if the view ID is unchanged.
    /// Auto-saves if a config path is configured.
    pub fn set_active_view(&self, id: &str) -> Option<UIStateChange> {
        let change = {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            if inner.active_view_id == id {
                return None;
            }
            inner.active_view_id = id.to_string();
            Some(UIStateChange::ActiveView(inner.active_view_id.clone()))
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

    /// Add a board path to the open boards list, setting it as the active board.
    ///
    /// If the path is already in the list, updates active without duplicating.
    /// Auto-saves if a config path is configured.
    pub fn add_open_board(&self, path: &str) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            if !inner.open_boards.contains(&path.to_string()) {
                inner.open_boards.push(path.to_string());
            }
            inner.active_board_path = Some(path.to_string());
        }
        self.try_save();
    }

    /// Remove a board path from the open boards list.
    ///
    /// If the removed board was active, switches active to another open board
    /// (the last remaining one) or sets active to None.
    /// Auto-saves if a config path is configured.
    pub fn remove_open_board(&self, path: &str) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            inner.open_boards.retain(|p| p != path);
            // Clear per-window entries pointing to this board
            inner.window_boards.retain(|_, p| p != path);
            // Update active if needed
            if inner.active_board_path.as_deref() == Some(path) {
                inner.active_board_path = inner.open_boards.last().cloned();
            }
        }
        self.try_save();
    }

    /// Set the globally active board path.
    ///
    /// Auto-saves if a config path is configured.
    pub fn set_active_board_path(&self, path: &str) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            inner.active_board_path = Some(path.to_string());
        }
        self.try_save();
    }

    /// Get the globally active board path.
    pub fn active_board_path(&self) -> Option<String> {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .active_board_path
            .clone()
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
    /// Auto-saves if a config path is configured.
    pub fn set_window_board(&self, label: &str, path: &str) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            inner
                .window_boards
                .insert(label.to_string(), path.to_string());
        }
        self.try_save();
    }

    /// Get the board path assigned to a specific window.
    pub fn window_board(&self, label: &str) -> Option<String> {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .window_boards
            .get(label)
            .cloned()
    }

    /// Restore the open boards list and active board path from persisted data.
    ///
    /// Used at startup to populate UIState from legacy AppConfig data when
    /// UIState has no boards yet (first migration).
    pub fn restore_boards(&self, open_boards: Vec<String>, active_board_path: Option<String>) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            if inner.open_boards.is_empty() {
                inner.open_boards = open_boards;
                inner.active_board_path = active_board_path;
            }
        }
        // No try_save here — this is called at startup with already-persisted data.
    }

    /// Set the inspector stack directly (used for startup restoration from config).
    ///
    /// Auto-saves if a config path is configured.
    pub fn set_inspector_stack(&self, stack: Vec<String>) {
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            inner.inspector_stack = stack;
        }
        self.try_save();
    }

    /// Get a clone of the current inspector stack.
    pub fn inspector_stack(&self) -> Vec<String> {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .inspector_stack
            .clone()
    }

    /// Get the current active view ID.
    pub fn active_view_id(&self) -> String {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .active_view_id
            .clone()
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
            "inspector_stack": inner.inspector_stack,
            "active_view_id": inner.active_view_id,
            "palette_open": inner.palette_open,
            "keymap_mode": inner.keymap_mode,
            "scope_chain": inner.scope_chain,
            "open_boards": inner.open_boards,
            "active_board_path": inner.active_board_path,
            "window_boards": inner.window_boards,
        })
    }
}

impl std::fmt::Debug for UIState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        f.debug_struct("UIState")
            .field("inspector_stack", &inner.inspector_stack)
            .field("active_view_id", &inner.active_view_id)
            .field("palette_open", &inner.palette_open)
            .field("keymap_mode", &inner.keymap_mode)
            .field("scope_chain", &inner.scope_chain)
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
        state.inspect("task:01XYZ");
        assert_eq!(state.inspector_stack(), vec!["task:01XYZ"]);
    }

    #[test]
    fn inspect_stacks_any_types() {
        let state = UIState::new();
        state.inspect("task:01XYZ");
        state.inspect("tag:01TAG");
        assert_eq!(state.inspector_stack(), vec!["task:01XYZ", "tag:01TAG"]);
    }

    #[test]
    fn inspect_stacks_same_type() {
        let state = UIState::new();
        state.inspect("task:01XYZ");
        state.inspect("tag:01TAG");
        state.inspect("task:01ABC");
        assert_eq!(
            state.inspector_stack(),
            vec!["task:01XYZ", "tag:01TAG", "task:01ABC"]
        );
    }

    #[test]
    fn inspect_same_moniker_on_top_is_noop() {
        let state = UIState::new();
        state.inspect("task:01XYZ");
        state.inspect("task:01XYZ");
        assert_eq!(state.inspector_stack(), vec!["task:01XYZ"]);
    }

    #[test]
    fn inspect_existing_moniker_moves_to_top() {
        let state = UIState::new();
        state.inspect("task:01XYZ");
        state.inspect("tag:01A");
        state.inspect("task:01XYZ");
        assert_eq!(state.inspector_stack(), vec!["tag:01A", "task:01XYZ"]);
    }

    #[test]
    fn inspector_close_pops() {
        let state = UIState::new();
        state.inspect("task:01XYZ");
        state.inspect("tag:01TAG");
        let change = state.inspector_close();
        assert!(change.is_some());
        assert_eq!(state.inspector_stack(), vec!["task:01XYZ"]);
    }

    #[test]
    fn inspector_close_empty_returns_none() {
        let state = UIState::new();
        assert!(state.inspector_close().is_none());
    }

    #[test]
    fn inspector_close_all_clears() {
        let state = UIState::new();
        state.inspect("task:01XYZ");
        state.inspect("tag:01TAG");
        let change = state.inspector_close_all();
        assert!(change.is_some());
        assert!(state.inspector_stack().is_empty());
    }

    #[test]
    fn inspector_close_all_empty_returns_none() {
        let state = UIState::new();
        assert!(state.inspector_close_all().is_none());
    }

    #[test]
    fn set_active_view_changes() {
        let state = UIState::new();
        let change = state.set_active_view("board-view");
        assert!(change.is_some());
        assert_eq!(state.active_view_id(), "board-view");
    }

    #[test]
    fn set_active_view_same_returns_none() {
        let state = UIState::new();
        state.set_active_view("board-view");
        let change = state.set_active_view("board-view");
        assert!(change.is_none());
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
        state.set_inspector_stack(vec!["task:01XYZ".into(), "tag:01TAG".into()]);
        assert_eq!(state.inspector_stack(), vec!["task:01XYZ", "tag:01TAG"]);
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
        assert!(state.inspector_stack().is_empty());
        assert_eq!(state.active_view_id(), "");
    }

    #[test]
    fn load_malformed_yaml_returns_defaults() {
        let path = temp_yaml_path("malformed");
        fs::write(&path, b":::not valid yaml:::").unwrap();
        let state = UIState::load(&path);
        assert_eq!(state.keymap_mode(), "cua");
        assert!(state.inspector_stack().is_empty());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn round_trip_persists_state() {
        let path = temp_yaml_path("roundtrip");
        {
            let state = UIState::load(&path);
            state.set_keymap_mode("vim");
            state.inspect("task:01XYZ");
            state.set_active_view("board-view");
            state.save().unwrap();
        }
        // Load again and verify
        let state2 = UIState::load(&path);
        assert_eq!(state2.keymap_mode(), "vim");
        assert_eq!(state2.inspector_stack(), vec!["task:01XYZ"]);
        assert_eq!(state2.active_view_id(), "board-view");
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
        let change = state.inspect("task:01XYZ");
        match change {
            UIStateChange::InspectorStack(stack) => assert_eq!(stack, vec!["task:01XYZ"]),
            other => panic!("Expected InspectorStack, got {:?}", other),
        }

        // set_active_view returns ActiveView
        let change = state.set_active_view("my-view").unwrap();
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
}
