//! Core view types for the views registry.
//!
//! ViewDef is a simple metadata record describing a view. The `kind` field
//! is a renderer hint -- the actual rendering logic lives in the frontend.

use serde::{Deserialize, Serialize};

/// Unique identifier for a view definition (ULID string).
pub type ViewId = String;

/// The kind of view -- a renderer hint. The frontend uses this to select
/// which component to render. New kinds can be added without changing Rust.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ViewKind {
    Board,
    List,
    Calendar,
    Timeline,
    #[serde(other)]
    Unknown,
}

/// A command declared in a view definition.
///
/// Commands are metadata only -- the frontend attaches `execute` implementations
/// at mount time by matching on `id`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewCommand {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<ViewCommandKeys>,
}

/// Keybindings for a view command, per keymap mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewCommandKeys {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vim: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cua: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emacs: Option<String>,
}

/// A view definition -- metadata describing a named view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewDef {
    pub id: ViewId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub kind: ViewKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub card_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<ViewCommand>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_kind_yaml_round_trip() {
        let kind = ViewKind::Board;
        let yaml = serde_yaml::to_string(&kind).unwrap();
        let parsed: ViewKind = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(kind, parsed);
    }

    #[test]
    fn view_def_yaml_round_trip() {
        let def = ViewDef {
            id: "01JMVIEW0000000000BOARD0".into(),
            name: "Board".into(),
            icon: Some("kanban".into()),
            kind: ViewKind::Board,
            entity_type: Some("task".into()),
            card_fields: vec!["title".into(), "tags".into()],
            commands: vec![ViewCommand {
                id: "board.newCard".into(),
                name: "New Card".into(),
                description: None,
                keys: Some(ViewCommandKeys {
                    vim: Some(":card new".into()),
                    cua: Some("Mod+N".into()),
                    emacs: None,
                }),
            }],
        };
        let yaml = serde_yaml::to_string(&def).unwrap();
        let parsed: ViewDef = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(def, parsed);
    }

    #[test]
    fn view_def_from_yaml() {
        let yaml = r#"
id: 01JMVIEW0000000000BOARD0
name: Board
icon: kanban
kind: board
entity_type: task
card_fields:
  - title
  - tags
  - assignees
  - progress
commands:
  - id: board.newCard
    name: New Card
    keys:
      vim: ":card new"
      cua: Mod+N
  - id: board.collapseAll
    name: Collapse Lanes
  - id: board.expandAll
    name: Expand Lanes
"#;
        let def: ViewDef = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.name, "Board");
        assert_eq!(def.kind, ViewKind::Board);
        assert_eq!(def.card_fields.len(), 4);
        assert_eq!(def.commands.len(), 3);
        assert_eq!(def.commands[0].id, "board.newCard");
        assert!(def.commands[0].keys.is_some());
        assert!(def.commands[1].keys.is_none());
    }

    #[test]
    fn view_def_minimal() {
        let yaml = r#"
id: "01ABC"
name: Test
kind: list
"#;
        let def: ViewDef = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.kind, ViewKind::List);
        assert!(def.icon.is_none());
        assert!(def.card_fields.is_empty());
        assert!(def.commands.is_empty());
    }

    #[test]
    fn unknown_kind_deserializes() {
        let yaml = r#"
id: "01ABC"
name: Test
kind: gantt
"#;
        let def: ViewDef = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.kind, ViewKind::Unknown);
    }
}
