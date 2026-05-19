//! End-to-end integration test for the committed `cli-echo` example plugin —
//! the demonstration of the `{ cli }` stdio-subprocess transport and the
//! `unload()` lifecycle hook.
//!
//! Where `kanban_tasks_e2e.rs` and `file_notes_e2e.rs` drive in-process
//! `{ rust }` servers, this test proves the same committed-bundle round-trip
//! works for the **only non-`rust` `ServerSource`** — a real MCP server spawned
//! as a child process and addressed over its stdio. It is also the only example
//! test that exercises teardown: it proves the plugin's `unload()` hook body
//! actually ran, not merely that the host disposed the plugin's registrations.
//!
//! # The fixture subprocess
//!
//! The `cli-echo` plugin registers `echo` as a `{ cli: [<command>] }` source.
//! The committed `entry.ts` carries a named placeholder token where the command
//! belongs — a committed example cannot hard-code an absolute binary path. This
//! test stages the bundle with [`support::stage_example_with`], which rewrites
//! the placeholder in the throwaway staged copy with the real path of the
//! crate's `cli_server_fixture` binary: a genuine `rmcp` stdio MCP server
//! exposing a flat `echo` tool. Cargo sets `CARGO_BIN_EXE_cli_server_fixture`
//! for every integration test, so the path always points at the freshly built
//! fixture.
//!
//! # Proving the `unload()` hook itself ran
//!
//! [`PluginHost::unload`] runs the plugin's optional `unload()` hook *and then*
//! unconditionally disposes every registration the plugin made. The disposal is
//! the authoritative cleanup: it produces the post-unload `ServerUnavailable`
//! tombstone for a registered server *whether or not the plugin's own
//! `unload()` did anything* — so observing the `echo` server is gone proves the
//! host's disposal ran, not that the plugin's hook body did.
//!
//! To prove the hook itself ran, the `cli-echo` plugin's `unload()` performs an
//! effect host-side disposal can never produce: it adds a sentinel task to a
//! kanban board through a server that is still live at `unload()` time. The
//! test seeds an empty board, asserts the sentinel is **absent** while the
//! plugin is loaded, and asserts it is **present** after `host.unload`. Only the
//! plugin's `unload()` body running can put it there; deleting the bundle's
//! `unload` export, or throwing inside `unload()` before the sentinel write,
//! fails this test.
//!
//! # What a passing run proves
//!
//! 1. **The stdio round-trip.** The plugin's `load()` calls `echo` on the `cli`
//!    server with a known payload and throws unless the echoed text comes back
//!    verbatim. So a successful `discover_and_load_all` already proves a
//!    `tools/call` crossed the subprocess's stdio in both directions — a broken
//!    transport fails `load()` and discovery returns an error. The test then
//!    *independently* re-confirms the round-trip by calling the same `echo`
//!    server through [`PluginHost::call`] while the plugin is still loaded.
//! 2. **The `unload()` hook ran.** After `host.unload`, the sentinel task the
//!    plugin's `unload()` adds appears on the board — an effect host-side
//!    disposal alone cannot produce.
//! 3. **The host disposed the registration.** After `host.unload`, routing a
//!    call to the `echo` server through [`PluginHost::call`] fails with
//!    [`Error::ServerUnavailable`] — the registry's tombstone for a name that
//!    *was* registered and has since been disposed.
//!
//! # Isolation
//!
//! The test owns its own [`tempfile::TempDir`] roots and a fresh
//! [`PluginHost`]; nothing is `static` and no temp dir is reused. Every
//! cross-thread interaction is bounded by [`support::TIMEOUT`] so a wedged
//! isolate or subprocess fails the test fast instead of hanging CI.

mod support;

use serde_json::json;
use swissarmyhammer_directory::SwissarmyhammerConfig;
use swissarmyhammer_plugin::{CallerId, Error, PluginHost};

/// The placeholder token the committed `cli-echo` `entry.ts` carries where the
/// CLI command belongs. [`support::stage_example_with`] rewrites it in the
/// staged copy with the real fixture binary path; it must match the token
/// spelled in the committed bundle exactly.
const CLI_COMMAND_TOKEN: &str = "__CLI_ECHO_COMMAND__";

/// The server name the `cli-echo` plugin registers its `{ cli }` source under.
/// It must match the name in the bundle's `plugin.json` `provides` and the
/// `register` call in its `entry.ts` — the test routes calls to this name.
const ECHO_SERVER: &str = "echo";

/// The flat tool the fixture stdio MCP server exposes. The test invokes it
/// directly through [`PluginHost::call`] to confirm the server is live before
/// unload and gone after.
const ECHO_TOOL: &str = "echo";

/// A message the test sends through the live `echo` server to confirm the
/// stdio round-trip independently of the one the plugin's `load()` performs.
const TEST_PAYLOAD: &str = "payload routed by the test over the cli-echo subprocess";

/// The board name the test initializes the temp `.kanban` board with before
/// the plugin loads — the `kanban` `add task` op the plugin's `unload()` runs
/// requires an initialized board.
const BOARD_NAME: &str = "cli-echo unload-sentinel board";

/// The title of the sentinel task the `cli-echo` plugin's `unload()` adds to
/// the board. It must match `UNLOAD_SENTINEL_TITLE` in the bundle's `entry.ts`
/// — the test asserts the board carries exactly this task after unload, and
/// none before.
const UNLOAD_SENTINEL_TITLE: &str = "cli-echo unload() ran";

/// The committed `cli-echo` example plugin, discovered from disk, round-trips a
/// `tools/call` over a spawned stdio MCP subprocess, and its `unload()` hook
/// runs an observable teardown effect.
///
/// This single test stitches the CLI-transport and unload-lifecycle story
/// together end to end against a committed bundle:
///
/// - the committed `cli-echo` bundle is staged into the project layer with
///   [`support::stage_example_with`], which rewrites the bundle's placeholder
///   CLI-command token with the real `cli_server_fixture` binary path in the
///   throwaway staged copy — the committed source stays a clean example;
/// - the real in-process `kanban` operation tool is exposed to the host over a
///   temp board; the plugin's `unload()` writes a sentinel task there;
/// - discovery transpiles the bundle's `entry.ts`, creates a fresh V8 isolate,
///   and runs the exported `load`, which registers `echo` as a `{ cli }` source
///   (the host spawns the fixture subprocess) and calls its `echo` tool;
/// - while the plugin is loaded, the test routes its own `echo` call through
///   [`PluginHost::call`] and asserts the payload round-trips over stdio, and
///   asserts the board does *not* yet carry the unload sentinel;
/// - `host.unload` runs the plugin's `unload()` hook and disposes its
///   registrations; the test asserts the `echo` server is no longer routable
///   *and* that the unload sentinel task is now on the board — the effect only
///   the plugin's `unload()` body could produce.
#[tokio::test]
async fn cli_echo_plugin_round_trips_over_stdio_and_unloads() {
    // Per-test isolation: every root is this test's own `TempDir`.
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let project_root = tempfile::TempDir::new().expect("project plugin root temp");
    let board_dir = tempfile::TempDir::new().expect("kanban board temp");

    // Cargo sets `CARGO_BIN_EXE_<name>` for every binary target when it builds
    // the crate's integration tests, so this always points at the freshly
    // built fixture stdio MCP server.
    let fixture_binary = env!("CARGO_BIN_EXE_cli_server_fixture");

    // Stage the committed `cli-echo` bundle into the project layer's `plugins/`
    // directory, rewriting the bundle's placeholder CLI-command token with the
    // real fixture path in the throwaway staged copy. The committed bundle is
    // never touched — only the temp copy is specialized.
    support::stage_example_with(
        "cli-echo",
        project_root.path(),
        &[(CLI_COMMAND_TOKEN, fixture_binary)],
    );

    // Expose the real in-process `kanban` operation tool, resolving its board
    // at `<board_dir>/.kanban`. The plugin registers it as `board`; its
    // `unload()` writes a sentinel task here. The handle is kept alive for the
    // whole test so the exposed module outlives both `load()` and `unload()`.
    let kanban = support::expose_kanban_module(board_dir.path()).await;

    // The `kanban` `add task` op the plugin's `unload()` runs requires an
    // initialized board. Create one before the plugin loads.
    kanban
        .call(json!({ "op": "init board", "name": BOARD_NAME }))
        .await
        .expect("initializing the temp board should succeed");

    // A fresh host, with the project layer pointed at the temp plugin root.
    let host = PluginHost::for_tests(
        work_dir.path().to_path_buf(),
        Some(project_root.path().to_path_buf()),
    );

    // Expose the `kanban` operation tool to the host. No module is live until
    // the plugin activates it with `register("board", { rust: "kanban" })`.
    tokio::time::timeout(support::TIMEOUT, kanban.expose_to(&host))
        .await
        .expect("exposing the kanban module should not hang")
        .expect("exposing the kanban module should succeed");

    // Trigger discovery: the host scans the project layer, transpiles the
    // bundle's `entry.ts`, creates a fresh isolate, and runs the exported
    // `load` — whose body spawns the fixture subprocess and drives an `echo`
    // call over its stdio. A broken transport fails `load()` here.
    let loaded = tokio::time::timeout(
        support::TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("discovering and loading the cli-echo plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one cli-echo example plugin should be discovered and loaded"
    );
    let plugin_id = loaded.into_iter().next().expect("the one loaded plugin id");

    // Assertion 1 — the `echo` server the plugin registered is live and routes
    // a real `tools/call` over the subprocess's stdio. The plugin's `load()`
    // already proved this once; routing the test's own call through
    // `PluginHost::call` re-confirms it independently against the same server.
    let echoed = tokio::time::timeout(
        support::TIMEOUT,
        host.call(
            CallerId::HostInternal,
            ECHO_SERVER,
            ECHO_TOOL,
            json!({ "message": TEST_PAYLOAD }),
        ),
    )
    .await
    .expect("an echo call over the cli subprocess should not hang")
    .expect("the cli-echo plugin's echo server should be live and routable");
    let rendered = serde_json::to_string(&echoed).expect("an echo result is serializable");
    assert!(
        rendered.contains(TEST_PAYLOAD),
        "the echoed payload must round-trip back over stdio, got {rendered}"
    );

    // Assertion 2 — the unload sentinel is *not* on the board yet. The plugin's
    // `unload()` has not run; only `load()` has. This pins down the baseline so
    // the post-unload assertion proves the sentinel was added by `unload()`
    // rather than having been there all along.
    let before_unload = kanban
        .call(json!({ "op": "list tasks" }))
        .await
        .expect("listing tasks on the temp board should succeed");
    assert!(
        !support::task_titles(&before_unload)
            .iter()
            .any(|title| title == UNLOAD_SENTINEL_TITLE),
        "the unload sentinel task must NOT be on the board before unload — \
         the plugin's unload() has not run yet, got {:?}",
        support::task_titles(&before_unload)
    );

    // Unload the plugin. The host runs the plugin's `unload()` hook — which
    // adds the sentinel task to the still-live `board` server, then calls
    // `this.unregister("echo")` and `super.unload()` — and then authoritatively
    // disposes every registration the plugin made.
    tokio::time::timeout(support::TIMEOUT, host.unload(&plugin_id))
        .await
        .expect("unloading the cli-echo plugin should not hang")
        .expect("unloading the cli-echo plugin should succeed");

    // Assertion 3 — the plugin's `unload()` hook body ran. The sentinel task is
    // now on the board. Adding a kanban task is an effect the host's automatic
    // registration disposal can never produce — it only unregisters servers and
    // disposes callbacks. So the sentinel's presence proves the plugin's own
    // `unload()` body executed: deleting the bundle's `unload` export, or
    // throwing inside `unload()` before the sentinel write, leaves it absent.
    let after_unload = kanban
        .call(json!({ "op": "list tasks" }))
        .await
        .expect("listing tasks on the temp board should succeed");
    let titles = support::task_titles(&after_unload);
    assert_eq!(
        titles.len(),
        1,
        "the board must hold exactly the one task the plugin's unload() added, got {titles:?}"
    );
    assert_eq!(
        titles[0], UNLOAD_SENTINEL_TITLE,
        "the plugin's unload() hook must have added the sentinel task, got {titles:?}"
    );

    // Assertion 4 — the `echo` server is gone from the live registry. Routing a
    // call to it now fails with `ServerUnavailable`: the registry's tombstone
    // for a name that *was* registered and has since been disposed. This is the
    // host's authoritative registration disposal — the cleanup that runs
    // regardless of what the plugin's `unload()` did — completing the teardown.
    let server_err = tokio::time::timeout(
        support::TIMEOUT,
        host.call(
            CallerId::HostInternal,
            ECHO_SERVER,
            ECHO_TOOL,
            json!({ "message": TEST_PAYLOAD }),
        ),
    )
    .await
    .expect("a routed call after unload should not hang")
    .expect_err("the echo server must be gone from the live registry after unload");
    assert!(
        matches!(server_err, Error::ServerUnavailable),
        "a call to the disposed echo server should fail with ServerUnavailable, got {server_err:?}"
    );
}
