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
use swissarmyhammer_perspectives::{PerspectiveContext, PerspectiveStore};
use swissarmyhammer_plugin::{
    CallerId, InProcessServer, McpServer as PluginMcpServer, PluginHost, PLUGINS_SUBDIR,
};
use swissarmyhammer_store::{StoreContext, StoreHandle};
use swissarmyhammer_views::{ViewStore, ViewsContext, ViewsServer};
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
// Exposing the real in-process `views` tool over a real kernel substrate
// ───────────────────────────────────────────────────────────────────────────

/// A handle to the live views substrate, kept alive for the test's duration so
/// the storage root and shared contexts outlive the plugin's `load()` and every
/// `execute`.
struct ExposedViews {
    _dir: TempDir,
    _store_ctx: Arc<StoreContext>,
    perspectives: Arc<RwLock<PerspectiveContext>>,
}

/// Build a `views` substrate (mirroring `builtin_kanban_misc_e2e`), wrap a
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
        perspectives,
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
    let persp_id = op_payload(&saved)["perspective"]["id"]
        .as_str()
        .expect("save must return the minted perspective id")
        .to_string();
    // The view_id scope-chain param threaded through to the saved perspective.
    assert_eq!(
        op_payload(&saved)["perspective"]["view_id"],
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
        views.perspectives.read().await.get_by_id(&persp_id).unwrap().filter.as_deref(),
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
        views.perspectives.read().await.get_by_id(&persp_id).unwrap().group.as_deref(),
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
            "ctx": { "args": { "perspective_id": persp_id } },
        }),
    )
    .await;
    assert_eq!(
        switched["structuredContent"]["ok"],
        json!(true),
        "perspective.switch should succeed, got {switched}"
    );
    // switch surfaces the perspective's filter for the caller to evaluate.
    assert_eq!(
        op_payload(&switched)["perspective"]["id"],
        json!(persp_id),
        "perspective.switch must resolve the perspective by id, got {switched}"
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
        assert_eq!(cmd["params"], spec.params, "{} params must match the YAML 1:1", spec.id);
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
            keys: Some(json!({ "cua": "Mod+]", "vim": "gt" })),
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
            keys: Some(json!({ "cua": "Mod+[", "vim": "gT" })),
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
