use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Keybindings for a command, per keymap mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct KeysDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vim: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cua: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emacs: Option<String>,
}

/// Where a parameter value comes from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ParamSource {
    ScopeChain,
    Target,
    Args,
    Default,
}

/// A parameter definition for a command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParamDef {
    pub name: String,
    pub from: ParamSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
}

/// Where a command should appear in the native OS menu bar.
///
/// Commands with this metadata are collected by the Rust menu builder
/// and placed into native submenus. `path` names the menu hierarchy
/// (e.g. `["App"]` or `["App", "Settings"]`), `group` controls
/// separator grouping, and `order` sorts within a group.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MenuPlacement {
    /// Menu path hierarchy. The first element is the top-level menu name,
    /// subsequent elements create nested submenus.
    pub path: Vec<String>,
    /// Separator group within the menu (items in the same group are contiguous).
    #[serde(default)]
    pub group: usize,
    /// Sort order within the group.
    #[serde(default)]
    pub order: usize,
    /// If set, this item is part of a radio group (mutually exclusive check items).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radio_group: Option<String>,
}

/// YAML-loaded command metadata.
///
/// Describes a command's identity, scope requirements, keybindings,
/// parameters, and behavioral flags. Loaded from builtin YAML and
/// optionally overridden by `.kanban/commands/` files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CommandDef {
    pub id: String,
    pub name: String,
    /// Optional display name for native menus. Falls back to `name` when absent.
    /// Supports the same template variables as `name` (e.g. `{{entity.display_name}}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<KeysDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<ParamDef>,
    #[serde(default)]
    pub undoable: bool,
    #[serde(default)]
    pub context_menu: bool,
    /// Priority bucket for context-menu placement. Commands with the same
    /// `context_menu_group` render contiguously; a separator is inserted
    /// between groups. Lower values render first. Omit for "uncategorised"
    /// (sorts after all explicit groups).
    ///
    /// Intentionally independent of [`MenuPlacement::group`]: the native
    /// menu bar and the right-click context menu are two separate surfaces
    /// with different grouping needs (e.g. Cut/Copy/Paste share a native
    /// Edit-menu group, but context menus add Delete/Archive and Inspect
    /// buckets that have no native-menu placement).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_menu_group: Option<u32>,
    /// Sort order within the same [`Self::context_menu_group`]. Omit for
    /// default (0). Ties within the same group are broken by command id
    /// to keep emission deterministic regardless of YAML load order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_menu_order: Option<u32>,
    /// Optional native menu bar placement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu: Option<MenuPlacement>,
}

fn default_true() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

/// A command invocation — the wire format from frontend to backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandInvocation {
    pub cmd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_chain: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<HashMap<String, Value>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_def_yaml_round_trip() {
        let def = CommandDef {
            id: "foo.add".into(),
            name: "New Foo".into(),
            menu_name: None,
            scope: Some("entity:widget".into()),
            visible: true,
            keys: Some(KeysDef {
                vim: Some("a".into()),
                cua: Some("Mod+N".into()),
                emacs: None,
            }),
            params: vec![ParamDef {
                name: "widget".into(),
                from: ParamSource::ScopeChain,
                entity_type: Some("widget".into()),
                default: None,
            }],
            undoable: true,
            context_menu: true,
            context_menu_group: Some(1),
            context_menu_order: Some(2),
            menu: None,
        };
        let yaml = serde_yaml_ng::to_string(&def).unwrap();
        let parsed: CommandDef = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(def, parsed);
        // The new context-menu fields must survive a full YAML round trip so
        // downstream `commands/` overrides can opt into them.
        assert_eq!(parsed.context_menu_group, Some(1));
        assert_eq!(parsed.context_menu_order, Some(2));
    }

    #[test]
    fn command_def_minimal_yaml() {
        let yaml = r#"
id: app.quit
name: Quit
"#;
        let def: CommandDef = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(def.id, "app.quit");
        assert_eq!(def.name, "Quit");
        assert!(def.scope.is_none());
        assert!(def.visible);
        assert!(def.keys.is_none());
        assert!(def.params.is_empty());
        assert!(!def.undoable);
        assert!(!def.context_menu);
        assert!(def.context_menu_group.is_none());
        assert!(def.context_menu_order.is_none());
        assert!(def.menu.is_none());
    }

    #[test]
    fn command_def_with_all_fields() {
        let yaml = r#"
id: foo.remove
name: Remove Foo
scope: "entity:widget"
visible: true
undoable: true
context_menu: true
keys:
  vim: "x"
  cua: "Delete"
params:
  - name: widget
    from: scope_chain
    entity_type: widget
  - name: gadget
    from: scope_chain
    entity_type: gadget
"#;
        let def: CommandDef = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(def.id, "foo.remove");
        assert_eq!(def.scope.as_deref(), Some("entity:widget"));
        assert!(def.undoable);
        assert!(def.context_menu);
        assert_eq!(def.params.len(), 2);
        assert_eq!(def.params[0].from, ParamSource::ScopeChain);
        assert!(def.menu.is_none());
    }

    #[test]
    fn command_def_with_menu_placement() {
        let yaml = r#"
id: foo.create
name: New Foo
menu:
  path: [File]
  group: 0
  order: 0
"#;
        let def: CommandDef = serde_yaml_ng::from_str(yaml).unwrap();
        let menu = def.menu.unwrap();
        assert_eq!(menu.path, vec!["File"]);
        assert_eq!(menu.group, 0);
        assert_eq!(menu.order, 0);
        assert!(menu.radio_group.is_none());
    }

    #[test]
    fn menu_placement_with_radio_group() {
        let yaml = r#"
id: settings.keymap.vim
name: Vim Keybindings
menu:
  path: [App, Settings]
  group: 0
  order: 1
  radio_group: keymap
"#;
        let def: CommandDef = serde_yaml_ng::from_str(yaml).unwrap();
        let menu = def.menu.unwrap();
        assert_eq!(menu.path, vec!["App", "Settings"]);
        assert_eq!(menu.radio_group.as_deref(), Some("keymap"));
    }

    #[test]
    fn command_invocation_construction() {
        let inv = CommandInvocation {
            cmd: "foo.move".into(),
            scope_chain: Some(vec!["widget:01ABC".into(), "gadget:42".into()]),
            target: Some("gadget:99".into()),
            args: Some(HashMap::from([("drop_index".into(), serde_json::json!(2))])),
        };
        assert_eq!(inv.cmd, "foo.move");
        assert_eq!(inv.scope_chain.as_ref().unwrap().len(), 2);
        assert_eq!(inv.target.as_deref(), Some("gadget:99"));
        assert_eq!(inv.args.as_ref().unwrap()["drop_index"], 2);
    }

    #[test]
    fn command_invocation_minimal() {
        let inv = CommandInvocation {
            cmd: "app.quit".into(),
            scope_chain: None,
            target: None,
            args: None,
        };
        let json = serde_json::to_string(&inv).unwrap();
        let parsed: CommandInvocation = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.cmd, "app.quit");
        assert!(parsed.scope_chain.is_none());
    }

    #[test]
    fn command_def_with_menu_name_deserializes() {
        let yaml = r#"
id: foo.switch
name: "Switch to {{entity.display_name}}"
menu_name: "{{entity.display_name}} ({{entity.context.display_name}})"
"#;
        let def: CommandDef = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(def.id, "foo.switch");
        assert_eq!(def.name, "Switch to {{entity.display_name}}");
        assert_eq!(
            def.menu_name.as_deref(),
            Some("{{entity.display_name}} ({{entity.context.display_name}})")
        );
    }

    #[test]
    fn command_def_without_menu_name_deserializes_to_none() {
        let yaml = r#"
id: app.quit
name: Quit
"#;
        let def: CommandDef = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(def.menu_name.is_none());
    }

    #[test]
    fn command_def_menu_name_omitted_from_serialization_when_none() {
        let yaml = r#"
id: app.quit
name: Quit
"#;
        let def: CommandDef = serde_yaml_ng::from_str(yaml).unwrap();
        let serialized = serde_yaml_ng::to_string(&def).unwrap();
        assert!(!serialized.contains("menu_name"));
    }
}
