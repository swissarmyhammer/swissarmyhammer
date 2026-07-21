//! Client-side receipt e2e for `review` `notifications/progress` over a real
//! byte-stream transport.
//!
//! The production failure this guards against: a `review sha` call armed the
//! progress bridge but the client received ZERO `notifications/progress` in
//! 1801 s and aborted at its MCP timeout — every event was emitted after the
//! scope stage, and nothing proved delivery. The only admissible evidence of
//! delivery is a real rmcp CLIENT's overridden `on_progress` firing after the
//! bytes crossed a real transport boundary — never a server-side channel,
//! sink, or buffer.
//!
//! ## Transport choice
//!
//! The strongest harness would spawn the `sah` binary (`serve` over stdio) as
//! a child process, but the binary cannot run a scripted review agent — the
//! injection seam (`McpServer::set_review_factories`) is in-process only, and
//! a real-model run is forbidden here. So this test uses the sanctioned
//! fallback: the REAL `McpServer` served by `rmcp::serve_server` over a raw
//! duplex byte stream (`tokio::io::duplex`), with a real rmcp client connected
//! to the other end. Every notification is fully serialized to JSON-RPC
//! bytes, crosses the stream, and is deserialized by the client's receive
//! loop — the same wire path stdio uses, minus the process boundary.
//!
//! ## Determinism
//!
//! The scripted agent carries a [`PromptGate`]: its FIRST prompt (the fleet's
//! prime turn) blocks until the test releases it. While the gate is closed no
//! agent work can complete, so the test can assert — without racing the run —
//! that:
//!
//! 1. The client RECEIVES scope-phase notifications (`Scoping <file>`) before
//!    the agent's first prompt is answered (the events themselves are emitted
//!    before the fleet issues any prompt).
//! 2. During the injected >10 s stall the client receives keep-alive re-sends
//!    of the latest param — identical `progress`/`total`, so the wire value
//!    never regresses.
//!
//! After release the run completes and the whole capture is checked for token
//! uniformity, `total >= progress`, and the order-free corollaries of a
//! monotonic wire emission (see the final assertion block for why arrival
//! order cannot be compared directly at an rmcp client).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rmcp::model::{
    CallToolRequestParams, ClientCapabilities, ClientInfo, Implementation,
    LoggingMessageNotificationParam, NumberOrString, ProgressNotificationParam, ProgressToken,
    RequestParamsMeta,
};
use rmcp::service::NotificationContext;
use rmcp::{serve_server, ClientHandler, RoleClient, ServiceExt};
use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};
use swissarmyhammer_templating::TemplateLibrary;
use swissarmyhammer_tools::mcp::McpServer;
use swissarmyhammer_validators::review::test_support::PromptGate;

// Reuse the shared review fixture (temp git repo + planted diff + on-disk
// code_context index + scripted ACP agent + mock embedder). Pulled in by path
// because integration support modules are not a library.
#[path = "integration/review_fixture.rs"]
mod review_fixture;

use review_fixture::{
    gated_planted_agent, mock_embedder_factory, plant_diff, scripted_factory, seed_on_disk_index,
    TestRepo, CLAIM_DUP, CLAIM_GUARD_HERRING, CLAIM_RED_HERRING, CLAIM_SECRET,
};

/// Byte capacity of the in-memory duplex pipe both peers read/write through.
/// Large enough that neither side blocks on a full pipe mid-message.
const DUPLEX_BUFFER_BYTES: usize = 64 * 1024;

/// How long the test waits for any awaited milestone (gate entry, first
/// receipt, keep-alive receipt, run completion) before failing loudly.
const WAIT_DEADLINE: Duration = Duration::from_secs(60);

/// The receive loop is considered settled once no new notification lands for
/// this long — used to snapshot a stable baseline before the injected stall.
const QUIESCE_WINDOW: Duration = Duration::from_millis(1500);

/// Client handler that captures every `notifications/progress` AND every
/// `notifications/message` it sees. The progress channel carries the pair-count
/// ticks; the message channel carries the streamed review CONTENT (findings and
/// verdicts) this test asserts on.
///
/// This deliberately mirrors the capturing client in
/// `review_progress_notifications_test.rs` — separate integration test
/// binaries are separate compilation units that cannot import each other.
#[derive(Clone)]
struct CapturingClient {
    info: ClientInfo,
    captured: Arc<Mutex<Vec<ProgressNotificationParam>>>,
    captured_logs: Arc<Mutex<Vec<LoggingMessageNotificationParam>>>,
}

impl CapturingClient {
    fn new() -> Self {
        Self {
            info: ClientInfo::new(
                ClientCapabilities::default(),
                Implementation::new("review-progress-stdio-capturing-client", "1.0.0"),
            ),
            captured: Arc::new(Mutex::new(Vec::new())),
            captured_logs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn snapshot(&self) -> Vec<ProgressNotificationParam> {
        self.captured.lock().unwrap().clone()
    }

    fn count(&self) -> usize {
        self.captured.lock().unwrap().len()
    }

    /// Every `notifications/message` (review content) the client has received.
    fn snapshot_logs(&self) -> Vec<LoggingMessageNotificationParam> {
        self.captured_logs.lock().unwrap().clone()
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

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        self.captured_logs.lock().unwrap().push(params);
    }
}

/// Build a string-typed progress token to thread through `_meta`.
fn make_token(s: &str) -> ProgressToken {
    ProgressToken(NumberOrString::String(s.into()))
}

/// Poll `predicate` every 50 ms until it holds, failing the test with
/// `what` after [`WAIT_DEADLINE`].
async fn wait_until(what: &str, mut predicate: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + WAIT_DEADLINE;
    while !predicate() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for: {what}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Wait until no new notification has landed for [`QUIESCE_WINDOW`], then
/// return the settled capture.
async fn settled_snapshot(handler: &CapturingClient) -> Vec<ProgressNotificationParam> {
    let deadline = tokio::time::Instant::now() + WAIT_DEADLINE;
    loop {
        let before = handler.count();
        tokio::time::sleep(QUIESCE_WINDOW).await;
        if handler.count() == before {
            return handler.snapshot();
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "notifications never quiesced while the agent was gated"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn review_progress_is_received_by_a_real_client_over_a_byte_stream_transport() {
    // Keep the server-side code_context bootstrap hermetic: this test asserts
    // review progress delivery, not semantic embeddings, so skip the multi-GB
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

    // 2. The real McpServer, wired with the gated scripted agent + mock
    //    embedder through the production injection seam.
    let (gate, mut controller) = PromptGate::new();
    let server =
        McpServer::new_with_work_dir(TemplateLibrary::default(), repo.path().to_path_buf(), None)
            .await
            .expect("server builds");
    server
        .set_review_factories(
            scripted_factory(gated_planted_agent(gate)),
            Some(mock_embedder_factory()),
            None,
        )
        .await;

    // 3. Serve the server over one end of a raw duplex byte stream and connect
    //    a real rmcp client to the other end. Every JSON-RPC message is
    //    serialized to bytes, crosses the pipe, and is deserialized by the
    //    counterpart — a real transport boundary.
    let (client_io, server_io) = tokio::io::duplex(DUPLEX_BUFFER_BYTES);
    // `serve_server(...).await` resolves only after the client's `initialize`
    // handshake, so the two ends must be brought up concurrently.
    let server_task = tokio::spawn(serve_server(server, tokio::io::split(server_io)));

    let handler = CapturingClient::new();
    let client = handler
        .clone()
        .serve(tokio::io::split(client_io))
        .await
        .expect("client connects over the duplex transport");
    let running_server = server_task
        .await
        .expect("server task joins")
        .expect("server serves over the duplex transport");

    // 4. Call `review working` with a progress token in `_meta`, concurrently
    //    with the assertions below. rmcp's `progress_token_provider` may
    //    rewrite the client-supplied token (its multiplexing contract), so the
    //    assertions check only that every notification carries the SAME token.
    let mut params = CallToolRequestParams::new("review").with_arguments({
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("review working"));
        args.insert("backend".to_string(), serde_json::json!("local"));
        args
    });
    params.set_progress_token(make_token("review-progress-stdio-token"));
    let peer = client.peer().clone();
    let call = tokio::spawn(async move { peer.call_tool(params).await });

    // 5. The scripted agent reached its first (prime) prompt and is now
    //    blocked: no agent work has completed, and none can until release.
    tokio::time::timeout(WAIT_DEADLINE, controller.entered())
        .await
        .expect("the scripted agent never received its first prompt");

    // 6. While the agent is gated the CLIENT must already be receiving
    //    progress — including scope-phase notifications, which are emitted per
    //    file before the fleet stage issues any prompt. This is the assertion
    //    that kills the production failure mode (first event only after the
    //    scope stage).
    wait_until(
        "first notifications/progress received by the client",
        || handler.count() > 0,
    )
    .await;
    let while_gated = settled_snapshot(&handler).await;
    assert!(
        while_gated
            .iter()
            .filter_map(|n| n.message.as_deref())
            .any(|m| m.starts_with("Scoping ")),
        "the client must receive scope-phase (\"Scoping <file>\") notifications \
         before the agent's first prompt is answered; got: {:#?}",
        while_gated
            .iter()
            .map(|n| n.message.clone())
            .collect::<Vec<_>>()
    );

    // 7. Injected stall: the agent stays gated (silent engine) for longer than
    //    the bridge's ~10 s keep-alive interval. The client must receive a
    //    re-send of the wire-latest param — same progress/total, so the wire
    //    value never regresses — proving the keep-alive crosses the transport.
    //
    //    Arrival order != wire order at an rmcp client (see the final
    //    assertion block), so the wire-latest param is identified order-free:
    //    within the gated phase the counters only grow, so the wire-latest
    //    carries the phase's maximum progress and maximum total.
    assert!(
        !while_gated.is_empty(),
        "at least one notification was received while gated"
    );
    let baseline = while_gated.len();
    let latest_progress = while_gated
        .iter()
        .map(|n| n.progress)
        .fold(0.0_f64, f64::max);
    let latest_total = while_gated
        .iter()
        .filter_map(|n| n.total)
        .fold(0.0_f64, f64::max);
    wait_until(
        "keep-alive re-send received during the injected stall",
        || handler.count() > baseline,
    )
    .await;
    let during_stall = handler.snapshot().split_off(baseline);
    for keep_alive in &during_stall {
        assert_eq!(
            keep_alive.progress, latest_progress,
            "a keep-alive re-send must carry the wire-latest progress: {keep_alive:#?}"
        );
        assert_eq!(
            keep_alive.total,
            Some(latest_total),
            "a keep-alive re-send must carry the wire-latest total: {keep_alive:#?}"
        );
    }

    // 8. Release the gate; the run completes normally.
    controller.release();
    let result = tokio::time::timeout(WAIT_DEADLINE, call)
        .await
        .expect("review call did not complete after release")
        .expect("call task joins")
        .expect("review working call succeeds");
    assert_eq!(
        result.is_error,
        Some(false),
        "review working returned an error: {result:?}"
    );

    // Give the client receive loop one final yield to land any tail buffered
    // notification (the server side already awaits its drain task).
    tokio::time::sleep(Duration::from_millis(100)).await;
    let captured = handler.snapshot();

    // The streamed review CONTENT — findings + verdicts — is delivered over
    // notifications/message on the SAME transport. Wait for the client to
    // receive the three shapes this test pins (a findings payload, a confirmed
    // verdict, and a refuted verdict), then snapshot the content capture before
    // shutting the client down. Receipt at the client's `on_logging_message` is
    // the only admissible evidence the new channel crosses the wire.
    let has_findings_with = |logs: &[LoggingMessageNotificationParam], claim: &str| {
        logs.iter().any(|m| {
            m.data["kind"] == "review.findings"
                && m.data["findings"]
                    .as_array()
                    .is_some_and(|fs| fs.iter().any(|f| f["claim"] == claim))
        })
    };
    let has_verdict = |logs: &[LoggingMessageNotificationParam], claim: &str, confirmed: bool| {
        logs.iter().any(|m| {
            m.data["kind"] == "review.verdict"
                && m.data["confirmed"] == confirmed
                && m.data["finding"]["claim"] == claim
        })
    };
    wait_until(
        "streamed review findings + verdicts received by the client",
        || {
            let logs = handler.snapshot_logs();
            // A full findings payload, plus one confirmed and one refuted verdict.
            // These outcomes are probe-independent (they ride the scripted agent's
            // verify rules, not the seeded index), so they hold in this hermetic
            // run. The guard-vs-agent deciding-layer split is pinned by the
            // engine-level unit test, not here.
            has_findings_with(&logs, CLAIM_DUP)
                && has_verdict(&logs, CLAIM_SECRET, true)
                && has_verdict(&logs, CLAIM_RED_HERRING, false)
        },
    )
    .await;
    let logs = handler.snapshot_logs();

    // The final report the tool returned, for the streamed-vs-final cross-check.
    let report_markdown = match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => {
            let body: serde_json::Value =
                serde_json::from_str(&t.text).expect("review response is JSON");
            body["markdown"].as_str().unwrap_or_default().to_string()
        }
        other => panic!("expected text content, got: {other:?}"),
    };

    // 9. Shut everything down cleanly before the whole-capture assertions.
    client.cancel().await.ok();
    running_server.cancel().await.ok();

    // Every notification from this one tool call carries the same token.
    let first_token = captured[0].progress_token.clone();
    for (i, n) in captured.iter().enumerate() {
        assert_eq!(
            n.progress_token, first_token,
            "notification {i} carries a different progress token; all \
             notifications from one tool call must share a token"
        );
    }

    // Wire-order monotonicity, asserted through order-free corollaries.
    //
    // rmcp's client dispatches every incoming notification on its own spawned
    // task (rmcp 1.7 `service.rs`, `spawn_service_task` per peer
    // notification), so `on_progress` ARRIVAL order can differ from wire
    // order under a burst — an adjacent-pair comparison over the capture
    // would assert the client runtime's task scheduling, not the server's
    // wire behavior. The server's monotonic emission is instead proven by
    // properties that survive any reordering of an intact set:
    //
    // 1. `total >= progress` on every notification (per-message, order-free).
    for n in &captured {
        assert!(
            n.total.map(|t| t >= n.progress).unwrap_or(true),
            "total < progress (spec violation): {n:?}"
        );
    }

    // 2. The completion counter is dense and complete: the "Reviewed ..."
    //    notifications carry exactly the values 1..=N (N = the planned pair
    //    total) with no gap and no repeat — the only set a monotonically
    //    incremented counter can emit. A regression or skip on the wire would
    //    surface here as a duplicate or a hole. Keep-alive re-sends are exact
    //    copies of a previous param, so exact-duplicate triples are collapsed
    //    first (a genuine counter bug pairs one progress value with two
    //    different messages and still fails).
    let final_total = captured
        .iter()
        .filter_map(|n| n.total)
        .fold(0.0_f64, f64::max);
    let mut seen_exact: Vec<(u64, Option<u64>, Option<&str>)> = Vec::new();
    let mut completions: Vec<u64> = Vec::new();
    for n in captured.iter().filter(|n| {
        n.message
            .as_deref()
            .is_some_and(|m| m.starts_with("Reviewed "))
    }) {
        let key = (
            n.progress as u64,
            n.total.map(|t| t as u64),
            n.message.as_deref(),
        );
        if !seen_exact.contains(&key) {
            seen_exact.push(key);
            completions.push(n.progress as u64);
        }
    }
    completions.sort_unstable();
    assert_eq!(
        completions,
        (1..=final_total as u64).collect::<Vec<_>>(),
        "the received completion notifications must carry the dense counter \
         sequence 1..={final_total} — the signature of a monotonic wire emission"
    );

    // 3. The run closes the bar: a notification with progress == total ==
    //    the final total was received (existence, not position — see above).
    assert!(
        captured
            .iter()
            .any(|n| n.progress == final_total && n.total == Some(final_total)),
        "a closing notification with progress == total == {final_total} must \
         be received; got: {captured:#?}"
    );

    // ---- streamed review CONTENT (notifications/message) --------------------
    //
    // The new channel: the client received the engine's ACTUAL findings and
    // verdicts as they resolved, on the SAME transport, in FULL — not a summary
    // and not truncated. Every assertion below reads what the client's
    // `on_logging_message` captured, never a server-side channel.

    // A) A findings payload carries the COMPLETE Finding JSON — every
    //    load-bearing field, untruncated, and validator-tagged by the engine.
    let dup_msg = logs
        .iter()
        .find(|m| {
            m.data["kind"] == "review.findings"
                && m.data["findings"]
                    .as_array()
                    .is_some_and(|fs| fs.iter().any(|f| f["claim"] == CLAIM_DUP))
        })
        .expect("a review.findings message carrying the duplication finding");
    assert_eq!(
        dup_msg.logger.as_deref(),
        Some("review"),
        "content notifications carry the review logger name: {dup_msg:#?}"
    );
    assert_eq!(dup_msg.data["validator"], "duplication");
    let dup_finding = dup_msg.data["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["claim"] == CLAIM_DUP)
        .unwrap();
    assert!(
        dup_finding["file"].as_str().is_some_and(|s| !s.is_empty()),
        "the streamed finding carries its file path: {dup_finding:#?}"
    );
    assert_eq!(
        dup_finding["validator"], "duplication",
        "the streamed finding is authoritatively validator-tagged"
    );
    assert!(
        dup_finding["rule"].is_string() && dup_finding["line"].is_number(),
        "the full Finding shape is present (rule + line), not a summary: {dup_finding:#?}"
    );
    assert!(
        dup_finding["evidence"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "the streamed finding carries its evidence untruncated: {dup_finding:#?}"
    );

    // B) Both verdict polarities stream as they resolve: a CONFIRMED finding
    //    (the hardcoded secret, confirmed by the scripted verifier) and a
    //    REFUTED one (the correct-but-looks-buggy red herring the verifier
    //    disproves). These ride the agent's verify rules, so they are robust in
    //    this hermetic run regardless of which deciding layer the probes select.
    assert!(
        has_verdict(&logs, CLAIM_SECRET, true),
        "a confirmed verdict must stream as it resolves: {logs:#?}"
    );
    assert!(
        has_verdict(&logs, CLAIM_RED_HERRING, false),
        "a refuted verdict must stream as it resolves: {logs:#?}"
    );

    // C) The stream matches the final report modulo synthesize's file:line dedup.
    //    The report is the deduped subset of the confirmed stream, so the correct
    //    direction is: every claim rendered in the report was streamed as a
    //    CONFIRMED verdict (report ⊆ confirmed-stream). Asserting the inverse
    //    (every confirmed claim appears verbatim) would wrongly fail on two
    //    confirmed findings sharing a file:line — exactly what dedup collapses.
    let confirmed_claims: std::collections::HashSet<String> = logs
        .iter()
        .filter(|m| m.data["kind"] == "review.verdict" && m.data["confirmed"] == true)
        .filter_map(|m| m.data["finding"]["claim"].as_str().map(String::from))
        .collect();
    assert!(
        !confirmed_claims.is_empty(),
        "at least one confirmed verdict must have streamed"
    );
    // A representative confirmed finding with a unique file:line (so dedup keeps
    // it) is streamed confirmed AND rendered in the report.
    assert!(
        confirmed_claims.contains(CLAIM_SECRET),
        "the secret finding must have streamed a confirmed verdict"
    );
    assert!(
        report_markdown.contains(CLAIM_SECRET),
        "a streamed confirmed finding must appear in the final report: {report_markdown}"
    );
    // No refuted claim leaks into the report: the streamed refutations (agent and
    // guard/default) are absent from the returned findings.
    assert!(
        !report_markdown.contains(CLAIM_RED_HERRING),
        "an agent-refuted claim must not appear in the final report"
    );
    assert!(
        !report_markdown.contains(CLAIM_GUARD_HERRING),
        "a refuted red-herring claim must not appear in the final report"
    );
}
