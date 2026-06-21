//! End-to-end test for the committed `app-shell-commands` builtin plugin.
//!
//! This is the acceptance for the SINGLE app-command bundle: the port of the
//! three small platform-shell YAML files — `app.yaml` (9), `settings.yaml`
//! (3), `drag.yaml` (3) — PLUS the former `ui-commands` bundle folded in by
//! the ui.*→app.* rename (mop-up card 01KTEBZSVGAZ881RAZZWWZXGPE): the ported
//! `ui.yaml` commands (every id now `app.*`), the Card D UI-surface commands
//! (`field.edit` / `field.editEnter` / `pressable.activate` /
//! `pressable.activateSpace`), the Card E editor drill-ins
//! (`filter_editor.drillIn` / `app.ai-panel.composer.drillIn` /
//! `app.ai-panel.elicitation.field.drillIn`), and the Card G consolidated
//! `entity.inspect` — 33 commands total. The bundle fans out across FIVE
//! backends by concern:
//!
//!   - `app.quit` / `app.about` / `app.help`         → the `app` server
//!     (`swissarmyhammer-app-service::AppService` over a recording spy
//!     `AppShell`), exposed under id `"app"`.
//!   - `app.undo` / `app.redo`                        → the `store` server
//!     (`swissarmyhammer-store::StoreServer` over the board's ONE shared
//!     `StoreContext`), exposed under id `"store"`.
//!   - the UI-toggle / keymap / drag families — `app.command` / `app.palette` /
//!     `app.search` / `app.dismiss`, `settings.keymap.{cua,vim,emacs}`, and
//!     `drag.{start,cancel,complete}` — plus the inspector / palette / mode /
//!     rename / inspect / focus-record families — `app.inspect` /
//!     `entity.inspect`, `app.inspector.{close,close_all,set_width}`,
//!     `app.palette.{open,close}`, `app.entity.startRename`, `app.mode.set`,
//!     `app.setFocus` — route to the `ui_state` server
//!     (`swissarmyhammer-ui-state::UiStateServer` over a temp-file `UiState`),
//!     exposed under id `"ui_state"`.
//!   - `window.new`                                   → the `window` server
//!     (`swissarmyhammer-window-service::WindowService` over a recording spy
//!     `WindowShell`), exposed under id `"window"`.
//!   - the `focus` server (`swissarmyhammer-focus::FocusServer`) is ensured by
//!     the bundle's `load()` (the spatial-nav React layer reaches the focus
//!     kernel through the generic `command_tool_call` bridge), so it is
//!     exposed here too — and `app.setFocus` must NOT commit on it (the
//!     command records the scope chain into `ui_state`).
//!   - `field.*` / `pressable.*` / the editor drill-ins — NO backend: webview-
//!     bus handled (the owning React components register the live handlers
//!     while focused); the host executes are inert no-ops.
//!
//! What a passing run proves:
//!
//! 1. **Discovery + registration** — after load, all 33 commands are
//!    registered, and none carries the retired `ui.` id prefix.
//! 2. **Metadata fidelity** — each command's `name` / `keys` / `menu` /
//!    `scope` / `context_menu*` / `visible` / `undoable` / `params` match the
//!    source baseline 1:1 (table-test).
//! 3. **Real effects** —
//!    - `app.undo` / `app.redo` drive the store server's stack-wide undo/redo
//!      and revert / reapply a real entity write on the ONE shared
//!      `StoreContext`.
//!    - `settings.keymap.vim` then `settings.keymap.cua` flip the active keymap
//!      mode observed on the shared `UiState`.
//!    - `drag.start` → `drag.complete` progress the `UiState` drag state
//!      machine (session present, then taken).
//!    - `app.quit` / `app.about` / `app.help` hit the recording `AppShell` spy.
//!    - `app.inspect` pushes the target moniker onto the `UiState` inspector
//!      stack, `app.inspector.close` pops it, `app.inspector.close_all` clears
//!      it, `app.inspector.set_width` persists the width
//!      (regression `no-client-side-inspect`: via the Command service, not a
//!      React shortcut).
//!    - `app.palette.open` flips the palette-open flag, `app.palette.close`
//!      clears it.
//!    - `app.mode.set` switches the active keymap mode.
//!    - `app.entity.startRename` reaches the backend no-op (`{ ok: true }`).
//!    - `app.setFocus` records the scope chain into `ui_state` and leaves the
//!      spatial focus kernel untouched.
//!    - `window.new` hits the recording `WindowShell` spy.
//!    - the webview-bus commands dispatch host-side as inert `{ ok: true }`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use swissarmyhammer_app_service::{AboutInfo, AppService, AppShell};
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_directory::KanbanConfig;
use swissarmyhammer_entity::{Entity, EntityContext};
use swissarmyhammer_focus::{
    FocusLayer, FocusServer, FullyQualifiedMoniker, LayerName, SegmentMoniker, SpatialState,
    WindowLabel,
};
use swissarmyhammer_kanban::board::InitBoard;
use swissarmyhammer_kanban::{KanbanContext, KanbanOperationProcessor, OperationProcessor};
use swissarmyhammer_plugin::{
    CallerId, InProcessServer, McpServer as PluginMcpServer, PluginHost, PLUGINS_SUBDIR,
};
use swissarmyhammer_store::{StoreContext, StoreServer};
use swissarmyhammer_ui_state::{UiState, UiStateServer};
use swissarmyhammer_window_service::{
    ContextMenuItem, CreatedBoard, MonitorInfo, NewWindow, OpenedBoard, WindowPosition,
    WindowService, WindowShell,
};
use tempfile::TempDir;
use tokio::sync::Mutex as TokioMutex;

use crate::support::{
    assert_inspect_applies_to, call_command, copy_dir_recursive, execute_result, try_call_command,
};

/// A generous upper bound on any single host or isolate interaction.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// The window the ui-origin commands operate on throughout the test.
///
/// Deliberately NOT `"main"`: the window is carried only in the scope chain's
/// `window:` moniker (the production shape), and the ui_state server defaults a
/// chainless op to `"main"`. Using a non-default label proves the window is
/// resolved from the scope chain rather than silently falling back to the
/// default — the exact regression where palette/inspector state was written to
/// a `"main"` slot no real board window reads.
const WINDOW: &str = "board-test";

/// The production-shape scope chain a real dispatch carries: a `window:<label>`
/// moniker plus the `engine` root. The window is the single structured
/// parameter every per-window ui_state op resolves its target from — there is
/// no denormalized `window_label`.
fn window_scope() -> Value {
    json!([format!("window:{WINDOW}"), "engine"])
}

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

/// Stage the committed `builtin/plugins/app-shell-commands` bundle into a temp
/// builtin-layer root so `discover_and_load_all` finds it at
/// `<layer_root>/plugins/app-shell-commands/`.
fn stage_app_shell_commands(layer_root: &Path) {
    let source = workspace_root()
        .join("builtin/plugins")
        .join("app-shell-commands");
    assert!(
        source.is_dir(),
        "the committed app-shell-commands bundle must exist at {}",
        source.display()
    );
    let destination = layer_root.join(PLUGINS_SUBDIR).join("app-shell-commands");
    copy_dir_recursive(&source, &destination);
}

// ───────────────────────────────────────────────────────────────────────────
// The `app` backend: a recording spy AppShell
// ───────────────────────────────────────────────────────────────────────────

/// A recording [`AppShell`] used to assert which shell method `app.quit` /
/// `app.about` / `app.help` drove. Each call appends a tag to `calls`.
struct SpyAppShell {
    calls: Mutex<Vec<&'static str>>,
}

impl SpyAppShell {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }
}

impl AppShell for SpyAppShell {
    fn quit(&self) {
        self.calls.lock().unwrap().push("quit");
    }

    fn show_about(&self) -> AboutInfo {
        self.calls.lock().unwrap().push("about");
        AboutInfo {
            name: "kanban-app".to_string(),
            version: "9.9.9".to_string(),
        }
    }

    fn show_help(&self) -> String {
        self.calls.lock().unwrap().push("help");
        "https://help.example/docs".to_string()
    }
}

// ───────────────────────────────────────────────────────────────────────────
// The `window` backend: a recording spy WindowShell
// ───────────────────────────────────────────────────────────────────────────

/// A recording [`WindowShell`] that captures `open_new_window` so the test can
/// assert the ported `window.new` command reached the window-manager action.
/// Every other shell method is an inert stub.
struct SpyWindowShell {
    /// Ordered log of `<method>` tags, one per call.
    calls: Mutex<Vec<String>>,
}

impl SpyWindowShell {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl WindowShell for SpyWindowShell {
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
// Exposing the five real in-process backends
// ───────────────────────────────────────────────────────────────────────────

/// A handle to every live backend, kept alive for the test's duration so the
/// board root and shared kernels outlive the plugin's `load()` and every
/// `execute`.
struct ExposedBackends {
    _dir: TempDir,
    /// The ONE shared substrate `app.undo` / `app.redo` dispatch against.
    _store_ctx: Arc<StoreContext>,
    /// The entity kernel used to seed + observe the undo-reverted write.
    entity_ctx: Arc<EntityContext>,
    /// The shared UI state `ui_state`-routed commands mutate (keymap, drag,
    /// inspector, palette, rename, scope chain).
    ui_state: Arc<UiState>,
    /// The recording app-shell spy `app.quit` / `about` / `help` hit.
    shell: Arc<SpyAppShell>,
    /// The focus kernel's spatial state, read back to assert `app.setFocus`
    /// does NOT commit on it.
    spatial_state: Arc<TokioMutex<SpatialState>>,
    /// The recording window shell `window.new` hits.
    window_shell: Arc<SpyWindowShell>,
}

/// Wrap an `rmcp` server handler in an [`InProcessServer`] and expose it to
/// `host` under `name`. The single wrap-and-expose step every backend repeats,
/// factored out so `expose_backends` reads as a list of (name, server) pairs
/// rather than five copies of the same three-line incantation.
async fn expose_backend<S>(host: &PluginHost, name: &str, server: S)
where
    S: rmcp::ServerHandler + Send + Sync + 'static,
{
    let module = InProcessServer::new(server)
        .await
        .unwrap_or_else(|e| panic!("wrapping the {name} server in an InProcessServer: {e}"));
    host.expose_rust_module(name.to_string(), Arc::new(module) as Arc<dyn PluginMcpServer>)
        .await
        .unwrap_or_else(|e| panic!("exposing the {name} module: {e}"));
}

/// Build the `app`, `ui_state`, `store`, `window`, and `focus` backends over a
/// real board substrate and expose all five to `host` under their public ids.
/// Seeds a window-root layer on the focus kernel so scope-chain ops can
/// resolve the owning window from the snapshot's layer.
async fn expose_backends(host: &PluginHost) -> ExposedBackends {
    let dir = TempDir::new().expect("backend substrate temp dir");
    let kanban = KanbanContext::new(dir.path().join(".kanban"));

    KanbanOperationProcessor::new()
        .process(&InitBoard::new("App Shell Board"), &kanban)
        .await
        .expect("board init");

    let kanban = Arc::new(kanban);

    // ONE shared StoreContext — the substrate invariant. Entity writes push
    // onto it, and the store server's undo/redo revert/reapply against it.
    let store_ctx = swissarmyhammer_kanban::wire_store_substrate(&kanban).await;
    let entity_ctx = kanban.entity_context().await.expect("entity_context");

    // --- store server over the shared StoreContext ---
    expose_backend(host, "store", StoreServer::new(Arc::clone(&store_ctx))).await;

    // --- ui_state server over a temp-file-backed UiState ---
    let ui_state = Arc::new(UiState::load(dir.path().join("ui_state.yaml")));
    expose_backend(host, "ui_state", UiStateServer::new(Arc::clone(&ui_state))).await;

    // --- app server over a recording spy AppShell ---
    let shell = Arc::new(SpyAppShell::new());
    expose_backend(
        host,
        "app",
        AppService::new(Arc::clone(&shell) as Arc<dyn AppShell>),
    )
    .await;

    // --- focus server over a real SpatialRegistry / SpatialState ---
    let focus_server = FocusServer::new();
    let spatial_registry = focus_server.registry();
    let spatial_state = focus_server.state();
    // Seed a window-root layer `/L` owned by WINDOW so scope-chain ops can
    // derive the owning window from the snapshot's layer (exactly as
    // `push layer` does over the wire).
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
    expose_backend(host, "focus", focus_server).await;

    // --- window server over a recording spy WindowShell ---
    let window_shell = Arc::new(SpyWindowShell::new());
    expose_backend(
        host,
        "window",
        WindowService::new(Arc::clone(&window_shell) as Arc<dyn WindowShell>),
    )
    .await;

    ExposedBackends {
        _dir: dir,
        _store_ctx: store_ctx,
        entity_ctx,
        ui_state,
        shell,
        spatial_state,
        window_shell,
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
    resp
}

/// Like [`execute_ok`], but returns the inner backend result
/// (`structuredContent.result`) — the shape the ui-origin effect assertions
/// inspect.
async fn execute_inner_ok(
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

// ───────────────────────────────────────────────────────────────────────────
// The test
// ───────────────────────────────────────────────────────────────────────────

/// The committed `app-shell-commands` builtin plugin, discovered from a builtin
/// layer, registers all 33 commands with 1:1 metadata and produces each
/// platform-shell family's real effect against the live backends.
#[tokio::test]
async fn app_shell_commands_plugin_registers_and_executes() {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");

    stage_app_shell_commands(builtin_root.path());

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

    // Expose all five backends BEFORE discovery, so the plugin's
    // `ensureServices(this, ["commands", "app", "ui_state", "store", "window",
    // "focus"])` finds them already exposed.
    let backends = tokio::time::timeout(TIMEOUT, expose_backends(&host))
        .await
        .expect("exposing backends should not hang");

    let loaded = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the app-shell-commands builtin plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one app-shell-commands builtin plugin should be discovered, got {loaded:?}"
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
        "app.about",
        "app.help",
        "app.quit",
        "app.command",
        "app.palette",
        "app.search",
        "app.dismiss",
        "app.undo",
        "app.redo",
        "settings.keymap.vim",
        "settings.keymap.cua",
        "settings.keymap.emacs",
        "drag.start",
        "drag.cancel",
        "drag.complete",
        // The former ui-commands bundle, folded in by the ui.*→app.* rename:
        // every id is app.* — there is no `ui.*` command namespace.
        "app.inspect",
        "app.inspector.close",
        "app.inspector.close_all",
        "app.inspector.set_width",
        "app.palette.open",
        "app.palette.close",
        "app.entity.startRename",
        "app.mode.set",
        "app.setFocus",
        "window.new",
        // Card D — UI-surface commands moved out of React, webview-bus handled.
        "field.edit",
        "field.editEnter",
        "pressable.activate",
        "pressable.activateSpace",
        // Card E — editor drill-in commands moved out of React, webview-bus
        // handled.
        "filter_editor.drillIn",
        "app.ai-panel.composer.drillIn",
        "app.ai-panel.elicitation.field.drillIn",
        // Card G — the consolidated global Space inspect command.
        "entity.inspect",
    ] {
        assert!(
            commands.contains_key(id),
            "list command must include the ported command {id:?}; got {:?}",
            commands.keys().collect::<Vec<_>>()
        );
    }
    assert_eq!(
        commands.len(),
        33,
        "exactly the 33 app-shell-commands registrations should be present, got {:?}",
        commands.keys().collect::<Vec<_>>()
    );

    // The `ui.*` command namespace is retired (mop-up card
    // 01KTEBZSVGAZ881RAZZWWZXGPE): no registration in this bundle may carry
    // the old prefix.
    let ui_prefixed: Vec<&String> = commands.keys().filter(|id| id.starts_with("ui.")).collect();
    assert!(
        ui_prefixed.is_empty(),
        "no command id may start with the retired `ui.` prefix; got {ui_prefixed:?}"
    );

    // ── (2) Metadata fidelity: lock each command's metadata 1:1 vs the YAML ─
    for (id, assert_fn) in metadata_asserts() {
        assert_fn(&commands[id]);
    }

    // ── (3a) app.quit / about / help hit the recording AppShell spy ─────────
    execute_ok(&service, "app.about", json!({})).await;
    execute_ok(&service, "app.help", json!({})).await;
    execute_ok(&service, "app.quit", json!({})).await;
    assert_eq!(
        backends.shell.calls(),
        vec!["about", "help", "quit"],
        "app.about / app.help / app.quit must each drive the app shell spy in order"
    );

    // ── (3b) app.undo / app.redo revert + reapply a real shared-stack edit ──
    // Write a tag through the entity kernel — pushes onto the ONE shared stack.
    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    backends.entity_ctx.write(&tag).await.expect("write tag");
    let tag_id = tag.id.as_str().to_string();
    assert_eq!(
        backends
            .entity_ctx
            .list("tag")
            .await
            .expect("list tags")
            .len(),
        1,
        "the seeded tag is live before undo"
    );

    // app.undo → store `undo stack`: the tag write reverts.
    execute_ok(&service, "app.undo", json!({})).await;
    assert_eq!(
        backends
            .entity_ctx
            .list("tag")
            .await
            .expect("list tags")
            .len(),
        0,
        "app.undo must revert the tag write via the shared store stack"
    );

    // app.redo → store `redo stack`: the tag write reapplies.
    execute_ok(&service, "app.redo", json!({})).await;
    let after_redo = backends.entity_ctx.list("tag").await.expect("list tags");
    assert_eq!(
        after_redo.len(),
        1,
        "app.redo must reapply the tag write via the shared store stack"
    );
    assert_eq!(
        after_redo[0].id.as_str(),
        tag_id,
        "the reapplied tag must be the same one that was undone"
    );

    // ── (3c) settings.keymap.* flip the active keymap mode on the UiState ───
    assert_eq!(
        backends.ui_state.keymap_mode(),
        "cua",
        "precondition: the default keymap mode is cua"
    );
    execute_ok(&service, "settings.keymap.vim", json!({})).await;
    assert_eq!(
        backends.ui_state.keymap_mode(),
        "vim",
        "settings.keymap.vim must set the active keymap mode to vim"
    );
    execute_ok(&service, "settings.keymap.cua", json!({})).await;
    assert_eq!(
        backends.ui_state.keymap_mode(),
        "cua",
        "settings.keymap.cua must switch the active keymap mode back to cua"
    );

    // ── (3d) drag.start → drag.complete progress the UiState drag machine ───
    assert!(
        backends.ui_state.drag_session().is_none(),
        "precondition: no drag session before drag.start"
    );
    execute_ok(
        &service,
        "drag.start",
        json!({
            "args": {
                "session_id": "01DRAG0000000000000000001",
                "entity_type": "task",
                "entity_id": "01TASK0000000000000000001",
                "source_board_path": "/tmp/board",
                "source_window_label": "main",
                "copy_mode": false,
                "started_at_ms": 12345u64,
            },
        }),
    )
    .await;
    let session = backends
        .ui_state
        .drag_session()
        .expect("drag.start must open a drag session on the UiState");
    assert_eq!(
        session.session_id, "01DRAG0000000000000000001",
        "the open session carries the started session id"
    );

    execute_ok(&service, "drag.complete", json!({})).await;
    assert!(
        backends.ui_state.drag_session().is_none(),
        "drag.complete must take (clear) the active drag session"
    );
}

/// The ui-origin commands folded into `app-shell-commands` (the former
/// `app-shell-commands` bundle, every id renamed ui.*→app.*) produce each command's
/// real effect against the live backends — and each routes to the SAME MCP
/// server it did before the rename: inspector / palette / mode / rename /
/// inspect / focus-record → `ui_state`; `window.new` → `window`.
#[tokio::test]
async fn ui_origin_commands_execute_against_their_backends() {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");

    stage_app_shell_commands(builtin_root.path());

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

    let backends = tokio::time::timeout(TIMEOUT, expose_backends(&host))
        .await
        .expect("exposing backends should not hang");

    let loaded = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the app-shell-commands builtin plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one app-shell-commands builtin plugin should be discovered, got {loaded:?}"
    );

    // ── (a) app.inspect pushes the target moniker onto the inspector stack ──
    // Regression (`no-client-side-inspect`): this goes via the Command service
    // into the ui_state backend, NOT a React-side shortcut — the mutation is
    // observed on the shared UiState.
    assert!(
        backends.ui_state.inspector_stack(WINDOW).is_empty(),
        "precondition: the inspector stack is empty before app.inspect"
    );
    execute_inner_ok(
        &service,
        "app.inspect",
        json!({ "target": "task:01ABC", "scope_chain": window_scope() }),
    )
    .await;
    assert_eq!(
        backends.ui_state.inspector_stack(WINDOW),
        vec!["task:01ABC".to_string()],
        "app.inspect must push the target moniker onto the ui_state inspector stack"
    );

    // A second inspect deepens the stack.
    execute_inner_ok(
        &service,
        "app.inspect",
        json!({ "target": "tag:bug", "scope_chain": window_scope() }),
    )
    .await;
    assert_eq!(
        backends.ui_state.inspector_stack(WINDOW),
        vec!["task:01ABC".to_string(), "tag:bug".to_string()],
        "a second app.inspect deepens the inspector stack"
    );

    // ── (a') app.inspect with NO target resolves from the scope chain ───────
    // The palette-pick shape (kanban card 01KTY6XTJQFCG9ENKTAMC6N3JV): the
    // palette row "Inspect Task" dispatches `app.inspect` with the focused
    // scope chain and NO explicit target. The command must resolve the same
    // way as `entity.inspect` — the innermost inspectable-entity moniker —
    // never inspect the empty string (the live bug: `ctx.target ?? ""`
    // pushed `""` onto the inspector stack, a silent visible no-op).
    execute_inner_ok(
        &service,
        "app.inspect",
        json!({
            "scope_chain": [
                "task:01PALETTE",
                "column:todo",
                format!("window:{WINDOW}"),
                "engine",
            ],
        }),
    )
    .await;
    assert_eq!(
        backends.ui_state.inspector_stack(WINDOW),
        vec![
            "task:01ABC".to_string(),
            "tag:bug".to_string(),
            "task:01PALETTE".to_string(),
        ],
        "app.inspect with no target must resolve the innermost inspectable \
         moniker from the scope chain (the palette-pick shape) — never \
         inspect the empty string"
    );

    // ── (a'') app.inspect with a CHROME-leaf target falls through ────────────
    // The toolbar (nav-bar) context-menu dispatch shape (kanban card
    // 01KV5KYZT9J2BXFJ6H2X782E14): `lib/context-menu.ts` sets `target` to the
    // INNERMOST scope-chain moniker, which on the toolbar is a `ui:navbar.*`
    // CHROME leaf, not an entity. The caption ("Inspect Board") is rendered
    // by `focused_entity_type`, which skips chrome and resolves the `board:`
    // ANCESTOR — so a chrome `target` that wins verbatim would inspect a
    // non-inspectable moniker and no-op while the caption promised a board.
    // `resolveInspectTarget` must therefore IGNORE a non-inspectable explicit
    // target and fall through to the innermost inspectable scope-chain
    // moniker (the `board:` ancestor), matching the caption.
    execute_inner_ok(
        &service,
        "app.inspect",
        json!({
            "target": "ui:navbar.board-selector",
            "scope_chain": [
                "ui:navbar.board-selector",
                "board:01BOARD",
                format!("window:{WINDOW}"),
                "engine",
            ],
        }),
    )
    .await;
    assert_eq!(
        backends.ui_state.inspector_stack(WINDOW),
        vec![
            "task:01ABC".to_string(),
            "tag:bug".to_string(),
            "task:01PALETTE".to_string(),
            "board:01BOARD".to_string(),
        ],
        "app.inspect with a non-inspectable CHROME-leaf target \
         (ui:navbar.board-selector) must fall through to the innermost \
         inspectable scope-chain moniker (board:01BOARD) — the toolbar \
         context-menu Inspect-Board shape — never inspect the chrome leaf"
    );
    execute_inner_ok(
        &service,
        "app.inspector.close",
        json!({ "scope_chain": window_scope() }),
    )
    .await;

    execute_inner_ok(
        &service,
        "app.inspector.close",
        json!({ "scope_chain": window_scope() }),
    )
    .await;

    // ── (b) app.inspector.close pops the topmost entry ──────────────────────
    execute_inner_ok(
        &service,
        "app.inspector.close",
        json!({ "scope_chain": window_scope() }),
    )
    .await;
    assert_eq!(
        backends.ui_state.inspector_stack(WINDOW),
        vec!["task:01ABC".to_string()],
        "app.inspector.close must pop the topmost inspector entry"
    );

    // ── (c) app.inspector.close_all clears the stack ────────────────────────
    execute_inner_ok(
        &service,
        "app.inspector.close_all",
        json!({ "scope_chain": window_scope() }),
    )
    .await;
    assert!(
        backends.ui_state.inspector_stack(WINDOW).is_empty(),
        "app.inspector.close_all must clear the inspector stack"
    );

    // ── (c') entity.inspect resolves its target SERVER-SIDE (Card G) ────────
    // Three contracts, against the same shared `UiState` inspector stack:
    //   1. An explicit `ctx.target` wins verbatim (the palette-result-row /
    //      programmatic dispatch shape).
    //   2. With no target, the INNERMOST inspectable-entity moniker in the
    //      scope chain is inspected — replacing the React-side
    //      `INSPECTABLE_ENTITY_PREFIXES` filter (`buildRootInspectCommand`)
    //      and the per-`<Inspectable>` scope `CommandDef`. `field:` monikers
    //      are NOT entities (kanban card 01KTY6XTJQFCG9ENKTAMC6N3JV): a
    //      focused field's `field:{type}:{id}.{name}` projection moniker is
    //      skipped and the CONTAINING entity wins — Space on a task's field
    //      inspects the task, matching the `{{entity.type}}` caption the
    //      palette renders from the same chain.
    //   3. A chain with no inspectable entity (Space on chrome / no focus)
    //      is a harmless `{ ok: true }` no-op — nothing is pushed.
    assert!(
        backends.ui_state.inspector_stack(WINDOW).is_empty(),
        "precondition: the inspector stack is empty before entity.inspect"
    );
    execute_inner_ok(
        &service,
        "entity.inspect",
        json!({ "target": "task:01EXPL", "scope_chain": window_scope() }),
    )
    .await;
    assert_eq!(
        backends.ui_state.inspector_stack(WINDOW),
        vec!["task:01EXPL".to_string()],
        "entity.inspect must honor an explicit target verbatim"
    );

    execute_inner_ok(
        &service,
        "entity.inspect",
        json!({
            "scope_chain": [
                "field:task:01ABC.title",
                "ui:field",
                "task:01ABC",
                format!("window:{WINDOW}"),
                "engine",
            ],
        }),
    )
    .await;
    assert_eq!(
        backends.ui_state.inspector_stack(WINDOW),
        vec!["task:01EXPL".to_string(), "task:01ABC".to_string()],
        "entity.inspect with no target must skip the `field:` projection \
         moniker and inspect the CONTAINING task from the scope chain \
         (task:01ABC, not field:task:01ABC.title)"
    );

    let noop = execute_inner_ok(
        &service,
        "entity.inspect",
        json!({
            "scope_chain": [
                "perspective_tab:active",
                format!("window:{WINDOW}"),
                "engine",
            ],
        }),
    )
    .await;
    assert_eq!(
        noop["ok"],
        json!(true),
        "entity.inspect on a chrome-only chain returns the inert {{ ok: true }}; got {noop}"
    );
    assert_eq!(
        backends.ui_state.inspector_stack(WINDOW),
        vec!["task:01EXPL".to_string(), "task:01ABC".to_string()],
        "entity.inspect on a chrome-only chain must NOT push an inspector entry"
    );

    // Restore the empty stack for the sections below.
    execute_inner_ok(
        &service,
        "app.inspector.close_all",
        json!({ "scope_chain": window_scope() }),
    )
    .await;

    // ── (d) app.inspector.set_width persists the width ──────────────────────
    execute_inner_ok(
        &service,
        "app.inspector.set_width",
        json!({ "scope_chain": window_scope(), "args": { "width": 480 } }),
    )
    .await;
    assert_eq!(
        backends.ui_state.inspector_width(WINDOW),
        Some(480),
        "app.inspector.set_width must persist the inspector width on the UiState"
    );

    // ── (e) app.palette.open / app.palette.close flip the palette flag ──────
    assert!(
        !backends.ui_state.palette_open(WINDOW),
        "precondition: the palette is closed before app.palette.open"
    );
    execute_inner_ok(
        &service,
        "app.palette.open",
        json!({ "scope_chain": window_scope() }),
    )
    .await;
    assert!(
        backends.ui_state.palette_open(WINDOW),
        "app.palette.open must open the command palette on the UiState"
    );
    // The window must come from the scope chain, NOT default to "main": the
    // exact regression where palette state landed on a window no board reads.
    assert!(
        !backends.ui_state.palette_open("main"),
        "app.palette.open must NOT write to the default 'main' window when the \
         scope chain names a different window"
    );
    execute_inner_ok(
        &service,
        "app.palette.close",
        json!({ "scope_chain": window_scope() }),
    )
    .await;
    assert!(
        !backends.ui_state.palette_open(WINDOW),
        "app.palette.close must close the command palette on the UiState"
    );

    // ── (f) app.mode.set switches the active keymap mode ────────────────────
    assert_eq!(
        backends.ui_state.keymap_mode(),
        "cua",
        "precondition: the default keymap mode is cua"
    );
    execute_inner_ok(
        &service,
        "app.mode.set",
        json!({ "args": { "mode": "vim" } }),
    )
    .await;
    assert_eq!(
        backends.ui_state.keymap_mode(),
        "vim",
        "app.mode.set must switch the active keymap mode to vim"
    );

    // ── (g) app.entity.startRename reaches the backend no-op ────────────────
    // StartRename is a backend no-op (the frontend intercepts the command
    // before it reaches the backend); reaching it through the Command service
    // into the ui_state backend is the signal — `execute_inner_ok` already
    // asserted the envelope `ok: true`, which only succeeds if the ui_state
    // dispatch resolved.
    execute_inner_ok(
        &service,
        "app.entity.startRename",
        json!({ "scope_chain": window_scope() }),
    )
    .await;

    // ── (h) app.setFocus records the focus scope chain in ui_state ──────────
    // app.setFocus routes to the ui_state `set scope_chain` op — it records the
    // UI-state focus scope chain the frontend already computes (leaf-first).
    // The spatial focus KERNEL is the separate `focus` server; app.setFocus
    // must NOT touch it.
    assert!(
        backends.ui_state.scope_chain().is_empty(),
        "precondition: no focus scope chain recorded before app.setFocus"
    );
    assert!(
        backends
            .spatial_state
            .lock()
            .await
            .focused_in(&WindowLabel::from_string(WINDOW))
            .is_none(),
        "precondition: no spatial-focus slot before app.setFocus"
    );
    let chain = vec![
        "field:k1".to_string(),
        format!("window:{WINDOW}"),
        "engine".to_string(),
    ];
    let focus = execute_inner_ok(
        &service,
        "app.setFocus",
        json!({ "args": { "scope_chain": chain } }),
    )
    .await;
    // The dispatch returns the ui_state op's `{ ok, change }` envelope under
    // `structuredContent`; the recorded chain is the `ScopeChain` change.
    assert_eq!(
        focus["structuredContent"]["change"]["ScopeChain"],
        json!(chain),
        "app.setFocus must return the recorded scope chain in its change payload"
    );
    assert_eq!(
        backends.ui_state.scope_chain(),
        chain,
        "app.setFocus must record the focus scope chain into ui_state"
    );
    // ...and it must NOT have committed anything on the spatial focus kernel:
    // that is the separate `focus` server's concern.
    assert!(
        backends
            .spatial_state
            .lock()
            .await
            .focused_in(&WindowLabel::from_string(WINDOW))
            .is_none(),
        "app.setFocus must not commit on the spatial focus kernel"
    );

    // ── (i) window.new hits the recording WindowShell spy ──────────────────
    assert!(
        backends.window_shell.calls().is_empty(),
        "precondition: no window-shell calls before window.new"
    );
    execute_inner_ok(&service, "window.new", json!({})).await;
    assert_eq!(
        backends.window_shell.calls(),
        vec!["open_new_window:None".to_string()],
        "window.new must drive the window shell's open_new_window action"
    );

    // ── (j) field.* / pressable.* / drill-in dispatch host-side as no-ops ──
    // The webview command bus owns the live effect (Cards D + E); the host
    // execute exists only to satisfy the registration contract. A successful
    // `{ ok: true }` proves the execute reaches no backend.
    for id in [
        "field.edit",
        "field.editEnter",
        "pressable.activate",
        "pressable.activateSpace",
        "filter_editor.drillIn",
        "app.ai-panel.composer.drillIn",
        "app.ai-panel.elicitation.field.drillIn",
    ] {
        let result = execute_inner_ok(&service, id, json!({})).await;
        assert_eq!(
            result["ok"],
            json!(true),
            "the inert host execute for {id} returns {{ ok: true }}; got {result}"
        );
    }
}

/// The committed `app-shell-commands` bundle gates the VISIBLE inspect surface
/// (`app.inspect`) by the INSPECTABLE entity capability set: `list command`
/// suppresses it when the focus is a `field:` projection moniker (a field is
/// NOT an entity), still offers it when a real entity (task) is focused, AND
/// still offers it on the root `board:` focus ("Inspect Board" — the board is
/// inspectable even though it is never a cut/copy/delete SUBJECT). The inspect
/// set is decoupled from the board-less subject set on purpose. `entity.inspect`
/// (the Space gesture, ungated, server-side target resolution) is unaffected —
/// it carries no `applies_to`, so it lists in every focus and resolves the
/// containing entity itself.
#[tokio::test]
async fn app_inspect_suppressed_on_a_field_offered_on_an_entity() {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");

    stage_app_shell_commands(builtin_root.path());

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

    let _backends = tokio::time::timeout(TIMEOUT, expose_backends(&host))
        .await
        .expect("exposing backends should not hang");

    tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the app-shell-commands builtin plugin should succeed");

    // ── Field focus: app.inspect ABSENT; entity.inspect still PRESENT ───────
    let on_field = commands_by_id(
        &call_command(
            &service,
            CallerId::HostInternal,
            json!({
                "op": "list command",
                "ctx": {
                    "target": "field:task:01ABC.title",
                    "scope_chain": ["field:task:01ABC.title", "ui:field", "task:01ABC"],
                },
            }),
        )
        .await,
    );
    assert!(
        !on_field.contains_key("app.inspect"),
        "app.inspect must NOT surface when a field is focused — a field is a \
         projection, not an entity; got {:?}",
        on_field.keys().collect::<Vec<_>>()
    );
    assert!(
        on_field.contains_key("entity.inspect"),
        "entity.inspect (ungated Space gesture) must still list on a field \
         focus — it resolves the containing entity server-side; got {:?}",
        on_field.keys().collect::<Vec<_>>()
    );

    // ── Task focus: app.inspect PRESENT (no regression) ─────────────────────
    let on_task = commands_by_id(
        &call_command(
            &service,
            CallerId::HostInternal,
            json!({
                "op": "list command",
                "ctx": {
                    "target": "task:01ABC",
                    "scope_chain": ["task:01ABC", "column:todo"],
                },
            }),
        )
        .await,
    );
    assert!(
        on_task.contains_key("app.inspect"),
        "app.inspect MUST surface when a real entity (task) is focused; got {:?}",
        on_task.keys().collect::<Vec<_>>()
    );

    // ── Root board focus: app.inspect PRESENT ("Inspect Board") ─────────────
    // Inspect is decoupled from the subject ops: the root board can never be
    // cut/copied/deleted from itself, but it CAN be inspected. `board:` is in
    // the inspectable set, so "Inspect Board" must surface on the root board
    // background — the regression this card restores.
    let on_board = commands_by_id(
        &call_command(
            &service,
            CallerId::HostInternal,
            json!({
                "op": "list command",
                "ctx": {
                    "target": "board:b1",
                    "scope_chain": ["board:b1"],
                },
            }),
        )
        .await,
    );
    assert!(
        on_board.contains_key("app.inspect"),
        "app.inspect MUST surface on the root board — \"Inspect Board\" is a \
         meaningful root-board affordance (board is inspectable); got {:?}",
        on_board.keys().collect::<Vec<_>>()
    );

    // ── field.edit is the COMPLEMENT of the suppression above ────────────────
    // "Edit Field" is the one command that makes sense on a field. It is gated
    // by its `scope: ["ui:field"]` marker (NOT `applies_to`), so it surfaces
    // whenever the `ui:field` marker is in the focused chain — both the
    // context-menu surface (scope filter = `ui:field`) and the palette surface
    // (scope filter = the dynamic `field:` leaf, with `ui:field` in the chain)
    // — and nowhere else (card 01KV30ZXHWPS4FZK9WEH4DMMZY).

    // Context-menu over a field row: the surface filters by the marker scope.
    let field_ctx_menu = commands_by_id(
        &call_command(
            &service,
            CallerId::HostInternal,
            json!({
                "op": "list command",
                "scope": "ui:field",
                "ctx": {
                    "target": "field:task:01ABC.title",
                    "scope_chain": ["field:task:01ABC.title", "ui:field", "task:01ABC"],
                },
            }),
        )
        .await,
    );
    assert!(
        field_ctx_menu.contains_key("field.edit"),
        "field.edit MUST surface on a field context menu; got {:?}",
        field_ctx_menu.keys().collect::<Vec<_>>()
    );

    // Command palette with a field focused: the surface filters by the dynamic
    // `field:` leaf, and the `ui:field` marker is its parent in the chain.
    let field_palette = commands_by_id(
        &call_command(
            &service,
            CallerId::HostInternal,
            json!({
                "op": "list command",
                "scope": "field:task:01ABC.title",
                "ctx": {
                    "scope_chain": ["field:task:01ABC.title", "ui:field", "task:01ABC"],
                },
            }),
        )
        .await,
    );
    assert!(
        field_palette.contains_key("field.edit"),
        "field.edit MUST surface in the palette for a focused field; got {:?}",
        field_palette.keys().collect::<Vec<_>>()
    );

    // A non-field entity (task) focus, queried the way a surface does — with a
    // `scope` filter for the focused leaf. "Edit Field" is nonsensical here:
    // the `ui:field` marker is not in a task's chain, so the scope gate omits
    // it. (The unfiltered `on_task` list above carries every scope-gated
    // command because a `scope`-less `list command` skips the scope gate
    // entirely; real surfaces always pass the focused leaf as `scope`.)
    let task_palette = commands_by_id(
        &call_command(
            &service,
            CallerId::HostInternal,
            json!({
                "op": "list command",
                "scope": "task:01ABC",
                "ctx": {
                    "target": "task:01ABC",
                    "scope_chain": ["task:01ABC", "column:todo"],
                },
            }),
        )
        .await,
    );
    assert!(
        !task_palette.contains_key("field.edit"),
        "field.edit must NOT surface on a non-field entity focus; got {:?}",
        task_palette.keys().collect::<Vec<_>>()
    );
}

// ───────────────────────────────────────────────────────────────────────────
// Per-command metadata regression asserts (locked against the source YAMLs)
// ───────────────────────────────────────────────────────────────────────────

/// One row of the metadata-fidelity table: a command id and its assertion.
type MetadataAssert = (&'static str, fn(&Value));

/// The metadata-fidelity table: each ported command id paired with its
/// per-command assertion, exercised across all 33 in the test body.
fn metadata_asserts() -> Vec<MetadataAssert> {
    vec![
        ("app.about", assert_app_about),
        ("app.help", assert_app_help),
        ("app.quit", assert_app_quit),
        ("app.command", assert_app_command),
        ("app.palette", assert_app_palette),
        ("app.search", assert_app_search),
        ("app.dismiss", assert_app_dismiss),
        ("app.undo", assert_app_undo),
        ("app.redo", assert_app_redo),
        ("settings.keymap.vim", assert_keymap_vim),
        ("settings.keymap.cua", assert_keymap_cua),
        ("settings.keymap.emacs", assert_keymap_emacs),
        ("drag.start", assert_drag_start),
        ("drag.cancel", assert_drag_cancel),
        ("drag.complete", assert_drag_complete),
        // The ui-origin commands (former ui-commands bundle, ids renamed
        // ui.*→app.*; metadata otherwise locked 1:1 against ui.yaml).
        ("app.inspect", assert_app_inspect),
        ("app.inspector.close", assert_inspector_close),
        ("app.inspector.close_all", assert_inspector_close_all),
        ("app.inspector.set_width", assert_inspector_set_width),
        ("app.palette.open", assert_palette_open),
        ("app.palette.close", assert_palette_close),
        ("app.entity.startRename", assert_start_rename),
        ("app.mode.set", assert_mode_set),
        ("app.setFocus", assert_set_focus),
        ("window.new", assert_window_new),
        ("field.edit", assert_field_edit),
        ("field.editEnter", assert_field_edit_enter),
        ("pressable.activate", assert_pressable_activate),
        ("pressable.activateSpace", assert_pressable_activate_space),
        ("filter_editor.drillIn", assert_filter_editor_drill_in),
        ("app.ai-panel.composer.drillIn", assert_composer_drill_in),
        (
            "app.ai-panel.elicitation.field.drillIn",
            assert_elicitation_field_drill_in,
        ),
        ("entity.inspect", assert_entity_inspect),
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

/// `app.about` — app.yaml: menu {path:[App], group:0, order:0}; no keys.
fn assert_app_about(cmd: &Value) {
    assert_eq!(cmd["name"], json!("About"), "app.about name");
    assert_eq!(
        cmd["menu"],
        json!({ "path": ["App"], "group": 0, "order": 0 }),
        "app.about menu"
    );
    assert_no_keys(cmd, "app.about");
}

/// `app.help` — app.yaml: keys vim:F1 / cua:F1; no menu.
fn assert_app_help(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Help"), "app.help name");
    assert_eq!(
        cmd["keys"],
        json!({ "vim": "F1", "cua": "F1" }),
        "app.help keys"
    );
    assert_no_menu(cmd, "app.help");
}

/// `app.quit` — app.yaml: keys cua:Mod+Q / vim:":q", menu {path:[App], group:2,
/// order:0}.
fn assert_app_quit(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Quit"), "app.quit name");
    // Canonical lowercase form `normalizeKeyEvent` emits for an unshifted
    // letter chord (`Mod+q`, not `Mod+Q`). The registry is the only webview
    // key source post Card I, and `extractKeymapBindings` matches the literal;
    // an uppercase unshifted letter is unreachable from a real keydown. Quit
    // has no `BINDING_TABLES` entry (it rides the native menu accelerator,
    // which parses letters case-insensitively), so the lowercase form keeps
    // the accelerator AND makes the chord live in the webview on non-Mac.
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+q", "vim": ":q" }),
        "app.quit keys"
    );
    assert_eq!(
        cmd["menu"],
        json!({ "path": ["App"], "group": 2, "order": 0 }),
        "app.quit menu"
    );
}

/// `app.command` — app.yaml: keys vim:":" / cua:Mod+Shift+P / emacs:Mod+Shift+P;
/// no menu.
fn assert_app_command(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Command Palette"), "app.command name");
    assert_eq!(
        cmd["keys"],
        json!({ "vim": ":", "cua": "Mod+Shift+P", "emacs": "Mod+Shift+P" }),
        "app.command keys"
    );
    assert_no_menu(cmd, "app.command");
}

/// `app.palette` — app.yaml: visible:false; keys cua/vim/emacs all Mod+Shift+P.
fn assert_app_palette(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Command Palette"), "app.palette name");
    assert_eq!(cmd["visible"], json!(false), "app.palette visible:false");
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+Shift+P", "vim": "Mod+Shift+P", "emacs": "Mod+Shift+P" }),
        "app.palette keys"
    );
    assert_no_menu(cmd, "app.palette");
}

/// `app.search` — keys vim:"/" / cua:Mod+f, menu {path:[Edit], group:0,
/// order:2}.
///
/// cua canonicalized to the lowercase `Mod+f` form `normalizeKeyEvent` emits
/// (matching `BINDING_TABLES.cua`). The emacs key is DROPPED, not lowercased:
/// `BINDING_TABLES.emacs` binds `Mod+f` to `nav.right` (the non-Mac
/// normalization of emacs forward-char `Ctrl+f`), so canonicalizing app.search's
/// emacs key to `Mod+f` would steal forward-char and re-open the first-id-wins
/// nondeterminism (card 01KMT56FTBAP8PQ4QQND08MP97 / 01KTQ6QZNB3VN4MAND7VPASM21).
/// Emacs Find stays palette-only.
fn assert_app_search(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Find"), "app.search name");
    assert_eq!(
        cmd["keys"],
        json!({ "vim": "/", "cua": "Mod+f" }),
        "app.search keys"
    );
    assert_eq!(
        cmd["menu"],
        json!({ "path": ["Edit"], "group": 0, "order": 2 }),
        "app.search menu"
    );
}

/// `app.dismiss` — intentionally unbound from Escape (card
/// 01KTPDTH772HSEV5F7R1DKYDNJ): Escape is owned by `nav.drillOut`, while
/// `app.dismiss` remains a keyless command for per-surface dispatch
/// (backdrop click, quick-capture); no keys, no menu.
fn assert_app_dismiss(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Dismiss"), "app.dismiss name");
    assert_no_keys(cmd, "app.dismiss");
    assert_no_menu(cmd, "app.dismiss");
}

/// `app.undo` — undoable:false; keys cua:Mod+z / vim:u / emacs:Ctrl+/, menu
/// {path:[Edit], group:0, order:0}. cua canonicalized to lowercase `Mod+z`
/// (`BINDING_TABLES.cua` agrees).
///
/// The emacs `Ctrl+/` binding moved here from `app-shell.tsx`'s deleted
/// `STATIC_GLOBAL_COMMANDS` (Card I) — the registry is now the only key
/// source for the webview hotkey path.
fn assert_app_undo(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Undo"), "app.undo name");
    assert_eq!(cmd["undoable"], json!(false), "app.undo undoable:false");
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+z", "vim": "u", "emacs": "Ctrl+/" }),
        "app.undo keys"
    );
    assert_eq!(
        cmd["menu"],
        json!({ "path": ["Edit"], "group": 0, "order": 0 }),
        "app.undo menu"
    );
}

/// `app.redo` — undoable:false; keys cua:Mod+Shift+Z / vim:Mod+r, menu
/// {path:[Edit], group:0, order:1}.
///
/// vim canonicalized to `Mod+r` per `BINDING_TABLES.vim` (`Mod+r` → app.redo).
/// The legacy `Ctrl+R` literal was unreachable: on non-Mac `Ctrl+r` normalizes
/// to `Mod+r`, and on Mac `Ctrl` stays distinct so `Ctrl+R` never appeared in
/// the table. `Mod+r` means Cmd+R on Mac, Ctrl+R elsewhere — both reachable.
fn assert_app_redo(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Redo"), "app.redo name");
    assert_eq!(cmd["undoable"], json!(false), "app.redo undoable:false");
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+Shift+Z", "vim": "Mod+r" }),
        "app.redo keys"
    );
    assert_eq!(
        cmd["menu"],
        json!({ "path": ["Edit"], "group": 0, "order": 1 }),
        "app.redo menu"
    );
}

/// Shared keymap-menu assertion: each is a radio-group entry under
/// [App, Settings] group 0 at its declared order, with no keys.
fn assert_keymap(cmd: &Value, id: &str, name: &str, order: i64) {
    assert_eq!(cmd["name"], json!(name), "{id} name");
    assert_eq!(
        cmd["menu"],
        json!({ "path": ["App", "Settings"], "group": 0, "order": order, "radio_group": "keymap" }),
        "{id} menu"
    );
    assert_no_keys(cmd, id);
}

/// `settings.keymap.vim` — settings.yaml: order 1.
fn assert_keymap_vim(cmd: &Value) {
    assert_keymap(cmd, "settings.keymap.vim", "Vim Keybindings", 1);
}

/// `settings.keymap.cua` — settings.yaml: order 0.
fn assert_keymap_cua(cmd: &Value) {
    assert_keymap(cmd, "settings.keymap.cua", "Standard Keybindings", 0);
}

/// `settings.keymap.emacs` — settings.yaml: order 2.
fn assert_keymap_emacs(cmd: &Value) {
    assert_keymap(cmd, "settings.keymap.emacs", "Emacs Keybindings", 2);
}

/// Shared drag assertion: each is undoable:false, visible:false, no keys/menu.
fn assert_drag(cmd: &Value, id: &str, name: &str) {
    assert_eq!(cmd["name"], json!(name), "{id} name");
    assert_eq!(cmd["undoable"], json!(false), "{id} undoable:false");
    assert_eq!(cmd["visible"], json!(false), "{id} visible:false");
    assert_no_keys(cmd, id);
    assert_no_menu(cmd, id);
}

/// `drag.start` — drag.yaml: undoable:false, visible:false.
fn assert_drag_start(cmd: &Value) {
    assert_drag(cmd, "drag.start", "Start Drag");
}

/// `drag.cancel` — drag.yaml: undoable:false, visible:false.
fn assert_drag_cancel(cmd: &Value) {
    assert_drag(cmd, "drag.cancel", "Cancel Drag");
}

/// `drag.complete` — drag.yaml: undoable:false, visible:false.
fn assert_drag_complete(cmd: &Value) {
    assert_drag(cmd, "drag.complete", "Complete Drag");
}

// ───────────────────────────────────────────────────────────────────────────
// Per-command metadata asserts for the ui-origin commands (former ui-commands
// bundle, ids renamed ui.*→app.*; metadata locked 1:1 against ui.yaml)
// ───────────────────────────────────────────────────────────────────────────

/// Assert a command carries a single `{ name, from }` param entry.
fn assert_single_param(cmd: &Value, id: &str, name: &str, from: &str) {
    let params = cmd["params"].as_array().unwrap_or_else(|| {
        panic!("{id} must carry a params array, got {}", cmd["params"]);
    });
    assert_eq!(params.len(), 1, "{id} carries exactly one param");
    assert_eq!(params[0]["name"], json!(name), "{id} param name");
    assert_eq!(params[0]["from"], json!(from), "{id} param from");
}

/// `app.inspect` — ui.yaml: context_menu (group 3, order 0); param
/// moniker(target); no keys/menu.
fn assert_app_inspect(cmd: &Value) {
    // Registered as "Inspect {{entity.type}}" — rendered to the generic
    // fallback by `list command` (no ctx supplied here).
    assert_eq!(cmd["name"], json!("Inspect"), "app.inspect name");
    assert_eq!(cmd["context_menu"], json!(true), "app.inspect context_menu");
    assert_eq!(
        cmd["context_menu_group"],
        json!(3),
        "app.inspect context_menu_group"
    );
    assert_eq!(
        cmd["context_menu_order"],
        json!(0),
        "app.inspect context_menu_order"
    );
    assert_single_param(cmd, "app.inspect", "moniker", "target");
    assert_no_keys(cmd, "app.inspect");
    assert_no_menu(cmd, "app.inspect");
    // Inspect gates on the INSPECTABLE set (board PRESENT) — NOT the board-less
    // subject set: "Inspect Board" is a meaningful root-board affordance.
    assert_inspect_applies_to(cmd, "app.inspect");
}

/// `entity.inspect` — Card G's consolidated global Space inspect command.
/// Keys are Space across all three keymaps (copied 1:1 from the retired
/// React `CommandDef`s in `inspectable.tsx` / `app-shell.tsx`); GLOBAL (no
/// scope) so the binding lives in the global key table; not palette-visible
/// (`app.inspect` remains the visible / context-menu "Inspect"); no menu.
fn assert_entity_inspect(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Inspect"), "entity.inspect name");
    assert_eq!(
        cmd["visible"],
        json!(false),
        "entity.inspect visible:false — app.inspect owns the visible entry"
    );
    assert_eq!(
        cmd["undoable"],
        json!(false),
        "entity.inspect undoable:false"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "vim": "Space", "cua": "Space", "emacs": "Space" }),
        "entity.inspect keys — Space in all three keymaps"
    );
    assert!(
        cmd.get("scope").is_none() || cmd["scope"].is_null() || cmd["scope"] == json!([]),
        "entity.inspect carries no scope (global Space binding), got {}",
        cmd["scope"]
    );
    assert!(
        cmd.get("context_menu").is_none() || cmd["context_menu"] == json!(false),
        "entity.inspect carries no context_menu (app.inspect owns it), got {}",
        cmd["context_menu"]
    );
    assert_no_menu(cmd, "entity.inspect");
}

/// `app.inspector.close` — keys vim:q only; cua:Escape was removed (card
/// 01KTPDTH772HSEV5F7R1DKYDNJ): inspector Escape-close flows through
/// `nav.drillOut` → `dismiss ui`, while vim `q` remains a direct close;
/// no menu.
fn assert_inspector_close(cmd: &Value) {
    assert_eq!(
        cmd["name"],
        json!("Close Inspector"),
        "app.inspector.close name"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "vim": "q" }),
        "app.inspector.close keys"
    );
    assert_no_menu(cmd, "app.inspector.close");
}

/// `app.inspector.close_all` — ui.yaml: keys cua:Mod+Escape / vim:Q; no menu.
fn assert_inspector_close_all(cmd: &Value) {
    assert_eq!(
        cmd["name"],
        json!("Close All Inspectors"),
        "app.inspector.close_all name"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+Escape", "vim": "Q" }),
        "app.inspector.close_all keys"
    );
    assert_no_menu(cmd, "app.inspector.close_all");
}

/// `app.inspector.set_width` — ui.yaml: visible:false, undoable:false; param
/// width(args); no keys/menu.
fn assert_inspector_set_width(cmd: &Value) {
    assert_eq!(
        cmd["name"],
        json!("Set Inspector Width"),
        "app.inspector.set_width name"
    );
    assert_eq!(
        cmd["visible"],
        json!(false),
        "app.inspector.set_width visible:false"
    );
    assert_eq!(
        cmd["undoable"],
        json!(false),
        "app.inspector.set_width undoable:false"
    );
    assert_single_param(cmd, "app.inspector.set_width", "width", "args");
    assert_no_keys(cmd, "app.inspector.set_width");
    assert_no_menu(cmd, "app.inspector.set_width");
}

/// `app.palette.open` — keys cua:Mod+K / vim:":" (unchanged from the former
/// `ui.palette.open`); carries an App-menu placement (the rename fold gave
/// the palette its OS-menu affordance). Routing to ui_state `open palette` is
/// unchanged.
fn assert_palette_open(cmd: &Value) {
    assert_eq!(
        cmd["name"],
        json!("Command Palette"),
        "app.palette.open name"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+K", "vim": ":" }),
        "app.palette.open keys"
    );
    assert_eq!(
        cmd["menu"],
        json!({ "path": ["App"], "group": 1, "order": 0 }),
        "app.palette.open menu — App submenu affordance from the rename fold"
    );
}

/// `app.palette.close` — ui.yaml: visible:false; no keys/menu.
fn assert_palette_close(cmd: &Value) {
    assert_eq!(
        cmd["name"],
        json!("Close Palette"),
        "app.palette.close name"
    );
    assert_eq!(
        cmd["visible"],
        json!(false),
        "app.palette.close visible:false"
    );
    assert_no_keys(cmd, "app.palette.close");
    assert_no_menu(cmd, "app.palette.close");
}

/// `app.entity.startRename` — scope entity:perspective; keys cua/vim/emacs
/// all F2 (rename is a deliberate gesture; Enter on a focused tab ACTIVATES
/// the perspective — card 01KTYQY0ZB62KHN6BPK3FBMBD7); context_menu so the
/// tab's right-click menu carries a Rename row; no menu.
fn assert_start_rename(cmd: &Value) {
    assert_eq!(
        cmd["name"],
        json!("Rename Perspective"),
        "app.entity.startRename name"
    );
    assert_eq!(
        cmd["scope"],
        json!(["entity:perspective"]),
        "app.entity.startRename scope"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "F2", "vim": "F2", "emacs": "F2" }),
        "app.entity.startRename keys"
    );
    assert_eq!(
        cmd["context_menu"],
        json!(true),
        "app.entity.startRename context_menu"
    );
    assert_no_menu(cmd, "app.entity.startRename");
}

/// `app.mode.set` — ui.yaml: visible:false, undoable:false; param mode(args);
/// no keys/menu.
fn assert_mode_set(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Set App Mode"), "app.mode.set name");
    assert_eq!(cmd["visible"], json!(false), "app.mode.set visible:false");
    assert_eq!(cmd["undoable"], json!(false), "app.mode.set undoable:false");
    assert_single_param(cmd, "app.mode.set", "mode", "args");
    assert_no_keys(cmd, "app.mode.set");
    assert_no_menu(cmd, "app.mode.set");
}

/// `app.setFocus` — ui.yaml: visible:false, undoable:false; no keys/menu.
fn assert_set_focus(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Set Focus"), "app.setFocus name");
    assert_eq!(cmd["visible"], json!(false), "app.setFocus visible:false");
    assert_eq!(cmd["undoable"], json!(false), "app.setFocus undoable:false");
    assert_no_keys(cmd, "app.setFocus");
    assert_no_menu(cmd, "app.setFocus");
}

/// Shared shape check for the Card D / E UI-surface commands: scope-gated to
/// the surface's literal chain moniker, no menu placement (the retired React
/// defs had none — the OS menu stays unchanged).
fn assert_ui_surface_command(cmd: &Value, id: &str, name: &str, keys: Value, scope: &str) {
    assert_eq!(cmd["name"], json!(name), "{id} name");
    assert_eq!(cmd["keys"], keys, "{id} keys");
    // The scope keeps the keys out of the global table: they bind only while
    // the focused scope chain contains the surface's literal moniker.
    assert_eq!(cmd["scope"], json!([scope]), "{id} scope");
    assert_no_menu(cmd, id);
}

/// `field.edit` — retired field.tsx def: keys vim:i / cua:Enter; gated to the
/// `ui:field` marker scope the field zone mounts above its `<FocusScope>`.
///
/// Unlike the other six UI-surface commands (keybinding-only), `field.edit`
/// ALSO surfaces on the palette + context menu: "Edit Field" is the one
/// command that makes sense on a field (card `01KV30ZXHWPS4FZK9WEH4DMMZY`). It
/// carries `context_menu: true` (group 0, order 0) and `params:
/// [{ moniker(target) }]`, modelled on `app.inspect`.
fn assert_field_edit(cmd: &Value) {
    assert_ui_surface_command(
        cmd,
        "field.edit",
        "Edit Field",
        json!({ "vim": "i", "cua": "Enter" }),
        "ui:field",
    );
    assert_eq!(cmd["context_menu"], json!(true), "field.edit context_menu");
    assert_eq!(
        cmd["context_menu_group"],
        json!(0),
        "field.edit context_menu_group"
    );
    assert_eq!(
        cmd["context_menu_order"],
        json!(0),
        "field.edit context_menu_order"
    );
    assert_eq!(
        cmd["params"],
        json!([{ "name": "moniker", "from": "target" }]),
        "field.edit params"
    );
}

/// `field.editEnter` — retired field.tsx def: keys vim:Enter (vim parity for
/// the cua Enter binding on `field.edit`).
fn assert_field_edit_enter(cmd: &Value) {
    assert_ui_surface_command(
        cmd,
        "field.editEnter",
        "Edit Field (Enter)",
        json!({ "vim": "Enter" }),
        "ui:field",
    );
}

/// `pressable.activate` — retired pressable.tsx def: keys vim:Enter /
/// cua:Enter; gated to the `ui:pressable` marker scope.
fn assert_pressable_activate(cmd: &Value) {
    assert_ui_surface_command(
        cmd,
        "pressable.activate",
        "Activate",
        json!({ "vim": "Enter", "cua": "Enter" }),
        "ui:pressable",
    );
}

/// `pressable.activateSpace` — retired pressable.tsx def: keys cua:Space only
/// (Web/CUA convention binds both Enter and Space; vim leaves Space free).
fn assert_pressable_activate_space(cmd: &Value) {
    assert_ui_surface_command(
        cmd,
        "pressable.activateSpace",
        "Activate (Space)",
        json!({ "cua": "Space" }),
        "ui:pressable",
    );
}

/// The shared keys block of the three Card E editor drill-in commands: every
/// keymap binds Enter, copied 1:1 from the retired React `CommandDef`s.
fn drill_in_keys() -> Value {
    json!({ "cua": "Enter", "vim": "Enter", "emacs": "Enter" })
}

/// `filter_editor.drillIn` — retired perspective-tab-bar.tsx def: Enter on the
/// focused filter formula bar drills DOM focus into the CM6 filter editor;
/// gated to the `ui:filter_editor` marker scope the formula bar mounts above
/// its dynamic `filter_editor:{id}` `<FocusScope>`.
fn assert_filter_editor_drill_in(cmd: &Value) {
    assert_ui_surface_command(
        cmd,
        "filter_editor.drillIn",
        "Edit Filter",
        drill_in_keys(),
        "ui:filter_editor",
    );
}

/// `app.ai-panel.composer.drillIn` — retired ai-prompt-composer.tsx def: Enter
/// on the focused composer scope drills DOM focus into the CM6 prompt; gated
/// to the composer `<FocusScope>`'s own constant `ui:ai-panel.composer`
/// moniker (no marker needed — the zone moniker is already literal).
fn assert_composer_drill_in(cmd: &Value) {
    assert_ui_surface_command(
        cmd,
        "app.ai-panel.composer.drillIn",
        "Edit Prompt",
        drill_in_keys(),
        "ui:ai-panel.composer",
    );
}

/// `app.ai-panel.elicitation.field.drillIn` — retired ai-elements/elicitation
/// .tsx def (formerly minted per field as `...drillIn:{key}`): ONE base id,
/// gated to the `ui:ai-panel.elicitation.field` marker scope each text-like
/// field mounts above its dynamic `ui:ai-panel.elicitation.field:{key}`
/// `<FocusScope>`. The per-field variation lives in the focus-gated webview
/// bus registration (the focused instance's closure), not in N minted ids.
fn assert_elicitation_field_drill_in(cmd: &Value) {
    assert_ui_surface_command(
        cmd,
        "app.ai-panel.elicitation.field.drillIn",
        "Edit Field",
        drill_in_keys(),
        "ui:ai-panel.elicitation.field",
    );
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
