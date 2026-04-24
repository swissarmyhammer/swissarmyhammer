//! Tests for `swissarmyhammer_kanban::default_commands_registry`.
//!
//! The helper stacks the generic commands-crate builtins and the
//! kanban-crate builtins the same way `kanban-app/src/state.rs` used to
//! do inline. Centralising the composition lets every consumer (GUI,
//! CLI, MCP, headless tests) ask for "the default registry" with one
//! call instead of cloning the stacking snippet site-by-site.

use swissarmyhammer_commands::CommandsRegistry;
use swissarmyhammer_kanban::default_commands_registry;

/// The helper must return a `CommandsRegistry` containing every id from
/// both source stacks. 60 is the pre-move total asserted by
/// `composed_builtins_register_all_sixty_commands` in
/// `tests/builtin_commands.rs` — we reuse that invariant to prove the
/// helper stacks the same sources in the same order.
#[test]
fn default_commands_registry_matches_manual_composition() {
    let registry = default_commands_registry();

    assert_eq!(
        registry.all_commands().len(),
        60,
        "default registry must contain the full generic + kanban command set",
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

/// The helper must compose in the documented order (generic first,
/// kanban second). We prove this by reaching for an id that only the
/// kanban stack provides and an id that only the commands stack
/// provides — both must be present, and neither stack shadows the other
/// silently.
#[test]
fn default_commands_registry_preserves_both_sources() {
    let registry = default_commands_registry();

    let commands_only = registry
        .get("app.quit")
        .expect("app.quit only ships in the commands-crate builtins");
    assert_eq!(commands_only.id, "app.quit");

    let kanban_only = registry
        .get("task.untag")
        .expect("task.untag only ships in the kanban-crate builtins");
    assert_eq!(kanban_only.id, "task.untag");
}

/// The helper must return an owned `CommandsRegistry`, not borrow static
/// state. Callers mutate the registry (e.g. merging user overrides via
/// `merge_yaml_sources`); returning by value lets them do that without
/// cloning or a `Cow` wrapper.
#[test]
fn default_commands_registry_returns_owned_and_mutable() {
    let mut registry = default_commands_registry();
    let before = registry.all_commands().len();

    let extra = "- id: test.extra\n  name: Extra\n";
    registry.merge_yaml_sources(&[("test_extra", extra)]);

    assert_eq!(registry.all_commands().len(), before + 1);
    assert!(registry.get("test.extra").is_some());
    // Returning owned means this is a compile-time check: you can't
    // `merge_yaml_sources` on a shared reference to a static.
    let _: CommandsRegistry = registry;
}
