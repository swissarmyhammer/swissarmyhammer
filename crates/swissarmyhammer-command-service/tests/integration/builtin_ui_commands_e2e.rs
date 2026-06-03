//! End-to-end test for the committed `ui-commands` builtin plugin.
//!
//! This is the acceptance for the port of `ui.yaml` — 10 commands — into the
//! one `builtin/plugins/ui-commands/` bundle. It is the LAST builtin-commands
//! port, and like `app-shell-commands` every command fans out across MULTIPLE
//! backends by concern — but here the three backends are `ui_state`, `focus`,
//! and `window`:
//!
//!   - inspector / palette / mode / rename / inspect — `ui.inspect`,
//!     `ui.inspector.{close,close_all,set_width}`, `ui.palette.{open,close}`,
//!     `ui.entity.startRename`, `ui.mode.set` — route to the `ui_state` server
//!     (`swissarmyhammer-ui-state::UiStateServer` over a temp-file `UIState`),
//!     exposed under id `"ui_state"`.
//!   - `ui.setFocus` → the `focus` server
//!     (`swissarmyhammer-focus::FocusServer` over a real `SpatialRegistry` /
//!     `SpatialState`), exposed under id `"focus"`.
//!   - `window.new` → the `window` server
//!     (`swissarmyhammer-window-service::WindowService` over a recording spy
//!     `WindowShell`), exposed under id `"window"`.
//!
//! What a passing run proves:
//!
//! 1. **Discovery + registration** — after load, all 10 ported commands are
//!    registered.
//! 2. **Metadata fidelity** — each command's `name` / `keys` / `menu` /
//!    `scope` / `context_menu*` / `visible` / `undoable` / `params` match the
//!    `ui.yaml` baseline 1:1 (table-test).
//! 3. **Real effects** —
//!    - `ui.inspect` pushes the target moniker onto the `UIState` inspector
//!      stack, `ui.inspector.close` pops it, `ui.inspector.close_all` clears it,
//!      `ui.inspector.set_width` persists the width.
//!    - `ui.palette.open` flips the palette-open flag, `ui.palette.close`
//!      clears it.
//!    - `ui.mode.set` switches the active keymap mode.
//!    - `ui.entity.startRename` reaches the backend no-op (`{ ok: true }`).
//!    - `ui.setFocus` changes the `SpatialState` focused slot via the `focus`
//!      server.
//!    - `window.new` hits the recording `WindowShell` spy.
//! 4. **Regression (`no-client-side-inspect`)** — `ui.inspect` goes through the
//!    Command service into the `ui_state` backend, NOT a React-side shortcut:
//!    the inspector stack mutation is observed on the shared `UIState`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_directory::KanbanConfig;
use swissarmyhammer_focus::{
    FocusLayer, FocusServer, FullyQualifiedMoniker, LayerName, SegmentMoniker, SpatialState,
    WindowLabel,
};
use swissarmyhammer_plugin::{
    CallerId, InProcessServer, McpServer as PluginMcpServer, PluginHost, PLUGINS_SUBDIR,
};
use swissarmyhammer_ui_state::{UIState, UiStateServer};
use swissarmyhammer_window_service::{
    ContextMenuItem, CreatedBoard, MonitorInfo, NewWindow, OpenedBoard, WindowPosition,
    WindowService, WindowShell,
};
use tempfile::TempDir;
use tokio::sync::Mutex as TokioMutex;

use crate::support::{call_command, execute_result, try_call_command};

/// A generous upper bound on any single host or isolate interaction.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// The window the ui commands operate on throughout the test.
const WINDOW: &str = "main";

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

/// Stage the committed `builtin/plugins/ui-commands` bundle into a temp builtin
/// layer root so `discover_and_load_all` finds it at
/// `<layer_root>/plugins/ui-commands/`.
fn stage_ui_commands(layer_root: &Path) {
    let source = workspace_root().join("builtin/plugins").join("ui-commands");
    assert!(
        source.is_dir(),
        "the committed ui-commands bundle must exist at {}",
        source.display()
    );
    let destination = layer_root.join(PLUGINS_SUBDIR).join("ui-commands");
    copy_dir_recursive(&source, &destination);
}

// ───────────────────────────────────────────────────────────────────────────
// The `window` backend: a recording spy WindowShell
// ───────────────────────────────────────────────────────────────────────────

/// A recording [`WindowShell`] that captures `open_new_window` so the test can
/// assert the ported `window.new` command reached the window-manager action.
/// Every other shell method is an inert stub.
struct SpyShell {
    /// Ordered log of `<method>` tags, one per call.
    calls: Mutex<Vec<String>>,
}

impl SpyShell {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl WindowShell for SpyShell {
    fn open_new_window(&self, board_path: Option<String>) -> Result<NewWindow, String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("open_new_window:{board_path:?}"));
        Ok(NewWindow {
            label: "window-2".to_string(),
            board_path,
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

    fn switch_board(&self, _path: &str) -> Result<(), String> {
        Ok(())
    }

    fn close_board(&self, _path: &str) -> Result<(), String> {
        Ok(())
    }

    fn new_board(&self) -> Result<CreatedBoard, String> {
        Ok(CreatedBoard {
            path: String::new(),
            name: String::new(),
        })
    }

    fn open_board(&self) -> Result<Option<OpenedBoard>, String> {
        Ok(None)
    }

    fn show_context_menu(
        &self,
        _items: Vec<ContextMenuItem>,
        _window_label: Option<String>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn list_open_boards(&self) -> Result<Value, String> {
        Ok(json!([]))
    }

    fn get_board_data(&self, _board_path: Option<String>) -> Result<Value, String> {
        Ok(json!({}))
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Exposing the three in-process backends
// ───────────────────────────────────────────────────────────────────────────

/// A handle to every live backend, kept alive for the test's duration so the
/// shared kernels outlive the plugin's `load()` and every `execute`.
struct ExposedBackends {
    _dir: TempDir,
    /// The shared UI state the ui_state-routed commands mutate.
    ui_state: Arc<UIState>,
    /// The focus kernel's spatial state, read back to assert `ui.setFocus`.
    spatial_state: Arc<TokioMutex<SpatialState>>,
    /// The recording window shell `window.new` hits.
    shell: Arc<SpyShell>,
}

/// Build the `ui_state`, `focus`, and `window` backends and expose all three to
/// `host` under their public ids. Seeds a window-root layer on the focus kernel
/// so `ui.setFocus` can resolve the owning window from the snapshot's layer.
async fn expose_backends(host: &PluginHost) -> ExposedBackends {
    let dir = TempDir::new().expect("backend substrate temp dir");

    // --- ui_state server over a temp-file-backed UIState ---
    let ui_state = Arc::new(UIState::load(dir.path().join("ui_state.yaml")));
    let ui_state_server = UiStateServer::new(Arc::clone(&ui_state));
    let ui_state_module = InProcessServer::new(ui_state_server)
        .await
        .expect("wrapping the ui_state server in an InProcessServer should succeed");
    host.expose_rust_module(
        "ui_state".to_string(),
        Arc::new(ui_state_module) as Arc<dyn PluginMcpServer>,
    )
    .await
    .expect("exposing the ui_state module should succeed");

    // --- focus server over a real SpatialRegistry / SpatialState ---
    let focus_server = FocusServer::new();
    let spatial_registry = focus_server.registry();
    let spatial_state = focus_server.state();
    // Seed a window-root layer `/L` owned by WINDOW so `set focus` can derive
    // the owning window from the snapshot's layer (exactly as `push layer` does
    // over the wire). The ui.setFocus snapshot below references this layer.
    {
        let mut registry = spatial_registry.lock().await;
        registry.push_layer(FocusLayer {
            fq: FullyQualifiedMoniker::from_string("/L"),
            segment: SegmentMoniker::from_string("window"),
            name: LayerName::from_string("window"),
            parent: None,
            window_label: WindowLabel::from_string(WINDOW),
            last_focused: None,
        });
    }
    let focus_module = InProcessServer::new(focus_server)
        .await
        .expect("wrapping the focus server in an InProcessServer should succeed");
    host.expose_rust_module(
        "focus".to_string(),
        Arc::new(focus_module) as Arc<dyn PluginMcpServer>,
    )
    .await
    .expect("exposing the focus module should succeed");

    // --- window server over a recording spy WindowShell ---
    let shell = Arc::new(SpyShell::new());
    let window_service = WindowService::new(Arc::clone(&shell) as Arc<dyn WindowShell>);
    let window_module = InProcessServer::new(window_service)
        .await
        .expect("wrapping the window service in an InProcessServer should succeed");
    host.expose_rust_module(
        "window".to_string(),
        Arc::new(window_module) as Arc<dyn PluginMcpServer>,
    )
    .await
    .expect("exposing the window module should succeed");

    ExposedBackends {
        _dir: dir,
        ui_state,
        spatial_state,
        shell,
    }
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

/// Execute a command by id with the given `ctx` payload and assert it succeeded.
/// Returns the inner backend result (`structuredContent.result`).
async fn execute_ok(
    service: &swissarmyhammer_command_service::CommandService,
    id: &str,
    ctx: Value,
) -> Value {
    let resp = try_call_command(
        service,
        CallerId::HostInternal,
        json!({ "op": "execute command", "id": id, "ctx": ctx }),
    )
    .await
    .unwrap_or_else(|e| panic!("executing {id} raised: {e:?}"));
    assert_eq!(
        resp["structuredContent"]["ok"],
        json!(true),
        "executing {id} should succeed, got {resp}"
    );
    execute_result(&resp)
}

/// A single-scope snapshot under layer `/L` containing `fq` at a 10x10 rect —
/// the minimal shape `set focus` needs to commit (mirrors the focus e2e).
fn snapshot_one(fq: &str) -> Value {
    json!({
        "layer_fq": "/L",
        "scopes": [
            { "fq": fq, "rect": { "x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0 },
              "parent_zone": null, "nav_override": {} }
        ]
    })
}

// ───────────────────────────────────────────────────────────────────────────
// The test
// ───────────────────────────────────────────────────────────────────────────

/// The committed `ui-commands` builtin plugin, discovered from a builtin layer,
/// registers all 10 commands with 1:1 metadata and produces each command's real
/// effect against the three live backends.
#[tokio::test]
async fn ui_commands_plugin_registers_and_executes() {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");

    stage_ui_commands(builtin_root.path());

    let host = PluginHost::new(
        Some(builtin_root.path().to_path_buf()),
        user_root.path().to_path_buf(),
        None,
        user_root.path().to_path_buf(),
        false,
        user_root.path().to_path_buf(),
    );

    let service = install_commands_module(&host)
        .await
        .expect("install_commands_module must succeed");

    // Expose all three backends BEFORE discovery, so the plugin's
    // `ensureServices(this, ["commands", "ui_state", "window", "focus"])` finds
    // them already exposed.
    let backends = tokio::time::timeout(TIMEOUT, expose_backends(&host))
        .await
        .expect("exposing backends should not hang");

    let loaded = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the ui-commands builtin plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one ui-commands builtin plugin should be discovered, got {loaded:?}"
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
        "ui.inspect",
        "ui.inspector.close",
        "ui.inspector.close_all",
        "ui.inspector.set_width",
        "ui.palette.open",
        "ui.palette.close",
        "ui.entity.startRename",
        "ui.mode.set",
        "ui.setFocus",
        "window.new",
    ] {
        assert!(
            commands.contains_key(id),
            "list command must include the ported command {id:?}; got {:?}",
            commands.keys().collect::<Vec<_>>()
        );
    }
    assert_eq!(
        commands.len(),
        10,
        "exactly the 10 ported commands should be registered, got {:?}",
        commands.keys().collect::<Vec<_>>()
    );

    // ── (2) Metadata fidelity: lock each command's metadata 1:1 vs ui.yaml ──
    for (id, assert_fn) in metadata_asserts() {
        assert_fn(&commands[id]);
    }

    // ── (3a) ui.inspect pushes the target moniker onto the inspector stack ──
    // Regression (`no-client-side-inspect`): this goes via the Command service
    // into the ui_state backend, NOT a React-side shortcut — the mutation is
    // observed on the shared UIState.
    assert!(
        backends.ui_state.inspector_stack(WINDOW).is_empty(),
        "precondition: the inspector stack is empty before ui.inspect"
    );
    execute_ok(
        &service,
        "ui.inspect",
        json!({ "target": "task:01ABC", "args": { "window_label": WINDOW } }),
    )
    .await;
    assert_eq!(
        backends.ui_state.inspector_stack(WINDOW),
        vec!["task:01ABC".to_string()],
        "ui.inspect must push the target moniker onto the ui_state inspector stack"
    );

    // A second inspect deepens the stack.
    execute_ok(
        &service,
        "ui.inspect",
        json!({ "target": "tag:bug", "args": { "window_label": WINDOW } }),
    )
    .await;
    assert_eq!(
        backends.ui_state.inspector_stack(WINDOW),
        vec!["task:01ABC".to_string(), "tag:bug".to_string()],
        "a second ui.inspect deepens the inspector stack"
    );

    // ── (3b) ui.inspector.close pops the topmost entry ──────────────────────
    execute_ok(
        &service,
        "ui.inspector.close",
        json!({ "args": { "window_label": WINDOW } }),
    )
    .await;
    assert_eq!(
        backends.ui_state.inspector_stack(WINDOW),
        vec!["task:01ABC".to_string()],
        "ui.inspector.close must pop the topmost inspector entry"
    );

    // ── (3c) ui.inspector.close_all clears the stack ────────────────────────
    execute_ok(
        &service,
        "ui.inspector.close_all",
        json!({ "args": { "window_label": WINDOW } }),
    )
    .await;
    assert!(
        backends.ui_state.inspector_stack(WINDOW).is_empty(),
        "ui.inspector.close_all must clear the inspector stack"
    );

    // ── (3d) ui.inspector.set_width persists the width ──────────────────────
    execute_ok(
        &service,
        "ui.inspector.set_width",
        json!({ "args": { "window_label": WINDOW, "width": 480 } }),
    )
    .await;
    assert_eq!(
        backends.ui_state.inspector_width(WINDOW),
        Some(480),
        "ui.inspector.set_width must persist the inspector width on the UIState"
    );

    // ── (3e) ui.palette.open / ui.palette.close flip the palette flag ───────
    assert!(
        !backends.ui_state.palette_open(WINDOW),
        "precondition: the palette is closed before ui.palette.open"
    );
    execute_ok(
        &service,
        "ui.palette.open",
        json!({ "args": { "window_label": WINDOW } }),
    )
    .await;
    assert!(
        backends.ui_state.palette_open(WINDOW),
        "ui.palette.open must open the command palette on the UIState"
    );
    execute_ok(
        &service,
        "ui.palette.close",
        json!({ "args": { "window_label": WINDOW } }),
    )
    .await;
    assert!(
        !backends.ui_state.palette_open(WINDOW),
        "ui.palette.close must close the command palette on the UIState"
    );

    // ── (3f) ui.mode.set switches the active keymap mode ────────────────────
    assert_eq!(
        backends.ui_state.keymap_mode(),
        "cua",
        "precondition: the default keymap mode is cua"
    );
    execute_ok(
        &service,
        "ui.mode.set",
        json!({ "args": { "mode": "vim" } }),
    )
    .await;
    assert_eq!(
        backends.ui_state.keymap_mode(),
        "vim",
        "ui.mode.set must switch the active keymap mode to vim"
    );

    // ── (3g) ui.entity.startRename reaches the backend no-op ────────────────
    // StartRename is a backend no-op (the frontend intercepts the command
    // before it reaches the backend); reaching it through the Command service
    // into the ui_state backend is the signal — `execute_ok` already asserted
    // the envelope `ok: true`, which only succeeds if the ui_state dispatch
    // resolved.
    execute_ok(
        &service,
        "ui.entity.startRename",
        json!({ "args": { "window_label": WINDOW } }),
    )
    .await;

    // ── (3h) ui.setFocus changes the focus state via the focus server ───────
    assert!(
        backends
            .spatial_state
            .lock()
            .await
            .focused_in(&WindowLabel::from_string(WINDOW))
            .is_none(),
        "precondition: no focus slot before ui.setFocus"
    );
    let focus = execute_ok(
        &service,
        "ui.setFocus",
        json!({ "args": { "fq": "/L/k1", "snapshot": snapshot_one("/L/k1") } }),
    )
    .await;
    // The focus dispatch returns the raw MCP `tools/call` response; the focus
    // server's payload (`{ ok: true, event: <FocusChangedEvent> }`) is under
    // `structuredContent`.
    let focus_event = &focus["structuredContent"]["event"];
    assert_eq!(focus_event["window_label"], json!(WINDOW));
    assert_eq!(focus_event["next_fq"], json!("/L/k1"));
    // ...and the focus state is observably updated on the shared SpatialState.
    let focused = backends
        .spatial_state
        .lock()
        .await
        .focused_in(&WindowLabel::from_string(WINDOW))
        .map(|fq| fq.as_str().to_string());
    assert_eq!(
        focused,
        Some("/L/k1".to_string()),
        "ui.setFocus must commit the focus on the focus server's SpatialState"
    );

    // ── (3i) window.new hits the recording WindowShell spy ──────────────────
    assert!(
        backends.shell.calls().is_empty(),
        "precondition: no window-shell calls before window.new"
    );
    execute_ok(&service, "window.new", json!({})).await;
    assert_eq!(
        backends.shell.calls(),
        vec!["open_new_window:None".to_string()],
        "window.new must drive the window shell's open_new_window action"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// Per-command metadata regression asserts (locked against ui.yaml)
// ───────────────────────────────────────────────────────────────────────────

/// One row of the metadata-fidelity table: a command id and its assertion.
type MetadataAssert = (&'static str, fn(&Value));

/// The metadata-fidelity table: each ported command id paired with its
/// per-command assertion, exercised across all 10 in the test body.
fn metadata_asserts() -> Vec<MetadataAssert> {
    vec![
        ("ui.inspect", assert_ui_inspect),
        ("ui.inspector.close", assert_inspector_close),
        ("ui.inspector.close_all", assert_inspector_close_all),
        ("ui.inspector.set_width", assert_inspector_set_width),
        ("ui.palette.open", assert_palette_open),
        ("ui.palette.close", assert_palette_close),
        ("ui.entity.startRename", assert_start_rename),
        ("ui.mode.set", assert_mode_set),
        ("ui.setFocus", assert_set_focus),
        ("window.new", assert_window_new),
    ]
}

/// Assert a command carries no `keys` (absent or empty).
fn assert_no_keys(cmd: &Value, id: &str) {
    assert!(
        cmd.get("keys").is_none() || cmd["keys"] == json!({}),
        "{id} carries no keys, got {}",
        cmd["keys"]
    );
}

/// Assert a command carries no `menu` (absent or null).
fn assert_no_menu(cmd: &Value, id: &str) {
    assert!(
        cmd.get("menu").is_none() || cmd["menu"].is_null(),
        "{id} carries no menu, got {}",
        cmd["menu"]
    );
}

/// Assert a command carries a single `{ name, from }` param entry.
fn assert_single_param(cmd: &Value, id: &str, name: &str, from: &str) {
    let params = cmd["params"].as_array().unwrap_or_else(|| {
        panic!("{id} must carry a params array, got {}", cmd["params"]);
    });
    assert_eq!(params.len(), 1, "{id} carries exactly one param");
    assert_eq!(params[0]["name"], json!(name), "{id} param name");
    assert_eq!(params[0]["from"], json!(from), "{id} param from");
}

/// `ui.inspect` — ui.yaml: context_menu (group 3, order 0); param
/// moniker(target); no keys/menu.
fn assert_ui_inspect(cmd: &Value) {
    assert_eq!(
        cmd["name"],
        json!("Inspect {{entity.type}}"),
        "ui.inspect name"
    );
    assert_eq!(cmd["context_menu"], json!(true), "ui.inspect context_menu");
    assert_eq!(
        cmd["context_menu_group"],
        json!(3),
        "ui.inspect context_menu_group"
    );
    assert_eq!(
        cmd["context_menu_order"],
        json!(0),
        "ui.inspect context_menu_order"
    );
    assert_single_param(cmd, "ui.inspect", "moniker", "target");
    assert_no_keys(cmd, "ui.inspect");
    assert_no_menu(cmd, "ui.inspect");
}

/// `ui.inspector.close` — ui.yaml: keys cua:Escape / vim:q; no menu.
fn assert_inspector_close(cmd: &Value) {
    assert_eq!(
        cmd["name"],
        json!("Close Inspector"),
        "ui.inspector.close name"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Escape", "vim": "q" }),
        "ui.inspector.close keys"
    );
    assert_no_menu(cmd, "ui.inspector.close");
}

/// `ui.inspector.close_all` — ui.yaml: keys cua:Mod+Escape / vim:Q; no menu.
fn assert_inspector_close_all(cmd: &Value) {
    assert_eq!(
        cmd["name"],
        json!("Close All Inspectors"),
        "ui.inspector.close_all name"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+Escape", "vim": "Q" }),
        "ui.inspector.close_all keys"
    );
    assert_no_menu(cmd, "ui.inspector.close_all");
}

/// `ui.inspector.set_width` — ui.yaml: visible:false, undoable:false; param
/// width(args); no keys/menu.
fn assert_inspector_set_width(cmd: &Value) {
    assert_eq!(
        cmd["name"],
        json!("Set Inspector Width"),
        "ui.inspector.set_width name"
    );
    assert_eq!(
        cmd["visible"],
        json!(false),
        "ui.inspector.set_width visible:false"
    );
    assert_eq!(
        cmd["undoable"],
        json!(false),
        "ui.inspector.set_width undoable:false"
    );
    assert_single_param(cmd, "ui.inspector.set_width", "width", "args");
    assert_no_keys(cmd, "ui.inspector.set_width");
    assert_no_menu(cmd, "ui.inspector.set_width");
}

/// `ui.palette.open` — ui.yaml: keys cua:Mod+K / vim:":"; no menu.
fn assert_palette_open(cmd: &Value) {
    assert_eq!(
        cmd["name"],
        json!("Command Palette"),
        "ui.palette.open name"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+K", "vim": ":" }),
        "ui.palette.open keys"
    );
    assert_no_menu(cmd, "ui.palette.open");
}

/// `ui.palette.close` — ui.yaml: visible:false; no keys/menu.
fn assert_palette_close(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Close Palette"), "ui.palette.close name");
    assert_eq!(
        cmd["visible"],
        json!(false),
        "ui.palette.close visible:false"
    );
    assert_no_keys(cmd, "ui.palette.close");
    assert_no_menu(cmd, "ui.palette.close");
}

/// `ui.entity.startRename` — ui.yaml: scope entity:perspective; keys
/// cua/vim/emacs all Enter; no menu.
fn assert_start_rename(cmd: &Value) {
    assert_eq!(
        cmd["name"],
        json!("Rename Perspective"),
        "ui.entity.startRename name"
    );
    assert_eq!(
        cmd["scope"],
        json!(["entity:perspective"]),
        "ui.entity.startRename scope"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Enter", "vim": "Enter", "emacs": "Enter" }),
        "ui.entity.startRename keys"
    );
    assert_no_menu(cmd, "ui.entity.startRename");
}

/// `ui.mode.set` — ui.yaml: visible:false, undoable:false; param mode(args);
/// no keys/menu.
fn assert_mode_set(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Set App Mode"), "ui.mode.set name");
    assert_eq!(cmd["visible"], json!(false), "ui.mode.set visible:false");
    assert_eq!(cmd["undoable"], json!(false), "ui.mode.set undoable:false");
    assert_single_param(cmd, "ui.mode.set", "mode", "args");
    assert_no_keys(cmd, "ui.mode.set");
    assert_no_menu(cmd, "ui.mode.set");
}

/// `ui.setFocus` — ui.yaml: visible:false, undoable:false; no keys/menu.
fn assert_set_focus(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Set Focus"), "ui.setFocus name");
    assert_eq!(cmd["visible"], json!(false), "ui.setFocus visible:false");
    assert_eq!(cmd["undoable"], json!(false), "ui.setFocus undoable:false");
    assert_no_keys(cmd, "ui.setFocus");
    assert_no_menu(cmd, "ui.setFocus");
}

/// `window.new` — ui.yaml: keys cua/vim/emacs all Mod+Shift+N, menu
/// {path:[Window], group:0, order:0}.
fn assert_window_new(cmd: &Value) {
    assert_eq!(cmd["name"], json!("New Window"), "window.new name");
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+Shift+N", "vim": "Mod+Shift+N", "emacs": "Mod+Shift+N" }),
        "window.new keys"
    );
    assert_eq!(
        cmd["menu"],
        json!({ "path": ["Window"], "group": 0, "order": 0 }),
        "window.new menu"
    );
}
