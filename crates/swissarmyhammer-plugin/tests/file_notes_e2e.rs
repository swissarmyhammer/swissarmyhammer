//! End-to-end integration test for the committed `file-notes` example plugin —
//! the filesystem-effect demonstration of the in-process `files` MCP tool.
//!
//! Where `files_dispatch_e2e.rs` drives the `files` tool with **absolute**
//! paths interpolated into a test-generated bundle, this test proves the same
//! `write file` → `read file` → `write file` round-trip works for a **real,
//! committed bundle** discovered from disk — a bundle that, being committed,
//! cannot hard-code an absolute temp path and so addresses the `files` tool
//! with **relative** paths.
//!
//! # The relative-path / session-working-dir contract
//!
//! The `files` tool resolves a relative path against the **session working
//! directory** before it touches the disk — never the process CWD. Every file
//! operation resolves through `ToolContext::session_root`, which returns the
//! server's configured `working_dir` (the board dir the server was constructed
//! with); the process current directory is only a last-resort fallback for a
//! stand-alone caller that never set one. This is the project's
//! `session-cwd-for-tools` design: the bundled GUI app launches with CWD `/`
//! (a read-only root) and a single process hosts many boards, so a per-session
//! root cannot be the process CWD. A committed example must therefore use
//! relative paths and let the host's `working_dir` decide where they land.
//!
//! This test honors that contract by giving the server its own fresh
//! [`tempfile::TempDir`] `work_dir`: the example's relative paths resolve under
//! it, so the notes land there and the real source tree is never written to.
//! No process-CWD pinning is needed — the resolution root is per-server state,
//! not global process state.
//!
//! # What a passing run proves
//!
//! The committed `file-notes` bundle's `load()` registers the host-exposed
//! `files` tool as `fs` and, against relative paths, writes a note, reads it
//! back, and writes the read-back content into a second note. Asserting both
//! note files exist **under the server's `work_dir`** with the expected
//! contents proves the whole round trip — and that the real source tree was
//! never touched.
//!
//! # Isolation
//!
//! The test owns its own [`tempfile::TempDir`] roots and a fresh
//! [`PluginHost`]; nothing is `static` and no temp dir is reused. Every
//! cross-thread interaction is bounded by [`support::TIMEOUT`] so a wedged
//! isolate fails the test fast instead of hanging CI.

mod support;

use swissarmyhammer_directory::SwissarmyhammerConfig;
use swissarmyhammer_plugin::PluginHost;

/// The bundle-relative path of the first note the plugin writes. It must match
/// the path hard-coded in the example bundle's `index.ts` — the test asserts
/// the file lands here, under the server's `work_dir`, with the expected
/// contents.
const HELLO_NOTE: &str = "notes/hello.txt";

/// The bundle-relative path of the second note — the echo of the first, proof
/// the `read file` return value crossed back into the isolate. It must match
/// the path the example bundle's `index.ts` writes.
const ECHO_NOTE: &str = "notes/echo.txt";

/// The exact text the `file-notes` plugin writes into the first note and then
/// reads back into the second. It must match the constant in the bundle's
/// `index.ts`; the test asserts it verbatim in **both** note files.
const NOTE_BODY: &str = "a note round-tripped through the in-process files tool";

/// The committed `file-notes` example plugin, discovered from disk, drives the
/// real `files` operation tool against relative paths and lands two real note
/// files under the server's `work_dir` (its session working directory).
///
/// This single test stitches the filesystem-effect round-trip together end to
/// end against a committed bundle:
///
/// - the real `files` operation tool is built by [`support::build_mcp_server`]
///   against a fresh temp `work_dir` and exposed to the host with
///   `expose_tools_to_plugin_host`; no mock. The example's relative paths
///   resolve against that `work_dir` (the session root), so the notes land
///   there and the real source tree is never written to;
/// - the committed `file-notes` bundle is staged into the project layer with
///   [`support::stage_example`] and discovered through `discover_and_load_all`,
///   which transpiles its `index.ts`, creates a fresh V8 isolate, and runs the
///   exported `load`;
/// - inside the isolate the SDK turns each `this.fs.files({ op, … })` into a
///   real `tools/call` routed by the host dispatcher into the live `files`
///   handler.
///
/// Both assertions observe the filesystem under the server's `work_dir` — the
/// only honest verification for an in-process tool whose state *is* the
/// filesystem.
#[tokio::test]
async fn file_notes_plugin_round_trips_through_files_tool() {
    // Per-test isolation: every root is this test's own `TempDir`. The server's
    // `work_dir` doubles as the session root the example's relative note paths
    // resolve against, so the notes land under it and the real source tree is
    // never touched — no process-CWD pinning needed.
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let project_root = tempfile::TempDir::new().expect("project plugin root temp");

    // Stage the committed `file-notes` bundle into the project layer's
    // `plugins/` directory, where discovery will find it. The bundle is real,
    // committed source — no inline plugin is written here.
    support::stage_example("file-notes", project_root.path());

    // The real in-process tool set, including the unified `files` tool.
    let server = support::build_mcp_server(work_dir.path()).await;

    // A fresh host, with the project layer pointed at the temp plugin root.
    let host = PluginHost::for_tests(
        work_dir.path().to_path_buf(),
        Some(project_root.path().to_path_buf()),
    );

    // Expose every in-process tool — `files` among them — as an addressable
    // Rust module. No module is live until a plugin activates it with
    // `register("fs", { rust: "files" })`.
    tokio::time::timeout(support::TIMEOUT, server.expose_tools_to_plugin_host(&host))
        .await
        .expect("exposing the in-process tools should not hang")
        .expect("exposing the in-process tools should succeed");

    // Trigger discovery: the host scans the project layer, transpiles the
    // bundle's `index.ts`, creates a fresh isolate, and runs the exported
    // `load` — whose body performs the three real `files` calls.
    let loaded = tokio::time::timeout(
        support::TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("discovering and loading the file-notes plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one file-notes example plugin should be discovered and loaded"
    );

    // Assertion 1 — the first note exists under the server's `work_dir` with
    // the written body. This can only be true if a `tools/call` carrying
    // `op: "write file"` and a *relative* path reached the real `files`
    // handler, which resolved it against the session root (the server's
    // `work_dir`).
    let hello_path = work_dir.path().join(HELLO_NOTE);
    let hello_content = std::fs::read_to_string(&hello_path).unwrap_or_else(|error| {
        panic!(
            "the first note must exist at {} — a relative-path write through the \
             real files handler did not land it: {error}",
            hello_path.display()
        )
    });
    assert_eq!(
        hello_content, NOTE_BODY,
        "the first note must hold exactly the body the plugin wrote"
    );

    // Assertion 2 — the echo note exists holding the read-back content. This
    // can only be true if the `read file` return value crossed back through
    // the dispatcher into the isolate and was usable by plugin code: the
    // plugin wrote the *read-back* string, not a constant.
    let echo_path = work_dir.path().join(ECHO_NOTE);
    let echo_content = std::fs::read_to_string(&echo_path).unwrap_or_else(|error| {
        panic!(
            "the echo note must exist at {} — the read-file return value did not \
             cross back into the isolate: {error}",
            echo_path.display()
        )
    });
    assert_eq!(
        echo_content, NOTE_BODY,
        "the echo note must hold the content the plugin read back from the \
         first note — proving the dispatcher's return value round-tripped"
    );
}
