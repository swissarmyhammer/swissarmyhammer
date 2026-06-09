//! Prove the review engine's observability surfaces under a **process-global**
//! `tracing` subscriber — the kind `sah serve` installs — not just a thread-local
//! scoped one.
//!
//! ## Why this is its own test binary
//!
//! Production `sah serve` installs its subscriber with
//! `tracing::subscriber::set_global_default` (see
//! `apps/swissarmyhammer-cli/src/logging.rs`: `registry().with(EnvFilter::new(
//! "rmcp=warn,{level}")).with(fmt::layer()...).init()`). A global default can be
//! installed **once per process**, so this test owns its whole binary: a single
//! `#[test]` installs the subscriber and no other test can race it.
//!
//! ## Why a global subscriber differs from the scoped one the old test used
//!
//! The earlier observability check (`tools_tests` →
//! `review_working_emits_observability_traces_through_spawn_blocking`) used
//! `tracing-test`'s `#[traced_test]`, which installs a **thread-local scoped**
//! dispatcher via `set_default` and asserts through that thread-local capture.
//! The review pipeline runs inside `spawn_blocking` on a fresh thread with its own
//! current-thread tokio runtime (`review_op::run_review_request`). A thread-local
//! scoped subscriber is only visible on the thread it was installed on, so making
//! the engine's lines visible across that thread boundary required the tool to
//! *carry the dispatcher across* (`get_default` → `set_default` on the blocking
//! thread). That carry is exactly what the scoped test exercised — and it could
//! pass even if the carry were the only thing making the lines visible.
//!
//! A **global** subscriber is registered process-wide and is visible from every
//! thread with no dispatcher dance at all. This test therefore reproduces the real
//! `sah serve` condition: it installs a global subscriber and asserts the engine's
//! `review scope resolved` / `fleet fan-out` / `review synthesis complete` lines
//! land in the buffer when the tool is driven on the real path. If they do not,
//! the bug the production `.sah/mcp.log` showed (engine lines absent) is
//! reproduced here.

use std::io::Write;
use std::sync::{Arc, Mutex};

use serde_json::json;
use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};
use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
use swissarmyhammer_tools::mcp::tools::review::ReviewTool;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

// Reuse the shared review fixture (temp git repo + planted diff + on-disk
// code_context index + scripted ACP agent + mock embedder). It is the same module
// the e2e binary uses; pulled in by path because integration support modules are
// not a library.
#[path = "integration/review_fixture.rs"]
mod review_fixture;

use review_fixture::{
    context_at, mock_embedder_factory, plant_diff, planted_agent, scripted_factory,
    seed_on_disk_index, TestRepo,
};

/// A `MakeWriter` over a shared in-memory buffer, so the test can read back
/// exactly what the global `fmt` layer wrote — the stand-in for `sah serve`'s
/// `.sah/mcp.log` file sink.
#[derive(Clone)]
struct BufferWriter(Arc<Mutex<Vec<u8>>>);

impl Write for BufferWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for BufferWriter {
    type Writer = BufferWriter;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Drive the real `review working` tool path under a global subscriber installed
/// the same way `logging.rs` installs it, and assert the engine's stage lines land
/// in the captured buffer.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_working_engine_traces_reach_a_global_subscriber() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    // --- install the GLOBAL subscriber exactly like `sah serve` does ---
    // registry().with(EnvFilter::new("rmcp=warn,debug")).with(fmt::layer()
    //   .with_writer(<buffer>).with_ansi(false)) via set_global_default.
    let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = BufferWriter(buffer.clone());
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::new("rmcp=warn,debug"))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_ansi(false),
        );
    tracing::subscriber::set_global_default(subscriber)
        .expect("install global subscriber (this test owns its binary, so this is the only one)");

    // --- the real tool path: registered production tool, planted repo + index +
    // scripted agent + mock embedder, `review working`, local single worker ---
    let repo = TestRepo::new();
    plant_diff(&repo);
    seed_on_disk_index(repo.path());
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let mut registry = ToolRegistry::new();
    registry.register(
        ReviewTool::new()
            .with_agent_factory(scripted_factory(planted_agent()))
            .with_embedder_factory(mock_embedder_factory()),
    );
    let tool = registry.get_tool("review").expect("review tool registered");
    let context = context_at(repo.path()).await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review working"));
    args.insert("backend".to_string(), json!("local"));
    tool.execute(args, &context)
        .await
        .expect("review working dispatch");

    let captured = String::from_utf8(buffer.lock().unwrap().clone()).expect("utf8 log buffer");

    // --- the engine's three stage lines must be present in the GLOBAL sink ---
    // scope: the selection summary names the matched validators.
    assert!(
        captured.contains("review scope resolved"),
        "scope stage line missing from the global subscriber buffer.\n\
         --- captured log ---\n{captured}\n--- end ---"
    );
    // fleet: the fan-out batching line, carrying the applied rules.
    assert!(
        captured.contains("fleet fan-out"),
        "fleet fan-out stage line missing from the global subscriber buffer.\n\
         --- captured log ---\n{captured}\n--- end ---"
    );
    assert!(
        captured.contains("rules="),
        "fleet fan-out line must carry the applied `rules=[...]`.\n\
         --- captured log ---\n{captured}\n--- end ---"
    );
    // synthesize: the final-counts line.
    assert!(
        captured.contains("review synthesis complete"),
        "synthesis stage line missing from the global subscriber buffer.\n\
         --- captured log ---\n{captured}\n--- end ---"
    );
}
