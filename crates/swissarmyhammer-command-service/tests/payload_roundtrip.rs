//! Proves the `CommandRegistration` payload doesn't lose any field that
//! today's command YAML carries.
//!
//! For every entry in every builtin YAML file (under
//! `swissarmyhammer-kanban/builtin/commands/` and
//! `swissarmyhammer-commands/builtin/commands/`), this test:
//!
//! 1. Parses the YAML entry into a [`YamlCommandDef`] mirror (same shape as
//!    today's on-disk schema).
//! 2. Converts it into a [`CommandRegistration`] (synthesizing the required
//!    `execute` callback marker, which the runtime would supply).
//! 3. Round-trips that [`CommandRegistration`] through JSON.
//! 4. Asserts structural equality with the original.
//!
//! A drop or rename of any field on `CommandRegistration` surfaces here as
//! a failure on every YAML file that uses the affected field.

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use swissarmyhammer_command_service::{
    CallbackMarker, CommandRegistration, ParamDef, ParamShape, ParamSource,
};

/// Mirror of today's on-disk YAML command schema.
///
/// Kept here so the roundtrip test exercises every field a YAML author
/// could write, not just the subset the live `CommandDef` happens to
/// declare today. New fields must be added here AND on
/// [`CommandRegistration`].
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct YamlCommandDef {
    id: String,
    name: String,
    #[serde(default)]
    menu_name: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    visible: Option<bool>,
    #[serde(default)]
    keys: Option<HashMap<String, String>>,
    #[serde(default)]
    params: Option<Vec<YamlParamDef>>,
    #[serde(default)]
    undoable: Option<bool>,
    #[serde(default)]
    context_menu: Option<bool>,
    #[serde(default)]
    context_menu_group: Option<u32>,
    #[serde(default)]
    context_menu_order: Option<u32>,
    #[serde(default)]
    menu: Option<Value>,
    #[serde(default)]
    view_kinds: Option<Vec<String>>,
    #[serde(default)]
    tab_button: Option<Value>,
}

/// Mirror of today's on-disk YAML param schema.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct YamlParamDef {
    name: String,
    from: ParamSource,
    #[serde(default)]
    entity_type: Option<String>,
    #[serde(default)]
    default: Option<Value>,
    #[serde(default)]
    shape: Option<ParamShape>,
    #[serde(default)]
    options_from: Option<String>,
    #[serde(default)]
    options: Option<Vec<YamlParamOption>>,
    #[serde(default)]
    clear_command: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct YamlParamOption {
    value: String,
    label: String,
}

/// All YAML command files this test exercises.
///
/// Resolved relative to this crate's `CARGO_MANIFEST_DIR`. Adding a new
/// builtin command file requires adding it here so the roundtrip test
/// picks it up.
fn yaml_files() -> Vec<PathBuf> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate path must have a workspace root ancestor")
        .to_path_buf();

    let kanban = workspace_root.join("crates/swissarmyhammer-kanban/builtin/commands");
    let commands = workspace_root.join("crates/swissarmyhammer-commands/builtin/commands");

    let mut files = Vec::new();
    for dir in [&kanban, &commands] {
        for entry in std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("expected builtin commands dir {dir:?} to be readable: {e}"))
        {
            let path = entry.unwrap().path();
            if path.extension().map(|s| s == "yaml").unwrap_or(false) {
                files.push(path);
            }
        }
    }
    files.sort();
    assert!(
        files.len() >= 12,
        "expected at least 12 builtin command YAML files, found {}: {files:?}",
        files.len()
    );
    files
}

/// Convert a parsed YAML entry into a [`CommandRegistration`].
///
/// Synthesizes the required `execute` callback marker that the runtime
/// would supply; the YAML files are declaration-only (no callbacks).
fn yaml_to_registration(def: YamlCommandDef) -> CommandRegistration {
    CommandRegistration {
        id: def.id,
        name: def.name,
        menu_name: def.menu_name,
        description: None,
        category: None,
        scope: def.scope.map(|s| vec![s]),
        keys: def.keys,
        menu: def.menu,
        context_menu: def.context_menu,
        context_menu_group: def.context_menu_group,
        context_menu_order: def.context_menu_order,
        tab_button: def.tab_button,
        view_kinds: def.view_kinds,
        undoable: def.undoable,
        visible: def.visible,
        params: def
            .params
            .map(|ps| ps.into_iter().map(yaml_param_to_param).collect()),
        available: None,
        execute: CallbackMarker::new("cb_synthetic_execute"),
    }
}

fn yaml_param_to_param(p: YamlParamDef) -> ParamDef {
    ParamDef {
        name: p.name,
        from: p.from,
        entity_type: p.entity_type,
        default: p.default,
        shape: p.shape,
        options_from: p.options_from,
        options: p.options.map(|os| {
            os.into_iter()
                .map(|o| swissarmyhammer_command_service::ParamOption {
                    value: o.value,
                    label: o.label,
                })
                .collect()
        }),
        clear_command: p.clear_command,
    }
}

#[test]
fn every_yaml_command_round_trips_through_json() {
    let files = yaml_files();
    let mut total = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for path in &files {
        let yaml_text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));
        let entries: Vec<YamlCommandDef> = serde_yaml_ng::from_str(&yaml_text)
            .unwrap_or_else(|e| panic!("failed to parse {path:?}: {e}"));

        for entry in entries {
            total += 1;
            let id = entry.id.clone();
            let original = yaml_to_registration(entry);

            let json = match serde_json::to_value(&original) {
                Ok(v) => v,
                Err(e) => {
                    failures.push(format!("{id}: serialize: {e}"));
                    continue;
                }
            };
            let parsed: CommandRegistration = match serde_json::from_value(json.clone()) {
                Ok(p) => p,
                Err(e) => {
                    failures.push(format!("{id}: deserialize: {e}\n  json = {json}"));
                    continue;
                }
            };

            if original != parsed {
                failures.push(format!(
                    "{id}: round-trip diverged\n  original = {original:#?}\n  parsed   = {parsed:#?}"
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {total} commands failed to round-trip:\n{}",
        failures.len(),
        failures.join("\n")
    );

    // Sanity bound: today there are ~60+ builtin commands. If the count
    // drops well below that, something is silently filtering YAML entries.
    assert!(
        total >= 60,
        "expected at least 60 YAML command entries across all builtin files, only saw {total}"
    );
}

/// The `keys: {}` empty-map case (used by every command in
/// `perspective.yaml`) must round-trip cleanly — it differs from `keys`
/// being absent and the type system has to allow both shapes.
#[test]
fn empty_keys_map_round_trips() {
    let reg = CommandRegistration {
        id: "test.empty_keys".into(),
        name: "Test Empty Keys".into(),
        menu_name: None,
        description: None,
        category: None,
        scope: None,
        keys: Some(HashMap::new()),
        menu: None,
        context_menu: None,
        context_menu_group: None,
        context_menu_order: None,
        tab_button: None,
        view_kinds: None,
        undoable: None,
        visible: None,
        params: None,
        available: None,
        execute: CallbackMarker::new("cb_x"),
    };

    let json = serde_json::to_value(&reg).unwrap();
    let parsed: CommandRegistration = serde_json::from_value(json).unwrap();
    assert_eq!(reg, parsed);
    assert_eq!(parsed.keys.as_ref().unwrap().len(), 0);
}

/// `CallbackMarker` serializes as `{ "$callback": "<id>" }` and
/// deserializes from the same shape — the wire format the plugin SDK
/// emits.
#[test]
fn callback_marker_wire_shape_is_dollar_callback() {
    let marker = CallbackMarker::new("cb_abc");
    let json = serde_json::to_value(&marker).unwrap();
    assert_eq!(json, serde_json::json!({"$callback": "cb_abc"}));

    let parsed: CallbackMarker =
        serde_json::from_value(serde_json::json!({"$callback": "cb_xyz"})).unwrap();
    assert_eq!(parsed.callback_id, "cb_xyz");
}
