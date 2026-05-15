//! Headless test for the relocated `build_dynamic_sources` helper.
//!
//! PR #40 review required `build_dynamic_sources` to live in a crate that
//! does NOT depend on `tauri`/GUI chrome, so the same assembly logic that
//! feeds `list_commands_for_scope` in the live app is fully exercisable from
//! a Rust integration test without standing up any Tauri scaffolding.
//!
//! This file proves the relocated entry point produces the exact same
//! `DynamicSources` shape the GUI crate used to assemble inline: views,
//! boards, and perspectives computed from a bare `UIState` + one or more
//! `KanbanContext`s, with `WindowInfo` supplied by the caller (since live
//! window titles/focus states can only come from the GUI runtime).
//!
//! The test then pipes the resulting `DynamicSources` through
//! `commands_for_scope` and asserts the downstream dynamic-command rows
//! (`view.set` + args, `board.switch:*`, `window.focus:*`,
//! `perspective.switch` + args, `entity.add:*`) are emitted exactly as
//! production would emit them. View/perspective navigation rows ship as
//! the canonical `view.set` / `perspective.switch` command with pre-filled
//! `args` since task 01KPZMXXEXKVE3RNPA4XJP0105 retired the legacy
//! `view.switch:{id}` / `perspective.goto:{id}` indirection.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::json;
use swissarmyhammer_commands::{compose_registry, UIState, WindowInfo};
use swissarmyhammer_kanban::dynamic_sources::{build_dynamic_sources, DynamicSourcesInputs};
use swissarmyhammer_kanban::scope_commands::{commands_for_scope, DynamicSources};
use swissarmyhammer_kanban::{
    board::InitBoard, dispatch::execute_operation, parse::parse_input, Execute, KanbanContext,
};
use tempfile::TempDir;

/// Open a fresh board under a temp dir and return the context and its
/// canonical `.kanban` path. The context has the views + perspectives
/// sub-contexts eagerly populated via `KanbanContext::open`.
async fn open_board(name: &str) -> (TempDir, Arc<KanbanContext>, PathBuf) {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::open(&kanban_dir)
        .await
        .expect("KanbanContext::open must succeed");

    InitBoard::new(name)
        .execute(&ctx)
        .await
        .into_result()
        .expect("InitBoard must succeed");

    let canonical = kanban_dir
        .canonicalize()
        .unwrap_or_else(|_| kanban_dir.clone());
    (temp, Arc::new(ctx), canonical)
}

/// Add a perspective via the standard dispatch path so the change is
/// persisted on disk exactly as the live app would persist it.
async fn add_perspective(ctx: &KanbanContext, name: &str, view: &str) -> String {
    add_perspective_inner(ctx, name, view, None).await
}

/// Add a perspective pinned to a specific view instance (`view_id`).
async fn add_perspective_scoped(
    ctx: &KanbanContext,
    name: &str,
    view: &str,
    view_id: &str,
) -> String {
    add_perspective_inner(ctx, name, view, Some(view_id)).await
}

/// Underlying helper shared by [`add_perspective`] and
/// [`add_perspective_scoped`].
async fn add_perspective_inner(
    ctx: &KanbanContext,
    name: &str,
    view: &str,
    view_id: Option<&str>,
) -> String {
    let mut payload = json!({
        "op": "add perspective",
        "name": name,
        "view": view,
    });
    if let Some(vid) = view_id {
        payload
            .as_object_mut()
            .expect("payload is an object")
            .insert("view_id".into(), json!(vid));
    }
    let ops = parse_input(payload).expect("parse_input should succeed");
    let out = execute_operation(ctx, &ops[0])
        .await
        .expect("execute_operation should succeed");
    out["id"]
        .as_str()
        .expect("add perspective must return an id")
        .to_string()
}

/// Register a grid-kind view with the supplied id+name against the active
/// board's views registry. Returns nothing — the id is the caller's input.
///
/// Used by the per-`view_id` perspective scoping tests, which need to flip
/// `UIState`'s active view between two grid views sharing the same kind
/// to prove `view_id` resolves to different perspectives.
async fn register_grid_view(ctx: &KanbanContext, id: &str, name: &str) {
    use swissarmyhammer_views::{ViewDef, ViewKind};
    let views_lock = ctx.views().expect("KanbanContext must have a views ctx");
    let mut views = views_lock.write().await;
    let def = ViewDef {
        id: id.to_string(),
        name: name.to_string(),
        icon: None,
        kind: ViewKind::Grid,
        entity_type: Some("task".into()),
        card_fields: Vec::new(),
        commands: Vec::new(),
    };
    views
        .write_view(&def)
        .await
        .expect("write_view must succeed");
}

/// End-to-end headless assembly: seed a board + a perspective, point UIState
/// at that board + an active view, then call `build_dynamic_sources` and
/// assert every emitted dynamic command matches what the GUI path would
/// produce. No Tauri crate is in scope.
#[tokio::test]
async fn build_dynamic_sources_assembles_views_boards_perspectives_headless() {
    let (_tmp, ctx, board_path) = open_board("Sample").await;

    // Seed a perspective on the active board so the perspective gather path
    // has something to return.
    let persp_id = add_perspective(&ctx, "Active Sprint", "board").await;

    // Bare UIState: marks the board as open and selects a real view id so
    // `resolve_active_view` has something to return.
    let ui = UIState::new();
    let board_path_str = board_path.display().to_string();
    ui.add_open_board(&board_path_str);
    // Use the ULID id of the built-in `board` view (kind=board). ViewsContext
    // reads IDs from the YAML, not human-friendly slugs. Hardcoding the
    // builtin ULID here matches the fixtures used elsewhere in this crate
    // (see scope_commands tests).
    const BUILTIN_BOARD_VIEW_ID: &str = "01JMVIEW0000000000BOARD0";
    ui.set_active_view("main", BUILTIN_BOARD_VIEW_ID);

    let mut open_boards: HashMap<PathBuf, Arc<KanbanContext>> = HashMap::new();
    open_boards.insert(board_path.clone(), Arc::clone(&ctx));

    // Caller-provided live windows list — headless tests fabricate one,
    // the GUI path passes real Tauri-derived data. Shape unchanged.
    let windows = vec![WindowInfo {
        label: "main".to_string(),
        title: "SwissArmyHammer — Sample".to_string(),
        focused: true,
    }];

    let inputs = DynamicSourcesInputs {
        ui_state: &ui,
        active_ctx: Some(&ctx),
        open_board_ctxs: &open_boards,
        active_window_label: Some("main"),
        windows,
    };
    let dynamic: DynamicSources = build_dynamic_sources(inputs).await;

    // Views: the built-in view registry includes the `board` view.
    assert!(
        dynamic.views.iter().any(|v| v.id == BUILTIN_BOARD_VIEW_ID),
        "views must contain the built-in board view; got {:?}",
        dynamic.views.iter().map(|v| &v.id).collect::<Vec<_>>()
    );

    // Boards: exactly one BoardInfo, pointing at our temp path, with an
    // entity_name driven by the `init board` call above.
    assert_eq!(dynamic.boards.len(), 1, "exactly one open board");
    let board_info = &dynamic.boards[0];
    assert_eq!(board_info.path, board_path_str);
    assert_eq!(
        board_info.entity_name, "Sample",
        "entity_name must come from the real entity read"
    );
    assert!(
        !board_info.context_name.is_empty(),
        "context_name must be the KanbanContext::name() value"
    );

    // Windows: caller-supplied list passes through untouched.
    assert_eq!(dynamic.windows.len(), 1);
    assert_eq!(dynamic.windows[0].label, "main");

    // Perspectives: the one we added above must appear, filtered to the
    // active view kind ("board").
    assert!(
        dynamic
            .perspectives
            .iter()
            .any(|p| p.id == persp_id && p.view == "board"),
        "added perspective must be emitted; got {:?}",
        dynamic
            .perspectives
            .iter()
            .map(|p| (&p.id, &p.view))
            .collect::<Vec<_>>()
    );

    // Now pipe through `commands_for_scope` and verify the headless
    // DynamicSources drives the same dynamic-command emission the GUI
    // path exercises.
    let registry = compose_registry![swissarmyhammer_commands, swissarmyhammer_kanban];
    let impls: HashMap<String, Arc<dyn swissarmyhammer_commands::Command>> = HashMap::new();
    let ui_arc = Arc::new(ui);
    let scope = vec![
        format!("view:{}", BUILTIN_BOARD_VIEW_ID),
        format!("board:{}", board_path_str),
    ];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        ctx.fields(),
        &ui_arc,
        false,
        Some(&dynamic),
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

    // View navigation: `view.set` row carrying the builtin view id in args.
    assert!(
        cmds.iter().any(|c| c.id == "view.set"
            && c.args.as_ref().and_then(|v| v.get("view_id"))
                == Some(&serde_json::Value::String(BUILTIN_BOARD_VIEW_ID.into()))),
        "view.set with args.view_id={BUILTIN_BOARD_VIEW_ID} must be emitted \
         from headless DynamicSources; cmds={:?}",
        cmds.iter().map(|c| (&c.id, &c.args)).collect::<Vec<_>>()
    );
    assert!(
        ids.iter()
            .any(|id| *id == format!("board.switch:{}", board_path_str)),
        "board.switch for active board must be emitted; ids={:?}",
        ids
    );
    assert!(
        ids.iter().any(|id| id == &"window.focus:main"),
        "window.focus from the supplied windows list must be emitted; ids={:?}",
        ids
    );
    // Perspective navigation: `perspective.switch` row carrying the added
    // perspective's id in args.
    assert!(
        cmds.iter().any(|c| c.id == "perspective.switch"
            && c.args.as_ref().and_then(|v| v.get("perspective_id"))
                == Some(&serde_json::Value::String(persp_id.clone()))),
        "perspective.switch with args.perspective_id={persp_id} must be emitted \
         for the added perspective; cmds={:?}",
        cmds.iter().map(|c| (&c.id, &c.args)).collect::<Vec<_>>()
    );
}

/// When no active board context is in scope, the headless builder must still
/// return a `DynamicSources` — just with empty views and perspectives (since
/// both are derived from the active board). Boards and windows pass through
/// as usual. This mirrors the live-app behavior when `active_handle` is
/// `None` (no board focused).
#[tokio::test]
async fn build_dynamic_sources_handles_no_active_context() {
    let ui = UIState::new();
    let open_boards: HashMap<PathBuf, Arc<KanbanContext>> = HashMap::new();

    let inputs = DynamicSourcesInputs {
        ui_state: &ui,
        active_ctx: None,
        open_board_ctxs: &open_boards,
        active_window_label: Some("main"),
        windows: vec![],
    };
    let dynamic = build_dynamic_sources(inputs).await;

    assert!(dynamic.views.is_empty());
    assert!(dynamic.boards.is_empty());
    assert!(dynamic.windows.is_empty());
    assert!(dynamic.perspectives.is_empty());
}

/// Multi-board, multi-window scenario: two open boards with two `WindowInfo`
/// entries must each produce their own `board.switch:*` and `window.focus:*`
/// emissions. This covers the branch the task description called out as
/// "Multiple open boards, multiple windows" and that the original two tests
/// skipped.
#[tokio::test]
async fn build_dynamic_sources_emits_every_open_board_and_window() {
    let (_tmp_a, ctx_a, path_a) = open_board("Board Alpha").await;
    let (_tmp_b, ctx_b, path_b) = open_board("Board Beta").await;

    let ui = UIState::new();
    let path_a_str = path_a.display().to_string();
    let path_b_str = path_b.display().to_string();
    ui.add_open_board(&path_a_str);
    ui.add_open_board(&path_b_str);
    const BUILTIN_BOARD_VIEW_ID: &str = "01JMVIEW0000000000BOARD0";
    ui.set_active_view("main", BUILTIN_BOARD_VIEW_ID);

    let mut open_boards: HashMap<PathBuf, Arc<KanbanContext>> = HashMap::new();
    open_boards.insert(path_a.clone(), Arc::clone(&ctx_a));
    open_boards.insert(path_b.clone(), Arc::clone(&ctx_b));

    // Two windows — the second one focused — to prove both pass through.
    let windows = vec![
        WindowInfo {
            label: "main".to_string(),
            title: "SwissArmyHammer — Alpha".to_string(),
            focused: false,
        },
        WindowInfo {
            label: "board-beta".to_string(),
            title: "SwissArmyHammer — Beta".to_string(),
            focused: true,
        },
    ];

    let inputs = DynamicSourcesInputs {
        ui_state: &ui,
        active_ctx: Some(&ctx_a),
        open_board_ctxs: &open_boards,
        active_window_label: Some("main"),
        windows,
    };
    let dynamic: DynamicSources = build_dynamic_sources(inputs).await;

    // Both boards must be present with the entity names we initialised them
    // under — order is unspecified because it follows `UIState::open_boards`.
    assert_eq!(dynamic.boards.len(), 2, "both open boards must be emitted");
    let paths: Vec<&str> = dynamic.boards.iter().map(|b| b.path.as_str()).collect();
    assert!(paths.contains(&path_a_str.as_str()));
    assert!(paths.contains(&path_b_str.as_str()));
    assert!(
        dynamic
            .boards
            .iter()
            .any(|b| b.path == path_a_str && b.entity_name == "Board Alpha"),
        "Alpha's entity_name must resolve via its context; got {:?}",
        dynamic
            .boards
            .iter()
            .map(|b| (&b.path, &b.entity_name))
            .collect::<Vec<_>>()
    );
    assert!(
        dynamic
            .boards
            .iter()
            .any(|b| b.path == path_b_str && b.entity_name == "Board Beta"),
        "Beta's entity_name must resolve via its context; got {:?}",
        dynamic
            .boards
            .iter()
            .map(|b| (&b.path, &b.entity_name))
            .collect::<Vec<_>>()
    );

    // Both windows pass through unchanged.
    assert_eq!(dynamic.windows.len(), 2);
    let labels: Vec<&str> = dynamic.windows.iter().map(|w| w.label.as_str()).collect();
    assert!(labels.contains(&"main"));
    assert!(labels.contains(&"board-beta"));

    // Pipe through `commands_for_scope` and verify both `board.switch:*` and
    // both `window.focus:*` commands are emitted.
    let registry = compose_registry![swissarmyhammer_commands, swissarmyhammer_kanban];
    let impls: HashMap<String, Arc<dyn swissarmyhammer_commands::Command>> = HashMap::new();
    let ui_arc = Arc::new(ui);
    let scope = vec![
        format!("view:{}", BUILTIN_BOARD_VIEW_ID),
        format!("board:{}", path_a_str),
    ];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        ctx_a.fields(),
        &ui_arc,
        false,
        Some(&dynamic),
        None,
    );
    let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
    assert!(
        ids.iter()
            .any(|id| *id == format!("board.switch:{}", path_a_str)),
        "board.switch:Alpha must be emitted; ids={:?}",
        ids
    );
    assert!(
        ids.iter()
            .any(|id| *id == format!("board.switch:{}", path_b_str)),
        "board.switch:Beta must be emitted; ids={:?}",
        ids
    );
    assert!(
        ids.iter().any(|id| id == &"window.focus:main"),
        "window.focus:main must be emitted; ids={:?}",
        ids
    );
    assert!(
        ids.iter().any(|id| id == &"window.focus:board-beta"),
        "window.focus:board-beta must be emitted; ids={:?}",
        ids
    );
}

/// When `ui_state.open_boards()` names a path with no matching
/// `open_board_ctxs` entry, the headless builder must fall back to the
/// parent directory basename for both `entity_name` and `context_name`.
///
/// This branch is load-bearing on the live-app splash/welcome path,
/// where UIState lists recent boards the user has not opened yet — there
/// is no `KanbanContext` for them, but they still need to render in the
/// board-switcher menu as something humans can read.
#[tokio::test]
async fn build_dynamic_sources_falls_back_to_basename_when_ctx_missing() {
    // Use a stable path — the board is never opened, so nothing on disk
    // has to exist. The builder only reads `ui_state.open_boards()`, it
    // does not stat the filesystem.
    let recent_path = PathBuf::from("/tmp/swissarmyhammer-headless/recents-fixture/.kanban");
    let recent_str = recent_path.display().to_string();

    let ui = UIState::new();
    ui.add_open_board(&recent_str);

    // Intentionally empty: the path is in `open_boards` but we have no
    // matching context for it, which is exactly the splash/welcome case.
    let open_boards: HashMap<PathBuf, Arc<KanbanContext>> = HashMap::new();

    let inputs = DynamicSourcesInputs {
        ui_state: &ui,
        active_ctx: None,
        open_board_ctxs: &open_boards,
        active_window_label: Some("main"),
        windows: vec![],
    };
    let dynamic = build_dynamic_sources(inputs).await;

    assert_eq!(dynamic.boards.len(), 1, "recent board must still surface");
    let info = &dynamic.boards[0];
    assert_eq!(info.path, recent_str);
    // Parent-of-`.kanban` basename is "recents-fixture".
    assert_eq!(
        info.entity_name, "recents-fixture",
        "entity_name must fall back to parent dir basename when ctx is missing"
    );
    assert_eq!(
        info.context_name, "recents-fixture",
        "context_name must fall back to parent dir basename when ctx is missing"
    );
    assert_eq!(
        info.name, "recents-fixture",
        "name must be parent dir basename (it always is, regardless of ctx)"
    );
}

/// Negative filter assertion: a perspective registered with `view: "grid"`
/// must NOT appear when the active view kind resolves to `"board"`. This
/// guards the `is_none_or` filter in `gather_perspectives` from regressing
/// to the pre-fix behavior where the same Default perspective emitted once
/// per view kind.
#[tokio::test]
async fn build_dynamic_sources_filters_perspectives_by_active_view_kind() {
    let (_tmp, ctx, board_path) = open_board("Sample").await;

    // Add one perspective per view kind so we can tell which side of the
    // filter survives.
    let board_persp_id = add_perspective(&ctx, "Board Sprint", "board").await;
    let grid_persp_id = add_perspective(&ctx, "Grid Backlog", "grid").await;

    let ui = UIState::new();
    let board_path_str = board_path.display().to_string();
    ui.add_open_board(&board_path_str);
    // Active view kind is "board" — so grid perspectives must be filtered out.
    const BUILTIN_BOARD_VIEW_ID: &str = "01JMVIEW0000000000BOARD0";
    ui.set_active_view("main", BUILTIN_BOARD_VIEW_ID);

    let mut open_boards: HashMap<PathBuf, Arc<KanbanContext>> = HashMap::new();
    open_boards.insert(board_path.clone(), Arc::clone(&ctx));

    let inputs = DynamicSourcesInputs {
        ui_state: &ui,
        active_ctx: Some(&ctx),
        open_board_ctxs: &open_boards,
        active_window_label: Some("main"),
        windows: vec![],
    };
    let dynamic = build_dynamic_sources(inputs).await;

    assert!(
        dynamic
            .perspectives
            .iter()
            .any(|p| p.id == board_persp_id && p.view == "board"),
        "board perspective must be emitted; got {:?}",
        dynamic
            .perspectives
            .iter()
            .map(|p| (&p.id, &p.view))
            .collect::<Vec<_>>()
    );
    assert!(
        !dynamic.perspectives.iter().any(|p| p.id == grid_persp_id),
        "grid perspective must be filtered out when active view kind is 'board'; got {:?}",
        dynamic
            .perspectives
            .iter()
            .map(|p| (&p.id, &p.view))
            .collect::<Vec<_>>()
    );
    assert!(
        dynamic.perspectives.iter().all(|p| p.view != "grid"),
        "no 'grid'-view perspective may pass the filter"
    );
}

// View ids picked for the per-`view_id` scoping tests. Both are grid-kind
// views so the test can prove they are differentiated by id alone — kind
// is identical between them, only id differs.
const GRID_VIEW_A_ID: &str = "01JMVIEW0000000000TGRID0";
const GRID_VIEW_B_ID: &str = "01JMVIEW0000000000PGRID0";

/// Per-`view_id` scoping: a perspective pinned to view A's id must appear
/// only when view A is active and must NOT appear when view B (a sibling
/// of the same kind) is active.
///
/// Regression test for the bug where two grid views sharing the `"grid"`
/// kind would both surface every grid-kind perspective, even those tied
/// to a different view instance.
#[tokio::test]
async fn perspectives_are_scoped_by_view_id_when_set() {
    let (_tmp, ctx, board_path) = open_board("Sample").await;

    // Register two grid-kind views with distinct ids.
    register_grid_view(&ctx, GRID_VIEW_A_ID, "Grid A").await;
    register_grid_view(&ctx, GRID_VIEW_B_ID, "Grid B").await;

    // Perspective pinned to view A.
    let scoped_persp = add_perspective_scoped(&ctx, "Pinned to A", "grid", GRID_VIEW_A_ID).await;

    let board_path_str = board_path.display().to_string();
    let mut open_boards: HashMap<PathBuf, Arc<KanbanContext>> = HashMap::new();
    open_boards.insert(board_path.clone(), Arc::clone(&ctx));

    // With view B active, the scoped perspective must be filtered out.
    let ui_b = UIState::new();
    ui_b.add_open_board(&board_path_str);
    ui_b.set_active_view("main", GRID_VIEW_B_ID);
    let dynamic_b = build_dynamic_sources(DynamicSourcesInputs {
        ui_state: &ui_b,
        active_ctx: Some(&ctx),
        open_board_ctxs: &open_boards,
        active_window_label: Some("main"),
        windows: vec![],
    })
    .await;
    assert!(
        !dynamic_b.perspectives.iter().any(|p| p.id == scoped_persp),
        "scoped perspective must NOT appear when sibling view B is active; got {:?}",
        dynamic_b
            .perspectives
            .iter()
            .map(|p| (&p.id, &p.view))
            .collect::<Vec<_>>()
    );

    // With view A active, the scoped perspective must be present.
    let ui_a = UIState::new();
    ui_a.add_open_board(&board_path_str);
    ui_a.set_active_view("main", GRID_VIEW_A_ID);
    let dynamic_a = build_dynamic_sources(DynamicSourcesInputs {
        ui_state: &ui_a,
        active_ctx: Some(&ctx),
        open_board_ctxs: &open_boards,
        active_window_label: Some("main"),
        windows: vec![],
    })
    .await;
    assert!(
        dynamic_a.perspectives.iter().any(|p| p.id == scoped_persp),
        "scoped perspective MUST appear when its pinned view is active; got {:?}",
        dynamic_a
            .perspectives
            .iter()
            .map(|p| (&p.id, &p.view))
            .collect::<Vec<_>>()
    );
}

/// Legacy compatibility: a perspective written with `view_id == None` keeps
/// its pre-existing "shared by kind" behavior — every grid-kind view surfaces
/// it, but a board-kind view does not.
///
/// Guards the legacy-perspective fallback branch of
/// `perspective_belongs_to_active_view`.
#[tokio::test]
async fn legacy_kind_perspectives_remain_shared_by_kind() {
    let (_tmp, ctx, board_path) = open_board("Sample").await;
    register_grid_view(&ctx, GRID_VIEW_A_ID, "Grid A").await;
    register_grid_view(&ctx, GRID_VIEW_B_ID, "Grid B").await;

    // Legacy perspective — `view_id` deliberately omitted (None).
    let legacy_persp = add_perspective(&ctx, "Legacy Grid", "grid").await;

    let board_path_str = board_path.display().to_string();
    let mut open_boards: HashMap<PathBuf, Arc<KanbanContext>> = HashMap::new();
    open_boards.insert(board_path.clone(), Arc::clone(&ctx));

    for view_id in [GRID_VIEW_A_ID, GRID_VIEW_B_ID] {
        let ui = UIState::new();
        ui.add_open_board(&board_path_str);
        ui.set_active_view("main", view_id);
        let dynamic = build_dynamic_sources(DynamicSourcesInputs {
            ui_state: &ui,
            active_ctx: Some(&ctx),
            open_board_ctxs: &open_boards,
            active_window_label: Some("main"),
            windows: vec![],
        })
        .await;
        assert!(
            dynamic.perspectives.iter().any(|p| p.id == legacy_persp),
            "legacy view-id-less grid perspective must appear in every grid view; \
             missing when active_view_id={view_id}; got {:?}",
            dynamic
                .perspectives
                .iter()
                .map(|p| (&p.id, &p.view))
                .collect::<Vec<_>>()
        );
    }

    // Board-kind view: legacy grid perspective must NOT appear.
    let ui_board = UIState::new();
    ui_board.add_open_board(&board_path_str);
    const BUILTIN_BOARD_VIEW_ID: &str = "01JMVIEW0000000000BOARD0";
    ui_board.set_active_view("main", BUILTIN_BOARD_VIEW_ID);
    let dynamic_board = build_dynamic_sources(DynamicSourcesInputs {
        ui_state: &ui_board,
        active_ctx: Some(&ctx),
        open_board_ctxs: &open_boards,
        active_window_label: Some("main"),
        windows: vec![],
    })
    .await;
    assert!(
        !dynamic_board
            .perspectives
            .iter()
            .any(|p| p.id == legacy_persp),
        "legacy grid perspective must NOT appear when active view is board-kind; got {:?}",
        dynamic_board
            .perspectives
            .iter()
            .map(|p| (&p.id, &p.view))
            .collect::<Vec<_>>()
    );
}
