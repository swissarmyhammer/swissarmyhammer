//! The settle/debounce engine for diagnostics quiescence.
//!
//! Language servers re-flow diagnostics as they analyze: a single edit can
//! produce a burst of `publishDiagnostics` updates for the same document as the
//! server narrows in on the final answer. Reporting any one of those
//! intermediate re-flows would be misleading. This module watches the session's
//! in-process diagnostics fan-out for a set of uris and only emits once the
//! stream has gone quiet for a debounce window — so a consumer sees the
//! *settled* set, never a mid-analysis snapshot.
//!
//! ## Shape
//!
//! - [`settle`] is the session-facing entry point: it subscribes to the
//!   fan-out, seeds the initial per-uri state from the session's latest-per-uri
//!   cache, and runs the settle loop.
//! - [`settle_stream`] is the pure core: it is driven entirely by a
//!   [`broadcast::Receiver<DiagnosticUpdate>`] and an injectable [`Timer`], so
//!   tests drive it with a raw channel and a virtual clock — no LSP server, no
//!   real time, deterministic and fast.
//!
//! The timer is a trait so the debounce and hard-timeout clocks can be driven
//! deterministically in tests. Production uses [`TokioTimer`], which is just
//! `tokio::time::sleep`.

use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use lsp_types::Diagnostic;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use swissarmyhammer_lsp::client::LspTransport;
use swissarmyhammer_lsp::{file_path_from_uri, DiagnosticUpdate, LspSession};

use crate::config::DiagnosticsConfig;
use crate::record::{map, DiagnosticRecord};

/// A one-shot timer source, injectable so the settle loop's clocks can be
/// driven deterministically in tests.
///
/// Each call returns a future that completes after `dur`. Production wires this
/// to `tokio::time::sleep` via [`TokioTimer`]; tests supply a manual virtual
/// clock so a "rapid burst then quiet" stream and a "never quiescing" stream are
/// both reproducible without real time.
pub trait Timer: Send + Sync {
    /// Return a future that completes after `dur` has elapsed on this timer.
    fn sleep(&self, dur: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>>;
}

/// The production [`Timer`]: one-shot sleeps backed by `tokio::time`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioTimer;

impl Timer for TokioTimer {
    fn sleep(&self, dur: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(tokio::time::sleep(dur))
    }
}

/// The result of waiting for diagnostics to settle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettleOutcome {
    /// The stream went quiet within the hard timeout; carries the settled set of
    /// records (latest per uri, filtered and capped per the config).
    Settled(Vec<DiagnosticRecord>),
    /// The stream never quiesced before the hard timeout — a backstop for
    /// pathological analysis. The caller should treat this as "still analyzing".
    Pending,
}

/// Watch the diagnostics fan-out for `uris` and return the settled set.
///
/// Subscribes to the session's broadcast and seeds the initial per-uri state
/// from the session's latest-per-uri cache, so a document that already settled
/// before this call is reported correctly even if no further update arrives.
/// Then defers to [`settle_stream`].
///
/// Subscribing *before* reading the cache means no update can slip through the
/// gap between snapshot and subscription: a duplicate just overwrites the seeded
/// entry with the same or a newer set.
pub async fn settle<C, T>(
    session: &LspSession<C>,
    uris: &[String],
    config: &DiagnosticsConfig,
    timer: &T,
) -> SettleOutcome
where
    C: LspTransport,
    T: Timer,
{
    let rx = session.subscribe();
    // The authoritative latest-per-uri snapshot, read from the session's cache.
    // Used both to seed the initial state and to recover after a broadcast lag —
    // the cache never drops a uri's latest set, whereas the bounded broadcast
    // ring can evict a watched uri's update under churn from other documents.
    let snapshot = || -> BTreeMap<String, Vec<Diagnostic>> {
        uris.iter()
            .map(|uri| (uri.clone(), session.diagnostics_for(uri)))
            .collect()
    };
    let initial = snapshot();
    settle_stream(rx, uris, config, timer, initial, snapshot).await
}

/// The pure settle loop, driven by the fan-out channel and an injectable timer.
///
/// Maintains the latest diagnostics for each watched uri (seeded from
/// `initial`). On every update for a watched uri it replaces that uri's entry
/// and resets the debounce timer. It returns:
///
/// - [`SettleOutcome::Settled`] when no watched-uri update arrives for
///   `config.settle_window` (quiescence), carrying the latest set — never an
///   intermediate re-flow;
/// - [`SettleOutcome::Settled`] immediately when there are no uris to watch, or
///   when the broadcast is closed (the session is gone) — emitting whatever has
///   been collected;
/// - [`SettleOutcome::Pending`] once `config.settle_hard_timeout` elapses, as a
///   backstop against a server that never stops re-flowing.
///
/// Updates for uris outside the watched set are ignored and do **not** reset the
/// debounce, so unrelated churn cannot keep the report pending.
///
/// `resync` is the recovery source called when the broadcast reports `Lagged`
/// (the subscriber fell behind the ring buffer and updates were dropped): it
/// returns the authoritative latest-per-uri snapshot so a dropped watched-uri
/// update cannot make the engine emit a stale set. The session wrapper wires
/// this to the session's per-uri cache; a caller with no recovery source can
/// pass one returning an empty map.
pub async fn settle_stream<T, R>(
    mut rx: broadcast::Receiver<DiagnosticUpdate>,
    uris: &[String],
    config: &DiagnosticsConfig,
    timer: &T,
    initial: BTreeMap<String, Vec<Diagnostic>>,
    resync: R,
) -> SettleOutcome
where
    T: Timer,
    R: Fn() -> BTreeMap<String, Vec<Diagnostic>>,
{
    let watched: HashSet<&str> = uris.iter().map(String::as_str).collect();

    // Latest diagnostics per watched uri. Seed from the cache snapshot, dropping
    // any seed for a uri we are not watching.
    let mut state = filter_to_watched(initial, &watched);

    // Nothing to watch -> the (possibly seeded) set is trivially settled.
    if watched.is_empty() {
        return SettleOutcome::Settled(build_records(&state, config));
    }

    // The debounce resets on each update; the hard timeout is a fixed backstop
    // from the moment we start watching. Both are `Pin<Box<dyn Future>>`, which
    // is `Unpin`, so they can be polled by `&mut` and the debounce can be
    // replaced wholesale on reset.
    let mut debounce = timer.sleep(config.settle_window);
    let mut hard = timer.sleep(config.settle_hard_timeout);

    loop {
        tokio::select! {
            // Biased so the loop drains buffered updates before the debounce can
            // fire, and so the hard timeout always wins a tie — the backstop
            // must not be starved by a continuous re-flow.
            biased;

            _ = &mut hard => return SettleOutcome::Pending,

            update = rx.recv() => match update {
                Ok(u) => {
                    if watched.contains(u.uri.as_str()) {
                        state.insert(u.uri, u.diagnostics);
                        // Reset the quiescence window: a fresh update means the
                        // server is still re-flowing.
                        debounce = timer.sleep(config.settle_window);
                    }
                }
                // Fell behind the ring buffer and dropped updates. The dropped
                // set may include a watched uri's latest (evicted by churn on
                // other documents), so recover the authoritative snapshot from
                // the resync source rather than risk emitting a stale set, then
                // reset the debounce since the stream is clearly still active.
                Err(RecvError::Lagged(_)) => {
                    state = filter_to_watched(resync(), &watched);
                    debounce = timer.sleep(config.settle_window);
                }
                // The session dropped its sender; no more updates will ever
                // come, so emit what we have.
                Err(RecvError::Closed) => {
                    return SettleOutcome::Settled(build_records(&state, config));
                }
            },

            _ = &mut debounce => return SettleOutcome::Settled(build_records(&state, config)),
        }
    }
}

/// Keep only the entries whose uri is in the `watched` set.
///
/// Shared by the initial seed and the post-lag resync so the "drop unwatched
/// uris" rule lives in exactly one place.
fn filter_to_watched(
    map: BTreeMap<String, Vec<Diagnostic>>,
    watched: &HashSet<&str>,
) -> BTreeMap<String, Vec<Diagnostic>> {
    map.into_iter()
        .filter(|(uri, _)| watched.contains(uri.as_str()))
        .collect()
}

/// Flatten the per-uri latest state into a settled report's records.
///
/// Records are produced in a deterministic order (uris sorted, diagnostics in
/// server order), filtered to the severities the config reports, and truncated
/// to `config.per_report_cap`. `containing_symbol` is left `None` — symbol
/// enrichment is a consumer's job, not the settle engine's.
fn build_records(
    state: &BTreeMap<String, Vec<Diagnostic>>,
    config: &DiagnosticsConfig,
) -> Vec<DiagnosticRecord> {
    let mut records = Vec::new();
    for (uri, diagnostics) in state {
        let path = file_path_from_uri(uri);
        for diagnostic in diagnostics {
            let record = map(diagnostic, path.clone());
            if !config.includes_severity(record.severity) {
                continue;
            }
            records.push(record);
            if records.len() >= config.per_report_cap {
                return records;
            }
        }
    }
    records
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use lsp_types::{DiagnosticSeverity as LspSeverity, Position, Range};
    use serde_json::{json, Value};
    use swissarmyhammer_lsp::LspError;
    use tokio::sync::oneshot;

    use crate::config::DEFAULT_SETTLE_HARD_TIMEOUT;

    /// Capacity for the test diagnostics broadcast — comfortably larger than any
    /// burst a single test sends, so `recv()` never lags except where a test
    /// deliberately uses a smaller capacity to force `RecvError::Lagged`.
    const TEST_BROADCAST_CAPACITY: usize = 256;

    /// A do-nothing [`LspTransport`] for tests that drive an [`LspSession`]
    /// purely through its diagnostics cache/fan-out (via
    /// `handle_publish_diagnostics`) with no live server. The session is built
    /// with a `None` client, so these methods are never actually called.
    struct NullTransport;

    impl LspTransport for NullTransport {
        fn send_request(&mut self, _method: &str, _params: Value) -> Result<Value, LspError> {
            Err(LspError::NotRunning)
        }

        fn send_notification(&mut self, _method: &str, _params: Value) -> Result<(), LspError> {
            Err(LspError::NotRunning)
        }

        fn read_message(&mut self) -> Result<Value, LspError> {
            Err(LspError::NotRunning)
        }
    }

    /// A deterministic virtual clock for tests.
    ///
    /// `sleep(dur)` registers a waiter at `now + dur`; [`advance`](Self::advance)
    /// moves the virtual `now` forward and completes every waiter whose deadline
    /// has passed. No real time elapses, so a whole settle scenario runs in
    /// microseconds and is fully reproducible. Cloneable so the test keeps one
    /// handle to drive time while the engine holds another to register sleeps.
    #[derive(Clone, Default)]
    struct ManualTimer {
        inner: Arc<Mutex<ManualInner>>,
    }

    #[derive(Default)]
    struct ManualInner {
        now: Duration,
        waiters: Vec<(Duration, oneshot::Sender<()>)>,
    }

    impl Timer for ManualTimer {
        fn sleep(&self, dur: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            let (tx, rx) = oneshot::channel();
            {
                let mut inner = self.inner.lock().unwrap();
                let deadline = inner.now + dur;
                if deadline <= inner.now {
                    let _ = tx.send(());
                } else {
                    inner.waiters.push((deadline, tx));
                }
            }
            Box::pin(async move {
                let _ = rx.await;
            })
        }
    }

    impl ManualTimer {
        /// Advance the virtual clock and fire every waiter now due.
        fn advance(&self, dur: Duration) {
            let mut inner = self.inner.lock().unwrap();
            inner.now += dur;
            let now = inner.now;
            let mut still_waiting = Vec::new();
            for (deadline, tx) in std::mem::take(&mut inner.waiters) {
                if deadline <= now {
                    let _ = tx.send(());
                } else {
                    still_waiting.push((deadline, tx));
                }
            }
            inner.waiters = still_waiting;
        }
    }

    fn diag(severity: LspSeverity, message: &str) -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 1,
                },
            },
            severity: Some(severity),
            message: message.to_string(),
            ..Diagnostic::default()
        }
    }

    fn update(uri: &str, diagnostics: Vec<Diagnostic>) -> DiagnosticUpdate {
        DiagnosticUpdate {
            uri: uri.to_string(),
            diagnostics,
        }
    }

    /// Spawn the settle loop on the current-thread test runtime, owning the
    /// uris/config/timer so the future is `'static`. The test drives the
    /// returned channel and a clone of the timer.
    ///
    /// The tests in this module require the **current-thread** scheduler (the
    /// `#[tokio::test]` default): the `yield_now()`-then-`advance()` pattern
    /// relies on `tokio::spawn` not polling the settle task until the test task
    /// yields, so the virtual clock is only advanced once the task has parked on
    /// its timers. A multi-thread runtime would race the `advance` against the
    /// task's setup. The `resync` source is a no-op (empty map) here; the
    /// `Lagged` path is covered by its own test below.
    fn spawn_settle(
        rx: broadcast::Receiver<DiagnosticUpdate>,
        uris: Vec<String>,
        config: DiagnosticsConfig,
        timer: ManualTimer,
        initial: BTreeMap<String, Vec<Diagnostic>>,
    ) -> tokio::task::JoinHandle<SettleOutcome> {
        tokio::spawn(async move {
            settle_stream(rx, &uris, &config, &timer, initial, BTreeMap::new).await
        })
    }

    #[tokio::test(flavor = "current_thread")]
    async fn settle_emits_only_the_final_set_after_quiescence() {
        // Three rapid re-flows for one document, then quiet. The settled report
        // must reflect ONLY the final revision, never an intermediate one.
        let (tx, _keepalive) = broadcast::channel::<DiagnosticUpdate>(TEST_BROADCAST_CAPACITY);
        let uri = "file:///src/main.rs".to_string();
        let config = DiagnosticsConfig::default();
        let timer = ManualTimer::default();
        let rx = tx.subscribe();
        let handle = spawn_settle(
            rx,
            vec![uri.clone()],
            config.clone(),
            timer.clone(),
            BTreeMap::new(),
        );

        // Burst of intermediate re-flows; the middle one even carries an extra
        // warning that must NOT appear in the settled set.
        tx.send(update(&uri, vec![diag(LspSeverity::ERROR, "first")]))
            .unwrap();
        tokio::task::yield_now().await;
        tx.send(update(
            &uri,
            vec![
                diag(LspSeverity::ERROR, "second"),
                diag(LspSeverity::WARNING, "transient warning"),
            ],
        ))
        .unwrap();
        tokio::task::yield_now().await;
        tx.send(update(&uri, vec![diag(LspSeverity::ERROR, "final")]))
            .unwrap();
        tokio::task::yield_now().await;

        // Quiescence: advance past the settle window with no further updates.
        timer.advance(config.settle_window);

        match handle.await.unwrap() {
            SettleOutcome::Settled(records) => {
                assert_eq!(records.len(), 1, "only the final revision should remain");
                assert_eq!(records[0].message, "final");
                assert_eq!(records[0].path, "/src/main.rs");
            }
            SettleOutcome::Pending => panic!("expected Settled, got Pending"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn never_quiescing_stream_yields_pending_at_hard_timeout() {
        // Updates keep arriving faster than the settle window, so the debounce
        // never elapses; the hard timeout must fire and yield Pending.
        let (tx, _keepalive) = broadcast::channel::<DiagnosticUpdate>(TEST_BROADCAST_CAPACITY);
        let uri = "file:///src/lib.rs".to_string();
        let config = DiagnosticsConfig {
            settle_window: Duration::from_millis(300),
            settle_hard_timeout: Duration::from_secs(1),
            ..DiagnosticsConfig::default()
        };
        let timer = ManualTimer::default();
        let rx = tx.subscribe();
        let handle = spawn_settle(
            rx,
            vec![uri.clone()],
            config.clone(),
            timer.clone(),
            BTreeMap::new(),
        );

        // Five revisions 200ms apart (< the 300ms window) — debounce resets each
        // time. After the last, now = 1000ms = hard timeout, while the live
        // debounce deadline is 800 + 300 = 1100ms, so the hard timeout wins.
        for i in 0..5 {
            tx.send(update(
                &uri,
                vec![diag(LspSeverity::ERROR, &format!("rev {i}"))],
            ))
            .unwrap();
            tokio::task::yield_now().await;
            timer.advance(Duration::from_millis(200));
            tokio::task::yield_now().await;
        }

        assert_eq!(handle.await.unwrap(), SettleOutcome::Pending);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn seeded_state_settles_when_stream_is_quiet() {
        // A document that already settled before we subscribed: no updates ever
        // arrive, but the seeded cache state must still be reported once the
        // window elapses.
        let (tx, _keepalive) = broadcast::channel::<DiagnosticUpdate>(TEST_BROADCAST_CAPACITY);
        let uri = "file:///src/seeded.rs".to_string();
        let config = DiagnosticsConfig::default();
        let timer = ManualTimer::default();
        let rx = tx.subscribe();
        let mut initial = BTreeMap::new();
        initial.insert(uri.clone(), vec![diag(LspSeverity::ERROR, "from cache")]);
        let handle = spawn_settle(
            rx,
            vec![uri.clone()],
            config.clone(),
            timer.clone(),
            initial,
        );

        // Let the loop park on its timers, then advance past the window.
        tokio::task::yield_now().await;
        timer.advance(config.settle_window);

        match handle.await.unwrap() {
            SettleOutcome::Settled(records) => {
                assert_eq!(records.len(), 1);
                assert_eq!(records[0].message, "from cache");
            }
            SettleOutcome::Pending => panic!("expected Settled, got Pending"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn empty_uris_settles_immediately() {
        // No documents to watch: settle without waiting on any timer.
        let (tx, rx) = broadcast::channel::<DiagnosticUpdate>(TEST_BROADCAST_CAPACITY);
        let config = DiagnosticsConfig::default();
        let timer = ManualTimer::default();
        let outcome = settle_stream(rx, &[], &config, &timer, BTreeMap::new(), BTreeMap::new).await;
        assert_eq!(outcome, SettleOutcome::Settled(vec![]));
        drop(tx);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn closed_channel_settles_with_what_was_collected() {
        // The session goes away (sender dropped) mid-watch: emit the latest
        // collected state rather than blocking until the hard timeout.
        let (tx, rx) = broadcast::channel::<DiagnosticUpdate>(TEST_BROADCAST_CAPACITY);
        let uri = "file:///src/closing.rs".to_string();
        let config = DiagnosticsConfig::default();
        let timer = ManualTimer::default();
        let mut initial = BTreeMap::new();
        initial.insert(uri.clone(), vec![diag(LspSeverity::ERROR, "boom")]);
        let handle = spawn_settle(rx, vec![uri.clone()], config, timer, initial);

        // Drop the only sender; the loop's rx.recv() resolves to Closed.
        drop(tx);

        match handle.await.unwrap() {
            SettleOutcome::Settled(records) => {
                assert_eq!(records.len(), 1);
                assert_eq!(records[0].message, "boom");
            }
            SettleOutcome::Pending => panic!("expected Settled on close, got Pending"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn settled_set_filters_unconfigured_severities() {
        // The default config reports errors + warnings only; an info-severity
        // diagnostic in the final set must be dropped.
        let (tx, _keepalive) = broadcast::channel::<DiagnosticUpdate>(TEST_BROADCAST_CAPACITY);
        let uri = "file:///src/filter.rs".to_string();
        let config = DiagnosticsConfig::default();
        let timer = ManualTimer::default();
        let rx = tx.subscribe();
        let handle = spawn_settle(
            rx,
            vec![uri.clone()],
            config.clone(),
            timer.clone(),
            BTreeMap::new(),
        );

        tx.send(update(
            &uri,
            vec![
                diag(LspSeverity::ERROR, "kept error"),
                diag(LspSeverity::INFORMATION, "dropped info"),
            ],
        ))
        .unwrap();
        tokio::task::yield_now().await;
        timer.advance(config.settle_window);

        match handle.await.unwrap() {
            SettleOutcome::Settled(records) => {
                assert_eq!(records.len(), 1, "info severity should be filtered out");
                assert_eq!(records[0].message, "kept error");
            }
            SettleOutcome::Pending => panic!("expected Settled, got Pending"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn ignores_updates_for_unwatched_uris() {
        // Churn on an unrelated document must not reset the debounce nor leak
        // into the settled set for the watched document.
        let (tx, _keepalive) = broadcast::channel::<DiagnosticUpdate>(TEST_BROADCAST_CAPACITY);
        let watched = "file:///src/watched.rs".to_string();
        let other = "file:///src/other.rs".to_string();
        let config = DiagnosticsConfig::default();
        let timer = ManualTimer::default();
        let rx = tx.subscribe();
        let handle = spawn_settle(
            rx,
            vec![watched.clone()],
            config.clone(),
            timer.clone(),
            BTreeMap::new(),
        );

        tx.send(update(
            &watched,
            vec![diag(LspSeverity::ERROR, "watched error")],
        ))
        .unwrap();
        tokio::task::yield_now().await;
        tx.send(update(
            &other,
            vec![diag(LspSeverity::ERROR, "other error")],
        ))
        .unwrap();
        tokio::task::yield_now().await;
        timer.advance(config.settle_window);

        match handle.await.unwrap() {
            SettleOutcome::Settled(records) => {
                assert_eq!(records.len(), 1);
                assert_eq!(records[0].message, "watched error");
                assert_eq!(records[0].path, "/src/watched.rs");
            }
            SettleOutcome::Pending => panic!("expected Settled, got Pending"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lag_recovers_latest_from_resync_instead_of_emitting_stale_set() {
        // A watched uri's update is evicted from a tiny ring buffer by churn on
        // another document. Draining then yields RecvError::Lagged, and the
        // engine must recover the watched uri's authoritative latest from the
        // resync source rather than settle on a stale (here: empty) set.
        let (tx, _keepalive) = broadcast::channel::<DiagnosticUpdate>(1);
        let watched = "file:///src/watched.rs".to_string();
        let other = "file:///src/other.rs".to_string();
        let config = DiagnosticsConfig::default();
        let timer = ManualTimer::default();
        let rx = tx.subscribe();

        // Stands in for the session cache: it still holds the watched uri's
        // latest even though the broadcast dropped it.
        let watched_for_resync = watched.clone();
        let resync = move || {
            let mut snapshot = BTreeMap::new();
            snapshot.insert(
                watched_for_resync.clone(),
                vec![diag(LspSeverity::ERROR, "recovered latest")],
            );
            snapshot
        };

        let handle = {
            let uris = vec![watched.clone()];
            let config = config.clone();
            let timer = timer.clone();
            tokio::spawn(async move {
                settle_stream(rx, &uris, &config, &timer, BTreeMap::new(), resync).await
            })
        };

        // Overflow the capacity-1 ring before the task drains: the watched
        // update is evicted by the later other-uri update, so the receiver lags.
        tx.send(update(
            &watched,
            vec![diag(LspSeverity::ERROR, "lost update")],
        ))
        .unwrap();
        tx.send(update(&other, vec![diag(LspSeverity::ERROR, "other")]))
            .unwrap();
        tokio::task::yield_now().await;
        timer.advance(config.settle_window);

        match handle.await.unwrap() {
            SettleOutcome::Settled(records) => {
                assert_eq!(
                    records.len(),
                    1,
                    "resync should recover exactly the watched uri"
                );
                assert_eq!(
                    records[0].message, "recovered latest",
                    "after a lag the engine must resync from cache, not emit a stale set"
                );
            }
            SettleOutcome::Pending => panic!("expected Settled, got Pending"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn settle_session_seeds_from_cache_and_settles() {
        // The session-facing entry point: a document that already published
        // diagnostics into the session cache settles to that cached set, with no
        // live LSP client involved.
        let client: Arc<Mutex<Option<NullTransport>>> = Arc::new(Mutex::new(None));
        let session = LspSession::new(client, "rust");
        session.handle_publish_diagnostics(&json!({
            "uri": "file:///src/sess.rs",
            "diagnostics": [
                {
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 4 }
                    },
                    "severity": 1,
                    "message": "cached error"
                }
            ]
        }));

        let timer = ManualTimer::default();
        let handle = {
            let session = session.clone();
            let timer = timer.clone();
            let config = DiagnosticsConfig::default();
            let uris = vec!["file:///src/sess.rs".to_string()];
            tokio::spawn(async move { settle(&session, &uris, &config, &timer).await })
        };

        tokio::task::yield_now().await;
        timer.advance(DiagnosticsConfig::default().settle_window);

        match handle.await.unwrap() {
            SettleOutcome::Settled(records) => {
                assert_eq!(records.len(), 1);
                assert_eq!(records[0].message, "cached error");
                assert_eq!(records[0].path, "/src/sess.rs");
            }
            SettleOutcome::Pending => panic!("expected Settled, got Pending"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn settle_session_captures_update_published_after_subscribe() {
        // Proves the subscribe-before-snapshot ordering in `settle()`: an update
        // that lands after settle() subscribes (and after the initial empty cache
        // snapshot) is still observed and settled — no missed-update gap.
        let client: Arc<Mutex<Option<NullTransport>>> = Arc::new(Mutex::new(None));
        let session = LspSession::new(client, "rust");
        let timer = ManualTimer::default();

        let handle = {
            let session = session.clone();
            let timer = timer.clone();
            let config = DiagnosticsConfig::default();
            let uris = vec!["file:///src/late.rs".to_string()];
            tokio::spawn(async move { settle(&session, &uris, &config, &timer).await })
        };

        // Let settle() subscribe and snapshot the (empty) cache, then publish.
        tokio::task::yield_now().await;
        session.handle_publish_diagnostics(&json!({
            "uri": "file:///src/late.rs",
            "diagnostics": [
                {
                    "range": {
                        "start": { "line": 1, "character": 0 },
                        "end": { "line": 1, "character": 3 }
                    },
                    "severity": 2,
                    "message": "late warning"
                }
            ]
        }));
        tokio::task::yield_now().await;
        timer.advance(DiagnosticsConfig::default().settle_window);

        match handle.await.unwrap() {
            SettleOutcome::Settled(records) => {
                assert_eq!(records.len(), 1);
                assert_eq!(records[0].message, "late warning");
            }
            SettleOutcome::Pending => panic!("expected Settled, got Pending"),
        }
    }

    #[test]
    fn tokio_timer_is_the_default_production_clock() {
        // TokioTimer is the zero-cost production wiring; constructing it must not
        // require a runtime.
        let _timer = TokioTimer;
        assert_eq!(DEFAULT_SETTLE_HARD_TIMEOUT, Duration::from_secs(5));
    }
}
