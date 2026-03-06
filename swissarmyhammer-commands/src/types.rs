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
            id: "task.add".into(),
            name: "New Task".into(),
            scope: Some("entity:column".into()),
            visible: true,
            keys: Some(KeysDef {
                vim: Some("a".into()),
                cua: Some("Mod+N".into()),
                emacs: None,
            }),
            params: vec![ParamDef {
                name: "column".into(),
                from: ParamSource::ScopeChain,
                entity_type: Some("column".into()),
                default: None,
            }],
            undoable: true,
            context_menu: false,
        };
        let yaml = serde_yaml::to_string(&def).unwrap();
        let parsed: CommandDef = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(def, parsed);
    }

    #[test]
    fn command_def_minimal_yaml() {
        let yaml = r#"
id: app.quit
name: Quit
"#;
        let def: CommandDef = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.id, "app.quit");
        assert_eq!(def.name, "Quit");
        assert!(def.scope.is_none());
        assert!(def.visible);
        assert!(def.keys.is_none());
        assert!(def.params.is_empty());
        assert!(!def.undoable);
        assert!(!def.context_menu);
    }

    #[test]
    fn command_def_with_all_fields() {
        let yaml = r#"
id: task.untag
name: Remove Tag
scope: "entity:tag"
visible: true
undoable: true
context_menu: true
keys:
  vim: "x"
  cua: "Delete"
params:
  - name: tag
    from: scope_chain
    entity_type: tag
  - name: task
    from: scope_chain
    entity_type: task
"#;
        let def: CommandDef = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.id, "task.untag");
        assert_eq!(def.scope.as_deref(), Some("entity:tag"));
        assert!(def.undoable);
        assert!(def.context_menu);
        assert_eq!(def.params.len(), 2);
        assert_eq!(def.params[0].from, ParamSource::ScopeChain);
    }

    #[test]
    fn command_invocation_construction() {
        let inv = CommandInvocation {
            cmd: "task.move".into(),
            scope_chain: Some(vec!["task:01ABC".into(), "column:todo".into()]),
            target: Some("column:doing".into()),
            args: Some(HashMap::from([(
                "drop_index".into(),
                serde_json::json!(2),
            )])),
        };
        assert_eq!(inv.cmd, "task.move");
        assert_eq!(inv.scope_chain.as_ref().unwrap().len(), 2);
        assert_eq!(inv.target.as_deref(), Some("column:doing"));
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
}
