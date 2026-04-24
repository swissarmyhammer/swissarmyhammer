//! Integration tests for the kanban crate's builtin command YAML sources.
//!
//! These tests verify two contracts:
//!
//! 1. `swissarmyhammer_kanban::builtin_yaml_sources()` embeds every kanban-
//!    specific command YAML and parses into a registry with the expected
//!    28 command IDs.
//! 2. When the app composes both the generic
//!    (`swissarmyhammer_commands::builtin_yaml_sources`) and kanban
//!    (`swissarmyhammer_kanban::builtin_yaml_sources`) sources — the pattern
//!    used by `kanban-app/src/state.rs` — no command id is lost in the move.
//!
//! The equivalent assertions lived in `swissarmyhammer-commands/src/registry.rs`
//! before the split. They moved here once the YAMLs left the commands crate
//! (see task 01KPXY7Q6980X2R5DVNVCY4SZK).

use swissarmyhammer_commands::CommandsRegistry;

/// The 28 kanban-specific command IDs shipped under
/// `swissarmyhammer-kanban/builtin/commands/`.
///
/// Grouped by source file for quick auditing against the YAMLs on disk.
const KANBAN_COMMAND_IDS: &[&str] = &[
    // task.yaml (3)
    "task.move",
    "task.untag",
    "task.doThisNext",
    // column.yaml (1)
    "column.reorder",
    // tag.yaml (1)
    "tag.update",
    // attachment.yaml (2)
    "attachment.open",
    "attachment.reveal",
    // file.yaml (4)
    "file.switchBoard",
    "file.closeBoard",
    "file.newBoard",
    "file.openBoard",
    // view.yaml (1) — relocated from `ui.view.set` in
    // 01KPY02X405QTP5ACH67THHSN8. "View" is a kanban concept, not a generic
    // UI primitive, so the declaration lives in the kanban domain.
    "view.set",
    // perspective.yaml (16) — `perspective.set` relocated from
    // `ui.perspective.set` in 01KPY02X405QTP5ACH67THHSN8 for the same
    // reason as `view.set`.
    "perspective.load",
    "perspective.save",
    "perspective.delete",
    "perspective.rename",
    "perspective.filter",
    "perspective.clearFilter",
    "perspective.group",
    "perspective.clearGroup",
    "perspective.sort.set",
    "perspective.sort.clear",
    "perspective.sort.toggle",
    "perspective.next",
    "perspective.prev",
    "perspective.goto",
    "perspective.list",
    "perspective.set",
];

/// Build a registry from the kanban crate's YAMLs alone and assert every
/// expected id is present. Replaces the deleted
/// `perspective_commands_all_registered` test from the commands crate.
#[test]
fn kanban_builtin_yamls_register_all_ids() {
    let sources = swissarmyhammer_kanban::builtin_yaml_sources();
    let sources_ref: Vec<(&str, &str)> = sources.iter().map(|(n, c)| (*n, *c)).collect();
    let registry = CommandsRegistry::from_yaml_sources(&sources_ref);

    assert_eq!(
        registry.all_commands().len(),
        KANBAN_COMMAND_IDS.len(),
        "kanban builtin yamls must register exactly {} commands",
        KANBAN_COMMAND_IDS.len(),
    );

    for id in KANBAN_COMMAND_IDS {
        assert!(
            registry.get(id).is_some(),
            "kanban builtin command `{id}` must be registered",
        );
    }
}

/// Re-check the perspective-specific invariants that lived in the deleted
/// `test_perspective_yaml_parses`: params, visible flag, and command name.
#[test]
fn perspective_yaml_retains_scope_and_visibility() {
    let sources = swissarmyhammer_kanban::builtin_yaml_sources();
    let sources_ref: Vec<(&str, &str)> = sources.iter().map(|(n, c)| (*n, *c)).collect();
    let registry = CommandsRegistry::from_yaml_sources(&sources_ref);

    // Load/save/delete should have a 'name' param.
    let load = registry.get("perspective.load").expect("perspective.load");
    assert_eq!(load.name, "Load Perspective");
    assert!(load.params.iter().any(|p| p.name == "name"));

    // Filter command should have 'filter' and 'perspective_id' params.
    let filter = registry
        .get("perspective.filter")
        .expect("perspective.filter");
    assert_eq!(filter.name, "Set Filter");
    assert!(filter.params.iter().any(|p| p.name == "filter"));
    assert!(filter.params.iter().any(|p| p.name == "perspective_id"));

    // All perspective commands should be visible (default true) except the
    // ones intentionally hidden from the command palette:
    //   - perspective.list: read-only introspection command
    //   - perspective.goto: takes `id` + `view_kind` from args; the palette
    //     has no UI for supplying them, so the bare command stays hidden
    //     (the user-facing "Go to Perspective: X" palette rows are emitted
    //     as `perspective.set` with `perspective_id` pre-filled by
    //     `scope_commands::emit_perspective_goto` and do NOT go through
    //     this bare-`perspective.goto` path)
    //   - perspective.rename: requires `id` + `new_name` args and has no
    //     palette args UI; the user-facing entry is
    //     `ui.entity.startRename`
    //   - perspective.set: requires a `perspective_id` arg the palette has
    //     no generic UI for; the user-facing palette rows for it are
    //     fan-out entries emitted by `scope_commands::emit_perspective_goto`
    //     with `perspective_id` pre-filled, one per perspective
    let hidden = [
        "perspective.list",
        "perspective.goto",
        "perspective.rename",
        "perspective.set",
    ];
    for cmd in registry.all_commands() {
        if !cmd.id.starts_with("perspective.") {
            continue;
        }
        if hidden.contains(&cmd.id.as_str()) {
            assert!(!cmd.visible, "{} should not be visible", cmd.id);
        } else {
            assert!(cmd.visible, "{} should be visible", cmd.id);
        }
    }
}

/// Spot-check the other kanban YAMLs still carry their scope/context_menu
/// metadata after the move — byte-identical file moves should not drop
/// anything, but this proves it.
#[test]
fn kanban_yaml_preserves_command_metadata() {
    let sources = swissarmyhammer_kanban::builtin_yaml_sources();
    let sources_ref: Vec<(&str, &str)> = sources.iter().map(|(n, c)| (*n, *c)).collect();
    let registry = CommandsRegistry::from_yaml_sources(&sources_ref);

    // task.untag is a multi-scope context-menu command.
    let untag = registry.get("task.untag").expect("task.untag");
    assert!(untag.context_menu);
    assert!(untag.undoable);

    // file.closeBoard exists and is available (was previously asserted in the
    // commands crate's `builtin_yaml_files_parse`).
    assert!(registry.get("file.closeBoard").is_some());
    assert!(registry.get("file.newBoard").is_some());
}

/// Compose both builtin sources the way `kanban-app/src/state.rs` does and
/// assert the full command count matches the pre-move total. Proves that
/// the file move lost no commands.
///
/// Count: 32 (commands-crate) + 28 (kanban-crate) = 60.
///
/// The 34/26 → 32/28 shift came from relocating `ui.view.set` and
/// `ui.perspective.set` into the kanban domain (new ids `view.set` and
/// `perspective.set`) in 01KPY02X405QTP5ACH67THHSN8.
#[test]
fn composed_builtins_register_all_sixty_commands() {
    let commands_sources = swissarmyhammer_commands::builtin_yaml_sources();
    let kanban_sources = swissarmyhammer_kanban::builtin_yaml_sources();

    let mut composed: Vec<(&str, &str)> =
        Vec::with_capacity(commands_sources.len() + kanban_sources.len());
    composed.extend(commands_sources.iter().map(|(n, c)| (*n, *c)));
    composed.extend(kanban_sources.iter().map(|(n, c)| (*n, *c)));

    let registry = CommandsRegistry::from_yaml_sources(&composed);

    assert_eq!(
        registry.all_commands().len(),
        60,
        "composed registry must match the pre-move command count",
    );

    // Spot checks across both source sets.
    assert!(registry.get("app.quit").is_some(), "commands crate");
    assert!(registry.get("entity.add").is_some(), "commands crate");
    assert!(registry.get("ui.palette.open").is_some(), "commands crate");
    assert!(registry.get("drag.start").is_some(), "commands crate");
    assert!(registry.get("task.untag").is_some(), "kanban crate");
    assert!(registry.get("perspective.goto").is_some(), "kanban crate");
    assert!(registry.get("file.closeBoard").is_some(), "kanban crate");
}

/// Verify the relocated `view.set` and `perspective.set` commands are
/// registered with `visible: false` and each accepts the expected single
/// `*_id` arg pulled from `args`.
///
/// These two commands were moved out of the generic `ui.yaml` into the
/// kanban domain in 01KPY02X405QTP5ACH67THHSN8 because "view" and
/// "perspective" are kanban concepts, not generic UI primitives. The
/// palette-facing entries are dynamic rows emitted by
/// `scope_commands::emit_view_switch` / `emit_perspective_goto` that now
/// dispatch `view.set` / `perspective.set` directly with pre-filled args —
/// 01KPZMXXEXKVE3RNPA4XJP0105 retired the old `view.switch:{id}` /
/// `perspective.goto:{id}` rewrite indirection — so the canonical
/// definitions must stay hidden from the palette (each row is only
/// reachable via its dynamic-emission counterpart with args attached).
#[test]
fn view_set_and_perspective_set_registered_hidden() {
    let sources = swissarmyhammer_kanban::builtin_yaml_sources();
    let sources_ref: Vec<(&str, &str)> = sources.iter().map(|(n, c)| (*n, *c)).collect();
    let registry = CommandsRegistry::from_yaml_sources(&sources_ref);

    // view.set — single `view_id` arg, hidden.
    let view_set = registry
        .get("view.set")
        .expect("view.set must be registered by view.yaml");
    assert!(
        !view_set.visible,
        "view.set requires a view_id arg the palette cannot provide — \
         must be visible: false"
    );
    assert_eq!(
        view_set.params.len(),
        1,
        "view.set must accept exactly one param (view_id from args)",
    );
    assert_eq!(view_set.params[0].name, "view_id");

    // perspective.set — single `perspective_id` arg, hidden.
    let perspective_set = registry
        .get("perspective.set")
        .expect("perspective.set must be registered by perspective.yaml");
    assert!(
        !perspective_set.visible,
        "perspective.set requires a perspective_id arg the palette cannot \
         provide — must be visible: false"
    );
    assert_eq!(
        perspective_set.params.len(),
        1,
        "perspective.set must accept exactly one param (perspective_id from args)",
    );
    assert_eq!(perspective_set.params[0].name, "perspective_id");
}
