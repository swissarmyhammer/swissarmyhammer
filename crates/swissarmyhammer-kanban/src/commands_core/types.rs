use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub use swissarmyhammer_command_options::ParamOption;

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

/// Shape of a parameter for runtime collection.
///
/// `shape` answers "how should the UI ask the user for this value?" It is
/// distinct from [`ParamSource`] (`from`), which answers "where does the
/// value come from when the command runs?". A param with `from: Args` and
/// `shape: Some(Enum)` says: the value lives in the args bag at dispatch
/// time, AND when the UI needs to collect it from the user it should
/// render an enum picker. A param with `from: ScopeChain` and `shape:
/// None` says: the value already comes from the resolved scope chain —
/// the runtime never asks the user for it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ParamShape {
    /// User picks from a list of options. Options come from
    /// [`ParamDef::options_from`] resolver OR an inline
    /// [`ParamDef::options`].
    Enum,
    /// Single-line free text.
    Text,
    /// Multiline expression (e.g. filter DSL). Frontend hosts a
    /// rich editor (CodeMirror) for this shape.
    Expression,
    Number,
    Date,
    Boolean,
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
    /// Shape of this param for runtime collection. When `None`, the
    /// param's [`Self::from`] field (args / scope chain / target /
    /// default) already supplies the value — the runtime never asks the
    /// user for it and the frontend does not render a picker for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<ParamShape>,
    /// For enum-shaped params, names the backend resolver that supplies
    /// the concrete option list at `commands_for_scope` emission time.
    /// Resolver names are stringly-typed and looked up in a backend
    /// resolver registry (separate task in the command-driven-ui epic).
    /// When both `options_from` and [`Self::options`] are set, the
    /// resolver wins and inline `options` are treated as a fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options_from: Option<String>,
    /// Inline option list for enum-shaped params whose values are static
    /// and known at YAML write time. Prefer this over `options_from`
    /// when the list is fixed (e.g. sort direction = asc / desc).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<ParamOption>>,
    /// Sibling command id to dispatch in place of this command when the
    /// user picks the "clear" sentinel (empty-string value) for an
    /// enum-shaped param.
    ///
    /// Surfaces a "None" affordance inside the picker popover so the user
    /// can clear state in one click instead of leaving the popover and
    /// hunting through the right-click menu. The frontend
    /// [`CommandPopover`] auto-prepends a "(none)" `<option>` with
    /// `value=""` whenever a param carries this annotation, and the
    /// [`CommandButton`] commit handler intercepts the empty-string
    /// submission to dispatch `clear_command` (instead of the parent
    /// command) with the same scope-resolved args.
    ///
    /// Set on the YAML param of any "set X" command whose paired
    /// "clear X" command is reachable from the same scope chain. The
    /// first user is `perspective.group`'s `group` param, which
    /// redirects to `perspective.clearGroup` — restoring the legacy
    /// `<GroupSelector>` "None" entry that the command-driven-ui
    /// migration would otherwise drop. The Sort migration is expected
    /// to reuse the same annotation when it lands.
    ///
    /// The id is stringly-typed; the runtime does not validate it
    /// against the registry at YAML-load time. A typo surfaces as a
    /// "command not found" dispatch error in the live app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clear_command: Option<String>,
}

/// Tab-button affordance metadata for a command.
///
/// When set on a [`CommandDef`], the command renders as a tab-button on
/// surfaces that consume `tab_button`-tagged commands (today: the
/// perspective tab bar). Absent means no tab-button affordance — the
/// command still surfaces in palettes / menus per its other metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TabButtonDef {
    /// Lucide-react icon component name (e.g. `"filter"`, `"group"`,
    /// `"arrow-up-down"`). Resolved by the frontend's icon registry at
    /// render time; an unknown name renders a fallback glyph rather
    /// than failing the surface.
    pub icon: String,
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
    /// Declarative view-kind UI-surface filter.
    ///
    /// `None` (the default) means the command surfaces in every view kind —
    /// the common case. `Some(list)` restricts emission to scopes whose
    /// resolved view kind matches one of the listed values (e.g.
    /// `["grid"]` for grid-only sort commands).
    ///
    /// View kinds are encoded as the kebab-case strings produced by
    /// `ViewKind`'s `#[serde(rename_all = "kebab-case")]`. Storing them as
    /// `String` keeps `swissarmyhammer-commands` independent of the
    /// `swissarmyhammer-views` crate.
    ///
    /// This is a UI-surface gate only — the dispatcher still routes the
    /// command at runtime if it is somehow invoked from a non-matching
    /// view (e.g. via MCP or shell). The filter exists to keep palettes,
    /// context menus, and native menus from offering commands that have
    /// no meaningful behavior in the active view kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_kinds: Option<Vec<String>>,
    /// When set, this command renders as a tab-button affordance on
    /// surfaces that consume `tab_button`-tagged commands (today: the
    /// perspective tab bar). Absent means no tab-button affordance.
    ///
    /// The frontend `<CommandButton>` component is the consumer; the
    /// resolver registry pre-populates each tab-button command's params
    /// (e.g. enum options for a filter-field picker) at
    /// `commands_for_scope` emission time so the button can render
    /// without an extra round-trip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_button: Option<TabButtonDef>,
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
                shape: None,
                options_from: None,
                options: None,
                clear_command: None,
            }],
            undoable: true,
            context_menu: true,
            context_menu_group: Some(1),
            context_menu_order: Some(2),
            menu: None,
            view_kinds: None,
            tab_button: None,
        };
        let yaml = serde_yaml_ng::to_string(&def).unwrap();
        let parsed: CommandDef = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(def, parsed);
        // The new context-menu fields must survive a full YAML round trip so
        // downstream `commands/` overrides can opt into them.
        assert_eq!(parsed.context_menu_group, Some(1));
        assert_eq!(parsed.context_menu_order, Some(2));
    }

    /// `view_kinds` round-trips through YAML so command YAML files can opt
    /// into the declarative view-kind UI-surface filter without bespoke
    /// Rust support. Mirrors the shape of `command_def_yaml_round_trip` but
    /// with a non-`None` `view_kinds` so the field is exercised end-to-end.
    #[test]
    fn command_def_view_kinds_yaml_round_trip() {
        let def = CommandDef {
            id: "perspective.sort.set".into(),
            name: "Sort Field".into(),
            menu_name: None,
            scope: Some("entity:perspective".into()),
            visible: true,
            keys: None,
            params: vec![],
            undoable: true,
            context_menu: false,
            context_menu_group: None,
            context_menu_order: None,
            menu: None,
            view_kinds: Some(vec!["grid".into()]),
            tab_button: None,
        };
        let yaml = serde_yaml_ng::to_string(&def).unwrap();
        let parsed: CommandDef = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(def, parsed);
        assert_eq!(
            parsed.view_kinds.as_deref(),
            Some(&["grid".to_string()][..])
        );
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
        assert!(def.view_kinds.is_none());
    }

    /// Minimal YAML without a `view_kinds:` key must parse with
    /// `view_kinds == None` — i.e. the command surfaces in every view kind
    /// by default. This is the regression guard against accidentally
    /// requiring every YAML entry to declare the field after the schema is
    /// extended.
    #[test]
    fn command_def_view_kinds_defaults_to_none() {
        let yaml = r#"
id: app.quit
name: Quit
"#;
        let def: CommandDef = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(
            def.view_kinds.is_none(),
            "view_kinds must default to None when omitted from YAML"
        );
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

    /// A `CommandDef` carrying a `tab_button` survives a YAML round-trip so
    /// downstream surfaces (the perspective tab bar today, other tab-button
    /// surfaces in the future) can opt into tab-button rendering by writing
    /// the metadata directly in YAML.
    #[test]
    fn command_def_with_tab_button_round_trips() {
        let def = CommandDef {
            id: "perspective.filter.set".into(),
            name: "Filter".into(),
            menu_name: None,
            scope: Some("entity:perspective".into()),
            visible: true,
            keys: None,
            params: vec![],
            undoable: false,
            context_menu: false,
            context_menu_group: None,
            context_menu_order: None,
            menu: None,
            view_kinds: None,
            tab_button: Some(TabButtonDef {
                icon: "filter".into(),
            }),
        };
        let yaml = serde_yaml_ng::to_string(&def).unwrap();
        let parsed: CommandDef = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(def, parsed);
        assert_eq!(parsed.tab_button.as_ref().unwrap().icon, "filter");
    }

    /// A `ParamDef` carrying `shape`, `options_from`, and inline `options`
    /// must round-trip through YAML — the resolver registry, the frontend
    /// `<CommandPopover>`, and the per-command migrations all rely on these
    /// fields being preserved end-to-end.
    #[test]
    fn command_def_with_param_shape_and_options_round_trips() {
        let def = CommandDef {
            id: "perspective.sort.set".into(),
            name: "Sort".into(),
            menu_name: None,
            scope: Some("entity:perspective".into()),
            visible: true,
            keys: None,
            params: vec![ParamDef {
                name: "field".into(),
                from: ParamSource::Args,
                entity_type: None,
                default: None,
                shape: Some(ParamShape::Enum),
                options_from: Some("perspective.fields".into()),
                options: Some(vec![ParamOption {
                    value: "status".into(),
                    label: "Status".into(),
                }]),
                clear_command: None,
            }],
            undoable: false,
            context_menu: false,
            context_menu_group: None,
            context_menu_order: None,
            menu: None,
            view_kinds: None,
            tab_button: None,
        };
        let yaml = serde_yaml_ng::to_string(&def).unwrap();
        let parsed: CommandDef = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(def, parsed);
        let param = &parsed.params[0];
        assert_eq!(param.shape, Some(ParamShape::Enum));
        assert_eq!(param.options_from.as_deref(), Some("perspective.fields"));
        let opts = param.options.as_ref().unwrap();
        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].value, "status");
        assert_eq!(opts[0].label, "Status");
    }

    /// A `ParamDef` carrying `clear_command: Some(...)` must round-trip
    /// through YAML — the annotation drives the frontend
    /// `<CommandPopover>`'s "(none)" affordance and the
    /// `<CommandButton>` commit-handler's clear-sentinel redirection, so
    /// a regression that silently drops it during (de)serialization
    /// would re-introduce the legacy "no way to clear" UX bug. The
    /// existing
    /// [`command_def_with_param_shape_and_options_round_trips`] test
    /// covers the `clear_command: None` path; this test pins the
    /// `Some(...)` half of the contract.
    #[test]
    fn command_def_with_param_clear_command_round_trips() {
        let def = CommandDef {
            id: "perspective.group".into(),
            name: "Group By".into(),
            menu_name: None,
            scope: Some("entity:perspective".into()),
            visible: true,
            keys: None,
            params: vec![ParamDef {
                name: "group".into(),
                from: ParamSource::Args,
                entity_type: None,
                default: None,
                shape: Some(ParamShape::Enum),
                options_from: Some("perspective.fields".into()),
                options: None,
                clear_command: Some("perspective.clearGroup".into()),
            }],
            undoable: false,
            context_menu: false,
            context_menu_group: None,
            context_menu_order: None,
            menu: None,
            view_kinds: None,
            tab_button: None,
        };
        let yaml = serde_yaml_ng::to_string(&def).unwrap();
        let parsed: CommandDef = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(def, parsed);
        let param = &parsed.params[0];
        assert_eq!(
            param.clear_command.as_deref(),
            Some("perspective.clearGroup"),
            "clear_command: Some(...) must survive a full YAML round trip"
        );
    }

    /// A minimal `CommandDef` must not emit any of the new fields when
    /// serialized — existing YAML files round-trip unchanged after the
    /// schema is extended, and `commands_for_scope` payloads stay small
    /// for commands that don't opt into tab-button / param-picker UX.
    #[test]
    fn command_def_without_new_fields_omits_them_from_yaml() {
        let yaml = r#"
id: app.quit
name: Quit
"#;
        let def: CommandDef = serde_yaml_ng::from_str(yaml).unwrap();
        let serialized = serde_yaml_ng::to_string(&def).unwrap();
        assert!(
            !serialized.contains("tab_button"),
            "tab_button must be omitted when None: {serialized}"
        );
        assert!(
            !serialized.contains("shape"),
            "shape must be omitted when None: {serialized}"
        );
        assert!(
            !serialized.contains("options_from"),
            "options_from must be omitted when None: {serialized}"
        );
        assert!(
            !serialized.contains("options"),
            "options must be omitted when None: {serialized}"
        );
        // Sanity: the round-trip still produces an equivalent CommandDef.
        let parsed: CommandDef = serde_yaml_ng::from_str(&serialized).unwrap();
        assert!(parsed.tab_button.is_none());
        assert!(parsed.params.is_empty());
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
