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
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use crate::client::{LspJsonRpcClient, LspTransport};
use crate::error::LspError;

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

/// The shared interior of an [`LspSession`]: the single client handle plus the
/// open-document set. Every clone of the session points at the same `Arc`.
struct SessionInner<C: LspTransport> {
    /// The one client. `None` while the daemon is not running (starting or
    /// restarting). Operations against a `None` client return
    /// [`LspError::NotRunning`].
    client: Arc<Mutex<Option<C>>>,
    /// What the server believes is open: `uri -> `[`DocState`].
    docs: Mutex<HashMap<String, DocState>>,
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
        Self {
            inner: Arc::new(SessionInner {
                client,
                docs: Mutex::new(HashMap::new()),
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

    /// Forget every open document without sending any `didClose`.
    ///
    /// Called by the owning daemon when the underlying server process is gone
    /// (shutdown, health-check failure, or just before a restart): the new
    /// process knows nothing about what the old one had open, so the session's
    /// open set must be cleared to match. Sending `didClose` here would be
    /// wrong — the pipe is closed and the documents no longer exist from the
    /// server's perspective. After a reset the next `open` for any uri emits a
    /// fresh `didOpen` instead of being suppressed as a stale duplicate.
    pub fn reset_documents(&self) {
        self.lock_docs().clear();
    }

    /// Lock the open-document map, recovering from a poisoned mutex.
    fn lock_docs(&self) -> std::sync::MutexGuard<'_, HashMap<String, DocState>> {
        self.inner
            .docs
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
