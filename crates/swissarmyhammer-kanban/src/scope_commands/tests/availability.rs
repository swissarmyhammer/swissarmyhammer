//! The gates that keep a command out of the list.
//!
//! These tests hold the global commands that are always present, the
//! keybindings that pass through, the `visible` flag, the parent that a
//! command needs, the scope pin of a registry command, and the target that
//! each row carries.

use super::*;

// =========================================================================
// Global commands always present
// =========================================================================

#[test]
fn app_quit_always_available() {
    let (registry, impls, fields, ui) = setup();
    for scope in [
        vec![],
        vec!["board:b".into()],
        vec!["task:t".into(), "column:c".into()],
        vec!["tag:x".into(), "task:t".into(), "column:c".into()],
    ] {
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            None,
            None,
        );
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(
            ids.contains(&"app.quit"),
            "app.quit should be in scope {:?}",
            scope
        );
    }
}

#[test]
fn app_undo_redo_filtered_out_when_stack_empty() {
    let (registry, impls, fields, ui) = setup();
    // Default UIState: can_undo=false, can_redo=false
    let scope = vec!["task:01X".into(), "column:todo".into()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
    assert!(
        !ids.contains(&"app.undo"),
        "undo should not appear when stack is empty"
    );
    assert!(
        !ids.contains(&"app.redo"),
        "redo should not appear when stack is empty"
    );
}

#[test]
fn app_undo_available_when_ui_state_says_so() {
    let (registry, impls, fields, ui) = setup();
    ui.set_undo_redo_state(true, false);
    let scope = vec!["task:01X".into(), "column:todo".into()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
    assert!(
        ids.contains(&"app.undo"),
        "undo should appear when can_undo is true"
    );
    assert!(
        !ids.contains(&"app.redo"),
        "redo should not appear when can_redo is false"
    );
}

#[test]
fn app_redo_available_when_ui_state_says_so() {
    let (registry, impls, fields, ui) = setup();
    ui.set_undo_redo_state(false, true);
    let scope = vec!["task:01X".into(), "column:todo".into()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
    assert!(
        !ids.contains(&"app.undo"),
        "undo should not appear when can_undo is false"
    );
    assert!(
        ids.contains(&"app.redo"),
        "redo should appear when can_redo is true"
    );
}

// =========================================================================
// Keys pass through
// =========================================================================

#[test]
fn copy_task_has_keybindings() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["task:01X".into(), "column:todo".into()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );
    let copy = cmds.iter().find(|c| c.name == "Copy Task").unwrap();
    let keys = copy.keys.as_ref().expect("Copy Task should have keys");
    assert_eq!(keys.cua.as_deref(), Some("Mod+C"));
    assert_eq!(keys.vim.as_deref(), Some("y"));
}

// =========================================================================
// Visible flag
// =========================================================================

#[test]
fn invisible_commands_not_returned() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["task:01X".into(), "column:todo".into()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
    // entity.update_field is visible: false in YAML
    assert!(
        !ids.contains(&"entity.update_field"),
        "invisible commands should be excluded"
    );
}

// =========================================================================
// Cut tag requires task in scope
// =========================================================================

#[test]
fn cut_tag_not_available_without_task_parent() {
    // A tag is in scope but no task — `entity.cut` with a tag target
    // requires a task in scope to untag from. `CutEntityCmd::available()`
    // gates this and the auto-emitted command must be filtered out.
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["tag:bug".into()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );
    let cut_tag = cmds
        .iter()
        .find(|c| c.id == "entity.cut" && c.target.as_deref() == Some("tag:bug"));
    assert!(
        cut_tag.is_none(),
        "entity.cut on a tag target must NOT surface without a task in \
         scope (no destructive op is defined); got: {:?}",
        cut_tag,
    );
}

// =========================================================================
// Targets are correct
// =========================================================================

#[test]
fn entity_commands_have_correct_targets() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "tag:01TAG".into(),
        "task:01TASK".into(),
        "column:todo".into(),
    ];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );

    // With dedup-by-id (innermost wins), the tag scope wins for entity.copy.
    // "Copy Tag" appears with target "tag:01TAG"; "Copy Task" is deduped away.
    let copy_tag = cmds.iter().find(|c| c.name == "Copy Tag").unwrap();
    assert_eq!(copy_tag.target.as_deref(), Some("tag:01TAG"));

    let copy_task = cmds.iter().find(|c| c.name == "Copy Task");
    assert!(
        copy_task.is_none(),
        "Copy Task should be deduped away when tag is innermost scope"
    );

    // Task-only scope to verify task target is correct when no tag present
    let task_only_scope = vec!["task:01TASK".into(), "column:todo".into()];
    let task_cmds = commands_for_scope(
        &task_only_scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );
    let copy_task_direct = task_cmds.iter().find(|c| c.name == "Copy Task").unwrap();
    assert_eq!(copy_task_direct.target.as_deref(), Some("task:01TASK"));

    let inspect_col = cmds.iter().find(|c| c.name == "Inspect Column");
    if let Some(ic) = inspect_col {
        assert_eq!(ic.target.as_deref(), Some("column:todo"));
    }
}

// =========================================================================
// Scoped registry commands (task.add needs column, task.untag needs tag+task)
// =========================================================================

/// Task creation must flow through the dynamic `entity.add:task`
/// emission (driven by the active view's `entity_type`), NOT the legacy
/// `task.add` registry entry. Having both live produced duplicate
/// "New Task" items in the palette and a slug-id collision that caused
/// the second and later creates to silently drop.
#[test]
fn task_add_never_emitted_from_registry() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["column:todo".into(), "board:board".into()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
    assert!(
        !ids.contains(&"task.add"),
        "task.add must be gone — creation is dynamic `entity.add:task`. got: {:?}",
        ids
    );
}

#[test]
fn task_untag_available_with_tag_and_task() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["tag:bug".into(), "task:01X".into(), "column:todo".into()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
    assert!(
        ids.contains(&"task.untag"),
        "task.untag should be available with tag+task: {:?}",
        ids
    );
}

#[test]
fn task_untag_not_available_without_tag() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["task:01X".into(), "column:todo".into()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
    assert!(
        !ids.contains(&"task.untag"),
        "task.untag should NOT be available without tag"
    );
}

// =========================================================================
// Other entity types (actor, attachment)
// =========================================================================

#[test]
fn actor_scope_has_inspect() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["actor:alice".into()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );
    let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
    assert!(
        names.contains(&"Inspect Actor"),
        "actor should have Inspect Actor: {:?}",
        names
    );
}

/// `ui.inspect` must surface on an actor scope purely from the cross-cutting
/// auto-emit pass — `actor.yaml` declares no `commands:` opt-in, so the only
/// way `ui.inspect` reaches an actor moniker is the dispatcher walking
/// `from: target` registry commands and emitting them per-moniker.
///
/// This is the GREEN-step companion to the YAML hygiene guard
/// (`yaml_hygiene_entity_schemas_have_no_commands_key`): that test forbids
/// entity YAML files from declaring any `commands:` key at all, this test
/// proves the command still appears without any per-entity opt-in.
/// Together they pin the "declare once, auto-emit per moniker" contract
/// for actors.
#[test]
fn ui_inspect_auto_emits_on_actor_without_opt_in() {
    // Guard: if a future change re-introduces a `commands:` block on
    // actor.yaml (or otherwise re-lists `ui.inspect` there), this test's
    // premise — that auto-emit alone is responsible for the surfaced
    // command — is invalidated. Fail loudly rather than silently passing
    // for the wrong reason.
    let actor_yaml = builtin_entity_definitions()
        .into_iter()
        .find_map(|(name, yaml)| (name == "actor").then_some(yaml))
        .expect("builtin entity definitions must include actor");
    let actor_raw: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(actor_yaml).expect("builtin actor.yaml must parse as generic YAML");
    assert!(
        actor_raw.get("commands").is_none(),
        "actor.yaml must not carry a `commands:` key — `ui.inspect` is \
         expected to come from the cross-cutting auto-emit pass, not a \
         per-entity opt-in"
    );

    let (registry, impls, fields, ui) = setup();
    let scope = vec!["actor:alice".into()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );

    let inspect = cmds
        .iter()
        .find(|c| c.id == "ui.inspect")
        .unwrap_or_else(|| {
            panic!(
                "ui.inspect must auto-emit on scope [actor:alice] without \
                 a per-entity opt-in; got commands: {:?}",
                cmds.iter().map(|c| (&c.id, &c.target)).collect::<Vec<_>>()
            )
        });
    assert_eq!(
        inspect.target.as_deref(),
        Some("actor:alice"),
        "ui.inspect target must equal the actor moniker, got: {:?}",
        inspect.target
    );
    assert!(
        inspect.context_menu,
        "ui.inspect must opt into the context menu for an actor scope"
    );
    assert!(
        inspect.available,
        "ui.inspect must be available for an actor scope — \
         first_inspectable + ctx.target both qualify"
    );
}

// =========================================================================
// Unknown entity type in scope
// =========================================================================

#[test]
fn unknown_entity_type_ignored() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["foo:bar".into(), "task:01X".into(), "column:todo".into()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );
    // Should still have task commands — unknown type just gets skipped
    let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"Copy Task"));
    // Should NOT have commands for "foo" type
    assert!(!cmds.iter().any(|c| c.target.as_deref() == Some("foo:bar")));
}

// =========================================================================
// Drag commands (visible: false) excluded
// =========================================================================

#[test]
fn drag_commands_never_appear() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["task:01X".into(), "column:todo".into(), "board:b".into()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
    assert!(!ids.contains(&"drag.start"));
    assert!(!ids.contains(&"drag.cancel"));
    assert!(!ids.contains(&"drag.complete"));
}

// =========================================================================
// Targets
// =========================================================================

#[test]
fn global_commands_have_no_target() {
    let (registry, impls, fields, ui) = setup();
    ui.set_undo_redo_state(true, true);
    let scope = vec!["task:01X".into(), "column:todo".into()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );

    let undo = cmds.iter().find(|c| c.id == "app.undo").unwrap();
    assert!(undo.target.is_none());

    let quit = cmds.iter().find(|c| c.id == "app.quit").unwrap();
    assert!(quit.target.is_none());
}
