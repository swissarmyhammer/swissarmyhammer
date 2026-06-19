//! Follower → leader routing for the live code-context LSP ops.
//!
//! A follower process spawns no LSP supervisor of its own (the elected leader
//! owns the single one, per the leader-gated spawn), so it has no in-process
//! [`SharedLspSession`](swissarmyhammer_code_context::SharedLspSession). Without
//! routing, every live op (`get definition`, `get hover`, `get references`, …)
//! sees `session = None`, short-circuits its live-LSP layer, and silently
//! degrades to the tree-sitter / persisted-index layers — never crossing the
//! socket to the leader's live rust-analyzer.
//!
//! This module supplies the missing path. It builds a
//! [`LiveLspRouter`](swissarmyhammer_code_context::LiveLspRouter) — the
//! dependency-inversion seam `swissarmyhammer-code-context` exposes — backed by
//! the SAME [`SessionRequestClient`] / `lsp_request` multiplexer the
//! `diagnostics` tool already uses (`diagnose_via_leader`). The router lets a
//! follower's layered ops take their live-LSP branch and parse/enrich the
//! leader's real answer, with no second transport/client/envelope and no crate
//! cycle: the routing lives at the tool layer (which depends on both
//! `code-context` and `diagnostics`), never inside `code-context`.
//!
//! ## Single-request ops only
//!
//! The router serves the *single-request* live ops (definition, type-definition,
//! hover, references, implementations, workspace-symbol): each bottoms out in one
//! `session.request(method, params)` the leader can multiplex. The multi-step
//! call-hierarchy / code-action / rename ops hold the client lock across several
//! requests (`prepareCallHierarchy` then `incomingCalls`, etc.) and cannot be
//! reproduced by a single round-trip; on a follower they fall back to their
//! documented index / tree-sitter best-effort, which is unchanged here.

use rusqlite::Connection;
use serde_json::Value;

use swissarmyhammer_code_context::{
    CodeContextError, LayeredContext, LiveLspRouter, SharedLspSession,
};
use swissarmyhammer_diagnostics::{IpcError, SessionRequestClient};

use super::open_workspace;
use crate::mcp::tool_registry::ToolContext;

/// Resolve the follower leader-route for a live code-context op, *before* the
/// workspace DB handle is opened.
///
/// Returns `Some(router)` only when this process owns no in-process `session`
/// (it is a follower) *and* a [`SessionRequestClient`] connects to the elected
/// leader; otherwise `None` (leader/in-process session present, or no leader
/// reachable → the op degrades to the index / tree-sitter layers as before).
///
/// This is split from [`build_layered_context`] because connecting the client is
/// `async` while the workspace DB handle (`rusqlite::Connection`) is `!Send` and
/// must not be held across an `.await` — exactly the constraint the diagnose
/// leader-route observes. The caller resolves the router first (no DB held),
/// then opens the DB and builds the context synchronously.
pub(crate) async fn follower_route_for_op(
    session: &Option<SharedLspSession>,
    context: &ToolContext,
) -> Option<LiveLspRouter> {
    if session.is_some() {
        return None;
    }
    build_follower_router(context).await
}

/// Build the [`LayeredContext`] for a live code-context op from the (possibly
/// `None`) in-process session and the (possibly `None`) follower router resolved
/// by [`follower_route_for_op`].
///
/// - **In-process session present** → a normal session-backed context.
/// - **Follower with a router** → the live-LSP branch routes to the leader.
/// - **Neither** → a session-less context that degrades to the index /
///   tree-sitter layers.
///
/// Synchronous: it holds no `.await`, so the `!Send` DB handle is safe.
pub(crate) fn build_layered_context<'a>(
    db: &'a Connection,
    session: Option<SharedLspSession>,
    router: Option<LiveLspRouter>,
) -> LayeredContext<'a> {
    if session.is_some() {
        return LayeredContext::new(db, session);
    }
    match router {
        Some(router) => LayeredContext::with_live_lsp_router(db, router),
        None => LayeredContext::new(db, None),
    }
}

/// Map an [`IpcError`] from the leader round-trip into the layered ops'
/// [`CodeContextError`] degradation contract.
///
/// Every IPC failure is a *genuine* failure surfaced to the caller as an
/// [`CodeContextError::LspError`] — never a silent `Ok(None)` that would let the
/// op return a wrong-empty result when the leader could have answered. The
/// typed [`IpcError::NotLeader`] keeps its leader-PID attribution in the message.
fn ipc_err_to_code_context(err: IpcError) -> CodeContextError {
    CodeContextError::LspError(format!("leader LSP request failed: {err}"))
}

/// Whether the current tokio runtime supports [`tokio::task::block_in_place`].
///
/// `block_in_place` panics on a current-thread runtime; the follower router uses
/// it to bridge the synchronous layered-op seam to the async IPC client, so the
/// router is only built when this returns `true` (the production MCP server runs
/// on the multi-thread runtime). On a current-thread runtime the follower
/// degrades to the index / tree-sitter layers instead.
fn current_runtime_supports_block_in_place() -> bool {
    tokio::runtime::Handle::current().runtime_flavor()
        != tokio::runtime::RuntimeFlavor::CurrentThread
}

/// Round-trip one live LSP request to the leader over `client`.
///
/// `file_path` is the document scope: when non-empty the request is sent via
/// [`SessionRequestClient::lsp_request_with_document`] so the leader syncs the
/// document onto its session before the request (mirroring the local
/// `lsp_request_with_document` open-then-request contract); when empty (a
/// workspace-wide op such as `workspace/symbol`) it is sent via the plain
/// [`SessionRequestClient::lsp_request`].
///
/// Returns `Ok(Some(json))` with the leader's raw LSP result (which the op's
/// existing parser then turns into the typed result), or an error mapped through
/// [`ipc_err_to_code_context`].
async fn route_one(
    client: &SessionRequestClient,
    file_path: &str,
    method: &str,
    params: Value,
) -> Result<Option<Value>, CodeContextError> {
    let result = if file_path.is_empty() {
        client.lsp_request(method, params).await
    } else {
        client
            .lsp_request_with_document(file_path, method, params)
            .await
    };
    result.map(Some).map_err(ipc_err_to_code_context)
}

/// Build a [`LiveLspRouter`] for a follower, connecting a [`SessionRequestClient`]
/// to the elected leader's request socket.
///
/// Returns `None` — meaning the op proceeds with no live layer and degrades to
/// its persisted-index / tree-sitter layers — in two cases:
///
/// - The workspace cannot be opened to discover the socket/lock paths.
/// - No leader is bound right now (connect fails). For a follower **read** op
///   this index/tree-sitter fallback is the documented best-effort behavior: the
///   bug this routing fixes is silently returning empty when a leader *was*
///   reachable, not the absence of a leader. The connect failure is logged at
///   debug.
///
/// When a leader *is* reachable the router is returned; if the leader then errors
/// mid-request (serve failure, dead session), that genuine failure surfaces as a
/// typed [`CodeContextError`] via [`route_one`] / [`ipc_err_to_code_context`] —
/// never a silent wrong-empty.
///
/// The returned router bridges the synchronous layered-op seam to the async IPC
/// client: each call runs the round-trip on the current tokio runtime via
/// [`tokio::task::block_in_place`] + [`tokio::runtime::Handle::block_on`]. That
/// pair requires a **multi-thread** runtime ([`tokio::task::block_in_place`]
/// panics on a current-thread runtime). The MCP server runs on the multi-thread
/// runtime, so the layered op (driven from an async tool handler) can block the
/// calling worker for the one round-trip without starving the runtime. If this is
/// ever called from a current-thread runtime we return `None` (degrade to the
/// index / tree-sitter layers) rather than hand back a router that would panic on
/// first use.
pub(crate) async fn build_follower_router(context: &ToolContext) -> Option<LiveLspRouter> {
    // block_in_place panics on a current-thread runtime. Degrade rather than
    // build a router that would panic on its first request.
    if !current_runtime_supports_block_in_place() {
        tracing::debug!(
            "follower leader-route disabled on a current-thread runtime; degrading to index layers"
        );
        return None;
    }

    let workspace = open_workspace(context).ok()?;
    let socket_path = workspace.socket_path().to_path_buf();
    let lock_path = workspace.lock_path().to_path_buf();

    let client = match SessionRequestClient::connect(&socket_path, &lock_path).await {
        Ok(client) => client,
        // No leader bound right now → no live layer; the read op falls back to
        // its index / tree-sitter layers (documented best-effort).
        Err(err) => {
            tracing::debug!(error = %err, "follower could not connect to the diagnostics leader");
            return None;
        }
    };

    let handle = tokio::runtime::Handle::current();
    Some(Box::new(
        move |file_path: &str, method: &str, params: Value| {
            let client = client.clone();
            let file_path = file_path.to_string();
            let method = method.to_string();
            tokio::task::block_in_place(|| {
                handle.block_on(route_one(&client, &file_path, &method, params))
            })
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_error_maps_to_lsp_error_not_a_silent_empty() {
        // A NotLeader IPC failure must surface as a CodeContextError carrying the
        // leader PID, never an Ok(None) wrong-empty.
        let err = ipc_err_to_code_context(IpcError::NotLeader {
            leader_pid: Some(4321),
            source: std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused"),
        });
        let rendered = format!("{err}");
        assert!(
            rendered.contains("4321"),
            "must attribute leader pid: {rendered}"
        );
        assert!(rendered.contains("leader LSP request failed"), "{rendered}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn block_in_place_guard_false_on_current_thread_runtime() {
        // On a current-thread runtime block_in_place would panic, so the router
        // must not be built — the follower degrades to its index layers instead.
        assert!(!current_runtime_supports_block_in_place());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn block_in_place_guard_true_on_multi_thread_runtime() {
        // The production MCP server runtime: block_in_place is supported, so the
        // router can be built.
        assert!(current_runtime_supports_block_in_place());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn route_one_surfaces_typed_not_leader_on_connect_failure() {
        // Connecting to an unbound socket fails with the typed not-leader error;
        // route_one (and thus the router) must propagate it as a CodeContextError,
        // never swallow it.
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("missing.sock");
        let lock_path = dir.path().join("leader.lock");
        std::fs::write(&lock_path, "7788\n").unwrap();

        // No server is bound, so connect must fail typed.
        let connect = SessionRequestClient::connect(&socket_path, &lock_path).await;
        let err = connect.expect_err("connect to unbound socket must fail");
        let mapped = ipc_err_to_code_context(err);
        assert!(
            format!("{mapped}").contains("7788"),
            "the typed not-leader error must carry the leader pid"
        );
    }
}
