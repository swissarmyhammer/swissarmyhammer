//! Unit tests for the review pipeline ops.

use super::*;

use rmcp::model::{LoggingLevel, ProgressToken};
use swissarmyhammer_validators::review::ReviewProgressEvent;
use tokio::sync::OnceCell;

use crate::mcp::tool_registry::ToolContext;
use std::sync::atomic::{AtomicUsize, Ordering};
use swissarmyhammer_validators::review::{
    synthesize, FleetTally, ReviewReport, TasksAttempted, TasksFailed,
};

/// Build a string-typed progress token for tests.
fn token(s: &str) -> ProgressToken {
    ProgressToken(rmcp::model::NumberOrString::String(s.into()))
}

/// One event per state transition through a two-pair run: the wire params
/// must echo the token, stay monotonic, name the validator and the FULL
/// untruncated file path, and close with `progress == total`.
#[test]
fn review_progress_params_are_monotonic_and_close_at_the_planned_total() {
    let mut state = ReviewProgressState::default();
    let tok = token("review-tok");
    let long_path = "src/a/very/deeply/nested/module/path/payments_processing.rs";
    let events = [
        ReviewProgressEvent::Planned { total_pairs: 2 },
        ReviewProgressEvent::PairStarted {
            validator: "duplication".to_string(),
            file: long_path.to_string(),
        },
        ReviewProgressEvent::PairStarted {
            validator: "reuse".to_string(),
            file: "src/util.rs".to_string(),
        },
        ReviewProgressEvent::PairDone {
            validator: "duplication".to_string(),
            file: long_path.to_string(),
        },
        ReviewProgressEvent::PairDone {
            validator: "reuse".to_string(),
            file: "src/util.rs".to_string(),
        },
    ];
    let params: Vec<_> = events
        .iter()
        .map(|event| review_progress_param(&mut state, &tok, event).expect("progress variant"))
        .collect();

    // Every notification echoes the request's token.
    assert!(params.iter().all(|p| p.progress_token == tok));

    // `progress` is monotonically non-decreasing and never exceeds `total`.
    for w in params.windows(2) {
        assert!(
            w[1].progress >= w[0].progress,
            "progress regressed: {:?} -> {:?}",
            w[0],
            w[1]
        );
    }
    assert!(params.iter().all(|p| p.total.unwrap() >= p.progress));

    // The plan announces the pair total before any pair completes.
    assert_eq!(params[0].progress, 0.0);
    assert_eq!(params[0].total, Some(2.0));

    // Messages name the validator and the full untruncated file path.
    let started = params[1].message.as_deref().unwrap();
    assert!(
        started.contains("duplication") && started.contains(long_path),
        "message must name validator + full path: {started}"
    );
    let done = params[3].message.as_deref().unwrap();
    assert!(
        done.contains("duplication") && done.contains(long_path),
        "message must name validator + full path: {done}"
    );

    // The final PairDone closes the bar: progress == total == planned pairs.
    let last = params.last().unwrap();
    assert_eq!(Some(last.progress), last.total);
    assert_eq!(last.progress, 2.0);
}

/// The bridge is the run's single sequencer: MANY concurrent producers hand
/// it `PairDone` events at once — the review pool's fan-out shape — and the
/// emitted params still carry a dense, non-decreasing counter.
///
/// This is the load property that matters. No pool worker ever computes a
/// pair count: every event crosses one mpsc into one
/// [`run_review_progress_mapping`] task owning the one
/// [`ReviewProgressState`], and `PairDone` only ever does `completed += 1`.
/// So arrival interleaving reorders the MESSAGES (which pair finished when)
/// but can never reorder the counter. A future change that moved the
/// counter into the workers — or added a second emitter — would surface
/// here as a duplicate, a hole, or a regression.
///
/// Deliberately `multi_thread`: the producers must genuinely race.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_producers_still_emit_a_dense_monotonic_counter() {
    /// Concurrent producer tasks, standing in for the review pool's workers.
    const PRODUCERS: usize = 8;
    /// Pairs each producer completes.
    const PAIRS_PER_PRODUCER: usize = 25;
    const TOTAL_PAIRS: usize = PRODUCERS * PAIRS_PER_PRODUCER;
    /// A keep-alive window far longer than the test can run, so no
    /// keep-alive re-send can add a duplicate param to the sequence
    /// asserted below.
    const TEST_KEEP_ALIVE_VERY_LONG: std::time::Duration = std::time::Duration::from_secs(3600);

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (param_tx, mut param_rx) = tokio::sync::mpsc::unbounded_channel();
    let mapping = tokio::spawn(run_review_progress_mapping(
        event_rx,
        param_tx,
        None,
        Some(token("concurrent")),
        TEST_KEEP_ALIVE_VERY_LONG,
    ));

    event_tx
        .send(ReviewProgressEvent::Planned {
            total_pairs: TOTAL_PAIRS,
        })
        .expect("the mapping task is receiving");

    let producers: Vec<_> = (0..PRODUCERS)
        .map(|worker| {
            let tx = event_tx.clone();
            tokio::spawn(async move {
                for pair in 0..PAIRS_PER_PRODUCER {
                    tx.send(ReviewProgressEvent::PairDone {
                        validator: format!("validator-{worker}"),
                        file: format!("src/worker_{worker}/pair_{pair}.rs"),
                    })
                    .expect("the mapping task is still receiving");
                    // Interleave the producers rather than letting each run
                    // its whole burst in one poll.
                    tokio::task::yield_now().await;
                }
            })
        })
        .collect();
    for producer in producers {
        producer.await.expect("producer task joins");
    }

    // Dropping every sender ends the mapping loop, so the params below are
    // the complete emission — no timing window to race.
    drop(event_tx);
    mapping
        .await
        .expect("the mapping task ends with its senders");
    let params = take_buffered(&mut param_rx);

    // The plan announcement, then exactly one param per completed pair.
    assert_eq!(
        params.len(),
        TOTAL_PAIRS + 1,
        "expected the plan param plus one per pair"
    );
    for w in params.windows(2) {
        assert!(
            w[1].progress >= w[0].progress,
            "progress regressed under concurrent producers: {:?} -> {:?}",
            w[0],
            w[1]
        );
    }
    // Stronger than non-decreasing: the counter is dense, so a lost or
    // double-counted increment fails even though it stays monotonic.
    let emitted: Vec<u64> = params.iter().map(|p| p.progress as u64).collect();
    assert_eq!(
        emitted,
        (0..=TOTAL_PAIRS as u64).collect::<Vec<_>>(),
        "the emitted counter must be the dense sequence 0..={TOTAL_PAIRS}"
    );
    assert!(
        params
            .iter()
            .all(|p| p.total == Some(TOTAL_PAIRS as f64) && p.progress <= TOTAL_PAIRS as f64),
        "every param carries the single announced total"
    );
    let last = params.last().unwrap();
    assert_eq!(
        Some(last.progress),
        last.total,
        "the run closes the bar at progress == total"
    );
}

/// `DownloadingModel` events map to zero-progress params that name the full
/// file and both byte counts, and never regress the wire progress when the
/// plan/pair events that follow move the counters.
#[test]
fn downloading_model_events_map_to_zero_progress_params_naming_file_and_bytes() {
    let mut state = ReviewProgressState::default();
    let tok = token("dl-tok");
    let file = "models/qwen3-embedding/model-00001-of-00002.safetensors";
    let events = [
        ReviewProgressEvent::DownloadingModel {
            file: file.to_string(),
            downloaded_bytes: 0,
            total_bytes: 500,
        },
        ReviewProgressEvent::DownloadingModel {
            file: file.to_string(),
            downloaded_bytes: 500,
            total_bytes: 500,
        },
        ReviewProgressEvent::Planned { total_pairs: 1 },
        ReviewProgressEvent::PairDone {
            validator: "duplication".to_string(),
            file: "src/a.rs".to_string(),
        },
    ];
    let params: Vec<_> = events
        .iter()
        .map(|event| review_progress_param(&mut state, &tok, event).expect("progress variant"))
        .collect();

    // Every param echoes the request's token.
    assert!(params.iter().all(|p| p.progress_token == tok));

    // Downloads precede planning: their params sit at zero progress.
    assert_eq!(params[0].progress, 0.0);
    assert_eq!(params[1].progress, 0.0);

    // The message names the FULL untruncated path and both byte counts.
    let msg = params[1].message.as_deref().unwrap();
    assert!(msg.contains(file), "message must name the full file: {msg}");
    assert!(
        msg.contains("500"),
        "message must carry the byte counts: {msg}"
    );

    // Wire progress never regresses across download → plan → done.
    for w in params.windows(2) {
        assert!(
            w[1].progress >= w[0].progress,
            "progress regressed: {:?} -> {:?}",
            w[0],
            w[1]
        );
        assert!(w[1].total.unwrap() >= w[1].progress);
    }

    // The one planned pair completing closes the bar.
    let last = params.last().unwrap();
    assert_eq!(Some(last.progress), last.total);
    assert_eq!(last.progress, 1.0);
}

/// A single `DownloadingModel` event through the real bridge mapping must
/// emit ONE token-echoing param (proving it is not a no-op) AND arm the
/// keep-alive — the pre-scope model download is exactly what fills the
/// otherwise-silent window before `scope_review` emits its first event, so
/// its param must both reach the wire and re-send during continued silence.
#[tokio::test(start_paused = true)]
async fn a_downloading_model_event_emits_a_param_and_arms_the_keep_alive() {
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (param_tx, mut param_rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(run_review_progress_mapping(
        event_rx,
        param_tx,
        None,
        Some(token("dl")),
        TEST_KEEP_ALIVE,
    ));

    event_tx
        .send(ReviewProgressEvent::DownloadingModel {
            file: "models/qwen3-embedding/model.safetensors".to_string(),
            downloaded_bytes: 10,
            total_bytes: 100,
        })
        .unwrap();
    advance(std::time::Duration::ZERO).await;
    let mapped = take_buffered(&mut param_rx);
    assert_eq!(
        mapped.len(),
        1,
        "the download event maps to one real param, not a no-op"
    );
    assert_eq!(mapped[0].progress_token, token("dl"));
    let msg = mapped[0].message.as_deref().unwrap();
    assert!(
        msg.contains("models/qwen3-embedding/model.safetensors") && msg.contains("100"),
        "the param names the file and byte counts: {msg}"
    );

    // The emitted param armed the keep-alive: continued silence re-sends it
    // verbatim, holding the client's timeout through the download window.
    advance(TEST_KEEP_ALIVE + std::time::Duration::from_millis(1)).await;
    let tick = take_buffered(&mut param_rx);
    assert_eq!(tick.len(), 1, "the download param armed the keep-alive");
    assert_eq!(tick[0].message, mapped[0].message);
    assert_eq!(tick[0].progress, mapped[0].progress);
}

/// A scope-phase event names the file with no counter movement: progress
/// and total stay at their current values (zero at the start of a run),
/// so the run's first notifications are valid and monotonic.
#[test]
fn file_scoped_events_carry_a_scoping_message_without_moving_counters() {
    let mut state = ReviewProgressState::default();
    let tok = token("scope-tok");
    let param = review_progress_param(
        &mut state,
        &tok,
        &ReviewProgressEvent::FileScoped {
            file: "src/a/very/deep/path.rs".to_string(),
        },
    )
    .expect("a FileScoped event maps to a progress param");
    assert_eq!(param.progress, 0.0);
    assert_eq!(param.total, Some(0.0));
    assert_eq!(
        param.message.as_deref(),
        Some("Scoping src/a/very/deep/path.rs"),
        "the message names the full untruncated path"
    );
    assert_eq!(param.progress_token, tok);
}

/// A sample validator-tagged finding whose fields are all distinctive so a
/// streamed payload can be asserted field-by-field.
fn sample_finding() -> swissarmyhammer_validators::review::Finding {
    swissarmyhammer_validators::review::Finding {
        file: "src/payments.rs".to_string(),
        line: 8,
        validator: "duplication".to_string(),
        rule: Some("no-copy-paste".to_string()),
        claim: "copy-pasted block duplicates existing_total".to_string(),
        evidence: "`find_duplicates`: 0.94 match".to_string(),
        suggestion: Some("extract a shared helper".to_string()),
    }
}

/// A `Findings` event maps to a `notifications/message` log param carrying the
/// FULL `Finding` JSON — never a progress param, and never truncated.
#[test]
fn findings_events_map_to_a_content_log_param_with_full_finding_json() {
    let event = ReviewProgressEvent::Findings {
        validator: "duplication".to_string(),
        findings: vec![sample_finding()],
    };

    // Content never produces a progress param — the wire counter must not move.
    let mut state = ReviewProgressState::default();
    assert!(
        review_progress_param(&mut state, &token("t"), &event).is_none(),
        "a Findings event must not map to a progress param"
    );

    let param = review_content_log_param(&event).expect("a Findings event maps to a log param");
    assert_eq!(param.logger.as_deref(), Some("review"));
    assert!(matches!(param.level, LoggingLevel::Info));
    assert_eq!(param.data["kind"], "review.findings");
    assert_eq!(param.data["validator"], "duplication");
    // The full Finding JSON is present — every load-bearing field, untruncated.
    let f = &param.data["findings"][0];
    assert_eq!(f["file"], "src/payments.rs");
    assert_eq!(f["line"], 8);
    assert_eq!(f["validator"], "duplication");
    assert_eq!(f["rule"], "no-copy-paste");
    assert_eq!(f["claim"], "copy-pasted block duplicates existing_total");
    assert_eq!(f["evidence"], "`find_duplicates`: 0.94 match");
}

/// A `Verdict` event maps to a `notifications/message` log param carrying the
/// full finding, the confirmed flag, and the reason — never a progress param.
#[test]
fn verdict_events_map_to_a_content_log_param_with_full_finding_and_reason() {
    let event = ReviewProgressEvent::Verdict {
        finding: sample_finding(),
        confirmed: true,
        reason: "substantiated by the evidence".to_string(),
    };

    let mut state = ReviewProgressState::default();
    assert!(
        review_progress_param(&mut state, &token("t"), &event).is_none(),
        "a Verdict event must not map to a progress param"
    );

    let param = review_content_log_param(&event).expect("a Verdict event maps to a log param");
    assert_eq!(param.data["kind"], "review.verdict");
    assert_eq!(param.data["confirmed"], true);
    assert_eq!(param.data["reason"], "substantiated by the evidence");
    assert_eq!(
        param.data["finding"]["claim"],
        "copy-pasted block duplicates existing_total"
    );
    assert_eq!(param.data["finding"]["file"], "src/payments.rs");
}

/// The progress-tick variants carry no content — they route to
/// notifications/progress, never notifications/message.
#[test]
fn progress_tick_events_have_no_content_log_param() {
    assert!(review_content_log_param(&ReviewProgressEvent::Planned { total_pairs: 1 }).is_none());
    assert!(review_content_log_param(&ReviewProgressEvent::PairStarted {
        validator: "v".to_string(),
        file: "a.rs".to_string(),
    })
    .is_none());
    assert!(review_content_log_param(&ReviewProgressEvent::PairDone {
        validator: "v".to_string(),
        file: "a.rs".to_string(),
    })
    .is_none());
}

/// Drain everything currently buffered on `rx` without waiting.
fn take_buffered<T>(rx: &mut tokio::sync::mpsc::UnboundedReceiver<T>) -> Vec<T> {
    let mut out = Vec::new();
    while let Ok(param) = rx.try_recv() {
        out.push(param);
    }
    out
}

/// The keep-alive test's silence window. Chosen distinct from the
/// production [`REVIEW_PROGRESS_KEEP_ALIVE_INTERVAL`] (10s) so the tests
/// pin the schedule's *shape* (re-send after `keep_alive` of silence), not
/// the production constant's value.
const TEST_KEEP_ALIVE: std::time::Duration = std::time::Duration::from_secs(7);

/// Let the paused-time runtime run every ready task, then advance the
/// clock by `dur` and let timers fire.
async fn advance(dur: std::time::Duration) {
    // Yield first so the mapping task processes any just-sent events
    // before the clock moves (paused time only advances when idle).
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    tokio::time::advance(dur).await;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
}

/// With no engine event for longer than the keep-alive interval, the
/// mapping re-sends the latest param verbatim — and keeps re-sending it
/// every interval while the silence lasts.
#[tokio::test(start_paused = true)]
async fn keep_alive_resends_the_latest_param_during_engine_silence() {
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (param_tx, mut param_rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(run_review_progress_mapping(
        event_rx,
        param_tx,
        None,
        Some(token("ka")),
        TEST_KEEP_ALIVE,
    ));

    event_tx
        .send(ReviewProgressEvent::Planned { total_pairs: 3 })
        .unwrap();
    advance(std::time::Duration::ZERO).await;
    let initial = take_buffered(&mut param_rx);
    assert_eq!(initial.len(), 1, "the event maps to one param");

    // One full silence window: the latest param is re-sent unchanged.
    advance(TEST_KEEP_ALIVE + std::time::Duration::from_millis(1)).await;
    let first_tick = take_buffered(&mut param_rx);
    assert_eq!(first_tick.len(), 1, "one keep-alive per silence window");
    assert_eq!(first_tick[0].progress, initial[0].progress);
    assert_eq!(first_tick[0].total, initial[0].total);
    assert_eq!(first_tick[0].message, initial[0].message);

    // The silence continues: another window, another identical re-send.
    advance(TEST_KEEP_ALIVE + std::time::Duration::from_millis(1)).await;
    let second_tick = take_buffered(&mut param_rx);
    assert_eq!(second_tick.len(), 1, "keep-alives repeat while silent");
    assert_eq!(second_tick[0].progress, initial[0].progress);
}

/// Before any engine event exists there is nothing to re-send: the timer
/// stays disarmed no matter how long the run takes to produce its first
/// event.
#[tokio::test(start_paused = true)]
async fn keep_alive_stays_disarmed_before_the_first_event() {
    let (_event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (param_tx, mut param_rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(run_review_progress_mapping(
        event_rx,
        param_tx,
        None,
        Some(token("disarmed")),
        TEST_KEEP_ALIVE,
    ));

    advance(TEST_KEEP_ALIVE * 6).await;
    assert!(
        take_buffered(&mut param_rx).is_empty(),
        "no event yet means nothing to re-send"
    );
}

/// Every engine event restarts the silence window — a steadily streaming
/// run never produces keep-alive duplicates.
#[tokio::test(start_paused = true)]
async fn keep_alive_window_resets_on_every_event() {
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (param_tx, mut param_rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(run_review_progress_mapping(
        event_rx,
        param_tx,
        None,
        Some(token("reset")),
        TEST_KEEP_ALIVE,
    ));

    // Three events spaced just under the window: no tick ever fires.
    let just_under = TEST_KEEP_ALIVE - std::time::Duration::from_secs(1);
    for _ in 0..3 {
        event_tx
            .send(ReviewProgressEvent::FileScoped {
                file: "src/streaming.rs".to_string(),
            })
            .unwrap();
        advance(just_under).await;
    }
    assert_eq!(
        take_buffered(&mut param_rx).len(),
        3,
        "steady streaming maps 1:1 with no keep-alive duplicates"
    );

    // Then real silence: the tick fires once the full window elapses.
    advance(TEST_KEEP_ALIVE).await;
    assert_eq!(
        take_buffered(&mut param_rx).len(),
        1,
        "the window re-arms from the last event"
    );
}

/// Dropping the engine sender ends the mapping (and with it the ticks):
/// a finished run cannot keep emitting keep-alives forever.
#[tokio::test(start_paused = true)]
async fn keep_alive_stops_when_the_engine_sender_drops() {
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (param_tx, mut param_rx) = tokio::sync::mpsc::unbounded_channel();
    let mapping = tokio::spawn(run_review_progress_mapping(
        event_rx,
        param_tx,
        None,
        Some(token("done")),
        TEST_KEEP_ALIVE,
    ));

    event_tx
        .send(ReviewProgressEvent::Planned { total_pairs: 1 })
        .unwrap();
    advance(std::time::Duration::ZERO).await;
    drop(event_tx);
    advance(TEST_KEEP_ALIVE * 3).await;

    assert!(mapping.is_finished(), "the mapping ends with the engine");
    // Only the one mapped event ever reached the wire side.
    assert_eq!(take_buffered(&mut param_rx).len(), 1);
}

/// A bare `ToolContext` (no token, no sink, no peer).
fn bare_context() -> ToolContext {
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let tool_handlers = Arc::new(crate::mcp::tool_handlers::ToolHandlers::new());
    let agent_config = Arc::new(swissarmyhammer_config::ChatModelConfig::default());
    ToolContext::new(tool_handlers, git_ops, agent_config)
}

/// With no transport at all (no token, no sink, no peer) there is nowhere
/// to ship any notification — no bridge. A missing token alone is NOT what
/// skips the bridge anymore: see the plan table below.
#[tokio::test]
async fn no_transport_at_all_means_no_bridge() {
    assert!(spawn_review_progress_bridge(&bare_context()).is_none());
}

/// A token with neither transport (no peer, no sink) cannot ship
/// notifications anywhere — the bridge is skipped, not half-built.
#[tokio::test]
async fn a_token_without_peer_or_sink_means_no_bridge() {
    let context = bare_context().with_progress_token(token("t"));
    assert!(spawn_review_progress_bridge(&context).is_none());
}

/// The bridge's transport table: content streaming needs only a peer —
/// `notifications/progress` alone is token-gated (MCP spec), so a peer
/// WITHOUT a token gets a content-only bridge (the field regression:
/// tokenless clients used to get no bridge and total silence), and the
/// in-process sink carries token-keyed progress params only.
#[test]
fn bridge_plan_streams_content_to_a_peer_without_a_token() {
    use ReviewBridgePlan as Plan;
    // (token, sink, peer) → plan
    let cases = [
        ((false, false, false), Plan::Nothing),
        ((true, false, false), Plan::Nothing),
        ((true, true, false), Plan::SinkProgressOnly),
        ((true, true, true), Plan::SinkProgressOnly),
        ((true, false, true), Plan::PeerProgressAndContent),
        ((false, false, true), Plan::PeerContentOnly),
        ((false, true, false), Plan::Nothing),
        ((false, true, true), Plan::Nothing),
    ];
    for ((has_token, has_sink, has_peer), want) in cases {
        assert_eq!(
            review_bridge_plan(has_token, has_sink, has_peer),
            want,
            "plan for (token={has_token}, sink={has_sink}, peer={has_peer})"
        );
    }
}

/// The tokenless mapping contract, under paused time: content events flow
/// to the content channel; progress ticks are dropped (token-gated); and
/// after the first content send, the gap between consecutive
/// `notifications/message` sends never exceeds the keep-alive interval —
/// even while tick events (which produce NO wire traffic without a token)
/// keep arriving, so a tokenless client's tool timeout keeps resetting.
#[tokio::test(start_paused = true)]
async fn tokenless_mapping_streams_content_and_bounds_message_gaps_by_the_keep_alive() {
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (param_tx, mut param_rx) = tokio::sync::mpsc::unbounded_channel();
    let (content_tx, mut content_rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(run_review_progress_mapping(
        event_rx,
        param_tx,
        Some(content_tx),
        None,
        TEST_KEEP_ALIVE,
    ));

    // Progress ticks BEFORE any content: no wire traffic on either
    // channel, and the keep-alive stays disarmed (nothing sent yet).
    event_tx
        .send(ReviewProgressEvent::Planned { total_pairs: 2 })
        .unwrap();
    advance(TEST_KEEP_ALIVE * 3).await;
    assert!(
        take_buffered(&mut param_rx).is_empty(),
        "no token means no progress params"
    );
    assert!(
        take_buffered(&mut content_rx).is_empty(),
        "the keep-alive stays disarmed before the first wire send"
    );

    // First content event: one message crosses, arming the keep-alive.
    event_tx
        .send(ReviewProgressEvent::Findings {
            validator: "v".to_string(),
            findings: vec![],
        })
        .unwrap();
    advance(std::time::Duration::ZERO).await;
    let first = take_buffered(&mut content_rx);
    assert_eq!(first.len(), 1, "the content event maps to one message");
    assert_eq!(first[0].data["kind"], "review.findings");

    // The engine keeps ticking progress — traffic-free for a tokenless
    // call — so the message-channel silence must still be capped: one
    // keep-alive message per full window since the LAST SEND, not since
    // the last engine event.
    let just_under = TEST_KEEP_ALIVE - std::time::Duration::from_secs(1);
    advance(just_under).await;
    event_tx
        .send(ReviewProgressEvent::PairDone {
            validator: "v".to_string(),
            file: "src/a.rs".to_string(),
        })
        .unwrap();
    advance(std::time::Duration::from_secs(1) + std::time::Duration::from_millis(1)).await;
    let keep_alives = take_buffered(&mut content_rx);
    assert_eq!(
        keep_alives.len(),
        1,
        "one keep-alive per silence window since the last message send"
    );
    assert_eq!(keep_alives[0].data["kind"], "review.keep-alive");
    assert_eq!(keep_alives[0].logger.as_deref(), Some("review"));
    assert!(
        take_buffered(&mut param_rx).is_empty(),
        "still zero notifications/progress"
    );

    // Continued silence: another window, another keep-alive.
    advance(TEST_KEEP_ALIVE + std::time::Duration::from_millis(1)).await;
    assert_eq!(
        take_buffered(&mut content_rx).len(),
        1,
        "keep-alives repeat while the wire stays silent"
    );
}

/// Token + in-process sink wires the bridge: engine events are mapped to
/// wire params carrying the token, and dropping the engine sender flushes
/// the drain to completion.
#[tokio::test]
async fn a_token_with_a_sink_bridges_engine_events_to_the_sink() {
    let (sink_tx, mut sink_rx) = tokio::sync::mpsc::unbounded_channel();
    let context = bare_context()
        .with_progress_token(token("bridge-tok"))
        .with_progress_sink(sink_tx);
    let bridge = spawn_review_progress_bridge(&context).expect("bridge wired");
    let (sender, drain) = bridge.into_parts();

    sender
        .send(ReviewProgressEvent::Planned { total_pairs: 1 })
        .unwrap();
    // A content event on the sink path must NOT reach the progress sink — the
    // sink contract is progress params only. It is dropped (no peer here), so
    // it neither becomes a sink param nor moves the wire counter.
    sender
        .send(ReviewProgressEvent::Findings {
            validator: "v".to_string(),
            findings: vec![],
        })
        .unwrap();
    sender
        .send(ReviewProgressEvent::PairDone {
            validator: "v".to_string(),
            file: "src/a.rs".to_string(),
        })
        .unwrap();

    // Dropping the engine's sender winds the bridge down; awaiting the
    // drain proves every buffered notification flushed first.
    drop(sender);
    drain.await.expect("drain joins cleanly");

    let mut got = Vec::new();
    while let Ok(param) = sink_rx.try_recv() {
        got.push(param);
    }
    assert_eq!(
        got.len(),
        2,
        "only the two progress events reach the sink; the content event does not: {got:#?}"
    );
    assert!(got.iter().all(|p| p.progress_token == token("bridge-tok")));
    let last = got.last().unwrap();
    assert_eq!(
        Some(last.progress),
        last.total,
        "the single planned pair completed, closing the bar"
    );
}

/// A multi-batch run emits one `Planned` per batch; the wire `total` is the
/// running sum so progress still closes at the whole run's pair count.
#[test]
fn review_progress_totals_accumulate_across_batches() {
    let mut state = ReviewProgressState::default();
    let tok = token("t");

    let first = review_progress_param(
        &mut state,
        &tok,
        &ReviewProgressEvent::Planned { total_pairs: 2 },
    )
    .expect("a Planned event maps to a progress param");
    assert_eq!(first.total, Some(2.0));

    for file in ["src/a.rs", "src/b.rs"] {
        let _ = review_progress_param(
            &mut state,
            &tok,
            &ReviewProgressEvent::PairDone {
                validator: "v".to_string(),
                file: file.to_string(),
            },
        );
    }

    let second_plan = review_progress_param(
        &mut state,
        &tok,
        &ReviewProgressEvent::Planned { total_pairs: 3 },
    )
    .expect("a Planned event maps to a progress param");
    assert_eq!(
        second_plan.total,
        Some(5.0),
        "totals accumulate across batches"
    );
    assert_eq!(second_plan.progress, 2.0, "completed pairs carry over");
}

/// A report carrying the given fan-out task tally and no findings, built
/// through the engine's own `synthesize` (the one construction path a
/// `ReviewReport` has now that its fields are encapsulated).
fn report_with_tally(attempted: TasksAttempted, failed: TasksFailed) -> ReviewReport {
    synthesize(
        vec![],
        &FleetTally::new(attempted, failed),
        &[],
        &swissarmyhammer_validators::review::ToolReport::default(),
        "now",
    )
}

/// Parity guard: the `backend` modifier influences ONLY the pool's worker
/// count, never which agent/model runs.
///
/// The review pipeline drives a single agent built by `agent_factory()` from
/// the resolved review `ChatModelConfig` (default `--model haiku`), shared
/// across every pool worker. `backend` reaches only `pool_config_for`, so a
/// `local` and a `session` run over the same config resolve the SAME model —
/// the two backends differ exclusively in worker count and AIMD, never in the
/// agent. This asserts that contract so a future change cannot quietly route
/// `local` to a different agent and drift the model.
#[test]
fn backend_only_governs_pool_policy_not_the_agent_model() {
    let local = pool_config_for(Some("local"), None);
    let session = pool_config_for(Some("session"), None);

    // The local backend serializes to one in-process model/GPU worker; the
    // session backend runs the remote default fan-out. This is the ONLY
    // axis `backend` controls.
    assert_eq!(local.workers, 1, "local backend is single-worker");
    assert_eq!(
        session.workers, DEFAULT_REMOTE_WORKERS,
        "session backend runs the remote default fan-out"
    );

    // A pinned `review.concurrency` overrides the worker count for BOTH
    // backends identically, confirming the only difference is the policy —
    // not the agent the worker drives.
    let local_pinned = pool_config_for(Some("local"), Some(3));
    let session_pinned = pool_config_for(Some("session"), Some(3));
    assert_eq!(local_pinned.workers, session_pinned.workers);
    assert_eq!(local_pinned.workers, 3);
}

#[test]
fn a_majority_failed_review_is_never_refused_now_returned_with_the_incomplete_banner() {
    // The calcutron symptom: every fan-out task failed. There is no retry —
    // the run's report is returned as-is, never refused as a tool error;
    // `synthesize` already stamps the loud INCOMPLETE banner so an all-failed
    // run cannot be mistaken for a clean pass.
    let report = report_with_tally(TasksAttempted(60), TasksFailed(60));
    assert!(
        report.markdown().contains("results are INCOMPLETE"),
        "an all-failed report must render the INCOMPLETE banner: {}",
        report.markdown()
    );
    assert_eq!(report.counts().tasks_attempted(), 60);
    assert_eq!(report.counts().tasks_failed(), 60);
}

#[test]
fn a_majority_failed_review_report_carries_the_failure_tally() {
    // A majority (not all) failing is the same "no retry, return flagged"
    // contract — the threshold that used to gate the refusal no longer
    // matters at all: every failure rate is returned.
    let report = report_with_tally(TasksAttempted(10), TasksFailed(7));
    assert!(
        report.markdown().contains("results are INCOMPLETE"),
        "a majority-failed report must render the INCOMPLETE banner: {}",
        report.markdown()
    );
    assert_eq!(report.counts().tasks_attempted(), 10);
    assert_eq!(report.counts().tasks_failed(), 7);
}

#[test]
fn a_minority_failed_review_report_still_carries_the_incomplete_banner() {
    // A minority of tasks failed (1 of 10) — the report is returned with the
    // gap flagged, exactly as a majority/all-failed run is: there is no
    // separate threshold behavior left at this boundary.
    let report = report_with_tally(TasksAttempted(10), TasksFailed(1));
    assert!(
        report.markdown().contains("results are INCOMPLETE"),
        "any non-zero failure must render the INCOMPLETE banner: {}",
        report.markdown()
    );
    assert_eq!(report.counts().tasks_failed(), 1);
}

#[test]
fn a_fully_successful_review_report_carries_no_incomplete_banner() {
    let report = report_with_tally(TasksAttempted(8), TasksFailed(0));
    assert!(
        !report.markdown().contains("INCOMPLETE"),
        "a clean run must not render the banner: {}",
        report.markdown()
    );
    assert_eq!(report.counts().tasks_failed(), 0);
}

#[test]
fn a_run_that_attempted_no_tasks_carries_no_incomplete_banner() {
    // An empty diff attempts no fan-out tasks — there is no failure rate to
    // speak of, so no banner renders.
    let report = report_with_tally(TasksAttempted(0), TasksFailed(0));
    assert!(!report.markdown().contains("INCOMPLETE"));
    assert_eq!(report.counts().tasks_attempted(), 0);
    assert_eq!(report.counts().tasks_failed(), 0);
}

fn mock() -> Arc<dyn model_embedding::TextEmbedder> {
    Arc::new(model_embedding::mock::MockEmbedder::new(4)) as Arc<dyn model_embedding::TextEmbedder>
}

/// The shared-embedder cache runs `init` exactly once and hands every caller
/// the same `Arc` — the load-once-share contract `default_embedder_factory`
/// relies on so the model isn't reloaded per review run.
#[tokio::test]
async fn shared_embedder_initializes_once_and_shares_the_arc() {
    let cell = OnceCell::new();
    let calls = AtomicUsize::new(0);

    let first = shared_embedder(&cell, || async {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(mock())
    })
    .await
    .expect("first init");

    let second = shared_embedder(&cell, || async {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(mock())
    })
    .await
    .expect("second call hits the cache");

    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "init must run exactly once"
    );
    assert!(
        Arc::ptr_eq(&first, &second),
        "both calls must share the one cached Arc"
    );
}

/// A failed `init` is not cached: a later call retries rather than handing
/// back a poisoned/failed cell forever.
#[tokio::test]
async fn shared_embedder_does_not_cache_a_failed_init() {
    let cell = OnceCell::new();

    let failed = shared_embedder(&cell, || async {
        Err::<Arc<dyn model_embedding::TextEmbedder>, EmbedderError>(EmbedderError::Load(
            "load failed".to_string(),
        ))
    })
    .await;
    assert!(failed.is_err(), "the failed init surfaces as an error");

    let retried = shared_embedder(&cell, || async { Ok(mock()) }).await;
    assert!(
        retried.is_ok(),
        "a failed init must not poison the cache; a later init succeeds"
    );
}

/// Encapsulating `ReviewResponse`/`ReviewCountsView` must not change the
/// serialized wire shape: the same top-level keys and count keys as the
/// public-field era, values readable back through the getters.
#[test]
fn review_response_wire_shape_and_getters_survive_encapsulation() {
    let response = ReviewResponse::from(report_with_tally(TasksAttempted(10), TasksFailed(1)));

    let json = serde_json::to_value(&response).expect("serializes");
    assert!(json["markdown"].is_string(), "markdown key present: {json}");
    for key in ["findings", "confirmed", "refuted", "attempted", "failed"] {
        assert!(json["counts"][key].is_u64(), "counts.{key} present: {json}");
    }
    assert_eq!(json["counts"]["attempted"], serde_json::json!(10));
    assert_eq!(json["counts"]["failed"], serde_json::json!(1));

    // Getters read the same values the wire carries.
    assert!(response.markdown().starts_with("## Review Findings"));
    assert_eq!(response.counts().attempted(), 10);
    assert_eq!(response.counts().failed(), 1);
    assert_eq!(response.counts().findings(), 0);

    // The counts view is a value type: Clone + Eq hold.
    let cloned = response.counts().clone();
    assert_eq!(&cloned, response.counts());
    // The skipped-file list is present on the wire, empty when no file
    // was skipped.
    assert_eq!(json["counts"]["skipped_files"], serde_json::json!([]));
    assert_eq!(response.counts().skipped_files(), &[] as &[String]);
}
