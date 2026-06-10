//! End-to-end tests for the `ui_state` MCP server's verbs.
//!
//! Each test drives a verb through the real `UiStateServer` /
//! `ServerHandler::call_tool` path over a temp-file-backed `UIState`, then
//! asserts both the structured response and the observed persisted state. The
//! verbs cover every group the `_meta` tree advertises: inspector, palette,
//! keymap, rename, drag, and the app-UI toggles.

use serde_json::json;

use super::common::{call_tool, Harness};

/// `inspect inspector` pushes the moniker onto the window's inspector stack.
#[tokio::test]
async fn inspect_pushes_onto_inspector_stack() {
    let h = Harness::new();
    let service = h.service();

    let res = call_tool(
        &service,
        "inspect inspector",
        json!({ "op": "inspect inspector", "scope_chain": ["window:main"], "moniker": "task:01XYZ" }),
    )
    .await
    .expect("inspect should succeed");

    assert_eq!(res["ok"], json!(true));
    // The change payload carries the new stack as an externally-tagged enum.
    assert_eq!(res["change"]["InspectorStack"], json!(["task:01XYZ"]));
    // And the persisted state reflects the push.
    assert_eq!(h.ui_state.inspector_stack("main"), vec!["task:01XYZ"]);
}

/// A second inspect stacks on top; `close inspector` pops just the top entry.
#[tokio::test]
async fn inspect_stacks_then_close_pops_top() {
    let h = Harness::new();
    let service = h.service();

    for moniker in ["task:01XYZ", "tag:01TAG"] {
        call_tool(
            &service,
            "inspect inspector",
            json!({ "op": "inspect inspector", "scope_chain": ["window:main"], "moniker": moniker }),
        )
        .await
        .expect("inspect should succeed");
    }
    assert_eq!(
        h.ui_state.inspector_stack("main"),
        vec!["task:01XYZ", "tag:01TAG"]
    );

    let res = call_tool(
        &service,
        "close inspector",
        json!({ "op": "close inspector", "scope_chain": ["window:main"] }),
    )
    .await
    .expect("close inspector should succeed");

    assert_eq!(res["change"]["InspectorStack"], json!(["task:01XYZ"]));
    assert_eq!(h.ui_state.inspector_stack("main"), vec!["task:01XYZ"]);
}

/// `close_all inspector` clears the whole stack for the window.
#[tokio::test]
async fn close_all_clears_inspector_stack() {
    let h = Harness::new();
    let service = h.service();

    for moniker in ["task:01XYZ", "tag:01TAG"] {
        call_tool(
            &service,
            "inspect inspector",
            json!({ "op": "inspect inspector", "scope_chain": ["window:main"], "moniker": moniker }),
        )
        .await
        .expect("inspect should succeed");
    }

    let res = call_tool(
        &service,
        "close_all inspector",
        json!({ "op": "close_all inspector", "scope_chain": ["window:main"] }),
    )
    .await
    .expect("close_all inspector should succeed");

    assert_eq!(res["change"]["InspectorStack"], json!([]));
    assert!(h.ui_state.inspector_stack("main").is_empty());
}

/// `set_width inspector` persists the chosen width, clamped into bounds.
#[tokio::test]
async fn set_width_persists_clamped_inspector_width() {
    let h = Harness::new();
    let service = h.service();

    // In-range width persists verbatim.
    let res = call_tool(
        &service,
        "set_width inspector",
        json!({ "op": "set_width inspector", "scope_chain": ["window:main"], "width": 540 }),
    )
    .await
    .expect("set_width should succeed");
    assert_eq!(res["change"]["InspectorWidth"]["width"], json!(540));
    assert_eq!(h.ui_state.inspector_width("main"), Some(540));

    // A tiny width is clamped up to the 320 px floor.
    call_tool(
        &service,
        "set_width inspector",
        json!({ "op": "set_width inspector", "scope_chain": ["window:main"], "width": 1 }),
    )
    .await
    .expect("set_width should succeed");
    assert_eq!(h.ui_state.inspector_width("main"), Some(320));
}

/// `set keymap` sets the active keymap mode and persists it.
#[tokio::test]
async fn set_keymap_mode_sets_active_keymap() {
    let h = Harness::new();
    let service = h.service();
    // Default keymap is "cua".
    assert_eq!(h.ui_state.keymap_mode(), "cua");

    let res = call_tool(
        &service,
        "set keymap",
        json!({ "op": "set keymap", "mode": "vim" }),
    )
    .await
    .expect("set keymap should succeed");

    assert_eq!(res["change"]["KeymapMode"], json!("vim"));
    assert_eq!(h.ui_state.keymap_mode(), "vim");
}

/// `set scope_chain` records the focus scope chain the frontend sends — the
/// `ui.setFocus` routing target. The op consumes `scope_chain` directly; there
/// is no `fq`.
#[tokio::test]
async fn set_scope_chain_records_the_focus_scope_chain() {
    let h = Harness::new();
    let service = h.service();
    assert!(
        h.ui_state.scope_chain().is_empty(),
        "scope chain starts empty"
    );

    let res = call_tool(
        &service,
        "set scope_chain",
        json!({
            "op": "set scope_chain",
            "scope_chain": ["field:T1.title", "card:T1", "board:main"],
        }),
    )
    .await
    .expect("set scope_chain should succeed");

    assert_eq!(
        res["change"]["ScopeChain"],
        json!(["field:T1.title", "card:T1", "board:main"]),
        "the op returns the recorded chain in its change payload"
    );
    assert_eq!(
        h.ui_state.scope_chain(),
        vec![
            "field:T1.title".to_string(),
            "card:T1".to_string(),
            "board:main".to_string()
        ],
        "the chain is recorded into UI state for command-gating fallback"
    );
}

/// `open palette` flips the palette flag on; `close palette` flips it off.
#[tokio::test]
async fn palette_open_then_close_toggles_flag() {
    let h = Harness::new();
    let service = h.service();
    assert!(!h.ui_state.palette_open("main"));

    let res = call_tool(
        &service,
        "open palette",
        json!({ "op": "open palette", "scope_chain": ["window:main"], "mode": "search" }),
    )
    .await
    .expect("open palette should succeed");
    assert_eq!(res["change"]["PaletteOpen"], json!(true));
    assert!(h.ui_state.palette_open("main"));
    assert_eq!(h.ui_state.palette_mode("main"), "search");

    let res = call_tool(
        &service,
        "close palette",
        json!({ "op": "close palette", "scope_chain": ["window:main"] }),
    )
    .await
    .expect("close palette should succeed");
    assert_eq!(res["change"]["PaletteOpen"], json!(false));
    assert!(!h.ui_state.palette_open("main"));
}

/// `open palette` defaults to command mode when `mode` is omitted.
#[tokio::test]
async fn palette_open_defaults_to_command_mode() {
    let h = Harness::new();
    let service = h.service();

    call_tool(
        &service,
        "open palette",
        json!({ "op": "open palette", "scope_chain": ["window:main"] }),
    )
    .await
    .expect("open palette should succeed");

    assert!(h.ui_state.palette_open("main"));
    assert_eq!(h.ui_state.palette_mode("main"), "command");
}

/// A full drag lifecycle: `start drag` stores a session, `complete drag`
/// takes it and leaves no active session behind.
#[tokio::test]
async fn drag_start_then_complete_transitions_session() {
    let h = Harness::new();
    let service = h.service();
    assert!(h.ui_state.drag_session().is_none());

    let res = call_tool(
        &service,
        "start drag",
        json!({
            "op": "start drag",
            "session_id": "01DRAG",
            "entity_type": "task",
            "entity_id": "01TASK",
            "source_board_path": "/boards/a.kanban",
            "source_window_label": "main",
            "copy_mode": true,
            "started_at_ms": 1234,
        }),
    )
    .await
    .expect("start drag should succeed");
    assert_eq!(res["session"]["session_id"], json!("01DRAG"));
    // The session is now active and carries the source fields.
    let active = h.ui_state.drag_session().expect("session is active");
    assert_eq!(active.session_id, "01DRAG");
    assert!(active.copy_mode);
    assert_eq!(active.entity_id(), Some("01TASK"));

    // Completing takes and returns the session.
    let res = call_tool(&service, "complete drag", json!({ "op": "complete drag" }))
        .await
        .expect("complete drag should succeed");
    assert_eq!(res["session"]["session_id"], json!("01DRAG"));
    // No active session remains after completion.
    assert!(h.ui_state.drag_session().is_none());
}

/// `cancel drag` clears the active session without returning it.
#[tokio::test]
async fn drag_cancel_clears_session() {
    let h = Harness::new();
    let service = h.service();

    call_tool(
        &service,
        "start drag",
        json!({
            "op": "start drag",
            "session_id": "01DRAG",
            "entity_type": "task",
            "entity_id": "01TASK",
            "source_board_path": "/boards/a.kanban",
            "source_window_label": "main",
        }),
    )
    .await
    .expect("start drag should succeed");
    assert!(h.ui_state.drag_session().is_some());

    let res = call_tool(&service, "cancel drag", json!({ "op": "cancel drag" }))
        .await
        .expect("cancel drag should succeed");
    assert_eq!(res["ok"], json!(true));
    assert!(h.ui_state.drag_session().is_none());
}

/// `start rename` is a backend no-op that reports success.
#[tokio::test]
async fn start_rename_is_backend_noop() {
    let h = Harness::new();
    let service = h.service();

    let res = call_tool(
        &service,
        "start rename",
        json!({ "op": "start rename", "scope_chain": ["window:main"] }),
    )
    .await
    .expect("start rename should succeed");

    assert_eq!(res["ok"], json!(true));
    // No state changed.
    assert!(!h.ui_state.palette_open("main"));
    assert!(h.ui_state.inspector_stack("main").is_empty());
}

/// `show command` opens the palette in command mode.
#[tokio::test]
async fn show_command_opens_palette_in_command_mode() {
    let h = Harness::new();
    let service = h.service();

    call_tool(
        &service,
        "show command",
        json!({ "op": "show command", "scope_chain": ["window:main"] }),
    )
    .await
    .expect("show command should succeed");

    assert!(h.ui_state.palette_open("main"));
    assert_eq!(h.ui_state.palette_mode("main"), "command");
}

/// `show search` opens the palette in search mode.
#[tokio::test]
async fn show_search_opens_palette_in_search_mode() {
    let h = Harness::new();
    let service = h.service();

    call_tool(
        &service,
        "show search",
        json!({ "op": "show search", "scope_chain": ["window:main"] }),
    )
    .await
    .expect("show search should succeed");

    assert!(h.ui_state.palette_open("main"));
    assert_eq!(h.ui_state.palette_mode("main"), "search");
}

/// `show palette` opens the palette for the window.
#[tokio::test]
async fn show_palette_opens_palette() {
    let h = Harness::new();
    let service = h.service();

    let res = call_tool(
        &service,
        "show palette",
        json!({ "op": "show palette", "scope_chain": ["window:main"] }),
    )
    .await
    .expect("show palette should succeed");

    assert_eq!(res["change"]["PaletteOpen"], json!(true));
    assert!(h.ui_state.palette_open("main"));
}

/// `dismiss ui` closes the palette first when one is open.
#[tokio::test]
async fn dismiss_closes_open_palette_first() {
    let h = Harness::new();
    let service = h.service();

    // Open the palette and stack an inspector entry.
    call_tool(
        &service,
        "show command",
        json!({ "op": "show command", "scope_chain": ["window:main"] }),
    )
    .await
    .expect("show command should succeed");
    call_tool(
        &service,
        "inspect inspector",
        json!({ "op": "inspect inspector", "scope_chain": ["window:main"], "moniker": "task:01XYZ" }),
    )
    .await
    .expect("inspect should succeed");

    // First dismiss closes the palette, leaving the inspector intact.
    let res = call_tool(
        &service,
        "dismiss ui",
        json!({ "op": "dismiss ui", "scope_chain": ["window:main"] }),
    )
    .await
    .expect("dismiss should succeed");
    assert_eq!(res["change"]["PaletteOpen"], json!(false));
    assert!(!h.ui_state.palette_open("main"));
    assert_eq!(h.ui_state.inspector_stack("main"), vec!["task:01XYZ"]);

    // Second dismiss pops the inspector.
    let res = call_tool(
        &service,
        "dismiss ui",
        json!({ "op": "dismiss ui", "scope_chain": ["window:main"] }),
    )
    .await
    .expect("dismiss should succeed");
    assert_eq!(res["change"]["InspectorStack"], json!([]));
    assert!(h.ui_state.inspector_stack("main").is_empty());
}

/// Every per-window mutation op is REJECTED when the scope chain carries no
/// `window:` moniker — it must never silently fall back to the `main` window
/// slot (the regression class a2002c330 fixed: palette/inspector state written
/// to a window no board reads).
#[tokio::test]
async fn per_window_ops_reject_scope_chain_without_window_moniker() {
    let h = Harness::new();
    let service = h.service();

    // A plausible-but-broken producer chain: monikers present, window absent.
    let chain = serde_json::json!(["task:01ABC", "board:foo"]);

    let calls: Vec<(&str, serde_json::Value)> = vec![
        (
            "inspect inspector",
            json!({ "op": "inspect inspector", "scope_chain": chain, "moniker": "task:01XYZ" }),
        ),
        (
            "close inspector",
            json!({ "op": "close inspector", "scope_chain": chain }),
        ),
        (
            "close_all inspector",
            json!({ "op": "close_all inspector", "scope_chain": chain }),
        ),
        (
            "set_width inspector",
            json!({ "op": "set_width inspector", "scope_chain": chain, "width": 540 }),
        ),
        (
            "open palette",
            json!({ "op": "open palette", "scope_chain": chain, "mode": "command" }),
        ),
        (
            "close palette",
            json!({ "op": "close palette", "scope_chain": chain }),
        ),
        (
            "set active_view",
            json!({ "op": "set active_view", "scope_chain": chain, "view_id": "board-view" }),
        ),
        (
            "show command",
            json!({ "op": "show command", "scope_chain": chain }),
        ),
        (
            "show palette",
            json!({ "op": "show palette", "scope_chain": chain }),
        ),
        (
            "show search",
            json!({ "op": "show search", "scope_chain": chain }),
        ),
        (
            "dismiss ui",
            json!({ "op": "dismiss ui", "scope_chain": chain }),
        ),
    ];

    for (op, args) in calls {
        match call_tool(&service, op, args).await {
            Ok(res) => panic!("op {op:?} must error without a window: moniker, got: {res}"),
            Err(err) => assert!(
                err.message.contains("window:"),
                "op {op:?} error should name the missing `window:` moniker: {}",
                err.message
            ),
        }
    }

    // Nothing was mutated: no window slot (not even "main") was created.
    assert!(
        h.ui_state.all_windows().is_empty(),
        "rejected ops must not create or mutate any window slot, got: {:?}",
        h.ui_state.all_windows().keys().collect::<Vec<_>>()
    );
}

/// An EMPTY scope chain is also rejected on the per-window path — the
/// historical `unwrap_or("main")` default would have silently targeted `main`.
#[tokio::test]
async fn per_window_op_rejects_empty_scope_chain() {
    let h = Harness::new();
    let service = h.service();

    let err = call_tool(
        &service,
        "open palette",
        json!({ "op": "open palette", "scope_chain": [], "mode": "command" }),
    )
    .await
    .expect_err("open palette with an empty scope chain must error");

    assert!(
        err.message.contains("window:"),
        "error should name the missing `window:` moniker: {}",
        err.message
    );
    assert!(
        !h.ui_state.palette_open("main"),
        "the main slot must not be touched by the rejected op"
    );
    assert!(h.ui_state.all_windows().is_empty());
}

/// A per-window op with a non-"main" `window:` moniker mutates THAT window's
/// slot and leaves `main` untouched (the positive half of the hardening).
#[tokio::test]
async fn per_window_op_targets_the_scope_chain_window_not_main() {
    let h = Harness::new();
    let service = h.service();

    call_tool(
        &service,
        "set active_view",
        json!({
            "op": "set active_view",
            "scope_chain": ["view:v1", "window:board-2"],
            "view_id": "grid-view",
        }),
    )
    .await
    .expect("set active_view with a window: moniker should succeed");

    assert_eq!(h.ui_state.active_view_id("board-2"), "grid-view");
    assert_eq!(
        h.ui_state.active_view_id("main"),
        "",
        "the main slot must stay untouched"
    );
}

/// An unknown op surfaces a structured `invalid_params` error.
#[tokio::test]
async fn unknown_op_errors() {
    let h = Harness::new();
    let service = h.service();

    let err = call_tool(&service, "frobnicate ui", json!({ "op": "frobnicate ui" }))
        .await
        .expect_err("unknown op should error");

    assert!(
        err.message.contains("frobnicate ui"),
        "error should name the unknown op: {}",
        err.message
    );
}
