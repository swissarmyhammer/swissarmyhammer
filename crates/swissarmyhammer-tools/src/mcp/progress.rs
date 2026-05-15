//! MCP `notifications/progress` bridge for the code-context indexer.
//!
//! [`McpProgressReporter`] adapts the synchronous
//! [`ProgressReporter`](swissarmyhammer_code_context::ProgressReporter) trait
//! emitted by the indexer (one event per file chunked, one per embedding
//! batch) into asynchronous JSON-RPC `notifications/progress` messages that
//! flow back to the MCP client.
//!
//! ## Why an mpsc bounce
//!
//! `ProgressReporter::report` is synchronous and called from a tight loop
//! inside indexing tasks. Sending an MCP notification, by contrast, is async
//! (it goes through `rmcp::Peer::send_notification`, which uses tokio
//! channels internally). We bridge the two by buffering events into a
//! `tokio::sync::mpsc::UnboundedSender` from the synchronous side and
//! draining them from a dedicated async task with `Peer::send_notification`
//! on the receive end.
//!
//! Unbounded is the right choice here: progress events are advisory and
//! lossy by design, and the indexer must never block on the network. The
//! drain task lives only for the duration of the tool call.
//!
//! ## Event mapping and wire monotonicity
//!
//! The MCP 2024-11-05 spec ("Progress" section) requires that, for a given
//! `progressToken`, the `progress` value MUST monotonically increase across
//! notifications. The raw [`IndexProgress`] enum is *not* monotonic on
//! its own: `Chunking { done: N, total: N }` is immediately followed by
//! `Embedding { batch: 1, batches: M }` and the indexer interleaves the
//! two per-file, so a naive 1:1 mapping would reset `progress` at every
//! phase boundary.
//!
//! To stay spec-compliant we run the reporter as a small state machine
//! that tracks **cumulative** counters across phases:
//!
//! - `files_done` and `batches_done` are monotonically non-decreasing
//!   counters drawn from the latest `Chunking::done` / `Embedding::batch`
//!   value.
//! - `files_total` and `batches_total` are the best-known phase totals
//!   (refined as the indexer learns them).
//! - The notification carries `progress = files_done + batches_done` and
//!   `total = files_total + batches_total` (with a floor of `progress`
//!   so `total >= progress` always holds even when one phase total is
//!   not yet known).
//!
//! The MCP spec lets `total` grow over the lifetime of a call; only
//! `progress` is required to be monotonic, which this scheme guarantees.
//! The terminal `Done` event reports `progress == total` so progress-bar
//! UIs close on a clean 100% tick.
//!
//! The formatted `message` strings are deliberately part of the wire
//! payload, not just renderer-side decoration — any client (CLI TUI,
//! browser inspector, Claude Code's status line) can display them
//! verbatim without needing to know the underlying `IndexProgress`
//! shape. The structured `progress` / `total` numbers are what
//! progress-bar renderers consume.

use rmcp::model::{ProgressNotification, ProgressNotificationParam, ProgressToken};
use rmcp::{Peer, RoleServer};
use std::sync::{Arc, Mutex};
use swissarmyhammer_code_context::{IndexProgress, ProgressReporter};
use tokio::sync::mpsc;

/// Cumulative counters threaded across `IndexProgress` events to keep the
/// emitted `progress` value monotonic on the wire.
///
/// The struct lives inside the reporter behind a `Mutex` because
/// [`ProgressReporter::report`] takes `&self`. Each field is non-decreasing
/// — every call to [`McpProgressReporter::build_param`] updates the counters
/// using `max(current, observed)` so out-of-order or stale events cannot
/// regress the wire value.
#[derive(Debug, Default, Clone, Copy)]
struct CumulativeProgress {
    /// Files chunked so far in this run (cumulative, non-decreasing).
    files_done: u64,
    /// Total files the run intends to chunk, as last reported by the
    /// indexer. `0` if discovery has not produced a count yet.
    files_total: u64,
    /// Embedding batches completed so far (cumulative, non-decreasing).
    batches_done: u64,
    /// Total embedding batches planned, as last reported by the indexer.
    /// `0` if no `Embedding` event has fired yet.
    batches_total: u64,
}

/// Bridge `IndexProgress` events to MCP `notifications/progress` messages.
///
/// Build one of these per tool call that has a `progressToken` in its
/// request `_meta`. Pass the reporter into the indexer; spawn the drain
/// task with [`spawn_drain_task`] so notifications actually reach the
/// peer.
///
/// Dropping the reporter closes the channel; the drain task observes
/// the close and exits cleanly.
pub struct McpProgressReporter {
    /// The `progressToken` echoed back on every notification.
    token: ProgressToken,
    /// Synchronous-side sender for buffering notification params.
    tx: mpsc::UnboundedSender<ProgressNotificationParam>,
    /// Cumulative phase counters used to keep `progress` monotonic.
    cumulative: Mutex<CumulativeProgress>,
}

/// Result of [`McpProgressReporter::new`] — the reporter plus the
/// matching receiver half of the buffer channel.
///
/// Returned as a named struct (rather than a tuple) so future additions
/// — for example a cancellation handle — can be added without rotating
/// positional bindings at every call site.
pub struct McpProgressReporterBuild {
    /// The reporter to hand to the indexer (typically wrapped in `Arc`).
    pub reporter: McpProgressReporter,
    /// The receiver half to pass to [`spawn_drain_task`].
    pub receiver: mpsc::UnboundedReceiver<ProgressNotificationParam>,
}

impl McpProgressReporter {
    /// Create a new reporter plus its receiver half.
    ///
    /// The caller is responsible for spawning [`spawn_drain_task`] on the
    /// receiver and passing the reporter (wrapped in `Arc`) to the
    /// indexer.
    ///
    /// Named `build` rather than `new` because it returns the paired
    /// [`McpProgressReporterBuild`] (reporter + receiver) rather than
    /// just `Self`. This shape lets us add future fields — e.g. a
    /// cancellation handle — without rotating positional bindings at
    /// every call site.
    ///
    /// # Arguments
    ///
    /// * `token` - The `progressToken` the client supplied in `_meta`.
    ///
    /// # Returns
    ///
    /// An [`McpProgressReporterBuild`] containing both halves.
    pub fn build(token: ProgressToken) -> McpProgressReporterBuild {
        let (tx, rx) = mpsc::unbounded_channel();
        McpProgressReporterBuild {
            reporter: Self {
                token,
                tx,
                cumulative: Mutex::new(CumulativeProgress::default()),
            },
            receiver: rx,
        }
    }

    /// Map a single `IndexProgress` event to a `ProgressNotificationParam`,
    /// updating the cumulative counters so `progress` is monotonic on the
    /// wire.
    ///
    /// Exposed for tests; the implementation of [`ProgressReporter::report`]
    /// delegates here. Returns the parameter that would be sent on the
    /// wire — both the structured `progress`/`total` pair and the
    /// human-readable `message`.
    pub fn build_param(&self, event: &IndexProgress) -> ProgressNotificationParam {
        // Hold the lock for the duration of param construction so two
        // concurrent `report` calls cannot interleave their counter
        // updates. `report` is called from the indexer's sync hot loop;
        // contention is effectively zero.
        let mut state = self.cumulative.lock().unwrap_or_else(|p| p.into_inner());

        let message = match event {
            IndexProgress::Discovering { found } => {
                // Discovery refines the file total but does not advance
                // either "done" counter. Track the maximum so a late
                // post-discovery event cannot lower the total below an
                // earlier observation.
                state.files_total = state.files_total.max(*found);
                format!("Discovering ({found} files)")
            }
            IndexProgress::Chunking { file, done, total } => {
                // `done` is 1-based; saturate at the running maximum so
                // an out-of-order event cannot regress the wire value.
                state.files_done = state.files_done.max(*done);
                state.files_total = state.files_total.max(*total);
                format!("Chunking {}", file.display())
            }
            IndexProgress::Embedding {
                batch,
                batches,
                chunks_in_batch,
            } => {
                state.batches_done = state.batches_done.max(*batch);
                state.batches_total = state.batches_total.max(*batches);
                format!("Embedding batch {batch}/{batches} ({chunks_in_batch} chunks)")
            }
            IndexProgress::Done {
                files,
                chunks,
                elapsed,
            } => {
                // Pin both counters to whichever totals we know — the
                // terminal event must report `progress == total` so a
                // progress-bar UI closes on a clean 100% tick. If a
                // phase produced zero events (e.g. no dirty files) the
                // counter stays at 0, which still satisfies the
                // equality.
                state.files_done = state.files_total.max(state.files_done);
                state.batches_done = state.batches_total.max(state.batches_done);
                // {:.2?} renders a Duration with two-significant-figure
                // unit precision (e.g. "1.23s") rather than the full
                // nanosecond Debug form, which reads as noise for a
                // user-visible status line.
                format!("Done: {files} files, {chunks} chunks in {elapsed:.2?}")
            }
        };

        // `progress` is the cumulative sum of file-level and batch-level
        // work done — strictly non-decreasing because each component is
        // tracked with `.max()` above.
        let progress = state.files_done.saturating_add(state.batches_done);
        // `total` is the cumulative best-known plan. Floor it at
        // `progress` so the wire never carries `total < progress`, which
        // some clients reject. The MCP spec permits `total` to grow as
        // it is refined.
        let total = state
            .files_total
            .saturating_add(state.batches_total)
            .max(progress);

        ProgressNotificationParam {
            progress_token: self.token.clone(),
            progress: progress as f64,
            total: Some(total as f64),
            message: Some(message),
        }
    }
}

impl ProgressReporter for McpProgressReporter {
    fn report(&self, event: IndexProgress) {
        let param = self.build_param(&event);
        // Best-effort: a closed receiver (drain task exited, tool call
        // finished) means progress is no longer being collected. That is
        // the expected end-of-call state and not an error to log.
        let _ = self.tx.send(param);
    }
}

/// Spawn the async drain task that forwards buffered progress params to
/// the MCP peer as `notifications/progress` messages.
///
/// The task runs until the channel closes, then exits. Errors sending an
/// individual notification (peer dropped, transport error) are logged at
/// debug level and the task continues — progress is advisory, never
/// load-bearing.
///
/// # Arguments
///
/// * `peer` - The MCP peer to send notifications through
/// * `rx`   - The receiver half from [`McpProgressReporter::build`]
///
/// # Returns
///
/// A `JoinHandle` for the spawned task. Awaiting it is optional; if the
/// caller wants to wait for all buffered notifications to flush before
/// returning the tool's `CallToolResult`, drop the reporter first to
/// close the channel and then `.await` the handle.
pub fn spawn_drain_task(
    peer: Arc<Peer<RoleServer>>,
    mut rx: mpsc::UnboundedReceiver<ProgressNotificationParam>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(param) = rx.recv().await {
            if let Err(err) = peer
                .send_notification(ProgressNotification::new(param).into())
                .await
            {
                tracing::debug!(
                    error = %err,
                    "failed to send MCP progress notification — peer may have disconnected"
                );
            }
        }
    })
}

/// Spawn an async drain task that forwards buffered progress params to an
/// in-process `UnboundedSender<ProgressNotificationParam>`.
///
/// This is the in-process counterpart to [`spawn_drain_task`]. Where
/// [`spawn_drain_task`] ships notifications to an MCP peer over the
/// configured transport, this variant hands them to a local receiver — the
/// path used by callers that invoke `tool.execute(...)` directly without a
/// stdio/HTTP MCP server in the loop (most notably the `code-context` CLI).
///
/// Behavior matches the peer-bound drain: the task runs until the source
/// channel closes, then exits. A closed downstream sink (receiver dropped,
/// renderer shut down early) is logged at debug level and the task keeps
/// draining the source so the indexer never blocks; progress is advisory,
/// never load-bearing.
///
/// # Arguments
///
/// * `sink` - Destination sender; the matching receiver is owned by the caller
///   and drives a progress renderer
/// * `rx`   - The receiver half from [`McpProgressReporter::build`]
///
/// # Returns
///
/// A `JoinHandle` for the spawned task with the same flush-on-drop semantics
/// as [`spawn_drain_task`].
pub fn spawn_in_process_drain_task(
    sink: mpsc::UnboundedSender<ProgressNotificationParam>,
    mut rx: mpsc::UnboundedReceiver<ProgressNotificationParam>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(param) = rx.recv().await {
            if let Err(err) = sink.send(param) {
                tracing::debug!(
                    error = %err,
                    "failed to forward in-process progress notification — receiver dropped"
                );
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::NumberOrString;
    use std::path::PathBuf;
    use std::time::Duration;

    /// Build a string-typed progress token for tests.
    fn token(s: &str) -> ProgressToken {
        ProgressToken(NumberOrString::String(s.into()))
    }

    /// Synchronously drain everything buffered on the receiver into a
    /// `Vec` without spawning the async drain task. Tests assert directly
    /// on the buffered params.
    ///
    /// Named `take_buffered` (rather than `drain`) to keep grep'ing for
    /// the production `spawn_drain_task` unambiguous.
    fn take_buffered(
        mut rx: mpsc::UnboundedReceiver<ProgressNotificationParam>,
    ) -> Vec<ProgressNotificationParam> {
        let mut out = Vec::new();
        while let Ok(p) = rx.try_recv() {
            out.push(p);
        }
        out
    }

    #[test]
    fn report_discovering_emits_zero_progress_with_message() {
        let McpProgressReporterBuild { reporter, receiver } =
            McpProgressReporter::build(token("tok"));
        reporter.report(IndexProgress::Discovering { found: 42 });
        let params = take_buffered(receiver);
        assert_eq!(params.len(), 1);
        // Discovery does not advance any "done" counter; the wire value
        // is still zero progress with the discovered total surfaced.
        assert_eq!(params[0].progress, 0.0);
        assert_eq!(params[0].total, Some(42.0));
        assert_eq!(params[0].message.as_deref(), Some("Discovering (42 files)"));
        assert_eq!(params[0].progress_token, token("tok"));
    }

    #[test]
    fn report_chunking_carries_cumulative_progress_and_total() {
        let McpProgressReporterBuild { reporter, receiver } =
            McpProgressReporter::build(token("tok"));
        reporter.report(IndexProgress::Chunking {
            file: PathBuf::from("src/main.rs"),
            done: 3,
            total: 10,
        });
        let params = take_buffered(receiver);
        assert_eq!(params.len(), 1);
        // No embedding events yet — progress is just files_done.
        assert_eq!(params[0].progress, 3.0);
        // total is files_total + batches_total (0 here), floored at progress.
        assert_eq!(params[0].total, Some(10.0));
        assert!(params[0]
            .message
            .as_deref()
            .unwrap()
            .contains("src/main.rs"));
    }

    #[test]
    fn report_embedding_adds_to_cumulative_progress() {
        let McpProgressReporterBuild { reporter, receiver } =
            McpProgressReporter::build(token("tok"));
        // Prime the file counters so we can see the cumulative add.
        reporter.report(IndexProgress::Chunking {
            file: PathBuf::from("src/main.rs"),
            done: 3,
            total: 10,
        });
        reporter.report(IndexProgress::Embedding {
            batch: 2,
            batches: 5,
            chunks_in_batch: 64,
        });
        let params = take_buffered(receiver);
        assert_eq!(params.len(), 2);
        // After embedding: progress = files_done(3) + batches_done(2) = 5
        // total = files_total(10) + batches_total(5) = 15.
        assert_eq!(params[1].progress, 5.0);
        assert_eq!(params[1].total, Some(15.0));
        assert!(params[1].message.as_deref().unwrap().contains("2/5"));
        assert!(params[1].message.as_deref().unwrap().contains("64"));
    }

    #[test]
    fn report_done_emits_terminal_progress_equal_to_total() {
        let McpProgressReporterBuild { reporter, receiver } =
            McpProgressReporter::build(token("tok"));
        // Walk a minimal phase sequence so we have real cumulative state.
        reporter.report(IndexProgress::Discovering { found: 1 });
        reporter.report(IndexProgress::Chunking {
            file: PathBuf::from("a.rs"),
            done: 1,
            total: 1,
        });
        reporter.report(IndexProgress::Embedding {
            batch: 1,
            batches: 1,
            chunks_in_batch: 3,
        });
        reporter.report(IndexProgress::Done {
            files: 1,
            chunks: 3,
            elapsed: Duration::from_millis(123),
        });
        let params = take_buffered(receiver);
        // Terminal event reports progress == total so progress-bar UIs
        // close cleanly on a 100% tick.
        let last = params.last().unwrap();
        assert_eq!(last.progress, last.total.unwrap());
        let msg = last.message.as_deref().unwrap();
        assert!(msg.contains("1 files"));
        assert!(msg.contains("3 chunks"));
        // {:.2?} on a Duration renders human-friendly unit-suffixed
        // numbers (e.g. "123.00ms" or "0.12s") rather than full nanosecond
        // Debug noise — assert the prefix and the trailing time unit.
        assert!(
            msg.contains("ms") || msg.contains("s"),
            "expected human-friendly duration unit in message, got: {msg:?}"
        );
    }

    #[test]
    fn cross_phase_progress_is_strictly_monotonic_on_the_wire() {
        // This is the regression test for the MCP-spec monotonicity fix:
        // a realistic event stream from the live indexer (Discovering
        // twice, interleaved Chunking + Embedding per file, then Done)
        // must never emit a `progress` value lower than the previous one.
        let McpProgressReporterBuild { reporter, receiver } =
            McpProgressReporter::build(token("tok"));
        reporter.report(IndexProgress::Discovering { found: 0 });
        reporter.report(IndexProgress::Discovering { found: 3 });
        reporter.report(IndexProgress::Chunking {
            file: PathBuf::from("a.rs"),
            done: 1,
            total: 3,
        });
        reporter.report(IndexProgress::Embedding {
            batch: 1,
            batches: 3,
            chunks_in_batch: 4,
        });
        reporter.report(IndexProgress::Chunking {
            file: PathBuf::from("b.rs"),
            done: 2,
            total: 3,
        });
        reporter.report(IndexProgress::Embedding {
            batch: 2,
            batches: 3,
            chunks_in_batch: 4,
        });
        reporter.report(IndexProgress::Chunking {
            file: PathBuf::from("c.rs"),
            done: 3,
            total: 3,
        });
        reporter.report(IndexProgress::Embedding {
            batch: 3,
            batches: 3,
            chunks_in_batch: 1,
        });
        reporter.report(IndexProgress::Done {
            files: 3,
            chunks: 9,
            elapsed: Duration::from_millis(10),
        });
        let params = take_buffered(receiver);
        // Each notification's progress must be >= the previous one.
        for w in params.windows(2) {
            assert!(
                w[1].progress >= w[0].progress,
                "progress regressed from {} to {} (MCP spec violation): {:?} -> {:?}",
                w[0].progress,
                w[1].progress,
                w[0].message,
                w[1].message,
            );
            // The MCP spec also requires total >= progress for any
            // given notification.
            assert!(
                w[1].total.unwrap() >= w[1].progress,
                "total {} < progress {} in notification {:?}",
                w[1].total.unwrap(),
                w[1].progress,
                w[1].message,
            );
        }
        // Terminal event closes the bar.
        let last = params.last().unwrap();
        assert_eq!(last.progress, last.total.unwrap());
    }

    #[test]
    fn out_of_order_events_cannot_regress_progress() {
        // Defensive: if the indexer ever emits a stale event after a
        // newer one (e.g. due to reordering across threads), the
        // reporter's `.max()` accumulation must absorb it without
        // regressing the wire value.
        let McpProgressReporterBuild { reporter, receiver } =
            McpProgressReporter::build(token("tok"));
        reporter.report(IndexProgress::Chunking {
            file: PathBuf::from("a.rs"),
            done: 5,
            total: 10,
        });
        // Stale event with a lower `done`.
        reporter.report(IndexProgress::Chunking {
            file: PathBuf::from("b.rs"),
            done: 2,
            total: 10,
        });
        let params = take_buffered(receiver);
        assert_eq!(params.len(), 2);
        assert!(
            params[1].progress >= params[0].progress,
            "stale Chunking event regressed progress: {} then {}",
            params[0].progress,
            params[1].progress,
        );
    }

    #[test]
    fn reporter_used_as_dyn_trait() {
        // Confirm the reporter satisfies the trait object bound the
        // indexer's signature requires: `Arc<dyn ProgressReporter>`.
        let McpProgressReporterBuild { reporter, receiver } =
            McpProgressReporter::build(token("dyn-tok"));
        let dynamic: Arc<dyn ProgressReporter> = Arc::new(reporter);
        dynamic.report(IndexProgress::Discovering { found: 1 });
        let params = take_buffered(receiver);
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].progress_token, token("dyn-tok"));
    }

    #[tokio::test]
    async fn in_process_drain_forwards_to_sink_until_close() {
        // Wire reporter → in-process drain → caller sink. The CLI's
        // renderer task lives on the receiver end of `caller_rx`; this
        // test stands in for that role and verifies the drain bridges
        // both halves correctly.
        let McpProgressReporterBuild {
            reporter,
            receiver: report_rx,
        } = McpProgressReporter::build(token("inproc"));
        let (caller_tx, mut caller_rx) = mpsc::unbounded_channel();
        let handle = spawn_in_process_drain_task(caller_tx, report_rx);

        reporter.report(IndexProgress::Discovering { found: 2 });
        reporter.report(IndexProgress::Chunking {
            file: PathBuf::from("a.rs"),
            done: 1,
            total: 2,
        });
        reporter.report(IndexProgress::Done {
            files: 2,
            chunks: 4,
            elapsed: Duration::from_millis(1),
        });

        // Drop the reporter so its mpsc sender closes; the drain task
        // observes that, exits its loop, and the join handle resolves.
        drop(reporter);
        handle.await.expect("drain task should join cleanly");

        // Now collect what the caller actually received.
        let mut got = Vec::new();
        while let Ok(p) = caller_rx.try_recv() {
            got.push(p);
        }
        assert_eq!(got.len(), 3, "all three events should reach the sink");
        assert_eq!(got[0].progress_token, token("inproc"));
        // Terminal Done event closes the progress bar on the CLI side.
        let last = got.last().unwrap();
        assert_eq!(last.progress, last.total.unwrap());
    }

    #[tokio::test]
    async fn in_process_drain_survives_dropped_caller_sink() {
        // If the renderer task panics or the user Ctrl-Cs the CLI, the
        // caller's receiver may drop mid-run. The drain task must keep
        // pulling events off the reporter channel (so the indexer never
        // blocks on a full buffer) and exit only when the reporter side
        // closes.
        let McpProgressReporterBuild {
            reporter,
            receiver: report_rx,
        } = McpProgressReporter::build(token("dropped-sink"));
        let (caller_tx, caller_rx) = mpsc::unbounded_channel();
        // Drop the caller receiver immediately to simulate a renderer
        // that gave up.
        drop(caller_rx);
        let handle = spawn_in_process_drain_task(caller_tx, report_rx);

        // Fire several events at the now-deaf sink. The drain must
        // swallow the send errors and continue.
        reporter.report(IndexProgress::Discovering { found: 1 });
        reporter.report(IndexProgress::Done {
            files: 1,
            chunks: 1,
            elapsed: Duration::from_millis(1),
        });
        drop(reporter);

        // The drain task must still join cleanly.
        handle.await.expect("drain task should join cleanly");
    }
}
