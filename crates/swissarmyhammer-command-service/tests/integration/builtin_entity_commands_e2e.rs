//! End-to-end test for the committed `entity-commands` builtin plugin.
//!
//! This is the acceptance for the port of `entity.yaml` — the eight
//! cross-cutting entity CRUD + clipboard commands — into the one
//! `builtin/plugins/entity-commands/` bundle. It mirrors
//! `builtin_kanban_misc_e2e` exactly, but every command routes to a SINGLE
//! backend: the generic, type-agnostic `entity` operation tool
//! (`swissarmyhammer-entity-mcp::EntityServer`), exposed to the host under id
//! `"entity"` and reached by the plugin via `this.entity...`.
//!
//! The exposed server is built with FULL clipboard wiring
//! (`EntityServer::with_clipboard`) over a real `KanbanContext` board
//! substrate plus an `InMemoryClipboard` and `UIState` — the exact harness the
//! entity-mcp crate's `entity_clipboard_e2e` uses. The clipboard wiring is
//! load-bearing: `entity.cut` / `entity.copy` / `entity.paste` are inert on a
//! bare `EntityServer::new`. CRUD / archive verbs work on either, so one
//! clipboard-wired server backs all eight commands.
//!
//! What a passing run proves:
//!
//! 1. **Discovery + registration** — after load, the eight ported commands are
//!    registered.
//! 2. **Metadata fidelity** — each command's `name` / `undoable` / `visible` /
//!    `context_menu` / `context_menu_group` / `context_menu_order` / `keys` /
//!    `menu` / `params` match the source-YAML baseline 1:1 (table-test).
//! 3. **Real effect** — CRUD round-trip (`entity.add` → `entity.update_field`
//!    → `entity.delete`, plus `entity.archive` → `entity.unarchive`) mutates
//!    the shared entity kernel, and a clipboard `entity.copy` → `entity.paste`
//!    creates a duplicate task on disk.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_directory::KanbanConfig;
use swissarmyhammer_entity::{Entity, EntityContext};
use swissarmyhammer_entity_mcp::EntityServer;
use swissarmyhammer_kanban::board::InitBoard;
use swissarmyhammer_kanban::clipboard::{ClipboardProvider, InMemoryClipboard};
use swissarmyhammer_kanban::commands::clipboard_commands::COPYABLE_ENTITY_TYPES;
use swissarmyhammer_kanban::{KanbanContext, KanbanOperationProcessor, OperationProcessor};
use swissarmyhammer_plugin::{
    CallerId, InProcessServer, McpServer as PluginMcpServer, PluginHost, PLUGINS_SUBDIR,
};
use swissarmyhammer_store::StoreContext;
use swissarmyhammer_ui_state::UIState;
use tempfile::TempDir;

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

/// Stage the committed `builtin/plugins/entity-commands` bundle into a temp
/// builtin-layer root so `discover_and_load_all` finds it at
/// `<layer_root>/plugins/entity-commands/`.
fn stage_entity_commands(layer_root: &Path) {
    let source = workspace_root()
        .join("builtin/plugins")
        .join("entity-commands");
    assert!(
        source.is_dir(),
        "the committed entity-commands bundle must exist at {}",
        source.display()
    );
    let destination = layer_root.join(PLUGINS_SUBDIR).join("entity-commands");
    copy_dir_recursive(&source, &destination);
}

// ───────────────────────────────────────────────────────────────────────────
// Exposing the real in-process `entity` tool over a clipboard-wired server
// ───────────────────────────────────────────────────────────────────────────

/// A handle to the live entity substrate, kept alive for the test's duration
/// so the board root and shared kernel outlive the plugin's `load()` and every
/// `execute`.
struct ExposedEntity {
    _dir: TempDir,
    _store_ctx: Arc<StoreContext>,
    kanban: Arc<KanbanContext>,
    entity_ctx: Arc<EntityContext>,
}

impl ExposedEntity {
    /// List every live entity of a type through the shared kernel.
    async fn list(&self, entity_type: &str) -> Vec<Entity> {
        self.entity_ctx
            .list(entity_type)
            .await
            .expect("list entities")
    }

    /// Add a task directly through the kernel-backed kanban processor and
    /// return its id — used to seed the clipboard copy→paste round-trip.
    async fn add_task(&self, title: &str) -> String {
        use swissarmyhammer_kanban::task::AddTask;
        let result = KanbanOperationProcessor::new()
            .process(&AddTask::new(title), self.kanban.as_ref())
            .await
            .expect("add task");
        result["id"].as_str().expect("task id").to_string()
    }
}

/// Build a clipboard-wired `entity` server over a real `KanbanContext` board
/// substrate (mirroring the entity-mcp crate's `ClipboardHarness`), wrap it in
/// an `InProcessServer`, and expose it to `host` under id `"entity"`.
async fn expose_entity_module(host: &PluginHost) -> ExposedEntity {
    let dir = TempDir::new().expect("entity substrate temp dir");
    let kanban = KanbanContext::new(dir.path().join(".kanban"));

    KanbanOperationProcessor::new()
        .process(&InitBoard::new("Entity Commands Board"), &kanban)
        .await
        .expect("board init");

    let kanban = Arc::new(kanban);

    // Wire the one shared StoreContext into the kernel exactly as production
    // does, so writes are undoable on this stack.
    let store_ctx = swissarmyhammer_kanban::wire_store_substrate(&kanban).await;
    let entity_ctx = kanban.entity_context().await.expect("entity_context");

    let clipboard = Arc::new(InMemoryClipboard::new());
    let ui_state = Arc::new(UIState::new());

    // Clipboard-wired so copy/cut/paste are live; CRUD/archive work too.
    let server = EntityServer::with_clipboard(
        Arc::clone(&kanban),
        Arc::clone(&clipboard) as Arc<dyn ClipboardProvider>,
        Arc::clone(&ui_state),
    )
    .await
    .expect("clipboard-wired entity server");

    let module = InProcessServer::new(server)
        .await
        .expect("wrapping the entity server in an InProcessServer should succeed");
    host.expose_rust_module(
        "entity".to_string(),
        Arc::new(module) as Arc<dyn PluginMcpServer>,
    )
    .await
    .expect("exposing the entity module should succeed");

    ExposedEntity {
        _dir: dir,
        _store_ctx: store_ctx,
        kanban,
        entity_ctx,
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

// ───────────────────────────────────────────────────────────────────────────
// The test
// ───────────────────────────────────────────────────────────────────────────

/// The committed `entity-commands` builtin plugin, discovered from a builtin
/// layer, registers all eight `entity.yaml` commands with 1:1 metadata and
/// produces each command's real effect against the clipboard-wired `entity`
/// backend (CRUD + archive round-trip + clipboard copy→paste duplicate).
#[tokio::test]
async fn entity_commands_plugin_registers_and_executes() {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");

    // Stage the committed bundle into the builtin layer's plugins/ dir.
    stage_entity_commands(builtin_root.path());

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

    // Expose the entity backend BEFORE discovery, so the plugin's
    // `ensureServices(this, ["commands", "entity"])` finds it already exposed.
    let entity = tokio::time::timeout(TIMEOUT, expose_entity_module(&host))
        .await
        .expect("exposing entity should not hang");

    // Discover + load the builtin layer: runs the bundle's `load()`, which
    // registers the eight commands through the SDK convention.
    let loaded = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the entity-commands builtin plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one entity-commands builtin plugin should be discovered, got {loaded:?}"
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
        "entity.add",
        "entity.update_field",
        "entity.delete",
        "entity.archive",
        "entity.unarchive",
        "entity.cut",
        "entity.copy",
        "entity.paste",
    ] {
        assert!(
            commands.contains_key(id),
            "list command must include the ported command {id:?}; got {:?}",
            commands.keys().collect::<Vec<_>>()
        );
    }

    // ── (2) Metadata fidelity: lock each command's metadata 1:1 vs the YAML ─
    for (id, assert_fn) in metadata_asserts() {
        assert_fn(&commands[id]);
    }

    // ── (3a) CRUD round-trip: add → update_field → delete (on a `tag`) ──────
    let add = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "entity.add",
            "ctx": {
                "args": {
                    "entity_type": "tag",
                    "fields": { "tag_name": "Blue", "color": "#0000ff" },
                },
            },
        }),
    )
    .await;
    assert_eq!(
        add["structuredContent"]["ok"],
        json!(true),
        "executing entity.add should succeed, got {add}"
    );
    // The execute envelope is `{ ok, result: <plugin return> }`; the plugin's
    // single `entity add entity` call returns the backend `CallToolResult`, so
    // the minted id lives under `result.structuredContent.id`.
    let tag_id = add["structuredContent"]["result"]["structuredContent"]["id"]
        .as_str()
        .expect("entity.add must return the new tag id")
        .to_string();
    assert_eq!(
        entity.list("tag").await.len(),
        1,
        "entity.add must have created the tag through the kernel"
    );

    let update = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "entity.update_field",
            "ctx": {
                "args": {
                    "entity_type": "tag",
                    "id": tag_id,
                    "field_name": "tag_name",
                    "value": "Indigo",
                },
            },
        }),
    )
    .await;
    assert_eq!(
        update["structuredContent"]["ok"],
        json!(true),
        "executing entity.update_field should succeed, got {update}"
    );
    let renamed = entity
        .entity_ctx
        .read("tag", &tag_id)
        .await
        .expect("read renamed tag");
    assert_eq!(
        renamed.get("tag_name"),
        Some(&json!("Indigo")),
        "entity.update_field must have written the new field value through the kernel"
    );

    let delete = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "entity.delete",
            "ctx": { "target": format!("tag:{tag_id}") },
        }),
    )
    .await;
    assert_eq!(
        delete["structuredContent"]["ok"],
        json!(true),
        "executing entity.delete should succeed, got {delete}"
    );
    assert_eq!(
        entity.list("tag").await.len(),
        0,
        "entity.delete must have trashed the tag through the kernel"
    );

    // ── (3b) Archive round-trip: archive → unarchive (on a fresh `tag`) ─────
    let added_archivable = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "entity.add",
            "ctx": {
                "args": {
                    "entity_type": "tag",
                    "fields": { "tag_name": "Green", "color": "#00ff00" },
                },
            },
        }),
    )
    .await;
    let archivable_id = added_archivable["structuredContent"]["result"]["structuredContent"]["id"]
        .as_str()
        .expect("the archivable tag id")
        .to_string();

    let archive = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "entity.archive",
            "ctx": { "target": format!("tag:{archivable_id}") },
        }),
    )
    .await;
    assert_eq!(
        archive["structuredContent"]["ok"],
        json!(true),
        "executing entity.archive should succeed, got {archive}"
    );
    assert!(
        entity
            .list("tag")
            .await
            .iter()
            .all(|t| t.id.as_str() != archivable_id),
        "entity.archive must have removed the tag from the live list"
    );

    let unarchive = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "entity.unarchive",
            "ctx": { "target": format!("tag:{archivable_id}") },
        }),
    )
    .await;
    assert_eq!(
        unarchive["structuredContent"]["ok"],
        json!(true),
        "executing entity.unarchive should succeed, got {unarchive}"
    );
    assert!(
        entity
            .list("tag")
            .await
            .iter()
            .any(|t| t.id.as_str() == archivable_id),
        "entity.unarchive must have restored the tag to the live list"
    );

    // ── (3c) Clipboard: copy → paste creates a duplicate task on disk ───────
    let source_id = entity.add_task("Source task").await;
    let tasks_before = entity.list("task").await;
    assert_eq!(tasks_before.len(), 1, "the seeded source task is on disk");

    let copy = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "entity.copy",
            "ctx": { "target": format!("task:{source_id}") },
        }),
    )
    .await;
    assert_eq!(
        copy["structuredContent"]["ok"],
        json!(true),
        "executing entity.copy should succeed, got {copy}"
    );

    let paste = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "entity.paste",
            "ctx": {
                "target": "column:doing",
                "scope_chain": ["column:doing"],
            },
        }),
    )
    .await;
    assert_eq!(
        paste["structuredContent"]["ok"],
        json!(true),
        "executing entity.paste should succeed, got {paste}"
    );

    let tasks_after = entity.list("task").await;
    assert_eq!(
        tasks_after.len(),
        2,
        "entity.copy + entity.paste must duplicate the task on disk, got {tasks_after:?}"
    );
    assert!(
        tasks_after.iter().any(|t| t.id.as_str() == source_id),
        "the source task must survive a copy"
    );
    assert!(
        tasks_after.iter().any(|t| t.id.as_str() != source_id),
        "the paste must mint a fresh duplicate id"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// Per-command metadata regression asserts (locked against entity.yaml)
// ───────────────────────────────────────────────────────────────────────────

/// One row of the metadata-fidelity table: a command id and its assertion.
type MetadataAssert = (&'static str, fn(&Value));

/// The metadata-fidelity table: each ported command id paired with its
/// per-command assertion, exercised across all eight in the test body.
fn metadata_asserts() -> Vec<MetadataAssert> {
    vec![
        ("entity.add", assert_entity_add_metadata),
        ("entity.update_field", assert_entity_update_field_metadata),
        ("entity.delete", assert_entity_delete_metadata),
        ("entity.archive", assert_entity_archive_metadata),
        ("entity.unarchive", assert_entity_unarchive_metadata),
        ("entity.cut", assert_entity_cut_metadata),
        ("entity.copy", assert_entity_copy_metadata),
        ("entity.paste", assert_entity_paste_metadata),
    ]
}

/// Assert a command carries no context_menu (absent or explicitly false).
fn assert_no_context_menu(cmd: &Value, id: &str) {
    assert!(
        cmd.get("context_menu").is_none() || cmd["context_menu"] == json!(false),
        "{id} carries no context_menu, got {}",
        cmd["context_menu"]
    );
}

/// Assert a command carries no scope (absent / null / empty list).
fn assert_no_scope(cmd: &Value, id: &str) {
    assert!(
        cmd.get("scope").is_none() || cmd["scope"].is_null() || cmd["scope"] == json!([]),
        "{id} carries no scope, got {}",
        cmd["scope"]
    );
}

/// `entity.add` — entity.yaml: undoable, visible:false, no scope/keys/
/// context_menu; param entity_type(args).
fn assert_entity_add_metadata(cmd: &Value) {
    assert_eq!(cmd["name"], json!("New Entity"), "entity.add name");
    assert_eq!(cmd["undoable"], json!(true), "entity.add undoable");
    assert_eq!(cmd["visible"], json!(false), "entity.add visible:false");
    assert_no_scope(cmd, "entity.add");
    assert_no_context_menu(cmd, "entity.add");
    assert_eq!(
        cmd["params"],
        json!([{ "name": "entity_type", "from": "args" }]),
        "entity.add params must match entity.yaml 1:1"
    );
}

/// `entity.update_field` — entity.yaml: undoable, visible:false; params
/// entity_type / id / field_name / value (all args).
fn assert_entity_update_field_metadata(cmd: &Value) {
    assert_eq!(
        cmd["name"],
        json!("Update Field"),
        "entity.update_field name"
    );
    assert_eq!(cmd["undoable"], json!(true), "entity.update_field undoable");
    assert_eq!(
        cmd["visible"],
        json!(false),
        "entity.update_field visible:false"
    );
    assert_no_scope(cmd, "entity.update_field");
    assert_no_context_menu(cmd, "entity.update_field");
    assert_eq!(
        cmd["params"],
        json!([
            { "name": "entity_type", "from": "args" },
            { "name": "id", "from": "args" },
            { "name": "field_name", "from": "args" },
            { "name": "value", "from": "args" },
        ]),
        "entity.update_field params must match entity.yaml 1:1"
    );
}

/// `entity.delete` — entity.yaml: undoable, context_menu (group 2, order 0),
/// keys cua:Mod+Backspace; param moniker(target).
///
/// The plugin registers the caption template `"Delete {{entity.type}}"`;
/// `list command` renders it at display time (generic fallback here — no
/// `ctx` is supplied), so the listed name is the clean "Delete".
fn assert_entity_delete_metadata(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Delete"), "entity.delete name");
    assert_eq!(cmd["undoable"], json!(true), "entity.delete undoable");
    assert_eq!(
        cmd["context_menu"],
        json!(true),
        "entity.delete context_menu"
    );
    assert_eq!(
        cmd["context_menu_group"],
        json!(2),
        "entity.delete context_menu_group"
    );
    assert_eq!(
        cmd["context_menu_order"],
        json!(0),
        "entity.delete context_menu_order"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+Backspace" }),
        "entity.delete keys"
    );
    assert_eq!(
        cmd["params"],
        json!([{ "name": "moniker", "from": "target" }]),
        "entity.delete params must match entity.yaml 1:1"
    );
}

/// `entity.archive` — entity.yaml: undoable, context_menu (group 2, order 1),
/// keys vim:dd; param moniker(target).
fn assert_entity_archive_metadata(cmd: &Value) {
    // Registered as "Archive {{entity.type}}" — rendered to the generic
    // fallback by `list command` (no ctx supplied here).
    assert_eq!(cmd["name"], json!("Archive"), "entity.archive name");
    assert_eq!(cmd["undoable"], json!(true), "entity.archive undoable");
    assert_eq!(
        cmd["context_menu"],
        json!(true),
        "entity.archive context_menu"
    );
    assert_eq!(
        cmd["context_menu_group"],
        json!(2),
        "entity.archive context_menu_group"
    );
    assert_eq!(
        cmd["context_menu_order"],
        json!(1),
        "entity.archive context_menu_order"
    );
    // vim `d d` is a chord (Card J): two canonical keystrokes separated by
    // a space, resolved by the webview chord machine.
    assert_eq!(cmd["keys"], json!({ "vim": "d d" }), "entity.archive keys");
    assert_eq!(
        cmd["params"],
        json!([{ "name": "moniker", "from": "target" }]),
        "entity.archive params must match entity.yaml 1:1"
    );
}

/// `entity.unarchive` — entity.yaml: undoable, context_menu (group 2, order 2),
/// no keys; param moniker(target).
fn assert_entity_unarchive_metadata(cmd: &Value) {
    // Registered as "Unarchive {{entity.type}}" — rendered to the generic
    // fallback by `list command` (no ctx supplied here).
    assert_eq!(cmd["name"], json!("Unarchive"), "entity.unarchive name");
    assert_eq!(cmd["undoable"], json!(true), "entity.unarchive undoable");
    assert_eq!(
        cmd["context_menu"],
        json!(true),
        "entity.unarchive context_menu"
    );
    assert_eq!(
        cmd["context_menu_group"],
        json!(2),
        "entity.unarchive context_menu_group"
    );
    assert_eq!(
        cmd["context_menu_order"],
        json!(2),
        "entity.unarchive context_menu_order"
    );
    assert!(
        cmd.get("keys").is_none() || cmd["keys"] == json!({}),
        "entity.unarchive carries no keys, got {}",
        cmd["keys"]
    );
    assert_eq!(
        cmd["params"],
        json!([{ "name": "moniker", "from": "target" }]),
        "entity.unarchive params must match entity.yaml 1:1"
    );
}

/// `entity.cut` — entity.yaml: undoable, context_menu (group 1, order 0), keys
/// cua:Mod+X / vim:x, menu {path:[Edit], group:1, order:0}; param
/// moniker(target).
fn assert_entity_cut_metadata(cmd: &Value) {
    // Registered as "Cut {{entity.type}}" — rendered to the generic
    // fallback by `list command` (no ctx supplied here).
    assert_eq!(cmd["name"], json!("Cut"), "entity.cut name");
    assert_eq!(cmd["undoable"], json!(true), "entity.cut undoable");
    assert_eq!(cmd["context_menu"], json!(true), "entity.cut context_menu");
    assert_eq!(
        cmd["context_menu_group"],
        json!(1),
        "entity.cut context_menu_group"
    );
    assert_eq!(
        cmd["context_menu_order"],
        json!(0),
        "entity.cut context_menu_order"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+X", "vim": "x" }),
        "entity.cut keys"
    );
    assert_eq!(
        cmd["menu"],
        json!({ "path": ["Edit"], "group": 1, "order": 0 }),
        "entity.cut menu"
    );
    assert_eq!(
        cmd["params"],
        json!([{ "name": "moniker", "from": "target" }]),
        "entity.cut params must match entity.yaml 1:1"
    );
    assert_clipboard_applies_to(cmd, "entity.cut");
}

/// `entity.copy` — entity.yaml: undoable:false, context_menu (group 1, order 1),
/// keys cua:Mod+C / vim:y, menu {path:[Edit], group:1, order:1}; param
/// moniker(target).
fn assert_entity_copy_metadata(cmd: &Value) {
    // Registered as "Copy {{entity.type}}" — rendered to the generic
    // fallback by `list command` (no ctx supplied here).
    assert_eq!(cmd["name"], json!("Copy"), "entity.copy name");
    assert_eq!(cmd["undoable"], json!(false), "entity.copy undoable:false");
    assert_eq!(cmd["context_menu"], json!(true), "entity.copy context_menu");
    assert_eq!(
        cmd["context_menu_group"],
        json!(1),
        "entity.copy context_menu_group"
    );
    assert_eq!(
        cmd["context_menu_order"],
        json!(1),
        "entity.copy context_menu_order"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+C", "vim": "y" }),
        "entity.copy keys"
    );
    assert_eq!(
        cmd["menu"],
        json!({ "path": ["Edit"], "group": 1, "order": 1 }),
        "entity.copy menu"
    );
    assert_eq!(
        cmd["params"],
        json!([{ "name": "moniker", "from": "target" }]),
        "entity.copy params must match entity.yaml 1:1"
    );
    assert_clipboard_applies_to(cmd, "entity.copy");
}

/// `entity.paste` — entity.yaml: undoable, context_menu (group 1, order 2),
/// keys cua:Mod+V / vim:p, menu {path:[Edit], group:1, order:2}; param
/// moniker(target).
fn assert_entity_paste_metadata(cmd: &Value) {
    // Registered as "Paste {{entity.type}}" — rendered to the generic
    // fallback by `list command` (no ctx supplied here).
    assert_eq!(cmd["name"], json!("Paste"), "entity.paste name");
    assert_eq!(cmd["undoable"], json!(true), "entity.paste undoable");
    assert_eq!(
        cmd["context_menu"],
        json!(true),
        "entity.paste context_menu"
    );
    assert_eq!(
        cmd["context_menu_group"],
        json!(1),
        "entity.paste context_menu_group"
    );
    assert_eq!(
        cmd["context_menu_order"],
        json!(2),
        "entity.paste context_menu_order"
    );
    assert_eq!(
        cmd["keys"],
        json!({ "cua": "Mod+V", "vim": "p" }),
        "entity.paste keys"
    );
    assert_eq!(
        cmd["menu"],
        json!({ "path": ["Edit"], "group": 1, "order": 2 }),
        "entity.paste menu"
    );
    assert_eq!(
        cmd["params"],
        json!([{ "name": "moniker", "from": "target" }]),
        "entity.paste params must match entity.yaml 1:1"
    );
    assert_clipboard_applies_to(cmd, "entity.paste");
}

/// Drift guard: the clipboard trio (`entity.cut` / `entity.copy` /
/// `entity.paste`) must declare a list-time `applies_to` set that is EQUAL to
/// the dispatch-time capability set — the canonical `COPYABLE_ENTITY_TYPES`
/// in `swissarmyhammer-kanban::commands::clipboard_commands`.
///
/// The capability lives in two places that cannot import each other:
///
///   - TS `CLIPBOARD_ENTITY_TYPES` (`builtin/plugins/entity-commands/index.ts`)
///     — surfaced here through the real registered metadata as each command's
///     `applies_to`. This is the LIST-time gate: `list command` reads
///     `applies_to` and suppresses the command when the focused object's type
///     is absent from the set.
///   - Rust `COPYABLE_ENTITY_TYPES` — the DISPATCH-time `available()` gate.
///
/// This assertion pins the TS-surfaced `applies_to` against the Rust constant
/// directly (NOT a third hand-maintained literal), so declared (list) and
/// enforced (dispatch) can never silently diverge: if Rust drops `actor` from
/// `COPYABLE_ENTITY_TYPES` while TS keeps listing it, this goes RED rather
/// than re-introducing a shown-but-unsupported command. View / perspective
/// are absent from both — they have no clipboard semantics.
fn assert_clipboard_applies_to(cmd: &Value, id: &str) {
    // Order-insensitive set comparison: the TS source and the Rust constant
    // are two independently authored lists; only their MEMBERSHIP must match.
    let mut declared: Vec<String> = cmd["applies_to"]
        .as_array()
        .unwrap_or_else(|| panic!("{id} applies_to must be an array"))
        .iter()
        .map(|t| {
            t.as_str()
                .unwrap_or_else(|| panic!("{id} applies_to entries must be strings"))
                .to_string()
        })
        .collect();
    declared.sort();

    let mut enforced: Vec<String> = COPYABLE_ENTITY_TYPES
        .iter()
        .map(|t| t.to_string())
        .collect();
    enforced.sort();

    assert_eq!(
        declared, enforced,
        "{id} list-time applies_to (TS CLIPBOARD_ENTITY_TYPES, surfaced via \
         list command) must equal the dispatch-time COPYABLE_ENTITY_TYPES \
         (swissarmyhammer-kanban) — declared and enforced clipboard capability \
         must not drift"
    );

    // Anchor: neither list may name a clipboard-less entity type. This holds
    // independently of the set-equality above, so a future edit that adds
    // view/perspective to BOTH lists still trips here.
    for unsupported in ["view", "perspective"] {
        assert!(
            !declared.iter().any(|t| t == unsupported),
            "{id} applies_to must NOT include {unsupported:?} — it has no clipboard semantics"
        );
    }
}
