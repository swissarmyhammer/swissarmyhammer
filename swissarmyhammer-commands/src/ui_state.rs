use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::context::parse_moniker;

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
pub struct UIState {
    inner: RwLock<UIStateInner>,
}

/// Interior mutable state behind the RwLock.
struct UIStateInner {
    /// Stack of open inspector monikers (e.g. `["task:01XYZ", "tag:01TAG"]`).
    inspector_stack: Vec<String>,
    /// ID of the currently active view.
    active_view_id: String,
    /// Whether the command palette is open.
    palette_open: bool,
    /// Current keymap mode: "cua", "vim", or "emacs".
    keymap_mode: String,
    /// Current focus scope chain (innermost first).
    scope_chain: Vec<String>,
}

/// Entity types considered "primary" for inspector stack replacement logic.
const PRIMARY_TYPES: &[&str] = &["task", "column", "board"];

impl UIState {
    /// Create a new UIState with default values.
    ///
    /// Defaults: empty inspector stack, empty active_view_id, palette closed,
    /// keymap mode "cua", empty scope chain.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(UIStateInner {
                inspector_stack: Vec::new(),
                active_view_id: String::new(),
                palette_open: false,
                keymap_mode: "cua".to_string(),
                scope_chain: Vec::new(),
            }),
        }
    }

    /// Open the inspector for the given moniker.
    ///
    /// Primary entity types (task, column, board) replace the entire stack.
    /// Secondary types push onto the stack, replacing any existing entry of the
    /// same entity type.
    pub fn inspect(&self, moniker: &str) -> UIStateChange {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        let entity_type = parse_moniker(moniker).map(|(t, _)| t).unwrap_or("");

        if PRIMARY_TYPES.contains(&entity_type) {
            inner.inspector_stack = vec![moniker.to_string()];
        } else {
            // Remove any existing entry with the same entity type
            inner
                .inspector_stack
                .retain(|m| parse_moniker(m).map(|(t, _)| t) != Some(entity_type));
            inner.inspector_stack.push(moniker.to_string());
        }

        UIStateChange::InspectorStack(inner.inspector_stack.clone())
    }

    /// Close the topmost inspector entry.
    ///
    /// Returns `None` if the stack was already empty.
    pub fn inspector_close(&self) -> Option<UIStateChange> {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        if inner.inspector_stack.is_empty() {
            return None;
        }
        inner.inspector_stack.pop();
        Some(UIStateChange::InspectorStack(
            inner.inspector_stack.clone(),
        ))
    }

    /// Close all inspector entries.
    ///
    /// Returns `None` if the stack was already empty.
    pub fn inspector_close_all(&self) -> Option<UIStateChange> {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        if inner.inspector_stack.is_empty() {
            return None;
        }
        inner.inspector_stack.clear();
        Some(UIStateChange::InspectorStack(
            inner.inspector_stack.clone(),
        ))
    }

    /// Set the active view ID.
    ///
    /// Returns `None` if the view ID is unchanged.
    pub fn set_active_view(&self, id: &str) -> Option<UIStateChange> {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        if inner.active_view_id == id {
            return None;
        }
        inner.active_view_id = id.to_string();
        Some(UIStateChange::ActiveView(inner.active_view_id.clone()))
    }

    /// Set whether the command palette is open.
    ///
    /// Returns `None` if the value is unchanged.
    pub fn set_palette_open(&self, open: bool) -> Option<UIStateChange> {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        if inner.palette_open == open {
            return None;
        }
        inner.palette_open = open;
        Some(UIStateChange::PaletteOpen(inner.palette_open))
    }

    /// Set the keymap mode (e.g. "cua", "vim", "emacs").
    ///
    /// Returns `None` if the mode is unchanged.
    pub fn set_keymap_mode(&self, mode: &str) -> Option<UIStateChange> {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        if inner.keymap_mode == mode {
            return None;
        }
        inner.keymap_mode = mode.to_string();
        Some(UIStateChange::KeymapMode(inner.keymap_mode.clone()))
    }

    /// Set the focus scope chain. Always returns the new scope chain.
    pub fn set_scope_chain(&self, chain: Vec<String>) -> UIStateChange {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.scope_chain = chain;
        UIStateChange::ScopeChain(inner.scope_chain.clone())
    }

    /// Get a clone of the current inspector stack.
    pub fn inspector_stack(&self) -> Vec<String> {
        self.inner.read().unwrap_or_else(|e| e.into_inner()).inspector_stack.clone()
    }

    /// Get the current active view ID.
    pub fn active_view_id(&self) -> String {
        self.inner.read().unwrap_or_else(|e| e.into_inner()).active_view_id.clone()
    }

    /// Get whether the palette is open.
    pub fn palette_open(&self) -> bool {
        self.inner.read().unwrap_or_else(|e| e.into_inner()).palette_open
    }

    /// Get the current keymap mode.
    pub fn keymap_mode(&self) -> String {
        self.inner.read().unwrap_or_else(|e| e.into_inner()).keymap_mode.clone()
    }

    /// Get a clone of the current scope chain.
    pub fn scope_chain(&self) -> Vec<String> {
        self.inner.read().unwrap_or_else(|e| e.into_inner()).scope_chain.clone()
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

    #[test]
    fn inspect_primary_replaces_stack() {
        let state = UIState::new();
        state.inspect("task:01XYZ");
        assert_eq!(state.inspector_stack(), vec!["task:01XYZ"]);
    }

    #[test]
    fn inspect_secondary_pushes() {
        let state = UIState::new();
        state.inspect("task:01XYZ");
        state.inspect("tag:01TAG");
        assert_eq!(
            state.inspector_stack(),
            vec!["task:01XYZ", "tag:01TAG"]
        );
    }

    #[test]
    fn inspect_primary_replaces_after_secondary() {
        let state = UIState::new();
        state.inspect("task:01XYZ");
        state.inspect("tag:01TAG");
        state.inspect("task:01ABC");
        assert_eq!(state.inspector_stack(), vec!["task:01ABC"]);
    }

    #[test]
    fn inspect_secondary_replaces_same_type() {
        let state = UIState::new();
        state.inspect("task:01XYZ");
        state.inspect("tag:01A");
        state.inspect("tag:01B");
        assert_eq!(
            state.inspector_stack(),
            vec!["task:01XYZ", "tag:01B"]
        );
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
        assert_eq!(
            state.scope_chain(),
            vec!["task:01XYZ", "column:todo"]
        );
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
