//! The SAH request API carried over the leader-election request socket.
//!
//! [`swissarmyhammer_leader_election::request_ipc`] gives a generic, correlated
//! request/response channel between a follower and the elected leader. This
//! module layers the *SAH request API* onto it: it maps method names onto the
//! leader's single owned [`LspSession`], so a follower process (which spawns no
//! LSP server of its own) can still get a diagnostics report or an LSP query
//! answered — the leader multiplexes every follower's calls onto its one stdio
//! session and the IPC layer demuxes the replies by id.
//!
//! Two method families are served:
//!
//! - `"diagnose"` — params `{ "paths": [String] }` → a [`DiagnosticsReport`]
//!   (the [`diagnose`] core API). This is the diagnostics request surface.
//! - `"lsp_request"` — params `{ "method": String, "params": <json>,
//!   "file_path"?: String }` → the LSP result of `session.request(method,
//!   params)`. When `file_path` is present the leader `sync_open`s that document
//!   on its session before the request (mirroring the local
//!   `lsp_request_with_document` open-then-request contract); when absent the op
//!   is workspace-wide and no document is synced. This covers code-context query
//!   ops that bottom out in a single LSP request (e.g. `textDocument/definition`,
//!   `textDocument/hover`, `textDocument/references`, `workspace/symbol`).
//!
//! ## Leader vs follower vs in-process subagent
//!
//! - **Leader**: calls [`serve_session_requests`] with its session; the IPC
//!   server accepts follower connections and routes each request through
//!   [`dispatch`].
//! - **Out-of-process follower**: connects a [`SessionRequestClient`] to the
//!   leader socket and calls [`SessionRequestClient::diagnose`] /
//!   [`SessionRequestClient::lsp_request`]. It owns no session.
//! - **In-process subagent**: shares the parent's [`LspSession`] handle directly
//!   (a cheap `Arc` clone) and calls [`diagnose`] / `session.request` with no
//!   socket round-trip at all — the socket exists for *cross-process* followers.

use std::sync::Arc;

use serde_json::{json, Value};

use swissarmyhammer_leader_election::request_ipc::{IpcError, RequestClient, RequestServer};
use swissarmyhammer_lsp::client::LspTransport;
use swissarmyhammer_lsp::LspSession;

use crate::config::DiagnosticsConfig;
use crate::diagnose::{diagnose, Dependents};
use crate::record::DiagnosticsReport;
use crate::settle::{Timer, TokioTimer};

/// Method name for the diagnostics request op.
pub const METHOD_DIAGNOSE: &str = "diagnose";

/// Method name for a raw single-shot LSP request op.
pub const METHOD_LSP_REQUEST: &str = "lsp_request";

/// Method name for an atomic *multi-step* LSP request op.
///
/// Unlike [`METHOD_LSP_REQUEST`] (one round-trip), this runs an ordered batch of
/// LSP requests under ONE `with_client` lock on the leader's session, so a
/// multi-step exchange (e.g. `prepareRename` then `rename`) stays atomic and no
/// other consumer interleaves a request and steals a response off the shared
/// stdio pipe. The document is synced once before the batch.
pub const METHOD_LSP_MULTI_REQUEST: &str = "lsp_multi_request";

/// Dispatch one SAH request `(method, params)` against `session`, returning the
/// JSON result or an error message.
///
/// This is the leader-side router shared by [`serve_session_requests`] and the
/// unit tests. `dependents` and `timer` are the same resolver/clock that
/// [`diagnose`] takes; they are reused for every `"diagnose"` call so the leader
/// folds in broken dependents exactly as an in-process caller would.
///
/// An unknown method is an error (not a panic): the channel is shared and a
/// future op should be added here, not crash the leader.
pub async fn dispatch<C, D, T>(
    session: &LspSession<C>,
    dependents: &D,
    timer: &T,
    config: &DiagnosticsConfig,
    method: &str,
    params: Value,
) -> Result<Value, String>
where
    C: LspTransport + Send + Sync + 'static,
    D: Dependents,
    T: Timer,
{
    match method {
        METHOD_DIAGNOSE => {
            let paths = parse_paths(&params)?;
            let report = diagnose(session, &paths, config, dependents, timer).await;
            serde_json::to_value(&report).map_err(|e| format!("failed to encode report: {e}"))
        }
        METHOD_LSP_REQUEST => {
            let (lsp_method, lsp_params, file_path) = parse_lsp_request(&params)?;
            lsp_request_blocking(session, lsp_method, lsp_params, file_path).await
        }
        METHOD_LSP_MULTI_REQUEST => {
            let (file_path, steps) = parse_lsp_multi_request(&params)?;
            lsp_multi_request_blocking(session, file_path, steps).await
        }
        other => Err(format!("unknown request method: {other}")),
    }
}

/// Run one synchronous LSP round-trip off the async runtime, syncing the
/// document first when a `file_path` is supplied.
///
/// [`LspSession::request`] blocks the calling thread for the whole stdio
/// request/response cycle (it locks a `std::sync::Mutex` and waits on the pipe).
/// The leader's serve loop dispatches this on the tokio runtime, where a blocking
/// call would pin a worker thread and starve concurrent follower requests and the
/// leader's own async work. The session is cheap to clone (`Arc`-backed), so the
/// blocking call is moved onto [`tokio::task::spawn_blocking`]; the runtime thread
/// stays free to drive every other task while the round-trip is in flight.
///
/// `file_path` mirrors the local
/// [`lsp_request_with_document`](swissarmyhammer_code_context::LayeredContext::lsp_request_with_document)
/// contract: a follower's code-context op (definition/hover/references/…) opens
/// or refreshes the document on its session before the request so the server
/// analyzes the *current* on-disk content. Routed to the leader, the same sync
/// must happen on the leader's single session, or the server answers against a
/// buffer it has never opened. When `file_path` is absent (a workspace-wide op
/// such as `workspace/symbol`), no document is synced. A sync failure (file gone
/// / unreadable) surfaces as the request error rather than silently querying a
/// stale buffer.
async fn lsp_request_blocking<C>(
    session: &LspSession<C>,
    lsp_method: String,
    lsp_params: Value,
    file_path: Option<String>,
) -> Result<Value, String>
where
    C: LspTransport + Send + Sync + 'static,
{
    let session = session.clone();
    tokio::task::spawn_blocking(move || {
        if let Some(path) = file_path {
            let path = std::path::PathBuf::from(&path);
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("lsp request failed to read {}: {e}", path.display()))?;
            session
                .sync_open(&path, &text)
                .map_err(|e| format!("lsp request failed to sync document: {e}"))?;
        }
        session
            .request(&lsp_method, lsp_params)
            .map_err(|e| format!("lsp request failed: {e}"))
    })
    .await
    .map_err(|e| format!("lsp request task failed: {e}"))?
}

/// Run an ordered batch of LSP requests as ONE atomic exchange off the async
/// runtime, syncing the document first, and return one raw response per step.
///
/// This is the leader-side handler for [`METHOD_LSP_MULTI_REQUEST`]. The whole
/// batch runs under a single [`LspSession::with_client`] lock so the multi-step
/// exchange (e.g. `prepareRename` then `rename`) cannot be interleaved by another
/// follower's request — holding the lock across every `send_request` is exactly
/// what keeps a response from being stolen off the shared stdio pipe (a separate
/// `session.request` per step would re-lock between steps and lose that
/// guarantee).
///
/// `file_path` syncs the document onto the leader's single session before the
/// batch (mirroring the local open-then-request contract); when absent no
/// document is synced. As with [`lsp_request_blocking`], the blocking round-trips
/// are moved onto [`tokio::task::spawn_blocking`] so the serve runtime stays free.
///
/// Returns a JSON array of the raw step responses, in step order. A
/// [`LspError::NotRunning`] (no live client) or a transport error surfaces as an
/// error string — never a silent empty.
async fn lsp_multi_request_blocking<C>(
    session: &LspSession<C>,
    file_path: Option<String>,
    steps: Vec<(String, Value)>,
) -> Result<Value, String>
where
    C: LspTransport + Send + Sync + 'static,
{
    let session = session.clone();
    tokio::task::spawn_blocking(move || {
        if let Some(path) = &file_path {
            let path = std::path::PathBuf::from(path);
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("lsp multi request failed to read {}: {e}", path.display()))?;
            session
                .sync_open(&path, &text)
                .map_err(|e| format!("lsp multi request failed to sync document: {e}"))?;
        }
        // Hold the client lock across the WHOLE batch so no other consumer
        // interleaves a request and steals a step's response off the pipe.
        let responses = session
            .with_client(|client| {
                let mut out = Vec::with_capacity(steps.len());
                for (method, params) in &steps {
                    out.push(client.send_request(method, params.clone())?);
                }
                Ok(out)
            })
            .map_err(|e| format!("lsp multi request failed: {e}"))?
            .ok_or_else(|| "lsp multi request failed: no live LSP client".to_string())?;
        Ok(Value::Array(responses))
    })
    .await
    .map_err(|e| format!("lsp multi request task failed: {e}"))?
}

/// Reject a follower-supplied path that contains a `..` parent-dir component.
///
/// Every path the leader reads or syncs arrives from untrusted follower JSON
/// over the request socket. A `..` component is the directory-traversal vector
/// that would let a follower walk out of the workspace and have the leader read
/// or open an arbitrary file (e.g. `src/../../etc/passwd`). All three leader
/// read paths (`diagnose`'s [`parse_paths`], the single-request
/// [`parse_lsp_request`], and the multi-request [`parse_lsp_multi_request`])
/// share this one guard so the contract cannot drift between them.
///
/// Absolute paths are *not* rejected: the leader read surface is contractually
/// **absolute-space** ([`diagnose`] and the document-sync `file_path` are both
/// handed absolute repo paths), so rejecting absolute paths here would reject
/// every legitimate follower call. The escape risk is `..` traversal, which is
/// what we block. `context` names the op for the error message.
fn reject_parent_dir_traversal(path_str: &str, context: &str) -> Result<(), String> {
    if std::path::Path::new(path_str)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(format!(
            "{context}: path must not contain a `..` parent-dir component: {path_str}"
        ));
    }
    Ok(())
}

/// Extract and validate the `paths` array from a `"diagnose"` request's params.
///
/// Paths arrive from untrusted follower JSON, so each is hardened via
/// [`reject_parent_dir_traversal`] before it is handed to [`diagnose`].
fn parse_paths(params: &Value) -> Result<Vec<String>, String> {
    let paths = params
        .get("paths")
        .and_then(Value::as_array)
        .ok_or_else(|| "diagnose: missing `paths` array".to_string())?;
    paths
        .iter()
        .map(|p| {
            let path_str = p
                .as_str()
                .ok_or_else(|| "diagnose: every path must be a string".to_string())?;
            reject_parent_dir_traversal(path_str, "diagnose")?;
            Ok(path_str.to_string())
        })
        .collect()
}

/// Extract `(method, params, file_path)` from an `"lsp_request"` request's params.
///
/// `file_path` is optional: when present, the leader syncs that document onto its
/// session before issuing the request (mirroring the local
/// `lsp_request_with_document` open-then-request contract); when absent, the op
/// is workspace-wide (e.g. `workspace/symbol`) and no document is synced. A
/// present `file_path` is hardened via [`reject_parent_dir_traversal`] — the
/// leader reads and `sync_open`s it, so a `..` traversal would otherwise let a
/// follower open an arbitrary file on the leader's session.
fn parse_lsp_request(params: &Value) -> Result<(String, Value, Option<String>), String> {
    let method = params
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| "lsp_request: missing `method` string".to_string())?
        .to_string();
    let inner = params.get("params").cloned().unwrap_or(Value::Null);
    let file_path = params
        .get("file_path")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(path) = &file_path {
        reject_parent_dir_traversal(path, "lsp_request")?;
    }
    Ok((method, inner, file_path))
}

/// Extract `(file_path, steps)` from an `"lsp_multi_request"` request's params.
///
/// `file_path` is optional: when present the leader syncs that document onto its
/// session once before the batch; when absent the batch is document-less. A
/// present `file_path` is hardened via [`reject_parent_dir_traversal`] — the
/// leader reads and `sync_open`s it, so a `..` traversal would otherwise let a
/// follower open an arbitrary file on the leader's session. `steps` is the
/// ordered list of `{ method, params }` objects to run under one lock; an empty
/// or malformed list is an error rather than a silent no-op.
#[allow(clippy::type_complexity)]
fn parse_lsp_multi_request(
    params: &Value,
) -> Result<(Option<String>, Vec<(String, Value)>), String> {
    let file_path = params
        .get("file_path")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(path) = &file_path {
        reject_parent_dir_traversal(path, "lsp_multi_request")?;
    }
    let steps_json = params
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| "lsp_multi_request: missing `steps` array".to_string())?;
    if steps_json.is_empty() {
        return Err("lsp_multi_request: `steps` must not be empty".to_string());
    }
    let steps = steps_json
        .iter()
        .map(|step| {
            let method = step
                .get("method")
                .and_then(Value::as_str)
                .ok_or_else(|| "lsp_multi_request: every step needs a `method` string".to_string())?
                .to_string();
            let inner = step.get("params").cloned().unwrap_or(Value::Null);
            Ok((method, inner))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((file_path, steps))
}

/// Serve the SAH request API on `server`, routing every follower request onto
/// `session`.
///
/// The elected leader binds a [`RequestServer`] at its election socket and calls
/// this. `session` is the one owned [`LspSession`]; `dependents` and `timer` are
/// the resolver/clock used for `"diagnose"` calls. All three are captured into
/// the per-request handler, so they must be `Send + Sync + 'static` — the
/// production session (`Arc`-backed), [`crate::PrecomputedDependents`], and
/// [`TokioTimer`] all satisfy this.
///
/// Returns only if the listener fails irrecoverably; otherwise it serves
/// forever, so the caller typically spawns it.
pub async fn serve_session_requests<C, D>(
    server: RequestServer,
    session: LspSession<C>,
    dependents: D,
    config: DiagnosticsConfig,
) -> Result<(), IpcError>
where
    C: LspTransport + Send + Sync + 'static,
    D: Dependents + Send + Sync + 'static,
{
    let ctx = Arc::new(HandlerCtx {
        session,
        dependents,
        timer: TokioTimer,
        config,
    });
    server
        .serve(move |method, params| {
            let ctx = Arc::clone(&ctx);
            async move {
                dispatch(
                    &ctx.session,
                    &ctx.dependents,
                    &ctx.timer,
                    &ctx.config,
                    &method,
                    params,
                )
                .await
            }
        })
        .await
}

/// The captured leader-side context shared (via `Arc`) across every request.
struct HandlerCtx<C: LspTransport, D: Dependents> {
    session: LspSession<C>,
    dependents: D,
    timer: TokioTimer,
    config: DiagnosticsConfig,
}

/// A follower-side client for the SAH request API.
///
/// Wraps the generic [`RequestClient`] and exposes the request ops as typed
/// methods, so a follower satisfies `diagnose` / LSP queries by round-tripping
/// to the leader with no local LSP server. Cloning is cheap and shares the one
/// connection.
#[derive(Clone, Debug)]
pub struct SessionRequestClient {
    client: RequestClient,
}

impl SessionRequestClient {
    /// Connect to the leader's request socket.
    ///
    /// `socket_path` is the election socket; `lock_path` is the election lock
    /// file (used to attribute a connect failure to the leader PID). A failure
    /// to connect — i.e. there is no bound leader — surfaces as
    /// [`IpcError::NotLeader`] carrying the leader PID when readable.
    pub async fn connect(
        socket_path: impl AsRef<std::path::Path>,
        lock_path: impl AsRef<std::path::Path>,
    ) -> Result<Self, IpcError> {
        Ok(Self {
            client: RequestClient::connect(socket_path, lock_path).await?,
        })
    }

    /// Build a client over an existing generic [`RequestClient`].
    pub fn new(client: RequestClient) -> Self {
        Self { client }
    }

    /// Ask the leader to diagnose `paths`, returning the report it produces from
    /// its single session.
    pub async fn diagnose(&self, paths: &[String]) -> Result<DiagnosticsReport, IpcError> {
        let result = self
            .client
            .call(METHOD_DIAGNOSE, json!({ "paths": paths }))
            .await?;
        serde_json::from_value(result).map_err(IpcError::Decode)
    }

    /// Round-trip one raw LSP request `(method, params)` to the leader's session
    /// and return its result.
    ///
    /// Use this for workspace-wide ops that are not scoped to a document the
    /// leader must first open (e.g. `workspace/symbol`). For a document-scoped op
    /// use [`lsp_request_with_document`](Self::lsp_request_with_document) so the
    /// leader syncs the file before the request.
    pub async fn lsp_request(&self, method: &str, params: Value) -> Result<Value, IpcError> {
        self.client
            .call(
                METHOD_LSP_REQUEST,
                lsp_request_envelope(method, params, None),
            )
            .await
    }

    /// Round-trip a document-scoped LSP request to the leader, asking it to sync
    /// `file_path` onto its session before issuing the request.
    ///
    /// This mirrors the in-process
    /// [`LayeredContext::lsp_request_with_document`](swissarmyhammer_code_context::LayeredContext::lsp_request_with_document)
    /// contract: a follower's code-context op (definition/hover/references/…)
    /// would locally open or refresh the document before the request so the
    /// server analyzes the current on-disk content; routed to the leader, the
    /// `file_path` makes the leader do that same `sync_open` on its single
    /// session before the request.
    pub async fn lsp_request_with_document(
        &self,
        file_path: &str,
        method: &str,
        params: Value,
    ) -> Result<Value, IpcError> {
        self.client
            .call(
                METHOD_LSP_REQUEST,
                lsp_request_envelope(method, params, Some(file_path)),
            )
            .await
    }

    /// Round-trip an ordered batch of LSP requests to the leader, to be run as
    /// ONE atomic exchange under a single client lock on the leader's session.
    ///
    /// This is the multi-step sibling of [`lsp_request_with_document`](Self::lsp_request_with_document):
    /// a follower's multi-step code-context op (rename: `prepareRename` then
    /// `rename`) cannot be expressed as separate single requests without risking
    /// another consumer interleaving and stealing a response off the leader's
    /// shared stdio pipe. Sending the whole `steps` list in one call lets the
    /// leader hold its `with_client` lock across every step. `file_path` is synced
    /// once before the batch. Returns the ordered raw step responses (a JSON
    /// array), which the caller unwraps per step.
    pub async fn lsp_multi_request_with_document(
        &self,
        file_path: &str,
        steps: Vec<(String, Value)>,
    ) -> Result<Vec<Value>, IpcError> {
        let result = self
            .client
            .call(
                METHOD_LSP_MULTI_REQUEST,
                lsp_multi_request_envelope(file_path, steps),
            )
            .await?;
        match result {
            Value::Array(responses) => Ok(responses),
            other => Err(IpcError::Decode(serde::de::Error::custom(format!(
                "lsp_multi_request expected an array of step responses, got: {other}"
            )))),
        }
    }
}

/// Build the `lsp_request` request envelope `{ method, params, file_path? }`.
///
/// The leader-side [`parse_lsp_request`] reads `file_path` back out and syncs
/// that document before the request; an absent `file_path` is omitted from the
/// envelope so a workspace-wide op carries no document scope.
fn lsp_request_envelope(method: &str, params: Value, file_path: Option<&str>) -> Value {
    let mut envelope = json!({ "method": method, "params": params });
    if let Some(path) = file_path {
        envelope["file_path"] = Value::String(path.to_string());
    }
    envelope
}

/// Build the `lsp_multi_request` request envelope `{ file_path?, steps: [{ method, params }] }`.
///
/// The leader-side [`parse_lsp_multi_request`] reads `steps` back out and runs
/// them under one lock; `file_path` is the single document synced before the
/// batch. An empty `file_path` is treated as document-less and omitted.
fn lsp_multi_request_envelope(file_path: &str, steps: Vec<(String, Value)>) -> Value {
    let steps_json: Vec<Value> = steps
        .into_iter()
        .map(|(method, params)| json!({ "method": method, "params": params }))
        .collect();
    let mut envelope = json!({ "steps": steps_json });
    if !file_path.is_empty() {
        envelope["file_path"] = Value::String(file_path.to_string());
    }
    envelope
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use serde_json::json;
    use swissarmyhammer_lsp::LspError;

    use crate::test_support::{ManualTimer, NullTransport};
    use crate::PrecomputedDependents;

    /// A session whose diagnostics cache is pre-seeded but which has no live
    /// client, so `diagnose` reports from the cache with no real server.
    fn seeded_session() -> LspSession<NullTransport> {
        let session = LspSession::new(Arc::new(Mutex::new(None)), "rust");
        session.handle_publish_diagnostics(&json!({
            "uri": "file:///src/a.rs",
            "diagnostics": [{
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 1 }
                },
                "severity": 1,
                "message": "A broke"
            }]
        }));
        session
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dispatch_diagnose_returns_an_encodable_report() {
        // The dispatch router maps "diagnose" onto the session and the report
        // round-trips through JSON, model-free via NullTransport + ManualTimer.
        let session = seeded_session();
        let timer = ManualTimer::default();
        let driver = timer.clone();
        let config = DiagnosticsConfig::default();
        let deps = PrecomputedDependents::default();

        let handle = tokio::spawn(async move {
            dispatch(
                &session,
                &deps,
                &timer,
                &config,
                METHOD_DIAGNOSE,
                json!({ "paths": ["/src/a.rs"] }),
            )
            .await
        });
        tokio::task::yield_now().await;
        driver.advance(DiagnosticsConfig::default().settle_window);
        let value = handle.await.unwrap().expect("dispatch should succeed");

        let report: DiagnosticsReport = serde_json::from_value(value).expect("decode report");
        assert_eq!(report.counts.errors, 1);
        assert_eq!(report.diagnostics[0].message, "A broke");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dispatch_unknown_method_is_an_error_not_a_panic() {
        let session = seeded_session();
        let err = dispatch(
            &session,
            &PrecomputedDependents::default(),
            &ManualTimer::default(),
            &DiagnosticsConfig::default(),
            "nope",
            json!({}),
        )
        .await
        .expect_err("unknown method must error");
        assert!(err.contains("unknown request method"), "got: {err}");
    }

    /// A transport whose `send_request` blocks the calling thread for a fixed
    /// span, so a test can observe whether the leader-side dispatch offloads the
    /// synchronous LSP round-trip off the async runtime.
    struct BlockingTransport {
        block: std::time::Duration,
    }

    impl LspTransport for BlockingTransport {
        fn send_request(&mut self, _method: &str, _params: Value) -> Result<Value, LspError> {
            std::thread::sleep(self.block);
            Ok(json!({ "ok": true }))
        }
        fn send_notification(&mut self, _method: &str, _params: Value) -> Result<(), LspError> {
            Ok(())
        }
        fn read_message(&mut self) -> Result<Value, LspError> {
            Err(LspError::NotRunning)
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dispatch_lsp_request_does_not_block_the_runtime_thread() {
        // The single owned LspSession's `request` is a SYNCHRONOUS, blocking
        // stdio round-trip. On the leader's serve path the dispatch handler runs
        // on the async runtime, so calling `request` inline would pin a worker
        // thread for the whole round-trip and starve concurrent work. The
        // dispatch must therefore offload the blocking call (spawn_blocking).
        //
        // Proof on a current_thread runtime: if dispatch blocked the only thread,
        // a concurrently-spawned async task could not advance until dispatch
        // returned. With the call offloaded, the runtime stays free and the async
        // task completes FIRST despite the long block.
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Mutex as StdMutex;

        let block = std::time::Duration::from_millis(200);
        let client: BlockingTransport = BlockingTransport { block };
        let session = LspSession::new(Arc::new(StdMutex::new(Some(client))), "rust");

        let async_done = Arc::new(AtomicBool::new(false));
        let async_done_for_task = Arc::clone(&async_done);
        // A short async sleep that, on a free runtime, finishes well before the
        // 200ms blocking request.
        let pinger = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            async_done_for_task.store(true, Ordering::SeqCst);
        });

        let dispatched = dispatch(
            &session,
            &PrecomputedDependents::default(),
            &ManualTimer::default(),
            &DiagnosticsConfig::default(),
            METHOD_LSP_REQUEST,
            json!({ "method": "textDocument/definition", "params": {} }),
        )
        .await
        .expect("blocking lsp request should still resolve");

        assert_eq!(dispatched, json!({ "ok": true }));
        assert!(
            async_done.load(Ordering::SeqCst),
            "the concurrent async task must finish during the blocking request — \
             dispatch must offload the blocking LSP round-trip, not pin the runtime thread",
        );
        pinger.await.unwrap();
    }

    /// A recording transport that logs every `(method, params)` it is handed
    /// into a shared `Arc<Mutex<Vec<..>>>`, so a test can read the wire order
    /// back after the session has been consumed. `send_request` answers any
    /// method with a benign empty object.
    #[derive(Clone)]
    struct SharedRecordingTransport {
        log: Arc<Mutex<Vec<(String, Value)>>>,
    }

    impl LspTransport for SharedRecordingTransport {
        fn send_request(&mut self, method: &str, params: Value) -> Result<Value, LspError> {
            self.log.lock().unwrap().push((method.to_string(), params));
            Ok(json!({}))
        }
        fn send_notification(&mut self, method: &str, params: Value) -> Result<(), LspError> {
            self.log.lock().unwrap().push((method.to_string(), params));
            Ok(())
        }
        fn read_message(&mut self) -> Result<Value, LspError> {
            Err(LspError::NotRunning)
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dispatch_lsp_request_syncs_the_document_before_the_request() {
        // A follower's code-context op (e.g. textDocument/definition) goes
        // through the local lsp_request_with_document, which opens/syncs the
        // document before issuing the request so the server analyzes the
        // current on-disk content. Routed to the leader, the same contract must
        // hold: when the request carries a `file_path`, dispatch must sync the
        // document (didOpen) on the leader's session before the request, or the
        // server answers against a buffer it has never seen.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("main.rs");
        std::fs::write(&file, "fn main() {}\n").unwrap();

        let log = Arc::new(Mutex::new(Vec::new()));
        let transport = SharedRecordingTransport {
            log: Arc::clone(&log),
        };
        let session = LspSession::new(Arc::new(Mutex::new(Some(transport))), "rust");

        let uri = format!("file://{}", file.display());
        dispatch(
            &session,
            &PrecomputedDependents::default(),
            &ManualTimer::default(),
            &DiagnosticsConfig::default(),
            METHOD_LSP_REQUEST,
            json!({
                "method": "textDocument/definition",
                "params": {
                    "textDocument": { "uri": uri },
                    "position": { "line": 0, "character": 3 }
                },
                "file_path": file.to_string_lossy(),
            }),
        )
        .await
        .expect("lsp request with a file_path should route");

        let recorded = log.lock().unwrap().clone();
        let methods: Vec<&str> = recorded.iter().map(|(m, _)| m.as_str()).collect();
        let open_idx = methods
            .iter()
            .position(|m| *m == "textDocument/didOpen")
            .expect("a didOpen must be emitted to sync the document on the leader");
        let req_idx = methods
            .iter()
            .position(|m| *m == "textDocument/definition")
            .expect("the routed request must be issued");
        assert!(
            open_idx < req_idx,
            "the document must be synced BEFORE the request: {methods:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dispatch_lsp_request_against_dead_session_reports_error() {
        // No live client → session.request fails with NotRunning, surfaced as a
        // dispatch error string (not a panic).
        let session = seeded_session();
        let err = dispatch(
            &session,
            &PrecomputedDependents::default(),
            &ManualTimer::default(),
            &DiagnosticsConfig::default(),
            METHOD_LSP_REQUEST,
            json!({ "method": "textDocument/definition", "params": {} }),
        )
        .await
        .expect_err("lsp request without a live client must error");
        assert!(err.contains("lsp request failed"), "got: {err}");
    }

    #[test]
    fn lsp_request_envelope_roundtrips_through_parse_with_document() {
        // The client builds the envelope; the leader parses it. A document-scoped
        // request must carry the file_path end to end so the leader syncs it.
        let envelope = lsp_request_envelope(
            "textDocument/hover",
            json!({ "position": { "line": 1, "character": 2 } }),
            Some("/repo/src/a.rs"),
        );
        let (method, params, file_path) =
            parse_lsp_request(&envelope).expect("envelope must parse");
        assert_eq!(method, "textDocument/hover");
        assert_eq!(params["position"]["line"], 1);
        assert_eq!(file_path.as_deref(), Some("/repo/src/a.rs"));
    }

    #[test]
    fn lsp_request_envelope_without_document_carries_no_file_path() {
        // A workspace-wide op (workspace/symbol) carries no document scope, so the
        // leader must not try to sync any file.
        let envelope = lsp_request_envelope("workspace/symbol", json!({ "query": "foo" }), None);
        assert!(
            envelope.get("file_path").is_none(),
            "no file_path key when document-less"
        );
        let (_method, _params, file_path) =
            parse_lsp_request(&envelope).expect("envelope must parse");
        assert_eq!(file_path, None);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dispatch_lsp_multi_request_runs_steps_in_order_under_one_lock() {
        // The multi-request op runs an ordered batch of LSP requests as ONE
        // atomic exchange on the leader's single session. dispatch must sync the
        // document (didOpen) first, then issue each step in order, and return one
        // raw envelope per step. The recording transport logs the wire order so
        // we can assert didOpen precedes the steps and the steps keep their order.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("main.rs");
        std::fs::write(&file, "fn main() {}\n").unwrap();

        let log = Arc::new(Mutex::new(Vec::new()));
        let transport = SharedRecordingTransport {
            log: Arc::clone(&log),
        };
        let session = LspSession::new(Arc::new(Mutex::new(Some(transport))), "rust");

        let uri = format!("file://{}", file.display());
        let result = dispatch(
            &session,
            &PrecomputedDependents::default(),
            &ManualTimer::default(),
            &DiagnosticsConfig::default(),
            METHOD_LSP_MULTI_REQUEST,
            json!({
                "file_path": file.to_string_lossy(),
                "steps": [
                    { "method": "textDocument/prepareRename",
                      "params": { "textDocument": { "uri": uri }, "position": { "line": 0, "character": 3 } } },
                    { "method": "textDocument/rename",
                      "params": { "textDocument": { "uri": uri }, "position": { "line": 0, "character": 3 }, "newName": "x" } }
                ]
            }),
        )
        .await
        .expect("multi request should dispatch");

        // The result is an ordered list of raw step responses (one per step).
        let steps_out = result.as_array().expect("multi result is an array");
        assert_eq!(steps_out.len(), 2, "one response per step");

        let recorded = log.lock().unwrap().clone();
        let methods: Vec<&str> = recorded.iter().map(|(m, _)| m.as_str()).collect();
        let open_idx = methods
            .iter()
            .position(|m| *m == "textDocument/didOpen")
            .expect("a didOpen must sync the document on the leader first");
        let prepare_idx = methods
            .iter()
            .position(|m| *m == "textDocument/prepareRename")
            .expect("prepareRename must be issued");
        let rename_idx = methods
            .iter()
            .position(|m| *m == "textDocument/rename")
            .expect("rename must be issued");
        assert!(
            open_idx < prepare_idx,
            "didOpen before the first step: {methods:?}"
        );
        assert!(
            prepare_idx < rename_idx,
            "steps must run in order: {methods:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dispatch_lsp_multi_request_against_dead_session_reports_error() {
        // No live client → with_client yields no client; the multi-request op
        // must surface an error string (not a panic, not a silent empty).
        let session = seeded_session();
        let err = dispatch(
            &session,
            &PrecomputedDependents::default(),
            &ManualTimer::default(),
            &DiagnosticsConfig::default(),
            METHOD_LSP_MULTI_REQUEST,
            json!({
                "file_path": "/does/not/matter.rs",
                "steps": [ { "method": "textDocument/prepareRename", "params": {} } ]
            }),
        )
        .await
        .expect_err("multi request without a live client must error");
        assert!(err.contains("lsp"), "got: {err}");
    }

    #[test]
    fn lsp_multi_request_envelope_roundtrips_through_parse() {
        // The client builds the multi-request envelope; the leader parses it.
        let envelope = lsp_multi_request_envelope(
            "/repo/src/a.rs",
            vec![
                ("textDocument/prepareRename".to_string(), json!({ "a": 1 })),
                ("textDocument/rename".to_string(), json!({ "b": 2 })),
            ],
        );
        let (file_path, steps) = parse_lsp_multi_request(&envelope).expect("envelope must parse");
        assert_eq!(file_path.as_deref(), Some("/repo/src/a.rs"));
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].0, "textDocument/prepareRename");
        assert_eq!(steps[1].1["b"], 2);
    }

    #[test]
    fn parse_paths_rejects_non_string_entries() {
        let err = parse_paths(&json!({ "paths": [1, 2] })).expect_err("non-string must reject");
        assert!(err.contains("must be a string"));
    }

    #[test]
    fn parse_paths_rejects_parent_dir_traversal() {
        let err = parse_paths(&json!({ "paths": ["src/../../etc/passwd"] }))
            .expect_err("a `..` traversal component must be rejected");
        assert!(err.contains("parent-dir"), "got: {err}");
    }

    #[test]
    fn parse_lsp_request_rejects_parent_dir_traversal() {
        // The leader reads + sync_opens `file_path` on its session, so a
        // follower-supplied `..` traversal would let it open an arbitrary file.
        // The single-request seam must reject it like parse_paths does.
        let err = parse_lsp_request(&json!({
            "method": "textDocument/definition",
            "params": {},
            "file_path": "src/../../etc/passwd"
        }))
        .expect_err("a `..` traversal in file_path must be rejected");
        assert!(err.contains("parent-dir"), "got: {err}");
    }

    #[test]
    fn parse_lsp_request_accepts_absolute_file_path() {
        // The leader read surface is absolute-space, so a `..`-free absolute
        // file_path is a legitimate follower request and must be accepted.
        let (_method, _params, file_path) = parse_lsp_request(&json!({
            "method": "textDocument/definition",
            "params": {},
            "file_path": "/repo/src/a.rs"
        }))
        .expect("a `..`-free absolute file_path must be accepted");
        assert_eq!(file_path.as_deref(), Some("/repo/src/a.rs"));
    }

    #[test]
    fn parse_lsp_multi_request_rejects_parent_dir_traversal() {
        // The multi-request seam reads + sync_opens `file_path` on the leader's
        // session too, so it must reject `..` traversal like parse_paths does.
        let err = parse_lsp_multi_request(&json!({
            "file_path": "src/../../etc/passwd",
            "steps": [ { "method": "textDocument/prepareRename", "params": {} } ]
        }))
        .expect_err("a `..` traversal in file_path must be rejected");
        assert!(err.contains("parent-dir"), "got: {err}");
    }

    #[test]
    fn parse_lsp_multi_request_accepts_absolute_file_path() {
        // A `..`-free absolute file_path is a legitimate follower batch request.
        let (file_path, _steps) = parse_lsp_multi_request(&json!({
            "file_path": "/repo/src/a.rs",
            "steps": [ { "method": "textDocument/prepareRename", "params": {} } ]
        }))
        .expect("a `..`-free absolute file_path must be accepted");
        assert_eq!(file_path.as_deref(), Some("/repo/src/a.rs"));
    }

    #[test]
    fn parse_paths_accepts_absolute_repo_paths() {
        // diagnose is an absolute-space API, so a `..`-free absolute path is a
        // legitimate follower request and must be accepted.
        let paths = parse_paths(&json!({ "paths": ["/repo/src/a.rs", "src/b.rs"] }))
            .expect("`..`-free paths must be accepted");
        assert_eq!(
            paths,
            vec!["/repo/src/a.rs".to_string(), "src/b.rs".to_string()]
        );
    }
}
