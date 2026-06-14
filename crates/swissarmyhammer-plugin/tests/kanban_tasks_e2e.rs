//! End-to-end integration test for the committed `kanban-tasks` example
//! plugin — the flagship demonstration of operation-tool `_meta` path dispatch.
//!
//! Where `files_dispatch_e2e.rs` drives the unified `files` tool with the
//! **direct** `{ op: "..." }` form, and `operation_meta_e2e.rs` exercises the
//! `_meta` path sugar with a test-generated probe plugin, this test proves the
//! same `_meta` round-trip works for a **real, committed bundle** discovered
//! from disk, driving a **real operation tool** — the in-process `kanban` tool
//! — whose observable state is a `.kanban` board on disk.
//!
//! # Why the `kanban` tool is the operation tool under test
//!
//! The `kanban` tool is a genuine in-process MCP operation tool: its
//! `tools/list` definition carries an `io.swissarmyhammer/operations` `_meta`
//! tree keyed by noun (`task`, `tasks`, `board`, …) and verb (`add`, `list`,
//! `init`, …), with each leaf's `op` being the canonical `"<verb> <noun>"`
//! selector. Unlike the `files` tool, its state is not the filesystem at large
//! but a single `.kanban` board directory — so verification is unambiguous:
//! "did the tasks the plugin added appear on the board."
//!
//! The `kanban` tool is exposed to the host with [`support::expose_kanban_module`],
//! which mirrors the production wiring in the kanban desktop app
//! (`apps/kanban-app/src/plugins.rs`) — a real [`ToolRegistry`] holding the
//! kanban tools, paired with a [`ToolContext`] pointed at a temp board root.
//! No mock, no hand-built `_meta`.
//!
//! # What a passing run proves
//!
//! The committed `kanban-tasks` bundle's `load()` registers the host-exposed
//! `kanban` tool as `board` and adds two tasks through the SDK's **path form**
//! — `this.board.kanban.task.add({ title })`. The path form carries no `op`;
//! the SDK must read the tool's
//! `_meta["io.swissarmyhammer/operations"]["task"]["add"].op`, find
//! `"add task"`, and dispatch `tools/call("kanban", { op: "add task", … })`.
//!
//! Reading the temp board back and finding **exactly** those two tasks proves
//! the whole round trip: a broken `_meta` lookup would raise `UnknownOperation`
//! and fail `load()` before either task was ever created.
//!
//! # Isolation
//!
//! The test owns its own [`tempfile::TempDir`] roots and a fresh
//! [`PluginHost`]; nothing is `static` and no temp dir is reused. Every
//! cross-thread interaction is bounded by [`support::TIMEOUT`] so a wedged
//! isolate fails the test fast instead of hanging CI.

mod support;

use serde_json::json;
use swissarmyhammer_directory::KanbanConfig;
use swissarmyhammer_plugin::PluginHost;

/// The board name the test initializes the temp `.kanban` board with.
const BOARD_NAME: &str = "kanban-tasks example board";

/// The titles the committed `kanban-tasks` plugin adds through the SDK path
/// form. They must match the titles hard-coded in the example bundle's
/// `index.ts` — the test asserts the board holds exactly these two.
const FIRST_TASK_TITLE: &str = "Draft the plugin proposal";
const SECOND_TASK_TITLE: &str = "Review the plugin proposal";

/// The committed `kanban-tasks` example plugin, discovered from disk, drives
/// the real `kanban` operation tool through the SDK's `_meta` path sugar and
/// lands two real tasks on a temp board.
///
/// This single test stitches the operation-`_meta` round-trip together end to
/// end against a committed bundle:
///
/// - the real `kanban` operation tool — carrying its
///   `io.swissarmyhammer/operations` `_meta` — is built by
///   [`support::expose_kanban_module`] over a temp board root and exposed to
///   the host with `expose_rust_module`; no mock, no hand-built `_meta`;
/// - the committed `kanban-tasks` bundle is staged into the project layer with
///   [`support::stage_example`] and discovered through `discover_and_load_all`,
///   which transpiles its `index.ts`, creates a fresh V8 isolate, and runs the
///   exported `load`;
/// - inside the isolate the SDK reads the operation tool's `_meta` to turn each
///   `task.add` path into `tools/call("kanban", { op: "add task", … })`.
///
/// The assertion observes the board on disk — the only honest verification for
/// an in-process operation tool whose state *is* the `.kanban` board.
#[tokio::test]
async fn kanban_tasks_plugin_adds_tasks_via_meta_path() {
    // Per-test isolation: every root is this test's own `TempDir`.
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let project_root = tempfile::TempDir::new().expect("project plugin root temp");
    let board_dir = tempfile::TempDir::new().expect("kanban board temp");

    // Stage the committed `kanban-tasks` bundle into the project layer's
    // `plugins/` directory, where discovery will find it. The bundle is real,
    // committed source — no inline plugin is written here.
    support::stage_example("kanban-tasks", project_root.path());

    // Expose the real in-process `kanban` operation tool as the module id
    // `kanban`, resolving its board at `<board_dir>/.kanban`. The handle is
    // kept alive for the whole test so the exposed module's registry and
    // context outlive `load()`.
    let kanban = support::expose_kanban_module(board_dir.path()).await;

    // The `kanban` `add task` op requires an initialized board. Create one on
    // the temp board root before the plugin loads, exactly as a user of the
    // kanban app would have an initialized board before a plugin runs.
    kanban
        .call(json!({ "op": "init board", "name": BOARD_NAME }))
        .await
        .expect("initializing the temp board should succeed");

    // A fresh host, with the project layer pointed at the temp plugin root.
    let host = PluginHost::for_tests(
        work_dir.path().to_path_buf(),
        Some(project_root.path().to_path_buf()),
    );

    // Expose the `kanban` operation tool to the host. No module is live until a
    // plugin activates it with `register("board", { rust: "kanban" })`.
    tokio::time::timeout(support::TIMEOUT, kanban.expose_to(&host))
        .await
        .expect("exposing the kanban module should not hang")
        .expect("exposing the kanban module should succeed");

    // Trigger discovery: the host scans the project layer, transpiles the
    // bundle's `index.ts`, creates a fresh isolate, and runs the exported
    // `load` — whose body adds two tasks through the `_meta` path form.
    let loaded = tokio::time::timeout(
        support::TIMEOUT,
        host.discover_and_load_all::<KanbanConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("discovering and loading the kanban-tasks plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one kanban-tasks example plugin should be discovered and loaded"
    );

    // Read the temp board back through the same real `kanban` tool. `list
    // tasks` is the `_meta` noun `tasks`, verb `list` — the canonical listing
    // op. The result is a `CallToolResult` shape; its single text content is
    // the JSON task list.
    let listed = kanban
        .call(json!({ "op": "list tasks" }))
        .await
        .expect("listing tasks on the temp board should succeed");
    let titles = support::task_titles(&listed);

    // The board holds exactly the two tasks the plugin added through the path
    // form. This can *only* be true if the SDK read the `kanban` tool's `_meta`
    // and built `op: "add task"` from each `task.add` path — a broken `_meta`
    // lookup would have raised `UnknownOperation` and failed `load()` before a
    // single task was created.
    assert_eq!(
        titles.len(),
        2,
        "the plugin must have added exactly two tasks, got {titles:?}"
    );
    assert!(
        titles.iter().any(|t| t == FIRST_TASK_TITLE),
        "the board must hold the first task '{FIRST_TASK_TITLE}', got {titles:?}"
    );
    assert!(
        titles.iter().any(|t| t == SECOND_TASK_TITLE),
        "the board must hold the second task '{SECOND_TASK_TITLE}', got {titles:?}"
    );
}
