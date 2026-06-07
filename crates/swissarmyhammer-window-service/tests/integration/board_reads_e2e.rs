//! End-to-end tests for the `window` MCP server's board-management reads.
//!
//! `list open boards` and `get board data` ride the `window` server alongside
//! the board-lifecycle writes (the server already owns the open/close/new/switch
//! board lifecycle). Each test drives the verb through the real `ServerHandler` /
//! `call_tool` path against a recording `SpyShell` and asserts both the
//! structured response and the recorded shell call — mirroring the
//! board-lifecycle dispatch tests.

use serde_json::json;

use super::common::{call_tool, Harness, SpyShell};

/// Build a `Harness` whose spy returns the given canned board reads.
fn harness_with_board_reads(
    open_boards: serde_json::Value,
    board_data: serde_json::Value,
) -> Harness {
    let base = Harness::new();
    let positions = base.shell.positions.clone();
    let spy = SpyShell::new(
        base.shell.new_window.clone(),
        positions,
        base.shell.monitors.clone(),
    )
    .with_board_reads(open_boards, board_data);
    Harness::with_shell(spy)
}

/// `list open boards` routes through the shell's `list_open_boards` and wraps
/// the canned board array under `boards` — the same data the original
/// `list_open_boards` Tauri command returned.
#[tokio::test]
async fn list_open_boards_wraps_shell_array() {
    let boards = json!([
        { "path": "/a/.kanban", "name": "A", "is_active": true },
        { "path": "/b/.kanban", "name": "B", "is_active": false },
    ]);
    let h = harness_with_board_reads(boards.clone(), json!({}));
    let service = h.service();

    let res = call_tool(
        &service,
        "list open boards",
        json!({ "op": "list open boards" }),
    )
    .await
    .expect("list open boards should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["boards"], boards);
    assert_eq!(h.shell.calls(), vec!["list_open_boards".to_string()]);
}

/// `get board data` merges the shell's projection into the envelope and forwards
/// the `board_path` argument through to the shell.
#[tokio::test]
async fn get_board_data_merges_projection_and_forwards_path() {
    let data = json!({
        "board": { "name": "A" },
        "columns": [],
        "tags": [],
        "virtual_tag_meta": [],
        "summary": { "total_tasks": 0 },
    });
    let h = harness_with_board_reads(json!([]), data.clone());
    let service = h.service();

    let res = call_tool(
        &service,
        "get board data",
        json!({ "op": "get board data", "board_path": "/a/.kanban" }),
    )
    .await
    .expect("get board data should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["board"], data["board"]);
    assert_eq!(res["summary"], data["summary"]);
    assert_eq!(h.shell.calls(), vec!["get_board_data".to_string()]);
    assert_eq!(h.shell.last_board_path(), Some("/a/.kanban".to_string()));
}

/// `get board data` with no `board_path` forwards `None` — the shell's
/// resolve-active branch, matching the original command's `resolve_handle(None)`.
#[tokio::test]
async fn get_board_data_without_path_forwards_none() {
    let h = harness_with_board_reads(json!([]), json!({ "board": {} }));
    let service = h.service();

    call_tool(
        &service,
        "get board data",
        json!({ "op": "get board data" }),
    )
    .await
    .expect("get board data should succeed");

    assert_eq!(h.shell.last_board_path(), None);
}
