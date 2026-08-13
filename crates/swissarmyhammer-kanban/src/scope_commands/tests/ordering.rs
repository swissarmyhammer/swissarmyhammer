//! The order and the uniqueness of the merged list.
//!
//! These tests hold the dedupe by command id, the innermost-scope-first order,
//! the order and the groups of the context menu, and the dedupe that the menu
//! bar does.

use super::*;

// =========================================================================
// Dedup-by-id: innermost scope wins for shared command IDs
// =========================================================================

#[test]
fn dedup_by_id_tag_task_scope_only_one_cut_command() {
    // entity.cut appears in both tag and task schemas.
    // With scope ["tag:X", "task:Y"], only the innermost (tag) cut should appear.
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "tag:some-tag".into(),
        "task:01TASK".into(),
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

    let cut_cmds: Vec<_> = cmds.iter().filter(|c| c.id == "entity.cut").collect();
    assert_eq!(
        cut_cmds.len(),
        1,
        "entity.cut should appear exactly once (innermost wins): {:?}",
        cut_cmds.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    assert_eq!(
        cut_cmds[0].name, "Cut Tag",
        "the single cut command should be 'Cut Tag' (tag is innermost): {:?}",
        cut_cmds[0]
    );
    assert_eq!(
        cut_cmds[0].target.as_deref(),
        Some("tag:some-tag"),
        "cut target should be the tag"
    );
}

#[test]
fn dedup_by_id_task_only_scope_shows_cut_task() {
    // When the scope has no tag, "Cut Task" should appear normally.
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "task:01TASK".into(),
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

    let cut_cmds: Vec<_> = cmds.iter().filter(|c| c.id == "entity.cut").collect();
    assert_eq!(
        cut_cmds.len(),
        1,
        "entity.cut should appear exactly once: {:?}",
        cut_cmds
    );
    assert_eq!(
        cut_cmds[0].name, "Cut Task",
        "only task in scope → should show 'Cut Task'"
    );
    assert_eq!(
        cut_cmds[0].target.as_deref(),
        Some("task:01TASK"),
        "cut target should be the task"
    );
}

#[test]
fn dedup_by_id_applies_to_copy_and_inspect_too() {
    // Verify that entity.copy and entity.inspect also follow dedup-by-id,
    // showing only the innermost (tag) version when both tag and task are in scope.
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "tag:some-tag".into(),
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

    // entity.copy — only tag version
    let copy_cmds: Vec<_> = cmds.iter().filter(|c| c.id == "entity.copy").collect();
    assert_eq!(
        copy_cmds.len(),
        1,
        "entity.copy should appear exactly once: {:?}",
        copy_cmds.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    assert_eq!(
        copy_cmds[0].name, "Copy Tag",
        "entity.copy should show 'Copy Tag'"
    );

    // ui.inspect — only tag version
    let inspect_cmds: Vec<_> = cmds.iter().filter(|c| c.id == "ui.inspect").collect();
    assert_eq!(
        inspect_cmds.len(),
        1,
        "ui.inspect should appear exactly once: {:?}",
        inspect_cmds.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    assert_eq!(
        inspect_cmds[0].name, "Inspect Tag",
        "ui.inspect should show 'Inspect Tag'"
    );
}

// =========================================================================
// Scope ordering — innermost scope commands first
// =========================================================================

#[test]
fn attachment_commands_appear_before_task_commands() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "attachment:/path/to/file.png".into(),
        "task:01X".into(),
        "column:todo".into(),
    ];
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
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

    // attachment.open and attachment.reveal should be present
    assert!(
        ids.contains(&"attachment.open"),
        "attachment.open should be in context menu"
    );
    assert!(
        ids.contains(&"attachment.reveal"),
        "attachment.reveal should be in context menu"
    );

    // The semantic claim is that attachment-group commands (anything
    // resolved against the inner `attachment:*` moniker) precede
    // task-group commands (resolved against the outer `task:*` moniker).
    // Match on the resolved `group` field — relying on id prefixes
    // breaks the moment a cross-cutting command (e.g. `ui.inspect`,
    // `entity.archive`) gets emitted with an attachment target.
    let open_pos = ids.iter().position(|&id| id == "attachment.open").unwrap();
    let reveal_pos = ids
        .iter()
        .position(|&id| id == "attachment.reveal")
        .unwrap();

    let first_task_pos = cmds.iter().position(|c| c.group == "task");

    if let Some(task_pos) = first_task_pos {
        assert!(
            open_pos < task_pos,
            "attachment.open (pos {open_pos}) should appear before first task-group command (pos {task_pos})"
        );
        assert!(
            reveal_pos < task_pos,
            "attachment.reveal (pos {reveal_pos}) should appear before first task-group command (pos {task_pos})"
        );
    }
}

#[test]
fn attachment_commands_grouped_as_attachment_not_global() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["attachment:/path/to/file.png".into(), "task:01X".into()];
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

    let open_cmd = cmds.iter().find(|c| c.id == "attachment.open");
    assert!(open_cmd.is_some(), "attachment.open should exist");
    assert_eq!(
        open_cmd.unwrap().group,
        "attachment",
        "attachment.open should have group 'attachment', not 'global'"
    );
}

/// Right-clicking an `attachment:<path>` chip must surface the four
/// cross-cutting commands (`entity.delete`, `entity.cut`, `entity.copy`,
/// `entity.paste`) alongside the attachment-specific `attachment.open`
/// and `attachment.reveal`. Without this the menu is missing Delete,
/// Cut, Copy, and Paste — see kanban task
/// 01KR70R8YRRB36H6FVZMQMWFT1.
///
/// Setup mirrors the real focus chain the frontend builds for an
/// attachment chip (`[attachment:..., task:..., column:..., board:...]`)
/// and seeds an attachment-shaped clipboard so `entity.paste` resolves
/// — paste availability gates on `clipboard_entity_type` matching a
/// registered `(clipboard_type, target_type)` PasteHandler.
#[test]
fn attachment_context_menu_includes_cross_cutting_commands() {
    let (registry, impls, fields, ui) = setup();
    // An attachment lives on a task. Seed a non-empty clipboard so the
    // paste availability guard fires — without `clipboard_entity_type`
    // set, `PasteEntityCmd::available()` returns false up-front.
    ui.set_clipboard_entity_type("attachment");

    let scope = vec![
        "attachment:/path/to/file.png".into(),
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
        true,
        None,
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

    for required in [
        "attachment.open",
        "attachment.reveal",
        "entity.delete",
        "entity.cut",
        "entity.copy",
        "entity.paste",
    ] {
        assert!(
            ids.contains(&required),
            "{required} must surface in the attachment context menu; got: {:?}",
            ids,
        );
    }

    // Each of the four cross-cutting commands must resolve with the
    // attachment moniker as its target — that's how the dispatch path
    // distinguishes "delete this attachment" from "delete the parent
    // task" when both monikers are in scope.
    for id in ["entity.delete", "entity.cut", "entity.copy", "entity.paste"] {
        let cmd = cmds
            .iter()
            .find(|c| c.id == id)
            .unwrap_or_else(|| panic!("{id} must be present"));
        assert_eq!(
            cmd.target.as_deref(),
            Some("attachment:/path/to/file.png"),
            "{id} target must be the attachment moniker (innermost scope), \
             not the parent task: got {:?}",
            cmd.target,
        );
        assert!(
            cmd.available,
            "{id} must be available on an attachment scope (clipboard \
             seeded with attachment, parent task in scope chain)",
        );
    }
}

#[test]
fn tag_commands_appear_before_task_commands_in_context_menu() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["tag:bug".into(), "task:01X".into(), "column:todo".into()];
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
    let groups: Vec<&str> = cmds.iter().map(|c| c.group.as_str()).collect();

    // First commands should be tag-grouped, then task-grouped
    let first_tag = groups.iter().position(|&g| g == "tag");
    let first_task = groups.iter().position(|&g| g == "task");

    if let (Some(tag_pos), Some(task_pos)) = (first_tag, first_task) {
        assert!(
            tag_pos < task_pos,
            "tag commands (pos {tag_pos}) should appear before task commands (pos {task_pos})"
        );
    }
}

// =========================================================================
// Context-menu ordering and grouping
// =========================================================================

/// Right-clicking a task must produce the cross-cutting commands in a
/// stable, grouped order with distinct `group` strings that trigger
/// separator insertion in the frontend renderer:
///
///   1. Cut / Copy / Paste    (group ctx1)
///   2. Delete / Archive      (group ctx2)
///   3. Inspect               (group ctx3)
///
/// The frontend renderer at `context-menu.ts` inserts a separator
/// whenever `cmd.group !== lastGroup`, so three distinct group strings
/// yield the two user-visible separators the design calls for.
#[test]
fn cross_cutting_context_menu_is_ordered_and_grouped() {
    let (registry, impls, fields, ui) = setup();
    // Put a task on the clipboard so `entity.paste` is available on a task
    // scope (PasteEntityCmd validates clipboard-vs-target compatibility).
    ui.set_clipboard_entity_type("tag");
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

    // Filter down to the cross-cutting entries we care about, in the order
    // `commands_for_scope` emitted them.
    let cross_cutting: Vec<&ResolvedCommand> = cmds
        .iter()
        .filter(|c| {
            matches!(
                c.id.as_str(),
                "entity.cut"
                    | "entity.copy"
                    | "entity.paste"
                    | "entity.delete"
                    | "entity.archive"
                    | "entity.unarchive"
                    | "ui.inspect"
            )
        })
        .collect();
    let ids: Vec<&str> = cross_cutting.iter().map(|c| c.id.as_str()).collect();

    // Expected order. `entity.unarchive` is unavailable on a todo-column
    // task (nothing to unarchive) so it's filtered out by `available`.
    let expected: &[&str] = &[
        "entity.cut",
        "entity.copy",
        "entity.paste",
        "entity.delete",
        "entity.archive",
        "ui.inspect",
    ];
    assert_eq!(
        ids, expected,
        "cross-cutting context-menu commands must appear in the documented \
         order (cut/copy/paste → delete/archive → inspect); got {:?}",
        ids
    );

    // Group strings must partition the list into three contiguous buckets
    // so the frontend separator logic (new group → separator) triggers at
    // the right spots. All buckets are per-entity-type-suffixed so
    // `dedupe_by_id` across monikers still sees them as the same command.
    let group_of = |id: &str| -> &str {
        cross_cutting
            .iter()
            .find(|c| c.id == id)
            .expect("command in filtered list")
            .group
            .as_str()
    };
    let g_cut = group_of("entity.cut");
    let g_copy = group_of("entity.copy");
    let g_paste = group_of("entity.paste");
    let g_delete = group_of("entity.delete");
    let g_archive = group_of("entity.archive");
    let g_inspect = group_of("ui.inspect");

    assert_eq!(
        g_cut, g_copy,
        "cut and copy must share a group to render contiguously"
    );
    assert_eq!(
        g_copy, g_paste,
        "copy and paste must share a group to render contiguously"
    );
    assert_eq!(
        g_delete, g_archive,
        "delete and archive must share a group to render contiguously"
    );
    assert_ne!(
        g_paste, g_delete,
        "cut/copy/paste group must differ from delete/archive group so a \
         separator appears between them"
    );
    assert_ne!(
        g_archive, g_inspect,
        "delete/archive group must differ from inspect group so a \
         separator appears between them"
    );
    assert!(
        g_cut.contains("ctx1"),
        "cut/copy/paste bucket should be tagged ctx1; got {:?}",
        g_cut
    );
    assert!(
        g_delete.contains("ctx2"),
        "delete/archive bucket should be tagged ctx2; got {:?}",
        g_delete
    );
    assert!(
        g_inspect.contains("ctx3"),
        "inspect bucket should be tagged ctx3; got {:?}",
        g_inspect
    );
}

/// Two back-to-back calls to `commands_for_scope` must return the
/// cross-cutting context-menu commands in the exact same order.
///
/// The registry is backed by a `HashMap<String, CommandDef>`, and Rust's
/// `DefaultHasher` reseeds per process — so iteration order is stable
/// within one process run but different across runs. Running twice in one
/// test guards the *intra-process* invariant, which is what matters for a
/// single UI session: the menu doesn't reshuffle when you right-click a
/// second time.
#[test]
fn cross_cutting_order_is_stable_across_runs() {
    let (registry, impls, fields, ui) = setup();
    ui.set_clipboard_entity_type("tag");
    let scope = vec!["task:01X".into(), "column:todo".into()];

    let extract = || -> Vec<String> {
        commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            true,
            None,
            None,
        )
        .into_iter()
        .filter(|c| {
            matches!(
                c.id.as_str(),
                "entity.cut"
                    | "entity.copy"
                    | "entity.paste"
                    | "entity.delete"
                    | "entity.archive"
                    | "entity.unarchive"
                    | "ui.inspect"
            )
        })
        .map(|c| c.id)
        .collect()
    };

    let first = extract();
    let second = extract();
    assert_eq!(
        first, second,
        "cross-cutting command order must be deterministic — HashMap \
         iteration order must not leak into the emission sequence"
    );
}

// =========================================================================
// Menu-bar dedupe
// =========================================================================

/// Build a synthetic `ResolvedCommand` carrying just the fields the
/// menu-bar dedupe helper inspects. Mirrors what
/// `emit_cross_cutting_commands` would produce for a given (id, target)
/// pair before the global `dedupe_by_id` pass collapses them.
fn make_resolved(id: &str, target: &str) -> ResolvedCommand {
    ResolvedCommand {
        id: id.into(),
        name: format!("Cmd {id} on {target}"),
        menu_name: None,
        target: Some(target.into()),
        group: target
            .split_once(':')
            .map(|(t, _)| t.to_string())
            .unwrap_or_default(),
        context_menu: true,
        keys: None,
        available: true,
        args: None,
        params: Vec::new(),
        tab_button: None,
    }
}

/// `dedupe_for_menu_bar` collapses per-target emissions of the same
/// cross-cutting command id (e.g. `entity.copy` once per moniker in a
/// `[tag, task, column]` scope) down to a single menu-bar row. The raw
/// per-target list is what `emit_cross_cutting_commands` produces before
/// `commands_for_scope`'s final `dedupe_by_id` pass — exactly what a
/// menu-bar caller would receive if it bypassed the inner dedupe to keep
/// per-target context-menu entries.
#[test]
fn menu_bar_dedupes_cross_cutting_commands() {
    // Simulate the cross-cutting pass output for a `[tag, task, column]`
    // scope: entity.copy emitted once per entity moniker, innermost first.
    let mut menu_bar = vec![
        make_resolved("entity.copy", "tag:01T"),
        make_resolved("entity.copy", "task:01X"),
        make_resolved("entity.copy", "column:todo"),
    ];

    // Pre-dedupe: the raw cross-cutting stream carries one entity.copy per
    // target — that's what a context-menu renderer wants. (This mirrors the
    // task acceptance criterion: "context-menu output contains it three
    // times, one per target".)
    let copies_before: Vec<&ResolvedCommand> =
        menu_bar.iter().filter(|c| c.id == "entity.copy").collect();
    assert_eq!(
        copies_before.len(),
        3,
        "raw cross-cutting stream should carry one entity.copy per moniker, \
         got: {:?}",
        copies_before.iter().map(|c| &c.target).collect::<Vec<_>>()
    );

    // Apply the menu-bar dedupe: collapse to a single row per id.
    dedupe_for_menu_bar(&mut menu_bar);

    let copies_after: Vec<&ResolvedCommand> =
        menu_bar.iter().filter(|c| c.id == "entity.copy").collect();
    assert_eq!(
        copies_after.len(),
        1,
        "menu-bar dedupe must leave entity.copy exactly once regardless of \
         how many entity monikers were in scope, got: {:?}",
        copies_after.iter().map(|c| &c.target).collect::<Vec<_>>()
    );
}

/// The menu-bar dedupe must keep the **innermost** target so that picking
/// Edit → Cut from the macOS menu bar dispatches to the most-specific
/// entity in the current scope (matching what the user would right-click
/// on). `commands_for_scope` emits monikers innermost-first, so retaining
/// the first occurrence per id satisfies this contract.
#[test]
fn menu_bar_entry_targets_innermost() {
    // Same `[tag, task, column]` scope, this time also varying the command
    // id so the assertion narrows on the innermost target for entity.copy
    // without picking up unrelated entries.
    let mut menu_bar = vec![
        make_resolved("entity.copy", "tag:01T"),
        make_resolved("entity.copy", "task:01X"),
        make_resolved("entity.copy", "column:todo"),
        make_resolved("entity.cut", "tag:01T"),
        make_resolved("entity.cut", "task:01X"),
        make_resolved("entity.cut", "column:todo"),
    ];

    dedupe_for_menu_bar(&mut menu_bar);

    let copy = menu_bar
        .iter()
        .find(|c| c.id == "entity.copy")
        .expect("entity.copy must survive menu-bar dedupe");
    assert_eq!(
        copy.target.as_deref(),
        Some("tag:01T"),
        "menu-bar entry for entity.copy must dispatch to the innermost \
         target (tag), got: {:?}",
        copy.target
    );

    let cut = menu_bar
        .iter()
        .find(|c| c.id == "entity.cut")
        .expect("entity.cut must survive menu-bar dedupe");
    assert_eq!(
        cut.target.as_deref(),
        Some("tag:01T"),
        "menu-bar entry for entity.cut must dispatch to the innermost \
         target (tag), got: {:?}",
        cut.target
    );
}

/// A list that already has at most one entry per id is a no-op for the
/// menu-bar dedupe — the helper must not reorder or drop entries that
/// don't share an id. This guards against accidentally narrowing the
/// dedupe key beyond `id` (e.g. keying on `(id, target)` would still
/// retain everything but break the cross-cutting dedupe contract above).
#[test]
fn menu_bar_dedupe_is_noop_on_already_unique_list() {
    let mut menu_bar = vec![
        make_resolved("entity.copy", "tag:01T"),
        make_resolved("entity.cut", "tag:01T"),
        make_resolved("entity.paste", "column:todo"),
        make_resolved("ui.inspect", "task:01X"),
    ];
    let before = menu_bar.clone();

    dedupe_for_menu_bar(&mut menu_bar);

    assert_eq!(
        menu_bar.len(),
        before.len(),
        "dedupe_for_menu_bar must be a no-op on a list with no duplicate ids"
    );
    for (after, before) in menu_bar.iter().zip(before.iter()) {
        assert_eq!(after.id, before.id, "order must be preserved");
        assert_eq!(after.target, before.target, "target must be preserved");
    }
}
