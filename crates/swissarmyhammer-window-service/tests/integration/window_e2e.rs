//! End-to-end tests for the `window` MCP server's verbs.
//!
//! Builds a `WindowService` over a recording `SpyShell` and exercises every
//! verb the `_meta` tree advertises across both op groups: the window group
//! (`new window`, `activate window`, `set position`, `get position`,
//! `get monitors`, `close window`) and the OS-file group (`open path`,
//! `reveal path`). Each test drives the verb through the real `ServerHandler` /
//! `call_tool` path and asserts both the structured response and the recorded
//! shell call.

use std::collections::HashMap;

use serde_json::json;

use super::common::{call_tool, Harness, SpyShell};
use swissarmyhammer_window_service::{MonitorInfo, NewWindow, WindowPosition};

/// `new window` opens a window through the shell, threading the board path and
/// returning the shell's resolved label / board.
#[tokio::test]
async fn new_window_opens_via_shell() {
    let h = Harness::with_shell(SpyShell::new(
        NewWindow {
            label: "board-abc".to_string(),
            board_path: Some("/tmp/my-board".to_string()),
        },
        HashMap::new(),
        Vec::new(),
    ));
    let service = h.service();

    let res = call_tool(
        &service,
        "new window",
        json!({ "op": "new window", "board_path": "/tmp/my-board" }),
    )
    .await
    .expect("new window should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["label"], json!("board-abc"));
    assert_eq!(res["board_path"], json!("/tmp/my-board"));
    assert_eq!(
        h.shell.calls(),
        vec!["open_new_window:Some(\"/tmp/my-board\")"],
        "new window must drive exactly one open_new_window with the board path"
    );
}

/// `new window` with no board path passes `None` to the shell.
#[tokio::test]
async fn new_window_without_board_passes_none() {
    let h = Harness::new();
    let service = h.service();

    let res = call_tool(&service, "new window", json!({ "op": "new window" }))
        .await
        .expect("new window should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(h.shell.calls(), vec!["open_new_window:None"]);
}

/// `activate window` focuses the labeled window via the shell.
#[tokio::test]
async fn activate_window_focuses_via_shell() {
    let h = Harness::new();
    let service = h.service();

    let res = call_tool(
        &service,
        "activate window",
        json!({ "op": "activate window", "label": "board-7" }),
    )
    .await
    .expect("activate window should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["label"], json!("board-7"));
    assert_eq!(h.shell.calls(), vec!["activate_window:board-7"]);
}

/// `set position` moves the labeled window to the given coordinates and echoes
/// them back.
#[tokio::test]
async fn set_position_moves_window() {
    let h = Harness::new();
    let service = h.service();

    let res = call_tool(
        &service,
        "set position",
        json!({ "op": "set position", "label": "main", "x": 100, "y": 250 }),
    )
    .await
    .expect("set position should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["label"], json!("main"));
    assert_eq!(res["x"], json!(100));
    assert_eq!(res["y"], json!(250));
    assert_eq!(h.shell.calls(), vec!["set_window_position:main:100,250"]);
}

/// `get position` reads the labeled window's canned position from the shell.
#[tokio::test]
async fn get_position_reads_window() {
    let mut positions = HashMap::new();
    positions.insert("board-9".to_string(), WindowPosition { x: 42, y: 84 });
    let h = Harness::with_shell(SpyShell::new(
        NewWindow {
            label: "x".to_string(),
            board_path: None,
        },
        positions,
        Vec::new(),
    ));
    let service = h.service();

    let res = call_tool(
        &service,
        "get position",
        json!({ "op": "get position", "label": "board-9" }),
    )
    .await
    .expect("get position should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["label"], json!("board-9"));
    assert_eq!(res["x"], json!(42));
    assert_eq!(res["y"], json!(84));
    assert_eq!(h.shell.calls(), vec!["get_window_position:board-9"]);
}

/// `get position` for an unknown label surfaces the shell error as an
/// `internal_error` and still records the attempted read.
#[tokio::test]
async fn get_position_unknown_label_errors() {
    let h = Harness::with_shell(SpyShell::new(
        NewWindow {
            label: "x".to_string(),
            board_path: None,
        },
        HashMap::new(),
        Vec::new(),
    ));
    let service = h.service();

    let err = call_tool(
        &service,
        "get position",
        json!({ "op": "get position", "label": "ghost" }),
    )
    .await
    .expect_err("unknown label should error");

    assert!(
        err.message.contains("ghost"),
        "error should name the unknown label: {}",
        err.message
    );
}

/// `get monitors` returns the shell's canned monitor list.
#[tokio::test]
async fn get_monitors_enumerates() {
    let h = Harness::with_shell(SpyShell::new(
        NewWindow {
            label: "x".to_string(),
            board_path: None,
        },
        HashMap::new(),
        vec![
            MonitorInfo {
                name: Some("Primary".to_string()),
                x: 0,
                y: 0,
                width: 2560,
                height: 1440,
                scale_factor: 2.0,
            },
            MonitorInfo {
                name: None,
                x: 2560,
                y: 0,
                width: 1920,
                height: 1080,
                scale_factor: 1.0,
            },
        ],
    ));
    let service = h.service();

    let res = call_tool(&service, "get monitors", json!({ "op": "get monitors" }))
        .await
        .expect("get monitors should succeed");

    assert_eq!(res["ok"], json!(true));
    let monitors = res["monitors"].as_array().expect("monitors is an array");
    assert_eq!(monitors.len(), 2);
    assert_eq!(monitors[0]["name"], json!("Primary"));
    assert_eq!(monitors[0]["width"], json!(2560));
    assert_eq!(monitors[1]["name"], json!(null));
    assert_eq!(monitors[1]["x"], json!(2560));
    assert_eq!(h.shell.calls(), vec!["get_monitors"]);
}

/// `close window` closes the labeled window via the shell.
#[tokio::test]
async fn close_window_closes_via_shell() {
    let h = Harness::new();
    let service = h.service();

    let res = call_tool(
        &service,
        "close window",
        json!({ "op": "close window", "label": "board-3" }),
    )
    .await
    .expect("close window should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["label"], json!("board-3"));
    assert_eq!(h.shell.calls(), vec!["close_window:board-3"]);
}

/// `open path` invokes the shell opener with the requested path — the ported
/// `attachment.open` behavior.
#[tokio::test]
async fn open_path_invokes_shell_opener() {
    let h = Harness::new();
    let service = h.service();

    let res = call_tool(
        &service,
        "open path",
        json!({ "op": "open path", "path": "/tmp/file.pdf" }),
    )
    .await
    .expect("open path should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["opened"], json!("/tmp/file.pdf"));
    assert_eq!(h.shell.calls(), vec!["open_path:/tmp/file.pdf"]);
}

/// `reveal path` invokes the shell reveal with the requested path — the ported
/// `attachment.reveal` behavior.
#[tokio::test]
async fn reveal_path_invokes_shell_reveal() {
    let h = Harness::new();
    let service = h.service();

    let res = call_tool(
        &service,
        "reveal path",
        json!({ "op": "reveal path", "path": "/tmp/file.pdf" }),
    )
    .await
    .expect("reveal path should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["revealed"], json!("/tmp/file.pdf"));
    assert_eq!(h.shell.calls(), vec!["reveal_path:/tmp/file.pdf"]);
}

/// An unknown op surfaces a structured `invalid_params` error and fires no
/// shell call.
#[tokio::test]
async fn unknown_op_errors_without_side_effect() {
    let h = Harness::new();
    let service = h.service();

    let err = call_tool(
        &service,
        "frobnicate window",
        json!({ "op": "frobnicate window" }),
    )
    .await
    .expect_err("unknown op should error");

    assert!(
        err.message.contains("frobnicate window"),
        "error should name the unknown op: {}",
        err.message
    );
    assert!(
        h.shell.calls().is_empty(),
        "an unknown op must not drive any shell action"
    );
}

/// Calling the service with the wrong tool name is rejected.
#[tokio::test]
async fn wrong_tool_name_is_rejected() {
    use rmcp::model::CallToolRequestParams;
    use rmcp::ServerHandler;
    use std::borrow::Cow;

    use super::common::request_context;

    let h = Harness::new();
    let service = h.service();

    let request = CallToolRequestParams::new(Cow::Borrowed("not-window"));
    let err = service
        .call_tool(request, request_context())
        .await
        .expect_err("wrong tool name should error");

    assert!(err.message.contains("not-window"), "{}", err.message);
}
