//! End-to-end tests for the `window` MCP server's board-lifecycle verbs.
//!
//! Two layers are covered:
//!
//! - **Service dispatch** (via `SpyShell`): each of `switch board`, `close
//!   board`, `new board`, and `open board` drives the matching `WindowShell`
//!   method with the right argument and shapes the right structured response —
//!   including the cancelled-picker (`opened: false`) path for `open board`.
//! - **Ported dialog file/IO** (via `run_new_board` / `run_open_board` against
//!   the injectable picker shim): `new board` initializes a board on disk at the
//!   chosen folder's `.kanban` directory, and `open board` resolves the chosen
//!   folder to its `.kanban` path. These exercise the relocated
//!   `new_board_dialog` / `open_board_dialog` logic without a native dialog.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::json;
use tempfile::tempdir;

use super::common::{call_tool, Harness, SpyShell};
use swissarmyhammer_window_service::{
    run_new_board, run_open_board, CreatedBoard, OpenedBoard, PickFolderFn,
};

// ── Service dispatch (spy) ───────────────────────────────────────────────

/// `switch board` drives `AppState`-backed board switch through the shell with
/// the requested path and echoes it back.
#[tokio::test]
async fn switch_board_drives_shell_with_path() {
    let h = Harness::new();
    let service = h.service();

    let res = call_tool(
        &service,
        "switch board",
        json!({ "op": "switch board", "path": "/tmp/board-a" }),
    )
    .await
    .expect("switch board should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["path"], json!("/tmp/board-a"));
    assert_eq!(h.shell.calls(), vec!["switch_board:/tmp/board-a"]);
}

/// `close board` drives `AppState`-backed board close through the shell with
/// the requested path and echoes it back.
#[tokio::test]
async fn close_board_drives_shell_with_path() {
    let h = Harness::new();
    let service = h.service();

    let res = call_tool(
        &service,
        "close board",
        json!({ "op": "close board", "path": "/tmp/board-b" }),
    )
    .await
    .expect("close board should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["path"], json!("/tmp/board-b"));
    assert_eq!(h.shell.calls(), vec!["close_board:/tmp/board-b"]);
}

/// `new board` runs the dialog path through the shell and returns the created
/// board's resolved path and name.
#[tokio::test]
async fn new_board_returns_created_board() {
    let h = Harness::new();
    let service = h.service();

    let res = call_tool(&service, "new board", json!({ "op": "new board" }))
        .await
        .expect("new board should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["path"], json!("/tmp/new-board"));
    assert_eq!(res["name"], json!("New Board"));
    assert_eq!(h.shell.calls(), vec!["new_board"]);
}

/// `open board` with a picker that resolved to a folder reports the opened
/// board's path and `opened: true`.
#[tokio::test]
async fn open_board_with_chosen_folder_reports_opened() {
    let h = Harness::new();
    let service = h.service();

    let res = call_tool(&service, "open board", json!({ "op": "open board" }))
        .await
        .expect("open board should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["opened"], json!(true));
    assert_eq!(res["path"], json!("/tmp/opened-board"));
    assert_eq!(h.shell.calls(), vec!["open_board"]);
}

/// `open board` with a cancelled picker is a success with `opened: false` and a
/// null path — the user declined to open anything, which is not an error.
#[tokio::test]
async fn open_board_cancelled_reports_not_opened() {
    let h = Harness::with_shell(
        SpyShell::new(
            swissarmyhammer_window_service::NewWindow {
                label: "x".to_string(),
                board_path: None,
            },
            Default::default(),
            Vec::new(),
        )
        .with_open_board(None),
    );
    let service = h.service();

    let res = call_tool(&service, "open board", json!({ "op": "open board" }))
        .await
        .expect("open board (cancel) should still succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["opened"], json!(false));
    assert_eq!(res["path"], json!(null));
    assert_eq!(h.shell.calls(), vec!["open_board"]);
}

// ── Ported dialog file/IO (picker shim) ──────────────────────────────────

/// A picker shim that always returns the given path, modelling "user chose this
/// folder".
fn picker_returning(path: PathBuf) -> PickFolderFn {
    Arc::new(move || Some(path.clone()))
}

/// A picker shim that always returns `None`, modelling "user cancelled".
fn picker_cancelled() -> PickFolderFn {
    Arc::new(|| None)
}

/// `run_new_board` resolves the chosen folder to its `.kanban` directory, runs
/// the init callback against that path, and reports the derived name. The init
/// callback creates the board directory on disk, which we assert exists.
#[test]
fn run_new_board_initializes_board_on_disk() {
    let dir = tempdir().expect("tempdir");
    let folder = dir.path().join("my-project");
    std::fs::create_dir(&folder).expect("create project folder");

    let init_calls: Arc<Mutex<Vec<(PathBuf, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&init_calls);

    let result = run_new_board(&picker_returning(folder.clone()), |kanban_path, name| {
        recorded
            .lock()
            .unwrap()
            .push((kanban_path.to_path_buf(), name.to_string()));
        // Simulate the kanban `InitBoard` step by creating the .kanban dir.
        std::fs::create_dir_all(kanban_path).map_err(|e| e.to_string())
    })
    .expect("run_new_board should succeed");

    let (created, chosen_folder): (CreatedBoard, PathBuf) =
        result.expect("picker chose a folder, so a board is created");

    // Name derives from the chosen folder's final component.
    assert_eq!(created.name, "my-project");
    // The resolved board path is the `.kanban` child of the chosen folder.
    let expected_kanban = std::fs::canonicalize(&folder).unwrap().join(".kanban");
    assert_eq!(PathBuf::from(&created.path), expected_kanban);
    // The init callback was invoked once with the resolved path + name.
    let calls = init_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, expected_kanban);
    assert_eq!(calls[0].1, "my-project");
    // The board directory now exists on disk.
    assert!(expected_kanban.is_dir(), "board .kanban dir created on disk");
    // The originally-chosen folder is threaded back for the AppState open seam.
    assert_eq!(chosen_folder, folder);
}

/// `run_new_board` with a cancelled picker creates nothing and never runs the
/// init callback.
#[test]
fn run_new_board_cancelled_does_nothing() {
    let init_ran = Arc::new(Mutex::new(false));
    let flag = Arc::clone(&init_ran);

    let result = run_new_board(&picker_cancelled(), |_, _| {
        *flag.lock().unwrap() = true;
        Ok(())
    })
    .expect("cancelled new board is not an error");

    assert!(result.is_none(), "cancelled picker creates no board");
    assert!(!*init_ran.lock().unwrap(), "init must not run on cancel");
}

/// `run_open_board` resolves the chosen existing board folder to its `.kanban`
/// directory.
#[test]
fn run_open_board_resolves_chosen_folder() {
    let dir = tempdir().expect("tempdir");
    let folder = dir.path().join("existing");
    let kanban = folder.join(".kanban");
    std::fs::create_dir_all(&kanban).expect("create existing .kanban dir");

    let result = run_open_board(&picker_returning(folder.clone()))
        .expect("run_open_board should succeed");

    let (opened, chosen_folder): (OpenedBoard, PathBuf) =
        result.expect("picker chose a folder, so a board is opened");

    let expected_kanban = std::fs::canonicalize(&folder).unwrap().join(".kanban");
    assert_eq!(PathBuf::from(&opened.path), expected_kanban);
    assert_eq!(chosen_folder, folder);
}

/// `run_open_board` with a cancelled picker resolves to `None`.
#[test]
fn run_open_board_cancelled_returns_none() {
    let result = run_open_board(&picker_cancelled()).expect("cancel is not an error");
    assert!(result.is_none(), "cancelled picker opens no board");
}
