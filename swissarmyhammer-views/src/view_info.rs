//! Lightweight runtime descriptor for dynamic command generation.
//!
//! Lives here (in the views crate) rather than `swissarmyhammer-kanban`
//! because `ViewInfo` is pure view-domain data: id, display name, the
//! entity-type a view renders (when set), and the kebab-case `ViewKind`
//! string. Every consumer that drives the command registry off a set of
//! known views produces this shape; kanban is just one such consumer.

/// Lightweight view descriptor for dynamic command generation.
///
/// Only carries the fields needed to produce a palette row that
/// dispatches `view.set` with a pre-filled `view_id`. Intentionally
/// decoupled from [`crate::types::ViewDef`] so the dispatch path does
/// not need to load the entire view definition for every emit.
#[derive(Debug, Clone)]
pub struct ViewInfo {
    /// View identifier (e.g. `"board-view"`, `"tasks-grid"`).
    pub id: String,
    /// Human-readable name (e.g. `"Board View"`, `"Task Grid"`).
    pub name: String,
    /// Entity type this view renders (e.g. `"task"`, `"tag"`, `"project"`).
    ///
    /// When present, the scope dispatcher emits a dynamic
    /// `entity.add:{entity_type}` command so every view type gets a
    /// generic "New {Type}" creation action without per-type Rust code.
    pub entity_type: Option<String>,
    /// View kind serialized as a kebab-case string (e.g. `"board"`,
    /// `"grid"`, `"list"`, `"calendar"`, `"timeline"`, `"unknown"`).
    ///
    /// Drives the `CommandDef.view_kinds` UI-surface filter:
    /// `commands_for_scope` resolves the innermost `view:{id}` moniker
    /// in the scope chain to this string, and skips any command whose
    /// `view_kinds` list is non-empty and does not contain the
    /// resolved kind. The same kebab-case representation is produced
    /// by [`crate::types::ViewKind`]'s `#[serde(rename_all = "kebab-case")]`.
    pub kind: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ViewKind;

    /// `ViewInfo` is plain data — fields are directly readable. This
    /// test pins the shape so a future rename surfaces here instead of
    /// in every downstream consumer.
    #[test]
    fn view_info_construction() {
        let v = ViewInfo {
            id: "board-view".into(),
            name: "Board View".into(),
            entity_type: Some("task".into()),
            kind: ViewKind::Board.as_kebab_str().to_string(),
        };
        assert_eq!(v.id, "board-view");
        assert_eq!(v.name, "Board View");
        assert_eq!(v.entity_type.as_deref(), Some("task"));
        assert_eq!(v.kind, "board");
    }

    /// `entity_type: None` is the dashboard-style case (no
    /// `entity.add:*` emission). This test pins the optional shape so
    /// the contract stays visible.
    #[test]
    fn view_info_without_entity_type() {
        let v = ViewInfo {
            id: "dashboard".into(),
            name: "Dashboard".into(),
            entity_type: None,
            kind: ViewKind::Grid.as_kebab_str().to_string(),
        };
        assert!(v.entity_type.is_none());
        assert_eq!(v.kind, "grid");
    }

    /// `Clone` is derived so a consumer can fan a `ViewInfo` out to
    /// multiple dispatch sites (e.g. one per moniker). This test pins
    /// the trait so removing it surfaces here.
    #[test]
    fn view_info_clone() {
        let v = ViewInfo {
            id: "calendar".into(),
            name: "Calendar".into(),
            entity_type: Some("event".into()),
            kind: ViewKind::Calendar.as_kebab_str().to_string(),
        };
        let v2 = v.clone();
        assert_eq!(v.id, v2.id);
        assert_eq!(v.kind, v2.kind);
    }
}
