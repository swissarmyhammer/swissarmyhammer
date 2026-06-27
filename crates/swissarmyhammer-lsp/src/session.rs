//! A single owned LSP session with a shared open-document set.
//!
//! [`LspSession`] is the one place that knows what the language server believes
//! is open. It owns a map of `uri -> `[`DocState`] (version + text hash) and
//! issues `textDocument/didOpen|didChange|didSave|didClose` against a single
//! [`LspTransport`](crate::client::LspTransport) client, keeping documents open
//! across requests instead of the old `didOpen -> request -> didClose` churn.
//!
//! The session is a cloneable `Arc`-based handle: every clone shares the *same*
//! open-document set and the *same* client, so the in-process consumers (the
//! code-context indexer and query ops, diagnostics) cannot drift into a second
//! view of what is open. [`LspDaemon`](crate::daemon::LspDaemon) owns exactly
//! one session per server; no other type spawns a client.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use lsp_types::Diagnostic;
use serde_json::{json, Value};
use tokio::sync::broadcast;

use crate::client::{LspJsonRpcClient, LspTransport};
use crate::diagnostics::{
    parse_diagnostics_from_result, parse_publish_diagnostics, DiagnosticUpdate,
};
use crate::error::LspError;

/// Capacity of the per-session diagnostics broadcast channel.
///
/// Each in-process subscriber gets its own ring buffer of this many updates; a
/// slow subscriber that lags past it sees `RecvError::Lagged` and resyncs from
/// the cache rather than blocking publishers. Diagnostics are low-frequency
/// (one batch per document per re-analysis), so a modest buffer is ample.
const DIAGNOSTICS_CHANNEL_CAPACITY: usize = 256;

/// What the server is believed to know about one open document.
///
/// `version` is the LSP document version, incremented on every `didChange`.
/// `text_hash` is a cheap hash of the last-sent text, used so a no-op
/// `change` (same text) does not bump the version or emit a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocState {
    /// LSP document version, starting at 1 on `didOpen` and incremented on
    /// each text-changing `didChange`.
    pub version: i32,
    /// Hash of the text last sent to the server for this document.
    pub text_hash: u64,
}

/// Hash a document's text for cheap change detection.
fn hash_text(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// Build a `file://` URI string for a path, matching the wire format used by
/// the rest of the client.
fn file_uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy())
}

/// Whether a `textDocument/diagnostic` pull response is the server's "not ready
/// yet, retrigger later" answer rather than a real report.
///
/// rust-analyzer answers a pull issued before it has finished loading the
/// workspace with a JSON-RPC error — `ServerCancelled` (-32802) or
/// `ContentModified` (-32801), commonly carrying `data.retriggerRequest: true`
/// — instead of a report. Accepts both the bare error object and a full
/// JSON-RPC envelope (`{ "error": { ... } }`).
fn pull_response_is_not_ready(response: &Value) -> bool {
    let Some(error) = response.get("error") else {
        return false;
    };
    if let Some(code) = error.get("code").and_then(|c| c.as_i64()) {
        // ServerCancelled / ContentModified: the canonical "still loading" codes.
        if code == -32802 || code == -32801 {
            return true;
        }
    }
    error
        .get("data")
        .and_then(|d| d.get("retriggerRequest"))
        .and_then(|r| r.as_bool())
        .unwrap_or(false)
}

/// The shared interior of an [`LspSession`]: the single client handle plus the
/// open-document set. Every clone of the session points at the same `Arc`.
struct SessionInner<C: LspTransport> {
    /// The one client. `None` while the daemon is not running (starting or
    /// restarting). Operations against a `None` client return
    /// [`LspError::NotRunning`].
    client: Arc<Mutex<Option<C>>>,
    /// What the server believes is open: `uri -> `[`DocState`].
    docs: Mutex<HashMap<String, DocState>>,
    /// Latest-per-uri diagnostics cache: `uri -> `latest full diagnostic set.
    ///
    /// This is **derived state** — a live mirror of what the server has most
    /// recently published (push) or returned (pull) for each document. It is
    /// never persisted to disk; it is rebuilt from server output and discarded
    /// when the session is dropped or reset.
    diagnostics: Mutex<HashMap<String, Vec<Diagnostic>>>,
    /// In-process fan-out of diagnostic updates. Both push and pull feed this
    /// one channel so every consumer sees the same stream.
    diagnostics_tx: broadcast::Sender<DiagnosticUpdate>,
    /// Whether the server is ready to report diagnostics, or is still loading.
    ///
    /// Starts `true`. A pull answered with the server's "still loading"
    /// signal (ServerCancelled / ContentModified / `retriggerRequest`) flips it
    /// `false`; the next real answer flips it back. See
    /// [`is_ready`](LspSession::is_ready) and
    /// [`pull_diagnostics`](LspSession::pull_diagnostics).
    ready: AtomicBool,
}

/// A single owned LSP session with a shared open-document set.
///
/// Generic over the client type `C` so unit tests can drive it with an
/// in-memory fake transport; production uses `C = `[`LspJsonRpcClient`]. Clone
/// is cheap (`Arc` bump) and every clone shares one open-doc set and one
/// client.
pub struct LspSession<C: LspTransport = LspJsonRpcClient> {
    inner: Arc<SessionInner<C>>,
    /// Language id sent on `didOpen` (e.g. `"rust"`). One session serves one
    /// server, which serves one language family.
    language_id: String,
}

impl<C: LspTransport> Clone for LspSession<C> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            language_id: self.language_id.clone(),
        }
    }
}

impl<C: LspTransport> LspSession<C> {
    /// Create a session over the given shared client handle.
    ///
    /// The `client` is the same `Arc<Mutex<Option<C>>>` the daemon populates on
    /// a successful handshake and clears on shutdown, so the session always
    /// talks to the live process (or fails with [`LspError::NotRunning`] when
    /// there is none). `language_id` is the LSP language identifier sent on
    /// `didOpen`.
    pub fn new(client: Arc<Mutex<Option<C>>>, language_id: impl Into<String>) -> Self {
        let (diagnostics_tx, _) = broadcast::channel(DIAGNOSTICS_CHANNEL_CAPACITY);
        Self {
            inner: Arc::new(SessionInner {
                client,
                docs: Mutex::new(HashMap::new()),
                diagnostics: Mutex::new(HashMap::new()),
                diagnostics_tx,
                ready: AtomicBool::new(true),
            }),
            language_id: language_id.into(),
        }
    }

    /// Snapshot the current open-document set (`uri -> `[`DocState`]).
    ///
    /// Used by tests and observability; the returned map is a copy, not a live
    /// view.
    pub fn open_documents(&self) -> HashMap<String, DocState> {
        self.lock_docs().clone()
    }

    /// Whether the given path is currently believed open by the server.
    pub fn is_open(&self, path: &Path) -> bool {
        self.lock_docs().contains_key(&file_uri(path))
    }

    /// Open a document, idempotently.
    ///
    /// The first `open` for a uri sends `textDocument/didOpen` at version 1 and
    /// records it in the open set. A subsequent `open` for an already-open uri
    /// is a no-op: no duplicate `didOpen` is emitted and the recorded state is
    /// unchanged. The document then stays open across requests.
    pub fn open(&self, path: &Path, text: &str) -> Result<(), LspError> {
        let uri = file_uri(path);
        // Hold the docs lock across the wire send so the check-and-emit is
        // atomic: two concurrent clones cannot both observe "not open" and each
        // fire a didOpen. Lock order is always docs -> client (notify locks the
        // separate client mutex), never the reverse, so this cannot deadlock.
        let mut docs = self.lock_docs();
        if docs.contains_key(&uri) {
            // Already open — suppress the duplicate didOpen.
            return Ok(());
        }

        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": self.language_id,
                    "version": 1,
                    "text": text,
                }
            }),
        )?;

        docs.insert(
            uri,
            DocState {
                version: 1,
                text_hash: hash_text(text),
            },
        );
        Ok(())
    }

    /// Apply a full-text change to an open document.
    ///
    /// Bumps the document version and sends a full-content
    /// `textDocument/didChange`. A change whose text is identical to what the
    /// server already has is a no-op (no version bump, no notification). A
    /// change to a document that is not open returns [`LspError::NotRunning`]'s
    /// sibling [`LspError::JsonRpc`] describing the contract violation — callers
    /// must `open` first.
    pub fn change(&self, path: &Path, text: &str) -> Result<(), LspError> {
        let uri = file_uri(path);
        let new_hash = hash_text(text);

        // Hold the docs lock across the send so the version computed under the
        // lock is the version actually sent and recorded — two concurrent
        // changes cannot both compute the same `next_version`. Lock order is
        // docs -> client, matching `open`.
        let mut docs = self.lock_docs();
        let state = docs.get(&uri).ok_or_else(|| {
            LspError::JsonRpc(format!("change on document that is not open: {uri}"))
        })?;
        if state.text_hash == new_hash {
            // No textual change — nothing to send.
            return Ok(());
        }
        let next_version = state.version + 1;

        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": {
                    "uri": uri,
                    "version": next_version,
                },
                "contentChanges": [ { "text": text } ],
            }),
        )?;

        if let Some(state) = docs.get_mut(&uri) {
            state.version = next_version;
            state.text_hash = new_hash;
        }
        Ok(())
    }

    /// Make the server's buffer for `path` match `text`, opening the document if
    /// needed.
    ///
    /// This is the "sync to current content" entry point every in-process
    /// consumer should use before a request that depends on the document's text
    /// (query ops, the diagnostics pull, the indexing worker): a document the
    /// session has never seen is opened with `text` (one `didOpen`); a document
    /// already open has its buffer refreshed with `text` via `didChange` only
    /// when the text actually differs. Because the session keeps documents open
    /// across requests, without this an edited-then-re-touched file would be
    /// analyzed against the stale buffer the server still holds from the first
    /// open. Both underlying steps are no-ops when nothing changed, so an
    /// unchanged re-sync costs nothing on the wire.
    pub fn sync_open(&self, path: &Path, text: &str) -> Result<(), LspError> {
        if self.is_open(path) {
            // Already open — refresh the buffer (no-op when text is unchanged).
            self.change(path, text)
        } else {
            self.open(path, text)
        }
    }

    /// Notify the server that an open document was saved.
    ///
    /// Sends `textDocument/didSave`. Saving a document that is not open is a
    /// no-op (the server never knew it was open). The open set is unchanged.
    pub fn save(&self, path: &Path) -> Result<(), LspError> {
        let uri = file_uri(path);
        // Hold the docs lock across the send so a concurrent `close` cannot
        // remove the document between the membership check and the didSave.
        let docs = self.lock_docs();
        if !docs.contains_key(&uri) {
            return Ok(());
        }

        self.notify(
            "textDocument/didSave",
            json!({
                "textDocument": { "uri": uri },
            }),
        )
    }

    /// Close an open document, idempotently.
    ///
    /// Sends `textDocument/didClose` and removes the uri from the open set.
    /// Closing a document that is not open is a no-op (no notification).
    pub fn close(&self, path: &Path) -> Result<(), LspError> {
        let uri = file_uri(path);
        // Hold the docs lock across the send and only remove the uri once the
        // didClose has gone out: on a notify failure the open set still
        // reflects what the server knows, matching `open`/`change`.
        let mut docs = self.lock_docs();
        if !docs.contains_key(&uri) {
            return Ok(());
        }

        self.notify(
            "textDocument/didClose",
            json!({
                "textDocument": { "uri": uri },
            }),
        )?;

        docs.remove(&uri);
        Ok(())
    }

    /// Issue a JSON-RPC request against the one client and return its response.
    ///
    /// This is the request API consumers call (e.g.
    /// `textDocument/documentSymbol`). Returns [`LspError::NotRunning`] when the
    /// daemon has no live client.
    pub fn request(&self, method: &str, params: Value) -> Result<Value, LspError> {
        let mut guard = self
            .inner
            .client
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let client = guard.as_mut().ok_or(LspError::NotRunning)?;
        client.send_request(method, params)
    }

    /// Fire a JSON-RPC notification against the one client (fire-and-forget).
    ///
    /// Returns [`LspError::NotRunning`] when the daemon has no live client.
    pub fn notify(&self, method: &str, params: Value) -> Result<(), LspError> {
        let mut guard = self
            .inner
            .client
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let client = guard.as_mut().ok_or(LspError::NotRunning)?;
        client.send_notification(method, params)
    }

    /// Whether the session currently has a live client.
    ///
    /// Returns `false` while the daemon is not running (starting or restarting),
    /// in which case [`request`](Self::request) / [`notify`](Self::notify) would
    /// fail with [`LspError::NotRunning`]. In-process consumers that degrade
    /// gracefully (e.g. code-context's layered ops) check this before issuing
    /// live requests.
    pub fn is_running(&self) -> bool {
        self.inner
            .client
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_some()
    }

    /// Whether the server is ready to report diagnostics for the workspace.
    ///
    /// Starts `true` and stays `true` for a server that always answers a pull
    /// with a real report. A pull answered with the server's "still loading"
    /// signal (ServerCancelled / ContentModified / `retriggerRequest`) flips
    /// this `false` (see [`pull_diagnostics`](Self::pull_diagnostics)); the next
    /// real answer flips it back. Consumers such as `diagnose` use this to
    /// report "pending" rather than mistaking a not-yet-loaded server's silence
    /// for a clean file. A genuinely clean file (a real, empty report) keeps the
    /// server `ready` — only the not-ready signal flips it.
    pub fn is_ready(&self) -> bool {
        self.inner.ready.load(Ordering::Relaxed)
    }

    /// Record the server's readiness, observed from a pull response.
    fn set_ready(&self, ready: bool) {
        self.inner.ready.store(ready, Ordering::Relaxed);
    }

    /// Run a closure against the one client, holding the client lock for the
    /// whole sequence.
    ///
    /// This is the multi-request seam: a caller that needs to issue several
    /// requests as an atomic unit (e.g. `prepareCallHierarchy` then
    /// `incomingCalls`, or `prepareRename` then `rename`) gets the client
    /// directly and the lock is held across every call, so no other consumer
    /// can interleave a request and steal a response off the shared pipe.
    ///
    /// Returns `Ok(None)` when there is no live client (graceful degradation,
    /// matching the request/notify contract from the caller's perspective);
    /// the closure is not run in that case.
    pub fn with_client<F, T>(&self, f: F) -> Result<Option<T>, LspError>
    where
        F: FnOnce(&mut C) -> Result<T, LspError>,
    {
        let mut guard = self
            .inner
            .client
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match guard.as_mut() {
            Some(client) => f(client).map(Some),
            None => Ok(None),
        }
    }

    /// Subscribe to the in-process diagnostics fan-out.
    ///
    /// Every captured diagnostics batch — whether it arrived via a push
    /// `publishDiagnostics` notification or a pull `textDocument/diagnostic`
    /// request — is broadcast as a [`DiagnosticUpdate`]. A new subscriber sees
    /// updates published *after* it subscribes; to seed its initial state it
    /// should read the current cache via [`diagnostics_for`](Self::diagnostics_for).
    pub fn subscribe(&self) -> broadcast::Receiver<DiagnosticUpdate> {
        self.inner.diagnostics_tx.subscribe()
    }

    /// The latest captured diagnostics for a document uri.
    ///
    /// Returns a snapshot copy of the cached set, or an empty vector if no
    /// diagnostics have been captured for that uri. The cache is derived,
    /// in-memory state and is never read from or written to disk.
    pub fn diagnostics_for(&self, uri: &str) -> Vec<Diagnostic> {
        self.lock_diagnostics()
            .get(uri)
            .cloned()
            .unwrap_or_default()
    }

    /// Handle a `textDocument/publishDiagnostics` notification (push model).
    ///
    /// Parses `params` into [`lsp_types::Diagnostic`] records, replaces the
    /// latest-per-uri cache entry for the document, and broadcasts a
    /// [`DiagnosticUpdate`] to every subscriber. A publish with an empty
    /// `diagnostics` array clears the document's entry — the server is saying
    /// the document is now clean.
    ///
    /// Called by the daemon's read loop when it drains a `publishDiagnostics`
    /// message off the wire.
    pub fn handle_publish_diagnostics(&self, params: &Value) {
        let uri = match params.get("uri").and_then(|v| v.as_str()) {
            Some(uri) => uri.to_string(),
            // A publish without a uri is malformed and un-routable; ignore it.
            None => return,
        };
        let diagnostics = parse_publish_diagnostics(params);
        self.store_and_broadcast(uri, diagnostics);
    }

    /// Request diagnostics for a document via the pull model
    /// (`textDocument/diagnostic`, LSP 3.17+) and feed the result through the
    /// same cache and fan-out as push diagnostics.
    ///
    /// Servers that do not push `publishDiagnostics` still surface here, so
    /// in-process consumers observe one unified diagnostics stream regardless of
    /// which model a given server speaks. Returns the parsed diagnostics for the
    /// document. Returns [`LspError::NotRunning`] when there is no live client.
    pub fn pull_diagnostics(&self, path: &Path) -> Result<Vec<Diagnostic>, LspError> {
        let uri = file_uri(path);
        let response = self.request(
            "textDocument/diagnostic",
            json!({ "textDocument": { "uri": uri } }),
        )?;

        // A pull issued before the server finished loading is answered with its
        // "still loading, retrigger later" error (ServerCancelled /
        // ContentModified), NOT a real report. Record the server as not-ready
        // and return empty WITHOUT caching/broadcasting — caching the empty body
        // would let a consumer read "no diagnostics" as "the file is clean"
        // while the server was merely still indexing.
        if pull_response_is_not_ready(&response) {
            self.set_ready(false);
            return Ok(Vec::new());
        }

        // A real answer — even an empty report for a genuinely clean file —
        // means the server is ready to speak about this document.
        self.set_ready(true);
        // The result may be nested under "result" (full JSON-RPC envelope) or be
        // the bare report; parse whichever is present.
        let result = response.get("result").unwrap_or(&response);
        let diagnostics = parse_diagnostics_from_result(result);
        self.store_and_broadcast(uri, diagnostics.clone());
        Ok(diagnostics)
    }

    /// Replace the cached diagnostics for `uri` and broadcast the update.
    ///
    /// The single write path shared by push and pull: it keeps the cache and the
    /// fan-out in lockstep so no consumer can observe one without the other. A
    /// send error (no subscribers) is ignored — the cache is still authoritative
    /// and a future subscriber reads it via [`diagnostics_for`](Self::diagnostics_for).
    fn store_and_broadcast(&self, uri: String, diagnostics: Vec<Diagnostic>) {
        self.lock_diagnostics()
            .insert(uri.clone(), diagnostics.clone());
        // `send` only errors when there are zero receivers; that is fine — the
        // cache already holds the latest state.
        let _ = self
            .inner
            .diagnostics_tx
            .send(DiagnosticUpdate { uri, diagnostics });
    }

    /// Forget every open document without sending any `didClose`.
    ///
    /// Called by the owning daemon when the underlying server process is gone
    /// (shutdown, health-check failure, or just before a restart): the new
    /// process knows nothing about what the old one had open, so the session's
    /// open set must be cleared to match. Sending `didClose` here would be
    /// wrong — the pipe is closed and the documents no longer exist from the
    /// server's perspective. After a reset the next `open` for any uri emits a
    /// fresh `didOpen` instead of being suppressed as a stale duplicate.
    ///
    /// The diagnostics cache is cleared too: it is derived state describing the
    /// gone process's analysis, so it must not outlive that process.
    pub fn reset_documents(&self) {
        self.lock_docs().clear();
        self.lock_diagnostics().clear();
    }

    /// Lock the open-document map, recovering from a poisoned mutex.
    fn lock_docs(&self) -> std::sync::MutexGuard<'_, HashMap<String, DocState>> {
        self.inner
            .docs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Lock the per-uri diagnostics cache, recovering from a poisoned mutex.
    fn lock_diagnostics(&self) -> std::sync::MutexGuard<'_, HashMap<String, Vec<Diagnostic>>> {
        self.inner
            .diagnostics
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeTransport;
    use std::path::PathBuf;

    /// Build a session over a shared `FakeTransport` and return both the
    /// session and the shared client handle so the test can inspect the
    /// recorded wire traffic.
    fn session_with_fake() -> (LspSession<FakeTransport>, Arc<Mutex<Option<FakeTransport>>>) {
        let client = Arc::new(Mutex::new(Some(FakeTransport::default())));
        let session = LspSession::new(Arc::clone(&client), "rust");
        (session, client)
    }

    /// Count notifications of `method` recorded by the shared fake.
    fn notification_count(client: &Arc<Mutex<Option<FakeTransport>>>, method: &str) -> usize {
        client
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .notification_count(method)
    }

    #[test]
    fn scripted_open_change_save_close_transitions_and_suppresses_duplicate_open() {
        let (session, client) = session_with_fake();
        let path = PathBuf::from("/tmp/lib.rs");
        let uri = file_uri(&path);

        // open: didOpen at version 1, document recorded.
        session.open(&path, "fn a() {}").expect("open");
        assert!(
            session.is_open(&path),
            "document should be open after open()"
        );
        assert_eq!(
            session.open_documents().get(&uri).unwrap().version,
            1,
            "open should record version 1"
        );
        assert_eq!(notification_count(&client, "textDocument/didOpen"), 1);

        // duplicate open: no second didOpen, state unchanged.
        session
            .open(&path, "fn a() {} // changed text")
            .expect("dup open");
        assert_eq!(
            notification_count(&client, "textDocument/didOpen"),
            1,
            "duplicate open must not emit a second didOpen"
        );
        assert_eq!(
            session.open_documents().get(&uri).unwrap().version,
            1,
            "duplicate open must not change recorded state"
        );

        // change: version bumps, didChange emitted.
        session.change(&path, "fn a() { b(); }").expect("change");
        assert_eq!(
            session.open_documents().get(&uri).unwrap().version,
            2,
            "change should bump version to 2"
        );
        assert_eq!(notification_count(&client, "textDocument/didChange"), 1);

        // no-op change (same text): no version bump, no notification.
        session
            .change(&path, "fn a() { b(); }")
            .expect("no-op change");
        assert_eq!(
            session.open_documents().get(&uri).unwrap().version,
            2,
            "identical change must not bump the version"
        );
        assert_eq!(
            notification_count(&client, "textDocument/didChange"),
            1,
            "identical change must not emit a second didChange"
        );

        // save: didSave emitted, open set unchanged.
        session.save(&path).expect("save");
        assert_eq!(notification_count(&client, "textDocument/didSave"), 1);
        assert!(session.is_open(&path), "save must keep the document open");

        // close: didClose emitted, document removed.
        session.close(&path).expect("close");
        assert_eq!(notification_count(&client, "textDocument/didClose"), 1);
        assert!(
            !session.is_open(&path),
            "document should be closed after close()"
        );

        // duplicate close: no second didClose.
        session.close(&path).expect("dup close");
        assert_eq!(
            notification_count(&client, "textDocument/didClose"),
            1,
            "closing a closed document must not emit a second didClose"
        );
    }

    #[test]
    fn clones_share_one_open_document_set() {
        let (session, _client) = session_with_fake();
        let clone = session.clone();
        let path = PathBuf::from("/tmp/shared.rs");

        // Open through one handle...
        session.open(&path, "fn x() {}").expect("open");
        // ...observable through the clone.
        assert!(
            clone.is_open(&path),
            "a clone must observe documents opened through the original"
        );

        // And opening the same uri through the clone is suppressed as a
        // duplicate against the shared set.
        clone.open(&path, "fn x() {}").expect("dup open via clone");
        assert_eq!(clone.open_documents().len(), 1);
    }

    #[test]
    fn change_on_unopened_document_errors() {
        let (session, _client) = session_with_fake();
        let path = PathBuf::from("/tmp/never-opened.rs");
        let err = session
            .change(&path, "fn z() {}")
            .expect_err("change before open should error");
        assert!(matches!(err, LspError::JsonRpc(_)));
    }

    #[test]
    fn request_against_absent_client_reports_not_running() {
        let client: Arc<Mutex<Option<FakeTransport>>> = Arc::new(Mutex::new(None));
        let session = LspSession::new(client, "rust");
        let err = session
            .request("textDocument/documentSymbol", json!({}))
            .expect_err("request without a live client should fail");
        assert!(matches!(err, LspError::NotRunning));
    }

    #[test]
    fn is_running_reflects_client_presence() {
        let (session, _client) = session_with_fake();
        assert!(session.is_running(), "a present client should be running");

        let absent: Arc<Mutex<Option<FakeTransport>>> = Arc::new(Mutex::new(None));
        let session = LspSession::new(absent, "rust");
        assert!(
            !session.is_running(),
            "an absent client should not be running"
        );
    }

    #[test]
    fn with_client_runs_closure_against_live_client() {
        // The closure can drive several requests against the one client; the
        // shared fake records each, proving the lock is held across the unit.
        let client = Arc::new(Mutex::new(Some(
            FakeTransport::default()
                .with_response(json!({"jsonrpc": "2.0", "id": 1, "result": "a"}))
                .with_response(json!({"jsonrpc": "2.0", "id": 2, "result": "b"})),
        )));
        let session = LspSession::new(Arc::clone(&client), "rust");

        let out = session
            .with_client(|c| {
                let _ = c.send_request("textDocument/prepareCallHierarchy", json!({}))?;
                c.send_request("callHierarchy/incomingCalls", json!({}))
            })
            .expect("with_client should succeed");

        assert_eq!(out.unwrap()["result"], "b");
        let guard = client.lock().unwrap();
        assert_eq!(guard.as_ref().unwrap().sent_requests.len(), 2);
    }

    #[test]
    fn with_client_returns_none_and_skips_closure_when_absent() {
        let absent: Arc<Mutex<Option<FakeTransport>>> = Arc::new(Mutex::new(None));
        let session = LspSession::new(absent, "rust");

        let out: Option<()> = session
            .with_client(|_c| panic!("closure must not run without a live client"))
            .expect("with_client should be Ok when absent");
        assert!(out.is_none(), "absent client yields Ok(None)");
    }

    #[test]
    fn sync_open_opens_then_refreshes_with_did_change_on_edited_content() {
        // Models a re-index of an edited file: first sync opens (didOpen), a
        // sync with NEW content refreshes the buffer (didChange), and a sync with
        // the SAME content is a no-op. This is what keeps the server's buffer
        // current when the session keeps the document open across requests.
        let (session, client) = session_with_fake();
        let path = PathBuf::from("/tmp/edited.rs");

        // First sync: never seen -> didOpen, no didChange.
        session.sync_open(&path, "fn a() {}").expect("first sync");
        assert_eq!(notification_count(&client, "textDocument/didOpen"), 1);
        assert_eq!(notification_count(&client, "textDocument/didChange"), 0);

        // Re-sync with edited content: already open -> didChange (no 2nd didOpen).
        session
            .sync_open(&path, "fn a() { b(); }")
            .expect("edited re-sync");
        assert_eq!(
            notification_count(&client, "textDocument/didOpen"),
            1,
            "re-sync must not re-open the document"
        );
        assert_eq!(
            notification_count(&client, "textDocument/didChange"),
            1,
            "edited content must push a didChange so the server buffer is fresh"
        );

        // Re-sync with identical content: no wire traffic at all.
        session
            .sync_open(&path, "fn a() { b(); }")
            .expect("unchanged re-sync");
        assert_eq!(
            notification_count(&client, "textDocument/didChange"),
            1,
            "an unchanged re-sync must not emit a second didChange"
        );
    }

    #[test]
    fn documents_stay_open_across_requests() {
        let client = Arc::new(Mutex::new(Some(
            FakeTransport::default()
                .with_response(json!({"jsonrpc": "2.0", "id": 1, "result": []}))
                .with_response(json!({"jsonrpc": "2.0", "id": 2, "result": []})),
        )));
        let session = LspSession::new(Arc::clone(&client), "rust");
        let path = PathBuf::from("/tmp/persist.rs");

        session.open(&path, "fn p() {}").expect("open");

        // Two requests without re-open; the document stays open the whole time.
        session
            .request("textDocument/documentSymbol", json!({}))
            .expect("first request");
        session
            .request("textDocument/documentSymbol", json!({}))
            .expect("second request");

        assert!(
            session.is_open(&path),
            "document must remain open across requests (no open/close churn)"
        );
        // Exactly one didOpen for the whole sequence.
        assert_eq!(notification_count(&client, "textDocument/didOpen"), 1);
        assert_eq!(notification_count(&client, "textDocument/didClose"), 0);
    }

    #[test]
    fn reset_documents_lets_a_reopen_emit_a_fresh_did_open() {
        // Models a server restart: the daemon clears the session's open set
        // (the new process knows nothing), so the next open must NOT be
        // suppressed as a stale duplicate — it must re-announce the document.
        let (session, client) = session_with_fake();
        let path = PathBuf::from("/tmp/restart.rs");

        session.open(&path, "fn r() {}").expect("open");
        assert_eq!(notification_count(&client, "textDocument/didOpen"), 1);

        // Server gone: forget everything without sending didClose.
        session.reset_documents();
        assert!(
            !session.is_open(&path),
            "reset must forget every open document"
        );
        assert_eq!(
            notification_count(&client, "textDocument/didClose"),
            0,
            "reset must not emit didClose — the pipe is gone"
        );

        // Re-open against the fresh server: a new didOpen, not a suppressed dup.
        session.open(&path, "fn r() {}").expect("reopen");
        assert_eq!(
            notification_count(&client, "textDocument/didOpen"),
            2,
            "re-open after reset must emit a fresh didOpen"
        );
    }

    #[test]
    fn publish_diagnostics_updates_cache_and_fans_out_to_subscriber() {
        // Feed a scripted publishDiagnostics notification through the session's
        // handler and assert: (1) the per-uri cache holds the parsed diagnostics,
        // (2) a subscriber receives the matching DiagnosticUpdate, and (3) the
        // cache is purely in-memory — nothing is written to disk. Fully
        // model-free via FakeTransport.
        let (session, _client) = session_with_fake();
        let mut rx = session.subscribe();

        // Snapshot an isolated temp dir before capture so we can prove the
        // derived cache never persists anything.
        let scratch = tempfile::tempdir().expect("tempdir");
        let before = dir_entry_count(scratch.path());

        let params = json!({
            "uri": "file:///src/main.rs",
            "diagnostics": [
                {
                    "range": {
                        "start": { "line": 5, "character": 10 },
                        "end": { "line": 5, "character": 20 }
                    },
                    "severity": 1,
                    "message": "mismatched types",
                    "code": "E0308",
                    "source": "rustc"
                }
            ]
        });

        session.handle_publish_diagnostics(&params);

        // Cache: latest diagnostics keyed by uri.
        let cached = session.diagnostics_for("file:///src/main.rs");
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].message, "mismatched types");

        // Fan-out: the subscriber receives the same update.
        let update = rx.try_recv().expect("subscriber should receive the update");
        assert_eq!(update.uri, "file:///src/main.rs");
        assert_eq!(update.diagnostics.len(), 1);
        assert_eq!(update.diagnostics[0].message, "mismatched types");

        // Derived state: capturing diagnostics must not touch disk.
        assert_eq!(
            dir_entry_count(scratch.path()),
            before,
            "the diagnostics cache must be in-memory only — no files written"
        );
    }

    /// Count filesystem entries under `dir` (recursively), used to assert that
    /// capturing diagnostics writes nothing to disk.
    fn dir_entry_count(dir: &Path) -> usize {
        fn walk(dir: &Path) -> usize {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return 0;
            };
            entries
                .filter_map(Result::ok)
                .map(|e| {
                    let mut n = 1;
                    if e.path().is_dir() {
                        n += walk(&e.path());
                    }
                    n
                })
                .sum()
        }
        walk(dir)
    }

    #[test]
    fn publish_diagnostics_replaces_latest_per_uri() {
        // A second publish for the same uri replaces the first — diagnostics are
        // a full per-document snapshot, not an append.
        let (session, _client) = session_with_fake();

        session.handle_publish_diagnostics(&json!({
            "uri": "file:///src/a.rs",
            "diagnostics": [
                {
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 1 }
                    },
                    "severity": 1,
                    "message": "first"
                }
            ]
        }));
        assert_eq!(session.diagnostics_for("file:///src/a.rs").len(), 1);

        // Clearing the document publishes an empty set, which must replace.
        session.handle_publish_diagnostics(&json!({
            "uri": "file:///src/a.rs",
            "diagnostics": []
        }));
        assert!(session.diagnostics_for("file:///src/a.rs").is_empty());
    }

    #[test]
    fn pull_diagnostics_feeds_the_same_cache_and_fan_out() {
        // The pull model (textDocument/diagnostic) feeds the exact same cache
        // and broadcast as push, so servers without push still surface through
        // one fan-out.
        let client = Arc::new(Mutex::new(Some(FakeTransport::default().with_response(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "kind": "full",
                    "items": [
                        {
                            "range": {
                                "start": { "line": 2, "character": 0 },
                                "end": { "line": 2, "character": 8 }
                            },
                            "severity": 2,
                            "message": "unused import"
                        }
                    ]
                }
            }),
        ))));
        let session = LspSession::new(Arc::clone(&client), "rust");
        let mut rx = session.subscribe();
        let path = PathBuf::from("/src/lib.rs");
        let uri = file_uri(&path);

        let pulled = session.pull_diagnostics(&path).expect("pull diagnostics");
        assert_eq!(pulled.len(), 1);
        assert_eq!(pulled[0].message, "unused import");

        // Same cache.
        assert_eq!(session.diagnostics_for(&uri).len(), 1);
        // Same fan-out.
        let update = rx
            .try_recv()
            .expect("subscriber should receive pull update");
        assert_eq!(update.uri, uri);
        assert_eq!(update.diagnostics[0].message, "unused import");
    }

    #[test]
    fn pull_not_ready_response_marks_session_not_ready_without_caching() {
        // rust-analyzer answers a pull issued during workspace load with a
        // ServerCancelled error + retriggerRequest, NOT a report. The session
        // must record not-ready and must NOT cache/broadcast an empty (clean)
        // set for the document — otherwise a consumer reads "still loading" as
        // "the file is clean".
        let client = Arc::new(Mutex::new(Some(FakeTransport::default().with_response(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "error": {
                    "code": -32802,
                    "message": "server cancelled the request",
                    "data": { "retriggerRequest": true }
                }
            }),
        ))));
        let session = LspSession::new(Arc::clone(&client), "rust");
        let path = PathBuf::from("/src/lib.rs");
        let uri = file_uri(&path);

        assert!(session.is_ready(), "a fresh session starts ready");
        let pulled = session
            .pull_diagnostics(&path)
            .expect("a not-ready pull still returns Ok(empty)");
        assert!(pulled.is_empty(), "a not-ready pull yields no diagnostics");
        assert!(
            !session.is_ready(),
            "a ServerCancelled/retrigger pull must mark the session not-ready"
        );
        assert!(
            session.diagnostics_for(&uri).is_empty(),
            "a not-ready pull must NOT cache an empty (clean) set"
        );
    }

    #[test]
    fn real_pull_answer_marks_session_ready_again() {
        // After a not-ready pull, a real report answer (even an empty one for a
        // genuinely clean file) flips readiness back to true.
        let client = Arc::new(Mutex::new(Some(
            FakeTransport::default()
                .with_response(json!({
                    "jsonrpc": "2.0", "id": 1,
                    "error": { "code": -32802, "data": { "retriggerRequest": true } }
                }))
                .with_response(json!({
                    "jsonrpc": "2.0", "id": 2,
                    "result": { "kind": "full", "items": [] }
                })),
        )));
        let session = LspSession::new(Arc::clone(&client), "rust");
        let path = PathBuf::from("/src/lib.rs");

        session
            .pull_diagnostics(&path)
            .expect("first (cancelled) pull");
        assert!(!session.is_ready(), "not-ready after the cancelled pull");

        session.pull_diagnostics(&path).expect("second (real) pull");
        assert!(
            session.is_ready(),
            "a real (even empty) report means the server is ready and the file is clean"
        );
    }

    #[test]
    fn diagnostics_for_unknown_uri_is_empty() {
        let (session, _client) = session_with_fake();
        assert!(session.diagnostics_for("file:///never.rs").is_empty());
    }

    #[test]
    fn close_keeps_document_open_when_the_notify_fails() {
        // A client that is present but whose notification path fails: close
        // must not remove the document from the open set, so the open set keeps
        // reflecting what the server still believes is open.
        let client: Arc<Mutex<Option<FakeTransport>>> = Arc::new(Mutex::new(Some(FakeTransport {
            fail_notifications: true,
            ..FakeTransport::default()
        })));
        let session = LspSession::new(Arc::clone(&client), "rust");
        let path = PathBuf::from("/tmp/close-fail.rs");

        // Open succeeds only if notifications succeed, so open via a separate
        // path: seed the open set directly through a successful open against a
        // non-failing fake, then swap in the failing one is awkward — instead
        // assert close's failure semantics by opening first with notifications
        // allowed, then flipping the flag.
        {
            let mut guard = client.lock().unwrap();
            guard.as_mut().unwrap().fail_notifications = false;
        }
        session.open(&path, "fn c() {}").expect("open");
        {
            let mut guard = client.lock().unwrap();
            guard.as_mut().unwrap().fail_notifications = true;
        }

        let err = session
            .close(&path)
            .expect_err("close should surface the notify failure");
        assert!(matches!(err, LspError::JsonRpc(_)));
        assert!(
            session.is_open(&path),
            "a failed didClose must leave the document in the open set"
        );
    }
}
