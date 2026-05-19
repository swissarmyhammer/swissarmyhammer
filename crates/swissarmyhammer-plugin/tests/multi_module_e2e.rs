//! End-to-end integration test for the committed `multi-module` example
//! plugin — the demonstration that the sandboxed module loader resolves
//! **relative sibling-module imports** inside a real, committed plugin bundle.
//!
//! Where `kanban_tasks_e2e.rs` and `file_notes_e2e.rs` drive single-`index.ts`
//! bundles, this test proves a bundle can be split across files: the
//! `multi-module` example ships an `index.ts` plus a sibling `board-helpers.ts`
//! module, and `index.ts` imports that sibling with a **relative specifier**
//! (`./board-helpers.ts`).
//!
//! # Why a kanban board is the observable effect
//!
//! `module_loader.rs` already covers relative-import resolution against
//! *hand-written temp bundles*. This test instead exercises the **committed**
//! multi-file bundle through the real plugin platform, and proves the import
//! resolved by observing a side effect only the imported helper can produce:
//! the helper module — not `index.ts` — adds a tagged task to a `.kanban`
//! board. If the relative import had failed to resolve, the V8 isolate would
//! never have linked `board-helpers.ts`, `load()` would have thrown at module
//! resolution, and the task would never reach the board.
//!
//! # What a passing run proves
//!
//! The committed `multi-module` bundle's `index.ts` imports two helpers from
//! `./board-helpers.ts`: a pure function that normalizes a task title, and an
//! `async` function that adds a tagged task through a server dispatcher. Its
//! `load()` registers the host-exposed `kanban` tool as `board` and calls the
//! async helper. Reading the temp board back and finding the helper-produced
//! task — under the helper's normalized title — proves the whole chain: the
//! relative import resolved, the sibling module linked, and its exported code
//! ran inside the isolate.
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
const BOARD_NAME: &str = "multi-module example board";

/// The task title the imported `board-helpers.ts` helper produces.
///
/// The bundle's `index.ts` passes a raw, untrimmed title to the helper's
/// `normalizeTaskTitle`; the helper trims it and collapses internal runs of
/// whitespace. This constant is the **normalized** result the test asserts on —
/// it must match the title the helper module yields, the proof the imported
/// helper's code actually ran.
const NORMALIZED_TASK_TITLE: &str = "Ship the multi-module example";

/// The committed `multi-module` example plugin, discovered from disk, proves
/// the sandboxed loader resolves a **relative sibling-module import** inside a
/// real bundle.
///
/// The bundle is two files: `index.ts` imports `./board-helpers.ts` with a
/// relative specifier. The end-to-end chain this single test stitches together:
///
/// - the committed bundle is staged into the project layer with
///   [`support::stage_example`] — which copies **every** file in the bundle
///   directory, `board-helpers.ts` included — and discovered through
///   `discover_and_load_all`, which transpiles `index.ts`, creates a fresh V8
///   isolate, resolves and links the relative import, and runs `load()`;
/// - the real `kanban` operation tool is built by
///   [`support::expose_kanban_module`] over a temp board root and exposed to
///   the host, so the imported helper has a real server to drive;
/// - inside `load()`, `index.ts` calls the helper exported from
///   `./board-helpers.ts`, which adds one tagged task to the board.
///
/// The assertion observes the board on disk. The helper-produced task can be
/// there **only if** the relative import resolved and the sibling module ran —
/// a failed import would have thrown at module resolution before `load()`
/// reached any board call.
#[tokio::test]
async fn multi_module_plugin_loads_sibling_module() {
    // Per-test isolation: every root is this test's own `TempDir`.
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let project_root = tempfile::TempDir::new().expect("project plugin root temp");
    let board_dir = tempfile::TempDir::new().expect("kanban board temp");

    // Stage the committed `multi-module` bundle into the project layer's
    // `plugins/` directory, where discovery will find it. `stage_example`
    // copies the whole bundle directory recursively, so the sibling
    // `board-helpers.ts` module is staged alongside `index.ts` — without it
    // the relative import could not resolve.
    support::stage_example("multi-module", project_root.path());

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
    // bundle's `index.ts`, creates a fresh isolate, resolves the relative
    // `./board-helpers.ts` import, links the sibling module, and runs the
    // exported `load` — whose body calls the imported helper to add a task.
    let loaded = tokio::time::timeout(
        support::TIMEOUT,
        host.discover_and_load_all::<KanbanConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("discovering and loading the multi-module plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one multi-module example plugin should be discovered and loaded"
    );

    // Read the temp board back through the same real `kanban` tool.
    let listed = kanban
        .call(json!({ "op": "list tasks" }))
        .await
        .expect("listing tasks on the temp board should succeed");
    let titles = support::task_titles(&listed);

    // The board holds exactly the one task the imported helper module added,
    // under the title the helper's `normalizeTaskTitle` produced. This can
    // *only* be true if the relative `./board-helpers.ts` import resolved, the
    // sibling module linked into the isolate, and its exported helper ran — a
    // failed import would have thrown at module resolution and failed `load()`
    // before a single task was created.
    assert_eq!(
        titles,
        vec![NORMALIZED_TASK_TITLE.to_string()],
        "the imported helper module must have added exactly the one normalized task"
    );
}
