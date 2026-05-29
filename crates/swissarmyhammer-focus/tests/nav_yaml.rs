//! Tests for the YAML-shipped nav.* command metadata contributed by the
//! focus crate.
//!
//! The focus crate exposes its 9 universal navigation commands (`nav.up`,
//! `nav.down`, `nav.left`, `nav.right`, `nav.first`, `nav.last`,
//! `nav.drillIn`, `nav.drillOut`, `nav.jump`) as data-only YAML stubs
//! that the `kanban-app` crate composes into its `CommandsRegistry`
//! via `compose_registry!`. Execution lives in React closures (they
//! need live `SpatialFocusActions`, or for `nav.jump`, the AppShell's
//! `jumpOpen` setter), so the YAML carries id, name, keys, and menu
//! placement only — see the task `app-shell.tsx` /
//! `keybindings.ts` cross-references.
//!
//! This test parses the YAML the same way `CommandsRegistry::from_yaml_sources`
//! does and asserts the union of all entries contains the nine nav ids
//! with the expected per-mode keys and `menu.path == ["Navigation"]`.

use std::collections::HashMap;

#[derive(serde::Deserialize)]
struct NavCommandDef {
    id: String,
    #[serde(default)]
    menu: Option<NavMenu>,
    #[serde(default)]
    keys: Option<NavKeys>,
}

#[derive(serde::Deserialize)]
struct NavMenu {
    #[serde(default)]
    path: Vec<String>,
}

#[derive(serde::Deserialize)]
struct NavKeys {
    #[serde(default)]
    vim: Option<String>,
    #[serde(default)]
    cua: Option<String>,
    #[serde(default)]
    emacs: Option<String>,
}

/// Parse every YAML source contributed by the focus crate and collapse
/// them into a single `id → CommandDef` map.
///
/// Mirrors the loader used by `CommandsRegistry::from_yaml_sources` —
/// each source is a top-level YAML sequence of `CommandDef` entries.
fn load_focus_commands() -> HashMap<String, NavCommandDef> {
    let sources = swissarmyhammer_focus::builtin_yaml_sources();
    let mut commands = HashMap::new();
    for (name, content) in sources {
        let defs: Vec<NavCommandDef> = serde_yaml_ng::from_str(content)
            .unwrap_or_else(|e| panic!("focus YAML source `{name}` failed to parse: {e}"));
        for def in defs {
            commands.insert(def.id.clone(), def);
        }
    }
    commands
}

/// All 9 nav command ids must ship from the focus crate's builtin YAML,
/// each with `menu.path == ["Navigation"]` so they collect into a single
/// top-level Navigation submenu, and each with the per-mode bindings the
/// React side has been carrying inline.
///
/// Bindings sourced from `kanban-app/ui/src/components/app-shell.tsx`
/// (`NAV_COMMAND_SPEC`) for the directional commands and from
/// `kanban-app/ui/src/lib/keybindings.ts` (`BINDING_TABLES`) for
/// `nav.drillIn` / `nav.drillOut` / `nav.jump`.
#[test]
fn nav_yaml_registers_all_nine_commands() {
    let commands = load_focus_commands();

    let expected_ids = [
        "nav.up",
        "nav.down",
        "nav.left",
        "nav.right",
        "nav.first",
        "nav.last",
        "nav.drillIn",
        "nav.drillOut",
        "nav.jump",
    ];
    for id in &expected_ids {
        assert!(
            commands.contains_key(*id),
            "focus YAML missing nav command `{id}`; got ids = {:?}",
            commands.keys().collect::<Vec<_>>(),
        );
    }
    assert_eq!(
        commands.len(),
        expected_ids.len(),
        "focus YAML must contain only the 9 nav.* commands; got {:?}",
        commands.keys().collect::<Vec<_>>(),
    );

    // Every nav command lands under the Navigation top-level menu.
    for id in &expected_ids {
        let cmd = &commands[*id];
        let placement = cmd
            .menu
            .as_ref()
            .unwrap_or_else(|| panic!("`{id}` is missing menu placement"));
        assert_eq!(
            placement.path,
            vec!["Navigation".to_string()],
            "`{id}` must place under the Navigation menu, got {:?}",
            placement.path,
        );
    }

    // Per-mode keybindings — pulled from `NAV_COMMAND_SPEC` (directional)
    // and `BINDING_TABLES` (drillIn/drillOut). The expected bindings are
    // the load-bearing contract: changes to the bindings must be
    // reflected here so a stray edit fails this test loudly.
    for spec in expected_key_bindings() {
        let cmd = &commands[spec.id];
        let keys = cmd
            .keys
            .as_ref()
            .unwrap_or_else(|| panic!("`{}` must have keys", spec.id));
        assert_eq!(
            keys.vim.as_deref(),
            spec.vim,
            "`{}` vim binding mismatch",
            spec.id,
        );
        assert_eq!(
            keys.cua.as_deref(),
            spec.cua,
            "`{}` cua binding mismatch",
            spec.id,
        );
        assert_eq!(
            keys.emacs.as_deref(),
            spec.emacs,
            "`{}` emacs binding mismatch",
            spec.id,
        );
    }
}

/// Per-command expected keybindings. Wrapped in a struct rather than a
/// 4-tuple so clippy's `type_complexity` lint stays quiet and the call
/// site reads like a table.
struct KeySpec {
    id: &'static str,
    vim: Option<&'static str>,
    cua: Option<&'static str>,
    emacs: Option<&'static str>,
}

/// Source of truth for the test's expected per-mode bindings. Pulled
/// from `NAV_COMMAND_SPEC` in `app-shell.tsx` and `BINDING_TABLES` in
/// `keybindings.ts`.
fn expected_key_bindings() -> Vec<KeySpec> {
    vec![
        KeySpec {
            id: "nav.up",
            vim: Some("k"),
            cua: Some("ArrowUp"),
            emacs: Some("Ctrl+p"),
        },
        KeySpec {
            id: "nav.down",
            vim: Some("j"),
            cua: Some("ArrowDown"),
            emacs: Some("Ctrl+n"),
        },
        KeySpec {
            id: "nav.left",
            vim: Some("h"),
            cua: Some("ArrowLeft"),
            emacs: Some("Ctrl+b"),
        },
        KeySpec {
            id: "nav.right",
            vim: Some("l"),
            cua: Some("ArrowRight"),
            emacs: Some("Ctrl+f"),
        },
        KeySpec {
            id: "nav.first",
            vim: None,
            cua: Some("Home"),
            emacs: Some("Alt+<"),
        },
        KeySpec {
            id: "nav.last",
            vim: Some("Shift+G"),
            cua: Some("End"),
            emacs: Some("Alt+>"),
        },
        KeySpec {
            id: "nav.drillIn",
            vim: Some("Enter"),
            cua: Some("Enter"),
            emacs: Some("Enter"),
        },
        KeySpec {
            id: "nav.drillOut",
            vim: Some("Escape"),
            cua: Some("Escape"),
            emacs: Some("Escape"),
        },
        KeySpec {
            id: "nav.jump",
            vim: Some("s"),
            cua: Some("Mod+G"),
            emacs: Some("Mod+G"),
        },
    ]
}
