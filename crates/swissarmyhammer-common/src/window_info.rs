//! Lightweight open-window descriptor for dynamic command generation.
//!
//! Lives here (in the consumer-agnostic common crate) rather than in
//! `swissarmyhammer-kanban` or the Tauri-coupled window-service crate
//! because the descriptor has nothing to do with kanban specifically and
//! must stay free of any Tauri dependency. It is GUI runtime data the
//! scope dispatcher consumes when emitting `window.focus:{label}` rows.
//! Non-Tauri consumers (`swissarmyhammer-statusline`,
//! `swissarmyhammer-kanban`) produce and consume this shape, so it belongs
//! in the shared base crate every consumer already depends on.

/// Lightweight open-window descriptor for dynamic command generation.
///
/// Only carries the fields needed to produce a `window.focus:{label}`
/// command. Intentionally decoupled from Tauri's `WebviewWindow` so
/// the commands crate does not depend on Tauri directly.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// Tauri window label (e.g. `"main"`, `"board-01jxyz"`).
    pub label: String,
    /// Human-readable window title (e.g. `"SwissArmyHammer"`).
    pub title: String,
    /// Whether this window currently has focus.
    pub focused: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `WindowInfo` is plain data: it constructs by struct literal and
    /// the public fields are directly readable. This test pins the
    /// shape so a future field rename surfaces here instead of in
    /// every downstream consumer.
    #[test]
    fn window_info_construction() {
        let w = WindowInfo {
            label: "main".into(),
            title: "SwissArmyHammer".into(),
            focused: true,
        };
        assert_eq!(w.label, "main");
        assert_eq!(w.title, "SwissArmyHammer");
        assert!(w.focused);
    }

    /// `Clone` and `Debug` are derived for diagnostic logging and for
    /// the GUI runtime's pattern of cloning the descriptor when it
    /// hands a snapshot to `commands_for_scope`. This test pins both
    /// traits so removing them surfaces here.
    #[test]
    fn window_info_clone_and_debug() {
        let w = WindowInfo {
            label: "secondary".into(),
            title: "Board: tasks".into(),
            focused: false,
        };
        let w2 = w.clone();
        assert_eq!(w2.label, "secondary");
        let debug_str = format!("{:?}", w);
        assert!(debug_str.contains("secondary"));
        assert!(debug_str.contains("Board: tasks"));
    }
}
