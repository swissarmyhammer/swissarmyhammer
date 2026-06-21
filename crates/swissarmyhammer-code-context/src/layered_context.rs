//! Layered resolution context for code intelligence operations.
//!
//! Provides a single access point for three data layers:
//! 1. **Live LSP** -- real-time requests to a running LSP server
//! 2. **LSP index** -- persisted symbols and call edges from previous LSP sessions
//! 3. **Tree-sitter index** -- structural chunks extracted by tree-sitter
//!
//! Each layer has its own method family. Convenience methods like
//! [`LayeredContext::enrich_location`] try all layers in priority order.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::CodeContextError;

/// Extract the `result` field from a JSON-RPC response envelope.
///
/// LSP responses are wrapped: `{"id": N, "jsonrpc": "2.0", "result": ...}`.
/// If the response contains an `error` field instead, convert it to a
/// `CodeContextError`. If neither `result` nor `error` is present, return
/// the response as-is (some callers may handle raw envelopes).
fn unwrap_lsp_result(response: Value) -> Result<Value, CodeContextError> {
    if let Some(error) = response.get("error") {
        let message = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown LSP error");
        return Err(CodeContextError::LspError(message.to_string()));
    }
    // Return the result field if present, otherwise the whole response
    Ok(response.get("result").cloned().unwrap_or(response))
}

/// Send a single LSP request, log the outcome, and unwrap the result envelope.
///
/// Factored out of `lsp_request_with_document` so the multi-request closure
/// path (`lsp_multi_request_with_document`) and the single-request path share
/// one send-and-unwrap implementation.
fn send_and_unwrap_lsp_request(
    rpc: &mut LspJsonRpcClient,
    method: &str,
    params: Value,
) -> Result<Value, CodeContextError> {
    tracing::debug!(method = %method, "lsp_request_with_document");
    let response = rpc.send_request(method, params);
    match &response {
        Ok(v) => {
            tracing::debug!(response = %serde_json::to_string(v).unwrap_or_default(), "lsp_request_with_document OK")
        }
        Err(e) => tracing::warn!(error = %e, "lsp_request_with_document ERR"),
    }
    unwrap_lsp_result(response?)
}

/// Map a transport-level [`LspError`] from the session into the graceful
/// degradation contract the layered ops expect.
///
/// [`LspError::NotRunning`] means the daemon has no live client — for the
/// layered ops that is not an error but a signal to fall back to the index
/// layers, so it maps to `Ok(None)`. Every other transport error is a genuine
/// failure and propagates as [`CodeContextError::LspError`].
fn not_running_is_none<T>(result: Result<T, LspError>) -> Result<Option<T>, CodeContextError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(LspError::NotRunning) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

use crate::lsp_communication::LspJsonRpcClient;
use swissarmyhammer_lsp::{LspError, LspSession};

/// The concrete session handle the layered context consumes.
///
/// `LayeredContext` is a pure *consumer* of the shared [`LspSession`]: it never
/// spawns a client or opens/closes documents on its own lifecycle. The session
/// (owned by the daemon) keeps documents open across requests, so the layered
/// ops just `open` the document they touch and issue their request — the
/// open-document set and didClose lifecycle belong to the session, not here.
pub type SharedLspSession = LspSession<LspJsonRpcClient>;

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

/// A range in an LSP-style coordinate system (0-based lines and characters).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspRange {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

/// Information about a symbol from any data layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub name: String,
    pub qualified_path: Option<String>,
    pub kind: String,
    pub detail: Option<String>,
    pub file_path: String,
    pub range: LspRange,
}

/// A call edge connecting a symbol to its call sites.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallEdgeInfo {
    pub symbol: SymbolInfo,
    pub call_sites: Vec<LspRange>,
}

/// A chunk of source code extracted by tree-sitter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkInfo {
    pub text: String,
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
}

/// A set of text edits to apply to a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEdit {
    pub file_path: String,
    pub text_edits: Vec<TextEdit>,
}

/// A single text replacement within a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: LspRange,
    pub new_text: String,
}

/// A location where a symbol is defined, with optional source text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefinitionLocation {
    pub file_path: String,
    pub range: LspRange,
    pub source_text: Option<String>,
    pub symbol: Option<SymbolInfo>,
}

/// Which data layer provided a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceLayer {
    /// Result came from a live LSP server.
    LiveLsp,
    /// Result came from the persisted LSP symbol index.
    LspIndex,
    /// Result came from the tree-sitter chunk index.
    TreeSitter,
    /// No layer had data.
    None,
}

/// Result of enriching a location with the best available data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentResult {
    pub symbol: Option<SymbolInfo>,
    pub source_layer: SourceLayer,
}

// ---------------------------------------------------------------------------
// LayeredContext
// ---------------------------------------------------------------------------

/// Single access point for all three data layers.
///
/// Private `conn` and `session` fields ensure callers use typed methods rather
/// than reaching into the raw database or LSP client directly. The optional
/// [`SharedLspSession`] is a cheap clone of the daemon-owned session: every
/// live request routes through it, so this context shares the one open-document
/// set with the indexing worker and the diagnostics path and never spawns a
/// client of its own.
pub struct LayeredContext<'a> {
    conn: &'a Connection,
    session: Option<SharedLspSession>,
    live_lsp_router: Option<LiveLspRouter>,
    multi_lsp_router: Option<MultiLspRouter>,
}

/// A follower's live-LSP override: routes a single LSP request to a live server
/// the *consuming crate* reaches out-of-process (the elected leader's session),
/// when this process owns no in-process [`SharedLspSession`].
///
/// This is the dependency-inversion seam that lets a follower's layered ops
/// (`get_definition`, `get_hover`, …) take their live-LSP branch and get the
/// leader's real rust-analyzer answer, without `swissarmyhammer-code-context`
/// depending on the leader-election / diagnostics IPC crates: code-context owns
/// only this closure *type*, and the tools layer supplies the implementation
/// that round-trips to the leader over the existing request multiplexer.
///
/// The arguments mirror [`LayeredContext::lsp_request_with_document`]:
/// `(file_path, method, params)`. `file_path` is the document the leader must
/// sync before the request (empty for the workspace-wide
/// [`LayeredContext::lsp_request`] seam). The result follows the same graceful
/// contract as a live session: `Ok(None)` means "no live layer reachable, fall
/// back to the index layers", `Ok(Some(json))` is the LSP result, and `Err` is a
/// genuine failure (e.g. the leader was unreachable) that must not be masked as a
/// silent empty.
pub type LiveLspRouter =
    Box<dyn Fn(&str, &str, Value) -> Result<Option<Value>, CodeContextError> + Send + Sync>;

/// A follower's *multi-step* live-LSP override: routes an ordered batch of LSP
/// requests to a live server the consuming crate reaches out-of-process (the
/// elected leader's session), to be run as ONE atomic exchange.
///
/// This is the multi-step sibling of [`LiveLspRouter`]. The single-request seam
/// cannot express an op that must hold the server's client lock across several
/// requests (e.g. `prepareRename` then `rename`): each routed single request is
/// a separate round-trip and the leader does not keep its lock between them, so
/// another consumer could interleave a request and steal a response off the
/// shared stdio pipe. This seam routes the *whole* ordered step list in ONE call
/// so the consuming crate (the tools layer) can run them under one leader-side
/// `with_client` lock and return the ordered results.
///
/// The arguments mirror [`LayeredContext::lsp_multi_request_batch`]: `file_path`
/// is the single document the leader must sync before the batch (empty for a
/// document-less batch), and `steps` is the ordered `(method, params)` list. The
/// result follows the same graceful contract as a live session: `Ok(None)` means
/// "no live layer reachable, fall back" (so an op like rename degrades to
/// `can_rename: false`), `Ok(Some(results))` is one bare LSP result per step in
/// order, and `Err` is a genuine failure that must not be masked as a silent
/// empty.
pub type MultiLspRouter = Box<
    dyn Fn(&str, Vec<(String, Value)>) -> Result<Option<Vec<Value>>, CodeContextError>
        + Send
        + Sync,
>;

impl<'a> LayeredContext<'a> {
    /// Create a new layered context.
    ///
    /// # Arguments
    /// * `conn` - Reference to the SQLite connection for index queries.
    /// * `session` - Optional shared LSP session for live requests. `None`
    ///   means no live layer; the context degrades gracefully to the index
    ///   layers. The session is cloned (a cheap `Arc` bump) and shared with the
    ///   daemon and every other in-process consumer.
    pub fn new(conn: &'a Connection, session: Option<SharedLspSession>) -> Self {
        Self {
            conn,
            session,
            live_lsp_router: None,
            multi_lsp_router: None,
        }
    }

    /// Create a layered context whose live-LSP layer is served by a router
    /// instead of an in-process session.
    ///
    /// This is the follower constructor: the process owns no
    /// [`SharedLspSession`], so the live-LSP requests issued by the layered ops
    /// are routed through `router` (see [`LiveLspRouter`]) to the live server the
    /// consuming crate reaches out-of-process — in production, the elected
    /// leader's single session over the request multiplexer. The op functions are
    /// unchanged: [`has_live_lsp`](Self::has_live_lsp) reports the live layer
    /// available, so they take their live branch and the request goes to the
    /// router.
    pub fn with_live_lsp_router(conn: &'a Connection, router: LiveLspRouter) -> Self {
        Self {
            conn,
            session: None,
            live_lsp_router: Some(router),
            multi_lsp_router: None,
        }
    }

    /// Create a follower layered context whose *multi-step* live-LSP layer is
    /// served by a [`MultiLspRouter`].
    ///
    /// The multi-step sibling of [`with_live_lsp_router`](Self::with_live_lsp_router):
    /// the process owns no [`SharedLspSession`], so a multi-step op
    /// ([`lsp_multi_request_batch`](Self::lsp_multi_request_batch), used by the
    /// rename op) routes its whole ordered batch through `router` to be run under
    /// one leader-side lock.
    pub fn with_multi_lsp_router(conn: &'a Connection, router: MultiLspRouter) -> Self {
        Self {
            conn,
            session: None,
            live_lsp_router: None,
            multi_lsp_router: Some(router),
        }
    }

    /// Create a follower layered context wired with BOTH the single-request and
    /// the multi-step live-LSP routers.
    ///
    /// This is the production follower constructor: a follower has no in-process
    /// session, and its layered ops span both seams — single-request ops
    /// (definition/hover/inbound-calls' individual requests/…) route through
    /// `single`, while a multi-step op that must run atomically under one leader
    /// lock (rename's prepare+rename) routes its batch through `multi`. Either may
    /// be `None` (e.g. no leader reachable for one seam).
    pub fn with_live_lsp_routers(
        conn: &'a Connection,
        single: Option<LiveLspRouter>,
        multi: Option<MultiLspRouter>,
    ) -> Self {
        Self {
            conn,
            session: None,
            live_lsp_router: single,
            multi_lsp_router: multi,
        }
    }

    // === Layer availability ===

    /// Returns true if a live LSP layer is available.
    ///
    /// True when this process owns a running in-process session, *or* when a
    /// [`LiveLspRouter`] is wired (a follower routing live requests to the
    /// leader). The op functions gate their live-LSP branch on this, so both the
    /// in-process and the routed follower path take the live layer.
    pub fn has_live_lsp(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.is_running())
            || self.live_lsp_router.is_some()
            || self.multi_lsp_router.is_some()
    }

    /// Returns true if the given file has been indexed by LSP.
    pub fn has_lsp_index(&self, file_path: &str) -> bool {
        self.conn
            .query_row(
                "SELECT lsp_indexed FROM indexed_files WHERE file_path = ?1",
                [file_path],
                |row| row.get::<_, i32>(0),
            )
            .map(|v| v == 1)
            .unwrap_or(false)
    }

    /// Returns true if the given file has been indexed by tree-sitter.
    pub fn has_ts_index(&self, file_path: &str) -> bool {
        self.conn
            .query_row(
                "SELECT ts_indexed FROM indexed_files WHERE file_path = ?1",
                [file_path],
                |row| row.get::<_, i32>(0),
            )
            .map(|v| v == 1)
            .unwrap_or(false)
    }

    // === Layer 1: Live LSP ===

    /// Send an arbitrary LSP request through the shared session. Returns
    /// `Ok(None)` if no live session is available.
    ///
    /// This is a graceful degradation -- callers should fall back to index layers
    /// when the live session is absent.
    pub fn lsp_request(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Option<Value>, CodeContextError> {
        let session = match &self.session {
            Some(s) => s,
            // No in-process session: route through the follower router if one is
            // wired (workspace-wide ops carry no document, so the file_path is
            // empty), else there is no live layer.
            None => return self.route_via_router("", method, params),
        };
        match not_running_is_none(session.request(method, params))? {
            Some(response) => Ok(Some(unwrap_lsp_result(response)?)),
            None => Ok(None),
        }
    }

    /// Route a single LSP request through the follower [`LiveLspRouter`] if one is
    /// wired, else report no live layer (`Ok(None)`).
    ///
    /// `file_path` is the document scope the router (and the leader) needs to sync
    /// before the request; it is empty for workspace-wide ops.
    ///
    /// The leader answers with the *full* JSON-RPC envelope (its
    /// `session.request` returns `{jsonrpc, id, result}` / `{… error …}`, the same
    /// raw value the in-process transport returns). So the routed value is passed
    /// through [`unwrap_lsp_result`] here — exactly as the in-process session
    /// seams ([`lsp_request`](Self::lsp_request) /
    /// [`send_and_unwrap_lsp_request`]) unwrap it — so the op parsers always
    /// receive the bare result and a JSON-RPC `error` envelope surfaces as a
    /// [`CodeContextError`] rather than a silently-empty parse.
    fn route_via_router(
        &self,
        file_path: &str,
        method: &str,
        params: Value,
    ) -> Result<Option<Value>, CodeContextError> {
        match &self.live_lsp_router {
            Some(router) => match router(file_path, method, params)? {
                Some(response) => Ok(Some(unwrap_lsp_result(response)?)),
                None => Ok(None),
            },
            None => Ok(None),
        }
    }

    /// Send an LSP notification through the shared session. No-op if no live
    /// session is available.
    pub fn lsp_notify(&self, method: &str, params: Value) -> Result<(), CodeContextError> {
        // Notifications are fire-and-forget. If no session, silently succeed.
        let session = match &self.session {
            Some(s) => s,
            None => return Ok(()),
        };
        // A missing client is the same "no live layer" signal as a missing
        // session — drop the notification rather than surfacing an error.
        not_running_is_none(session.notify(method, params))?;
        Ok(())
    }

    /// Open the document on the shared session, then issue a single LSP request
    /// against it.
    ///
    /// Documents are opened (and kept open) by the session, which owns the
    /// shared open-document set; this context never sends `didClose` — the
    /// session keeps the document open across requests instead of the old
    /// `didOpen -> request -> didClose` churn.
    ///
    /// Returns `Ok(None)` if no live LSP session is available (graceful degradation).
    ///
    /// # Arguments
    /// * `file_path` - Path to the file to open before the request.
    /// * `method` - The LSP method to call (e.g. `"textDocument/hover"`).
    /// * `params` - The JSON parameters for the request.
    pub fn lsp_request_with_document(
        &self,
        file_path: &str,
        method: &str,
        params: Value,
    ) -> Result<Option<Value>, CodeContextError> {
        // No in-process session but a follower router is wired: route the
        // single, document-scoped request to the leader, handing it the
        // file_path so it syncs the document before the request. This is the
        // path the layered ops (definition/hover/references/…) take on a
        // follower; the leader's open-document lifecycle replaces the local one.
        if self.session.is_none() {
            return self.route_via_router(file_path, method, params);
        }
        // Delegate to lsp_multi_request_with_document so the open-document
        // lifecycle is handled in one place.
        let method = method.to_owned();
        self.lsp_multi_request_with_document(file_path, |rpc| {
            send_and_unwrap_lsp_request(rpc, &method, params)
        })
    }

    /// Open the document on the shared session, then run a closure that issues
    /// one or more LSP requests against the client, holding the client lock for
    /// the whole closure.
    ///
    /// Holding the lock across the closure keeps a multi-step exchange (e.g.
    /// `prepareCallHierarchy` then `incomingCalls`) atomic so no other consumer
    /// interleaves a request and steals a response off the shared pipe. The
    /// document is opened via the session before the closure runs; the session
    /// owns the open-document set, so there is no per-request `didClose` here.
    ///
    /// Returns `Ok(None)` if no live LSP session is available.
    ///
    /// # Arguments
    /// * `file_path` - Path to the file to open before the requests.
    /// * `f` - Closure that receives the RPC client and performs the requests.
    pub fn lsp_multi_request_with_document<F, T>(
        &self,
        file_path: &str,
        f: F,
    ) -> Result<Option<T>, CodeContextError>
    where
        F: FnOnce(&mut LspJsonRpcClient) -> Result<T, CodeContextError>,
    {
        let session = match &self.session {
            Some(s) => s,
            None => return Ok(None),
        };

        // Sync the document to the session (open, then refresh the buffer with a
        // `didChange` if it was already open with stale text). The session owns
        // the open-document set and the didClose lifecycle.
        match self.sync_document(session, file_path)? {
            Some(()) => {}
            None => return Ok(None),
        }

        // Run the caller's requests against the locked client. The closure
        // returns `Result<T, CodeContextError>`; we wrap it in `Ok(..)` so the
        // transport-level `with_client` cannot conflate a closure failure with
        // a transport failure, then flatten: a missing client maps to None.
        match session.with_client(|rpc| Ok(f(rpc)))? {
            Some(closure_result) => closure_result.map(Some),
            None => Ok(None),
        }
    }

    /// Run an ordered batch of LSP requests as ONE atomic exchange, syncing the
    /// document once first, and return one bare LSP result per step.
    ///
    /// This is the *data-driven* multi-step seam: instead of a closure that
    /// issues requests against a live client (which cannot cross a process
    /// boundary), the caller hands an ordered `(method, params)` list. The whole
    /// batch runs under one client lock — locally via [`LspSession::with_client`]
    /// on the in-process session, or, on a follower with no session, routed
    /// through a [`MultiLspRouter`] so the consuming crate runs it under the
    /// leader's single `with_client` lock. Holding the lock across the batch keeps
    /// the exchange atomic so no other consumer interleaves a request and steals a
    /// response off the shared pipe.
    ///
    /// Each step's result is unwrapped from its JSON-RPC envelope (so the op
    /// parser always receives the bare `result`, and a JSON-RPC `error` envelope
    /// surfaces as a [`CodeContextError`] rather than a silently-empty parse) —
    /// the same contract as the single-request seam.
    ///
    /// Returns `Ok(None)` when there is no live LSP layer (no session and no
    /// router), so multi-step ops degrade to their documented best-effort.
    ///
    /// # Arguments
    /// * `file_path` - Path to the document to sync before the batch.
    /// * `steps` - The ordered `(method, params)` requests to run under one lock.
    pub fn lsp_multi_request_batch(
        &self,
        file_path: &str,
        steps: Vec<(String, Value)>,
    ) -> Result<Option<Vec<Value>>, CodeContextError> {
        // No in-process session but a multi-step router is wired (a follower):
        // route the whole batch to the leader, which runs it under one
        // with_client lock and returns one envelope per step. Unwrap each here so
        // the op parser receives bare results — exactly like route_via_router.
        if self.session.is_none() {
            return match &self.multi_lsp_router {
                Some(router) => match router(file_path, steps)? {
                    Some(responses) => Ok(Some(
                        responses
                            .into_iter()
                            .map(unwrap_lsp_result)
                            .collect::<Result<Vec<_>, _>>()?,
                    )),
                    None => Ok(None),
                },
                None => Ok(None),
            };
        }

        // In-process session: run the batch under one with_client lock, syncing
        // the document first, so the local path is atomic exactly like the
        // routed one. send_and_unwrap_lsp_request unwraps each step's envelope.
        self.lsp_multi_request_with_document(file_path, |rpc| {
            steps
                .into_iter()
                .map(|(method, params)| send_and_unwrap_lsp_request(rpc, &method, params))
                .collect::<Result<Vec<_>, _>>()
        })
    }

    /// Pull diagnostics for a file through the shared session's unified
    /// diagnostics path.
    ///
    /// This is the single diagnostics entry point for the layered ops: it syncs
    /// the document to the session (open, then push the current on-disk text via
    /// `didChange` if the server's buffer is stale), issues a
    /// `textDocument/diagnostic` pull through [`LspSession::pull_diagnostics`],
    /// and returns the parsed [`lsp_types::Diagnostic`] records. The pull result
    /// is also fed into the session's latest-per-uri cache and fan-out, so push
    /// (`publishDiagnostics`) and pull consumers observe one unified stream.
    ///
    /// Returns `Ok(None)` when there is no diagnostics layer to consult: no live
    /// session, or the pull could not run because the client is gone
    /// ([`LspError::NotRunning`]) or hit a transport failure. The caller then
    /// reports `SourceLayer::None`. `Ok(Some(..))` means a live server *answered*
    /// the pull — `Some(vec![])` for a clean report (the session also collapses a
    /// JSON-RPC error envelope such as method-not-found into an empty answered
    /// pull), `Some(diagnostics)` otherwise — and the caller reports
    /// `SourceLayer::LiveLsp`.
    pub fn lsp_diagnostics(
        &self,
        file_path: &str,
    ) -> Result<Option<Vec<lsp_types::Diagnostic>>, CodeContextError> {
        let session = match &self.session {
            Some(s) => s,
            None => return Ok(None),
        };

        // Sync the document to the session so the server analyzes the *current*
        // content; the session owns the open-document lifecycle (no didClose).
        let path = std::path::Path::new(file_path);
        match self.sync_document(session, file_path)? {
            Some(()) => {}
            None => return Ok(None),
        }

        // A successful pull (including a valid empty report) is the live layer.
        // A server that does not support pull diagnostics — or any other
        // transport error — is "no diagnostics layer" (`Ok(None)`), matching the
        // pre-session path which reported `SourceLayer::None` for an
        // unsupported/failed pull rather than a misleading live-but-empty result.
        match session.pull_diagnostics(path) {
            Ok(diagnostics) => Ok(Some(diagnostics)),
            Err(LspError::NotRunning) => Ok(None),
            Err(_) => Ok(None),
        }
    }

    /// Sync a document to the shared session so a following request analyzes its
    /// current on-disk content.
    ///
    /// Thin wrapper over [`LspSession::sync_open`] (the one place that opens or
    /// refreshes the server buffer) that reads the file and maps
    /// [`LspError::NotRunning`] to `Ok(None)` for graceful degradation.
    ///
    /// LSP-unavailability is checked *first*, before touching the filesystem: if
    /// the session has no live client the document can never be synced, so this
    /// short-circuits to `Ok(None)` (fall back to the index layers) without
    /// reading the file. When a live client *is* present, a failure to read the
    /// file (deleted, inaccessible, non-UTF-8) is a *real* error and propagates —
    /// syncing empty content on a read failure would feed stale/empty text into
    /// LSP state and silently corrupt later requests.
    fn sync_document(
        &self,
        session: &SharedLspSession,
        file_path: &str,
    ) -> Result<Option<()>, CodeContextError> {
        if !session.is_running() {
            return Ok(None);
        }
        let path = std::path::Path::new(file_path);
        let text = std::fs::read_to_string(file_path)?;
        not_running_is_none(session.sync_open(path, &text))
    }

    // === Layer 2: LSP index (lsp_symbols, lsp_call_edges tables) ===

    /// Look up a symbol at the given range from the LSP index.
    pub fn lsp_symbol_at(&self, file_path: &str, range: &LspRange) -> Option<SymbolInfo> {
        self.conn
            .query_row(
                "SELECT id, name, kind, detail, start_line, start_char, end_line, end_char
                 FROM lsp_symbols
                 WHERE file_path = ?1
                   AND start_line <= ?2 AND end_line >= ?3
                 ORDER BY (end_line - start_line) ASC
                 LIMIT 1",
                rusqlite::params![file_path, range.start_line as i64, range.end_line as i64,],
                |row| {
                    Ok(SymbolInfo {
                        name: row.get(1)?,
                        qualified_path: row.get::<_, Option<String>>(0).ok().flatten(),
                        kind: symbol_kind_int_to_string(row.get::<_, i32>(2)?).to_string(),
                        detail: row.get(3)?,
                        file_path: file_path.to_string(),
                        range: LspRange {
                            start_line: row.get::<_, i32>(4)? as u32,
                            start_character: row.get::<_, i32>(5)? as u32,
                            end_line: row.get::<_, i32>(6)? as u32,
                            end_character: row.get::<_, i32>(7)? as u32,
                        },
                    })
                },
            )
            .ok()
    }

    /// List all symbols in a file from the LSP index.
    pub fn lsp_symbols_in_file(&self, file_path: &str) -> Vec<SymbolInfo> {
        let mut stmt = match self.conn.prepare(
            "SELECT id, name, kind, detail, start_line, start_char, end_line, end_char
             FROM lsp_symbols WHERE file_path = ?1
             ORDER BY start_line",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([file_path], |row| {
            Ok(SymbolInfo {
                name: row.get(1)?,
                qualified_path: row.get::<_, Option<String>>(0).ok().flatten(),
                kind: symbol_kind_int_to_string(row.get::<_, i32>(2)?).to_string(),
                detail: row.get(3)?,
                file_path: file_path.to_string(),
                range: LspRange {
                    start_line: row.get::<_, i32>(4)? as u32,
                    start_character: row.get::<_, i32>(5)? as u32,
                    end_line: row.get::<_, i32>(6)? as u32,
                    end_character: row.get::<_, i32>(7)? as u32,
                },
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Search for symbols by name from the LSP index.
    pub fn lsp_symbols_by_name(&self, query: &str, max: usize) -> Vec<SymbolInfo> {
        let pattern = format!("%{}%", query);
        let mut stmt = match self.conn.prepare(
            "SELECT id, name, kind, detail, file_path, start_line, start_char, end_line, end_char
             FROM lsp_symbols WHERE name LIKE ?1
             LIMIT ?2",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map(rusqlite::params![pattern, max as i64], |row| {
            Ok(SymbolInfo {
                name: row.get(1)?,
                qualified_path: row.get::<_, Option<String>>(0).ok().flatten(),
                kind: symbol_kind_int_to_string(row.get::<_, i32>(2)?).to_string(),
                detail: row.get(3)?,
                file_path: row.get(4)?,
                range: LspRange {
                    start_line: row.get::<_, i32>(5)? as u32,
                    start_character: row.get::<_, i32>(6)? as u32,
                    end_line: row.get::<_, i32>(7)? as u32,
                    end_character: row.get::<_, i32>(8)? as u32,
                },
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Find callers of a symbol from the LSP call edge index.
    pub fn lsp_callers_of(&self, symbol_id: &str) -> Vec<CallEdgeInfo> {
        self.query_call_edges(
            "SELECT s.id, s.name, s.kind, s.detail, s.file_path,
                    s.start_line, s.start_char, s.end_line, s.end_char,
                    e.from_ranges
             FROM lsp_call_edges e
             JOIN lsp_symbols s ON e.caller_id = s.id
             WHERE e.callee_id = ?1",
            symbol_id,
        )
    }

    /// Find callees of a symbol from the LSP call edge index.
    pub fn lsp_callees_of(&self, symbol_id: &str) -> Vec<CallEdgeInfo> {
        self.query_call_edges(
            "SELECT s.id, s.name, s.kind, s.detail, s.file_path,
                    s.start_line, s.start_char, s.end_line, s.end_char,
                    e.from_ranges
             FROM lsp_call_edges e
             JOIN lsp_symbols s ON e.callee_id = s.id
             WHERE e.caller_id = ?1",
            symbol_id,
        )
    }

    /// Shared helper for caller/callee queries.
    fn query_call_edges(&self, sql: &str, symbol_id: &str) -> Vec<CallEdgeInfo> {
        let mut stmt = match self.conn.prepare(sql) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([symbol_id], |row| {
            let from_ranges_json: String = row.get(9)?;
            let call_sites = parse_from_ranges(&from_ranges_json);
            Ok(CallEdgeInfo {
                symbol: SymbolInfo {
                    name: row.get(1)?,
                    qualified_path: row.get::<_, Option<String>>(0).ok().flatten(),
                    kind: symbol_kind_int_to_string(row.get::<_, i32>(2)?).to_string(),
                    detail: row.get(3)?,
                    file_path: row.get(4)?,
                    range: LspRange {
                        start_line: row.get::<_, i32>(5)? as u32,
                        start_character: row.get::<_, i32>(6)? as u32,
                        end_line: row.get::<_, i32>(7)? as u32,
                        end_character: row.get::<_, i32>(8)? as u32,
                    },
                },
                call_sites,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    // === Layer 3: Tree-sitter index (ts_chunks table) ===

    /// Find the chunk containing the given line from the tree-sitter index.
    pub fn ts_chunk_at(&self, file_path: &str, line: u32) -> Option<ChunkInfo> {
        self.conn
            .query_row(
                "SELECT text, start_line, end_line
                 FROM ts_chunks
                 WHERE file_path = ?1 AND start_line <= ?2 AND end_line >= ?2
                 ORDER BY (end_line - start_line) ASC
                 LIMIT 1",
                rusqlite::params![file_path, line as i64],
                |row| {
                    Ok(ChunkInfo {
                        text: row.get(0)?,
                        file_path: file_path.to_string(),
                        start_line: row.get::<_, i32>(1)? as u32,
                        end_line: row.get::<_, i32>(2)? as u32,
                    })
                },
            )
            .ok()
    }

    /// List all symbols in a file from the tree-sitter index.
    ///
    /// Converts ts_chunks with a `symbol_path` into SymbolInfo entries.
    pub fn ts_symbols_in_file(&self, file_path: &str) -> Vec<SymbolInfo> {
        let mut stmt = match self.conn.prepare(
            "SELECT text, start_line, end_line, symbol_path
             FROM ts_chunks
             WHERE file_path = ?1 AND symbol_path IS NOT NULL
             ORDER BY start_line",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([file_path], |row| {
            let symbol_path: String = row.get(3)?;
            let name = symbol_path
                .rsplit("::")
                .next()
                .unwrap_or(&symbol_path)
                .to_string();
            Ok(SymbolInfo {
                name,
                qualified_path: Some(symbol_path),
                kind: "chunk".to_string(),
                detail: None,
                file_path: file_path.to_string(),
                range: LspRange {
                    start_line: row.get::<_, i32>(1)? as u32,
                    start_character: 0,
                    end_line: row.get::<_, i32>(2)? as u32,
                    end_character: 0,
                },
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Search for chunks matching a text query in the tree-sitter index.
    pub fn ts_chunks_matching(&self, query: &str, max: usize) -> Vec<ChunkInfo> {
        let pattern = format!("%{}%", query);
        let mut stmt = match self.conn.prepare(
            "SELECT text, file_path, start_line, end_line
             FROM ts_chunks WHERE text LIKE ?1
             LIMIT ?2",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map(rusqlite::params![pattern, max as i64], |row| {
            Ok(ChunkInfo {
                text: row.get(0)?,
                file_path: row.get(1)?,
                start_line: row.get::<_, i32>(2)? as u32,
                end_line: row.get::<_, i32>(3)? as u32,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Find callers of a symbol from the tree-sitter call edge index.
    pub fn ts_callers_of(&self, file_path: &str, symbol: &str) -> Vec<CallEdgeInfo> {
        // Tree-sitter call edges use the same lsp_call_edges table with source='treesitter'
        let mut stmt = match self.conn.prepare(
            "SELECT s.id, s.name, s.kind, s.detail, s.file_path,
                    s.start_line, s.start_char, s.end_line, s.end_char,
                    e.from_ranges
             FROM lsp_call_edges e
             JOIN lsp_symbols s ON e.caller_id = s.id
             WHERE e.callee_file = ?1 AND s.name LIKE ?2 AND e.source = 'treesitter'",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let pattern = format!("%{}%", symbol);
        stmt.query_map(rusqlite::params![file_path, pattern], |row| {
            let from_ranges_json: String = row.get(9)?;
            let call_sites = parse_from_ranges(&from_ranges_json);
            Ok(CallEdgeInfo {
                symbol: SymbolInfo {
                    name: row.get(1)?,
                    qualified_path: row.get::<_, Option<String>>(0).ok().flatten(),
                    kind: symbol_kind_int_to_string(row.get::<_, i32>(2)?).to_string(),
                    detail: row.get(3)?,
                    file_path: row.get(4)?,
                    range: LspRange {
                        start_line: row.get::<_, i32>(5)? as u32,
                        start_character: row.get::<_, i32>(6)? as u32,
                        end_line: row.get::<_, i32>(7)? as u32,
                        end_character: row.get::<_, i32>(8)? as u32,
                    },
                },
                call_sites,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    // === Layered convenience ===

    /// Enrich a location by trying all layers in priority order.
    ///
    /// Tries: LSP index first (live LSP requires async and is skipped here),
    /// then tree-sitter. Returns the best available data with the source layer.
    pub fn enrich_location(&self, file_path: &str, range: &LspRange) -> EnrichmentResult {
        // Try LSP index first
        if let Some(symbol) = self.lsp_symbol_at(file_path, range) {
            return EnrichmentResult {
                symbol: Some(symbol),
                source_layer: SourceLayer::LspIndex,
            };
        }

        // Fall back to tree-sitter
        if let Some(chunk) = self.ts_chunk_at(file_path, range.start_line) {
            let name = chunk.text.lines().next().unwrap_or("").trim().to_string();
            return EnrichmentResult {
                symbol: Some(SymbolInfo {
                    name,
                    qualified_path: None,
                    kind: "chunk".to_string(),
                    detail: None,
                    file_path: file_path.to_string(),
                    range: LspRange {
                        start_line: chunk.start_line,
                        start_character: 0,
                        end_line: chunk.end_line,
                        end_character: 0,
                    },
                }),
                source_layer: SourceLayer::TreeSitter,
            };
        }

        EnrichmentResult {
            symbol: None,
            source_layer: SourceLayer::None,
        }
    }

    /// Find a symbol at a specific position by trying all layers.
    pub fn find_symbol(&self, file_path: &str, line: u32, char: u32) -> Option<SymbolInfo> {
        let range = LspRange {
            start_line: line,
            start_character: char,
            end_line: line,
            end_character: char,
        };
        let result = self.enrich_location(file_path, &range);
        result.symbol
    }

    /// Generate a human-readable notice about which layer provided data.
    ///
    /// Returns `None` for live LSP (best case -- no notice needed).
    pub fn layer_notice(source: SourceLayer) -> Option<String> {
        match source {
            SourceLayer::LiveLsp => None,
            SourceLayer::LspIndex => {
                Some("Results from LSP index (live LSP not available)".to_string())
            }
            SourceLayer::TreeSitter => {
                Some("Results from tree-sitter index only (LSP not available)".to_string())
            }
            SourceLayer::None => Some("No index data available for this location".to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert an LSP SymbolKind integer to a human-readable string.
///
/// Returns a static string slice to avoid per-call allocation. Callers that
/// need an owned `String` (e.g. for `SymbolInfo.kind`) should call
/// `.to_string()` at the use site.
pub(crate) fn symbol_kind_int_to_string(kind: i32) -> &'static str {
    match kind {
        1 => "file",
        2 => "module",
        3 => "namespace",
        4 => "package",
        5 => "class",
        6 => "method",
        7 => "property",
        8 => "field",
        9 => "constructor",
        10 => "enum",
        11 => "interface",
        12 => "function",
        13 => "variable",
        14 => "constant",
        15 => "string",
        16 => "number",
        17 => "boolean",
        18 => "array",
        19 => "object",
        20 => "key",
        21 => "null",
        22 => "enum_member",
        23 => "struct",
        24 => "event",
        25 => "operator",
        26 => "type_parameter",
        _ => "unknown",
    }
}

/// Parse the `from_ranges` JSON column from lsp_call_edges into LspRange entries.
fn parse_from_ranges(json: &str) -> Vec<LspRange> {
    // Format is a JSON array of range objects like:
    // [{"start":{"line":10,"character":5},"end":{"line":10,"character":15}}]
    let parsed: Result<Vec<serde_json::Value>, _> = serde_json::from_str(json);
    match parsed {
        Ok(ranges) => ranges
            .iter()
            .filter_map(|r| {
                let start = r.get("start")?;
                let end = r.get("end")?;
                Some(LspRange {
                    start_line: start.get("line")?.as_u64()? as u32,
                    start_character: start.get("character")?.as_u64()? as u32,
                    end_line: end.get("line")?.as_u64()? as u32,
                    end_character: end.get("character")?.as_u64()? as u32,
                })
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{
        insert_call_edge, insert_file, insert_ts_chunk, mock_lsp_session, none_session,
        spawn_mock_lsp, test_db,
    };

    /// Insert an LSP symbol (without detail, for layered_context tests).
    #[allow(clippy::too_many_arguments)]
    fn insert_lsp_symbol(
        conn: &Connection,
        id: &str,
        name: &str,
        kind: i32,
        file_path: &str,
        start_line: i32,
        start_char: i32,
        end_line: i32,
        end_char: i32,
    ) {
        crate::test_fixtures::insert_lsp_symbol(
            conn, id, name, kind, file_path, start_line, start_char, end_line, end_char, None,
        );
    }

    // --- Layer availability ---

    #[test]
    fn test_has_live_lsp_returns_false_when_no_client() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);
        assert!(!ctx.has_live_lsp());
    }

    #[test]
    fn test_has_live_lsp_returns_false_when_client_is_none() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, Some(none_session()));
        assert!(!ctx.has_live_lsp());
    }

    #[test]
    fn test_has_lsp_index_returns_true_when_indexed() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 1);
        let ctx = LayeredContext::new(&conn, None);
        assert!(ctx.has_lsp_index("src/main.rs"));
    }

    #[test]
    fn test_has_lsp_index_returns_false_when_not_indexed() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 0);
        let ctx = LayeredContext::new(&conn, None);
        assert!(!ctx.has_lsp_index("src/main.rs"));
    }

    #[test]
    fn test_has_ts_index_returns_true_when_indexed() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);
        let ctx = LayeredContext::new(&conn, None);
        assert!(ctx.has_ts_index("src/main.rs"));
    }

    // --- Layer 1: Live LSP ---

    #[test]
    fn test_lsp_request_returns_ok_none_when_no_client() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);
        let result = ctx
            .lsp_request("textDocument/hover", serde_json::json!({}))
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_lsp_request_returns_ok_none_when_client_is_none() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, Some(none_session()));
        let result = ctx
            .lsp_request("textDocument/hover", serde_json::json!({}))
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_live_lsp_router_routes_lsp_request_when_no_session() {
        // A follower has no in-process session, but is given a live-LSP router
        // (the leader-routing closure supplied by the tools layer). lsp_request
        // must route through the router instead of short-circuiting to Ok(None),
        // and has_live_lsp() must report the live layer as available so the op
        // functions take their live-LSP branch.
        use std::sync::{Arc, Mutex};
        let conn = test_db();
        let seen: Arc<Mutex<Vec<(String, serde_json::Value)>>> = Arc::new(Mutex::new(Vec::new()));
        let seen_for_router = Arc::clone(&seen);
        let ctx = LayeredContext::with_live_lsp_router(
            &conn,
            Box::new(move |_file_path, method, params| {
                seen_for_router
                    .lock()
                    .unwrap()
                    .push((method.to_string(), params));
                Ok(Some(serde_json::json!({ "routed": true })))
            }),
        );
        assert!(
            ctx.has_live_lsp(),
            "a router makes the live layer available"
        );
        let result = ctx
            .lsp_request("workspace/symbol", serde_json::json!({ "query": "x" }))
            .unwrap();
        assert_eq!(result, Some(serde_json::json!({ "routed": true })));
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert_eq!(seen.lock().unwrap()[0].0, "workspace/symbol");
    }

    #[test]
    fn test_live_lsp_router_unwraps_the_jsonrpc_result_envelope() {
        // The leader returns the FULL JSON-RPC envelope (LspJsonRpcClient's
        // send_request returns {jsonrpc,id,result}, NOT the bare result). The
        // local session path unwraps that envelope via unwrap_lsp_result before
        // the op parser sees it; the routed follower path MUST do the same, or
        // the op parsers (parse_definition_locations, etc.) get an enveloped
        // value and silently return wrong-empty. This locks the contract: the
        // router seam returns the BARE result, exactly like the session seam.
        let conn = test_db();
        let ctx = LayeredContext::with_live_lsp_router(
            &conn,
            Box::new(|_file_path, _method, _params| {
                Ok(Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 7,
                    "result": { "uri": "file:///src/main.rs" }
                })))
            }),
        );
        let result = ctx
            .lsp_request("textDocument/definition", serde_json::json!({}))
            .unwrap();
        assert_eq!(
            result,
            Some(serde_json::json!({ "uri": "file:///src/main.rs" })),
            "the router seam must unwrap the JSON-RPC `result` envelope, like the session seam"
        );
    }

    #[test]
    fn test_live_lsp_router_surfaces_jsonrpc_error_envelope() {
        // A leader response carrying a JSON-RPC `error` envelope must become a
        // CodeContextError, exactly as unwrap_lsp_result does on the local path —
        // not a silently-returned error object the parser would treat as empty.
        let conn = test_db();
        let ctx = LayeredContext::with_live_lsp_router(
            &conn,
            Box::new(|_file_path, _method, _params| {
                Ok(Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "error": { "code": -32601, "message": "method not found" }
                })))
            }),
        );
        let err = ctx
            .lsp_request("textDocument/definition", serde_json::json!({}))
            .expect_err("a JSON-RPC error envelope must surface as an error");
        assert!(format!("{err}").contains("method not found"));
    }

    #[test]
    fn test_live_lsp_router_routes_request_with_document_carrying_file_path() {
        // The document-scoped seam must hand the router the file_path so the
        // leader can sync the document before the request.
        use std::sync::{Arc, Mutex};
        let conn = test_db();
        let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let seen_for_router = Arc::clone(&seen);
        let ctx = LayeredContext::with_live_lsp_router(
            &conn,
            Box::new(move |file_path, _method, _params| {
                seen_for_router.lock().unwrap().push(file_path.to_string());
                Ok(Some(serde_json::json!(null)))
            }),
        );
        let _ = ctx
            .lsp_request_with_document(
                "src/main.rs",
                "textDocument/definition",
                serde_json::json!({}),
            )
            .unwrap();
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            &["src/main.rs".to_string()]
        );
    }

    #[test]
    fn test_live_lsp_router_error_propagates() {
        // A router error (e.g. the leader connect/serve failed) must surface as a
        // CodeContextError, not a silent Ok(None) wrong-empty.
        let conn = test_db();
        let ctx = LayeredContext::with_live_lsp_router(
            &conn,
            Box::new(|_file_path, _method, _params| {
                Err(CodeContextError::LspError("leader unreachable".to_string()))
            }),
        );
        let err = ctx
            .lsp_request("workspace/symbol", serde_json::json!({}))
            .expect_err("router error must propagate");
        assert!(format!("{err}").contains("leader unreachable"));
    }

    #[test]
    fn test_lsp_notify_succeeds_when_no_client() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);
        ctx.lsp_notify("textDocument/didOpen", serde_json::json!({}))
            .unwrap();
    }

    // --- Multi-step router (lsp_multi_request_batch) ---

    #[test]
    fn test_multi_router_routes_ordered_steps_when_no_session() {
        // A follower has no in-process session but is given a multi-step router
        // (the leader-routing closure supplied by the tools layer).
        // lsp_multi_request_batch must route the WHOLE ordered batch through the
        // router (one IPC round-trip) instead of short-circuiting to Ok(None),
        // hand it the file_path to sync, and the steps in order.
        use std::sync::{Arc, Mutex};
        let conn = test_db();
        let seen: Arc<Mutex<(String, Vec<String>)>> =
            Arc::new(Mutex::new((String::new(), Vec::new())));
        let seen_for_router = Arc::clone(&seen);
        let ctx = LayeredContext::with_multi_lsp_router(
            &conn,
            Box::new(move |file_path, steps| {
                let mut g = seen_for_router.lock().unwrap();
                g.0 = file_path.to_string();
                g.1 = steps.iter().map(|(m, _)| m.clone()).collect();
                // Return one bare result per step, in order.
                Ok(Some(
                    steps
                        .iter()
                        .map(|(m, _)| serde_json::json!({ "echo": m }))
                        .collect(),
                ))
            }),
        );
        assert!(
            ctx.has_live_lsp(),
            "a multi router makes the live layer available"
        );
        let results = ctx
            .lsp_multi_request_batch(
                "src/main.rs",
                vec![
                    (
                        "textDocument/prepareRename".to_string(),
                        serde_json::json!({}),
                    ),
                    ("textDocument/rename".to_string(), serde_json::json!({})),
                ],
            )
            .unwrap()
            .expect("router present → Some");
        let g = seen.lock().unwrap();
        assert_eq!(g.0, "src/main.rs", "must hand the router the file_path");
        assert_eq!(
            g.1,
            vec![
                "textDocument/prepareRename".to_string(),
                "textDocument/rename".to_string()
            ],
            "steps must be routed in order"
        );
        assert_eq!(results.len(), 2, "one result per step");
        assert_eq!(
            results[0],
            serde_json::json!({ "echo": "textDocument/prepareRename" })
        );
    }

    #[test]
    fn test_multi_router_unwraps_each_step_jsonrpc_envelope() {
        // The leader returns the FULL JSON-RPC envelope per step
        // ({jsonrpc,id,result}). The routed multi-step path MUST unwrap each
        // step's envelope to the bare result — exactly like the single-request
        // seam — or the op parsers get an enveloped value and wrong-empty.
        let conn = test_db();
        let ctx = LayeredContext::with_multi_lsp_router(
            &conn,
            Box::new(|_file_path, steps| {
                Ok(Some(
                    steps
                        .iter()
                        .enumerate()
                        .map(|(i, _)| {
                            serde_json::json!({
                                "jsonrpc": "2.0",
                                "id": i,
                                "result": { "step": i }
                            })
                        })
                        .collect(),
                ))
            }),
        );
        let results = ctx
            .lsp_multi_request_batch(
                "src/main.rs",
                vec![
                    (
                        "textDocument/prepareRename".to_string(),
                        serde_json::json!({}),
                    ),
                    ("textDocument/rename".to_string(), serde_json::json!({})),
                ],
            )
            .unwrap()
            .expect("router present");
        assert_eq!(
            results,
            vec![
                serde_json::json!({ "step": 0 }),
                serde_json::json!({ "step": 1 })
            ],
            "each step's JSON-RPC `result` envelope must be unwrapped to the bare result"
        );
    }

    #[test]
    fn test_multi_router_surfaces_step_jsonrpc_error_envelope() {
        // A step response carrying a JSON-RPC `error` envelope must become a
        // CodeContextError, exactly as the single-request seam does — never a
        // silently-returned error object the parser treats as empty.
        let conn = test_db();
        let ctx = LayeredContext::with_multi_lsp_router(
            &conn,
            Box::new(|_file_path, _steps| {
                Ok(Some(vec![serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "error": { "code": -32601, "message": "method not found" }
                })]))
            }),
        );
        let err = ctx
            .lsp_multi_request_batch(
                "src/main.rs",
                vec![(
                    "textDocument/prepareRename".to_string(),
                    serde_json::json!({}),
                )],
            )
            .expect_err("a JSON-RPC error envelope in a step must surface as an error");
        assert!(format!("{err}").contains("method not found"));
    }

    #[test]
    fn test_multi_router_error_propagates_not_silent_empty() {
        // A router error (leader connect/serve failed) must surface as a
        // CodeContextError, not a silent Ok(None) wrong-empty.
        let conn = test_db();
        let ctx = LayeredContext::with_multi_lsp_router(
            &conn,
            Box::new(|_file_path, _steps| {
                Err(CodeContextError::LspError("leader unreachable".to_string()))
            }),
        );
        let err = ctx
            .lsp_multi_request_batch(
                "src/main.rs",
                vec![(
                    "textDocument/prepareRename".to_string(),
                    serde_json::json!({}),
                )],
            )
            .expect_err("router error must propagate");
        assert!(format!("{err}").contains("leader unreachable"));
    }

    #[test]
    fn test_multi_request_batch_returns_none_when_no_session_and_no_router() {
        // Neither an in-process session nor a multi router: the batch seam
        // reports no live layer (Ok(None)), so the op degrades.
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);
        let result = ctx
            .lsp_multi_request_batch(
                "src/main.rs",
                vec![(
                    "textDocument/prepareRename".to_string(),
                    serde_json::json!({}),
                )],
            )
            .unwrap();
        assert!(result.is_none());
    }

    // --- Layer 2: LSP index ---

    #[test]
    fn test_lsp_symbol_at_returns_data_from_lsp_symbols() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);
        insert_lsp_symbol(&conn, "sym1", "main", 12, "src/main.rs", 5, 0, 20, 1);

        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 10,
            start_character: 0,
            end_line: 10,
            end_character: 0,
        };
        let symbol = ctx.lsp_symbol_at("src/main.rs", &range);
        assert!(symbol.is_some());
        let sym = symbol.unwrap();
        assert_eq!(sym.name, "main");
        assert_eq!(sym.kind, "function");
    }

    #[test]
    fn test_lsp_symbol_at_returns_none_when_no_match() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);

        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 10,
            start_character: 0,
            end_line: 10,
            end_character: 0,
        };
        assert!(ctx.lsp_symbol_at("src/main.rs", &range).is_none());
    }

    #[test]
    fn test_lsp_symbols_in_file_returns_ordered_list() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 1);
        insert_lsp_symbol(&conn, "sym1", "foo", 12, "src/lib.rs", 10, 0, 20, 1);
        insert_lsp_symbol(&conn, "sym2", "bar", 12, "src/lib.rs", 1, 0, 8, 1);

        let ctx = LayeredContext::new(&conn, None);
        let symbols = ctx.lsp_symbols_in_file("src/lib.rs");
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "bar");
        assert_eq!(symbols[1].name, "foo");
    }

    #[test]
    fn test_lsp_symbols_by_name_finds_matches() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 1);
        insert_lsp_symbol(
            &conn,
            "sym1",
            "process_request",
            12,
            "src/lib.rs",
            1,
            0,
            10,
            1,
        );
        insert_lsp_symbol(
            &conn,
            "sym2",
            "handle_response",
            12,
            "src/lib.rs",
            20,
            0,
            30,
            1,
        );

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.lsp_symbols_by_name("process", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "process_request");
    }

    // --- Layer 3: Tree-sitter index ---

    #[test]
    fn test_ts_chunk_at_returns_data_from_ts_chunks() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);
        insert_ts_chunk(
            &conn,
            "src/main.rs",
            5,
            20,
            "fn main() {\n    println!(\"hello\");\n}",
            None,
        );

        let ctx = LayeredContext::new(&conn, None);
        let chunk = ctx.ts_chunk_at("src/main.rs", 10);
        assert!(chunk.is_some());
        let c = chunk.unwrap();
        assert_eq!(c.start_line, 5);
        assert_eq!(c.end_line, 20);
        assert!(c.text.contains("fn main()"));
    }

    #[test]
    fn test_ts_chunk_at_returns_none_when_no_match() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);

        let ctx = LayeredContext::new(&conn, None);
        assert!(ctx.ts_chunk_at("src/main.rs", 100).is_none());
    }

    #[test]
    fn test_ts_symbols_in_file_returns_named_chunks() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        insert_ts_chunk(&conn, "src/lib.rs", 1, 10, "fn foo() {}", Some("lib::foo"));
        insert_ts_chunk(&conn, "src/lib.rs", 15, 25, "fn bar() {}", Some("lib::bar"));
        insert_ts_chunk(&conn, "src/lib.rs", 30, 40, "// comment block", None);

        let ctx = LayeredContext::new(&conn, None);
        let symbols = ctx.ts_symbols_in_file("src/lib.rs");
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "foo");
        assert_eq!(symbols[1].name, "bar");
    }

    #[test]
    fn test_ts_chunks_matching_finds_by_text() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);
        insert_ts_chunk(&conn, "src/main.rs", 1, 5, "fn hello_world() {}", None);
        insert_ts_chunk(&conn, "src/main.rs", 10, 15, "fn goodbye() {}", None);

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_chunks_matching("hello", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].text.contains("hello_world"));
    }

    // --- Layered convenience ---

    #[test]
    fn test_enrich_location_prefers_lsp_index_over_treesitter() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);
        insert_lsp_symbol(&conn, "sym1", "main", 12, "src/main.rs", 5, 0, 20, 1);
        insert_ts_chunk(&conn, "src/main.rs", 5, 20, "fn main() {}", None);

        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 10,
            start_character: 0,
            end_line: 10,
            end_character: 0,
        };
        let result = ctx.enrich_location("src/main.rs", &range);
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
        assert!(result.symbol.is_some());
        assert_eq!(result.symbol.unwrap().name, "main");
    }

    #[test]
    fn test_enrich_location_falls_back_to_treesitter_when_lsp_empty() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);
        insert_ts_chunk(&conn, "src/main.rs", 5, 20, "fn main() {}", None);

        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 10,
            start_character: 0,
            end_line: 10,
            end_character: 0,
        };
        let result = ctx.enrich_location("src/main.rs", &range);
        assert_eq!(result.source_layer, SourceLayer::TreeSitter);
        assert!(result.symbol.is_some());
    }

    #[test]
    fn test_enrich_location_returns_none_when_all_empty() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 0);

        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 10,
            start_character: 0,
            end_line: 10,
            end_character: 0,
        };
        let result = ctx.enrich_location("src/main.rs", &range);
        assert_eq!(result.source_layer, SourceLayer::None);
        assert!(result.symbol.is_none());
    }

    // --- Shared types serialization ---

    #[test]
    fn test_lsp_range_serializable() {
        let range = LspRange {
            start_line: 1,
            start_character: 5,
            end_line: 10,
            end_character: 20,
        };
        let json = serde_json::to_string(&range).unwrap();
        let roundtrip: LspRange = serde_json::from_str(&json).unwrap();
        assert_eq!(range, roundtrip);
    }

    #[test]
    fn test_source_layer_serializable() {
        let layer = SourceLayer::LspIndex;
        let json = serde_json::to_string(&layer).unwrap();
        let roundtrip: SourceLayer = serde_json::from_str(&json).unwrap();
        assert_eq!(layer, roundtrip);
    }

    #[test]
    fn test_enrichment_result_serializable() {
        let result = EnrichmentResult {
            symbol: Some(SymbolInfo {
                name: "test".to_string(),
                qualified_path: None,
                kind: "function".to_string(),
                detail: None,
                file_path: "test.rs".to_string(),
                range: LspRange {
                    start_line: 0,
                    start_character: 0,
                    end_line: 5,
                    end_character: 0,
                },
            }),
            source_layer: SourceLayer::TreeSitter,
        };
        let json = serde_json::to_string(&result).unwrap();
        let _roundtrip: EnrichmentResult = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_layer_notice_live_lsp_returns_none() {
        assert!(LayeredContext::layer_notice(SourceLayer::LiveLsp).is_none());
    }

    #[test]
    fn test_layer_notice_lsp_index_returns_message() {
        let notice = LayeredContext::layer_notice(SourceLayer::LspIndex);
        assert!(notice.is_some());
        assert!(notice.unwrap().contains("LSP index"));
    }

    #[test]
    fn test_layer_notice_treesitter_returns_message() {
        let notice = LayeredContext::layer_notice(SourceLayer::TreeSitter);
        assert!(notice.is_some());
        assert!(notice.unwrap().contains("tree-sitter"));
    }

    #[test]
    fn test_layer_notice_none_returns_message() {
        let notice = LayeredContext::layer_notice(SourceLayer::None);
        assert!(notice.is_some());
        assert!(notice.unwrap().contains("No index data"));
    }

    #[test]
    fn test_find_symbol_delegates_to_enrich() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);
        insert_lsp_symbol(&conn, "sym1", "main", 12, "src/main.rs", 5, 0, 20, 1);

        let ctx = LayeredContext::new(&conn, None);
        let sym = ctx.find_symbol("src/main.rs", 10, 0);
        assert!(sym.is_some());
        assert_eq!(sym.unwrap().name, "main");
    }

    // --- Helper tests ---

    #[test]
    fn test_symbol_kind_int_to_string() {
        assert_eq!(symbol_kind_int_to_string(12), "function");
        assert_eq!(symbol_kind_int_to_string(5), "class");
        assert_eq!(symbol_kind_int_to_string(23), "struct");
        assert_eq!(symbol_kind_int_to_string(999), "unknown");
    }

    #[test]
    fn test_parse_from_ranges_valid() {
        let json = r#"[{"start":{"line":10,"character":5},"end":{"line":10,"character":15}}]"#;
        let ranges = parse_from_ranges(json);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 10);
        assert_eq!(ranges[0].start_character, 5);
    }

    #[test]
    fn test_parse_from_ranges_empty() {
        let ranges = parse_from_ranges("[]");
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_parse_from_ranges_invalid_json() {
        let ranges = parse_from_ranges("not json");
        assert!(ranges.is_empty());
    }

    // --- symbol_kind_int_to_string exhaustive coverage ---

    /// Verify every LSP SymbolKind integer (1-26) maps to the correct string,
    /// and that out-of-range values return "unknown".
    #[test]
    fn test_symbol_kind_int_to_string_all_variants() {
        let expected: &[(i32, &str)] = &[
            (1, "file"),
            (2, "module"),
            (3, "namespace"),
            (4, "package"),
            (5, "class"),
            (6, "method"),
            (7, "property"),
            (8, "field"),
            (9, "constructor"),
            (10, "enum"),
            (11, "interface"),
            (12, "function"),
            (13, "variable"),
            (14, "constant"),
            (15, "string"),
            (16, "number"),
            (17, "boolean"),
            (18, "array"),
            (19, "object"),
            (20, "key"),
            (21, "null"),
            (22, "enum_member"),
            (23, "struct"),
            (24, "event"),
            (25, "operator"),
            (26, "type_parameter"),
        ];
        for &(kind, label) in expected {
            assert_eq!(
                symbol_kind_int_to_string(kind),
                label,
                "SymbolKind {} should map to {:?}",
                kind,
                label,
            );
        }
    }

    /// Out-of-range values (0, negative, >26) all return "unknown".
    #[test]
    fn test_symbol_kind_int_to_string_unknown_cases() {
        for kind in [0, -1, 27, 100, i32::MAX, i32::MIN] {
            assert_eq!(
                symbol_kind_int_to_string(kind),
                "unknown",
                "SymbolKind {} should be unknown",
                kind,
            );
        }
    }

    // --- lsp_callees_of coverage ---

    /// When a caller symbol has call edges, lsp_callees_of returns the callee symbols.
    #[test]
    fn test_lsp_callees_of_returns_callee_symbols() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);
        insert_file(&conn, "src/helper.rs", 1, 1);

        // The caller symbol
        insert_lsp_symbol(
            &conn,
            "sym:caller",
            "do_work",
            12,
            "src/main.rs",
            1,
            0,
            10,
            1,
        );
        // Two callee symbols
        insert_lsp_symbol(
            &conn,
            "sym:callee_a",
            "helper_a",
            12,
            "src/helper.rs",
            1,
            0,
            5,
            1,
        );
        insert_lsp_symbol(
            &conn,
            "sym:callee_b",
            "helper_b",
            6,
            "src/helper.rs",
            10,
            0,
            20,
            1,
        );

        // Edges: caller -> callee_a and caller -> callee_b
        let from_ranges_json =
            r#"[{"start":{"line":3,"character":4},"end":{"line":3,"character":12}}]"#;
        insert_call_edge(
            &conn,
            "sym:caller",
            "sym:callee_a",
            "src/main.rs",
            "src/helper.rs",
            "lsp",
            from_ranges_json,
        );
        insert_call_edge(
            &conn,
            "sym:caller",
            "sym:callee_b",
            "src/main.rs",
            "src/helper.rs",
            "lsp",
            "[]",
        );

        let ctx = LayeredContext::new(&conn, None);
        let callees = ctx.lsp_callees_of("sym:caller");

        assert_eq!(callees.len(), 2);

        let names: Vec<&str> = callees.iter().map(|c| c.symbol.name.as_str()).collect();
        assert!(names.contains(&"helper_a"));
        assert!(names.contains(&"helper_b"));

        // Verify the first callee's call_sites were parsed from from_ranges
        let a = callees
            .iter()
            .find(|c| c.symbol.name == "helper_a")
            .unwrap();
        assert_eq!(a.call_sites.len(), 1);
        assert_eq!(a.call_sites[0].start_line, 3);
        assert_eq!(a.call_sites[0].start_character, 4);

        // Verify kind was translated through symbol_kind_int_to_string
        assert_eq!(a.symbol.kind, "function");
        let b = callees
            .iter()
            .find(|c| c.symbol.name == "helper_b")
            .unwrap();
        assert_eq!(b.symbol.kind, "method");
    }

    /// When a symbol has no outgoing call edges, lsp_callees_of returns an empty vec.
    #[test]
    fn test_lsp_callees_of_returns_empty_when_no_edges() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);
        insert_lsp_symbol(
            &conn,
            "sym:lonely",
            "lonely_fn",
            12,
            "src/main.rs",
            1,
            0,
            5,
            1,
        );

        let ctx = LayeredContext::new(&conn, None);
        let callees = ctx.lsp_callees_of("sym:lonely");
        assert!(callees.is_empty());
    }

    // --- ts_callers_of ---

    /// A single tree-sitter call edge is returned with correct symbol info and call sites.
    #[test]
    fn test_ts_callers_of_single_caller() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        insert_file(&conn, "src/main.rs", 1, 0);

        insert_lsp_symbol(
            &conn,
            "caller1",
            "run_process",
            12,
            "src/main.rs",
            10,
            0,
            25,
            1,
        );
        insert_lsp_symbol(&conn, "callee1", "do_work", 12, "src/lib.rs", 1, 0, 5, 1);

        let ranges_json =
            r#"[{"start":{"line":15,"character":4},"end":{"line":15,"character":20}}]"#;
        insert_call_edge(
            &conn,
            "caller1",
            "callee1",
            "src/main.rs",
            "src/lib.rs",
            "treesitter",
            ranges_json,
        );

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_callers_of("src/lib.rs", "process");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].symbol.name, "run_process");
        assert_eq!(results[0].symbol.file_path, "src/main.rs");
        assert_eq!(results[0].symbol.kind, "function");
        assert_eq!(results[0].symbol.range.start_line, 10);
        assert_eq!(results[0].symbol.range.end_line, 25);
        assert_eq!(results[0].call_sites.len(), 1);
        assert_eq!(results[0].call_sites[0].start_line, 15);
        assert_eq!(results[0].call_sites[0].start_character, 4);
    }

    /// Multiple callers from different files are all returned.
    #[test]
    fn test_ts_callers_of_multiple_callers() {
        let conn = test_db();
        insert_file(&conn, "src/target.rs", 1, 0);
        insert_file(&conn, "src/a.rs", 1, 0);
        insert_file(&conn, "src/b.rs", 1, 0);

        insert_lsp_symbol(&conn, "c1", "invoke_target", 12, "src/a.rs", 1, 0, 10, 1);
        insert_lsp_symbol(&conn, "c2", "call_target", 12, "src/b.rs", 5, 0, 15, 1);
        insert_lsp_symbol(&conn, "t1", "some_target", 12, "src/target.rs", 1, 0, 20, 1);

        let ranges1 = r#"[{"start":{"line":3,"character":0},"end":{"line":3,"character":10}}]"#;
        let ranges2 = r#"[{"start":{"line":8,"character":2},"end":{"line":8,"character":12}}]"#;
        insert_call_edge(
            &conn,
            "c1",
            "t1",
            "src/a.rs",
            "src/target.rs",
            "treesitter",
            ranges1,
        );
        insert_call_edge(
            &conn,
            "c2",
            "t1",
            "src/b.rs",
            "src/target.rs",
            "treesitter",
            ranges2,
        );

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_callers_of("src/target.rs", "target");
        assert_eq!(results.len(), 2);

        let names: Vec<&str> = results.iter().map(|r| r.symbol.name.as_str()).collect();
        assert!(names.contains(&"invoke_target"));
        assert!(names.contains(&"call_target"));
    }

    /// When no edges exist for a file/symbol, ts_callers_of returns an empty vec.
    #[test]
    fn test_ts_callers_of_no_callers() {
        let conn = test_db();
        insert_file(&conn, "src/orphan.rs", 1, 0);

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_callers_of("src/orphan.rs", "some_func");
        assert!(results.is_empty());
    }

    /// Edges with source != 'treesitter' are excluded from ts_callers_of results.
    #[test]
    fn test_ts_callers_of_ignores_non_treesitter_edges() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        insert_file(&conn, "src/main.rs", 1, 0);

        insert_lsp_symbol(
            &conn,
            "lsp_caller",
            "do_stuff",
            12,
            "src/main.rs",
            1,
            0,
            10,
            1,
        );
        insert_lsp_symbol(&conn, "target", "the_target", 12, "src/lib.rs", 1, 0, 5, 1);

        let ranges = r#"[{"start":{"line":5,"character":0},"end":{"line":5,"character":8}}]"#;
        insert_call_edge(
            &conn,
            "lsp_caller",
            "target",
            "src/main.rs",
            "src/lib.rs",
            "lsp",
            ranges,
        );

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_callers_of("src/lib.rs", "stuff");
        assert!(results.is_empty());
    }

    /// Multiple call sites within a single edge are all parsed and returned.
    #[test]
    fn test_ts_callers_of_parses_multiple_call_sites() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        insert_file(&conn, "src/main.rs", 1, 0);

        insert_lsp_symbol(
            &conn,
            "multi",
            "run_process",
            12,
            "src/main.rs",
            1,
            0,
            50,
            1,
        );
        insert_lsp_symbol(&conn, "callee", "target_fn", 12, "src/lib.rs", 1, 0, 10, 1);

        let ranges = r#"[{"start":{"line":10,"character":4},"end":{"line":10,"character":15}},{"start":{"line":30,"character":8},"end":{"line":30,"character":19}}]"#;
        insert_call_edge(
            &conn,
            "multi",
            "callee",
            "src/main.rs",
            "src/lib.rs",
            "treesitter",
            ranges,
        );

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_callers_of("src/lib.rs", "process");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].call_sites.len(), 2);
        assert_eq!(results[0].call_sites[0].start_line, 10);
        assert_eq!(results[0].call_sites[0].start_character, 4);
        assert_eq!(results[0].call_sites[1].start_line, 30);
        assert_eq!(results[0].call_sites[1].start_character, 8);
    }

    // --- enrich_location additional coverage ---

    /// Verify the symbol fields produced by the tree-sitter fallback path.
    #[test]
    fn test_enrich_location_treesitter_symbol_fields() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        insert_ts_chunk(
            &conn,
            "src/lib.rs",
            10,
            25,
            "fn process_data() {\n    let x = 42;\n}",
            None,
        );

        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 15,
            start_character: 0,
            end_line: 15,
            end_character: 0,
        };
        let result = ctx.enrich_location("src/lib.rs", &range);
        assert_eq!(result.source_layer, SourceLayer::TreeSitter);

        let sym = result.symbol.expect("should have symbol");
        // Name is extracted from the first line of chunk text, trimmed.
        assert_eq!(sym.name, "fn process_data() {");
        assert_eq!(sym.kind, "chunk");
        assert_eq!(sym.file_path, "src/lib.rs");
        assert_eq!(sym.range.start_line, 10);
        assert_eq!(sym.range.end_line, 25);
        assert_eq!(sym.range.start_character, 0);
        assert_eq!(sym.range.end_character, 0);
    }

    /// A file that does not exist in the DB at all yields SourceLayer::None.
    #[test]
    fn test_enrich_location_file_not_in_db() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 0,
            start_character: 0,
            end_line: 0,
            end_character: 0,
        };
        let result = ctx.enrich_location("nonexistent.rs", &range);
        assert_eq!(result.source_layer, SourceLayer::None);
        assert!(result.symbol.is_none());
    }

    /// A range that falls outside all indexed chunks yields SourceLayer::None.
    #[test]
    fn test_enrich_location_range_outside_all_chunks() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);
        insert_ts_chunk(&conn, "src/main.rs", 1, 5, "fn small() {}", None);

        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 500,
            start_character: 0,
            end_line: 500,
            end_character: 0,
        };
        let result = ctx.enrich_location("src/main.rs", &range);
        assert_eq!(result.source_layer, SourceLayer::None);
        assert!(result.symbol.is_none());
    }

    // --- ts_symbols_in_file coverage ---

    /// Qualified paths with "::" produce a short name from the last segment.
    #[test]
    fn test_ts_symbols_in_file_extracts_name_from_qualified_path() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        insert_ts_chunk(
            &conn,
            "src/lib.rs",
            10,
            20,
            "fn method() {}",
            Some("module::Struct::method"),
        );

        let ctx = LayeredContext::new(&conn, None);
        let symbols = ctx.ts_symbols_in_file("src/lib.rs");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "method");
        assert_eq!(
            symbols[0].qualified_path.as_deref(),
            Some("module::Struct::method")
        );
    }

    /// A simple name (no "::") is returned as-is via the unwrap_or fallback.
    #[test]
    fn test_ts_symbols_in_file_simple_name_no_separator() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        insert_ts_chunk(
            &conn,
            "src/lib.rs",
            1,
            5,
            "fn standalone() {}",
            Some("standalone"),
        );

        let ctx = LayeredContext::new(&conn, None);
        let symbols = ctx.ts_symbols_in_file("src/lib.rs");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "standalone");
        assert_eq!(symbols[0].qualified_path.as_deref(), Some("standalone"));
    }

    /// When no chunks have symbol_path set, returns empty vec.
    #[test]
    fn test_ts_symbols_in_file_empty_when_no_symbol_paths() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        // Insert chunks without symbol_path
        insert_ts_chunk(&conn, "src/lib.rs", 1, 10, "// just a comment block", None);
        insert_ts_chunk(&conn, "src/lib.rs", 15, 25, "let x = 42;", None);

        let ctx = LayeredContext::new(&conn, None);
        let symbols = ctx.ts_symbols_in_file("src/lib.rs");
        assert!(symbols.is_empty());
    }

    /// When no file exists in the DB at all, returns empty vec.
    #[test]
    fn test_ts_symbols_in_file_empty_for_unknown_file() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);
        let symbols = ctx.ts_symbols_in_file("nonexistent.rs");
        assert!(symbols.is_empty());
    }

    /// Range mapping: start_line and end_line from the DB are propagated
    /// into the SymbolInfo range, with characters set to 0.
    #[test]
    fn test_ts_symbols_in_file_range_mapping() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        insert_ts_chunk(&conn, "src/lib.rs", 42, 99, "fn deep() {}", Some("deep"));

        let ctx = LayeredContext::new(&conn, None);
        let symbols = ctx.ts_symbols_in_file("src/lib.rs");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].range.start_line, 42);
        assert_eq!(symbols[0].range.end_line, 99);
        assert_eq!(symbols[0].range.start_character, 0);
        assert_eq!(symbols[0].range.end_character, 0);
    }

    /// File path is propagated correctly into each SymbolInfo.
    #[test]
    fn test_ts_symbols_in_file_propagates_file_path() {
        let conn = test_db();
        insert_file(&conn, "src/deep/nested/module.rs", 1, 0);
        insert_ts_chunk(
            &conn,
            "src/deep/nested/module.rs",
            1,
            10,
            "fn func() {}",
            Some("module::func"),
        );

        let ctx = LayeredContext::new(&conn, None);
        let symbols = ctx.ts_symbols_in_file("src/deep/nested/module.rs");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].file_path, "src/deep/nested/module.rs");
    }

    /// All symbols have kind "chunk" since they come from tree-sitter chunks.
    #[test]
    fn test_ts_symbols_in_file_kind_is_chunk() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        insert_ts_chunk(&conn, "src/lib.rs", 1, 5, "struct Foo {}", Some("Foo"));
        insert_ts_chunk(&conn, "src/lib.rs", 10, 20, "fn bar() {}", Some("bar"));

        let ctx = LayeredContext::new(&conn, None);
        let symbols = ctx.ts_symbols_in_file("src/lib.rs");
        assert_eq!(symbols.len(), 2);
        for sym in &symbols {
            assert_eq!(sym.kind, "chunk");
        }
    }

    /// Results are ordered by start_line (ascending).
    #[test]
    fn test_ts_symbols_in_file_ordered_by_start_line() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        // Insert out of order
        insert_ts_chunk(&conn, "src/lib.rs", 50, 60, "fn last() {}", Some("last"));
        insert_ts_chunk(&conn, "src/lib.rs", 1, 10, "fn first() {}", Some("first"));
        insert_ts_chunk(
            &conn,
            "src/lib.rs",
            25,
            35,
            "fn middle() {}",
            Some("middle"),
        );

        let ctx = LayeredContext::new(&conn, None);
        let symbols = ctx.ts_symbols_in_file("src/lib.rs");
        assert_eq!(symbols.len(), 3);
        assert_eq!(symbols[0].name, "first");
        assert_eq!(symbols[1].name, "middle");
        assert_eq!(symbols[2].name, "last");
    }

    /// Symbols from other files are not included in the results.
    #[test]
    fn test_ts_symbols_in_file_filters_by_file() {
        let conn = test_db();
        insert_file(&conn, "src/a.rs", 1, 0);
        insert_file(&conn, "src/b.rs", 1, 0);
        insert_ts_chunk(&conn, "src/a.rs", 1, 10, "fn in_a() {}", Some("in_a"));
        insert_ts_chunk(&conn, "src/b.rs", 1, 10, "fn in_b() {}", Some("in_b"));

        let ctx = LayeredContext::new(&conn, None);
        let symbols = ctx.ts_symbols_in_file("src/a.rs");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "in_a");
    }

    // --- unwrap_lsp_result ---

    /// A JSON-RPC response with an `error` field is converted to Err.
    #[test]
    fn test_unwrap_lsp_result_error_field_returns_err() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32600,
                "message": "Invalid request"
            }
        });
        let result = unwrap_lsp_result(response);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("Invalid request"),
            "error message should contain the LSP error text, got: {}",
            err_msg
        );
    }

    /// An error field without a `message` sub-field falls back to "unknown LSP error".
    #[test]
    fn test_unwrap_lsp_result_error_without_message() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": { "code": -32600 }
        });
        let result = unwrap_lsp_result(response);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("unknown LSP error"),
            "should fall back to 'unknown LSP error', got: {}",
            err_msg
        );
    }

    /// A JSON-RPC response with a `result` field extracts just that field.
    #[test]
    fn test_unwrap_lsp_result_extracts_result_field() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "contents": "hover info" }
        });
        let value = unwrap_lsp_result(response).unwrap();
        assert_eq!(value, serde_json::json!({ "contents": "hover info" }));
    }

    /// A response with neither `error` nor `result` is returned as-is.
    #[test]
    fn test_unwrap_lsp_result_plain_value_returned_as_is() {
        let response = serde_json::json!({ "some_field": 42 });
        let value = unwrap_lsp_result(response.clone()).unwrap();
        assert_eq!(value, response);
    }

    /// A null `result` field is correctly extracted (not treated as absent).
    #[test]
    fn test_unwrap_lsp_result_null_result_field() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": null
        });
        let value = unwrap_lsp_result(response).unwrap();
        assert!(value.is_null());
    }

    // --- lsp_callers_of ---

    /// lsp_callers_of returns caller symbols with correct fields and call sites.
    #[test]
    fn test_lsp_callers_of_returns_caller_symbols() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);
        insert_file(&conn, "src/lib.rs", 1, 1);

        // The callee (target) symbol
        insert_lsp_symbol(
            &conn,
            "sym:target",
            "process",
            12,
            "src/lib.rs",
            1,
            0,
            10,
            1,
        );
        // Two caller symbols
        insert_lsp_symbol(
            &conn,
            "sym:caller_a",
            "run_main",
            12,
            "src/main.rs",
            1,
            0,
            20,
            1,
        );
        insert_lsp_symbol(
            &conn,
            "sym:caller_b",
            "do_setup",
            6,
            "src/main.rs",
            25,
            0,
            40,
            1,
        );

        // Edges: caller_a -> target and caller_b -> target
        let ranges_a = r#"[{"start":{"line":5,"character":4},"end":{"line":5,"character":11}}]"#;
        insert_call_edge(
            &conn,
            "sym:caller_a",
            "sym:target",
            "src/main.rs",
            "src/lib.rs",
            "lsp",
            ranges_a,
        );
        insert_call_edge(
            &conn,
            "sym:caller_b",
            "sym:target",
            "src/main.rs",
            "src/lib.rs",
            "lsp",
            "[]",
        );

        let ctx = LayeredContext::new(&conn, None);
        let callers = ctx.lsp_callers_of("sym:target");

        assert_eq!(callers.len(), 2);

        let names: Vec<&str> = callers.iter().map(|c| c.symbol.name.as_str()).collect();
        assert!(names.contains(&"run_main"));
        assert!(names.contains(&"do_setup"));

        // Verify call sites parsed correctly for caller_a
        let a = callers
            .iter()
            .find(|c| c.symbol.name == "run_main")
            .unwrap();
        assert_eq!(a.call_sites.len(), 1);
        assert_eq!(a.call_sites[0].start_line, 5);
        assert_eq!(a.call_sites[0].start_character, 4);
        assert_eq!(a.symbol.kind, "function");
        assert_eq!(a.symbol.file_path, "src/main.rs");
        assert_eq!(a.symbol.range.start_line, 1);
        assert_eq!(a.symbol.range.end_line, 20);

        // Verify kind translation for method (kind=6)
        let b = callers
            .iter()
            .find(|c| c.symbol.name == "do_setup")
            .unwrap();
        assert_eq!(b.symbol.kind, "method");
        assert!(b.call_sites.is_empty());
    }

    /// lsp_callers_of returns empty when no edges point to the callee.
    #[test]
    fn test_lsp_callers_of_returns_empty_when_no_edges() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 1);
        insert_lsp_symbol(
            &conn,
            "sym:orphan",
            "orphan_fn",
            12,
            "src/lib.rs",
            1,
            0,
            5,
            1,
        );

        let ctx = LayeredContext::new(&conn, None);
        let callers = ctx.lsp_callers_of("sym:orphan");
        assert!(callers.is_empty());
    }

    /// lsp_callers_of parses multiple call sites from a single edge.
    #[test]
    fn test_lsp_callers_of_parses_multiple_call_sites() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);
        insert_file(&conn, "src/lib.rs", 1, 1);

        insert_lsp_symbol(
            &conn,
            "sym:caller",
            "multi_caller",
            12,
            "src/main.rs",
            1,
            0,
            50,
            1,
        );
        insert_lsp_symbol(
            &conn,
            "sym:callee",
            "target_fn",
            12,
            "src/lib.rs",
            1,
            0,
            10,
            1,
        );

        let ranges = r#"[{"start":{"line":10,"character":4},"end":{"line":10,"character":13}},{"start":{"line":30,"character":8},"end":{"line":30,"character":17}}]"#;
        insert_call_edge(
            &conn,
            "sym:caller",
            "sym:callee",
            "src/main.rs",
            "src/lib.rs",
            "lsp",
            ranges,
        );

        let ctx = LayeredContext::new(&conn, None);
        let callers = ctx.lsp_callers_of("sym:callee");
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].call_sites.len(), 2);
        assert_eq!(callers[0].call_sites[0].start_line, 10);
        assert_eq!(callers[0].call_sites[1].start_line, 30);
    }

    /// lsp_callers_of includes the symbol's detail field when present.
    #[test]
    fn test_lsp_callers_of_includes_detail() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);
        insert_file(&conn, "src/lib.rs", 1, 1);

        // Insert caller with detail via the full fixture helper
        crate::test_fixtures::insert_lsp_symbol(
            &conn,
            "sym:detailed",
            "handler",
            6,
            "src/main.rs",
            1,
            0,
            15,
            1,
            Some("impl Server"),
        );
        insert_lsp_symbol(
            &conn,
            "sym:target",
            "process",
            12,
            "src/lib.rs",
            1,
            0,
            10,
            1,
        );

        insert_call_edge(
            &conn,
            "sym:detailed",
            "sym:target",
            "src/main.rs",
            "src/lib.rs",
            "lsp",
            "[]",
        );

        let ctx = LayeredContext::new(&conn, None);
        let callers = ctx.lsp_callers_of("sym:target");
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].symbol.detail.as_deref(), Some("impl Server"));
    }

    // --- ts_chunks_matching additional coverage ---

    /// ts_chunks_matching returns all ChunkInfo fields correctly.
    #[test]
    fn test_ts_chunks_matching_verifies_all_fields() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);
        insert_ts_chunk(
            &conn,
            "src/main.rs",
            10,
            25,
            "fn process_data(input: &str) -> Result<()> { Ok(()) }",
            Some("process_data"),
        );

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_chunks_matching("process_data", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "src/main.rs");
        assert_eq!(results[0].start_line, 10);
        assert_eq!(results[0].end_line, 25);
        assert!(results[0].text.contains("process_data"));
    }

    /// ts_chunks_matching respects the limit parameter.
    #[test]
    fn test_ts_chunks_matching_respects_limit() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);
        insert_ts_chunk(&conn, "src/main.rs", 1, 5, "fn alpha() {}", None);
        insert_ts_chunk(&conn, "src/main.rs", 10, 15, "fn alpha_beta() {}", None);
        insert_ts_chunk(&conn, "src/main.rs", 20, 25, "fn alpha_gamma() {}", None);

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_chunks_matching("alpha", 2);
        assert_eq!(results.len(), 2);
    }

    /// ts_chunks_matching returns results from multiple files.
    #[test]
    fn test_ts_chunks_matching_across_files() {
        let conn = test_db();
        insert_file(&conn, "src/a.rs", 1, 0);
        insert_file(&conn, "src/b.rs", 1, 0);
        insert_ts_chunk(&conn, "src/a.rs", 1, 5, "fn shared_name() {}", None);
        insert_ts_chunk(&conn, "src/b.rs", 1, 5, "fn shared_name() {}", None);

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_chunks_matching("shared_name", 10);
        assert_eq!(results.len(), 2);

        let paths: Vec<&str> = results.iter().map(|r| r.file_path.as_str()).collect();
        assert!(paths.contains(&"src/a.rs"));
        assert!(paths.contains(&"src/b.rs"));
    }

    /// ts_chunks_matching returns empty when nothing matches.
    #[test]
    fn test_ts_chunks_matching_no_match() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);
        insert_ts_chunk(&conn, "src/main.rs", 1, 5, "fn foo() {}", None);

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_chunks_matching("nonexistent_text", 10);
        assert!(results.is_empty());
    }

    // --- ts_symbols_in_file: verify detail is always None ---

    /// ts_symbols_in_file always sets detail to None since ts_chunks has no detail column.
    #[test]
    fn test_ts_symbols_in_file_detail_is_none() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        insert_ts_chunk(
            &conn,
            "src/lib.rs",
            1,
            10,
            "impl Foo { fn bar() {} }",
            Some("Foo::bar"),
        );

        let ctx = LayeredContext::new(&conn, None);
        let symbols = ctx.ts_symbols_in_file("src/lib.rs");
        assert_eq!(symbols.len(), 1);
        assert!(
            symbols[0].detail.is_none(),
            "ts_symbols_in_file should always set detail to None"
        );
    }

    // --- enrich_location: verify LSP index symbol fields in detail ---

    /// enrich_location via LSP index returns SourceLayer::LspIndex with full symbol fields.
    #[test]
    fn test_enrich_location_lsp_index_symbol_fields() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);
        crate::test_fixtures::insert_lsp_symbol(
            &conn,
            "sym:enrich",
            "handle_request",
            6,
            "src/main.rs",
            10,
            4,
            30,
            5,
            Some("impl Server"),
        );

        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 15,
            start_character: 0,
            end_line: 15,
            end_character: 0,
        };
        let result = ctx.enrich_location("src/main.rs", &range);
        assert_eq!(result.source_layer, SourceLayer::LspIndex);

        let sym = result.symbol.expect("should have symbol");
        assert_eq!(sym.name, "handle_request");
        assert_eq!(sym.kind, "method");
        assert_eq!(sym.detail.as_deref(), Some("impl Server"));
        assert_eq!(sym.file_path, "src/main.rs");
        assert_eq!(sym.range.start_line, 10);
        assert_eq!(sym.range.start_character, 4);
        assert_eq!(sym.range.end_line, 30);
        assert_eq!(sym.range.end_character, 5);
    }

    // --- Mock LSP helpers for live-path tests ---
    // `spawn_mock_lsp` / `mock_lsp_session` are the shared
    // `crate::test_fixtures` helpers, imported above.

    /// Create a temp directory with a `test.rs` file so that
    /// `lsp_request_with_document` / `lsp_multi_request_with_document` can
    /// read the file content when building the didOpen notification.
    fn create_temp_source_file() -> tempfile::TempDir {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let file = dir.path().join("test.rs");
        let mut f = std::fs::File::create(&file).unwrap();
        writeln!(f, "fn main() {{}}").unwrap();
        dir
    }

    // --- Layer 1: Live LSP — mock-based tests ---

    #[test]
    fn test_lsp_request_with_mock_returns_response() {
        // The mock LSP reads one request and replies with a canned response.
        let hover_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "contents": "fn main()" }
        });
        let responses = vec![hover_response];

        let mut child = spawn_mock_lsp(&responses);
        let session = mock_lsp_session(&mut child);

        let conn = test_db();
        let ctx = LayeredContext::new(&conn, Some(session));
        let result = ctx
            .lsp_request("textDocument/hover", serde_json::json!({}))
            .unwrap();

        assert!(result.is_some());
        let value = result.unwrap();
        assert_eq!(value["contents"], "fn main()");
    }

    #[test]
    fn test_lsp_notify_with_mock_succeeds() {
        // The mock LSP reads one notification (null = no reply).
        let responses = vec![serde_json::Value::Null];

        let mut child = spawn_mock_lsp(&responses);
        let session = mock_lsp_session(&mut child);

        let conn = test_db();
        let ctx = LayeredContext::new(&conn, Some(session));
        ctx.lsp_notify("textDocument/didOpen", serde_json::json!({}))
            .unwrap();
    }

    #[test]
    fn test_lsp_notify_returns_ok_when_inner_client_is_none() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, Some(none_session()));
        ctx.lsp_notify("textDocument/didOpen", serde_json::json!({}))
            .unwrap();
    }

    #[test]
    fn test_lsp_request_with_document_returns_response() {
        // Protocol: didOpen (notification) -> hover (request+response) -> didClose (notification)
        let hover_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "contents": "fn main()" }
        });
        let responses = vec![
            serde_json::Value::Null, // didOpen notification
            hover_response,          // hover request
            serde_json::Value::Null, // didClose notification
        ];

        let mut child = spawn_mock_lsp(&responses);
        let session = mock_lsp_session(&mut child);

        let temp_dir = create_temp_source_file();
        let file_path = temp_dir.path().join("test.rs");

        let conn = test_db();
        let ctx = LayeredContext::new(&conn, Some(session));
        let result = ctx
            .lsp_request_with_document(
                file_path.to_str().unwrap(),
                "textDocument/hover",
                serde_json::json!({}),
            )
            .unwrap();

        assert!(result.is_some());
        let value = result.unwrap();
        assert_eq!(value["contents"], "fn main()");
    }

    #[test]
    fn test_lsp_request_with_document_returns_none_when_no_client() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);
        let result = ctx
            .lsp_request_with_document("src/main.rs", "textDocument/hover", serde_json::json!({}))
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_lsp_request_with_document_returns_none_when_inner_client_is_none() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, Some(none_session()));
        let result = ctx
            .lsp_request_with_document("src/main.rs", "textDocument/hover", serde_json::json!({}))
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_lsp_multi_request_with_document_calls_closure() {
        // Protocol: didOpen (notification) -> request (request+response) -> didClose (notification)
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": [1, 2, 3]
        });
        let responses = vec![
            serde_json::Value::Null, // didOpen notification
            response,                // request from closure
            serde_json::Value::Null, // didClose notification
        ];

        let mut child = spawn_mock_lsp(&responses);
        let session = mock_lsp_session(&mut child);

        let temp_dir = create_temp_source_file();
        let file_path = temp_dir.path().join("test.rs");

        let conn = test_db();
        let ctx = LayeredContext::new(&conn, Some(session));
        let result = ctx
            .lsp_multi_request_with_document(file_path.to_str().unwrap(), |rpc| {
                let resp = rpc.send_request("textDocument/references", serde_json::json!({}))?;
                unwrap_lsp_result(resp)
            })
            .unwrap();

        assert!(result.is_some());
        let value = result.unwrap();
        assert_eq!(value, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn test_lsp_multi_request_with_document_returns_none_when_no_client() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);
        let result: Result<Option<Value>, _> =
            ctx.lsp_multi_request_with_document("src/main.rs", |_rpc| {
                panic!("closure should not be called when no client");
            });
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_lsp_multi_request_with_document_returns_none_when_inner_client_is_none() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, Some(none_session()));
        let result: Result<Option<Value>, _> =
            ctx.lsp_multi_request_with_document("src/main.rs", |_rpc| {
                panic!("closure should not be called when inner client is None");
            });
        assert!(result.unwrap().is_none());
    }
}
