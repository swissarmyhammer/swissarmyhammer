//! End-to-end test for the committed `task-commands` builtin plugin.
//!
//! This is the headline acceptance for the FIRST builtin command-plugin port
//! (`task.yaml` → `builtin/plugins/task-commands/`). It stands up the real
//! plugin platform exactly as the kanban desktop app does
//! (`apps/kanban-app/src/plugins.rs`):
//!
//! - the command service is bootstrapped into a real [`PluginHost`] via
//!   [`install_commands_module`], exposing the `commands` module;
//! - the in-process `kanban` operation tool is exposed over a temp board root
//!   via `swissarmyhammer-tools`' `register_kanban_tools` / `build_tool_modules`
//!   — the same triple the production app uses — so the plugin's `execute`
//!   callbacks reach a real board;
//! - the committed `task-commands` bundle is staged into a temp builtin-layer
//!   root and discovered through `discover_and_load_all`, so it loads as a
//!   first-class builtin-layer plugin (isolate created, `load()` run, the three
//!   commands registered through the SDK's `registerCommands` convention).
//!
//! What a passing run proves:
//!
//! 1. **Discovery + registration** — after load, `list command { scope:
//!    "entity:task" }` returns exactly the three ported commands.
//! 2. **Metadata fidelity** — each command's `keys` / `scope` / `params` /
//!    `undoable` / `context_menu` match the `task.yaml` baseline 1:1 (a dropped
//!    field fails the per-command regression asserts).
//! 3. **Real effect** — executing `task.move` with a task in scope and a column
//!    target moves the task on the underlying kanban store, observable by
//!    reading the board back through the exposed `kanban` tool.
//! 4. **Preconditions** — `available command` reflects the YAML preconditions:
//!    `task.move` is unavailable with no task / no column target.
//!
//! [`install_commands_module`]: swissarmyhammer_command_service::bootstrap::install_commands_module
//! [`PluginHost`]: swissarmyhammer_plugin::PluginHost

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_config::ModelConfig;
use swissarmyhammer_directory::KanbanConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_plugin::{
    CallerId, McpServer as PluginMcpServer, PluginHost, PLUGINS_SUBDIR,
};
use swissarmyhammer_tools::mcp::plugin_bridge::build_tool_modules;
use swissarmyhammer_tools::mcp::ToolHandlers;
use swissarmyhammer_tools::{register_kanban_tools, ToolContext, ToolRegistry};
use tempfile::TempDir;
use tokio::sync::{Mutex as TokioMutex, RwLock};

use crate::support::call_command;

/// A generous upper bound on any single host or isolate interaction.
const TIMEOUT: Duration = Duration::from_secs(60);

// The in-process `kanban` operation tool is exposed under the module id
// `"kanban"` (the kanban tool's own name) — the public service name the
// `task-commands` plugin's `ensureServices(this, ["commands", "kanban"])`
// activates and reaches as `this.kanban`.

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

/// Stage the committed `builtin/plugins/task-commands` bundle into a temp
/// builtin-layer root so `discover_and_load_all` finds it at
/// `<layer_root>/plugins/task-commands/`.
fn stage_task_commands(layer_root: &Path) {
    let source = workspace_root()
        .join("builtin/plugins")
        .join("task-commands");
    assert!(
        source.is_dir(),
        "the committed task-commands bundle must exist at {}",
        source.display()
    );
    let destination = layer_root.join(PLUGINS_SUBDIR).join("task-commands");
    copy_dir_recursive(&source, &destination);
}

// ───────────────────────────────────────────────────────────────────────────
// Exposing the real in-process kanban tool (mirrors the kanban app's wiring)
// ───────────────────────────────────────────────────────────────────────────

/// A handle to the in-process `kanban` operation tool exposed for the test.
///
/// Owns the live [`ToolRegistry`] and [`ToolContext`] so they outlive the
/// plugin's `load()` and every `execute` call. The same module is exposed to
/// the host (the plugin reaches it as `this.kanban`) and driven directly by the
/// test (to seed the board and read it back) — both see the same `.kanban`
/// board.
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
// Result-shape helpers (the kanban tool returns CallToolResult JSON)
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

/// Read the column id of the task with `task_id` off a `list tasks` result, or
/// `None` if the task is absent. The rich task JSON carries the column nested
/// under `position.column` (see `task_entity_to_json`).
fn task_column(list_result: &Value, task_id: &str) -> Option<String> {
    kanban_payload(list_result)
        .get("tasks")
        .and_then(Value::as_array)
        .expect("list tasks must carry a `tasks` array")
        .iter()
        .find(|task| task.get("id").and_then(Value::as_str) == Some(task_id))
        .and_then(|task| task.get("position").and_then(|p| p.get("column")))
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

/// The committed `task-commands` builtin plugin, discovered from a builtin
/// layer, registers all three `task.yaml` commands with 1:1 metadata and moves
/// a real task on the kanban store through `task.move`.
#[tokio::test]
async fn task_commands_plugin_registers_and_moves_a_task() {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");
    let board_dir = TempDir::new().expect("kanban board temp dir");

    // Stage the committed bundle into the builtin layer's plugins/ dir.
    stage_task_commands(builtin_root.path());

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

    // Expose the real `kanban` tool BEFORE discovery, so the plugin's
    // `ensureServices(this, ["commands", "kanban"])` finds it already exposed.
    let kanban = expose_kanban_module(board_dir.path()).await;
    tokio::time::timeout(TIMEOUT, kanban.expose_to(&host))
        .await
        .expect("exposing kanban should not hang");

    // Seed a board with one task in `todo` before the plugin loads.
    kanban
        .call(json!({ "op": "init board", "name": "Task Commands Board" }))
        .await;
    let added = kanban
        .call(json!({ "op": "add task", "title": "Ship the port" }))
        .await;
    let task_id = kanban_payload(&added)
        .get("id")
        .and_then(Value::as_str)
        .expect("add task must return the new task id")
        .to_string();

    // Discover + load the builtin layer: runs the bundle's `load()`, which
    // registers the three commands through the SDK convention.
    let loaded = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the task-commands builtin plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one task-commands builtin plugin should be discovered, got {loaded:?}"
    );

    // ── (1) Discovery + registration: list scoped to entity:task ──────────
    let listed = call_command(
        &service,
        CallerId::HostInternal,
        json!({ "op": "list command", "scope": "entity:task" }),
    )
    .await;
    let commands = commands_by_id(&listed);
    let mut ids: Vec<&String> = commands.keys().collect();
    ids.sort();
    assert_eq!(
        ids,
        vec![
            &"task.doThisNext".to_string(),
            &"task.move".to_string(),
            &"task.untag".to_string(),
        ],
        "list command {{ scope: entity:task }} must return the three ported commands, got {ids:?}"
    );

    // ── (2) Metadata fidelity: lock each command's metadata 1:1 vs task.yaml
    assert_task_move_metadata(&commands["task.move"]);
    assert_task_untag_metadata(&commands["task.untag"]);
    assert_task_do_this_next_metadata(&commands["task.doThisNext"]);

    // ── (4) Preconditions: task.move unavailable with no task / no column ──
    let unavailable_empty = call_command(
        &service,
        CallerId::HostInternal,
        json!({ "op": "available command", "id": "task.move", "ctx": {} }),
    )
    .await;
    assert_eq!(
        unavailable_empty["structuredContent"]["ok"],
        json!(false),
        "task.move must be unavailable with no task in scope, got {unavailable_empty}"
    );

    let unavailable_no_column = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "available command",
            "id": "task.move",
            "ctx": { "scope_chain": [format!("task:{task_id}")] },
        }),
    )
    .await;
    assert_eq!(
        unavailable_no_column["structuredContent"]["ok"],
        json!(false),
        "task.move must be unavailable without a column target, got {unavailable_no_column}"
    );

    // The task starts in the default `todo` column.
    let before = kanban.call(json!({ "op": "list tasks" })).await;
    assert_eq!(
        task_column(&before, &task_id).as_deref(),
        Some("todo"),
        "the seeded task should start in the todo column"
    );

    // ── (3) Real effect: execute task.move into the `doing` column ────────
    let executed = call_command(
        &service,
        CallerId::HostInternal,
        json!({
            "op": "execute command",
            "id": "task.move",
            "ctx": {
                "scope_chain": [format!("task:{task_id}")],
                "target": "column:doing",
            },
        }),
    )
    .await;
    assert_eq!(
        executed["structuredContent"]["ok"],
        json!(true),
        "executing task.move should succeed, got {executed}"
    );

    // Read the board back: the task moved to `doing` — the plugin's single
    // `this.kanban.kanban.task.move(...)` call reached the real store.
    let after = kanban.call(json!({ "op": "list tasks" })).await;
    assert_eq!(
        task_column(&after, &task_id).as_deref(),
        Some("doing"),
        "task.move must have moved the task to the doing column on the real store"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// Per-command metadata regression asserts (locked against task.yaml)
// ───────────────────────────────────────────────────────────────────────────

/// `task.move` — scope entity:task, undoable, no keys/context_menu; params
/// task(scope_chain) / column(target) / drop_index(args).
fn assert_task_move_metadata(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Move Task"), "task.move name");
    assert_eq!(cmd["scope"], json!(["entity:task"]), "task.move scope");
    assert_eq!(cmd["undoable"], json!(true), "task.move undoable");
    assert!(
        cmd.get("keys").is_none() || cmd["keys"].is_null(),
        "task.move carries no keys, got {}",
        cmd["keys"]
    );
    assert!(
        cmd.get("context_menu").is_none() || cmd["context_menu"].is_null(),
        "task.move carries no context_menu"
    );
    assert_eq!(
        cmd["params"],
        json!([
            { "name": "task", "from": "scope_chain", "entity_type": "task" },
            { "name": "column", "from": "target", "entity_type": "column" },
            { "name": "drop_index", "from": "args" },
        ]),
        "task.move params must match task.yaml 1:1"
    );
}

/// `task.untag` — scope entity:tag,entity:task (as a two-element array),
/// undoable, context_menu, keys vim:x / cua:Delete; params tag/task scope_chain.
fn assert_task_untag_metadata(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Remove Tag"), "task.untag name");
    assert_eq!(
        cmd["scope"],
        json!(["entity:tag", "entity:task"]),
        "task.untag scope (the YAML's `entity:tag,entity:task` as a list)"
    );
    assert_eq!(cmd["undoable"], json!(true), "task.untag undoable");
    assert_eq!(cmd["context_menu"], json!(true), "task.untag context_menu");
    assert_eq!(
        cmd["keys"],
        json!({ "vim": "x", "cua": "Delete" }),
        "task.untag keys must match task.yaml"
    );
    assert_eq!(
        cmd["params"],
        json!([
            { "name": "tag", "from": "scope_chain", "entity_type": "tag" },
            { "name": "task", "from": "scope_chain", "entity_type": "task" },
        ]),
        "task.untag params must match task.yaml 1:1"
    );
}

/// `task.doThisNext` — scope entity:task, undoable, context_menu, no keys;
/// params task(scope_chain).
fn assert_task_do_this_next_metadata(cmd: &Value) {
    assert_eq!(cmd["name"], json!("Do This Next"), "task.doThisNext name");
    assert_eq!(
        cmd["scope"],
        json!(["entity:task"]),
        "task.doThisNext scope"
    );
    assert_eq!(cmd["undoable"], json!(true), "task.doThisNext undoable");
    assert_eq!(
        cmd["context_menu"],
        json!(true),
        "task.doThisNext context_menu"
    );
    assert!(
        cmd.get("keys").is_none() || cmd["keys"].is_null(),
        "task.doThisNext carries no keys"
    );
    assert_eq!(
        cmd["params"],
        json!([{ "name": "task", "from": "scope_chain", "entity_type": "task" }]),
        "task.doThisNext params must match task.yaml 1:1"
    );
}
