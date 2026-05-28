//! End-to-end test for the committed `kanban-misc-commands` builtin plugin.
//!
//! This is the acceptance for the port of the four small kanban-domain YAMLs
//! (`column.yaml`, `attachment.yaml`, `tag.yaml`, `view.yaml`) into the one
//! `builtin/plugins/kanban-misc-commands/` bundle of five commands. It mirrors
//! `builtin_task_commands_e2e` exactly, but exposes THREE backends instead of
//! one because the five commands fan out across servers:
//!
//! - `column.reorder` / `tag.update` → the in-process `kanban` operation tool
//!   (exposed over a temp board root via `register_kanban_tools` /
//!   `build_tool_modules`, the same triple the production app uses);
//! - `attachment.open` / `attachment.reveal` → the in-process `window`
//!   operation tool, here wrapped over a recording `SpyShell` so the OS file
//!   actions are observable without a live file manager (the window-service
//!   tests' `WindowShell` spy seam);
//! - `view.set` → the in-process `views` operation tool, wired over a real
//!   `PerspectiveContext` + `ViewsContext` substrate (the views-service tests'
//!   substrate).
//!
//! Each `ServerHandler` backend (`window`, `views`) is wrapped in an
//! [`InProcessServer`] to satisfy `expose_rust_module`'s `McpServer` contract;
//! the `kanban` tool already implements `McpServer` via the tool bridge.
//!
//! What a passing run proves:
//!
//! 1. **Discovery + registration** — after load, the five ported commands are
//!    registered.
//! 2. **Metadata fidelity** — each command's `scope` / `params` / `undoable` /
//!    `visible` / `context_menu` match the source-YAML baseline 1:1.
//! 3. **Real effect** — executing each command produces its observable effect:
//!    `column.reorder` + `tag.update` mutate the kanban store; `attachment.open`
//!    / `attachment.reveal` drive the recorded shell calls; `view.set` writes a
//!    `ViewDef` through the views kernel.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_config::ModelConfig;
use swissarmyhammer_directory::KanbanConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_perspectives::{PerspectiveContext, PerspectiveStore};
use swissarmyhammer_plugin::{
    CallerId, InProcessServer, McpServer as PluginMcpServer, PluginHost, PLUGINS_SUBDIR,
};
use swissarmyhammer_store::{StoreContext, StoreHandle};
use swissarmyhammer_tools::mcp::plugin_bridge::build_tool_modules;
use swissarmyhammer_tools::mcp::ToolHandlers;
use swissarmyhammer_tools::{register_kanban_tools, ToolContext, ToolRegistry};
use swissarmyhammer_views::{ViewStore, ViewsContext, ViewsServer};
use swissarmyhammer_window_service::{
    CreatedBoard, MonitorInfo, NewWindow, OpenedBoard, WindowPosition, WindowService, WindowShell,
};
use tempfile::TempDir;
use tokio::sync::{Mutex as TokioMutex, RwLock};

use crate::support::call_command;

/// A generous upper bound on any single host or isolate interaction.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

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

/// Stage the committed `builtin/plugins/kanban-misc-commands` bundle into a temp
/// builtin-layer root so `discover_and_load_all` finds it at
/// `<layer_root>/plugins/kanban-misc-commands/`.
fn stage_kanban_misc_commands(layer_root: &Path) {
    let source = workspace_root()
        .join("builtin/plugins")
        .join("kanban-misc-commands");
    assert!(
        source.is_dir(),
        "the committed kanban-misc-commands bundle must exist at {}",
        source.display()
    );
    let destination = layer_root.join(PLUGINS_SUBDIR).join("kanban-misc-commands");
    copy_dir_recursive(&source, &destination);
}

// ───────────────────────────────────────────────────────────────────────────
// Exposing the real in-process kanban tool (mirrors the kanban app's wiring)
// ───────────────────────────────────────────────────────────────────────────

/// A handle to the in-process `kanban` operation tool exposed for the test.
struct ExposedKanban {
    _registry: Arc<RwLock<ToolRegistry>>,
    _context: Arc<ToolContext>,
    module_id: String,
    module: Arc<dyn PluginMcpServer>,
}

impl ExposedKanban {
    /// Expose the wrapped `kanban` module to `host` under its module id.
    async fn expose_to(&self, host: &PluginHost) {
        host.expose_rust_module(self.module_id.clone(), Arc::clone(&self.module))
            .await
            .expect("exposing the kanban module should succeed");
    }

    /// Invoke the `kanban` tool directly with an arguments object.
    async fn call(&self, args: Value) -> Value {
        self.module
            .invoke(CallerId::HostInternal, &self.module_id, args)
            .await
            .expect("a direct kanban call should succeed")
    }
}

/// Build and return the in-process `kanban` operation tool rooted at
/// `board_root`, wired exactly as `apps/kanban-app/src/plugins.rs` does.
async fn expose_kanban_module(board_root: &Path) -> ExposedKanban {
    let mut registry = ToolRegistry::new();
    register_kanban_tools(&mut registry);
    let registry = Arc::new(RwLock::new(registry));

    let git_ops = Arc::new(TokioMutex::new(None::<GitOperations>));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ModelConfig::default());
    let context = ToolContext::new(tool_handlers, git_ops, agent_config)
        .with_tool_registry(Arc::clone(&registry))
        .with_working_dir(board_root.to_path_buf());
    let context = Arc::new(context);

    let modules = build_tool_modules(Arc::clone(&registry), Arc::clone(&context)).await;
    let mut modules = modules.into_iter();
    let (module_id, module) = modules
        .next()
        .expect("the kanban registry must yield its one tool module");
    assert!(
        modules.next().is_none(),
        "the kanban-only registry must expose exactly one module",
    );

    ExposedKanban {
        _registry: registry,
        _context: context,
        module_id,
        module,
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Exposing the real in-process `window` tool over a recording spy shell
// ───────────────────────────────────────────────────────────────────────────

/// A recording [`WindowShell`] that captures `open_path` / `reveal_path` so the
/// test can assert the ported `attachment.open` / `attachment.reveal` commands
/// reached the OS file actions with the expected path. Every other shell method
/// is an inert stub: the kanban-misc commands only drive the two OS-file verbs.
struct SpyShell {
    /// Ordered log of `<method>:<arg>` tags, one per call.
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

    fn record(&self, tag: impl Into<String>) {
        self.calls.lock().unwrap().push(tag.into());
    }
}

impl WindowShell for SpyShell {
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

    fn open_path(&self, path: &str) -> Result<(), String> {
        self.record(format!("open_path:{path}"));
        Ok(())
    }

    fn reveal_path(&self, path: &str) -> Result<(), String> {
        self.record(format!("reveal_path:{path}"));
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
}

/// Expose the `window` tool (over the spy shell) to `host` under id `"window"`,
/// returning the shared spy so the test can read back the recorded calls.
async fn expose_window_module(host: &PluginHost) -> Arc<SpyShell> {
    let shell = Arc::new(SpyShell::new());
    let service = WindowService::new(Arc::clone(&shell) as Arc<dyn WindowShell>);
    let module = InProcessServer::new(service)
        .await
        .expect("wrapping the window service in an InProcessServer should succeed");
    host.expose_rust_module("window".to_string(), Arc::new(module) as Arc<dyn PluginMcpServer>)
        .await
        .expect("exposing the window module should succeed");
    shell
}

// ───────────────────────────────────────────────────────────────────────────
// Exposing the real in-process `views` tool over a real kernel substrate
// ───────────────────────────────────────────────────────────────────────────

/// A handle to the live views substrate, kept alive for the test's duration so
/// the storage root and shared contexts outlive the plugin's `load()` and every
/// `execute`.
struct ExposedViews {
    _dir: TempDir,
    _store_ctx: Arc<StoreContext>,
    views: Arc<RwLock<ViewsContext>>,
}

/// Build a `views` substrate (mirroring `wire_store_substrate`), wrap a
/// `ViewsServer` over it in an `InProcessServer`, and expose it to `host` under
/// id `"views"`.
async fn expose_views_module(host: &PluginHost) -> ExposedViews {
    let dir = TempDir::new().expect("views substrate temp dir");
    let store_ctx = Arc::new(StoreContext::new(dir.path().to_path_buf()));

    // Perspective context + store.
    let perspectives_dir = dir.path().join("perspectives");
    let perspective_ctx = PerspectiveContext::open(&perspectives_dir)
        .await
        .expect("perspective context should open");
    let perspective_store = PerspectiveStore::new(&perspectives_dir);
    let p_handle = Arc::new(StoreHandle::new(Arc::new(perspective_store)));
    store_ctx.register(p_handle.clone()).await;
    let perspectives = {
        let mut pctx = perspective_ctx;
        pctx.set_store_handle(p_handle);
        pctx.set_store_context(Arc::clone(&store_ctx));
        Arc::new(RwLock::new(pctx))
    };

    // Views context + store.
    let views_dir = dir.path().join("views");
    let views_ctx = ViewsContext::open(&views_dir)
        .build()
        .await
        .expect("views context should open");
    let view_store = ViewStore::new(&views_dir);
    let v_handle = Arc::new(StoreHandle::new(Arc::new(view_store)));
    store_ctx.register(v_handle.clone()).await;
    let views = {
        let mut vctx = views_ctx;
        vctx.set_store_handle(v_handle);
        vctx.set_store_context(Arc::clone(&store_ctx));
        Arc::new(RwLock::new(vctx))
    };

    let server = ViewsServer::new(Arc::clone(&perspectives), Arc::clone(&views));
    let module = InProcessServer::new(server)
        .await
        .expect("wrapping the views server in an InProcessServer should succeed");
    host.expose_rust_module("views".to_string(), Arc::new(module) as Arc<dyn PluginMcpServer>)
        .await
        .expect("exposing the views module should succeed");

    ExposedViews {
        _dir: dir,
        _store_ctx: store_ctx,
        views,
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Result-shape helpers
// ───────────────────────────────────────────────────────────────────────────

/// Parse the `content[0].text` JSON payload out of a kanban `CallToolResult`.
fn kanban_payload(result: &Value) -> Value {
    let text = result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|content| content.first())
        .and_then(|entry| entry.get("text"))
        .and_then(Value::as_str)
        .expect("a kanban result must carry text content");
    serde_json::from_str(text).expect("kanban content must be JSON")
}

/// Read the `order` of the column with `column_id` off a `list columns` result.
fn column_order(list_result: &Value, column_id: &str) -> Option<u64> {
    kanban_payload(list_result)
        .get("columns")
        .and_then(Value::as_array)
        .expect("list columns must carry a `columns` array")
        .iter()
        .find(|c| c.get("id").and_then(Value::as_str) == Some(column_id))
        .and_then(|c| c.get("order").and_then(Value::as_u64))
}

/// Read a tag's `name` off a `get tag` result.
fn tag_name(get_result: &Value) -> Option<String> {
    kanban_payload(get_result)
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string)
}

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

/// The committed `kanban-misc-commands` builtin plugin, discovered from a
/// builtin layer, registers all five YAML commands with 1:1 metadata and
/// produces each command's real effect across the kanban / window / views
/// backends.
#[tokio::test]
async fn kanban_misc_commands_plugin_registers_and_executes() {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");
    let board_dir = TempDir::new().expect("kanban board temp dir");

    // Stage the committed bundle into the builtin layer's plugins/ dir.
    stage_kanban_misc_commands(builtin_root.path());

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

    // Expose the three backends BEFORE discovery, so the plugin's
    // `ensureServices(this, ["commands", "kanban", "window", "views"])` finds
    // them already exposed.
    let kanban = expose_kanban_module(board_dir.path()).await;
    tokio::time::timeout(TIMEOUT, kanban.expose_to(&host))
        .await
        .expect("exposing kanban should not hang");
    let window_spy = tokio::time::timeout(TIMEOUT, expose_window_module(&host))
        .await
        .expect("exposing window should not hang");
    let views = tokio::time::timeout(TIMEOUT, expose_views_module(&host))
        .await
        .expect("exposing views should not hang");

    // Seed a board with three default columns (todo/doing/done) and one tag.
    kanban
        .call(json!({ "op": "init board", "name": "Kanban Misc Board" }))
        .await;
    let added_tag = kanban
        .call(json!({ "op": "add tag", "name": "bug" }))
        .await;
    let tag_id = kanban_payload(&added_tag)
        .get("id")
        .and_then(Value::as_str)
        .expect("add tag must return the new tag id")
        .to_string();

    // Discover + load the builtin layer: runs the bundle's `load()`, which
    // registers the five commands through the SDK convention.
    let loaded = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the kanban-misc-commands builtin plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one kanban-misc-commands builtin plugin should be discovered, got {loaded:?}"
    );

    // ── (1) Discovery + registration: list every command ───────────────────
    // `list command` with no scope filter returns every registered command,
    // hidden (`visible: false`) ones included — the registry's list filters
    // only on scope / category / id-prefix, not visibility.
    let listed = call_command(
        &service,
        CallerId::HostInternal,
        json!({ "op": "list command" }),
    )
    .await;
    let commands = commands_by_id(&listed);
    for id in [
        "column.reorder",
        "attachment.open",
        "attachment.reveal",
        "tag.update",
        "view.set",
    ] {
        assert!(
            commands.contains_key(id),
            "list command must include the ported command {id:?}; got {:?}",
            commands.keys().collect::<Vec<_>>()
        );
    }

    // ── (2) Metadata fidelity: lock each command's metadata 1:1 vs the YAML ─
    assert_column_reorder_metadata(&commands["column.reorder"]);
    assert_attachment_open_metadata(&commands["attachment.open"]);
    assert_attachment_reveal_metadata(&commands["attachment.reveal"]);
    assert_tag_update_metadata(&commands["tag.update"]);
    assert_view_set_metadata(&commands["view.set"]);

    // ── (3a) Real effect: column.reorder → kanban `update column` ──────────
    // `doing` starts at order 1; reorder it to index 0.
    let before = kanban.call(json!({ "op": "list columns" })).await;
    assert_eq!(
        column_order(&before, "doing"),
        Some(1),
        "the seeded `doing` column should start at order 1"
    );
    let reorder = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "column.reorder",
            "ctx": { "args": { "id": "doing", "target_index": 0 } },
        }),
    )
    .await;
    assert_eq!(
        reorder["structuredContent"]["ok"],
        json!(true),
        "executing column.reorder should succeed, got {reorder}"
    );
    let after = kanban.call(json!({ "op": "list columns" })).await;
    assert_eq!(
        column_order(&after, "doing"),
        Some(0),
        "column.reorder must have written `doing`'s new order through the kanban store"
    );

    // ── (3b) Real effect: tag.update → kanban `update tag` ─────────────────
    let tag_update = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "tag.update",
            "ctx": {
                "scope_chain": [format!("tag:{tag_id}")],
                "args": { "name": "defect" },
            },
        }),
    )
    .await;
    assert_eq!(
        tag_update["structuredContent"]["ok"],
        json!(true),
        "executing tag.update should succeed, got {tag_update}"
    );
    let got_tag = kanban
        .call(json!({ "op": "get tag", "id": tag_id }))
        .await;
    assert_eq!(
        tag_name(&got_tag).as_deref(),
        Some("defect"),
        "tag.update must have renamed the tag through the kanban store"
    );

    // ── (3c) Real effect: attachment.open / .reveal → window OS file actions
    let attach_path = "/tmp/kanban-misc-attachment.pdf";
    let open = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "attachment.open",
            "ctx": { "scope_chain": [format!("attachment:{attach_path}")] },
        }),
    )
    .await;
    assert_eq!(
        open["structuredContent"]["ok"],
        json!(true),
        "executing attachment.open should succeed, got {open}"
    );
    let reveal = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "attachment.reveal",
            "ctx": { "scope_chain": [format!("attachment:{attach_path}")] },
        }),
    )
    .await;
    assert_eq!(
        reveal["structuredContent"]["ok"],
        json!(true),
        "executing attachment.reveal should succeed, got {reveal}"
    );
    assert_eq!(
        window_spy.calls(),
        vec![
            format!("open_path:{attach_path}"),
            format!("reveal_path:{attach_path}"),
        ],
        "attachment.open / attachment.reveal must drive the window shell's OS file actions"
    );

    // ── (3d) Real effect: view.set → views `set view` ──────────────────────
    let view_id = "01VIEWSETTEST00000000000000";
    let set_view = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "view.set",
            "ctx": { "args": { "view_id": view_id } },
        }),
    )
    .await;
    assert_eq!(
        set_view["structuredContent"]["ok"],
        json!(true),
        "executing view.set should succeed, got {set_view}"
    );
    // The execute envelope is `{ ok, result: <plugin return> }`; the plugin's
    // single `views set view` call returns that backend's full `CallToolResult`
    // (`{ content, structuredContent: { ok, view, entry_id }, isError }`). The
    // written view id therefore lives under
    // `structuredContent.result.structuredContent.view.id`.
    let view_id_value =
        &set_view["structuredContent"]["result"]["structuredContent"]["view"]["id"];
    assert_eq!(
        view_id_value,
        &json!(view_id),
        "view.set must have written the view through the views kernel, got {set_view}"
    );
    // And the views kernel actually holds the written view.
    let stored_has_view = views
        .views
        .read()
        .await
        .all_views()
        .iter()
        .any(|v| v.id == view_id);
    assert!(
        stored_has_view,
        "view.set must have persisted the view in the views kernel"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// Per-command metadata regression asserts (locked against the source YAMLs)
// ───────────────────────────────────────────────────────────────────────────

/// `column.reorder` — column.yaml: undoable, visible:false, no scope/keys/
/// context_menu; params id(args) / target_index(args).
fn assert_column_reorder_metadata(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Reorder Columns"), "column.reorder name");
    assert_eq!(cmd["undoable"], json!(true), "column.reorder undoable");
    assert_eq!(cmd["visible"], json!(false), "column.reorder visible:false");
    assert!(
        cmd.get("scope").is_none() || cmd["scope"].is_null() || cmd["scope"] == json!([]),
        "column.reorder carries no scope, got {}",
        cmd["scope"]
    );
    assert!(
        cmd.get("context_menu").is_none() || cmd["context_menu"] == json!(false),
        "column.reorder carries no context_menu"
    );
    assert_eq!(
        cmd["params"],
        json!([
            { "name": "id", "from": "args" },
            { "name": "target_index", "from": "args" },
        ]),
        "column.reorder params must match column.yaml 1:1"
    );
}

/// `attachment.open` — attachment.yaml: scope "attachment", context_menu, no
/// undoable/params.
fn assert_attachment_open_metadata(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Open"), "attachment.open name");
    assert_eq!(
        cmd["scope"],
        json!(["attachment"]),
        "attachment.open scope (the YAML's `attachment` as a list)"
    );
    assert_eq!(
        cmd["context_menu"],
        json!(true),
        "attachment.open context_menu"
    );
    assert!(
        cmd.get("undoable").is_none() || cmd["undoable"] == json!(false),
        "attachment.open is not undoable"
    );
    assert!(
        cmd.get("params").is_none() || cmd["params"] == json!([]),
        "attachment.open carries no params, got {}",
        cmd["params"]
    );
}

/// `attachment.reveal` — attachment.yaml: scope "attachment", context_menu.
fn assert_attachment_reveal_metadata(cmd: &Value) {
    assert_eq!(
        cmd["name"],
        json!("Show in Finder"),
        "attachment.reveal name"
    );
    assert_eq!(
        cmd["scope"],
        json!(["attachment"]),
        "attachment.reveal scope"
    );
    assert_eq!(
        cmd["context_menu"],
        json!(true),
        "attachment.reveal context_menu"
    );
    assert!(
        cmd.get("undoable").is_none() || cmd["undoable"] == json!(false),
        "attachment.reveal is not undoable"
    );
    assert!(
        cmd.get("params").is_none() || cmd["params"] == json!([]),
        "attachment.reveal carries no params"
    );
}

/// `tag.update` — tag.yaml: scope entity:tag, undoable, visible:false; param
/// id(scope_chain, entity_type tag).
fn assert_tag_update_metadata(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Update Tag"), "tag.update name");
    assert_eq!(cmd["scope"], json!(["entity:tag"]), "tag.update scope");
    assert_eq!(cmd["undoable"], json!(true), "tag.update undoable");
    assert_eq!(cmd["visible"], json!(false), "tag.update visible:false");
    assert_eq!(
        cmd["params"],
        json!([{ "name": "id", "from": "scope_chain", "entity_type": "tag" }]),
        "tag.update params must match tag.yaml 1:1"
    );
}

/// `view.set` — view.yaml: visible:false, no scope/keys/context_menu; param
/// view_id(args).
fn assert_view_set_metadata(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Switch View"), "view.set name");
    assert_eq!(cmd["visible"], json!(false), "view.set visible:false");
    assert!(
        cmd.get("scope").is_none() || cmd["scope"].is_null() || cmd["scope"] == json!([]),
        "view.set carries no scope, got {}",
        cmd["scope"]
    );
    assert!(
        cmd.get("undoable").is_none() || cmd["undoable"] == json!(false),
        "view.set carries no undoable flag"
    );
    assert_eq!(
        cmd["params"],
        json!([{ "name": "view_id", "from": "args" }]),
        "view.set params must match view.yaml 1:1"
    );
}
