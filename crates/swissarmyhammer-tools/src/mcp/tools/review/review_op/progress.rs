//! Review progress plumbing: map engine [`ReviewProgressEvent`]s onto MCP
//! `notifications/progress` (token-gated) and `notifications/message` content
//! logs (never token-gated), with a keep-alive so a long tool run never goes
//! silent.

use std::sync::Arc;

use rmcp::model::{
    LoggingLevel, LoggingMessageNotification, LoggingMessageNotificationParam,
    ProgressNotificationParam, ProgressToken,
};
use rmcp::{Peer, RoleServer};

use swissarmyhammer_validators::review::{ReviewProgressEvent, ReviewProgressSender};

use crate::mcp::progress::{spawn_drain_task, spawn_in_process_drain_task};
use crate::mcp::tool_registry::ToolContext;

/// Cumulative pair counters threaded across [`ReviewProgressEvent`]s so the
/// emitted wire `progress` value is monotonic.
///
/// `planned` sums every batch's `Planned { total_pairs }` (a multi-batch run
/// announces each batch as it plans it, so the wire `total` grows — the MCP
/// spec permits a growing total); `completed` counts `PairDone` events and
/// only ever increases, which is what keeps `progress` monotonic.
#[derive(Debug, Default)]
pub(super) struct ReviewProgressState {
    /// Planned (validator, file) pairs summed across every batch.
    planned: u64,
    /// Completed pairs (`PairDone` count) — monotonically non-decreasing.
    completed: u64,
}

/// Map one engine [`ReviewProgressEvent`] to the wire
/// [`ProgressNotificationParam`], updating the cumulative counters.
///
/// The wire contract: `progress` = completed pairs, `total` = planned pairs
/// (floored at `progress` so `total >= progress` always holds), the request's
/// `token` echoed on every notification, and a human `message` naming the
/// validator and the FULL file path — never truncated.
///
/// Returns `None` for the content-carrying variants
/// ([`Findings`](ReviewProgressEvent::Findings) /
/// [`Verdict`](ReviewProgressEvent::Verdict)): they carry content, not progress,
/// and route to `notifications/message` via [`review_content_log_param`] — they
/// must never move the wire counter.
pub(super) fn review_progress_param(
    state: &mut ReviewProgressState,
    token: &ProgressToken,
    event: &ReviewProgressEvent,
) -> Option<ProgressNotificationParam> {
    let message = match event {
        ReviewProgressEvent::DownloadingModel {
            file,
            downloaded_bytes,
            total_bytes,
        } => {
            // Model download precedes planning: no pair counters exist yet, so
            // the wire values stay at their current (zero) state and cannot
            // regress when planning begins. The byte detail and the full,
            // untruncated filename ride in the message.
            format!("Downloading {file}: {downloaded_bytes}/{total_bytes} bytes")
        }
        ReviewProgressEvent::FileScoped { file } => {
            // Scope-phase announcement: no pair counters exist yet, so the
            // wire values stay at their current (zero) state — the event's
            // job is existence, not arithmetic.
            format!("Scoping {file}")
        }
        ReviewProgressEvent::Planned { total_pairs } => {
            state.planned += *total_pairs as u64;
            format!("Planned {total_pairs} (validator, file) review pairs")
        }
        ReviewProgressEvent::PairStarted { validator, file } => {
            format!("Reviewing {file} against {validator}")
        }
        ReviewProgressEvent::PairDone { validator, file } => {
            state.completed += 1;
            format!("Reviewed {file} against {validator}")
        }
        // Content-carrying variants have no progress param — they route to
        // notifications/message and must not touch the wire counters.
        ReviewProgressEvent::Findings { .. } | ReviewProgressEvent::Verdict { .. } => {
            return None;
        }
    };
    let progress = state.completed;
    let total = state.planned.max(progress);
    Some(ProgressNotificationParam {
        progress_token: token.clone(),
        progress: progress as f64,
        total: Some(total as f64),
        message: Some(message),
    })
}

/// The MCP logger name every review content notification carries.
pub(super) const REVIEW_LOG_LOGGER: &str = "review";
/// The `kind` tag on a streamed findings payload.
pub(super) const REVIEW_FINDINGS_KIND: &str = "review.findings";
/// The `kind` tag on a streamed verdict payload.
pub(super) const REVIEW_VERDICT_KIND: &str = "review.verdict";

/// Map a content-carrying [`ReviewProgressEvent`] to its `notifications/message`
/// [`LoggingMessageNotificationParam`], or `None` for the progress-tick variants
/// (which route to `notifications/progress` via [`review_progress_param`]).
///
/// The two content shapes carry the FULL structured payload — never a summary,
/// never truncated (a finding is streamed as complete `Finding` JSON):
///
/// - [`Findings`](ReviewProgressEvent::Findings) →
///   `{"kind": "review.findings", "validator": …, "findings": [Finding…]}`
/// - [`Verdict`](ReviewProgressEvent::Verdict) →
///   `{"kind": "review.verdict", "finding": Finding, "confirmed": …, "reason": …}`
///
/// Logger `"review"`, level [`Info`](LoggingLevel::Info). Serialization of a
/// `Finding`/`Vec<Finding>` is infallible (plain data), so `serde_json::json!`
/// never panics here.
pub(super) fn review_content_log_param(
    event: &ReviewProgressEvent,
) -> Option<LoggingMessageNotificationParam> {
    let data = match event {
        ReviewProgressEvent::Findings {
            validator,
            findings,
        } => serde_json::json!({
            "kind": REVIEW_FINDINGS_KIND,
            "validator": validator,
            "findings": findings,
        }),
        ReviewProgressEvent::Verdict {
            finding,
            confirmed,
            reason,
        } => serde_json::json!({
            "kind": REVIEW_VERDICT_KIND,
            "finding": finding,
            "confirmed": confirmed,
            "reason": reason,
        }),
        _ => return None,
    };
    Some(LoggingMessageNotificationParam {
        level: LoggingLevel::Info,
        logger: Some(REVIEW_LOG_LOGGER.to_string()),
        data,
    })
}

/// The `kind` tag on a content-channel keep-alive payload.
///
/// Sent as a `notifications/message` when a call has a content transport but
/// no re-sendable progress param (the tokenless case): it is distinct from
/// [`REVIEW_FINDINGS_KIND`]/[`REVIEW_VERDICT_KIND`] so a consumer accumulating
/// findings never mistakes a keep-alive for a duplicate review event.
pub(super) const REVIEW_KEEP_ALIVE_KIND: &str = "review.keep-alive";

/// The content-channel keep-alive `notifications/message` payload.
///
/// Carried on the same logger/level as the review content stream so any
/// client already listening for review messages receives it — its only job is
/// producing wire traffic that resets the client's tool timeout during engine
/// silence when there is no progress param to re-send.
pub(super) fn review_keep_alive_log_param() -> LoggingMessageNotificationParam {
    LoggingMessageNotificationParam {
        level: LoggingLevel::Info,
        logger: Some(REVIEW_LOG_LOGGER.to_string()),
        data: serde_json::json!({
            "kind": REVIEW_KEEP_ALIVE_KIND,
            "message": "review in progress",
        }),
    }
}

/// Send one review content notification to the MCP peer as a
/// `notifications/message`, logging the full payload first (never truncated) so
/// the send path is provable from the log. A failed send is logged at WARN and
/// swallowed — content streaming is advisory, never load-bearing.
pub(super) async fn send_review_content_log(
    peer: &Peer<RoleServer>,
    param: LoggingMessageNotificationParam,
) {
    tracing::info!(
        logger = param.logger.as_deref().unwrap_or(""),
        data = %param.data,
        "sending review content notifications/message to MCP peer"
    );
    if let Err(err) = peer
        .send_notification(LoggingMessageNotification::new(param).into())
        .await
    {
        tracing::warn!(
            error = %err,
            "failed to send MCP review content notification — peer may have disconnected"
        );
    }
}

/// The two live halves of a review progress bridge: the engine-facing sender
/// and the drain task flushing mapped notifications to the client.
#[derive(Debug)]
pub struct ReviewProgressBridge {
    /// The sender to thread into [`run_review_request`]; dropping it (when the
    /// pipeline finishes) winds the whole bridge down. Consumed through
    /// [`into_parts`](Self::into_parts); private so the bridge's layout is not
    /// a field-level API commitment.
    sender: ReviewProgressSender,
    /// The drain task flushing both wire channels — mapped
    /// `notifications/progress` params to the MCP peer or in-process sink, and
    /// content `notifications/message` params to the peer. Await it after the
    /// run so buffered notifications flush before the final result returns.
    /// Consumed through [`into_parts`](Self::into_parts); private for the same
    /// reason as [`sender`](Self::into_parts).
    drain: tokio::task::JoinHandle<()>,
}

impl ReviewProgressBridge {
    /// Consume the bridge into its two halves: the engine-facing sender and the
    /// drain task.
    ///
    /// The caller hands the sender into [`run_review_request`] (which drops it
    /// when the pipeline finishes) and awaits the drain afterwards so buffered
    /// notifications flush before the final result returns — both halves are
    /// used by value, so the honest accessor is a by-value split.
    pub fn into_parts(self) -> (ReviewProgressSender, tokio::task::JoinHandle<()>) {
        (self.sender, self.drain)
    }
}

/// Which transports one review call's streaming bridge drives, derived from
/// the call's (`progressToken`, in-process sink, MCP peer) triple.
///
/// The two wire channels have DIFFERENT gates, per the MCP spec:
/// `notifications/progress` must echo a client-supplied token, so progress
/// ticks are token-gated; `notifications/message` carries no token, so review
/// content flows to a peer with or without one. The in-process sink is typed
/// for progress params only and never carries content. When both sink and
/// peer are present the sink takes priority (the explicit in-process opt-in,
/// matching `code_context`'s `rebuild index`) and content is suppressed
/// entirely — neither carried on the sink nor leaked to a peer the caller did
/// not choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReviewBridgePlan {
    /// No transport can carry anything — no bridge, zero notifications.
    /// Covers no-peer-and-no-sink, and a sink without a token (sink params
    /// are token-keyed, and the sink never carries content).
    Nothing,
    /// Token + in-process sink: progress params to the sink, no content.
    SinkProgressOnly,
    /// Token + MCP peer (no sink): both channels to the peer.
    PeerProgressAndContent,
    /// Peer without a token (no sink): content-only streaming — progress
    /// ticks are disabled (no token to echo), while findings/verdicts and the
    /// keep-alive flow as `notifications/message`.
    PeerContentOnly,
}

/// Derive the [`ReviewBridgePlan`] for one call's transport triple.
///
/// Kept as a pure function of the three booleans so the transport table is
/// unit-testable without constructing a real `Peer<RoleServer>`.
pub(super) fn review_bridge_plan(
    has_token: bool,
    has_sink: bool,
    has_peer: bool,
) -> ReviewBridgePlan {
    match (has_token, has_sink, has_peer) {
        (true, true, _) => ReviewBridgePlan::SinkProgressOnly,
        (true, false, true) => ReviewBridgePlan::PeerProgressAndContent,
        (false, false, true) => ReviewBridgePlan::PeerContentOnly,
        (false, true, _) | (_, false, false) => ReviewBridgePlan::Nothing,
    }
}

/// Forward review content params to the MCP peer as `notifications/message`
/// until the channel closes, mirroring the progress `spawn_drain_task`.
pub(super) fn spawn_content_drain_task(
    peer: Arc<Peer<RoleServer>>,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<LoggingMessageNotificationParam>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(param) = rx.recv().await {
            send_review_content_log(&peer, param).await;
        }
    })
}

/// Wire the review streaming bridge for one tool call.
///
/// The bridge is built whenever ANY transport can carry something — see
/// [`review_bridge_plan`] for the full table. In particular an MCP peer
/// WITHOUT a `progressToken` still gets a bridge: content
/// (`notifications/message`) and its keep-alive are not token-gated, only
/// `notifications/progress` is. That call is logged at WARN (once) because
/// the client is forgoing progress ticks. Only when no transport can carry
/// anything does this return `None` and the review emits zero notifications.
///
/// Must be called on the OUTER runtime (it spawns the mapping and drain
/// tasks there) BEFORE [`run_review_request`] enters its `spawn_blocking`;
/// only the returned synchronous sender crosses into the nested runtime.
pub fn spawn_review_progress_bridge(context: &ToolContext) -> Option<ReviewProgressBridge> {
    let token = context.progress_token.clone();
    let plan = review_bridge_plan(
        token.is_some(),
        context.progress_sink.is_some(),
        context.peer.is_some(),
    );

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<ReviewProgressEvent>();
    let (param_tx, param_rx) = tokio::sync::mpsc::unbounded_channel::<ProgressNotificationParam>();
    let (content_tx, content_rx) =
        tokio::sync::mpsc::unbounded_channel::<LoggingMessageNotificationParam>();

    let (progress_drain, content_drain) = match plan {
        ReviewBridgePlan::Nothing => {
            match (&token, context.progress_sink.is_some()) {
                (Some(_), _) => tracing::warn!(
                    "review: progressToken present but no MCP peer or progress sink — emitting no progress"
                ),
                (None, true) => tracing::warn!(
                    "review: progress sink present but no progressToken — the sink carries token-keyed progress params only, emitting no notifications"
                ),
                (None, false) => {}
            }
            return None;
        }
        ReviewBridgePlan::SinkProgressOnly => {
            let sink = context.progress_sink.clone().expect("plan implies a sink");
            tracing::debug!(?token, "review: wiring progress bridge to in-process sink");
            (Some(spawn_in_process_drain_task(sink, param_rx)), None)
        }
        ReviewBridgePlan::PeerProgressAndContent => {
            let peer = context.peer.clone().expect("plan implies a peer");
            tracing::debug!(?token, "review: wiring progress bridge to MCP peer");
            (
                Some(spawn_drain_task(Arc::clone(&peer), param_rx)),
                Some(spawn_content_drain_task(peer, content_rx)),
            )
        }
        ReviewBridgePlan::PeerContentOnly => {
            let peer = context.peer.clone().expect("plan implies a peer");
            tracing::warn!(
                "review: MCP peer present but the call carries no _meta.progressToken — \
                 notifications/progress ticks are disabled for this call; streaming review \
                 content (findings/verdicts) and keep-alives as notifications/message only"
            );
            (None, Some(spawn_content_drain_task(peer, content_rx)))
        }
    };
    // A channel whose drain was not spawned has no receiver: hand the mapping
    // task `None` so it never routes anything there.
    let content_tx = content_drain.is_some().then_some(content_tx);

    // The mapping task owns the cumulative counters and both wire channels: it
    // maps progress-tick variants to `param_tx` (drained to sink or peer, when
    // a token exists) and content variants to `content_tx` (drained to the
    // peer as `notifications/message`). It ends when the engine drops its
    // sender; dropping `param_tx`/`content_tx` then closes the drains' sources
    // so they flush and exit.
    tokio::spawn(run_review_progress_mapping(
        event_rx,
        param_tx,
        content_tx,
        token,
        REVIEW_PROGRESS_KEEP_ALIVE_INTERVAL,
    ));

    // One awaitable handle covering however many drains this plan spawned, so
    // the caller's flush-before-return contract stays a single `.await`.
    let drain = tokio::spawn(async move {
        if let Some(handle) = progress_drain {
            let _ = handle.await;
        }
        if let Some(handle) = content_drain {
            let _ = handle.await;
        }
    });

    Some(ReviewProgressBridge {
        sender: event_tx,
        drain,
    })
}

/// How long the bridge tolerates engine silence before re-sending the latest
/// wire param as a keep-alive.
///
/// Clients (Claude Code among them) reset their MCP tool timeout on every
/// `notifications/progress` they receive, so a long silent stretch — the scope
/// stage's whole-set diff + probe pass, one long agent turn, the verify stage —
/// must still produce periodic notifications or the call is aborted as dead.
/// Ten seconds is frequent enough to hold any realistic client timeout and far
/// too infrequent to matter as traffic.
pub(super) const REVIEW_PROGRESS_KEEP_ALIVE_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(10);

/// The bridge's mapping loop: map engine events to wire params on both
/// channels, and re-send a keep-alive whenever the WIRE stays silent for
/// `keep_alive`.
///
/// Progress-tick variants are token-gated (`token: None` drops them — there
/// is no client token to echo on a `notifications/progress`); content
/// variants flow to `content_tx` whenever it exists, token or not.
///
/// The keep-alive window measures time since the last WIRE SEND on either
/// channel — not since the last engine event, because a tokenless call's
/// progress ticks produce no traffic and must not silence the timer. On
/// expiry it re-sends the exact latest progress param when one exists
/// (identical `progress`/`total`/`message`, so wire monotonicity is preserved
/// by construction); otherwise it emits a [`review_keep_alive_log_param`]
/// content message, so a tokenless client sees traffic within `keep_alive` of
/// its first event. Before anything has been sent there is nothing to
/// re-assure the client about, so the timer stays disarmed.
///
/// The loop ends when the engine drops its sender (or a drain side closes);
/// dropping `param_tx`/`content_tx` then closes the drains' sources so they
/// flush and exit.
///
/// Extracted from [`spawn_review_progress_bridge`] so the keep-alive schedule
/// is unit-testable under paused time.
pub(super) async fn run_review_progress_mapping(
    mut event_rx: tokio::sync::mpsc::UnboundedReceiver<ReviewProgressEvent>,
    param_tx: tokio::sync::mpsc::UnboundedSender<ProgressNotificationParam>,
    content_tx: Option<tokio::sync::mpsc::UnboundedSender<LoggingMessageNotificationParam>>,
    token: Option<ProgressToken>,
    keep_alive: std::time::Duration,
) {
    let mut state = ReviewProgressState::default();
    let mut latest: Option<ProgressNotificationParam> = None;
    // When the last wire send happened on either channel; `None` until the
    // first send keeps the keep-alive disarmed.
    let mut last_sent: Option<tokio::time::Instant> = None;
    loop {
        let deadline = last_sent.map(|at| at + keep_alive);
        tokio::select! {
            event = event_rx.recv() => {
                let Some(event) = event else { break };
                // Content-carrying events (findings, verdicts) route to
                // notifications/message whenever a content transport exists —
                // regardless of any progress token (per the MCP spec only
                // notifications/progress needs a client-supplied token). With
                // no content transport (the in-process sink path), they are
                // skipped: the sink contract is progress params.
                if let Some(log_param) = review_content_log_param(&event) {
                    if let Some(tx) = content_tx.as_ref() {
                        if tx.send(log_param).is_err() {
                            break;
                        }
                        last_sent = Some(tokio::time::Instant::now());
                    }
                    continue;
                }
                // Progress-tick variants are token-gated: with no token there
                // is no notifications/progress to emit, so the tick is dropped
                // (content and the keep-alive still flow above/below).
                let Some(token) = token.as_ref() else { continue };
                let Some(param) = review_progress_param(&mut state, token, &event) else {
                    continue;
                };
                latest = Some(param.clone());
                if param_tx.send(param).is_err() {
                    break;
                }
                last_sent = Some(tokio::time::Instant::now());
            }
            // Keep-alive: fires `keep_alive` after the last wire send on
            // either channel; disarmed until the first send exists. (The
            // sleep future is only polled when armed — the `unwrap_or_else`
            // merely satisfies evaluation of a disabled branch.)
            _ = tokio::time::sleep_until(deadline.unwrap_or_else(tokio::time::Instant::now)),
                if deadline.is_some() =>
            {
                if let Some(param) = latest.clone() {
                    if param_tx.send(param).is_err() {
                        break;
                    }
                } else if let Some(tx) = content_tx.as_ref() {
                    if tx.send(review_keep_alive_log_param()).is_err() {
                        break;
                    }
                }
                last_sent = Some(tokio::time::Instant::now());
            }
        }
    }
}
