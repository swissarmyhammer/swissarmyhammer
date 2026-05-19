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
//! # The relative-path / process-CWD contract
//!
//! The `files` tool resolves a relative path against the **process** current
//! working directory before it touches the disk. Each operation resolves at
//! its own site: `write file` joins the relative path onto
//! `std::env::current_dir()` (`files/write/mod.rs`), and `read file` resolves
//! through `FilePathValidator::validate_path` (`files/read/mod.rs`). A
//! committed example must therefore use relative paths, and any test that
//! drives it must pin the process CWD somewhere safe — otherwise the example
//! would write into the real source tree.
//!
//! This test honors that contract two ways, exactly as the project's
//! test-isolation guidance requires for any process-CWD test:
//!
//! - it pins the process CWD to a fresh [`tempfile::TempDir`] for the whole
//!   test with [`CurrentDirGuard`], which restores the original CWD on drop
//!   (even on panic);
//! - it is annotated `#[serial_test::serial]` so it never races another
//!   CWD-touching test — process CWD is global mutable state.
//!
//! # What a passing run proves
//!
//! The committed `file-notes` bundle's `load()` registers the host-exposed
//! `files` tool as `fs` and, against relative paths, writes a note, reads it
//! back, and writes the read-back content into a second note. Asserting both
//! note files exist **under the temp CWD** with the expected contents proves
//! the whole round trip — and that the real source tree was never touched.
//!
//! # Isolation
//!
//! The test owns its own [`tempfile::TempDir`] roots and a fresh
//! [`PluginHost`]; nothing is `static` and no temp dir is reused. Every
//! cross-thread interaction is bounded by [`support::TIMEOUT`] so a wedged
//! isolate fails the test fast instead of hanging CI.

mod support;

use swissarmyhammer_common::test_utils::CurrentDirGuard;
use swissarmyhammer_directory::SwissarmyhammerConfig;
use swissarmyhammer_plugin::PluginHost;

/// The bundle-relative path of the first note the plugin writes. It must match
/// the path hard-coded in the example bundle's `entry.ts` — the test asserts
/// the file lands here, under the temp CWD, with the expected contents.
const HELLO_NOTE: &str = "notes/hello.txt";

/// The bundle-relative path of the second note — the echo of the first, proof
/// the `read file` return value crossed back into the isolate. It must match
/// the path the example bundle's `entry.ts` writes.
const ECHO_NOTE: &str = "notes/echo.txt";

/// The exact text the `file-notes` plugin writes into the first note and then
/// reads back into the second. It must match the constant in the bundle's
/// `entry.ts`; the test asserts it verbatim in **both** note files.
const NOTE_BODY: &str = "a note round-tripped through the in-process files tool";

/// The committed `file-notes` example plugin, discovered from disk, drives the
/// real `files` operation tool against relative paths and lands two real note
/// files under a temp working directory.
///
/// This single test stitches the filesystem-effect round-trip together end to
/// end against a committed bundle:
///
/// - the process CWD is pinned to a fresh temp dir with [`CurrentDirGuard`], so
///   the relative paths the example uses resolve there and the real source
///   tree is never written to;
/// - the real `files` operation tool is built by [`support::build_mcp_server`]
///   and exposed to the host with `expose_tools_to_plugin_host`; no mock;
/// - the committed `file-notes` bundle is staged into the project layer with
///   [`support::stage_example`] and discovered through `discover_and_load_all`,
///   which transpiles its `entry.ts`, creates a fresh V8 isolate, and runs the
///   exported `load`;
/// - inside the isolate the SDK turns each `this.fs.files({ op, … })` into a
///   real `tools/call` routed by the host dispatcher into the live `files`
///   handler.
///
/// Both assertions observe the filesystem under the temp CWD — the only honest
/// verification for an in-process tool whose state *is* the filesystem.
#[tokio::test]
#[serial_test::serial]
async fn file_notes_plugin_round_trips_through_files_tool() {
    // Per-test isolation: every root is this test's own `TempDir`.
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let project_root = tempfile::TempDir::new().expect("project plugin root temp");
    let cwd_dir = tempfile::TempDir::new().expect("process cwd temp");

    // Pin the process CWD to the temp dir for the whole test. The `files` tool
    // resolves the example's relative paths against this directory, so both
    // note files land under `cwd_dir`. The guard restores the original CWD on
    // drop, even on panic; `#[serial]` keeps it from racing other CWD tests.
    let _cwd_guard = CurrentDirGuard::new(cwd_dir.path())
        .expect("pinning the process CWD to the temp dir should succeed");

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
    // bundle's `entry.ts`, creates a fresh isolate, and runs the exported
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

    // Assertion 1 — the first note exists under the temp CWD with the written
    // body. This can only be true if a `tools/call` carrying `op: "write file"`
    // and a *relative* path reached the real `files` handler, which resolved
    // it against the pinned process CWD.
    let hello_path = cwd_dir.path().join(HELLO_NOTE);
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
    let echo_path = cwd_dir.path().join(ECHO_NOTE);
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
