//! End-to-end MCP test for `notifications/progress` from the `review` tool.
//!
//! Modeled on `rebuild_index_progress_notifications_test.rs`: it boots the real
//! in-process HTTP MCP server, connects an rmcp client that overrides
//! `ClientHandler::on_progress` to capture every `notifications/progress`
//! message, and issues a `tools/call` for `review` `op: "review working"`
//! carrying a `progressToken` in the JSON-RPC `_meta` field. The review engine
//! is driven by the shared scripted ACP agent + mock embedder fixture, so no
//! real model runs.
//!
//! It asserts:
//!
//! 1. At least one `notifications/progress` arrives, and every captured
//!    notification echoes the same `progressToken`.
//! 2. `progress` is monotonically non-decreasing on the wire and `total >=
//!    progress` on every notification.
//! 3. Every announced (validator, file) pair ("Reviewing <file> against
//!    <validator>") also completes ("Reviewed <file> against <validator>"),
//!    and the final notification closes with `progress == total` — one
//!    completion per planned pair.
//! 4. The pair messages name the validator and the full file path.

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
use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};
use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server_with_options, McpServerMode};

// Reuse the shared review fixture (temp git repo + planted diff + on-disk
// code_context index + scripted ACP agent + mock embedder). Pulled in by path
// because integration support modules are not a library.
#[path = "integration/review_fixture.rs"]
mod review_fixture;

use review_fixture::{
    mock_embedder_factory, plant_diff, planted_agent, scripted_factory, seed_on_disk_index,
    TestRepo, FILE_PAYMENTS,
};

/// Client handler that captures every `notifications/progress` it sees.
///
/// This deliberately mirrors the capturing client in
/// `rebuild_index_progress_notifications_test.rs` — separate integration test
/// binaries are separate compilation units that cannot import each other.
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
                Implementation::new("review-progress-capturing-client", "1.0.0"),
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

/// Build a string-typed progress token to thread through `_meta`.
fn make_token(s: &str) -> ProgressToken {
    ProgressToken(NumberOrString::String(s.into()))
}

/// The pair a "Reviewing <file> against <validator>" (or "Reviewed ...")
/// message names, parsed back out of the wire message.
fn parse_pair<'m>(message: &'m str, verb_prefix: &str) -> Option<(&'m str, &'m str)> {
    let rest = message.strip_prefix(verb_prefix)?;
    let (file, validator) = rest.split_once(" against ")?;
    Some((file, validator))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_working_emits_progress_notifications_per_pair_when_token_supplied() {
    // Keep the server-side code_context bootstrap hermetic: this test asserts
    // review progress wiring, not semantic embeddings, so skip the multi-GB
    // embedding-model load. nextest runs each test in its own process, so the
    // env var does not leak into others.
    std::env::set_var("SAH_DISABLE_EMBEDDING", "1");

    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    // 1. A temp git repo with the planted diff and the seeded on-disk index
    //    the production review tool opens read-only.
    let repo = TestRepo::new();
    plant_diff(&repo);
    seed_on_disk_index(repo.path());
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    // 2. Stand up the in-process HTTP MCP server scoped to the repo, then
    //    swap in the scripted review factories (the production wiring seam).
    let mut server = start_mcp_server_with_options(
        McpServerMode::Http { port: None },
        None,
        None,
        Some(repo.path().to_path_buf()),
    )
    .await
    .expect("start MCP server");
    server
        .server()
        .expect("HTTP handle exposes the McpServer")
        .set_review_factories(
            scripted_factory(planted_agent()),
            Some(mock_embedder_factory()),
            None,
        )
        .await;

    // 3. Connect a custom client that captures every progress notification.
    let handler = CapturingClient::new();
    let transport = StreamableHttpClientTransport::with_client(reqwest::Client::default(), {
        let mut config = StreamableHttpClientTransportConfig::default();
        config.uri = server.url().into();
        config
    });
    let client = handler
        .clone()
        .serve(transport)
        .await
        .expect("connect client");

    // 4. Call `review working` with a progress token in `_meta`. rmcp's
    //    `progress_token_provider` may rewrite the client-supplied token (its
    //    multiplexing contract), so the assertions below check only that every
    //    notification carries the SAME token, not this exact value.
    let mut params = CallToolRequestParams::new("review").with_arguments({
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("review working"));
        args.insert("backend".to_string(), serde_json::json!("local"));
        args
    });
    params.set_progress_token(make_token("review-progress-test-token"));
    let result = client.call_tool(params).await.expect("review working call");
    assert_eq!(
        result.is_error,
        Some(false),
        "review working returned an error: {result:?}"
    );

    // Give the client receive loop one final yield to land any tail buffered
    // notification (the server side already awaits its drain task).
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let captured = handler.snapshot();

    // 5. Shut everything down cleanly before assertions.
    client.cancel().await.ok();
    server.shutdown().await.ok();

    // 6. Assertions.
    assert!(
        !captured.is_empty(),
        "expected at least one notifications/progress message, got zero. \
         The server should have extracted progressToken from _meta and \
         bridged the review engine's ReviewProgressEvents to the peer."
    );

    // Every notification from this one tool call carries the same token.
    let first_token = captured[0].progress_token.clone();
    for (i, n) in captured.iter().enumerate() {
        assert_eq!(
            n.progress_token, first_token,
            "notification {i} carries a different progress token; all \
             notifications from one tool call must share a token"
        );
    }

    // The MCP spec requires monotonic `progress` and permits a growing
    // `total`; `total >= progress` must hold on every notification.
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
    }
    for n in &captured {
        assert!(
            n.total.map(|t| t >= n.progress).unwrap_or(true),
            "total < progress (spec violation): {n:?}"
        );
    }

    // Every announced pair also completes: a "Reviewing <file> against
    // <validator>" has a matching "Reviewed <file> against <validator>" —
    // that is the one-notification-per-pair contract, failure included.
    let messages: Vec<&str> = captured
        .iter()
        .filter_map(|n| n.message.as_deref())
        .collect();
    let started: Vec<(&str, &str)> = messages
        .iter()
        .filter_map(|m| parse_pair(m, "Reviewing "))
        .collect();
    let done: Vec<(&str, &str)> = messages
        .iter()
        .filter_map(|m| parse_pair(m, "Reviewed "))
        .collect();
    assert!(
        !started.is_empty(),
        "at least one (validator, file) pair must be announced: {messages:#?}"
    );
    for pair in &started {
        assert!(
            done.contains(pair),
            "pair {pair:?} was announced but never completed: {messages:#?}"
        );
    }

    // The pair messages name a real validator and the full file path.
    assert!(
        started
            .iter()
            .any(|(file, validator)| *file == FILE_PAYMENTS && *validator == "duplication"),
        "expected the (duplication, {FILE_PAYMENTS}) pair to be announced \
         with the full untruncated path: {messages:#?}"
    );

    // The final notification closes the bar: progress == total, and the
    // completion count matches — progress reaches the planned pair total.
    let last = captured.last().unwrap();
    assert_eq!(
        Some(last.progress),
        last.total,
        "the final progress notification must report progress == total; got \
         progress={}, total={:?}, message={:?}",
        last.progress,
        last.total,
        last.message,
    );
    assert_eq!(
        done.len() as f64,
        last.progress,
        "one completion notification per planned pair: {messages:#?}"
    );
}
