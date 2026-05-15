//! Lightweight open-window descriptor for dynamic command generation.
//!
//! Lives here (in the consumer-agnostic commands crate) rather than in
//! `swissarmyhammer-kanban` because the descriptor has nothing to do
//! with kanban specifically — it is GUI runtime data the scope
//! dispatcher consumes when emitting `window.focus:{label}` rows. Any
//! consumer that drives a multi-window GUI off the command registry
//! produces and consumes this shape.

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
