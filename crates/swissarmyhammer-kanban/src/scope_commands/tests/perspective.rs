//! The filter, group and sort commands of a perspective.
//!
//! These tests hold the commands that a perspective in scope adds. They also
//! hold the view kinds that keep a sort command out of a board view.

use super::*;

// =========================================================================
// Perspective scope
// =========================================================================

#[test]
fn perspective_scope_has_filter_and_group_commands() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["perspective:01ABC".into(), "board:my-board".into()];
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
        ids.contains(&"perspective.filter"),
        "perspective scope should have perspective.filter: {:?}",
        ids
    );
    assert!(
        ids.contains(&"perspective.group"),
        "perspective scope should have perspective.group: {:?}",
        ids
    );
}

#[test]
fn perspective_mutation_commands_available_when_perspective_in_scope() {
    // Perspective mutation commands (filter, group, sort) declare
    // `scope: "entity:perspective"` — they are filtered out of scopes
    // with no perspective moniker (e.g. right-click on a tag, actor,
    // or column). The frontend's `PerspectivesContainer` injects the
    // active perspective's moniker at the view-body level, so in
    // practice every palette/context-menu invocation from within a
    // view carries `perspective:<id>` in its chain and these commands
    // are available.
    //
    // The resolver still consults args → scope → UIState → first-for-
    // view-kind, so the command works whether or not the id was
    // supplied explicitly; this test only guards the static
    // registry-level scope filter.
    //
    // Sort commands are NOT asserted here — they additionally carry
    // `view_kinds: [grid]` and are therefore filtered out of scopes
    // whose innermost `view:{id}` does not resolve to a grid (or that
    // have no `view:{id}` at all, as in this test). The grid-scoped
    // counterpart lives in
    // `perspective_sort_commands_available_from_grid_palette_scope`.
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "perspective:01P".into(),
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
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

    for id in [
        "perspective.filter",
        "perspective.clearFilter",
        "perspective.group",
        "perspective.clearGroup",
    ] {
        assert!(
            ids.contains(&id),
            "{id} should be available when a perspective moniker is in scope: {ids:?}",
        );
    }
}

/// Symmetric guard: the same commands are filtered out when **no**
/// perspective moniker is in scope. This is the regression guard for
/// the bug the task fixes — right-clicking on a tag, actor, or column
/// must NOT surface perspective-mutation commands, because the scope
/// chain alone cannot identify which perspective to mutate.
#[test]
fn perspective_mutation_commands_hidden_without_perspective_in_scope() {
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
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

    for id in [
        "perspective.filter",
        "perspective.clearFilter",
        "perspective.group",
        "perspective.clearGroup",
        "perspective.sort.set",
        "perspective.sort.clear",
        "perspective.sort.toggle",
    ] {
        assert!(
            !ids.contains(&id),
            "{id} must NOT appear in scopes without a perspective moniker: {ids:?}",
        );
    }
}

/// Sort commands declare `view_kinds: [grid]` so a scope chain whose
/// innermost `view:{id}` resolves to a `kind: board` view (per
/// `DynamicSources.views`) must NOT surface them in palettes or context
/// menus. The board view organises by column grouping, not by sort
/// order, so "Sort Field" / "Clear Sort" / "Toggle Sort" would offer
/// behavior the user cannot see.
#[test]
fn sort_commands_absent_from_board_view_scope() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "view:board-view".into(),
        "perspective:01P".into(),
        "board:my-board".into(),
    ];
    let dynamic = DynamicSources {
        views: vec![ViewInfo {
            id: "board-view".into(),
            name: "Board View".into(),
            entity_type: Some("task".into()),
            kind: "board".into(),
        }],
        ..Default::default()
    };
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        Some(&dynamic),
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

    for id in [
        "perspective.sort.set",
        "perspective.sort.clear",
        "perspective.sort.toggle",
    ] {
        assert!(
            !ids.contains(&id),
            "{id} must NOT appear in a board-kind view scope (view_kinds: [grid]): {ids:?}",
        );
    }
}

/// Symmetric guard for `sort_commands_absent_from_board_view_scope`:
/// when the innermost `view:{id}` resolves to a `kind: grid` view, the
/// three sort commands must surface in the palette. This pins the "sort
/// is the primary ordering mechanism on grids" half of the
/// `view_kinds: [grid]` filter so a regression that always-filters them
/// fails this test, not just the negative one above.
#[test]
fn sort_commands_present_in_grid_view_scope() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "view:tasks-grid".into(),
        "perspective:01P".into(),
        "board:my-board".into(),
    ];
    let dynamic = DynamicSources {
        views: vec![ViewInfo {
            id: "tasks-grid".into(),
            name: "Tasks Grid".into(),
            entity_type: Some("task".into()),
            kind: "grid".into(),
        }],
        ..Default::default()
    };
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        Some(&dynamic),
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

    for id in [
        "perspective.sort.set",
        "perspective.sort.clear",
        "perspective.sort.toggle",
    ] {
        assert!(
            ids.contains(&id),
            "{id} must appear on a grid-kind view scope (view_kinds: [grid]): {ids:?}",
        );
    }
}

/// Filter and group commands have no `view_kinds` filter — they remain
/// available on every view kind, including the board view. Regression
/// guard against over-filtering: a sloppy implementation that suppresses
/// every `perspective.*` command on board views would pass
/// `sort_commands_absent_from_board_view_scope` while breaking real
/// users who set filters from the board's tab bar.
#[test]
fn view_kind_filter_leaves_filter_and_group_commands_alone() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "view:board-view".into(),
        "perspective:01P".into(),
        "board:my-board".into(),
    ];
    let dynamic = DynamicSources {
        views: vec![ViewInfo {
            id: "board-view".into(),
            name: "Board View".into(),
            entity_type: Some("task".into()),
            kind: "board".into(),
        }],
        ..Default::default()
    };
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        Some(&dynamic),
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

    for id in [
        "perspective.filter",
        "perspective.clearFilter",
        "perspective.group",
        "perspective.clearGroup",
    ] {
        assert!(
            ids.contains(&id),
            "{id} must still appear on a board view scope — the view_kinds \
             filter only scopes the three sort commands, not filter/group: {ids:?}",
        );
    }
}

/// Grid-scoped counterpart to
/// `perspective_mutation_commands_available_when_perspective_in_scope`.
/// Asserts that on a scope whose innermost `view:{id}` resolves to a
/// `kind: grid` view, every perspective-mutation command (filter,
/// group, AND sort) is available in the palette.
#[test]
fn perspective_sort_commands_available_from_grid_palette_scope() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec![
        "view:tasks-grid".into(),
        "perspective:01P".into(),
        "board:my-board".into(),
    ];
    let dynamic = DynamicSources {
        views: vec![ViewInfo {
            id: "tasks-grid".into(),
            name: "Tasks Grid".into(),
            entity_type: Some("task".into()),
            kind: "grid".into(),
        }],
        ..Default::default()
    };
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        Some(&dynamic),
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

    for id in [
        "perspective.filter",
        "perspective.clearFilter",
        "perspective.group",
        "perspective.clearGroup",
        "perspective.sort.set",
        "perspective.sort.clear",
        "perspective.sort.toggle",
    ] {
        assert!(
            ids.contains(&id),
            "{id} must appear on a grid-view palette scope with a perspective \
             moniker — this is the grid-scoped counterpart to the board-only \
             test that drops sort: {ids:?}",
        );
    }
}

/// Pin the documented "no view in scope -> hard no-match" branch of the
/// view-kind filter. The other three view-kind tests all build scope
/// chains containing a `view:{id}` moniker, which only exercises the
/// "view found and kind compared" path. This test deliberately omits any
/// `view:` moniker — the scope is just `["perspective:01P"]` — and
/// supplies an empty `DynamicSources.views` list, so the filter cannot
/// resolve any active view kind.
///
/// The contract in the task spec is explicit: with no `view:{id}` in
/// scope, commands carrying a `view_kinds` allow-list MUST be dropped.
/// That branch is the safe default for tests / shell-only invocations
/// where the UI surface is not bound to a particular view. Without this
/// test, a regression that flipped `None => false` to `None => true`
/// would silently re-enable sort commands in palette-only contexts and
/// every existing test would still pass (they all have a view in scope).
#[test]
fn view_kinds_constrained_commands_dropped_when_no_view_in_scope() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["perspective:01P".into()];
    let dynamic = DynamicSources {
        views: vec![],
        ..Default::default()
    };
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        Some(&dynamic),
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

    for id in [
        "perspective.sort.set",
        "perspective.sort.clear",
        "perspective.sort.toggle",
    ] {
        assert!(
            !ids.contains(&id),
            "{id} declares view_kinds: [grid] and the scope chain has no \
             view:{{id}} moniker — the safe-default branch must drop it: {ids:?}",
        );
    }
}
