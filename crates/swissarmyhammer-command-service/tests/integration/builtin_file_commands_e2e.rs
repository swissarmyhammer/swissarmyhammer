//! End-to-end test for the committed `file-commands` builtin plugin.
//!
//! This is the acceptance for the port of `file.yaml` (the four board-file
//! lifecycle commands `file.switchBoard`, `file.closeBoard`, `file.newBoard`,
//! `file.openBoard`) into the one `builtin/plugins/file-commands/` bundle. It
//! mirrors `builtin_kanban_misc_e2e` exactly, but exposes a SINGLE backend —
//! the in-process `window` operation tool, wrapped over a recording
//! `WindowShell` (`BoardShell`) — because all four commands route to the
//! window server's board-lifecycle verbs:
//!
//! - `file.switchBoard` → window `switch board`;
//! - `file.closeBoard`  → window `close board`;
//! - `file.newBoard`    → window `new board`  (the folder-picker shim);
//! - `file.openBoard`   → window `open board` (the OS file-open dialog shim).
//!
//! The `window` `ServerHandler` is wrapped in an [`InProcessServer`] to satisfy
//! `expose_rust_module`'s `McpServer` contract.
//!
//! What a passing run proves:
//!
//! 1. **Discovery + registration** — after load, the four ported commands are
//!    registered.
//! 2. **Metadata fidelity** — each command's `undoable` / `keys` / `menu`
//!    match the source-YAML baseline 1:1.
//! 3. **Real effect** — executing `file.newBoard` drives the recorded shell's
//!    `new_board` callback (the board-creation effect), and the execute
//!    envelope carries the created board's path / name back from the picker
//!    shim. The other three verbs' shell calls are likewise recorded.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_directory::KanbanConfig;
use swissarmyhammer_plugin::{
    CallerId, InProcessServer, McpServer as PluginMcpServer, PluginHost, PLUGINS_SUBDIR,
};
use swissarmyhammer_window_service::{
    ContextMenuItem, CreatedBoard, MonitorInfo, NewWindow, OpenedBoard, WindowPosition,
    WindowService, WindowShell,
};
use tempfile::TempDir;

use crate::support::call_command;

/// A generous upper bound on any single host or isolate interaction.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// The path the `new_board` picker shim resolves to in this test.
const NEW_BOARD_PATH: &str = "/tmp/file-commands-new-board/.kanban";
/// The board name the `new_board` picker shim derives.
const NEW_BOARD_NAME: &str = "file-commands-new-board";

// ───────────────────────────────────────────────────────────────────────────
// Staging the committed builtin bundle
// ───────────────────────────────────────────────────────────────────────────

/// Resolve the workspace root (two levels above this crate's manifest dir).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root is two levels above the crate manifest dir")
        .to_path_buf()
}

/// Recursively copy a directory tree from `source` to `destination`.
fn copy_dir_recursive(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("staging directory should be created");
    for entry in std::fs::read_dir(source).expect("bundle dir should be readable") {
        let entry = entry.expect("a directory entry should be readable");
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to);
        } else {
            std::fs::copy(&from, &to).expect("bundle file should copy");
        }
    }
}

/// Stage the committed `builtin/plugins/file-commands` bundle into a temp
/// builtin-layer root so `discover_and_load_all` finds it at
/// `<layer_root>/plugins/file-commands/`.
fn stage_file_commands(layer_root: &Path) {
    let source = workspace_root()
        .join("builtin/plugins")
        .join("file-commands");
    assert!(
        source.is_dir(),
        "the committed file-commands bundle must exist at {}",
        source.display()
    );
    let destination = layer_root.join(PLUGINS_SUBDIR).join("file-commands");
    copy_dir_recursive(&source, &destination);
}

// ───────────────────────────────────────────────────────────────────────────
// Exposing the real in-process `window` tool over a recording board shell
// ───────────────────────────────────────────────────────────────────────────

/// A recording [`WindowShell`] that captures the four board-lifecycle calls so
/// the test can assert the ported `file.*` commands reached the OS / picker
/// actions with the expected arguments. The `new_board` / `open_board` picker
/// shims resolve to fixed paths so `file.newBoard` (the board-creation effect)
/// is observable without a native dialog. Every other shell method is an inert
/// stub: the file-commands plugin only drives the board-lifecycle verbs.
struct BoardShell {
    /// Ordered log of `<method>:<arg>` tags, one per board-lifecycle call.
    calls: Mutex<Vec<String>>,
}

impl BoardShell {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, tag: impl Into<String>) {
        self.calls.lock().unwrap().push(tag.into());
    }
}

impl WindowShell for BoardShell {
    fn open_new_window(&self, _board_path: Option<String>) -> Result<NewWindow, String> {
        Ok(NewWindow {
            label: "unused".to_string(),
            board_path: None,
        })
    }

    fn activate_window(&self, _label: &str) -> Result<(), String> {
        Ok(())
    }

    fn set_window_position(&self, _label: &str, _position: WindowPosition) -> Result<(), String> {
        Ok(())
    }

    fn get_window_position(&self, _label: &str) -> Result<WindowPosition, String> {
        Ok(WindowPosition { x: 0, y: 0 })
    }

    fn get_monitors(&self) -> Result<Vec<MonitorInfo>, String> {
        Ok(Vec::new())
    }

    fn close_window(&self, _label: &str) -> Result<(), String> {
        Ok(())
    }

    fn open_path(&self, _path: &str) -> Result<(), String> {
        Ok(())
    }

    fn reveal_path(&self, _path: &str) -> Result<(), String> {
        Ok(())
    }

    fn switch_board(&self, path: &str) -> Result<(), String> {
        self.record(format!("switch_board:{path}"));
        Ok(())
    }

    fn close_board(&self, path: &str) -> Result<(), String> {
        self.record(format!("close_board:{path}"));
        Ok(())
    }

    fn new_board(&self) -> Result<CreatedBoard, String> {
        self.record("new_board");
        Ok(CreatedBoard {
            path: NEW_BOARD_PATH.to_string(),
            name: NEW_BOARD_NAME.to_string(),
        })
    }

    fn open_board(&self) -> Result<Option<OpenedBoard>, String> {
        self.record("open_board");
        Ok(Some(OpenedBoard {
            path: "/tmp/file-commands-opened-board/.kanban".to_string(),
        }))
    }

    fn show_context_menu(
        &self,
        _items: Vec<ContextMenuItem>,
        _window_label: Option<String>,
    ) -> Result<(), String> {
        Ok(())
    }
}

/// Expose the `window` tool (over the recording board shell) to `host` under id
/// `"window"`, returning the shared spy so the test can read back the recorded
/// calls.
async fn expose_window_module(host: &PluginHost) -> Arc<BoardShell> {
    let shell = Arc::new(BoardShell::new());
    let service = WindowService::new(Arc::clone(&shell) as Arc<dyn WindowShell>);
    let module = InProcessServer::new(service)
        .await
        .expect("wrapping the window service in an InProcessServer should succeed");
    host.expose_rust_module(
        "window".to_string(),
        Arc::new(module) as Arc<dyn PluginMcpServer>,
    )
    .await
    .expect("exposing the window module should succeed");
    shell
}

// ───────────────────────────────────────────────────────────────────────────
// Result-shape helpers
// ───────────────────────────────────────────────────────────────────────────

/// Pull the `commands` array out of a `list command` response, keyed by id.
fn commands_by_id(list_result: &Value) -> BTreeMap<String, Value> {
    list_result
        .get("structuredContent")
        .and_then(|sc| sc.get("commands"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|cmd| {
            let id = cmd.get("id").and_then(Value::as_str)?.to_string();
            Some((id, cmd))
        })
        .collect()
}

// ───────────────────────────────────────────────────────────────────────────
// The test
// ───────────────────────────────────────────────────────────────────────────

/// The committed `file-commands` builtin plugin, discovered from a builtin
/// layer, registers all four `file.yaml` commands with 1:1 metadata and routes
/// each to the `window` board-lifecycle verbs, with `file.newBoard` driving the
/// recorded board-creation effect.
#[tokio::test]
async fn file_commands_plugin_registers_and_executes() {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");

    // Stage the committed bundle into the builtin layer's plugins/ dir.
    stage_file_commands(builtin_root.path());

    // A host whose lowest-precedence builtin layer is the staged root.
    let host = PluginHost::new(
        Some(builtin_root.path().to_path_buf()),
        user_root.path().to_path_buf(),
        None,
        false,
        user_root.path().to_path_buf(),
    );

    // Bootstrap the command service into the host (exposes `commands`).
    let service = install_commands_module(&host)
        .await
        .expect("install_commands_module must succeed");

    // Expose the window backend BEFORE discovery, so the plugin's
    // `ensureServices(this, ["commands", "window"])` finds it already exposed.
    let window_spy = tokio::time::timeout(TIMEOUT, expose_window_module(&host))
        .await
        .expect("exposing window should not hang");

    // Discover + load the builtin layer: runs the bundle's `load()`, which
    // registers the four commands through the SDK convention.
    let loaded = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the file-commands builtin plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one file-commands builtin plugin should be discovered, got {loaded:?}"
    );

    // ── (1) Discovery + registration: list every command ───────────────────
    let listed = call_command(
        &service,
        CallerId::HostInternal,
        json!({ "op": "list command" }),
    )
    .await;
    let commands = commands_by_id(&listed);
    for id in [
        "file.switchBoard",
        "file.closeBoard",
        "file.newBoard",
        "file.openBoard",
    ] {
        assert!(
            commands.contains_key(id),
            "list command must include the ported command {id:?}; got {:?}",
            commands.keys().collect::<Vec<_>>()
        );
    }

    // ── (2) Metadata fidelity: lock each command's metadata 1:1 vs file.yaml ─
    assert_switch_board_metadata(&commands["file.switchBoard"]);
    assert_close_board_metadata(&commands["file.closeBoard"]);
    assert_new_board_metadata(&commands["file.newBoard"]);
    assert_open_board_metadata(&commands["file.openBoard"]);

    // ── (3) Real effect: file.newBoard → window `new board` ────────────────
    // `file.newBoard` only touches the host (picker shim → board creation), so
    // it is the command the card asks the test to exercise: executing it must
    // drive the shell's `new_board` callback and surface the created board's
    // path / name back through the execute envelope.
    let new_board = call_command(
        &service,
        CallerId::HostInternal,
        json!({ "op": "execute command", "id": "file.newBoard" }),
    )
    .await;
    assert_eq!(
        new_board["structuredContent"]["ok"],
        json!(true),
        "executing file.newBoard should succeed, got {new_board}"
    );
    // The execute envelope is `{ ok, result: <plugin return> }`; the plugin's
    // single `window new board` call returns that backend's `CallToolResult`
    // (`{ content, structuredContent: { ok, path, name }, isError }`). The
    // created board's path / name therefore live under
    // `structuredContent.result.structuredContent.{path,name}`.
    let created = &new_board["structuredContent"]["result"]["structuredContent"];
    assert_eq!(
        created["path"],
        json!(NEW_BOARD_PATH),
        "file.newBoard must surface the created board's path from the picker shim, got {new_board}"
    );
    assert_eq!(
        created["name"],
        json!(NEW_BOARD_NAME),
        "file.newBoard must surface the created board's name, got {new_board}"
    );

    // ── (3b) The other three verbs route to their board-lifecycle calls ─────
    let switch_path = "/tmp/file-commands-switch/.kanban";
    let switch = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "file.switchBoard",
            "ctx": { "args": { "path": switch_path } },
        }),
    )
    .await;
    assert_eq!(
        switch["structuredContent"]["ok"],
        json!(true),
        "executing file.switchBoard should succeed, got {switch}"
    );

    let close_path = "/tmp/file-commands-close/.kanban";
    let close = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "file.closeBoard",
            "ctx": { "args": { "path": close_path } },
        }),
    )
    .await;
    assert_eq!(
        close["structuredContent"]["ok"],
        json!(true),
        "executing file.closeBoard should succeed, got {close}"
    );

    let open = call_command(
        &service,
        CallerId::HostInternal,
        json!({ "op": "execute command", "id": "file.openBoard" }),
    )
    .await;
    assert_eq!(
        open["structuredContent"]["ok"],
        json!(true),
        "executing file.openBoard should succeed, got {open}"
    );

    // The recorded shell must show every board-lifecycle call in order, with
    // the threaded paths for switch / close and the picker invocations for
    // new / open.
    assert_eq!(
        window_spy.calls(),
        vec![
            "new_board".to_string(),
            format!("switch_board:{switch_path}"),
            format!("close_board:{close_path}"),
            "open_board".to_string(),
        ],
        "the four file.* commands must drive the window shell's board-lifecycle calls"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// Per-command metadata regression asserts (locked against file.yaml)
// ───────────────────────────────────────────────────────────────────────────

/// Assert a command carries no scope / params / context_menu — the file.yaml
/// commands are all bare board-file actions with no entity scope.
fn assert_no_scope_or_params(cmd: &Value, id: &str) {
    assert!(
        cmd.get("scope").is_none() || cmd["scope"].is_null() || cmd["scope"] == json!([]),
        "{id} carries no scope, got {}",
        cmd["scope"]
    );
    assert!(
        cmd.get("params").is_none() || cmd["params"] == json!([]),
        "{id} carries no params, got {}",
        cmd["params"]
    );
    assert!(
        cmd.get("context_menu").is_none() || cmd["context_menu"] == json!(false),
        "{id} carries no context_menu"
    );
}

/// `file.switchBoard` — file.yaml: undoable:false, no keys/menu.
fn assert_switch_board_metadata(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Switch Board"), "file.switchBoard name");
    assert_eq!(
        cmd["undoable"],
        json!(false),
        "file.switchBoard undoable:false"
    );
    assert!(
        cmd.get("keys").is_none() || cmd["keys"] == json!({}),
        "file.switchBoard carries no keys, got {}",
        cmd["keys"]
    );
    assert!(
        cmd.get("menu").is_none() || cmd["menu"].is_null(),
        "file.switchBoard carries no menu, got {}",
        cmd["menu"]
    );
    assert_no_scope_or_params(cmd, "file.switchBoard");
}

/// `file.closeBoard` — file.yaml: undoable:false, keys cua/vim Mod+W, menu
/// File/0/2.
fn assert_close_board_metadata(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Close Board"), "file.closeBoard name");
    assert_eq!(
        cmd["undoable"],
        json!(false),
        "file.closeBoard undoable:false"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+W", "vim": "Mod+W" }),
        "file.closeBoard keys"
    );
    assert_eq!(
        cmd["menu"],
        json!({ "path": ["File"], "group": 0, "order": 2 }),
        "file.closeBoard menu"
    );
    assert_no_scope_or_params(cmd, "file.closeBoard");
}

/// `file.newBoard` — file.yaml: undoable:false, keys cua Mod+Shift+B, menu
/// File/0/0.
fn assert_new_board_metadata(cmd: &Value) {
    assert_eq!(cmd["name"], json!("New Board"), "file.newBoard name");
    assert_eq!(
        cmd["undoable"],
        json!(false),
        "file.newBoard undoable:false"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+Shift+B" }),
        "file.newBoard keys"
    );
    assert_eq!(
        cmd["menu"],
        json!({ "path": ["File"], "group": 0, "order": 0 }),
        "file.newBoard menu"
    );
    assert_no_scope_or_params(cmd, "file.newBoard");
}

/// `file.openBoard` — file.yaml: undoable:false, keys cua Mod+O, menu
/// File/0/1.
fn assert_open_board_metadata(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Open Board"), "file.openBoard name");
    assert_eq!(
        cmd["undoable"],
        json!(false),
        "file.openBoard undoable:false"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+O" }),
        "file.openBoard keys"
    );
    assert_eq!(
        cmd["menu"],
        json!({ "path": ["File"], "group": 0, "order": 1 }),
        "file.openBoard menu"
    );
    assert_no_scope_or_params(cmd, "file.openBoard");
}
