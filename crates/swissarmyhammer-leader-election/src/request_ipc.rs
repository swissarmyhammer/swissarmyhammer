//! Request/response IPC over the leader-election socket.
//!
//! The pub/sub bus ([`crate::proxy`] / [`crate::bus`]) is fire-and-forget — it
//! cannot carry a *reply*. This module adds the missing half: a correlated
//! request/response channel a follower uses to dispatch a call to the elected
//! leader and get its result back.
//!
//! The leader binds a [`tokio::net::UnixListener`] at the election
//! [`socket_path`](crate::LeaderGuard::socket_path) — the same path that, until
//! now, was only ever written as a liveness marker and removed on drop. A
//! follower connects to that socket as a client. Both speak a tiny correlated
//! framed envelope:
//!
//! ```text
//! request:  {"id": <u64>, "method": "<str>", "params": <json>}
//! response: {"id": <u64>, "result": <json>}        // success
//!           {"id": <u64>, "error":  "<str>"}        // failure
//! ```
//!
//! Each envelope is a single line of JSON terminated by `\n` (newline-delimited
//! framing), so a reader can split the stream into messages without a length
//! prefix and `params`/`result` never contain a raw newline (they are compact
//! JSON values).
//!
//! ## Why this lives here, and why it is generic
//!
//! The transport is deliberately free of any knowledge of *what* it carries:
//! [`RequestServer::serve`] takes a handler closure `(method, params) -> result`,
//! and [`RequestClient::call`] round-trips an opaque `(method, params)`. The SAH
//! request API (`diagnose`, code-context query ops) is layered on top by the
//! consumer crate that owns an [`LspSession`]; the leader-election crate must
//! not depend on it. Keeping the mechanism generic means the deferred
//! rebuild-index follower writes can ride the very same channel later — one
//! request multiplexer, not two.
//!
//! ## Multiplexing and correlation
//!
//! The leader serves many followers concurrently. The client tags every request
//! with a process-unique `id`; the server echoes that `id` on the matching
//! response. Because requests on a single connection may complete out of order
//! (the leader handles them concurrently), [`RequestClient`] demuxes by `id`:
//! it owns one reader task that routes each response to the waiting caller's
//! oneshot channel. The single shared [`LspSession`] on the leader serializes
//! the actual stdio traffic at its own mutex, so concurrent handler invocations
//! cannot interleave on the wire — exactly the multiplex/demux the design needs.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{oneshot, Mutex};

use crate::election::peek_leader_pid;

/// A request sent from a follower to the leader.
///
/// `id` is echoed verbatim on the matching [`ResponseEnvelope`] so a client that
/// has several requests in flight on one connection can route each reply back to
/// the right caller. `method` names the SAH request op (e.g. `"diagnose"`,
/// `"get definition"`); `params` is the op's opaque argument payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestEnvelope {
    /// Per-connection correlation id, echoed on the response.
    pub id: u64,
    /// The request method/op name.
    pub method: String,
    /// Opaque per-method parameters.
    pub params: Value,
}

/// A response sent from the leader back to a follower.
///
/// Exactly one of `result` / `error` is set, mirroring a JSON-RPC result.
/// `id` matches the originating [`RequestEnvelope::id`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    /// Correlation id copied from the request.
    pub id: u64,
    /// The successful result, present iff the call succeeded.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub result: Option<Value>,
    /// The error message, present iff the call failed.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
}

impl RequestEnvelope {
    /// Build a request envelope.
    pub fn new(id: u64, method: impl Into<String>, params: Value) -> Self {
        Self {
            id,
            method: method.into(),
            params,
        }
    }

    /// Encode as a single newline-terminated JSON line for the wire.
    pub fn encode(&self) -> Result<String, IpcError> {
        encode_line(self)
    }

    /// Decode a single JSON line (the trailing newline may be present or not).
    pub fn decode(line: &str) -> Result<Self, IpcError> {
        serde_json::from_str(line.trim_end()).map_err(IpcError::Decode)
    }
}

impl ResponseEnvelope {
    /// Build a success response for `id`.
    pub fn ok(id: u64, result: Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Build an error response for `id`.
    pub fn err(id: u64, message: impl Into<String>) -> Self {
        Self {
            id,
            result: None,
            error: Some(message.into()),
        }
    }

    /// Encode as a single newline-terminated JSON line for the wire.
    pub fn encode(&self) -> Result<String, IpcError> {
        encode_line(self)
    }

    /// Decode a single JSON line (the trailing newline may be present or not).
    pub fn decode(line: &str) -> Result<Self, IpcError> {
        serde_json::from_str(line.trim_end()).map_err(IpcError::Decode)
    }
}

/// Serialize `value` to a single compact JSON line terminated by `\n`.
///
/// Shared by both envelope encoders so the framing is identical in both
/// directions: compact JSON (no embedded newlines) plus exactly one trailing
/// `\n` delimiter.
fn encode_line<T: Serialize>(value: &T) -> Result<String, IpcError> {
    let mut line = serde_json::to_string(value).map_err(IpcError::Encode)?;
    line.push('\n');
    Ok(line)
}

/// Errors raised by the request/response IPC channel.
#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    /// The leader socket could not be connected to. When a PID could be read
    /// from the lock file it is attached so the caller can attribute the
    /// failure ("leader is PID X"); `None` means the leader has gone.
    #[error("not the leader for this workspace{}", match .leader_pid {
        Some(pid) => format!(" (leader is PID {pid})"),
        None => " (no leader running)".to_string(),
    })]
    NotLeader {
        /// The PID recorded in the leader lock file, if readable.
        leader_pid: Option<u32>,
        /// The underlying connect error.
        #[source]
        source: std::io::Error,
    },

    /// An I/O error on an established connection (read/write/accept).
    #[error("ipc i/o error: {0}")]
    Io(#[source] std::io::Error),

    /// Failed to serialize an envelope to the wire.
    #[error("ipc encode error: {0}")]
    Encode(#[source] serde_json::Error),

    /// Failed to deserialize an envelope from the wire.
    #[error("ipc decode error: {0}")]
    Decode(#[source] serde_json::Error),

    /// The leader closed the connection before answering an in-flight request.
    #[error("leader closed the connection before responding")]
    ConnectionClosed,

    /// The remote returned an error response for the request.
    #[error("remote error: {0}")]
    Remote(String),
}

/// A request/response server bound to the leader-election socket.
///
/// The elected leader constructs one of these at its [`socket_path`] and calls
/// [`serve`](Self::serve) to accept follower connections forever. Each accepted
/// connection is driven on its own task; each request line on that connection is
/// dispatched to the handler on a further spawned task so a slow request never
/// head-of-line-blocks a concurrent one. Concurrency onto the single underlying
/// [`LspSession`] is bounded by that session's own client mutex, so the handler
/// can be invoked concurrently without corrupting the shared stdio pipe.
#[derive(Debug)]
pub struct RequestServer {
    listener: UnixListener,
    socket_path: PathBuf,
}

impl RequestServer {
    /// Bind a request server at `socket_path`, removing any stale socket first.
    ///
    /// The leader owns the socket: a prior unclean exit can leave a stale socket
    /// file that would make `bind` fail with `AddrInUse`, so we unlink it first
    /// (the flock — not the socket file — is the leadership source of truth, so
    /// removing a stale socket here is safe).
    pub fn bind(socket_path: impl AsRef<Path>) -> Result<Self, IpcError> {
        let socket_path = socket_path.as_ref().to_path_buf();
        // Best-effort: clear a stale socket from a previous leader.
        let _ = std::fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path).map_err(IpcError::Io)?;
        Ok(Self {
            listener,
            socket_path,
        })
    }

    /// The socket path this server is bound to.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Accept connections forever, dispatching every request to `handler`.
    ///
    /// `handler` is the SAH request API: it maps `(method, params)` to a result
    /// or an error message. It is wrapped in an `Arc` and cloned per request, so
    /// it must be `Send + Sync + 'static`; the future it returns is awaited on a
    /// per-request task. This never returns under normal operation; it returns
    /// `Err` only if `accept` itself fails irrecoverably.
    pub async fn serve<H, F>(&self, handler: H) -> Result<(), IpcError>
    where
        H: Fn(String, Value) -> F + Send + Sync + 'static,
        F: std::future::Future<Output = Result<Value, String>> + Send + 'static,
    {
        let handler = Arc::new(handler);
        loop {
            let (stream, _addr) = self.listener.accept().await.map_err(IpcError::Io)?;
            let handler = Arc::clone(&handler);
            tokio::spawn(async move {
                if let Err(e) = serve_connection(stream, handler).await {
                    tracing::debug!(error = %e, "request-ipc connection ended with error");
                }
            });
        }
    }
}

impl Drop for RequestServer {
    fn drop(&mut self) {
        // Remove the socket file so a later leader's `bind` does not trip over a
        // stale path. The flock-backed LeaderGuard also removes it on drop; this
        // makes the server self-contained when used without the guard (tests).
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// Drive a single accepted connection: read request lines, dispatch each to the
/// handler concurrently, and write back correlated responses.
///
/// Responses are serialized through a shared writer mutex so two concurrent
/// handler completions cannot interleave bytes on the wire. The reader loop ends
/// when the follower closes its half of the socket (EOF).
async fn serve_connection<H, F>(stream: UnixStream, handler: Arc<H>) -> Result<(), IpcError>
where
    H: Fn(String, Value) -> F + Send + Sync + 'static,
    F: std::future::Future<Output = Result<Value, String>> + Send + 'static,
{
    let (read_half, write_half) = stream.into_split();
    let writer = Arc::new(Mutex::new(write_half));
    let mut lines = BufReader::new(read_half).lines();

    while let Some(line) = lines.next_line().await.map_err(IpcError::Io)? {
        if line.trim().is_empty() {
            continue;
        }
        let request = match RequestEnvelope::decode(&line) {
            Ok(req) => req,
            Err(e) => {
                tracing::debug!(error = %e, line = %line, "dropping undecodable request line");
                continue;
            }
        };
        let handler = Arc::clone(&handler);
        let writer = Arc::clone(&writer);
        tokio::spawn(async move {
            let id = request.id;
            let response = match handler(request.method, request.params).await {
                Ok(result) => ResponseEnvelope::ok(id, result),
                Err(message) => ResponseEnvelope::err(id, message),
            };
            if let Err(e) = write_response(&writer, &response).await {
                tracing::debug!(error = %e, "failed to write request-ipc response");
            }
        });
    }
    Ok(())
}

/// Write one response line through the shared writer, flushing it.
async fn write_response(
    writer: &Mutex<tokio::net::unix::OwnedWriteHalf>,
    response: &ResponseEnvelope,
) -> Result<(), IpcError> {
    let line = response.encode()?;
    let mut guard = writer.lock().await;
    guard
        .write_all(line.as_bytes())
        .await
        .map_err(IpcError::Io)?;
    guard.flush().await.map_err(IpcError::Io)?;
    Ok(())
}

/// A request/response client connected to the leader socket.
///
/// A follower constructs one with [`connect`](Self::connect) and issues calls
/// via [`call`](Self::call). The client owns a background reader task that
/// demuxes responses by `id` to per-call oneshot channels, so many calls may be
/// in flight concurrently over the single connection and each still gets its own
/// correlated reply. Cloning is cheap (`Arc` bump) and every clone shares the
/// one connection.
#[derive(Clone)]
pub struct RequestClient {
    inner: Arc<ClientInner>,
}

impl std::fmt::Debug for RequestClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestClient")
            .field("next_id", &self.inner.next_id)
            .finish()
    }
}

struct ClientInner {
    /// Write half, behind a mutex so concurrent `call`s serialize their request
    /// writes (each write is one whole line, so they cannot interleave).
    writer: Mutex<tokio::net::unix::OwnedWriteHalf>,
    /// Pending calls awaiting a response, keyed by request id.
    pending: Mutex<std::collections::HashMap<u64, oneshot::Sender<ResponseEnvelope>>>,
    /// Monotonic id source for outgoing requests.
    next_id: AtomicU64,
}

/// Whether the client reader task should keep reading or stop.
enum ReaderStep {
    /// Keep reading the next line.
    Continue,
    /// Every client clone has dropped; end the reader task.
    Stop,
}

/// Demux one response line to the waiting caller's oneshot channel.
///
/// Extracted from the reader task in [`RequestClient::from_stream`] so the demux
/// invariants live in one flat place rather than four nested levels. Returns
/// [`ReaderStep::Stop`] when the shared state has been dropped (no caller is left
/// to deliver to); otherwise [`ReaderStep::Continue`]. An empty or undecodable
/// line is ignored — a malformed frame must not tear the connection down.
async fn deliver_response_line(line: &str, reader_inner: &Weak<ClientInner>) -> ReaderStep {
    if line.trim().is_empty() {
        return ReaderStep::Continue;
    }
    // If every client clone has dropped, there is no one left to deliver to.
    let Some(inner) = reader_inner.upgrade() else {
        return ReaderStep::Stop;
    };
    let Ok(response) = ResponseEnvelope::decode(line) else {
        return ReaderStep::Continue;
    };
    if let Some(sender) = inner.pending.lock().await.remove(&response.id) {
        let _ = sender.send(response);
    }
    ReaderStep::Continue
}

impl RequestClient {
    /// Connect to the leader's request socket at `socket_path`.
    ///
    /// On a connect failure (no leader bound), returns
    /// [`IpcError::NotLeader`] carrying the leader PID read from `lock_path` via
    /// [`peek_leader_pid`] when available, so the caller can render a
    /// "leader is PID X" diagnostic. `lock_path` is the same election lock file
    /// whose flock arbitrates leadership.
    pub async fn connect(
        socket_path: impl AsRef<Path>,
        lock_path: impl AsRef<Path>,
    ) -> Result<Self, IpcError> {
        let stream = UnixStream::connect(socket_path.as_ref())
            .await
            .map_err(|source| IpcError::NotLeader {
                leader_pid: peek_leader_pid(lock_path.as_ref()),
                source,
            })?;
        Ok(Self::from_stream(stream))
    }

    /// Build a client over an already-connected stream (the connection seam used
    /// by both [`connect`](Self::connect) and the in-process tests).
    fn from_stream(stream: UnixStream) -> Self {
        let (read_half, write_half) = stream.into_split();
        let inner = Arc::new(ClientInner {
            writer: Mutex::new(write_half),
            pending: Mutex::new(std::collections::HashMap::new()),
            next_id: AtomicU64::new(1),
        });
        // The reader task demuxes responses to waiting callers by id. It holds a
        // *Weak* reference to the shared state, NOT a strong `Arc`: `ClientInner`
        // owns the write half, so a strong hold here would keep the write half
        // alive after the last `RequestClient` clone is dropped, the server would
        // never see EOF, and the connection/task/fds would leak. With a `Weak`,
        // dropping the last client drops `inner` → drops the write half → the
        // server sees EOF → this task's `next_line` returns EOF and the loop ends.
        let reader_inner = Arc::downgrade(&inner);
        tokio::spawn(async move {
            let mut lines = BufReader::new(read_half).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        // `Continue` keeps reading; `Stop` ends the task (every
                        // client clone dropped, so there is no one to deliver to).
                        if let ReaderStep::Stop = deliver_response_line(&line, &reader_inner).await
                        {
                            break;
                        }
                    }
                    // EOF or read error: fail every in-flight call so callers do
                    // not hang. Dropping the senders signals ConnectionClosed.
                    _ => {
                        if let Some(inner) = reader_inner.upgrade() {
                            inner.pending.lock().await.clear();
                        }
                        break;
                    }
                }
            }
        });
        Self { inner }
    }

    /// Round-trip one `(method, params)` call to the leader and return its
    /// result.
    ///
    /// Allocates a unique id, registers a oneshot for the reply, writes the
    /// request line, and awaits the correlated response. A remote error response
    /// surfaces as [`IpcError::Remote`]; a dropped connection (the reader task
    /// cleared the pending map) surfaces as [`IpcError::ConnectionClosed`].
    pub async fn call(&self, method: impl Into<String>, params: Value) -> Result<Value, IpcError> {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.inner.pending.lock().await.insert(id, tx);

        // Send the request. On any failure before the reply arrives, drop the
        // now-orphaned `pending` entry so a failed/degraded connection does not
        // accumulate dead senders.
        if let Err(e) = self.write_request(id, method, params).await {
            self.inner.pending.lock().await.remove(&id);
            return Err(e);
        }

        let response = rx.await.map_err(|_| IpcError::ConnectionClosed)?;
        match (response.result, response.error) {
            (Some(result), _) => Ok(result),
            (None, Some(message)) => Err(IpcError::Remote(message)),
            (None, None) => Ok(Value::Null),
        }
    }

    /// Encode and write one request line through the shared writer.
    ///
    /// Split out of [`call`](Self::call) so the caller can clean up the
    /// `pending` entry on any send-path failure with a single error branch.
    async fn write_request(
        &self,
        id: u64,
        method: impl Into<String>,
        params: Value,
    ) -> Result<(), IpcError> {
        let line = RequestEnvelope::new(id, method, params).encode()?;
        let mut writer = self.inner.writer.lock().await;
        writer
            .write_all(line.as_bytes())
            .await
            .map_err(IpcError::Io)?;
        writer.flush().await.map_err(IpcError::Io)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Base per-handler delay, in milliseconds, for the out-of-order demux test.
    const HANDLER_BASE_DELAY_MS: u64 = 50;
    /// How much each unit of `n` shortens the handler delay, in milliseconds.
    const DELAY_MULTIPLIER: u64 = 5;
    /// Backoff between connect attempts while the listener is still binding.
    const CONNECT_RETRY_DELAY_MS: u64 = 5;
    /// Bound on how long a regression assertion may wait before failing.
    const TEST_TIMEOUT_SECS: u64 = 2;

    #[test]
    fn request_envelope_round_trips_through_the_wire_line() {
        let req = RequestEnvelope::new(7, "diagnose", json!({ "paths": ["a.rs"] }));
        let line = req.encode().expect("encode");
        assert!(
            line.ends_with('\n'),
            "framing must terminate with a newline"
        );
        assert!(
            !line.trim_end().contains('\n'),
            "a request must be exactly one line"
        );
        let decoded = RequestEnvelope::decode(&line).expect("decode");
        assert_eq!(decoded, req);
    }

    #[test]
    fn response_ok_and_err_encode_distinct_shapes() {
        let ok = ResponseEnvelope::ok(3, json!({ "errors": 0 }));
        let ok_line = ok.encode().unwrap();
        let ok_back = ResponseEnvelope::decode(&ok_line).unwrap();
        assert_eq!(ok_back.id, 3);
        assert_eq!(ok_back.result, Some(json!({ "errors": 0 })));
        assert_eq!(ok_back.error, None);

        let err = ResponseEnvelope::err(4, "boom");
        let err_line = err.encode().unwrap();
        let err_back = ResponseEnvelope::decode(&err_line).unwrap();
        assert_eq!(err_back.id, 4);
        assert_eq!(err_back.result, None);
        assert_eq!(err_back.error.as_deref(), Some("boom"));
    }

    #[tokio::test]
    async fn interleaved_concurrent_calls_get_correlated_responses() {
        // A handler that echoes its id-bearing params back, but with a per-id
        // delay so responses arrive OUT OF ORDER relative to request order.
        // The client must still route each reply to the right caller by id.
        let tmp = tempfile::tempdir().unwrap();
        let socket = tmp.path().join("req.sock");
        let lock = tmp.path().join("lock");

        let server = RequestServer::bind(&socket).unwrap();
        tokio::spawn(async move {
            let _ = server
                .serve(|method, params| async move {
                    // method == "echo": sleep longer for smaller n, so a later
                    // request returns first.
                    assert_eq!(method, "echo");
                    let n = params.get("n").and_then(Value::as_u64).unwrap();
                    let delay = HANDLER_BASE_DELAY_MS.saturating_sub(n * DELAY_MULTIPLIER);
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    Ok(json!({ "n": n }))
                })
                .await;
        });

        // Give the listener a moment to bind, then connect.
        let client = loop {
            match RequestClient::connect(&socket, &lock).await {
                Ok(c) => break c,
                Err(_) => {
                    tokio::time::sleep(std::time::Duration::from_millis(CONNECT_RETRY_DELAY_MS))
                        .await
                }
            }
        };

        // Fire N concurrent calls; assert each returns its own n.
        let mut handles = Vec::new();
        for n in 0..10u64 {
            let client = client.clone();
            handles.push(tokio::spawn(async move {
                let result = client.call("echo", json!({ "n": n })).await.unwrap();
                assert_eq!(result.get("n").and_then(Value::as_u64), Some(n));
                n
            }));
        }
        let mut seen = Vec::new();
        for h in handles {
            seen.push(h.await.unwrap());
        }
        seen.sort_unstable();
        assert_eq!(seen, (0..10).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn connect_to_absent_leader_yields_not_leader_with_pid() {
        let tmp = tempfile::tempdir().unwrap();
        let socket = tmp.path().join("absent.sock");
        let lock = tmp.path().join("leader.lock");
        // Seed a PID into the lock file so NotLeader can attribute the failure.
        std::fs::write(&lock, "4242\n").unwrap();

        let err = RequestClient::connect(&socket, &lock)
            .await
            .expect_err("connect to an unbound socket must fail");
        match err {
            IpcError::NotLeader { leader_pid, .. } => {
                assert_eq!(leader_pid, Some(4242));
            }
            other => panic!("expected NotLeader, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn remote_error_surfaces_as_remote_variant() {
        let tmp = tempfile::tempdir().unwrap();
        let socket = tmp.path().join("err.sock");
        let lock = tmp.path().join("lock");

        let server = RequestServer::bind(&socket).unwrap();
        tokio::spawn(async move {
            let _ = server
                .serve(|_method, _params| async move { Err("handler failed".to_string()) })
                .await;
        });

        let client = loop {
            match RequestClient::connect(&socket, &lock).await {
                Ok(c) => break c,
                Err(_) => {
                    tokio::time::sleep(std::time::Duration::from_millis(CONNECT_RETRY_DELAY_MS))
                        .await
                }
            }
        };

        let err = client
            .call("anything", json!({}))
            .await
            .expect_err("a handler error must surface");
        assert!(matches!(err, IpcError::Remote(m) if m == "handler failed"));
    }

    #[tokio::test]
    async fn dropping_the_last_client_closes_the_connection_so_the_peer_sees_eof() {
        // Regression: the reader task must hold only a Weak reference to the
        // shared state, so dropping the last RequestClient drops the write half
        // and the server peer observes EOF. A strong hold would keep the write
        // half alive forever, leaking the connection/task/fds.
        use tokio::io::AsyncReadExt;

        let tmp = tempfile::tempdir().unwrap();
        let socket = tmp.path().join("drop.sock");
        let lock = tmp.path().join("lock");

        // A raw listener so the test holds the server side of the connection and
        // can read it to EOF directly.
        let listener = UnixListener::bind(&socket).unwrap();
        let accept = tokio::spawn(async move { listener.accept().await.unwrap().0 });

        let client = RequestClient::connect(&socket, &lock)
            .await
            .expect("connect");
        let mut server_stream = accept.await.unwrap();

        // Drop the only client clone. The reader task (Weak) must let `inner`
        // drop, which drops the write half and closes the connection.
        drop(client);

        // The server side must observe EOF promptly (read returns 0 bytes).
        let mut buf = [0u8; 16];
        let n = tokio::time::timeout(
            std::time::Duration::from_secs(TEST_TIMEOUT_SECS),
            server_stream.read(&mut buf),
        )
        .await
        .expect("server read must not hang after the client drops")
        .expect("read");
        assert_eq!(
            n, 0,
            "dropping the last client must close the connection (EOF)"
        );
    }

    #[tokio::test]
    async fn call_on_a_dead_connection_does_not_leak_a_pending_entry() {
        // Regression: a send-path failure must remove the orphaned pending
        // entry. Here the server immediately closes the connection, so the
        // reader task clears pending on EOF and the call resolves to
        // ConnectionClosed without leaving a dead sender behind.
        let tmp = tempfile::tempdir().unwrap();
        let socket = tmp.path().join("dead.sock");
        let lock = tmp.path().join("lock");

        let listener = UnixListener::bind(&socket).unwrap();
        // Accept then immediately drop the server side, closing the connection.
        let accept = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            drop(stream);
        });

        let client = RequestClient::connect(&socket, &lock)
            .await
            .expect("connect");
        accept.await.unwrap();

        // The call resolves (ConnectionClosed or Io), never hangs.
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(TEST_TIMEOUT_SECS),
            client.call("x", json!({})),
        )
        .await
        .expect("call must not hang on a dead connection");
        assert!(result.is_err(), "a dead connection must surface an error");
    }
}
