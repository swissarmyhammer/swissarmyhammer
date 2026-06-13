//! End-to-end test for the committed `perspective-commands` builtin plugin.
//!
//! This is the acceptance for the largest of the builtin command-plugin ports:
//! `perspective.yaml`'s seventeen commands into the one
//! `builtin/plugins/perspective-commands/` bundle (split into one helper module
//! per sub-domain — lifecycle / filter / group / sort / nav). Every command
//! routes to the in-process `views` operation tool, wired over a real
//! `PerspectiveContext` + `ViewsContext` substrate (the views-service tests'
//! substrate, mirroring `builtin_kanban_misc_e2e`).
//!
//! What a passing run proves:
//!
//! 1. **Discovery + registration** — after load, all seventeen ported commands
//!    are registered.
//! 2. **Metadata fidelity** — a table-test asserts every command's YAML field
//!    (`name` / `scope` / `undoable` / `visible` / `context_menu` /
//!    `view_kinds` / `tab_button` / `keys` / `params`) survives the port 1:1.
//! 3. **Real effect** — one representative command from EACH sub-domain is
//!    executed and the perspective state change is observed through the live
//!    perspective kernel: a save→load roundtrip (lifecycle), set filter
//!    (filter), set group (group), set sort (sort), and goto + switch (nav).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_directory::KanbanConfig;
use swissarmyhammer_entity_mcp::EntityServer;
use swissarmyhammer_kanban::board::InitBoard;
use swissarmyhammer_kanban::clipboard::{ClipboardProvider, InMemoryClipboard};
use swissarmyhammer_kanban::{KanbanContext, KanbanOperationProcessor, OperationProcessor};
use swissarmyhammer_perspectives::PerspectiveContext;
use swissarmyhammer_plugin::{
    CallerId, InProcessServer, McpServer as PluginMcpServer, PluginHost, PLUGINS_SUBDIR,
};
use swissarmyhammer_store::StoreContext;
use swissarmyhammer_ui_state::UIState;
use swissarmyhammer_views::{ViewsContext, ViewsServer};
use tempfile::TempDir;
use tokio::sync::RwLock;

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

/// Stage the committed `builtin/plugins/perspective-commands` bundle (entry +
/// `commands/` sub-modules) into a temp builtin-layer root so
/// `discover_and_load_all` finds it at
/// `<layer_root>/plugins/perspective-commands/`.
fn stage_perspective_commands(layer_root: &Path) {
    let source = workspace_root()
        .join("builtin/plugins")
        .join("perspective-commands");
    assert!(
        source.is_dir(),
        "the committed perspective-commands bundle must exist at {}",
        source.display()
    );
    let destination = layer_root.join(PLUGINS_SUBDIR).join("perspective-commands");
    copy_dir_recursive(&source, &destination);
}

// ───────────────────────────────────────────────────────────────────────────
// Exposing the real in-process `views` + `entity` tools over ONE board
// substrate
// ───────────────────────────────────────────────────────────────────────────

/// A handle to the live board substrate, kept alive for the test's duration
/// so the storage root and shared contexts outlive the plugin's `load()` and
/// every `execute`.
struct ExposedViews {
    _dir: TempDir,
    _store_ctx: Arc<StoreContext>,
    perspectives: Arc<RwLock<PerspectiveContext>>,
    views: Arc<RwLock<ViewsContext>>,
    /// The shared per-window UI state the activation commands
    /// (`perspective.switch` / `.next` / `.prev`) write through the exposed
    /// `entity` module — observed directly by the activation pins.
    ui_state: Arc<UIState>,
}

/// Build ONE `KanbanContext` board substrate and expose BOTH backends the
/// plugin's `ensureServices(this, ["commands", "views", "entity"])` needs:
///
/// - `views` — `ViewsServer` over the board's own perspective + views
///   kernels (resolution + perspective CRUD).
/// - `entity` — clipboard-wired `EntityServer` over the same
///   `KanbanContext` plus a shared `UIState` (the board-bundle server the
///   three activation commands route to).
///
/// Sharing one substrate mirrors the production wiring in
/// `apps/kanban-app/src/commands.rs`, where both modules resolve the SAME
/// active board's kernels.
async fn expose_views_module(host: &PluginHost) -> ExposedViews {
    let dir = TempDir::new().expect("board substrate temp dir");
    let kanban_dir = dir.path().join(".kanban");
    std::fs::create_dir_all(&kanban_dir).expect("kanban dir");
    let kanban = KanbanContext::open(&kanban_dir)
        .await
        .expect("kanban context should open");
    KanbanOperationProcessor::new()
        .process(&InitBoard::new("Perspective Commands Board"), &kanban)
        .await
        .expect("board init");
    let kanban = Arc::new(kanban);
    let store_ctx = swissarmyhammer_kanban::wire_store_substrate(&kanban).await;

    let perspectives = kanban
        .perspective_context_arc()
        .await
        .expect("perspective context should open");
    let views = kanban.views_arc().expect("views context should open");

    let server = ViewsServer::new(Arc::clone(&perspectives), Arc::clone(&views));
    let module = InProcessServer::new(server)
        .await
        .expect("wrapping the views server in an InProcessServer should succeed");
    host.expose_rust_module(
        "views".to_string(),
        Arc::new(module) as Arc<dyn PluginMcpServer>,
    )
    .await
    .expect("exposing the views module should succeed");

    let ui_state = Arc::new(UIState::new());
    let entity_server = EntityServer::with_clipboard(
        Arc::clone(&kanban),
        Arc::new(InMemoryClipboard::new()) as Arc<dyn ClipboardProvider>,
        Arc::clone(&ui_state),
    )
    .await
    .expect("board-wired entity server");
    let entity_module = InProcessServer::new(entity_server)
        .await
        .expect("wrapping the entity server in an InProcessServer should succeed");
    host.expose_rust_module(
        "entity".to_string(),
        Arc::new(entity_module) as Arc<dyn PluginMcpServer>,
    )
    .await
    .expect("exposing the entity module should succeed");

    ExposedViews {
        _dir: dir,
        _store_ctx: store_ctx,
        perspectives,
        views,
        ui_state,
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

/// The execute envelope is `{ ok, result: <plugin return> }`; the plugin's one
/// `views` call returns that backend's full `CallToolResult`
/// (`{ content, structuredContent: <op payload>, isError }`). So the op's JSON
/// payload lives under `structuredContent.result.structuredContent`.
///
/// Exception: `perspective.save` and `perspective.list` unwrap the views
/// envelope in the plugin (`unwrapResult`) because the frontend reads their
/// payloads off the dispatch result — for those two the op payload sits at
/// `structuredContent.result` directly.
fn op_payload(execute_result: &Value) -> &Value {
    &execute_result["structuredContent"]["result"]["structuredContent"]
}

// ───────────────────────────────────────────────────────────────────────────
// The test
// ───────────────────────────────────────────────────────────────────────────

/// The committed `perspective-commands` builtin plugin, discovered from a
/// builtin layer, registers all seventeen YAML commands with 1:1 metadata and
/// produces each sub-domain's real effect against the live perspective kernel.
#[tokio::test]
async fn perspective_commands_plugin_registers_and_executes() {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");

    // Stage the committed bundle into the builtin layer's plugins/ dir.
    stage_perspective_commands(builtin_root.path());

    // A host whose lowest-precedence builtin layer is the staged root.
    let host = PluginHost::new(
        Some(builtin_root.path().to_path_buf()),
        user_root.path().to_path_buf(),
        None,
        user_root.path().to_path_buf(),
        false,
        user_root.path().to_path_buf(),
    );

    // Bootstrap the command service into the host (exposes `commands`).
    let service = install_commands_module(&host)
        .await
        .expect("install_commands_module must succeed");

    // Expose the `views` backend BEFORE discovery, so the plugin's
    // `ensureServices(this, ["commands", "views"])` finds it already exposed.
    let views = tokio::time::timeout(TIMEOUT, expose_views_module(&host))
        .await
        .expect("exposing views should not hang");

    // Discover + load the builtin layer: runs the bundle's `load()`, which
    // registers the seventeen commands through the SDK convention.
    let loaded = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the perspective-commands builtin plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one perspective-commands builtin plugin should be discovered, got {loaded:?}"
    );

    // ── (1) Discovery + registration: list every command ───────────────────
    let listed = call_command(
        &service,
        CallerId::HostInternal,
        json!({ "op": "list command" }),
    )
    .await;
    let commands = commands_by_id(&listed);
    for id in PERSPECTIVE_COMMAND_IDS {
        assert!(
            commands.contains_key(id),
            "list command must include the ported command {id:?}; got {:?}",
            commands.keys().collect::<Vec<_>>()
        );
    }

    // ── (2) Metadata fidelity: lock all seventeen vs the YAML (table-test) ──
    for spec in metadata_specs() {
        let cmd = &commands[spec.id];
        assert_command_metadata(cmd, &spec);
    }

    // Menu placement (card 01KTYQY0ZB62KHN6BPK3FBMBD7): the cycling pair
    // surfaces on the OS View menu — the menu whose existing occupant
    // (`ai.toggle`) also changes what the window shows. The MetaSpec table
    // predates the `menu` field, so the pair is pinned explicitly here.
    assert_eq!(
        commands["perspective.next"]["menu"],
        json!({ "path": ["View"], "group": 1, "order": 0 }),
        "perspective.next menu placement"
    );
    assert_eq!(
        commands["perspective.prev"]["menu"],
        json!({ "path": ["View"], "group": 1, "order": 1 }),
        "perspective.prev menu placement"
    );

    // ── (3a) lifecycle: save → load roundtrip ───────────────────────────────
    // Save a perspective and capture its minted id off the op payload.
    let saved = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "perspective.save",
            "ctx": {
                "scope_chain": ["view:01VIEWINSTANCE0000000000000"],
                "args": { "name": "Active Sprint", "view": "grid" },
            },
        }),
    )
    .await;
    assert_eq!(
        saved["structuredContent"]["ok"],
        json!(true),
        "perspective.save should succeed, got {saved}"
    );
    // `perspective.save` unwraps the views envelope (`unwrapResult`, same
    // precedent as `perspective.list`) so the frontend's `+` flow can read
    // the created `perspective.id` straight off the dispatch result — its
    // payload therefore sits at `structuredContent.result`, NOT at the
    // double-nested `op_payload` location the un-unwrapped commands use.
    let saved_payload = &saved["structuredContent"]["result"];
    let persp_id = saved_payload["perspective"]["id"]
        .as_str()
        .expect("save must return the minted perspective id")
        .to_string();
    // The view_id scope-chain param threaded through to the saved perspective.
    assert_eq!(
        saved_payload["perspective"]["view_id"],
        json!("01VIEWINSTANCE0000000000000"),
        "perspective.save must resolve view_id from the view: scope moniker"
    );
    // Load it back by name and confirm the same perspective comes through.
    let loaded_p = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "perspective.load",
            "ctx": { "args": { "name": "Active Sprint" } },
        }),
    )
    .await;
    assert_eq!(
        op_payload(&loaded_p)["perspective"]["id"],
        json!(persp_id),
        "perspective.load must round-trip the saved perspective, got {loaded_p}"
    );

    // ── (3b) filter: set filter ─────────────────────────────────────────────
    let set_filter = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "perspective.filter",
            "ctx": {
                "scope_chain": [format!("perspective:{persp_id}")],
                "args": { "perspective_id": persp_id, "filter": "#bug" },
            },
        }),
    )
    .await;
    assert_eq!(
        set_filter["structuredContent"]["ok"],
        json!(true),
        "perspective.filter should succeed, got {set_filter}"
    );
    assert_eq!(
        views
            .perspectives
            .read()
            .await
            .get_by_id(&persp_id)
            .unwrap()
            .filter
            .as_deref(),
        Some("#bug"),
        "perspective.filter must have written the filter through the perspective kernel"
    );

    // ── (3c) group: set group ───────────────────────────────────────────────
    let set_group = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "perspective.group",
            "ctx": {
                "scope_chain": [format!("perspective:{persp_id}")],
                "args": { "group": "status" },
            },
        }),
    )
    .await;
    assert_eq!(
        set_group["structuredContent"]["ok"],
        json!(true),
        "perspective.group should succeed, got {set_group}"
    );
    assert_eq!(
        views
            .perspectives
            .read()
            .await
            .get_by_id(&persp_id)
            .unwrap()
            .group
            .as_deref(),
        Some("status"),
        "perspective.group must have written the group field through the perspective kernel"
    );

    // ── (3d) sort: set sort ─────────────────────────────────────────────────
    let set_sort = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "perspective.sort.set",
            "ctx": {
                "scope_chain": [format!("perspective:{persp_id}")],
                "args": { "field": "priority", "direction": "desc" },
            },
        }),
    )
    .await;
    assert_eq!(
        set_sort["structuredContent"]["ok"],
        json!(true),
        "perspective.sort.set should succeed, got {set_sort}"
    );
    {
        let pctx = views.perspectives.read().await;
        let sort = &pctx.get_by_id(&persp_id).unwrap().sort;
        assert_eq!(sort.len(), 1, "one sort entry expected, got {sort:?}");
        assert_eq!(sort[0].field, "priority", "sort field");
    }

    // ── (3e) nav: goto + switch ──────────────────────────────────────────────
    let goto = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "perspective.goto",
            "ctx": { "args": { "id": persp_id } },
        }),
    )
    .await;
    assert_eq!(
        op_payload(&goto)["perspective"]["id"],
        json!(persp_id),
        "perspective.goto must resolve the perspective by id, got {goto}"
    );
    let switched = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "perspective.switch",
            "ctx": {
                "scope_chain": ["window:main"],
                "args": { "perspective_id": persp_id },
            },
        }),
    )
    .await;
    assert_eq!(
        switched["structuredContent"]["ok"],
        json!(true),
        "perspective.switch should succeed, got {switched}"
    );
    // switch ACTIVATES: the window's active perspective flips and the
    // atomic PerspectiveSwitch change surfaces on the result envelope.
    assert_eq!(
        views.ui_state.active_perspective_id("main"),
        persp_id,
        "perspective.switch must write the window's active_perspective_id"
    );
    assert_eq!(
        op_payload(&switched)["change"]["PerspectiveSwitch"]["perspective_id"],
        json!(persp_id),
        "perspective.switch must return the PerspectiveSwitch change, got {switched}"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// Live-regression pins (01KTY6T1GPY94VYWANE9X41SKJ)
// ───────────────────────────────────────────────────────────────────────────

/// A booted perspective-commands plugin over a live views substrate, with
/// every temp root kept alive for the test's duration.
struct PluginFixture {
    _user_root: TempDir,
    _builtin_root: TempDir,
    _host: PluginHost,
    service: Arc<swissarmyhammer_command_service::CommandService>,
    views: ExposedViews,
}

/// Stage the committed bundle, boot a host, install the command service,
/// expose the live `views` substrate, and load the plugin.
async fn boot_perspective_plugin() -> PluginFixture {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");
    stage_perspective_commands(builtin_root.path());

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
    let views = tokio::time::timeout(TIMEOUT, expose_views_module(&host))
        .await
        .expect("exposing views should not hang");
    let loaded = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the perspective-commands builtin plugin should succeed");
    assert_eq!(loaded.len(), 1, "one plugin expected, got {loaded:?}");

    PluginFixture {
        _user_root: user_root,
        _builtin_root: builtin_root,
        _host: host,
        service,
        views,
    }
}

/// `perspective.list` must return the op's JSON payload — NOT the raw
/// `CallToolResult` wire envelope of the plugin's `views` call.
///
/// The frontend (`usePerspectivesFetch` in
/// `apps/kanban-app/ui/src/lib/perspective-context.tsx`) reads
/// `result.perspectives` off the dispatch result. When the plugin port
/// returned the envelope verbatim, that read was always `undefined`, the
/// window's perspectives state stayed empty forever, and the tab bar showed
/// NO perspectives at all — the user-visible "Default perspectives gone
/// missing" live regression.
#[tokio::test]
async fn perspective_list_returns_the_op_payload_for_the_frontend() {
    let fx = boot_perspective_plugin().await;

    // Seed one perspective through the real command path.
    let saved = call_command(
        &fx.service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "perspective.save",
            "ctx": { "args": { "name": "Ready", "view": "board" } },
        }),
    )
    .await;
    assert_eq!(saved["structuredContent"]["ok"], json!(true));

    let listed = call_command(
        &fx.service,
        CallerId::HostInternal,
        json!({ "op": "execute command", "id": "perspective.list", "ctx": {} }),
    )
    .await;
    let result = &listed["structuredContent"]["result"];
    let perspectives = result
        .get("perspectives")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            panic!(
                "perspective.list must surface `perspectives` directly on the \
                 command result (the frontend reads `result.perspectives`), got {listed}"
            )
        });
    // The board substrate's open-reconciliation seeds a "Default"
    // perspective for the built-in Board view, so the saved one is found by
    // name rather than by exact count.
    assert!(
        perspectives.iter().any(|p| p["name"] == json!("Ready")),
        "the saved perspective must appear in the list payload, got {listed}"
    );
}

/// `perspective.save` with `if_absent` through the REAL plugin path must
/// converge on one default — even when the dispatching window's scope chain
/// carries the frontend's `"default"` placeholder view id (the exact live
/// shape: views not yet loaded → `view:default` scope moniker → a Default
/// pinned to a nonexistent view minted per window per boot).
#[tokio::test]
async fn if_absent_save_through_the_plugin_converges_on_one_default() {
    let fx = boot_perspective_plugin().await;

    // A real view exists, so the views registry is authoritative and the
    // `"default"` placeholder is verifiably dead.
    {
        let mut views = fx.views.views.write().await;
        let def = swissarmyhammer_views::ViewDef {
            id: "01JMVIEW0000000000BOARD0".to_string(),
            name: "Board".to_string(),
            icon: None,
            kind: swissarmyhammer_views::ViewKind::Board,
            entity_type: Some("task".to_string()),
            card_fields: Vec::new(),
            commands: Vec::new(),
        };
        views
            .write_view(&def)
            .await
            .expect("seeding the board view should succeed");
    }

    let ensure = json!({
        "op": "execute command",
        "id": "perspective.save",
        "ctx": {
            "scope_chain": ["view:default"],
            "args": { "name": "Default", "view": "board", "if_absent": true },
        },
    });
    // Two windows / two boots dispatching the same auto-create.
    let first = call_command(&fx.service, CallerId::HostInternal, ensure.clone()).await;
    assert_eq!(first["structuredContent"]["ok"], json!(true));
    let second = call_command(&fx.service, CallerId::HostInternal, ensure).await;
    assert_eq!(second["structuredContent"]["ok"], json!(true));

    let pctx = fx.views.perspectives.read().await;
    let all = pctx.all();
    // The board substrate's open-reconciliation already seeded the Board
    // view's pinned Default; the dead `default` placeholder must fall back
    // to the kind scope and CONVERGE on that existing perspective — never
    // mint a kind-scoped sibling per window per boot (the create/prune
    // churn this regression test pins).
    assert_eq!(
        all.len(),
        1,
        "repeated if_absent saves must converge on ONE default, got {all:?}"
    );
    assert_eq!(all[0].name, "Default");
    let converged_id = all[0].id.clone();
    assert_eq!(
        first["structuredContent"]["result"]["perspective"]["id"],
        json!(converged_id),
        "the first ensure must resolve to the one converged default"
    );
    assert_eq!(
        second["structuredContent"]["result"]["perspective"]["id"],
        json!(converged_id),
        "the second ensure must resolve to the SAME converged default"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// Metadata-fidelity table-test (locked against perspective.yaml)
// ───────────────────────────────────────────────────────────────────────────

/// Every perspective command id, in registration (sub-domain) order.
const PERSPECTIVE_COMMAND_IDS: [&str; 17] = [
    // lifecycle
    "perspective.load",
    "perspective.save",
    "perspective.delete",
    "perspective.rename",
    "perspective.list",
    // filter
    "perspective.filter.focus",
    "perspective.filter",
    "perspective.clearFilter",
    // group
    "perspective.group",
    "perspective.clearGroup",
    // sort
    "perspective.sort.set",
    "perspective.sort.clear",
    "perspective.sort.toggle",
    // nav
    "perspective.next",
    "perspective.prev",
    "perspective.goto",
    "perspective.switch",
];

/// The expected metadata for one command, mirroring its `perspective.yaml`
/// entry exactly. `None` fields assert "absent / falsey" on the registration.
struct MetaSpec {
    id: &'static str,
    name: &'static str,
    scope: Option<Value>,
    undoable: bool,
    visible_false: bool,
    context_menu: bool,
    view_kinds: Option<Value>,
    tab_button: Option<Value>,
    keys: Option<Value>,
    params: Value,
}

/// Assert one command's registration matches its `MetaSpec` 1:1.
fn assert_command_metadata(cmd: &Value, spec: &MetaSpec) {
    assert_eq!(cmd["name"], json!(spec.name), "{} name", spec.id);

    match &spec.scope {
        Some(scope) => assert_eq!(cmd["scope"], *scope, "{} scope", spec.id),
        None => assert!(
            cmd.get("scope").is_none() || cmd["scope"].is_null() || cmd["scope"] == json!([]),
            "{} carries no scope, got {}",
            spec.id,
            cmd["scope"]
        ),
    }

    if spec.undoable {
        assert_eq!(cmd["undoable"], json!(true), "{} undoable", spec.id);
    } else {
        assert!(
            cmd.get("undoable").is_none() || cmd["undoable"] == json!(false),
            "{} is not undoable",
            spec.id
        );
    }

    if spec.visible_false {
        assert_eq!(cmd["visible"], json!(false), "{} visible:false", spec.id);
    } else {
        assert!(
            cmd.get("visible").is_none() || cmd["visible"] == json!(true),
            "{} is visible (no visible:false)",
            spec.id
        );
    }

    if spec.context_menu {
        assert_eq!(cmd["context_menu"], json!(true), "{} context_menu", spec.id);
    } else {
        assert!(
            cmd.get("context_menu").is_none() || cmd["context_menu"] == json!(false),
            "{} carries no context_menu",
            spec.id
        );
    }

    match &spec.view_kinds {
        Some(vk) => assert_eq!(cmd["view_kinds"], *vk, "{} view_kinds", spec.id),
        None => assert!(
            cmd.get("view_kinds").is_none()
                || cmd["view_kinds"].is_null()
                || cmd["view_kinds"] == json!([]),
            "{} carries no view_kinds, got {}",
            spec.id,
            cmd["view_kinds"]
        ),
    }

    match &spec.tab_button {
        Some(tb) => assert_eq!(cmd["tab_button"], *tb, "{} tab_button", spec.id),
        None => assert!(
            cmd.get("tab_button").is_none() || cmd["tab_button"].is_null(),
            "{} carries no tab_button, got {}",
            spec.id,
            cmd["tab_button"]
        ),
    }

    match &spec.keys {
        Some(keys) => assert_eq!(cmd["keys"], *keys, "{} keys", spec.id),
        None => assert!(
            cmd.get("keys").is_none() || cmd["keys"].is_null() || cmd["keys"] == json!({}),
            "{} carries no keys, got {}",
            spec.id,
            cmd["keys"]
        ),
    }

    // A command with no params carries no `params` key at all (the SDK omits
    // the empty array on the way out), so treat "absent / null / []" as the
    // empty-params shape; otherwise the array must match 1:1.
    if spec.params == json!([]) {
        assert!(
            cmd.get("params").is_none() || cmd["params"].is_null() || cmd["params"] == json!([]),
            "{} carries no params, got {}",
            spec.id,
            cmd["params"]
        );
    } else {
        assert_eq!(
            cmd["params"], spec.params,
            "{} params must match the YAML 1:1",
            spec.id
        );
    }
}

/// Build the seventeen `MetaSpec`s — one per command, mirroring perspective.yaml.
fn metadata_specs() -> Vec<MetaSpec> {
    vec![
        MetaSpec {
            id: "perspective.load",
            name: "Load Perspective",
            scope: None,
            undoable: false,
            visible_false: false,
            context_menu: false,
            view_kinds: None,
            tab_button: None,
            keys: None,
            params: json!([{ "name": "name", "from": "args" }]),
        },
        MetaSpec {
            id: "perspective.save",
            name: "Save Perspective",
            scope: None,
            undoable: true,
            visible_false: false,
            context_menu: false,
            view_kinds: None,
            tab_button: Some(json!({ "icon": "plus" })),
            keys: None,
            params: json!([
                { "name": "name", "from": "args", "shape": "text" },
                { "name": "view_id", "from": "scope_chain", "entity_type": "view" },
            ]),
        },
        MetaSpec {
            id: "perspective.delete",
            name: "Delete Perspective",
            scope: Some(json!(["entity:perspective"])),
            undoable: true,
            visible_false: false,
            context_menu: true,
            view_kinds: None,
            tab_button: None,
            keys: None,
            params: json!([{ "name": "name", "from": "args" }]),
        },
        MetaSpec {
            id: "perspective.rename",
            name: "Rename Perspective",
            scope: None,
            undoable: true,
            visible_false: true,
            context_menu: false,
            view_kinds: None,
            tab_button: None,
            keys: None,
            params: json!([
                { "name": "id", "from": "args" },
                { "name": "new_name", "from": "args" },
            ]),
        },
        MetaSpec {
            id: "perspective.list",
            name: "List Perspectives",
            scope: None,
            undoable: false,
            visible_false: true,
            context_menu: false,
            view_kinds: None,
            tab_button: None,
            keys: None,
            params: json!([]),
        },
        MetaSpec {
            id: "perspective.filter.focus",
            name: "Focus Filter",
            scope: Some(json!(["entity:perspective"])),
            undoable: false,
            visible_false: false,
            context_menu: false,
            view_kinds: None,
            tab_button: Some(json!({ "icon": "filter" })),
            keys: None,
            params: json!([
                { "name": "perspective_id", "from": "scope_chain", "entity_type": "perspective" },
            ]),
        },
        MetaSpec {
            id: "perspective.filter",
            name: "Set Filter",
            scope: Some(json!(["entity:perspective"])),
            undoable: true,
            visible_false: false,
            context_menu: false,
            view_kinds: None,
            tab_button: None,
            keys: None,
            params: json!([
                { "name": "filter", "from": "args" },
                { "name": "perspective_id", "from": "args" },
            ]),
        },
        MetaSpec {
            id: "perspective.clearFilter",
            name: "Clear Filter",
            scope: Some(json!(["entity:perspective"])),
            undoable: true,
            visible_false: false,
            context_menu: true,
            view_kinds: None,
            tab_button: None,
            keys: None,
            params: json!([{ "name": "perspective_id", "from": "args" }]),
        },
        MetaSpec {
            id: "perspective.group",
            name: "Group By",
            scope: Some(json!(["entity:perspective"])),
            undoable: true,
            visible_false: false,
            context_menu: false,
            view_kinds: None,
            tab_button: Some(json!({ "icon": "group" })),
            keys: None,
            params: json!([
                {
                    "name": "group",
                    "from": "args",
                    "shape": "enum",
                    "options_from": "perspective.fields",
                    "clear_command": "perspective.clearGroup",
                },
                { "name": "perspective_id", "from": "scope_chain", "entity_type": "perspective" },
            ]),
        },
        MetaSpec {
            id: "perspective.clearGroup",
            name: "Clear Group",
            scope: Some(json!(["entity:perspective"])),
            undoable: true,
            visible_false: false,
            context_menu: true,
            view_kinds: None,
            tab_button: None,
            keys: None,
            params: json!([{ "name": "perspective_id", "from": "args" }]),
        },
        MetaSpec {
            id: "perspective.sort.set",
            name: "Sort Field",
            scope: Some(json!(["entity:perspective"])),
            undoable: true,
            visible_false: false,
            context_menu: false,
            view_kinds: Some(json!(["grid"])),
            tab_button: Some(json!({ "icon": "arrow-up-down" })),
            keys: None,
            params: json!([
                { "name": "field", "from": "args", "shape": "enum", "options_from": "perspective.fields" },
                { "name": "direction", "from": "args", "shape": "enum", "options_from": "sort.directions" },
                { "name": "perspective_id", "from": "scope_chain", "entity_type": "perspective" },
            ]),
        },
        MetaSpec {
            id: "perspective.sort.clear",
            name: "Clear Sort",
            scope: Some(json!(["entity:perspective"])),
            undoable: true,
            visible_false: false,
            context_menu: true,
            view_kinds: Some(json!(["grid"])),
            tab_button: None,
            keys: None,
            params: json!([{ "name": "perspective_id", "from": "args" }]),
        },
        MetaSpec {
            id: "perspective.sort.toggle",
            name: "Toggle Sort",
            scope: Some(json!(["entity:perspective"])),
            undoable: true,
            visible_false: false,
            context_menu: false,
            view_kinds: Some(json!(["grid"])),
            tab_button: None,
            keys: None,
            params: json!([
                { "name": "field", "from": "args" },
                { "name": "perspective_id", "from": "args" },
            ]),
        },
        MetaSpec {
            id: "perspective.next",
            name: "Next Perspective",
            scope: None,
            undoable: false,
            visible_false: false,
            context_menu: false,
            view_kinds: None,
            tab_button: None,
            // vim `g t` is a chord (Card J): two canonical keystrokes
            // separated by a space, resolved by the webview chord machine.
            keys: Some(json!({ "cua": "Mod+]", "vim": "g t" })),
            params: json!([
                { "name": "view_kind", "from": "args" },
                { "name": "view_id", "from": "args" },
            ]),
        },
        MetaSpec {
            id: "perspective.prev",
            name: "Previous Perspective",
            scope: None,
            undoable: false,
            visible_false: false,
            context_menu: false,
            view_kinds: None,
            tab_button: None,
            // vim `g Shift+T` — a chord whose second step carries a
            // modifier (the canonical form of the old `gT`).
            keys: Some(json!({ "cua": "Mod+[", "vim": "g Shift+T" })),
            params: json!([
                { "name": "view_kind", "from": "args" },
                { "name": "view_id", "from": "args" },
            ]),
        },
        MetaSpec {
            id: "perspective.goto",
            name: "Go to Perspective",
            scope: None,
            undoable: false,
            visible_false: true,
            context_menu: false,
            view_kinds: None,
            tab_button: None,
            keys: None,
            params: json!([
                { "name": "id", "from": "args" },
                { "name": "view_kind", "from": "args" },
                { "name": "view_id", "from": "args" },
            ]),
        },
        MetaSpec {
            id: "perspective.switch",
            name: "Switch Perspective",
            scope: None,
            undoable: false,
            visible_false: true,
            context_menu: false,
            view_kinds: None,
            tab_button: None,
            keys: None,
            params: json!([{ "name": "perspective_id", "from": "args" }]),
        },
    ]
}

// ───────────────────────────────────────────────────────────────────────────
// Activation pins (01KTYQY0ZB62KHN6BPK3FBMBD7 — "perspectives can't be
// SELECTED")
//
// The plugin port routed `perspective.switch` / `perspective.next` /
// `perspective.prev` to the `views` server's RESOLUTION ops, which hold no
// UIState — so dispatching them stopped writing the window's
// `active_perspective_id` + `filtered_task_ids` and clicking a tab no longer
// activated anything. These tests pin the restored contract over the
// PRODUCTION substrate shape: one `KanbanContext` board backing BOTH the
// `views` module (perspective storage) and the `entity` module (the
// board-bundle server that holds the `KanbanContext` + `UIState` pair, same
// wiring as `apps/kanban-app/src/commands.rs`), with a shared `UIState`
// observed directly.
// ───────────────────────────────────────────────────────────────────────────

/// Save a board-kind perspective through the real plugin path; returns its id.
async fn save_board_perspective(fx: &PluginFixture, name: &str) -> String {
    let saved = call_command(
        &fx.service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "perspective.save",
            "ctx": { "args": { "name": name, "view": "board" } },
        }),
    )
    .await;
    assert_eq!(
        saved["structuredContent"]["ok"],
        json!(true),
        "perspective.save should succeed, got {saved}"
    );
    saved["structuredContent"]["result"]["perspective"]["id"]
        .as_str()
        .expect("save must return the minted perspective id")
        .to_string()
}

/// Dispatch one of the activation commands with a `window:main` scope chain.
async fn dispatch_with_window(fx: &PluginFixture, id: &str, args: Value) -> Value {
    call_command(
        &fx.service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": id,
            "ctx": { "scope_chain": ["window:main"], "args": args },
        }),
    )
    .await
}

/// `perspective.switch` must ACTIVATE: write the dispatching window's
/// `active_perspective_id` + `filtered_task_ids` atomically and surface the
/// `PerspectiveSwitch` change envelope the host's `ui-state-changed` emit
/// unwraps (`structuredContent.change`). This is the click-a-tab /
/// Enter-on-a-tab live bug: routing to the views RESOLUTION op left UIState
/// untouched, so the bar never switched and the board never re-filtered.
#[tokio::test]
async fn perspective_switch_activates_the_perspective_for_the_window() {
    let fx = boot_perspective_plugin().await;
    let _p1 = save_board_perspective(&fx, "One").await;
    let p2 = save_board_perspective(&fx, "Two").await;

    let switched =
        dispatch_with_window(&fx, "perspective.switch", json!({ "perspective_id": p2 })).await;
    assert_eq!(
        switched["structuredContent"]["ok"],
        json!(true),
        "perspective.switch should succeed, got {switched}"
    );

    assert_eq!(
        fx.views.ui_state.active_perspective_id("main"),
        p2,
        "perspective.switch must write the window's active_perspective_id"
    );

    // The change envelope must surface where the host's
    // `emit_ui_state_change_if_needed` unwraps it: the plugin returns the
    // entity call's CallToolResult, so the change sits at
    // `structuredContent.result.structuredContent.change`.
    let change = &switched["structuredContent"]["result"]["structuredContent"]["change"];
    assert_eq!(
        change["PerspectiveSwitch"]["perspective_id"],
        json!(p2),
        "perspective.switch must return the atomic PerspectiveSwitch change, got {switched}"
    );
}

/// `perspective.next` / `perspective.prev` must cycle the window's visible
/// perspectives — including wrap-around in both directions — and ACTIVATE
/// the resolved target (UIState write, not just resolution).
#[tokio::test]
async fn perspective_next_prev_cycle_visible_perspectives_with_wraparound() {
    let fx = boot_perspective_plugin().await;
    let p1 = save_board_perspective(&fx, "One").await;
    let p2 = save_board_perspective(&fx, "Two").await;
    let p3 = save_board_perspective(&fx, "Three").await;

    // Establish the starting active perspective.
    dispatch_with_window(&fx, "perspective.switch", json!({ "perspective_id": p1 })).await;
    assert_eq!(fx.views.ui_state.active_perspective_id("main"), p1);

    // next: p1 → p2 → p3, then wraps to p1.
    dispatch_with_window(&fx, "perspective.next", json!({})).await;
    assert_eq!(
        fx.views.ui_state.active_perspective_id("main"),
        p2,
        "next p1→p2"
    );
    dispatch_with_window(&fx, "perspective.next", json!({})).await;
    assert_eq!(
        fx.views.ui_state.active_perspective_id("main"),
        p3,
        "next p2→p3"
    );
    dispatch_with_window(&fx, "perspective.next", json!({})).await;
    assert_eq!(
        fx.views.ui_state.active_perspective_id("main"),
        p1,
        "next wraps p3→p1"
    );

    // prev: wraps backwards p1 → p3.
    dispatch_with_window(&fx, "perspective.prev", json!({})).await;
    assert_eq!(
        fx.views.ui_state.active_perspective_id("main"),
        p3,
        "prev wraps p1→p3"
    );
}

/// With a single visible perspective there is nothing to cycle to —
/// `perspective.next` succeeds as a no-op and the active perspective is
/// unchanged.
#[tokio::test]
async fn perspective_next_is_a_noop_with_a_single_visible_perspective() {
    let fx = boot_perspective_plugin().await;
    let p1 = save_board_perspective(&fx, "Only").await;

    dispatch_with_window(&fx, "perspective.switch", json!({ "perspective_id": p1 })).await;
    assert_eq!(fx.views.ui_state.active_perspective_id("main"), p1);

    let next = dispatch_with_window(&fx, "perspective.next", json!({})).await;
    assert_eq!(
        next["structuredContent"]["ok"],
        json!(true),
        "perspective.next should succeed (as a no-op), got {next}"
    );
    assert_eq!(
        fx.views.ui_state.active_perspective_id("main"),
        p1,
        "a single visible perspective must leave the active id unchanged"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// Delete pins (01KTYVSA68WDFGXCEJ44T4VFNW — "Delete fails on a perspective
// tab")
//
// The tab's context-menu Delete dispatches `perspective.delete` with the
// tab's scope chain — the innermost moniker is `perspective:<id>` (see
// `usePerspectiveScopeChain` in perspective-tab-bar.tsx). The plugin port
// resolves the id off that moniker and routes to the `entity` server's
// `delete perspective` op (NOT views — the entity server holds the per-window
// UIState the active-selection fallback writes). These pins drive the LIVE
// shape end-to-end through the real plugin path.
// ───────────────────────────────────────────────────────────────────────────

/// Dispatch `perspective.delete` exactly as the tab's context menu does: the
/// tab's scope chain — `perspective:<id>` innermost, the dispatching
/// `window:main` outermost (the frontend's command-scope provider appends the
/// window moniker) — and no `name` arg. Returns the raw execute envelope.
async fn delete_via_tab_scope(fx: &PluginFixture, perspective_id: &str) -> Value {
    call_command(
        &fx.service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "perspective.delete",
            "ctx": {
                "scope_chain": [
                    format!("perspective:{perspective_id}"),
                    "window:main",
                ],
                "args": {},
            },
        }),
    )
    .await
}

/// Deleting a non-default perspective from its tab succeeds: the command
/// resolves the id off the `perspective:<id>` scope moniker, the views op
/// removes it from the kernel, and the dispatch envelope reports `ok: true`.
#[tokio::test]
async fn perspective_delete_from_tab_scope_succeeds() {
    let fx = boot_perspective_plugin().await;
    let _survivor = save_board_perspective(&fx, "Survivor").await;
    let doomed = save_board_perspective(&fx, "Doomed").await;

    // Sanity: both perspectives are present before the delete.
    assert!(
        fx.views
            .perspectives
            .read()
            .await
            .get_by_id(&doomed)
            .is_some(),
        "the doomed perspective must exist before the delete"
    );

    let deleted = delete_via_tab_scope(&fx, &doomed).await;
    assert_eq!(
        deleted["structuredContent"]["ok"],
        json!(true),
        "deleting a perspective from the tab scope must succeed, got {deleted}"
    );

    // The perspective is gone from the kernel; the survivor remains.
    let pctx = fx.views.perspectives.read().await;
    assert!(
        pctx.get_by_id(&doomed).is_none(),
        "the deleted perspective must be removed from the kernel"
    );
    assert!(
        pctx.all().iter().any(|p| p.name == "Survivor"),
        "the survivor perspective must remain after the delete"
    );
}

/// Deleting the ACTIVE perspective from its tab must leave the window
/// pointing at a surviving perspective — never at the just-deleted id (the
/// "empty bar" the never-zero invariant forbids). The delete itself routes
/// through `perspective.delete`; selection must fall back to another visible
/// perspective for the dispatching window.
#[tokio::test]
async fn perspective_delete_of_active_falls_back_to_a_survivor() {
    let fx = boot_perspective_plugin().await;
    let survivor = save_board_perspective(&fx, "Survivor").await;
    let active = save_board_perspective(&fx, "Active").await;

    // Make the doomed perspective the window's active selection.
    dispatch_with_window(
        &fx,
        "perspective.switch",
        json!({ "perspective_id": active }),
    )
    .await;
    assert_eq!(
        fx.views.ui_state.active_perspective_id("main"),
        active,
        "the doomed perspective must be active before the delete"
    );

    let deleted = delete_via_tab_scope(&fx, &active).await;
    assert_eq!(
        deleted["structuredContent"]["ok"],
        json!(true),
        "deleting the active perspective must succeed, got {deleted}"
    );

    // The window must NOT still point at the deleted perspective.
    let now_active = fx.views.ui_state.active_perspective_id("main");
    assert_ne!(
        now_active, active,
        "after deleting the active perspective the window must not still select it"
    );
    assert_eq!(
        now_active, survivor,
        "selection must fall back to the surviving perspective, got {now_active}"
    );
}

/// Deleting the ACTIVE perspective must FORWARD the reselection's
/// `PerspectiveSwitch` change on the result envelope — the same
/// `structuredContent.change` contract `perspective.switch` rides — so the
/// host's `ui-state-changed` emit fires for the new selection. Without this
/// the backend reselect is invisible to the UI: it writes the new active id
/// server-side but emits no event, leaving the app to recover only via the
/// frontend list-reconciliation. The change must carry the SURVIVOR id.
#[tokio::test]
async fn perspective_delete_of_active_forwards_the_reselect_change() {
    let fx = boot_perspective_plugin().await;
    let survivor = save_board_perspective(&fx, "Survivor").await;
    let active = save_board_perspective(&fx, "Active").await;

    dispatch_with_window(
        &fx,
        "perspective.switch",
        json!({ "perspective_id": active }),
    )
    .await;
    assert_eq!(
        fx.views.ui_state.active_perspective_id("main"),
        active,
        "the doomed perspective must be active before the delete"
    );

    let deleted = delete_via_tab_scope(&fx, &active).await;
    assert_eq!(
        deleted["structuredContent"]["ok"],
        json!(true),
        "deleting the active perspective must succeed, got {deleted}"
    );

    // The reselect change must surface where the host's
    // `emit_ui_state_change_if_needed` unwraps it — the plugin returns the
    // entity call's CallToolResult raw, so the change sits at
    // `structuredContent.result.structuredContent.change`, exactly like switch.
    let change = &deleted["structuredContent"]["result"]["structuredContent"]["change"];
    assert_eq!(
        change["PerspectiveSwitch"]["perspective_id"],
        json!(survivor),
        "delete of the active perspective must forward the reselect's \
         PerspectiveSwitch change to the survivor, got {deleted}"
    );
}

/// Deleting the ONLY perspective for the active view clears the window's
/// active id (the no-survivor branch). There is nothing to fall back to, so
/// the selection fallback writes an empty active id rather than leaving a
/// dangling pointer; never-zero recovery (recreating a Default) is an external
/// save/open reconciliation concern, so this pins only the clear.
#[tokio::test]
async fn perspective_delete_of_the_only_perspective_clears_the_active_id() {
    let fx = boot_perspective_plugin().await;
    let only = save_board_perspective(&fx, "Only").await;

    dispatch_with_window(&fx, "perspective.switch", json!({ "perspective_id": only })).await;
    assert_eq!(
        fx.views.ui_state.active_perspective_id("main"),
        only,
        "the only perspective must be active before the delete"
    );

    let deleted = delete_via_tab_scope(&fx, &only).await;
    assert_eq!(
        deleted["structuredContent"]["ok"],
        json!(true),
        "deleting the only perspective must succeed, got {deleted}"
    );

    assert_eq!(
        fx.views.ui_state.active_perspective_id("main"),
        "",
        "with no survivor the window's active id must be cleared, not left \
         dangling at the deleted id"
    );
}
