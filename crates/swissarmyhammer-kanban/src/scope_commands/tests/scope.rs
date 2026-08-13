//! The command set that each moniker in the scope chain gives.
//!
//! These tests hold `commands_for_scope` to the rows that a board, a task, a
//! tag on a task and an empty scope must give. They also hold the name
//! resolution, the context-menu filter, and the match between the clipboard
//! type and the scope type.

use super::*;

// =========================================================================
// Board scope
// =========================================================================

#[test]
fn board_scope_has_global_commands() {
    let (registry, impls, fields, ui) = setup();
    ui.set_undo_redo_state(true, true);
    let scope = vec!["board:my-board".into()];
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
    assert!(ids.contains(&"app.undo"), "board scope should have undo");
    assert!(ids.contains(&"app.redo"), "board scope should have redo");
    // entity.copy / entity.cut are now target-driven cross-cutting
    // commands and auto-emit on every entity moniker in scope, including
    // boards — copying a board to the clipboard is a meaningful op.
}

#[test]
fn board_scope_no_paste_without_clipboard() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["board:my-board".into()];
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
    assert!(!ids.contains(&"entity.paste"), "no paste without clipboard");
}

/// With a task on the clipboard and a board in scope, `entity.paste` must
/// surface as an available command — `PasteEntityCmd::available()` returns
/// true because task-on-clipboard + board-in-scope is a valid paste target
/// (paste creates a task in the board's first column).
///
/// This test pins the behavior that drives "right-click on a board
/// background shows Paste" without `board.yaml` opting into
/// `entity.paste` directly: the command must come from the registry's
/// global emission pass alone, gated by `PasteEntityCmd::available()`
/// against the target moniker and clipboard state.
#[test]
fn entity_paste_surfaces_on_board_when_task_clipboard() {
    let (registry, impls, fields, ui) = setup();
    ui.set_clipboard_entity_type("task");
    let scope = vec!["board:main".into()];
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

    let paste = cmds
        .iter()
        .find(|c| c.id == "entity.paste")
        .unwrap_or_else(|| {
            panic!(
                "entity.paste must surface on board scope when a task is on \
                 the clipboard; got commands: {:?}",
                cmds.iter().map(|c| &c.id).collect::<Vec<_>>()
            )
        });
    // `commands_for_scope` filters out unavailable commands at the end of
    // its pipeline, so a `find` hit already implies `available: true`.
    // The explicit assertion documents the contract for future readers.
    assert!(
        paste.available,
        "entity.paste must be available (task clipboard + board in scope is a \
         valid paste target)"
    );
}

// =========================================================================
// Task scope
// =========================================================================

#[test]
fn task_scope_has_copy_cut_inspect_archive() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "task:01X".into(),
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
    let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();

    assert!(
        names.contains(&"Copy Task"),
        "should have Copy Task: {:?}",
        names
    );
    assert!(
        names.contains(&"Cut Task"),
        "should have Cut Task: {:?}",
        names
    );
    assert!(
        names.contains(&"Inspect Task"),
        "should have Inspect Task: {:?}",
        names
    );
    assert!(
        names.contains(&"Archive Task"),
        "should have Archive Task: {:?}",
        names
    );
}

/// Regression guard for https://… — right-clicking a task used to render
/// two identical "Delete Task" entries in the context menu: one from the
/// cross-cutting `entity.delete` (template-resolved to "Delete Task") and
/// one from the retired type-specific `task.delete` (hardcoded name).
///
/// The fix removes `task.delete` entirely and migrates its only unique
/// affordance (the `Mod+Backspace` keybinding) onto `entity.delete`.
/// This test pins the surface contract: exactly one context-menu command
/// whose display name is "Delete Task", and its id is `entity.delete`.
#[test]
fn task_context_menu_has_exactly_one_delete_task() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["task:01X".into(), "column:todo".into()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        true,
        None,
        None,
    );

    let deletes: Vec<&ResolvedCommand> = cmds.iter().filter(|c| c.name == "Delete Task").collect();

    assert_eq!(
        deletes.len(),
        1,
        "expected exactly one 'Delete Task' in the task context menu, got {}: {:?}",
        deletes.len(),
        deletes.iter().map(|c| &c.id).collect::<Vec<_>>()
    );
    assert_eq!(
        deletes[0].id, "entity.delete",
        "the surviving 'Delete Task' must be the cross-cutting `entity.delete`, \
         not a type-specific `task.delete`"
    );
}

// =========================================================================
// Tag on task scope
// =========================================================================

#[test]
fn tag_on_task_has_only_tag_copy_cut_inspect() {
    // With dedup-by-id (innermost wins), right-clicking a tag pill shows
    // only the tag-level commands for shared IDs like entity.copy, entity.cut,
    // entity.inspect. The task-level versions are suppressed.
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "tag:bug".into(),
        "task:01X".into(),
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
    let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();

    // Innermost (tag) versions are present
    assert!(
        names.contains(&"Copy Tag"),
        "should have Copy Tag: {:?}",
        names
    );
    assert!(
        names.contains(&"Cut Tag"),
        "should have Cut Tag: {:?}",
        names
    );
    assert!(
        names.contains(&"Inspect Tag"),
        "should have Inspect Tag: {:?}",
        names
    );

    // Outer (task) versions are suppressed by dedup-by-id
    assert!(
        !names.contains(&"Copy Task"),
        "should NOT have Copy Task (deduped by id, tag wins): {:?}",
        names
    );
    assert!(
        !names.contains(&"Cut Task"),
        "should NOT have Cut Task (deduped by id, tag wins): {:?}",
        names
    );
    assert!(
        !names.contains(&"Inspect Task"),
        "should NOT have Inspect Task (deduped by id, tag wins): {:?}",
        names
    );
}

#[test]
fn tag_on_task_no_paste_without_clipboard() {
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
    let paste_cmds: Vec<_> = cmds.iter().filter(|c| c.id == "entity.paste").collect();
    assert!(paste_cmds.is_empty(), "no paste without clipboard");
}

// =========================================================================
// Name resolution
// =========================================================================

#[test]
fn all_names_fully_resolved() {
    let (registry, impls, fields, ui) = setup();
    ui.set_clipboard_entity_type("task");
    let scope = vec![
        "tag:bug".into(),
        "task:01X".into(),
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
    for cmd in &cmds {
        assert!(
            !cmd.name.contains("{{"),
            "command '{}' has unresolved template: '{}'",
            cmd.id,
            cmd.name
        );
    }
}

// =========================================================================
// Context menu filter
// =========================================================================

#[test]
fn context_menu_only_filters() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "task:01X".into(),
        "column:todo".into(),
        "board:my-board".into(),
    ];
    let all = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );
    let ctx_only = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        true,
        None,
        None,
    );

    assert!(
        ctx_only.len() < all.len(),
        "context menu should have fewer commands"
    );
    for cmd in &ctx_only {
        assert!(cmd.context_menu, "'{}' should be context_menu", cmd.id);
    }
}

// =========================================================================
// Empty scope
// =========================================================================

#[test]
fn empty_scope_has_only_global_commands() {
    let (registry, impls, fields, ui) = setup();
    ui.set_undo_redo_state(true, true);
    let cmds = commands_for_scope(
        &[],
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );

    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
    assert!(ids.contains(&"app.undo"));
    assert!(!ids.contains(&"entity.copy"));
    for cmd in &cmds {
        assert!(cmd.target.is_none(), "'{}' should have no target", cmd.id);
    }
}

// =========================================================================
// Paste cross-matching: clipboard type vs scope type
// =========================================================================

#[test]
fn task_clipboard_task_focused_no_paste() {
    // Task on clipboard + task focused (no column) → can't paste task here
    let (registry, impls, fields, ui) = setup();
    ui.set_clipboard_entity_type("task");
    let scope = vec!["task:01X".into()];
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
    let paste: Vec<_> = cmds.iter().filter(|c| c.id == "entity.paste").collect();
    assert!(paste.is_empty(), "can't paste task without column in scope");
}

#[test]
fn tag_clipboard_column_focused_no_paste() {
    // Tag on clipboard + column focused (no task) → can't paste tag here
    let (registry, impls, fields, ui) = setup();
    ui.set_clipboard_entity_type("tag");
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
    let paste: Vec<_> = cmds.iter().filter(|c| c.id == "entity.paste").collect();
    assert!(paste.is_empty(), "can't paste tag without task in scope");
}
