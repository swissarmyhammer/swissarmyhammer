//! The entity schema as a command source.
//!
//! These tests hold the commands that come from an entity schema. They also
//! show that the scope walk skips a field moniker.

use super::*;

// =========================================================================
// Field monikers are skipped in scope chain
// =========================================================================

#[test]
fn field_moniker_skipped_inspect_targets_entity() {
    // With the `field:` prefix, grid cell monikers like
    // "field:tag:tag-1.color" are skipped entirely. The inspect command
    // targets the real entity moniker "tag:tag-1".
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "field:tag:tag-1.color".into(),
        "tag:tag-1".into(),
        "board:board".into(),
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

    let inspect = cmds.iter().find(|c| c.id == "ui.inspect");
    assert!(inspect.is_some(), "should have inspect command");
    assert_eq!(
        inspect.unwrap().target.as_deref(),
        Some("tag:tag-1"),
        "inspect target should be the entity moniker, not the field moniker"
    );
}

#[test]
fn field_moniker_dedup_emits_one_inspect() {
    // "field:tag:tag-1.color" is skipped, so only "tag:tag-1" produces
    // commands — exactly one inspect command.
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "field:tag:tag-1.color".into(),
        "tag:tag-1".into(),
        "board:board".into(),
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

    let inspect_cmds: Vec<_> = cmds.iter().filter(|c| c.id == "ui.inspect").collect();
    assert_eq!(
        inspect_cmds.len(),
        1,
        "should have exactly one inspect command, got {}: {:?}",
        inspect_cmds.len(),
        inspect_cmds.iter().map(|c| &c.target).collect::<Vec<_>>()
    );
}

// =========================================================================
// Entity schema as primary source for scoped commands
// =========================================================================

/// Regression guard: after the unified-creation refactor, no entity
/// schema should declare `task.add`. If one slips back in, this test
/// fails because the resolved commands contain the legacy id.
#[test]
fn task_add_not_emitted_from_entity_schema() {
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
    assert!(
        cmds.iter().all(|c| c.id != "task.add"),
        "task.add must not be emitted by any path; entity schema duplicates are banned. \
         got: {:?}",
        cmds.iter().map(|c| &c.id).collect::<Vec<_>>()
    );
}

/// `entity.archive` is a cross-cutting command and must surface on any
/// non-task entity scope. With the registry scope pin (`scope: "entity:task"`)
/// stripped from `entity.yaml`, archive should appear with `available: true`
/// when a tag moniker is in scope — proving the cross-cutting contract holds
/// independent of any per-entity schema duplication.
///
/// The cross-cutting pass supplies the resolved command with
/// `target: Some("tag:01X")` — locking in that cross-cutting commands
/// reach every entity moniker without needing a per-entity YAML opt-in.
#[test]
fn entity_archive_surfaces_on_non_task_entity() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["tag:01X".into()];
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

    let archive = cmds
        .iter()
        .find(|c| c.id == "entity.archive" && c.available)
        .unwrap_or_else(|| {
            panic!(
                "entity.archive should surface as available on a tag scope; \
                 got: {:?}",
                cmds.iter()
                    .map(|c| (&c.id, c.available))
                    .collect::<Vec<_>>()
            )
        });
    assert!(
        archive.available,
        "entity.archive must be available on tag scope"
    );
}

/// `entity.delete` is a cross-cutting command — it auto-emits per entity
/// moniker via `emit_cross_cutting_commands`. With `project.delete`
/// stripped from `project.yaml`, the project's right-click menu still gets
/// a Delete entry through the registry-driven auto-emit path.
///
/// This locks in the contract that purging the per-entity opt-in does not
/// regress the user-facing Delete affordance — the cross-cutting pass
/// supplies an `entity.delete` resolved command with `target ==
/// "project:backend"` and `available: true` for any project moniker in
/// scope.
#[test]
fn entity_delete_surfaces_on_project_via_autoemit() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["project:backend".into()];
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

    let delete = cmds
        .iter()
        .find(|c| c.id == "entity.delete")
        .unwrap_or_else(|| {
            panic!(
                "entity.delete must auto-emit on project scope; got: {:?}",
                cmds.iter()
                    .map(|c| (&c.id, &c.target, c.available))
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(
        delete.target.as_deref(),
        Some("project:backend"),
        "entity.delete target must equal the project moniker, got: {:?}",
        delete.target
    );
    assert!(
        delete.context_menu,
        "entity.delete must opt into the context menu on a project scope"
    );
    assert!(
        delete.available,
        "entity.delete must be available on a project scope"
    );
}

// =========================================================================
// Field monikers are skipped
// =========================================================================

#[test]
fn field_moniker_in_scope_does_not_produce_entity_commands() {
    // A scope chain with "field:task:abc.title" should not generate commands
    // for a phantom entity "abc.title" — the field moniker is skipped entirely.
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "field:task:abc.title".into(),
        "task:abc".into(),
        "column:todo".into(),
        "board:my-board".into(),
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

    // No command should target the field moniker
    for cmd in &cmds {
        if let Some(target) = &cmd.target {
            assert!(
                !target.starts_with("field:"),
                "command '{}' should not target a field moniker, got: {}",
                cmd.id,
                target
            );
        }
    }

    // Task commands should still be present (from the real task:abc moniker)
    let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
    assert!(
        names.contains(&"Inspect Task"),
        "real task commands should still appear: {:?}",
        names
    );
}
