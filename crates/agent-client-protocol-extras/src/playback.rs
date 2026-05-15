//! `PlaybackAgent` — leaf agent that replays a recorded JSONL session over
//! the ACP 0.11 wire.
//!
//! In ACP 0.10 `PlaybackAgent` implemented the now-removed `Agent` trait and
//! returned recorded responses by method name. ACP 0.11 replaces the trait
//! with a Role/Builder/handler model; an "agent" is anything that implements
//! [`ConnectTo<Client>`] and serves JSON-RPC messages on a [`Channel`].
//!
//! `PlaybackAgent` is the **inverse** of [`crate::RecordingAgent`]:
//!
//! ```text
//!     Client  <----[real channel]---->  PlaybackAgent
//!                                       (replays calls.json)
//! ```
//!
//! Unlike [`crate::TracingAgent`] / [`crate::RecordingAgent`] which wrap an
//! inner agent, `PlaybackAgent` is a **leaf** — it terminates the connection.
//! It does not forward to anything; it serves recorded responses directly to
//! whatever client wires up to it.
//!
//! ## On-disk format
//!
//! `PlaybackAgent` consumes the same [`RecordedSession`] schema that
//! [`crate::RecordingAgent`] produces, with method names mapped to their
//! legacy 0.10 form (`initialize`, `new_session`, `prompt`, `load_session`,
//! `set_session_mode`, `cancel`). The wire-level method received from the
//! client is mapped to the legacy name via [`legacy_method_for`] (shared with
//! the recorder) and the next call with that method is replayed in order.
//!
//! ## Replay semantics
//!
//! For each incoming request:
//! 1. Look up the next recorded call. If the call's `method` does not match
//!    the legacy name of the wire method, log a warning but continue using
//!    the recorded call — older fixtures may have benign drift.
//! 2. For every recorded notification, send a `session/update` JSON-RPC
//!    notification back to the client.
//! 3. Send the recorded response keyed by the request's id. If the response
//!    is shaped as `{"error": {…}}` (the recorder's representation of an
//!    error response), emit a JSON-RPC error response instead.
//!
//! Client-originated notifications (no JSON-RPC id, e.g. `session/cancel`)
//! are accepted and silently dropped — they had no recorded response in the
//! first place.

use crate::recording::{legacy_method_for, RecordedSession};
use agent_client_protocol::jsonrpcmsg::{
    Error as JsonRpcError, Id, Message, Params, Request, Response,
};
use agent_client_protocol::{Channel, Client, ConnectTo, Result as AcpResult};
use std::path::PathBuf;
use std::sync::Mutex;
use swissarmyhammer_common::Pretty;

// ---------------------------------------------------------------------------
// PlaybackAgent
// ---------------------------------------------------------------------------

/// Leaf agent that replays a recorded session to whatever client connects.
///
/// `PlaybackAgent::new` loads a `RecordedSession` from a JSON file at
/// construction time. When the agent is `connect_to`'d to a client, every
/// incoming request consumes the next recorded call and replays its
/// notifications and response.
///
/// # Example
///
/// ```ignore
/// use agent_client_protocol_extras::PlaybackAgent;
/// use std::path::PathBuf;
///
/// let agent = PlaybackAgent::new(PathBuf::from("session.json"), "claude");
/// // `agent` is `ConnectTo<Client>` — wire it to a transport (stdio, ByteStreams, …)
/// // or a duplex channel and the client will see the recorded responses.
/// ```
pub struct PlaybackAgent {
    /// All recorded calls, in arrival order.
    session: RecordedSession,
    /// Index of the next call to replay. Behind a mutex so that the
    /// `connect_to` future (which takes `self` by value) can still treat the
    /// cursor as interior-mutable while replaying.
    cursor: Mutex<usize>,
    /// Human-readable tag used for logging — typically the agent type
    /// (`"claude"`, `"llama"`, `"test"`, …).
    agent_type: &'static str,
}

impl PlaybackAgent {
    /// Load a recorded session from `path` and return a `PlaybackAgent` ready
    /// to replay it.
    ///
    /// On read or parse failure, the agent is constructed with an **empty**
    /// session and a warning is logged. This matches the 0.10 behaviour and
    /// keeps the constructor infallible — tests that point at a missing
    /// fixture get a no-op agent rather than a panic during setup.
    ///
    /// # Arguments
    /// * `path` - JSON file produced by [`crate::RecordingAgent`] (or a
    ///   compatible legacy 0.10 fixture)
    /// * `agent_type` - static label used in log lines
    pub fn new(path: PathBuf, agent_type: &'static str) -> Self {
        tracing::info!("PlaybackAgent: Loading from {}", Pretty(&path));

        let session = std::fs::read_to_string(&path)
            .and_then(|content| {
                serde_json::from_str::<RecordedSession>(&content)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            })
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "Failed to load fixture from {}: {}, using empty",
                    Pretty(&path),
                    e
                );
                RecordedSession { calls: vec![] }
            });

        tracing::info!("PlaybackAgent: Loaded {} calls", session.calls.len());

        Self {
            session,
            cursor: Mutex::new(0),
            agent_type,
        }
    }

    /// Static log tag passed at construction.
    pub fn agent_type(&self) -> &'static str {
        self.agent_type
    }

    /// Number of calls in the loaded session — useful for tests asserting
    /// that a fixture was found.
    pub fn call_count(&self) -> usize {
        self.session.calls.len()
    }
}

impl ConnectTo<Client> for PlaybackAgent {
    /// Serve recorded responses to a connected client.
    ///
    /// Drives the client transport, reading every incoming JSON-RPC message
    /// and replying with the next recorded notifications + response. The
    /// future resolves when the client transport closes.
    async fn connect_to(
        self,
        client: impl ConnectTo<<Client as agent_client_protocol::Role>::Counterpart>,
    ) -> AcpResult<()> {
        let (client_channel, client_future) = client.into_channel_and_future();
        let serve = serve_recorded(self, client_channel);

        match futures::try_join!(serve, client_future) {
            Ok(((), ())) => Ok(()),
            Err(err) => Err(err),
        }
    }
}

// ---------------------------------------------------------------------------
// Per-message replay loop
// ---------------------------------------------------------------------------

/// Read incoming messages from the client and replay recorded responses.
///
/// On request: look up the next recorded call, send its notifications, then
/// send the response keyed by the request's id. On client notification:
/// drop silently. On transport error: log and continue — a single decoded
/// error from the client should not tear down playback.
async fn serve_recorded(agent: PlaybackAgent, mut channel: Channel) -> AcpResult<()> {
    use futures::StreamExt;

    let PlaybackAgent {
        session,
        cursor,
        agent_type,
    } = agent;

    while let Some(msg) = channel.rx.next().await {
        match msg {
            Ok(Message::Request(req)) => {
                handle_incoming_request(agent_type, &session, &cursor, &channel.tx, &req)?;
            }
            Ok(Message::Response(resp)) => {
                // PlaybackAgent does not initiate any agent → client requests
                // (yet), so any response coming from the client direction is
                // unexpected. Log and drop.
                tracing::debug!(
                    "[{}] PlaybackAgent: ignoring unexpected response id={:?}",
                    agent_type,
                    resp.id
                );
            }
            Err(err) => {
                tracing::warn!("[{}] PlaybackAgent: transport error: {}", agent_type, err);
            }
        }
    }

    Ok(())
}

/// Replay one request: route notifications first, then the paired response.
fn handle_incoming_request(
    agent_type: &str,
    session: &RecordedSession,
    cursor: &Mutex<usize>,
    tx: &futures::channel::mpsc::UnboundedSender<AcpResult<Message>>,
    req: &Request,
) -> AcpResult<()> {
    let Some(id) = req.id.clone() else {
        // Client-originated notification (e.g. session/cancel). No response
        // expected; drop without consuming a recorded call.
        tracing::debug!(
            "[{}] PlaybackAgent: dropping client notification method={}",
            agent_type,
            req.method
        );
        return Ok(());
    };

    let legacy = legacy_method_for(&req.method);
    let Some(call) = next_call(session, cursor, agent_type, &legacy, &req.method) else {
        // No more recorded calls — emit an internal error so the client sees
        // a deterministic failure rather than a hung future.
        let err = JsonRpcError {
            code: -32603,
            message: format!(
                "PlaybackAgent: no recorded call for method {} (legacy: {})",
                req.method, legacy
            ),
            data: None,
        };
        send_message(tx, Message::Response(Response::error_v2(err, Some(id))))?;
        return Ok(());
    };

    send_recorded_notifications(tx, &call.notifications)?;
    send_recorded_response(tx, &call.response, id)
}

/// Pop the next recorded call, logging a method mismatch as a warning.
///
/// The mismatch is non-fatal: legacy fixtures occasionally diverge from the
/// wire trace (e.g. when a recorder/replayer pair disagrees on whether to
/// record `cancel`). Logging keeps the divergence visible without turning
/// every minor edit into a hard failure.
fn next_call<'a>(
    session: &'a RecordedSession,
    cursor: &Mutex<usize>,
    agent_type: &str,
    legacy: &str,
    wire_method: &str,
) -> Option<&'a crate::recording::RecordedCall> {
    let mut idx = cursor.lock().unwrap();
    let call = session.calls.get(*idx)?;
    if call.method != legacy {
        tracing::warn!(
            "[{}] PlaybackAgent: method mismatch — recorded={}, expected legacy={} for wire={}",
            agent_type,
            call.method,
            legacy,
            wire_method
        );
    }
    *idx += 1;
    Some(call)
}

/// Send each recorded `session/update` notification to the client.
///
/// Empty or non-object recorded entries are skipped with a debug log — the
/// recorder always writes well-formed objects, but legacy fixtures may
/// contain stray nulls.
fn send_recorded_notifications(
    tx: &futures::channel::mpsc::UnboundedSender<AcpResult<Message>>,
    notifications: &[serde_json::Value],
) -> AcpResult<()> {
    for value in notifications {
        let Some(params) = value_to_params(value) else {
            tracing::debug!("PlaybackAgent: skipping non-object notification entry");
            continue;
        };
        let notification = Request::notification_v2("session/update".to_string(), Some(params));
        send_message(tx, Message::Request(notification))?;
    }
    Ok(())
}

/// Send the recorded response, choosing success vs error based on the
/// recorded shape.
fn send_recorded_response(
    tx: &futures::channel::mpsc::UnboundedSender<AcpResult<Message>>,
    response: &serde_json::Value,
    id: Id,
) -> AcpResult<()> {
    let message = if let Some(error) = recorded_error(response) {
        Message::Response(Response::error_v2(error, Some(id)))
    } else {
        Message::Response(Response::success_v2(response.clone(), Some(id)))
    };
    send_message(tx, message)
}

/// Deserialize a recorded response into a JSON-RPC error if it follows the
/// `{"error": {"code", "message", "data"}}` envelope produced by the
/// recorder for failed `ext_method` calls.
fn recorded_error(response: &serde_json::Value) -> Option<JsonRpcError> {
    let envelope = response.get("error")?;
    let code = envelope
        .get("code")
        .and_then(|c| c.as_i64())
        .unwrap_or(-32603) as i32;
    let message = envelope
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("Internal error")
        .to_string();
    let data = envelope.get("data").cloned();
    Some(JsonRpcError {
        code,
        message,
        data,
    })
}

/// Convert a recorded `serde_json::Value` to JSON-RPC `Params` if it can be
/// represented (object or array). Other shapes (null, scalar) are not legal
/// notification params and are filtered out.
fn value_to_params(value: &serde_json::Value) -> Option<Params> {
    match value {
        serde_json::Value::Object(map) => Some(Params::Object(map.clone())),
        serde_json::Value::Array(arr) => Some(Params::Array(arr.clone())),
        _ => None,
    }
}

/// Push a message onto the outgoing side of the client channel.
///
/// A send failure means the client has already disconnected — we surface it
/// as an internal error so the `connect_to` future can wind down.
fn send_message(
    tx: &futures::channel::mpsc::UnboundedSender<AcpResult<Message>>,
    message: Message,
) -> AcpResult<()> {
    tx.unbounded_send(Ok(message))
        .map_err(|e| agent_client_protocol::util::internal_error(e.to_string()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording::RecordedCall;
    use agent_client_protocol::jsonrpcmsg::{Id, Message, Params, Request};
    use futures::channel::mpsc;
    use futures::StreamExt;

    // -- Constructor / loading -----------------------------------------------

    #[test]
    fn new_loads_session_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rec.json");
        std::fs::write(
            &path,
            r#"{"calls":[{"method":"initialize","request":{},"response":{}}]}"#,
        )
        .unwrap();

        let agent = PlaybackAgent::new(path, "test");
        assert_eq!(agent.call_count(), 1);
        assert_eq!(agent.agent_type(), "test");
    }

    #[test]
    fn new_returns_empty_session_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let agent = PlaybackAgent::new(dir.path().join("missing.json"), "test");
        assert_eq!(agent.call_count(), 0);
    }

    #[test]
    fn new_returns_empty_session_when_file_unparseable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not json").unwrap();
        let agent = PlaybackAgent::new(path, "test");
        assert_eq!(agent.call_count(), 0);
    }

    // -- recorded_error: error-envelope detection ----------------------------

    #[test]
    fn recorded_error_recognises_error_envelope() {
        let value = serde_json::json!({
            "error": {"code": -32601, "message": "method not found"}
        });
        let err = recorded_error(&value).unwrap();
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "method not found");
        assert!(err.data.is_none());
    }

    #[test]
    fn recorded_error_carries_data_field() {
        let value = serde_json::json!({
            "error": {
                "code": 42,
                "message": "custom",
                "data": {"detail": "extra"}
            }
        });
        let err = recorded_error(&value).unwrap();
        assert_eq!(err.code, 42);
        assert_eq!(err.data, Some(serde_json::json!({"detail": "extra"})));
    }

    #[test]
    fn recorded_error_returns_none_for_success_shapes() {
        assert!(recorded_error(&serde_json::json!({"sessionId": "s1"})).is_none());
        assert!(recorded_error(&serde_json::Value::Null).is_none());
    }

    #[test]
    fn recorded_error_falls_back_to_internal_when_code_missing() {
        let value = serde_json::json!({"error": {"message": "boom"}});
        let err = recorded_error(&value).unwrap();
        assert_eq!(err.code, -32603);
        assert_eq!(err.message, "boom");
    }

    // -- value_to_params: notification param shape ---------------------------

    #[test]
    fn value_to_params_accepts_object_and_array() {
        let obj = serde_json::json!({"sessionId": "s1"});
        assert!(matches!(value_to_params(&obj), Some(Params::Object(_))));

        let arr = serde_json::json!([1, 2, 3]);
        assert!(matches!(value_to_params(&arr), Some(Params::Array(_))));
    }

    #[test]
    fn value_to_params_rejects_scalars_and_null() {
        assert!(value_to_params(&serde_json::Value::Null).is_none());
        assert!(value_to_params(&serde_json::json!(42)).is_none());
        assert!(value_to_params(&serde_json::json!("string")).is_none());
    }

    // -- next_call: cursor advancement and method-mismatch tolerance ---------

    fn session_with(calls: Vec<RecordedCall>) -> RecordedSession {
        RecordedSession { calls }
    }

    fn dummy_call(method: &str) -> RecordedCall {
        RecordedCall {
            method: method.to_string(),
            request: serde_json::json!({}),
            response: serde_json::json!({}),
            notifications: vec![],
        }
    }

    #[test]
    fn next_call_advances_cursor_in_order() {
        let session = session_with(vec![
            dummy_call("initialize"),
            dummy_call("new_session"),
            dummy_call("prompt"),
        ]);
        let cursor = Mutex::new(0);

        assert_eq!(
            next_call(&session, &cursor, "test", "initialize", "initialize")
                .unwrap()
                .method,
            "initialize"
        );
        assert_eq!(
            next_call(&session, &cursor, "test", "new_session", "session/new")
                .unwrap()
                .method,
            "new_session"
        );
        assert_eq!(
            next_call(&session, &cursor, "test", "prompt", "session/prompt")
                .unwrap()
                .method,
            "prompt"
        );
        assert!(next_call(&session, &cursor, "test", "prompt", "session/prompt").is_none());
    }

    #[test]
    fn next_call_returns_none_when_session_empty() {
        let session = session_with(vec![]);
        let cursor = Mutex::new(0);
        assert!(next_call(&session, &cursor, "test", "initialize", "initialize").is_none());
    }

    #[test]
    fn next_call_warns_but_returns_call_on_method_mismatch() {
        // Mismatches must not block playback — fixtures occasionally drift
        // from the wire trace and we still want the recorded response.
        let session = session_with(vec![dummy_call("prompt")]);
        let cursor = Mutex::new(0);
        let call = next_call(&session, &cursor, "test", "initialize", "initialize").unwrap();
        assert_eq!(call.method, "prompt");
    }

    // -- send_recorded_notifications + send_recorded_response ---------------

    /// Drain all messages currently buffered on `rx` without awaiting more.
    async fn drain(rx: &mut mpsc::UnboundedReceiver<AcpResult<Message>>) -> Vec<Message> {
        let mut out = Vec::new();
        while let Ok(Some(item)) =
            tokio::time::timeout(std::time::Duration::from_millis(20), rx.next()).await
        {
            match item {
                Ok(msg) => out.push(msg),
                Err(e) => panic!("unexpected transport error: {e}"),
            }
        }
        out
    }

    #[tokio::test]
    async fn send_recorded_notifications_emits_session_update_for_each() {
        let (tx, mut rx) = mpsc::unbounded::<AcpResult<Message>>();
        let notifications = vec![
            serde_json::json!({"sessionId": "s1", "update": {"sessionUpdate": "agent_message_chunk"}}),
            serde_json::json!({"sessionId": "s1", "update": {"sessionUpdate": "agent_message_chunk"}}),
        ];

        send_recorded_notifications(&tx, &notifications).unwrap();
        drop(tx);

        let messages = drain(&mut rx).await;
        assert_eq!(messages.len(), 2);
        for msg in &messages {
            let Message::Request(req) = msg else {
                panic!("expected a notification request, got {msg:?}");
            };
            assert_eq!(req.method, "session/update");
            assert!(req.id.is_none(), "notifications must not carry an id");
        }
    }

    #[tokio::test]
    async fn send_recorded_notifications_skips_non_object_entries() {
        let (tx, mut rx) = mpsc::unbounded::<AcpResult<Message>>();
        send_recorded_notifications(
            &tx,
            &[
                serde_json::Value::Null,
                serde_json::json!({"sessionId": "s1"}),
            ],
        )
        .unwrap();
        drop(tx);

        let messages = drain(&mut rx).await;
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn send_recorded_response_emits_success_for_normal_shape() {
        let (tx, mut rx) = mpsc::unbounded::<AcpResult<Message>>();
        let response = serde_json::json!({"sessionId": "s1"});

        send_recorded_response(&tx, &response, Id::Number(7)).unwrap();
        drop(tx);

        let messages = drain(&mut rx).await;
        assert_eq!(messages.len(), 1);
        let Message::Response(resp) = &messages[0] else {
            panic!("expected response");
        };
        assert_eq!(resp.id, Some(Id::Number(7)));
        assert!(resp.error.is_none());
        assert_eq!(resp.result, Some(response));
    }

    #[tokio::test]
    async fn send_recorded_response_emits_error_for_error_envelope() {
        let (tx, mut rx) = mpsc::unbounded::<AcpResult<Message>>();
        let response = serde_json::json!({
            "error": {"code": -32601, "message": "method not found"}
        });

        send_recorded_response(&tx, &response, Id::Number(1)).unwrap();
        drop(tx);

        let messages = drain(&mut rx).await;
        let Message::Response(resp) = &messages[0] else {
            panic!("expected response");
        };
        let err = resp.error.as_ref().expect("error envelope expected");
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "method not found");
    }

    // -- handle_incoming_request: full per-request flow ---------------------

    /// Build a JSON-RPC request with object params for use in tests.
    fn request_with_params(method: &str, params: serde_json::Value, id: u64) -> Request {
        let params = match params {
            serde_json::Value::Object(map) => Some(Params::Object(map)),
            serde_json::Value::Null => None,
            other => panic!("test only uses object/null params, got: {other}"),
        };
        Request::new_v2(method.to_string(), params, Some(Id::Number(id)))
    }

    #[tokio::test]
    async fn handle_incoming_request_replays_notifications_then_response() {
        let session = session_with(vec![RecordedCall {
            method: "prompt".to_string(),
            request: serde_json::json!({"sessionId": "s1"}),
            response: serde_json::json!({"stopReason": "end_turn"}),
            notifications: vec![serde_json::json!({
                "sessionId": "s1",
                "update": {"sessionUpdate": "agent_message_chunk"}
            })],
        }]);
        let cursor = Mutex::new(0);
        let (tx, mut rx) = mpsc::unbounded::<AcpResult<Message>>();

        let req = request_with_params("session/prompt", serde_json::json!({"sessionId": "s1"}), 5);
        handle_incoming_request("test", &session, &cursor, &tx, &req).unwrap();
        drop(tx);

        let messages = drain(&mut rx).await;
        assert_eq!(messages.len(), 2, "notification then response");

        let Message::Request(notif) = &messages[0] else {
            panic!("first message should be the recorded notification");
        };
        assert_eq!(notif.method, "session/update");
        assert!(notif.id.is_none());

        let Message::Response(resp) = &messages[1] else {
            panic!("second message should be the response");
        };
        assert_eq!(resp.id, Some(Id::Number(5)));
        assert_eq!(
            resp.result,
            Some(serde_json::json!({"stopReason": "end_turn"}))
        );
    }

    #[tokio::test]
    async fn handle_incoming_request_drops_client_notifications() {
        // session/cancel is a notification (no id) — no recorded call should
        // be consumed and no response should be emitted.
        let session = session_with(vec![dummy_call("prompt")]);
        let cursor = Mutex::new(0);
        let (tx, mut rx) = mpsc::unbounded::<AcpResult<Message>>();

        let cancel = Request::notification_v2(
            "session/cancel".to_string(),
            Some(Params::Object(serde_json::Map::new())),
        );
        handle_incoming_request("test", &session, &cursor, &tx, &cancel).unwrap();
        drop(tx);

        assert!(drain(&mut rx).await.is_empty());
        assert_eq!(*cursor.lock().unwrap(), 0, "cursor must not advance");
    }

    #[tokio::test]
    async fn handle_incoming_request_emits_internal_error_when_session_exhausted() {
        let session = session_with(vec![]);
        let cursor = Mutex::new(0);
        let (tx, mut rx) = mpsc::unbounded::<AcpResult<Message>>();

        let req = request_with_params("initialize", serde_json::json!({}), 1);
        handle_incoming_request("test", &session, &cursor, &tx, &req).unwrap();
        drop(tx);

        let messages = drain(&mut rx).await;
        let Message::Response(resp) = &messages[0] else {
            panic!("expected error response");
        };
        let err = resp.error.as_ref().expect("error expected");
        assert_eq!(err.code, -32603);
        assert!(err.message.contains("no recorded call"));
    }

    // -- End-to-end: PlaybackAgent + Channel duplex --------------------------

    /// A mini "client" component that just hands its channel back to the
    /// caller through a oneshot. Lets the test drive the agent's wire-side
    /// directly without needing a real client builder.
    ///
    /// `PlaybackAgent` is `ConnectTo<Client>` — its `connect_to` expects
    /// something `ConnectTo<Client::Counterpart>` (i.e. `ConnectTo<Agent>`).
    /// `ChannelExposer` therefore implements the agent-side trait.
    struct ChannelExposer {
        send: tokio::sync::oneshot::Sender<Channel>,
    }

    impl ConnectTo<agent_client_protocol::Agent> for ChannelExposer {
        async fn connect_to(
            self,
            agent: impl ConnectTo<
                <agent_client_protocol::Agent as agent_client_protocol::Role>::Counterpart,
            >,
        ) -> AcpResult<()> {
            // We are the agent's "client" — to drive it, we ask the agent
            // for its channel-and-future and ship the channel back to the
            // test.
            let (channel, agent_future) = agent.into_channel_and_future();
            let _ = self.send.send(channel);
            agent_future.await
        }
    }

    #[tokio::test]
    async fn playback_agent_replays_initialize_via_connect_to() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rec.json");
        let session = serde_json::json!({
            "calls": [
                {
                    "method": "initialize",
                    "request": {"protocolVersion": 1},
                    "response": {"protocolVersion": 1},
                    "notifications": []
                }
            ]
        });
        std::fs::write(&path, serde_json::to_string(&session).unwrap()).unwrap();

        let agent = PlaybackAgent::new(path, "test");
        let (channel_tx, channel_rx) = tokio::sync::oneshot::channel();
        let exposer = ChannelExposer { send: channel_tx };

        let agent_task = tokio::spawn(async move { agent.connect_to(exposer).await });

        let mut channel = channel_rx.await.expect("channel from agent");

        // Send an initialize request — expect a recorded response back.
        let req = request_with_params("initialize", serde_json::json!({"protocolVersion": 1}), 1);
        channel
            .tx
            .unbounded_send(Ok(Message::Request(req)))
            .unwrap();

        let response = tokio::time::timeout(std::time::Duration::from_secs(1), channel.rx.next())
            .await
            .expect("response did not arrive in time")
            .expect("channel closed before response")
            .expect("transport error");
        let Message::Response(resp) = response else {
            panic!("expected response, got {response:?}");
        };
        assert_eq!(resp.id, Some(Id::Number(1)));
        assert_eq!(resp.result, Some(serde_json::json!({"protocolVersion": 1})));

        // Closing the client tx ends the agent's serve loop.
        drop(channel);
        let _ = agent_task.await;
    }
}
