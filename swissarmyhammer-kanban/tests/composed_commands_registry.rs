//! Tests for composing the kanban app's command registry via
//! `swissarmyhammer_commands::compose_registry!`.
//!
//! The macro stacks the generic commands-crate builtins and the
//! kanban-crate builtins. Centralising the composition at the macro
//! call site lets the app layer (kanban-app, kanban-cli, etc.) decide
//! which contributors to compose and in what order, while contributor
//! crates remain simple data-providers exposing `builtin_yaml_sources`.

use swissarmyhammer_commands::{compose_registry, CommandsRegistry};

/// The macro must return a `CommandsRegistry` containing every id from
/// the composed source stacks. 60 was the pre-focus total
/// (`commands` + `kanban`); composing in the focus crate's 8 `nav.*`
/// stubs lifted it to 68; adding `nav.jump` lifts it to 69.
#[test]
fn composed_registry_matches_manual_composition() {
    let registry: CommandsRegistry = compose_registry![
        swissarmyhammer_commands,
        swissarmyhammer_focus,
        swissarmyhammer_kanban,
    ];

    assert_eq!(
        registry.all_commands().len(),
        69,
        "composed registry must contain the full generic + focus + kanban command set",
    );

    // Spot checks across all three source sets.
    assert!(registry.get("app.quit").is_some(), "commands crate");
    assert!(registry.get("entity.add").is_some(), "commands crate");
    assert!(registry.get("ui.palette.open").is_some(), "commands crate");
    assert!(registry.get("drag.start").is_some(), "commands crate");
    assert!(registry.get("nav.up").is_some(), "focus crate");
    assert!(registry.get("nav.drillIn").is_some(), "focus crate");
    assert!(registry.get("task.untag").is_some(), "kanban crate");
    assert!(registry.get("perspective.goto").is_some(), "kanban crate");
    assert!(registry.get("file.closeBoard").is_some(), "kanban crate");
}

/// The macro must compose in the documented order (generic first,
/// focus middle, kanban last). We prove this by reaching for ids that
/// each contributor uniquely provides — none must shadow another
/// silently.
#[test]
fn composed_registry_preserves_both_sources() {
    let registry = compose_registry![
        swissarmyhammer_commands,
        swissarmyhammer_focus,
        swissarmyhammer_kanban,
    ];

    let commands_only = registry
        .get("app.quit")
        .expect("app.quit only ships in the commands-crate builtins");
    assert_eq!(commands_only.id, "app.quit");

    let focus_only = registry
        .get("nav.up")
        .expect("nav.up only ships in the focus-crate builtins");
    assert_eq!(focus_only.id, "nav.up");

    let kanban_only = registry
        .get("task.untag")
        .expect("task.untag only ships in the kanban-crate builtins");
    assert_eq!(kanban_only.id, "task.untag");
}

/// The macro must return an owned `CommandsRegistry`, not borrow static
/// state. Callers mutate the registry (e.g. merging user overrides via
/// `merge_yaml_sources`); returning by value lets them do that without
/// cloning or a `Cow` wrapper.
#[test]
fn composed_registry_returns_owned_and_mutable() {
    let mut registry = compose_registry![
        swissarmyhammer_commands,
        swissarmyhammer_focus,
        swissarmyhammer_kanban,
    ];
    let before = registry.all_commands().len();

    let extra = "- id: test.extra\n  name: Extra\n";
    registry.merge_yaml_sources(&[("test_extra", extra)]);

    assert_eq!(registry.all_commands().len(), before + 1);
    assert!(registry.get("test.extra").is_some());
    // Returning owned means this is a compile-time check: you can't
    // `merge_yaml_sources` on a shared reference to a static.
    let _: CommandsRegistry = registry;
}

/// Snapshot the full sorted set of command ids produced by the
/// macro-composed registry. This is the "command_id_set_unchanged_after
/// _macro_refactor" invariant: every id that was in the registry before
/// the macro refactor must still be there afterward. Capturing the full
/// sorted vec lets a future refactor catch any silent loss/addition at
/// review time.
#[test]
fn composed_registry_command_id_set_snapshot() {
    let registry = compose_registry![
        swissarmyhammer_commands,
        swissarmyhammer_focus,
        swissarmyhammer_kanban,
    ];

    let mut ids: Vec<&str> = registry
        .all_commands()
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    ids.sort();

    // Snapshot of the full sorted command-id set produced by the
    // composed registry. Originally captured at 60 entries from the
    // `commands` + `kanban` macro composition; intentionally expanded
    // to 68 when the focus crate landed its 8 `nav.*` YAML stubs (see
    // task `01KQYWM5BHFRPCRD70GF8YRCGY`). Bumped to 69 when
    // `nav.jump` (the AceJump-style overlay) joined the focus crate's
    // YAML (task `01KQYWV9DC866DGRPBRFR17ZEY`). If you intentionally
    // add or remove a command, update this list and explain why in
    // the commit message.
    let expected: Vec<&str> = vec![
        "app.about",
        "app.command",
        "app.dismiss",
        "app.help",
        "app.palette",
        "app.quit",
        "app.redo",
        "app.search",
        "app.undo",
        "attachment.open",
        "attachment.reveal",
        "column.reorder",
        "drag.cancel",
        "drag.complete",
        "drag.start",
        "entity.add",
        "entity.archive",
        "entity.copy",
        "entity.cut",
        "entity.delete",
        "entity.paste",
        "entity.unarchive",
        "entity.update_field",
        "file.closeBoard",
        "file.newBoard",
        "file.openBoard",
        "file.switchBoard",
        "nav.down",
        "nav.drillIn",
        "nav.drillOut",
        "nav.first",
        "nav.jump",
        "nav.last",
        "nav.left",
        "nav.right",
        "nav.up",
        "perspective.clearFilter",
        "perspective.clearGroup",
        "perspective.delete",
        "perspective.filter",
        "perspective.goto",
        "perspective.group",
        "perspective.list",
        "perspective.load",
        "perspective.next",
        "perspective.prev",
        "perspective.rename",
        "perspective.save",
        "perspective.set",
        "perspective.sort.clear",
        "perspective.sort.set",
        "perspective.sort.toggle",
        "settings.keymap.cua",
        "settings.keymap.emacs",
        "settings.keymap.vim",
        "tag.update",
        "task.doThisNext",
        "task.move",
        "task.untag",
        "ui.entity.startRename",
        "ui.inspect",
        "ui.inspector.close",
        "ui.inspector.close_all",
        "ui.mode.set",
        "ui.palette.close",
        "ui.palette.open",
        "ui.setFocus",
        "view.set",
        "window.new",
    ];

    assert_eq!(ids, expected, "command id set drifted; ids = {ids:?}",);
    assert_eq!(ids.len(), 69);
}
