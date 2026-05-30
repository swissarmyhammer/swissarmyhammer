//! Full-baseline acceptance test for Stage 4 of the kanban cut-over.
//!
//! Stage 4 deleted the 12 builtin command YAMLs (`crates/swissarmyhammer-commands/builtin/commands/*.yaml`
//! and `crates/swissarmyhammer-kanban/builtin/commands/*.yaml` except
//! `ai.yaml`) and retired the legacy dispatch fallback —
//! `CommandService` (fed by the 7 builtin command plugins at app
//! startup) is now the sole source of every command's metadata. This
//! test pins the "all 62 commands wire through the new path" gate the
//! cut-over depends on.
//!
//! Scope delivered: **registration coverage** end-to-end.
//!
//! 1. Boot a real [`PluginHost`] against a temp builtin-layer with
//!    every committed builtin command plugin staged.
//! 2. Install the `commands` module via the production bootstrap
//!    [`install_commands_module`].
//! 3. Stub every other backend module the plugins'
//!    `ensureServices(this, [...])` calls reach for, so each bundle's
//!    `load()` can run to completion and its `registerCommands(...)`
//!    helper lands on the service.
//! 4. Discover and load every plugin.
//! 5. Assert the union of registered commands matches the locked
//!    62-id baseline (set-equality with order-stable diff so missing,
//!    extra, or renamed ids surface explicitly).
//!
//! Dispatch coverage (per-command real effects against the matching
//! backend) lives in the per-plugin `builtin_*_commands_e2e.rs` tests
//! in this suite — each stages one bundle and wires the real backend
//! that bundle uses. The full baseline does not re-run those; its job
//! is to catch a regression where a plugin silently fails to register
//! one of its commands, the sort of drop the per-plugin tests cannot
//! catch because each only stages one bundle in isolation.
//!
//! The 62 commands across the 7 builtin command plugins (matches each
//! plugin's `registerCommands(...)` call set):
//!
//! - `app-shell-commands` (15): help, about, quit, command, search,
//!   palette, dismiss, undo, redo, drag.{start,cancel,complete},
//!   settings.keymap.{vim,cua,emacs}.
//! - `entity-commands` (8): entity.add, entity.update_field, entity.delete,
//!   entity.archive, entity.unarchive, entity.copy, entity.cut, entity.paste.
//! - `file-commands` (4): file.{switchBoard,closeBoard,newBoard,openBoard}.
//! - `kanban-misc-commands` (5): column.reorder, tag.update, attachment.open,
//!   attachment.reveal, view.set.
//! - `perspective-commands` (17): perspective.{switch,delete,rename,goto,
//!   load,save,list,next,prev,filter,filter.focus,clearFilter,group,
//!   clearGroup,sort.set,sort.clear,sort.toggle}.
//! - `task-commands` (3): task.{move,untag,doThisNext}.
//! - `ui-commands` (10): ui.inspect, ui.inspector.{close,close_all,set_width},
//!   ui.palette.{open,close}, ui.entity.startRename, ui.mode.set, ui.setFocus,
//!   window.new.
//!
//! TOTAL: 62 commands.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::{json, Map, Value};
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_directory::KanbanConfig;
use swissarmyhammer_plugin::{
    CallerId, McpServer as PluginMcpServer, PluginHost, Result as PluginResult, ToolMetadata,
    PLUGINS_SUBDIR,
};
use tempfile::TempDir;

use crate::support::call_command;

/// A generous upper bound on the discovery+load step. Loading 7
/// TypeScript bundles through 7 Deno isolates runs well under this in
/// practice; the limit only exists so a hung test does not block the
/// rest of the suite.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// The 7 builtin command plugins, in the order the app loads them.
/// The grand union of their `commands.yaml` registrations is the 62
/// ids asserted below.
const BUILTIN_COMMAND_PLUGINS: &[&str] = &[
    "app-shell-commands",
    "entity-commands",
    "file-commands",
    "kanban-misc-commands",
    "perspective-commands",
    "task-commands",
    "ui-commands",
];

/// Backend modules every builtin command plugin's
/// `ensureServices(this, [...])` may reach for. The set is the union
/// across the 7 bundles' load steps; exposing them all up-front means
/// no bundle's `ensureServices` step trips on a missing module, so the
/// `registerCommands(...)` helper inside each `load()` runs to
/// completion.
///
/// `commands` is intentionally absent — `install_commands_module`
/// exposes it as part of the bootstrap; trying to expose it twice
/// errors out on the host's available-modules table.
const STUB_BACKENDS: &[&str] = &[
    "store", "entity", "kanban", "views", "ui_state", "window", "app", "focus",
];

/// A no-op [`PluginMcpServer`] used to satisfy the per-plugin
/// `ensureServices(this, [...])` step at load time. The server
/// advertises one tool whose name matches the module id; any
/// `tools/call` returns `null` — registration only needs the module
/// to exist, not for its tools to do real work. Per-plugin e2e tests
/// remain the canonical proof for real-effect dispatch.
struct StubBackend {
    tool_name: String,
}

impl StubBackend {
    fn new(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
        }
    }
}

#[async_trait::async_trait]
impl PluginMcpServer for StubBackend {
    fn tools(&self) -> Vec<ToolMetadata> {
        let mut schema = Map::new();
        schema.insert("type".to_string(), json!("object"));
        let tool = Tool::new(
            self.tool_name.clone(),
            "stub backend for full_baseline_e2e",
            Arc::new(schema),
        );
        vec![ToolMetadata::new(tool)]
    }

    async fn invoke(&self, _caller: CallerId, _tool: &str, _input: Value) -> PluginResult<Value> {
        Ok(Value::Null)
    }
}

/// Resolve the workspace root (two levels above this crate's manifest dir).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root is two levels above the crate manifest dir")
        .to_path_buf()
}

/// Recursively copy a directory tree from `source` into `destination`.
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

/// Stage every committed builtin command plugin bundle into a temp
/// builtin-layer root at `<layer_root>/plugins/<plugin>/`, so
/// `discover_and_load_all` finds them all in one pass.
fn stage_all_builtin_command_plugins(layer_root: &Path) {
    for plugin in BUILTIN_COMMAND_PLUGINS {
        let source = workspace_root().join("builtin/plugins").join(plugin);
        assert!(
            source.is_dir(),
            "the committed builtin command plugin bundle must exist at {}",
            source.display()
        );
        let destination = layer_root.join(PLUGINS_SUBDIR).join(plugin);
        copy_dir_recursive(&source, &destination);
    }
}

/// Expose every stub backend the plugins need to satisfy `ensureServices`.
async fn expose_stub_backends(host: &PluginHost) {
    for module_id in STUB_BACKENDS {
        let backend = StubBackend::new(*module_id);
        host.expose_rust_module(
            module_id.to_string(),
            Arc::new(backend) as Arc<dyn PluginMcpServer>,
        )
        .await
        .unwrap_or_else(|e| {
            panic!("exposing stub backend {module_id:?} should succeed: {e:?}")
        });
    }
}

/// Pull the registered command id-set out of the `list command` reply.
fn registered_ids(list_result: &Value) -> BTreeSet<String> {
    list_result
        .get("structuredContent")
        .and_then(|sc| sc.get("commands"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            entry
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

/// The locked 62-id baseline.
///
/// Mirrors the actual union of each plugin's `registerCommands(...)`
/// calls as of the Stage 4 cut-over. If a plugin's command set
/// legitimately changes, update this list alongside that plugin's
/// per-bundle `builtin_*_commands_e2e.rs` test.
fn expected_command_ids() -> BTreeSet<String> {
    [
        // app-shell-commands (15)
        "app.about",
        "app.command",
        "app.dismiss",
        "app.help",
        "app.palette",
        "app.quit",
        "app.redo",
        "app.search",
        "app.undo",
        "drag.cancel",
        "drag.complete",
        "drag.start",
        "settings.keymap.cua",
        "settings.keymap.emacs",
        "settings.keymap.vim",
        // entity-commands (8)
        "entity.add",
        "entity.archive",
        "entity.copy",
        "entity.cut",
        "entity.delete",
        "entity.paste",
        "entity.unarchive",
        "entity.update_field",
        // file-commands (4)
        "file.closeBoard",
        "file.newBoard",
        "file.openBoard",
        "file.switchBoard",
        // kanban-misc-commands (5)
        "attachment.open",
        "attachment.reveal",
        "column.reorder",
        "tag.update",
        "view.set",
        // perspective-commands (17)
        "perspective.clearFilter",
        "perspective.clearGroup",
        "perspective.delete",
        "perspective.filter",
        "perspective.filter.focus",
        "perspective.goto",
        "perspective.group",
        "perspective.list",
        "perspective.load",
        "perspective.next",
        "perspective.prev",
        "perspective.rename",
        "perspective.save",
        "perspective.sort.clear",
        "perspective.sort.set",
        "perspective.sort.toggle",
        "perspective.switch",
        // task-commands (3)
        "task.doThisNext",
        "task.move",
        "task.untag",
        // ui-commands (10)
        "ui.entity.startRename",
        "ui.inspect",
        "ui.inspector.close",
        "ui.inspector.close_all",
        "ui.inspector.set_width",
        "ui.mode.set",
        "ui.palette.close",
        "ui.palette.open",
        "ui.setFocus",
        "window.new",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Boot a real `PluginHost` against a temp builtin-layer with all 7
/// committed builtin command plugins staged, install the commands
/// module, stub every other backend the plugins reach for, load every
/// plugin, and assert the union of registered commands equals the
/// locked baseline.
#[tokio::test]
async fn all_seven_builtin_command_plugins_register_their_full_command_set() {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");

    stage_all_builtin_command_plugins(builtin_root.path());

    let host = PluginHost::new(
        Some(builtin_root.path().to_path_buf()),
        user_root.path().to_path_buf(),
        None,
        false,
        user_root.path().to_path_buf(),
    );

    let service = install_commands_module(&host)
        .await
        .expect("install_commands_module must succeed");

    // Stub every backend the bundles' `ensureServices(this, [...])`
    // step reaches for BEFORE discovery. With those modules present
    // on the host's available-modules table, each plugin's `load()`
    // runs to completion and its `registerCommands(...)` helper lands
    // on the service.
    tokio::time::timeout(TIMEOUT, expose_stub_backends(&host))
        .await
        .expect("stub-backend exposure should not hang");

    // Discover and load every staged plugin in one pass.
    let _ = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discover_and_load_all should not hang");

    // List every registered command and compare the id-set against
    // the locked baseline.
    let listed = call_command(
        &service,
        CallerId::HostInternal,
        json!({ "op": "list command" }),
    )
    .await;
    let got: BTreeSet<String> = registered_ids(&listed);
    let want: BTreeSet<String> = expected_command_ids();

    let missing: Vec<&String> = want.difference(&got).collect();
    let extra: Vec<&String> = got.difference(&want).collect();

    assert!(
        missing.is_empty() && extra.is_empty(),
        "command registration drifted from the locked baseline.\n\
         missing ({} ids): {:?}\n\
         extra ({} ids): {:?}\n\
         expected {} ids, got {} ids",
        missing.len(),
        missing,
        extra.len(),
        extra,
        want.len(),
        got.len(),
    );

    assert_eq!(
        got.len(),
        62,
        "the 7 builtin command plugins must collectively register exactly 62 commands"
    );
}
