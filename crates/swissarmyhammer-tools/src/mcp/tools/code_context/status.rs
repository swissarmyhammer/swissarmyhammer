//! The `code_context` handlers that report on, rebuild, or wipe the index.
//!
//! `get status` and `lsp status` read the index and the LSP supervisor;
//! `rebuild index` drives the real indexer through
//! [`index_discovered_files_async`](super::indexing::index_discovered_files_async);
//! `clear status` wipes the stored index. None of them gates on tree-sitter
//! readiness — reporting on a half-built index is the point.

use crate::mcp::op_tool_helpers::json_result;
use crate::mcp::tool_registry::ToolContext;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_code_context::BuildLayer;

use super::doctor;
use super::indexing::index_discovered_files_async;
use super::support::{context_err, open_workspace, LSP_SUPERVISOR};

/// Execute the "get status" operation.
///
/// Returns a health report with file counts, indexing progress, and chunk/edge counts.
/// Also includes LSP server availability from doctor check.
pub(super) fn execute_get_status(context: &ToolContext) -> Result<CallToolResult, McpError> {
    let ws = open_workspace(context)?;
    let workspace_root = ws.workspace_root().to_path_buf();

    // Run doctor check to report on LSP availability
    let doctor_report = doctor::run_doctor(&workspace_root);
    tracing::debug!("Doctor report: {:?}", doctor_report);

    // Log LSP availability for debugging
    for lsp in &doctor_report.lsp_servers {
        if lsp.installed {
            tracing::debug!("LSP available: {} at {:?}", lsp.name, lsp.path);
        } else {
            tracing::debug!("LSP NOT available: {}", lsp.name);
        }
    }

    let status = swissarmyhammer_code_context::get_status(&ws.db()).map_err(context_err)?;

    // Merge LSP daemon status into the response
    let mut result = serde_json::to_value(&status).unwrap_or_default();
    if let Some(sup) = LSP_SUPERVISOR.get() {
        if let Ok(guard) = sup.try_lock() {
            let daemon_status = guard.status();
            if let Ok(daemon_json) = serde_json::to_value(&daemon_status) {
                result["lsp_daemons"] = daemon_json;
            }
        }
    }

    // Surface doctor report: detected project types and LSP availability
    if let Ok(v) = serde_json::to_value(&doctor_report.project_types) {
        result["project_types"] = v;
    }
    if let Ok(v) = serde_json::to_value(&doctor_report.lsp_servers) {
        result["lsp_availability"] = v;
    }

    json_result(&result)
}

/// Execute the "rebuild index" operation.
///
/// Resets the indexed flag for the specified layer and then drives the
/// synchronous tree-sitter indexer over the resulting dirty set. Returns
/// real run stats (`files_indexed`, `chunks_written`, `elapsed_ms`) rather
/// than just the marking count, so the MCP caller knows the rebuild
/// actually completed by the time the response lands.
///
/// ## Scope of the synchronous contract
///
/// Only the tree-sitter layer is driven to completion here. The LSP
/// indexer is a long-running background worker owned by the leader, and
/// this op does not await it — flipping `lsp_indexed=0` queues files for
/// that worker, but `rebuild index` returns once tree-sitter is done.
///
/// As a result:
/// - `layer=treesitter` — `files_indexed` / `chunks_written` describe
///   the full rebuild; the dirty set the marker produced (`WHERE
///   ts_indexed = 0`) is exactly the set the synchronous indexer drains.
/// - `layer=both` — same counters, same scope; the LSP rows are also
///   marked dirty for the background worker but those are not in the
///   tree-sitter dirty set and the counters don't account for them. The
///   `note` field on the response surfaces this caveat.
/// - `layer=lsp` — only `lsp_indexed=0` is flipped. The synchronous
///   indexer below queries `WHERE ts_indexed = 0` and finds nothing, so
///   the response always reports `files_indexed=0, chunks_written=0,
///   elapsed_ms~=0`. The dirty bits still take effect for the
///   background LSP worker; callers monitor progress via `get status`'s
///   `lsp_indexed_percent`. The `note` field on the response documents
///   this so callers aren't misled by the zero counters.
pub(super) async fn execute_rebuild_index(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let layer = match args.get("layer").and_then(|v| v.as_str()) {
        Some("treesitter") => BuildLayer::TreeSitter,
        Some("lsp") => BuildLayer::Lsp,
        Some("both") | None => BuildLayer::Both,
        Some(other) => {
            return Err(McpError::invalid_params(
                format!(
                    "invalid layer '{}'. Valid values: 'treesitter', 'lsp', 'both'",
                    other
                ),
                None,
            ))
        }
    };

    let ws = open_workspace(context)?;
    let workspace_root = ws.workspace_root().to_path_buf();

    // 1. Mark files dirty. Use write_db() so followers get a typed
    //    ReadOnlyFollower error mapped to invalid_request rather than an
    //    opaque SQLite "readonly database" failure after the UPDATE runs.
    //    The DbRef guard is dropped at the end of this block so the
    //    indexer can acquire the shared mutex on its own (it locks
    //    per-file, not for the whole run).
    let mut result = {
        let db = ws.write_db().map_err(context_err)?;
        swissarmyhammer_code_context::rebuild_index(&db, layer).map_err(context_err)?
    };

    // 2. Drive the indexer synchronously over the dirty set we just
    //    produced. `write_db()` already proved we're the leader, so
    //    `shared_db()` returning `None` would mean the workspace mode
    //    changed under us — surface that as an internal error rather
    //    than panicking.
    //
    //    The bootstrap pass and the file watcher run their own indexer
    //    invocations in the background. They use the same single shared
    //    `Mutex<Connection>` as we do, so concurrent indexer runs are
    //    serialised at the per-DB-call granularity rather than
    //    contending for distinct connections. The worst case is that
    //    a concurrent run sees an empty dirty set and exits in milliseconds.
    //    Adding a coarser advisory lock would just trade that benign
    //    waste for additional state to maintain.
    //
    //    Stats note: `files_indexed` and `chunks_written` count what this
    //    specific run produced, not net-new rows across the workspace.
    //    Concurrent rebuild/bootstrap/watcher runs each report their own
    //    non-zero counters for the same logical dirty set — that is the
    //    price of the lock-free design.
    //
    //    When the client supplied a `progressToken` in the request
    //    `_meta` (plumbed through `ToolContext::progress_token`) and the
    //    MCP peer is available, we build an `McpProgressReporter` that
    //    forwards `IndexProgress` events as `notifications/progress`
    //    messages. A dedicated drain task consumes the synchronous
    //    reporter channel and ships notifications through the peer.
    //    Dropping the reporter at the end of indexing closes the
    //    channel, the drain task exits, and we await its
    //    `JoinHandle` so any buffered terminal `Done` event is flushed
    //    before this op returns its `CallToolResult`. Absent a token or
    //    peer we fall back to the no-op reporter so the tool stays
    //    silent — progress is strictly opt-in by the client.
    let shared_db = ws.shared_db().ok_or_else(|| {
        McpError::internal_error(
            "workspace lost leader status before rebuild could run",
            None,
        )
    })?;
    // Pick a progress reporter based on what the caller wired up.
    //
    // Three cases, in priority order:
    //
    // 1. `progress_token` + `progress_sink` (in-process caller, e.g. CLI):
    //    build the standard `McpProgressReporter` and forward each
    //    notification to the caller-provided `UnboundedSender` instead of
    //    going through a peer. The sink takes priority over `peer` because
    //    it is the explicit in-process opt-in — when both are set we honor
    //    the more specific request.
    //
    // 2. `progress_token` + `peer` (MCP client over stdio/HTTP): build the
    //    `McpProgressReporter` and ship notifications via
    //    `peer.send_notification`. This is the original wiring.
    //
    // 3. Neither / token without a transport: fall back to the noop
    //    reporter. Progress is opt-in by the client; a token without any
    //    transport is a misconfiguration but progress is advisory so we
    //    log a warning and proceed silently.
    let (reporter, drain_handle): (
        std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter>,
        Option<tokio::task::JoinHandle<()>>,
    ) = match (
        context.progress_token.clone(),
        context.progress_sink.clone(),
        context.peer.clone(),
    ) {
        (Some(token), Some(sink), _) => {
            tracing::debug!(
                ?token,
                "rebuild_index: wiring McpProgressReporter to in-process progress sink"
            );
            let crate::mcp::progress::McpProgressReporterBuild { reporter, receiver } =
                crate::mcp::progress::McpProgressReporter::build(token);
            let handle = crate::mcp::progress::spawn_in_process_drain_task(sink, receiver);
            (std::sync::Arc::new(reporter), Some(handle))
        }
        (Some(token), None, Some(peer)) => {
            tracing::debug!(
                ?token,
                "rebuild_index: wiring McpProgressReporter for client-supplied progressToken"
            );
            let crate::mcp::progress::McpProgressReporterBuild { reporter, receiver } =
                crate::mcp::progress::McpProgressReporter::build(token);
            let handle = crate::mcp::progress::spawn_drain_task(peer, receiver);
            (std::sync::Arc::new(reporter), Some(handle))
        }
        (None, _, _) => {
            tracing::debug!(
                "rebuild_index: no progressToken in request _meta — using noop reporter"
            );
            (swissarmyhammer_code_context::noop_reporter(), None)
        }
        (Some(_), None, None) => {
            tracing::warn!(
                "rebuild_index: progressToken present but no MCP peer or progress sink — using noop reporter"
            );
            (swissarmyhammer_code_context::noop_reporter(), None)
        }
    };
    let stats = index_discovered_files_async(
        &workspace_root,
        shared_db,
        std::sync::Arc::clone(&reporter),
        ws.leader_shutdown_flag()
            .unwrap_or_else(swissarmyhammer_code_context::new_shutdown_flag),
    )
    .await;

    // Drop the reporter so the mpsc channel closes; then await the
    // drain task so any buffered notifications (notably the terminal
    // `Done` event) are flushed before we return to the client.
    //
    // A `JoinError` from the drain task means the task panicked or was
    // cancelled (e.g. a hypothetical future rmcp version that panics on
    // a closed peer). Progress is advisory so we still return the tool's
    // result, but log at warn level so the panic isn't silently lost —
    // the drain task itself logs send errors at debug, so a join failure
    // deserves at least the same surfacing.
    drop(reporter);
    if let Some(handle) = drain_handle {
        if let Err(err) = handle.await {
            tracing::warn!(
                error = ?err,
                "rebuild_index: progress drain task did not join cleanly"
            );
        }
    }

    result.files_indexed = stats.files;
    result.chunks_written = stats.chunks;
    result.elapsed_ms = stats.elapsed.as_millis() as u64;

    json_result(&result)
}

/// Execute the "clear status" operation.
///
/// Wipes all index data from all tables and returns stats about what was cleared.
pub(super) fn execute_clear_status(context: &ToolContext) -> Result<CallToolResult, McpError> {
    let ws = open_workspace(context)?;
    // Followers get a typed ReadOnlyFollower error here rather than an opaque
    // SQLite failure once the DELETE runs.
    let db = ws.write_db().map_err(context_err)?;
    let result = swissarmyhammer_code_context::clear_status(&db).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "lsp status" operation.
///
/// Queries indexed file extensions, cross-references with the LSP registry,
/// and returns which languages are present, which LSPs are installed or missing,
/// and install hints.
pub(super) fn execute_lsp_status(context: &ToolContext) -> Result<CallToolResult, McpError> {
    let ws = open_workspace(context)?;
    let conn = ws.db();

    // Get distinct file extensions from the index
    let exts = swissarmyhammer_code_context::distinct_extensions(&conn).map_err(context_err)?;

    // Convert to &str slice for the registry lookup
    let ext_refs: Vec<&str> = exts.iter().map(|s| s.as_str()).collect();
    let matching_servers = swissarmyhammer_lsp::servers_for_extensions(&ext_refs);

    // Build the response
    let mut languages = Vec::new();
    for spec in &matching_servers {
        // Check which of this server's extensions are present in the index
        let present_exts: Vec<&str> = spec
            .file_extensions
            .iter()
            .filter(|e| exts.contains(e.as_str()))
            .map(|e| e.as_str())
            .collect();

        let installed = swissarmyhammer_code_context::find_executable(&spec.command).is_some();

        languages.push(serde_json::json!({
            "icon": spec.icon,
            "extensions": present_exts,
            "lsp_server": spec.command,
            "installed": installed,
            "install_hint": if installed { None } else { Some(&spec.install_hint) },
        }));
    }

    let all_healthy = languages
        .iter()
        .all(|l| l["installed"].as_bool().unwrap_or(false));

    let result = serde_json::json!({
        "languages": languages,
        "all_healthy": all_healthy,
    });

    json_result(&result)
}
