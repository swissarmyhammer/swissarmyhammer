//! Full-baseline acceptance test for the kanban command cut-over.
//!
//! Stage 4 deleted the 12 builtin command YAMLs (`crates/swissarmyhammer-commands/builtin/commands/*.yaml`
//! and `crates/swissarmyhammer-kanban/builtin/commands/*.yaml` except
//! `ai.yaml`) and retired the legacy dispatch fallback; the follow-up
//! card 01KT6WWYYWFQ2F4PGQ358SAHY7 then ported the final `ai.yaml`
//! window-layer commands onto the `ai-commands` builtin plugin; Card A
//! then ported the `nav.*` commands onto the `nav-commands` builtin
//! plugin, retiring the last YAML command source (`nav.yaml`).
//! `CommandService` (fed by the 9 builtin command plugins at app startup)
//! is now the sole source of every command's metadata. This test pins the
//! "all 100 commands wire through the new path" gate the cut-over depends on.
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
//!    100-id baseline (set-equality with order-stable diff so missing,
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
//! The 100 commands across the 10 builtin command plugins (matches each
//! plugin's `registerCommands(...)` call set):
//!
//! - `app-shell-commands` (33): help, about, quit, command, search,
//!   palette, dismiss, undo, redo, drag.{start,cancel,complete},
//!   settings.keymap.{vim,cua,emacs}, plus the former `ui-commands`
//!   bundle folded in by the ui.*→app.* rename (mop-up card
//!   01KTEBZSVGAZ881RAZZWWZXGPE): app.inspect,
//!   app.inspector.{close,close_all,set_width},
//!   app.palette.{open,close}, app.entity.startRename, app.mode.set,
//!   app.setFocus, window.new, the Card D UI-surface commands
//!   field.{edit,editEnter} and pressable.{activate,activateSpace}, the
//!   Card E editor drill-in commands filter_editor.drillIn,
//!   app.ai-panel.composer.drillIn, and
//!   app.ai-panel.elicitation.field.drillIn — all webview-bus handled
//!   (the owning React components register the live handlers while
//!   focused; the host executes are inert no-ops) — and the Card G
//!   consolidated entity.inspect (the global Space inspect command:
//!   explicit target, else the innermost inspectable scope-chain
//!   moniker, else an inert no-op; routes to ui_state
//!   `inspect inspector`).
//! - `entity-commands` (8): entity.add, entity.update_field, entity.delete,
//!   entity.archive, entity.unarchive, entity.copy, entity.cut, entity.paste.
//! - `file-commands` (4): file.{switchBoard,closeBoard,newBoard,openBoard}.
//! - `kanban-misc-commands` (5): column.reorder, tag.update, attachment.open,
//!   attachment.reveal, view.set.
//! - `perspective-commands` (17): perspective.{switch,delete,rename,goto,
//!   load,save,list,next,prev,filter,filter.focus,clearFilter,group,
//!   clearGroup,sort.set,sort.clear,sort.toggle}.
//! - `task-commands` (3): task.{move,untag,doThisNext}.
//! - `ai-commands` (5): ai.{toggle,focus,newChat,model,cancel} — webview-
//!   reactive no-ops; the metadata surfaces in the unified registry while the
//!   AI panel React tree owns the live effect.
//! - `nav-commands` (10): nav.{up,down,left,right,first,last,drillIn,drillOut,
//!   jump,focus} — directional/drill route to the focus kernel (host-driven),
//!   jump is webview-bus handled, focus routes to the focus `set focus` op.
//!   The first nine are ported from the retired `nav.yaml` overlay; `nav.focus`
//!   is the programmatic focus-claim command (no keys, no menu).
//! - `grid-commands` (11): grid.{moveToRowStart,moveToRowEnd,firstCell,
//!   lastCell,edit,editEnter,exitEdit,toggleVisual,deleteRow,newBelow,
//!   newAbove} — all webview-bus handled (Card C); the grid React tree
//!   registers the live handlers, the host executes are inert no-ops.
//! - `board-commands` (4): board.{newTask,firstColumn,lastColumn} +
//!   group.toggleCollapse — Card F plus the vim `z o` group collapse-toggle.
//!   firstColumn/lastColumn route to the focus kernel's `navigate focus` op
//!   (first/last) host-driven; newTask and group.toggleCollapse are webview-bus
//!   handled (the board React tree / each group section registers the live
//!   handler, the host execute is inert).
//!
//! TOTAL: 100 commands.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::{json, Map, Value};
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_directory::KanbanConfig;
use swissarmyhammer_plugin::{
    CallerId, InProcessServer, McpServer as PluginMcpServer, PluginHost, Result as PluginResult,
    ToolMetadata, PLUGINS_SUBDIR,
};
use swissarmyhammer_ui_state::{UiState, UiStateServer};
use tempfile::TempDir;

use crate::support::{call_command, copy_dir_recursive};

/// A generous upper bound on the discovery+load step. Loading 7
/// TypeScript bundles through 7 Deno isolates runs well under this in
/// practice; the limit only exists so a hung test does not block the
/// rest of the suite.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// The 10 builtin command plugins, in the order the app loads them.
/// The grand union of their command registrations is the 100 ids asserted
/// below. (The former `ui-commands` bundle was folded into
/// `app-shell-commands` by the ui.*→app.* rename — there is no `ui.*`
/// command namespace.)
const BUILTIN_COMMAND_PLUGINS: &[&str] = &[
    "app-shell-commands",
    "entity-commands",
    "file-commands",
    "kanban-misc-commands",
    "perspective-commands",
    "task-commands",
    "ai-commands",
    "nav-commands",
    "grid-commands",
    "board-commands",
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
/// errors out on the host's available-modules table. `ui_state` is also
/// absent here: it is exposed as the REAL [`UiStateServer`] (not a stub) so
/// its declared `aiStreaming` notification `_meta` is present — the
/// `ai-commands` bundle's `this.ui_state.on("aiStreaming", …)` subscription
/// resolves against it at load time and would otherwise throw
/// `UnknownNotification`.
const STUB_BACKENDS: &[&str] = &[
    "store", "entity", "kanban", "views", "window", "app", "focus",
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

/// Expose every backend the plugins need to satisfy `ensureServices`.
///
/// Most backends are no-op [`StubBackend`]s — registration only needs the
/// module to exist. `ui_state` is the exception: it is the REAL
/// [`UiStateServer`] so its `aiStreaming` notification `_meta` is advertised and
/// the `ai-commands` bundle's `this.ui_state.on("aiStreaming", …)` resolves
/// (the kept `_dir` keeps the temp-file substrate alive for the load).
async fn expose_stub_backends(host: &PluginHost, dir: &TempDir) {
    for module_id in STUB_BACKENDS {
        let backend = StubBackend::new(*module_id);
        host.expose_rust_module(
            module_id.to_string(),
            Arc::new(backend) as Arc<dyn PluginMcpServer>,
        )
        .await
        .unwrap_or_else(|e| panic!("exposing stub backend {module_id:?} should succeed: {e:?}"));
    }

    let ui_state = Arc::new(UiState::load(dir.path().join("ui_state.yaml")));
    let ui_state_server = UiStateServer::new(ui_state);
    let ui_state_module = InProcessServer::new(ui_state_server)
        .await
        .expect("wrapping the real ui_state server should succeed");
    host.expose_rust_module(
        "ui_state".to_string(),
        Arc::new(ui_state_module) as Arc<dyn PluginMcpServer>,
    )
    .await
    .expect("exposing the real ui_state module should succeed");
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
        .filter_map(|entry| entry.get("id").and_then(Value::as_str).map(str::to_string))
        .collect()
}

/// The locked 100-id baseline.
///
/// Mirrors the actual union of each plugin's `registerCommands(...)`
/// calls as of the Stage 4 cut-over. If a plugin's command set
/// legitimately changes, update this list alongside that plugin's
/// per-bundle `builtin_*_commands_e2e.rs` test.
fn expected_command_ids() -> BTreeSet<String> {
    [
        // app-shell-commands (33) — the 15 platform-shell commands plus the
        // former ui-commands bundle folded in by the ui.*→app.* rename
        // (mop-up card 01KTEBZSVGAZ881RAZZWWZXGPE).
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
        // ...the former ui-commands set, every id now app.* (the `ui.*`
        // command namespace is retired). field.* / pressable.* are the
        // Card D UI-surface commands: webview-bus handled, host executes
        // inert.
        "app.entity.startRename",
        "app.inspect",
        "app.inspector.close",
        "app.inspector.close_all",
        "app.inspector.set_width",
        "app.mode.set",
        "app.palette.close",
        "app.palette.open",
        "app.setFocus",
        "window.new",
        "field.edit",
        "field.editEnter",
        "pressable.activate",
        "pressable.activateSpace",
        // ...and the Card E editor drill-in commands: webview-bus handled,
        // host executes inert. ONE base elicitation id — the per-field
        // variation lives in the focus-gated bus registration, not minted ids.
        "filter_editor.drillIn",
        "app.ai-panel.composer.drillIn",
        "app.ai-panel.elicitation.field.drillIn",
        // ...and the Card G consolidated global Space inspect command:
        // explicit target, else the innermost inspectable scope-chain
        // moniker, else an inert no-op.
        "entity.inspect",
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
        // ai-commands (5)
        "ai.cancel",
        "ai.focus",
        "ai.model",
        "ai.newChat",
        "ai.toggle",
        // nav-commands (10) — `nav.focus` is the programmatic focus-claim
        // command (never in nav.yaml: no keys, no menu placement).
        "nav.up",
        "nav.down",
        "nav.left",
        "nav.right",
        "nav.first",
        "nav.last",
        "nav.drillIn",
        "nav.drillOut",
        "nav.jump",
        "nav.focus",
        // grid-commands (11) — all webview-bus handled (Card C): the grid
        // React tree registers the live handlers; host executes are inert.
        "grid.moveToRowStart",
        "grid.moveToRowEnd",
        "grid.firstCell",
        "grid.lastCell",
        "grid.edit",
        "grid.editEnter",
        "grid.exitEdit",
        "grid.toggleVisual",
        "grid.deleteRow",
        "grid.newBelow",
        "grid.newAbove",
        // board-commands (4) — Card F: firstColumn/lastColumn route to the
        // focus kernel's `navigate focus` op (first/last); newTask and
        // group.toggleCollapse are webview-bus handled (the board React tree /
        // each group section registers the live handler; the host execute is
        // inert).
        "board.newTask",
        "board.firstColumn",
        "board.lastColumn",
        // group.toggleCollapse — vim `z o`, webview-bus handled (each group
        // section registers a focus-gated handler that flips the focused
        // group's collapsed state); the host execute is inert. Lives in the
        // board-commands bundle because grouping is a board-view affordance.
        "group.toggleCollapse",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Everything [`boot_all_builtin_plugins`] hands back. The temp dirs ride
/// along so the staged builtin layer stays alive for the test's duration.
struct BootedBuiltins {
    service: Arc<swissarmyhammer_command_service::CommandService>,
    _host: PluginHost,
    _user_root: TempDir,
    _builtin_root: TempDir,
    /// Temp-file substrate for the real `ui_state` backend, kept alive so its
    /// `UiState` file outlives the plugin loads.
    _ui_state_dir: TempDir,
}

/// Boot a real `PluginHost` against a temp builtin-layer with all 10
/// committed builtin command plugins staged, install the commands module,
/// stub every other backend the plugins reach for, and load every plugin.
async fn boot_all_builtin_plugins() -> BootedBuiltins {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");

    stage_all_builtin_command_plugins(builtin_root.path());

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

    // Stub every backend the bundles' `ensureServices(this, [...])`
    // step reaches for BEFORE discovery. With those modules present
    // on the host's available-modules table, each plugin's `load()`
    // runs to completion and its `registerCommands(...)` helper lands
    // on the service.
    let ui_state_dir = TempDir::new().expect("ui_state substrate temp dir");
    tokio::time::timeout(TIMEOUT, expose_stub_backends(&host, &ui_state_dir))
        .await
        .expect("stub-backend exposure should not hang");

    // Discover and load every staged plugin in one pass.
    let _ = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discover_and_load_all should not hang");

    BootedBuiltins {
        service,
        _host: host,
        _user_root: user_root,
        _builtin_root: builtin_root,
        _ui_state_dir: ui_state_dir,
    }
}

/// Boot all 10 committed builtin command plugins and assert the union of
/// registered commands equals the locked baseline.
#[tokio::test]
async fn all_builtin_command_plugins_register_their_full_command_set() {
    let booted = boot_all_builtin_plugins().await;
    let service = &booted.service;

    // List every registered command and compare the id-set against
    // the locked baseline.
    let listed = call_command(
        service,
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
        100,
        "the 10 builtin command plugins must collectively register exactly 100 commands"
    );
}

/// Guard (mop-up card 01KTEBZSVGAZ881RAZZWWZXGPE): the `ui.*` command
/// namespace is retired — every former `ui.*` command is an `app.*` command.
/// The command-id namespace is independent of which MCP server backs the
/// command (the `ui_state` SERVER keeps its name; only command ids are
/// covered here). No registered command id may ever begin with `ui.` again.
#[tokio::test]
async fn no_registered_command_id_starts_with_the_retired_ui_prefix() {
    let booted = boot_all_builtin_plugins().await;

    let listed = call_command(
        &booted.service,
        CallerId::HostInternal,
        json!({ "op": "list command" }),
    )
    .await;
    let offenders: Vec<String> = registered_ids(&listed)
        .into_iter()
        .filter(|id| id.starts_with("ui."))
        .collect();

    assert!(
        offenders.is_empty(),
        "the `ui.*` command namespace is retired — every UI-surface command \
         is an `app.*` command; offending registrations: {offenders:?}"
    );
}

/// Guard: no command surfaced by `list command` may ever carry a raw
/// `{{...}}` placeholder in its display captions (`name` / `menu_name`) —
/// neither with a focused-entity context (placeholders render against the
/// focused object) nor without one (placeholders fall back to a clean
/// generic form).
///
/// Sweeps every command registered by all 10 committed builtin plugins, so a
/// future plugin caption that introduces a placeholder the renderer cannot
/// resolve fails here instead of leaking into the palette / menus
/// (regression guard for kanban card 01KTRMXRNH66GZCWSNR1YGE28E).
#[tokio::test]
async fn no_surfaced_display_caption_contains_raw_placeholders() {
    let booted = boot_all_builtin_plugins().await;
    let service = &booted.service;

    // Both flavors of listing context: a focused task (the palette with a
    // card focused) and no context at all (a context-free surface).
    let arguments = [
        json!({
            "op": "list command",
            "ctx": { "scope_chain": ["task:01HTASK", "view:01HVIEW", "board:01HBOARD"] },
        }),
        json!({ "op": "list command" }),
    ];

    for args in arguments {
        let listed = call_command(service, CallerId::HostInternal, args.clone()).await;
        let commands = listed
            .get("structuredContent")
            .and_then(|sc| sc.get("commands"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(
            !commands.is_empty(),
            "the builtin plugins must register commands for the sweep to be meaningful"
        );

        for cmd in &commands {
            let id = cmd.get("id").and_then(Value::as_str).unwrap_or("<no id>");
            for field in ["name", "menu_name"] {
                if let Some(caption) = cmd.get(field).and_then(Value::as_str) {
                    assert!(
                        !caption.contains("{{"),
                        "command {id:?} surfaced a raw placeholder in its {field}: \
                         {caption:?} (args: {args})"
                    );
                }
            }
        }
    }
}
