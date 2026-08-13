//! The rows that come from `DynamicSources`.
//!
//! These tests hold the view-switch, board-switch, perspective-goto and
//! window-focus rows. Each row is a canonical command whose arguments are
//! already filled in.

use super::*;

// =========================================================================
// Dynamic view switch commands
// =========================================================================

/// The palette surfaces one `view.set` row per known view, each carrying
/// the matching `view_id` pre-filled in its `args`. The dispatcher takes
/// these rows verbatim — no suffix rewriting — so the wire format must
/// match the canonical `view.set` command's param contract.
#[test]
fn view_switch_commands_emit_canonical_view_set_with_args() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["board:my-board".into()];
    let dynamic = DynamicSources {
        views: vec![
            ViewInfo {
                id: "board-view".into(),
                name: "Board View".into(),
                entity_type: None,
                kind: "board".into(),
            },
            ViewInfo {
                id: "tasks-grid".into(),
                name: "Task Grid".into(),
                entity_type: None,
                kind: "grid".into(),
            },
            ViewInfo {
                id: "tags-grid".into(),
                name: "Tag Grid".into(),
                entity_type: None,
                kind: "grid".into(),
            },
        ],
        boards: vec![],
        windows: vec![],
        perspectives: vec![],
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

    // Legacy `view.switch:{id}` ids must NOT appear — the indirection is gone.
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
    assert!(
        !ids.iter().any(|id| id.starts_with("view.switch:")),
        "legacy view.switch:* ids must not be emitted: {:?}",
        ids
    );

    // One `view.set` row per view, distinguished by args.view_id.
    let view_set_rows: Vec<&ResolvedCommand> = cmds.iter().filter(|c| c.id == "view.set").collect();
    let args_view_ids: Vec<&str> = view_set_rows
        .iter()
        .map(|c| {
            c.args
                .as_ref()
                .and_then(|v| v.get("view_id"))
                .and_then(|v| v.as_str())
                .expect("every view.set palette row must carry args.view_id")
        })
        .collect();
    assert!(
        args_view_ids.contains(&"board-view"),
        "should have view.set row for board-view: {:?}",
        args_view_ids
    );
    assert!(
        args_view_ids.contains(&"tasks-grid"),
        "should have view.set row for tasks-grid: {:?}",
        args_view_ids
    );
    assert!(
        args_view_ids.contains(&"tags-grid"),
        "should have view.set row for tags-grid: {:?}",
        args_view_ids
    );
}

/// Per-view `view.set` rows keep the same display name and group the
/// legacy `view.switch:*` entries carried, so palette rendering (which
/// keys on `name`/`group`) is unchanged.
#[test]
fn view_switch_commands_have_correct_names() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["board:my-board".into()];
    let dynamic = DynamicSources {
        views: vec![
            ViewInfo {
                id: "board-view".into(),
                name: "Board View".into(),
                entity_type: None,
                kind: "board".into(),
            },
            ViewInfo {
                id: "tasks-grid".into(),
                name: "Task Grid".into(),
                entity_type: None,
                kind: "grid".into(),
            },
        ],
        boards: vec![],
        windows: vec![],
        perspectives: vec![],
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

    let board_switch = cmds
        .iter()
        .find(|c| {
            c.id == "view.set"
                && c.args.as_ref().and_then(|v| v.get("view_id"))
                    == Some(&serde_json::Value::String("board-view".into()))
        })
        .expect("view.set row for board-view must exist");
    assert_eq!(board_switch.name, "Switch to Board View");
    assert_eq!(board_switch.group, "view");
    assert!(board_switch.target.is_none());

    let grid_switch = cmds
        .iter()
        .find(|c| {
            c.id == "view.set"
                && c.args.as_ref().and_then(|v| v.get("view_id"))
                    == Some(&serde_json::Value::String("tasks-grid".into()))
        })
        .expect("view.set row for tasks-grid must exist");
    assert_eq!(grid_switch.name, "Switch to Task Grid");
}

/// Right-click on a view button must NOT surface any "Switch to X"
/// commands — view switching is a palette-only action. This holds
/// regardless of which `view:*` moniker is in the scope chain.
///
/// After 01KPZMXXEXKVE3RNPA4XJP0105 the dynamic rows emit `view.set`
/// directly with args instead of the legacy `view.switch:{id}` id, so
/// the guard checks both: the legacy prefix must stay absent, and the
/// "switch" group must not contribute any context-menu rows.
#[test]
fn view_switch_context_menu_only_emits_in_scope_view() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["view:board-view".into()];
    let dynamic = DynamicSources {
        views: vec![
            ViewInfo {
                id: "board-view".into(),
                name: "Board View".into(),
                entity_type: None,
                kind: "board".into(),
            },
            ViewInfo {
                id: "tasks-grid".into(),
                name: "Task Grid".into(),
                entity_type: None,
                kind: "grid".into(),
            },
            ViewInfo {
                id: "tags-grid".into(),
                name: "Tag Grid".into(),
                entity_type: None,
                kind: "grid".into(),
            },
        ],
        boards: vec![],
        windows: vec![],
        perspectives: vec![],
    };
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        true, // context_menu_only
        Some(&dynamic),
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

    assert!(
        !ids.iter().any(|id| id.starts_with("view.switch:")),
        "legacy view.switch:* prefix must never appear: {:?}",
        ids
    );
    assert!(
        !cmds.iter().any(|c| c.group == "view"),
        "\"Switch to X\" rows (group == \"view\") must NOT appear in \
         right-click menu regardless of scope: {:?}",
        cmds.iter().map(|c| (&c.id, &c.group)).collect::<Vec<_>>()
    );
}

/// Palette behavior (`context_menu_only == false`) must be unchanged:
/// a "Switch to X" row still appears for every known view regardless of
/// which view moniker is in the scope chain. Guards against a regression
/// where the per-view scope filter accidentally suppresses palette
/// entries. Each row now emits as a canonical `view.set` command with
/// its `view_id` pre-filled in `args`.
#[test]
fn view_switch_palette_still_emits_all_views() {
    let (registry, impls, fields, ui) = setup();
    // No view:* in scope — palette shouldn't care either way.
    let scope: Vec<String> = vec![];
    let dynamic = DynamicSources {
        views: vec![
            ViewInfo {
                id: "board-view".into(),
                name: "Board View".into(),
                entity_type: None,
                kind: "board".into(),
            },
            ViewInfo {
                id: "tasks-grid".into(),
                name: "Task Grid".into(),
                entity_type: None,
                kind: "grid".into(),
            },
            ViewInfo {
                id: "tags-grid".into(),
                name: "Tag Grid".into(),
                entity_type: None,
                kind: "grid".into(),
            },
        ],
        boards: vec![],
        windows: vec![],
        perspectives: vec![],
    };
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false, // context_menu_only == false → palette
        Some(&dynamic),
        None,
    );

    let view_set_args: Vec<&str> = cmds
        .iter()
        .filter(|c| c.id == "view.set")
        .filter_map(|c| {
            c.args
                .as_ref()
                .and_then(|v| v.get("view_id"))
                .and_then(|v| v.as_str())
        })
        .collect();

    assert!(
        view_set_args.contains(&"board-view"),
        "palette must show view.set for every view: {:?}",
        view_set_args
    );
    assert!(
        view_set_args.contains(&"tasks-grid"),
        "palette must show view.set for every view: {:?}",
        view_set_args
    );
    assert!(
        view_set_args.contains(&"tags-grid"),
        "palette must show view.set for every view: {:?}",
        view_set_args
    );
}

// =========================================================================
// Dynamic board switch commands
// =========================================================================

#[test]
fn board_switch_commands_appear_when_boards_provided() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["board:my-board".into()];
    let dynamic = DynamicSources {
        views: vec![],
        boards: vec![
            BoardInfo {
                path: "/home/user/project-a".into(),
                name: "Project A".into(),
                entity_name: "Project A".into(),
                context_name: "project-a".into(),
            },
            BoardInfo {
                path: "/home/user/project-b".into(),
                name: "Project B".into(),
                entity_name: "Project B".into(),
                context_name: "project-b".into(),
            },
        ],
        windows: vec![],
        perspectives: vec![],
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

    assert!(
        ids.contains(&"board.switch:/home/user/project-a"),
        "should have project-a switch: {:?}",
        ids
    );
    assert!(
        ids.contains(&"board.switch:/home/user/project-b"),
        "should have project-b switch: {:?}",
        ids
    );
}

#[test]
fn board_switch_commands_have_correct_names_and_ids() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["board:my-board".into()];
    let dynamic = DynamicSources {
        views: vec![],
        boards: vec![BoardInfo {
            path: "/tmp/my-kanban".into(),
            name: "My Kanban".into(),
            entity_name: "My Kanban".into(),
            context_name: "my-kanban".into(),
        }],
        windows: vec![],
        perspectives: vec![],
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

    let board_cmd = cmds
        .iter()
        .find(|c| c.id == "board.switch:/tmp/my-kanban")
        .unwrap();
    assert_eq!(board_cmd.name, "Switch to Board: My Kanban (my-kanban)");
    assert_eq!(board_cmd.menu_name.as_deref(), Some("my-kanban"));
    assert_eq!(board_cmd.group, "board");
    assert!(board_cmd.target.is_none());
    assert!(!board_cmd.context_menu);
}

#[test]
fn view_and_board_commands_not_in_context_menu() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["board:my-board".into()];
    let dynamic = DynamicSources {
        views: vec![ViewInfo {
            id: "board-view".into(),
            name: "Board View".into(),
            entity_type: None,
            kind: "board".into(),
        }],
        boards: vec![BoardInfo {
            path: "/tmp/board".into(),
            name: "Board".into(),
            entity_name: "Board".into(),
            context_name: "board".into(),
        }],
        windows: vec![],
        perspectives: vec![],
    };
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        true,
        Some(&dynamic),
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

    // Dynamic commands have context_menu: false, so they should be filtered out
    assert!(
        !ids.iter().any(|id| id.starts_with("view.switch:")),
        "view commands should not appear in context menu"
    );
    assert!(
        !ids.iter().any(|id| id.starts_with("board.switch:")),
        "board commands should not appear in context menu"
    );
}

#[test]
fn no_dynamic_commands_when_none_provided() {
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

    assert!(
        !ids.iter().any(|id| id.starts_with("view.switch:")),
        "no view commands without dynamic sources"
    );
    assert!(
        !ids.iter().any(|id| id.starts_with("board.switch:")),
        "no board commands without dynamic sources"
    );
}

// =========================================================================
// Dynamic perspective goto commands
// =========================================================================

/// The palette surfaces one `perspective.switch` row per known perspective,
/// each carrying the matching `perspective_id` pre-filled in its `args`.
/// The dispatcher takes these rows verbatim — no suffix rewriting — so
/// the wire format must match the canonical `perspective.switch` command's
/// param contract.
#[test]
fn perspective_goto_commands_emit_canonical_perspective_switch_with_args() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["board:my-board".into()];
    let dynamic = DynamicSources {
        perspectives: vec![
            PerspectiveInfo {
                id: "p1".into(),
                name: "Alpha".into(),
                view: "board".into(),
                fields: vec![],
            },
            PerspectiveInfo {
                id: "p2".into(),
                name: "Beta".into(),
                view: "board".into(),
                fields: vec![],
            },
        ],
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

    // Legacy `perspective.goto:{id}` ids must NOT appear — the
    // indirection is gone.
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
    assert!(
        !ids.iter().any(|id| id.starts_with("perspective.goto:")),
        "legacy perspective.goto:* ids must not be emitted: {:?}",
        ids
    );

    // One `perspective.switch` row per perspective, distinguished by
    // args.perspective_id.
    let args_ids: Vec<&str> = cmds
        .iter()
        .filter(|c| c.id == "perspective.switch")
        .filter_map(|c| {
            c.args
                .as_ref()
                .and_then(|v| v.get("perspective_id"))
                .and_then(|v| v.as_str())
        })
        .collect();
    assert!(
        args_ids.contains(&"p1"),
        "should have perspective.switch row for p1: {:?}",
        args_ids
    );
    assert!(
        args_ids.contains(&"p2"),
        "should have perspective.switch row for p2: {:?}",
        args_ids
    );

    let p1 = cmds
        .iter()
        .find(|c| {
            c.id == "perspective.switch"
                && c.args.as_ref().and_then(|v| v.get("perspective_id"))
                    == Some(&serde_json::Value::String("p1".into()))
        })
        .expect("perspective.switch row for p1 must exist");
    assert_eq!(p1.name, "Go to Perspective: Alpha");
    assert_eq!(p1.group, "perspective");
}

/// Right-click must not surface any perspective-navigation commands —
/// "Go to Perspective: X" is a palette-only action. After
/// 01KPZMXXEXKVE3RNPA4XJP0105 the rows emit as `perspective.switch` with
/// args, so the guard checks both the legacy `perspective.goto:*` prefix
/// (must not reappear) and the "perspective" group (must not leak into
/// the context menu).
#[test]
fn perspective_goto_commands_not_in_context_menu() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["board:my-board".into()];
    let dynamic = DynamicSources {
        perspectives: vec![PerspectiveInfo {
            id: "p1".into(),
            name: "Alpha".into(),
            view: "board".into(),
            fields: vec![],
        }],
        ..Default::default()
    };
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        true,
        Some(&dynamic),
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

    assert!(
        !ids.iter().any(|id| id.starts_with("perspective.goto:")),
        "legacy perspective.goto:* prefix must never appear"
    );
    // The dynamic palette row we emit has group "perspective" — none of
    // those should be reachable in context-menu-only mode.
    assert!(
        !cmds
            .iter()
            .any(|c| c.id == "perspective.switch" && c.group == "perspective"),
        "perspective.switch navigation rows should not appear in context menu: {:?}",
        cmds.iter().map(|c| (&c.id, &c.group)).collect::<Vec<_>>()
    );
}

/// Without `DynamicSources`, no perspective navigation rows are emitted.
/// Guards against a regression where the dynamic emitter runs on stale
/// or missing runtime data and accidentally leaks a naked
/// `perspective.switch` row (without args) into the palette.
#[test]
fn no_perspective_commands_without_dynamic_sources() {
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

    assert!(
        !ids.iter().any(|id| id.starts_with("perspective.goto:")),
        "legacy perspective.goto:* prefix must never appear"
    );
    // With no dynamic sources, the only way a `perspective.switch` row could
    // sneak through is the registry itself — which should not happen in
    // `context_menu_only == false` without a perspective in scope.
    assert!(
        !cmds
            .iter()
            .any(|c| c.id == "perspective.switch" && c.group == "perspective"),
        "no perspective navigation rows without dynamic sources"
    );
}

// =========================================================================
// Dynamic window focus commands
// =========================================================================

#[test]
fn window_focus_commands_generated_from_dynamic_sources() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["board:my-board".into()];
    let dynamic = DynamicSources {
        views: vec![],
        boards: vec![],
        windows: vec![
            WindowInfo {
                label: "main".into(),
                title: "SwissArmyHammer".into(),
                focused: true,
            },
            WindowInfo {
                label: "board-01abc".into(),
                title: "My Project".into(),
                focused: false,
            },
        ],
        perspectives: vec![],
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

    assert!(
        ids.contains(&"window.focus:main"),
        "should have main window focus: {:?}",
        ids
    );
    assert!(
        ids.contains(&"window.focus:board-01abc"),
        "should have board-01abc window focus: {:?}",
        ids
    );
}

#[test]
fn window_focus_commands_have_correct_names_and_ids() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["board:my-board".into()];
    let dynamic = DynamicSources {
        views: vec![],
        boards: vec![],
        windows: vec![WindowInfo {
            label: "main".into(),
            title: "SwissArmyHammer".into(),
            focused: true,
        }],
        perspectives: vec![],
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

    let win_cmd = cmds
        .iter()
        .find(|c| c.id == "window.focus:main")
        .expect("should have window.focus:main");
    assert_eq!(win_cmd.name, "SwissArmyHammer");
    assert_eq!(win_cmd.menu_name.as_deref(), Some("SwissArmyHammer"));
    assert_eq!(win_cmd.group, "window");
    assert!(win_cmd.target.is_none());
    assert!(!win_cmd.context_menu);
    assert!(win_cmd.available);
}

#[test]
fn window_focus_commands_not_in_context_menu() {
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["board:my-board".into()];
    let dynamic = DynamicSources {
        views: vec![],
        boards: vec![],
        windows: vec![WindowInfo {
            label: "main".into(),
            title: "SwissArmyHammer".into(),
            focused: true,
        }],
        perspectives: vec![],
    };
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        true,
        Some(&dynamic),
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

    assert!(
        !ids.iter().any(|id| id.starts_with("window.focus:")),
        "window focus commands should not appear in context menu"
    );
}

#[test]
fn no_window_commands_without_dynamic_sources() {
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

    assert!(
        !ids.iter().any(|id| id.starts_with("window.focus:")),
        "no window commands without dynamic sources"
    );
}
