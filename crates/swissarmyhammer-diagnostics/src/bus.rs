//! Cross-process diagnostics fan-out over the leader-election pub/sub bus.
//!
//! The in-process fan-out ([`LspSession::subscribe`](swissarmyhammer_lsp::LspSession::subscribe))
//! broadcasts a [`DiagnosticUpdate`](swissarmyhammer_lsp::DiagnosticUpdate) to
//! consumers *inside the leader process*. A follower process spawns no LSP
//! server of its own (see `^7a5h2bj`), so it cannot observe that stream. This
//! module extends the fan-out across the process boundary: the leader tees its
//! in-process diagnostics stream onto the **existing** ZMQ pub/sub bus
//! (`Publisher`/`Subscriber`/`BusMessage` in
//! [`swissarmyhammer_leader_election`]), and followers subscribe to receive the
//! same per-uri updates.
//!
//! This reuses the one bus — it does **not** add a second transport. It is also
//! distinct from the request/response IPC in
//! [`swissarmyhammer_leader_election::request_ipc`] (used by the
//! [`request_api`](crate::request_api) follower→leader RPC): that channel carries
//! correlated replies, this one is fire-and-forget broadcast.

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use swissarmyhammer_leader_election::{BusMessage, ElectionError};
use swissarmyhammer_lsp::{file_path_from_uri, DiagnosticUpdate};

use crate::record::{map, DiagnosticRecord};

/// The ZMQ topic every diagnostics update rides under.
///
/// A single topic (rather than per-uri topics) keeps the subscription filter
/// constant: a follower subscribes once to [`DIAGNOSTICS_TOPIC`] and receives
/// every document's updates, demultiplexing by the [`uri`](DiagnosticsBusMessage::uri)
/// carried in each message.
pub const DIAGNOSTICS_TOPIC: &[u8] = b"diagnostics";

/// One per-uri diagnostics update, serialized for the leader-election bus.
///
/// This is the cross-process mirror of
/// [`DiagnosticUpdate`](swissarmyhammer_lsp::DiagnosticUpdate): it carries the
/// *latest complete* set of diagnostics for `uri` (diagnostics are a full
/// replacement per document, never a delta). The leader builds one from each
/// in-process update — mapping `lsp_types::Diagnostic` to the model-free,
/// serde-ready [`DiagnosticRecord`] — and publishes it; a follower receives it
/// and applies it to its own view, keyed by `uri`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsBusMessage {
    /// The document URI the diagnostics apply to (e.g. `file:///src/main.rs`).
    pub uri: String,
    /// The latest complete set of diagnostics for `uri`.
    pub diagnostics: Vec<DiagnosticRecord>,
}

impl DiagnosticsBusMessage {
    /// Build a message for `uri` carrying its latest full diagnostics set.
    pub fn new(uri: impl Into<String>, diagnostics: Vec<DiagnosticRecord>) -> Self {
        Self {
            uri: uri.into(),
            diagnostics,
        }
    }
}

/// Convert an in-process [`DiagnosticUpdate`] into a bus message.
///
/// This is the pure bridge from the in-process fan-out to the cross-process
/// payload: each `lsp_types::Diagnostic` is mapped to the serde-ready
/// [`DiagnosticRecord`] via [`crate::record::map`], keyed by the file path
/// derived from the update's `file://` uri (so the records' `path` matches the
/// space the in-process consumers use). No I/O, no enrichment.
pub fn message_from_update(update: &DiagnosticUpdate) -> DiagnosticsBusMessage {
    let path = file_path_from_uri(&update.uri);
    let diagnostics = update
        .diagnostics
        .iter()
        .map(|d| map(d, path.clone()))
        .collect();
    DiagnosticsBusMessage::new(update.uri.clone(), diagnostics)
}

/// Re-publish a session's in-process diagnostics fan-out onto the bus.
///
/// The leader owns the one [`LspSession`]; this is the leader-side task that
/// tees that session's in-process [`DiagnosticUpdate`] broadcast across the
/// process boundary. It maps each update to a [`DiagnosticsBusMessage`]
/// ([`message_from_update`]) and hands it to `publish` — which in production is
/// the leader's
/// [`LeaderGuard::publish`](swissarmyhammer_leader_election::LeaderGuard::publish),
/// keeping the reuse of the single bus. The `publish` callback is injected so
/// this loop is unit-testable with a plain capturing closure and a raw channel,
/// no ZMQ proxy required.
///
/// Takes the [`broadcast::Receiver`] directly (the caller does
/// `session.subscribe()`) rather than a session handle, so the loop holds no
/// session clone — keeping a clone would also keep the broadcast `Sender` alive
/// and the loop could never observe [`RecvError::Closed`]. The production caller
/// spawns this on a task that owns only the receiver.
///
/// Runs until the session's broadcast closes (the session was dropped). A
/// broadcast lag (a slow re-publisher under churn) is logged and skipped rather
/// than aborting the loop — diagnostics are full-replacement per uri, so the
/// next update for a lagged uri resyncs it. A `publish` error is logged and the
/// loop continues; losing one update must not tear down the fan-out.
pub async fn fan_out_to_bus<P>(mut rx: broadcast::Receiver<DiagnosticUpdate>, mut publish: P)
where
    P: FnMut(&DiagnosticsBusMessage) -> swissarmyhammer_leader_election::Result<()>,
{
    loop {
        match rx.recv().await {
            Ok(update) => {
                let msg = message_from_update(&update);
                if let Err(e) = publish(&msg) {
                    tracing::warn!(uri = %msg.uri, error = %e, "diagnostics bus publish failed");
                }
            }
            Err(RecvError::Closed) => break,
            Err(RecvError::Lagged(n)) => {
                tracing::warn!(
                    skipped = n,
                    "diagnostics bus re-publisher lagged; skipping evicted updates"
                );
            }
        }
    }
}

/// Open a bus publisher on the leader's frontend and re-publish a session's
/// diagnostics fan-out over it, forever.
///
/// The one call site the leader uses: it builds a typed
/// [`Publisher`](swissarmyhammer_leader_election::Publisher) over the leader's
/// own bus frontend (the public `open` seam — reusing the one proxy, not a
/// second transport) and drives [`fan_out_to_bus`] until the broadcast closes.
/// Keeping this here means the consuming crate (e.g. the MCP server) does not
/// have to name the leader-election `Publisher` type itself — it hands over the
/// frontend address and the receiver, and this owns the wiring.
///
/// Returns `Err` only when the publisher cannot connect; on success it runs to
/// completion (the session being dropped).
pub async fn fan_out_over_bus(
    frontend_addr: &str,
    rx: tokio::sync::broadcast::Receiver<DiagnosticUpdate>,
) -> swissarmyhammer_leader_election::Result<()> {
    let publisher: swissarmyhammer_leader_election::Publisher<DiagnosticsBusMessage> =
        swissarmyhammer_leader_election::Publisher::open(frontend_addr)?;
    fan_out_to_bus(rx, move |msg| publisher.send(msg)).await;
    Ok(())
}

/// Subscribe to the leader's diagnostics broadcast on a follower and hand each
/// received per-uri update to `on_update`, forever.
///
/// The follower-side counterpart to [`fan_out_over_bus`]: a follower process
/// owns no LSP server, so it cannot observe diagnostics in-process — instead it
/// opens a [`Subscriber`](swissarmyhammer_leader_election::Subscriber) on the
/// leader's bus backend (the public `open` seam, filtered to
/// [`DIAGNOSTICS_TOPIC`]) and receives every [`DiagnosticsBusMessage`] the
/// leader publishes. Each decoded message is handed to `on_update`; a decode
/// error is logged and skipped (one malformed frame must not kill the loop).
///
/// `on_update` is the application seam — the follower decides what to do with a
/// per-uri update (e.g. fold it into a follower-side view). The receive loop
/// itself is transport-only, so it is testable with a plain capturing closure.
///
/// **Blocking**: the subscriber's `recv` blocks the calling thread, so callers
/// must run this on a dedicated thread (e.g. `tokio::task::spawn_blocking` or a
/// `std::thread`), not directly on an async runtime worker.
///
/// Returns `Err` only when the subscriber cannot connect; otherwise it runs
/// until the channel disconnects (the leader's proxy is gone).
pub fn subscribe_diagnostics_over_bus<F>(
    backend_addr: &str,
    mut on_update: F,
) -> swissarmyhammer_leader_election::Result<()>
where
    F: FnMut(DiagnosticsBusMessage),
{
    let subscriber: swissarmyhammer_leader_election::Subscriber<DiagnosticsBusMessage> =
        swissarmyhammer_leader_election::Subscriber::open(backend_addr, &[DIAGNOSTICS_TOPIC])?;
    loop {
        match subscriber.recv_timeout(std::time::Duration::from_millis(500)) {
            Some(Ok(msg)) => on_update(msg),
            // A timeout is normal quiet — keep waiting.
            None => continue,
            Some(Err(e)) => {
                // A disconnect ends the loop (the leader's proxy went away);
                // any other recv error is logged and skipped.
                //
                // The fragility of distinguishing a disconnect by string-match
                // here is the SAME mechanism tracked in the accepted follow-up
                // ^343hrm0 (an ipc disconnect surfaces as EAGAIN, not
                // "disconnected", so this loop only ends at teardown).
                // Intentionally not fixed here — see ^343hrm0.
                if e.to_string().contains("disconnected") {
                    break;
                }
                tracing::warn!(error = %e, "diagnostics bus subscriber recv error");
            }
        }
    }
    Ok(())
}

impl BusMessage for DiagnosticsBusMessage {
    fn topic(&self) -> &[u8] {
        DIAGNOSTICS_TOPIC
    }

    fn to_frames(&self) -> swissarmyhammer_leader_election::Result<Vec<Vec<u8>>> {
        // Two frames: the uri (UTF-8 bytes) and the JSON-encoded records. Keeping
        // the uri in its own frame mirrors `HebEvent`'s header/body split and
        // lets a future consumer route on the uri without decoding the records.
        let uri = self.uri.as_bytes().to_vec();
        let records =
            serde_json::to_vec(&self.diagnostics).map_err(ElectionError::Serialization)?;
        Ok(vec![uri, records])
    }

    fn from_frames(
        _topic: &[u8],
        frames: &[Vec<u8>],
    ) -> swissarmyhammer_leader_election::Result<Self> {
        if frames.len() < 2 {
            return Err(ElectionError::Message(
                "DiagnosticsBusMessage requires 2 frames (uri + records)".to_string(),
            ));
        }
        let uri = String::from_utf8(frames[0].clone())
            .map_err(|e| ElectionError::Message(format!("uri frame is not valid UTF-8: {e}")))?;
        let diagnostics: Vec<DiagnosticRecord> =
            serde_json::from_slice(&frames[1]).map_err(ElectionError::Serialization)?;
        Ok(Self { uri, diagnostics })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::Range;
    use swissarmyhammer_lsp::DiagnosticSeverity;

    fn sample_record(message: &str) -> DiagnosticRecord {
        DiagnosticRecord {
            path: "src/main.rs".to_string(),
            range: Range {
                start_line: 5,
                start_character: 10,
                end_line: 5,
                end_character: 20,
            },
            severity: DiagnosticSeverity::Error,
            message: message.to_string(),
            code: Some("E0308".to_string()),
            source: Some("rustc".to_string()),
            containing_symbol: None,
        }
    }

    #[test]
    fn topic_is_the_diagnostics_topic() {
        let msg = DiagnosticsBusMessage::new("file:///src/main.rs", vec![]);
        assert_eq!(msg.topic(), DIAGNOSTICS_TOPIC);
    }

    #[test]
    fn round_trip_preserves_uri_and_records() {
        let msg = DiagnosticsBusMessage::new(
            "file:///src/main.rs",
            vec![sample_record("mismatched types"), sample_record("second")],
        );

        let frames = msg.to_frames().expect("encode");
        assert_eq!(frames.len(), 2);

        let restored =
            DiagnosticsBusMessage::from_frames(DIAGNOSTICS_TOPIC, &frames).expect("decode");
        assert_eq!(restored, msg);
    }

    #[test]
    fn round_trip_with_empty_diagnostics_means_document_is_clean() {
        // An empty set is the "now clean" signal and must survive the round trip.
        let msg = DiagnosticsBusMessage::new("file:///src/lib.rs", vec![]);
        let frames = msg.to_frames().expect("encode");
        let restored =
            DiagnosticsBusMessage::from_frames(DIAGNOSTICS_TOPIC, &frames).expect("decode");
        assert_eq!(restored, msg);
        assert!(restored.diagnostics.is_empty());
    }

    #[test]
    fn from_frames_rejects_too_few_frames() {
        let err = DiagnosticsBusMessage::from_frames(DIAGNOSTICS_TOPIC, &[b"uri".to_vec()])
            .expect_err("one frame must be rejected");
        assert!(err.to_string().contains("2 frames"));
    }

    #[test]
    fn from_frames_rejects_malformed_records_json() {
        let frames = vec![b"file:///x.rs".to_vec(), b"not json".to_vec()];
        let err = DiagnosticsBusMessage::from_frames(DIAGNOSTICS_TOPIC, &frames)
            .expect_err("bad json must be rejected");
        assert!(err.to_string().contains("serialization"));
    }

    #[test]
    fn message_from_update_keys_records_by_path_and_carries_uri() {
        use lsp_types::{Diagnostic, Position};
        let update = DiagnosticUpdate {
            uri: "file:///repo/src/main.rs".to_string(),
            diagnostics: vec![Diagnostic {
                range: lsp_types::Range {
                    start: Position {
                        line: 2,
                        character: 0,
                    },
                    end: Position {
                        line: 2,
                        character: 7,
                    },
                },
                severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                message: "boom".to_string(),
                ..Diagnostic::default()
            }],
        };
        let msg = message_from_update(&update);
        assert_eq!(msg.uri, "file:///repo/src/main.rs");
        assert_eq!(msg.diagnostics.len(), 1);
        // The record path is the file path derived from the uri, not the uri.
        assert_eq!(msg.diagnostics[0].path, "/repo/src/main.rs");
        assert_eq!(msg.diagnostics[0].severity, DiagnosticSeverity::Error);
        assert_eq!(msg.diagnostics[0].message, "boom");
    }

    #[test]
    fn message_from_update_empty_is_clean_signal() {
        let update = DiagnosticUpdate {
            uri: "file:///repo/src/lib.rs".to_string(),
            diagnostics: vec![],
        };
        let msg = message_from_update(&update);
        assert_eq!(msg.uri, "file:///repo/src/lib.rs");
        assert!(msg.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn fan_out_to_bus_publishes_each_in_process_update() {
        use crate::test_support::NullTransport;
        use serde_json::json;
        use std::sync::{Arc, Mutex};
        use swissarmyhammer_lsp::LspSession;

        let client: Arc<Mutex<Option<NullTransport>>> = Arc::new(Mutex::new(None));
        let session = LspSession::new(client, "rust");

        // Capture every message the fan-out hands to `publish`.
        let captured: Arc<Mutex<Vec<DiagnosticsBusMessage>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&captured);

        // Subscribe up front, then run the fan-out over just the receiver — it
        // holds no session clone, so dropping the session closes the broadcast
        // and ends the loop.
        let rx = session.subscribe();
        let handle = tokio::spawn(async move {
            fan_out_to_bus(rx, move |msg| {
                sink.lock().unwrap().push(msg.clone());
                Ok(())
            })
            .await;
        });

        // Let the subscriber attach before publishing.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        session.handle_publish_diagnostics(&json!({
            "uri": "file:///repo/src/main.rs",
            "diagnostics": [{
                "range": { "start": { "line": 1, "character": 0 }, "end": { "line": 1, "character": 5 } },
                "severity": 1,
                "message": "boom"
            }]
        }));
        // An empty publish — the "now clean" signal — must also fan out.
        session.handle_publish_diagnostics(&json!({
            "uri": "file:///repo/src/lib.rs",
            "diagnostics": []
        }));

        // Drop every session handle so the broadcast closes and the loop ends.
        drop(session);
        handle.await.expect("fan-out task joins");

        let got = captured.lock().unwrap();
        assert_eq!(got.len(), 2, "both updates fan out");
        assert_eq!(got[0].uri, "file:///repo/src/main.rs");
        assert_eq!(got[0].diagnostics.len(), 1);
        assert_eq!(got[0].diagnostics[0].path, "/repo/src/main.rs");
        assert_eq!(got[1].uri, "file:///repo/src/lib.rs");
        assert!(got[1].diagnostics.is_empty());
    }
}
