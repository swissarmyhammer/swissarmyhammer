//! End-to-end MCP test for `notifications/progress` from `rebuild index`.
//!
//! This test boots the real in-process HTTP MCP server, connects an rmcp
//! client that overrides `ClientHandler::on_progress` to capture every
//! `notifications/progress` message, and issues a `tools/call` for
//! `code_context` `op: "rebuild index"` carrying a `progressToken` in
//! the JSON-RPC `_meta` field.
//!
//! It asserts:
//!
//! 1. At least one `notifications/progress` arrives (the indexer emits
//!    at least a terminal `Done` event).
//! 2. Every captured notification echoes the same `progressToken` the
//!    client supplied.
//! 3. The final notification reports `progress == total` (the
//!    `IndexProgress::Done` → terminal-tick contract that lets
//!    progress-bar UIs close cleanly).
//!
//! The test is the live wiring check: it exercises
//! `McpServer::call_tool` extracting the token from `request.meta`,
//! `execute_rebuild_index` building an `McpProgressReporter`, the
//! drain task forwarding events to the peer, and the JSON-RPC
//! transport delivering them to the client.

use rmcp::model::{
    CallToolRequestParams, ClientCapabilities, ClientInfo, Implementation, NumberOrString,
    ProgressNotificationParam, ProgressToken, RequestParamsMeta,
};
use rmcp::service::NotificationContext;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use std::sync::{Arc, Mutex};
use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server_with_options, McpServerMode};
use tempfile::TempDir;

/// Client handler that captures every `notifications/progress` it sees.
///
/// The captured params are pushed into a shared `Vec` so the test can
/// inspect them after the tool call returns. `on_progress` is the only
/// hook we override; `get_info` returns our `ClientInfo` so the
/// initialize handshake negotiates correctly. Every other client-side
/// request/notification falls back to the default rmcp behavior.
#[derive(Clone)]
struct CapturingClient {
    info: ClientInfo,
    captured: Arc<Mutex<Vec<ProgressNotificationParam>>>,
}

impl CapturingClient {
    fn new() -> Self {
        Self {
            info: ClientInfo::new(
                ClientCapabilities::default(),
                Implementation::new("progress-capturing-client", "1.0.0"),
            ),
            captured: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn snapshot(&self) -> Vec<ProgressNotificationParam> {
        self.captured.lock().unwrap().clone()
    }
}

impl ClientHandler for CapturingClient {
    fn get_info(&self) -> ClientInfo {
        self.info.clone()
    }

    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        self.captured.lock().unwrap().push(params);
    }
}

/// Build a minimal Rust workspace so `rebuild index` has real files to
/// chunk and embed. The two source files exist purely to give the
/// tree-sitter pass non-empty work — they aren't read by assertions.
fn create_test_project() -> TempDir {
    let tmp = TempDir::new().expect("create temp dir");
    let root = tmp.path();

    std::fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "progress-test-project"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();

    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/main.rs"),
        r#"fn main() {
    let _ = compute(7);
}

fn compute(x: i32) -> i32 {
    x * 2 + 1
}
"#,
    )
    .unwrap();

    std::fs::write(
        root.join("src/lib.rs"),
        r#"pub struct Config {
    pub value: i32,
}

impl Config {
    pub fn new(value: i32) -> Self {
        Self { value }
    }
}
"#,
    )
    .unwrap();

    tmp
}

/// Build a string-typed progress token to thread through `_meta`.
fn make_token(s: &str) -> ProgressToken {
    ProgressToken(NumberOrString::String(s.into()))
}

#[tokio::test]
async fn rebuild_index_emits_progress_notifications_when_token_supplied() {
    // This test asserts the progress-notification wiring, not semantic
    // embeddings. Skip the multi-GB embedding-model load (which on a clean
    // machine downloads from HuggingFace and dominates the run) so the test
    // exercises the chunk/progress path quickly and hermetically. nextest runs
    // each test in its own process, so this env var does not leak into others.
    std::env::set_var("SAH_DISABLE_EMBEDDING", "1");

    // 1. Stand up an in-process HTTP MCP server scoped to a tempdir so it
    //    doesn't walk the host repo. The full tool union is registered, so
    //    `code_context` is available.
    let project = create_test_project();
    let mut server = start_mcp_server_with_options(
        McpServerMode::Http { port: None },
        None,
        None,
        Some(project.path().to_path_buf()),
    )
    .await
    .expect("start MCP server");

    // 2. Connect a custom client that captures every progress notification.
    let handler = CapturingClient::new();
    let transport = StreamableHttpClientTransport::with_client(reqwest::Client::default(), {
        let mut config = StreamableHttpClientTransportConfig::default();
        config.uri = server.url().into();
        config
    });
    // The handler itself implements `ClientHandler::get_info` returning
    // our `ClientInfo`, so the initialize handshake negotiates with our
    // capabilities while our `on_progress` override still runs for every
    // incoming `notifications/progress`.
    let client = handler
        .clone()
        .serve(transport)
        .await
        .expect("connect client");

    // 3. Prime the workspace so `indexed_files` is populated. `get
    //    status` triggers startup_cleanup which discovers source files
    //    and inserts them with `ts_indexed = 0`. Without this the
    //    indexer's dirty set would be empty and no chunking events would
    //    fire.
    {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));
        client
            .call_tool(CallToolRequestParams::new("code_context").with_arguments(args))
            .await
            .expect("priming get status");
    }

    // 4. Call `rebuild index` with a progress token in `_meta`. The
    //    server must extract the token and build an `McpProgressReporter`
    //    that ships events back as `notifications/progress`.
    //
    //    rmcp's `progress_token_provider` may rewrite the client-supplied
    //    token to its own generated value (that is rmcp's contract for
    //    multiplexing one client across many outstanding tool calls), so
    //    we call `set_progress_token` here purely to exercise that API
    //    path and prove we're not accidentally depending on rmcp not
    //    auto-generating one. The actual wire token is whatever rmcp
    //    settles on; the test asserts only that every notification
    //    carries the *same* token, not that it equals
    //    `rebuild-progress-test-token`.
    let mut params = CallToolRequestParams::new("code_context").with_arguments({
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("rebuild index"));
        args.insert("layer".to_string(), serde_json::json!("treesitter"));
        args
    });
    params.set_progress_token(make_token("rebuild-progress-test-token"));
    let result = client.call_tool(params).await.expect("rebuild index call");
    assert_eq!(
        result.is_error,
        Some(false),
        "rebuild index returned an error: {:?}",
        result
    );

    // Give the drain task one final yield to flush any tail buffered
    // notification through the transport before we snapshot. The server
    // side already `.await`s the drain handle inside
    // `execute_rebuild_index`, but the client receive loop runs on its
    // own task so a tiny yield here removes a theoretical race.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let captured = handler.snapshot();

    // 5. Shut everything down cleanly before assertions so test
    //    failures don't leave a hot server task behind.
    client.cancel().await.ok();
    server.shutdown().await.ok();

    // 6. Assertions.
    assert!(
        !captured.is_empty(),
        "expected at least one notifications/progress message, got zero. \
         The server should have extracted progressToken from _meta and \
         forwarded IndexProgress events through McpProgressReporter."
    );

    // Every notification must echo *some* progress token (rmcp auto-
    // generates one per call from its `progress_token_provider`, which
    // overrides any token the caller manually set via
    // `params.set_progress_token`). All notifications from one tool call
    // must carry the same token — that's the invariant that lets a UI
    // multiplex notifications back to the originating request.
    let first_token = captured[0].progress_token.clone();
    for (i, n) in captured.iter().enumerate() {
        assert_eq!(
            n.progress_token, first_token,
            "notification {i} carries a different progress token than the \
             first one in the batch — all notifications from one tool call \
             must share a token. first={:?}, this={:?}",
            first_token, n.progress_token,
        );
    }

    // The MCP 2024-11-05 spec ("Progress" section) requires `progress`
    // to monotonically increase across notifications for a given
    // `progressToken`. The reporter tracks cumulative file + batch
    // counters specifically to honour this — assert that on the wire.
    for (i, w) in captured.windows(2).enumerate() {
        assert!(
            w[1].progress >= w[0].progress,
            "notifications/progress regressed between index {i} and {next} \
             (MCP spec violation): {prev_p} -> {next_p} (messages: \
             {prev_m:?} -> {next_m:?})",
            next = i + 1,
            prev_p = w[0].progress,
            next_p = w[1].progress,
            prev_m = w[0].message,
            next_m = w[1].message,
        );
        assert!(
            w[1].total.map(|t| t >= w[1].progress).unwrap_or(true),
            "total < progress at index {next} (spec violation): \
             progress={p}, total={t:?}, message={m:?}",
            next = i + 1,
            p = w[1].progress,
            t = w[1].total,
            m = w[1].message,
        );
    }

    // The terminal notification is the `IndexProgress::Done` event,
    // which is mapped to `progress == total` so progress-bar UIs close
    // on a clean 100% tick.
    let last = captured.last().unwrap();
    assert_eq!(
        Some(last.progress),
        last.total,
        "the final progress notification must report progress == total \
         (the IndexProgress::Done → terminal-tick contract); got \
         progress={}, total={:?}, message={:?}",
        last.progress,
        last.total,
        last.message,
    );
}
