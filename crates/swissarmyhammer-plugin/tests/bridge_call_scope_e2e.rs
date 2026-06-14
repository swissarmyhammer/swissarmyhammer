//! End-to-end regression test for the **bridge-call context-propagation seam**
//! ([`BridgeCallScope`]).
//!
//! # The bug this guards
//!
//! A plugin command's `execute` callback runs on the plugin's isolate worker
//! thread. When that callback calls back into an in-process server (in
//! production: `this.store`, `this.entity`, `this.views`), the call leaves the
//! isolate as a `toolsCall` envelope and the host services it by spawning the
//! routed future onto the host's long-lived `bridge_runtime` — a *different*
//! tokio task on a *different* runtime than the one that drove the dispatch.
//!
//! Production servers resolve their per-board services from a
//! `tokio::task_local!` scoped by the dispatcher around the dispatch. That
//! task-local does **not** cross the thread/runtime hop, so on the bridge
//! runtime those servers ran unscoped — "no board scoped" failures for every
//! store/entity/views call a command callback made.
//!
//! [`PluginHost::set_bridge_call_scope`] is the fix: a higher layer installs a
//! [`BridgeCallScope`] that re-establishes its ambient task-local scopes around
//! the routed call future on the bridge runtime.
//!
//! # What this test models
//!
//! It mirrors the production topology with a self-contained, in-crate server —
//! `swissarmyhammer-plugin` cannot depend on the store/entity/views crates that
//! own the real task-locals (those depend on *it*). The
//! [`BoardScopedServer`]'s only tool reads a crate-local `tokio::task_local!`
//! ([`CURRENT_BOARD`]) and reports it (or the sentinel `"UNSCOPED"` when no
//! scope is active). A real probe plugin, loaded from disk into a real V8
//! isolate, registers that server and — in `load()` — calls the tool through
//! the **real** [`HostBridge`] / `bridge_runtime`, writing the reported value to
//! a probe file.
//!
//! With a [`BridgeCallScope`] installed that scopes [`CURRENT_BOARD`] to a known
//! board id, the probe file must hold that board id. Before the seam applied the
//! scope on the bridge runtime, the file held `"UNSCOPED"` — the exact shape of
//! the production failure.
//!
//! # Isolation
//!
//! The test owns its own [`tempfile::TempDir`] roots and a fresh
//! [`PluginHost`]; nothing is `static` except the crate-local task-local the
//! test itself defines, and every cross-thread interaction is bounded by a
//! timeout.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use swissarmyhammer_directory::SwissarmyhammerConfig;
use swissarmyhammer_plugin::{
    BridgeCallFuture, BridgeCallScope, InProcessServer, McpServer as PluginMcpServer, PluginHost,
};

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use serde::{Deserialize, Serialize};

/// A generous upper bound on any single host or isolate interaction.
const TIMEOUT: Duration = Duration::from_secs(60);

/// The module id the board-scoped server is exposed under.
const BOARD_MODULE_ID: &str = "board_scoped";

/// The server name the probe plugin registers the board-scoped server under.
const BOARD_SERVER_NAME: &str = "board";

/// The probe file the plugin's `load()` writes the tool's reported board into.
const PROBE_FILE: &str = "board_probe.txt";

/// The board id the test scopes the task-local to — the value a correct seam
/// must surface on the bridge runtime.
const SCOPED_BOARD: &str = "board-under-test-42";

/// The sentinel the tool reports when no board scope is active on its task.
const UNSCOPED_SENTINEL: &str = "UNSCOPED";

tokio::task_local! {
    /// Crate-local stand-in for the production per-board task-locals
    /// (`CURRENT_STORE_CTX`, `CURRENT_ENTITY_BOARD_SERVICES`,
    /// `CURRENT_VIEWS_BOARD_SERVICES`). The [`BoardScopedServer`] resolves the
    /// active board from it exactly as those servers resolve their services.
    static CURRENT_BOARD: String;
}

/// Arguments for the board-scoped server's single tool: where to write.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ReportArgs {
    /// Absolute path the tool writes the resolved board id into.
    out_path: String,
}

/// A real `rmcp` server whose tool resolves the active board from a
/// `tokio::task_local!` — the in-crate model of a per-board substrate server.
#[derive(Clone)]
struct BoardScopedServer {
    /// The macro-generated tool router for this handler.
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl BoardScopedServer {
    /// Builds a [`BoardScopedServer`] with its tool router wired up.
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// Resolves the active board from [`CURRENT_BOARD`] and writes it to disk.
    ///
    /// Reports [`UNSCOPED_SENTINEL`] when no scope is active on the calling
    /// task — the observable signature of the cross-runtime context loss.
    #[tool(
        name = "report_board",
        description = "Writes the task-local-resolved board id to a file."
    )]
    async fn report_board(&self, Parameters(args): Parameters<ReportArgs>) -> String {
        let board = CURRENT_BOARD
            .try_with(|board| board.clone())
            .unwrap_or_else(|_| UNSCOPED_SENTINEL.to_string());
        std::fs::write(&args.out_path, &board).expect("probe write should succeed");
        board
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for BoardScopedServer {}

/// A [`BridgeCallScope`] that scopes [`CURRENT_BOARD`] to a fixed board id
/// around every routed bridge call — the in-crate analogue of the kanban app
/// re-establishing its store/entity/views task-locals.
struct BoardScope {
    /// The board id to scope every wrapped call to.
    board: String,
}

impl BridgeCallScope for BoardScope {
    fn scope(&self, call: BridgeCallFuture) -> BridgeCallFuture {
        let board = self.board.clone();
        Box::pin(CURRENT_BOARD.scope(board, call))
    }
}

/// Writes the probe plugin bundle into `<project_root>/plugins/probe/`.
///
/// The plugin registers the exposed board-scoped server and, in `load()`, calls
/// its `report_board` tool through the real bridge, handing it the absolute
/// `out_path` the test owns.
fn write_probe_plugin(project_root: &Path, out_path: &Path) {
    let plugin_dir = project_root
        .join(swissarmyhammer_plugin::PLUGINS_SUBDIR)
        .join("probe");
    std::fs::create_dir_all(&plugin_dir).expect("probe plugin directory should be created");

    let entry = format!(
        "import {{ Plugin }} from '@swissarmyhammer/plugin';\n\
         \n\
         export default class ProbePlugin extends Plugin {{\n\
         \x20 async load(): Promise<void> {{\n\
         \x20   this.register({server}, {{ rust: {module} }});\n\
         \x20   await this.{server_ident}.report_board({{ out_path: {out_path} }});\n\
         \x20 }}\n\
         }}\n",
        server = json_string(BOARD_SERVER_NAME),
        module = json_string(BOARD_MODULE_ID),
        server_ident = BOARD_SERVER_NAME,
        out_path = json_string(&out_path.to_string_lossy()),
    );
    std::fs::write(plugin_dir.join("index.ts"), entry).expect("probe index.ts should be written");
}

/// Encodes `value` as a JSON/TypeScript string literal, quotes included.
fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("a string always serializes to JSON")
}

/// A probe plugin's callback, routed through the real bridge runtime, must see
/// the board the dispatcher scoped — proving [`BridgeCallScope`] re-establishes
/// the task-local across the thread/runtime hop.
#[tokio::test]
async fn bridge_call_sees_scoped_board_across_the_runtime_hop() {
    let project_root = tempfile::TempDir::new().expect("project plugin root temp");
    let output_dir = tempfile::TempDir::new().expect("probe output temp");
    let out_path = output_dir.path().join(PROBE_FILE);

    write_probe_plugin(project_root.path(), &out_path);

    // A fresh per-board-style host with the project layer pointed at the temp
    // plugin root.
    let host = PluginHost::for_tests(
        output_dir.path().to_path_buf(),
        Some(project_root.path().to_path_buf()),
    );

    // Expose the real (in-crate) board-scoped server as an addressable module.
    let server: Arc<dyn PluginMcpServer> = Arc::new(
        InProcessServer::new(BoardScopedServer::new())
            .await
            .expect("wrapping the board-scoped rmcp handler should succeed"),
    );
    host.expose_rust_module(BOARD_MODULE_ID, server)
        .await
        .expect("exposing the board-scoped module should succeed");

    // Install the scope BEFORE loading: the plugin's `load()` makes the bridge
    // call, so the scope must be live when discovery drives that callback.
    let _scope_guard = host.set_bridge_call_scope(Arc::new(BoardScope {
        board: SCOPED_BOARD.to_string(),
    }));

    let loaded = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("discovering and loading the probe plugin should succeed");
    assert_eq!(loaded.len(), 1, "exactly the one probe plugin should load");

    let reported = std::fs::read_to_string(&out_path).unwrap_or_else(|error| {
        panic!(
            "the probe file must exist at {} — the bridge call did not reach the \
             board-scoped tool: {error}",
            out_path.display()
        )
    });

    assert_eq!(
        reported, SCOPED_BOARD,
        "the command callback's bridge-routed tool call must resolve the board \
         the dispatcher scoped ({SCOPED_BOARD}); got {reported:?}. A value of \
         {UNSCOPED_SENTINEL:?} means the task-local scope was lost across the \
         isolate→bridge_runtime hop — the exact bug BridgeCallScope fixes."
    );
}

/// Control: with NO scope installed, the same bridge call resolves the
/// unscoped sentinel — proving the assertion above is meaningful (the scope is
/// what makes the difference, not some always-on default).
#[tokio::test]
async fn bridge_call_is_unscoped_without_a_scope_installed() {
    let project_root = tempfile::TempDir::new().expect("project plugin root temp");
    let output_dir = tempfile::TempDir::new().expect("probe output temp");
    let out_path = output_dir.path().join(PROBE_FILE);

    write_probe_plugin(project_root.path(), &out_path);

    let host = PluginHost::for_tests(
        output_dir.path().to_path_buf(),
        Some(project_root.path().to_path_buf()),
    );

    let server: Arc<dyn PluginMcpServer> = Arc::new(
        InProcessServer::new(BoardScopedServer::new())
            .await
            .expect("wrapping the board-scoped rmcp handler should succeed"),
    );
    host.expose_rust_module(BOARD_MODULE_ID, server)
        .await
        .expect("exposing the board-scoped module should succeed");

    // No `set_bridge_call_scope` here.
    let loaded = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("discovering and loading the probe plugin should succeed");
    assert_eq!(loaded.len(), 1, "exactly the one probe plugin should load");

    let reported = std::fs::read_to_string(&out_path).expect("the probe file must exist");
    assert_eq!(
        reported, UNSCOPED_SENTINEL,
        "with no scope installed the routed call must run unscoped"
    );
}
